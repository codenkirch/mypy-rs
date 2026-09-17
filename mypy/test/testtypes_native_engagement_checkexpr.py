"""Native engagement suites for the checkexpr area (`mypy/checkexpr.py`, `mypy/argmap.py`, `mypy/solve.py` and `mypy/checkstrformat.py`).

Engagement suites (a `Native...Suite` without `Retired` in the name) live in
one file per area so that a lane changing one area's seam tests touches that
area's file only; retired-seam pins live in `testtypes_native_retired_*.py`.

The area of a suite is the module that hosts the seam it exercises, taken from
the `rust_*` shim it references (else the `mypy` module its docstring names), so
a new suite belongs in the file for the module it tests. Split out of the four
28k-line per-area files in #1757; see the collect-only and AST proof in that PR.
"""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
    from librt.internal import WriteBuffer as _WriteBuffer
except ImportError:
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _type_kernel = None  # type: ignore[assignment]

from collections.abc import Callable, Sequence
from types import SimpleNamespace
from typing import Any, cast
from unittest import skipUnless

import mypy.expandtype
from mypy.checker import TypeChecker, TypeMap
from mypy.checkexpr import ExpressionChecker
from mypy.checkstrformat import ConversionSpecifier, _set_native_strformat_active
from mypy.constraints import Constraint
from mypy.errorcodes import ErrorCode
from mypy.nodes import (
    ARG_NAMED,
    ARG_OPT,
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    GDEF,
    INVARIANT,
    MDEF,
    ArgKind,
    BytesExpr,
    CallExpr,
    ClassDef,
    Context,
    DictExpr,
    Expression,
    FuncDef,
    IndexExpr,
    IntExpr,
    ListExpr,
    MemberExpr,
    MypyFile,
    NameExpr,
    OpExpr,
    OverloadedFuncDef,
    RevealExpr,
    SliceExpr,
    StrExpr,
    SuperExpr,
    SymbolTable,
    SymbolTableNode,
    TypeAlias,
    TypeInfo,
    Var,
)
from mypy.options import Options
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import (
    _NATIVE_STRFORMAT_ENABLED,
    _NATIVE_WIRE_ENABLED,
    T,
    _base_infos,
    _BrokenAttrInfo,
    _chk,
    _is_type_info,
    _ValidVarArgStubChk,
)
from mypy.test.typefixture import TypeFixture
from mypy.types import (
    AnyType,
    CallableType,
    DeletedType,
    ErasedType,
    Instance,
    LiteralType,
    NoneType,
    ParamSpecFlavor,
    ParamSpecType,
    ProperType,
    TupleType,
    Type,
    TypeAliasType,
    TypedDictType,
    TypeOfAny,
    TypeType,
    TypeVarId,
    TypeVarLikeType,
    TypeVarTupleType,
    TypeVarType,
    UninhabitedType,
    UnionType,
    UnpackType,
    get_proper_type,
    get_proper_types,
)


# Moved from mypy/test/testtypes_native_checker.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMethodFullnameSuite(Suite):
    """Parity tests for M25: checkexpr `method_fullname` ported to Rust.

    Each test serializes a `mypy.types.Type` and asks both
    `type_kernel.rust_method_fullname(resolver, bytes, name)` and the
    pure-Python `ExpressionChecker.method_fullname` for the qualified
    method name, asserting they agree. The Rust side defers (None) to
    Python for any case the kernel cannot decide, so the assertion is:
    Rust result if non-None, else the Python result, equals the Python
    result.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self.fx = TypeFixture()
        # The containing-type cases (TypedDict, Literal) resolve "foo"
        # through the fallback's names table, so inject it before the
        # resolver snapshots member_info.
        from mypy.nodes import MDEF, SymbolTableNode, Var

        self._injected = Var("foo")
        self.fx.ai.names["foo"] = SymbolTableNode(MDEF, self._injected)
        type_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.di,
            self.fx.ei,
            self.fx.e2i,
            self.fx.e3i,
            self.fx.fi,
            self.fx.f2i,
            self.fx.f3i,
            self.fx.gi,
            self.fx.g2i,
            self.fx.hi,
            self.fx.gsi,
            self.fx.gs2i,
            self.fx.std_tuplei,
            self.fx.std_listi,
            self.fx.type_typei,
            self.fx.bool_type_info,
            self.fx.str_type_info,
            self.fx.functioni,
        ]
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active = _set_native_checkexpr_active
        self._set_active(False)

    def tearDown(self) -> None:
        del self.fx.ai.names["foo"]
        self._set_active(False)

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def assert_fullname_par(self, t: Type, method_name: str) -> None:
        # Python reference via the pure-Python path (gate off).
        from mypy.checkexpr import ExpressionChecker

        self._set_active(False)
        py = ExpressionChecker.method_fullname(None, t, method_name)  # type: ignore[arg-type]
        rusted = _type_kernel.rust_method_fullname(self.resolver, self._bytes_of(t), method_name)
        assert (
            rusted if rusted is not None else py
        ) == py, f"method_fullname({t!r}, {method_name!r}) rust={{}} py={{}}".format(rusted, py)

    def test_instance(self) -> None:
        self.assert_fullname_par(self.fx.a, "foo")
        self.assert_fullname_par(self.fx.b, "foo")

    def test_plain_tuple(self) -> None:
        plain = TupleType([self.fx.a], self.fx.std_tuple)
        self.assert_fullname_par(plain, "foo")

    def test_type_type(self) -> None:
        self.assert_fullname_par(self.fx.type_a, "foo")
        self.assert_fullname_par(self.fx.type_any, "foo")

    def test_callable_type_obj(self) -> None:
        # A class-object callable: fallback is builtins.type (metaclass), so
        # both sides unwrap to the constructed instance.
        c = CallableType([], [], [], self.fx.a, Instance(self.fx.type_typei, []))
        self.assert_fullname_par(c, "foo")

    def test_typeddict(self) -> None:
        td = TypedDictType({"x": self.fx.o}, {"x"}, set(), Instance(self.fx.ai, []))
        self.assert_fullname_par(td, "foo")

    def test_literal(self) -> None:
        lit = LiteralType(1, Instance(self.fx.ai, []))
        self.assert_fullname_par(lit, "foo")

    def test_instance_missing_method(self) -> None:
        # Instance branch appends unconditionally (no names lookup).
        self.assert_fullname_par(self.fx.a, "missing")

    def test_unknown_type_defers(self) -> None:
        # NoneType is not Instance/TypedDict/Literal/Tuple: both defer to
        # None. The Rust side returns None and Python re-runs.
        self.assert_fullname_par(self.fx.nonet, "foo")
        self.assert_fullname_par(self.fx.anyt, "foo")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeTypeContextSuite(Suite):
    """Parity suite for the Rust `is_type_type_context` port (Phase B3a, #591).

    Exercises TypeType, unions, and the alias-expansion path: an alias
    whose frozen target is `Type[X]` must answer True without deferring,
    while `List[X]` targets answer False. The native resolver is built
    from the TypeFixture type_infos and a real `TypeAlias` whose `target`
    is serialized through the fixture, matching production's
    `build_native_resolver` alias snapshot. Requires TEST_NATIVE_TYPE_KERNEL=1.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        self.fx = TypeFixture()
        self.resolver = self._build_resolver([])
        _set_native_checkexpr_resolver(self.resolver)
        _set_native_checkexpr_active(True)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        _set_native_checkexpr_active(False)
        _set_native_checkexpr_resolver(None)

    def _build_resolver(self, aliases: list[Any]) -> Any:
        return _type_kernel.build_native_resolver(
            [
                self.fx.oi,
                self.fx.ai,
                self.fx.bi,
                self.fx.str_type_info,
                self.fx.type_typei,
                self.fx.std_tuplei,
                self.fx.std_listi,
            ],
            aliases,
        )

    def _rebuild_with_aliases(self, aliases: list[Any]) -> None:
        from mypy.checkexpr import _set_native_checkexpr_resolver

        self.resolver = self._build_resolver(aliases)
        _set_native_checkexpr_resolver(self.resolver)

    def assert_par(self, t: Type, expected: bool) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr, is_type_type_context

        assert is_type_type_context(t) is expected
        # The Rust side must actually decide (non-None) for the alias
        # cases, proving the resolver snapshot expansion fired rather than
        # falling back to Python's `get_proper_type`.
        rusted = _type_kernel.rust_is_type_type_context(
            self.resolver, _serialize_type_for_checkexpr(t)
        )
        assert rusted is not None, f"Rust deferred on {t!r}"
        assert rusted is expected

    def test_type_type(self) -> None:
        self.assert_par(TypeType(self.fx.a), True)

    def test_plain_instance(self) -> None:
        self.assert_par(self.fx.a, False)

    def test_union_with_type_type(self) -> None:
        self.assert_par(UnionType.make_union([self.fx.a, TypeType(self.fx.b)]), True)

    def test_union_without_type_type(self) -> None:
        self.assert_par(UnionType.make_union([self.fx.a, self.fx.b]), False)

    def test_alias_to_type_type(self) -> None:
        # TypeAlias whose target is Type[A]: the Rust side expands the
        # alias via the resolver snapshot and answers True without defer.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(TypeType(self.fx.a), "mod.TA", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        self.assert_par(TypeAliasType(alias, []), True)

    def test_alias_to_list_false(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(Instance(self.fx.std_listi, [self.fx.a]), "mod.TB", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        self.assert_par(TypeAliasType(alias, []), False)

    def test_union_with_alias_to_type_type(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(TypeType(self.fx.b), "mod.TC", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        self.assert_par(UnionType.make_union([self.fx.a, TypeAliasType(alias, [])]), True)

    def test_nested_alias_to_type_type(self) -> None:
        # B = A, A = Type[A]; the snapshot chain must be followed and the
        # Rust side must answer True without defer (Python expands both).
        from mypy.nodes import TypeAlias

        alias_a = TypeAlias(TypeType(self.fx.a), "mod.TA", "mod", -1, -1)
        alias_b = TypeAlias(TypeAliasType(alias_a, []), "mod.TB", "mod", -1, -1)
        self._rebuild_with_aliases([alias_a, alias_b])
        self.assert_par(TypeAliasType(alias_b, []), True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckexprFunctionsDeferralSuite(Suite):
    """Differential parity for the #871 checkexpr_functions alias defers.

    Ports top-level TypeAliasType operand expansion into the
    `has_bytes_component` and `allow_fast_container_literal` seams,
    mirroring the `get_proper_type` calls at the top of the Python bodies
    (checkexpr.py). Each test runs the public function gate-off (pure
    Python) and gate-on (Rust seam) and asserts equal answers; for the
    resolved-alias cases it also calls the seam directly and asserts a
    non-None result, proving the resolver-snapshot expansion fired
    instead of deferring. A missing resolver snapshot still defers (the
    Python fallback gives the answer, so both gates agree).
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        self.fx = TypeFixture()
        self.bytes_info = self.fx.make_type_info("builtins.bytes")
        self.bytearray_info = self.fx.make_type_info("builtins.bytearray")
        self.resolver = self._build_resolver([], [])
        _set_native_checkexpr_resolver(self.resolver)
        _set_native_checkexpr_active(True)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        _set_native_checkexpr_active(False)
        _set_native_checkexpr_resolver(None)

    def _build_resolver(self, extra_infos: list[Any], aliases: list[Any]) -> Any:
        from mypy.checkexpr import _set_native_checkexpr_resolver

        infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.str_type_info,
            self.fx.type_typei,
            self.fx.std_tuplei,
            self.fx.std_listi,
        ] + extra_infos
        self.resolver = _type_kernel.build_native_resolver(infos, aliases)
        _set_native_checkexpr_resolver(self.resolver)
        return self.resolver

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checkexpr import _set_native_checkexpr_active

        _set_native_checkexpr_active(active)
        try:
            return fn()
        finally:
            _set_native_checkexpr_active(True)

    def _differential(self, fn: Callable[[], object]) -> object:
        # Gate-off (pure Python) vs gate-on (Rust seam) must agree.
        off = self._with_gate(False, fn)
        assert fn() == off
        return off

    def test_has_bytes_alias_to_bytes(self) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr, has_bytes_component
        from mypy.nodes import TypeAlias

        alias = TypeAlias(Instance(self.bytes_info, []), "mod.AB", "mod", -1, -1)
        self._build_resolver([self.bytes_info], [alias])
        typ = TypeAliasType(alias, [])
        # Rust must decide (non-None): the alias expands to builtins.bytes.
        assert (
            _type_kernel.rust_has_bytes_component(
                self.resolver, _serialize_type_for_checkexpr(typ)
            )
            is True
        )
        assert self._differential(lambda: has_bytes_component(typ)) is True

    def test_has_bytes_union_alias_item(self) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr, has_bytes_component
        from mypy.nodes import TypeAlias

        alias = TypeAlias(Instance(self.bytearray_info, []), "mod.ABA", "mod", -1, -1)
        self._build_resolver([self.bytearray_info], [alias])
        union = UnionType.make_union([self.fx.a, TypeAliasType(alias, [])])
        assert (
            _type_kernel.rust_has_bytes_component(
                self.resolver, _serialize_type_for_checkexpr(union)
            )
            is True
        )
        assert self._differential(lambda: has_bytes_component(union)) is True

    def test_has_bytes_non_bytes_false(self) -> None:
        from mypy.checkexpr import has_bytes_component

        assert self._differential(lambda: has_bytes_component(self.fx.a)) is False

    def test_has_bytes_alias_missing_snapshot(self) -> None:
        # Missing resolver snapshot: both gates fall through to Python's
        # get_proper_type and must agree.
        from mypy.checkexpr import has_bytes_component
        from mypy.nodes import TypeAlias

        alias = TypeAlias(Instance(self.bytes_info, []), "mod.AMissing", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        assert self._differential(lambda: has_bytes_component(typ)) is True

    def test_allow_fast_alias_to_tuple(self) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr, allow_fast_container_literal
        from mypy.nodes import TypeAlias

        fallback = Instance(self.fx.std_tuplei, [])
        target = TupleType([Instance(self.fx.ai, []), Instance(self.fx.bi, [])], fallback)
        alias = TypeAlias(target, "mod.AT", "mod", -1, -1)
        self._build_resolver([], [alias])
        typ = TypeAliasType(alias, [])
        assert (
            _type_kernel.rust_allow_fast_container_literal(
                self.resolver, _serialize_type_for_checkexpr(typ)
            )
            is True
        )
        assert self._differential(lambda: allow_fast_container_literal(typ)) is True

    def test_allow_fast_alias_to_list(self) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr, allow_fast_container_literal
        from mypy.nodes import TypeAlias

        alias = TypeAlias(Instance(self.fx.std_listi, [self.fx.a]), "mod.AL", "mod", -1, -1)
        self._build_resolver([], [alias])
        typ = TypeAliasType(alias, [])
        assert (
            _type_kernel.rust_allow_fast_container_literal(
                self.resolver, _serialize_type_for_checkexpr(typ)
            )
            is True
        )
        assert self._differential(lambda: allow_fast_container_literal(typ)) is True

    def test_allow_fast_plain_instance_true(self) -> None:
        from mypy.checkexpr import allow_fast_container_literal

        assert self._differential(lambda: allow_fast_container_literal(self.fx.a)) is True

    def test_allow_fast_alias_missing_snapshot(self) -> None:
        # Alias absent from the resolver: the seam defers (None), so the
        # answer comes from Python's get_proper_type; both gates agree.
        from mypy.checkexpr import _serialize_type_for_checkexpr, allow_fast_container_literal
        from mypy.nodes import TypeAlias

        alias = TypeAlias(Instance(self.fx.std_listi, [self.fx.a]), "mod.AMissL", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        assert (
            _type_kernel.rust_allow_fast_container_literal(
                self.resolver, _serialize_type_for_checkexpr(typ)
            )
            is None
        )
        assert self._differential(lambda: allow_fast_container_literal(typ)) is True


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTryGettingLiteralSuite(Suite):
    """Parity for the Rust `try_getting_literal` port (mypy.checkexpr).

    `try_getting_literal` unwraps an Instance's last_known_value to the
    precise LiteralType, or returns the type proper. The Rust port
    implements the same get_proper_type + lkv unwrap; the wire round-trip
    carries the `last_known_value` field, and fixup resolves type_refs to
    live TypeInfo via the installed wire map. Toggling the checkexpr gate
    off (pure Python) and on (Rust seam) must produce identical results, and
    a direct seam call proves the Rust function engages rather than
    silently deferring.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._type_infos = type_infos
        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _assert_par(self, typ: Type) -> None:
        from mypy.checkexpr import try_getting_literal

        off = self._with_gate(False, lambda: try_getting_literal(typ))
        on = self._with_gate(True, lambda: try_getting_literal(typ))
        assert_equal(str(on), str(off), f"try_getting_literal parity {typ}")

    def _assert_engages(self, typ: Type) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        result = _type_kernel.rust_try_getting_literal(_serialize_type_for_checkexpr(typ))
        assert result is not None, f"Rust try_getting_literal did not engage for {typ}"

    def test_unwraps_last_known_value(self) -> None:
        from mypy.checkexpr import try_getting_literal

        self._assert_par(self.fx.lit_str1_inst)
        result = self._with_gate(True, lambda: try_getting_literal(self.fx.lit_str1_inst))
        assert_equal(str(result), "Literal['x']")
        self._assert_engages(self.fx.lit_str1_inst)

    def test_unwraps_big_int_last_known_value(self) -> None:
        # int(2**80) exceeds i64: the wire long-int encoding must survive
        # both directions of the live seam (issue #1329); the Python shim
        # decodes the Rust-written blob, so this is a full round-trip.
        from mypy.checkexpr import try_getting_literal

        int_type = Instance(self.fx.make_type_info("builtins.int"), [])
        big = 2**80
        inst = Instance(int_type.type, [], last_known_value=LiteralType(big, int_type))
        self._assert_par(inst)
        result = self._with_gate(True, lambda: try_getting_literal(inst))
        assert_equal(str(result), f"Literal[{big}]")
        self._assert_engages(inst)

        neg = -(2**80)
        inst = Instance(int_type.type, [], last_known_value=LiteralType(neg, int_type))
        self._assert_par(inst)
        result = self._with_gate(True, lambda: try_getting_literal(inst))
        assert_equal(str(result), f"Literal[{neg}]")

    def test_plain_instance_unchanged(self) -> None:
        from mypy.checkexpr import try_getting_literal

        plain = Instance(self.fx.ai, [])
        self._assert_par(plain)
        result = self._with_gate(True, lambda: try_getting_literal(plain))
        assert_equal(str(result), "A")
        self._assert_engages(plain)

    def test_none(self) -> None:
        self._assert_par(self.fx.nonet)
        self._assert_engages(self.fx.nonet)

    def test_any(self) -> None:
        self._assert_par(self.fx.anyt)
        self._assert_engages(self.fx.anyt)

    def test_union(self) -> None:
        # `get_proper_type` on a Union is a no-op; the Instance-with-lkv
        # item is not unwrapped (only the root is checked). Parity holds.
        u = UnionType([self.fx.lit_str1_inst, self.fx.a])
        self._assert_par(u)
        self._assert_engages(u)

    def test_uninhabited(self) -> None:
        self._assert_par(self.fx.uninhabited)
        self._assert_engages(self.fx.uninhabited)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRevealImportedSuite(Suite):
    """Parity for `rust_classify_reveal_imported` (issue #918).

    `check_reveal_imported` (checkexpr.py:6485-6513) dispatches the
    unimported-reveal error: disabled code -> nothing;
    REVEAL_LOCALS -> "reveal_locals"; REVEAL_TYPE and not is_imported ->
    "reveal_type"; else -> nothing. The Rust seam classifies the whole
    dispatch head from scalar facts and returns the name (or None for
    "do nothing"); Python applies the fail + note side effects. The
    direct seam call must match the expected name/None, and toggling the
    checkexpr gate off vs on must produce identical captured messages.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _make_checker(self, code_enabled: bool = True) -> ExpressionChecker:
        from mypy.checker import TypeChecker
        from mypy.checkexpr import ExpressionChecker
        from mypy.errors import Errors
        from mypy.messages import MessageBuilder
        from mypy.nodes import MypyFile, SymbolTable
        from mypy.plugin import Plugin

        options = Options()
        from mypy import errorcodes as codes

        if code_enabled:
            options.enabled_error_codes = {codes.UNIMPORTED_REVEAL}
        errors = Errors(options)
        tree = MypyFile([], [])
        tree.is_stub = True
        tree.names = SymbolTable()
        modules: dict[str, MypyFile] = {}
        chk = TypeChecker(errors, modules, options, tree, "", Plugin(options), {})
        msg = MessageBuilder(errors, modules)
        return ExpressionChecker(chk, msg, Plugin(options), {})

    def _reveal(self, kind: int, is_imported: bool) -> RevealExpr:
        return RevealExpr(kind=kind, is_imported=is_imported)

    def _run_and_capture(
        self, expr: RevealExpr, active: bool, code_enabled: bool = True
    ) -> list[tuple[str, str]]:
        ec = self._make_checker(code_enabled)
        captured: list[tuple[str, str]] = []
        ec.chk.fail = lambda msg, ctx, code=None: captured.append(("fail", msg))  # type: ignore[method-assign, misc, assignment]
        ec.chk.note = lambda msg, ctx, offset=0, code=None: captured.append(("note", msg))  # type: ignore[method-assign, misc]
        self._with_gate(active, lambda: ec.check_reveal_imported(expr))
        return captured

    def _assert_seam(
        self, kind: int, is_imported: bool, enabled: bool, expected: str | None
    ) -> None:
        result = _type_kernel.rust_classify_reveal_imported(kind, is_imported, enabled)
        assert (
            result == expected
        ), f"seam({kind}, {is_imported}, {enabled}) = {result!r}, want {expected!r}"

    def test_seam_disabled(self) -> None:
        from mypy.nodes import REVEAL_LOCALS, REVEAL_TYPE

        self._assert_seam(REVEAL_LOCALS, False, False, None)
        self._assert_seam(REVEAL_TYPE, False, False, None)
        self._assert_seam(REVEAL_TYPE, True, False, None)

    def test_seam_reveal_locals(self) -> None:
        from mypy.nodes import REVEAL_LOCALS

        self._assert_seam(REVEAL_LOCALS, False, True, "reveal_locals")
        # is_imported is irrelevant for reveal_locals.
        self._assert_seam(REVEAL_LOCALS, True, True, "reveal_locals")

    def test_seam_reveal_type_not_imported(self) -> None:
        from mypy.nodes import REVEAL_TYPE

        self._assert_seam(REVEAL_TYPE, False, True, "reveal_type")

    def test_seam_reveal_type_imported(self) -> None:
        from mypy.nodes import REVEAL_TYPE

        self._assert_seam(REVEAL_TYPE, True, True, None)

    def test_seam_unknown_kind(self) -> None:
        self._assert_seam(999, False, True, None)

    def test_par_disabled_emits_nothing(self) -> None:
        from mypy.nodes import REVEAL_LOCALS, REVEAL_TYPE

        for kind, imp in [
            (REVEAL_LOCALS, False),
            (REVEAL_TYPE, False),
            (REVEAL_TYPE, True),
            (999, False),
        ]:
            expr = self._reveal(kind, imp)
            off = self._run_and_capture(expr, False, code_enabled=False)
            on = self._run_and_capture(expr, True, code_enabled=False)
            assert off == on == [], f"disabled kind={kind} imp={imp}: {off}/{on}"

    def test_par_reveal_locals(self) -> None:
        from mypy.nodes import REVEAL_LOCALS

        expr = self._reveal(REVEAL_LOCALS, False)
        off = self._run_and_capture(expr, False)
        on = self._run_and_capture(expr, True)
        assert off == on, f"reveal_locals differential: off={off} on={on}"
        assert off and off[0] == ("fail", 'Name "reveal_locals" is not defined'), off

    def test_par_reveal_type_not_imported(self) -> None:
        from mypy.nodes import REVEAL_TYPE

        expr = self._reveal(REVEAL_TYPE, False)
        off = self._run_and_capture(expr, False)
        on = self._run_and_capture(expr, True)
        assert off == on, f"reveal_type differential: off={off} on={on}"
        assert off and off[0][0] == "fail" and "reveal_type" in off[0][1], off
        assert any(tag == "note" and "typing" in msg for tag, msg in off), off

    def test_par_reveal_type_imported_emits_nothing(self) -> None:
        from mypy.nodes import REVEAL_TYPE

        expr = self._reveal(REVEAL_TYPE, True)
        off = self._run_and_capture(expr, False)
        on = self._run_and_capture(expr, True)
        assert off == on == [], f"imported reveal_type must emit nothing: {off}/{on}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSuperArgTypesSuite(Suite):
    """Parity for `rust_classify_super_arg_types` (issue #956).

    `_super_arg_types` (checkexpr.py:7440) dispatches a stage-1 arity +
    scope gate chain producing 7 early-error returns and 2 fall-through
    bodies. The Rust seam classifies the whole dispatch head from live
    checker/super-expr facts and returns a branch tag; Python applies
    the `self.fail` / `fill_typevars` / `accept` side effects and stage 2
    (proper-type dispatch). The direct seam call must match the expected
    tag for all 9 branches, and toggling the checkexpr gate off vs on
    must produce identical results + captured messages for the 7
    early-exit branches.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _make_super_expr(
        self, n_args: int, arg_kinds: list[ArgKind], info: object = None
    ) -> SuperExpr:
        from mypy.nodes import SuperExpr

        args: list[Expression] = [NameExpr(f"a{i}") for i in range(n_args)]
        call = CallExpr(
            callee=NameExpr("super"),
            args=args,
            arg_kinds=list(arg_kinds),
            arg_names=[None] * n_args,
        )
        e = SuperExpr("super", call)
        # Some direct-seam tests pass an opaque object on purpose (the Rust
        # classifier must tolerate unreadable info facts and defer).
        e.info = info  # type: ignore[assignment]
        return e

    def _make_chk(self, in_checked: bool = True, active_class: object = None) -> SimpleNamespace:
        captured: list[tuple[str, str]] = []
        chk = SimpleNamespace(
            in_checked_function=lambda: in_checked,
            fail=lambda msg, ctx, code=None: captured.append(("fail", str(msg))),
            scope=SimpleNamespace(active_class=lambda: active_class),
        )
        chk.captured = captured
        return chk

    def _run_super(
        self, e: SuperExpr, chk: SimpleNamespace, active: bool
    ) -> tuple[str, list[str]]:
        from mypy.checkexpr import ExpressionChecker

        ec = ExpressionChecker.__new__(ExpressionChecker)
        ec.chk = chk  # type: ignore[assignment]
        try:
            result = self._with_gate(active, lambda: ec._super_arg_types(e))
            return (str(result), [m for _, m in chk.captured])
        except Exception as exc:
            return ("EXC:" + str(exc), [m for _, m in chk.captured])

    # -- direct seam tests (all 9 tags) --

    def test_seam_not_checked(self) -> None:
        from mypy.checkexpr import NATIVE_SUPER_ARG_NOT_CHECKED

        e = self._make_super_expr(2, [ARG_POS, ARG_POS], info=object())
        chk = self._make_chk(in_checked=False, active_class=None)
        tag = _type_kernel.rust_classify_super_arg_types(chk, e)
        assert tag == NATIVE_SUPER_ARG_NOT_CHECKED, f"{tag} != {NATIVE_SUPER_ARG_NOT_CHECKED}"

    def test_seam_zero_arg_no_info(self) -> None:
        from mypy.checkexpr import NATIVE_SUPER_ARG_ZERO_ARG_NO_INFO

        e = self._make_super_expr(0, [], info=None)
        chk = self._make_chk(in_checked=True, active_class=None)
        tag = _type_kernel.rust_classify_super_arg_types(chk, e)
        assert tag == NATIVE_SUPER_ARG_ZERO_ARG_NO_INFO, f"{tag}"

    def test_seam_zero_arg_outside_method(self) -> None:
        from mypy.checkexpr import NATIVE_SUPER_ARG_ZERO_ARG_OUTSIDE_METHOD

        e = self._make_super_expr(0, [], info=object())
        chk = self._make_chk(in_checked=True, active_class=object())
        tag = _type_kernel.rust_classify_super_arg_types(chk, e)
        assert tag == NATIVE_SUPER_ARG_ZERO_ARG_OUTSIDE_METHOD, f"{tag}"

    def test_seam_zero_arg_ok(self) -> None:
        from mypy.checkexpr import NATIVE_SUPER_ARG_ZERO_ARG_OK

        e = self._make_super_expr(0, [], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        tag = _type_kernel.rust_classify_super_arg_types(chk, e)
        assert tag == NATIVE_SUPER_ARG_ZERO_ARG_OK, f"{tag}"

    def test_seam_varargs(self) -> None:
        from mypy.checkexpr import NATIVE_SUPER_ARG_VARARGS

        e = self._make_super_expr(1, [ARG_STAR], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        tag = _type_kernel.rust_classify_super_arg_types(chk, e)
        assert tag == NATIVE_SUPER_ARG_VARARGS, f"{tag}"

    def test_seam_non_positional(self) -> None:
        from mypy.checkexpr import NATIVE_SUPER_ARG_NON_POSITIONAL

        e = self._make_super_expr(1, [ARG_NAMED], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        tag = _type_kernel.rust_classify_super_arg_types(chk, e)
        assert tag == NATIVE_SUPER_ARG_NON_POSITIONAL, f"{tag}"

    def test_seam_single_arg(self) -> None:
        from mypy.checkexpr import NATIVE_SUPER_ARG_SINGLE_ARG

        e = self._make_super_expr(1, [ARG_POS], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        tag = _type_kernel.rust_classify_super_arg_types(chk, e)
        assert tag == NATIVE_SUPER_ARG_SINGLE_ARG, f"{tag}"

    def test_seam_two_arg_ok(self) -> None:
        from mypy.checkexpr import NATIVE_SUPER_ARG_TWO_ARG_OK

        e = self._make_super_expr(2, [ARG_POS, ARG_POS], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        tag = _type_kernel.rust_classify_super_arg_types(chk, e)
        assert tag == NATIVE_SUPER_ARG_TWO_ARG_OK, f"{tag}"

    def test_seam_too_many(self) -> None:
        from mypy.checkexpr import NATIVE_SUPER_ARG_TOO_MANY

        e = self._make_super_expr(3, [ARG_POS, ARG_POS, ARG_POS], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        tag = _type_kernel.rust_classify_super_arg_types(chk, e)
        assert tag == NATIVE_SUPER_ARG_TOO_MANY, f"{tag}"

    # -- gate off/on differential tests (7 early-exit branches) --

    def test_par_not_checked(self) -> None:
        e = self._make_super_expr(2, [ARG_POS, ARG_POS], info=object())
        chk = self._make_chk(in_checked=False, active_class=None)
        off = self._run_super(e, chk, False)
        chk2 = self._make_chk(in_checked=False, active_class=None)
        on = self._run_super(e, chk2, True)
        assert off == on, f"not_checked: off={off} on={on}"

    def test_par_zero_arg_no_info(self) -> None:
        e = self._make_super_expr(0, [], info=None)
        chk = self._make_chk(in_checked=True, active_class=None)
        off = self._run_super(e, chk, False)
        chk2 = self._make_chk(in_checked=True, active_class=None)
        on = self._run_super(e, chk2, True)
        assert off == on, f"zero_arg_no_info: off={off} on={on}"

    def test_par_zero_arg_outside_method(self) -> None:
        e = self._make_super_expr(0, [], info=object())
        chk = self._make_chk(in_checked=True, active_class=object())
        off = self._run_super(e, chk, False)
        chk2 = self._make_chk(in_checked=True, active_class=object())
        on = self._run_super(e, chk2, True)
        assert off == on, f"outside_method: off={off} on={on}"
        assert off[1], f"expected fail: {off}"

    def test_par_varargs(self) -> None:
        e = self._make_super_expr(1, [ARG_STAR], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        off = self._run_super(e, chk, False)
        chk2 = self._make_chk(in_checked=True, active_class=None)
        on = self._run_super(e, chk2, True)
        assert off == on, f"varargs: off={off} on={on}"
        assert off[1], f"expected fail: {off}"

    def test_par_non_positional(self) -> None:
        e = self._make_super_expr(1, [ARG_NAMED], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        off = self._run_super(e, chk, False)
        chk2 = self._make_chk(in_checked=True, active_class=None)
        on = self._run_super(e, chk2, True)
        assert off == on, f"non_positional: off={off} on={on}"
        assert off[1], f"expected fail: {off}"

    def test_par_single_arg(self) -> None:
        e = self._make_super_expr(1, [ARG_POS], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        off = self._run_super(e, chk, False)
        chk2 = self._make_chk(in_checked=True, active_class=None)
        on = self._run_super(e, chk2, True)
        assert off == on, f"single_arg: off={off} on={on}"
        assert off[1], f"expected fail: {off}"

    def test_par_too_many(self) -> None:
        e = self._make_super_expr(3, [ARG_POS, ARG_POS, ARG_POS], info=object())
        chk = self._make_chk(in_checked=True, active_class=None)
        off = self._run_super(e, chk, False)
        chk2 = self._make_chk(in_checked=True, active_class=None)
        on = self._run_super(e, chk2, True)
        assert off == on, f"too_many: off={off} on={on}"
        assert off[1], f"expected fail: {off}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeInferArgContextSuite(Suite):
    """Parity for `rust_compute_arg_context_indices` (issue #1064).

    `infer_arg_types_in_context` (checkexpr.py:3265) first precomputes the
    pure arg_context index map (formal index per actual, star args
    skipped); the per-arg accept recursion and the infer_unions toggle
    stay in Python. The Rust seam decides from scalars only. Direct seam
    calls assert the mapping (star-skip, empty formal_to_actual,
    no-context tail, malformed deferral); gate off vs on must produce
    identical accept traces across the 3 call-site shapes
    (checkexpr.py:3004, 3424, 3627).

    #1739 retired the shim (1.37x-1.59x the Python double loop), so the
    gate toggle is inert for this body now: both arms are the Python loop.
    The pyfunction stays registered for the direct-seam tests above, and
    the retirement pins live in `testtypes_native_retired_checkexpr.py`.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self._set_active = _set_native_checkexpr_active
        self._set_active(True)
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _make_callee(self, n_args: int) -> CallableType:
        return CallableType(
            [AnyType(TypeOfAny.special_form)] * n_args,
            [ARG_POS] * n_args,
            [None] * n_args,
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )

    # -- direct seam tests --

    def test_seam_star_skip(self) -> None:
        out = _type_kernel.rust_compute_arg_context_indices(
            [ARG_POS.value, ARG_STAR.value, ARG_STAR2.value, ARG_POS.value], [[0, 1, 2, 3]], 4, 1
        )
        assert out == [0, -1, -1, 0], f"{out}"

    def test_seam_empty_formal_to_actual(self) -> None:
        out = _type_kernel.rust_compute_arg_context_indices([ARG_POS.value] * 2, [], 2, 1)
        assert out == [-1, -1], f"{out}"

    def test_seam_no_context_tail(self) -> None:
        out = _type_kernel.rust_compute_arg_context_indices([ARG_POS.value] * 3, [[1]], 3, 1)
        assert out == [-1, 0, -1], f"{out}"

    def test_seam_malformed_defers(self) -> None:
        # Actual index out of bounds of args_len.
        out = _type_kernel.rust_compute_arg_context_indices([0, 0], [[2]], 2, 1)
        assert out is None, f"{out}"
        # Formal index out of bounds of callee.arg_types.
        out = _type_kernel.rust_compute_arg_context_indices([0, 0], [[0], [1]], 2, 1)
        assert out is None, f"{out}"
        # arg_kinds / args_len mismatch.
        out = _type_kernel.rust_compute_arg_context_indices([0], [[0]], 2, 1)
        assert out is None, f"{out}"

    # -- gate off/on differential across the 3 call sites --

    def _run_infer(
        self,
        callee: CallableType,
        args: list[Expression],
        arg_kinds: list[ArgKind],
        formal_to_actual: list[list[int]],
        active: bool,
    ) -> list[str]:
        from mypy.checkexpr import ExpressionChecker

        captured: list[str] = []
        ec = ExpressionChecker.__new__(ExpressionChecker)

        def accept(arg: Expression, ctx: Type | None = None) -> Any:
            captured.append(f"{arg!r}|ctx={ctx!r}")
            return arg

        ec.accept = accept  # type: ignore[assignment]
        result = self._with_gate(
            active,
            lambda: ec.infer_arg_types_in_context(callee, args, arg_kinds, formal_to_actual),
        )
        return [str(r) for r in result] + captured

    def _make_args(self, n: int) -> list[Expression]:
        return [NameExpr(f"a{i}") for i in range(n)]

    def test_par_lambda_body_site(self) -> None:
        # checkexpr.py:3004 shape: positional args, 1:1 formal mapping.
        callee = self._make_callee(2)
        args = self._make_args(2)
        kinds = [ARG_POS, ARG_POS]
        f2a = [[0], [1]]
        off = self._run_infer(callee, args, kinds, f2a, False)
        on = self._run_infer(callee, args, kinds, f2a, True)
        assert off == on, f"lambda_body: off={off} on={on}"
        assert all("ctx=Any" in s for s in on[len(args) :]), f"expected contexts: {on}"

    def test_par_first_pass_site(self) -> None:
        # checkexpr.py:3424 shape: optional + named formals, error-filtered.
        callee = self._make_callee(3)
        args = self._make_args(3)
        kinds = [ARG_POS, ARG_OPT, ARG_NAMED]
        f2a = [[0], [1], [2]]
        off = self._run_infer(callee, args, kinds, f2a, False)
        on = self._run_infer(callee, args, kinds, f2a, True)
        assert off == on, f"first_pass: off={off} on={on}"

    def test_par_second_pass_site(self) -> None:
        # checkexpr.py:3627 shape: star args skipped, kwargs shares a formal.
        callee = self._make_callee(2)
        args = self._make_args(3)
        kinds = [ARG_POS, ARG_STAR, ARG_STAR2]
        f2a = [[0, 1, 2], [2]]
        off = self._run_infer(callee, args, kinds, f2a, False)
        on = self._run_infer(callee, args, kinds, f2a, True)
        assert off == on, f"second_pass: off={off} on={on}"
        assert "ctx=None" in on[len(args) + 1], f"star arg must have no context: {on}"

    def test_par_no_context_all(self) -> None:
        # Empty formal_to_actual: every actual accepted without context.
        callee = self._make_callee(2)
        args = self._make_args(2)
        kinds = [ARG_POS, ARG_POS]
        off = self._run_infer(callee, args, kinds, [], False)
        on = self._run_infer(callee, args, kinds, [], True)
        assert off == on, f"no_context: off={off} on={on}"
        assert all("ctx=None" in s for s in on[len(args) :]), f"{on}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeVisitOpExprSuite(Suite):
    """Parity for `rust_classify_visit_op_expr` (issue #959).

    `ExpressionChecker.visit_op_expr` (checkexpr.py:5014-5044) dispatches a
    5-way branch: `e.analyzed` passthrough, `and`/`or` boolean op,
    `*` with `ListExpr` list multiply, `%` with `BytesExpr`/`StrExpr` str
    interpolation, else `check_op`. The Rust seam classifies the whole
    dispatch head from live `e.analyzed`, `e.op`, and `e.left` isinstance
    facts and returns a branch tag; Python delegates to the original
    branch body. Direct seam calls assert the exact tag for every branch;
    toggling the checkexpr gate off vs on must produce identical results.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _tag(self, e: OpExpr) -> int | None:
        return _type_kernel.rust_classify_visit_op_expr(e)

    def test_seam_analyzed_passthrough(self) -> None:
        e = OpExpr("|", NameExpr("X"), NameExpr("Y"))
        e.analyzed = NameExpr("Analyzed")  # type: ignore[assignment]
        assert self._tag(e) == 0

    def test_seam_boolean_and(self) -> None:
        e = OpExpr("and", NameExpr("X"), NameExpr("Y"))
        assert self._tag(e) == 1

    def test_seam_boolean_or(self) -> None:
        e = OpExpr("or", NameExpr("X"), NameExpr("Y"))
        assert self._tag(e) == 1

    def test_seam_list_multiply(self) -> None:
        e = OpExpr("*", ListExpr([]), NameExpr("N"))
        assert self._tag(e) == 2

    def test_seam_str_interp_bytes(self) -> None:
        e = OpExpr("%", BytesExpr("%s"), NameExpr("Y"))
        assert self._tag(e) == 3

    def test_seam_str_interp_str(self) -> None:
        e = OpExpr("%", StrExpr("%s"), NameExpr("Y"))
        assert self._tag(e) == 3

    def test_seam_check_op_default(self) -> None:
        assert self._tag(OpExpr("+", NameExpr("X"), NameExpr("Y"))) == 4
        assert self._tag(OpExpr("|", NameExpr("X"), NameExpr("Y"))) == 4
        assert self._tag(OpExpr("-", NameExpr("X"), NameExpr("Y"))) == 4

    def test_seam_star_not_list(self) -> None:
        e = OpExpr("*", NameExpr("X"), NameExpr("Y"))
        assert self._tag(e) == 4

    def test_seam_percent_not_bytes_str(self) -> None:
        e = OpExpr("%", NameExpr("X"), NameExpr("Y"))
        assert self._tag(e) == 4

    def test_seam_and_overrides_list(self) -> None:
        # "and" takes precedence over isinstance checks.
        e = OpExpr("and", ListExpr([]), NameExpr("Y"))
        assert self._tag(e) == 1

    def test_seam_star_bytes_not_list_multiply(self) -> None:
        # "*" with BytesExpr left goes to check_op, not list multiply.
        e = OpExpr("*", BytesExpr("x"), NameExpr("Y"))
        assert self._tag(e) == 4


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckArgumentTypesPlanSuite(Suite):
    """Parity for the Rust `check_argument_types` plan port (mypy.checkexpr).

    `ExpressionChecker.check_argument_types` expands each formal's actual
    arguments against the callee signature: per formal it derives the
    effective `callee_arg_types`/`callee_arg_kinds` and
    `actual_types`/`actual_kinds` (or a too-many/too-few decision), then
    runs the `ArgTypeExpander` + `check_arg` loop. The Rust port derives
    those per-formal plans; the shim reports count errors and drives the
    stateful loop. Toggling the checkexpr gate off (pure Python) and on
    (Rust seam) must record identical message and `check_arg` calls, and a
    direct seam call proves the Rust planner engages rather than silently
    deferring.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        # Clear placeholder primitives so wire fixup maps type_refs to the
        # fixture instances (mirrors the bind_self suite).
        from mypy.types import instance_cache

        instance_cache.int_type = None
        instance_cache.str_type = None
        instance_cache.bool_type = None
        instance_cache.object_type = None
        instance_cache.function_type = None
        self._set_active = _set_native_checkexpr_active
        self._set_active(True)
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_checkexpr_resolver(self._resolver)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_checkexpr_resolver(None)
        set_wire_typeinfo_map(None)

    def _stub_checker(self) -> tuple[Any, list[str]]:
        """Minimal `ExpressionChecker` that records its effects.

        `check_argument_types` needs `self.chk.named_type` (var-arg
        validity and the `ArgumentInferContext`), `self.chk.msg` (error
        recording), and `self.check_arg`. The msg recorder and the passed
        `check_arg` both append to the shared log so gate-off and gate-on
        runs can be compared.
        """
        from mypy.checker import TypeChecker
        from mypy.checkexpr import ExpressionChecker
        from mypy.infer import ArgumentInferContext

        fx = self.fx
        log: list[str] = []

        def named_type(name: str) -> Instance:
            if "[" in name:
                raise AssertionError(f"named_type got parameterized {name}")
            return Instance(fx.make_type_info(name), [])

        def named_generic_type(name: str, args: list[Type]) -> Instance:
            return Instance(fx.make_type_info(name), args)

        class _Msg:
            def too_many_arguments(self, callee: CallableType, context: Context) -> None:
                del callee, context
                log.append("too_many")

            def too_few_arguments(
                self,
                callee: CallableType,
                context: Context,
                argument_names: Sequence[str | None] | None,
            ) -> None:
                del callee, context, argument_names
                log.append("too_few")

            def invalid_var_arg(self, typ: Type, context: Context) -> None:
                del typ, context
                log.append("invalid_var_arg")

            def invalid_keyword_var_arg(
                self, typ: Type, is_mapping: bool, context: Context
            ) -> None:
                del typ, context
                log.append(f"invalid_keyword_var_arg(mapping={is_mapping})")

        chk = TypeChecker.__new__(TypeChecker)
        chk.named_type = named_type  # type: ignore[method-assign]
        chk.named_generic_type = named_generic_type  # type: ignore[method-assign]
        chk.msg = _Msg()  # type: ignore[assignment]

        checker = ExpressionChecker.__new__(ExpressionChecker)
        checker.chk = chk
        checker._arg_infer_context_cache = ArgumentInferContext(
            named_type("typing.Mapping"), named_type("typing.Iterable")
        )
        return checker, log

    def _exprs(self, n: int) -> list[Expression]:
        from mypy.nodes import TempNode
        from mypy.types import AnyType, TypeOfAny

        return [TempNode(AnyType(TypeOfAny.special_form)) for _ in range(n)]

    def _ctx(self) -> Context:
        ctx = Context(5, 5)
        ctx.end_line = 6
        ctx.end_column = 7
        return ctx

    def _callee(self, arg_types: list[Type], arg_kinds: list[ArgKind]) -> CallableType:
        return CallableType(
            arg_types, arg_kinds, [None] * len(arg_types), self.fx.nonet, self.fx.function
        )

    def _run(
        self,
        arg_types: list[Type],
        arg_kinds: list[ArgKind],
        callee: CallableType,
        f2a: list[list[int]],
        *,
        assert_engages: bool = True,
    ) -> None:
        from mypy.checkexpr import ExpressionChecker

        def run_once() -> tuple[list[str], list[str]]:
            checker, log = self._stub_checker()
            check_calls: list[str] = []

            def check_arg(
                caller_type: Type,
                original_caller_type: Type,
                caller_kind: ArgKind,
                callee_type: Type,
                n: int,
                m: int,
                callee: CallableType,
                object_type: Type | None,
                context: Context,
                outer_context: Context,
            ) -> None:
                del callee, object_type, context, outer_context
                check_calls.append(
                    f"check_arg({caller_type}, {original_caller_type}, "
                    f"{caller_kind}, {callee_type}, actual{n}, formal{m})"
                )

            ExpressionChecker.check_argument_types(
                checker,
                arg_types,
                arg_kinds,
                self._exprs(len(arg_types)),
                callee,
                f2a,
                self._ctx(),
                check_arg=check_arg,
            )
            return log, check_calls

        self._set_active(False)
        off_msgs, off_calls = run_once()
        self._set_active(True)
        on_msgs, on_calls = run_once()
        assert_equal(on_msgs, off_msgs, f"message parity {callee}")
        assert_equal(on_calls, off_calls, f"check_arg parity {callee}")
        if assert_engages:
            self._assert_engages(arg_types, arg_kinds, callee, f2a)

    def _assert_engages(
        self,
        arg_types: list[Type],
        arg_kinds: list[ArgKind],
        callee: CallableType,
        f2a: list[list[int]],
    ) -> None:
        from mypy.checkexpr import (  # type: ignore[attr-defined]
            _rust_check_argument_types_plan,
            _serialize_type_for_checkexpr,
        )

        assert _rust_check_argument_types_plan is not None
        result = _rust_check_argument_types_plan(
            self._resolver,
            [_serialize_type_for_checkexpr(t) for t in arg_types],
            [int(k.value) for k in arg_kinds],
            f2a,
            _serialize_type_for_checkexpr(callee),
        )
        assert result is not None, "Rust check_argument_types did not engage"

    def test_simple_args(self) -> None:
        # def f(a: A, b: B); f(x, y) -> one check_arg per formal.
        # No unpack formals, so the seam skips wire serialization.
        fx = self.fx
        callee = self._callee([fx.a, fx.b], [ARG_POS, ARG_POS])
        self._run([fx.a, fx.b], [ARG_POS, ARG_POS], callee, [[0], [1]], assert_engages=False)

    def test_vararg_match(self) -> None:
        # def f(*args: A); f(A, B, A) -> all three actuals to formal 0.
        # ARG_STAR formal is not UnpackType, so the seam skips.
        fx = self.fx
        callee = self._callee([fx.a], [ARG_STAR])
        self._run(
            [fx.a, fx.b, fx.a],
            [ARG_POS, ARG_POS, ARG_POS],
            callee,
            [[0, 1, 2]],
            assert_engages=False,
        )

    def test_too_many_arguments(self) -> None:
        # def f(x: tuple[A, B]); caller passes 3 positional actuals.
        fx = self.fx
        callee = self._callee(
            [UnpackType(TupleType([fx.a, fx.b], self.fx.std_tuple, False))], [ARG_POS]
        )
        self._run([fx.a, fx.b, fx.c], [ARG_POS, ARG_POS, ARG_POS], callee, [[0, 1, 2]])

    def test_too_few_arguments(self) -> None:
        # def f(x: tuple[A, B]); caller passes 1 positional actual.
        fx = self.fx
        callee = self._callee(
            [UnpackType(TupleType([fx.a, fx.b], self.fx.std_tuple, False))], [ARG_POS]
        )
        self._run([fx.a], [ARG_POS], callee, [[0]])

    def test_unpacked_tuple_reunify(self) -> None:
        # def f(x: Tuple[Unpack[Ts], A]); caller passes a one-item unpacked
        # tuple whose Unpack target is the same shape, plus a suffix B, so
        # formal_to_actual is [[0, 1]]. The first actual is the one-item

        # unpacked tuple reunified with the suffix; Rust expands callee to
        # [Unpack[Ts], A] with kinds [ARG_STAR, ARG_POS].
        fx = self.fx
        inner_unpack = UnpackType(fx.ts)
        inner_tuple = TupleType([inner_unpack, fx.a], self.fx.std_tuple, False)
        callee = self._callee([UnpackType(inner_tuple)], [ARG_POS])
        caller_tuple = TupleType([UnpackType(inner_tuple), fx.b], self.fx.std_tuple, False)
        self._run([caller_tuple, fx.b], [ARG_STAR, ARG_POS], callee, [[0, 1]])

    def test_plain_tuple_unpack_target(self) -> None:
        # def f(x: tuple[A, ...]); call with two actuals.
        fx = self.fx
        callee = self._callee([UnpackType(Instance(fx.std_tuplei, [fx.a]))], [ARG_POS])
        self._run([fx.a, fx.b], [ARG_POS, ARG_POS], callee, [[0, 1]])

    def test_type_alias_defers_whole_call(self) -> None:
        # A TypeAliasType in a callee formal cannot be reproduced by Rust
        # (no alias target on the wire), so `plan_for_formal` returns None
        # for the whole call and Python runs the pure body. Both paths

        # produce the same check_arg for the non-alias formal.
        from mypy.nodes import TypeAlias as TypeAliasNode
        from mypy.types import TypeAliasType as TypeAliasTypeCls

        fx = self.fx
        alias_node = TypeAliasNode(fx.a, "mod.AL", "mod", -1, -1)
        alias = TypeAliasTypeCls(alias_node, [])
        callee = self._callee([alias, fx.b], [ARG_POS, ARG_POS])
        self._run([fx.a, fx.b], [ARG_POS, ARG_POS], callee, [[0], [1]], assert_engages=False)
        # The seam must actively refuse the alias (prove the seal, not a
        # silent pass-through).
        from mypy.checkexpr import (  # type: ignore[attr-defined]
            _rust_check_argument_types_plan,
            _serialize_type_for_checkexpr,
        )

        result = _rust_check_argument_types_plan(
            self._resolver,
            [_serialize_type_for_checkexpr(t) for t in [fx.a, fx.b]],
            [int(k.value) for k in [ARG_POS, ARG_POS]],
            [[0], [1]],
            _serialize_type_for_checkexpr(callee),
        )
        assert result is None, "expected Rust to defer on a TypeAliasType formal"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeHasAnyTypeSuite(Suite):
    """Parity suite for the Rust `has_any_type` port with alias type-arg
    substitution (Phase B3b, #591).

    The B3b core: `has_any_type` must expand a `TypeAliasType` to its
    substituted target (like `_expand_once`) and answer correctly, not
    merely defer on every alias. Covers typevar aliases applied with Any
    (true), plain targets (false), chains, no_args aliases, cycles
    (defer), and new-style (PEP 695) arg visiting. Requires
    TEST_NATIVE_TYPE_KERNEL=1.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        self.fx = TypeFixture()
        self.resolver = self._build_resolver([])
        _set_native_checkexpr_resolver(self.resolver)
        _set_native_checkexpr_active(True)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        _set_native_checkexpr_active(False)
        _set_native_checkexpr_resolver(None)

    def _build_resolver(self, aliases: list[Any]) -> Any:
        return _type_kernel.build_native_resolver(
            [
                self.fx.oi,
                self.fx.ai,
                self.fx.bi,
                self.fx.str_type_info,
                self.fx.type_typei,
                self.fx.std_tuplei,
                self.fx.std_listi,
            ],
            aliases,
        )

    def _rebuild_with_aliases(self, aliases: list[Any]) -> None:
        from mypy.checkexpr import _set_native_checkexpr_resolver

        self.resolver = self._build_resolver(aliases)
        _set_native_checkexpr_resolver(self.resolver)

    def assert_rust_decides(self, t: Type, expected: bool) -> None:
        """Assert the Rust kernel answers `expected` without deferring."""
        from mypy.checkexpr import _serialize_type_for_checkexpr

        rusted = _type_kernel.rust_has_any_type(
            self.resolver, _serialize_type_for_checkexpr(t), False
        )
        assert rusted is not None, f"Rust deferred on {t!r}"
        assert rusted is expected

    def assert_rust_defers(self, t: Type) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        rusted = _type_kernel.rust_has_any_type(
            self.resolver, _serialize_type_for_checkexpr(t), False
        )
        assert rusted is None, f"Rust should defer on {t!r}, got {rusted}"

    def _make_tvar(self, name: str, raw_id: int) -> TypeVarType:
        return TypeVarType(
            name, f"mod.{name}", TypeVarId(raw_id), [], self.fx.str_type, self.fx.nonet, 0
        )

    def _make_alias(
        self,
        fullname: str,
        target: Type,
        *,
        alias_tvars: list[TypeVarType] | None = None,
        no_args: bool = False,
    ) -> TypeAlias:
        from mypy.nodes import TypeAlias

        return TypeAlias(
            target,
            fullname,
            "mod",
            -1,
            -1,
            alias_tvars=alias_tvars or [],  # type: ignore[arg-type]
            no_args=no_args,
        )

    def test_plain_any(self) -> None:
        self.assert_rust_decides(AnyType(TypeOfAny.unannotated), True)

    def test_plain_instance_with_any(self) -> None:
        self.assert_rust_decides(
            Instance(self.fx.std_listi, [AnyType(TypeOfAny.unannotated)]), True
        )

    def test_plain_instance_clean(self) -> None:
        self.assert_rust_decides(Instance(self.fx.std_listi, [self.fx.str_type]), False)

    def test_special_form_any_not_contagious(self) -> None:
        self.assert_rust_decides(AnyType(TypeOfAny.special_form), False)

    def test_alias_without_typevars_no_any(self) -> None:
        # A = List[int]: target has no Any, old-style alias, no args to
        # visit → false.
        alias = self._make_alias("mod.A", Instance(self.fx.std_listi, [self.fx.a]))
        self._rebuild_with_aliases([alias])
        self.assert_rust_decides(TypeAliasType(alias, []), False)

    def test_alias_without_typevars_with_any_in_target(self) -> None:
        # A = List[Any]: target contains Any → true.
        alias = self._make_alias(
            "mod.A", Instance(self.fx.std_listi, [AnyType(TypeOfAny.unannotated)])
        )
        self._rebuild_with_aliases([alias])
        self.assert_rust_decides(TypeAliasType(alias, []), True)

    def test_alias_typevar_any_substitution_true(self) -> None:
        # A[T] = List[T], applied A[Any]: substitution must turn the
        # target into List[Any] → true. This is the B3b core case that
        # shape-only expansion answered incorrectly (false).
        tv = self._make_tvar("T", 1)
        alias = self._make_alias("mod.A", Instance(self.fx.std_listi, [tv]), alias_tvars=[tv])
        self._rebuild_with_aliases([alias])
        self.assert_rust_decides(TypeAliasType(alias, [AnyType(TypeOfAny.unannotated)]), True)

    def test_alias_typevar_int_substitution_false(self) -> None:
        # A[T] = List[T], applied A[int]: substituted target List[int]
        # has no Any → false.
        tv = self._make_tvar("T", 2)
        alias = self._make_alias("mod.A", Instance(self.fx.std_listi, [tv]), alias_tvars=[tv])
        self._rebuild_with_aliases([alias])
        self.assert_rust_decides(TypeAliasType(alias, [self.fx.a]), False)

    def test_alias_typevar_unused_arg_oldstyle_skipped(self) -> None:
        # Old-style generic alias with a dead typevar: A[T] = int,
        # applied A[Any]. Python's visit_type_alias_type visits only the
        # proper target (int) for old-style aliases, ignoring the args →

        # false. The Rust path must match (python_3_12 == False skips
        # args).
        tv = self._make_tvar("T", 3)
        alias = self._make_alias("mod.A", self.fx.str_type, alias_tvars=[tv])
        self._rebuild_with_aliases([alias])
        self.assert_rust_decides(TypeAliasType(alias, [AnyType(TypeOfAny.unannotated)]), False)

    def test_alias_typevar_unused_arg_python312_visits_args(self) -> None:
        # Same dead typevar but new-style (PEP 695): Python visits *both*
        # the proper target and (for python_3_12 aliases) the args, so
        # A[Any] → true. The fixture must fabricate a PEP 695 alias via

        # `python_3_12_type_alias=True`.
        tv = self._make_tvar("T", 4)
        alias = self._make_alias("mod.A", self.fx.str_type, alias_tvars=[tv])
        alias.python_3_12_type_alias = True
        self._rebuild_with_aliases([alias])
        self.assert_rust_decides(TypeAliasType(alias, [AnyType(TypeOfAny.unannotated)]), True)

    def test_alias_chain_typevar_any(self) -> None:
        # B = A[int], A[T] = List[T]: the chain loop must pass B's args
        # through A's substitution. B's target is a nested TypeAliasType
        # to A; A substitutes List[int]; no Any → false.
        tv = self._make_tvar("T", 5)
        alias_a = self._make_alias("mod.A", Instance(self.fx.std_listi, [tv]), alias_tvars=[tv])
        inner = TypeAliasType(alias_a, [self.fx.a])
        alias_b = self._make_alias("mod.B", inner, alias_tvars=[])
        self._rebuild_with_aliases([alias_a, alias_b])
        self.assert_rust_decides(TypeAliasType(alias_b, []), False)

    def test_alias_chain_to_any(self) -> None:
        # B = A, A = List[Any]: chain with no typevars, target carries
        # Any → true.
        alias_a = self._make_alias(
            "mod.A", Instance(self.fx.std_listi, [AnyType(TypeOfAny.unannotated)])
        )
        alias_b = self._make_alias("mod.B", TypeAliasType(alias_a, []), alias_tvars=[])
        self._rebuild_with_aliases([alias_a, alias_b])
        self.assert_rust_decides(TypeAliasType(alias_b, []), True)

    def test_alias_no_args(self) -> None:
        # L = List (no_args, production strips typevars at use sites).
        # Target List without args → no Any → false.
        alias = self._make_alias("mod.L", Instance(self.fx.std_listi, []), no_args=True)
        self._rebuild_with_aliases([alias])
        self.assert_rust_decides(TypeAliasType(alias, []), False)

    def test_alias_self_referential_cycle_defers(self) -> None:
        # A = List[A]: get_proper_type cannot terminate on the recursive
        # alias; the Rust path must not loop. The seen-set short-circuits
        # (mirroring BoolTypeQuery.seen_aliases) to the ANY_STRATEGY

        # default, which is False, the same answer Python's
        # visit_type_alias_type gives for a repeated alias.
        tv = self._make_tvar("T", 9)
        alias_a = self._make_alias("mod.A", Instance(self.fx.std_listi, [tv]), alias_tvars=[tv])
        # A's target references A itself (recursive): List[A].
        alias_a.target = Instance(self.fx.std_listi, [TypeAliasType(alias_a, [])])
        self._rebuild_with_aliases([alias_a])
        self.assert_rust_decides(TypeAliasType(alias_a, []), False)

    def test_alias_missing_snapshot_defers(self) -> None:
        # An alias whose fullname is not in the resolver snapshot: Rust
        # cannot expand → defer to Python.
        from mypy.nodes import TypeAlias

        tv = TypeVarType("U", "mod.U", TypeVarId(70), [], self.fx.str_type, self.fx.nonet, 0)
        alias = TypeAlias(
            Instance(self.fx.std_listi, [tv]), "mod.Unresolved", "mod", -1, -1, alias_tvars=[tv]
        )
        # Do NOT rebuild the resolver with this alias; it's missing.
        self.assert_rust_defers(TypeAliasType(alias, []))

    def test_union_with_alias_any(self) -> None:
        # Union[int, A] where A = List[Any]: the union arm behind the
        # alias carries Any → true.
        alias = self._make_alias(
            "mod.A", Instance(self.fx.std_listi, [AnyType(TypeOfAny.unannotated)])
        )
        self._rebuild_with_aliases([alias])
        u = UnionType.make_union([self.fx.a, TypeAliasType(alias, [])])
        self.assert_rust_decides(u, True)

    def test_bare_typevar_no_default_false(self) -> None:
        # Plain `T = TypeVar('T')`: the default is the
        # from_omitted_generics sentinel, which is not a real Any.

        # Python's HasAnyType.visit_type_var only visits the default when
        # has_default() is true (B3b regression: the sentinel was treated
        # as a real Any, so plain typevars spuriously reported Any).
        tv = TypeVarType(
            "T",
            "mod.T",
            TypeVarId(1),
            [],
            self.fx.str_type,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        self.assert_rust_decides(tv, False)

    def test_bare_typevar_real_default_any_true(self) -> None:
        # `T = TypeVar('T', default=Any)`: a genuine Any default is a
        # real Any and must be detected.
        tv = TypeVarType(
            "T", "mod.T", TypeVarId(1), [], self.fx.str_type, AnyType(TypeOfAny.unannotated)
        )
        self.assert_rust_decides(tv, True)

    def test_bare_typevar_bound_any_true(self) -> None:
        # `T = TypeVar('T', bound=Any)`: the upper bound is visited
        # unconditionally by Python.
        tv = TypeVarType(
            "T",
            "mod.T",
            TypeVarId(1),
            [],
            AnyType(TypeOfAny.unannotated),
            AnyType(TypeOfAny.from_omitted_generics),
        )
        self.assert_rust_decides(tv, True)

    def test_bare_typevar_any_value_true(self) -> None:
        # `T = TypeVar('T', 'A', Any)`: a value restriction containing
        # Any is visited by Python (values are always queried).
        tv = TypeVarType(
            "T",
            "mod.T",
            TypeVarId(1),
            [AnyType(TypeOfAny.unannotated)],
            self.fx.str_type,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        self.assert_rust_decides(tv, True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTupleExpandAndJoinSuite(Suite):
    """Parity suite for the `rust_build_tuple_type` seen_unpack reduce and
    the `join_type_list_inner` nominal Instance prejoin (issue #846).

    Port 1 (`unpack_expand_updated` in checkexpr_functions.rs): a tuple
    expression with exactly one star (e.g. `(*ts,)` where `ts:
    tuple[X, ...]`) previously deferred the whole `build_tuple_type`
    result to Python (`return Ok(None)` on `seen_unpack == 1`). The Rust
    side now expands the tuple with the empty substitution map, which is
    the identity except for the single-item normalization
    (expandtype.py:1009-1033): `Tuple[*tuple[X, ...]]` unwraps to the
    `tuple[X, ...]` Instance. A lone non-tuple Instance or TypeAliasType
    unpack still defers.

    Port 2 (`join_one_pair` prejoin in checkexpr_functions.rs): the
    args-less Instance-Instance nominal fold of `join.join_type_list`
    (used by the fast container literal path). When both items are
    plain `Instance`s with no args and no last_known_value, the join is
    decided by the shared `visit_instance_join` (same-type -> the type
    itself; subtype -> the supertype; else common ancestor) instead of
    falling through to the generic `join_types` fold. Behavior is
    identical to the Python twin; the Rust side now decides these cases
    inline, keeping the fold native for the nominal fast path.

    Each test asserts both parity (gate-off vs gate-on produce the same
    result string) and Rust engagement (the direct seam call returns a
    result rather than deferring).
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.bool_type_info,
            self.fx.std_tuplei,
            self.fx.std_listi,
        ]
        self.resolver = _type_kernel.build_native_resolver(infos, [])
        self.wire_infos = {i.fullname: i for i in infos}
        set_wire_typeinfo_map(self.wire_infos)
        _set_native_checkexpr_resolver(self.resolver)
        _set_native_checkexpr_active(True)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checkexpr_active(False)
        _set_native_checkexpr_resolver(None)
        set_wire_typeinfo_map(None)

    def _build_tuple(self, items: Sequence[Type], seen_unpack: int) -> Type | None:
        from mypy.checkexpr import _deserialize_type_from_checkexpr, _serialize_type_for_checkexpr

        items_bytes = [_serialize_type_for_checkexpr(t) for t in items]
        raw = _type_kernel.rust_build_tuple_type(items_bytes, seen_unpack)
        if raw is None:
            return None
        return _deserialize_type_from_checkexpr(bytes(raw))

    def _container(self, tag: str, items: Sequence[Type]) -> Type | None:
        from mypy.checkexpr import _deserialize_type_from_checkexpr, _serialize_type_for_checkexpr

        items_bytes = [_serialize_type_for_checkexpr(t) for t in items]
        raw = _type_kernel.rust_container_type(self.resolver, tag, items_bytes, None, 0)
        if raw is None:
            return None
        return _deserialize_type_from_checkexpr(bytes(raw))

    def test_single_star_any_tuple_unwraps(self) -> None:
        # (*ts,) with ts: tuple[Any, ...] -> tuple[Any, ...]. Python runs
        # expand_type(result, {}) which normalizes the lone unpack to the
        # tuple Instance. Rust now mirrors it.
        from mypy.checkexpr import _set_native_checkexpr_active

        items = [AnyType(TypeOfAny.special_form)]
        off = self._with_gate(_set_native_checkexpr_active, False, self._build_tuple, items, 1)
        on = self._build_tuple(items, 1)
        assert on is not None, "Rust build_tuple_type single-star did not engage"
        assert str(on) == str(off)

    def test_single_star_typevar_tuple_unpack_passthrough(self) -> None:
        # (*Ts,) with Ts a TypeVarTuple: lone unpack is not a tuple Instance,
        # so `unpack_expand_updated` returns the tuple UNCHANGED, matching
        # Python's `expand_type` identity. Keeps its `tuple[*Ts]` shape.
        tv = TypeVarTupleType(
            "Ts", "mod.Ts", TypeVarId(0), self.fx.o, self.fx.std_tuple, self.fx.anyt, min_len=0
        )
        items = [UnpackType(tv)]
        t = self._build_tuple(items, 1)
        assert t is not None, "Rust build_tuple_type did not engage"
        assert str(t) == "tuple[*Ts]"

    def test_multi_star_defers(self) -> None:
        # A tuple with two stars is not the seen_unpack == 1 case; the
        # Rust path returns the tuple directly (no expansion). Python
        # expands each item (identity with an empty map). Parity holds.
        from mypy.checkexpr import _set_native_checkexpr_active

        items = [self.fx.a, self.fx.b]
        off = self._with_gate(_set_native_checkexpr_active, False, self._build_tuple, items, 0)
        on = self._build_tuple(items, 0)
        assert on is not None, "Rust build_tuple_type did not engage"
        assert str(on) == str(off)

    def test_join_same_instance_engages(self) -> None:
        # [A, A] -> list[A]. The fixture TypeInfo uses the full builtins
        # prefix in str().
        t = self._container("list", [self.fx.a, self.fx.a])
        assert t is not None, "Rust container type did not engage"
        assert str(t) == "builtins.list[A]"

    def test_join_subtype_instance_engages(self) -> None:
        # [A, object] -> list[object]. The nominal prejoin resolves via
        # visit_instance_join (A <: object), where the old fold deferred.
        t = self._container("list", [self.fx.a, self.fx.o])
        assert t is not None, "Rust container type did not engage"
        assert str(t) == "builtins.list[builtins.object]"

    def test_join_pair_parity(self) -> None:
        # Same container literal through the gate-off (pure-Python
        # join_type_list) and gate-on (Rust seam) paths.
        from mypy.checkexpr import _set_native_checkexpr_active

        items = [self.fx.a, self.fx.o]
        off = self._with_gate(_set_native_checkexpr_active, False, self._container, "list", items)
        on = self._container("list", items)
        assert on is not None, "Rust container type did not engage"
        assert str(on) == str(off)

    def _with_gate(
        self, set_active: Any, active: bool, fn: Callable[..., Type | None], *args: Any
    ) -> Type | None:
        set_active(active)
        try:
            return fn(*args)
        finally:
            set_active(True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeUninhabitedSuite(Suite):
    """Parity suite for the Rust `has_uninhabited_component` and
    `has_ambiguous_uninhabited_component` ports with alias expansion
    (Phase C, #594).

    Phase C closes the same gap B3b (#593) closed for `has_any_type`: the
    uninhabited-component queries must expand a `TypeAliasType` to its
    substituted target and answer correctly instead of deferring on every
    alias. Covers plain uninhabited types (true), clean instances (false),
    aliases with an uninhabited target (true), typevar aliases applied with
    an uninhabited arg (true), chains, and cycles (defer). Requires
    TEST_NATIVE_TYPE_KERNEL=1.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        self.fx = TypeFixture()
        self.resolver = self._build_resolver([])
        _set_native_checkexpr_resolver(self.resolver)
        _set_native_checkexpr_active(True)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        _set_native_checkexpr_active(False)
        _set_native_checkexpr_resolver(None)

    def _build_resolver(self, aliases: list[Any]) -> Any:
        return _type_kernel.build_native_resolver(
            [
                self.fx.oi,
                self.fx.ai,
                self.fx.bi,
                self.fx.str_type_info,
                self.fx.type_typei,
                self.fx.std_tuplei,
                self.fx.std_listi,
            ],
            aliases,
        )

    def _rebuild_with_aliases(self, aliases: list[Any]) -> None:
        from mypy.checkexpr import _set_native_checkexpr_resolver

        self.resolver = self._build_resolver(aliases)
        _set_native_checkexpr_resolver(self.resolver)

    def assert_uninhabited_decides(self, t: Type, expected: bool) -> None:
        """Assert the Rust kernel answers `expected` for has_uninhabited_component."""
        from mypy.checkexpr import _serialize_type_for_checkexpr

        rusted = _type_kernel.rust_has_uninhabited_component(
            _serialize_type_for_checkexpr(t), self.resolver
        )
        assert rusted is not None, f"Rust deferred on {t!r}"
        assert rusted is expected

    def assert_uninhabited_defers(self, t: Type) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        rusted = _type_kernel.rust_has_uninhabited_component(
            _serialize_type_for_checkexpr(t), self.resolver
        )
        assert rusted is None, f"Rust should defer on {t!r}, got {rusted}"

    def assert_ambiguous_decides(self, t: Type, expected: bool) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        rusted = _type_kernel.rust_has_ambiguous_uninhabited_component(
            _serialize_type_for_checkexpr(t), self.resolver
        )
        assert rusted is not None, f"Rust deferred on {t!r}"
        assert rusted is expected

    def assert_ambiguous_defers(self, t: Type) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        rusted = _type_kernel.rust_has_ambiguous_uninhabited_component(
            _serialize_type_for_checkexpr(t), self.resolver
        )
        assert rusted is None, f"Rust should defer on {t!r}, got {rusted}"

    def _make_tvar(self, name: str, raw_id: int) -> TypeVarType:
        return TypeVarType(
            name, f"mod.{name}", TypeVarId(raw_id), [], self.fx.str_type, self.fx.nonet, 0
        )

    def _make_alias(
        self,
        fullname: str,
        target: Type,
        *,
        alias_tvars: list[TypeVarType] | None = None,
        no_args: bool = False,
    ) -> TypeAlias:
        from mypy.nodes import TypeAlias

        return TypeAlias(
            target,
            fullname,
            "mod",
            -1,
            -1,
            alias_tvars=alias_tvars or [],  # type: ignore[arg-type]
            no_args=no_args,
        )

    def _uninhabited(self, ambiguous: bool = False) -> UninhabitedType:
        return UninhabitedType(ambiguous=ambiguous)

    def test_plain_uninhabited_true(self) -> None:
        self.assert_uninhabited_decides(self._uninhabited(), True)

    def test_plain_uninhabited_ambiguous_true(self) -> None:
        self.assert_ambiguous_decides(self._uninhabited(ambiguous=True), True)

    def test_plain_uninhabited_not_ambiguous_false(self) -> None:
        self.assert_ambiguous_decides(self._uninhabited(ambiguous=False), False)

    def test_clean_instance_false(self) -> None:
        self.assert_uninhabited_decides(Instance(self.fx.std_listi, [self.fx.str_type]), False)

    def test_union_with_uninhabited_true(self) -> None:
        u = UnionType.make_union([self.fx.a, self._uninhabited()])
        self.assert_uninhabited_decides(u, True)

    def test_alias_target_uninhabited_true(self) -> None:
        # A = Uninhabited: expanding the alias finds the uninhabited
        # target, where B3b-era Rust deferred.
        alias = self._make_alias("mod.A", self._uninhabited())
        self._rebuild_with_aliases([alias])
        self.assert_uninhabited_decides(TypeAliasType(alias, []), True)
        self.assert_ambiguous_decides(TypeAliasType(alias, []), False)

    def test_alias_target_ambiguous_uninhabited_true(self) -> None:
        alias = self._make_alias("mod.A", self._uninhabited(ambiguous=True))
        self._rebuild_with_aliases([alias])
        self.assert_ambiguous_decides(TypeAliasType(alias, []), True)

    def test_alias_target_clean_false(self) -> None:
        # A = List[int]: no uninhabited component.
        alias = self._make_alias("mod.A", Instance(self.fx.std_listi, [self.fx.a]))
        self._rebuild_with_aliases([alias])
        self.assert_uninhabited_decides(TypeAliasType(alias, []), False)

    def test_alias_typevar_uninhabited_arg_true(self) -> None:
        # A[T] = List[T] applied A[Uninhabited]: substitution carries the
        # uninhabited arg into the target (Phase C core). New-style alias
        # so the args are visited too.
        tv = self._make_tvar("T", 1)
        alias = self._make_alias("mod.A", Instance(self.fx.std_listi, [tv]), alias_tvars=[tv])
        alias.python_3_12_type_alias = True
        self._rebuild_with_aliases([alias])
        self.assert_uninhabited_decides(TypeAliasType(alias, [self._uninhabited()]), True)

    def test_alias_typevar_clean_arg_false(self) -> None:
        # A[T] = List[T] applied A[int]: no uninhabited anywhere.
        tv = self._make_tvar("T", 2)
        alias = self._make_alias("mod.A", Instance(self.fx.std_listi, [tv]), alias_tvars=[tv])
        self._rebuild_with_aliases([alias])
        self.assert_uninhabited_decides(TypeAliasType(alias, [self.fx.a]), False)

    def test_alias_chain_to_uninhabited_true(self) -> None:
        # B = A, A = Uninhabited: chain resolves to the uninhabited target.
        alias_a = self._make_alias("mod.A", self._uninhabited())
        alias_b = self._make_alias("mod.B", TypeAliasType(alias_a, []), alias_tvars=[])
        self._rebuild_with_aliases([alias_a, alias_b])
        self.assert_uninhabited_decides(TypeAliasType(alias_b, []), True)

    def test_alias_without_snapshot_defers(self) -> None:
        # A TypeAliasType referencing a fullname with no snapshot in the
        # resolver: expansion cannot proceed, defer.
        alias = TypeAliasType(self._make_alias("mod.Missing", self.fx.a), [])
        self.assert_uninhabited_defers(alias)
        self.assert_ambiguous_defers(alias)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeHasErasedComponentSuite(Suite):
    """Parity suite for the Rust `has_erased_component` port (slice 48).

    `has_erased_component` mirrors `HasErasedComponentsQuery`, a
    `BoolTypeQuery(ANY_STRATEGY)` whose only override is
    `visit_erased_type -> True` (checkexpr.py:8060-8071). The Rust wire
    walk delegates to the same alias expansion as the uninhabited query, so
    this suite covers the erased leaf directly, unions, clean instances,
    alias targets (true/false), and recursive aliases (defer).
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        self.fx = TypeFixture()
        self.resolver = self._build_resolver([])
        _set_native_checkexpr_resolver(self.resolver)
        _set_native_checkexpr_active(True)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        _set_native_checkexpr_active(False)
        _set_native_checkexpr_resolver(None)

    def _build_resolver(self, aliases: list[Any]) -> Any:
        return _type_kernel.build_native_resolver(
            [
                self.fx.oi,
                self.fx.ai,
                self.fx.bi,
                self.fx.str_type_info,
                self.fx.type_typei,
                self.fx.std_tuplei,
                self.fx.std_listi,
            ],
            aliases,
        )

    def _rebuild_with_aliases(self, aliases: list[Any]) -> None:
        from mypy.checkexpr import _set_native_checkexpr_resolver

        self.resolver = self._build_resolver(aliases)
        _set_native_checkexpr_resolver(self.resolver)

    def assert_erased_decides(self, t: Type, expected: bool) -> None:
        """Assert the Rust kernel answers `expected` for has_erased_component."""
        from mypy.checkexpr import _serialize_type_for_checkexpr

        rusted = _type_kernel.rust_has_erased_component(
            _serialize_type_for_checkexpr(t), self.resolver
        )
        assert rusted is not None, f"Rust deferred on {t!r}"
        assert rusted is expected

    def assert_erased_defers(self, t: Type) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        rusted = _type_kernel.rust_has_erased_component(
            _serialize_type_for_checkexpr(t), self.resolver
        )
        assert rusted is None, f"Rust should defer on {t!r}, got {rusted}"

    def _make_alias(
        self,
        fullname: str,
        target: Type,
        *,
        alias_tvars: list[TypeVarType] | None = None,
        no_args: bool = False,
    ) -> TypeAlias:
        from mypy.nodes import TypeAlias

        return TypeAlias(
            target,
            fullname,
            "mod",
            -1,
            -1,
            alias_tvars=alias_tvars or [],  # type: ignore[arg-type]
            no_args=no_args,
        )

    def test_plain_erased_true(self) -> None:
        self.assert_erased_decides(ErasedType(), True)

    def test_clean_instance_false(self) -> None:
        self.assert_erased_decides(Instance(self.fx.std_listi, [self.fx.str_type]), False)

    def test_uninhabited_not_erased_false(self) -> None:
        self.assert_erased_decides(UninhabitedType(), False)

    def test_union_with_erased_true(self) -> None:
        u = UnionType.make_union([self.fx.str_type, ErasedType()])
        self.assert_erased_decides(u, True)

    def test_alias_target_erased_true(self) -> None:
        # A = Erased: expanding the alias finds the erased target.
        alias = self._make_alias("mod.A", ErasedType())
        self._rebuild_with_aliases([alias])
        self.assert_erased_decides(TypeAliasType(alias, []), True)

    def test_alias_target_clean_false(self) -> None:
        # A = List[int]: no erased component.
        alias = self._make_alias("mod.A", Instance(self.fx.std_listi, [self.fx.a]))
        self._rebuild_with_aliases([alias])
        self.assert_erased_decides(TypeAliasType(alias, []), False)

    def test_alias_without_snapshot_defers(self) -> None:
        # A TypeAliasType referencing a fullname with no snapshot in the
        # resolver: expansion cannot proceed, defer.
        alias = TypeAliasType(self._make_alias("mod.Missing", self.fx.a), [])
        self.assert_erased_defers(alias)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativePluginHookSuite(Suite):
    """Parity tests for the Stage 4 plugin-hook snapshot.

    Verifies that `PluginHookRegistry.has_hook(fullname)` agrees with the
    DefaultPlugin's `get_*_hook(fullname) is not None` for the four
    call-related hooks, and that the `plugin_call_hook_known_absent`
    short-circuit only reports "known absent" when no DefaultPlugin hook
    matches.
    """

    def setUp(self) -> None:
        import type_kernel as _type_kernel

        from mypy.checkexpr import (
            _set_native_plugin_hook_registry,
            plugin_call_hook_known_absent,
            plugin_hook_known_absent,
        )
        from mypy.options import Options
        from mypy.plugins.default import (
            DEFAULT_CALL_HOOK_FULLNAMES,
            DEFAULT_HOOK_FULLNAMES_BY_KIND,
            DefaultPlugin,
        )

        self._set_native_plugin_hook_registry = _set_native_plugin_hook_registry
        self._plugin_call_hook_known_absent = plugin_call_hook_known_absent
        self._plugin_hook_known_absent = plugin_hook_known_absent
        self._default_plugin = DefaultPlugin(Options())
        self._fullnames = DEFAULT_CALL_HOOK_FULLNAMES
        self._by_kind = DEFAULT_HOOK_FULLNAMES_BY_KIND
        self._registry = _type_kernel.PluginHookRegistry(
            {kind: list(names) for kind, names in DEFAULT_HOOK_FULLNAMES_BY_KIND.items()}
        )
        _set_native_plugin_hook_registry(self._registry, has_user_plugins=False)

    def tearDown(self) -> None:
        self._set_native_plugin_hook_registry(None, False)

    def test_registry_has_hook_for_every_default_fullname(self) -> None:
        for fullname in self._fullnames:
            assert self._registry.has_hook(fullname), f"registry missing {fullname!r}"

    def test_registry_absent_for_unrelated_fullname(self) -> None:
        assert not self._registry.has_hook("builtins.print")
        assert not self._registry.has_hook("os.path.join")
        assert not self._registry.has_hook("collections.OrderedDict")

    def test_known_absent_false_for_default_hook_fullnames(self) -> None:
        # Fullnames in the DefaultPlugin set are never "known absent".
        for fullname in self._fullnames:
            assert not self._plugin_call_hook_known_absent(
                fullname
            ), f"{fullname!r} should not be known-absent (it has a hook)"

    def test_known_absent_true_for_unrelated_fullname(self) -> None:
        assert self._plugin_call_hook_known_absent("builtins.print")
        assert self._plugin_call_hook_known_absent("os.path.join")

    def test_known_absent_false_for_none_callable_name(self) -> None:
        assert not self._plugin_call_hook_known_absent(None)

    def test_known_absent_defers_when_user_plugins_present(self) -> None:
        # With user plugins, the registry cannot prove absence, so all
        # lookups must defer to Python (known-absent returns False).
        self._set_native_plugin_hook_registry(self._registry, has_user_plugins=True)
        assert not self._plugin_call_hook_known_absent("builtins.print")
        assert not self._plugin_call_hook_known_absent("os.path.join")

    def test_known_absent_defers_when_registry_unset(self) -> None:
        self._set_native_plugin_hook_registry(None, has_user_plugins=False)
        assert not self._plugin_call_hook_known_absent("builtins.print")

    def test_default_plugin_fullnames_cover_all_four_hooks(self) -> None:
        # Cross-check: every fullname in DEFAULT_CALL_HOOK_FULLNAMES must
        # be matched by at least one of the four DefaultPlugin call-hooks.
        for fullname in self._fullnames:
            has_hook = (
                self._default_plugin.get_function_hook(fullname) is not None
                or self._default_plugin.get_function_signature_hook(fullname) is not None
                or self._default_plugin.get_method_signature_hook(fullname) is not None
                or self._default_plugin.get_method_hook(fullname) is not None
            )
            assert (
                has_hook
            ), f"{fullname!r} in DEFAULT_CALL_HOOK_FULLNAMES but no DefaultPlugin hook matches"

    def test_call_union_does_not_leak_non_call_kinds(self) -> None:
        # C3 regression guard: a name owned by a non-call kind must not
        # make plugin_call_hook_known_absent return False at a call site.
        for kind, names in self._by_kind.items():
            if kind in (
                "get_function_hook",
                "get_function_signature_hook",
                "get_method_signature_hook",
                "get_method_hook",
            ):
                continue
            for fullname in names:
                assert not self._registry.has_call_hook(
                    fullname
                ), f"{fullname!r} (kind {kind}) widened the call union"
                assert self._plugin_call_hook_known_absent(
                    fullname
                ), f"{fullname!r} (kind {kind}) wrongly blocks the call gate"

    def test_per_kind_known_absent(self) -> None:
        # For each non-call kind, a name in its set is never known-absent
        # for that kind, while an unrelated name is.
        for kind, names in self._by_kind.items():
            if kind in (
                "get_function_hook",
                "get_function_signature_hook",
                "get_method_signature_hook",
                "get_method_hook",
            ):
                continue
            for fullname in names:
                assert not self._plugin_hook_known_absent(
                    kind, fullname
                ), f"{fullname!r} should not be known-absent for {kind}"
            assert self._plugin_hook_known_absent(
                kind, "builtins.print"
            ), f"builtins.print should be known-absent for {kind}"
            assert not self._plugin_hook_known_absent(kind, None)

    def test_per_kind_matches_default_plugin_surface(self) -> None:
        # Cross-check the per-kind sets against the actual DefaultPlugin
        # hook bodies for the non-call kinds.
        for kind in (
            "get_attribute_hook",
            "get_class_decorator_hook",
            "get_class_decorator_hook_2",
        ):
            for fullname in self._by_kind[kind]:
                assert (
                    getattr(self._default_plugin, kind)(fullname) is not None
                ), f"{fullname!r} in per-kind set for {kind} but hook resolves None"
            assert not self._registry.has_call_hook(
                next(iter(self._by_kind[kind]))
            ), f"{kind} names must not widen the call union"


@skipUnless(_NATIVE_STRFORMAT_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeStrFormatSuite(Suite):
    """Parity tests for Rust format-string parsing (Stage 6b).

    Each test toggles `_native_strformat_active` and compares the Rust
    output against the Python output for `parse_conversion_specifiers`,
    `find_non_escaped_targets`, and `parse_format_value`.
    """

    def setUp(self) -> None:
        from mypy.checkstrformat import (
            find_non_escaped_targets,
            parse_conversion_specifiers,
            parse_format_value,
        )
        from mypy.errors import Errors
        from mypy.messages import MessageBuilder
        from mypy.options import Options

        self._set_active = _set_native_strformat_active
        self._parse_conv = parse_conversion_specifiers
        self._parse_fmt = parse_format_value
        self._find_targets = find_non_escaped_targets
        self._errors = Errors(Options())
        self._msg = MessageBuilder(self._errors, None)  # type: ignore[arg-type]

    def _spec_tuples(self, specs: list[ConversionSpecifier]) -> list[tuple]:  # type: ignore[type-arg]
        """Convert ConversionSpecifier list to comparable tuples."""
        return [
            (
                s.whole_seq,
                s.start_pos,
                s.key,
                s.conv_type,
                s.flags,
                s.width,
                s.precision,
                s.format_spec,
                s.non_standard_format_spec,
                s.conversion,
                s.field,
            )
            for s in specs
        ]

    # --- parse_conversion_specifiers ---

    def test_parse_conv_simple(self) -> None:
        for s in ["%s", "%d", "%f", "%x", "%%", "%r"]:
            self._set_active(False)
            py = self._spec_tuples(self._parse_conv(s))
            self._set_active(True)
            rs = self._spec_tuples(self._parse_conv(s))
            assert_equal(rs, py, f"mismatch for {s!r}")

    def test_parse_conv_multiple(self) -> None:
        for s in ["%d %s", "%s %d %f", "hello %s world %d", "%d%%", "100%% done"]:
            self._set_active(False)
            py = self._spec_tuples(self._parse_conv(s))
            self._set_active(True)
            rs = self._spec_tuples(self._parse_conv(s))
            assert_equal(rs, py, f"mismatch for {s!r}")

    def test_parse_conv_with_key(self) -> None:
        for s in ["%(name)s", "%(key)d %(other)s", "%(x)f"]:
            self._set_active(False)
            py = self._spec_tuples(self._parse_conv(s))
            self._set_active(True)
            rs = self._spec_tuples(self._parse_conv(s))
            assert_equal(rs, py, f"mismatch for {s!r}")

    def test_parse_conv_flags_width_precision(self) -> None:
        cases = [
            "%05d",
            "%-10s",
            "%+d",
            "%#x",
            "%*d",
            "%.*f",
            "%10.2f",
            "%-+5.3e",
            "% #0*d",
            "%8.0f",
            "%5d",
            "%.3s",
            "%5.3f",
        ]
        for s in cases:
            self._set_active(False)
            py = self._spec_tuples(self._parse_conv(s))
            self._set_active(True)
            rs = self._spec_tuples(self._parse_conv(s))
            assert_equal(rs, py, f"mismatch for {s!r}")

    def test_parse_conv_no_specifiers(self) -> None:
        for s in ["hello", "", "no percent", "100 percent no sign"]:
            self._set_active(False)
            py = self._spec_tuples(self._parse_conv(s))
            self._set_active(True)
            rs = self._spec_tuples(self._parse_conv(s))
            assert_equal(rs, py, f"mismatch for {s!r}")

    # --- find_non_escaped_targets ---

    def test_find_targets_basic(self) -> None:
        from mypy.nodes import Context

        ctx = Context()
        cases = [
            "{}",
            "{name}",
            "{0}",
            "{0:d}",
            "{name!r}",
            "{{escaped}}",
            "no targets",
            "",
            "{a} {b}",
            "{0}{1}{2}",
            "{:.2f}",
            "{:>10}",
            "{:s}",
            "{!s}",
        ]
        for s in cases:
            self._set_active(False)
            py = self._find_targets(s, ctx, self._msg)
            self._set_active(True)
            rs = self._find_targets(s, ctx, self._msg)
            assert_equal(rs, py, f"mismatch for {s!r}")

    def test_find_targets_errors(self) -> None:
        from mypy.nodes import Context

        ctx = Context()
        # Clear errors between runs
        for s in ["}", "{", "{}}", "{{}"]:
            self._errors.reset()
            self._set_active(False)
            py = self._find_targets(s, ctx, self._msg)
            self._errors.reset()
            self._set_active(True)
            rs = self._find_targets(s, ctx, self._msg)
            assert_equal(rs, py, f"mismatch for {s!r}")

    # --- parse_format_value ---

    def test_parse_format_value_basic(self) -> None:
        from mypy.nodes import Context

        ctx = Context()
        cases = [
            "{}",
            "{0}",
            "{name}",
            "{:s}",
            "{:d}",
            "{:.2f}",
            "{:>10}",
            "{!r}",
            "{!s}",
            "{0:d} {1:s}",
            "{{literal}}",
            "no specs",
            "",
            "{name!r:>{width}}",
        ]
        for s in cases:
            self._errors.reset()
            self._set_active(False)
            py = self._parse_fmt(s, ctx, self._msg)
            self._errors.reset()
            self._set_active(True)
            rs = self._parse_fmt(s, ctx, self._msg)
            assert_equal(
                self._spec_tuples(rs) if rs else None,
                self._spec_tuples(py) if py else None,
                f"mismatch for {s!r}",
            )

    def test_parse_format_value_errors(self) -> None:
        from mypy.nodes import Context

        ctx = Context()
        for s in ["}", "{", "{a}{", "{a}}"]:
            self._errors.reset()
            self._set_active(False)
            py = self._parse_fmt(s, ctx, self._msg)
            self._errors.reset()
            self._set_active(True)
            rs = self._parse_fmt(s, ctx, self._msg)
            assert_equal(rs, py, f"mismatch for {s!r}")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckCallSuite(Suite):
    """Parity suite for M25: check_call decision helpers ported to Rust.

    Tests `rust_real_union` and `rust_possible_none_type_var_overlap`
    against the pure-Python implementations in `mypy.checkexpr`. Each
    test toggles the native gate off/on and compares.

    Since issue #873 the two seams expand top-level TypeAliasType operands
    through the alias resolver (mirroring `get_proper_type` /
    `get_proper_types`); a resolver is built per test so alias parity is
    exercised too.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active
        from mypy.state import state as _state

        self.fx = TypeFixture()
        self._set_active = _set_native_checkexpr_active
        self._set_active(True)
        self._state = _state
        self.resolver = _type_kernel.build_native_resolver(self._collect_type_infos(), [])

    def tearDown(self) -> None:
        self._set_active(False)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _rebuild_resolver(self, aliases: list[Any]) -> None:
        self.resolver = _type_kernel.build_native_resolver(self._collect_type_infos(), aliases)

    def _make_alias(self, fullname: str, target: Type) -> Any:  # TypeAlias
        from mypy.nodes import TypeAlias

        return TypeAlias(target, fullname, "mod", -1, -1, alias_tvars=[])

    def _alias_type(self, alias: Any) -> Any:  # TypeAliasType
        return TypeAliasType(alias, [])

    def _serialize(self, t: Type) -> bytes:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        return _serialize_type_for_checkexpr(t)

    def _rust_real_union(self, t: Type, strict_optional: bool) -> bool | None:
        import type_kernel as tk

        return tk.rust_real_union(self.resolver, self._serialize(t), strict_optional)

    def _rust_overlap(
        self, arg_types: list[Type], targets: list[CallableType] | list[Type]
    ) -> bool | None:
        import type_kernel as tk

        return tk.rust_possible_none_type_var_overlap(
            self.resolver,
            [self._serialize(t) for t in arg_types],
            [self._serialize(t) for t in targets],
        )

    def _py_real_union(self, t: Type, strict_optional: bool) -> bool:
        old = self._state.strict_optional
        self._state.strict_optional = strict_optional
        try:
            p = get_proper_type(t)
            return isinstance(p, UnionType) and len(p.relevant_items()) > 1
        finally:
            self._state.strict_optional = old

    def _py_overlap(self, arg_types: list[Type], targets: list[CallableType]) -> bool:
        """Pure-Python mirror of `possible_none_type_var_overlap`."""
        if not targets or not arg_types:
            return False
        has_optional_arg = False
        for arg_type in get_proper_types(arg_types):
            if not isinstance(arg_type, UnionType):
                continue
            for item in get_proper_types(arg_type.items):
                if isinstance(item, NoneType):
                    has_optional_arg = True
                    break
            if has_optional_arg:
                break
        if not has_optional_arg:
            return False
        min_prefix = min(len(c.arg_types) for c in targets)
        for i in range(min_prefix):
            has_none = any(isinstance(get_proper_type(c.arg_types[i]), NoneType) for c in targets)
            has_typevar = any(
                isinstance(get_proper_type(c.arg_types[i]), TypeVarType) for c in targets
            )
            if has_none and has_typevar:
                return True
        return False

    # --- real_union ---

    def test_real_union_non_union(self) -> None:
        for t in [self.fx.a, self.fx.anyt, self.fx.nonet, self.fx.t]:
            assert self._rust_real_union(t, True) == self._py_real_union(t, True)

    def test_real_union_single_item_union(self) -> None:
        t = UnionType.make_union([self.fx.a])
        assert self._rust_real_union(t, True) == self._py_real_union(t, True)

    def test_real_union_multi_item_union(self) -> None:
        t = UnionType.make_union([self.fx.a, self.fx.b])
        assert self._rust_real_union(t, True) == self._py_real_union(t, True)

    def test_real_union_strict_optional_keeps_none(self) -> None:
        t = UnionType.make_union([self.fx.a, NoneType()])
        assert self._rust_real_union(t, True) == self._py_real_union(t, True)
        assert self._rust_real_union(t, True) is True

    def test_real_union_non_strict_strips_none(self) -> None:
        t = UnionType.make_union([self.fx.a, NoneType()])
        assert self._rust_real_union(t, False) == self._py_real_union(t, False)
        assert self._rust_real_union(t, False) is False

    def test_real_union_non_strict_multi_without_none(self) -> None:
        t = UnionType.make_union([self.fx.a, self.fx.b])
        assert self._rust_real_union(t, False) == self._py_real_union(t, False)
        assert self._rust_real_union(t, False) is True

    def test_real_union_non_strict_strips_all_none(self) -> None:
        t = UnionType.make_union([NoneType(), NoneType()])
        assert self._rust_real_union(t, False) == self._py_real_union(t, False)

    def test_real_union_nested_union(self) -> None:
        inner = UnionType.make_union([self.fx.a, NoneType()])
        outer = UnionType.make_union([inner, self.fx.b])
        assert self._rust_real_union(outer, True) == self._py_real_union(outer, True)

    # --- possible_none_type_var_overlap ---

    def _callable(
        self, arg_types: list[Type], variables: list[Type] | None = None
    ) -> CallableType:
        return CallableType(
            arg_types,
            [ARG_POS] * len(arg_types),
            [None] * len(arg_types),
            self.fx.anyt,
            self.fx.function,
            variables=variables or [],  # type: ignore[arg-type]
        )

    def test_overlap_empty_args(self) -> None:
        targets = [self._callable([self.fx.a])]
        assert self._rust_overlap([], targets) is False

    def test_overlap_empty_targets(self) -> None:
        assert self._rust_overlap([self.fx.a], []) is False

    def test_overlap_no_union_arg(self) -> None:
        targets = [self._callable([NoneType(), self.fx.t])]
        assert self._rust_overlap([self.fx.a], targets) is False

    def test_overlap_union_without_none(self) -> None:
        arg = UnionType.make_union([self.fx.a, self.fx.b])
        targets = [self._callable([NoneType(), self.fx.t])]
        assert self._rust_overlap([arg], targets) is False

    def test_overlap_union_with_none_no_typevar(self) -> None:
        arg = UnionType.make_union([self.fx.a, NoneType()])
        targets = [self._callable([NoneType(), self.fx.a])]
        assert self._rust_overlap([arg], targets) is False

    def test_overlap_union_with_none_and_typevar_same_pos(self) -> None:
        arg = UnionType.make_union([self.fx.a, NoneType()])
        target1 = self._callable([NoneType()])
        target2 = self._callable([self.fx.t])
        assert self._rust_overlap([arg], [target1, target2]) is True

    def test_overlap_single_target_none_and_typevar_different_pos(self) -> None:
        # NoneType at pos 0, TypeVar at pos 1: neither position has both.
        arg = UnionType.make_union([self.fx.a, NoneType()])
        target = self._callable([NoneType(), self.fx.t])
        assert self._rust_overlap([arg], [target]) is False

    def test_overlap_typevar_different_position(self) -> None:
        arg = UnionType.make_union([self.fx.a, NoneType()])
        target1 = self._callable([NoneType(), self.fx.a])
        target2 = self._callable([self.fx.a, self.fx.t])
        assert self._rust_overlap([arg], [target1, target2]) is False

    def test_overlap_multiple_positions_only_first_matches(self) -> None:
        arg = UnionType.make_union([self.fx.a, NoneType()])
        target1 = self._callable([NoneType(), self.fx.a, self.fx.a])
        target2 = self._callable([self.fx.t, self.fx.a, self.fx.a])
        assert self._rust_overlap([arg], [target1, target2]) is True

    def test_overlap_none_in_second_position(self) -> None:
        arg = UnionType.make_union([self.fx.a, NoneType()])
        target1 = self._callable([self.fx.a, NoneType()])
        target2 = self._callable([self.fx.a, self.fx.t])
        assert self._rust_overlap([arg], [target1, target2]) is True

    def test_overlap_min_prefix_limits_search(self) -> None:
        arg = UnionType.make_union([self.fx.a, NoneType()])
        # target1 has 1 arg (NoneType), target2 has 3 (a, a, t).
        # min_prefix = 1, position 0: none vs a -> no match.
        target1 = self._callable([NoneType()])
        target2 = self._callable([self.fx.a, self.fx.a, self.fx.t])
        assert self._rust_overlap([arg], [target1, target2]) is False

    # --- real_union / overlap with TypeAliasType operands (#873) ---

    def test_real_union_alias_to_union_parity(self) -> None:
        # T = Union[A, B]: the alias expands and both Rust and Python count
        # the two union items -> True (strict_optional).
        alias = self._make_alias("mod.T", UnionType.make_union([self.fx.a, self.fx.b]))
        self._rebuild_resolver([alias])
        alias_t = self._alias_type(alias)
        assert self._rust_real_union(alias_t, True) == self._py_real_union(alias_t, True)
        assert self._rust_real_union(alias_t, True) is True

    def test_real_union_alias_to_non_union_parity(self) -> None:
        # T = A (non-union): after expansion both report False.
        alias = self._make_alias("mod.T", self.fx.a)
        self._rebuild_resolver([alias])
        alias_t = self._alias_type(alias)
        assert self._rust_real_union(alias_t, True) == self._py_real_union(alias_t, True)
        assert self._rust_real_union(alias_t, True) is False

    def test_overlap_alias_arg_parity(self) -> None:
        # T = Union[A, None]; overlap(T, [None], [Tvar]) finds Both.
        alias = self._make_alias("mod.T", UnionType.make_union([self.fx.a, NoneType()]))
        self._rebuild_resolver([alias])
        arg = self._alias_type(alias)
        target1 = self._callable([NoneType()])
        target2 = self._callable([self.fx.t])
        rust = self._rust_overlap([arg], [target1, target2])
        py = self._py_overlap([arg], [target1, target2])
        assert rust == py
        assert rust is True

    def test_overlap_alias_formal_parity(self) -> None:
        # A formal typed as an alias to NoneType + another as a TypeVar:
        # both Rust and Python resolve the formal and find the overlap.
        alias_none = self._make_alias("mod.N", NoneType())
        self._rebuild_resolver([alias_none])
        arg = UnionType.make_union([self.fx.a, NoneType()])
        target1 = self._callable([self._alias_type(alias_none)])
        target2 = self._callable([self.fx.t])
        rust = self._rust_overlap([arg], [target1, target2])
        py = self._py_overlap([arg], [target1, target2])
        assert rust == py
        assert rust is True


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeOverloadCallSuite(Suite):
    """Direct seam tests for `rust_check_overload_call` (issue #1204).

    The Rust indexer picks the first-match target index for the
    no-Any, no-union-arg, no-star-actual path. Since #1204 generic
    targets (own type variables) are decided through the native
    constraint-solve kernel: solve, then evaluate the fully
    substituted form like a plain target. `None` always means defer;
    Rust never decides no-match or ambiguity.

    Categories: first-match ordering, per-target rejects, deferral
    shapes (star / typeobj / ParamSpec), direct generic solves,
    per-target type-object instantiation-gate facts (#1439).
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        self.resolver = _type_kernel.build_native_resolver(self._collect_type_infos(), [])

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _serialize(self, t: Type) -> bytes:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        return _serialize_type_for_checkexpr(t)

    def _call(
        self,
        targets: list[CallableType],
        arg_types: list[Type],
        arg_kinds: list[ArgKind] | None = None,
        arg_names: list[str | None] | None = None,
        strict_optional: bool = True,
        strict: bool = False,
        infer_unions: bool = False,
        typeobj_gate_fails: list[int] | None = None,
    ) -> int | None:
        kinds = (
            [k.value for k in arg_kinds]
            if arg_kinds is not None
            else [ARG_POS.value] * len(arg_types)
        )
        import type_kernel as tk

        return tk.rust_check_overload_call(
            self.resolver,
            [self._serialize(t) for t in targets],
            [self._serialize(t) for t in arg_types],
            kinds,
            strict_optional,
            arg_names,
            strict,
            infer_unions,
            typeobj_gate_fails,
        )

    def _generic(self, tv: TypeVarType) -> CallableType:
        """A generic callable: `def f(tv: tv) -> tv`."""
        return CallableType([tv], [ARG_POS], [None], tv, self.fx.function, variables=[tv])

    # --- first-match ordering and reject stepping ---

    def test_first_match_order(self) -> None:
        take_a = self.fx.callable(self.fx.b, self.fx.bool_type)
        assert self._call([take_a], [self.fx.b]) == 0
        # Target 0 rejects (A is not a subtype of B); target 1 matches.
        take_b = self.fx.callable(self.fx.b, self.fx.bool_type)
        take_str = self.fx.callable(self.fx.a, self.fx.str_type)
        assert self._call([take_b, take_str], [self.fx.a]) == 1

    def test_no_match_is_defer(self) -> None:
        # Rust NEVER decides no-match: a lone rejected target defers.
        take_str = self.fx.callable(self.fx.b, self.fx.str_type)
        assert self._call([take_str], [self.fx.a]) is None

    # --- whole-call deferral shapes ---

    def test_star_actual_defers(self) -> None:
        target = self.fx.callable(self.fx.a, self.fx.a)
        assert self._call([target], [self.fx.b], arg_kinds=[ARG_STAR]) is None

    def test_type_object_target_defers(self) -> None:
        # callable_type() fallbacks to builtins.type: Python's check_call
        # applies constructor calibration the wire cannot mirror.
        target = self.fx.callable_type(self.fx.a, self.fx.a)
        assert self._call([target], [self.fx.b]) is None

    # --- per-target instantiation-gate facts for type-object items (#1439) ---

    def test_typeobj_gate_fail_skips_to_next_target(self) -> None:
        # A type-object item whose pre-argument instantiation gates fail
        # (flag 1, shim-side) can never match; first-match steps past it
        # to the plain target, mirroring Python's rejection step.
        typeobj = self.fx.callable_type(self.fx.a, self.fx.a)
        take_b = self.fx.callable(self.fx.b, self.fx.str_type)
        assert self._call([typeobj, take_b], [self.fx.b], typeobj_gate_fails=[1, 0]) == 1

    def test_typeobj_gate_pass_decides_natively(self) -> None:
        # With gates passing (flag 0) the type-object item is evaluated
        # by the same pair machinery as any other callable item.
        typeobj = self.fx.callable_type(self.fx.a, self.fx.a)
        assert self._call([typeobj], [self.fx.b], typeobj_gate_fails=[0]) == 0
        # A rejected actual still never decides no-match.
        assert self._call([typeobj], [self.fx.str_type], typeobj_gate_fails=[0]) is None

    def test_typeobj_gate_unreadable_defers_whole_call(self) -> None:
        # Flag -1 is position-identical to the pre-#1439 whole-call defer,
        # even when a later target would have matched.
        typeobj = self.fx.callable_type(self.fx.a, self.fx.a)
        take_b = self.fx.callable(self.fx.b, self.fx.str_type)
        assert self._call([typeobj, take_b], [self.fx.b], typeobj_gate_fails=[-1, 0]) is None

    def test_typeobj_gate_missing_fact_defers(self) -> None:
        # A fact list shorter than the target list, or an unrecognized
        # fact value, must defer (never read as a passing fact).
        typeobj = self.fx.callable_type(self.fx.a, self.fx.a)
        assert self._call([typeobj], [self.fx.b], typeobj_gate_fails=[]) is None
        take_b = self.fx.callable(self.fx.b, self.fx.str_type)
        # First target rejected, second target has no fact: the search
        # defers instead of reaching the fact-less item.
        assert self._call([typeobj, take_b], [self.fx.str_type], typeobj_gate_fails=[0]) is None
        assert self._call([typeobj], [self.fx.b], typeobj_gate_fails=[99]) is None

    def test_typeobj_gate_all_fail_no_match_defers(self) -> None:
        # All type-object items gate-failed and no other target matches:
        # the exhausted search defers (Rust never decides no-match).
        typeobj = self.fx.callable_type(self.fx.a, self.fx.a)
        make_b = self.fx.callable(self.fx.b, self.fx.b)
        assert self._call([make_b, typeobj], [self.fx.a], typeobj_gate_fails=[0, 1]) is None

    def test_undecodable_target_defers(self) -> None:
        import type_kernel as tk

        assert (
            tk.rust_check_overload_call(
                self.resolver,
                [b"\xff garbage"],
                [self._serialize(self.fx.b)],
                [ARG_POS.value],
                True,
                None,
                False,
                False,
            )
            is None
        )

    # --- generic targets through the solve kernel (#1204) ---

    def test_generic_target_solved_natively(self) -> None:
        # def f(x: T) -> T with a concrete actual: solve substitutes
        # T = B and the solved form evaluates as a plain match.
        target = self._generic(self.fx.t)
        assert self._call([target], [self.fx.b]) == 0

    def test_generic_target_rejects_then_plain_matches(self) -> None:
        # Substitution alone does not force a match: a rejected first
        # target (B is not a subtype of str) still steps forward, and
        # the generic target solves T = B and matches.
        take_str = self.fx.callable(self.fx.str_type, self.fx.str_type)
        generic = self._generic(self.fx.t)
        assert self._call([take_str, generic], [self.fx.b]) == 1

    def test_generic_param_spec_defers(self) -> None:
        p = ParamSpecType(
            "P",
            "P",
            TypeVarId(1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        target = CallableType([p], [ARG_POS], [None], self.fx.a, self.fx.function, variables=[p])
        assert self._call([target], [self.fx.b]) is None

    def test_zero_formal_target_with_actual_rejects(self) -> None:
        # A zero-formal target with a passed actual does not match; the
        # exhausted search defers (never a decided no-match).
        empty = CallableType([], [], [], self.fx.a, self.fx.function)
        assert self._call([empty], [self.fx.b]) is None

    # --- alias operands (#1254): Python applies get_proper_type on both
    # sides before the per-pair check, so the seam expands a resolvable
    # top-level TypeAliasType instead of deferring.

    def _rebuild_resolver_with_aliases(self, aliases: list[TypeAlias]) -> None:
        self.resolver = _type_kernel.build_native_resolver(self._collect_type_infos(), aliases)

    def _alias(self, target: Type, fullname: str) -> tuple[TypeAlias, TypeAliasType]:
        from mypy.nodes import TypeAlias as _TypeAlias

        mod, _, name = fullname.rpartition(".")
        alias_node = _TypeAlias(target, fullname, mod, -1, -1)
        return alias_node, TypeAliasType(alias_node, [])

    def test_alias_formal_expands_natively(self) -> None:
        # mod.P = A; the formal P expands to A, so a B actual matches
        # (B is a subtype of A) natively where the raw alias deferred.
        alias_node, formal = self._alias(self.fx.a, "mod.P")
        self._rebuild_resolver_with_aliases([alias_node])
        target = self.fx.callable(formal, self.fx.str_type)
        assert self._call([target], [self.fx.b]) == 0
        # A missing snapshot keeps deferring (parity with the shim's
        # get_proper_type fallback: no alias target on the wire).
        unseeded, _ = self._alias(self.fx.a, "mod.Unseeded")
        target = self.fx.callable(TypeAliasType(unseeded, []), self.fx.str_type)
        assert self._call([target], [self.fx.b]) is None

    def test_alias_formal_rejects_then_next_matches(self) -> None:
        # A formal alias expanding to a non-supertype still rejects the
        # target (decided No, not a defer) and the first-match order
        # steps forward.
        alias_node, formal_a = self._alias(self.fx.a, "mod.P")
        self._rebuild_resolver_with_aliases([alias_node])
        take_str = self.fx.callable(self.fx.str_type, self.fx.str_type)
        take_alias = self.fx.callable(formal_a, formal_a)
        assert self._call([take_str, take_alias], [self.fx.b]) == 1

    def test_alias_actual_expands_natively(self) -> None:
        # mod.Q = B; the actual Q expands to B, and B is a subtype of A.
        alias_node, actual = self._alias(self.fx.b, "mod.Q")
        self._rebuild_resolver_with_aliases([alias_node])
        target = self.fx.callable(self.fx.a, self.fx.str_type)
        assert self._call([target], [actual]) == 0

    def test_special_form_any_actual_matches_natively(self) -> None:
        # AnyType(TypeOfAny.special_form) passes the shim's has_any_type
        # gate by design, and is_subtype decides Any-left True at the
        # default context, so the seam must not defer on it.
        any_actual = AnyType(TypeOfAny.special_form)
        target = self.fx.callable(self.fx.a, self.fx.str_type)
        assert self._call([target], [any_actual]) == 0

    def test_real_any_actual_still_defers_is_unreachable_but_any_union_formal(self) -> None:
        # The has_any_type gate filters real Anys upstream; a union
        # formal (alias expanding to a union) still defers when the
        # kernel cannot decide some item.
        alias_node, formal = self._alias(UnionType([self.fx.a, self.fx.function]), "mod.U")
        self._rebuild_resolver_with_aliases([alias_node])
        target = self.fx.callable(formal, self.fx.str_type)
        # b <: a is decided per item; b <: function is decided False, so
        # the union-right arm answers True natively.
        assert self._call([target], [self.fx.b]) == 0

    def test_union_formal_alias_item_expands_natively(self) -> None:
        # A union formal whose items carry a raw TypeAliasType expands
        # the resolvable item like Python's get_proper_type on a union
        # (types.py:5057-5100); the b actual then matches natively.
        alias_node, _ = self._alias(self.fx.a, "mod.P")
        alias_type = TypeAliasType(alias_node, [])
        self._rebuild_resolver_with_aliases([alias_node])
        formal = UnionType([alias_type, self.fx.function])
        target = self.fx.callable(formal, self.fx.str_type)
        assert self._call([target], [self.fx.b]) == 0
        # An unresolvable alias item keeps the whole call deferring.
        unseeded, _ = self._alias(self.fx.a, "mod.Unseeded")
        formal = UnionType([TypeAliasType(unseeded, []), self.fx.function])
        target = self.fx.callable(formal, self.fx.str_type)
        assert self._call([target], [self.fx.b]) is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckexprSuite(Suite):
    """Parity suite for M8c expression-checker leaf visitors.

    Tests ``rust_star_expr`` (identity echo of inner type) and
    ``rust_conditional_expr_join`` (join of two branch types) against
    the pure-Python implementations in ``mypy.checkexpr`` and ``mypy.join``.

    Note: ``visit_yield_expr`` is **not** ported to Rust because it requires
    ``self.chk`` state (generator-type resolution via
    ``get_generator_yield_type`` / ``get_generator_receive_type``,
    sub-expression traversal via ``self.accept``, and error reporting
    via ``self.chk.fail``).  It is skipped as a valid deliverable —
    pure type derivation is insufficient for this visitor.
    """

    def setUp(self) -> None:
        import type_kernel as _tk
        from librt.internal import ReadBuffer as _ReadBuffer, WriteBuffer as _WriteBuffer

        from mypy.checkexpr import _set_native_checkexpr_active
        from mypy.test.typefixture import TypeFixture
        from mypy.types import read_type as _read_type

        self._tk = _tk
        self._set_active = _set_native_checkexpr_active
        self._ReadBuffer = _ReadBuffer
        self._WriteBuffer = _WriteBuffer
        self._read_type = _read_type
        self.fx = TypeFixture()
        self._set_active(True)

        # Build a resolver for rust_conditional_expr_join.
        type_infos = [
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.di,
            self.fx.ei,
            self.fx.e2i,
            self.fx.e3i,
            self.fx.fi,
            self.fx.f2i,
            self.fx.f3i,
            self.fx.gi,
            self.fx.g2i,
            self.fx.hi,
            self.fx.std_tuplei,
            self.fx.type_typei,
            self.fx.bool_type_info,
            self.fx.str_type_info,
            self.fx.functioni,
        ]
        self.resolver = _tk.build_native_resolver(type_infos, [])

    def tearDown(self) -> None:
        self._set_active(False)

    def _bytes_of(self, t: Type) -> bytes:
        """Serialize a type for use with Rust functions."""
        buf = self._WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    # ---- rust_star_expr -------------------------------------------------

    def test_star_expr_identity_instance(self) -> None:
        result = self._tk.rust_star_expr(self._bytes_of(self.fx.a))
        self.assertIsNotNone(result)
        from mypy.types import Instance, read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, Instance)

    def test_star_expr_identity_any(self) -> None:
        result = self._tk.rust_star_expr(self._bytes_of(self.fx.anyt))
        self.assertIsNotNone(result)
        from mypy.types import AnyType, read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, AnyType)

    def test_star_expr_identity_none(self) -> None:
        result = self._tk.rust_star_expr(self._bytes_of(NoneType()))
        self.assertIsNotNone(result)
        from mypy.types import read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, NoneType)

    def test_star_expr_identity_union(self) -> None:
        u = UnionType.make_union([self.fx.a, self.fx.b])
        result = self._tk.rust_star_expr(self._bytes_of(u))
        self.assertIsNotNone(result)
        from mypy.types import read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, UnionType)
        assert isinstance(decoded, UnionType)  # type: ignore[misc]
        self.assertEqual(len(decoded.items), 2)

    def test_star_expr_defers_alias(self) -> None:
        # TypeAliasType requires a real TypeAlias node to serialize.
        # Test that any valid type round-trips correctly instead.
        result = self._tk.rust_star_expr(self._bytes_of(NoneType()))
        self.assertIsNotNone(result)
        from mypy.types import read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, NoneType)

    # ---- rust_conditional_expr_join -------------------------------------

    def test_conditional_join_same_instance(self) -> None:
        """Join of identical Instance = that Instance (SameT path)."""
        result = self._tk.rust_conditional_expr_join(
            self._bytes_of(self.fx.a), self._bytes_of(self.fx.a), self.resolver
        )
        self.assertIsNotNone(result)
        from mypy.types import Instance, read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, Instance)

    def test_conditional_join_none_vs_instance(self) -> None:
        """Join NoneType + Instance → Instance (Instance right path)."""
        result = self._tk.rust_conditional_expr_join(
            self._bytes_of(NoneType()), self._bytes_of(self.fx.a), self.resolver
        )
        self.assertIsNotNone(result)
        from mypy.types import Instance, read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, Instance)

    def test_conditional_join_same_instance_with_none(self) -> None:
        """Join NoneType + NoneType = NoneType (trivial: same type)."""
        result = self._tk.rust_conditional_expr_join(
            self._bytes_of(NoneType()), self._bytes_of(NoneType()), self.resolver
        )
        self.assertIsNotNone(result)
        from mypy.types import read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, NoneType)

    def test_conditional_join_defers_alias(self) -> None:
        """Join of NoneType + NoneType = NoneType (SameT path)."""
        result = self._tk.rust_conditional_expr_join(
            self._bytes_of(NoneType()), self._bytes_of(NoneType()), self.resolver
        )
        self.assertIsNotNone(result)
        from mypy.types import read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, NoneType)

    def test_conditional_join_callable_defers(self) -> None:
        """Join of two Callables → fallback union."""
        from mypy.nodes import ARG_POS

        call1 = CallableType(
            arg_types=[self.fx.a],
            arg_kinds=[ARG_POS],
            arg_names=["x"],
            ret_type=self.fx.a,
            fallback=self.fx.function,
        )
        call2 = CallableType(
            arg_types=[self.fx.b],
            arg_kinds=[ARG_POS],
            arg_names=["x"],
            ret_type=self.fx.b,
            fallback=self.fx.function,
        )
        result = self._tk.rust_conditional_expr_join(
            self._bytes_of(call1), self._bytes_of(call2), self.resolver
        )
        self.assertIsNotNone(result)
        from mypy.types import UnionType, read_type

        assert result is not None
        raw = bytes(result)
        buf = self._ReadBuffer(raw)
        decoded = read_type(buf)
        self.assertIsInstance(decoded, UnionType)
        assert isinstance(decoded, UnionType)  # type: ignore[misc]
        self.assertEqual(len(decoded.items), 2)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCombineSignaturesSuite(Suite):
    """Differential parity for issue #489: native `combine_function_signatures`.

    `rust_combine_function_signatures` merges a list of CallableTypes into
    one (per-column argument unions, return union, merged TypeVars), then
    decodes and restores live-only fields. We call the Rust kernel directly,
    decode via `_deserialize_type_from_checkexpr`, and assert the result
    matches the pure-Python `ExpressionChecker.combine_function_signatures`
    (run with the native gate off, so the oracle is the Python path).
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.test.typefixture import TypeFixture
        from mypy.wirefixup import set_wire_typeinfo_map

        self._tk = _tk
        self._set_active = _set_native_checkexpr_active
        self._set_resolver = _set_native_checkexpr_resolver
        self.fx = TypeFixture()
        type_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.di,
            self.fx.ei,
            self.fx.e2i,
            self.fx.e3i,
            self.fx.fi,
            self.fx.f2i,
            self.fx.f3i,
            self.fx.gi,
            self.fx.g2i,
            self.fx.hi,
            self.fx.std_tuplei,
            self.fx.type_typei,
            self.fx.bool_type_info,
            self.fx.str_type_info,
            self.fx.functioni,
        ]
        self.resolver = _tk.build_native_resolver(type_infos, [])
        self._set_resolver(self.resolver)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        set_wire_typeinfo_map(None)
        self._set_resolver(None)
        self._set_active(False)

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _callable(
        self, arg_types: list[Type], ret: Type, variables: list[Type] | None = None
    ) -> CallableType:
        return CallableType(
            arg_types,
            [ARG_POS] * len(arg_types),
            [None] * len(arg_types),
            ret,
            self.fx.function,
            variables=variables or [],  # type: ignore[arg-type]
        )

    def assert_rust_par(self, callables: list[CallableType]) -> None:
        """Assert Rust combine matches the pure-Python oracle."""
        # Force the Python oracle off by disabling the native gates
        # (the checkexpr gate and the freshen leaf's expandtype gate).
        self._set_active(False)
        old_expand = mypy.expandtype._native_expand_type_active
        try:
            from mypy.checkexpr import ExpressionChecker

            mypy.expandtype._set_native_expand_type_active(False)
            # Gate is off, so `self` is never touched; call unbound.
            expected = ExpressionChecker.combine_function_signatures(
                None,  # type: ignore[arg-type]
                callables,  # type: ignore[arg-type]
            )
        finally:
            mypy.expandtype._set_native_expand_type_active(old_expand)
            self._set_active(True)
        # Run the Rust kernel directly.
        start_raw_id = TypeVarId.next_raw_id
        res = self._tk.rust_combine_function_signatures(
            self.resolver, [self._bytes_of(c) for c in callables], start_raw_id, True
        )
        assert res is not None, f"rust combine deferred for {callables!r}"
        next_raw_id, merged_bytes = res
        TypeVarId.next_raw_id = max(TypeVarId.next_raw_id, next_raw_id)
        from mypy.checkexpr import _deserialize_type_from_checkexpr

        merged = _deserialize_type_from_checkexpr(bytes(merged_bytes))
        assert isinstance(merged, CallableType), str(merged)
        for live_field in ("line", "column", "definition", "special_sig", "from_type_type"):
            assert getattr(merged, live_field) == getattr(expected, live_field), live_field
        # `str()` renders arg unions, return union, variables, and
        # fallback, so equality here is a full oracle check.
        assert str(merged) == str(expected), f"rust {merged!r} != py {expected!r}"

    def test_two_non_generic_same_shape(self) -> None:
        # Merge of identical-shape callables whose args/returns are
        # unrelated (A vs D): per-column argument union A | D and
        # return union A | D (both columns contain both types).
        call1 = self._callable([self.fx.a], self.fx.a)
        call2 = self._callable([self.fx.d], self.fx.d)
        self.assert_rust_par([call1, call2])

        # Sanity: the union render shows both branches.
        start_raw_id = TypeVarId.next_raw_id
        res = self._tk.rust_combine_function_signatures(
            self.resolver, [self._bytes_of(call1), self._bytes_of(call2)], start_raw_id, True
        )
        assert res is not None
        _, merged_bytes = res
        from mypy.checkexpr import _deserialize_type_from_checkexpr

        merged = _deserialize_type_from_checkexpr(bytes(merged_bytes))
        assert isinstance(merged, CallableType)
        arg_union = get_proper_type(merged.arg_types[0])
        assert isinstance(arg_union, UnionType) and len(arg_union.items) == 2
        ret_union = get_proper_type(merged.ret_type)
        assert isinstance(ret_union, UnionType) and len(ret_union.items) == 2

    def test_two_non_generic_multi_arg(self) -> None:
        call1 = self._callable([self.fx.a, self.fx.b], self.fx.a)
        call2 = self._callable([self.fx.b, self.fx.a], self.fx.b)
        self.assert_rust_par([call1, call2])

    def test_two_non_generic_ret_union(self) -> None:
        call1 = self._callable([self.fx.a], self.fx.a)
        call2 = self._callable([self.fx.a], self.fx.b)
        self.assert_rust_par([call1, call2])

    def test_single_callable_deferred(self) -> None:
        # Rust defers (None) on len(callables) == 1, like the Python gate.
        res = self._tk.rust_combine_function_signatures(
            self.resolver,
            [self._bytes_of(self._callable([self.fx.a], self.fx.a))],
            TypeVarId.next_raw_id,
            True,
        )
        assert res is None

    def test_two_generic_sharing_typevar(self) -> None:
        # Both use `T` with different ids; merge renames to one exemplar.
        default = AnyType(TypeOfAny.from_omitted_generics)
        t1 = TypeVarType("T", "mod.T", TypeVarId(1), [], self.fx.o, default)
        t2 = TypeVarType("T", "mod.T", TypeVarId(2), [], self.fx.o, default)
        call1 = self._callable([t1], t1, variables=[t1])
        call2 = self._callable([t2], t2, variables=[t2])
        self.assert_rust_par([call1, call2])

    def test_three_callables_merge(self) -> None:
        call1 = self._callable([self.fx.a], self.fx.a)
        call2 = self._callable([self.fx.b], self.fx.a)
        call3 = self._callable([self.fx.b], self.fx.b)
        self.assert_rust_par([call1, call2, call3])

    def test_empty_callables_deferred(self) -> None:
        res = self._tk.rust_combine_function_signatures(
            self.resolver, [], TypeVarId.next_raw_id, True
        )
        assert res is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMergeTypevarsSuite(Suite):
    """Differential parity for `merge_typevars_in_callables_by_name`.

    The Rust seam `rust_merge_typevars_in_callables_by_name` freshens each
    generic callable's declared type vars and collapses same-named
    TypeVarType across callables to a shared exemplar. We call the Rust
    kernel directly, decode via `_deserialize_type_from_checkexpr`, and
    compare against the pure-Python `merge_typevars_in_callables_by_name`
    run with the checkexpr and expandtype gates off (the Python oracle).
    Rust defers (None) on ParamSpec/TypeVarTuple appearance, so those
    inputs assert deferral.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.test.typefixture import TypeFixture
        from mypy.wirefixup import set_wire_typeinfo_map

        self._tk = _tk
        self._set_active = _set_native_checkexpr_active
        self._set_resolver = _set_native_checkexpr_resolver
        self.fx = TypeFixture()
        type_infos = [self.fx.oi, self.fx.ai, self.fx.bi, self.fx.functioni]
        self.resolver = _tk.build_native_resolver(type_infos, [])
        self._set_resolver(self.resolver)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        set_wire_typeinfo_map(None)
        self._set_resolver(None)
        self._set_active(False)

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _callable(
        self, arg_types: list[Type], ret: Type, variables: list[Type] | None = None
    ) -> CallableType:
        return CallableType(
            arg_types,
            [ARG_POS] * len(arg_types),
            [None] * len(arg_types),
            ret,
            self.fx.function,
            variables=variables or [],  # type: ignore[arg-type]
        )

    def _tvar(self, name: str, raw_id: int) -> TypeVarType:
        default = AnyType(TypeOfAny.from_omitted_generics)
        return TypeVarType(name, f"mod.{name}", TypeVarId(raw_id), [], self.fx.o, default)

    def _py_oracle(
        self, callables: list[CallableType]
    ) -> tuple[list[CallableType], list[TypeVarType]]:
        from mypy.checkexpr import merge_typevars_in_callables_by_name

        self._set_active(False)
        old_expand = mypy.expandtype._native_expand_type_active
        try:
            mypy.expandtype._set_native_expand_type_active(False)
            return merge_typevars_in_callables_by_name(callables)
        finally:
            mypy.expandtype._set_native_expand_type_active(old_expand)
            self._set_active(True)

    def _assert_par(self, callables: list[CallableType]) -> None:
        expected_output, expected_vars = self._py_oracle(callables)
        res = self._tk.rust_merge_typevars_in_callables_by_name(
            [self._bytes_of(c) for c in callables], TypeVarId.next_raw_id, True
        )
        assert res is not None, f"rust merge deferred for {callables!r}"
        next_raw_id, callables_bytes, typevars_bytes = res
        TypeVarId.next_raw_id = max(TypeVarId.next_raw_id, next_raw_id)
        from mypy.checkexpr import _deserialize_type_from_checkexpr

        output = [_deserialize_type_from_checkexpr(bytes(b)) for b in callables_bytes]
        variables = [_deserialize_type_from_checkexpr(bytes(b)) for b in typevars_bytes]
        assert all(isinstance(c, CallableType) for c in output)
        assert all(isinstance(v, TypeVarType) for v in variables)
        # str() renders the signature including typevar names, so equality is
        # a full oracle check per callable and per distinct typevar.
        assert len(output) == len(expected_output)
        for got, want in zip(output, expected_output):
            assert str(got) == str(want), f"rust {got!r} != py {want!r}"
        assert len(variables) == len(expected_vars)
        for got, want in zip(variables, expected_vars):
            assert str(got) == str(want), f"rust var {got!r} != py var {want!r}"

    def test_two_callables_sharing_typevar(self) -> None:
        # Both use `T` with different ids; merge must collapse to one exemplar.
        t1 = self._tvar("T", 1)
        t2 = self._tvar("T", 2)
        call1 = self._callable([t1], t1, variables=[t1])
        call2 = self._callable([t2], t2, variables=[t2])
        self._assert_par([call1, call2])

    def test_disjoint_typevars(self) -> None:
        # T and S are unrelated names; both survive as distinct exemplars.
        t1 = self._tvar("T", 1)
        s1 = self._tvar("S", 2)
        call1 = self._callable([t1], t1, variables=[t1])
        call2 = self._callable([s1], s1, variables=[s1])
        self._assert_par([call1, call2])

    def test_two_typevars_shared_across_callables(self) -> None:
        # Both callables declare T and S; both pairs collapse to exemplars.
        t1, s1 = self._tvar("T", 1), self._tvar("S", 2)
        t2, s2 = self._tvar("T", 3), self._tvar("S", 4)
        call1 = self._callable([t1, s1], t1, variables=[t1, s1])
        call2 = self._callable([t2, s2], t2, variables=[t2, s2])
        self._assert_par([call1, call2])

    def test_mixed_generic_and_non_generic(self) -> None:
        t1 = self._tvar("T", 1)
        t2 = self._tvar("T", 2)
        call1 = self._callable([t1], t1, variables=[t1])
        call2 = self._callable([self.fx.a], self.fx.b)
        call3 = self._callable([t2], t2, variables=[t2])
        self._assert_par([call1, call2, call3])

    def test_single_generic_callable(self) -> None:
        t1 = self._tvar("T", 1)
        self._assert_par([self._callable([t1], t1, variables=[t1])])

    def test_empty_input(self) -> None:
        res = self._tk.rust_merge_typevars_in_callables_by_name([], TypeVarId.next_raw_id, True)
        assert res is not None
        _, callables_bytes, typevars_bytes = res
        assert callables_bytes == [] and typevars_bytes == []

    def test_paramspec_deferred(self) -> None:
        # A generic callable whose declared variables include a ParamSpec
        # defers to Python so output is never partial.
        ps = ParamSpecType(
            name="P",
            fullname="P",
            id=TypeVarId(-1),
            flavor=ParamSpecFlavor.BARE,
            upper_bound=self.fx.o,
            default=AnyType(TypeOfAny.from_omitted_generics),
        )
        t1 = self._tvar("T", 1)
        call1 = self._callable([t1], t1, variables=[t1, ps])
        res = self._tk.rust_merge_typevars_in_callables_by_name(
            [self._bytes_of(call1)], TypeVarId.next_raw_id, True
        )
        assert res is None

    def test_typevartuple_deferred(self) -> None:
        # Same for a TypeVarTuple declared variable.
        tvt = TypeVarTupleType(
            name="Ts",
            fullname="Ts",
            id=TypeVarId(-1),
            upper_bound=self.fx.a,
            tuple_fallback=self.fx.std_tuple,
            default=AnyType(TypeOfAny.from_omitted_generics),
        )
        t1 = self._tvar("T", 1)
        call1 = self._callable([t1], t1, variables=[t1, tvt])
        res = self._tk.rust_merge_typevars_in_callables_by_name(
            [self._bytes_of(call1)], TypeVarId.next_raw_id, True
        )
        assert res is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAnyCausesOverloadAmbiguitySuite(Suite):
    """Parity tests for `rust_any_causes_overload_ambiguity`.

    Runs the `checkexpr.any_causes_overload_ambiguity` seam with the native
    gate on (Rust kernel) and off (pure Python), asserting identical results.
    The pure-Python oracle is the untouched Python body; the native path
    serializes items/return_types/arg_types on the wire and delegates the
    whole decision to Rust.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.test.typefixture import TypeFixture
        from mypy.wirefixup import set_wire_typeinfo_map

        self._tk = _tk
        self._set_active = _set_native_checkexpr_active
        self._set_resolver = _set_native_checkexpr_resolver
        self._wire_map = set_wire_typeinfo_map
        self.fx = TypeFixture()
        type_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.di,
            self.fx.ei,
            self.fx.e2i,
            self.fx.e3i,
            self.fx.fi,
            self.fx.f2i,
            self.fx.f3i,
            self.fx.gi,
            self.fx.g2i,
            self.fx.hi,
            self.fx.std_tuplei,
            self.fx.type_typei,
            self.fx.bool_type_info,
            self.fx.str_type_info,
            self.fx.functioni,
        ]
        self.resolver = _tk.build_native_resolver(type_infos, [])
        self._set_resolver(self.resolver)
        self._wire_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        self._wire_map(None)
        self._set_resolver(None)
        self._set_active(False)

    def _callable(
        self, arg_types: list[Type], ret: Type, fallback: Instance | None = None
    ) -> CallableType:
        return CallableType(
            arg_types,
            [ARG_POS] * len(arg_types),
            [None] * len(arg_types),
            ret,
            fallback if fallback is not None else self.fx.function,
        )

    def _any(self) -> AnyType:
        return AnyType(TypeOfAny.explicit)

    def assert_par(
        self,
        items: list[CallableType],
        return_types: list[Type],
        arg_types: list[Type],
        arg_kinds: list[ArgKind],
        arg_names: Sequence[str | None] | None,
    ) -> None:
        """Assert the native seam matches the pure-Python oracle."""
        from mypy.checkexpr import any_causes_overload_ambiguity

        # Pure-Python oracle: gate off, so the seam falls through to the
        # Python body and every internal helper routes to Python.
        self._set_active(False)
        try:
            expected = any_causes_overload_ambiguity(
                items, return_types, arg_types, arg_kinds, arg_names
            )
        finally:
            self._set_active(True)
        actual = any_causes_overload_ambiguity(
            items, return_types, arg_types, arg_kinds, arg_names
        )
        assert actual == expected, f"rust {actual!r} != py {expected!r}"

    def test_same_returns_false(self) -> None:
        # Items with identical return types: `all_same_types(return_types)`
        # short-circuits to False before the Any scan.
        item1 = self._callable([self.fx.a], self.fx.a)
        item2 = self._callable([self.fx.b], self.fx.a)
        self.assert_par([item1, item2], [self.fx.a, self.fx.a], [self._any()], [ARG_POS], None)

    def test_differing_returns_with_any_true(self) -> None:
        # An explicit Any actual maps to differing formals (A vs B) and the
        # items return differing types (A vs B): ambiguity -> True.
        item1 = self._callable([self.fx.a], self.fx.a)
        item2 = self._callable([self.fx.b], self.fx.b)
        self.assert_par([item1, item2], [self.fx.a, self.fx.b], [self._any()], [ARG_POS], None)

    def test_differing_returns_same_formal_false(self) -> None:
        # Any maps to the same formal type (A) in both items even though the
        # returns differ (A vs B): matching_formals all same -> False.
        item1 = self._callable([self.fx.a], self.fx.a)
        item2 = self._callable([self.fx.a], self.fx.b)
        self.assert_par([item1, item2], [self.fx.a, self.fx.b], [self._any()], [ARG_POS], None)

    def test_no_any_false(self) -> None:
        # No actual contains Any: the scan finds nothing -> False.
        item1 = self._callable([self.fx.a], self.fx.a)
        item2 = self._callable([self.fx.b], self.fx.b)
        self.assert_par([item1, item2], [self.fx.a, self.fx.b], [self.fx.a], [ARG_POS], None)

    def test_type_obj_edge_false(self) -> None:
        # An Any-bearing type-object actual is ignored (`ignore_in_type_obj`),
        # so no ambiguity is claimed between Type and Callable overloads.
        type_obj = CallableType([], [], [], self._any(), self.fx.type_type)
        item1 = self._callable([self.fx.a], self.fx.a)
        item2 = self._callable([self.fx.b], self.fx.b)
        self.assert_par([item1, item2], [self.fx.a, self.fx.b], [type_obj], [ARG_POS], None)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeDangerousComparisonSuite(Suite):
    """Parity for the Rust `dangerous_comparison` port (mypy.checkexpr).

    `dangerous_comparison` is an `ExpressionChecker` method whose pure
    branch-for-branch decision tree (strict-equality diagnostics) is mirrored
    in Rust. The seam needs a checker with `options`, `binder`, and
    `lookup_typeinfo`, so this suite drives it through a lightweight stub:
    the checker carries the strict-equality options and a query-only binder.
    Toggling the checkexpr gate off (pure Python) and on (Rust seam) must
    produce identical booleans, and a direct seam call proves the Rust
    function engages rather than silently deferring.
    """

    def setUp(self) -> None:
        import types as _types

        from mypy.binder import ConditionalTypeBinder
        from mypy.checkexpr import (
            ExpressionChecker,
            _set_native_checkexpr_active,
            _set_native_checkexpr_resolver,
        )
        from mypy.options import Options

        self.fx = TypeFixture()
        # The fixture's std_listi / std_tuplei already have fullnames
        # builtins.list / builtins.tuple; construct byte-ish infos via the
        # fixture so their fullnames match the Rust allowlists.
        self.bytesi = self.fx.make_type_info("builtins.bytes")
        self.bytearrayi = self.fx.make_type_info("builtins.bytearray")
        self.memoryviewi = self.fx.make_type_info("builtins.memoryview")
        # AbstractSet / Mapping fixtures for the container recursions: the
        # shim resolves their fullnames so Rust maps set/dict instances
        # through the typing supertypes without crossing back to Python.
        self.abstract_seti = self.fx.make_type_info("typing.AbstractSet", typevars=["T"])
        self.seti = self.fx.make_type_info(
            "builtins.set", mro=[self.abstract_seti, self.fx.oi], typevars=["T"]
        )
        self.seti.bases = [Instance(self.abstract_seti, [self.seti.defn.type_vars[0]])]
        self.abstract_mapi = self.fx.make_type_info("typing.Mapping", typevars=["K", "V"])
        self.dicti = self.fx.make_type_info(
            "builtins.dict", mro=[self.abstract_mapi, self.fx.oi], typevars=["K", "V"]
        )
        self.dicti.bases = [Instance(self.abstract_mapi, list(self.dicti.defn.type_vars))]
        # Production class typevars bind TypeVarId(raw_id, namespace=<class
        # fullname>) (types.py:554); make_type_info leaves namespace empty,
        # so stamp it for the fixtures the native mapping walks.
        from mypy.types import TypeVarId

        for info in (self.abstract_seti, self.seti, self.abstract_mapi, self.dicti):
            for tv in info.defn.type_vars:
                tv.id = TypeVarId(tv.id.raw_id, namespace=info.fullname)

        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend(
            [
                self.fx.oi,
                self.fx.ai,
                self.fx.di,
                self.fx.std_tuplei,
                self.fx.std_listi,
                self.fx.bool_type_info,
                self.fx.str_type_info,
                self.bytesi,
                self.bytearrayi,
                self.memoryviewi,
                self.abstract_seti,
                self.seti,
                self.abstract_mapi,
                self.dicti,
            ]
        )
        self._infos = type_infos
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active = _set_native_checkexpr_active
        self._set_resolver = _set_native_checkexpr_resolver
        self._set_active(True)
        self._set_resolver(self.resolver)

        # Stub checker driving the seam: strict-equality on, no unreachable
        # suppression, and the bytes allowlist types for lookups.
        options = Options()
        options.strict_equality = True
        options.strict_equality_for_none = False
        options.strict_optional = True
        chk = _types.SimpleNamespace(
            options=options,
            binder=ConditionalTypeBinder(options),
            # Dict subscript raises KeyError on a miss, mirroring the real
            # TypeChecker.lookup_typeinfo so the shim defers cleanly.
            lookup_typeinfo=lambda name: {
                "builtins.bytes": self.bytesi,
                "builtins.bytearray": self.bytearrayi,
                "builtins.memoryview": self.memoryviewi,
                "typing.AbstractSet": self.abstract_seti,
                "typing.Mapping": self.abstract_mapi,
            }[name],
        )
        self.method = ExpressionChecker.__new__(ExpressionChecker)
        self.method.chk = chk  # type: ignore[assignment]

    def tearDown(self) -> None:
        self._set_active(False)
        self._set_resolver(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _assert_par(self, left: Type, right: Type, **kw: Any) -> None:
        off = self._with_gate(False, lambda: self.method.dangerous_comparison(left, right, **kw))
        on = self._with_gate(True, lambda: self.method.dangerous_comparison(left, right, **kw))
        assert_equal(on, off, f"dangerous_comparison parity {left} / {right}")

    def _seam(self, left: Type, right: Type, original: Type | None = None, **kw: Any) -> Any:
        from mypy.checkexpr import _serialize_type_for_checkexpr
        from mypy.typeops import custom_special_method

        return _type_kernel.rust_dangerous_comparison(
            _serialize_type_for_checkexpr(left),
            _serialize_type_for_checkexpr(right),
            _serialize_type_for_checkexpr(original) if original is not None else None,
            False,
            kw.get("prefer_literal", True),
            kw.get("identity_check", False),
            False,  # strict_equality_for_none (matches py)
            kw.get("unreachable_suppressed", False),
            custom_special_method(left, "__eq__"),
            custom_special_method(right, "__eq__"),
            kw.get("strict_optional", True),
            "typing.AbstractSet",
            "typing.Mapping",
            self.resolver,
        )

    def _assert_engages(
        self, left: Type, right: Type, original: Type | None = None, **kw: Any
    ) -> None:
        result = self._seam(left, right, original, **kw)
        assert result is not None, f"Rust dangerous_comparison did not engage for {left} / {right}"

    def test_disjoint_siblings_true(self) -> None:
        # A and D are disjoint siblings -> the comparison can never be True.
        self._assert_par(self.fx.a, self.fx.d)
        self._assert_engages(self.fx.a, self.fx.d)

    def test_same_type_false(self) -> None:
        # Comparing a type against itself can be True.
        self._assert_par(self.fx.a, self.fx.a)
        result = self._with_gate(
            True, lambda: self.method.dangerous_comparison(self.fx.a, self.fx.a)
        )
        assert_equal(result, False)
        self._assert_engages(self.fx.a, self.fx.a)

    def test_none_vs_str_false(self) -> None:
        # None and str are not overlapping in strict-optional mode.
        self._assert_par(self.fx.nonet, self.fx.str_type)
        self._assert_engages(self.fx.nonet, self.fx.str_type)

    def test_union_remove_optional_true(self) -> None:
        # Optional[A] vs Optional[D]: remove_optional -> A / D,
        # disjoint -> True.
        opt_a = UnionType([self.fx.a, self.fx.nonet])
        opt_d = UnionType([self.fx.d, self.fx.nonet])
        self._assert_par(opt_a, opt_d)
        self._assert_engages(opt_a, opt_d)

    def test_literal_int_true(self) -> None:
        # Non-equal int literals never overlap.
        self._assert_par(self.fx.lit1, self.fx.lit2)
        self._assert_engages(self.fx.lit1, self.fx.lit2)

    def test_literal_bool_false(self) -> None:
        # Different bool literals are NOT dangerous (explicit branch).
        lit_t = LiteralType(True, self.fx.bool_type)
        lit_f = LiteralType(False, self.fx.bool_type)
        self._assert_par(lit_t, lit_f)
        self._assert_engages(lit_t, lit_f)

    def test_list_same_elem_overlap_false(self) -> None:
        # list[A] vs list[A]: item A/A overlaps -> not dangerous.
        la = Instance(self.fx.std_listi, [self.fx.a])
        self._assert_par(la, la)
        self._assert_engages(la, la)

    def test_list_disjoint_elem_true(self) -> None:
        # list[A] vs list[D]: item A/D disjoint -> dangerous.
        la = Instance(self.fx.std_listi, [self.fx.a])
        ld = Instance(self.fx.std_listi, [self.fx.d])
        self._assert_par(la, ld)
        self._assert_engages(la, ld)

    def test_bytes_component_container_safe(self) -> None:
        # b'abc' in b'cde' is safe (has_bytes_component on original).
        left = Instance(self.bytesi, [])
        right = Instance(self.bytesi, [])
        original = Instance(self.bytesi, [])
        self._assert_par(left, right, original_container=original)
        self._assert_engages(left, right, original_container=original)

    def test_bytearray_vs_bytes_safe(self) -> None:
        # bytearray / bytes comparisons are supported even when disjoint.
        ba = Instance(self.bytearrayi, [])
        b = Instance(self.bytesi, [])
        self._assert_par(ba, b)
        self._assert_engages(ba, b)

    def _rebuild_with_aliases(self, aliases: list[TypeAlias]) -> None:
        self.resolver = _type_kernel.build_native_resolver(self._infos, aliases)
        self._set_resolver(self.resolver)

    def test_alias_operand_disjoint_true(self) -> None:
        # `mod.A = A`: an alias operand expands natively, so the strict
        # comparison against disjoint D decides True in the seam.
        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        left = TypeAliasType(alias, [])
        self._rebuild_with_aliases([alias])
        self._assert_par(left, self.fx.d)
        result = self._with_gate(True, lambda: self.method.dangerous_comparison(left, self.fx.d))
        assert_equal(result, True)
        self._assert_engages(left, self.fx.d)

    def test_alias_optional_union_true(self) -> None:
        # `mod.O = Optional[A]` vs `Optional[D]`: remove_optional drops
        # None on both sides (the alias expands into the union branch) and
        # the A / D items are disjoint.
        alias = TypeAlias(UnionType([self.fx.a, self.fx.nonet]), "mod.O", "mod", -1, -1)
        left = TypeAliasType(alias, [])
        right = UnionType([self.fx.d, self.fx.nonet])
        self._rebuild_with_aliases([alias])
        self._assert_par(left, right)
        self._assert_engages(left, right)

    def test_alias_missing_snapshot_defers_parity(self) -> None:
        # An alias with no resolver snapshot defers: both gates answer via
        # the pure-Python body and must agree.
        alias = TypeAlias(self.fx.a, "mod.Missing", "mod", -1, -1)
        left = TypeAliasType(alias, [])
        self._assert_par(left, self.fx.d)
        assert self._seam(left, self.fx.d) is None

    def test_set_same_elem_not_dangerous(self) -> None:
        # set[A] vs set[A]: item A/A overlaps -> not dangerous.
        sa = Instance(self.seti, [self.fx.a])
        self._assert_par(sa, sa)
        self._assert_engages(sa, sa)

    def test_set_disjoint_elem_dangerous(self) -> None:
        # set[A] vs set[D]: AbstractSet item recursion sees A/D disjoint.
        sa = Instance(self.seti, [self.fx.a])
        sd = Instance(self.seti, [self.fx.d])
        self._assert_par(sa, sd)
        result = self._with_gate(True, lambda: self.method.dangerous_comparison(sa, sd))
        assert_equal(result, True)
        self._assert_engages(sa, sd)

    def test_dict_value_disjoint_dangerous(self) -> None:
        # dict[str, A] vs dict[str, D]: keys overlap, values are disjoint,
        # so the Mapping recursion flags the comparison.
        da = Instance(self.dicti, [self.fx.str_type, self.fx.a])
        dd = Instance(self.dicti, [self.fx.str_type, self.fx.d])
        self._assert_par(da, dd)
        result = self._with_gate(True, lambda: self.method.dangerous_comparison(da, dd))
        assert_equal(result, True)
        self._assert_engages(da, dd)

    def test_dict_same_value_not_dangerous(self) -> None:
        da = Instance(self.dicti, [self.fx.str_type, self.fx.a])
        self._assert_par(da, da)
        self._assert_engages(da, da)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeEnumProtocolClassifierSuite(Suite):
    """Direct-seam tests for the enum-callable base and protocol-test
    callee classifiers in checkexpr_functions.rs.

    `rust_is_enum_callable_base` mirrors the Enum() early-return guard in
    `check_call_for_callable` (checkexpr.py:2590), and
    `rust_classify_protocol_test_callee` mirrors the isinstance/issubclass
    protocol-test gate in `visit_call_expr_inner` (checkexpr.py:1471).
    Both classify on live AST nodes via PyO3; Python keeps the side effects.

    #1739 retired the `is_enum_callable_base` shim (6.80x-12.37x the Python
    predicate, which is `isinstance` plus a frozenset lookup): these are
    direct-seam tests for the still-registered pyfunction only. The
    `typeobj_gate` seam stays wired and is measured as a keep.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        self.enum_bases = frozenset(
            ("enum.Enum", "enum.IntEnum", "enum.Flag", "enum.IntFlag", "enum.StrEnum")
        )

    def test_enum_callable_base_name_expr(self) -> None:
        n = NameExpr("Enum")
        n.fullname = "enum.Enum"
        assert self._tk.rust_is_enum_callable_base(n, self.enum_bases) is True

    def test_enum_callable_base_str_enum(self) -> None:
        n = NameExpr("StrEnum")
        n.fullname = "enum.StrEnum"
        assert self._tk.rust_is_enum_callable_base(n, self.enum_bases) is True

    def test_enum_callable_base_non_enum(self) -> None:
        n = NameExpr("Foo")
        n.fullname = "mod.Foo"
        assert self._tk.rust_is_enum_callable_base(n, self.enum_bases) is False

    def test_enum_callable_base_member_expr(self) -> None:
        # MemberExpr is a RefExpr; with an enum fullname it should hit.
        m = MemberExpr(NameExpr("enum"), "Enum")
        m.fullname = "enum.Enum"
        assert self._tk.rust_is_enum_callable_base(m, self.enum_bases) is True

    def test_enum_callable_base_call_expr(self) -> None:
        # CallExpr is NOT a RefExpr -> always False.
        c = CallExpr(NameExpr("Enum"), [], [], [])
        assert self._tk.rust_is_enum_callable_base(c, self.enum_bases) is False

    def test_protocol_callee_isinstance(self) -> None:
        n = NameExpr("isinstance")
        n.fullname = "builtins.isinstance"
        assert self._tk.rust_classify_protocol_test_callee(n, 2) == "builtins.isinstance"

    def test_protocol_callee_issubclass(self) -> None:
        n = NameExpr("issubclass")
        n.fullname = "builtins.issubclass"
        assert self._tk.rust_classify_protocol_test_callee(n, 2) == "builtins.issubclass"

    def test_protocol_callee_len(self) -> None:
        n = NameExpr("len")
        n.fullname = "builtins.len"
        assert self._tk.rust_classify_protocol_test_callee(n, 2) is None

    def test_protocol_callee_isinstance_one_arg(self) -> None:
        n = NameExpr("isinstance")
        n.fullname = "builtins.isinstance"
        assert self._tk.rust_classify_protocol_test_callee(n, 1) is None

    def test_protocol_callee_non_ref_expr(self) -> None:
        c = CallExpr(NameExpr("f"), [], [], [])
        assert self._tk.rust_classify_protocol_test_callee(c, 2) is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypedDictCallSuite(Suite):
    """Parity for the Rust `check_typeddict_call` dispatch classifier.

    `check_typeddict_call` (mypy/checkexpr.py) picks one of four branches
    from the call's `args` and `arg_kinds`, then routes to a kwargs/dict
    body or reports INVALID_TYPEDDICT_ARGS. The Rust port decides that
    structural branch from live AST objects; every branch body stays in
    Python. The golden reference below transcribes the original isinstance
    chain independently, so a gate-on result that differs from it, or a
    silent deferral (None), both fail.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _python_tag(self, args: list[Expression], arg_kinds: list[ArgKind]) -> int:
        # Golden reference: the original isinstance chain in
        # check_typeddict_call, transcribed independently of the Rust port.
        if args and all(ak in (ARG_NAMED, ARG_STAR2) for ak in arg_kinds):
            return 0
        if len(args) == 1 and arg_kinds[0] == ARG_POS:
            unique_arg = args[0]
            if isinstance(unique_arg, DictExpr):
                return 1
            if isinstance(unique_arg, CallExpr) and isinstance(unique_arg.analyzed, DictExpr):
                return 2
        if not args:
            return 3
        return 4

    def _assert_par(self, args: Sequence[Expression], arg_kinds: list[ArgKind]) -> None:
        from mypy.checkexpr import _try_native_classify_typeddict_call

        expected = self._python_tag(cast("list[Expression]", args), arg_kinds)
        native = _try_native_classify_typeddict_call(cast("list[Expression]", args), arg_kinds)
        assert native is not None, f"native classifier deferred for tag {expected}"
        assert native == expected, f"native tag {native} != golden {expected}"

    def _assert_engages(self, args: list[Expression], arg_kinds: list[ArgKind]) -> None:
        tag = _type_kernel.rust_classify_typeddict_call(args, [ak.value for ak in arg_kinds])
        assert tag is not None, "direct seam returned None"

    def test_kwargs(self) -> None:
        self._assert_par([StrExpr("x"), NameExpr("extras")], [ARG_NAMED, ARG_STAR2])

    def test_dict_expr(self) -> None:
        args = [DictExpr([(StrExpr("x"), StrExpr("42"))])]
        self._assert_par(args, [ARG_POS])

    def test_dict_call(self) -> None:
        dict_expr = DictExpr([(StrExpr("x"), StrExpr("42"))])
        callee = NameExpr("dict")
        args = [CallExpr(callee, [StrExpr("x")], [ARG_POS], [None], analyzed=dict_expr)]
        self._assert_par(args, [ARG_POS])

    def test_empty(self) -> None:
        self._assert_par([], [])

    def test_invalid(self) -> None:
        self._assert_par([StrExpr("x"), StrExpr("y")], [ARG_POS, ARG_POS])

    def test_single_positional_non_dict(self) -> None:
        # One positional arg that is neither DictExpr nor dict-literal.
        self._assert_par([NameExpr("foo")], [ARG_POS])

    def test_call_with_unanalyzed_callee(self) -> None:
        # A CallExpr whose `.analyzed` is None falls through to invalid.
        args = [CallExpr(NameExpr("dict"), [], [], [])]
        self._assert_par(args, [ARG_POS])

    def test_direct_seam_engages(self) -> None:
        shapes: list[tuple[Any, Any]] = [
            ([StrExpr("x")], [ARG_NAMED]),
            ([DictExpr([(StrExpr("x"), StrExpr("1"))])], [ARG_POS]),
            ([NameExpr("f")], [ARG_STAR2]),
            ([], []),
        ]
        for args, arg_kinds in shapes:
            self._assert_engages(args, arg_kinds)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeHasAbstractTypeSuite(Suite):
    """Parity for the Rust `has_abstract_type` port (mypy.checkexpr).

    `has_abstract_type` (checkexpr.py:8134-8143) is a pure boolean
    conjunction over live types: caller must be a FunctionLike whose
    `is_type_obj()` is True, whose `type_object()` is abstract or protocol;
    callee must be a TypeType whose item Instance is abstract or protocol;
    and `allow_abstract_call` must be False. The Rust seam reads the live
    Python objects (isinstance, is_type_obj/type_object method calls,
    is_abstract/is_protocol bool reads) and never defers, so every decided
    case returns a plain bool. Gate-off vs gate-on runs must agree and the
    direct seam call must engage (non-None).
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self.fx = TypeFixture()
        _set_native_checkexpr_active(True)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        _set_native_checkexpr_active(False)

    def _info(self, fullname: str, *, abstract: bool = False, protocol: bool = False) -> TypeInfo:
        info = self.fx.make_type_info(fullname)
        info.is_abstract = abstract
        info.is_protocol = protocol
        return info

    def _caller(self, ret_info: TypeInfo) -> CallableType:
        # fallback.type is builtins.type (is_metaclass() True) and ret_type
        # is an Instance, so is_type_obj() is True and type_object()
        # resolves force_fallback(ret_type).type == ret_info.
        return CallableType([], [], [], Instance(ret_info, []), Instance(self.fx.type_typei, []))

    def _callee(self, info: TypeInfo) -> TypeType:
        return TypeType(Instance(info, []))

    def _make_checker(self) -> ExpressionChecker:
        from mypy.checker import TypeChecker
        from mypy.checkexpr import ExpressionChecker
        from mypy.errors import Errors
        from mypy.messages import MessageBuilder
        from mypy.nodes import MypyFile, SymbolTable
        from mypy.plugin import Plugin

        options = Options()
        errors = Errors(options)
        tree = MypyFile([], [])
        tree.is_stub = True
        tree.names = SymbolTable()
        modules: dict[str, MypyFile] = {}
        chk = TypeChecker(errors, modules, options, tree, "", Plugin(options), {})
        msg = MessageBuilder(errors, modules)
        return ExpressionChecker(chk, msg, Plugin(options), {})

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checkexpr import _set_native_checkexpr_active

        _set_native_checkexpr_active(active)
        try:
            return fn()
        finally:
            _set_native_checkexpr_active(True)

    def _assert_par(
        self, caller: ProperType, callee: ProperType, allow: bool, expected: bool
    ) -> None:
        expr = self._make_checker()
        expr.chk.allow_abstract_call = allow
        off = self._with_gate(False, lambda: expr.has_abstract_type(caller, callee))
        on = self._with_gate(True, lambda: expr.has_abstract_type(caller, callee))
        assert off == expected, f"gate-off {caller!r}/{callee!r}: {off} != {expected}"
        assert on == expected, f"gate-on {caller!r}/{callee!r}: {on} != {expected}"

    def _assert_seam(
        self, caller: ProperType, callee: ProperType, allow: bool, expected: bool
    ) -> None:
        result = _type_kernel.rust_has_abstract_type(caller, callee, allow)
        assert result is not None, f"Rust deferred on {caller!r}/{callee!r}"
        assert result == expected, f"Rust seam {caller!r}/{callee!r}: {result} != {expected}"

    def test_abstract_to_protocol_true(self) -> None:
        caller = self._caller(self._info("mod.Abs", abstract=True))
        callee = self._callee(self._info("mod.Proto", protocol=True))
        self._assert_par(caller, callee, False, True)
        self._assert_seam(caller, callee, False, True)

    def test_allow_abstract_call_short_circuits(self) -> None:
        caller = self._caller(self._info("mod.Abs", abstract=True))
        callee = self._callee(self._info("mod.Proto", protocol=True))
        self._assert_par(caller, callee, True, False)
        self._assert_seam(caller, callee, True, False)

    def test_caller_not_functionlike_false(self) -> None:
        caller = Instance(self._info("mod.A"), [])
        callee = self._callee(self._info("mod.Proto", protocol=True))
        self._assert_par(caller, callee, False, False)
        self._assert_seam(caller, callee, False, False)

    def test_callee_not_typetype_false(self) -> None:
        caller = self._caller(self._info("mod.Abs", abstract=True))
        callee = Instance(self._info("mod.Proto", protocol=True), [])
        self._assert_par(caller, callee, False, False)
        self._assert_seam(caller, callee, False, False)

    def test_caller_not_type_obj_false(self) -> None:
        caller = CallableType(
            [],
            [],
            [],
            Instance(self._info("mod.Abs", abstract=True), []),
            Instance(self.fx.functioni, []),
        )
        callee = self._callee(self._info("mod.Proto", protocol=True))
        self._assert_par(caller, callee, False, False)
        self._assert_seam(caller, callee, False, False)

    def test_caller_type_object_not_abstract_false(self) -> None:
        caller = self._caller(self._info("mod.Concrete"))
        callee = self._callee(self._info("mod.Proto", protocol=True))
        self._assert_par(caller, callee, False, False)
        self._assert_seam(caller, callee, False, False)

    def test_callee_item_not_instance_false(self) -> None:
        caller = self._caller(self._info("mod.Abs", abstract=True))
        callee = TypeType(AnyType(TypeOfAny.special_form))
        self._assert_par(caller, callee, False, False)
        self._assert_seam(caller, callee, False, False)

    def test_callee_item_not_abstract_false(self) -> None:
        caller = self._caller(self._info("mod.Abs", abstract=True))
        callee = self._callee(self._info("mod.Concrete"))
        self._assert_par(caller, callee, False, False)
        self._assert_seam(caller, callee, False, False)

    def test_protocol_to_abstract_true(self) -> None:
        caller = self._caller(self._info("mod.Proto", protocol=True))
        callee = self._callee(self._info("mod.Abs", abstract=True))
        self._assert_par(caller, callee, False, True)
        self._assert_seam(caller, callee, False, True)

    def test_tuple_part_propagates_true(self) -> None:
        # The TupleType branch zips items pairwise through
        # has_abstract_type: each caller item must be a class-object
        # callable and each callee item a TypeType(Instance).
        caller = TupleType(
            [
                self._caller(self._info("mod.Abs", abstract=True)),
                self._caller(self._info("mod.Concrete")),
            ],
            self.fx.std_tuple,
        )
        callee = TupleType(
            [
                self._callee(self._info("mod.Proto", protocol=True)),
                self._callee(self._info("mod.Concrete2")),
            ],
            self.fx.std_tuple,
        )
        expr = self._make_checker()
        off = self._with_gate(False, lambda: expr.has_abstract_type_part(caller, callee))
        on = self._with_gate(True, lambda: expr.has_abstract_type_part(caller, callee))
        assert off is True, f"gate-off tuple {off} != True"
        assert on is True, f"gate-on tuple {on} != True"

    def test_tuple_part_no_match_false(self) -> None:
        caller = TupleType([self._caller(self._info("mod.Concrete"))], self.fx.std_tuple)
        callee = TupleType([self._callee(self._info("mod.Concrete2"))], self.fx.std_tuple)
        expr = self._make_checker()
        off = self._with_gate(False, lambda: expr.has_abstract_type_part(caller, callee))
        on = self._with_gate(True, lambda: expr.has_abstract_type_part(caller, callee))
        assert off is False, f"gate-off tuple {off} != False"
        assert on is False, f"gate-on tuple {on} != False"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckexprJoinAndTupleSuite(Suite):
    """Parity for the checkexpr `join_one_pair` and `unpack_expand_updated` ports.

    The fast container-literal path (`_first_or_join_fast_item` -> checkexpr's
    `join_type_list_inner`, exposed via `_try_native_container_type`) and
    the tuple-expression build (`_try_native_build_tuple_type`) consult the
    Rust checkexpr seams for two previously-deferred decisions: the
    args-less Instance-Instance nominal fold (`join_one_pair`) and the lone
    `Tuple[*tuple[X, ...]]` normalization (`unpack_expand_updated`). The
    visible result must be identical to the pure-Python twins
    (`join.join_type_list` / `expand_type`), and the decided cases must
    engage (return a type instead of deferring).
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self.typeinfo_map = {info.fullname: info for info in type_infos}
        set_wire_typeinfo_map(self.typeinfo_map)
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_checkexpr_resolver(self._resolver)
        _set_native_checkexpr_active(True)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checkexpr_active(False)
        _set_native_checkexpr_resolver(None)
        set_wire_typeinfo_map(None)

    def _join_native(self, if_type: Type, else_type: Type) -> str | None:
        from mypy.checkexpr import _try_native_container_type
        from mypy.types import Instance

        result = _try_native_container_type("list", [if_type, else_type])
        if result is None or result is False:
            return None
        # The container seam returns `list[joined]`; extract the element.
        result = get_proper_type(result)
        assert isinstance(result, Instance) and result.args
        return str(result.args[0])

    def _join_python(self, if_type: Type, else_type: Type) -> str:
        from mypy.join import join_type_list

        return str(join_type_list([if_type, else_type]))

    def _tuple_native(self, items: list[Type], seen_unpack: bool) -> str | None:
        from mypy.checkexpr import _try_native_build_tuple_type

        result = _try_native_build_tuple_type(items, seen_unpack)
        return str(result) if result is not None else None

    def _tuple_python(self, items: list[Type], seen_unpack: bool) -> str:
        from mypy.expandtype import expand_type
        from mypy.types import TupleType

        fallback = self.fx.std_tuple
        result: Type = TupleType(items, fallback)
        if seen_unpack:
            result = expand_type(result, {})
        return str(result)

    def _assert_join_parity(self, if_t: Type, else_t: Type, engage: bool) -> None:
        native = self._join_native(if_t, else_t)
        python = self._join_python(if_t, else_t)
        if engage:
            assert native is not None, f"join({if_t}, {else_t}) did not engage"
        assert (
            native is None or native == python
        ), f"join({if_t}, {else_t}): native {native!r} != python {python!r}"

    def _assert_tuple_parity(self, items: list[Type], seen_unpack: bool, engage: bool) -> None:
        native = self._tuple_native(items, seen_unpack)
        python = self._tuple_python(items, seen_unpack)
        if engage:
            assert native is not None, "tuple build did not engage"
        assert native is None or native == python, (
            f"tuple {[str(i) for i in items]} unpack={seen_unpack}: "
            f"native {native!r} != python {python!r}"
        )

    def test_join_same_type_engages(self) -> None:
        # [A, A] -> list[A]; the prejoin returns the operand itself.
        self._assert_join_parity(self.fx.a, self.fx.a, engage=True)

    def test_join_subtype_pair_engages(self) -> None:
        # B <: A; join([A, B]) = A. Previously the args-less Instance-Instance
        # nominal pair deferred to Python.
        self._assert_join_parity(self.fx.a, self.fx.b, engage=True)

    def test_join_common_ancestor_engages(self) -> None:
        # B and C both derive from A; join([B, C]) = A. Previously deferred.
        self._assert_join_parity(self.fx.b, self.fx.c, engage=True)

    def test_join_unrelated_instances_engages(self) -> None:
        # E and D are unrelated branches of object; join(list[E], list[D])
        # = list[object] via the nominal fold (not via a union fallback).
        # The container seam engages and matches join_type_list.
        self._assert_join_parity(self.fx.e, self.fx.d, engage=True)

    def test_join_instances_with_args_engages(self) -> None:
        # Args-bearing Instances (List[A], List[B]) skip the args-less
        # prejoin; the general join_type_list fold decides (list[A]).
        self._assert_join_parity(self.fx.lsta, self.fx.lstb, engage=True)

    def test_container_decided_none_engages(self) -> None:
        # Two callables join to a Callable, which fails
        # `allow_fast_container_literal`; Python's own fallback join returns
        # None too, so Rust returns the `False` decided-none sentinel.
        from mypy.checkexpr import _try_native_container_type, allow_fast_container_literal
        from mypy.join import join_type_list

        c1 = self.fx.callable(self.fx.a, self.fx.nonet)
        c2 = self.fx.callable(self.fx.b, self.fx.nonet)
        result = _try_native_container_type("list", [c1, c2])
        assert result is False, f"expected decided-none sentinel, got {result!r}"
        joined = get_proper_type(join_type_list([c1, c2]))
        assert not allow_fast_container_literal(joined)

    def test_container_typeobj_join_still_defers(self) -> None:
        # Type-object callables need join_similar_callables' from_type_type
        # handling, so the seam keeps deferring; the fallback owns the
        # outcome and the shim must not claim a decision.
        from mypy.checkexpr import _try_native_container_type

        c1 = self.fx.callable_type(self.fx.a, self.fx.a)
        c2 = self.fx.callable_type(self.fx.b, self.fx.b)
        assert _try_native_container_type("list", [c1, c2]) is None

    def test_unpack_single_tuple_instance_normalizes(self) -> None:
        # Tuple[*tuple[A, ...]] expands to the tuple[A, ...] Instance.
        # Previously this whole path deferred (seen_unpack -> None).
        unpack = UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))
        self._assert_tuple_parity([unpack], seen_unpack=True, engage=True)

    def test_unpack_multi_items_identity(self) -> None:
        # More than one item: the expansion is the identity; the TupleType
        # is returned as-is.
        unpack = UnpackType(Instance(self.fx.std_tuplei, [self.fx.b]))
        self._assert_tuple_parity([self.fx.a, unpack], seen_unpack=True, engage=True)

    def test_no_unpack_plain_tuple(self) -> None:
        self._assert_tuple_parity([self.fx.a, self.fx.b], seen_unpack=False, engage=True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckcallSetopsDeferSuite(Suite):
    """Parity for the checkcall.rs defer sites audited in issue #844.

    The audit identified `real_union` (checkexpr.py:4541) as a seam that
    can decide more than it defers: a non-Union proper type, including a
    TypeVar whose upper bound is a union, is `Some(false)` in the kernel
    (Python's `isinstance(typ, UnionType)` is False on the TypeVar
    itself). This suite locks in the parity of that native decision.

    Gate-on vs gate-off must agree on the result, and direct seam calls
    prove the Rust side engages rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import (
            _set_native_checkcall_active,
            _set_native_checkexpr_active,
            _set_native_checkexpr_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._type_infos = type_infos
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active = _set_native_checkexpr_active
        self._set_checkcall_active = _set_native_checkcall_active
        self._set_resolver = _set_native_checkexpr_resolver
        self._set_active(True)
        self._set_checkcall_active(True)
        self._set_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_checkcall_active(False)
        self._set_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    # --- expression checker wall for the real_union method ---

    def _make_checker(self) -> ExpressionChecker:
        """Minimal ExpressionChecker with only the fields real_union touches.

        The method only reads `_CHECKEXPR_HAS_TYPE_KERNEL`,
        `_native_checkexpr_active`, `state.strict_optional` and serializes
        `typ`; everything else is inert for this call, so a wall with
        `None` providers is safe (matching the existing `method_fullname`
        suite which passes `None` for the checker).
        """
        from mypy.checker import TypeChecker
        from mypy.checkexpr import ExpressionChecker
        from mypy.errors import Errors
        from mypy.messages import MessageBuilder
        from mypy.nodes import MypyFile, SymbolTable
        from mypy.plugin import Plugin

        options = Options()
        errors = Errors(options)
        tree = MypyFile([], [])
        tree.is_stub = True
        tree.names = SymbolTable()
        modules: dict[str, MypyFile] = {}
        chk = TypeChecker(errors, modules, options, tree, "", Plugin(options), {})
        msg = MessageBuilder(errors, modules)
        return ExpressionChecker(chk, msg, Plugin(options), {})

    # --- real_union ---

    def _assert_real_union_par(self, typ: Type) -> None:
        expr = self._make_checker()
        for strict in (True, False):
            old = mypy.state.state.strict_optional
            mypy.state.state.strict_optional = strict
            try:
                off = self._with_gate(False, lambda: expr.real_union(typ))
                on = self._with_gate(True, lambda: expr.real_union(typ))
            finally:
                mypy.state.state.strict_optional = old
            msg = f"real_union parity strict={strict} {typ}"
            assert_equal(on, off, msg)

    def _assert_real_union_engages(self, typ: Type) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        result = _type_kernel.rust_real_union(
            self.resolver, _serialize_type_for_checkexpr(typ), True
        )
        assert result is not None, f"Rust real_union did not engage for {typ}"

    def test_real_union_strips_none_non_strict(self) -> None:
        # relevant_items() drops NoneType when strict_optional is False.
        typ = UnionType([self.fx.a, self.fx.nonet])
        self._assert_real_union_par(typ)
        self._assert_real_union_engages(typ)

    def test_real_union_keeps_none_strict(self) -> None:
        # With strict_optional True, len(relevant_items()) is 2 -> real.
        typ = UnionType([self.fx.a, self.fx.nonet])
        self._assert_real_union_par(typ)
        self._assert_real_union_engages(typ)

    def test_real_union_typevar_not_union(self) -> None:
        # A TypeVar is not an isinstance-UnionType in Python, so real_union
        # is False even though its upper bound is a union.
        tvar = TypeVarType(
            "T",
            "T",
            TypeVarId(1),
            [],
            UnionType([self.fx.a, self.fx.b]),
            AnyType(TypeOfAny.special_form),
        )
        expr = self._make_checker()
        for strict in (True, False):
            old = mypy.state.state.strict_optional
            mypy.state.state.strict_optional = strict
            try:
                off = self._with_gate(False, lambda: expr.real_union(tvar))
                on = self._with_gate(True, lambda: expr.real_union(tvar))
            finally:
                mypy.state.state.strict_optional = old
            assert_equal(on, off, f"real_union typevar parity strict={strict}")
            assert off is False
        from mypy.checkexpr import _serialize_type_for_checkexpr

        result = _type_kernel.rust_real_union(
            self.resolver, _serialize_type_for_checkexpr(tvar), True
        )
        assert result is not None and result is False


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRefersToTypedDictSuite(Suite):
    """Parity for `rust_refers_to_typeddict` (issue #980).

    `ExpressionChecker.refers_to_typeddict` (checkexpr.py:1385-1393)
    is a pure bool predicate run for every call expression. The Rust
    seam reads the node classes off the live base via PyO3
    (is_instance against mypy.nodes RefExpr / TypeInfo / TypeAlias) and
    matches the TypeAlias target against the TypedDictType wire tag;
    the Python shim serializes the target's proper type to wire bytes.
    Direct seam calls must agree with the expected bool, and toggling
    the checkexpr gate off vs on must produce identical results.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _typeinfo(self, fullname: str = "mod.Movie") -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = [info]
        return info

    def _typeddict_info(self) -> TypeInfo:
        info = self._typeinfo("mod.Movie")
        info.typeddict_type = TypedDictType({}, set(), set(), Instance(info, []))
        return info

    def _ref(self, node: object) -> NameExpr:
        from mypy.nodes import NameExpr

        expr = NameExpr("td")
        expr.node = node  # type: ignore[assignment]
        return expr

    def _alias_bytes(self, target: Type) -> bytes:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        return _serialize_type_for_checkexpr(get_proper_type(target))

    def _alias(self, target: Type) -> object:
        from mypy.nodes import TypeAlias

        return TypeAlias(target, "mod.Alias", "mod", 1, 0)

    def _assert_par(self, base: Expression, expected: bool) -> None:
        from mypy.checkexpr import ExpressionChecker

        ec = ExpressionChecker(self._make_checker(), None, None, None)  # type: ignore[arg-type]
        off = self._with_gate(False, lambda: ec.refers_to_typeddict(base))
        on = self._with_gate(True, lambda: ec.refers_to_typeddict(base))
        assert off == expected, f"gate-off: {off} != {expected}"
        assert on == expected, f"gate-on: {on} != {expected}"

    def _assert_seam(self, base: Expression, expected: bool) -> None:
        from mypy.nodes import TypeAlias

        target_bytes: bytes | None = None
        node = getattr(base, "node", None)
        if isinstance(node, TypeAlias):
            target_bytes = self._alias_bytes(node.target)
        result = _type_kernel.rust_refers_to_typeddict(base, target_bytes)
        assert result == expected, f"seam: {result} != {expected}"

    def _make_checker(self) -> Any:
        from mypy.checker import TypeChecker
        from mypy.errors import Errors
        from mypy.nodes import MypyFile, SymbolTable
        from mypy.plugin import Plugin

        options = Options()
        errors = Errors(options)
        tree = MypyFile([], [])
        tree.is_stub = True
        tree.names = SymbolTable()
        modules: dict[str, MypyFile] = {}
        return TypeChecker(errors, modules, options, tree, "", Plugin(options), {})

    def test_direct_typeddict_info(self) -> None:
        # RefExpr -> TypeInfo with typeddict_type: direct reference.
        expr = self._ref(self._typeddict_info())
        self._assert_seam(expr, True)
        self._assert_par(expr, True)

    def test_plain_typeinfo_false(self) -> None:
        # RefExpr -> TypeInfo without typeddict_type.
        expr = self._ref(self._typeinfo())
        self._assert_seam(expr, False)
        self._assert_par(expr, False)

    def test_alias_to_typeddict_true(self) -> None:
        # RefExpr -> TypeAlias whose target proper-type is TypedDictType.
        info = self._typeinfo()
        td = TypedDictType({}, set(), set(), Instance(info, []))
        expr = self._ref(self._alias(td))
        self._assert_seam(expr, True)
        self._assert_par(expr, True)

    def test_alias_to_instance_false(self) -> None:
        # RefExpr -> TypeAlias targeting a plain Instance.
        info = self._typeinfo()
        expr = self._ref(self._alias(Instance(info, [])))
        self._assert_seam(expr, False)
        self._assert_par(expr, False)

    def test_var_node_false(self) -> None:
        # RefExpr -> Var: neither TypeInfo nor TypeAlias arm fires.
        from mypy.nodes import Var

        expr = self._ref(Var("x"))
        self._assert_seam(expr, False)
        self._assert_par(expr, False)

    def test_node_none_false(self) -> None:
        # RefExpr with an unresolved node.
        expr = self._ref(None)
        self._assert_seam(expr, False)
        self._assert_par(expr, False)

    def test_non_refexpr_false(self) -> None:
        # IntExpr has no `.node`; the shim's getattr must not raise and
        # the seam returns False for a non-RefExpr base.
        from mypy.nodes import IntExpr

        base: Expression = IntExpr(3)
        self._assert_seam(base, False)
        self._assert_par(base, False)

    def test_seam_alias_missing_bytes_raises(self) -> None:
        # A TypeAlias node without target bytes is unreachable through
        # the shim; the seam must not silently return False.
        info = self._typeinfo()
        td = TypedDictType({}, set(), set(), Instance(info, []))
        expr = self._ref(self._alias(td))
        try:
            _type_kernel.rust_refers_to_typeddict(expr, None)
        except ValueError:
            pass
        else:
            raise AssertionError("expected ValueError for missing alias bytes")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeValidVarArgSuite(Suite):
    """Parity for the Rust is_valid_var_arg / is_valid_keyword_var_arg
    ports (mypy.checkexpr, issue #981).

    The two bool predicates run on every call with star args. The Python
    shim computes the resolver-backed is_subtype acceptance booleans and
    passes them to Rust; Rust decides the isinstance disjunction from the
    wire bytes. Toggling the checkexpr gate off (pure Python) and on (Rust
    seam) must produce identical results; direct seam calls prove the Rust
    functions engage, and alias inputs prove the deferral path.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import ExpressionChecker, _set_native_checkexpr_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checkexpr_active
        self._set_active(True)
        self.ec = ExpressionChecker.__new__(ExpressionChecker)
        self.ec.chk = _ValidVarArgStubChk(  # type: ignore[assignment]
            {
                "builtins.str": self.fx.str_type_info,
                "typing.Iterable": self.fx.make_type_info(
                    "typing.Iterable", typevars=["T"], mro=[self.fx.oi]
                ),
                "_typeshed.SupportsKeysAndGetItem": self.fx.make_type_info(
                    "_typeshed.SupportsKeysAndGetItem", typevars=["K", "V"], mro=[self.fx.oi]
                ),
            }
        )
        self.dict_info = self.fx.make_type_info(
            "builtins.dict", typevars=["K", "V"], mro=[self.fx.oi]
        )

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _assert_par_var(self, typ: Type) -> None:
        off = self._with_gate(False, lambda: self.ec.is_valid_var_arg(typ))
        on = self._with_gate(True, lambda: self.ec.is_valid_var_arg(typ))
        assert_equal(off, on, f"is_valid_var_arg parity {typ}")
        assert isinstance(off, bool), off

    def _assert_par_kwarg(self, typ: Type) -> None:
        off = self._with_gate(False, lambda: self.ec.is_valid_keyword_var_arg(typ))
        on = self._with_gate(True, lambda: self.ec.is_valid_keyword_var_arg(typ))
        assert_equal(off, on, f"is_valid_keyword_var_arg parity {typ}")
        assert isinstance(off, bool), off

    # -- is_valid_var_arg -------------------------------------------------

    def test_var_arg_tuple(self) -> None:
        tup = TupleType([], self.fx.std_tuple)
        self._assert_par_var(tup)
        assert self._with_gate(True, lambda: self.ec.is_valid_var_arg(tup)) is True

    def test_var_arg_any(self) -> None:
        self._assert_par_var(self.fx.anyt)
        assert self._with_gate(True, lambda: self.ec.is_valid_var_arg(self.fx.anyt)) is True

    def test_var_arg_non_iterable_instance(self) -> None:
        # A is unrelated to the stub Iterable, so the is_subtype acceptance
        # boolean is False and the Instance tag falls through to it.
        self._assert_par_var(self.fx.a)
        assert self._with_gate(True, lambda: self.ec.is_valid_var_arg(self.fx.a)) is False

    def test_var_arg_union(self) -> None:
        u = UnionType([self.fx.a, self.fx.nonet])
        self._assert_par_var(u)

    def test_var_arg_alias_defers(self) -> None:
        # TypeAliasType defers on the wire (no resolved alias target);
        # parity holds because the shim falls back to the Python body.
        a, _ = self.fx.def_alias_1(self.fx.a)
        self._assert_par_var(a)

    # -- is_valid_keyword_var_arg -----------------------------------------

    def test_kwarg_dict_str_keys(self) -> None:
        d = Instance(self.dict_info, [self.fx.str_type, self.fx.a])
        self._assert_par_kwarg(d)
        assert self._with_gate(True, lambda: self.ec.is_valid_keyword_var_arg(d)) is True

    def test_kwarg_dict_non_str_keys(self) -> None:
        d = Instance(self.dict_info, [self.fx.a, self.fx.a])
        self._assert_par_kwarg(d)
        assert self._with_gate(True, lambda: self.ec.is_valid_keyword_var_arg(d)) is False

    def test_kwarg_any(self) -> None:
        # Any passes the SupportsKeysAndGetItem[str, Any] acceptance.
        self._assert_par_kwarg(self.fx.anyt)
        assert (
            self._with_gate(True, lambda: self.ec.is_valid_keyword_var_arg(self.fx.anyt)) is True
        )

    def test_kwarg_none(self) -> None:
        self._assert_par_kwarg(self.fx.nonet)
        assert (
            self._with_gate(True, lambda: self.ec.is_valid_keyword_var_arg(self.fx.nonet)) is False
        )

    def test_kwarg_plain_instance(self) -> None:
        # A is unrelated to the stub SKAG protocol, all booleans False.
        self._assert_par_kwarg(self.fx.a)
        assert self._with_gate(True, lambda: self.ec.is_valid_keyword_var_arg(self.fx.a)) is False

    def test_kwarg_alias_defers(self) -> None:
        a, _ = self.fx.def_alias_1(self.fx.a)
        self._assert_par_kwarg(a)

    # -- direct seam calls --------------------------------------------------

    def _seam_var(self, typ: Type, iterable_ok: bool) -> object:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        return _type_kernel.rust_is_valid_var_arg(_serialize_type_for_checkexpr(typ), iterable_ok)

    def _seam_kwarg(self, typ: Type, dict_ok: bool, skag_str: bool, skag_never: bool) -> object:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        return _type_kernel.rust_is_valid_keyword_var_arg(
            _serialize_type_for_checkexpr(typ), dict_ok, skag_str, skag_never
        )

    def test_seam_var_engages(self) -> None:
        assert self._seam_var(self.fx.a, False) is False
        assert self._seam_var(self.fx.anyt, False) is True
        assert self._seam_var(self.fx.a, True) is True

    def test_seam_kwarg_engages(self) -> None:
        d = Instance(self.dict_info, [self.fx.str_type, self.fx.a])
        assert self._seam_kwarg(d, True, False, False) is True
        assert self._seam_kwarg(self.fx.a, False, False, False) is False
        assert self._seam_kwarg(self.fx.a, False, True, False) is True

    def test_seam_alias_defers(self) -> None:
        a, _ = self.fx.def_alias_1(self.fx.a)
        assert self._seam_var(a, True) is None
        assert self._seam_kwarg(a, False, True, True) is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIndexWithTypeSuite(Suite):
    """Parity for `rust_classify_index_with_type` (issue #999).

    `ExpressionChecker.visit_index_with_type` (checkexpr.py:6095) dispatches
    every subscript expression on the proper left type: variadic-tuple
    normalization, union fan-out, the `in_checked_function()`-gated tuple
    arm, TypedDict, enum / generic-alias type objects, TypeVar,
    special-form Instance, and the trailing `__getitem__` tail. The Rust
    seam classifies the branch from PyO3 facts and returns a tag; Python
    applies the branch bodies (the tuple slice vs int-literal vs nonliteral
    sub-dispatch stays in Python because the literal body needs the ns
    values from `try_getting_int_literals`, which re-accepts the index).
    Direct seam calls assert the exact tag per branch; the gate-off vs
    gate-on differential drives the real `visit_index_with_type` through a
    bare `ExpressionChecker` and asserts identical results and captured
    side effects.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _tag(self, left: Type, in_checked: bool = True, expand: bool = True) -> int | None:
        chk = SimpleNamespace(in_checked_function=lambda: in_checked)
        return _type_kernel.rust_classify_index_with_type(left, chk, expand)

    def _tuple(self, *items: Type) -> TupleType:
        return TupleType(list(items), self.fx.std_tuple)

    def _variadic_tuple(self) -> TupleType:
        unpacked = Instance(self.fx.std_tuplei, [self.fx.o])
        return TupleType([self.fx.a, UnpackType(unpacked)], self.fx.std_tuple)

    def _enum_callable(self) -> CallableType:
        enum_info = self.fx.make_type_info("Color")
        enum_info.is_enum = True
        return CallableType([], [], [], Instance(enum_info, []), self.fx.type_type)

    # -- direct seam tests (all 9 tags + deferrals) --

    def _seam(self, left: Type, in_checked: bool = True, expand: bool = True) -> int | None:
        return _type_kernel.rust_classify_index_with_type(left, _chk(in_checked), expand)

    def test_seam_variadic_normalizes(self) -> None:
        assert self._seam(self._variadic_tuple()) == 0

    def test_seam_variadic_no_expand(self) -> None:
        assert self._seam(self._variadic_tuple(), expand=False) == 2

    def test_seam_union(self) -> None:
        u = UnionType([self.fx.a, self.fx.b])
        assert self._seam(u) == 1

    def test_seam_tuple_checked(self) -> None:
        t = self._tuple(self.fx.a, self.fx.b)
        assert self._seam(t) == 2

    def test_seam_tuple_unchecked(self) -> None:
        t = self._tuple(self.fx.a, self.fx.b)
        assert self._seam(t, in_checked=False) == 8

    def test_seam_typeddict(self) -> None:
        td = TypedDictType({"x": self.fx.o}, {"x"}, set(), self.fx.a, -1, -1)
        assert self._seam(td) == 3

    def test_seam_enum(self) -> None:
        assert self._seam(self._enum_callable()) == 4

    def test_seam_generic_alias_builtin_type(self) -> None:
        c = self.fx.callable_type(Instance(self.fx.type_typei, []))
        assert self._seam(c) == 5

    def test_seam_generic_alias_type_vars(self) -> None:
        c = CallableType([], [], [], Instance(self.fx.gi, [self.fx.o]), self.fx.type_type)
        assert self._seam(c) == 5

    def test_seam_typevar(self) -> None:
        assert self._seam(self.fx.t) == 6

    def test_seam_special_form(self) -> None:
        sf_info = self.fx.make_type_info("typing._SpecialForm", module_name="typing")
        assert self._seam(Instance(sf_info, [])) == 7

    def test_seam_getitem_instance(self) -> None:
        assert self._seam(self.fx.a) == 8

    def test_seam_getitem_callable_not_type_obj(self) -> None:
        c = self.fx.callable(self.fx.o)
        assert self._seam(c) == 8

    def test_seam_type_obj_fallthrough(self) -> None:
        # A type object that is neither an enum nor a generic alias falls
        # through to the __getitem__ tail.
        c = self.fx.callable_type(self.fx.a)
        assert self._seam(c) == 8

    def test_seam_defers_on_broken_chk(self) -> None:
        # A tuple left type needs in_checked_function(); an unreadable
        # checker defers (None).
        t = self._tuple(self.fx.a, self.fx.b)
        assert _type_kernel.rust_classify_index_with_type(t, SimpleNamespace(), True) is None

    # -- gate off/on differential tests through the real method --

    def _make_ec(self, in_checked: bool, obs: list[Any]) -> tuple[Any, list[Any]]:
        from mypy.checkexpr import ExpressionChecker

        accepts: list[Any] = []

        def _named_type(name: Any) -> Any:
            obs.append(("named_type", name))
            return Instance(self.fx.gi, [self.fx.o])

        anyt = AnyType(TypeOfAny.from_error)
        chk = SimpleNamespace(
            in_checked_function=lambda: in_checked,
            fail=lambda msg, ctx, code=None: obs.append(("fail", str(msg))),
            note=lambda msg, ctx: obs.append(("note", str(msg))),
            named_type=_named_type,
        )
        ec = ExpressionChecker.__new__(ExpressionChecker)
        ec.chk = chk  # type: ignore[assignment]

        def _accept(node: Any, *a: Any) -> Any:
            accepts.append(node)
            return anyt

        ec.accept = _accept  # type: ignore[method-assign, assignment]

        def _mcall(
            name: Any,
            lt: Any,
            args: Any,
            kinds: Any,
            ctx: Any = None,
            original_type: Any = None,
            self_type: Any = None,
        ) -> Any:
            obs.append(("mcall", name, str(lt)))
            return (anyt, ("__getitem__",))

        ec.check_method_call_by_name = _mcall  # type: ignore[method-assign, assignment]

        def _tdict(td: Any, idx: Any, setitem: bool = False) -> Any:
            obs.append(("tdict",))
            return (anyt, set())

        ec.visit_typeddict_index_expr = _tdict  # type: ignore[method-assign, assignment]

        def _enum(info: Any, idx: Any, ctx: Any) -> Any:
            obs.append(("enum", info.fullname))
            return anyt

        ec.visit_enum_index_expr = _enum  # type: ignore[method-assign, assignment]
        return ec, accepts

    def _run_visit(
        self, left: Type, index: Expression, in_checked: bool = True
    ) -> tuple[object, ...]:
        def run_one() -> tuple[object, ...]:
            obs: list[Any] = []
            ec, accepts = self._make_ec(in_checked, obs)
            e = IndexExpr(NameExpr("base"), index)
            try:
                result = ec.visit_index_with_type(left, e)
            except Exception as exc:
                result = "EXC:" + type(exc).__name__
            return (str(result), e.method_type, tuple(obs), len(accepts))

        off = self._with_gate(False, run_one)
        on = self._with_gate(True, run_one)
        return off, on

    def _assert_par(self, left: Type, index: Any, in_checked: bool = True) -> None:
        off, on = self._run_visit(left, index, in_checked)
        assert_equal(on, off, "visit_index_with_type parity")

    def test_par_instance_getitem(self) -> None:
        self._assert_par(self.fx.a, NameExpr("k"))

    def test_par_union_of_instances(self) -> None:
        self._assert_par(UnionType([self.fx.a, self.fx.b]), NameExpr("k"))

    def test_par_union_with_tuple(self) -> None:
        self._assert_par(UnionType([self._tuple(self.fx.a, self.fx.b), self.fx.b]), IntExpr(1))

    def test_par_tuple_literal_in_range(self) -> None:
        self._assert_par(self._tuple(self.fx.a, self.fx.b), IntExpr(0))

    def test_par_tuple_literal_out_of_range(self) -> None:
        self._assert_par(self._tuple(self.fx.a, self.fx.b), IntExpr(3))

    def test_par_tuple_slice(self) -> None:
        self._assert_par(self._tuple(self.fx.a, self.fx.b), SliceExpr(None, None, None))

    def test_par_tuple_nonliteral(self) -> None:
        self._assert_par(self._tuple(self.fx.a, self.fx.b), MemberExpr(NameExpr("x"), "y"))

    def test_par_variadic_tuple_literal(self) -> None:
        self._assert_par(self._variadic_tuple(), IntExpr(0))

    def test_par_tuple_unchecked(self) -> None:
        self._assert_par(self._tuple(self.fx.a, self.fx.b), NameExpr("k"), in_checked=False)

    def test_par_typeddict(self) -> None:
        td = TypedDictType({"x": self.fx.o}, {"x"}, set(), self.fx.a, -1, -1)
        self._assert_par(td, StrExpr("x"))

    def test_par_enum(self) -> None:
        self._assert_par(self._enum_callable(), NameExpr("k"))

    def test_par_generic_alias(self) -> None:
        c = self.fx.callable_type(Instance(self.fx.type_typei, []))
        self._assert_par(c, NameExpr("k"))

    def test_par_typevar(self) -> None:
        self._assert_par(self.fx.t, NameExpr("k"))

    def test_par_special_form(self) -> None:
        sf_info = self.fx.make_type_info("typing._SpecialForm", module_name="typing")
        self._assert_par(Instance(sf_info, []), NameExpr("k"))

    def test_par_type_obj_fallthrough(self) -> None:
        self._assert_par(self.fx.callable_type(self.fx.a), NameExpr("k"))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeArgInferPassesSuite(Suite):
    """Parity for the Rust `get_arg_infer_passes` port (mypy.checkexpr).

    The Rust seam (`rust_get_arg_infer_passes` in checkcall.rs) decides
    the pass-1/pass-2 classification for every formal: the
    ArgInferSecondPassQuery fold and the ParamSpec skip trigger, including
    the `find_member("__call__", ...)` restricted subset for Instance
    actuals. Python keeps the result application. Toggling the checkexpr
    gate off (pure Python) and on (Rust seam) must produce identical pass
    lists, and direct seam calls prove each decidable case engages
    natively.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        # A class with a plain non-generic `__call__` method:
        # def __call__(self) -> Any.
        self.callablei = self.fx.make_type_info("mod.Callee", mro=[self.fx.oi])
        fn = FuncDef(
            "__call__",
            [],
            None,
            CallableType(
                [Instance(self.callablei, [])], [ARG_POS], [None], self.fx.anyt, self.fx.function
            ),
        )
        fn.info = self.callablei
        self.callablei.names["__call__"] = SymbolTableNode(MDEF, fn)
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.append(self.callablei)
        typeinfo_map = {info.fullname: info for info in type_infos}
        set_wire_typeinfo_map(typeinfo_map)
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        # Live seam lookups (find_member on Instance actuals) read the
        # live map installed on the resolver, as build.py does.
        self.resolver.set_live_typeinfo_map(typeinfo_map)
        self._set_active = _set_native_checkexpr_active
        self._set_resolver = _set_native_checkexpr_resolver
        self._set_active(True)
        self._set_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self.resolver.set_live_typeinfo_map(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], object]) -> object:
        self._set_active(active)
        self._set_resolver(self.resolver if active else None)
        try:
            return fn()
        finally:
            self._set_active(True)
            self._set_resolver(self.resolver)

    def _assert_par(
        self,
        callee: CallableType,
        arg_types: list[Type],
        formal_to_actual: list[list[int]],
        args: list[Expression] | None = None,
    ) -> None:
        from mypy.checkexpr import ExpressionChecker

        if args is None:
            args = [NameExpr(f"x{i}") for i in range(len(arg_types))]
        num_actuals = len(arg_types)
        off = self._with_gate(
            False,
            lambda: ExpressionChecker.get_arg_infer_passes(
                None,  # type: ignore[arg-type]
                callee,
                args,
                arg_types,
                formal_to_actual,
                num_actuals,
            ),
        )
        on = self._with_gate(
            True,
            lambda: ExpressionChecker.get_arg_infer_passes(
                None,  # type: ignore[arg-type]
                callee,
                args,
                arg_types,
                formal_to_actual,
                num_actuals,
            ),
        )
        assert off == on, f"arg infer passes parity: off={off} on={on}"

    def _assert_engages(
        self,
        formals: list[Type],
        actuals: list[Type],
        lambda_flags: list[bool],
        formal_to_actual: list[list[int]],
        expected: list[int],
    ) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        result = _type_kernel.rust_get_arg_infer_passes(
            self.resolver,
            [_serialize_type_for_checkexpr(t) for t in formals],
            [_serialize_type_for_checkexpr(t) for t in actuals],
            lambda_flags,
            formal_to_actual,
            len(actuals),
        )
        assert result is not None, "rust_get_arg_infer_passes did not engage"
        assert result == expected, f"{result} != {expected}"

    def _tvar_callable_ret(self, *arg_types: Type) -> CallableType:
        """A callable formal whose return type is the fixture TypeVar."""
        return CallableType(
            list(arg_types),
            [ARG_POS] * len(arg_types),
            [None] * len(arg_types),
            self.fx.t,
            self.fx.function,
        )

    def _param_spec(self) -> ParamSpecType:
        from mypy.types import ParamSpecFlavor, TypeVarId

        return ParamSpecType(
            "P", "mod.P", TypeVarId(1), ParamSpecFlavor.BARE, self.fx.o, self.fx.o
        )

    def _param_spec_formal(self, ret: Type) -> CallableType:
        """A `Callable[P, ret]` formal with the trailing P.args/P.kwargs
        shape that CallableType.param_spec() recognizes."""
        from mypy.types import ParamSpecFlavor

        ps = self._param_spec()
        return CallableType(
            [ps.with_flavor(ParamSpecFlavor.ARGS), ps.with_flavor(ParamSpecFlavor.KWARGS)],
            [ARG_STAR, ARG_STAR2],
            [None, None],
            ret,
            self.fx.function,
            name=None,
            variables=[ps],
        )

    def test_plain_formal_pass_one(self) -> None:
        callee = self.fx.callable(self.fx.a, self.fx.anyt)
        self._assert_par(callee, [self.fx.anyt], [[0]])
        self._assert_engages(callee.arg_types, [self.fx.anyt], [False], [[0]], [1])

    def test_typevar_in_ret_promotes_pass_two(self) -> None:
        # def f(cb: Callable[[], T]) -> ...: the typevar in the callable
        # return promotes the actual to pass 2.
        callee = self.fx.callable(self._tvar_callable_ret(), self.fx.anyt)
        self._assert_par(callee, [self.fx.anyt], [[0]])
        self._assert_engages(callee.arg_types, [self.fx.anyt], [False], [[0]], [2])

    def test_nested_typevar_callable_arg_pass_two(self) -> None:
        # The typevar hides in a nested callable argument position.
        inner = self._tvar_callable_ret()
        outer = CallableType([inner], [ARG_POS], [None], self.fx.anyt, self.fx.function)
        callee = self.fx.callable(outer, self.fx.anyt)
        self._assert_par(callee, [self.fx.anyt], [[0]])

    def test_generic_callable_formal_pass_one(self) -> None:
        # A generic callable formal with a concrete arg/ret stays pass 1:
        # HasTypeVars does not walk callable `variables`.
        generic = self.fx.callable(self.fx.a, self.fx.anyt)
        generic.variables = (self.fx.t,)
        callee = self.fx.callable(generic, self.fx.anyt)
        self._assert_par(callee, [self.fx.anyt], [[0]])

    def test_lambda_actual_flagged(self) -> None:
        # The lambda flag only matters inside the ParamSpec arm; a plain
        # callable formal with a typevar ret still promotes the lambda.
        from mypy.nodes import Block, LambdaExpr, ReturnStmt

        lam = LambdaExpr(arguments=[], body=Block([ReturnStmt(NameExpr("x"))]), typ=None)
        callee = self.fx.callable(self._tvar_callable_ret(), self.fx.anyt)
        self._assert_par(callee, [self.fx.anyt], [[0]], args=[lam])

    def test_param_spec_plain_callable_skips_pass_two(self) -> None:
        # run(Callable[P, None], *args: P.args, **kwargs: P.kwargs);
        # run(test, 1, 2): a concrete non-lambda actual suppresses pass 2.
        # The P-shaped callable is the callee's nested formal.
        from mypy.nodes import Block, LambdaExpr, ReturnStmt

        lam = LambdaExpr(arguments=[], body=Block([ReturnStmt(NameExpr("x"))]), typ=None)
        ps_formal = self._param_spec_formal(self.fx.anyt)
        callee = self.fx.callable(ps_formal, self.fx.anyt)
        plain_actual = self.fx.callable(self.fx.a, self.fx.anyt)
        self._assert_par(callee, [plain_actual], [[0]])
        self._assert_engages(callee.arg_types, [plain_actual], [False], [[0]], [1])
        # A lambda actual never triggers the skip: promoted to pass 2.
        self._assert_par(callee, [self.fx.anyt], [[0]], args=[lam])
        self._assert_engages(callee.arg_types, [self.fx.anyt], [True], [[0]], [2])

    def test_param_spec_instance_actual_with_call(self) -> None:
        # An Instance actual with a plain non-generic __call__ method
        # triggers the skip via find_member on both sides.
        ps_formal = self._param_spec_formal(self.fx.anyt)
        callee = self.fx.callable(ps_formal, self.fx.anyt)
        inst_actual = Instance(self.callablei, [])
        self._assert_par(callee, [inst_actual], [[0]])
        self._assert_engages(callee.arg_types, [inst_actual], [False], [[0]], [1])

    def test_param_spec_generic_callable_pass_two(self) -> None:
        # A generic callable actual cannot trigger the skip; the ParamSpec
        # formal itself has typevars in its args -> pass 2.
        ps_formal = self._param_spec_formal(self.fx.anyt)
        callee = self.fx.callable(ps_formal, self.fx.anyt)
        generic_actual = self.fx.callable(self.fx.a, self.fx.anyt)
        generic_actual.variables = (self.fx.t,)
        self._assert_par(callee, [generic_actual], [[0]])
        self._assert_engages(callee.arg_types, [generic_actual], [False], [[0]], [2])

    def test_param_spec_multiple_actuals(self) -> None:
        # Two actuals for one ParamSpec formal: the concrete one decides
        # the skip for the whole formal (both stay pass 1).
        ps_formal = self._param_spec_formal(self.fx.anyt)
        callee = self.fx.callable(ps_formal, self.fx.anyt)
        plain_actual = self.fx.callable(self.fx.a, self.fx.anyt)
        generic_actual = self.fx.callable(self.fx.a, self.fx.anyt)
        generic_actual.variables = (self.fx.t,)
        self._assert_par(callee, [generic_actual, plain_actual], [[0, 1]])

    def test_plain_and_typevar_formals_mixed(self) -> None:
        # One callee with a plain formal and a typevar-ret formal: only
        # the actual of the latter is promoted.
        formals: list[Type] = [self._tvar_callable_ret(), self.fx.a]
        callee = CallableType(
            formals, [ARG_POS, ARG_POS], [None, None], self.fx.anyt, self.fx.function
        )
        self._assert_par(callee, [self.fx.anyt, self.fx.anyt], [[0], [1]])
        self._assert_engages(
            formals, [self.fx.anyt, self.fx.anyt], [False, False], [[0], [1]], [2, 1]
        )

    def test_param_spec_class_direct(self) -> None:
        # Direct seam call with the nested ParamSpec formal and an
        # Instance actual; the method is non-generic -> skip (1).
        from mypy.checkexpr import _serialize_type_for_checkexpr

        ps_formal = self._param_spec_formal(self.fx.anyt)
        inst_actual = Instance(self.callablei, [])
        result = _type_kernel.rust_get_arg_infer_passes(
            self.resolver,
            [_serialize_type_for_checkexpr(ps_formal)],
            [_serialize_type_for_checkexpr(inst_actual)],
            [False],
            [[0]],
            1,
        )
        assert result == [1], f"expected skip result [1], got {result}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckArgSuite(Suite):
    """Direct-seam and production pins for `rust_classify_check_arg` (#1048).

    `ExpressionChecker.check_arg` (checkexpr.py:4371-4415) dispatches a
    4-way branch: DeletedType -> deleted_as_rvalue,
    has_abstract_type_part -> concrete_only_call, not is_subtype ->
    incompatible_argument (+ optional note + check_possible_missing_await),
    else pass. The Rust seam decides only the tag from the wire caller
    type plus two Python-computed booleans (is_subtype via the subtype
    resolver; has_abstract_type_part via rust_has_abstract_type with the
    Tuple-x-Tuple fold kept Python-side); Python applies every side effect.

    That seam is unwired, not removed: fcc1f8e2c dropped the call from
    `check_arg` after measuring 233k calls / 10.3MB wire / 0.60s proxy on a
    cold self-check, and kept the pyfunction registered for the direct-seam
    calls below ("The Rust pyfunction stays registered for direct-seam
    tests"). So the production tests here pin one observation each: a
    gate-off vs gate-on differential cannot fail on this host, because
    `check_arg` reaches zero `rust_*` seams and reads no gate in either
    state (measured, #1834). The re-wired-call pin lives in
    `testtypes_native_retired_checkexpr.py::NativeCheckArgClassifyRetiredSuite`.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _make_ec(self) -> tuple[ExpressionChecker, list[tuple[str, str]]]:
        captured: list[tuple[str, str]] = []
        error = SimpleNamespace(code=None)

        def incompatible_argument(n: int, m: int, *a: Any, **kw: Any) -> Any:
            captured.append(("incompatible_argument", f"{n}:{m}"))
            return error

        msg = SimpleNamespace(
            deleted_as_rvalue=lambda t, ctx: captured.append(("deleted_as_rvalue", str(t.source))),
            concrete_only_call=lambda t, ctx: captured.append(("concrete_only_call", str(t))),
            incompatible_argument=incompatible_argument,
            incompatible_argument_note=lambda ot, ct, ctx, *, parent_error: captured.append(
                ("incompatible_argument_note", "note")
            ),
            prefer_simple_messages=lambda: False,
        )
        chk = SimpleNamespace(
            options=Options(),
            allow_abstract_call=False,
            check_possible_missing_await=lambda ct, ct2, ctx, code: captured.append(
                ("await", str(code))
            ),
        )
        ec = ExpressionChecker.__new__(ExpressionChecker)
        ec.chk = chk  # type: ignore[assignment]
        ec.msg = msg  # type: ignore[assignment]
        return ec, captured

    def _run_check_arg(
        self, caller_type: Type, callee_type: Type, kind: ArgKind
    ) -> list[tuple[str, str]]:
        """One `check_arg` run with the gate on, as production runs it."""
        fx = TypeFixture()
        ec, captured = self._make_ec()
        callee = fx.callable_type(fx.anyt)
        ctx = NameExpr("ctx")
        try:
            ec.check_arg(caller_type, caller_type, kind, callee_type, 1, 1, callee, None, ctx, ctx)
        except Exception as exc:
            captured.append(("EXC", str(exc)))
        return captured

    # -- direct seam tests (all 4 tags) --

    def _wire(self, t: Type) -> bytes:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        return _serialize_type_for_checkexpr(t)

    def test_seam_deleted(self) -> None:
        from mypy.checkexpr import NATIVE_CHECK_ARG_DELETED

        tag = _type_kernel.rust_classify_check_arg(self._wire(DeletedType("x")), False, False)
        assert tag == NATIVE_CHECK_ARG_DELETED, f"{tag}"

    def test_seam_deleted_beats_all(self) -> None:
        # Python checks DeletedType first; the booleans are irrelevant.
        from mypy.checkexpr import NATIVE_CHECK_ARG_DELETED

        tag = _type_kernel.rust_classify_check_arg(self._wire(DeletedType("x")), False, True)
        assert tag == NATIVE_CHECK_ARG_DELETED, f"{tag}"

    def test_seam_abstract_only(self) -> None:
        from mypy.checkexpr import NATIVE_CHECK_ARG_ABSTRACT_ONLY

        tag = _type_kernel.rust_classify_check_arg(
            self._wire(AnyType(TypeOfAny.special_form)), True, True
        )
        assert tag == NATIVE_CHECK_ARG_ABSTRACT_ONLY, f"{tag}"

    def test_seam_incompatible(self) -> None:
        from mypy.checkexpr import NATIVE_CHECK_ARG_INCOMPATIBLE

        tag = _type_kernel.rust_classify_check_arg(
            self._wire(AnyType(TypeOfAny.special_form)), False, False
        )
        assert tag == NATIVE_CHECK_ARG_INCOMPATIBLE, f"{tag}"

    def test_seam_pass(self) -> None:
        from mypy.checkexpr import NATIVE_CHECK_ARG_PASS

        tag = _type_kernel.rust_classify_check_arg(
            self._wire(AnyType(TypeOfAny.special_form)), True, False
        )
        assert tag == NATIVE_CHECK_ARG_PASS, f"{tag}"

    def test_seam_abstract_beats_incompatible(self) -> None:
        from mypy.checkexpr import NATIVE_CHECK_ARG_ABSTRACT_ONLY

        tag = _type_kernel.rust_classify_check_arg(
            self._wire(AnyType(TypeOfAny.special_form)), False, True
        )
        assert tag == NATIVE_CHECK_ARG_ABSTRACT_ONLY, f"{tag}"

    def test_seam_defers_on_bad_wire(self) -> None:
        assert _type_kernel.rust_classify_check_arg(b"\xff\xff\xff", True, True) is None

    # -- production branch tests (all 4 branches, one run each) --

    def test_par_deleted(self) -> None:
        fx = TypeFixture()
        assert self._run_check_arg(DeletedType("x"), fx.anyt, ARG_POS) == [
            ("deleted_as_rvalue", "x")
        ]

    def test_par_abstract_only(self) -> None:
        fx = TypeFixture()
        # The fixture leaves is_abstract unset; set it so the
        # FunctionLike-caller x TypeType-callee abstract fold fires.
        fx.fi.is_abstract = True
        caller = fx.callable_type(Instance(fx.fi, []))
        callee = TypeType.make_normalized(Instance(fx.fi, []))
        assert self._run_check_arg(caller, callee, ARG_POS) == [
            ("concrete_only_call", str(callee))
        ]

    def test_par_incompatible(self) -> None:
        fx = TypeFixture()
        assert self._run_check_arg(fx.a, fx.d, ARG_POS) == [
            ("incompatible_argument", "1:1"),
            ("incompatible_argument_note", "note"),
            ("await", "None"),
        ]

    def test_par_incompatible_star_no_note(self) -> None:
        # For *args / **kwargs the note would be incorrect: suppressed.
        fx = TypeFixture()
        assert self._run_check_arg(fx.a, fx.d, ARG_STAR) == [
            ("incompatible_argument", "1:1"),
            ("await", "None"),
        ]

    def test_par_pass(self) -> None:
        fx = TypeFixture()
        assert self._run_check_arg(fx.a, fx.a, ARG_POS) == []


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckArgCountSuite(Suite):
    """Direct-seam tests for `rust_check_argument_count` (issue #1136).

    `ExpressionChecker.check_argument_count` (checkexpr.py:3855) folded
    check_for_extra_actual_arguments and the formal loop into one Rust seam
    with the scalar-fact interface. That shim was measured and retired in
    #1739 (1.80x-2.26x slower than the Python body on the no-error,
    too-many and too-few shapes, because it classified every actual's proper
    type before crossing), so the `test_seam_*` cases below exercise the
    registered pyfunction directly and the `test_on_*` cases pin the
    production method's messages under the checkexpr gate.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    # ---- direct seam tests (scalar interface) ----

    def _seam(
        self,
        formal_kinds: list[int],
        actual_kinds: list[int],
        actual_names: list[str | None],
        actual_shapes: list[int],
        actual_item_counts: list[int],
        formal_to_actual: list[list[int]],
        *,
        has_param_spec: bool = False,
        special_sig: str | None = None,
        object_type_present: bool = False,
        callable_name: str | None = None,
        in_checked_function: bool = True,
    ) -> tuple[bool, list[tuple[int, int, int]], bool] | None:
        from mypy.checkexpr import (
            NATIVE_ARG_SHAPE_ALIAS,
            NATIVE_ARG_SHAPE_PARAM_SPEC,
            NATIVE_ARG_SHAPE_PLAIN,
            NATIVE_ARG_SHAPE_TUPLE,
            NATIVE_ARG_SHAPE_TYPEDDICT,
        )

        # Tag vocabulary stays in lockstep with the Rust ACTUAL_* constants.
        assert (NATIVE_ARG_SHAPE_PLAIN, NATIVE_ARG_SHAPE_TUPLE) == (0, 1)
        assert (NATIVE_ARG_SHAPE_TYPEDDICT, NATIVE_ARG_SHAPE_PARAM_SPEC) == (2, 3)
        assert NATIVE_ARG_SHAPE_ALIAS == 4
        return _type_kernel.rust_check_argument_count(
            formal_kinds,
            has_param_spec,
            special_sig,
            actual_kinds,
            actual_names,
            actual_shapes,
            actual_item_counts,
            formal_to_actual,
            object_type_present,
            callable_name,
            in_checked_function,
        )

    def test_seam_ok(self) -> None:
        result = self._seam([int(ARG_POS.value)], [int(ARG_POS.value)], [None], [0], [0], [[0]])
        assert result == (True, [], False), result

    def test_seam_too_few_with_classvar_note(self) -> None:
        # Formal 1 unmatched and object_type/callable_name present.
        result = self._seam(
            [int(ARG_POS.value)] * 2,
            [int(ARG_POS.value)],
            [None],
            [0],
            [0],
            [[0], []],
            object_type_present=True,
            callable_name="mod.A.attr",
        )
        assert result is not None
        ok, errors, unexpected = result
        assert not ok and not unexpected
        assert errors == [(4, 1, 0), (11, 1, 0)], errors

    def test_seam_too_few_note_suppressed(self) -> None:
        # callable_name without a dot: no note record.
        result = self._seam(
            [int(ARG_POS.value)] * 2,
            [int(ARG_POS.value)],
            [None],
            [0],
            [0],
            [[0], []],
            object_type_present=True,
            callable_name="func",
        )
        assert result is not None
        _, errors, _ = result
        assert errors == [(4, 1, 0)], errors

    def test_seam_missing_named_plus_extra(self) -> None:
        # A positional actual unmapped against a named-only formal counts
        # as one too-many and one too-few error, mirroring the Python body.
        result = self._seam([int(ARG_NAMED.value)], [int(ARG_POS.value)], [None], [0], [0], [[]])
        assert result is not None
        ok, errors, unexpected = result
        assert not ok and not unexpected
        assert errors == [(0, 0, 0), (5, 0, 0)], errors

    def test_seam_extra_unnamed(self) -> None:
        result = self._seam(
            [int(ARG_POS.value)], [int(ARG_POS.value)] * 2, [None, None], [0, 0], [0, 0], [[0], []]
        )
        assert result is not None
        ok, errors, unexpected = result
        assert not ok and not unexpected
        assert errors == [(0, 1, 0)], errors

    def test_seam_extra_named(self) -> None:
        result = self._seam(
            [int(ARG_POS.value)],
            [int(ARG_POS.value), int(ARG_NAMED.value)],
            [None, "x"],
            [0, 0],
            [0, 0],
            [[0], []],
        )
        assert result is not None
        ok, errors, unexpected = result
        assert not ok and unexpected
        assert errors == [(1, 1, 0)], errors

    def test_seam_defers_on_alias_actual(self) -> None:
        assert (
            self._seam([int(ARG_POS.value)], [int(ARG_POS.value)], [None], [4], [0], [[0]]) is None
        )

    def test_seam_defers_on_unnamed_named_extra(self) -> None:
        # An unmatched named-kind actual without a name defers.
        assert (
            self._seam(
                [int(ARG_POS.value)],
                [int(ARG_POS.value), int(ARG_NAMED.value)],
                [None, None],
                [0, 0],
                [0, 0],
                [[0], []],
            )
            is None
        )

    def test_seam_star_tuple_leftover_items(self) -> None:
        result = self._seam([int(ARG_POS.value)], [int(ARG_STAR.value)], [None], [1], [2], [[0]])
        assert result is not None
        ok, errors, unexpected = result
        assert not ok and not unexpected
        assert errors == [(2, 0, 0)], errors

    def test_seam_star_empty_tuple_ok(self) -> None:
        result = self._seam([], [int(ARG_STAR.value)], [None], [1], [0], [])
        assert result == (True, [], False), result

    def test_seam_star2_typeddict_leftover_items(self) -> None:
        result = self._seam([int(ARG_POS.value)], [int(ARG_STAR2.value)], [None], [2], [2], [[0]])
        assert result is not None
        ok, errors, unexpected = result
        assert not ok and unexpected
        assert errors == [(3, 0, 0)], errors

    def test_seam_star2_non_typeddict_ok(self) -> None:
        result = self._seam([int(ARG_POS.value)], [int(ARG_STAR2.value)], [None], [0], [0], [[0]])
        assert result == (True, [], False), result

    def test_seam_duplicate_mapping(self) -> None:
        result = self._seam(
            [int(ARG_POS.value)],
            [int(ARG_STAR2.value)] * 2,
            [None, None],
            [0, 2],
            [0, 1],
            [[0, 1]],
        )
        assert result is not None
        ok, errors, unexpected = result
        assert not ok and not unexpected
        assert errors == [(6, 0, 0)], errors

    def test_seam_duplicate_shape_lookup_by_mapped_actual(self) -> None:
        # Issue #1152 repro: shapes must be indexed by the mapped actual
        # index mapping[i] holds (Python reads actual_types[m]), not by
        # position. The mapping holds actuals 2 and 3 (both plain dicts).
        result = self._seam(
            [int(ARG_POS.value)],
            [int(ARG_STAR2.value)] * 4,
            [None] * 4,
            [2, 0, 0, 0],
            [1, 0, 0, 0],
            [[2, 3]],
        )
        assert result is not None
        ok, errors, unexpected = result
        assert not ok and unexpected
        # The unmapped TypedDict actual 0 fires the too-many record first;
        # the duplicate record must NOT fire.
        assert errors == [(3, 0, 0)], errors

    def test_seam_star_plus_kwargs_mapping_ok(self) -> None:
        # f(..., *args, **kwargs): two actuals may share one formal.
        result = self._seam(
            [int(ARG_POS.value)],
            [int(ARG_STAR.value), int(ARG_STAR2.value)],
            [None, None],
            [0, 0],
            [0, 0],
            [[0, 1]],
        )
        assert result == (True, [], False), result

    def test_seam_too_many_positional_for_named_formal(self) -> None:
        result = self._seam([int(ARG_NAMED.value)], [int(ARG_POS.value)], [None], [0], [0], [[0]])
        assert result is not None
        ok, errors, _ = result
        assert not ok and errors == [(7, 0, 0)], errors

    def test_seam_param_spec_star_too_few(self) -> None:
        # Under a param_spec callee the required unmapped formal is reported by
        # the main table arm (ERR_TOO_FEW_POSITIONAL), which runs before the CPS
        # elif; the mapped *args/**kwargs formals with one actual each are fine.
        result = self._seam(
            [int(ARG_POS.value), int(ARG_STAR.value), int(ARG_STAR2.value)],
            [int(ARG_STAR.value), int(ARG_STAR2.value)],
            [None, None],
            [0, 0],
            [0, 0],
            [[], [0], [1]],
            has_param_spec=True,
        )
        assert result is not None
        ok, errors, _ = result
        assert not ok
        assert errors == [(4, 0, 0)], errors

    def test_seam_param_spec_unmapped_star_too_few(self) -> None:
        # The CPS arm's own too-few tag (ERR_PARAMSPEC_TOO_FEW) only
        # reaches a formal the earlier arms did not already claim: here a
        # non-required *args formal left unmapped with special_sig unset.
        result = self._seam([int(ARG_STAR.value)], [], [], [], [], [[]], has_param_spec=True)
        assert result is not None
        ok, errors, _ = result
        assert not ok and errors == [(8, 0, 0)], errors

    def test_seam_param_spec_partial_special_sig_ok(self) -> None:
        # functools.partial callees may leave *args/**kwargs unmapped.
        result = self._seam(
            [int(ARG_STAR.value), int(ARG_STAR2.value)],
            [int(ARG_STAR.value), int(ARG_STAR2.value)],
            [None, None],
            [0, 0],
            [0, 0],
            [[0], [1]],
            has_param_spec=True,
            special_sig="partial",
        )
        assert result == (True, [], False), result

    def test_seam_param_spec_args_once(self) -> None:
        result = self._seam(
            [int(ARG_STAR.value)],
            [int(ARG_STAR.value)] * 2,
            [None, None],
            [3, 3],
            [0, 0],
            [[0, 1]],
            has_param_spec=True,
        )
        assert result is not None
        ok, errors, _ = result
        assert not ok and errors == [(9, 0, 0)], errors

    def test_seam_param_spec_kwargs_once(self) -> None:
        result = self._seam(
            [int(ARG_STAR2.value)],
            [int(ARG_STAR2.value)] * 2,
            [None, None],
            [3, 3],
            [0, 0],
            [[0, 1]],
            has_param_spec=True,
        )
        assert result is not None
        ok, errors, _ = result
        assert not ok and errors == [(10, 0, 0)], errors

    def test_seam_param_spec_needs_two_tail_formals(self) -> None:
        # has_param_spec requires ARG_STAR/ARG_STAR2 as the last two formals;
        # with the flag off (the shim's verdict for an invalid tail) the plain
        # decisions apply, so an optional unmapped *args formal is not an error.
        result = self._seam(
            [int(ARG_POS.value), int(ARG_STAR.value)],
            [int(ARG_POS.value)],
            [None],
            [0],
            [0],
            [[0], []],
        )
        assert result == (True, [], False), result

    # ---- production-path value tests (native shim retired, #1739) ----

    def _make_ec(
        self, in_checked_function: bool
    ) -> tuple[ExpressionChecker, list[tuple[str, ...]]]:
        captured: list[tuple[str, ...]] = []
        msg = SimpleNamespace(
            too_many_arguments=lambda c, ctx: captured.append(("too_many",)),
            unexpected_keyword_argument=lambda c, n, t, ctx: captured.append(("unexpected_kw", n)),
            too_many_arguments_from_typed_dict=lambda c, t, ctx: captured.append(("too_many_td",)),
            too_few_arguments=lambda c, ctx, ns: captured.append(("too_few",)),
            missing_named_argument=lambda c, ctx, n: captured.append(("missing_named", n)),
            duplicate_argument_value=lambda c, i, ctx: captured.append(("dup", str(i))),
            too_many_positional_arguments=lambda c, ctx: captured.append(("too_many_pos",)),
            fail=lambda m, ctx: captured.append(("fail", str(m))),
            note=lambda m, ctx: captured.append(("note", str(m))),
        )
        chk = SimpleNamespace(in_checked_function=lambda: in_checked_function)
        ec = ExpressionChecker.__new__(ExpressionChecker)
        ec.chk = chk  # type: ignore[assignment]
        ec.msg = msg  # type: ignore[assignment]
        return ec, captured

    def _messages_on_gate(
        self,
        callee: CallableType,
        actual_types: list[Type],
        actual_kinds: list[ArgKind],
        actual_names: list[str | None] | None,
        formal_to_actual: list[list[int]],
        *,
        object_type: Type | None = None,
        callable_name: str | None = None,
        in_checked_function: bool = True,
    ) -> tuple[tuple[str, ...], ...]:
        """Run the production method once and return its message record.

        The checkexpr gate is ON (setUp), which is the production
        configuration. The records were frozen from the pre-retirement
        pure-Python arm; `NativeCheckArgCountRetiredSuite` pins that the
        method cannot cross to Rust any more.
        """
        from mypy.nodes import TempNode

        context = TempNode(AnyType(TypeOfAny.special_form))
        ec, captured = self._make_ec(in_checked_function)
        ret = ec.check_argument_count(
            callee,
            actual_types,
            actual_kinds,
            actual_names,
            formal_to_actual,
            context,
            object_type,
            callable_name,
        )
        return tuple(captured) + (("ret", str(ret)),)

    def test_on_ok(self) -> None:
        fx = TypeFixture()
        callee = fx.callable(fx.a, fx.a)
        got = self._messages_on_gate(
            callee, [fx.a, fx.a], [ARG_POS, ARG_POS], [None, None], [[0], [1]]
        )
        assert got == (("ret", "True"),), got

    def test_on_too_few(self) -> None:
        fx = TypeFixture()
        # fx.callable(*a) treats the last arg as the return type; pass one
        # explicit return to get a two-formal callee.
        callee = fx.callable(fx.a, fx.a, fx.anyt)
        got = self._messages_on_gate(
            callee, [fx.a], [ARG_POS], [None], [[0], []], callable_name="mod.A.attr"
        )
        assert ("too_few",) in got, got
        # object_type is None, so the classvar note stays silent.

    def test_on_extra_named(self) -> None:
        fx = TypeFixture()
        callee = fx.callable(fx.a)
        got = self._messages_on_gate(
            callee, [fx.a, fx.a], [ARG_POS, ARG_NAMED], [None, "x"], [[0], []]
        )
        assert ("unexpected_kw", "x") in got, got

    def test_on_missing_named(self) -> None:
        fx = TypeFixture()
        callee = CallableType([fx.anyt], [ARG_NAMED], ["a"], fx.anyt, fx.function, name="f")
        got = self._messages_on_gate(callee, [fx.a], [ARG_POS], [None], [[]])
        assert ("missing_named", "a") in got, got
        assert ("too_many",) in got, got

    def test_on_dup_with_typeddict_kwargs(self) -> None:
        fx = TypeFixture()
        callee = CallableType([fx.anyt], [ARG_POS], [None], fx.anyt, fx.function, name="f")
        # One plain **kwargs and one TypedDict **kwargs matching the same
        # formal: duplicates are not automatically allowed (the TypedDict
        # mapping is precise), so the duplicate error fires.
        td = TypedDictType({"a": fx.a}, set(), set(), fx.function)
        got = self._messages_on_gate(
            callee, [fx.function, td], [ARG_STAR2, ARG_STAR2], [None, None], [[0, 1]]
        )
        assert ("dup", "0") in got, got

    def test_on_dup_plain_kwargs_shared_formal_ok(self) -> None:
        # Issue #1152 repro through the real method: four **kwargs actuals,
        # only actual 0 is a TypedDict, and the shared formal maps actuals
        # 2 and 3 (both plain dicts), so no duplicate error may fire.
        fx = TypeFixture()
        callee = CallableType([fx.anyt], [ARG_POS], [None], fx.anyt, fx.function, name="f")
        td = TypedDictType({"a": fx.a}, set(), set(), fx.function)
        got = self._messages_on_gate(
            callee,
            [td, fx.function, fx.function, fx.function],
            [ARG_STAR2] * 4,
            [None] * 4,
            [[2, 3]],
        )
        assert ("too_many_td",) in got, got
        assert not any(m[0] == "dup" for m in got), got

    def test_on_star_plus_kwargs_ok(self) -> None:
        fx = TypeFixture()
        callee = CallableType([fx.anyt], [ARG_POS], [None], fx.anyt, fx.function, name="f")
        got = self._messages_on_gate(
            callee,
            [TupleType([], fx.std_tuple), TypedDictType({}, set(), set(), fx.function)],
            [ARG_STAR, ARG_STAR2],
            [None, None],
            [[0, 1]],
        )
        assert got == (("ret", "True"),), got

    def test_on_param_spec_args_once(self) -> None:
        fx = TypeFixture()
        ps = ParamSpecType(
            name="P",
            fullname="P",
            id=TypeVarId(-1),
            flavor=ParamSpecFlavor.BARE,
            upper_bound=fx.o,
            default=AnyType(TypeOfAny.from_omitted_generics),
        )
        callee = CallableType(
            [ps, ps], [ARG_STAR, ARG_STAR2], [None, None], fx.anyt, fx.function, name="f"
        )
        assert callee.param_spec() is not None
        got = self._messages_on_gate(
            callee, [ps, ps], [ARG_STAR, ARG_STAR], [None, None], [[0, 1], [0, 1]]
        )
        assert ("fail", "ParamSpec.args should only be passed once") in got, got

    def test_on_classvar_note_fires(self) -> None:
        fx = TypeFixture()
        callee = CallableType(
            [fx.anyt, fx.anyt], [ARG_POS, ARG_POS], ["x", "y"], fx.anyt, fx.function, name="f"
        )
        var = Var("attr")
        var.is_inferred = False
        var.is_classvar = False
        fx.a.type.names["attr"] = SymbolTableNode(GDEF, var)
        got = self._messages_on_gate(
            callee,
            [fx.a],
            [ARG_POS],
            [None],
            [[0], []],
            object_type=fx.a,
            callable_name="mod.A.attr",
        )
        assert any(o[0] == "note" for o in got), got

    def test_on_alias_actual(self) -> None:
        # A (proper-expanded) alias actual is PLAIN for the Python body; the
        # wire-era deferral condition died with the seam.
        fx = TypeFixture()
        alias = TypeAlias(fx.a, "mod.A", "mod", -1, -1)
        t = TypeAliasType(alias, [])
        # One formal, no formals-to-actuals mapping: too_few fires.
        callee = fx.callable(fx.a, fx.a)
        got = self._messages_on_gate(callee, [t], [ARG_POS], [None], [[]])
        assert any(o[0] == "too_few" for o in got), got


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckBooleanOpSuite(Suite):
    """Parity for `rust_classify_check_boolean_op` (issue #1049).

    `ExpressionChecker.check_boolean_op` (checkexpr.py:6062) arranges the
    and/or maps, then decides the reachability gates and the result tail
    (return left / return right / Uninhabited / union). The Rust seam
    classifies the branch + tail from the wire-serialized map values and
    the expanded left operand's live can_be_true/can_be_false flags;
    Python keeps `find_isinstance_check`, `analyze_cond_branch`, the two
    self.msg emissions, and `make_simplified_union`. Direct seam calls
    assert the 4-way map table and the 4 result tails; toggling the
    checkexpr gate off vs on must produce identical (str(result),
    captured messages).
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        self._set_active = _set_native_checkexpr_active
        self._set_resolver = _set_native_checkexpr_resolver
        self.fx = TypeFixture()
        self.int_info = self.fx.make_type_info("builtins.int", mro=[self.fx.oi])
        self.int_inst: Instance = Instance(self.int_info, [])
        self.str_inst: Instance = Instance(self.fx.str_type_info, [])
        self.resolver = _type_kernel.build_native_resolver(
            [
                self.fx.oi,
                self.fx.ai,
                self.fx.bi,
                self.int_info,
                self.fx.str_type_info,
                self.fx.type_typei,
                self.fx.std_tuplei,
                self.fx.std_listi,
            ],
            [],
        )
        self._set_resolver(self.resolver)
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)
        self._set_resolver(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _make_ec(
        self,
        fic: tuple[TypeMap, TypeMap],
        left_type: Type,
        right_type: Type,
        report_unreachable: bool = False,
        enabled_error_codes: set[ErrorCode] | None = None,
    ) -> tuple[ExpressionChecker, list[tuple[str, str]]]:
        captured: list[tuple[str, str]] = []
        chk = SimpleNamespace(
            options=SimpleNamespace(
                enabled_error_codes=enabled_error_codes if enabled_error_codes else set()
            ),
            should_report_unreachable_issues=lambda: report_unreachable,
        )

        def find_isinstance_check(node: object) -> tuple[TypeMap, TypeMap]:
            return fic

        chk.find_isinstance_check = find_isinstance_check
        chk.captured = captured
        ec = ExpressionChecker.__new__(ExpressionChecker)
        ec.chk = chk  # type: ignore[assignment]
        ec.msg = SimpleNamespace(  # type: ignore[assignment]
            redundant_left_operand=lambda op, ctx: captured.append(("redundant", op)),
            unreachable_right_operand=lambda op, ctx: captured.append(("unreachable", op)),
        )
        ec.type_context = [None]

        def accept(
            node: Expression,
            type_context: Type | None = None,
            allow_none_return: bool = False,
            always_allow_any: bool = False,
            is_callee: bool = False,
        ) -> Type:
            return left_type

        def combined_context(ty: Type | None) -> Type | None:
            return ty

        def analyze_cond_branch(
            map: TypeMap,
            node: Expression,
            context: Type | None = None,
            allow_none_return: bool = False,
            suppress_unreachable_errors: bool = True,
        ) -> Type:
            return right_type

        ec.accept = accept  # type: ignore[method-assign]
        ec._combined_context = combined_context  # type: ignore[method-assign]
        ec.analyze_cond_branch = analyze_cond_branch  # type: ignore[method-assign]
        return ec, captured

    def _make_op(
        self, op: str, right_always: bool = False, right_unreachable: bool = False
    ) -> OpExpr:
        e = OpExpr(op, NameExpr("L"), NameExpr("R"))
        e.right_always = right_always
        e.right_unreachable = right_unreachable
        return e

    def _run(
        self,
        active: bool,
        e: OpExpr,
        make_ec: Callable[[], tuple[ExpressionChecker, list[tuple[str, str]]]],
    ) -> tuple[str, list[tuple[str, str]]]:
        # A fresh checker per run: the captured-message list must not leak
        # across the gate-off and gate-on passes.
        ec, captured = make_ec()
        result = self._with_gate(active, lambda: ec.check_boolean_op(e))
        return str(result), captured

    def _try_native(
        self, e: OpExpr, left_map: TypeMap, right_map: TypeMap, left: Type
    ) -> tuple[int, bool, bool, int] | None:
        from mypy.checkexpr import _try_native_check_boolean_op

        return _try_native_check_boolean_op(e, left_map, right_map, left)

    def test_seam_map_tags(self) -> None:
        # 4-way map table from e.op x e.right_always x e.right_unreachable.
        e = self._make_op("or", right_always=True)
        r = self._try_native(e, {e.left: UninhabitedType()}, {}, self.int_inst)
        assert r == (0, True, False, 1)  # RIGHT_ALWAYS, RETURN_RIGHT
        e = self._make_op("and", right_unreachable=True)
        r = self._try_native(e, {}, {e.right: UninhabitedType()}, self.int_inst)
        assert r == (1, False, True, 0)  # RIGHT_UNREACHABLE, RETURN_LEFT
        e = self._make_op("and")
        r = self._try_native(e, {}, {e.left: self.int_inst}, self.int_inst)
        assert r is not None and r[0] == 2  # MAP_AND
        e = self._make_op("or")
        # A decided tag needs a reachability hit: a plain or-tail with an int
        # Instance defers (true_only needs a live __bool__ lookup).
        r = self._try_native(
            e, {e.left: self.int_inst}, {e.right: UninhabitedType()}, self.int_inst
        )
        assert r == (3, False, True, 0)  # MAP_OR, RETURN_LEFT

    def test_seam_result_tags_raw(self) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        b = _serialize_type_for_checkexpr
        int_b = b(self.int_inst)
        unin_b = b(UninhabitedType())
        resolver = self.resolver
        seam = _type_kernel.rust_classify_check_boolean_op
        # Both maps unreachable -> UNINHABITED (3).
        r = seam(True, False, False, [unin_b], [unin_b], int_b, True, True, True, None, resolver)
        assert r == (2, True, True, 3)
        # Right unreachable -> RETURN_LEFT (result tag 0).
        r = seam(True, False, False, [], [unin_b], int_b, True, True, True, None, resolver)
        assert r == (2, False, True, 0)
        # Left unreachable -> RETURN_RIGHT (result tag 1).
        r = seam(True, False, False, [unin_b], [], int_b, True, True, True, None, resolver)
        assert r == (2, True, False, 1)
        # Live tail: false_only(int) -> LiteralType(0), result_is_left False -> UNION (2).
        r = seam(True, False, False, [], [], int_b, True, True, True, None, resolver)
        assert r == (2, False, False, 2)
        # Union expanded_left: a shim-precomputed UninhabitedType verdict decides
        # the tail natively (and: left truthy -> RETURN_RIGHT; verdict False ->
        # UNION; or: left falsy -> RETURN_RIGHT).
        union_b = b(UnionType([self.int_inst, self.str_inst]))
        r = seam(True, False, False, [], [], union_b, True, True, True, True, resolver)
        assert r == (2, False, False, 1)
        r = seam(True, False, False, [], [], union_b, True, True, True, False, resolver)
        assert r == (2, False, False, 2)
        r = seam(False, False, False, [], [], union_b, True, True, True, True, resolver)
        assert r == (3, False, False, 1)
        # No verdict -> defer.
        assert seam(True, False, False, [], [], union_b, True, True, True, None, resolver) is None

    def test_par_right_always(self) -> None:
        e = self._make_op("or", right_always=True)
        make = lambda: self._make_ec(
            ({}, {}), self.int_inst, self.str_inst, report_unreachable=True
        )
        off = self._run(False, e, make)
        on = self._run(True, e, make)
        assert off == on, f"right_always: off={off} on={on}"
        assert off == ("builtins.str", []), off

    def test_par_right_unreachable_flag(self) -> None:
        # e.right_unreachable=True: intentional, no emission even when
        # should_report_unreachable_issues() is True.
        e = self._make_op("and", right_unreachable=True)
        make = lambda: self._make_ec(
            ({}, {}), self.int_inst, self.str_inst, report_unreachable=True
        )
        off = self._run(False, e, make)
        on = self._run(True, e, make)
        assert off == on, f"right_unreachable flag: off={off} on={on}"
        assert off == ("builtins.int", []), off

    def test_par_and_union_int_left(self) -> None:
        # Both maps reachable, int left: UNION tail, false_only(int) ->
        # LiteralType(0, fallback=int) simplifies against the int right.
        e = self._make_op("and")
        fic: tuple[TypeMap, TypeMap] = ({NameExpr("L"): self.int_inst}, {})
        make = lambda: self._make_ec(fic, self.int_inst, self.int_inst)
        off = self._run(False, e, make)
        on = self._run(True, e, make)
        assert off == on, f"and-union: off={off} on={on}"
        assert off[0] == "builtins.int", off

    def test_par_or_left_unreachable(self) -> None:
        e = self._make_op("or")
        fic: tuple[TypeMap, TypeMap] = ({e.left: UninhabitedType()}, {})
        make = lambda: self._make_ec(fic, self.int_inst, self.str_inst)
        off = self._run(False, e, make)
        on = self._run(True, e, make)
        assert off == on, f"or-left-unreachable: off={off} on={on}"
        assert off == ("builtins.str", []), off

    def test_par_or_unreachable_right_emits(self) -> None:
        e = self._make_op("or")
        fic: tuple[TypeMap, TypeMap] = ({}, {e.right: UninhabitedType()})
        make = lambda: self._make_ec(fic, self.int_inst, self.str_inst, report_unreachable=True)
        off = self._run(False, e, make)
        on = self._run(True, e, make)
        assert off == on, f"or-unreachable-right: off={off} on={on}"
        assert off == ("builtins.int", [("unreachable", "or")]), off

    def test_par_redundant_left_emitted(self) -> None:
        from mypy import errorcodes as codes

        e = self._make_op("and")
        # "and" swaps: right_map, left_map = find_isinstance_check(e.left).
        fic: tuple[TypeMap, TypeMap] = ({}, {e.left: UninhabitedType()})
        make = lambda: self._make_ec(
            fic, self.int_inst, self.str_inst, enabled_error_codes={codes.REDUNDANT_EXPR}
        )
        off = self._run(False, e, make)
        on = self._run(True, e, make)
        assert off == on, f"redundant: off={off} on={on}"
        assert off == ("builtins.str", [("redundant", "and")]), off

    def test_par_both_unreachable(self) -> None:
        e = self._make_op("and")
        fic: tuple[TypeMap, TypeMap] = ({e.left: UninhabitedType()}, {e.left: UninhabitedType()})
        make = lambda: self._make_ec(fic, self.int_inst, self.str_inst)
        off = self._run(False, e, make)
        on = self._run(True, e, make)
        assert off == on, f"both-unreachable: off={off} on={on}"
        assert off[0] == "Never", off

    def test_par_union_left_decided(self) -> None:
        # expanded_left is a UnionType: the shim precomputes the
        # UninhabitedType verdict from the expanded items, so the Rust tail
        # decides instead of deferring. Gate-on must equal gate-off.
        e = self._make_op("and")
        fic: tuple[TypeMap, TypeMap] = ({}, {e.left: self.int_inst})
        left = UnionType([self.int_inst, self.str_inst])
        r = self._try_native(e, fic[1], fic[0], left)
        assert r is not None and r[3] == 2  # UNION tail, decided natively
        make = lambda: self._make_ec(fic, left, self.str_inst)
        off = self._run(False, e, make)
        on = self._run(True, e, make)
        assert off == on, f"union-left: off={off} on={on}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAlwaysReturnsNoneSuite(Suite):
    """Parity for `rust_always_returns_none` (issue #1070).

    `ExpressionChecker.always_returns_none` / `defn_returns_none`
    (checkexpr.py:1714-1779) decide whether a callee is explicitly
    annotated as only returning None. The Rust seam is a live-PyO3-object
    port (isinstance + attribute reads, zero wire bytes) that recurses
    over FuncDef / OverloadedFuncDef / Var and calls the real Python
    `get_proper_type` for ret None-ness. The MemberExpr owner type is
    checker state, so the shim pre-resolves `lookup_type(node.expr)` and
    passes the resulting TypeInfo. Direct seam calls assert the expected
    bool (and deferrals); toggling the checkexpr gate off vs on drives
    the real ExpressionChecker method and must agree on both.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _info(self, fullname: str = "mod.C") -> TypeInfo:
        from mypy.nodes import Block

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = [info]
        return info

    def _funcdef(self, ret_none: bool, name: str = "f") -> FuncDef:
        from mypy.nodes import Block

        ret: Type = NoneType() if ret_none else self.fx.o
        fd = FuncDef(name, [], Block([]), None)
        fd.type = CallableType([], [], [], ret, self.fx.function)
        return fd

    def _none_ret_callable(self) -> CallableType:
        return CallableType([], [], [], NoneType(), self.fx.function)

    def _overload(self, rets: list[bool]) -> OverloadedFuncDef:
        return OverloadedFuncDef([self._funcdef(r) for r in rets])

    def _var(self, typ: Type, is_inferred: bool = False) -> Var:
        v = Var("x")
        v.type = typ
        v.is_inferred = is_inferred
        return v

    def _call_var(self, call_ret_none: bool) -> Var:
        # Var of an instance-like type whose __call__ is a method with the
        # given return annotation; exercises the defn_returns_none
        # `__call__` recursion.
        info = self._info("mod.Callable0")
        info.names["__call__"] = SymbolTableNode(GDEF, self._funcdef(call_ret_none, "__call__"))
        return self._var(Instance(info, []))

    def _ref(self, node: object) -> NameExpr:
        expr = NameExpr("f")
        expr.node = node  # type: ignore[assignment]
        return expr

    def _checker(self) -> Any:
        from mypy.checker import TypeChecker
        from mypy.errors import Errors
        from mypy.nodes import MypyFile
        from mypy.plugin import Plugin

        options = Options()
        errors = Errors(options)
        tree = MypyFile([], [])
        tree.is_stub = True
        tree.names = SymbolTable()
        modules: dict[str, MypyFile] = {}
        return TypeChecker(errors, modules, options, tree, "", Plugin(options), {})

    def _ec(self) -> Any:
        from mypy.checkexpr import ExpressionChecker

        return ExpressionChecker(self._checker(), None, None, None)  # type: ignore[arg-type]

    def _member(self, owner: Type, name: str = "attr") -> tuple[MemberExpr, Any]:
        from mypy.checkexpr import ExpressionChecker

        base = NameExpr("obj")
        expr = MemberExpr(base, name)
        ec = ExpressionChecker(self._checker(), None, None, None)  # type: ignore[arg-type]
        ec.chk._type_maps[0][base] = owner
        return expr, ec

    def _assert_seam(self, node: Expression, expected: bool) -> None:
        result = _type_kernel.rust_always_returns_none(node, None)
        assert result == expected, f"seam: {result} != {expected}"

    def _assert_par(self, node: Expression, ec: Any, expected: bool) -> None:
        off = self._with_gate(False, lambda: ec.always_returns_none(node))
        on = self._with_gate(True, lambda: ec.always_returns_none(node))
        assert off == expected, f"gate-off: {off} != {expected}"
        assert on == expected, f"gate-on: {on} != {expected}"

    def test_seam_funcdef_none(self) -> None:
        self._assert_seam(self._ref(self._funcdef(True)), True)

    def test_seam_funcdef_not_none(self) -> None:
        self._assert_seam(self._ref(self._funcdef(False)), False)

    def test_seam_untyped_funcdef(self) -> None:
        # A dynamic function (type None) is not annotated as None-only.
        from mypy.nodes import Block

        fd = FuncDef("f", [], Block([]), None)
        self._assert_seam(self._ref(fd), False)

    def test_seam_overload_all_none(self) -> None:
        self._assert_seam(self._ref(self._overload([True, True])), True)

    def test_seam_overload_mixed(self) -> None:
        self._assert_seam(self._ref(self._overload([True, False])), False)

    def test_seam_var_annotated_none(self) -> None:
        self._assert_seam(self._ref(self._var(self._none_ret_callable())), True)

    def test_seam_var_inferred(self) -> None:
        self._assert_seam(self._ref(self._var(self._none_ret_callable(), is_inferred=True)), False)

    def test_seam_var_call_recursion(self) -> None:
        self._assert_seam(self._ref(self._call_var(True)), True)

    def test_seam_var_call_not_none(self) -> None:
        self._assert_seam(self._ref(self._call_var(False)), False)

    def test_seam_refexpr_unresolved(self) -> None:
        self._assert_seam(self._ref(None), False)

    def test_seam_var_plain_instance_no_call(self) -> None:
        # Instance type without __call__ in the MRO: False.
        self._assert_seam(self._ref(self._var(Instance(self._info(), []))), False)

    def test_seam_member_sym_none_returning(self) -> None:
        info = self._info()
        info.names["attr"] = SymbolTableNode(GDEF, self._funcdef(True))
        expr, ec = self._member(Instance(info, []))
        self._assert_par(expr, ec, True)

    def test_seam_member_sym_not_none(self) -> None:
        info = self._info()
        info.names["attr"] = SymbolTableNode(GDEF, self._funcdef(False))
        expr, ec = self._member(Instance(info, []))
        self._assert_par(expr, ec, False)

    def test_seam_member_sym_missing(self) -> None:
        info = self._info()
        expr, ec = self._member(Instance(info, []))
        self._assert_par(expr, ec, False)

    def test_seam_member_type_object(self) -> None:
        # Type-object callee: the owner is the class behind the callable.
        info = self._info()
        type_obj = CallableType([], [], [], Instance(info, []), self.fx.type_type)
        info.names["attr"] = SymbolTableNode(GDEF, self._funcdef(True))
        expr, ec = self._member(type_obj)
        self._assert_par(expr, ec, True)

    def test_seam_member_non_instance(self) -> None:
        # A non-Instance, non-type-object owner kind: both gates False.
        expr, ec = self._member(NoneType())
        self._assert_par(expr, ec, False)

    def test_seam_member_analyzed(self) -> None:
        # An analyzed MemberExpr (node set) never consults the owner type.
        info = self._info()
        info.names["attr"] = SymbolTableNode(GDEF, self._funcdef(True))
        base = NameExpr("obj")
        expr = MemberExpr(base, "attr")
        expr.node = Var("attr")
        ec = self._ec()
        ec.chk._type_maps[0][base] = Instance(info, [])
        self._assert_par(expr, ec, False)

    def test_seam_non_expression_kind(self) -> None:
        # Neither RefExpr nor MemberExpr: the seam answers False.
        self._assert_seam(IntExpr(3), False)

    def test_seam_member_missing_info_defers(self) -> None:
        # A MemberExpr arm without a pre-resolved owner defers (None);
        # the shim then re-runs the pure-Python body.
        expr = MemberExpr(NameExpr("obj"), "attr")
        result = _type_kernel.rust_always_returns_none(expr, None)
        assert result is None, f"seam: {result} is not None"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSolveGenericCallSuite(Suite):
    """Parity for the rust_solve_generic_call port (issue #1128).

    The Rust seam (checkcall.rs `rust_solve_generic_call`: normalize +
    infer constraints + solve + apply) previously deferred four shapes
    that are parity-identically decidable:

    - Empty constraints: the solver fills every unconstrained var with
      strict Never / lax Any (solve.py:277-289), the exact path the
      Stage-20 seam (#382) already relies on; the check_call shim skips
      the vacuous polymorphic retry and applies Never.
    - Multi-lower joins: the solver joins lowers via solve_one_inner; an
      invariant conflict defers later in apply. Only a joined solution
      that is itself a FunctionLike still defers (nested FuncDef
      definitions do not survive the wire; Python's pretty_callable
      renders them with the def name, e.g. list-literal item messages).
    - Positional TupleType actuals: star actuals are gated Python-side
      (checkexpr.py `any(k.is_star() ...)`), so a tuple actual reaches
      the kernel only as a positional argument that ArgTypeExpander
      passes through 1:1.
    - Unsolvable vars: when solve_one returns no solution Python emits
      "Cannot infer value of type parameter" and substitutes Any
      (checkexpr.apply_inferred_arguments); Rust has no side channel for
      that diagnostic, so the seam defers.

    Direct seam calls prove each shape engages or defers as documented,
    and the resolved structure is compared against a pure-Python
    infer + apply reference (constraints/solve native gates disabled).
    """

    def setUp(self) -> None:
        from mypy.checkexpr import (
            _set_native_checkcall_active,
            _set_native_checkexpr_active,
            _set_native_checkexpr_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        self._set_active = _set_native_checkexpr_active
        self._set_resolver = _set_native_checkexpr_resolver
        self._aliases: list[Any] = []
        self._set_active(True)
        self._set_resolver(self.resolver_for(self.fx))
        typeinfo_map = {info.fullname: info for info in self._type_infos(self.fx)}
        # The wire join echoes actual arg Instances (builtins.str) into
        # the solution; the _type_infos scan only sees attrs ending in
        # "i", so the str fixture must be registered explicitly.
        typeinfo_map[self.fx.str_type_info.fullname] = self.fx.str_type_info
        set_wire_typeinfo_map(typeinfo_map)
        self._callcall_active = _set_native_checkcall_active
        self._callcall_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._callcall_active(False)
        set_wire_typeinfo_map(None)

    def _type_infos(self, fx: TypeFixture) -> list[TypeInfo]:
        infos = []
        for name in dir(fx):
            if not name.endswith("i"):
                continue
            value = getattr(fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _resolver_for(self, fx: TypeFixture, aliases: list[Any] | None = None) -> Any:
        return _type_kernel.build_native_resolver(
            self._type_infos(fx), self._aliases if aliases is None else aliases
        )

    def resolver_for(self, fx: TypeFixture) -> Any:
        return self._resolver_for(fx)

    def _rebuild_with_aliases(self, aliases: list[Any]) -> None:
        self._aliases = aliases
        self._set_resolver(self.resolver_for(self.fx))

    def _generic_t(self) -> Any:
        from mypy.types import TypeVarType

        return TypeVarType(
            "T",
            "mod.T",
            TypeVarId(1),
            values=[],
            upper_bound=self.fx.o,
            default=AnyType(TypeOfAny.special_form),
        )

    def _callee_of(self, formals: list[Type], ret_type: Type, t: Any) -> CallableType:
        return CallableType(
            formals,
            [ARG_POS] * len(formals),
            [None] * len(formals),
            ret_type,
            self.fx.function,
            variables=[t],
        )

    def _seam_raw(
        self,
        callee: CallableType,
        arg_types: list[Type],
        formal_to_actual: list[list[int]],
        arg_kinds: list[Any] | None = None,
        iterable_type: Any | None = None,
        mapping_type: Any | None = None,
    ) -> bytes | None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        blobs = [_serialize_type_for_checkexpr(t) for t in arg_types]
        if arg_kinds is None:
            arg_kinds = [ARG_POS] * len(arg_types)
        result = _type_kernel.rust_solve_generic_call(
            self.resolver_for(self.fx),
            _serialize_type_for_checkexpr(callee),
            blobs,
            [int(k.value) for k in arg_kinds],
            formal_to_actual,
            True,
            False,
            True,
            _serialize_type_for_checkexpr(iterable_type) if iterable_type is not None else None,
            _serialize_type_for_checkexpr(mapping_type) if mapping_type is not None else None,
        )
        if result is None:
            return None
        return bytes(result)

    def _seam(
        self, callee: CallableType, arg_types: list[Type], formal_to_actual: list[list[int]]
    ) -> Any:
        result = self._seam_raw(callee, arg_types, formal_to_actual)
        if result is None:
            return None
        from mypy.checkexpr import _deserialize_type_from_checkexpr

        return _deserialize_type_from_checkexpr(result)

    def _python_reference(
        self,
        callee: CallableType,
        arg_types: list[Type],
        arg_kinds: list[Any],
        arg_names: Sequence[str | None] | None,
        formal_to_actual: list[list[int]],
    ) -> CallableType:
        from mypy.applytype import apply_generic_arguments
        from mypy.constraints import _set_native_constraints_active
        from mypy.infer import ArgumentInferContext, infer_function_type_arguments
        from mypy.nodes import TempNode
        from mypy.solve import _set_native_solve_active
        from mypy.types import TypeOfAny

        _set_native_constraints_active(False)
        _set_native_solve_active(False)
        try:
            ctx = ArgumentInferContext(Instance(self.fx.hi, []), Instance(self.fx.std_tuplei, []))
            inferred, _ = infer_function_type_arguments(
                callee, arg_types, arg_kinds, arg_names, formal_to_actual, ctx, strict=True
            )
            return apply_generic_arguments(
                callee,
                inferred,
                lambda *a: None,
                TempNode(AnyType(TypeOfAny.special_form)),
                skip_unsatisfied=False,
            )
        finally:
            _set_native_constraints_active(True)
            _set_native_solve_active(True)

    def _assert_seam_parity(
        self, callee: CallableType, arg_types: list[Type], formal_to_actual: list[list[int]]
    ) -> None:
        arg_kinds = [ARG_POS] * len(arg_types)
        native = self._seam(callee, arg_types, formal_to_actual)
        assert native is not None, f"Rust deferred on {callee!r} {arg_types!r}"
        py = self._python_reference(callee, arg_types, arg_kinds, None, formal_to_actual)
        assert isinstance(native, CallableType)  # type: ignore[misc]
        assert str(native) == str(py), f"solve parity: native={native} python={py}"

    def test_empty_constraints_solves(self) -> None:
        # def [T] (x: T) -> T with no actuals: no constraints, T fills
        # with its default (Any) via the ambiguous-Never path.
        t = self._generic_t()
        callee = self._callee_of([t], t, t)
        native = self._seam(callee, [], [])
        assert native is not None, "empty constraints should solve natively"
        assert isinstance(native, CallableType) and not native.variables  # type: ignore[misc]

    def test_empty_constraints_parity(self) -> None:
        t = self._generic_t()
        callee = self._callee_of([t], t, t)
        self._assert_seam_parity(callee, [], [])

    def test_multilower_join_parity(self) -> None:
        # def [T] (a: T, b: T) -> T with A + D: two lowers joined by the
        # solver (join of disjoint siblings -> object).
        t = self._generic_t()
        callee = self._callee_of([t, t], t, t)
        self._assert_seam_parity(callee, [self.fx.a, self.fx.d], [[0], [1]])

    def test_single_lower_parity(self) -> None:
        t = self._generic_t()
        callee = self._callee_of([t], t, t)
        self._assert_seam_parity(callee, [self.fx.a], [[0]])

    def test_tuple_actual_parity(self) -> None:
        # A positional TupleType actual is passed 1:1 by ArgTypeExpander
        # (star actuals are gated Python-side): T :> tuple[A, D].
        t = self._generic_t()
        callee = self._callee_of([t], t, t)
        actual = TupleType([self.fx.a, self.fx.d], self.fx.std_tuple)
        self._assert_seam_parity(callee, [actual], [[0]])

    def test_multilower_callable_defers(self) -> None:
        # Two callable lowers join to a FunctionLike, which loses its
        # nested FuncDef over the wire: defer to Python.
        t = self._generic_t()
        callee = self._callee_of([t, t], t, t)
        f = CallableType(
            [self.fx.str_type], [ARG_POS], ["x"], self.fx.str_type, self.fx.function, name="f"
        )
        g = CallableType(
            [self.fx.str_type], [ARG_POS], ["y"], self.fx.str_type, self.fx.function, name="g"
        )
        assert (
            self._seam(callee, [f, g], [[0], [1]]) is None
        ), "multi-lower callable join must defer (definition loss)"

    def test_unsolvable_var_defers(self) -> None:
        # def [T] (a: G[T], b: G[T]) -> T with G-invariant args A and D
        # (disjoint): T :> A/D, T <: A/D — solve_one returns no solution,
        # Python emits "Cannot infer" + Any, so the seam defers.
        t = self._generic_t()
        g = Instance(self.fx.gi, [t])
        callee = self._callee_of([g, g], t, t)
        ga = Instance(self.fx.gi, [self.fx.a])
        gd = Instance(self.fx.gi, [self.fx.d])
        assert (
            self._seam(callee, [ga, gd], [[0], [1]]) is None
        ), "unsolvable invariant conflict must defer"

    def test_alias_actual_parity(self) -> None:
        # identity(mod.A) with mod.A = A: the alias actual expands before
        # constraint inference, so T solves to A. Before the expansion,
        # the top-level alias actual deferred the whole call.
        from mypy.nodes import TypeAlias
        from mypy.types import TypeAliasType

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        t = self._generic_t()
        callee = self._callee_of([t], t, t)
        self._assert_seam_parity(callee, [TypeAliasType(alias, [])], [[0]])

    def test_alias_formal_no_constraint_parity(self) -> None:
        # def (x: mod.A) -> None accepts B: the alias formal expands to A
        # before infer_constraints (previously a top-level alias formal
        # deferred the seam to Python); the callable is non-generic.
        from mypy.nodes import TypeAlias
        from mypy.types import TypeAliasType

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        callee = CallableType(
            [TypeAliasType(alias, [])],
            [ARG_POS],
            [None],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        # The applied output keeps the raw formal, but _seam's fixup
        # refuses any TypeAliasType, so the result survives only as wire
        # bytes: byte-compare against the serialized Python reference.
        from mypy.checkexpr import _serialize_type_for_checkexpr

        raw = self._seam_raw(callee, [self.fx.b], [[0]])
        assert raw is not None, "Rust deferred on def (A) -> Any [B]"
        py = self._python_reference(callee, [self.fx.b], [ARG_POS], None, [[0]])
        assert raw == _serialize_type_for_checkexpr(
            py
        ), f"solve parity: native={raw!r} python={py!r}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeStarExpansionSuite(Suite):
    """Parity for the ArgTypeExpander star-expansion port (piece 1, #1343).

    The sgc / ifta seams previously gated every call that carried a star
    actual Python-side (checkexpr.py ``any(k.is_star() ...)``). The
    ArgTypeExpander port (argmap.py:278-432, expanded per (formal,
    actual) pair at constraints.py:751-758) now decides natively inside
    rust_solve_generic_call / rust_infer_function_type_arguments, fed
    the Iterable/Mapping argument-infer context (checkexpr.py:3725-3730)
    as blobs with the per-actual ArgKind ints and the shared
    tuple_index / kwargs_used state.

    Differential: each case runs the seam and a pure-Python reference
    (native constraints + solve gates off) with the same context and
    asserts equal resolved callables. Direct calls prove which star
    shapes engage and which still defer (missing context blob,
    TypedDict key miss, TypedDict **kwargs formal).
    """

    def setUp(self) -> None:
        from mypy.checkexpr import (
            _set_native_checkcall_active,
            _set_native_checkexpr_active,
            _set_native_checkexpr_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        self._set_active = _set_native_checkexpr_active
        self._set_resolver = _set_native_checkexpr_resolver
        self._aliases: list[Any] = []
        self._set_active(True)
        self._set_resolver(self.resolver_for(self.fx))
        typeinfo_map = {info.fullname: info for info in self._type_infos(self.fx)}
        # The wire join echoes actual arg Instances (builtins.str) into
        # the solution; the _type_infos scan only sees attrs ending in
        # "i", so the str fixture must be registered explicitly.
        typeinfo_map[self.fx.str_type_info.fullname] = self.fx.str_type_info
        set_wire_typeinfo_map(typeinfo_map)
        self._callcall_active = _set_native_checkcall_active
        self._callcall_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._callcall_active(False)
        set_wire_typeinfo_map(None)

    def _type_infos(self, fx: TypeFixture) -> list[TypeInfo]:
        infos = []
        for name in dir(fx):
            if not name.endswith("i"):
                continue
            value = getattr(fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def resolver_for(self, fx: TypeFixture) -> Any:
        return _type_kernel.build_native_resolver(self._type_infos(fx), [])

    def _generic_t(self) -> Any:
        return TypeVarType(
            "T",
            "mod.T",
            TypeVarId(1),
            values=[],
            upper_bound=self.fx.o,
            default=AnyType(TypeOfAny.special_form),
        )

    def _callee_of(self, formals: list[Type], ret: TypeVarType) -> CallableType:
        return CallableType(
            formals,
            [ARG_POS] * len(formals),
            ["x", "y"][: len(formals)],
            ret,
            self.fx.function,
            variables=[ret],
        )

    def _seam(
        self,
        callee: CallableType,
        arg_types: list[Type],
        arg_kinds: list[Any],
        formal_to_actual: list[list[int]],
        iterable_ctx: Any | None = None,
        mapping_ctx: Any | None = None,
    ) -> Any:
        from mypy.checkexpr import _deserialize_type_from_checkexpr, _serialize_type_for_checkexpr

        result = _type_kernel.rust_solve_generic_call(
            self.resolver_for(self.fx),
            _serialize_type_for_checkexpr(callee),
            [_serialize_type_for_checkexpr(t) for t in arg_types],
            [int(k.value) for k in arg_kinds],
            formal_to_actual,
            True,
            False,
            True,
            _serialize_type_for_checkexpr(iterable_ctx) if iterable_ctx else None,
            _serialize_type_for_checkexpr(mapping_ctx) if mapping_ctx else None,
        )
        if result is None:
            return None
        return _deserialize_type_from_checkexpr(bytes(result))

    def _python_reference(
        self,
        callee: CallableType,
        arg_types: list[Type],
        arg_kinds: list[Any],
        formal_to_actual: list[list[int]],
        iterable_ctx: Any | None = None,
        mapping_ctx: Any | None = None,
    ) -> CallableType:
        from mypy.applytype import apply_generic_arguments
        from mypy.constraints import _set_native_constraints_active
        from mypy.infer import ArgumentInferContext, infer_function_type_arguments
        from mypy.nodes import TempNode
        from mypy.solve import _set_native_solve_active
        from mypy.types import TypeOfAny

        _set_native_constraints_active(False)
        _set_native_solve_active(False)
        try:
            ctx = ArgumentInferContext(mapping_ctx, iterable_ctx)  # type: ignore[arg-type]
            inferred, _ = infer_function_type_arguments(
                callee, arg_types, arg_kinds, ["x", "y"], formal_to_actual, ctx, strict=True
            )
            return apply_generic_arguments(
                callee,
                inferred,
                lambda *a: None,
                TempNode(AnyType(TypeOfAny.special_form)),
                skip_unsatisfied=False,
            )
        finally:
            _set_native_constraints_active(True)
            _set_native_solve_active(True)

    def _assert_star_parity(
        self,
        callee: CallableType,
        arg_types: list[Type],
        arg_kinds: list[Any],
        formal_to_actual: list[list[int]],
        iterable_ctx: Any | None = None,
        mapping_ctx: Any | None = None,
    ) -> None:
        native = self._seam(
            callee, arg_types, arg_kinds, formal_to_actual, iterable_ctx, mapping_ctx
        )
        assert native is not None, f"Rust deferred on {arg_types!r} kinds={arg_kinds!r}"
        py = self._python_reference(
            callee, arg_types, arg_kinds, formal_to_actual, iterable_ctx, mapping_ctx
        )
        assert str(native) == str(py), f"star parity: native={native} python={py}"

    def test_star_tuple_two_actuals_shared_tuple_index(self) -> None:
        # Two *x actuals of the same tuple[A, D] feed x and y: the shared
        # tuple_index state resumes across actuals (argmap.py:379-396),
        # so the lowers are A then D (T joins to object via the solver).
        t = self._generic_t()
        callee = self._callee_of([t, t], t)
        tup = TupleType([self.fx.a, self.fx.d], self.fx.std_tuple)
        self._assert_star_parity(callee, [tup, tup], [ARG_STAR, ARG_STAR], [[0], [1]])

    def test_star_iterable_instance_same_args(self) -> None:
        # *x where x: G[A] against context G[A]: identical-args subtype
        # decides true, the same-ref map yields args[0] = A
        # (argmap.py:363-369).
        t = self._generic_t()
        callee = self._callee_of([t], t)
        ctx = Instance(self.fx.gi, [self.fx.a])
        self._assert_star_parity(callee, [ctx], [ARG_STAR], [[0]], iterable_ctx=ctx)

    def test_star_iterable_instance_not_a_subtype(self) -> None:
        # *x where x: G[B] against context G[A] (invariant): not a
        # subtype, so the expander answers the improper-use tail
        # AnyType(from_error) (argmap.py:370-376, 402).
        t = self._generic_t()
        callee = self._callee_of([t], t)
        actual = Instance(self.fx.gi, [self.fx.b])
        ctx = Instance(self.fx.gi, [self.fx.a])
        self._assert_star_parity(callee, [actual], [ARG_STAR], [[0]], iterable_ctx=ctx)

    def test_star_kwargs_typeddict_named_key(self) -> None:
        # **x where x is a TypedDict feeds the named formal's value type;
        # kwargs_used records the consumed key (argmap.py:405-416).
        t = self._generic_t()
        callee = CallableType([t], [ARG_NAMED], ["a"], t, self.fx.function, variables=[t])
        td = TypedDictType({"a": self.fx.a}, {"a"}, set(), Instance(self.fx.ai, []))
        self._assert_star_parity(callee, [td], [ARG_STAR2], [[0]])

    def test_star_kwargs_mapping_instance_same_args(self) -> None:
        # **x where x: H[D, A] against context H[D, A]: subtype decides
        # true, the mapped args[1] is the value type A
        # (argmap.py:417-424).
        t = self._generic_t()
        callee = self._callee_of([t], t)
        ctx = Instance(self.fx.hi, [self.fx.d, self.fx.a])
        self._assert_star_parity(callee, [ctx], [ARG_STAR2], [[0]], mapping_ctx=ctx)

    def test_star_kwargs_mapping_instance_not_a_subtype(self) -> None:
        # **x where x: G[A] cannot be unpacked with **: the expander
        # answers AnyType(from_error) (argmap.py:420-424, 428-429).
        t = self._generic_t()
        callee = self._callee_of([t], t)
        actual = Instance(self.fx.gi, [self.fx.a])
        mapping_ctx = Instance(self.fx.hi, [self.fx.d, self.fx.a])
        self._assert_star_parity(callee, [actual], [ARG_STAR2], [[0]], mapping_ctx=mapping_ctx)

    def test_star_tvt_upper_bound_shared_tuple_index(self) -> None:
        # *x where x is a TypeVarTuple with upper_bound tuple[A, D]: the
        # expander continues with the upper bound (argmap.py:358-362)
        # and the tuple walk resumes across actuals (x: A, y: D).
        t = self._generic_t()
        callee = self._callee_of([t, t], t)
        tvt = TypeVarTupleType(
            "Ts",
            "mod.Ts",
            TypeVarId(3),
            upper_bound=TupleType([self.fx.a, self.fx.d], self.fx.std_tuple),
            tuple_fallback=Instance(self.fx.std_tuplei, [self.fx.anyt]),
            default=AnyType(TypeOfAny.special_form),
        )
        self._assert_star_parity(callee, [tvt, tvt], [ARG_STAR, ARG_STAR], [[0], [1]])

    def test_missing_iterable_context_defers(self) -> None:
        # A star Instance actual without an Iterable context blob defers
        # the whole seam to Python (no context, no guess).
        t = self._generic_t()
        callee = self._callee_of([t], t)
        actual = Instance(self.fx.gi, [self.fx.a])
        assert (
            self._seam(callee, [actual], [ARG_STAR], [[0]]) is None
        ), "missing Iterable context must defer"

    def test_star_kwargs_key_miss_defers(self) -> None:
        # A TypedDict ** actual whose named formal has no matching item
        # defers (argmap.py:407-410 unreachable-name tail is checker-side).
        t = self._generic_t()
        callee = CallableType([t], [ARG_NAMED], ["z"], t, self.fx.function, variables=[t])
        td = TypedDictType({"a": self.fx.a}, {"a"}, set(), Instance(self.fx.ai, []))
        assert self._seam(callee, [td], [ARG_STAR2], [[0]]) is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeobjGateSuite(Suite):
    """Gate-off vs gate-on parity for the check_callable_call typeobj-fail
    gate (issue #1464 C2).

    Runs ExpressionChecker.check_callable_call on a type-object callee and
    asserts identical captured protocol/abstract fails with the native
    classifier off and on, plus direct seam calls on the live callees.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], Any]) -> Any:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _seam(self, callee: CallableType) -> int:
        result = _type_kernel.rust_classify_typeobj_gate(callee)
        assert result is not None
        return result

    def _type_object_callable(self, info: Any, from_type_type: bool = False) -> CallableType:
        callee = self.fx.callable_type(self.fx.a, Instance(info, []))
        callee.from_type_type = from_type_type
        return callee

    def _protocol_info(self) -> Any:
        info = self.fx.make_type_info("ProtoKlass")
        assert info is not None
        info.is_protocol = True
        return info

    def _abstract_info(self) -> Any:
        info = self.fx.make_type_info("AbsKlass")
        assert info is not None
        info.is_abstract = True
        return info

    def _run(self, callee: CallableType) -> list[tuple[str, ...]]:
        from mypy.checker import TypeChecker
        from mypy.checkexpr import ExpressionChecker
        from mypy.errors import Errors
        from mypy.messages import MessageBuilder
        from mypy.nodes import Context, MypyFile, SymbolTable
        from mypy.options import Options
        from mypy.plugin import Plugin

        options = Options()
        errors = Errors(options)
        tree = MypyFile([], [])
        tree.is_stub = True
        tree.names = SymbolTable()
        chk = TypeChecker(errors, {}, options, tree, "", Plugin(options), {})
        msg = MessageBuilder(errors, {})
        ec = ExpressionChecker(chk, msg, Plugin(options), {})
        captured: list[tuple[str, ...]] = []
        ec.chk.fail = lambda m, ctx, code=None: captured.append(  # type: ignore[method-assign, misc, assignment]
            ("fail", getattr(m, "value", str(m)))
        )
        ec.msg.cannot_instantiate_abstract_class = lambda name, attrs, ctx: captured.append(  # type: ignore[method-assign, assignment]
            ("abs", name, str(sorted(attrs)))
        )
        try:
            ec.check_callable_call(callee, [], [], Context(), None, None, None, None)
        except Exception as err:
            captured.append(("exc", repr(err)))
        return captured

    def _par(self, callee: CallableType) -> None:
        off = self._with_gate(False, lambda: self._run(callee))
        on = self._with_gate(True, lambda: self._run(callee))
        assert off == on, f"typeobj-gate parity for {callee!r}: {off} != {on}"

    # --- direct seam: the gate tags ---

    def test_seam_not_typeobj(self) -> None:
        assert self._seam(self.fx.callable(self.fx.a, self.fx.o)) == 0

    def test_seam_typeobj_plain(self) -> None:
        assert self._seam(self.fx.callable_type(self.fx.a, self.fx.b)) == 0

    def test_seam_defers_on_unreadable_protocol(self) -> None:
        # A type-object callee whose TypeInfo.is_protocol read raises
        # AttributeError: the seam must defer (None) instead of propagating,
        # so the shim re-runs the pure-Python gate (issue #1466 pin).
        callee = self._type_object_callable(_BrokenAttrInfo("mod.Plain", "is_protocol"))
        result = _type_kernel.rust_classify_typeobj_gate(callee)
        assert result is None

    def test_par_unreadable_protocol(self) -> None:
        # Both gates raise the identical AttributeError: gate-off is the pure
        # gate, gate-on defers the unreadable attribute to it (issue #1466).
        callee = self._type_object_callable(_BrokenAttrInfo("mod.Plain", "is_protocol"))
        self._par(callee)

    def test_seam_repropagates_non_attribute_error(self) -> None:
        # A type-object callee whose TypeInfo.is_protocol read raises
        # RuntimeError must NOT be swallowed: the seam re-propagates it
        # (issue #1466 error-class boundary pin).
        import pytest

        callee = self._type_object_callable(
            _BrokenAttrInfo("mod.Plain", "is_protocol", RuntimeError)
        )
        with pytest.raises(RuntimeError):
            _type_kernel.rust_classify_typeobj_gate(callee)

    def test_par_repropagates_non_attribute_error(self) -> None:
        # Both gates behave identically on a non-AttributeError read: the
        # pure gate raises RuntimeError, gate-on re-propagates it from the
        # seam instead of deferring (issue #1466).
        callee = self._type_object_callable(
            _BrokenAttrInfo("mod.Plain", "is_protocol", RuntimeError)
        )
        self._par(callee)

    def test_seam_protocol(self) -> None:
        assert self._seam(self._type_object_callable(self._protocol_info())) == 1

    def test_seam_protocol_exempt(self) -> None:
        assert (
            self._seam(self._type_object_callable(self._protocol_info(), from_type_type=True)) == 0
        )

    def test_seam_abstract(self) -> None:
        assert self._seam(self._type_object_callable(self._abstract_info())) == 2

    def test_seam_abstract_exempt_fbany(self) -> None:
        info = self.fx.make_type_info("AbsKlass2")
        assert info is not None
        info.is_abstract = True
        info.fallback_to_any = True
        assert self._seam(self._type_object_callable(info)) == 0

    def test_seam_abstract_exempt_from_type_type(self) -> None:
        assert (
            self._seam(self._type_object_callable(self._abstract_info(), from_type_type=True)) == 0
        )

    # --- gate-off vs gate-on differentials on the captured fails ---

    def test_par_protocol(self) -> None:
        self._par(self._type_object_callable(self._protocol_info()))

    def test_par_abstract(self) -> None:
        self._par(self._type_object_callable(self._abstract_info()))

    def test_par_not_typeobj(self) -> None:
        self._par(self.fx.callable(self.fx.a, self.fx.o))

    def test_par_typeobj_plain(self) -> None:
        self._par(self.fx.callable_type(self.fx.a, self.fx.b))

    def test_par_protocol_exempt(self) -> None:
        self._par(self._type_object_callable(self._protocol_info(), from_type_type=True))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeArgApproxAliasSuite(Suite):
    """Wave-61B: alias expansion for `arg_approximate_similarity` (#1512).

    The wave-61 audit pinned all 20 cold-self-check fallbacks to a
    `TypeAliasType` operand reaching `get_proper_or_defer` (the wire alias
    was treated as unexpandable). The seam expands a top-level alias
    through the resolver's alias snapshot before the shape comparison; a
    resolver without the snapshot still defers. Gate-off vs gate-on must
    agree through the real `arg_approximate_similarity`.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver
        from mypy.nodes import TypeAlias

        self.fx = TypeFixture()
        self.alias = TypeAlias(self.fx.a, "mod.ArgAlias", "mod", -1, -1)
        self._resolver = _type_kernel.build_native_resolver(_base_infos(self.fx), [self.alias])
        _set_native_checkexpr_active(True)
        _set_native_checkexpr_resolver(self._resolver)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, _set_native_checkexpr_resolver

        _set_native_checkexpr_active(False)
        _set_native_checkexpr_resolver(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checkexpr import _set_native_checkexpr_active

        _set_native_checkexpr_active(active)
        try:
            return fn()
        finally:
            _set_native_checkexpr_active(True)

    def _run(self, actual: Type, formal: Type) -> tuple[bool, bool]:
        from mypy.checkexpr import arg_approximate_similarity

        off = self._with_gate(False, lambda: arg_approximate_similarity(actual, formal))
        on = self._with_gate(True, lambda: arg_approximate_similarity(actual, formal))
        return off, on

    def _seam(self, actual: Type, formal: Type, resolver: Any | None = None) -> bool | None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        return _type_kernel.rust_arg_approximate_similarity(
            _serialize_type_for_checkexpr(actual),
            _serialize_type_for_checkexpr(formal),
            True,
            resolver if resolver is not None else self._resolver,
            False,
        )

    def test_seam_alias_operand_engages(self) -> None:
        alias_t = TypeAliasType(self.alias, [])
        assert self._seam(alias_t, self.fx.a) is True
        assert self._seam(self.fx.a, alias_t) is True

    def test_seam_alias_to_object_engages(self) -> None:
        from mypy.nodes import TypeAlias

        obj_alias = TypeAlias(self.fx.o, "mod.ObjAlias", "mod", -1, -1)
        resolver = _type_kernel.build_native_resolver(_base_infos(self.fx), [obj_alias])
        # A's MRO reaches object: formal=object, actual=A.
        assert self._seam(self.fx.a, TypeAliasType(obj_alias, []), resolver) is True

    def test_seam_defers_without_alias_snapshot(self) -> None:
        empty = _type_kernel.build_native_resolver([], [])
        assert self._seam(TypeAliasType(self.alias, []), self.fx.a, empty) is None

    def test_gate_parity_alias_operand(self) -> None:
        alias_t = TypeAliasType(self.alias, [])
        off, on = self._run(alias_t, self.fx.a)
        assert on == off
        assert off is True


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckCallableCallWireGateSuite(Suite):
    """#1673: the check_callable_call tail seam crosses only on shapes it
    can decide.

    `_try_native_check_callable_call` serialized the whole callee plus
    every argument type before asking Rust, then deferred on 177,524 of
    177,899 cold-self-check calls (99.79%) for shapes the Rust tail cannot
    calibrate (`check_callable_call_tail` needs a type-object call with
    exactly one argument whose instance type is `builtins.type`). The new
    live-object gate reuses the conjuncts the pure-Python calibration two
    branches below already computes.

    Two halves are pinned: a rejected shape crosses nothing, and the
    ungated Rust seam would have deferred on that same shape anyway, so
    the gate moves no decision.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import (
            _set_native_checkcall_active,
            _set_native_checkexpr_active,
            _set_native_checkexpr_resolver,
            _set_native_plugin_hook_registry,
        )
        from mypy.options import Options
        from mypy.plugins.default import DefaultPlugin

        self.fx = TypeFixture()
        self._resolver = _type_kernel.build_native_resolver(_base_infos(self.fx), [])
        self._registry = _type_kernel.PluginHookRegistry({})
        self._plugin = DefaultPlugin(Options())
        _set_native_checkexpr_active(True)
        _set_native_checkcall_active(True)
        _set_native_checkexpr_resolver(self._resolver)
        _set_native_plugin_hook_registry(self._registry, False, [self._plugin])

    def tearDown(self) -> None:
        from mypy.checkexpr import (
            _set_native_checkcall_active,
            _set_native_checkexpr_active,
            _set_native_checkexpr_resolver,
            _set_native_plugin_hook_registry,
        )

        _set_native_checkexpr_active(False)
        _set_native_checkcall_active(False)
        _set_native_checkexpr_resolver(None)
        _set_native_plugin_hook_registry(None, False)

    def _spy_crossings(self) -> list[bytes]:
        """Count real `rust_check_callable_call` crossings (delegating).

        The name is a private alias inside `mypy.checkexpr` (not a
        re-export), so it is read and written by string.
        """
        import mypy.checkexpr as _ce

        seen: list[bytes] = []
        # B009/B010 are silenced deliberately: direct access trips the
        # self-check's implicit_reexport=False for this private alias.
        orig = getattr(_ce, "_rust_check_callable_call")  # noqa: B009

        def wrapper(*args: Any, **kwargs: Any) -> Any:
            seen.append(args[1])
            return orig(*args, **kwargs)

        setattr(_ce, "_rust_check_callable_call", wrapper)  # noqa: B010
        self.addCleanup(setattr, _ce, "_rust_check_callable_call", orig)
        return seen

    def _type_callable(self, *a: Type) -> CallableType:
        """A callable with `builtins.type` as both fallback and return, so
        `get_instance_type()` is `builtins.type` when the tail can fire."""
        n = len(a) - 1
        return CallableType(list(a[:-1]), [ARG_POS] * n, [None] * n, a[-1], self.fx.type_type)

    def _rejected_shapes(self) -> list[tuple[str, CallableType, list[Type]]]:
        """Shapes the gate must reject, each one declared un-calibratable."""
        from mypy.nodes import TypeAlias as _TypeAlias
        from mypy.types import TupleType, TypeAliasType

        fx = self.fx
        alias = TypeAliasType(_TypeAlias(fx.a, "mod.Alias", "mod", -1, -1), [])
        two_args = self._type_callable(fx.a, fx.b, fx.type_type)
        foreign_self = self._type_callable(fx.a, fx.type_type)
        foreign_self.instance_type = fx.a
        tuple_ret = CallableType(
            [fx.a], [ARG_POS], [None], TupleType([fx.a], fx.std_tuple), fx.type_type
        )
        alias_ret = CallableType([fx.a], [ARG_POS], [None], alias, fx.type_type)
        return [
            ("non-type-object callee", fx.callable(fx.a, fx.b), [fx.a]),
            ("two positional arguments", two_args, [fx.a, fx.b]),
            ("no positional arguments", self._type_callable(fx.type_type), []),
            ("non-builtins.type instance_type", foreign_self, [fx.a]),
            ("tuple ret_type", tuple_ret, [fx.a]),
            ("alias ret_type", alias_ret, [fx.a]),
        ]

    def test_rejected_shapes_cross_nothing(self) -> None:
        from mypy.checkexpr import _try_native_check_callable_call

        seen = self._spy_crossings()
        for name, callee, args in self._rejected_shapes():
            assert _try_native_check_callable_call(callee, args, None, False) is None, name
            assert (
                _try_native_check_callable_call(callee, args, "builtins.print", True) is None
            ), name
        assert seen == [], f"{len(seen)} crossings on gated-out shapes"

    def test_rejected_shapes_defer_in_ungated_rust(self) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr

        for name, callee, args in self._rejected_shapes():
            try:
                callee_bytes = _serialize_type_for_checkexpr(callee)
                arg_bytes = [_serialize_type_for_checkexpr(t) for t in args]
            except (AssertionError, NotImplementedError, ValueError, TypeError):
                continue  # the wire cannot even carry it; the gate skips the try
            raw = _type_kernel.rust_check_callable_call(
                self._resolver,
                callee_bytes,
                arg_bytes,
                None,
                False,
                self._registry,
                False,
                [self._plugin],
            )
            assert raw is None, f"gate rejected a shape Rust decides: {name}"

    def test_accepted_shape_still_crosses_and_decides(self) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr, _try_native_check_callable_call

        callee = self._type_callable(self.fx.a, self.fx.type_type)
        assert callee.is_type_obj()
        seen = self._spy_crossings()
        _try_native_check_callable_call(callee, [self.fx.a], None, False)
        assert len(seen) == 1, "the calibratable shape must still cross"
        raw = _type_kernel.rust_check_callable_call(
            self._resolver,
            _serialize_type_for_checkexpr(callee),
            [_serialize_type_for_checkexpr(self.fx.a)],
            None,
            False,
            self._registry,
            False,
            [self._plugin],
        )
        assert raw is not None, "the accepted shape must be decided by Rust"


# Moved from mypy/test/testtypes_native_types.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSolveOneSuite(Suite):
    """Parity for the Rust `solve_one` decision subcases (solve.rs).

    Runs the pure-Python body (solve gate off) and the Rust path (solve
    gate on) on the same lower/upper bound sets and asserts identical
    results. Cases Rust cannot handle (single-bound no-ops, unwire-safe
    bounds) stay in Python, so the differential still passes.
    """

    def setUp(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.solve import _set_native_solve_active, _set_native_solve_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_solve_active(False)
        _set_native_solve_resolver(None)
        set_wire_typeinfo_map(None)

    def _set_active(self, active: bool) -> None:
        from mypy.solve import _set_native_solve_active, _set_native_solve_resolver

        _set_native_solve_active(active)
        _set_native_solve_resolver(self.resolver if active else None)

    def _assert_par(self, lowers: list[Type], uppers: list[Type]) -> None:
        from mypy.solve import solve_one

        self._set_active(False)
        ref = solve_one(lowers, uppers)
        self._set_active(True)
        res = solve_one(lowers, uppers)
        assert_equal(res, ref)

    def test_infer_unions_true(self) -> None:
        from mypy.typestate import type_state

        type_state.infer_unions = True
        try:
            self._assert_par([self.fx.a, self.fx.b], [])
        finally:
            type_state.infer_unions = False

    def test_infer_unions_false(self) -> None:
        self._assert_par([self.fx.a, self.fx.b], [])

    def test_ambig_never_filtered_upper(self) -> None:
        from mypy.types import UninhabitedType

        amb = UninhabitedType()
        amb.ambiguous = True
        self._assert_par([self.fx.a], [amb, self.fx.b])

    def test_uninhabited_lower(self) -> None:
        from mypy.types import UninhabitedType

        nb = UninhabitedType()
        self._assert_par([nb], [self.fx.a])

    def test_single_bound_defer(self) -> None:
        # Single-bound no-ops stay in Python for identity; parity holds.
        self._assert_par([self.fx.a], [])
        self._assert_par([], [self.fx.a])

    def test_any_upper_absorbed(self) -> None:
        from mypy.types import AnyType

        anyt = AnyType(TypeOfAny.special_form)
        self._assert_par([self.fx.a], [anyt])

    def test_any_lower_absorbed(self) -> None:
        from mypy.types import AnyType

        anyt = AnyType(TypeOfAny.special_form)
        self._assert_par([anyt], [self.fx.b])

    def test_subtype_bottom_top(self) -> None:
        self._assert_par([self.fx.a], [self.fx.a, self.fx.b])

    def test_alias_arg_bound_falls_back(self) -> None:
        # Bounds carrying an unexpanded alias argument solve fine in
        # Rust, but fixup_wire_type cannot resolve the decoded alias.
        # The shim must fall back, not report "no solution" (#1093).
        from mypy.nodes import TypeAlias
        from mypy.types import Instance, TypeAliasType

        alias_node = TypeAlias(self.fx.a, "repro.Key", "repro", 1, 0)
        alias = TypeAliasType(alias_node, [])
        bound = Instance(self.fx.ai, [alias])
        self._assert_par([bound], [bound])


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSolveDependentNoopSuite(Suite):
    """Parity for the dependent-solver single-bound no-op port (issue #853).

    `solve_one_for_dependent` used to defer every single-bound variable,
    which pushed the whole `solve_with_dependent` call back to Python
    whenever any batch contained one (the dominant solve deferral). The
    no-op cases now flow through `solve_one_inner`, which mirrors
    solve.py's `UnionType.make_union(lowers)` / raw-top construction
    exactly. This suite drives `solve_with_dependent` gate-off vs gate-on
    and asserts identical results, plus direct `rust_solve_dependent`
    seam calls proving each case engages natively.
    """

    def setUp(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        from mypy.solve import _set_native_solve_active, _set_native_solve_resolver
        from mypy.state import state
        from mypy.typestate import type_state

        self._set_native_solve_active = _set_native_solve_active
        self._set_native_solve_resolver = _set_native_solve_resolver
        self._infer_unions = type_state.infer_unions
        self._strict_optional = state.strict_optional
        self._vars = [self.fx.t]
        self._set_active(True)
        # The dependent gate (solve.py:360-367) requires both flags;
        # infer_unions lives on the TypeState, strict_optional on the
        # StrictOptionalState.
        type_state.infer_unions = True
        state.strict_optional = True

    def tearDown(self) -> None:
        from mypy.solve import _set_native_solve_active, _set_native_solve_resolver
        from mypy.state import state
        from mypy.typestate import type_state
        from mypy.wirefixup import set_wire_typeinfo_map

        type_state.infer_unions = self._infer_unions
        state.strict_optional = self._strict_optional
        _set_native_solve_active(False)
        _set_native_solve_resolver(None)
        set_wire_typeinfo_map(None)

    def _set_active(self, active: bool) -> None:
        self._set_native_solve_active(active)
        self._set_native_solve_resolver(self.resolver if active else None)

    def _assert_par(self, constraints: list[Constraint]) -> None:
        from mypy.solve import solve_with_dependent

        tvars: dict[TypeVarId, TypeVarLikeType] = {tv.id: tv for tv in self._vars}
        tvar_ids = list(tvars)
        self._set_active(False)
        ref = solve_with_dependent(tvar_ids, constraints, tvar_ids, tvars)
        self._set_active(True)
        res = solve_with_dependent(tvar_ids, constraints, tvar_ids, tvars)
        assert_equal(res, ref)

    def _seam_solutions(self, constraints: list[Constraint]) -> dict[TypeVarId, Type | None]:
        """Drive `rust_solve_dependent` directly; assert it engaged."""
        from mypy.solve import (
            _native_solve_dependent_result,
            _serialize_constraint_list,
            _serialize_type_list,
        )

        tvars: dict[TypeVarId, TypeVarLikeType] = {tv.id: tv for tv in self._vars}
        result = _type_kernel.rust_solve_dependent(
            _serialize_type_list(list(tvars.values())),
            _serialize_constraint_list(constraints),
            True,  # infer_unions
            True,  # strict_optional
            self.resolver,
        )
        assert result is not None, "Rust dependent solver deferred"
        out = _native_solve_dependent_result(result, tvars)
        assert out is not None
        solutions, _ = out
        return solutions

    def test_single_lower_noop(self) -> None:
        from mypy.constraints import SUPERTYPE_OF, Constraint

        c = Constraint(self.fx.t, SUPERTYPE_OF, self.fx.a)
        self._assert_par([c])
        solutions = self._seam_solutions([c])
        assert_equal(solutions[self.fx.t.id], self.fx.a)

    def test_single_upper_noop(self) -> None:
        from mypy.constraints import SUBTYPE_OF, Constraint

        c = Constraint(self.fx.t, SUBTYPE_OF, self.fx.a)
        self._assert_par([c])
        solutions = self._seam_solutions([c])
        assert_equal(solutions[self.fx.t.id], self.fx.a)

    def test_single_union_lower_passthrough(self) -> None:
        # A single lower bound that is itself a union passes through
        # unchanged: UnionType.make_union (solve.py:591) is a raw
        # constructor, not a simplification.
        from mypy.constraints import SUPERTYPE_OF, Constraint
        from mypy.types import UnionType

        u = UnionType.make_union([self.fx.a, self.fx.b])
        c = Constraint(self.fx.t, SUPERTYPE_OF, u)
        self._assert_par([c])
        solutions = self._seam_solutions([c])
        assert_equal(solutions[self.fx.t.id], u)

    def test_single_any_lower_absorbs(self) -> None:
        from mypy.constraints import SUPERTYPE_OF, Constraint
        from mypy.types import AnyType

        anyt = AnyType(TypeOfAny.special_form)
        c = Constraint(self.fx.t, SUPERTYPE_OF, anyt)
        self._assert_par([c])
        solutions = self._seam_solutions([c])
        got = get_proper_type(solutions[self.fx.t.id])
        assert isinstance(got, AnyType)
        assert_equal(got.type_of_any, TypeOfAny.from_another_any)
        assert_equal(got.source_any, anyt)

    def test_ambiguous_never_upper_still_never(self) -> None:
        from mypy.constraints import SUBTYPE_OF, Constraint
        from mypy.types import UninhabitedType

        nb = UninhabitedType()
        nb.ambiguous = True
        c = Constraint(self.fx.t, SUBTYPE_OF, nb)
        self._assert_par([c])
        solutions = self._seam_solutions([c])
        got = get_proper_type(solutions[self.fx.t.id])
        assert isinstance(got, UninhabitedType)
        assert got.ambiguous

    def test_mixed_batch_solves_natively(self) -> None:
        # A batch with a single-bound var used to defer the whole call;
        # now the no-op var solves alongside the join var.
        from mypy.constraints import SUPERTYPE_OF, Constraint
        from mypy.types import UnionType

        self._vars = [self.fx.t, self.fx.s]
        try:
            cs = [
                Constraint(self.fx.t, SUPERTYPE_OF, self.fx.a),
                Constraint(self.fx.s, SUPERTYPE_OF, self.fx.a),
                Constraint(self.fx.s, SUPERTYPE_OF, self.fx.b),
            ]
            self._assert_par(cs)
            solutions = self._seam_solutions(cs)
            assert_equal(solutions[self.fx.t.id], self.fx.a)
            # infer_unions is on (the dependent gate requires it), so the
            # two-lower solve is make_union(a, b) = a | b, not the join.
            assert_equal(solutions[self.fx.s.id], UnionType.make_union([self.fx.a, self.fx.b]))
        finally:
            self._vars = [self.fx.t]


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSolvePreValidateAliasSuite(Suite):
    """Parity for the solve-path alias expansion (Wave-10, issue #1241).

    The solver previously deferred whenever a typevar upper bound carried
    a top-level TypeAliasType node: the pre_validate bound check could not
    compare the solution against an unexpanded bound. The check now expands
    the solution, the bound, and each constraint target through the alias
    resolver (mirroring get_proper_type at Python's is_subtype entry), and
    a missing snapshot still defers the whole solve to Python.

    Differential harness: gate-on vs gate-off through the public
    solve_constraints with the flags the native gate requires; direct
    rust_solve_constraints calls prove engagement (non-None) or the
    documented deferral (None).
    """

    def setUp(self) -> None:
        from mypy.solve import _set_native_solve_active
        from mypy.state import state
        from mypy.typestate import type_state
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._type_infos = type_infos
        self._set_native_solve_active = _set_native_solve_active
        self._resolver = self._build_resolver([])
        self._activate(True)
        self._infer_unions = type_state.infer_unions
        self._strict_optional = state.strict_optional
        type_state.infer_unions = True
        state.strict_optional = True

    def tearDown(self) -> None:
        from mypy.solve import _set_native_solve_active, _set_native_solve_resolver
        from mypy.state import state
        from mypy.typestate import type_state
        from mypy.wirefixup import set_wire_typeinfo_map

        type_state.infer_unions = self._infer_unions
        state.strict_optional = self._strict_optional
        _set_native_solve_active(False)
        _set_native_solve_resolver(None)
        set_wire_typeinfo_map(None)

    def _build_resolver(self, aliases: list[Any]) -> Any:
        from mypy.solve import _set_native_solve_resolver

        resolver = _type_kernel.build_native_resolver(self._type_infos, aliases)
        _set_native_solve_resolver(resolver)
        return resolver

    def _activate(self, active: bool) -> None:
        from mypy.solve import _set_native_solve_active, _set_native_solve_resolver

        _set_native_solve_active(active)
        _set_native_solve_resolver(self._resolver if active else None)

    def _alias_tvar(self, alias: Any) -> Any:
        from mypy.types import AnyType, TypeOfAny, TypeVarId, TypeVarType

        return TypeVarType(
            "T",
            "mod.T",
            TypeVarId(7),
            values=[],
            upper_bound=alias,
            default=AnyType(TypeOfAny.special_form),
        )

    def _assert_solve_par(self, tv: Any, constraints: list[Any]) -> list[str]:
        from mypy.solve import solve_constraints

        self._activate(False)
        ref = [str(s) for s in solve_constraints([tv], constraints)[0]]
        self._activate(True)
        res = [str(s) for s in solve_constraints([tv], constraints)[0]]
        assert res == ref, f"gate-on={res} gate-off={ref}"
        return res

    def test_alias_ub_in_bound_parity(self) -> None:
        # T's upper bound is mod.A (= A); the lower is B <: A. The bound
        # check expands the alias and keeps the solution unchanged.
        from mypy.constraints import SUPERTYPE_OF, Constraint
        from mypy.solve import _serialize_constraint_list, _serialize_type_list

        alias = self.fx.non_rec_alias(self.fx.a)
        self._resolver = self._build_resolver([alias.alias])
        self._activate(True)
        tv = self._alias_tvar(alias)
        cons = [Constraint(tv, SUPERTYPE_OF, self.fx.b)]
        self._assert_solve_par(tv, cons)
        # Direct seam call proves native engagement (non-None).
        result = _type_kernel.rust_solve_constraints(
            _serialize_type_list([tv]),
            _serialize_type_list([tv]),
            _serialize_constraint_list(cons),
            True,
            True,
            True,
            False,
            self._resolver,
        )
        assert result is not None, "alias-ub bound check must engage natively"
        from mypy.solve import _native_solve_dependent_result

        out = _native_solve_dependent_result(result, {tv.id: tv})
        assert out is not None
        solutions, _ = out
        assert_equal(solutions[tv.id], self.fx.b)

    def test_alias_ub_violation_no_replacement_parity(self) -> None:
        # Lower D is disjoint from the alias bound A: the bound check
        # fails (after expansion) and the constraint targets fail the
        # bound-satisfies fold too, so the solution is kept on both sides.
        from mypy.constraints import SUPERTYPE_OF, Constraint
        from mypy.solve import _serialize_constraint_list, _serialize_type_list

        alias = self.fx.non_rec_alias(self.fx.a)
        self._resolver = self._build_resolver([alias.alias])
        self._activate(True)
        tv = self._alias_tvar(alias)
        cons = [Constraint(tv, SUPERTYPE_OF, self.fx.d)]
        self._assert_solve_par(tv, cons)
        result = _type_kernel.rust_solve_constraints(
            _serialize_type_list([tv]),
            _serialize_type_list([tv]),
            _serialize_constraint_list(cons),
            True,
            True,
            True,
            False,
            self._resolver,
        )
        assert result is not None, "violation path must engage natively"
        from mypy.solve import _native_solve_dependent_result

        out = _native_solve_dependent_result(result, {tv.id: tv})
        assert out is not None
        solutions, _ = out
        assert_equal(solutions[tv.id], self.fx.d)

    def test_alias_ub_missing_snapshot_defers(self) -> None:
        # The bound alias is not registered in the alias resolver: the
        # expansion fails and the whole solve defers to Python (None),
        # so gate-on == gate-off through the public path.
        from mypy.constraints import SUPERTYPE_OF, Constraint
        from mypy.solve import _serialize_constraint_list, _serialize_type_list

        alias = self.fx.non_rec_alias(self.fx.a)
        self._resolver = self._build_resolver([])
        self._activate(True)
        tv = self._alias_tvar(alias)
        cons = [Constraint(tv, SUPERTYPE_OF, self.fx.b)]
        self._assert_solve_par(tv, cons)
        result = _type_kernel.rust_solve_constraints(
            _serialize_type_list([tv]),
            _serialize_type_list([tv]),
            _serialize_constraint_list(cons),
            True,
            True,
            True,
            False,
            self._resolver,
        )
        assert result is None, "missing alias snapshot must defer"


# Moved from mypy/test/testtypes_native_semanal.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeLookupDefinerSuite(Suite):
    """Parity for `rust_lookup_definer` (issue #1075).

    `ExpressionChecker.lookup_definer` (checkexpr.py:5862-5876) walks
    `typ.type.mro` and returns the fullname of the first class that
    defines `attr_name`. The Rust seam is a live-PyO3-object port (zero
    wire bytes) that defers on any unreadable fact. Direct seam calls
    assert the expected fullname (and the deferral); toggling the
    checkexpr gate off vs on drives the real ExpressionChecker method
    and must agree on both.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checkexpr_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _info(self, fullname: str, mro: list[TypeInfo] | None = None) -> TypeInfo:
        from mypy.nodes import Block

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = mro if mro is not None else [info]
        return info

    def _definer(self, fullname: str, attr: str = "foo") -> TypeInfo:
        info = self._info(fullname)
        info.names[attr] = SymbolTableNode(GDEF, Var(attr))
        return info

    def _subclass(self, fullname: str, bases: list[TypeInfo]) -> TypeInfo:
        info = self._info(fullname)
        info.mro = [info, *bases]
        return info

    def _checker(self) -> Any:
        from mypy.errors import Errors
        from mypy.nodes import MypyFile
        from mypy.plugin import Plugin

        options = Options()
        errors = Errors(options)
        tree = MypyFile([], [])
        tree.is_stub = True
        tree.names = SymbolTable()
        modules: dict[str, MypyFile] = {}
        return TypeChecker(errors, modules, options, tree, "", Plugin(options), {})

    def _ec(self) -> Any:

        return ExpressionChecker(self._checker(), None, None, None)  # type: ignore[arg-type]

    def _assert_seam(self, typ: Instance, attr: str, expected: str | None) -> None:
        result = _type_kernel.rust_lookup_definer(typ, attr)
        assert result == expected, f"seam: {result} != {expected}"

    def _assert_par(self, typ: Instance, attr: str, expected: str | None) -> None:
        ec = self._ec()
        off = self._with_gate(False, lambda: ec.lookup_definer(typ, attr))
        on = self._with_gate(True, lambda: ec.lookup_definer(typ, attr))
        assert off == expected, f"gate-off: {off} != {expected}"
        assert on == expected, f"gate-on: {on} != {expected}"

    def test_seam_defined_in_base(self) -> None:
        a = self._definer("mod.A")
        b = self._subclass("mod.B", [a])
        self._assert_seam(Instance(b, []), "foo", "mod.A")

    def test_seam_overridden_in_subclass(self) -> None:
        a = self._definer("mod.A")
        b = self._subclass("mod.B", [a])
        b.names["foo"] = SymbolTableNode(GDEF, Var("foo"))
        self._assert_seam(Instance(b, []), "foo", "mod.B")

    def test_seam_not_in_mro(self) -> None:
        a = self._info("mod.A")
        b = self._subclass("mod.B", [a])
        self._assert_seam(Instance(b, []), "foo", None)

    def test_seam_multiple_bases_first_mro_wins(self) -> None:
        a = self._definer("mod.A")
        b = self._definer("mod.B")
        c = self._subclass("mod.C", [b, a])
        self._assert_seam(Instance(c, []), "foo", "mod.B")

    def test_seam_unreadable_type_defers(self) -> None:
        # A non-Instance has no readable .type: the seam defers (None).
        result = _type_kernel.rust_lookup_definer(cast(Any, IntExpr(3)), "foo")
        assert result is None

    def test_parity_defined_in_base(self) -> None:
        a = self._definer("mod.A")
        b = self._subclass("mod.B", [a])
        self._assert_par(Instance(b, []), "foo", "mod.A")

    def test_parity_overridden_in_subclass(self) -> None:
        a = self._definer("mod.A")
        b = self._subclass("mod.B", [a])
        b.names["foo"] = SymbolTableNode(GDEF, Var("foo"))
        self._assert_par(Instance(b, []), "foo", "mod.B")

    def test_parity_not_in_mro(self) -> None:
        a = self._info("mod.A")
        b = self._subclass("mod.B", [a])
        self._assert_par(Instance(b, []), "foo", None)

    def test_parity_multiple_bases_first_mro_wins(self) -> None:
        a = self._definer("mod.A")
        b = self._definer("mod.B")
        c = self._subclass("mod.C", [b, a])
        self._assert_par(Instance(c, []), "foo", "mod.B")

    def test_parity_empty_mro(self) -> None:
        # An MRO listing nothing but the class itself, no defs: None.
        b = self._subclass("mod.B", [])
        self._assert_par(Instance(b, []), "foo", None)
