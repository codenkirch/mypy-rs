"""Retired-seam pins for `mypy/typeanal.py` (#1739).

Two typeanal shims were micro-benched (min-of-7 ns/call, both arms in one
process, gate flag the only variable) against the Python work they wrapped;
both lost on every shape, so the shim is deleted and the Rust pyfunction stays
registered per the #1640/#1648 precedent.

`rust_validate_instance` (85,556 calls/corpus, 0 defers) replaced an O(n) scan
over `t.args` with a live-object crossing: 4.76x / 3.48x / 4.16x slower than the
Python body on the well-formed / too-few-arg / variadic shapes.

`rust_classify_type_with_info` (82,333 calls/corpus, 0 defers) replaced a
five-fact if-chain with a crossing that allocates the `fullname` string: 1.92x
to 2.77x slower across all six shapes, and on its dominant tag (8, plain
Instance) it displaced no work, since the body it classified to is the body it
then re-ran.

Structural evidence below: neither shim name survives on the module and neither
body loads a `rust_*` global with the gate ON, which is zero crossings. Value
evidence: gate-off and gate-on runs agree on `(result, messages)` for every
shape, and direct `type_kernel` calls prove both pyfunctions stay reachable.
"""

from __future__ import annotations

from typing import Any
from unittest import skipUnless

from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED
from mypy.test.typefixture import TypeFixture
from mypy.typeanal import TypeAnalyser, validate_instance
from mypy.types import Instance, Type, UnboundType


class _FakeApi:
    """Just enough `SemanticAnalyzerCoreInterface` for the typeanal bodies."""

    def __init__(self) -> None:
        self.final_iteration = False
        self.errors: list[str] = []

    def fail(self, msg: str, ctx: Any, code: Any = None) -> None:
        self.errors.append(msg)

    def note(self, msg: str, ctx: Any, code: Any = None) -> None:
        self.errors.append(f"note: {msg}")


def _analyser(api: _FakeApi) -> TypeAnalyser:
    ta = TypeAnalyser.__new__(TypeAnalyser)
    ta.api = api  # type: ignore[assignment]
    ta.fail_func = api.fail  # type: ignore[assignment]
    ta.note_func = api.note
    ta.tvar_scope = None  # type: ignore[assignment]
    from mypy.options import Options

    ta.options = Options()
    ta.defining_alias = False
    ta.nesting_level = 0
    ta.allow_type_any = False
    ta.allow_placeholder = False
    ta.allow_param_spec_literals = False
    ta.allow_type_var_tuple = -1
    ta.allow_unpack = False
    ta.allow_ellipsis = False
    ta.allow_final = False
    ta.allow_typed_dict_special_forms = False
    ta.allow_tuple_literal = False
    ta.allow_unbound_tvars = False
    ta.alias_type_params_names = None
    ta.allowed_alias_tvars = []
    ta.erase_tvar_defs = []
    ta.aliases_used = set()
    ta.is_typeshed_stub = False
    ta.analyzing_tvar_def = False
    ta.python_3_12_type_alias = False
    return ta


def _noop_fail(msg: str, ctx: Any, code: Any = None) -> None:
    """`MsgCallback` stand-in; the native path calls it with `code=`."""


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeValidateInstanceRetiredSuite(Suite):
    """Pin the #1739 retirement of `validate_instance`'s Rust shim."""

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def _shapes(self) -> list[tuple[str, Instance, bool, list[str]]]:
        gen = self.fx.make_type_info("mod.Gen", typevars=["T"])
        return [
            ("well-formed", Instance(gen, [self.fx.a]), False, []),
            (
                "too-few-empty",
                Instance(gen, []),
                True,
                ['"mod.Gen" expects 1 type argument, but none given'],
            ),
            ("variadic", Instance(self.fx.gvi, [self.fx.a]), False, []),
        ]

    def _run(self, instance: Instance, indexed: bool) -> tuple[bool, list[str]]:
        api = _FakeApi()
        messages: list[str] = []
        result = validate_instance(
            instance, lambda msg, ctx, code=None: messages.append(msg), indexed
        )
        assert api.errors == []
        return result, messages

    def test_shim_name_gone(self) -> None:
        import inspect

        from mypy import typeanal

        assert not hasattr(typeanal, "_rust_validate_instance")
        src = inspect.getsource(typeanal.validate_instance)
        assert "rust_" not in src, "validate_instance should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        _set_native_typeanal_active(True)
        try:
            loaded = [n for n in validate_instance.__code__.co_names if "rust_" in n]
            assert loaded == [], f"validate_instance still loads {loaded}"
        finally:
            _set_native_typeanal_active(False)

    def test_values_match_python_on_both_gates(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        for label, instance, indexed, want_messages in self._shapes():
            _set_native_typeanal_active(False)
            off = self._run(instance, indexed)
            _set_native_typeanal_active(True)
            try:
                on = self._run(instance, indexed)
            finally:
                _set_native_typeanal_active(False)
            assert_equal(on, off, f"validate_instance parity {label}")
            assert_equal(on[1], want_messages, f"validate_instance messages {label}")

    def test_pyfunction_stays_registered(self) -> None:
        import type_kernel

        gen = self.fx.make_type_info("mod.Gen", typevars=["T"])
        well_formed = type_kernel.rust_validate_instance(
            Instance(gen, [self.fx.a]), _noop_fail, False
        )
        too_few = type_kernel.rust_validate_instance(Instance(gen, []), _noop_fail, True)
        assert well_formed is True, well_formed
        assert too_few is False, too_few


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassifyTypeWithInfoRetiredSuite(Suite):
    """Pin the #1739 retirement of `analyze_type_with_type_info`'s classifier.

    The classification ran in Rust from raw node facts and returned a branch
    tag; Python acted inline on the tuple (1) and `types.NoneType` (7) tags and
    re-ran the body for the rest. The five-fact chain was cheaper in Python on
    all six shapes, and on tag 8 the crossing displaced no work.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def _call(
        self, info: Any, args: list[Type], empty_tuple_index: bool = False
    ) -> tuple[str, list[str]]:
        api = _FakeApi()
        ta = _analyser(api)
        result = TypeAnalyser.analyze_type_with_type_info(
            ta, info, list(args), UnboundType("ctx"), empty_tuple_index
        )
        return str(result), [m for m in api.errors if not m.startswith("note: ")]

    def _shapes(self) -> list[tuple[str, Any, list[Type]]]:
        return [
            (
                "tuple-2-args",
                self.fx.make_type_info("builtins.tuple", typevars=["T"]),
                [self.fx.a, self.fx.b],
            ),
            ("none-type", self.fx.make_type_info("types.NoneType"), []),
            ("plain-no-args", self.fx.make_type_info("mod.UserClass"), []),
            (
                "generic-1-arg",
                self.fx.make_type_info("mod.UserClass", typevars=["T"]),
                [self.fx.a],
            ),
        ]

    def test_shim_name_gone(self) -> None:
        from mypy import typeanal

        assert not hasattr(typeanal, "_rust_classify_type_with_info")

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        _set_native_typeanal_active(True)
        try:
            code = TypeAnalyser.analyze_type_with_type_info.__code__
            loaded = [n for n in code.co_names if "rust_" in n]
            assert loaded == [], f"analyze_type_with_type_info still loads {loaded}"
        finally:
            _set_native_typeanal_active(False)

    def test_values_match_python_on_both_gates(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        for label, info, args in self._shapes():
            _set_native_typeanal_active(False)
            off = self._call(info, args)
            _set_native_typeanal_active(True)
            try:
                on = self._call(info, args)
            finally:
                _set_native_typeanal_active(False)
            assert_equal(on, off, f"analyze_type_with_type_info parity {label}")
        _, none_messages = self._call(self.fx.make_type_info("types.NoneType"), [])
        assert len(none_messages) == 1, none_messages

    def test_pyfunction_stays_registered(self) -> None:
        import type_kernel

        assert (
            type_kernel.rust_classify_type_with_info("builtins.tuple", 2, True, False, False) == 1
        )
        assert (
            type_kernel.rust_classify_type_with_info("types.NoneType", 0, False, False, False) == 7
        )
        assert (
            type_kernel.rust_classify_type_with_info("mod.UserClass", 0, False, False, False) == 8
        )
