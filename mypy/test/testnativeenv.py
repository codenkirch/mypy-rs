"""Unit tests for the loud native-gate availability check (#1285)."""

from __future__ import annotations

import os
import types
import unittest
from unittest import mock

from mypy.test.helpers import _ensure_native_modules_available


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


if __name__ == "__main__":
    unittest.main()
