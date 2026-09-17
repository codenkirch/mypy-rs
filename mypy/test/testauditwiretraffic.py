"""Guards for `misc/audit_wire_traffic.py` (#1820, #1821).

The audit counts seam calls with an in-process proxy, so a check that fans
out to workers makes it report 0 calls for every seam and still succeed.
Two guards close that: a fan-out refusal before any work starts, and a
non-zero exit when the run counted nothing at all. A third refuses when
`type_kernel` itself is unimportable, the one cause the hollow-zero
message names that would otherwise die in a bare `ModuleNotFoundError`
before the guard could run (#1821).

`main()` is driven with the kernel probe and `mypy.main.main` stubbed out,
so the guards are proven without a self-check. A guard that rejected every
run would be as useless as the hollow zero, so each refusal is paired with
a positive control that must return 0.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import os
import sys
import tempfile
import types
import unittest
from pathlib import Path
from typing import Any
from unittest import mock

import mypy.main

_AUDIT_PATH = Path(__file__).resolve().parents[2] / "misc" / "audit_wire_traffic.py"


def _load_audit_module() -> types.ModuleType:
    """A fresh module per test: the counters are module-level globals."""
    spec = importlib.util.spec_from_file_location("audit_wire_traffic_under_test", _AUDIT_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class AuditGuardHarness(unittest.TestCase):
    """Drive the audit's `main()` with the kernel and mypy stubbed out."""

    def setUp(self) -> None:
        self.audit = _load_audit_module()
        self.touched: list[str] = []
        self.argv_before: list[str] = []
        self.argv_after: list[str] = []
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def config_file(self, num_workers: int) -> str:
        path = Path(self.tmp.name) / "audit.ini"
        path.write_text(f"[mypy]\nnum_workers = {num_workers}\n")
        return str(path)

    def run_main(
        self, args: str, tally: int = 0, workers_env: str | None = None
    ) -> tuple[int, str]:
        """Run `main()` once. `tally` is the seam calls the stub run "made"."""

        def kernel_stub() -> int:
            self.touched.append("patch_kernel")
            return 3

        def serializer_stub() -> int:
            self.touched.append("patch_serializers")
            return 0

        def probe_stub() -> int:
            self.touched.append("patch_probes")
            return 0

        def mypy_stub(*args_: Any, **kwargs: Any) -> None:
            self.touched.append("mypy.main.main")
            if tally:
                self.audit.seam_calls["rust_stub_probe"] += tally

        err = io.StringIO()
        with mock.patch.dict(os.environ, clear=False):
            for key in ("MYPY_AUDIT_ARGS", "MYPY_NUM_WORKERS"):
                os.environ.pop(key, None)
            os.environ["MYPY_AUDIT_ARGS"] = args
            if workers_env is not None:
                os.environ["MYPY_NUM_WORKERS"] = workers_env
            self.argv_before = list(sys.argv)
            with (
                mock.patch.object(self.audit, "patch_kernel", kernel_stub),
                mock.patch.object(self.audit, "patch_serializers", serializer_stub),
                mock.patch.object(self.audit, "patch_probes", probe_stub),
                mock.patch.object(mypy.main, "main", mypy_stub),
                mock.patch.object(sys, "argv", list(sys.argv)),
                contextlib.redirect_stderr(err),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                rc = self.audit.main()
                self.argv_after = list(sys.argv)
        return rc, err.getvalue()


class FanoutGuardSuite(AuditGuardHarness):
    def test_explicit_n4_refuses_before_any_work(self) -> None:
        rc, err = self.run_main("-n4 --no-incremental -p mypy")
        self.assertEqual(rc, 1)
        self.assertIn("REFUSING TO RUN", err)
        self.assertIn("num_workers = 4", err)
        self.assertIn("'-n4'", err)
        self.assertIn("-n0", err)
        # The refusal precedes the probe and the run: nothing was patched,
        # mypy was never entered, and no report was printed.
        self.assertEqual(self.touched, [])
        self.assertNotIn("wire-waste audit", err)
        # And the argv is never rewritten to hide the caller's mistake.
        self.assertEqual(self.argv_after, self.argv_before)

    def test_every_fanout_spelling_refuses(self) -> None:
        for args in (
            "-n4 -p mypy",
            "-n 4 -p mypy",
            "--num-workers 2 -p mypy",
            "--num-workers=3 -p mypy",
            "-n1 -p mypy",
        ):
            with self.subTest(args=args):
                self.touched.clear()
                rc, err = self.run_main(args)
                self.assertEqual(rc, 1, f"{args}: {err}")
                self.assertIn("REFUSING TO RUN", err)
                self.assertEqual(self.touched, [])

    def test_parallel_config_without_an_override_refuses(self) -> None:
        cfg = self.config_file(4)
        rc, err = self.run_main(f"-p mypy --config-file {cfg}")
        self.assertEqual(rc, 1)
        self.assertIn("num_workers = 4", err)
        self.assertIn(cfg, err)
        self.assertEqual(self.touched, [])

    def test_parallel_env_without_an_override_refuses(self) -> None:
        # MYPY_NUM_WORKERS alone fans the run out (`mypy/main.py`).
        rc, err = self.run_main(f"-p mypy --config-file {self.config_file(0)}", workers_env="4")
        self.assertEqual(rc, 1)
        self.assertIn("MYPY_NUM_WORKERS", err)
        self.assertIn("num_workers = 4", err)
        self.assertEqual(self.touched, [])

    def test_unparseable_worker_value_refuses(self) -> None:
        rc, err = self.run_main(f"-n foo -p mypy --config-file {self.config_file(0)}")
        self.assertEqual(rc, 1)
        self.assertIn("not an integer", err)
        self.assertEqual(self.touched, [])

    def test_explicit_n0_beats_a_parallel_config(self) -> None:
        # Positive control for the config path: the same config that refuses
        # above is accepted once the command line pins -n0.
        cfg = self.config_file(4)
        rc, err = self.run_main(f"-n0 -p mypy --config-file {cfg}", tally=1)
        self.assertEqual(rc, 0, err)
        self.assertNotIn("REFUSING TO RUN", err)
        self.assertNotIn("HOLLOW ZERO", err)
        self.assertIn("wire-waste audit", err)
        # The argv that ran is the argv that was recorded: the caller's own
        # `--config-file` and `-n0` survive verbatim, nothing is rewritten.
        self.assertEqual(
            self.argv_after,
            [
                "mypy",
                "--config-file",
                "mypy_self_check.ini",
                "-n0",
                "-p",
                "mypy",
                "--config-file",
                cfg,
            ],
        )

    def test_last_worker_flag_wins(self) -> None:
        # argparse keeps the last value, so the guard must too.
        cfg = self.config_file(0)
        rc, err = self.run_main(f"-n4 -n0 -p mypy --config-file {cfg}", tally=1)
        self.assertEqual(rc, 0, err)
        self.assertNotIn("REFUSING TO RUN", err)

    def test_default_invocation_is_accepted_and_runs(self) -> None:
        rc, err = self.run_main("", tally=1)
        self.assertEqual(rc, 0, err)
        # Non-empty first: a guard regression must not read as an IndexError.
        self.assertTrue(self.touched, f"nothing ran: {err}")
        self.assertEqual(self.touched[0], "patch_kernel")
        self.assertIn("mypy.main.main", self.touched)


class HollowZeroGuardSuite(AuditGuardHarness):
    def test_zero_calls_fail_after_the_report_is_printed(self) -> None:
        cfg = self.config_file(0)
        rc, err = self.run_main(f"-n0 -p mypy --config-file {cfg}", tally=0)
        self.assertEqual(rc, 1)
        self.assertEqual(
            self.touched, ["patch_kernel", "patch_serializers", "patch_probes", "mypy.main.main"]
        )
        # The partial report is printed, then the run fails: the numbers stay
        # visible, but they cannot be read as evidence.
        self.assertIn("wire-waste audit", err)
        self.assertIn("HOLLOW ZERO", err)
        self.assertLess(err.index("wire-waste audit"), err.index("HOLLOW ZERO"))
        self.assertIn("0 calls across 0 registered seam(s)", err)
        self.assertIn("-n0", err)

    def test_a_raising_run_still_reads_the_tally(self) -> None:
        # Accounting must not be bypassed on the abnormal path: the audited
        # run raising must not skip the guard.
        cfg = self.config_file(0)

        def raising_stub(*args_: Any, **kwargs: Any) -> None:
            self.touched.append("mypy.main.main")
            raise RuntimeError("audited run died")

        err = io.StringIO()
        with mock.patch.dict(os.environ, clear=False):
            for key in ("MYPY_AUDIT_ARGS", "MYPY_NUM_WORKERS"):
                os.environ.pop(key, None)
            os.environ["MYPY_AUDIT_ARGS"] = f"-n0 -p mypy --config-file {cfg}"
            with (
                mock.patch.object(self.audit, "patch_kernel", lambda: 0),
                mock.patch.object(self.audit, "patch_serializers", lambda: 0),
                mock.patch.object(self.audit, "patch_probes", lambda: 0),
                mock.patch.object(mypy.main, "main", raising_stub),
                mock.patch.object(sys, "argv", list(sys.argv)),
                contextlib.redirect_stderr(err),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                rc = self.audit.main()
        output = err.getvalue()
        self.assertEqual(rc, 1)
        self.assertIn("RuntimeError: audited run died", output)
        self.assertIn("HOLLOW ZERO", output)


class _MissingTypeKernel:
    """Meta-path finder that makes `import type_kernel` fail, nothing else."""

    def find_spec(self, name: str, path: Any = None, target: Any = None) -> None:
        if name == "type_kernel":
            raise ModuleNotFoundError(f"No module named {name!r}", name=name)
        return


class MissingKernelGuardSuite(AuditGuardHarness):
    """The real `patch_kernel` against an unimportable extension (#1821)."""

    def test_missing_kernel_refuses_with_the_remedy_before_any_work(self) -> None:
        # Unlike the other suites, `patch_kernel` is NOT stubbed: the real
        # import runs and fails, which is the defect #1821 reports.
        cfg = self.config_file(0)

        def serializer_stub() -> int:
            self.touched.append("patch_serializers")
            return 0

        def probe_stub() -> int:
            self.touched.append("patch_probes")
            return 0

        def mypy_stub(*args_: Any, **kwargs: Any) -> None:
            self.touched.append("mypy.main.main")

        err = io.StringIO()
        with mock.patch.dict(os.environ, clear=False):
            for key in ("MYPY_AUDIT_ARGS", "MYPY_NUM_WORKERS"):
                os.environ.pop(key, None)
            os.environ["MYPY_AUDIT_ARGS"] = f"-n0 -p mypy --config-file {cfg}"
            with (
                mock.patch.object(self.audit, "patch_serializers", serializer_stub),
                mock.patch.object(self.audit, "patch_probes", probe_stub),
                mock.patch.object(mypy.main, "main", mypy_stub),
                mock.patch.object(sys, "meta_path", [_MissingTypeKernel(), *sys.meta_path]),
                mock.patch.object(sys, "argv", list(sys.argv)),
                contextlib.redirect_stderr(err),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                rc = self.audit.main()
        output = err.getvalue()
        # Fail loud: non-zero, a named refusal, and the remedy the bare
        # `ModuleNotFoundError` never carried.
        self.assertEqual(rc, 1, output)
        self.assertIn("MISSING type_kernel EXTENSION (#1821)", output)
        self.assertIn("cause: ModuleNotFoundError: No module named 'type_kernel'", output)
        self.assertIn(
            "/private/tmp/mypy-rs-<lane>-tk"
            ":/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver",
            output,
        )
        # The stale shared-venv stub is why the remedy is not "just run it".
        self.assertIn("#1800", output)
        # A refusal, not a hollow-zero report: nothing ran, nothing is
        # registered, and no report was printed.
        self.assertEqual(self.touched, [])
        self.assertEqual(self.audit.registered_seams, set())
        self.assertNotIn("wire-waste audit", output)


class GuardPolicySuite(unittest.TestCase):
    """The policies called directly: no `main()`, no kernel, no run."""

    def setUp(self) -> None:
        self.audit = _load_audit_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def config_body(self, body: str, name: str = "audit.ini") -> str:
        path = Path(self.tmp.name) / name
        path.write_text(body)
        return str(path)

    def test_zero_worker_spellings_are_accepted(self) -> None:
        cfg = self.config_body("[mypy]\nnum_workers = 0\n")
        for extra in (["-n0"], ["-n", "0"], ["--num-workers", "0"], ["--num-workers=0"]):
            with self.subTest(extra=extra):
                argv = ["mypy", "--config-file", cfg, *extra]
                self.assertIsNone(self.audit.fanout_reason(argv, {"MYPY_NUM_WORKERS": "0"}))

    def test_absent_config_key_is_not_a_fanout(self) -> None:
        cfg = self.config_body("[mypy]\nstrict = True\n")
        self.assertIsNone(self.audit.fanout_reason(["mypy", "--config-file", cfg], {}))

    def test_argv_wins_over_env_and_config(self) -> None:
        cfg = self.config_body("[mypy]\nnum_workers = 4\n")
        argv = ["mypy", "--config-file", cfg, "-n0", "-p", "mypy"]
        self.assertIsNone(self.audit.fanout_reason(argv, {"MYPY_NUM_WORKERS": "4"}))

    def test_config_path_comes_from_argv(self) -> None:
        # The guard reads the config mypy will read, not a hardcoded name.
        parallel = self.config_body("[mypy]\nnum_workers = 2\n", "parallel.ini")
        sequential = self.config_body("[mypy]\nnum_workers = 0\n", "sequential.ini")
        self.assertIsNone(self.audit.fanout_reason(["mypy", "--config-file", sequential], {}))
        reason = self.audit.fanout_reason(["mypy", "--config-file", parallel], {})
        assert reason is not None
        self.assertIn(parallel, reason)
        self.assertIn("num_workers = 2", reason)

    def test_missing_config_is_left_to_mypy(self) -> None:
        # mypy errors on a missing --config-file itself; the guard must not
        # invent a fan-out it cannot see.
        missing = str(Path(self.tmp.name) / "absent.ini")
        self.assertIsNone(self.audit.fanout_reason(["mypy", "--config-file", missing], {}))

    def test_hollow_zero_reason_reads_the_tally(self) -> None:
        self.assertIsNone(self.audit.hollow_zero_reason(1, 12))
        self.assertIsNone(self.audit.hollow_zero_reason(17_584, 155))
        message = self.audit.hollow_zero_reason(0, 12)
        assert message is not None
        self.assertIn("0 calls across 12 registered seam(s)", message)
        self.assertIn("PYTHONPATH", message)
        self.assertIn("-n0", message)

    def test_pinned_invocation_keeps_n0(self) -> None:
        # The docstring's `-n0` is load-bearing (#1820), so it stays pinned.
        self.assertIn("-n0", self.audit.audit_argv(None))
        self.assertEqual(self.audit.audit_argv("-n0 -p mypy")[3:], ["-n0", "-p", "mypy"])
        self.assertEqual(self.audit.audit_argv("-p mypy")[3:], ["-p", "mypy"])

    def test_missing_kernel_reason_keeps_a_different_cause(self) -> None:
        # A transitive ImportError must stay diagnosable: the message carries
        # the cause verbatim rather than flattening it to "no type_kernel".
        message = self.audit.missing_kernel_reason(
            ModuleNotFoundError("No module named 'librt'", name="librt")
        )
        self.assertIn("cause: ModuleNotFoundError: No module named 'librt'", message)
        self.assertIn("type_kernel", message)


if __name__ == "__main__":
    unittest.main()
