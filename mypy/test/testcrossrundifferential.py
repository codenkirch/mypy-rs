"""Tests for `scripts/crossrun_differential.py` (#1770 section 4.3).

The runner compares two mypy configurations of one corpus from the outside,
because the checker is not idempotent and an in-run comparison would
double-apply its writes. Two properties are load-bearing and both are tested
here directly, not by inspection:

* a mismatch is reported under the leg that broke, with the artifact named;
* a degenerate comparison (an empty or truncated artifact, an arm that never
  reached the leg) fails instead of passing as agreement.

Every guard has a positive control next to it, since an assertion that rejects
everything is as useless as one that cannot fail.
"""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

from mypy.test.config import PREFIX

RUNNER = Path(PREFIX) / "scripts" / "crossrun_differential.py"
CORPUS = Path(PREFIX) / "scripts" / "crossrun_corpus"


def _load_runner() -> Any:
    spec = importlib.util.spec_from_file_location("crossrun_differential", RUNNER)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    # `dataclasses` resolves `cls.__module__` through `sys.modules`, so the
    # module has to be registered before it is executed.
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


runner = _load_runner()


def _write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=1, sort_keys=True), encoding="utf8")


def _make_run(root: Path, name: str, files: dict[str, Any]) -> dict[str, Any]:
    """Write `files` under `<root>/<name>/dump/` and return a run record."""
    arm_dir = root / name
    for relative, value in files.items():
        path = arm_dir / "dump" / relative
        if isinstance(value, str):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(value, encoding="utf8")
        else:
            _write_json(path, value)
    status = {
        "legs": {leg: {"ok": True, "detail": ""} for leg in runner.WORKER_LEGS},
        "stage": "done",
        "error": None,
    }
    return {
        "arm": name,
        "dir": arm_dir,
        "log": arm_dir / "worker.log",
        "returncode": 0,
        "seconds": 0.0,
        "status": status,
    }


def _gate(
    requested: int = 0,
    declared: int | None = None,
    in_force: int | None = None,
    default: int = 0,
    effective: int | None = None,
    **counters: int,
) -> dict[str, Any]:
    """One gate's provenance: the env value wins over the option default.

    `requested` holds the env value the worker saw ("0" when the env gate is
    unset), `default` the production serving default (#1860), and `effective`
    their resolution: the env value when present, else the default. Tests that
    need an env "0" overriding a default pass `effective` explicitly.
    """
    if effective is None:
        effective = requested if requested != 0 else default
    return {
        "declared": requested if declared is None else declared,
        "requested": requested,
        "default": default,
        "effective": effective,
        "in_force": effective if in_force is None else in_force,
        "counters": {"consulted": 0, "mismatched": 0, **counters},
    }


def _provenance(arm: str, **overrides: Any) -> dict[str, Any]:
    """A provenance record that passes every kernel-leg guard."""
    gates = {gate: _gate() for gate in runner.FLIP_GATES}
    record: dict[str, Any] = {
        "arm": arm,
        "tree": "/tree",
        "tree_ok": True,
        "mypy_file": "/tree/mypy/__init__.py",
        "kernel_file": "/ext/type_kernel.so",
        "python": "/venv/bin/python",
        "corpus_files": ["a.py"],
        "num_workers": 0,
        "gates": gates,
        "effective_options": {"native_type_kernel": True},
    }
    record.update(overrides)
    return record


def _typemap_summary(**overrides: Any) -> dict[str, Any]:
    summary: dict[str, Any] = {
        "checked": 2,
        "unchecked": [],
        "empty_maps": [],
        "entries": 10,
        "captured_entries": 10,
        "export_types_total": 10,
        "invariant_violations": [],
    }
    summary.update(overrides)
    return summary


def _deferral_build(**overrides: Any) -> dict[str, Any]:
    stats: dict[str, Any] = {
        "checked": 2,
        "modules": {},
        "last_pass": 2,
        "last_pass_values": {"2": 2},
        "second_pass_modules": ["a"],
        "second_pass_total": 1,
        "deferred_nodes_remaining": 0,
    }
    stats.update(overrides)
    return stats


def _deferral_daemon(**overrides: Any) -> dict[str, Any]:
    stats: dict[str, Any] = {
        "mutated_file": "a.py",
        "initialize": {"status": 0, "messages": ["one"]},
        "increment": {
            "status": 0,
            "messages": ["two"],
            "budgets": {"2": 0, "3": 1},
            "second_pass_calls": 1,
            "updated_modules": ["a"],
            "changed_modules": ["a"],
        },
    }
    stats.update(overrides)
    return stats


class ArmTokenSuite(unittest.TestCase):
    """An arm is (env vars, Options fields, mypy flags) and nothing else."""

    def test_each_prefix_parses(self) -> None:
        self.assertEqual(
            runner.parse_arm_token("env:MYPY_TK_NODE_READ_FLIP=2"),
            ("env", "MYPY_TK_NODE_READ_FLIP", "2"),
        )
        self.assertEqual(
            runner.parse_arm_token("opt:native_ast_mirror=1"), ("opt", "native_ast_mirror", "1")
        )
        self.assertEqual(
            runner.parse_arm_token("flag:--no-native-type-kernel"),
            ("flag", "--no-native-type-kernel", ""),
        )

    def test_malformed_tokens_are_refused(self) -> None:
        for token in (
            "MYPY_TK_NODE_READ_FLIP=2",
            "env:MYPY_TK_NODE_READ_FLIP",
            "env:=2",
            "flag:+",
        ):
            with self.subTest(token=token):
                with self.assertRaises(runner.UsageError):
                    runner.parse_arm_token(token)

    def test_unknown_prefix_is_refused(self) -> None:
        with self.assertRaises(runner.UsageError):
            runner.parse_arm_token("file:thing=1")

    def test_flips_preset_declares_both_states_of_every_gate(self) -> None:
        off, serve = runner.preset("flips")
        for gate in runner.FLIP_GATES:
            self.assertEqual(off.env_dict()[gate], "0")
            self.assertEqual(serve.env_dict()[gate], "2")
        self.assertEqual(dict(off.options), {"native_ast_mirror": "1"})
        self.assertEqual(dict(serve.options), {"native_ast_mirror": "1"})

    def test_kernel_preset_differs_only_by_the_kernel_flag(self) -> None:
        on, off = runner.preset("kernel")
        self.assertNotEqual(on.declared(), off.declared())
        self.assertEqual(on.flags, ())
        self.assertEqual(off.flags, ("--no-native-type-kernel",))

    def test_unknown_preset_is_refused(self) -> None:
        with self.assertRaises(runner.UsageError):
            runner.preset("everything")

    def test_leg_selection_keeps_registry_order(self) -> None:
        self.assertEqual(
            runner.resolve_legs("suites,errors", runner.ALL_LEGS), ("errors", "suites")
        )

    def test_leg_selection_refuses_unknown_and_empty(self) -> None:
        with self.assertRaises(runner.UsageError):
            runner.resolve_legs("errors,everything", runner.ALL_LEGS)
        with self.assertRaises(runner.UsageError):
            runner.resolve_legs("", runner.ALL_LEGS)
        self.assertEqual(runner.resolve_legs("", runner.OPTIONAL_LEGS, allow_empty=True), ())

    def test_ambient_gates_are_stripped_and_declared_ones_applied(self) -> None:
        base = {
            "PATH": "/bin",
            "MYPY_TK_NODE_READ_FLIP": "2",
            "TEST_NATIVE_TYPE_KERNEL": "1",
            "MYPY_NUM_WORKERS": "4",
            "PYTHONPATH": "/somewhere",
        }
        stripped_base, stripped = runner.split_ambient(base)
        arm = runner.arm_from_tokens("a", ["env:MYPY_TK_STMT_READ_FLIP=1"])
        env = runner.arm_environment(stripped_base, arm, ["/ext"])
        self.assertEqual(env["MYPY_TK_STMT_READ_FLIP"], "1")
        self.assertNotIn("MYPY_TK_NODE_READ_FLIP", env)
        self.assertNotIn("TEST_NATIVE_TYPE_KERNEL", env)
        self.assertNotIn("MYPY_NUM_WORKERS", env)
        self.assertEqual(env["PYTHONPATH"], os.pathsep.join(["/ext", "/somewhere"]))
        self.assertIn("MYPY_TK_NODE_READ_FLIP", stripped)

    def test_two_arms_do_not_share_env_tokens(self) -> None:
        """Regression for the leak: arm B must not inherit arm A's env tokens.

        One `arm_environment` call built both arms' environments before, so an
        `env:` token only arm A declared decided arm B and the kernel leg read
        the leaked value back as if arm B had declared it.
        """
        left = runner.arm_from_tokens("left", ["env:MYPY_TK_NODE_READ_FLIP=2"])
        right = runner.arm_from_tokens("right", ["env:MYPY_TK_STMT_READ_FLIP=2"])
        ambient, _ = runner.split_ambient({})
        env_left = runner.arm_environment(ambient, left, [])
        env_right = runner.arm_environment(ambient, right, [])
        self.assertEqual(env_left["MYPY_TK_NODE_READ_FLIP"], "2")
        self.assertNotIn("MYPY_TK_NODE_READ_FLIP", env_right)
        self.assertEqual(env_right["MYPY_TK_STMT_READ_FLIP"], "2")
        self.assertNotIn("MYPY_TK_STMT_READ_FLIP", env_left)

    def test_declared_gate_survives_the_strip(self) -> None:
        base = {"MYPY_TK_NODE_READ_FLIP": "2"}
        stripped_base, stripped = runner.split_ambient(base)
        arm = runner.arm_from_tokens("a", ["env:MYPY_TK_NODE_READ_FLIP=0"])
        env = runner.arm_environment(stripped_base, arm, [])
        self.assertEqual(env["MYPY_TK_NODE_READ_FLIP"], "0")
        self.assertIn("MYPY_TK_NODE_READ_FLIP", stripped)


class ArtifactSuite(unittest.TestCase):
    """Artifacts are read back and refused when they cannot be evidence."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_missing_and_empty_artifacts_are_refused(self) -> None:
        path = self.root / "nothing.json"
        _, refusal = runner.read_json_artifact(path, "a")
        self.assertIsNotNone(refusal)
        path.write_text("   \n", encoding="utf8")
        _, refusal = runner.read_json_artifact(path, "a")
        self.assertIsNotNone(refusal)
        path.write_text("{not json", encoding="utf8")
        _, refusal = runner.read_json_artifact(path, "a")
        self.assertIsNotNone(refusal)
        path.write_text("[]", encoding="utf8")
        value, refusal = runner.read_json_artifact(path, "a")
        self.assertIsNone(refusal)
        self.assertEqual(value, [])

    def test_identical_dump_dirs_agree(self) -> None:
        for side in ("a", "b"):
            _write_json(self.root / side / "m.json", {"entries": [1, 2]})
        comparison, count = runner.compare_dump_dirs(
            "ast", self.root / "a", self.root / "b", "trees"
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)
        self.assertEqual(count, 1)

    def test_a_mutated_module_dump_is_a_mismatch(self) -> None:
        _write_json(self.root / "a" / "m.json", {"entries": [1, 2]})
        _write_json(self.root / "b" / "m.json", {"entries": [1, 3]})
        comparison, count = runner.compare_dump_dirs(
            "ast", self.root / "a", self.root / "b", "trees"
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_MISMATCH)
        self.assertEqual(count, 1)
        self.assertIn("m.json", comparison.detail)

    def test_a_module_dumped_on_one_side_only_is_a_mismatch(self) -> None:
        _write_json(self.root / "a" / "m.json", {"entries": []})
        _write_json(self.root / "b" / "m.json", {"entries": []})
        _write_json(self.root / "b" / "extra.json", {"entries": []})
        comparison, _ = runner.compare_dump_dirs("ast", self.root / "a", self.root / "b", "trees")
        self.assertEqual(comparison.verdict, runner.VERDICT_MISMATCH)
        self.assertIn("extra.json", comparison.detail)

    def test_two_empty_dump_dirs_are_degenerate_not_agreement(self) -> None:
        (self.root / "a").mkdir()
        (self.root / "b").mkdir()
        comparison, _ = runner.compare_dump_dirs("ast", self.root / "a", self.root / "b", "trees")
        self.assertEqual(comparison.verdict, runner.VERDICT_DEGENERATE)

    def test_an_empty_side_is_degenerate_even_when_the_other_has_content(self) -> None:
        (self.root / "a").mkdir()
        _write_json(self.root / "b" / "m.json", {"entries": []})
        comparison, _ = runner.compare_dump_dirs("ast", self.root / "a", self.root / "b", "trees")
        self.assertEqual(comparison.verdict, runner.VERDICT_DEGENERATE)

    def test_a_zero_byte_module_dump_is_degenerate(self) -> None:
        (self.root / "a").mkdir()
        (self.root / "b").mkdir()
        for side in ("a", "b"):
            (self.root / side / "m.json").write_text("", encoding="utf8")
        comparison, _ = runner.compare_dump_dirs("ast", self.root / "a", self.root / "b", "trees")
        self.assertEqual(comparison.verdict, runner.VERDICT_DEGENERATE)

    def test_first_difference_points_at_the_line(self) -> None:
        self.assertIn("line 2", runner.first_difference("a\nb\n", "a\nc\n"))

    def test_diagnose_list_names_the_first_difference(self) -> None:
        self.assertIn("message 1", runner.diagnose_list(["x", "y"], ["x", "z"]))
        self.assertIn("counts differ", runner.diagnose_list(["x"], ["x", "y"]))
        self.assertIn("equal element-wise", runner.diagnose_list(["x"], ["x"]))

    def test_normalize_text_erases_the_scratch_roots(self) -> None:
        text = "/arm/corpus/a.py and /tree/mypy/x.py"
        normalized = runner.normalize_text(text, Path("/arm"), Path("/tree"))
        self.assertEqual(normalized, "<ARM>/corpus/a.py and <TREE>/mypy/x.py")


class ErrorLegSuite(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def _runs(self, left: Any, right: Any) -> dict[str, Any]:
        return {
            "a": _make_run(self.root, "a", {"errors.json": left}),
            "b": _make_run(self.root, "b", {"errors.json": right}),
        }

    def test_identical_output_agrees(self) -> None:
        runs = self._runs(["m1", "m2"], ["m1", "m2"])
        comparison = runner.compare_errors_leg(runs, ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)

    def test_an_extra_message_is_a_mismatch(self) -> None:
        runs = self._runs(["m1"], ["m1", "m2"])
        comparison = runner.compare_errors_leg(runs, ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_MISMATCH)
        self.assertIn("counts differ", comparison.detail)

    def test_empty_output_on_both_sides_is_degenerate(self) -> None:
        runs = self._runs([], [])
        comparison = runner.compare_errors_leg(runs, ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_DEGENERATE)

    def test_a_missing_artifact_never_reads_as_agreement(self) -> None:
        runs = self._runs(["m1"], ["m1"])
        (self.root / "b" / "dump" / "errors.json").unlink()
        comparison = runner.compare_errors_leg(runs, ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_DEGENERATE)


class KernelLegSuite(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def _runs(self, left: dict[str, Any], right: dict[str, Any]) -> dict[str, Any]:
        return {
            "a": _make_run(self.root, "a", {"provenance.json": left}),
            "b": _make_run(self.root, "b", {"provenance.json": right}),
        }

    def test_matching_provenance_agrees(self) -> None:
        comparison = runner.compare_kernel_leg(
            self._runs(_provenance("a"), _provenance("b")), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)

    def test_an_inert_gate_is_not_run(self) -> None:
        left = _provenance("a")
        left["gates"]["MYPY_TK_NODE_READ_FLIP"] = _gate(requested=2, in_force=0)
        comparison = runner.compare_kernel_leg(self._runs(left, _provenance("b")), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("is inert", comparison.detail)

    def test_an_option_default_gate_in_force_agrees(self) -> None:
        """A gate armed only by the option default (#1860) is not inert.

        An arm carrying no env token serves mode 1 through the production
        default, so the leg must compare it, not call it inert.
        """
        left = _provenance("a")
        left["gates"]["MYPY_TK_NODE_READ_FLIP"] = _gate(
            requested=0, default=1, effective=1, in_force=1, served=3, consulted=3
        )
        comparison = runner.compare_kernel_leg(self._runs(left, _provenance("b")), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)
        # The AGREE path never emits the NOT_RUN clause; what it can emit
        # for an armed gate nobody consulted is "ARMED BUT INERT", and a
        # consulted gate (consulted=3 here) must keep even that out.
        self.assertNotIn("INERT", comparison.detail)

    def test_an_inert_option_default_gate_is_not_run(self) -> None:
        left = _provenance("a")
        left["gates"]["MYPY_TK_NODE_READ_FLIP"] = _gate(
            requested=0, default=1, effective=1, in_force=0
        )
        comparison = runner.compare_kernel_leg(self._runs(left, _provenance("b")), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("is inert", comparison.detail)
        self.assertIn("option default 1", comparison.detail)

    def test_a_declared_env_zero_beats_the_option_default(self) -> None:
        left = _provenance("a")
        left["gates"]["MYPY_TK_NODE_READ_FLIP"] = _gate(
            requested=0, default=1, effective=0, in_force=0
        )
        comparison = runner.compare_kernel_leg(self._runs(left, _provenance("b")), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)
        self.assertNotIn("is inert", comparison.detail)

    def test_the_statement_option_default_in_force_agrees(self) -> None:
        """The statement gate's production default (#1869) is not inert.

        An arm carrying no env token serves mode 1 through the wiring's
        recorded decision, so the leg must compare it, not call it inert.
        """
        left = _provenance("a")
        left["gates"]["MYPY_TK_STMT_READ_FLIP"] = _gate(
            requested=0, default=1, effective=1, in_force=1, served=2, consulted=2
        )
        comparison = runner.compare_kernel_leg(self._runs(left, _provenance("b")), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)
        self.assertNotIn("INERT", comparison.detail)

    def test_an_inert_statement_option_default_gate_is_not_run(self) -> None:
        left = _provenance("a")
        left["gates"]["MYPY_TK_STMT_READ_FLIP"] = _gate(
            requested=0, default=1, effective=1, in_force=0
        )
        comparison = runner.compare_kernel_leg(self._runs(left, _provenance("b")), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("is inert", comparison.detail)
        self.assertIn("option default 1", comparison.detail)

    def test_a_declared_env_zero_beats_the_statement_option_default(self) -> None:
        left = _provenance("a")
        left["gates"]["MYPY_TK_STMT_READ_FLIP"] = _gate(
            requested=0, default=1, effective=0, in_force=0
        )
        comparison = runner.compare_kernel_leg(self._runs(left, _provenance("b")), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)
        self.assertNotIn("is inert", comparison.detail)

    def test_an_undeclared_channel_is_not_run(self) -> None:
        """Regression for the env leak: a served mode nothing declared fails.

        An arm whose process environment holds a gate value it never declared
        cannot have its mode in force attributed to its configuration, so the
        run must fail rather than report the leaked value as agreement.
        """
        left = _provenance("a")
        left["gates"]["MYPY_TK_NODE_READ_FLIP"] = _gate(requested=2, declared=0)
        comparison = runner.compare_kernel_leg(self._runs(left, _provenance("b")), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("undeclared channel", comparison.detail)

    def test_a_fanned_out_arm_is_not_run(self) -> None:
        comparison = runner.compare_kernel_leg(
            self._runs(_provenance("a", num_workers=4), _provenance("b")), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("hollow", comparison.detail)

    def test_a_foreign_tree_is_not_run(self) -> None:
        comparison = runner.compare_kernel_leg(
            self._runs(_provenance("a", tree_ok=False), _provenance("b")), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("outside the tree", comparison.detail)

    def test_a_mismatched_in_run_counter_is_not_run(self) -> None:
        left = _provenance("a")
        left["gates"]["MYPY_TK_VAR_KEY_FLIP"]["counters"] = {"consulted": 5, "mismatched": 3}
        comparison = runner.compare_kernel_leg(self._runs(left, _provenance("b")), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("mismatch", comparison.detail)

    def test_an_armed_but_inert_gate_is_reported(self) -> None:
        left = _provenance("a")
        right = _provenance("b")
        for record in (left, right):
            record["gates"]["MYPY_TK_NODE_READ_FLIP"] = _gate(requested=2)
        comparison = runner.compare_kernel_leg(self._runs(left, right), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)
        self.assertIn("ARMED BUT INERT", comparison.detail)


class TypemapLegSuite(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def _runs(self, left: dict[str, Any], right: dict[str, Any]) -> dict[str, Any]:
        runs = {}
        for name, summary in (("a", left), ("b", right)):
            runs[name] = _make_run(self.root, name, {"typemap-summary.json": summary})
            _write_json(self.root / name / "dump" / "typemap" / "m.json", {"module": "m"})
        return runs

    def test_matching_maps_agree(self) -> None:
        comparison = runner.compare_typemap_leg(
            self._runs(_typemap_summary(), _typemap_summary()), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)

    def test_an_invariant_violation_is_not_run(self) -> None:
        left = _typemap_summary(invariant_violations=["m: 2 type maps"])
        comparison = runner.compare_typemap_leg(self._runs(left, _typemap_summary()), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("invariant", comparison.detail)

    def test_an_undercounting_capture_is_not_run(self) -> None:
        left = _typemap_summary(captured_entries=9)
        comparison = runner.compare_typemap_leg(self._runs(left, _typemap_summary()), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("missed a module", comparison.detail)

    def test_no_checked_module_is_degenerate(self) -> None:
        left = _typemap_summary(checked=0, entries=0, captured_entries=0, export_types_total=0)
        comparison = runner.compare_typemap_leg(self._runs(left, _typemap_summary()), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_DEGENERATE)


class DeferralLegSuite(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def _runs(self, left: dict[str, Any], right: dict[str, Any]) -> dict[str, Any]:
        return {
            "a": _make_run(self.root, "a", {"deferral-build.json": left}),
            "b": _make_run(self.root, "b", {"deferral-build.json": right}),
        }

    def test_matching_budgets_agree(self) -> None:
        comparison = runner.compare_deferral_build_leg(
            self._runs(_deferral_build(), _deferral_build()), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)

    def test_no_second_pass_is_degenerate(self) -> None:
        left = _deferral_build(second_pass_total=0, second_pass_modules=[])
        comparison = runner.compare_deferral_build_leg(
            self._runs(left, _deferral_build()), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_DEGENERATE)
        self.assertIn("vacuous", comparison.detail)

    def test_an_unexpected_budget_is_not_run(self) -> None:
        left = _deferral_build(last_pass_values={"2": 1, "3": 1})
        comparison = runner.compare_deferral_build_leg(
            self._runs(left, _deferral_build()), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("last_pass", comparison.detail)

    def test_a_changed_budget_is_a_mismatch(self) -> None:
        left = _deferral_build(second_pass_modules=["a", "b"], second_pass_total=2)
        comparison = runner.compare_deferral_build_leg(
            self._runs(left, _deferral_build()), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_MISMATCH)


class DaemonLegSuite(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def _runs(self, left: dict[str, Any], right: dict[str, Any]) -> dict[str, Any]:
        return {
            "a": _make_run(self.root, "a", {"deferral-daemon.json": left}),
            "b": _make_run(self.root, "b", {"deferral-daemon.json": right}),
        }

    def test_matching_increments_agree(self) -> None:
        comparison = runner.compare_deferral_daemon_leg(
            self._runs(_deferral_daemon(), _deferral_daemon()), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)

    def test_an_unspent_daemon_budget_is_not_run(self) -> None:
        left = _deferral_daemon()
        left["increment"]["budgets"] = {"2": 1}
        comparison = runner.compare_deferral_daemon_leg(
            self._runs(left, _deferral_daemon()), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertIn("unmeasured", comparison.detail)

    def test_changed_daemon_output_is_a_mismatch(self) -> None:
        right = _deferral_daemon()
        right["increment"]["messages"] = ["three"]
        comparison = runner.compare_deferral_daemon_leg(
            self._runs(_deferral_daemon(), right), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_MISMATCH)
        self.assertIn("output differs", comparison.detail)


class SuitesLegSuite(unittest.TestCase):
    def test_pytest_lines_are_parsed_and_prose_is_not(self) -> None:
        line = "mypy/test/testfinegrained.py::FineGrainedSuite::testFoo PASSED [ 12%]"
        self.assertEqual(
            runner.parse_pytest_line(line),
            ("mypy/test/testfinegrained.py::FineGrainedSuite::testFoo", "PASSED"),
        )
        for junk in (
            "no pytest markers here",
            "====== 3 passed in 1.2s ======",
            "mypy/test/x.py::T::t RUNNING",
        ):
            self.assertEqual(runner.parse_pytest_line(junk), (None, None))

    def _record(self, outcomes: dict[str, dict[str, str]], **overrides: Any) -> dict[str, Any]:
        record: dict[str, Any] = {
            "ok": True,
            "detail": "",
            "returncode": 0,
            "suites": outcomes,
            "gates": {},
            "log": "/log",
        }
        record.update(overrides)
        return record

    def test_identical_outcomes_agree(self) -> None:
        outcomes = {"mypy/test/testfinegrained.py": {"a::b": "PASSED"}}
        comparison = runner.compare_suites_leg(
            self._record(outcomes), self._record(outcomes), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_AGREE)

    def test_a_changed_outcome_is_a_mismatch(self) -> None:
        left = {"mypy/test/testfinegrained.py": {"a::b": "PASSED"}}
        right = {"mypy/test/testfinegrained.py": {"a::b": "FAILED"}}
        comparison = runner.compare_suites_leg(self._record(left), self._record(right), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_MISMATCH)
        self.assertIn("changed outcome", comparison.detail)

    def test_a_collection_error_is_not_run(self) -> None:
        outcomes = {"mypy/test/testfinegrained.py": {"a::b": "PASSED"}}
        comparison = runner.compare_suites_leg(
            self._record(outcomes, returncode=2), self._record(outcomes), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)

    def test_no_collected_tests_is_not_run(self) -> None:
        comparison = runner.compare_suites_leg(
            self._record({}, returncode=5), self._record({}), ("a", "b")
        )
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)

    def test_an_untranslatable_token_is_not_run(self) -> None:
        arm = runner.arm_from_tokens("a", ["flag:--warn-unreachable"])
        record = self._record({})
        record["ok"] = False
        comparison = runner.compare_suites_leg(record, self._record({}), ("a", "b"))
        self.assertEqual(comparison.verdict, runner.VERDICT_NOT_RUN)
        self.assertEqual(runner.suite_environment(arm)[1], ["flag:--warn-unreachable"])

    def test_native_tokens_translate_to_harness_gates(self) -> None:
        arm = runner.arm_from_tokens(
            "a",
            [
                "opt:native_ast_mirror=1",
                "opt:native_ast_mirror_stmt_read=1",
                "flag:--no-native-type-kernel",
            ],
        )
        translated, unmappable = runner.suite_environment(arm)
        self.assertEqual(unmappable, [])
        self.assertEqual(translated["TEST_NATIVE_AST_MIRROR"], "1")
        self.assertEqual(translated["TEST_NATIVE_AST_MIRROR_STMT_READ"], "1")
        self.assertEqual(translated["TEST_NATIVE_TYPE_KERNEL"], "0")


class ReportSuite(unittest.TestCase):
    def _report(self, corpora: dict[str, Path], results: list[Any]) -> str:
        arms = (runner.arm_from_tokens("a", []), runner.arm_from_tokens("b", []))
        runs: dict[str, dict[str, Any]] = {
            name: {
                "arm": name,
                "dir": Path("/out") / name,
                "log": Path("/log"),
                "returncode": 0,
                "seconds": 0.0,
                "status": {"legs": {}, "stage": "done", "error": None},
            }
            for name in ("a", "b")
        }
        return str(
            runner.render_report(
                arms, runs, results, ("errors",), corpora, Path("/tree"), [], Path("/out")
            )
        )

    def test_a_failed_leg_is_named_in_the_verdict(self) -> None:
        report = self._report(
            {"a": CORPUS, "b": CORPUS},
            [runner.Comparison("errors", runner.VERDICT_MISMATCH, "1 vs 2 messages")],
        )
        self.assertIn("errors", report)
        self.assertIn("mismatch", report)
        self.assertIn("1 of 1 compared leg(s) failed", report)

    def test_an_unselected_leg_reports_as_skipped(self) -> None:
        report = self._report({"a": CORPUS, "b": CORPUS}, [])
        self.assertIn("skipped", report)

    def test_divergent_corpora_raise_the_control_banner(self) -> None:
        report = self._report({"a": CORPUS, "b": Path("/somewhere/else")}, [])
        self.assertIn("CONTROL RUN", report)


class UsageSuite(unittest.TestCase):
    """`main` refuses a configuration it cannot report honestly about."""

    def _run_main(self, argv: list[str]) -> int:
        return int(runner.main(argv))

    def test_one_sided_arm_tokens_are_refused(self) -> None:
        self.assertEqual(self._run_main(["--a", "env:MYPY_TK_NODE_READ_FLIP=0"]), 2)

    def test_unknown_leg_is_refused(self) -> None:
        self.assertEqual(self._run_main(["--legs", "errors,everything"]), 2)

    def test_identical_arm_names_are_refused(self) -> None:
        self.assertEqual(self._run_main(["--name-a", "same", "--name-b", "same"]), 2)

    def test_a_bad_arm_token_is_refused(self) -> None:
        self.assertEqual(self._run_main(["--a", "nope", "--b", "nope"]), 2)


class IntegrationSuite(unittest.TestCase):
    """End to end, the two directions the differential has to have.

    The positive control is one configuration against itself; the negative
    control points one arm at a corpus copy that gains an extra diagnostic, and
    requires the *named* legs to fail while the others still report.

    Both arms declare `flag:--no-native-parser` so the test is hermetic: the
    shared venv resolves `ast_serialize` to the PyPI stub, which rejects the
    current `parse()` keyword and makes the native parser raise (#1800), and a
    test must not silently depend on a scratch extension build.
    """

    LEGS = "kernel,errors,ast,typemap,deferral.build"

    def _run(
        self,
        out: Path,
        extra_a: tuple[str, ...] = (),
        extra_b: tuple[str, ...] = (),
        corpus_b: Path | None = None,
        legs: str | None = None,
    ) -> Any:
        command = [sys.executable, str(RUNNER), "--name-a", "left", "--name-b", "right"]
        for token in ("flag:--no-native-parser", *extra_a):
            command += ["--a", token]
        for token in ("flag:--no-native-parser", *extra_b):
            command += ["--b", token]
        command += [
            "--legs",
            legs or self.LEGS,
            "--corpus",
            str(CORPUS),
            "--out",
            str(out),
            "--tree",
            PREFIX,
            "--python",
            sys.executable,
        ]
        if corpus_b is not None:
            command += ["--corpus-b", str(corpus_b)]
        return subprocess.run(
            command,
            cwd=PREFIX,
            env=dict(os.environ, PYTHONPATH=PREFIX),
            capture_output=True,
            text=True,
            timeout=900,
        )

    def _verdicts(self, report: str) -> dict[str, str]:
        verdicts: dict[str, str] = {}
        for line in report.splitlines():
            for leg in runner.ALL_LEGS:
                if line.startswith(f"{leg} "):
                    verdicts[leg] = line.split()[1]
        return verdicts

    def test_positive_control_agrees_and_counts_modules(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            completed = self._run(Path(tmp) / "out")
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            self.assertIn("all 5 compared leg(s) agree", completed.stdout)
            report = (Path(tmp) / "out" / "report.txt").read_text(encoding="utf8")
            # A leg reported as compared has to say what it compared: an
            # agreement over an empty artifact is the hollow zero this runner
            # refuses, and "0 module dump(s)" would be exactly that.
            self.assertEqual(
                [self._verdicts(report)[leg] for leg in self.LEGS.split(",")], ["agree"] * 5
            )
            self.assertNotIn("0 message(s)", report)
            self.assertNotIn("0 module dump(s)", report)

    def test_negative_control_fails_the_named_legs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            mutant = Path(tmp) / "corpus"
            mutant.mkdir()
            for source in sorted(CORPUS.glob("*.py")):
                (mutant / source.name).write_text(source.read_text(encoding="utf8"))
            extra = (
                '\n\ndef extra_diagnostic() -> int:\n    broken: int = "text"\n    return broken\n'
            )
            with open(mutant / "seeded_error.py", "a", encoding="utf8") as handle:
                handle.write(extra)
            completed = self._run(Path(tmp) / "out", corpus_b=mutant)
            self.assertEqual(completed.returncode, 1, completed.stdout + completed.stderr)
            self.assertIn("CONTROL RUN", completed.stdout)
            report = (Path(tmp) / "out" / "report.txt").read_text(encoding="utf8")
            verdicts = self._verdicts(report)
            self.assertEqual(verdicts["errors"], "mismatch")
            self.assertEqual(verdicts["ast"], "mismatch")
            # Legs that cannot see the extra diagnostic still report agreement:
            # one failing leg must not stop the others from being compared.
            self.assertEqual(verdicts["kernel"], "agree")
            self.assertEqual(verdicts["deferral.build"], "agree")

    def test_a_custom_pair_does_not_leak_env_tokens(self) -> None:
        """Regression for the arm-environment leak.

        `env:` tokens that do not overlap are the case the presets cannot reach:
        one `arm_environment` call used to build both arms' environments, so arm
        A's undeclared token reached arm B's process and the kernel leg read the
        leaked mode back as if arm B had declared it. The artifacts are the
        instrument: arm B's gate must read `0` in all three fields.
        """
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "out"
            self._run(
                out,
                extra_a=("env:MYPY_TK_NODE_READ_FLIP=2",),
                extra_b=("env:MYPY_TK_STMT_READ_FLIP=2",),
                legs="kernel",
            )
            gates = {}
            for arm in ("left", "right"):
                provenance = json.loads(
                    (out / arm / "dump" / "provenance.json").read_text(encoding="utf8")
                )
                gates[arm] = provenance["gates"]
            self.assertEqual(gates["left"]["MYPY_TK_NODE_READ_FLIP"]["declared"], 2)
            self.assertEqual(gates["left"]["MYPY_TK_NODE_READ_FLIP"]["requested"], 2)
            for field in ("declared", "requested", "in_force"):
                self.assertEqual(
                    gates["right"]["MYPY_TK_NODE_READ_FLIP"][field],
                    0,
                    f"arm right carries arm left's undeclared token ({field})",
                )
            self.assertEqual(gates["right"]["MYPY_TK_STMT_READ_FLIP"]["requested"], 2)
            self.assertEqual(gates["left"]["MYPY_TK_STMT_READ_FLIP"]["requested"], 0)

    def test_an_opt_gated_pair_takes_effect_on_the_daemon_path(self) -> None:
        """Regression for the daemon leg dropping `opt:` tokens.

        The daemon leg builds its own `Options` from the build argv, which never
        carried `opt:` tokens, so a pair differing only by one compared two runs
        at the option defaults. `strict_optional` changes the diagnostic set on
        this corpus, so the leg must report a mismatch.
        """
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "out"
            completed = self._run(
                out,
                extra_a=("opt:strict_optional=0",),
                extra_b=("opt:strict_optional=1",),
                legs="errors,deferral.daemon",
            )
            self.assertEqual(completed.returncode, 1, completed.stdout + completed.stderr)
            verdicts = self._verdicts((out / "report.txt").read_text(encoding="utf8"))
            self.assertEqual(verdicts["errors"], "mismatch")
            self.assertEqual(verdicts["deferral.daemon"], "mismatch")
            daemon = json.loads(
                (out / "left" / "dump" / "deferral-daemon.json").read_text(encoding="utf8")
            )
            declared = daemon["options"]["strict_optional"]
            self.assertEqual(declared["declared"], "0")
            self.assertIs(declared["applied"], False)
            self.assertIs(declared["default"], True)


if __name__ == "__main__":
    unittest.main()
