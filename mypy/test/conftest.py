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

import os

from mypy.build import BuildManager


def _install_native_resolvers_patch() -> None:
    original = BuildManager._build_native_resolvers

    def patched(self):
        original(self)
        if not self.options.native_type_kernel:
            return
        try:
            import type_kernel as _type_kernel
        except ImportError:
            return
        from mypy.join import (
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_resolver

        type_infos = self._collect_type_infos()
        resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_resolver(resolver)
        _set_native_join_resolver(resolver)
        _set_native_join_typeinfo_map(
            {info.fullname: info for info in type_infos}
        )

        # Forward-compatible: install the MRO resolver if the Stage 5
        # shim is present (ships with B1 / PR #69). Wrapped so the
        # parity gate works on main before B1 merges.
        try:
            from mypy.mro import _set_native_mro_resolver

            _set_native_mro_resolver(
                resolver, {info.fullname: info for info in type_infos}
            )
        except ImportError:
            pass

        # Forward-compatible: install the expand_type resolver if the
        # Stage 3d shim is present (ships with B2). Gated behind a
        # separate env var because the Rust expand_type port still has
        # ~316 testcheck failures (unexpanded TypeVars). The parity
        # CI gate does NOT set this var until those are resolved.
        if os.environ.get("MYPY_NATIVE_PARITY_INSTALL_EXPAND_RESOLVERS"):
            try:
                from mypy.expandtype import (
                    _set_native_expand_type_resolver,
                    _set_native_expand_type_typeinfo_map,
                )

                _set_native_expand_type_resolver(resolver)
                _set_native_expand_type_typeinfo_map(
                    {info.fullname: info for info in type_infos}
                )
            except ImportError:
                pass

    BuildManager._build_native_resolvers = patched


if (
    os.environ.get("MYPY_NATIVE_PARITY_INSTALL_RESOLVERS")
    and os.environ.get("TEST_NATIVE_TYPE_KERNEL")
):
    _install_native_resolvers_patch()
