"""Native engagement suites for the misc area (`mypy/plugin.py` and the plugin registry).

Engagement suites (a `Native...Suite` without `Retired` in the name) live in
one file per area so that a lane changing one area's seam tests touches that
area's file only; retired-seam pins live in `testtypes_native_retired_*.py`.

The area of a suite is the module that hosts the seam it exercises, taken from
the `rust_*` shim it references (else the `mypy` module its docstring names), so
a new suite belongs in the file for the module it tests. Split out of the four
28k-line per-area files in #1757; see the collect-only and AST proof in that PR.
"""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from unittest import skipUnless

from mypy.test.helpers import Suite
from mypy.test.testtypes import _HAS_TYPE_KERNEL


# Moved from mypy/test/testtypes_native_checker.py.
@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativePluginHookDeclareSuite(Suite):
    """Tests for ``Plugin.declare_hook_fullnames`` and the
    ``ChainedPlugin`` union that feeds ``PluginHookRegistry``.

    With the declaration surface, the self-check's ``proper_plugin``
    no longer forces ``has_user_plugins=True``: the ``ChainedPlugin``
    unions ``DefaultPlugin`` (7 kinds, 81 names) with
    ``ProperTypePlugin`` (1 kind, 3 names) and the registry goes live.
    """

    def setUp(self) -> None:
        from mypy.options import Options
        from mypy.plugins.default import DEFAULT_HOOK_FULLNAMES_BY_KIND, DefaultPlugin
        from mypy.plugins.proper_plugin import ProperTypePlugin

        self._options = Options()
        self._default = DefaultPlugin(self._options)
        self._proper = ProperTypePlugin(self._options)
        self._by_kind = DEFAULT_HOOK_FULLNAMES_BY_KIND

    def test_base_plugin_returns_none(self) -> None:
        from mypy.plugin import Plugin

        assert Plugin(self._options).declare_hook_fullnames() is None

    def test_default_plugin_returns_seven_kinds(self) -> None:
        declared = self._default.declare_hook_fullnames()
        assert declared is not None
        assert set(declared.keys()) == set(self._by_kind.keys())
        for kind, names in self._by_kind.items():
            assert declared[kind] == names

    def test_proper_plugin_declares_get_function_hook(self) -> None:
        declared = self._proper.declare_hook_fullnames()
        assert declared is not None
        assert set(declared.keys()) == {"get_function_hook"}
        assert declared["get_function_hook"] == frozenset(
            {"builtins.isinstance", "mypy.types.get_proper_type", "mypy.types.get_proper_types"}
        )

    def test_chained_plugin_unions_default_and_proper(self) -> None:
        from mypy.plugin import ChainedPlugin

        chained = ChainedPlugin(self._options, [self._proper, self._default])
        declared = chained.declare_hook_fullnames()
        assert declared is not None
        # get_function_hook is the union of DefaultPlugin's 5 names and
        # ProperTypePlugin's 3 names.
        assert declared["get_function_hook"] == self._by_kind["get_function_hook"] | frozenset(
            {"builtins.isinstance", "mypy.types.get_proper_type", "mypy.types.get_proper_types"}
        )
        # Non-overlapping kinds pass through unchanged.
        for kind in (
            "get_function_signature_hook",
            "get_method_signature_hook",
            "get_method_hook",
        ):
            assert declared[kind] == self._by_kind[kind]

    def test_chained_plugin_returns_none_if_any_child_none(self) -> None:
        from mypy.plugin import ChainedPlugin, Plugin

        class NonEnumerablePlugin(Plugin):
            pass  # inherits Plugin.declare_hook_fullnames -> None

        chained = ChainedPlugin(self._options, [NonEnumerablePlugin(self._options), self._default])
        assert chained.declare_hook_fullnames() is None

    def test_registry_includes_proper_plugin_names(self) -> None:
        import type_kernel as _type_kernel

        from mypy.checkexpr import _set_native_plugin_hook_registry, plugin_hook_known_absent
        from mypy.plugin import ChainedPlugin

        chained = ChainedPlugin(self._options, [self._proper, self._default])
        declared = chained.declare_hook_fullnames()
        assert declared is not None
        registry = _type_kernel.PluginHookRegistry(
            {kind: list(names) for kind, names in declared.items()}
        )
        _set_native_plugin_hook_registry(registry, has_user_plugins=False)
        try:
            # builtins.isinstance is declared by ProperTypePlugin, so it
            # must NOT be known-absent for get_function_hook.
            assert not plugin_hook_known_absent("get_function_hook", "builtins.isinstance")
            # An unrelated name is still known-absent.
            assert plugin_hook_known_absent("get_function_hook", "builtins.print")
        finally:
            _set_native_plugin_hook_registry(None, False)
