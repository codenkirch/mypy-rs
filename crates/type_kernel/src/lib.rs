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

#![allow(non_local_definitions)]

mod aliases;
mod applytype;
mod argapprox;
mod argmap;
mod astwire;
mod attrs;
mod binder;
mod builtin_item;
mod cache;
mod callable_compat;
mod checkcall;
mod checker_functions;
mod checker_helpers;
mod checker_stmts;
mod checker_visitor;
mod checkexpr_argcheck;
mod checkexpr_argcount;
mod checkexpr_argtypes;
mod checkexpr_functions;
mod checkexpr_overload;
mod checkmember;
mod checkoperator;
mod checkpattern;
mod checkstrformat;
mod classmethod_static;
mod comparison_group;
mod cond_types;
mod condmaps;
mod constant_fold;
mod constraints;
mod constraints_filter;
mod constraints_helpers;
mod constraints_select;
mod copymodified;
mod covers_at_runtime;
mod dangerous_comparison;
mod dataclasses;
mod detach_callable;
mod equality_ambiguity;
mod equality_info;
mod erase;
mod erase_typevars;
mod errors;
mod errors_helpers;
mod expand;
mod expand_variants;
mod expandtype;
mod fixup;
mod freshen;
mod generators;
mod infer_variance;
mod joinfns;
mod lennarrow;
mod lkv;
mod maptype;
mod meet;
mod member_flags;
mod message_registry;
mod messages;
mod messages_find_overlaps;
mod modulefinder;
mod mro;
mod operators;
mod overlap_unsafe;
mod overload;
mod overload_never;
mod overload_override;
mod protocols;

mod partially_defined;
mod plugin_helpers;
mod plugin_hooks;
mod reachability;
mod refs;
mod remove_redundant;
mod semanal_algebra;
mod semanal_bases;
mod semanal_checks;
mod semanal_classprop;
mod semanal_lookup;
mod semanal_metaclass;
mod semanal_shared;
mod semanal_typeddict;
mod semanal_typeexpr;
mod semanal_visitor;
mod serverdeps;
mod setops;
mod solve;
mod stubgen;
mod subtypes;
mod suggestions;
mod supported_self_type;
mod traverser;
mod treetransform;
mod typealias_instantiate;
mod typeanal_callable;
mod typeanal_deprec;
mod typeanal_info;
mod typeanal_literal;
mod typeanal_queries;
mod typeanal_rawexpr;
mod typeanal_special;
mod typeanal_unbound;
mod typeanal_unbound2;
mod typeinfo;
mod typeops;
mod types_impl;
mod util;
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
    module.add_function(wrap_pyfunction!(
        constant_fold::rust_constant_fold_expr,
        module
    )?)?;
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
    module.add_function(wrap_pyfunction!(subtypes::rust_is_subtype_batch, module)?)?;
    module.add_function(wrap_pyfunction!(
        subtypes::rust_subtype_tvar_tuple_right,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        subtypes::rust_variadic_tuple_subtype,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_all_same_types, module)?)?;
    // Issue #465: pure-computation helpers from subtypes.py.
    module.add_function(wrap_pyfunction!(
        subtypes::rust_has_underscore_prefix,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_erased_instance, module)?)?;
    module.add_function(wrap_pyfunction!(
        subtypes::rust_erase_return_self_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        member_flags::rust_get_member_flags,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        protocols::rust_is_protocol_implementation,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        subtypes::rust_try_restrict_literal_union,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_more_precise, module)?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_equivalent, module)?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_same_type, module)?)?;
    module.add_function(wrap_pyfunction!(subtypes::rust_is_descriptor, module)?)?;
    module.add_function(wrap_pyfunction!(
        callable_compat::rust_callables_compatible,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        callable_compat::rust_are_parameters_compatible,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        subtypes::rust_are_args_compatible,
        module
    )?)?;
    // Issue #998: check_type_parameter variance-dispatch classifier head.
    module.add_function(wrap_pyfunction!(
        subtypes::rust_classify_type_parameter,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(meet::rust_is_overlapping_types, module)?)?;
    module.add_function(wrap_pyfunction!(meet::rust_narrow_declared_type, module)?)?;
    // Issue #526: get_possible_variants alongside narrow_declared_type.
    module.add_function(wrap_pyfunction!(meet::rust_get_possible_variants, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_trivial_join, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_trivial_meet, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_join_types, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_is_better, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_join_instances, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_meet_types, module)?)?;
    // Issue #494: variadic-tuple join/meet cores.
    module.add_function(wrap_pyfunction!(setops::rust_join_tuples, module)?)?;
    module.add_function(wrap_pyfunction!(setops::rust_meet_tuples, module)?)?;
    module.add_function(wrap_pyfunction!(
        joinfns::rust_object_or_any_from_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        joinfns::rust_object_from_instance,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        joinfns::rust_combine_similar_callables,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        joinfns::rust_object_from_instance,
        module
    )?)?;
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
        freshen::rust_freshen_function_type_vars,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        freshen::rust_match_generic_callables,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(expandtype::rust_remove_trivial, module)?)?;
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
        maptype::rust_map_instance_to_supertypes,
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
    module.add_function(wrap_pyfunction!(typeops::rust_bind_self, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_fill_typevars, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_fill_typevars_with_any,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_class_callable, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_function_type, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_callable_type, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_type_object_type_from_function,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_map_type_from_supertype,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_coerce_to_literal, module)?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_is_singleton_identity_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_is_singleton_equality_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_is_valid_constructor,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeops::rust_classify_type_object_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_is_disjoint_base, module)?)?;
    module.add_function(wrap_pyfunction!(typeops::rust_is_recursive_pair, module)?)?;
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
        checkexpr_argtypes::rust_check_argument_types_plan,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        applytype::rust_apply_generic_arguments,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(applytype::rust_has_no_typevars, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_has_abstract_type,
        module
    )?)?;
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
        checkexpr_functions::rust_has_erased_component,
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
        checkexpr_functions::rust_is_valid_var_arg,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_valid_keyword_var_arg,
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
    // Issue #980: RefExpr -> TypedDict target predicate; unregistered on
    // main, which disabled the whole checkexpr kernel block on import.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_refers_to_typeddict,
        module
    )?)?;
    // Issue #486: tuple-index / tuple-slice helpers.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_try_getting_int_literals,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_visit_tuple_index_helper,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_visit_tuple_slice_helper,
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
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_is_enum_callable_base,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_classify_protocol_test_callee,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_classify_typeddict_call,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_refers_to_typeddict,
        module
    )?)?;
    // Issue #1007: attribute_triggers port (member triggers for attribute access)
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_attribute_triggers,
        module
    )?)?;
    // check_unpacks_in_list: filters non-tuple Unpack items from a type-arg
    // list and reports the final unpack index. Python applies the fail and
    // rebuilds the item list from the kept indices. None defers.
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_check_unpacks_in_list,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_classify_reveal_imported,
        module
    )?)?;
    // Issue #956: _super_arg_types stage-1 dispatch. Rust classifies the
    // arity + scope gate into a branch tag; the fail / fill_typevars /
    // accept side effects and stage 2 stay in Python.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_classify_super_arg_types,
        module
    )?)?;
    // Issue #1064: infer_arg_types_in_context index decision. Rust returns
    // the formal-index-per-actual map (star args skipped); the accept
    // recursion and the infer_unions toggle stay in Python.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_compute_arg_context_indices,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_classify_visit_op_expr,
        module
    )?)?;
    // Issue #1048: check_arg decision head. Rust classifies the 4-way
    // dispatch (DeletedType / abstract-only / incompatible / pass) from
    // the wire caller type; message emission stays in Python.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_classify_check_arg,
        module
    )?)?;
    // Issue #1049: check_boolean_op decision head. Rust classifies the
    // unreachable-map branch and the result tail; find_isinstance_check,
    // analyze_cond_branch, the msg emissions, make_simplified_union stay in Python.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_classify_check_boolean_op,
        module
    )?)?;
    // Issue #999: visit_index_with_type dispatch head. Rust classifies the
    // left_type branch from PyO3 facts; the fail/note tails and branch
    // bodies (incl. the tuple sub-dispatch) stay in Python.
    module.add_function(wrap_pyfunction!(
        checkexpr_functions::rust_classify_index_with_type,
        module
    )?)?;
    // Issue #489: overload-result family (combine_function_signatures body).
    module.add_function(wrap_pyfunction!(
        checkexpr_overload::rust_combine_function_signatures,
        module
    )?)?;
    // Issue #489: merge_typevars_in_callables_by_name (checkexpr.py:8309-8351),
    // the freshen+rename step shared with combine_function_signatures.
    module.add_function(wrap_pyfunction!(
        checkexpr_overload::rust_merge_typevars_in_callables_by_name,
        module
    )?)?;
    // Issue #489: overload any-ambiguity detection (any_causes_overload_ambiguity).
    module.add_function(wrap_pyfunction!(
        checkexpr_overload::rust_any_causes_overload_ambiguity,
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
    module.add_function(wrap_pyfunction!(
        constraints_select::rust_any_constraints,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_select::rust_repack_callable_args,
        module
    )?)?;
    // Issue #1001: standalone constraint-list helper seams.
    module.add_function(wrap_pyfunction!(
        constraints_select::rust_merge_with_any,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_select::rust_filter_satisfiable,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        constraints_select::rust_is_same_constraints,
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
    // Issue #490: callable-arguments constraint inference.
    module.add_function(wrap_pyfunction!(
        constraints::rust_infer_callable_arguments_constraints,
        module
    )?)?;
    // Issue #475: Type.copy_modified field-swap seam.
    module.add_function(wrap_pyfunction!(copymodified::rust_copy_modified, module)?)?;
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
    // Issue #1000: two-pass argument-inference classifier. Rust decides
    // pass 1 vs pass 2 per actual; Python applies the results.
    module.add_function(wrap_pyfunction!(
        checkcall::rust_get_arg_infer_passes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        overload::rust_check_overload_call,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        overload::rust_find_matching_overload_items,
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
    // Issue #609 (Phase C2): except-handler-test classification.
    // Returns (tag, blob) pairs or None to defer to the pure-Python path.
    module.add_function(wrap_pyfunction!(
        checker_stmts::rust_classify_except_handler_tests,
        module
    )?)?;
    // Issue #493: len-based tuple narrowing. Returns (yes, no) type blobs
    // or None to defer to the pure-Python path.
    module.add_function(wrap_pyfunction!(lennarrow::rust_narrow_with_len, module)?)?;
    // Issue #1065: len-narrowing gate predicate. Returns the bool decision
    // or None to defer to the pure-Python path.
    module.add_function(wrap_pyfunction!(
        lennarrow::rust_can_be_narrowed_with_len,
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
    module.add_function(wrap_pyfunction!(checker_visitor::rust_is_method, module)?)?;
    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_is_empty_generator_function,
        module
    )?)?;
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
    module.add_function(wrap_pyfunction!(
        dangerous_comparison::rust_dangerous_comparison,
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
    // Issue #749: find_type_overlaps (messages.py:3055-3079).
    module.add_function(wrap_pyfunction!(
        messages_find_overlaps::rust_find_type_overlaps,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        messages::rust_append_numbers_notes,
        module
    )?)?;
    // Issue #982: make_inferred_type_note decision (messages.py:3770-3800).
    module.add_function(wrap_pyfunction!(
        messages::rust_make_inferred_type_note,
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
        messages::rust_classify_has_no_attr,
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
        equality_info::rust_equality_value_info,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        equality_ambiguity::rust_is_equality_ambiguous_for_narrowing,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        equality_ambiguity::rust_partition_equality_ambiguous_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        expand_variants::rust_expand_callable_variants,
        module
    )?)?;
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
        checkpattern::rust_classify_class_pattern_ranges,
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
        traverser::rust_has_await_in_generator,
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
    // Issue #541: remaining traverser seekers.
    module.add_function(wrap_pyfunction!(
        traverser::rust_count_return_statements_and_flags,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(traverser::rust_count_all_returns, module)?)?;
    module.add_function(wrap_pyfunction!(traverser::rust_has_yield_return, module)?)?;
    module.add_function(wrap_pyfunction!(traverser::rust_has_complex_slice, module)?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_count_non_extension_handlers,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(traverser::rust_is_global_expr, module)?)?;
    module.add_function(wrap_pyfunction!(
        traverser::rust_count_non_literal_handlers,
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
        semanal_visitor::rust_classify_simple_literal_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_defined_type_param,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_classify_setup_type_vars,
        module
    )?)?;
    // Issue #980 follow-up: Literal classification head; likewise missing
    // from the registration list, so semanal.py fell back on import.
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_classify_simple_literal_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_list_set_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_dict_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_template_str_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_unary_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_comparison_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_slice_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_conditional_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_super_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_raise_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_assert_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_operator_assignment_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(semanal_visitor::rust_visit_block, module)?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_if_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_is_valid_del_target,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_del_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_expression_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_break_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_continue_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_global_decl,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_match_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_return_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_block_maybe,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_while_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_name_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_star_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_as_pattern,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_or_pattern,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_value_pattern,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_sequence_pattern,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_starred_pattern,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_mapping_pattern,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_class_pattern,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_yield_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_yield_from_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_await_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_try_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_op_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_index_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_cast_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_type_form_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_assert_type_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_reveal_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_type_application,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_list_comprehension,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_set_comprehension,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_dictionary_comprehension,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_generator_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_lambda_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_overloaded_func_def,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_class_def,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_func_def,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_nonlocal_decl,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_for_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_with_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_assignment_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_import_all,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_import_from,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_assignment_stmt,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_import,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_call_expr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_visit_type_alias_stmt,
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
    module.add_function(wrap_pyfunction!(
        semanal_visitor::rust_classify_class_decorator,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_bases::rust_clean_up_bases,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(semanal_bases::rust_is_magic_base, module)?)?;
    module.add_function(wrap_pyfunction!(
        semanal_bases::rust_is_core_builtin_class,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_bases::rust_classify_with_metaclass,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_bases::rust_classify_add_metaclass,
        module
    )?)?;
    // semanal_bases: configure_base_classes per-base classifier + MRO tail.
    // Rust owns the wire classification and MRO tag; fails, fallback_to_any,
    // info.bases, configure_tuple_base_class, and the mro writes stay in Python.
    module.add_function(wrap_pyfunction!(
        semanal_bases::rust_classify_configure_bases,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_bases::rust_classify_configure_mro,
        module
    )?)?;
    // semanal_metaclass: get_declared_metaclass gate chain +
    // recalculate_metaclass decision heads (issue #1037). Rust owns the
    // tags; fails, fill_typevars, and metaclass writes stay in Python.
    module.add_function(wrap_pyfunction!(
        semanal_metaclass::rust_classify_declared_metaclass,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_metaclass::rust_classify_recalculate_metaclass,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_checks::rust_classify_function_signature,
        module
    )?)?;
    // semanal_checks: check_decorated_function_is_method predicate port.
    // Rust reads live analyzer state (self.type, is_func_scope()) and
    // returns the method/non-method decision; the self.fail stays in Python.
    module.add_function(wrap_pyfunction!(
        semanal_checks::rust_check_decorated_function_is_method,
        module
    )?)?;
    // semanal_checks: should_wait_rhs assignment-rvalue wait predicate.
    // Rust walks the rvalue node chain; lookups ride the real lookup
    // methods and the pure-Python body is the fallback on None.
    module.add_function(wrap_pyfunction!(
        semanal_checks::rust_should_wait_rhs,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_bases::rust_classify_lvalue_validity,
        module
    )?)?;
    // semanal_checks: check_fixed_args arg-count + arg-kinds arbitration.
    // Rust classifies the two gap checks into a tag; the self.fail
    // message emission stays in Python.
    module.add_function(wrap_pyfunction!(
        semanal_checks::rust_classify_fixed_args,
        module
    )?)?;
    // semanal_checks: prepare_method_signature method-signature dispatch
    // head. Rust classifies the branch from live FuncDef facts plus the
    // wire self type; writes, side effects, and fails stay in Python.
    module.add_function(wrap_pyfunction!(
        semanal_checks::rust_classify_method_signature,
        module
    )?)?;
    // semanal_checks: remove_unpack_kwargs unpack-kwargs arbitration.
    // Rust classifies the guard chain + overlap set; side effects in Python.
    module.add_function(wrap_pyfunction!(
        semanal_checks::rust_classify_remove_unpack_kwargs,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(semanal_visitor::rust_lookup, module)?)?;
    module.add_function(wrap_pyfunction!(
        semanal_lookup::rust_lookup_qualified,
        module
    )?)?;
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
        semanal_typeexpr::rust_classify_type_expression,
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
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_unknown_unpack,
        module
    )?)?;
    // Issue #852: resolver-backed variants. TypeAliasType expands through
    // the NativeTypeResolver alias snapshot instead of deferring; any
    // undecidable expansion still returns None -> Python fallback.
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_has_explicit_any_live,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_has_any_from_unimported_type_live,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_collect_all_inner_types_live,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_make_optional_type_live,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_unknown_unpack_live,
        module
    )?)?;
    // Issue #542: live-object query functions from typeanal.py.
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_find_self_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_validate_instance,
        module
    )?)?;
    // analyze_type_with_type_info: the decision front (tuple-with-args,
    // librt.vecs.vec, named-tuple/TypedDict tails, types.NoneType, plain
    // Instance). Rust returns a branch tag from raw node facts; Python

    // applies the side effects and builds the result objects for the two
    // tags it executes inline, the rest re-run the body. None defers.
    module.add_function(wrap_pyfunction!(
        typeanal_info::rust_classify_type_with_info,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_check_vec_type_args,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_is_typevar_default_recursive,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        typeanal_queries::rust_detect_diverging_alias,
        module
    )?)?;
    // instantiate_type_alias: normalize a TypeAlias node + type args.
    // Deferral-based seam: any path that would emit an error or call
    // set_any_tvars returns None and the Python shim falls back. The

    // no_args / max_tv_count==0 success paths (eager Instance) and the
    // plain TypeAliasType success path are returned as a branch tag +
    // argument wire blobs for the shim to rebuild live objects.
    module.add_function(wrap_pyfunction!(
        typealias_instantiate::rust_instantiate_type_alias,
        module
    )?)?;
    // analyze_unbound_type_without_type_info: the pure classification
    // front (Any-typed Var, allow_type_any special forms, unbound type
    // variable, enum member Literal). None defers to pure Python.
    module.add_function(wrap_pyfunction!(
        typeanal_unbound::rust_analyze_unbound_without_info,
        module
    )?)?;
    // visit_unbound_type_nonoptional: the decision front (placeholder /
    // node-None / ParamSpec / TypeVar / TypeVarTuple families). Rust
    // returns a branch tag from raw node facts; Python applies the side

    // effects and builds the result objects. None defers to pure Python.
    module.add_function(wrap_pyfunction!(
        typeanal_unbound2::rust_classify_unbound_front,
        module
    )?)?;
    // try_analyze_special_unbound_type: the special-form dispatch classifier
    // (builtins.None / Any / Final / Tuple / Union / Optional / Callable /
    // Type / TypeForm / ClassVar / Never / Annotated / Required /

    // NotRequired / ReadOnly). Rust returns a branch tag from scalar facts;
    // Python applies the side effects and builds the result objects. None
    // defers to pure Python for the branches needing recursive analysis.
    module.add_function(wrap_pyfunction!(
        typeanal_special::rust_classify_special_unbound,
        module
    )?)?;
    // analyze_literal_param: 9-way Literal-param dispatch head. Rust
    // returns a branch tag from scalar facts; Python applies the side
    // effects (LiteralType build, errors, recursion, union merge).
    module.add_function(wrap_pyfunction!(
        typeanal_literal::rust_classify_literal_param,
        module
    )?)?;
    // visit_raw_expression_type: 3-way message head (int/bool, float/
    // complex, else generic). Rust owns the set-membership branch and
    // returns a tag; Python formats the message. None defers to Python.
    module.add_function(wrap_pyfunction!(
        typeanal_rawexpr::rust_classify_raw_expression_type,
        module
    )?)?;
    // check_and_warn_deprecated: deprecation-warn arbitration head. Rust
    // decides silent/note/fail from scalar facts; Python emits the message
    // via the live info.deprecated string. Never defers.
    module.add_function(wrap_pyfunction!(
        typeanal_deprec::rust_classify_check_warn_deprecated,
        module
    )?)?;
    // visit_tuple_type: implicit-tuple message arbitration (OK / EMPTY /
    // SINGLE / MULTI). Rust owns the three-scalar branch; Python applies
    // the fail + one-of-three note and the reconstruction on OK.
    module.add_function(wrap_pyfunction!(
        typeanal_special::rust_classify_tuple_type_implicit,
        module
    )?)?;
    // analyze_callable_type: two-level dispatch (arity + arg0 kind). Rust
    // returns a branch tag from scalar facts; Python builds the live
    // CallableType / enters tvar_scope / emits fail/note.
    module.add_function(wrap_pyfunction!(
        typeanal_callable::rust_classify_analyze_callable_type,
        module
    )?)?;
    // anal_type_guard_arg / anal_type_is_arg (issue #1043): TypeGuard/
    // TypeIs argument-family + arity classifier. Rust decides from scalar
    // facts; Python applies the fail + Any or the anal_type recursion.
    module.add_function(wrap_pyfunction!(
        typeanal_special::rust_classify_type_guard_arg,
        module
    )?)?;
    // visit_tuple_type: implicit-tuple message arbitration (OK / EMPTY /
    // SINGLE / MULTI). Rust owns the three-scalar branch; Python applies
    // the fail + one-of-three note and the reconstruction on OK.
    module.add_function(wrap_pyfunction!(
        typeanal_special::rust_classify_tuple_type_implicit,
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
        checkmember::rust_analyze_instance_member_dispatch,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_analyze_member_method,
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
    module.add_function(wrap_pyfunction!(checkmember::rust_check_self_arg, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_expand_without_binding,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_expand_and_bind_callable,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checkmember::rust_add_class_tvars, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_descriptor_has_get_set,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(checkmember::rust_is_instance_var, module)?)?;
    module.add_function(wrap_pyfunction!(
        checkmember::rust_classify_type_type_member_access,
        module
    )?)?;
    // Issue #1056: analyze_var decision head. Rust classifies the
    // dispatch from live Var scalars; the shim applies side effects.
    module.add_function(wrap_pyfunction!(
        checkmember::rust_classify_analyze_var,
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
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_find_relative_leaf_module,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_find_unloaded_deps,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(serverdeps::rust_target_from_node, module)?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_merge_dependencies,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_non_trivial_bases,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(serverdeps::rust_has_user_bases, module)?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_compare_symbol_table_snapshots,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_is_expr_literal_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        serverdeps::rust_get_partial_instance_type,
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
    // Issue #854: resolver-enabled truthiness defaults (can_be_any_bool,
    // alias-target delegation, enum literals).
    types_impl::extension_seams::add_seams(module)?;
    // Issue #487: CallableType/Parameters arg-query helpers.
    module.add_function(wrap_pyfunction!(
        types_impl::rust_callable_formal_arguments,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_impl::rust_callable_argument_by_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        types_impl::rust_callable_argument_by_position,
        module
    )?)?;
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
    // Issue #488: conditional type-map algebra.
    module.add_function(wrap_pyfunction!(
        condmaps::rust_and_conditional_maps,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        condmaps::rust_or_conditional_maps,
        module
    )?)?;
    // Issue #538: semanal_classprop.py class-property calculators.
    module.add_function(wrap_pyfunction!(
        semanal_classprop::rust_calculate_class_abstract_status,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_classprop::rust_check_protocol_status,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_classprop::rust_calculate_class_vars,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_classprop::rust_add_type_promotion,
        module
    )?)?; // Issue #540: pure helpers from mypy/modulefinder.py.
    module.add_function(wrap_pyfunction!(modulefinder::rust_is_init_file, module)?)?;
    module.add_function(wrap_pyfunction!(modulefinder::rust_parse_version, module)?)?;
    module.add_function(wrap_pyfunction!(modulefinder::rust_mypy_path, module)?)?;
    module.add_function(wrap_pyfunction!(
        modulefinder::rust_typeshed_py_version,
        module
    )?)?;
    // Issue #539: fixup.py NodeFixer/TypeFixer port.
    module.add_function(wrap_pyfunction!(fixup::rust_fixup_type, module)?)?;
    module.add_function(wrap_pyfunction!(fixup::rust_fixup_type_info, module)?)?;
    module.add_function(wrap_pyfunction!(fixup::rust_resolve_cross_ref, module)?)?;
    module.add_function(wrap_pyfunction!(fixup::rust_fixup_symbol_table, module)?)?;
    module.add_function(wrap_pyfunction!(
        fixup::rust_fixup_overloaded_func_def,
        module
    )?)?;
    // Issue #533: pure utility functions from util.py.
    module.add_function(wrap_pyfunction!(util::rust_is_dunder, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_is_sunder, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_split_module_names, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_module_prefix, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_split_target, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_short_type, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_find_python_encoding, module)?)?;
    module.add_function(wrap_pyfunction!(
        util::rust_bytes_to_human_readable_repr,
        module
    )?)?;
    // Issue #534: pure helpers from mypy/errors.py.
    module.add_function(wrap_pyfunction!(
        errors_helpers::rust_remove_path_prefix,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        errors_helpers::rust_report_internal_error,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        errors_helpers::rust_format_messages_default_pretty,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        errors_helpers::rust_sort_within_context,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        errors_helpers::rust_create_errors,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        errors_helpers::rust_yield_nonoverlapping_types,
        module
    )?)?;
    // Issue #537: partially-defined variable detection
    // (port of mypy.partially_defined.PossiblyUndefinedVariableVisitor).
    module.add_function(wrap_pyfunction!(
        partially_defined::rust_find_possibly_undefined,
        module
    )?)?;
    // Issue #536: TransformVisitor identity deep-copy port.
    module.add_function(wrap_pyfunction!(
        treetransform::rust_transform_copy,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(fixup::rust_fixup_decorator, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_decode_python_encoding, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_trim_source_line, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_get_mypy_comments, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_get_prefix, module)?)?;
    module.add_function(wrap_pyfunction!(
        util::rust_correct_relative_import,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(util::rust_unmangle, module)?)?;
    module.add_function(wrap_pyfunction!(
        util::rust_get_unique_redefinition_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(util::rust_count_stats, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_split_words, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_soft_wrap, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_hash_digest, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_hash_digest_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_hash_path_stem, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_is_sub_path_normabs, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_is_typeshed_file, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_is_stdlib_file, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_is_stub_package_file, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_unnamed_function, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_time_spent_us, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_plural_s, module)?)?;
    module.add_function(wrap_pyfunction!(util::rust_json_dumps, module)?)?;
    module.add_class::<util::IdMapper>()?;
    module.add_function(wrap_pyfunction!(
        modulefinder::rust_default_lib_path,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        modulefinder::rust_load_stdlib_py_versions,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        modulefinder::rust_matches_exclude,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        modulefinder::rust_get_search_dirs,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        modulefinder::rust_compute_search_paths,
        module
    )?)?;
    module.add_class::<modulefinder::SearchPaths>()?;
    module.add_class::<modulefinder::BuildSource>()?;
    module.add_class::<modulefinder::BuildSourceSet>()?;
    // Issue #535: message_registry.py port — ErrorMessage class + factory fns.
    module.add_class::<message_registry::ErrorMessage>()?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_type_raw_enum_value,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::no_return_value_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::missing_return_statement,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::empty_body_abstract,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_implicit_return,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_return_value_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::return_value_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::no_return_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_exception,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_exception_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_exception_group,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::return_in_async_generator,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_return_type_for_generator,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_return_type_for_async_generator,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::yield_value_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types_in_assignment,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::covariant_override_of_mutable_attribute,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types_in_await,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_redefinition,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types_in_yield,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types_in_yield_from,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types_in_capture,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::must_have_none_return_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::tuple_index_out_of_range,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::ambiguous_slice_of_variadic_tuple,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::too_many_targets_for_variadic_unpack,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_infer_lambda_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::non_instance_new_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_new_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::bad_constructor_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::inconsistent_abstract_overload,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::multiple_overloads_required,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::read_only_property_overrides_read_write,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::return_type_cannot_be_contravariant,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::function_parameter_cannot_be_covariant,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_import_of,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::function_type_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::only_class_application,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::return_type_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::param_type_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::keyword_argument_requires_str_key_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::all_must_be_seq_str,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_typeddict_args,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::typeddict_key_must_be_string_literal,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::malformed_assert,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::duplicate_type_signatures,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::descriptor_set_not_callable,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::module_level_getattribute,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::name_not_in_slots,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_always_true,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_always_true_uniontype,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::function_always_true,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::iterable_always_true,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::too_many_args_for_super,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::super_with_single_arg_not_supported,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::unsupported_arg_1_for_super,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::unsupported_arg_2_for_super,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::super_varargs_not_supported,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::super_positional_args_required,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::super_arg_2_not_instance_of_arg_1,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::target_class_has_no_base_class,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::super_outside_of_method_not_supported,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::super_enclosing_positional_args_required,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::missing_or_invalid_self_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::erased_self_type_not_supertype,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_inherit_from_final,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::dependent_final_in_class_body,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_make_deletable_final,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_disjoint_bases,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::enum_members_attr_will_be_overridden,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_override_instance_var,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_override_class_var,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::runtime_protocol_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_instantiate_protocol,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::too_many_union_combinations,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::contiguous_iterable_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::iterable_type_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_guard_pos_arg_required,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::failed_to_merge_overloads,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_ignore_with_errcode_on_module,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_type_ignore,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_comment_syntax_error_value,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::ellipsis_with_other_typeparams,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_signature_too_many_params,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_signature_too_few_params,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::arg_constructor_name_expected,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::arg_constructor_too_many_args,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::multiple_values_for_name_kwarg,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::multiple_values_for_type_kwarg,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::arg_constructor_unexpected_arg,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::arg_name_expected_string_literal,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::narrowed_type_not_subtype,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_var_too_few_constrained_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_var_yield_expression_in_bound,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_var_named_expression_in_bound,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_var_await_expression_in_bound,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_var_generic_constraint_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_var_redeclared_in_nested_class,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_alias_with_yield_expression,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_alias_with_named_expression,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_alias_with_await_expression,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types_in_async_with_aenter,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types_in_async_with_aexit,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types_in_async_for,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_type_for_slots,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::async_for_outside_coroutine,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::async_with_outside_coroutine,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_types_in_str_interpolation,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_access_init,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_assign_to_method,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_assign_to_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::format_requires_mapping,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::typeddict_override_merge,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::descriptor_get_not_callable,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_var_conflicts_slots,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(message_registry::not_callable, module)?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_must_be_used,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::generic_instance_var_class_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::generic_class_var_access,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(message_registry::bare_generic, module)?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::implicit_generic_any_builtin,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::no_cyclic_default,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::no_default_after_typevar_tuple,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(message_registry::invalid_unpack, module)?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_unpack_position,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_param_spec_location,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_param_spec_location_note,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::incompatible_typevar_value,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_typevar_as_typearg,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_typevar_arg_bound,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::invalid_typevar_arg_value,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::typevar_variance_def,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::typevar_arg_must_be_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::typevar_unexpected_argument,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(message_registry::unbound_typevar, module)?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::type_parameters_should_be_declared,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_access_final_instance_attr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_access_instance_only_attr,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_var_with_generic_self,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_var_outside_of_class,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::missing_match_args,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::or_pattern_alternative_names,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_pattern_generic_type_alias,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_pattern_type_required,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_pattern_too_many_positional_args,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_pattern_keyword_matches_positional,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_pattern_duplicate_keyword_pattern,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_pattern_unknown_keyword,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::class_pattern_class_or_static_method,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::multiple_assignments_in_pattern,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::cannot_modify_match_args,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::dataclass_field_alias_must_be_literal,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        message_registry::dataclass_post_init_must_be_a_function,
        module
    )?)?;
    // Issue #525: is_overlapping_types + helpers from meet.py.
    // rust_is_overlapping_types is already registered above (line 161).
    module.add_function(wrap_pyfunction!(
        meet::rust_is_overlapping_erased_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        meet::rust_are_typed_dicts_overlapping,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(meet::rust_are_tuples_overlapping, module)?)?;
    module.add_function(wrap_pyfunction!(
        meet::rust_expand_tuple_if_possible,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(meet::rust_adjust_tuple, module)?)?;
    module.add_function(wrap_pyfunction!(meet::rust_is_tuple, module)?)?;
    module.add_function(wrap_pyfunction!(
        meet::rust_is_enum_overlapping_union,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(meet::rust_is_literal_in_union, module)?)?;
    module.add_function(wrap_pyfunction!(meet::rust_is_object, module)?)?;
    module.add_function(wrap_pyfunction!(meet::rust_is_none_object_overlap, module)?)?;
    module.add_function(wrap_pyfunction!(meet::rust_are_related_types, module)?)?;

    // Issue #532: semanal_typeddict + semanal_namedtuple helpers.
    module.add_function(wrap_pyfunction!(
        semanal_typeddict::rust_extract_meta_info,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_typeddict::rust_check_namedtuple_field_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_typeddict::rust_namedtuple_prohibited_names,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_typeddict::rust_primary_source,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_typeddict::rust_verify_requiredness_compatibility,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        semanal_typeddict::rust_verify_field_against_closed_bases,
        module
    )?)?;
    // Issue #560: reachability.py port.
    module.add_function(wrap_pyfunction!(
        reachability::rust_infer_condition_value,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        reachability::rust_infer_pattern_value,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        reachability::rust_assert_will_always_fail,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        reachability::rust_consider_sys_version_info,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        reachability::rust_consider_sys_platform,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(reachability::rust_is_sys_attr, module)?)?;
    module.add_function(wrap_pyfunction!(
        reachability::rust_contains_sys_version_info,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        reachability::rust_contains_int_or_tuple_of_ints,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        reachability::rust_fixed_comparison,
        module
    )?)?;

    // Issue #527: binder.py pure helper (get_declaration).
    module.add_function(wrap_pyfunction!(binder::rust_get_declaration, module)?)?;
    module.add_function(wrap_pyfunction!(
        classmethod_static::rust_is_classmethod_node,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        classmethod_static::rust_is_node_static,
        module
    )?)?;

    module.add_function(wrap_pyfunction!(
        checker_visitor::rust_get_property_type,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        setops::rust_try_contracting_literals_in_union,
        module
    )?)?;

    // detach_callable: extend a callable's variables with the class type
    // variables it uses (mypy.checker).
    module.add_function(wrap_pyfunction!(
        detach_callable::rust_detach_callable,
        module
    )?)?;

    // overload_never: overload argument-prefix compatibility per the
    // Callable-vs-Callable fast paths (mypy.checker). Generic/Overloaded
    // operands defer to Python.
    module.add_function(wrap_pyfunction!(
        overload_never::rust_overload_can_never_match,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        overload_never::rust_is_more_general_arg_prefix,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        overload_never::rust_is_same_arg_prefix,
        module
    )?)?;

    // is_unsafe_overlapping_overload_signatures: judge overload-overlap
    // safety on detached, expanded wire callables (mypy.checker).
    #[allow(clippy::unsafe_removed_from_name)]
    module.add_function(wrap_pyfunction!(
        overlap_unsafe::rust_is_unsafe_overlapping_overload_signatures,
        module
    )?)?;

    // overload_override: check_overlapping_overloads pairwise screening loop
    // over the three predicates above (mypy.checker). The impl-vs-items tail
    // and the message emission stay in Python.
    module.add_function(wrap_pyfunction!(
        overload_override::rust_check_overlapping_overloads,
        module
    )?)?;

    // checker_functions: check_compatibility_final_super decision-head port.
    // Rust classifies the final-super override into a branch tag; the message
    // emission and writability side effects stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_final_super,
        module
    )?)?;

    // checker_functions: check_final decision-head port. Rust classifies the
    // final_without_value gate and the per-lvalue final-assignment
    // arbitration (MRO walk + is_final flags); the message emissions stay
    // in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_check_final,
        module
    )?)?;
    // checker_functions: check_compatibility_classvar_super 2x2 predicate port.
    // Rust classifies the classvar override into a branch tag; the message
    // emission stays in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_classvar_super,
        module
    )?)?;

    // checker_functions: check_compatibility_all_supers gate-head port.
    // Rust classifies the entry gate + per-base MRO skip decisions into
    // tags; the check bodies and message emission stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_all_supers_gate,
        module
    )?)?;

    // checker_functions: check___new___signature 3-way return-type port.
    // Rust classifies metaclass / non-instance / instance from two scalar
    // facts; the check_subtype calls and message emission stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_new_signature,
        module
    )?)?;

    // checker_functions: check_getattr_method 4-way dispatch-head port.
    // Rust classifies module/getattribute/class/pass from Scope facts.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_getattr_method,
        module
    )?)?;

    // checker_functions: check_func_def_override 5-way dispatch port. Rust
    // classifies the override into a branch tag from scalar facts; bodies stay
    // in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_func_def_override,
        module
    )?)?;

    // checker_functions: check_metaclass_compatibility decision-head port.
    // Rust classifies the exempt/conflict predicate into a branch tag; the
    // METACLASS fail + note stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_metaclass_compat,
        module
    )?)?;

    // Issue #923: check_enum_new per-base fold. Rust classifies each
    // base into SKIP/ADVANCE/CONFLICT; self.fail and has_new stay Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_enum_new,
        module
    )?)?;

    // Issue #937: check_enum_bases fold. Rust classifies the first
    // non-enum base after an enum base; self.fail stays Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_enum_bases,
        module
    )?)?;

    // Issue #971: check_enum multi-arm classifier. Rust classifies the
    // three arms (a/b/c) and returns bit flags + offending base names;
    // self.fail/note stay Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_enum,
        module
    )?)?;

    // Issue #936: is_final_enum_value pure bool predicate. Rust reads the
    // live SymbolTableNode via PyO3 and returns the bool directly.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_is_final_enum_value,
        module
    )?)?;

    // Issue #942: check_for_untyped_decorator conjunction port. Rust folds
    // the disallow/typed-callback/untyped-decorator/not-deferred bool gate on
    // the wire format; the message emission stays in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_check_for_untyped_decorator,
        module
    )?)?;

    // Issue #939: check_explicit_override_decorator 5-flag conjunction.
    // Rust evaluates the predicate; message emission stays in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_check_explicit_override_decorator,
        module
    )?)?;

    // Issue #955: check_lvalue dispatch port. Rust classifies the lvalue
    // node kind into a branch tag; the per-branch bodies stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_check_lvalue,
        module
    )?)?;

    // Issue #986: check_match_args predicate port. Rust reads one wire
    // Type and returns the TupleType + string-literal bool; the
    // active_class gate and note emission stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_check_match_args,
        module
    )?)?;

    // Issue #1050: type_check_raise decision-head port. Rust classifies
    // the deleted / not-implemented arbitration into a branch tag; the
    // fail emissions and the check_call recursion stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_type_check_raise,
        module
    )?)?;

    // Issue #1003: check_rvalue_count_in_assignment dispatch port. Rust
    // classifies the arity/star decision into a branch tag; the fail and
    // wrong-number messages stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_rvalue_count,
        module
    )?)?;

    // Issue #1010: check_for_truthy_type decision-head port. Rust
    // classifies the strict-optional truthiness arbitration into a
    // branch tag; the format_type messages and fail emission stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_truthy_type,
        module
    )?)?;
    // Issue #1004: check_return_stmt two-phase decision port; the accept()
    // call and the fail/note emissions stay in Python.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_return_stmt_variant,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_return_stmt_pre,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_return_stmt_post,
        module
    )?)?;

    // Issue #1009: check_for_missing_annotations decision-head port. Rust
    // arbitrates the annotation-completeness gates; the fail/note emission
    // stays in Python. Tag contract in checker_functions.rs.
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_missing_annotations,
        module
    )?)?;

    // Issue #1055: check_simple_assignment decision-head port. Rust
    // arbitrates the stub / direct / fallback-context dispatch; the accept
    // recursion and the Python-side blocks stay in Python (see checker_functions.rs).
    module.add_function(wrap_pyfunction!(
        checker_functions::rust_classify_simple_assignment,
        module
    )?)?;

    // Stage 3e: typeops.supported_self_type (explicit self-type predicate).
    module.add_function(wrap_pyfunction!(
        supported_self_type::rust_supported_self_type,
        module
    )?)?;

    // #566: checker.group_comparison_operands (pure-data union-find port).
    module.add_function(wrap_pyfunction!(
        comparison_group::rust_group_comparison_operands,
        module
    )?)?;

    // checker.builtin_item_type (parity seam for a builtin container's
    // element type, optional narrow).
    module.add_function(wrap_pyfunction!(
        builtin_item::rust_builtin_item_type,
        module
    )?)?;

    // checker.conditional_types (the isinstance/equality narrowing split).
    module.add_function(wrap_pyfunction!(
        cond_types::rust_conditional_types,
        module
    )?)?;

    // Issue #745: subtypes.covers_at_runtime (runtime isinstance coverage).
    module.add_function(wrap_pyfunction!(
        covers_at_runtime::rust_covers_at_runtime,
        module
    )?)?;

    // infer_variance member-direction analysis (mypy.subtypes). The shim
    // keeps the variance loop, per-member this computes the co/contra flip
    // bitmask or defers (None) to the pure-Python member body.
    module.add_function(wrap_pyfunction!(
        infer_variance::rust_infer_variance_member,
        module
    )?)?;

    // typeops._remove_redundant_union_items (two-pass union dedup).
    module.add_function(wrap_pyfunction!(
        remove_redundant::rust_remove_redundant_union_items,
        module
    )?)?;

    Ok(())
}
