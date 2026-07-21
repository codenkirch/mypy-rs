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
//!
//! Shared infrastructure (`TypeRefs` class cache, `fallback_sentinel`/
//! `is_fallback`, `make_any`) lives in `refs` and is reused by both stages.
//! See `docs/rust-migration-strangler.md` ("Milestone 3/4/5 (Phase 4)") for the
//! full staging roadmap.

mod aliases;
mod erase;
mod lkv;
mod refs;
mod typeinfo;
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
    module.add_class::<typeinfo::NativeTypeResolver>()?;
    Ok(())
}
