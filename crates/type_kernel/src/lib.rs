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
#![allow(clippy::large_enum_variant)]

mod aliases;
mod applytype;
mod argapprox;
mod argmap;
mod astdiff_snapshot;
mod astdiff_symbols;
mod astwire;
mod attrs;
mod binder;
mod builtin_item;
mod cache;
mod callable_compat;
mod checkcall;
mod checkcall_typeobj;
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
mod comparison_narrowing;
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
// Phase F0 (#1349): identity service for future graph-owner phases.
mod identity;
// Phase F1 (#1370): dual-write shadow storage behind the mirror gate.
mod infer_variance;
mod joinfns;
mod lennarrow;
mod lkv;
mod lookup_definer;
mod maptype;
mod meet;
mod member_flags;
mod message_registry;
mod messages;
mod messages_find_overlaps;
mod mirror;
mod modulefinder;
mod mro;
mod operators;
mod overlap_unsafe;
mod overload;
mod overload_never;
mod overload_override;
mod protocols;
// Phase G1.0a (#1572): expression dual-write node shadow store.
mod node_mirror;

mod findmember;
mod partially_defined;
mod plugin_helpers;
mod plugin_hooks;
mod reachability;
mod refs;
mod remove_redundant;
mod returns_none;
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
// Issue #1632: native fine-grained dependency walk (DependencyVisitor).
mod depswalk;
mod setops;
mod solve;
mod stubgen;
// Issue #1635: native subexpr walk + aststrip helpers.
mod subexpr_strip;
mod subtypes;
mod suggestions;
mod supported_self_type;
// Phase G3.0a (#1581): namespace dual-write capture shadow store.
mod symtable_mirror;
mod traverser;
mod treetransform;
mod type_range;
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
mod typeview;
mod unify;
mod util;
mod visitor;
mod wire;

use pyo3::prelude::*;

/// PyO3 module entry point: registers the visitor functions (Stages 1/2)
/// and the parity-only wire readers (Stages 3a/3b) + the Stage 3c M8a
/// native resolver.
#[pymodule]
fn type_kernel(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    // Per-module registration (#1677): each defining module owns its
    // `register_registry`, called in pre-split first-statement order so the
    // cross-module name shadows keep resolving to the same implementations.
    erase::register_registry(module)?;
    lkv::register_registry(module)?;
    cache::register_registry(module)?;
    wire::register_registry(module)?;
    constant_fold::register_registry(module)?;
    typeinfo::register_registry(module)?;
    subtypes::register_registry(module)?;
    member_flags::register_registry(module)?;
    protocols::register_registry(module)?;
    callable_compat::register_registry(module)?;
    meet::register_registry(module)?;
    setops::register_registry(module)?;
    joinfns::register_registry(module)?;
    argmap::register_registry(module)?;
    expand::register_registry(module)?;
    mro::register_registry(module)?;
    expandtype::register_registry(module)?;
    freshen::register_registry(module)?;
    typeops::register_registry(module)?;
    maptype::register_registry(module)?;
    operators::register_registry(module)?;
    erase_typevars::register_registry(module)?;
    visitor::register_registry(module)?;
    checkexpr_argtypes::register_registry(module)?;
    applytype::register_registry(module)?;
    checkexpr_functions::register_registry(module)?;
    serverdeps::register_registry(module)?;
    typeanal_queries::register_registry(module)?;
    checkcall_typeobj::register_registry(module)?;
    checkexpr_overload::register_registry(module)?;
    argapprox::register_registry(module)?;
    generators::register_registry(module)?;
    errors::register_registry(module)?;
    constraints::register_registry(module)?;
    constraints_helpers::register_registry(module)?;
    constraints_select::register_registry(module)?;
    constraints_filter::register_registry(module)?;
    copymodified::register_registry(module)?;
    checkcall::register_registry(module)?;
    overload::register_registry(module)?;
    checker_stmts::register_registry(module)?;
    checkexpr_argcheck::register_registry(module)?;
    checkexpr_argcount::register_registry(module)?;
    lennarrow::register_registry(module)?;
    checker_visitor::register_registry(module)?;
    dangerous_comparison::register_registry(module)?;
    solve::register_registry(module)?;
    messages::register_registry(module)?;
    messages_find_overlaps::register_registry(module)?;
    checkstrformat::register_registry(module)?;
    checkpattern::register_registry(module)?;
    equality_info::register_registry(module)?;
    equality_ambiguity::register_registry(module)?;
    expand_variants::register_registry(module)?;
    traverser::register_registry(module)?;
    semanal_algebra::register_registry(module)?;
    semanal_visitor::register_registry(module)?;
    semanal_bases::register_registry(module)?;
    semanal_metaclass::register_registry(module)?;
    semanal_checks::register_registry(module)?;
    semanal_lookup::register_registry(module)?;
    semanal_typeexpr::register_registry(module)?;
    semanal_shared::register_registry(module)?;
    typeanal_info::register_registry(module)?;
    typealias_instantiate::register_registry(module)?;
    typeanal_unbound::register_registry(module)?;
    typeanal_unbound2::register_registry(module)?;
    typeanal_special::register_registry(module)?;
    typeanal_literal::register_registry(module)?;
    typeanal_rawexpr::register_registry(module)?;
    typeanal_deprec::register_registry(module)?;
    typeanal_callable::register_registry(module)?;
    checkmember::register_registry(module)?;
    checkoperator::register_registry(module)?;
    plugin_hooks::register_registry(module)?;
    suggestions::register_registry(module)?;
    depswalk::register_registry(module)?;
    astdiff_snapshot::register_registry(module)?;
    astdiff_symbols::register_registry(module)?;
    stubgen::register_registry(module)?;
    attrs::register_registry(module)?;
    dataclasses::register_registry(module)?;
    plugin_helpers::register_registry(module)?;
    types_impl::register_registry(module)?;
    checker_helpers::register_registry(module)?;
    condmaps::register_registry(module)?;
    semanal_classprop::register_registry(module)?;
    modulefinder::register_registry(module)?;
    fixup::register_registry(module)?;
    util::register_registry(module)?;
    errors_helpers::register_registry(module)?;
    partially_defined::register_registry(module)?;
    treetransform::register_registry(module)?;
    message_registry::register_registry(module)?;
    semanal_typeddict::register_registry(module)?;
    reachability::register_registry(module)?;
    binder::register_registry(module)?;
    classmethod_static::register_registry(module)?;
    detach_callable::register_registry(module)?;
    overload_never::register_registry(module)?;
    overlap_unsafe::register_registry(module)?;
    overload_override::register_registry(module)?;
    checker_functions::register_registry(module)?;
    type_range::register_registry(module)?;
    supported_self_type::register_registry(module)?;
    comparison_group::register_registry(module)?;
    comparison_narrowing::register_registry(module)?;
    builtin_item::register_registry(module)?;
    cond_types::register_registry(module)?;
    covers_at_runtime::register_registry(module)?;
    infer_variance::register_registry(module)?;
    remove_redundant::register_registry(module)?;
    returns_none::register_registry(module)?;
    lookup_definer::register_registry(module)?;
    findmember::register_registry(module)?;
    mirror::register_registry(module)?;
    node_mirror::register_registry(module)?;
    symtable_mirror::register_registry(module)?;
    subexpr_strip::register_registry(module)?;
    typeview::register_registry(module)?;
    Ok(())
}
