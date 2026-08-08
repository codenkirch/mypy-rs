"""Parity-test conftest for the native type-kernel.

When `MYPY_NATIVE_PARITY_INSTALL_RESOLVERS=1` is set, monkeypatches
`BuildManager._build_native_resolvers` so every testcheck build also installs
the Stage 3c subtype/join resolvers (and the Stage 5 MRO resolver if
present). This simulates the post-A3 production wiring without touching
`build.py`, so the full `testcheck.py` corpus runs against the Rust kernels.

No-op otherwise: the existing pr-gate runs with `TEST_NATIVE_TYPE_KERNEL=1`
but without this var, so this conftest stays dormant.

The patch is applied at module import time (NOT in `pytest_configure`) so
it survives across pytest-xdist worker forks: each worker re-imports this
module and re-applies the patch before any test runs.
"""

from __future__ import annotations

import importlib
import os
from pathlib import Path
from typing import Any

import pytest

from mypy.build import BuildManager


def _install_native_resolvers_patch() -> None:
    original = BuildManager._build_native_resolvers

    def patched(self: BuildManager) -> None:
        original(self)
        if not self.options.native_type_kernel:
            return
        try:
            import type_kernel as _type_kernel
        except ImportError:
            # A parity run that demands the native path must not silently
            # fall back: that would verify nothing. Fail loudly instead.
            if os.environ.get("MYPY_NATIVE_TYPE_KERNEL_REQUIRED"):
                raise
            return
        from mypy.join import _set_native_join_resolver, _set_native_join_typeinfo_map
        from mypy.subtypes import _set_native_subtype_resolver

        type_infos = self._collect_type_infos()
        resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_resolver(resolver)
        _set_native_join_resolver(resolver)
        _set_native_join_typeinfo_map({info.fullname: info for info in type_infos})

        try:
            from mypy.constraints import _set_native_constraints_active

            _set_native_constraints_active(True)
        except ImportError:
            pass

        try:
            from mypy.errors import _set_native_errors_active

            _set_native_errors_active(True)
        except ImportError:
            pass

        # Stage 11: install the solve resolver (parity path). The solve
        # shim needs the resolver to build Instance types (Object/Ancestor
        # setop results) and run is_subtype.
        try:
            from mypy.solve import _set_native_solve_resolver

            _set_native_solve_resolver(resolver)
        except ImportError:
            pass

        # Forward-compatible: install the MRO resolver if the Stage 5
        # shim is present (ships with B1 / PR #69). Wrapped so the
        # parity gate works on main before B1 merges.
        try:
            from mypy.mro import _set_native_mro_resolver

            _set_native_mro_resolver(resolver, {info.fullname: info for info in type_infos})
        except ImportError:
            pass

        # Stage 3e typeops helpers (parity-only). Install the resolver so
        # the typeops shim can call rust_make_simplified_union,
        # rust_is_simple_literal, rust_true_only, rust_false_only,
        # rust_true_or_false. Gated behind the same env var as expand.
        if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_TYPEOPS_RESOLVERS"):
            try:
                from mypy.typeops import _set_native_typeops_resolver

                _set_native_typeops_resolver(resolver)
            except ImportError:
                pass

        # Stage 4c erase_typevars (parity-only). Activates the wire-format
        # erase_typevars/replace_meta_vars gate. No resolver needed.
        if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_ERASEVARS"):
            try:
                from mypy.erasetype import _set_native_erase_typevars_active

                _set_native_erase_typevars_active(True)
            except ImportError:
                pass

        # Stage 7 visitor framework (parity-only). Activates the wire-format
        # has_type_vars, flatten_nested_unions, etc. gate. No resolver needed.
        if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_VISITOR"):
            try:
                from mypy.types import _set_native_visitor_active

                _set_native_visitor_active(True)
                # Type-returning functions (remove_dups, type_vars_as_args,
                # callable_with_ellipsis, split_with_prefix_and_suffix,
                # flatten_nested_unions, flatten_nested_tuples) lose
                # truthiness flags on wire round-trip. Separate gate.
                if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_VISITOR_TYPES"):
                    from mypy.types import _set_native_visitor_types_active

                    _set_native_visitor_types_active(True)
                # copy_type gate: round-trip loses truthiness flags.
                if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_COPYTYPE"):
                    from mypy.copytype import _set_native_copy_active

                    _set_native_copy_active(True)
            except ImportError:
                pass

        # Stage 9 standalone checker/checkexpr functions (parity-only).
        # Scalar-returning functions use _native_checker_active /
        # _native_checkexpr_active. Type-returning functions
        # (flatten_types_if_tuple, try_getting_literal) use the types
        # gate since wire round-trip loses truthiness flags.
        if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_CHECKEXPR"):
            try:
                from mypy.checkexpr import _set_native_checkexpr_active

                _set_native_checkexpr_active(True)
                from mypy.checker import (
                    _set_native_checker_active,
                    _set_native_checker_types_active,
                )

                _set_native_checker_active(True)
                if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_CHECKEXPR_TYPES"):
                    _set_native_checker_types_active(True)
            except ImportError:
                pass

        # Stage 4b check_call dispatch (parity-only). The guard reads
        # _native_checkexpr_active, so flipping it here activates the
        # dispatch verification; makes CHECKCALL alone sufficient.
        if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_CHECKCALL"):
            try:
                from mypy.checkexpr import _set_native_checkexpr_active

                _set_native_checkexpr_active(True)
            except ImportError:
                pass

        # M17 statement helpers (parity-only). type_requires_usage /
        # is_unreachable_map via the type wire, rust_stmt_outcome via the
        # statement wire (astwire).
        if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_CHECKER_STMTS"):
            try:
                from mypy.checker import _set_native_checker_stmts_active

                _set_native_checker_stmts_active(True)
            except ImportError:
                pass

        # Stage 6c small pure modules batch (parity-only).
        # apply_generic_arguments needs the resolver + typeinfo_map
        # (shares the subtype/expand resolvers). has_no_typevars needs
        # no resolver. Both gated behind the APPLYTYPE env var.
        if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_APPLYTYPE"):
            try:
                from mypy.applytype import (
                    _set_native_applytype_active,
                    _set_native_applytype_resolver,
                    _set_native_applytype_typeinfo_map,
                )

                _set_native_applytype_active(True)
                _set_native_applytype_resolver(resolver)
                _set_native_applytype_typeinfo_map({info.fullname: info for info in type_infos})
                from mypy.typevars import _set_native_typevars_active

                _set_native_typevars_active(True)
            except ImportError:
                pass

        # Stage 16 (#209): semanal visitor helpers (parity-only).
        # Activates the PyO3-on-live-objects gate for refers_to_fullname,
        # is_trivial_body, find_duplicate, is_valid_replacement,
        # is_same_symbol, names_modified_in_lvalue, etc.
        if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_SEMANAL_VISITOR"):
            try:
                from mypy.semanal import _set_native_semanal_visitor_active

                _set_native_semanal_visitor_active(True)
            except ImportError:
                pass

    BuildManager._build_native_resolvers = patched  # type: ignore[method-assign]


if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_RESOLVERS") and os.environ.get(
    "TEST_NATIVE_TYPE_KERNEL"
):
    _install_native_resolvers_patch()


@pytest.hookimpl(tryfirst=True)
def pytest_sessionstart(session: Any) -> None:
    """Fail loudly when the installed type_kernel extension is stale.

    The editable install's `type_kernel.abi3.so` silently goes stale when
    `crates/type_kernel/src/**` changes but the build step is skipped,
    producing confusing type errors and 'De-serialization failure' crashes
    that look like code bugs (issue #228). Compare the installed binary's
    mtime against the newest source file; on mismatch fail with the exact
    rebuild command instead of running tests against a stale kernel.

    Only active when `TEST_NATIVE_TYPE_KERNEL` is set (the parity runs).
    No-op when the source tree is absent (e.g. installed wheel without the
    Rust source) — there is nothing to compare against.
    """
    if not os.environ.get("TEST_NATIVE_TYPE_KERNEL"):
        return
    try:
        import type_kernel
    except ImportError:
        return
    kernel_root = Path(__file__).resolve().parents[2] / "crates" / "type_kernel"
    src_dir = kernel_root / "src"
    # Installed wheel without the Rust source tree: nothing to compare.
    if not src_dir.is_dir():
        return
    installed = Path(type_kernel.__file__).parent / "type_kernel.abi3.so"
    if not installed.exists():
        return
    newest_source = max(
        (p.stat().st_mtime for p in src_dir.rglob("*.rs") if p.is_file()), default=0
    )
    if installed.stat().st_mtime < newest_source:
        raise RuntimeError(
            "Installed type_kernel extension is stale (built before "
            f"{src_dir}/** changed). Rebuild:\n"
            "  cargo rustc -p mypy-type-kernel --features extension-module --lib "
            "--crate-type cdylib --release "
            "-- -C link-arg=-undefined -C link-arg=dynamic_lookup\n"
            f"  cp target/release/libtype_kernel.dylib {installed}"
        )


# Cross-suite isolation for the module-level native-gate globals (issue #322).
# Each entry is a (dotted module path, attribute) pair; the modules are already
# imported by test time, so the lazy import in the fixture is a cheap no-op.
_NATIVE_GATE_GLOBALS: list[tuple[str, str]] = [
    ("mypy.applytype", "_native_applytype_active"),
    ("mypy.applytype", "_native_applytype_resolver"),
    ("mypy.applytype", "_native_applytype_typeinfo_map"),
    ("mypy.argmap", "_native_argmap_active"),
    ("mypy.cache", "_native_cache_active"),
    ("mypy.checker", "_native_checker_active"),
    ("mypy.checker", "_native_checker_types_active"),
    ("mypy.checker", "_native_checker_stmts_active"),
    ("mypy.checkexpr", "_native_checkexpr_active"),
    ("mypy.checkexpr", "_native_checkexpr_resolver"),
    ("mypy.checkexpr", "_native_plugin_hook_registry"),
    ("mypy.checkexpr", "_native_plugin_hook_has_user_plugins"),
    ("mypy.checkmember", "_native_checkmember_active"),
    ("mypy.checkmember", "_native_checkmember_resolver"),
    ("mypy.checkpattern", "_native_checkpattern_active"),
    ("mypy.checkstrformat", "_native_strformat_active"),
    ("mypy.constraints", "_native_constraints_active"),
    ("mypy.constraints", "_native_constraints_resolver"),
    ("mypy.copytype", "_native_copy_active"),
    ("mypy.erasetype", "_native_erase_active"),
    ("mypy.erasetype", "_native_erase_typevars_active"),
    ("mypy.errors", "_native_errors_active"),
    ("mypy.expandtype", "_native_expand_type_active"),
    ("mypy.expandtype", "_native_expand_type_resolver"),
    ("mypy.expandtype", "_native_expand_type_typeinfo_map"),
    ("mypy.join", "_native_join_active"),
    ("mypy.join", "_native_join_resolver"),
    ("mypy.join", "_native_join_typeinfo_map"),
    ("mypy.messages", "_native_messages_active"),
    ("mypy.messages", "_native_messages_resolver"),
    ("mypy.messages", "_native_suggestions_active"),
    ("mypy.mro", "_native_mro_active"),
    ("mypy.mro", "_native_mro_resolver"),
    ("mypy.mro", "_native_mro_typeinfo_map"),
    ("mypy.semanal", "_native_semanal_active"),
    ("mypy.semanal", "_native_semanal_visitor_active"),
    ("mypy.server.deps", "_native_server_deps_active"),
    ("mypy.solve", "_native_solve_active"),
    ("mypy.solve", "_native_solve_resolver"),
    ("mypy.subtypes", "_native_subtype_active"),
    ("mypy.subtypes", "_native_subtype_resolver"),
    ("mypy.typeanal", "_native_typeanal_active"),
    ("mypy.typeops", "_native_typeops_active"),
    ("mypy.typeops", "_native_typeops_resolver"),
    ("mypy.types", "_native_visitor_active"),
    ("mypy.types", "_native_visitor_types_active"),
    ("mypy.typevars", "_native_typevars_active"),
    # Side channel fed by `join._set_native_join_typeinfo_map`; kept in sync
    # with the join globals so the wire-ref fixer never sees a stale map.
    ("mypy.wirefixup", "_wire_typeinfo_map"),
]


# Cross-suite isolation for the module-level native-gate globals (issue #336).
# Hooks, not an autouse fixture: DataDrivenTestCase items bypass fixture
# resolution via custom `runtest()`. Hooks fire for every item type.
_saved_native_gates: list[tuple[Any, str, object]] = []


def pytest_runtest_setup(item: Any) -> None:
    """Snapshot every module-level native-gate global before each test.

    `BuildManager.__init__` (mypy/build.py) sets every `_set_native_*_active`
    gate to options.native_type_kernel and installs the per-build
    `NativeTypeResolver` snapshot into the subtype/join/solve/constraints/
    mro/expand/applytype/typeops/messages/checkmember shims. None of that
    state is rolled back when the build ends. During a combined run a worker
    may therefore run a pure-Python testtypes suite (TypeOps/Join/Meet) right
    after a testcheck build; the pure suite then hits the Rust kernels with a
    foreign resolver built from the testcheck TypeInfo graph, producing
    mismatched join/subtype/expand results (~17 failures).

    The snapshot captures whatever the gate state is at setup time, so
    import-time activations are preserved by the restore. It never forces a
    value. Per-suite installs done by the Native* testtypes suites in their
    own setUp are wiped by the teardown restore, then re-installed by the
    next test's setUp. Running a single file is unaffected.
    """
    _saved_native_gates.clear()
    for module_name, attr in _NATIVE_GATE_GLOBALS:
        module = importlib.import_module(module_name)
        _saved_native_gates.append((module, attr, getattr(module, attr, None)))


def pytest_runtest_teardown(item: Any, nextitem: Any) -> None:
    """Restore the snapshot from `pytest_runtest_setup` after each test.

    `BuildManager` leaks gate/resolver state across builds with no rollback,
    and the Native* testtypes suites install their own resolver in setUp.
    Restoring the pre-test snapshot here keeps residue from one test from
    leaking into a later one on the same worker, while never fabricating
    state. Restore errors would silently mask a leak, so let them fail.
    """
    for module, attr, value in _saved_native_gates:
        setattr(module, attr, value)
    _saved_native_gates.clear()
