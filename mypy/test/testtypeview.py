"""Tests for the `Instance` replacement-view gate (#1671).

Two layers, deliberately split:

- The Rust store (`crates/type_kernel/src/typeview.rs`) has its own unit
  tests for the entry lifecycle, the staleness refusals, and the byte
  layout.
- This module tests the *seam*: that the gate preserves the `Instance`
  class contract, that a served encode is byte-identical to the Python
  walk it replaces, and that every unprovable case defers instead of
  guessing.

The byte-parity tests are the load-bearing ones. The funnel hands these
bytes straight to a Rust seam, so a view that emits one wrong byte is a
silent wrong answer downstream, not a crash.

Unittest style on purpose: this repo sets `python_functions = []` and
`python_classes = []`, so bare pytest functions are never collected.
"""

from __future__ import annotations

import unittest
from typing import Any

import mypy.typeview as typeview
import mypy.types as types
from mypy.nodes import ARG_POS, TypeInfo
from mypy.test.typefixture import TypeFixture
from mypy.types import Instance, Type, TypeVarType


def _kernel_available() -> bool:
    try:
        import type_kernel
    except ImportError:
        return False
    return hasattr(type_kernel, "rust_view_put")


def _reference_bytes(t: Type) -> bytes:
    """The encode the Python walk produces, with the view hook forced off.

    The hook is disabled for the call rather than the gate being torn down,
    so a test can compare the served encode against the walk it replaces
    without perturbing the activated class shape.
    """
    saved = types._native_type_view_encode
    types._set_native_type_view_encode(None)
    try:
        return types._serialize_type_for_visitor(t)
    finally:
        types._set_native_type_view_encode(saved)


class TypeViewSuite(unittest.TestCase):
    """The `Instance` replacement-view seam (issue #1671)."""

    fx: TypeFixture

    @classmethod
    def setUpClass(cls) -> None:
        if not _kernel_available():
            raise unittest.SkipTest("the rebuilt type_kernel extension is not on PYTHONPATH")
        cls.fx = TypeFixture()

    def setUp(self) -> None:
        # `types_mirror.activate()` patches `Instance.__setattr__` too, and
        # this process is shared with the mirror suites in testtypes.py, so the
        # invariant under test is a restore of whatever hook was found.
        typeview.deactivate()
        self.addCleanup(typeview.deactivate)
        self.setattr_before = Instance.__dict__.get("__setattr__", object.__setattr__)
        typeview.reset(clear_counts=True)

    def _activate(self, *, read_route: bool = False) -> None:
        if not typeview.activate(read_route=read_route, audit=True):
            raise unittest.SkipTest("type_kernel exposes no view store")
        typeview.reset(clear_counts=True)

    @staticmethod
    def _register(*instances: Any) -> None:
        for inst in instances:
            typeview.register(inst)

    # ---- class contract ----

    def test_gate_off_keeps_the_plain_slot_shape(self) -> None:
        """An unactivated gate installs nothing on `Instance`."""
        self.assertFalse(typeview.active())
        self.assertIsNone(types._native_type_view_encode)
        self.assertIs(Instance.__dict__.get("__setattr__", object.__setattr__), self.setattr_before)
        # `args` is still the `__slots__` member descriptor, not a property.
        self.assertNotIsInstance(Instance.__dict__["args"], property)
        self.assertIn("args", Instance.__slots__)

    def test_activation_routes_writes_and_restores(self) -> None:
        self._activate()
        self.assertTrue(typeview.active())
        self.assertIsNotNone(types._native_type_view_encode)
        self.assertIs(Instance.__dict__["__setattr__"], typeview._inst_setattr)
        typeview.deactivate()
        self.assertFalse(typeview.active())
        self.assertIsNone(types._native_type_view_encode)
        self.assertIs(Instance.__dict__.get("__setattr__", object.__setattr__), self.setattr_before)
        self.assertNotIsInstance(Instance.__dict__["args"], property)

    def test_chains_and_restores_a_pre_existing_setattr(self) -> None:
        """A co-active hook (the mirror's) must survive the gate's install."""
        seen: list[str] = []

        def foreign(inst: object, name: str, value: object) -> None:
            seen.append(name)
            object.__setattr__(inst, name, value)

        # `None` means "there was none", which the finally-block restores by
        # deletion: installing `object.__setattr__` instead would leak a class
        # shape change into every later test in this shared process.
        original = Instance.__dict__.get("__setattr__")
        Instance.__setattr__ = foreign  # type: ignore[method-assign]
        try:
            self._activate()
            inst = Instance(self.fx.std_listi, [self.fx.a])
            self.assertIn("args", seen)
            del inst
            typeview.deactivate()
            self.assertIs(Instance.__dict__["__setattr__"], foreign)
        finally:
            if original is None:
                del Instance.__setattr__
            else:
                Instance.__setattr__ = original  # type: ignore[method-assign]

    def test_deactivate_is_idempotent(self) -> None:
        self._activate()
        typeview.deactivate()
        typeview.deactivate()
        self.assertIs(Instance.__dict__.get("__setattr__", object.__setattr__), self.setattr_before)

    def test_read_arm_installs_and_restores_the_args_descriptor(self) -> None:
        self._activate(read_route=True)
        self.assertIsInstance(Instance.__dict__["args"], property)
        typeview.deactivate()
        self.assertNotIsInstance(Instance.__dict__["args"], property)
        self.assertIn("args", Instance.__slots__)

    def test_activation_reasserts_a_hook_stolen_by_a_co_resident(self) -> None:
        """Order must not matter: a later co-resident hook cannot go inert."""
        self._activate()
        stolen: list[str] = []

        def foreign(inst: object, name: str, value: object) -> None:
            stolen.append(name)
            object.__setattr__(inst, name, value)

        Instance.__setattr__ = foreign  # type: ignore[method-assign]
        # Already active, so this path must re-assert rather than assume.
        self._activate()
        self.assertIs(Instance.__dict__["__setattr__"], typeview._inst_setattr)
        # And the stolen hook is now the chain link, so neither mechanism is
        # silently inert.
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self.assertIn("args", stolen)
        del inst

    def test_arm_switch_two_to_one_uninstalls_the_route(self) -> None:
        """2 -> 1 must undo the routed property, or `_slot_get` recurses."""
        self._activate(read_route=True)
        self.assertIsInstance(Instance.__dict__["args"], property)
        self._activate(read_route=False)
        self.assertNotIsInstance(Instance.__dict__["args"], property)
        # A route miss on an unregistered instance must reach the slot, not
        # the property's own getter (unbounded recursion).
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self.assertEqual(inst.args, (self.fx.a,))

    def test_arm_switch_keeps_the_slot_descriptors(self) -> None:
        """A re-install must not capture its own property as the storage."""
        self._activate(read_route=True)
        self._activate(read_route=True)
        self.assertIsInstance(Instance.__dict__["args"], property)
        self.assertNotIsInstance(typeview._MEMBERS["args"], property)
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self._register(self.fx.a, inst)
        self.assertIsNotNone(typeview.encode(inst))

    # ---- byte parity ----

    def test_encode_byte_parity_leaf(self) -> None:
        """A no-arg Instance is served and matches the walk byte for byte."""
        reference = _reference_bytes(self.fx.a)
        self._activate()
        self._register(self.fx.a)
        served = typeview.encode(self.fx.a)
        self.assertEqual(served, reference)
        self.assertEqual(typeview.stats()["encodes"], 1)
        self.assertEqual(typeview.stats()["defers"], 0)

    def test_encode_byte_parity_simple_singleton(self) -> None:
        reference = _reference_bytes(self.fx.str_type)
        self._activate()
        self._register(self.fx.str_type)
        self.assertEqual(typeview.encode(self.fx.str_type), reference)

    def test_encode_byte_parity_generic(self) -> None:
        inst = Instance(self.fx.std_listi, [self.fx.a])
        reference = _reference_bytes(inst)
        self._activate()
        self._register(self.fx.a, inst)
        self.assertEqual(typeview.encode(inst), reference)

    def test_encode_byte_parity_two_args(self) -> None:
        inst = Instance(self.fx.std_tuplei, [self.fx.a, self.fx.b])
        reference = _reference_bytes(inst)
        self._activate()
        self._register(self.fx.a, self.fx.b, inst)
        self.assertEqual(typeview.encode(inst), reference)

    def test_encode_byte_parity_nested(self) -> None:
        """Recursion over registered Instance children stays byte-identical."""
        inner = Instance(self.fx.std_listi, [self.fx.a])
        outer = Instance(self.fx.std_listi, [inner])
        reference = _reference_bytes(outer)
        self._activate()
        self._register(self.fx.a, inner, outer)
        self.assertEqual(typeview.encode(outer), reference)
        self.assertGreater(len(reference), 8)

    # ---- refusals: a miss, never a wrong byte ----

    def test_encode_defers_on_unregistered_argument(self) -> None:
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self._activate()
        self._register(inst)  # `fx.a` is intentionally left unregistered
        self.assertIsNone(typeview.encode(inst))
        # The walk still produces the right bytes.
        self.assertTrue(_reference_bytes(inst))

    def test_encode_defers_on_non_instance(self) -> None:
        """Only the `Instance` family is served."""
        self._activate()
        self.assertIsNone(typeview.encode(self.fx.t))
        self.assertIsNone(typeview.encode(self.fx.anyt))
        self.assertIsNone(
            typeview.encode(
                types.CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function)
            )
        )

    def test_encode_defers_on_typevar_argument(self) -> None:
        """A tvar-carrying argument set is refused (the F3 cache rule)."""
        tvar: TypeVarType = self.fx.t
        inst = Instance(self.fx.std_listi, [tvar])
        self._activate()
        self._register(tvar, inst)
        self.assertIsNone(typeview.encode(inst))

    def test_encode_defers_on_side_field(self) -> None:
        """`last_known_value` and `extra_attrs` refuse the entry entirely."""
        inst = Instance(self.fx.std_listi, [self.fx.a])
        inst.extra_attrs = types.ExtraAttrs({"x": self.fx.a})
        self._activate()
        self._register(self.fx.a, inst)
        self.assertIsNone(typeview.encode(inst))
        self.assertTrue(_reference_bytes(inst))

    def test_extra_attrs_mutation_in_place_is_never_served(self) -> None:
        """ExtraAttrs mutated through its own dict cannot produce stale bytes."""
        inst = Instance(self.fx.std_listi, [self.fx.a])
        inst.extra_attrs = types.ExtraAttrs({})
        self._activate()
        self._register(self.fx.a, inst)
        self.assertIsNone(typeview.encode(inst))
        inst.extra_attrs.attrs["y"] = self.fx.b
        self.assertIsNone(typeview.encode(inst))

    def test_pre_fixup_instance_defers(self) -> None:
        """`type_ref is not None` marks a wire-decoded instance; never served."""
        inst = Instance(self.fx.std_listi, [self.fx.a])
        inst.type_ref = "builtins.list"
        self._activate()
        self._register(self.fx.a, inst)
        self.assertIsNone(typeview.encode(inst))
        inst.type_ref = None
        self._register(inst)
        self.assertIsNotNone(typeview.encode(inst))

    def test_retyped_instance_in_place_defers(self) -> None:
        """`Instance.type` has no writer hook, so the live fullname is verified."""
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self._activate()
        self._register(self.fx.a, inst)
        self.assertIsNotNone(typeview.encode(inst))
        # Retype in place. `type` is not a view field, so no entry is
        # dropped; the serve must refuse on the fullname mismatch instead.
        object.__setattr__(inst, "type", self.fx.std_tuplei)
        self.assertIsNone(typeview.encode(inst))

    # ---- write-through coherence ----

    def test_write_invalidates_and_reregisters(self) -> None:
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self._activate()
        self._register(self.fx.a, inst)
        self.assertIsNotNone(typeview.encode(inst))
        inst.args = (self.fx.b,)
        self._register(self.fx.b)
        self.assertEqual(typeview.encode(inst), _reference_bytes(inst))

    def test_write_to_unregistered_argument_defers(self) -> None:
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self._activate()
        self._register(self.fx.a, inst)
        self.assertIsNotNone(typeview.encode(inst))
        inst.args = (self.fx.b,)  # `fx.b` was never registered
        self.assertIsNone(typeview.encode(inst))

    def test_init_registers_without_an_explicit_call(self) -> None:
        """The `__setattr__` hook registers from the constructor writes."""
        self._activate()
        inst = Instance(self.fx.std_listi, [])
        self.assertIsNotNone(typeview.encode(inst))

    def test_unservable_instance_is_attempted_once_per_stamp(self) -> None:
        """The late-registration retry is memoized, not paid per seam call."""
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self._activate()
        self._register(inst)
        self.assertIsNone(typeview.encode(inst))
        first = typeview.report().get("a_encode.miss", 0)
        first_scan = typeview.report().get("a_register.unregistered_arg", 0)
        self.assertIsNone(typeview.encode(inst))
        self.assertIsNone(typeview.encode(inst))
        after = typeview.report().get("a_encode.miss", 0)
        self.assertEqual(after - first, 2)
        # The memoized part: two more calls must not pay the registration scan
        # again. Only a register-side counter can tell memoized from not.
        self.assertEqual(
            typeview.report().get("a_register.unregistered_arg", 0) - first_scan, 0
        )
        # A successful registration clears the memo.
        self._register(self.fx.a, inst)
        self.assertIsNotNone(typeview.encode(inst))

    def test_not_ready_typeinfo_never_registers(self) -> None:
        """`register` is total: a placeholder TypeInfo drops, never raises."""
        self._activate()
        inst = Instance(types.NOT_READY, [])
        typeview.register(inst)
        self.assertIsNone(typeview.encode(inst))

    # ---- read arm ----

    def test_args_read_route_matches_the_slot(self) -> None:
        first = Instance(self.fx.std_listi, [self.fx.a])
        self._activate(read_route=True)
        seed = Instance(self.fx.std_listi, [self.fx.a, first])
        self._register(self.fx.a, first, seed)
        self.assertIsNotNone(typeview.encode(seed))
        routed = seed.args
        self.assertIsInstance(routed, tuple)
        self.assertEqual(list(routed), [self.fx.a, first])
        for got, want in zip(routed, (self.fx.a, first)):
            self.assertIs(got, want)
        self.assertGreaterEqual(typeview.stats()["read_routes"], 1)

    def test_read_arm_does_not_change_bytes(self) -> None:
        inst = Instance(self.fx.std_listi, [self.fx.a])
        reference = _reference_bytes(inst)
        self._activate(read_route=True)
        self._register(self.fx.a, inst)
        self.assertEqual(typeview.encode(inst), reference)

    def test_read_arm_falls_back_when_the_entry_is_stale(self) -> None:
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self._activate(read_route=True)
        self._register(self.fx.a, inst)
        typeview.reset()
        self.assertEqual(inst.args, (self.fx.a,))

    # ---- lifecycle ----

    def test_reset_drops_entries(self) -> None:
        inst = Instance(self.fx.std_listi, [self.fx.a])
        self._activate()
        self._register(self.fx.a, inst)
        self.assertGreaterEqual(typeview.stats()["entries"], 2)
        typeview.reset()
        self.assertEqual(typeview.stats()["entries"], 0)
        self.assertIsNone(typeview.encode(inst))

    # ---- the whole-graph differential ----

    def test_funnel_differential_over_a_mixed_graph(self) -> None:
        """Gate-off and gate-on funnel calls agree across a mixed graph.

        The point of the gate is that it is invisible: every type the store
        cannot serve must produce the same bytes as the walk.
        """
        graph: list[Type] = [
            self.fx.a,
            self.fx.b,
            self.fx.o,
            self.fx.str_type,
            Instance(self.fx.std_listi, [self.fx.a]),
            Instance(self.fx.std_listi, [self.fx.a, self.fx.b]),
            Instance(self.fx.std_tuplei, [self.fx.str_type, self.fx.a]),
            Instance(self.fx.std_listi, []),
            self.fx.t,
            self.fx.nonet,
            self.fx.anyt,
            types.CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function),
            types.TupleType([self.fx.a, self.fx.b], Instance(self.fx.std_tuplei, [])),
            types.UnionType([self.fx.a, self.fx.b]),
            self.fx.lit_str1_inst,
        ]
        typeview.deactivate()
        reference = [_reference_bytes(t) for t in graph]
        self._activate()
        for t in graph:
            typeview.register(t)
            if type(t) is Instance:
                for arg in t.args:
                    typeview.register(arg)
        for original, expected in zip(graph, reference):
            self.assertEqual(types._serialize_type_for_visitor(original), expected)

    def test_served_bytes_feed_a_real_kernel_seam(self) -> None:
        """A served encode drives an actual seam, not just a byte compare."""
        from type_kernel import rust_can_be_true_default, rust_has_recursive_types

        inner = Instance(self.fx.std_listi, [])
        recursive = Instance(self.fx.std_listi, [inner])
        reference = _reference_bytes(recursive)
        self._activate()
        self._register(inner, recursive)
        served = typeview.encode(recursive)
        self.assertIsNotNone(served)
        assert served is not None
        self.assertEqual(rust_has_recursive_types(served), rust_has_recursive_types(reference))
        self.assertEqual(rust_can_be_true_default(served), rust_can_be_true_default(reference))

    def test_typeinfo_fullname_is_the_wire_key(self) -> None:
        """Guard on the assumption the store's byte layout rests on."""
        info = self.fx.std_listi
        assert isinstance(info, TypeInfo)
        self.assertTrue(info.fullname.startswith("builtins."))


if __name__ == "__main__":
    unittest.main()
