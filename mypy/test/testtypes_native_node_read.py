"""G1.1: the node shadow's serving read channel and its differential.

`mypy/nodes_mirror` serves the `RefExpr` binding scalars (`kind`,
`_fullname`, `is_new_def`, ...) from Rust storage when the record is
provably exact, and the native dependency walk (`depswalk.rs`) reads them
through `RefView`. Mode 0 (the default) serves nothing; mode 1 serves;
mode 2 serves and compares every served read against the live slots.

The suite has three jobs:

1. Pin the serving contract per read: what a record answers, what an
   unrecorded node defers, and that a read never writes the record.
2. Run the *real* consumer in every mode and compare the dependency maps
   it produces, with non-vacuity (`served`/`compared` > 0) and provenance
   (`deferred_unrecorded`) counters, so a "green" leg cannot be green
   because nothing was served or compared.
3. Keep its own green result falsifiable. `MYPY_TK_NODE_READ_CONTROL`
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
    GDEF,
    LDEF,
    MDEF,
    AssignmentStmt,
    CallExpr,
    Expression,
    MemberExpr,
    MypyFile,
    NameExpr,
    SymbolTable,
    Var,
)
from mypy.options import Options
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED
from mypy.test.typefixture import TypeFixture
from mypy.types import Type

# Harness mutations, so a mutation run needs no edit and the unmutated run
# is the control: "" (none), "off" (serving disabled), "desync" (a live
# slot written behind the capture hook).
_CONTROL_ENV = "MYPY_TK_NODE_READ_CONTROL"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NodeShadowServingSuite(Suite):
    """The serving channel's contract, per read and end to end."""

    def setUp(self) -> None:
        from mypy import nodes_mirror
        from mypy.server.deps import _set_native_server_deps_active

        self._m = nodes_mirror
        self._k = _type_kernel
        self._set_deps_active = _set_native_server_deps_active
        self._deps_active_before = self._deps_was_active()
        self.fx = TypeFixture()
        # #1864: the def_var arm (an entry without a ref capture) needs the
        # full scope.
        os.environ[nodes_mirror._CAPTURE_SCOPE_ENV] = "full"
        self.addCleanup(os.environ.pop, nodes_mirror._CAPTURE_SCOPE_ENV, None)
        self._m.activate(audit=True)
        self._m.set_read_flip(0)
        self._m.reset(clear_counts=True)
        self.control = os.environ.get(_CONTROL_ENV, "")

    def tearDown(self) -> None:
        self._m.set_read_flip(0)
        self._m.reset(clear_counts=True)
        self._set_deps_active(self._deps_active_before)

    def _deps_was_active(self) -> bool:
        from mypy.server import deps

        return bool(deps._native_server_deps_active)

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.read_counters()
        return {k: v - before.get(k, 0) for k, v in after.items() if v != before.get(k, 0)}

    # -- fixtures --

    def _adopted_ref(self, name: str = "x", fullname: str = "main.x") -> NameExpr:
        """A NameExpr the shadow holds an exact record for."""
        ref = NameExpr(name)
        ref.fullname = fullname
        ref.kind = GDEF
        assert id(ref) in self._m._NODE_HANDLES, "the binding write must adopt the node"
        return ref

    def _adopted_member(self) -> MemberExpr:
        """A MemberExpr lvalue the shadow holds an exact record for."""
        member = MemberExpr(NameExpr("o"), "y")
        member.kind = MDEF
        assert id(member) in self._m._NODE_HANDLES
        return member

    def _tree(self, defs: list[Any]) -> MypyFile:
        tree = MypyFile([], [])
        tree._fullname = "main"
        tree.names = SymbolTable()
        tree.path = ""
        tree.defs = defs
        return tree

    def _deps_tree(self) -> tuple[MypyFile, dict[Expression, Type]]:
        """Three shapes the walker reads the served scalars in.

        `process_lvalue`'s MemberExpr arm reads `kind` for any member
        lvalue (recorded and unrecorded), and the logical-deps assignment
        tail reads `fullname` + `is_new_def` of a defining lvalue whose
        callable is a reference.
        """
        member = self._adopted_member()
        unrecorded = MemberExpr(NameExpr("p"), "z")
        base = NameExpr("o")
        callee = self._adopted_ref("f", "main.f")
        call = CallExpr(callee, [], [], [])
        defining = self._adopted_ref("x", "main.x")
        defining.is_new_def = True
        assert (
            id(unrecorded) not in self._m._NODE_HANDLES
        ), "the unrecorded shape must stay unrecorded"
        type_map: dict[Expression, Type] = {base: self.fx.a, member: self.fx.a}
        tree = self._tree(
            [
                AssignmentStmt([member], base),
                AssignmentStmt([unrecorded], base),
                AssignmentStmt([defining], call),
            ]
        )
        self._apply_control(member)
        return tree, type_map

    def _apply_control(self, member: MemberExpr) -> None:
        """Apply the harness mutation, if any (see `_CONTROL_ENV`)."""
        if self.control == "desync":
            # A live write no capture saw: the record is now wrong, which
            # mode 2 must report instead of passing.
            self._m._ORIG_SETATTR(member, "kind", GDEF)
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
        ref = self._adopted_ref()
        assert self._m.read_flip() == 0
        assert self._k.rust_node_mirror_serve_ref(ref) is None
        assert self._k.rust_node_mirror_verify_ref(ref) is None
        counters = self._m.read_counters()
        assert counters["served"] == 0
        assert counters["deferred_off"] == 2
        assert counters["compared"] == 0

    def test_a_recorded_read_is_answered_exactly(self) -> None:
        ref = self._adopted_ref()
        target = Var("v")
        target._fullname = "main.v"
        ref.node = target
        ref.is_new_def = True
        self._m.set_read_flip(2)
        served = self._k.rust_node_mirror_serve_ref(ref)
        assert served is not None, "a recorded read must be served"
        kind, node_fullname, fullname, is_new_def, is_inferred_def = served
        assert (kind, node_fullname, fullname, is_new_def, is_inferred_def) == (
            GDEF,
            "main.v",
            "main.x",
            True,
            False,
        )
        assert self._k.rust_node_mirror_verify_ref(ref) is True
        counters = self._m.read_counters()
        assert counters["served"] == 2
        assert counters["compared"] == 2, "each serving read in mode 2 compares once"
        assert counters["mismatched"] == 0

    def test_an_unrecorded_read_defers_and_is_counted_as_such(self) -> None:
        never_written = NameExpr("untouched")
        self._m.set_read_flip(1)
        assert self._k.rust_node_mirror_serve_ref(never_written) is None
        counters = self._m.read_counters()
        assert counters["deferred_unrecorded"] == 1
        assert counters["served"] == 0

    def test_an_entry_from_another_field_is_not_served(self) -> None:
        # `def_var` mints an entry without a ref capture; its five scalars
        # are the untouched record default, so a read must not trust them.
        member = MemberExpr(NameExpr("o"), "y")
        member.def_var = Var("y")
        assert id(member) in self._m._NODE_HANDLES
        assert member.kind is None
        self._m.set_read_flip(1)
        assert self._k.rust_node_mirror_serve_ref(member) is None

    def test_a_served_read_changes_nothing(self) -> None:
        member = self._adopted_member()
        node_id = id(member)
        handle = self._m._NODE_HANDLES[node_id]
        captures_before = self._k.rust_node_mirror_field_captures(handle)
        before = self._m.read_counters()
        self._m.set_read_flip(2)
        for _ in range(3):
            assert self._k.rust_node_mirror_verify_ref(member) is True
        assert id(member) == node_id
        assert (member.kind, member.is_new_def, member.fullname) == (MDEF, False, "")
        assert self._k.rust_node_mirror_entry_count() == 1
        assert captures_before is not None
        assert self._k.rust_node_mirror_field_captures(handle) == captures_before
        after = self._delta(before)
        assert after["served"] == 3, "reads must not re-capture"

    def test_a_later_capture_refreshes_what_is_served(self) -> None:
        ref = self._adopted_ref()
        self._m.set_read_flip(1)
        assert self._k.rust_node_mirror_serve_ref(ref) is not None
        ref.is_inferred_def = True
        served = self._k.rust_node_mirror_serve_ref(ref)
        assert served is not None
        assert served[4] is True, "the captured write must reach the served read"

    def test_an_unreadable_target_fullname_is_not_a_mismatch(self) -> None:
        """#1780: the capture records `None` when a target `fullname`
        cannot be read, so the mode-2 differential must read the same
        error as `None` instead of inventing a desync for agreeing state.
        """

        class _RaisingTarget(Var):
            @property
            def fullname(self) -> str:
                raise RuntimeError("unreadable fullname")

        ref = self._adopted_ref()
        ref.node = _RaisingTarget("t")
        self._m.set_read_flip(2)
        before = self._m.read_counters()
        assert self._k.rust_node_mirror_verify_ref(ref) is True
        delta = self._delta(before)
        assert delta.get("compared") == 1, delta
        assert delta.get("mismatched", 0) == 0, delta
        assert delta.get("compare_errors", 0) == 0, delta

    def test_an_out_of_range_mode_is_refused(self) -> None:
        try:
            self._m.set_read_flip(3)
        except ValueError:
            return
        raise AssertionError("mode 3 must be refused, not silently accepted")

    def test_the_raw_seam_refuses_an_out_of_range_mode(self) -> None:
        # The stubs expose the pyfunction directly, so the range check
        # cannot live only on the Python wrapper (#1779).
        try:
            self._k.rust_node_mirror_set_read_mode(3)
        except ValueError:
            return
        raise AssertionError("the raw seam must refuse mode 3")

    # -- the gate's own state handling (#1779) --

    def test_a_mode_set_before_registration_is_still_readable(self) -> None:
        """`set_read_flip` before `activate` must not desync `read_flip`.

        The three entry points share one kernel lookup, so a mode the
        pre-activation call already handed to Rust reads back through
        `read_flip`/`read_counters` instead of reporting 0/{}.
        """
        saved_active = self._m._active
        saved_kernel = self._m._kernel_mod
        try:
            self._m._active = False
            self._m._kernel_mod = None
            assert self._m.set_read_flip(1) == 1
            assert self._m.read_flip() == 1, "the set mode must be readable back"
            assert self._m.read_counters() != {}, "the counters must see the same store"
        finally:
            self._m.set_read_flip(0)
            self._m._active = saved_active
            self._m._kernel_mod = saved_kernel

    def test_a_malformed_mode_env_fails_activate_with_no_state_change(self) -> None:
        """A bad `MYPY_TK_NODE_READ_FLIP` raises before anything is patched.

        Failure with no state change is what keeps the one-shot guard
        honest: the retry below must activate for real, not report
        success while no serving mode was ever set.
        """
        env_name = self._m._READ_FLIP_ENV
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
            assert self._m.read_flip() == 0
        finally:
            self._m._active = saved_active
            if saved_env is None:
                os.environ.pop(env_name, None)
            else:
                os.environ[env_name] = saved_env

    # -- the real consumer, in every mode --

    def test_deps_walk_serves_the_scalars_and_agrees_with_python(self) -> None:
        tree, type_map = self._deps_tree()
        totals = {"served": 0, "compared": 0, "mismatched": 0, "deferred_off": 0}
        for logical in (False, True):
            baseline: dict[str, set[str]] | None = None
            for mode in (0, 1, 2):
                # The `off` mutation disables serving for every leg below,
                # so the `served > 0` assertion must fail on a mutated run.
                effective = 0 if self.control == "off" else mode
                self._m.set_read_flip(effective)
                before = self._m.read_counters()
                result = self._walk(tree, type_map, logical)
                delta = self._delta(before)
                for key in totals:
                    totals[key] += delta.get(key, 0)
                if mode == 0:
                    assert delta.get("served", 0) == 0, "mode 0 must serve nothing"
                    baseline = result
                    continue
                assert baseline is not None
                assert_equal(result, baseline, "served reads must not change the deps map")
                if mode == 1:
                    assert delta.get("served", 0) > 0, "the walker must be answered by the record"
                else:
                    assert delta.get("compared", 0) > 0, "mode 2 must compare, not just return"
                    assert (
                        delta.get("mismatched", 0) == 0
                    ), "a served read must match the live slot"
        assert totals["served"] > 0, "the channel must have answered, not merely returned"
        assert totals["compared"] > 0, "the differential must have compared"
        assert totals["mismatched"] == 0, "no served read may disagree with the live slot"
        assert totals["deferred_off"] > 0, "the mode-0 legs must have read live"

    def test_the_provenance_counters_separate_recorded_from_served(self) -> None:
        tree, type_map = self._deps_tree()
        self._m.set_read_flip(1)
        self._walk(tree, type_map, logical=True)
        counters = self._m.read_counters()
        assert counters["consulted"] >= counters["served"] > 0
        assert (
            counters["deferred_unrecorded"] > 0
        ), "an unrecorded node in the same walk must be counted separately"
        assert counters["consulted"] == counters["served"] + counters["deferred_unrecorded"]

    # -- the #1785 record-shape contract and its detector --

    def _member_tree(self, member: MemberExpr) -> tuple[MypyFile, dict[Expression, Type]]:
        """One member-lvalue assignment: the shape `process_lvalue` reads."""
        base = NameExpr("o")
        type_map: dict[Expression, Type] = {base: self.fx.a, member: self.fx.a}
        return self._tree([AssignmentStmt([member], base)]), type_map

    def _walk_mode(
        self, tree: MypyFile, type_map: dict[Expression, Type], mode: int, logical: bool = False
    ) -> dict[str, int]:
        """Walk in `mode`, returning the read-counter delta.

        The `off` control forces mode 0 for every leg, which is what makes
        a mutated run fail where the unmutated one passes.
        """
        self._m.set_read_flip(0 if self.control == "off" else mode)
        before = self._m.read_counters()
        self._walk(tree, type_map, logical=logical)
        return self._delta(before)

    def _assert_mode_answers(
        self, tree: MypyFile, type_map: dict[Expression, Type], logical: bool = False
    ) -> None:
        """Mode 0 serves nothing, mode 1 serves, mode 2 serves and reports."""
        delta = self._walk_mode(tree, type_map, 0, logical=logical)
        assert delta.get("served", 0) == 0, "mode 0 must serve nothing"
        assert delta.get("deferred_off", 0) > 0, "the mode-0 leg must read live"
        assert delta.get("mismatched", 0) == 0
        delta = self._walk_mode(tree, type_map, 1, logical=logical)
        assert delta.get("served", 0) > 0, "mode 1 must answer from the record"
        assert delta.get("compared", 0) == 0, "mode 1 does not compare"
        assert delta.get("mismatched", 0) == 0
        delta = self._walk_mode(tree, type_map, 2, logical=logical)
        assert delta.get("served", 0) > 0
        assert delta.get("compared", 0) > 0, "mode 2 must compare, not just return"
        assert delta.get("mismatched", 0) > 0, "the detector must fire on the drift"

    def test_the_differential_fires_on_a_present_non_int_kind(self) -> None:
        """#1785 case 1: `kind` written to a present non-`int` after capture.

        `_capture_ref` hands `node.kind` to the `Option<i64>` seam; a
        present non-`int` fails conversion, the `except Exception` keeps
        the earlier record, and the served `kind_is_none` answers the
        stale scalar where the live plain-`None` check answers the new
        one. Pins what each mode answers so the limitation is explicit.
        """
        member = self._adopted_member()
        member.kind = None  # in-contract: the record refreshes to `None`
        tree, type_map = self._member_tree(member)

        # Positive control: an unfaulted record never fires the detector.
        clean = self._walk_mode(tree, type_map, 2)
        assert clean.get("served", 0) > 0, "the clean leg must serve"
        assert clean.get("mismatched", 0) == 0, "no drift, no mismatch"

        fails = self._m.report().get("capture_fail.ref", 0)
        # Out-of-contract: a present non-int must fail the capture.
        member.kind = "bogus"  # type: ignore[assignment]
        assert (
            self._m.report().get("capture_fail.ref", 0) == fails + 1
        ), "the non-int kind must fail the Option<i64> capture"
        self._apply_control(member)
        assert member.kind is not None, "the live slot holds a present kind"

        # Mode 0 serves nothing and the fallback reads the live slot; mode 1
        # serves the stale record. The two answers disagree by construction.
        self._m.set_read_flip(0)
        assert self._k.rust_node_mirror_serve_ref(member) is None
        mode0_answers_none = member.kind is None  # the live fallback
        self._m.set_read_flip(1)
        served = self._k.rust_node_mirror_serve_ref(member)
        assert served is not None
        mode1_answers_none = served[0] is None  # the served record
        assert (mode0_answers_none, mode1_answers_none) == (
            False,
            True,
        ), "mode 0 answers the live kind, mode 1 answers the stale one"
        assert self._k.rust_node_mirror_verify_ref(member) is False

        self._assert_mode_answers(tree, type_map)

    def test_the_differential_fires_on_a_none_fullname(self) -> None:
        """#1785 case 2: `_fullname` written to `None` after capture.

        The record field is a non-optional `String`, so the served
        `fullname_opt` always answers `Some(...)` while the live fallback
        (`opt_str_attr`) answers `None` for the same slot. The walk legs
        use logical deps with a `CallExpr` rvalue, the shape that reaches
        `RefView::fullname_opt` in `depswalk.rs`.
        """
        member = self._adopted_member()
        # `depswalk` returns early on a local (LDEF) lvalue, so the stale
        # fullname is served at the site without changing the deps map.
        member.kind = LDEF
        member.fullname = "main.y"  # in-contract: the record holds the string
        member.is_new_def = True
        callee = self._adopted_ref("f", "main.f")
        call = CallExpr(callee, [], [], [])
        type_map: dict[Expression, Type] = {}
        tree = self._tree([AssignmentStmt([member], call)])

        clean = self._walk_mode(tree, type_map, 2, logical=True)
        assert clean.get("served", 0) > 0, "the clean leg must serve"
        assert clean.get("mismatched", 0) == 0, "no drift, no mismatch"

        fails = self._m.report().get("capture_fail.ref", 0)
        # Out-of-contract: a `None` `_fullname` must fail the capture.
        member._fullname = None  # type: ignore[assignment]
        assert (
            self._m.report().get("capture_fail.ref", 0) == fails + 1
        ), "a None _fullname must fail the String capture"
        self._apply_control(member)
        # `str | None` keeps the read out of `fullname: str`'s narrowing.
        live_fullname: str | None = member.fullname
        assert live_fullname is None, "the live slot holds the out-of-contract value"

        # Mode 0 answers the live `None`, mode 1 the stale non-empty string.
        self._m.set_read_flip(0)
        assert self._k.rust_node_mirror_serve_ref(member) is None
        mode0_fullname: str | None = member.fullname  # the live fallback
        self._m.set_read_flip(1)
        served = self._k.rust_node_mirror_serve_ref(member)
        assert served is not None
        mode1_fullname = served[2]  # the served record
        assert (
            mode0_fullname is None and mode1_fullname == "main.y"
        ), "mode 0 answers the live fullname, mode 1 answers the stale one"
        assert self._k.rust_node_mirror_verify_ref(member) is False

        self._assert_mode_answers(tree, type_map, logical=True)
        # The logical-deps tail is the site that reads `fullname_opt`; a
        # serve count above the process_lvalue one proves the tail answered
        # the lvalue too, so the stale fullname was served by that site.
        mode1 = self._walk_mode(tree, type_map, 1, logical=True)
        assert (
            mode1.get("served", 0) >= 2
        ), "the logical-deps tail must serve the lvalue beside process_lvalue"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NodeSlotDeletionOutOfContractSuite(Suite):
    """#1856: a `del` on a tracked G1 slot keeps its record, pinned as such.

    The G1 classes carry only the `__setattr__` patch, so a deleted slot's
    record is not retracted - the same defect class #1841 closed for the
    def-family store. It stays open rather than half-closed because the G1
    record is not per-field where it matters: the five binding scalars the
    serving channel reads are one snapshot gated by a single presence
    marker (`ref_captures`), so the #1841-style per-field retire cannot
    cover them without new record state. The retraction lands with the G1
    serving flip going default-on; until then these pins document today's
    behavior, and the fix must flip them knowingly.
    """

    def setUp(self) -> None:
        from mypy import nodes_mirror

        self._m = nodes_mirror
        self._k = _type_kernel
        # #1864: the field/analyzed deletion pins need the full scope.
        os.environ[nodes_mirror._CAPTURE_SCOPE_ENV] = "full"
        self.addCleanup(os.environ.pop, nodes_mirror._CAPTURE_SCOPE_ENV, None)
        self._m.activate(audit=True)
        self._m.reset(clear_counts=True)

    def tearDown(self) -> None:
        self._m.reset(clear_counts=True)

    def _adopted_ref(self) -> NameExpr:
        """A NameExpr the shadow holds an exact snapshot record for."""
        ref = NameExpr("x")
        ref.fullname = "main.x"
        ref.kind = GDEF
        assert id(ref) in self._m._NODE_HANDLES, "the binding write must adopt the node"
        return ref

    def test_a_deleted_binding_scalar_keeps_the_stale_snapshot(self) -> None:
        ref = self._adopted_ref()
        before = self._m.report()
        del ref.kind
        record = self._k.rust_node_mirror_ref(self._m._NODE_HANDLES[id(ref)])
        assert record is not None, "the deletion is no store event: the entry stays"
        assert record[0] == GDEF, "out of contract (#1856): the stale scalar stays"
        assert self._m.report() == before, "no retraction hook exists to count it"

    def test_a_deleted_field_slot_keeps_its_map_record(self) -> None:
        member = MemberExpr(NameExpr("o"), "y")
        member.def_var = Var("y")
        assert id(member) in self._m._NODE_HANDLES
        del member.def_var
        handle = self._m._NODE_HANDLES[id(member)]
        fields = self._k.rust_node_mirror_fields(handle) or []
        assert "def_var" in fields, "out of contract (#1856): the deleted slot stays recorded"

    def test_a_deleted_analyzed_keeps_its_presence_bit(self) -> None:
        call = CallExpr(NameExpr("f"), [], [], [])
        call.analyzed = NameExpr("x")
        assert id(call) in self._m._NODE_HANDLES
        del call.analyzed
        record = self._k.rust_node_mirror_analyzed(self._m._NODE_HANDLES[id(call)])
        assert record == (True, "NameExpr"), "out of contract (#1856): the record stays"

    def test_a_deletion_on_a_never_adopted_node_adopts_nothing(self) -> None:
        saved = self._m._active
        self._m._active = False
        try:
            ref = NameExpr("x")
            ref.kind = LDEF
        finally:
            self._m._active = saved
        assert id(ref) not in self._m._NODE_HANDLES
        assert self._k.rust_node_mirror_entry_count() == 0
        del ref.kind
        # The identity layer keys on id() and outlives a store reset, so a
        # recycled address can answer handle_of; adoption is asserted on the
        # store's own state instead (the handle map and the entry count).
        assert id(ref) not in self._m._NODE_HANDLES, "a deletion must not adopt"
        assert self._k.rust_node_mirror_entry_count() == 0, "no entry exists to retract"
