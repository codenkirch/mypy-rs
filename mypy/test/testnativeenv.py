"""Unit tests for the loud native-gate availability check (#1285)."""

from __future__ import annotations

import types
import unittest

import mypy.test.helpers as helpers
from mypy.test.helpers import _ensure_native_modules_available


def _fail_import(name: str, *args: object) -> types.ModuleType:
    raise ImportError(f"cannot import {name}: simulated wrong-interpreter failure")


class NativeEnvAvailabilitySuite(unittest.TestCase):
    def test_no_request_is_silent(self) -> None:
        # No requested gate: the import probe is never attempted.
        original = helpers.importlib.import_module
        helpers.importlib.import_module = _fail_import
        try:
            _ensure_native_modules_available()
        finally:
            helpers.importlib.import_module = original

    def test_missing_module_fails_loudly(self) -> None:
        original_env = helpers.os.environ.pop("TEST_NATIVE_TYPE_KERNEL", None)
        original = helpers.importlib.import_module
        helpers.os.environ["TEST_NATIVE_TYPE_KERNEL"] = "1"
        helpers.importlib.import_module = _fail_import
        try:
            with self.assertRaises(SystemExit) as ctx:
                _ensure_native_modules_available()
            self.assertIn("TEST_NATIVE_TYPE_KERNEL is set", str(ctx.exception))
        finally:
            helpers.importlib.import_module = original
            if original_env is None:
                del helpers.os.environ["TEST_NATIVE_TYPE_KERNEL"]
            else:
                helpers.os.environ["TEST_NATIVE_TYPE_KERNEL"] = original_env

    def test_missing_probe_attribute_fails_loudly(self) -> None:
        original_env = helpers.os.environ.pop("TEST_NATIVE_TYPE_KERNEL", None)
        original = helpers.importlib.import_module
        helpers.os.environ["TEST_NATIVE_TYPE_KERNEL"] = "1"
        # Module loads but lacks the expected function (stub package, #1285).
        helpers.importlib.import_module = lambda name, *args: types.SimpleNamespace()
        try:
            with self.assertRaises(SystemExit) as ctx:
                _ensure_native_modules_available()
            self.assertIn("loaded without `erase_type`", str(ctx.exception))
        finally:
            helpers.importlib.import_module = original
            if original_env is None:
                del helpers.os.environ["TEST_NATIVE_TYPE_KERNEL"]
            else:
                helpers.os.environ["TEST_NATIVE_TYPE_KERNEL"] = original_env


if __name__ == "__main__":
    unittest.main()
