"""Unit tests for the native-gate env decoding and availability check (#1285, #1287)."""

from __future__ import annotations

import os
import types
import unittest
from io import StringIO
from unittest import mock

from mypy.build import BuildManager
from mypy.errors import Errors
from mypy.fscache import FileSystemCache
from mypy.modulefinder import BuildSourceSet, SearchPaths
from mypy.options import Options
from mypy.plugin import Plugin
from mypy.test.helpers import _ensure_native_modules_available, _env_gate


def _fail_import(name: str, *args: object) -> types.ModuleType:
    raise ImportError(f"cannot import {name}: simulated wrong-interpreter failure")


class NativeEnvAvailabilitySuite(unittest.TestCase):
    def test_no_request_is_silent(self) -> None:
        # No requested gate: the import probe is never attempted.
        with mock.patch("importlib.import_module", side_effect=_fail_import):
            _ensure_native_modules_available()

    def test_missing_module_fails_loudly(self) -> None:
        with mock.patch.dict(os.environ, {"TEST_NATIVE_TYPE_KERNEL": "1"}):
            with mock.patch("importlib.import_module", side_effect=_fail_import):
                with self.assertRaises(SystemExit) as ctx:
                    _ensure_native_modules_available()
        self.assertIn("TEST_NATIVE_TYPE_KERNEL is set", str(ctx.exception))

    def test_missing_probe_attribute_fails_loudly(self) -> None:
        with mock.patch.dict(os.environ, {"TEST_NATIVE_TYPE_KERNEL": "1"}):
            # Module loads but lacks the probe attr (stub package, #1285).
            with mock.patch("importlib.import_module", return_value=types.SimpleNamespace()):
                with self.assertRaises(SystemExit) as ctx:
                    _ensure_native_modules_available()
        self.assertIn("loaded without `erase_type`", str(ctx.exception))

    def test_gate_off_skips_the_probe(self) -> None:
        # TEST_NATIVE_*=0 must not demand a loadable .so (#1287).
        with mock.patch.dict(os.environ, {"TEST_NATIVE_TYPE_KERNEL": "0"}):
            with mock.patch("importlib.import_module", side_effect=_fail_import):
                _ensure_native_modules_available()


class EnvGateSuite(unittest.TestCase):
    def test_off_spellings(self) -> None:
        for env, val in [
            ("TEST_NATIVE_TYPE_KERNEL", None),
            ("TEST_NATIVE_TYPE_KERNEL", ""),
            ("TEST_NATIVE_TYPE_KERNEL", "0"),
            ("TEST_NATIVE_PARSER", "false"),
            ("TEST_NATIVE_RESOLVER", "OFF"),
        ]:
            with self.subTest(env=env, val=val), mock.patch.dict(os.environ, clear=False):
                if val is None:
                    os.environ.pop(env, None)
                else:
                    os.environ[env] = val
                self.assertFalse(_env_gate(env))

    def test_on_spellings(self) -> None:
        for val in ("1", "true", "YES", "on"):
            with (
                self.subTest(val=val),
                mock.patch.dict(
                    os.environ, {"TEST_NATIVE_TYPE_KERNEL": val}, clear=False
                ),
            ):
                self.assertTrue(_env_gate("TEST_NATIVE_TYPE_KERNEL"))

    def test_unknown_value_fails_loudly(self) -> None:
        with mock.patch.dict(os.environ, {"TEST_NATIVE_TYPE_KERNEL": "maybe"}):
            with self.assertRaises(SystemExit) as ctx:
                _env_gate("TEST_NATIVE_TYPE_KERNEL")
        self.assertIn("TEST_NATIVE_TYPE_KERNEL", str(ctx.exception))


class NativeGateOffPropagationSuite(unittest.TestCase):
    """The gate-off differential must be a real differential (#1287).

    Build-manager flag propagation keys every module-level kernel flag
    off `Options.native_type_kernel`, so an option parsed as False by
    `_env_gate` must leave all seams off even when a `.so` is importable.
    """

    _FLAG_MODULES = [
        "mypy.erasetype._native_erase_active",
        "mypy.subtypes._native_subtype_active",
        "mypy.checker._native_checker_active",
        "mypy.checkexpr._native_checkexpr_active",
        "mypy.maptype._native_map_active",
    ]

    def _flags(self) -> dict[str, bool]:
        import importlib

        return {
            path: bool(getattr(importlib.import_module(path.rsplit(".", 1)[0]), path.rsplit(".", 1)[1]))
            for path in self._FLAG_MODULES
        }

    def _build_manager(self, options: Options) -> BuildManager:
        return BuildManager(
            data_dir=".",
            search_paths=SearchPaths((), (), (), ()),
            ignore_prefix="",
            source_set=BuildSourceSet([]),
            reports=None,
            options=options,
            version_id="test",
            plugin=Plugin(options),
            plugins_snapshot={},
            errors=Errors(options),
            flush_errors=lambda *a, **kw: None,
            fscache=FileSystemCache(),
            stdout=StringIO(),
            stderr=StringIO(),
        )

    def test_gate_off_leaves_all_seams_inactive(self) -> None:
        options = Options()
        options.use_builtins_fixtures = True
        options.native_parser = _env_gate("TEST_NATIVE_PARSER")
        options.native_resolver = _env_gate("TEST_NATIVE_RESOLVER")
        options.native_type_kernel = _env_gate("TEST_NATIVE_TYPE_KERNEL")
        assert not options.native_type_kernel
        self._build_manager(options)
        after = self._flags()
        # The build manager itself must not have flipped any seam on.
        for path, value in after.items():
            self.assertFalse(value, f"{path} active under a gate-off options set")

    def test_gate_on_propagates_and_restores(self) -> None:
        options = Options()
        options.use_builtins_fixtures = True
        options.native_type_kernel = True
        flags = self._flags()
        try:
            self._build_manager(options)
            active = self._flags()
            for path, value in active.items():
                self.assertTrue(value, f"{path} not propagated from a gate-on options set")
        finally:
            for path, value in flags.items():
                mod_name, attr = path.rsplit(".", 1)
                import importlib

                setattr(importlib.import_module(mod_name), attr, value)


if __name__ == "__main__":
    unittest.main()
