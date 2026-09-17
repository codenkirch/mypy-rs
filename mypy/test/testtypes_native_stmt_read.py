"""G2.1: the statement family's serving read channel and its differential.

`mypy/nodes_mirror` serves `Block.is_unreachable` - the one registered
statement field native code reads live - from the Rust metadata record
when the stmt read flip is on, and the native dependency walk
(`depswalk.rs`) plus the native semanal visitor read it through
`serve_stmt_flag`. Mode 0 (the default) serves nothing; mode 1 serves;
mode 2 serves and compares every served read against the live slot.

The suite has three jobs:

1. Pin the serving contract per read: what a `Bool` record answers, what
   an unrecorded or shape-crossed record defers, and that a read never
   writes the record.
2. Run the *real* consumer in every mode and compare the dependency maps
   it produces, with non-vacuity (`served`/`compared` > 0) and provenance
   (`deferred_unrecorded`) counters, so a "green" leg cannot be green
   because nothing was served or compared.
3. Keep its own green result falsifiable. `MYPY_TK_STMT_READ_CONTROL`
   mutates the harness (`off` disables serving, `desync` writes a live
   slot the capture hook never saw); with a mutation the same assertions
   must fail, which is the receipt that the differential is not vacuous.
"""

from __future__ import annotations

import os
from typing import Any

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from unittest import skipUnless

from mypy.nodes import (
    AssignmentStmt,
    Block,
    CallExpr,
    Expression,
    MypyFile,
    NameExpr,
    PassStmt,
    SymbolTable,
)
from mypy.options import Options
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED
from mypy.test.typefixture import TypeFixture
from mypy.types import Type

# Harness mutations, so a mutation run needs no edit and the unmutated run
# is the control: "" (none), "off" (serving disabled), "desync" (a live
# slot written behind the capture hook).
_CONTROL_ENV = "MYPY_TK_STMT_READ_CONTROL"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class StmtShadowServingSuite(Suite):
    """The statement serving channel's contract, per read and end to end."""

    def setUp(self) -> None:
        from mypy import nodes_mirror
        from mypy.server.deps import _set_native_server_deps_active

        self._m = nodes_mirror
        self._k = _type_kernel
        self._set_deps_active = _set_native_server_deps_active
        self._deps_active_before = self._deps_was_active()
        self.fx = TypeFixture()
        self._m.activate(audit=True)
        self._m.set_stmt_read_flip(0)
        self._m.reset(clear_counts=True)
        self.control = os.environ.get(_CONTROL_ENV, "")

    def tearDown(self) -> None:
        self._m.set_stmt_read_flip(0)
        self._m.reset(clear_counts=True)
        self._set_deps_active(self._deps_active_before)

    def _deps_was_active(self) -> bool:
        from mypy.server import deps

        return bool(deps._native_server_deps_active)

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.stmt_read_counters()
        return {k: v - before.get(k, 0) for k, v in after.items() if v != before.get(k, 0)}

    # -- fixtures --

    def _adopted_block(self, body: list[Any]) -> Block:
        """A Block the metadata store holds a `Bool` record for."""
        block = Block(body)
        block.is_unreachable = True
        assert id(block) in self._m._META_HANDLES, "the flag write must adopt the block"
        return block

    def _tree(self, defs: list[Any]) -> MypyFile:
        tree = MypyFile([], [])
        tree._fullname = "main"
        tree.names = SymbolTable()
        tree.path = ""
        tree.defs = defs
        return tree

    def _deps_tree(self) -> tuple[MypyFile, dict[Expression, Type]]:
        """The two shapes the walker reads `is_unreachable` in.

        An unrecorded reachable Block (the baseline live read) and an
        adopted unreachable Block whose body defines a trigger: when the
        flag is served the walker must skip exactly that body, and a
        desynced live flag makes the two walkers disagree.
        """
        callee = NameExpr("f")
        callee.fullname = "main.f"
        call = CallExpr(callee, [], [], [])
        defining = NameExpr("x")
        defining.fullname = "main.x"
        defining.is_new_def = True
        trigger = AssignmentStmt([defining], call)
        unrecorded = Block([PassStmt()])
        adopted = self._adopted_block([trigger])
        assert (
            id(unrecorded) not in self._m._META_HANDLES
        ), "the unrecorded shape must stay unrecorded"
        tree = self._tree([unrecorded, adopted])
        self._apply_control(adopted)
        return tree, {}

    def _apply_control(self, adopted: Block) -> None:
        """Apply the harness mutation, if any (see `_CONTROL_ENV`)."""
        if self.control == "desync":
            # A live write no capture saw: the record still says
            # unreachable, so the served read skips a body the live
            # flag would walk - mode 2 must report the disagreement.
            self._m._ORIG_SETATTR(adopted, "is_unreachable", False)
        elif self.control not in ("", "off"):
            raise AssertionError(f"unknown {_CONTROL_ENV}={self.control!r}")

    def _walk(
        self, tree: MypyFile, type_map: dict[Expression, Type], logical: bool
    ) -> dict[str, set[str]]:
        """The real consumer: `get_dependencies` through the native walk."""
        from mypy.server.deps import get_dependencies

        options = Options()
        options.logical_deps = logical
        self._set_deps_active(False)
        python_map = get_dependencies(tree, type_map, (3, 13), options)
        self._set_deps_active(True)
        native_map = get_dependencies(tree, type_map, (3, 13), options)
        assert_equal(native_map, python_map, "native/Python deps mismatch")
        return native_map

    # -- the serving contract --

    def test_mode_zero_is_the_default_and_serves_nothing(self) -> None:
        block = self._adopted_block([])
        assert self._m.stmt_read_flip() == 0
        assert self._k.rust_node_mirror_serve_stmt_flag(block, "is_unreachable") is None
        assert self._k.rust_node_mirror_verify_stmt_flag(block, "is_unreachable") is None
        counters = self._m.stmt_read_counters()
        assert counters["served"] == 0
        assert counters["deferred_off"] == 2
        assert counters["compared"] == 0

    def test_a_recorded_flag_is_answered_exactly(self) -> None:
        block = self._adopted_block([])
        self._m.set_stmt_read_flip(2)
        served = self._k.rust_node_mirror_serve_stmt_flag(block, "is_unreachable")
        assert served is True, "a recorded flag must be served"
        assert self._k.rust_node_mirror_verify_stmt_flag(block, "is_unreachable") is True
        counters = self._m.stmt_read_counters()
        assert counters["served"] == 2
        assert counters["compared"] == 2, "each serving read in mode 2 compares once"
        assert counters["mismatched"] == 0

    def test_an_unrecorded_flag_defers_and_is_counted_as_such(self) -> None:
        never_written = Block([])
        self._m.set_stmt_read_flip(1)
        assert self._k.rust_node_mirror_serve_stmt_flag(never_written, "is_unreachable") is None
        counters = self._m.stmt_read_counters()
        assert counters["deferred_unrecorded"] == 1
        assert counters["served"] == 0

    def test_a_shape_crossed_record_is_not_served(self) -> None:
        # A raw pyfunction write lands an `Int` on the bool field: the
        # record exists, but its shape is not servable, so the live read
        # must stay in charge (#1785's drift class).
        block = Block([])
        self._k.rust_node_mirror_capture_meta(block, "is_unreachable", "int", None, 1, None)
        self._m.set_stmt_read_flip(1)
        assert self._k.rust_node_mirror_serve_stmt_flag(block, "is_unreachable") is None
        counters = self._m.stmt_read_counters()
        assert counters["deferred_unrecorded"] == 1
        assert counters["served"] == 0

    def test_a_served_read_changes_nothing(self) -> None:
        block = self._adopted_block([])
        handle = self._m._META_HANDLES[id(block)]
        captures_before = self._k.rust_node_mirror_meta_captures(handle)
        before = self._m.stmt_read_counters()
        self._m.set_stmt_read_flip(2)
        for _ in range(3):
            assert self._k.rust_node_mirror_verify_stmt_flag(block, "is_unreachable") is True
        assert block.is_unreachable is True
        assert captures_before is not None
        assert self._k.rust_node_mirror_meta_captures(handle) == captures_before
        after = self._delta(before)
        assert after["served"] == 3, "reads must not re-capture"

    def test_an_out_of_range_mode_is_refused(self) -> None:
        try:
            self._m.set_stmt_read_flip(3)
        except ValueError:
            return
        raise AssertionError("mode 3 must be refused, not silently accepted")

    def test_the_raw_seam_refuses_an_out_of_range_mode(self) -> None:
        # The stubs expose the pyfunction directly, so the range check
        # cannot live only on the Python wrapper (#1779).
        try:
            self._k.rust_node_mirror_set_stmt_read_mode(3)
        except ValueError:
            return
        raise AssertionError("the raw seam must refuse mode 3")

    # -- the gate's own state handling (#1779) --

    def test_a_mode_set_before_registration_is_still_readable(self) -> None:
        """`set_stmt_read_flip` before `activate` must not desync the read.

        The entry points share one kernel lookup, so a mode the
        pre-activation call already handed to Rust reads back through
        `stmt_read_flip`/`stmt_read_counters` instead of reporting 0/{}.
        """
        saved_active = self._m._active
        saved_kernel = self._m._kernel_mod
        try:
            self._m._active = False
            self._m._kernel_mod = None
            assert self._m.set_stmt_read_flip(1) == 1
            assert self._m.stmt_read_flip() == 1, "the set mode must be readable back"
            assert self._m.stmt_read_counters() != {}, "the counters must see the same store"
        finally:
            self._m.set_stmt_read_flip(0)
            self._m._active = saved_active
            self._m._kernel_mod = saved_kernel

    def test_a_malformed_mode_env_fails_activate_with_no_state_change(self) -> None:
        """A bad `MYPY_TK_STMT_READ_FLIP` raises before anything is patched.

        Failure with no state change is what keeps the one-shot guard
        honest: the retry below must activate for real, not report
        success while no serving mode was ever set.
        """
        env_name = self._m._STMT_READ_FLIP_ENV
        saved_active = self._m._active
        saved_env = os.environ.get(env_name)
        try:
            for bad in ("nonsense", "7"):
                self._m._active = False
                os.environ[env_name] = bad
                raised = False
                try:
                    self._m.activate()
                except ValueError:
                    raised = True
                assert raised, f"{env_name}={bad!r} must be refused"
                assert self._m._active is False, "a refused activate must change no state"
            os.environ.pop(env_name, None)
            assert self._m.activate() is True, "the retry must not be swallowed"
            assert self._m.stmt_read_flip() == 0
        finally:
            self._m._active = saved_active
            if saved_env is None:
                os.environ.pop(env_name, None)
            else:
                os.environ[env_name] = saved_env

    # -- the real consumer, in every mode --

    def test_deps_walk_serves_the_flag_and_agrees_with_python(self) -> None:
        tree, type_map = self._deps_tree()
        totals = {"served": 0, "compared": 0, "mismatched": 0, "deferred_off": 0}
        for logical in (False, True):
            baseline: dict[str, set[str]] | None = None
            for mode in (0, 1, 2):
                # The `off` mutation disables serving for every leg below,
                # so the `served > 0` assertion must fail on a mutated run.
                effective = 0 if self.control == "off" else mode
                self._m.set_stmt_read_flip(effective)
                before = self._m.stmt_read_counters()
                result = self._walk(tree, type_map, logical)
                delta = self._delta(before)
                for key in totals:
                    totals[key] += delta.get(key, 0)
                if mode == 0:
                    assert delta.get("served", 0) == 0, "mode 0 must serve nothing"
                    baseline = result
                    continue
                assert baseline is not None
                assert_equal(result, baseline, "served flags must not change the deps map")
                if mode == 1:
                    assert delta.get("served", 0) > 0, "the walker must be answered by the record"
                else:
                    assert delta.get("compared", 0) > 0, "mode 2 must compare, not just return"
                    assert (
                        delta.get("mismatched", 0) == 0
                    ), "a served flag must match the live slot"
        assert totals["served"] > 0, "the channel must have answered, not merely returned"
        assert totals["compared"] > 0, "the differential must have compared"
        assert totals["mismatched"] == 0, "no served flag may disagree with the live slot"
        assert totals["deferred_off"] > 0, "the mode-0 legs must have read live"

    def test_the_provenance_counters_separate_recorded_from_served(self) -> None:
        tree, type_map = self._deps_tree()
        self._m.set_stmt_read_flip(1)
        self._walk(tree, type_map, logical=True)
        counters = self._m.stmt_read_counters()
        assert counters["consulted"] >= counters["served"] > 0
        assert (
            counters["deferred_unrecorded"] > 0
        ), "an unrecorded block in the same walk must be counted separately"
        assert counters["consulted"] == counters["served"] + counters["deferred_unrecorded"]
