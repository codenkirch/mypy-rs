"""G2.4: the def-family store's cache-loaded contract and its provenance.

`mypy/nodes_mirror` shadows the def-family nodes (`Var`, `FuncDef`,
`Decorator`, `OverloadedFuncDef`) behind the AST-mirror gate. On a
cache-loaded run those nodes are not built by the parser: the fixed-format
reader (`read_symbol`, `read_overload_part`) and the JSON reader
(`SymbolNode.deserialize`) materialize them through plain attribute writes,
so the patched `__setattr__` saw them but recorded only the slots whose
value left the constructor default. Which slots a cached node's record held
therefore followed the reader's write list and values rather than a
contract, and nothing measured it.

The suite has three jobs:

1. Pin the contract: `seed_loaded` runs once per finished cache-loaded node
   and every tracked slot the live object still has is in the record, for
   all four cache-loaded def classes and for the parts a `Decorator` builds
   directly. Absence keeps meaning "not recorded": a slot the live object
   lacks is skipped and counted, never fabricated.
2. Pin the provenance: a field record is attributed on two axes, the
   capture origin (`meta_written.parse` / `.cache_fixed` / `.cache_json`)
   and the mechanism (`meta_written.*` for the capture funnel,
   `meta_seeded.*` for the seed), and a class the store tracks no slot for
   is counted as a declined seed rather than passing silently.
3. Keep its own green result falsifiable. `MYPY_TK_DEF_SEED_CONTROL`
   mutates the mechanism (`off` makes the seed a no-op, `blind` lets it run
   but take no field), and the named assertions must fail under it while
   the reader-side ones stay green - that boundary is the honest limit: for
   `Var`, `FuncDef` and `Decorator` the reader's incidental coverage is
   already complete, so their *field-absence* assertions do not bite under
   `off`; only the seed counters do. See
   `test_the_reader_alone_leaves_only_overloaded_items_unrecorded`.
"""

from __future__ import annotations

import os
from typing import Any

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from unittest import skipUnless

from mypy.cache import ReadBuffer, WriteBuffer, read_tag
from mypy.nodes import Block, Decorator, FuncDef, OverloadedFuncDef, TypeAlias, Var, read_symbol
from mypy.test.helpers import Suite
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED
from mypy.types import AnyType, TypeOfAny

# Harness mutations, so a mutation run needs no edit and the unmutated run is
# the control: "" (none), "off" (the seed never runs, so every seed counter is
# zero: the structural-zero control) and "blind" (the seed runs but takes nothing).
_CONTROL_ENV = "MYPY_TK_DEF_SEED_CONTROL"


class _BlindSeedKernel:
    """The `blind` control: the seed call is accepted and takes nothing.

    Wraps the real extension, so every other seam (`meta`, the read
    counters, the record shapes) behaves exactly as it does unmutated and
    only the seed's effect on the store is removed: the node is still
    identified, none of its fields are recorded. That splits the two
    controls' signatures: `off` zeroes the seed counters, `blind` keeps
    them moving while the record's coverage shrinks.
    """

    def __init__(self, real: Any) -> None:
        self._real = real

    def __getattr__(self, name: str) -> Any:
        if name == "rust_node_mirror_seed_loaded":
            real = self._real

            def blind_seed(obj: Any, fields: Any) -> tuple[int, bool, int, int]:
                return (real.rust_node_mirror_handle_of(obj) or 0, False, 0, 0)

            return blind_seed
        return getattr(self._real, name)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class DefCacheSeedSuite(Suite):
    """The load-time seed: contract, provenance, refusals and controls."""

    def setUp(self) -> None:
        from mypy import nodes_mirror

        self._m = nodes_mirror
        self._k = _type_kernel
        self._m.activate(audit=True)
        self._m.reset(clear_counts=True)
        # Captured before any control lands, so a test that needs the real
        # mechanism (the failure arm) can restore it whatever the control is.
        self._pristine = {"seed_loaded": self._m.seed_loaded, "kernel": self._m._kernel_mod}
        self.control = os.environ.get(_CONTROL_ENV, "")
        if self.control == "off":
            self._m.seed_loaded = lambda node: 0
        elif self.control == "blind":
            self._m._kernel_mod = _BlindSeedKernel(self._m._kernel_mod)
        elif self.control:
            raise AssertionError(f"unknown {_CONTROL_ENV}={self.control!r}")

    def tearDown(self) -> None:
        self._m.seed_loaded = self._pristine["seed_loaded"]
        self._m._kernel_mod = self._pristine["kernel"]
        self._m.reset(clear_counts=True)

    # -- fixtures --

    def _round_trip(self, node: Any) -> Any:
        """Serialize `node`, then read it back through the cache reader."""
        buf = WriteBuffer()
        node.write(buf)
        data = ReadBuffer(buf.getvalue())
        return read_symbol(data, read_tag(data))

    def _var(self, fullname: str = "mod.x") -> Var:
        var = Var("x")
        var._fullname = fullname
        return var

    def _func(self, name: str = "f", fullname: str = "mod.f") -> FuncDef:
        func = FuncDef(name, [], Block([]))
        func._fullname = fullname
        return func

    def _overload(self) -> OverloadedFuncDef:
        ofd = OverloadedFuncDef([self._func(), self._func()])
        ofd._fullname = "mod.f"
        return ofd

    def _decorator(self) -> Decorator:
        return Decorator(self._func(), [], self._var("mod.f"))

    def _record(self, node: Any) -> dict[str, Any] | None:
        # The identity registry's own lookup, not the id()-keyed Python map:
        # a recycled id() would answer with a stranger's record, or with the
        # handle a mutated kernel returned.
        handle = self._k.rust_node_mirror_handle_of(node)
        if handle is None:
            return None
        return self._k.rust_node_mirror_meta(handle)

    def _unrecorded(self, node: Any) -> list[str]:
        """Tracked slots the live node has and its record does not hold.

        This is the per-field absence measure: absence is the defer, so a
        cache-loaded node with slots missing here is under-covered.
        """
        tracked = self._m._meta_tracked(type(node))
        record = self._record(node) or {}
        return sorted(field for field in tracked if field not in record and hasattr(node, field))

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {k: v - before.get(k, 0) for k, v in after.items() if v != before.get(k, 0)}

    def _seed_node(self, node: Any) -> dict[str, int]:
        before = self._m.report()
        self._m.seed_loaded(node)
        return self._delta(before)

    # -- the contract --

    def test_every_tracked_slot_of_a_cache_loaded_node_is_recorded(self) -> None:
        """The contract, per cache-loaded def class, through the reader.

        Four classes and one nested shape: a `Decorator` read materializes
        its `func` and `var` directly, so those two are seeded too.
        """
        for label, node in (
            ("Var", self._round_trip(self._var())),
            ("FuncDef", self._round_trip(self._func())),
            ("OverloadedFuncDef", self._round_trip(self._overload())),
            ("Decorator", self._round_trip(self._decorator())),
        ):
            assert self._m._meta_tracked(type(node)), f"{label} must be a tracked class"
            assert self._unrecorded(node) == [], f"{label} left tracked slots unrecorded"
            assert self._record(node), f"{label} must have a record to serve from"

    def test_a_decorator_read_seeds_the_parts_it_builds_directly(self) -> None:
        """`Decorator.read` bypasses `read_symbol` for its two children."""
        before = self._m.report()
        dec = self._round_trip(self._decorator())
        delta = self._delta(before)
        assert self._record(dec) is not None
        for child in (dec.func, dec.var):
            assert self._record(child) is not None, f"{type(child).__name__} must be seeded"
            assert self._unrecorded(child) == []
        assert (
            delta.get("meta_seed_loaded", 0) == 3
        ), "one read seeds the decorator and both children"

    def test_the_seed_reports_a_node_the_reader_had_already_recorded(self) -> None:
        """`preexisting`/`replaced` are how much of the record is incidental."""
        before = self._m.report()
        ofd = self._round_trip(self._overload())
        delta = self._delta(before)
        assert delta.get("meta_seed_loaded", 0) == 3
        assert delta.get("meta_seed_loaded_preexisting", 0) == 3, (
            "every def node adopts while the reader writes it, so the seed "
            "always finds an entry: the incidental coverage must be visible"
        )
        assert delta.get("meta_seed_loaded_replaced", 0) > 0
        assert delta.get("meta_seed_loaded_minted", 0) == 2, (
            "an OverloadedFuncDef has exactly two tracked slots its reader "
            "never writes (`type`, `unanalyzed_type`); a different count "
            "means the reader or the tracked table moved"
        )
        assert self._unrecorded(ofd) == []

    def test_a_slot_the_live_object_lacks_is_skipped_and_counted(self) -> None:
        """Absence keeps meaning "not recorded": the seed never fabricates.

        The node is built with the store inactive, so nothing recorded the
        slot before it was deleted; the seed then skips the slot the live
        object lacks. A `del` on an *already captured* slot is the other
        half: it now retracts, pinned by
        `MetaSlotDeletionSuite.test_a_deleted_slot_retracts_its_record`.
        """
        saved_active = self._m._active
        self._m._active = False
        try:
            var = self._var()
            var.is_final = True
            del var.type
        finally:
            self._m._active = saved_active
        assert self._record(var) is None, "the inert store must not have adopted it"
        before = self._m.report()
        self._m.seed_loaded(var)
        delta = self._delta(before)
        record = self._record(var) or {}
        assert "type" not in record, "a deleted slot must not be recorded"
        assert "is_final" in record
        assert delta.get("meta_seed_loaded_missing", 0) == 1
        assert delta.get("meta_seed_loaded") == 1

    def test_the_seed_never_raises_into_the_reader(self) -> None:
        """A failing kernel call is accounted, not propagated (#1773's rule).

        Restores the unmutated wrapper first: this is the wrapper's own
        error arm, not a seed effect, so it must not depend on the control.
        """
        real = self._pristine["kernel"]
        before = self._m.report()

        class _Failing:
            def __getattr__(self, name: str) -> Any:
                if name == "rust_node_mirror_seed_loaded":
                    raise RuntimeError("kernel unavailable")
                return getattr(real, name)

        var = self._var()
        self._m.seed_loaded = self._pristine["seed_loaded"]
        self._m._kernel_mod = _Failing()
        try:
            assert self._m.seed_loaded(var) == 0
        finally:
            self._m._kernel_mod = real
        delta = self._delta(before)
        assert delta.get("meta_seed_failed", 0) == 1
        assert "meta_seed_loaded" not in delta

    # -- provenance and refusals --

    def test_an_untracked_class_is_counted_as_a_declined_seed(self) -> None:
        """The declined-vs-never-ran split, on the fixed-format reader."""
        alias = TypeAlias(AnyType(TypeOfAny.unannotated), "mod.A", "mod", 1, 1)
        before = self._m.report()
        node = self._round_trip(alias)
        delta = self._delta(before)
        assert self._m._meta_tracked(type(node)) == frozenset(), "TypeAlias is untracked"
        assert delta.get("meta_seed_rejected_untracked", 0) == 1
        assert "meta_seed_loaded" not in delta
        assert self._record(node) is None, "a declined seed must not mint a record"

    def test_an_all_unreadable_node_is_counted_and_leaves_no_record(self) -> None:
        """The second refusal shape: every tracked slot is gone.

        Reachability receipt for `meta_seed_rejected_unreadable`: no real
        cache-loaded def node reaches it (every tracked slot of the four
        classes is constructor-set), so the counter needs this receipt
        rather than a claimed live reading.
        """
        var = self._var()
        for field in self._m._meta_tracked(Var):
            try:
                delattr(var, field)
            except AttributeError:
                pass
        assert self._unrecorded(var) == [], "deleting the slots leaves nothing to record"
        delta = self._seed_node(var)
        assert delta.get("meta_seed_rejected_unreadable", 0) == 1
        assert "meta_seed_loaded" not in delta

    def test_the_two_provenance_axes_separate_parse_from_cache(self) -> None:
        """Origin and mechanism are independent, in one process.

        A parse-time capture, a fixed-format read and a JSON read must land
        in three different origins, and only the two reads may produce
        seeded records. Must not be collapsed: that is the #1825 control.
        """
        from mypy.nodes import SymbolNode

        before = self._m.report()
        parsed = self._var()
        parsed.is_final = True  # a parse-time capture, no reader involved
        self._round_trip(self._var("mod.cached"))
        var = self._var("mod.json")
        SymbolNode.deserialize(var.serialize())
        delta = self._delta(before)
        assert delta.get("meta_written.parse", 0) > 0, "a parse-time write must be attributed"
        assert delta.get("meta_written.cache_fixed", 0) > 0
        assert delta.get("meta_written.cache_json", 0) > 0, "the JSON reader is its own origin"
        assert delta.get("meta_seeded.cache_fixed", 0) > 0
        assert delta.get("meta_seeded.cache_json", 0) > 0, (
            "the JSON read must seed too: a cache leg that is parsed-only or "
            "fixed-format-only is not the whole cache path"
        )
        assert "meta_seeded.parse" not in delta, (
            "no production reader runs with the parse origin, so a count here "
            "means a call site lost its marker"
        )

    def test_the_origin_marker_is_restored_after_a_read(self) -> None:
        """A leaked marker would mis-attribute every later capture silently."""
        assert self._m._capture_origin == self._m.ORIGIN_PARSE
        self._round_trip(self._var())
        assert self._m._capture_origin == self._m.ORIGIN_PARSE
        before = self._m.report()
        after_parse = self._var("mod.after")
        after_parse.is_final = True
        delta = self._delta(before)
        assert delta.get("meta_written.parse", 0) > 0
        assert "meta_written.cache_fixed" not in delta

    def test_the_reset_boundary_restores_the_origin(self) -> None:
        self._m.cache_read_origin(self._m.ORIGIN_CACHE_FIXED)
        assert self._m._capture_origin == self._m.ORIGIN_CACHE_FIXED
        self._m.reset(clear_counts=True)
        assert self._m._capture_origin == self._m.ORIGIN_PARSE

    def test_the_gate_off_seed_is_inert(self) -> None:
        """Structural-zero control: no store, no counters, no record."""
        saved_active, saved_kernel = self._m._active, self._m._kernel_mod
        self._m._active = False
        self._m._kernel_mod = None
        try:
            assert self._m.seed_loaded(self._var()) == 0
            assert self._m.cache_read_origin(self._m.ORIGIN_CACHE_FIXED) is None
            assert (
                self._m.report() == {}
            ), "an inert store must count nothing at all, not merely not this key"
        finally:
            self._m._active, self._m._kernel_mod = saved_active, saved_kernel

    def test_the_reader_alone_leaves_only_overloaded_items_unrecorded(self) -> None:
        """The honest limit of this lane's evidence.

        With the seed mutated off, `Var`, `FuncDef` and `Decorator` still
        have complete records (their constructors adopt them and the reader
        rewrites every tracked slot), so their *field-absence* assertions do
        not bite; `OverloadedFuncDef` loses exactly two slots. The
        seed-counter assertions are therefore what carries the claim for the
        first three, and this test states that rather than letting a green
        absence assertion imply the seed did the work.
        """
        real = self._m.seed_loaded
        self._m.seed_loaded = lambda node: 0
        try:
            for label, node in (
                ("Var", self._round_trip(self._var())),
                ("FuncDef", self._round_trip(self._func())),
                ("Decorator", self._round_trip(self._decorator())),
            ):
                assert self._unrecorded(node) == [], (
                    f"{label} is fully covered by the reader alone; if this ever "
                    "changes, the seed is load-bearing for it too"
                )
            ofd = self._round_trip(self._overload())
            assert self._unrecorded(ofd) == ["type", "unanalyzed_type"], (
                "the reader skips exactly these two slots of an "
                "OverloadedFuncDef, which is the coverage the seed exists for"
            )
        finally:
            self._m.seed_loaded = real


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class MetaSlotDeletionSuite(Suite):
    """#1841: a `del` on a tracked slot retracts its record.

    The store's contract is "absence means not recorded", but the write
    hook only watched `__setattr__`: a deleted slot kept its stale record,
    so a served read answered from a slot the live object no longer had.
    The `__delattr__` patch now retracts the one field, leaving the rest
    of the entry alone.
    """

    def setUp(self) -> None:
        from mypy import nodes_mirror

        self._m = nodes_mirror
        self._k = _type_kernel
        self._m.activate(audit=True)
        self._m.reset(clear_counts=True)

    def tearDown(self) -> None:
        self._m.reset(clear_counts=True)

    def _record(self, node: Any) -> dict[str, Any] | None:
        handle = self._k.rust_node_mirror_handle_of(node)
        if handle is None:
            return None
        return self._k.rust_node_mirror_meta(handle)

    def _report(self) -> dict[str, int]:
        return self._m.report()

    def test_a_deleted_slot_retracts_its_record(self) -> None:
        var = Var("x")
        var._fullname = "mod.x"
        self._m.seed_loaded(var)  # adopt: the store now holds a record
        record = self._record(var)
        assert record is not None and "is_final" in record
        before = self._report()
        del var.is_final
        after = self._report()
        assert self._record(var) is not None, "the entry itself must survive"
        assert "is_final" not in (self._record(var) or {}), (
            "a deleted slot must not stay recorded: absence keeps meaning " "not recorded (#1841)"
        )
        assert after.get("meta_del.is_final", 0) - before.get("meta_del.is_final", 0) == 1

    def test_the_rest_of_the_entry_survives_a_deletion(self) -> None:
        var = Var("x")
        var._fullname = "mod.x"
        self._m.seed_loaded(var)
        del var.is_final
        record = self._record(var) or {}
        assert record, "deleting one slot must not drop the whole entry"
        assert "type" in record, "an untouched tracked slot keeps its record"

    def test_an_untracked_slot_deletion_touches_nothing(self) -> None:
        var = Var("x")
        var._fullname = "mod.x"
        before = self._report()
        del var.line  # a Node slot Var tracks no record for
        assert self._report() == before, "an untracked deletion is not a store event"

    def test_a_deletion_on_a_never_adopted_node_is_refused_not_crashed(self) -> None:
        before = self._report()
        saved = self._m._active
        self._m._active = False
        try:
            var = Var("y")  # built inert: no tracked write adopted it
        finally:
            self._m._active = saved
        del var.type
        assert self._record(var) is None, "a deletion must not adopt a node"
        assert (
            self._report().get("meta_del_unadopted", 0) - before.get("meta_del_unadopted", 0) == 1
        ), "the refusal is counted, so it cannot read as a silent success"
