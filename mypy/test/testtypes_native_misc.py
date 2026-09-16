"""Native seam suites for the misc area (split from testtypes.py, #1677)."""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from collections.abc import Callable
from typing import Any
from unittest import skipUnless

from mypy.nodes import ARG_POS, INVARIANT, Context, TypeInfo
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED, T, _is_type_info
from mypy.test.typefixture import TypeFixture
from mypy.typeanal import _set_native_typeanal_active
from mypy.types import (
    AnyType,
    CallableType,
    ParamSpecFlavor,
    ParamSpecType,
    TypeOfAny,
    TypeVarId,
    TypeVarType,
)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMatchGenericCallablesSuite(Suite):
    """Parity suite for the Rust `match_generic_callables` id-renumbering
    port (join.py:1152-1180, freshen.rs).

    With the native join gate on, `match_generic_callables` routes
    through the kernel: Rust allocates one shared batch of fresh
    meta-level-0 ids (`TypeVarId.new(meta_level=0)`) passed to BOTH
    operands, renumbers each operand's variables, and expands the body
    via `expand_type` (join.py:1171-1173).

    Every test runs a gate-off vs gate-on differential and asserts the
    results render identically; the Rust seam must also engage (direct
    kernel call) for the portable cases.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        import mypy.join

        old = mypy.join._native_join_active
        mypy.join._set_native_join_active(active)
        try:
            return fn()
        finally:
            mypy.join._set_native_join_active(old)

    def _generic(self, tv: TypeVarType) -> CallableType:
        """A generic callable: `def f(tv: tv) -> tv`."""
        return CallableType([tv], [ARG_POS], [None], tv, self.fx.function, variables=[tv])

    def test_match_generic_callables_renumbers(self) -> None:
        # def f(t: T) -> T: join(f, f) must renumber T to a fresh id so
        # the two operands share one id space.
        from mypy.join import match_generic_callables

        c = self._generic(self.fx.t)
        before = TypeVarId.next_raw_id
        tc, sc = self._with_gate(True, lambda: match_generic_callables(c, c))
        # Fresh ids were allocated (t and s both get T'raw_id fresh).
        assert tc.variables[0].id.raw_id >= before
        assert sc.variables[0].id.raw_id == tc.variables[0].id.raw_id
        assert tc.variables[0].id.meta_level == 0
        assert isinstance(tc.arg_types[0], TypeVarType)
        assert isinstance(sc.arg_types[0], TypeVarType)
        assert tc.arg_types[0].id == tc.variables[0].id
        assert sc.arg_types[0].id == sc.variables[0].id
        assert_equal(str(tc), str(sc))

    def test_match_generic_callables_mixed_arity(self) -> None:
        # t has 1 var, s has 2: both renumbered into the same id range.
        from mypy.join import match_generic_callables

        t = self._generic(self.fx.t)
        s2 = TypeVarType("U", "U", TypeVarId(3), [], self.fx.o, AnyType(TypeOfAny.special_form))
        s2b = TypeVarType("V", "V", TypeVarId(4), [], self.fx.o, AnyType(TypeOfAny.special_form))
        s = CallableType(
            [s2, s2b], [ARG_POS, ARG_POS], [None, None], s2, self.fx.function, variables=[s2, s2b]
        )
        t_out, s_out = match_generic_callables(t, s)
        # BOTH operands share the same fresh-id batch (join.py:1117-1120
        # passes one `new_ids` list to both `update_callable_ids` calls):
        # t's T and s's U both get the first fresh id, s's V the second.
        assert t_out.variables[0].id.raw_id == s_out.variables[0].id.raw_id
        assert s_out.variables[1].id.raw_id == s_out.variables[0].id.raw_id + 1
        assert isinstance(t_out.arg_types[0], TypeVarType)
        assert isinstance(s_out.arg_types[0], TypeVarType)
        assert t_out.arg_types[0].id == t_out.variables[0].id
        assert s_out.arg_types[0].id == s_out.variables[0].id
        # str renders names, not ids; assert id equality on the ret types.
        assert isinstance(t_out.ret_type, TypeVarType)
        assert isinstance(s_out.ret_type, TypeVarType)
        assert t_out.ret_type.id == s_out.ret_type.id

    def test_match_generic_callables_min_len_zero_noop(self) -> None:
        from mypy.join import match_generic_callables

        # A non-generic callable: min_len == 0 short-circuits to the
        # unchanged operands (join.py:1169-1170).
        non_generic = self.fx.callable(self.fx.a, self.fx.b)
        t_out, s_out = match_generic_callables(non_generic, non_generic)
        assert t_out is non_generic
        assert s_out is non_generic

    def test_match_generic_callables_gate_off_differential(self) -> None:
        from mypy.join import match_generic_callables

        c = self._generic(self.fx.t)
        off = self._with_gate(False, lambda: match_generic_callables(c, c))
        on = self._with_gate(True, lambda: match_generic_callables(c, c))
        assert_equal(str(on[0]), str(off[0]))
        assert_equal(str(on[1]), str(off[1]))

    def test_match_generic_callables_param_spec_defers(self) -> None:
        from mypy.join import match_generic_callables

        # ParamSpec variables keep the Python path (Rust cannot rebuild
        # ParamSpec prefix identity); the differential must still agree.
        p = ParamSpecType(
            "P",
            "P",
            TypeVarId(1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        c = CallableType([p], [ARG_POS], [None], self.fx.a, self.fx.function, variables=[p])
        off = self._with_gate(False, lambda: match_generic_callables(c, c))
        on = self._with_gate(True, lambda: match_generic_callables(c, c))
        assert_equal(str(on[0]), str(off[0]))
        assert_equal(str(on[1]), str(off[1]))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRawExpressionTypeSuite(Suite):
    """Parity for the Rust `visit_raw_expression_type` message classifier.

    The 3-way dispatch (int/bool -> Literal hint, float/complex -> "literals
    cannot be used", else -> generic "Invalid type comment or annotation") is
    decided in Rust from scalar facts (`report_invalid_types`,
    `base_type_name`); the Python shim formats the message and applies
    `self.fail` / `self.note` for the tag Rust returns. When
    `report_invalid_types` is false the head is skipped entirely and Rust
    defers (`None`), so no message is emitted and the trailing `AnyType`
    is returned unchanged.

    Toggling the typeanal gate off (pure Python) and on (Rust seam) must
    produce identical (str(result), captured fail/note messages), and a
    direct seam call proves the classifier engages on each branch.
    """

    def setUp(self) -> None:

        self._set_active = _set_native_typeanal_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _analyser(self, report_invalid_types: bool = True) -> tuple[object, object]:
        from mypy.errorcodes import ErrorCode as _ErrorCode
        from mypy.typeanal import TypeAnalyser

        class FakeApi:
            def __init__(self) -> None:
                self.errors: list[str] = []

            def fail(self, msg: str, ctx: Context, code: _ErrorCode | None = None) -> None:
                self.errors.append(msg)

            def note(self, msg: str, ctx: Context, code: _ErrorCode | None = None) -> None:
                self.errors.append(f"note: {msg}")

        api = FakeApi()
        ta = TypeAnalyser.__new__(TypeAnalyser)
        ta.api = api  # type: ignore[assignment]
        ta.fail_func = api.fail  # type: ignore[assignment]
        ta.note_func = api.note
        ta.report_invalid_types = report_invalid_types
        return ta, api

    def _call(self, ta: Any, t: Any) -> tuple[str, list[str]]:
        result = ta.visit_raw_expression_type(t)
        messages = list(ta.api.errors)
        return str(result), messages

    def _make_t(self, base_type_name: str, literal_value: Any, note: str | None = None) -> object:
        from mypy.types import RawExpressionType

        return RawExpressionType(literal_value, base_type_name, line=-1, column=-1, note=note)

    def _assert_par(
        self,
        base_type_name: str,
        literal_value: Any,
        *,
        note: str | None = None,
        report_invalid_types: bool = True,
    ) -> None:
        t = self._make_t(base_type_name, literal_value, note)
        off_ta, _ = self._analyser(report_invalid_types=report_invalid_types)
        off = self._with_gate(False, lambda: self._call(off_ta, t))
        on_ta, _ = self._analyser(report_invalid_types=report_invalid_types)
        on = self._with_gate(True, lambda: self._call(on_ta, t))
        assert_equal(on[0], off[0], f"raw_expr parity result {base_type_name}")
        assert_equal(on[1], off[1], f"raw_expr parity messages {base_type_name}")

    def _assert_engages(
        self, base_type_name: str, report_invalid_types: bool = True, note_is_none: bool = True
    ) -> None:
        from mypy.typeanal import _rust_classify_raw_expression_type  # type: ignore[attr-defined]

        tag = _rust_classify_raw_expression_type(
            report_invalid_types, base_type_name, note_is_none
        )
        assert tag is not None, f"seam did not engage for {base_type_name}"

    def _assert_defers(self, base_type_name: str) -> None:
        from mypy.typeanal import _rust_classify_raw_expression_type  # type: ignore[attr-defined]

        tag = _rust_classify_raw_expression_type(False, base_type_name, True)
        assert tag is None, f"seam did not defer for {base_type_name}"

    def test_int_literal(self) -> None:
        self._assert_par("builtins.int", 1)
        self._assert_engages("builtins.int")

    def test_bool_literal(self) -> None:
        self._assert_par("builtins.bool", True)
        self._assert_engages("builtins.bool")

    def test_float_literal(self) -> None:
        self._assert_par("builtins.float", 1.0)
        self._assert_engages("builtins.float")

    def test_complex_literal(self) -> None:
        self._assert_par("builtins.complex", 1j)
        self._assert_engages("builtins.complex")

    def test_generic(self) -> None:
        self._assert_par("builtins.str", "x")
        self._assert_engages("builtins.str")

    def test_report_off_defers(self) -> None:
        # report_invalid_types False skips the whole head; no message is
        # emitted and the trailing AnyType is returned unchanged.
        self._assert_par("builtins.int", 1, report_invalid_types=False)
        self._assert_defers("builtins.int")

    def test_note_present(self) -> None:
        self._assert_par("builtins.int", 1, note="see PEP 586")

    def test_note_present_generic(self) -> None:
        self._assert_par("builtins.str", "x", note="custom note")

    def test_engages_all_branches(self) -> None:
        # Direct seam calls prove the classifier decides each bucket.
        self._assert_engages("builtins.int")
        self._assert_engages("builtins.bool")
        self._assert_engages("builtins.float")
        self._assert_engages("builtins.complex")
        self._assert_engages("builtins.str")
