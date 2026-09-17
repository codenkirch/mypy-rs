"""Native engagement suites for the checker area (`mypy/checker.py` and `mypy/checkpattern.py`).

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

from collections.abc import Callable
from types import SimpleNamespace
from typing import Any, cast
from unittest import skipUnless

from mypy.checker import TypeChecker, TypeMap
from mypy.checker_shared import TypeRange
from mypy.nodes import (
    ARG_POS,
    ARG_STAR,
    CONTRAVARIANT,
    COVARIANT,
    GDEF,
    INVARIANT,
    MDEF,
    ArgKind,
    Argument,
    AssertStmt,
    AssignmentStmt,
    Block,
    CallExpr,
    ClassDef,
    ComparisonExpr,
    Context,
    Decorator,
    EllipsisExpr,
    Expression,
    ExpressionStmt,
    FuncBase,
    FuncDef,
    FuncItem,
    IndexExpr,
    IntExpr,
    ListExpr,
    Lvalue,
    MemberExpr,
    MypyFile,
    NameExpr,
    OverloadedFuncDef,
    PassStmt,
    RaiseStmt,
    RefExpr,
    ReturnStmt,
    StarExpr,
    Statement,
    StrExpr,
    SymbolTable,
    SymbolTableNode,
    TupleExpr,
    TypeAlias,
    TypeInfo,
    Var,
)
from mypy.options import Options
from mypy.patterns import ClassPattern
from mypy.state import state
from mypy.subtypes import is_subtype
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import (
    _HAS_TYPE_KERNEL,
    _NATIVE_WIRE_ENABLED,
    T,
    _base_infos,
    _BrokenAttrInfo,
    _BrokenFinalValueVar,
    _BrokenFinalVar,
    _FakeScope,
    _h1d_module,
    _h1d_names,
    _H1dBinderStub,
    _H1dCheckerStub,
    _H1dScopeCheckerStub,
    _is_type_info,
    _make_getattr_sig,
    _run_check,
    expect_fail_check,
)
from mypy.test.typefixture import TypeFixture
from mypy.types import (
    AnyType,
    CallableType,
    DeletedType,
    FunctionLike,
    Instance,
    LiteralType,
    NoneType,
    Overloaded,
    PartialType,
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
)


# Moved from mypy/test/testtypes_native_checker.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeEqualityValueInfoSuite(Suite):
    """Parity for the Rust `equality_value_info` port (mypy.checker).

    `equality_value_info` collects the value-equality domains a type
    participates in; `is_equality_ambiguous_for_narrowing` uses it to
    detect cross-domain equality (IntEnum vs int, StrEnum vs str). The
    Rust port folds `combine_equality_value_info` into the recursion and
    reads the resolver-required metadata (mro, is_enum) from the TypeInfo
    snapshot. Toggling the checker gate off (pure Python) and on (Rust)
    must produce identical results, and a direct seam call proves the Rust
    function engages rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver

        self.fx = TypeFixture()
        self.int_info = self.fx.make_type_info("builtins.int")
        # An IntEnum: is_enum set, mro through builtins.int -> numeric domain.
        self.enum_info = self.fx.make_type_info("mod.Color", mro=[self.int_info, self.fx.oi])
        self.enum_info.is_enum = True
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        # Make sure the str/object TypeInfos are snapshotted: the `dir(fx)`
        # scan above only picks `*i` attrs and `str_type_info` /
        # `bool_type_info` do not end in "i".
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        type_infos.extend([self.int_info, self.enum_info])
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_checker_active(True)
        _set_native_checker_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def _normalize(
        self, info: object
    ) -> tuple[bool, frozenset[tuple[str, frozenset[str], frozenset[str]]]]:
        from mypy.checker import EqualityValueInfo

        assert isinstance(info, EqualityValueInfo)
        return (
            info.is_top,
            frozenset(
                (domain, frozenset(domain_info.type_names), frozenset(domain_info.enum_type_names))
                for domain, domain_info in info.domains.items()
            ),
        )

    def _assert_par(self, typ: Type) -> None:
        from mypy.checker import equality_value_info

        off = self._normalize(self._with_gate(False, lambda: equality_value_info(typ)))
        on = self._normalize(self._with_gate(True, lambda: equality_value_info(typ)))
        assert_equal(on, off, f"equality_value_info parity {typ}")

    def _assert_engages(self, typ: Type) -> None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_equality_value_info(
            _serialize_type_for_checker(typ), self.resolver
        )
        assert result is not None, f"Rust equality_value_info did not engage for {typ}"

    def test_str_instance(self) -> None:
        # Instance of builtins.str -> open str domain, no enum type names.
        from mypy.checker import equality_value_info

        s = Instance(self.fx.str_type_info, [])
        self._assert_par(s)
        result = self._normalize(equality_value_info(s))
        assert_equal(
            result,
            (False, frozenset({("builtins.str", frozenset({"builtins.str"}), frozenset())})),
        )
        self._assert_engages(s)

    def test_int_enum_literal(self) -> None:
        # An IntEnum literal's fallback is the enum Instance; the enum's mro
        # hits builtins.int -> numeric domain with the enum name in both sets.
        from mypy.checker import equality_value_info

        enum_inst = Instance(self.enum_info, [])
        lit = LiteralType(1, enum_inst)
        self._assert_par(lit)
        result = self._normalize(equality_value_info(lit))
        assert_equal(
            result,
            (
                False,
                frozenset(
                    {("builtins.numeric", frozenset({"mod.Color"}), frozenset({"mod.Color"}))}
                ),
            ),
        )
        self._assert_engages(lit)

    def test_union(self) -> None:
        # combine over the member infos: str + plain non-enum A.
        u = UnionType([Instance(self.fx.str_type_info, []), self.fx.a])
        self._assert_par(u)
        self._assert_engages(u)

    def test_typevar_with_values(self) -> None:
        # TypeVar.values non-empty: combine over the values.
        tv = TypeVarType(
            "T",
            "T",
            TypeVarId(1),
            [Instance(self.fx.str_type_info, [])],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        self._assert_par(tv)
        self._assert_engages(tv)

    def test_typevar_upper_bound(self) -> None:
        # TypeVar.values empty: recurse on the upper_bound (object -> top).
        tv = TypeVarType(
            "T", "T", TypeVarId(1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )
        self._assert_par(tv)
        self._assert_engages(tv)

    def test_any(self) -> None:
        self._assert_par(self.fx.anyt)
        self._assert_engages(self.fx.anyt)

    def test_object_instance(self) -> None:
        # builtins.object is a top-like value info.
        self._assert_par(self.fx.o)
        self._assert_engages(self.fx.o)

    def test_plain_instance(self) -> None:
        # A has no value-equality domain members in its mro: no domains.
        self._assert_par(self.fx.a)
        self._assert_engages(self.fx.a)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeGetPropertyTypeSuite(Suite):
    """Parity for the Rust `get_property_type` port (mypy.checker).

    `get_property_type` maps a `CallableType` to the proper type of its
    `ret_type`, an `Overloaded` to the proper `items[0].ret_type`, and any
    other `ProperType` to itself. The Rust port reads the live object graph
    (no wire round-trip) and returns the live `ProperType`. Toggling the
    checker-stmts gate off (pure Python) and on (Rust seam) must produce
    identical `str(...)` output, and a direct seam call proves the Rust
    function engages rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_stmts_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_stmts_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _assert_par(self, t: ProperType, expected: str) -> None:
        from mypy.checker import get_property_type

        off = self._with_gate(False, lambda: get_property_type(t))
        on = self._with_gate(True, lambda: get_property_type(t))
        assert_equal(str(on), str(off), f"get_property_type parity {t}")
        result = self._with_gate(True, lambda: get_property_type(t))
        assert_equal(str(result), expected)
        assert isinstance(result, ProperType)

    def _assert_engages(self, t: ProperType) -> None:
        result = _type_kernel.rust_get_property_type(t)
        assert result is not None, f"Rust get_property_type did not engage for {t}"

    def test_callable_returns_ret_type(self) -> None:
        callable_t = self.fx.callable(self.fx.str_type, self.fx.a)
        self._assert_par(callable_t, "A")
        self._assert_engages(callable_t)

    def test_overloaded_returns_first_ret_type(self) -> None:
        overloaded = Overloaded(
            [self.fx.callable(self.fx.str_type, self.fx.a), self.fx.callable(self.fx.b, self.fx.b)]
        )
        self._assert_par(overloaded, "A")
        self._assert_engages(overloaded)

    def test_plain_instance_unchanged(self) -> None:
        plain = self.fx.a
        self._assert_par(plain, "A")
        self._assert_engages(plain)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeOverloadNeverSuite(Suite):
    """Parity for the Rust overload argument-prefix ports (mypy.checker).

    `overload_can_never_match`, `is_more_general_arg_prefix` and
    `is_same_arg_prefix` rest on the native `is_callable_compatible` engine
    with the `is_more_precise` / `is_proper_subtype` / `is_same_type`
    predicates. The Rust fast path covers the non-generic
    Callable-vs-Callable branch only: a generic (non-empty `variables`)
    operand defers, because Python unifies a generic left via
    `unify_generic_callable` before the parameter check, and
    `is_more_general_arg_prefix`'s Overloaded branch defers because the
    FunctionLike zip stays in Python. Toggling the checker gate off (pure
    Python) and on (Rust seam) must produce identical booleans, and a direct
    seam call proves the Rust function engages rather than silently deferring
    on the fast path.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active = _set_native_checker_active
        self._set_resolver = _set_native_checker_resolver
        _set_native_checker_active(True)
        _set_native_checker_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _callable(
        self,
        args: list[Type],
        arg_kinds: list[ArgKind],
        ret: Type,
        variables: list[TypeVarLikeType] | None = None,
    ) -> CallableType:
        return CallableType(
            args, arg_kinds, [None] * len(args), ret, self.fx.function, variables=variables or []
        )

    def _par_bool(self, fn: Callable[[], bool], label: str) -> bool:
        off = self._with_gate(False, fn)
        assert isinstance(off, bool)
        on = self._with_gate(True, fn)
        assert isinstance(on, bool)
        assert_equal(on, off, f"{label} parity")
        return on

    def test_overload_never_match_broader(self) -> None:
        from mypy.checker import overload_can_never_match

        broader = self._callable([self.fx.o], [ARG_POS], self.fx.o)
        narrower = self._callable([self.fx.b], [ARG_POS], self.fx.o)
        result = self._par_bool(
            lambda: overload_can_never_match(broader, narrower), "overload_can_never_match"
        )
        # (object) is strictly broader than (B): the (B) overload can never
        # be matched.
        assert_equal(result, True)
        self._assert_overload_engages(broader, narrower)

    def test_overload_never_match_not_broader(self) -> None:
        from mypy.checker import overload_can_never_match

        narrower = self._callable([self.fx.b], [ARG_POS], self.fx.o)
        broader = self._callable([self.fx.o], [ARG_POS], self.fx.o)
        result = self._par_bool(
            lambda: overload_can_never_match(narrower, broader), "overload_can_never_match"
        )
        assert_equal(result, False)

    def test_overload_never_match_generic_defers(self) -> None:
        from mypy.checker import overload_can_never_match

        # signature carries a type var: the erase+expand / unify path defers
        # to Python, but on==off must still hold.
        sig = self._callable([self.fx.t], [ARG_POS], self.fx.o, variables=[self.fx.t])
        other = self._callable([self.fx.o], [ARG_POS], self.fx.o)
        self._par_bool(lambda: overload_can_never_match(sig, other), "overload_can_never_match")
        # The raw seam must defer (return None) on the generic operand.
        from mypy.checker import _serialize_type_for_checker

        raw = _type_kernel.rust_overload_can_never_match(
            _serialize_type_for_checker(sig),
            _serialize_type_for_checker(other),
            state.strict_optional,
            self.resolver,
        )
        assert raw is None, "Rust overload_can_never_match should defer on generic signature"

    def test_is_more_general_arg_prefix_wider(self) -> None:
        from mypy.checker import is_more_general_arg_prefix

        wider = self._callable([self.fx.o, self.fx.o], [ARG_POS, ARG_POS], self.fx.o)
        narrower = self._callable([self.fx.b, self.fx.a], [ARG_POS, ARG_POS], self.fx.o)
        result = self._par_bool(
            lambda: is_more_general_arg_prefix(wider, narrower), "is_more_general_arg_prefix"
        )
        assert_equal(result, True)
        self._assert_prefix_engages(wider, narrower)

    def test_is_more_general_arg_prefix_narrower(self) -> None:
        from mypy.checker import is_more_general_arg_prefix

        wider = self._callable([self.fx.o, self.fx.o], [ARG_POS, ARG_POS], self.fx.o)
        narrower = self._callable([self.fx.b, self.fx.a], [ARG_POS, ARG_POS], self.fx.o)
        result = self._par_bool(
            lambda: is_more_general_arg_prefix(narrower, wider), "is_more_general_arg_prefix"
        )
        assert_equal(result, False)

    def test_is_more_general_arg_prefix_overloaded_defers(self) -> None:
        from mypy.checker import is_more_general_arg_prefix

        t_items = [
            self._callable([self.fx.o], [ARG_POS], self.fx.o),
            self._callable([self.fx.a], [ARG_POS], self.fx.o),
        ]
        s_items = [
            self._callable([self.fx.b], [ARG_POS], self.fx.o),
            self._callable([self.fx.a], [ARG_POS], self.fx.o),
        ]
        t = Overloaded(t_items)
        s = Overloaded(s_items)
        # The Overloaded (FunctionLike) branch stays in Python; on==off holds
        # and the raw Callable-vs-Callable seam defers on an Overloaded.
        result = self._par_bool(
            lambda: is_more_general_arg_prefix(t, s), "is_more_general_arg_prefix"
        )
        assert isinstance(result, bool)

    def test_is_same_arg_prefix_parity(self) -> None:
        from mypy.checker import _serialize_type_for_checker, is_same_arg_prefix

        same = self._callable([self.fx.b], [ARG_POS], self.fx.o)
        other = self._callable([self.fx.o], [ARG_POS], self.fx.o)

        def raw(t: CallableType, s: CallableType) -> bool | None:
            return cast(
                "bool | None",
                _type_kernel.rust_is_same_arg_prefix(
                    _serialize_type_for_checker(t),
                    _serialize_type_for_checker(s),
                    state.strict_optional,
                    self.resolver,
                ),
            )

        def via_fn(t: CallableType, s: CallableType) -> bool:
            return is_same_arg_prefix(t, s)

        # Equal args -> True, with the covariant is_same_type check.
        assert_equal(self._par_bool(lambda: via_fn(same, same), "is_same_arg_prefix"), True)
        assert raw(same, same) is not None, "Rust is_same_arg_prefix did not engage"
        # Different args -> False.
        assert_equal(self._par_bool(lambda: via_fn(same, other), "is_same_arg_prefix"), False)
        assert raw(same, other) is not None, "Rust is_same_arg_prefix did not engage"

    def _assert_overload_engages(self, sig: CallableType, other: CallableType) -> None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_overload_can_never_match(
            _serialize_type_for_checker(sig),
            _serialize_type_for_checker(other),
            state.strict_optional,
            self.resolver,
        )
        assert result is not None, "Rust overload_can_never_match did not engage"

    def _assert_prefix_engages(self, t: CallableType, s: CallableType) -> None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_is_more_general_arg_prefix(
            _serialize_type_for_checker(t),
            _serialize_type_for_checker(s),
            state.strict_optional,
            self.resolver,
        )
        assert result is not None, "Rust is_more_general_arg_prefix did not engage"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckPatternSuite(Suite):
    """Parity tests for the M22 checkpattern Rust helpers.

    Each test serializes a Type via Type.write(WriteBuffer) and asserts
    that the Rust helper agrees with the Python implementation. The suite
    mirrors the standalone functions in mypy/checkpattern.py:

    * rust_is_uninhabited vs isinstance(get_proper_type(t), UninhabitedType)
    * rust_get_match_arg_names vs the Python get_match_arg_names
    * rust_get_type_range vs the Python get_type_range bool-LKV unwrap logic
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        self._buf = _WriteBuffer()
        self._resolver = self._build_resolver(self._type_infos(), [])

    def _type_infos(self) -> list[TypeInfo]:

        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _build_resolver(self, infos: list[TypeInfo], aliases: list[Any]) -> Any:
        return _type_kernel.build_native_resolver(infos, aliases)

    def _bytes_of(self, t: Type) -> bytes:
        self._buf = _WriteBuffer()
        t.write(self._buf)
        return self._buf.getvalue()

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checkpattern import _set_native_checkpattern_active

        _set_native_checkpattern_active(active)
        try:
            return fn()
        finally:
            _set_native_checkpattern_active(True)

    def test_is_uninhabited_true(self) -> None:
        t = UninhabitedType()
        assert _type_kernel.rust_is_uninhabited(self._bytes_of(t), self._resolver) is True

    def test_is_uninhabited_false(self) -> None:
        t = self.fx.a
        assert _type_kernel.rust_is_uninhabited(self._bytes_of(t), self._resolver) is False

    def test_is_uninhabited_none_type(self) -> None:
        t = NoneType()
        assert _type_kernel.rust_is_uninhabited(self._bytes_of(t), self._resolver) is False

    def test_is_uninhabited_union_with_uninhabited(self) -> None:
        # Union containing UninhabitedType: get_proper_type keeps it in the
        # union, but is_uninhabited checks the whole type, not items.
        t = UnionType.make_union([self.fx.a, UninhabitedType()])
        assert _type_kernel.rust_is_uninhabited(self._bytes_of(t), self._resolver) is False

    def test_is_uninhabited_alias_to_uninhabited(self) -> None:
        # A TypeAliasType resolving to UninhabitedType must answer True
        # (Python: isinstance(get_proper_type(typ), UninhabitedType)); the
        # Rust seam expands the alias via the resolver snapshot.
        from mypy.checkpattern import is_uninhabited
        from mypy.nodes import TypeAlias

        alias = TypeAlias(UninhabitedType(), "mod.Never", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        resolver = self._build_resolver([], [alias])
        assert _type_kernel.rust_is_uninhabited(self._bytes_of(typ), resolver) is True
        off = self._with_gate(False, lambda: is_uninhabited(typ))
        on = self._with_gate(True, lambda: is_uninhabited(typ))
        assert_equal(on, off, "alias-to-uninhabited is_uninhabited parity")

    def test_is_uninhabited_alias_to_non_uninhabited(self) -> None:
        from mypy.checkpattern import is_uninhabited
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        resolver = self._build_resolver([], [alias])
        assert _type_kernel.rust_is_uninhabited(self._bytes_of(typ), resolver) is False
        assert_equal(
            self._with_gate(True, lambda: is_uninhabited(typ)),
            self._with_gate(False, lambda: is_uninhabited(typ)),
            "alias-to-instance is_uninhabited parity",
        )

    def test_is_uninhabited_alias_missing_snapshot(self) -> None:
        # Missing resolver snapshot: Rust defers (None) and Python falls back.
        from mypy.checkpattern import is_uninhabited
        from mypy.nodes import TypeAlias

        alias = TypeAlias(UninhabitedType(), "mod.Missing", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        off = self._with_gate(False, lambda: is_uninhabited(typ))
        on = self._with_gate(True, lambda: is_uninhabited(typ))
        assert_equal(on, off, "missing-snapshot is_uninhabited parity")
        assert on is True

    def test_get_match_arg_names_all_str(self) -> None:
        # TupleType with str-literal items -> list of str values.
        t = TupleType([self.fx.lit_str1, self.fx.lit_str2], self.fx.std_tuple)
        result = _type_kernel.rust_get_match_arg_names(self._bytes_of(t), self._resolver)
        assert result == ["x", "y"]

    def test_get_match_arg_names_mixed(self) -> None:
        # TupleType with a non-str-literal item -> None in that position.
        t = TupleType([self.fx.lit_str1, self.fx.a], self.fx.std_tuple)
        result = _type_kernel.rust_get_match_arg_names(self._bytes_of(t), self._resolver)
        assert result == ["x", None]

    def test_get_match_arg_names_no_literals(self) -> None:
        t = TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)
        result = _type_kernel.rust_get_match_arg_names(self._bytes_of(t), self._resolver)
        assert result == [None, None]

    def test_get_match_arg_names_empty(self) -> None:
        t = TupleType([], self.fx.std_tuple)
        result = _type_kernel.rust_get_match_arg_names(self._bytes_of(t), self._resolver)
        assert result == []

    def test_get_match_arg_names_alias_item(self) -> None:
        # An item that is a str-alias expands through the resolver to its
        # Literal[str], matching try_getting_str_literals_from_type.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.lit_str1, "mod.S", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        t = TupleType([typ, self.fx.lit_str2], self.fx.std_tuple)
        resolver = self._build_resolver([], [alias])
        assert _type_kernel.rust_get_match_arg_names(self._bytes_of(t), resolver) == ["x", "y"]

    def test_get_match_arg_names_alias_missing_snapshot_defers(self) -> None:
        # An unresolvable alias item: Rust defers (None) so Python resolves it.
        from mypy.checkpattern import get_match_arg_names
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.lit_str1, "mod.Missing", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        t = TupleType([typ, self.fx.lit_str2], self.fx.std_tuple)
        off = self._with_gate(False, lambda: get_match_arg_names(t))
        on = self._with_gate(True, lambda: get_match_arg_names(t))
        assert_equal(on, off, "missing-snapshot match-arg parity")
        assert on == ["x", "y"]

    def test_get_type_range_bool_lkv(self) -> None:
        # Instance with a bool last_known_value should return True (unwrap).
        t = Instance(self.fx.bool_type_info, [], last_known_value=self.fx.lit_true)
        assert _type_kernel.rust_get_type_range(self._bytes_of(t)) is True

    def test_get_type_range_bool_lkv_false(self) -> None:
        t = Instance(self.fx.bool_type_info, [], last_known_value=self.fx.lit_false)
        assert _type_kernel.rust_get_type_range(self._bytes_of(t)) is True

    def test_get_type_range_int_lkv(self) -> None:
        # Instance with an int last_known_value should return False (no unwrap).
        t = self.fx.lit1_inst
        assert _type_kernel.rust_get_type_range(self._bytes_of(t)) is False

    def test_get_type_range_no_lkv(self) -> None:
        t = self.fx.a
        assert _type_kernel.rust_get_type_range(self._bytes_of(t)) is False

    def test_get_type_range_none_type(self) -> None:
        t = NoneType()
        assert _type_kernel.rust_get_type_range(self._bytes_of(t)) is False

    def test_get_type_range_str_lkv(self) -> None:
        # Instance with a str last_known_value should return False (not bool).
        t = self.fx.lit_str1_inst
        assert _type_kernel.rust_get_type_range(self._bytes_of(t)) is False

    def test_should_self_match_int(self) -> None:
        # int is in self_match_type_names, so should_self_match(int) = True.
        from type_kernel import build_native_resolver

        int_info = self.fx.make_type_info("builtins.int")
        resolver = build_native_resolver([int_info, self.fx.oi], [])
        int_type = Instance(int_info, [])
        self_match_union = UnionType.make_union([int_type])
        result = _type_kernel.rust_should_self_match(
            self._bytes_of(int_type),
            False,  # has_match_args
            self._bytes_of(self_match_union),
            resolver,
        )
        assert result is True

    def test_should_self_match_custom_class(self) -> None:
        # A custom class not in self_match_type_names -> False.
        from type_kernel import build_native_resolver

        resolver = build_native_resolver([self.fx.ai, self.fx.oi], [])
        int_info = self.fx.make_type_info("builtins.int")
        int_type = Instance(int_info, [])
        self_match_union = UnionType.make_union([int_type])
        result = _type_kernel.rust_should_self_match(
            self._bytes_of(self.fx.a), False, self._bytes_of(self_match_union), resolver
        )
        assert result is False

    def test_should_self_match_any(self) -> None:
        # AnyType -> always False.
        from type_kernel import build_native_resolver

        int_info = self.fx.make_type_info("builtins.int")
        resolver = build_native_resolver([int_info, self.fx.oi], [])
        int_type = Instance(int_info, [])
        self_match_union = UnionType.make_union([int_type])
        any_t = AnyType(TypeOfAny.special_form)
        result = _type_kernel.rust_should_self_match(
            self._bytes_of(any_t), False, self._bytes_of(self_match_union), resolver
        )
        assert result is False

    def test_can_match_sequence_object(self) -> None:
        # object is more general than Sequence: is_subtype(Sequence, object)
        # is True, so can_match_sequence(object) = True.
        from type_kernel import build_native_resolver

        seq_info = self.fx.make_type_info("typing.Sequence", mro=[self.fx.oi])
        str_info = self.fx.make_type_info("builtins.str")
        resolver = build_native_resolver([seq_info, str_info, self.fx.oi], [])
        non_seq_union = UnionType.make_union([Instance(str_info, [])])
        seq_type = Instance(seq_info, [self.fx.a])
        # object: is_subtype(str, object) is False (non_seq check passes),
        # then is_subtype(object, Sequence) is False, but
        # is_subtype(Sequence, object) is True -> can_match = True.
        result = _type_kernel.rust_can_match_sequence(
            self._bytes_of(self.fx.o),
            self._bytes_of(non_seq_union),
            self._bytes_of(seq_type),
            resolver,
        )
        # Rust may defer (None) on generic subtype checks; when it does,
        # the Python fallback handles it. Assert True when it decides.
        assert result is None or result is True

    def test_can_match_sequence_str(self) -> None:
        # str is in non_sequence_match_types -> False.
        from type_kernel import build_native_resolver

        str_info = self.fx.make_type_info("builtins.str")
        seq_info = self.fx.make_type_info("typing.Sequence", mro=[self.fx.oi])
        resolver = build_native_resolver([str_info, seq_info, self.fx.oi], [])
        str_type = Instance(str_info, [])
        non_seq_union = UnionType.make_union([str_type])
        seq_type = Instance(seq_info, [self.fx.a])
        result = _type_kernel.rust_can_match_sequence(
            self._bytes_of(str_type),
            self._bytes_of(non_seq_union),
            self._bytes_of(seq_type),
            resolver,
        )
        assert result is False

    def test_can_match_sequence_any(self) -> None:
        # AnyType -> always True.
        from type_kernel import build_native_resolver

        str_info = self.fx.make_type_info("builtins.str")
        seq_info = self.fx.make_type_info("typing.Sequence", mro=[self.fx.oi])
        resolver = build_native_resolver([str_info, seq_info, self.fx.oi], [])
        any_t = AnyType(TypeOfAny.special_form)
        non_seq_union = UnionType.make_union([Instance(str_info, [])])
        seq_type = Instance(seq_info, [self.fx.a])
        result = _type_kernel.rust_can_match_sequence(
            self._bytes_of(any_t),
            self._bytes_of(non_seq_union),
            self._bytes_of(seq_type),
            resolver,
        )
        assert result is True


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassmethodStaticSuite(Suite):
    """Parity for the Rust `is_classmethod_node`/`is_node_static` ports.

    Both predicates unwrap a `Decorator` to its `func`, read `is_class` /
    `is_static` on `FuncDef` or `is_classmethod` / `is_staticmethod` on
    `Var`, and return `None` for any other node (including `None`). The
    Rust ports mirror the Python bodies in checker.py using live PyO3
    object reads. Toggling the checker-stmts gate off (pure Python) and on
    (Rust seam) must produce identical results, and direct seam calls prove
    both Rust functions engage rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_stmts_active

        self._set_active = _set_native_checker_stmts_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _assert_par(self, node: Any) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        off_cm = self._with_gate(False, lambda: is_classmethod_node(node))
        on_cm = self._with_gate(True, lambda: is_classmethod_node(node))
        assert_equal(on_cm, off_cm, f"is_classmethod_node parity {node!r}")
        off_st = self._with_gate(False, lambda: is_node_static(node))
        on_st = self._with_gate(True, lambda: is_node_static(node))
        assert_equal(on_st, off_st, f"is_node_static parity {node!r}")

    def _assert_engages(self, node: Any) -> None:
        assert (
            _type_kernel.rust_is_classmethod_node(node) is not None
        ), f"Rust is_classmethod_node did not engage for {node!r}"
        assert (
            _type_kernel.rust_is_node_static(node) is not None
        ), f"Rust is_node_static did not engage for {node!r}"

    def _func_def(self, name: str) -> FuncDef:
        return FuncDef(name)

    def _var(self, name: str) -> Var:
        return Var(name)

    def test_func_def_class(self) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        fd = self._func_def("f_class")
        fd.is_class = True
        self._assert_par(fd)
        assert_equal(self._with_gate(True, lambda: is_classmethod_node(fd)), True)
        assert_equal(self._with_gate(True, lambda: is_node_static(fd)), False)
        self._assert_engages(fd)

    def test_func_def_static(self) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        fd = self._func_def("f_static")
        fd.is_static = True
        self._assert_par(fd)
        assert_equal(self._with_gate(True, lambda: is_node_static(fd)), True)
        assert_equal(self._with_gate(True, lambda: is_classmethod_node(fd)), False)
        self._assert_engages(fd)

    def test_func_def_plain(self) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        fd = self._func_def("f_plain")
        self._assert_par(fd)
        assert_equal(self._with_gate(True, lambda: is_classmethod_node(fd)), False)
        assert_equal(self._with_gate(True, lambda: is_node_static(fd)), False)
        self._assert_engages(fd)

    def test_var_classmethod(self) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        v = self._var("v_class")
        v.is_classmethod = True
        self._assert_par(v)
        assert_equal(self._with_gate(True, lambda: is_classmethod_node(v)), True)
        assert_equal(self._with_gate(True, lambda: is_node_static(v)), False)
        self._assert_engages(v)

    def test_var_staticmethod(self) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        v = self._var("v_static")
        v.is_staticmethod = True
        self._assert_par(v)
        assert_equal(self._with_gate(True, lambda: is_node_static(v)), True)
        assert_equal(self._with_gate(True, lambda: is_classmethod_node(v)), False)
        self._assert_engages(v)

    def test_decorator_wraps_func_def(self) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        fd = self._func_def("f_dec")
        fd.is_class = True
        v = self._var("f_dec_var")
        dec = Decorator(fd, [], v)
        self._assert_par(dec)
        assert_equal(self._with_gate(True, lambda: is_classmethod_node(dec)), True)
        assert_equal(self._with_gate(True, lambda: is_node_static(dec)), False)
        self._assert_engages(dec)

    def test_decorator_wraps_static_func_def(self) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        fd = self._func_def("f_dec_static")
        fd.is_static = True
        v = self._var("f_dec_static_var")
        dec = Decorator(fd, [], v)
        self._assert_par(dec)
        assert_equal(self._with_gate(True, lambda: is_node_static(dec)), True)
        assert_equal(self._with_gate(True, lambda: is_classmethod_node(dec)), False)
        self._assert_engages(dec)

    def test_foreign_node_defers(self) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        # A non-FuncDef/Var/Decorator node yields None from both predicates
        # (real path), so Rust defers and Python runs the pure body.
        e = IntExpr(42)
        self._assert_par(e)
        assert_equal(self._with_gate(True, lambda: is_classmethod_node(e)), None)  # type: ignore[arg-type]
        assert_equal(self._with_gate(True, lambda: is_node_static(e)), None)  # type: ignore[arg-type]

    def test_none_input_defers(self) -> None:
        from mypy.checker import is_classmethod_node, is_node_static

        # None input yields None from both predicates (real path); Rust
        # defers and Python runs the pure body.
        self._assert_par(None)
        assert_equal(self._with_gate(True, lambda: is_classmethod_node(None)), None)
        assert_equal(self._with_gate(True, lambda: is_node_static(None)), None)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeGroupComparisonOperandsSuite(Suite):
    """Parity for the Rust `group_comparison_operands` port (mypy.checker).

    PURE DATA: no Type objects, no wire codec, no resolver. The seam maps
    each distinct literal hash (a `Key` tuple) to a stable integer id, and
    Rust union-finds operand chains over those ids, returning (op, sorted
    indices). Toggling the checker gate off (pure Python) and on (Rust) must
    yield identical output, and a direct seam call proves the Rust function
    engages rather than silently falling back.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _keymap(self, operands: dict[int, str]) -> dict[int, tuple[str, ...]]:
        # A hashable Key tuple; distinct names -> distinct ids, repeated
        # names -> the same id (the coalescing signal).
        return {index: ("FakeExpr", name) for index, name in operands.items()}

    def _assert_par(self, pairs: Any, keymap: Any, operators: set[str]) -> None:
        from mypy.checker import group_comparison_operands

        off = self._with_gate(False, lambda: group_comparison_operands(pairs, keymap, operators))
        on = self._with_gate(True, lambda: group_comparison_operands(pairs, keymap, operators))
        assert_equal(on, off, f"group_comparison_operands parity {pairs}")

    def test_doc_example_no_assignable(self) -> None:
        # x0 == x1 == x2 < x3 < x4 is x5 is x6 is not x7 is not x8
        from mypy.checker import group_comparison_operands

        x = [NameExpr(f"x{i}") for i in range(9)]
        pairs = [
            ("==", x[0], x[1]),
            ("==", x[1], x[2]),
            ("<", x[2], x[3]),
            ("<", x[3], x[4]),
            ("is", x[4], x[5]),
            ("is", x[5], x[6]),
            ("is not", x[6], x[7]),
            ("is not", x[7], x[8]),
        ]
        self._assert_par(pairs, self._keymap({}), {"==", "is"})
        result = self._with_gate(
            True, lambda: group_comparison_operands(pairs, self._keymap({}), {"==", "is"})
        )
        assert_equal(
            result,
            [
                ("==", [0, 1, 2]),
                ("<", [2, 3]),
                ("<", [3, 4]),
                ("is", [4, 5, 6]),
                ("is not", [6, 7]),
                ("is not", [7, 8]),
            ],
        )

    def test_doc_example_coalesce(self) -> None:
        # same == x < y == same
        from mypy.checker import group_comparison_operands

        same, xv, yv = NameExpr("same"), NameExpr("x"), NameExpr("y")
        pairs = [("==", same, xv), ("<", xv, yv), ("==", yv, same)]
        # Hashes present on operands 0 and 3 -> the two "==" chains merge.
        self._assert_par(pairs, self._keymap({0: "same", 3: "same"}), {"=="})
        result = self._with_gate(
            True,
            lambda: group_comparison_operands(pairs, self._keymap({0: "same", 3: "same"}), {"=="}),
        )
        assert_equal(result, [("==", [0, 1, 2, 3]), ("<", [1, 2])])
        # No hash entry -> no coalescing, plain size-2/3 groups.
        self._assert_par(pairs, self._keymap({}), {"=="})
        result = self._with_gate(
            True, lambda: group_comparison_operands(pairs, self._keymap({}), {"=="})
        )
        assert_equal(result, [("==", [0, 1]), ("<", [1, 2]), ("==", [2, 3])])

    def test_two_groups_merged(self) -> None:
        # x0==x1==x2 < x3==x4==x5 where x0 and x5 share a hash.
        from mypy.checker import group_comparison_operands

        x = [NameExpr(f"x{i}") for i in range(6)]
        pairs = [
            ("==", x[0], x[1]),
            ("==", x[1], x[2]),
            ("<", x[2], x[3]),
            ("==", x[3], x[4]),
            ("==", x[4], x[5]),
        ]
        self._assert_par(pairs, self._keymap({0: "x0", 5: "x0"}), {"=="})
        result = self._with_gate(
            True,
            lambda: group_comparison_operands(pairs, self._keymap({0: "x0", 5: "x0"}), {"=="}),
        )
        assert_equal(result, [("==", [0, 1, 2, 3, 4, 5]), ("<", [2, 3])])

    def test_different_operators_never_combine(self) -> None:
        # "==" and "is" groups never merge even when operands share a hash.
        from mypy.checker import group_comparison_operands

        x0, x1, x2, x3 = NameExpr("x0"), NameExpr("x1"), NameExpr("x2"), NameExpr("x3")
        pairs = [("==", x0, x1), ("==", x1, x2), ("is", x2, x3), ("is", x3, x0)]
        keymap = self._keymap({0: "x0", 1: "x1", 2: "x2", 3: "x3", 4: "x0"})
        self._assert_par(pairs, keymap, {"==", "is"})
        result = self._with_gate(
            True, lambda: group_comparison_operands(pairs, keymap, {"==", "is"})
        )
        assert_equal(result, [("==", [0, 1, 2]), ("is", [2, 3, 4])])

    def test_empty(self) -> None:
        from mypy.checker import group_comparison_operands

        self._assert_par([], self._keymap({}), {"=="})
        result = self._with_gate(
            True, lambda: group_comparison_operands([], self._keymap({}), {"=="})
        )
        assert_equal(result, [])

    def test_single_pair(self) -> None:
        # A lone "== a b" with no hashes is a size-2 group.
        from mypy.checker import group_comparison_operands

        a, b = NameExpr("a"), NameExpr("b")
        pairs = [("==", a, b)]
        for keymap in (self._keymap({}), self._keymap({0: "a"})):
            self._assert_par(pairs, keymap, {"=="})
        result = self._with_gate(True, lambda: group_comparison_operands(pairs, keymap, {"=="}))
        assert_equal(result, [("==", [0, 1])])

    def test_engages(self) -> None:
        # Direct seam call proves the Rust function runs and is correct.
        result = _type_kernel.rust_group_comparison_operands(
            [("==", 0, 1), ("<", 1, 2), ("==", 2, 3)], {0: 0, 3: 0}, ["=="]
        )
        assert_equal(result, [("==", [0, 1, 2, 3]), ("<", [1, 2])])


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeUnsafeOverlappingOverloadSuite(Suite):
    """Parity for the Rust `is_unsafe_overlapping_overload_signatures` port.

    The Rust entry (mypy.checker) detaches both callables, expands all
    type-variable combinations, and judges subset / overlap / callable
    compatibility on wire types in one call. Toggling the checker gate off
    (pure Python) and on (Rust seam) must produce identical booleans, and a
    direct seam call proves the Rust function engages rather than silently
    deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_checker_active(True)
        _set_native_checker_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def _callable(self, args: list[Type], ret: Type) -> CallableType:
        return CallableType(args, [ARG_POS] * len(args), [None] * len(args), ret, self.fx.function)

    def _assert_par(
        self, sig: CallableType, oth: CallableType, class_type_vars: Any, partial_only: bool = True
    ) -> None:
        from mypy.checker import is_unsafe_overlapping_overload_signatures

        off = self._with_gate(
            False,
            lambda: is_unsafe_overlapping_overload_signatures(
                sig, oth, class_type_vars, partial_only
            ),
        )
        on = self._with_gate(
            True,
            lambda: is_unsafe_overlapping_overload_signatures(
                sig, oth, class_type_vars, partial_only
            ),
        )
        assert_equal(on, off, f"is_unsafe_overlapping_overload_signatures parity {sig} / {oth}")

    def _assert_engages(
        self,
        sig: CallableType,
        oth: CallableType,
        class_type_vars: list[TypeVarType],
        partial_only: bool = True,
    ) -> None:
        from mypy.checker import _serialize_type_for_checker, _serialize_type_list
        from mypy.state import state

        result = _type_kernel.rust_is_unsafe_overlapping_overload_signatures(
            _serialize_type_for_checker(sig),
            _serialize_type_for_checker(oth),
            _serialize_type_list(class_type_vars),
            partial_only,
            state.strict_optional,
            self.resolver,
        )
        assert result is not None, "Rust is_unsafe_overlapping_overload_signatures did not engage"

    def test_disjoint_params_false(self) -> None:
        from mypy.checker import is_unsafe_overlapping_overload_signatures

        # A and D are disjoint siblings; neither call direction overlaps, and
        # the returns are not subsets. No expanded pair is reported -> False.
        sig = self._callable([self.fx.a], self.fx.a)
        oth = self._callable([self.fx.d], self.fx.b)
        self._assert_par(sig, oth, [])
        assert_equal(
            self._with_gate(True, lambda: is_unsafe_overlapping_overload_signatures(sig, oth, [])),
            False,
        )
        self._assert_engages(sig, oth, [])

    def test_subset_return_continue(self) -> None:
        from mypy.checker import is_unsafe_overlapping_overload_signatures

        # B <: A, so sig's return is a subset of oth's return: the pair is
        # skipped before the compatibility checks -> False.
        sig = self._callable([self.fx.a], self.fx.b)
        oth = self._callable([self.fx.a], self.fx.a)
        self._assert_par(sig, oth, [])
        assert_equal(
            self._with_gate(True, lambda: is_unsafe_overlapping_overload_signatures(sig, oth, [])),
            False,
        )
        self._assert_engages(sig, oth, [])

    def test_overlapping_params_partial_guard(self) -> None:
        from mypy.checker import is_unsafe_overlapping_overload_signatures

        # Params are equal (both A) and returns are non-subset, so A/B hold,
        # but the partial_only guard (subset A<:A) passes: not reported -> False.
        sig = self._callable([self.fx.a], self.fx.c)
        oth = self._callable([self.fx.a], self.fx.b)
        self._assert_par(sig, oth, [])
        assert_equal(
            self._with_gate(True, lambda: is_unsafe_overlapping_overload_signatures(sig, oth, [])),
            False,
        )
        self._assert_engages(sig, oth, [])

    def test_unsafe_overlap_true(self) -> None:
        from mypy.checker import is_unsafe_overlapping_overload_signatures

        # sig is more specific (A), oth is a catch-all (object). Returns C/D
        # are not subsets and oth's args are not a subset of sig's (object<:A
        # fails the covariant subset check), so the guard trips -> True.
        sig = self._callable([self.fx.a], self.fx.c)
        oth = self._callable([self.fx.o], self.fx.d)
        self._assert_par(sig, oth, [])
        assert_equal(
            self._with_gate(True, lambda: is_unsafe_overlapping_overload_signatures(sig, oth, [])),
            True,
        )
        self._assert_engages(sig, oth, [])

    def test_typevar_values_expand(self) -> None:
        from mypy.checker import is_unsafe_overlapping_overload_signatures

        # T has declared values [A, B]; expanding sig yields (x:A)->C and
        # (x:B)->C. The A variant overlaps the object catch-all and trips the
        # guard -> True.
        t = TypeVarType(
            "T",
            "T",
            TypeVarId(1),
            [self.fx.a, self.fx.b],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
            INVARIANT,
        )
        sig = self._callable([t], self.fx.c)
        oth = self._callable([self.fx.o], self.fx.d)
        self._assert_par(sig, oth, [])
        assert_equal(
            self._with_gate(True, lambda: is_unsafe_overlapping_overload_signatures(sig, oth, [])),
            True,
        )
        self._assert_engages(sig, oth, [])

    def test_class_type_vars_detach(self) -> None:
        from mypy.checker import is_unsafe_overlapping_overload_signatures

        # K is a class variable bound to A: detach pushes it onto
        # sig.variables, expansion binds it to A -> (x:A)->C. Against the
        # object catch-all the partial guard fails -> True.
        k = TypeVarType(
            "K",
            "K",
            TypeVarId(5),
            [],
            self.fx.a,
            AnyType(TypeOfAny.from_omitted_generics),
            INVARIANT,
        )
        sig = self._callable([k], self.fx.c)
        oth = self._callable([self.fx.o], self.fx.d)
        self._assert_par(sig, oth, [k])
        assert_equal(
            self._with_gate(
                True, lambda: is_unsafe_overlapping_overload_signatures(sig, oth, [k])
            ),
            True,
        )
        self._assert_engages(sig, oth, [k])

    def test_partial_only_false(self) -> None:
        from mypy.checker import is_unsafe_overlapping_overload_signatures

        # partial_only=False returns True as soon as any expanded pair
        # overlaps with non-subset returns, skipping the partial guard. oth
        # (B) is more specific than sig (A) so the guard would suppress it.
        sig = self._callable([self.fx.a], self.fx.a)
        oth = self._callable([self.fx.b], self.fx.b)
        self._assert_par(sig, oth, [], partial_only=False)
        assert_equal(
            self._with_gate(
                True, lambda: is_unsafe_overlapping_overload_signatures(sig, oth, [], False)
            ),
            True,
        )
        self._assert_engages(sig, oth, [], partial_only=False)

    def test_non_callable_defers(self) -> None:
        from mypy.checker import _serialize_type_for_checker, _serialize_type_list
        from mypy.state import state

        # A non-CallableType signature yields None from the Rust entry, so the
        # Python seam defers and runs the pure body unchanged.
        result = _type_kernel.rust_is_unsafe_overlapping_overload_signatures(
            _serialize_type_for_checker(self.fx.a),
            _serialize_type_for_checker(self._callable([self.fx.o], self.fx.d)),
            _serialize_type_list([]),
            True,
            state.strict_optional,
            self.resolver,
        )
        assert result is None, "Rust should defer on a non-callable signature"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeOverloadingOverloadsSuite(Suite):
    """Parity for the Rust `check_overlapping_overloads` screening-loop port.

    `TypeChecker.check_overlapping_overloads` (checker.py:1537-1658) runs a
    pairwise screening loop over every ordered pair of overload items and
    emits two message families from the results. The Rust driver
    (`overload_override.rs`) reproduces exactly that screening part (the
    argument-count gate, the never-match predicate, the unsafe-overlap
    predicate under strict_optional=True, and the flip note) on wire
    callables in one call. The impl-vs-items tail and the message emission
    stay in Python and are not exercised here.

    The reference replicates the loop with the pure-Python predicates
    (checker gate off); the native side calls the Rust driver directly.
    Both must produce identical `(i, j, kind, flip_note)` decision lists.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_checker_active(True)
        _set_native_checker_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)

    def _callable(self, args: list[Type], ret: Type) -> CallableType:
        return CallableType(args, [ARG_POS] * len(args), [None] * len(args), ret, self.fx.function)

    def _native_decisions(
        self,
        sigs: list[CallableType],
        class_type_vars: list[TypeVarLikeType],
        is_descriptor_get: bool,
        strict_optional: bool,
    ) -> list[tuple[int, int, int, bool]]:
        from mypy.checker import _serialize_type_for_checker, _serialize_type_list

        result = _type_kernel.rust_check_overlapping_overloads(
            [_serialize_type_for_checker(s) for s in sigs],
            _serialize_type_list(class_type_vars),
            is_descriptor_get,
            strict_optional,
            self.resolver,
        )
        assert result is not None, "Rust check_overlapping_overloads did not engage"
        return cast("list[tuple[int, int, int, bool]]", [tuple(r) for r in result])

    def _reference_decisions(
        self,
        sigs: list[CallableType],
        class_type_vars: list[TypeVarLikeType],
        is_descriptor_get: bool,
        strict_optional: bool,
    ) -> list[tuple[int, int, int, bool]]:
        """Replicate checker.py's pairwise screening loop with the checker gate off.

        Same pairing as check_overlapping_overloads (checker.py:1611-1658):
        the main never-match check reads the current strict_optional, while
        the unsafe-overlap check and the flip-note reversed checks always run
        under strict_optional=True. The checker gate is forced off around the
        loop so each predicate runs its pure-Python body, giving a true
        Python-vs-Rust differential against the native driver decisions.
        """
        from mypy.checker import (
            NATIVE_OVERLOAD_KIND_NEVER_MATCH,
            NATIVE_OVERLOAD_KIND_UNSAFE_OVERLAP,
            _set_native_checker_active,
            are_argument_counts_overlapping,
            is_unsafe_overlapping_overload_signatures,
            overload_can_never_match,
        )
        from mypy.state import state

        _set_native_checker_active(False)
        try:
            with state.strict_optional_set(strict_optional):
                out: list[tuple[int, int, int, bool]] = []
                for i, sig1 in enumerate(sigs):
                    for j, sig2 in enumerate(sigs[i + 1 :]):
                        if not are_argument_counts_overlapping(sig1, sig2):
                            continue
                        if overload_can_never_match(sig1, sig2):
                            out.append((i, j, NATIVE_OVERLOAD_KIND_NEVER_MATCH, False))
                        elif not is_descriptor_get:
                            with state.strict_optional_set(True):
                                if is_unsafe_overlapping_overload_signatures(
                                    sig1, sig2, class_type_vars
                                ):
                                    flip_note = (
                                        j == 0
                                        and not is_unsafe_overlapping_overload_signatures(
                                            sig2, sig1, class_type_vars
                                        )
                                        and not overload_can_never_match(sig2, sig1)
                                    )
                                    out.append(
                                        (i, j, NATIVE_OVERLOAD_KIND_UNSAFE_OVERLAP, flip_note)
                                    )
                return out
        finally:
            _set_native_checker_active(True)

    def _assert_parity(
        self,
        sigs: list[CallableType],
        class_vars: list[TypeVarLikeType],
        *,
        is_descriptor_get: bool = False,
        strict_optional: bool = True,
    ) -> None:
        native = self._native_decisions(sigs, class_vars, is_descriptor_get, strict_optional)
        reference = self._reference_decisions(sigs, class_vars, is_descriptor_get, strict_optional)
        assert_equal(
            native,
            reference,
            "check_overlapping_overloads screening parity for "
            f"{[str(s) for s in sigs]} class_vars={[str(t) for t in class_vars]}",
        )

    def test_never_match_reported(self) -> None:
        from mypy.checker import NATIVE_OVERLOAD_KIND_NEVER_MATCH

        # The first signature is broader (object), so the second can never be
        # matched: (0, 0) never-match decision, no flip note.
        sigs = [self._callable([self.fx.o], self.fx.a), self._callable([self.fx.a], self.fx.b)]
        self._assert_parity(sigs, [])
        assert_equal(
            self._native_decisions(sigs, [], False, True),
            [(0, 0, NATIVE_OVERLOAD_KIND_NEVER_MATCH, False)],
        )

    def test_no_decision_disjoint_counts(self) -> None:
        # Argument counts do not overlap (1 vs 3 positional), so the pair is
        # screened out before either predicate runs.
        sigs = [
            self._callable([self.fx.a], self.fx.a),
            self._callable([self.fx.a, self.fx.a, self.fx.a], self.fx.b),
        ]
        self._assert_parity(sigs, [])
        assert_equal(self._native_decisions(sigs, [], False, True), [])

    def test_unsafe_overlap_reversed_never_no_flip(self) -> None:
        from mypy.checker import NATIVE_OVERLOAD_KIND_UNSAFE_OVERLAP

        # sig (A)->A then a broader catch-all (object)->B: unsafe overlap (the
        # partial-only return guard trips), but the reversed direction is
        # never-match, so the flip note is suppressed (checker.py flip_note).
        sigs = [self._callable([self.fx.a], self.fx.a), self._callable([self.fx.o], self.fx.b)]
        self._assert_parity(sigs, [])
        assert_equal(
            self._native_decisions(sigs, [], False, True),
            [(0, 0, NATIVE_OVERLOAD_KIND_UNSAFE_OVERLAP, False)],
        )

    def test_unsafe_overlap_reversed_unsafe_no_flip(self) -> None:
        from mypy.checker import NATIVE_OVERLOAD_KIND_UNSAFE_OVERLAP

        # sig (A)->C then (B|D)->D: both directions are unsafely overlapping,
        # so the flip note is suppressed (reversed direction is also unsafe).
        sigs = [
            self._callable([self.fx.a], self.fx.c),
            self._callable([UnionType([self.fx.b, self.fx.d])], self.fx.d),
        ]
        self._assert_parity(sigs, [])
        assert_equal(
            self._native_decisions(sigs, [], False, True),
            [(0, 0, NATIVE_OVERLOAD_KIND_UNSAFE_OVERLAP, False)],
        )

    def test_descriptor_get_suppresses_unsafe(self) -> None:
        # An overloaded __get__ suppresses unsafe-overlap errors but keeps
        # never-match ones.
        sigs = [self._callable([self.fx.a], self.fx.c), self._callable([self.fx.o], self.fx.d)]
        self._assert_parity(sigs, [], is_descriptor_get=True)
        assert_equal(self._native_decisions(sigs, [], True, True), [])

    def test_three_items_middle_and_last(self) -> None:
        from mypy.checker import NATIVE_OVERLOAD_KIND_NEVER_MATCH

        # Three items exercise the two inner pairs (0,1) and (1,2) plus the
        # (0,2) pair, checking that j tracks the slice offset correctly.
        sigs = [
            self._callable([self.fx.o], self.fx.a),
            self._callable([self.fx.a], self.fx.c),
            self._callable([self.fx.a, self.fx.a], self.fx.b),
        ]
        self._assert_parity(sigs, [])
        assert_equal(
            self._native_decisions(sigs, [], False, True),
            [(0, 0, NATIVE_OVERLOAD_KIND_NEVER_MATCH, False)],
        )

    def test_strict_optional_current_state(self) -> None:
        # The main never-match check reads the current strict_optional; the
        # unsafe check always runs under strict_optional=True. The
        # (None)->int / (str)->str pair is unsafely overlapping only outside

        # strict-optional (checker.py:1584-1595), so under apt flags the
        # native driver must keep it unflagged.
        none_type = NoneType()
        sigs = [
            self._callable([none_type], self.fx.a),
            self._callable([self.fx.str_type], self.fx.b),
        ]
        self._assert_parity(sigs, [], strict_optional=True)
        self._assert_parity(sigs, [], strict_optional=False)
        assert_equal(self._native_decisions(sigs, [], False, True), [])


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAndOrConditionalMapsSuite(Suite):
    """Parity for the Rust `and_conditional_maps` / `or_conditional_maps` ports.

    `mypy.checker.and_conditional_maps` (checker.py:9704) and
    `or_conditional_maps` (checker.py:9783) combine two narrowing TypeMaps
    for the truth of `e1 and e2` / `e1 or e2`; the Rust ports in
    `condmaps.rs` mirror the Python precedence rules and defer (None) when a
    value fails to decode or a `meet_types` / `make_simplified_union` call
    defers. The `use_meet=True` path hard-defers in Rust (condmaps.rs), so
    both gates run the Python meet loop there. Toggling the checker gate off
    (Python) and on (Rust) must produce identical result maps for a few
    if/elif/while narrowing shapes, and the direct seam calls prove the Rust
    functions engage rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        # Cache a bound NameExpr per variable name so common-key scenarios
        # share the same Expression/Var identity across both maps, matching
        # production where and/or_conditional_maps receives the same

        # narrowed expression object in both TypeMaps.
        self._keys: dict[str, NameExpr] = {}
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
        self.resolver.set_live_typeinfo_map({info.fullname: info for info in type_infos})
        _set_native_checker_active(True)
        _set_native_checker_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)

    def _map(self, pairs: list[tuple[str, Type]]) -> TypeMap:
        """Build a TypeMap keyed by cached NameExprs.

        Each name resolves to one cached NameExpr bound to its own Var, so
        `literal_hash` yields a distinct, stable key per variable (an unbound
        NameExpr hashes as ("Var", None) for every name, collapsing all keys
        into one). Reusing the cached key across maps mirrors production,
        where the same narrowed expression object appears in both TypeMaps.
        """

        m: TypeMap = {}
        for name, t in pairs:
            e = self._keys.get(name)
            if e is None:
                e = NameExpr(name)
                e.node = Var(name)
                self._keys[name] = e
            m[e] = t
        return m

    def _normalize(self, m: TypeMap) -> set[tuple[str, str]]:
        return {(str(e), str(t)) for e, t in m.items()}

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def _assert_and_parity(self, m1: TypeMap, m2: TypeMap, *, use_meet: bool = False) -> None:
        from mypy.checker import and_conditional_maps

        off = self._with_gate(False, lambda: and_conditional_maps(m1, m2, use_meet=use_meet))
        on = self._with_gate(True, lambda: and_conditional_maps(m1, m2, use_meet=use_meet))
        # assert_equal reformats fmt with .format(), and dict reprs contain
        # braces, so build a brace-free message with the normalized sets.
        assert_equal(
            self._normalize(off),
            self._normalize(on),
            "and_conditional_maps parity gate-off={} vs gate-on={}".format(
                sorted(self._normalize(off)), sorted(self._normalize(on))
            ),
        )

    def _assert_or_parity(self, m1: TypeMap, m2: TypeMap, *, coalesce_any: bool = False) -> None:
        from mypy.checker import or_conditional_maps

        off = self._with_gate(
            False, lambda: or_conditional_maps(m1, m2, coalesce_any=coalesce_any)
        )
        on = self._with_gate(True, lambda: or_conditional_maps(m1, m2, coalesce_any=coalesce_any))
        assert_equal(
            self._normalize(off),
            self._normalize(on),
            "or_conditional_maps parity gate-off={} vs gate-on={}".format(
                sorted(self._normalize(off)), sorted(self._normalize(on))
            ),
        )

    def _and_engages(self, m1: TypeMap, m2: TypeMap, *, use_meet: bool = False) -> None:
        from mypy.checker import _serialize_type_for_checker
        from mypy.literals import literal_hash

        keys1 = [hash(literal_hash(e)) for e in m1]
        vals1 = [_serialize_type_for_checker(t) for t in m1.values()]
        keys2 = [hash(literal_hash(e)) for e in m2]
        vals2 = [_serialize_type_for_checker(t) for t in m2.values()]
        result = _type_kernel.rust_and_conditional_maps(
            keys1, vals1, keys2, vals2, use_meet, state.strict_optional, self.resolver
        )
        assert result is not None, f"Rust and_conditional_maps did not engage: {m1} vs {m2}"

    def _or_engages(self, m1: TypeMap, m2: TypeMap, *, coalesce_any: bool = False) -> None:
        from mypy.checker import _serialize_type_for_checker
        from mypy.literals import literal_hash

        keys1 = [hash(literal_hash(e)) for e in m1]
        vals1 = [_serialize_type_for_checker(t) for t in m1.values()]
        keys2 = [hash(literal_hash(e)) for e in m2]
        vals2 = [_serialize_type_for_checker(t) for t in m2.values()]
        result = _type_kernel.rust_or_conditional_maps(
            keys1, vals1, keys2, vals2, coalesce_any, state.strict_optional, self.resolver
        )
        assert result is not None, f"Rust or_conditional_maps did not engage: {m1} vs {m2}"

    def test_and_common_key_m2_precedence(self) -> None:
        from mypy.checker import and_conditional_maps

        # Same expression narrowed by both conditions; use_meet=False keeps
        # m2's value unless m1 is Never or m2 is Any over a non-Any union.
        m1 = self._map([("x", self.fx.a)])
        m2 = self._map([("x", self.fx.b)])
        self._assert_and_parity(m1, m2)
        result = self._with_gate(True, lambda: and_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), {("NameExpr(x)", "B")})
        self._and_engages(m1, m2)

    def test_and_disjoint_keys(self) -> None:
        from mypy.checker import and_conditional_maps

        # Different expressions: the result combines both maps.
        m1 = self._map([("x", self.fx.a)])
        m2 = self._map([("y", self.fx.b)])
        self._assert_and_parity(m1, m2)
        result = self._with_gate(True, lambda: and_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), {("NameExpr(x)", "A"), ("NameExpr(y)", "B")})
        self._and_engages(m1, m2)

    def test_and_m1_never_wins(self) -> None:
        from mypy.checker import and_conditional_maps

        # m1 narrowed to Never wins over m2's refinement.
        m1 = self._map([("x", UninhabitedType())])
        m2 = self._map([("x", self.fx.b)])
        self._assert_and_parity(m1, m2)
        result = self._with_gate(True, lambda: and_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), {("NameExpr(x)", "Never")})
        self._and_engages(m1, m2)

    def test_and_m2_any_uses_m1(self) -> None:
        from mypy.checker import and_conditional_maps

        # m2 is Any and m1 is a plain instance (not a union containing Any):
        # keep m1's precise refinement.
        m1 = self._map([("x", self.fx.a)])
        m2 = self._map([("x", AnyType(TypeOfAny.special_form))])
        self._assert_and_parity(m1, m2)
        result = self._with_gate(True, lambda: and_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), {("NameExpr(x)", "A")})
        self._and_engages(m1, m2)

    def test_and_m2_any_union_keeps_m2(self) -> None:
        from mypy.checker import and_conditional_maps

        # m1 is a union containing Any: m2's Any is kept.
        m1 = self._map([("x", UnionType([self.fx.a, AnyType(TypeOfAny.special_form)]))])
        m2 = self._map([("x", AnyType(TypeOfAny.special_form))])
        self._assert_and_parity(m1, m2)
        result = self._with_gate(True, lambda: and_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), {("NameExpr(x)", "Any")})
        self._and_engages(m1, m2)

    def test_and_use_meet_true_native(self) -> None:
        from mypy.checker import and_conditional_maps

        # use_meet=True is decided natively (#1298): the seam engages and the
        # meet result matches the Python meet loop.
        m1 = self._map([("x", self.fx.a)])
        m2 = self._map([("x", self.fx.b)])
        self._assert_and_parity(m1, m2, use_meet=True)
        self._and_engages(m1, m2, use_meet=True)
        result = self._with_gate(True, lambda: and_conditional_maps(m1, m2, use_meet=True))
        assert_equal(self._normalize(result), {("NameExpr(x)", "B")})

    def test_or_common_key_union(self) -> None:
        from mypy.checker import or_conditional_maps

        # Same expression narrowed differently to unrelated types: the
        # result joins both values. (fx.d is unrelated to fx.a, unlike
        # fx.b which is a subtype of A in the fixture.)
        m1 = self._map([("x", self.fx.a)])
        m2 = self._map([("x", self.fx.d)])
        self._assert_or_parity(m1, m2)
        result = self._with_gate(True, lambda: or_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), {("NameExpr(x)", "A | D")})
        self._or_engages(m1, m2)

    def test_or_m1_unreachable(self) -> None:
        from mypy.checker import or_conditional_maps

        # An unreachable m1 contributes nothing: the result is m2.
        m1 = self._map([("x", UninhabitedType())])
        m2 = self._map([("y", self.fx.b)])
        self._assert_or_parity(m1, m2)
        result = self._with_gate(True, lambda: or_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), {("NameExpr(y)", "B")})
        self._or_engages(m1, m2)

    def test_or_m2_unreachable(self) -> None:
        from mypy.checker import or_conditional_maps

        # An unreachable m2 contributes nothing: the result is m1.
        m1 = self._map([("x", self.fx.a)])
        m2 = self._map([("y", UninhabitedType())])
        self._assert_or_parity(m1, m2)
        result = self._with_gate(True, lambda: or_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), {("NameExpr(x)", "A")})
        self._or_engages(m1, m2)

    def test_or_no_common_keys(self) -> None:
        from mypy.checker import or_conditional_maps

        # No expression refined by both conditions: empty result.
        m1 = self._map([("x", self.fx.a)])
        m2 = self._map([("y", self.fx.b)])
        self._assert_or_parity(m1, m2)
        result = self._with_gate(True, lambda: or_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), set())
        self._or_engages(m1, m2)

    def test_or_coalesce_any_keeps_any(self) -> None:
        from mypy.checker import or_conditional_maps

        # coalesce_any with an Any m1 keeps the Any.
        m1 = self._map([("x", AnyType(TypeOfAny.special_form))])
        m2 = self._map([("x", self.fx.b)])
        self._assert_or_parity(m1, m2, coalesce_any=True)
        result = self._with_gate(True, lambda: or_conditional_maps(m1, m2, coalesce_any=True))
        assert_equal(self._normalize(result), {("NameExpr(x)", "Any")})
        self._or_engages(m1, m2, coalesce_any=True)

    def test_or_coalesce_any_false_unions(self) -> None:
        from mypy.checker import or_conditional_maps

        # Without coalesce_any, an Any m1 and a concrete m2 both weigh in;
        # make_simplified_union keeps both items (Any | B does not collapse).
        m1 = self._map([("x", AnyType(TypeOfAny.special_form))])
        m2 = self._map([("x", self.fx.b)])
        self._assert_or_parity(m1, m2)
        result = self._with_gate(True, lambda: or_conditional_maps(m1, m2))
        assert_equal(self._normalize(result), {("NameExpr(x)", "Any | B")})


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeConditionalTypesSuite(Suite):
    """Parity for the Rust `conditional_types` port (mypy.checker).

    `conditional_types` is the isinstance/equality narrowing that splits a
    `current_type` into `(proposed, remaining)`. The Rust port in
    `cond_types.rs` mirrors every branch of the Python function and returns
    `None` whenever a sub-step Rust cannot decide is reached, deferring the
    whole call to pure Python. Binary `TypeRange` serialization goes through
    `checker._serialize_type_ranges`. Toggling the checker gate off (Python)
    and on (Rust) must produce identical results; a direct seam call proves
    the Rust function engages rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.int_info = self.fx.make_type_info("builtins.int")
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        # builtins.bool, builtins.str, and hand-built builtins.int do not end
        # in "i", so the scan skips them; add them explicitly (production
        # always snapshots builtins).
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info, self.int_info])
        # A protocol (structural-subtype branch) and an enum (enum-literal
        # expansion) with explicitly-valued member names.
        self.proto_info = self.fx.make_type_info("mod.Proto", mro=[self.fx.oi])
        self.proto_info.is_protocol = True
        type_infos.append(self.proto_info)
        self.enum_info = self.fx.make_type_info("mod.Color", mro=[self.fx.oi])
        self.enum_info.is_enum = True
        for name in ("RED", "GREEN", "BLUE"):
            v = Var(name)
            v.has_explicit_value = True
            v.type = self.fx.o
            self.enum_info.names[name] = SymbolTableNode(MDEF, v)
        type_infos.append(self.enum_info)
        # A NewType: `conditional_types` unwraps it to `type.bases[0]`.
        # mro includes int_info so UserId is a proper subtype of int.
        self.newtype_info = self.fx.make_type_info("mod.UserId", mro=[self.int_info, self.fx.oi])
        self.newtype_info.is_newtype = True
        self.newtype_info.bases = [Instance(self.int_info, [])]
        type_infos.append(self.newtype_info)
        # A generic class (single TypeVar) and a variadic class (TypeVar +
        # TypeVarTuple) for the from_equality erasure tests.
        self.box_info = self.fx.make_type_info("mod.Box", mro=[self.fx.oi], typevars=["T"])
        type_infos.append(self.box_info)
        self.variad_info = self.fx.make_type_info(
            "mod.Variad", mro=[self.fx.oi], typevars=["T", "Ts"], typevar_tuple_index=1
        )
        type_infos.append(self.variad_info)
        # Three aliases (mod.MA -> A, mod.MB -> B, mod.MAny -> Any) for the
        # current/target alias narrowing tests below.
        self.inst_alias = TypeAlias(self.fx.a, "mod.MA", "mod", -1, -1)
        self.b_alias = TypeAlias(self.fx.b, "mod.MB", "mod", -1, -1)
        self.any_alias = TypeAlias(self.fx.anyt, "mod.MAny", "mod", -1, -1)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        from mypy.wirefixup import set_wire_alias_map

        set_wire_alias_map(
            {
                self.inst_alias.fullname: self.inst_alias,
                self.b_alias.fullname: self.b_alias,
                self.any_alias.fullname: self.any_alias,
            }
        )
        self.resolver = _type_kernel.build_native_resolver(
            type_infos, [self.inst_alias, self.b_alias, self.any_alias]
        )
        self._live_map = {info.fullname: info for info in type_infos}
        self.resolver.set_live_typeinfo_map(dict(self._live_map))
        _set_native_checker_active(True)
        _set_native_checker_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)
        set_wire_alias_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def _assert_par(
        self, current: Type, ranges: list[TypeRange] | None, default: Type | None
    ) -> None:
        from mypy.checker import conditional_types

        off = self._with_gate(False, lambda: conditional_types(current, ranges, default))
        on = self._with_gate(True, lambda: conditional_types(current, ranges, default))
        assert_equal(
            (str(off[0]), str(off[1])),
            (str(on[0]), str(on[1])),
            f"conditional_types parity {current} vs {ranges}",
        )

    def _assert_engages(
        self, current: Type, ranges: list[TypeRange] | None, default: Type | None
    ) -> None:
        from mypy.checker import _serialize_type_for_checker, _serialize_type_ranges

        result = _type_kernel.rust_conditional_types(
            _serialize_type_for_checker(current),
            _serialize_type_ranges(ranges) if ranges is not None else None,
            _serialize_type_for_checker(default) if default is not None else None,
            True,
            False,
            state.strict_optional,
            self.resolver,
        )
        assert result is not None, f"Rust conditional_types did not engage for {current}"

    def test_current_vs_none_ranges(self) -> None:
        # ranges=None: no isinstance information, keep current and default.
        from mypy.checker import conditional_types

        self._assert_par(self.fx.a, None, None)
        yes, no = self._with_gate(True, lambda: conditional_types(self.fx.a, None, None))
        assert_equal((str(yes), str(no)), ("A", "None"))
        self._assert_engages(self.fx.a, None, None)

    def test_empty_ranges(self) -> None:
        # Empty ranges: isinstance(x, ()) is always False -> unreachable.
        from mypy.checker import conditional_types

        self._assert_par(self.fx.a, [], None)
        yes, no = self._with_gate(True, lambda: conditional_types(self.fx.a, [], None))
        assert_equal(str(yes), "Never")
        assert_equal(str(no), "None")
        self._assert_engages(self.fx.a, [], None)

    def test_str_vs_int(self) -> None:
        current = self.fx.str_type
        ranges = [TypeRange(Instance(self.int_info, []), False)]
        from mypy.checker import conditional_types

        self._assert_par(current, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        # str and int do not overlap, so the if-branch is unreachable; with
        # no default the else branch stays None.
        assert_equal((str(yes), str(no)), ("Never", "None"))
        self._assert_engages(current, ranges, None)

    def test_int_vs_literal_union(self) -> None:
        # current=int, proposed=Literal[1] | Literal[2]. Not a proper
        # subtype, not structural, overlapping -> yes=proposed,
        # no=int minus {1,2}.
        current = Instance(self.int_info, [])
        int_inst = current
        lit1 = LiteralType(1, int_inst)
        lit2 = LiteralType(2, int_inst)
        ranges = [TypeRange(lit1, False), TypeRange(lit2, False)]
        from mypy.checker import conditional_types

        self._assert_par(current, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal(str(yes), "Literal[1] | Literal[2]")
        assert_equal(str(no), "builtins.int")
        self._assert_engages(current, ranges, None)

    def test_union_current_factorization(self) -> None:
        # Union current factorizes: isinstance(A | B, C) ->
        # yes = A_yes | B_yes, no = A_no | B_no. For A and B both unrelated
        # to C, the else keeps each item; B <: A simplifies the union to A.
        from mypy.checker import conditional_types

        current = UnionType([self.fx.a, self.fx.b])
        ranges = [TypeRange(self.fx.c, False)]
        self._assert_par(current, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal(str(yes), "C")
        assert_equal(str(no), "A")
        self._assert_engages(current, ranges, None)

    def test_newtype_unwrap(self) -> None:
        # NewType unwraps to its base before narrowing: UserId <: int is a
        # proper subtype, so yes=default(int) and no=Never.
        from mypy.checker import conditional_types

        newtype_inst = Instance(self.newtype_info, [])
        int_inst = Instance(self.int_info, [])
        ranges = [TypeRange(int_inst, False)]
        self._assert_par(newtype_inst, ranges, int_inst)
        yes, no = self._with_gate(
            True, lambda: conditional_types(newtype_inst, ranges, default=int_inst)
        )
        assert_equal(str(yes), "builtins.int")
        assert_equal(str(no), "Never")
        self._assert_engages(newtype_inst, ranges, int_inst)

    def test_bool_literal_expansion(self) -> None:
        # Single Literal[True] range expands current bool to
        # Literal[True] | Literal[False], then narrows: yes=True, no=False.
        from mypy.checker import conditional_types

        current = self.fx.bool_type
        ranges = [TypeRange(self.fx.lit_true, False)]
        self._assert_par(current, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal(str(yes), "Literal[True]")
        assert_equal(str(no), "Literal[False]")
        self._assert_engages(current, ranges, None)

    def test_enum_literal_expansion(self) -> None:
        # Single Literal["RED"] range expands current enum to
        # Literal["RED"] | Literal["GREEN"] | Literal["BLUE"].
        from mypy.checker import conditional_types

        enum_inst = Instance(self.enum_info, [])
        lit_red = LiteralType("RED", enum_inst)
        ranges = [TypeRange(lit_red, False)]
        self._assert_par(enum_inst, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(enum_inst, ranges, None))
        assert_equal(str(yes), "Literal[mod.Color.RED]")
        assert_equal(str(no), "Literal[mod.Color.GREEN] | Literal[mod.Color.BLUE]")
        self._assert_engages(enum_inst, ranges, None)

    def test_structural_protocol(self) -> None:
        # proposed is a protocol. The Rust `is_subtype` port now decides
        # protocol-right Instance pairs natively (issue #1111), so
        # structural-BTS goes through the Rust path end to end.
        from mypy.checker import (
            _serialize_type_for_checker,
            _serialize_type_ranges,
            conditional_types,
        )

        proto_inst = Instance(self.proto_info, [])
        current = self.fx.a
        ranges = [TypeRange(proto_inst, False)]
        self._assert_par(current, ranges, None)
        result = _type_kernel.rust_conditional_types(
            _serialize_type_for_checker(current),
            _serialize_type_ranges(ranges),
            None,
            True,
            False,
            state.strict_optional,
            self.resolver,
        )
        # Structural protocol checks decide natively now: the seam returns
        # the narrowed pair, and the Python answer below must be unchanged.
        assert (
            result is not None
        ), f"Rust conditional_types did not engage for {current} vs protocol"
        yes, no = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal((str(yes), str(no)), ("None", "Never"))

    def test_from_equality(self) -> None:
        # from_equality erases generic args before overlap; the Rust path
        # now decides the erase itself (erased_vars port), so a generic
        # proposed type engages natively instead of deferring.
        current = self.fx.str_type
        ranges = [TypeRange(self.fx.b, False)]
        self._assert_par(current, ranges, None)

    def test_from_equality_generic_engages(self) -> None:
        # A generic proposed type: the erase-eq port rewrites list[int] to
        # list[Any] natively, so the seam returns a decision.
        from mypy.checker import _serialize_type_for_checker, _serialize_type_ranges

        current = self.fx.str_type
        ranges = [TypeRange(Instance(self.box_info, [self.fx.o]), False)]
        self._assert_par(current, ranges, None)
        result = _type_kernel.rust_conditional_types(
            _serialize_type_for_checker(current),
            _serialize_type_ranges(ranges),
            None,
            True,
            True,
            state.strict_optional,
            self.resolver,
        )
        assert result is not None, "Rust conditional_types did not engage (generic erase)"

    def test_from_equality_variadic_slot_count(self) -> None:
        # A variadic proposed type: the erased instance carries one arg per
        # defn.type_vars slot (2), not per arg (3), with the TVT slot
        # becoming Unpack(tuple[Any]) (typevartuples.py:28-35).
        from mypy.checker import (
            _serialize_type_for_checker,
            _serialize_type_ranges,
            conditional_types,
        )

        current = self.fx.str_type
        three = [self.fx.o, self.fx.o, self.fx.o]
        ranges = [TypeRange(Instance(self.variad_info, three), False)]
        self._assert_par(current, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal(str(yes), "Never", "str is never mod.Variad[Any, *tuple[Any, ...]]")
        result = _type_kernel.rust_conditional_types(
            _serialize_type_for_checker(current),
            _serialize_type_ranges(ranges),
            None,
            True,
            True,
            state.strict_optional,
            self.resolver,
        )
        assert result is not None, "Rust conditional_types did not engage (variadic erase)"

    def test_current_alias_narrows_like_target(self) -> None:
        # cur-alias: current is a TypeAliasType (mod.MA -> A). The port expands
        # it once via get_proper_or_expand and reuses the proper type for the
        # Any check, union factorization, and the avoid-widening tail.
        from mypy.checker import conditional_types

        alias = TypeAliasType(self.inst_alias, [])
        ranges = [TypeRange(Instance(self.int_info, []), False)]
        self._assert_par(alias, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(alias, ranges, None))
        assert_equal((str(yes), str(no)), ("Never", "None"))
        self._assert_engages(alias, ranges, None)

    def test_union_current_with_alias_item(self) -> None:
        # cur-alias2: a union current whose first item is a TypeAliasType
        # factorizes through the same expanded current as the plain-union
        # case (Union[A, B] vs C).
        from mypy.checker import conditional_types

        current = UnionType([TypeAliasType(self.inst_alias, []), self.fx.b])
        ranges = [TypeRange(self.fx.c, False)]
        self._assert_par(current, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal((str(yes), str(no)), ("C", "A"))
        self._assert_engages(current, ranges, None)

    def test_target_alias_range(self) -> None:
        # tgt0-alias: the range item is a TypeAliasType (mod.MB -> B). The
        # port expands the target via the alias snapshot, mirroring Python's
        # get_proper_type on ranges[0].item.
        from mypy.checker import conditional_types

        current = self.fx.a
        ranges = [TypeRange(TypeAliasType(self.b_alias, []), False)]
        self._assert_par(current, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal((str(yes), str(no)), ("B", "A"))
        self._assert_engages(current, ranges, None)

    def test_alias_to_any_current(self) -> None:
        # An alias to Any must expand before the Any-current check, so the
        # alias case matches plain Any: yes = proposed, no = Any.
        from mypy.checker import conditional_types

        any_alias = self.any_alias
        current = TypeAliasType(any_alias, [])
        ranges = [TypeRange(self.fx.a, False)]
        self._assert_par(current, ranges, None)
        yes, no = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal((str(yes), str(no)), ("A", "Any"))
        self._assert_engages(current, ranges, None)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeEqualityAmbiguitySuite(Suite):
    """Parity for `partition_equality_ambiguous_types` and
    `is_equality_ambiguous_for_narrowing` (mypy.checker).

    These narrow enum/unions that compare equal through a value domain
    broader than their nominal type (IntEnum vs int, StrEnum vs str). The
    Rust port reuses `equality_value_info_inner` (the #679 port) for the
    per-type domain collection and implements the shared-domain comparison
    loop + union partition. Toggling the checker gate off (pure Python) and
    on (Rust seam) must produce identical narrowable/ambiguous splits, and a
    direct seam call proves the Rust function engages rather than silently
    deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.int_info = self.fx.make_type_info("builtins.int")
        self.bytes_info = self.fx.make_type_info("builtins.bytes")
        # A StrEnum: mro through builtins.str -> str domain.
        self.mystrenum_info = self.fx.make_type_info(
            "mod.MyStrEnum", mro=[self.fx.str_type_info, self.fx.oi]
        )
        self.mystrenum_info.is_enum = True
        # Two IntEnums: mro through builtins.int -> numeric domain.
        self.myintenum_info = self.fx.make_type_info(
            "mod.MyIntEnum", mro=[self.int_info, self.fx.oi]
        )
        self.myintenum_info.is_enum = True
        self.otherenum_info = self.fx.make_type_info(
            "mod.OtherEnum", mro=[self.int_info, self.fx.oi]
        )
        self.otherenum_info.is_enum = True
        # A closed-domain enum: mro through builtins.bytes -> bytes domain.
        self.bytese_info = self.fx.make_type_info("mod.BytesE", mro=[self.bytes_info, self.fx.oi])
        self.bytese_info.is_enum = True
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        # str_type_info / bool_type_info do not end in "i".
        type_infos.extend(
            [self.fx.str_type_info, self.fx.bool_type_info, self.int_info, self.bytes_info]
        )
        type_infos.extend(
            [self.mystrenum_info, self.myintenum_info, self.otherenum_info, self.bytese_info]
        )
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_checker_active(True)
        _set_native_checker_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def _normalize(self, pair: tuple[object, object]) -> tuple[object, object]:
        narrowable, ambiguous = pair
        return (
            str(narrowable) if narrowable is not None else None,
            str(ambiguous) if ambiguous is not None else None,
        )

    def _assert_par(
        self, current: Type, target: Type, *, is_identity: bool = False
    ) -> tuple[object, object]:
        from mypy.checker import partition_equality_ambiguous_types

        off = self._with_gate(
            False,
            lambda: partition_equality_ambiguous_types(current, target, is_identity=is_identity),
        )
        on = self._with_gate(
            True,
            lambda: partition_equality_ambiguous_types(current, target, is_identity=is_identity),
        )
        offn, offa = self._normalize(off)
        onn, ona = self._normalize(on)
        assert_equal((onn, ona), (offn, offa), f"partition parity {current} vs {target}")
        return onn, ona

    def _assert_ambiguity_par(self, left: Type, right: Type) -> bool:
        from mypy.checker import is_equality_ambiguous_for_narrowing

        off = self._with_gate(False, lambda: is_equality_ambiguous_for_narrowing(left, right))
        on = self._with_gate(True, lambda: is_equality_ambiguous_for_narrowing(left, right))
        assert_equal(on, off, f"ambiguity parity {left} vs {right}")
        return bool(on)

    def _assert_engages(self, current: Type, target: Type, *, is_identity: bool = False) -> None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_partition_equality_ambiguous_types(
            _serialize_type_for_checker(current),
            _serialize_type_for_checker(target),
            is_identity,
            True,
            self.resolver,
        )
        assert result is not None, f"Rust partition did not engage for {current} vs {target}"
        amb_result = _type_kernel.rust_is_equality_ambiguous_for_narrowing(
            _serialize_type_for_checker(current),
            _serialize_type_for_checker(target),
            self.resolver,
        )
        assert amb_result is not None, f"Rust ambiguity did not engage for {current} vs {target}"

    def test_strenum_union_vs_member(self) -> None:
        # MyStrEnum | str vs MyStrEnum.MEMBER: the enum portion narrows, the
        # str portion is equality-ambiguous and must stay in both branches.
        enum_inst = Instance(self.mystrenum_info, [])
        member = LiteralType("red", enum_inst)
        current = UnionType([enum_inst, Instance(self.fx.str_type_info, [])])
        narrowable, ambiguous = self._assert_par(current, member)
        assert_equal(ambiguous, "builtins.str")
        assert_equal(narrowable, "mod.MyStrEnum")
        self._assert_engages(current, member)

    def test_intenum_vs_int_is_ambiguous(self) -> None:
        # An IntEnum can compare equal to its underlying int.
        enum_inst = Instance(self.myintenum_info, [])
        target = Instance(self.int_info, [])
        self._assert_ambiguity_par(enum_inst, target)
        self._assert_par(enum_inst, target)
        self._assert_engages(enum_inst, target)

    def test_two_different_enums_are_ambiguous(self) -> None:
        # Distinct enum types share the numeric domain but differ in names.
        left = Instance(self.myintenum_info, [])
        right = Instance(self.otherenum_info, [])
        self._assert_ambiguity_par(left, right)
        self._assert_engages(left, right)

    def test_identity_narrowing_no_partition(self) -> None:
        # Identity narrowing returns (current_type, None) with no ambiguous
        # side from either branch.
        enum_inst = Instance(self.mystrenum_info, [])
        member = LiteralType("red", enum_inst)
        current = UnionType([enum_inst, Instance(self.fx.str_type_info, [])])
        narrowable, ambiguous = self._assert_par(current, member, is_identity=True)
        assert_equal(ambiguous, None)
        assert_equal(narrowable, str(current))
        self._assert_engages(current, member, is_identity=True)

    def test_closed_domain_same_enum_not_ambiguous(self) -> None:
        # Equality between two values of the same enum can narrow by literal
        # member even in a closed domain (bytes).
        b = Instance(self.bytese_info, [])
        self._assert_ambiguity_par(b, b)
        # A full partition over a closed-domain enum vs itself stays
        # narrowable (no ambiguous side).
        narrowable, ambiguous = self._assert_par(b, b)
        assert_equal(ambiguous, None)
        assert_equal(narrowable, "mod.BytesE")
        self._assert_engages(b, b)

    def test_closed_domain_enum_vs_plain_bytes_is_ambiguous(self) -> None:
        # A closed-domain enum's value may compare equal to its underlying
        # bytes, so narrowing against a plain bytes target is ambiguous.
        b = Instance(self.bytese_info, [])
        target = Instance(self.bytes_info, [])
        self._assert_ambiguity_par(b, target)
        self._assert_par(b, target)
        self._assert_engages(b, target)

    def test_top_object_vs_open_domain_enum_is_ambiguous(self) -> None:
        # A top-like info (builtins.object) is ambiguous against an enum in
        # an OPEN domain (str): the open domain cannot be exhausted.
        enum_inst = Instance(self.mystrenum_info, [])
        self._assert_ambiguity_par(self.fx.o, enum_inst)
        self._assert_engages(self.fx.o, enum_inst)

    def test_top_object_vs_closed_domain_enum_is_not_ambiguous(self) -> None:
        # A top-like info against a CLOSED domain (bytes) narrows to the
        # complete known set, so it is not equality-ambiguous.
        b = Instance(self.bytese_info, [])
        self._assert_ambiguity_par(self.fx.o, b)
        self._assert_engages(self.fx.o, b)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFinalSuperSuite(Suite):
    """Parity for the Rust `check_compatibility_final_super` decision-head port.

    `TypeChecker.check_compatibility_final_super` (checker.py:4608-4636) is a
    pure decision over the base attribute node shape (Var / FuncBase /
    Decorator), the two `is_final` flags, the overriding name, the base
    fullname, and the enum allowlists. The Rust classifier
    (`checker_functions.rs`) turns those facts into a branch tag; the Python
    shim applies the side effects (cant_override_final message,
    check_if_final_var_override_writable) and keeps the pure-Python body as
    the fallback.

    Direct seam calls assert the exact tag for every branch; the gate-off vs
    gate-on differential drives the real TypeChecker method through a stub
    message recorder and asserts identical (return, messages) pairs.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:

        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _var(self, name: str, is_final: bool) -> Var:
        v = Var(name)
        v.is_final = is_final
        return v

    def _funcdef(self, name: str, is_final: bool) -> FuncDef:
        f = FuncDef(name)
        f.is_final = is_final
        return f

    def _decorator(self, name: str, is_final: bool) -> Decorator:
        f = FuncDef(name)
        f.is_final = is_final
        return Decorator(f, [], self._var(name, is_final))

    def _base(self, fullname: str, name: str = "Base") -> Any:
        from types import SimpleNamespace

        return SimpleNamespace(name=name, fullname=fullname)

    def _tag(
        self, base_node: Any, node_is_final: bool, node_name: str, base_fullname: str
    ) -> int | None:
        from mypy.semanal_enum import ENUM_BASES, ENUM_SPECIAL_PROPS

        return _type_kernel.rust_classify_final_super(
            base_node,
            node_is_final,
            node_name,
            base_fullname,
            list(ENUM_BASES),
            list(ENUM_SPECIAL_PROPS),
        )

    def _run(
        self, node: Any, base: Any, base_node: Any, *, writable: bool = True
    ) -> tuple[bool, list[tuple[str, str]]]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> tuple[bool, list[tuple[str, str]]]:
            chk = TypeChecker.__new__(TypeChecker)
            msgs: list[tuple[str, str]] = []
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                cant_override_final=lambda n, bn, ctx: msgs.append(("cant_override", n)),
                final_cant_override_writable=lambda n, ctx: msgs.append(("writable", n)),
            )
            chk.is_writable_attribute = lambda base_n: writable  # type: ignore[assignment]
            ret = chk.check_compatibility_final_super(node, base, base_node)
            return ret, msgs

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on  # type: ignore[return-value]

    def _assert_par(self, node: Any, base: Any, base_node: Any) -> None:
        off, on = self._run(node, base, base_node)
        assert_equal(
            on, off, f"check_compatibility_final_super parity for base_node={base_node!r}"
        )

    def test_seam_none_base_node(self) -> None:
        assert self._tag(None, False, "attr", "mod.Base") == 0
        assert self._tag(None, True, "attr", "mod.Base") == 0

    def test_seam_private_name(self) -> None:
        base_node = self._var("base_attr", True)
        assert self._tag(base_node, True, "__priv", "mod.Base") == 1

    def test_seam_cant_override_final_var(self) -> None:
        base_node = self._var("base_attr", True)
        assert self._tag(base_node, True, "attr", "mod.Base") == 2

    def test_seam_cant_override_final_method(self) -> None:
        base_node = self._funcdef("base_method", True)
        assert self._tag(base_node, False, "attr", "mod.Base") == 2

    def test_seam_enum_base(self) -> None:
        base_node = self._var("base_attr", False)
        assert self._tag(base_node, True, "attr", "enum.Enum") == 3

    def test_seam_enum_special_prop(self) -> None:
        base_node = self._var("base_attr", False)
        assert self._tag(base_node, True, "name", "mod.Base") == 3

    def test_seam_check_writable(self) -> None:
        base_node = self._var("base_attr", False)
        assert self._tag(base_node, True, "attr", "mod.Base") == 4

    def test_seam_tail_pass(self) -> None:
        base_node = self._var("base_attr", False)
        assert self._tag(base_node, False, "attr", "mod.Base") == 5

    def test_decorator_base_is_not_var(self) -> None:
        # A final decorated base method overridden by a final var: error.
        base_node = self._decorator("base_method", True)
        assert self._tag(base_node, True, "attr", "mod.Base") == 2

    def test_parity_every_branch(self) -> None:
        node_final = self._var("attr", True)
        node_plain = self._var("attr", False)
        base = self._base("mod.Base")
        # None base node: pass.
        self._assert_par(node_plain, base, None)
        # Private name: pass.
        node_private = self._var("__priv", True)
        self._assert_par(node_private, base, self._var("base_attr", True))
        # Final var overriding final var: error.
        self._assert_par(node_final, base, self._var("base_attr", True))
        # Final var overriding final method (FuncBase, not Var): error.
        self._assert_par(node_plain, base, self._funcdef("base_method", True))
        # Enum base: pass (no writability check).
        self._assert_par(node_final, self._base("enum.Enum"), self._var("base_attr", False))
        # Enum special prop: pass.
        node_name = self._var("name", True)
        self._assert_par(node_name, base, self._var("base_attr", False))
        # Plain final override of writable base attr: writability check emitted.
        self._assert_par(node_final, base, self._var("base_attr", False))
        # Non-final override of non-final base var: trailing pass, no message.
        self._assert_par(node_plain, base, self._var("base_attr", False))

    def _run_once(
        self, node: Any, base: Any, base_node: Any, *, active: bool
    ) -> BaseException | None:
        """Run the real method under exactly one gate; return the raise.

        `_run` toggles both gates internally, which cannot observe a
        raising base_node (the gate-off raise short-circuits it), so the
        broken-var pins need a single-gate variant (issue #1470).
        """
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> None:
            chk = TypeChecker.__new__(TypeChecker)
            msgs: list[tuple[str, str]] = []
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                cant_override_final=lambda n, bn, ctx: msgs.append(("cant_override", n)),
                final_cant_override_writable=lambda n, ctx: msgs.append(("writable", n)),
            )
            chk.is_writable_attribute = lambda base_n: True  # type: ignore[assignment]
            chk.check_compatibility_final_super(node, base, base_node)

        try:
            self._with_gate(active, check_one)
            return None
        except Exception as err:
            return err

    def test_seam_defers_on_unreadable_final(self) -> None:
        # An `is_final` read raising AttributeError must defer (None) so the
        # shim re-runs the pure-Python body (issue #1470 pin).
        base_node = _BrokenFinalVar("base_attr")
        assert self._tag(base_node, True, "attr", "mod.Base") is None

    def test_seam_repropagates_non_attribute_error(self) -> None:
        # A read raising RuntimeError must NOT be swallowed: the seam
        # re-propagates it so genuine kernel bugs stay visible (issue #1470
        # error-class boundary pin).
        import pytest

        base_node = _BrokenFinalVar("base_attr", RuntimeError)
        with pytest.raises(RuntimeError):
            self._tag(base_node, True, "attr", "mod.Base")

    def test_par_unreadable_final(self) -> None:
        # Both gates raise the identical AttributeError: gate-off is the pure
        # body, gate-on defers the unreadable attribute to it (issue #1470).
        node = self._var("node_attr", True)
        base = self._base("mod.Base")
        base_node = _BrokenFinalVar("base_attr")
        off = self._run_once(node, base, base_node, active=False)
        on = self._run_once(node, base, base_node, active=True)
        assert isinstance(off, AttributeError), f"gate-off expected AttributeError: {off!r}"
        assert isinstance(on, AttributeError), f"gate-on expected AttributeError: {on!r}"
        assert str(off) == str(on), f"raise messages differ: {off!r} != {on!r}"

    def test_par_repropagates_non_attribute_error(self) -> None:
        # Both gates raise the identical RuntimeError: gate-off is the pure
        # body, gate-on re-propagates it from the seam (issue #1470).
        node = self._var("node_attr", True)
        base = self._base("mod.Base")
        base_node = _BrokenFinalVar("base_attr", RuntimeError)
        off = self._run_once(node, base, base_node, active=False)
        on = self._run_once(node, base, base_node, active=True)
        assert isinstance(off, RuntimeError), f"gate-off expected RuntimeError: {off!r}"
        assert isinstance(on, RuntimeError), f"gate-on expected RuntimeError: {on!r}"
        assert str(off) == str(on), f"raise messages differ: {off!r} != {on!r}"

    def test_seam_defers_on_inner_unreadable_bool(self) -> None:
        # An AttributeError from the truthiness of the already-read
        # `is_final` value must defer (None), like the getattr arm (issue
        # #1477 pin).
        base_node = _BrokenFinalValueVar("base_attr")
        assert self._tag(base_node, True, "attr", "mod.Base") is None

    def test_seam_repropagates_inner_non_attribute_error(self) -> None:
        # A RuntimeError from the truthiness of the `is_final` value must
        # NOT be swallowed into a deferral: it re-propagates from the seam
        # (issue #1477 pin).
        import pytest

        base_node = _BrokenFinalValueVar("base_attr", RuntimeError)
        with pytest.raises(RuntimeError):
            self._tag(base_node, True, "attr", "mod.Base")

    def test_par_inner_unreadable_bool(self) -> None:
        # Both gates raise the identical AttributeError: gate-off truthiness
        # in the pure body, gate-on defers the unreadable bool to it (issue
        # #1477).
        node = self._var("node_attr", True)
        base = self._base("mod.Base")
        base_node = _BrokenFinalValueVar("base_attr")
        off = self._run_once(node, base, base_node, active=False)
        on = self._run_once(node, base, base_node, active=True)
        assert isinstance(off, AttributeError), f"gate-off expected AttributeError: {off!r}"
        assert isinstance(on, AttributeError), f"gate-on expected AttributeError: {on!r}"
        assert str(off) == str(on), f"raise messages differ: {off!r} != {on!r}"

    def test_par_inner_repropagates_non_attribute_error(self) -> None:
        # Both gates raise the identical RuntimeError: gate-off truthiness in
        # the pure body, gate-on re-propagates it from the seam (issue
        # #1477).
        node = self._var("node_attr", True)
        base = self._base("mod.Base")
        base_node = _BrokenFinalValueVar("base_attr", RuntimeError)
        off = self._run_once(node, base, base_node, active=False)
        on = self._run_once(node, base, base_node, active=True)
        assert isinstance(off, RuntimeError), f"gate-off expected RuntimeError: {off!r}"
        assert isinstance(on, RuntimeError), f"gate-on expected RuntimeError: {on!r}"
        assert str(off) == str(on), f"raise messages differ: {off!r} != {on!r}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeUntypedDecoratorSuite(Suite):
    """Parity for the Rust `check_for_untyped_decorator` conjunction port.

    `TypeChecker.check_for_untyped_decorator` (checker.py:6955-6964) is the
    bool gate `disallow_untyped_decorators and is_typed_callable(func.type)
    and is_untyped_decorator(dec_type) and not current_node_deferred`. The
    Rust fold (checker_functions.rs) computes the func sub-predicate on the
    wire format and walks the decorator side as a live PyO3 object
    (checkexpr_functions.rs: the Instance arm runs the real
    `TypeInfo.get_method("__call__")`); the Python shim emits
    `typed_function_untyped_decorator` when it returns True and keeps the
    pure-Python body as the fallback.

    Direct seam calls prove engagement and short-circuit ordering; the
    gate-off vs gate-on differential drives the real TypeChecker method and
    asserts identical message output.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _typed_callable(self) -> CallableType:
        return CallableType([self.fx.a], [ARG_POS], [None], self.fx.a, self.fx.function)

    def _untyped_callable(self) -> CallableType:
        any_unannotated = AnyType(TypeOfAny.unannotated)
        return CallableType(
            [any_unannotated], [ARG_POS], [None], any_unannotated, self.fx.function
        )

    def _serialize(self, typ: Type | None) -> bytes | None:
        from mypy.checker import _serialize_type_for_checker

        return _serialize_type_for_checker(typ) if typ is not None else None

    def _seam(
        self, disallow: bool, func_type: Type | None, dec_type: Type | None, deferred: bool
    ) -> bool | None:
        # The decorator side rides the live object (the Instance arm needs
        # the real TypeInfo.get_method lookup).
        return _type_kernel.rust_check_for_untyped_decorator(
            disallow, self._serialize(func_type), dec_type, deferred
        )

    def _run(
        self, disallow: bool, func_type: Type | None, dec_type: Type | None, deferred: bool
    ) -> tuple[list[str], list[str]]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> list[str]:
            chk = TypeChecker.__new__(TypeChecker)
            names: list[str] = []
            chk.options = SimpleNamespace(disallow_untyped_decorators=disallow)  # type: ignore[assignment]
            chk.current_node_deferred = deferred
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                typed_function_untyped_decorator=lambda n, ctx: names.append(n)
            )
            func = FuncDef("func")
            func.type = func_type  # type: ignore[assignment]
            chk.check_for_untyped_decorator(func, dec_type, None)  # type: ignore[arg-type]
            return names

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(
        self, disallow: bool, func_type: Type | None, dec_type: Type | None, deferred: bool
    ) -> None:
        off, on = self._run(disallow, func_type, dec_type, deferred)
        assert_equal(
            on,
            off,
            "check_for_untyped_decorator parity (disallow={}, deferred={})".format(
                disallow, deferred
            ),
        )

    def test_seam_disallow_false(self) -> None:
        assert self._seam(False, None, None, True) is False

    def test_seam_typed_func_untyped_decorator(self) -> None:
        assert self._seam(True, self._typed_callable(), self._untyped_callable(), False) is True

    def test_seam_typed_func_untyped_decorator_deferred(self) -> None:
        assert self._seam(True, self._typed_callable(), self._untyped_callable(), True) is False

    def test_seam_untyped_func(self) -> None:
        assert self._seam(True, self._untyped_callable(), self._untyped_callable(), False) is False

    def test_seam_typed_decorator(self) -> None:
        assert self._seam(True, self._typed_callable(), self._typed_callable(), False) is False

    def test_seam_none_types(self) -> None:
        # is_typed_callable(None) is False; is_untyped_decorator(None) is True.
        assert self._seam(True, None, None, False) is False
        assert self._seam(True, self._typed_callable(), None, False) is True

    def _instance_with_call(self, method: FuncBase | Decorator) -> Instance:
        from mypy.nodes import MDEF, SymbolTableNode

        info = self._fixture_info()
        info.names["__call__"] = SymbolTableNode(MDEF, method)  # type: ignore[arg-type]
        return Instance(info, [])

    def _fixture_info(self) -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        defn = ClassDef("Callable", Block([]), None, [])
        defn.fullname = "mod.HasCall"
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        # `get_method` walks `info.mro`; seed it with the class itself so
        # the symbol-table entry is discoverable.
        info.mro.append(info)
        return info

    def _method_fdef(self, typ: Type | None) -> FuncDef:
        fdef = FuncDef("__call__")
        fdef.type = typ  # type: ignore[assignment]
        return fdef

    def _decorator_method(self, func_type: Type | None, var_type: Type | None) -> Decorator:
        from mypy.nodes import GDEF, SymbolTableNode, Var

        info = self._fixture_info()
        func = self._method_fdef(func_type)
        var = Var("__call__")
        var.type = var_type
        dec = Decorator(func, [], var)
        info.names["__call__"] = SymbolTableNode(GDEF, dec)
        return dec

    def test_seam_instance_without_call(self) -> None:
        # The walker decides: a class without __call__ is a typed decorator
        # (is_untyped_decorator answers False), so the seam returns Some.
        assert self._seam(True, self._typed_callable(), self.fx.a, False) is False

    def test_seam_instance_untyped_call(self) -> None:
        dec = self._instance_with_call(self._method_fdef(self._untyped_callable()))
        assert self._seam(True, self._typed_callable(), dec, False) is True

    def test_seam_instance_typed_call(self) -> None:
        dec = self._instance_with_call(self._method_fdef(self._typed_callable()))
        assert self._seam(True, self._typed_callable(), dec, False) is False

    def test_seam_instance_decorator_head(self) -> None:
        # Decorator head: untyped func.type decides True before var.type.
        dec = self._decorator_method(self._untyped_callable(), self._typed_callable())
        assert (
            self._seam(True, self._typed_callable(), self._instance_with_call(dec), False) is True
        )
        # Typed func.type defers to var.type; a None var.type is untyped.
        dec = self._decorator_method(self._typed_callable(), None)
        assert (
            self._seam(True, self._typed_callable(), self._instance_with_call(dec), False) is True
        )
        # None func.type is untyped on both paths (is_untyped_decorator(
        # None) is True), regardless of var.type.
        dec = self._decorator_method(None, self._typed_callable())
        assert (
            self._seam(True, self._typed_callable(), self._instance_with_call(dec), False) is True
        )

    def test_parity_decorator_none_func_type(self) -> None:
        # Python returns True at the func.type arm (is_untyped_decorator(
        # None) is True, `or` short-circuit); the walker must not fall
        # through to a typed var.type and answer False.
        dec = self._decorator_method(None, self._typed_callable())
        self._assert_par(True, self._typed_callable(), self._instance_with_call(dec), False)

    def test_seam_instance_overloaded_call(self) -> None:
        from mypy.types import Overloaded

        items = [self._untyped_callable(), self._typed_callable()]
        dec = self._instance_with_call(self._method_fdef(Overloaded(items)))
        assert self._seam(True, self._typed_callable(), dec, False) is True

    def test_seam_direct_walker_arms(self) -> None:
        # The standalone is_untyped_decorator seam on the same shapes.
        assert _type_kernel.rust_is_untyped_decorator(self._untyped_callable()) is True
        assert _type_kernel.rust_is_untyped_decorator(self._typed_callable()) is False
        assert _type_kernel.rust_is_untyped_decorator(self.fx.a) is False
        inst = self._instance_with_call(self._method_fdef(self._untyped_callable()))
        assert _type_kernel.rust_is_untyped_decorator(inst) is True

    def test_parity_all_false(self) -> None:
        self._assert_par(False, None, None, False)

    def test_parity_typed_func_untyped_decorator(self) -> None:
        self._assert_par(True, self._typed_callable(), self._untyped_callable(), False)

    def test_parity_typed_func_untyped_decorator_deferred(self) -> None:
        self._assert_par(True, self._typed_callable(), self._untyped_callable(), True)

    def test_parity_untyped_func(self) -> None:
        self._assert_par(True, self._untyped_callable(), self._untyped_callable(), False)

    def test_parity_typed_decorator(self) -> None:
        self._assert_par(True, self._typed_callable(), self._typed_callable(), False)

    def test_parity_instance_decorator(self) -> None:
        self._assert_par(True, self._typed_callable(), self.fx.a, False)

    def test_parity_instance_untyped_call(self) -> None:
        dec = self._instance_with_call(self._method_fdef(self._untyped_callable()))
        self._assert_par(True, self._typed_callable(), dec, False)

    def test_parity_instance_typed_call(self) -> None:
        dec = self._instance_with_call(self._method_fdef(self._typed_callable()))
        self._assert_par(True, self._typed_callable(), dec, False)

    def test_parity_none_decorator(self) -> None:
        self._assert_par(True, self._typed_callable(), None, False)

    def test_seam_alias_decorator_leaf(self) -> None:
        # A top-level alias expands via the real get_proper_type and
        # re-classifies; the alias itself must not defer the seam.
        untyped_alias = self.fx.non_rec_alias(self._untyped_callable())
        typed_alias = self.fx.non_rec_alias(self._typed_callable())
        assert self._seam(True, self._typed_callable(), untyped_alias, False) is True
        assert self._seam(True, self._typed_callable(), typed_alias, False) is False

    def test_parity_alias_decorator_leaf(self) -> None:
        alias = self.fx.non_rec_alias(self._untyped_callable())
        self._assert_par(True, self._typed_callable(), alias, False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeExplicitOverrideDecoratorSuite(Suite):
    """Parity for the Rust `check_explicit_override_decorator` conjunction port.

    `TypeChecker.check_explicit_override_decorator` (checker.py:3139) is a
    pure 5-flag conjunction over `plugin_generated`, `found_method_base_classes`
    truthiness, `defn.is_explicit_override`, `defn.name` membership in
    {"__init__", "__new__"}, and `is_private(defn.name)`. When true, the method
    emits `self.msg.explicit_override_decorator_missing(name, base_fullname,
    context)`. The Rust predicate (`checker_functions.rs`) evaluates the
    conjunction; the Python shim emits the message. `False` defers to the
    pure-Python body (mirrors the Python default for `plugin_generated`).

    Direct seam calls assert the bool for every flag combination; the gate-off
    vs gate-on differential drives the real TypeChecker method through a stub
    message recorder and asserts identical message records.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _info(self, name: str = "A") -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        defn = ClassDef(name, Block([]), None, [])
        defn.fullname = f"mod.{name}"
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        # `info.get(name)` walks `info.mro`; seed it with the class itself
        # so the symbol-table entries are discoverable.
        info.mro.append(info)
        return info

    def _defn(
        self,
        name: str,
        info: TypeInfo | None,
        is_explicit_override: bool = False,
        plugin_generated: bool = False,
    ) -> FuncDef:
        from mypy.nodes import GDEF, SymbolTableNode, Var

        if info is None:
            info = self._info()
        fdef = FuncDef(name)
        fdef.info = info
        fdef.is_explicit_override = is_explicit_override
        # `plugin_generated` lives on the SymbolTableNode, not on Var.
        node = Var(name)
        info.names[name] = SymbolTableNode(GDEF, node, plugin_generated=plugin_generated)
        return fdef

    def _bases(self, fullname: str = "mod.Base") -> list[TypeInfo]:
        return [self._info("Base")]

    def _run(
        self, defn: FuncDef, found: list[TypeInfo] | None
    ) -> tuple[list[tuple[Any, ...]], list[tuple[Any, ...]]]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> list[tuple[Any, ...]]:
            chk = TypeChecker.__new__(TypeChecker)
            msgs: list[tuple[Any, ...]] = []
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                explicit_override_decorator_missing=(
                    lambda n, bn, ctx: msgs.append(("missing", n, bn))
                )
            )
            chk.check_explicit_override_decorator(defn, found, defn)
            return msgs

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, defn: FuncDef, found: list[TypeInfo] | None) -> None:
        off, on = self._run(defn, found)
        assert_equal(on, off, f"check_explicit_override_decorator parity for name={defn.name!r}")

    def test_seam_emit_plain_override(self) -> None:
        fdef = self._defn("override", None)
        assert _type_kernel.rust_check_explicit_override_decorator(fdef, self._bases())

    def test_seam_plugin_generated_suppresses(self) -> None:
        fdef = self._defn("override", None, plugin_generated=True)
        assert not _type_kernel.rust_check_explicit_override_decorator(fdef, self._bases())

    def test_seam_no_base_classes(self) -> None:
        fdef = self._defn("override", None)
        assert not _type_kernel.rust_check_explicit_override_decorator(fdef, None)
        assert not _type_kernel.rust_check_explicit_override_decorator(fdef, [])

    def test_seam_explicit_override_suppresses(self) -> None:
        fdef = self._defn("override", None, is_explicit_override=True)
        assert not _type_kernel.rust_check_explicit_override_decorator(fdef, self._bases())

    def test_seam_init_and_new_suppress(self) -> None:
        fdef_init = self._defn("__init__", None)
        assert not _type_kernel.rust_check_explicit_override_decorator(fdef_init, self._bases())
        fdef_new = self._defn("__new__", None)
        assert not _type_kernel.rust_check_explicit_override_decorator(fdef_new, self._bases())

    def test_seam_private_name_suppresses(self) -> None:
        fdef = self._defn("__private", None)
        assert not _type_kernel.rust_check_explicit_override_decorator(fdef, self._bases())

    def test_parity_every_branch(self) -> None:
        # Emit: plain public override with a base class.
        self._assert_par(self._defn("override", None), self._bases())
        # Suppress: plugin-generated method.
        self._assert_par(self._defn("override", None, plugin_generated=True), self._bases())
        # Suppress: no base classes (None and empty list).
        self._assert_par(self._defn("override", None), None)
        self._assert_par(self._defn("override", None), [])
        # Suppress: is_explicit_override.
        self._assert_par(self._defn("override", None, is_explicit_override=True), self._bases())
        # Suppress: __init__ / __new__.
        self._assert_par(self._defn("__init__", None), self._bases())
        self._assert_par(self._defn("__new__", None), self._bases())
        # Suppress: private name.
        self._assert_par(self._defn("__private", None), self._bases())
        # Suppress: single-underscore name (not private by mypy's def,
        # so this should emit, confirming the boundary).
        self._assert_par(self._defn("_single", None), self._bases())


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCompatibilityClassvarSuperSuite(Suite):
    """Parity for the Rust `check_compatibility_classvar_super` 2x2 predicate port.

    `TypeChecker.check_compatibility_classvar_super` (checker.py:4796) is a
    pure decision over whether `base_node` is a `Var` and the `is_classvar`
    flags of the overriding `node` and the `base_node`. The Rust classifier
    (`checker_functions.rs`) turns those facts into a branch tag; the Python
    shim applies the side effects (CANNOT_OVERRIDE_INSTANCE_VAR /
    CANNOT_OVERRIDE_CLASS_VAR message emission) and keeps the pure-Python
    body as the fallback.

    Direct seam calls assert the exact tag for every branch; the gate-off
    vs gate-on differential drives the real TypeChecker method through a
    stub message recorder and asserts identical (return, messages) pairs.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _var(self, name: str, is_classvar: bool) -> Var:
        v = Var(name)
        v.is_classvar = is_classvar
        return v

    def _funcdef(self, name: str) -> FuncDef:
        return FuncDef(name)

    def _base(self, fullname: str, name: str = "Base") -> Any:
        from types import SimpleNamespace

        return SimpleNamespace(name=name, fullname=fullname)

    def _tag(self, base_node: Any, node_is_classvar: bool) -> int | None:
        return _type_kernel.rust_classify_classvar_super(base_node, node_is_classvar)

    def _run(self, node: Any, base: Any, base_node: Any) -> tuple[bool, list[tuple[str, str]]]:

        from mypy.checker import TypeChecker

        def check_one() -> tuple[bool, list[tuple[str, str]]]:
            chk = TypeChecker.__new__(TypeChecker)
            msgs: list[tuple[str, str]] = []
            chk.fail = lambda msg, ctx: msgs.append(("fail", str(msg)))  # type: ignore[assignment, misc]
            ret = chk.check_compatibility_classvar_super(node, base, base_node)
            return ret, msgs

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on  # type: ignore[return-value]

    def _assert_par(self, node: Any, base: Any, base_node: Any) -> None:
        off, on = self._run(node, base, base_node)
        assert_equal(
            on, off, f"check_compatibility_classvar_super parity for base_node={base_node!r}"
        )

    def test_seam_not_var(self) -> None:
        base_node = self._funcdef("base_method")
        assert self._tag(base_node, True) == 0
        assert self._tag(base_node, False) == 0

    def test_seam_none_base_node(self) -> None:
        assert self._tag(None, True) == 0
        assert self._tag(None, False) == 0

    def test_seam_both_classvar_ok(self) -> None:
        base_node = self._var("base_attr", True)
        assert self._tag(base_node, True) == 1

    def test_seam_both_not_classvar_ok(self) -> None:
        base_node = self._var("base_attr", False)
        assert self._tag(base_node, False) == 1

    def test_seam_instance_var_violation(self) -> None:
        base_node = self._var("base_attr", False)
        assert self._tag(base_node, True) == 2

    def test_seam_class_var_violation(self) -> None:
        base_node = self._var("base_attr", True)
        assert self._tag(base_node, False) == 3

    def test_parity_every_branch(self) -> None:
        node_cv = self._var("attr", True)
        node_iv = self._var("attr", False)
        base = self._base("mod.Base")
        # None base node: pass.
        self._assert_par(node_iv, base, None)
        # Non-Var base node: pass.
        self._assert_par(node_cv, base, self._funcdef("base_method"))
        # Both classvar: pass, no message.
        self._assert_par(node_cv, base, self._var("base_attr", True))
        # Both instance var: pass, no message.
        self._assert_par(node_iv, base, self._var("base_attr", False))
        # node classvar, base instance var: instance-var violation.
        self._assert_par(node_cv, base, self._var("base_attr", False))
        # node instance var, base classvar: class-var violation.
        self._assert_par(node_iv, base, self._var("base_attr", True))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeNewSignatureSuite(Suite):
    """Parity for the Rust `check___new___signature` 3-way dispatch port.

    `TypeChecker.check___new___signature` (checker.py:2630) picks one of
    three branches from two scalar facts: `fdef.info.is_metaclass()` and
    whether `get_proper_type(bound_type.ret_type)` is one of the five
    instance-kinds (AnyType / Instance / TupleType / UninhabitedType /
    LiteralType). The Rust classifier (`checker_functions.rs`) turns those
    into a branch tag; the Python shim keeps the two `check_subtype` calls
    and the `INVALID_NEW_TYPE` / `NON_INSTANCE_NEW_TYPE` emission.

    Direct seam calls assert the exact tag for every branch; the gate-off
    vs gate-on differential drives the real TypeChecker method through a
    stub message / subtype recorder and asserts identical records.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:

        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _tag(self, is_metaclass: bool, is_instance_ret: bool) -> int | None:
        return _type_kernel.rust_classify_new_signature(is_metaclass, is_instance_ret)

    def _info(self, *, metaclass: bool = False) -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        defn = ClassDef("A", Block([]), None, [])
        defn.fullname = "abc.ABCMeta" if metaclass else "mod.A"
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        return info

    def _fdef(self, info: TypeInfo) -> FuncDef:
        fdef = FuncDef("__new__")
        fdef.info = info
        return fdef

    def _run(self, metaclass: bool, ret_type: Type) -> tuple[object, object]:
        from mypy.checker import TypeChecker
        from mypy.options import Options

        info = self._info(metaclass=metaclass)
        fdef = self._fdef(info)
        fx = TypeFixture()
        typ = CallableType(
            [AnyType(TypeOfAny.special_form)], [ARG_POS], ["cls"], ret_type, fx.function
        )

        def check_one() -> list[tuple[Any, ...]]:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            chk.type_type = lambda: fx.type_type  # type: ignore[method-assign]
            records: list[tuple[Any, ...]] = []

            def record_subtype(
                subtype: Type, supertype: Type, _ctx: Any, msg: Any, l1: Any = None, l2: Any = None
            ) -> bool:
                records.append(("check_subtype", msg.value, str(subtype), str(supertype)))
                return True

            def record_fail(msg: Any, _ctx: Any) -> None:
                records.append(("fail", msg.value if hasattr(msg, "value") else msg))

            chk.check_subtype = record_subtype  # type: ignore[method-assign, assignment]
            chk.fail = record_fail  # type: ignore[method-assign, assignment]
            chk.check___new___signature(fdef, typ)
            return records

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, metaclass: bool, ret_type: Type) -> None:
        off, on = self._run(metaclass, ret_type)
        assert_equal(
            on, off, f"check___new___signature parity metaclass={metaclass} ret={ret_type}"
        )

    def test_seam_metaclass(self) -> None:
        assert self._tag(True, True) == 0
        assert self._tag(True, False) == 0

    def test_seam_non_instance(self) -> None:
        assert self._tag(False, False) == 1

    def test_seam_instance(self) -> None:
        assert self._tag(False, True) == 2

    def test_parity_metaclass(self) -> None:
        info = self._info(metaclass=True)
        self._assert_par(True, Instance(info, []))

    def test_parity_non_instance(self) -> None:
        fx = TypeFixture()
        callable_ret = CallableType([], [], [], AnyType(TypeOfAny.special_form), fx.function)
        self._assert_par(False, callable_ret)

    def test_parity_instance(self) -> None:
        info = self._info()
        self._assert_par(False, Instance(info, []))

    def test_parity_any(self) -> None:
        self._assert_par(False, AnyType(TypeOfAny.special_form))

    def test_parity_uninhabited(self) -> None:
        self._assert_par(False, UninhabitedType())

    def test_parity_tuple(self) -> None:
        fx = TypeFixture()
        self._assert_par(False, fx.std_tuple)

    def test_parity_literal(self) -> None:
        fx = TypeFixture()
        self._assert_par(False, LiteralType("x", fx.str_type))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFuncDefOverrideSuite(Suite):
    """Parity for the Rust `check_func_def_override` 5-way dispatch port.

    `TypeChecker.check_func_def_override` (checker.py:2106-2162) classifies
    the override into five arms plus an implicit no-op from scalar facts:
    original_def is a FuncDef, orig_type is None, orig_type is a partial
    with/without a resolved type, and the invalid-redefinition flag. The Rust
    classifier (`checker_functions.rs`) returns a branch tag; every branch
    body (function_type/is_same_type, partial fill, binder assign,
    check_subtype, error emission) stays in Python. Direct seam calls assert
    the exact tag; the gate-off vs gate-on differential drives the real
    TypeChecker method through stubs and asserts identical observations.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _tag(
        self,
        is_funcdef: bool,
        orig_none: bool,
        is_partial: bool,
        partial_none: bool,
        invalid: bool,
    ) -> int:
        return _type_kernel.rust_classify_func_def_override(
            is_funcdef, orig_none, is_partial, partial_none, invalid
        )

    def _any(self) -> Any:
        return AnyType(TypeOfAny.explicit)

    def _run(
        self, make: Callable[[], tuple[FuncDef, Any]], old_type_fn: Callable[[], Any] | None = None
    ) -> tuple[object, object]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> tuple[object, ...]:
            defn, new_type = make()
            chk = TypeChecker.__new__(TypeChecker)
            obs: list[object] = []
            old_type = old_type_fn() if old_type_fn is not None else new_type
            chk.function_type = lambda d: old_type  # type: ignore[assignment]
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                incompatible_conditional_function_def=lambda d, ot, nt: obs.append(
                    ("incompatible_cond", d.name)
                )
            )
            chk.find_partial_types = lambda var: {var: True}  # type: ignore[method-assign, dict-item]
            chk.fail = lambda msg, ctx: obs.append(("fail", msg))  # type: ignore[method-assign, misc, assignment]
            chk.binder = SimpleNamespace(  # type: ignore[assignment]
                assign_type=lambda expr, nt, ot: obs.append(("assign", expr.name))
            )
            chk.check_subtype = lambda nt, ot, ctx, msg, d1, d2: obs.append(("subtype",))  # type: ignore[method-assign, assignment]
            chk.check_func_def_override(defn, new_type)
            orig_def = defn.original_def
            if isinstance(orig_def, FuncDef):
                orig_type_repr = "FuncDef"
            else:
                orig_type_repr = repr(orig_def.type) if orig_def is not None else "None"
            return (tuple(obs), defn.is_invalid_redefinition, orig_type_repr)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(
        self, make: Callable[[], tuple[FuncDef, Any]], old_type_fn: Callable[[], Any] | None = None
    ) -> None:
        off, on = self._run(make, old_type_fn)
        assert_equal(on, off, "check_func_def_override parity")

    # ---- direct seam tag tests ----

    def test_seam_func_over_func(self) -> None:
        assert self._tag(True, False, False, False, False) == 0
        assert self._tag(True, True, True, True, True) == 0

    def test_seam_orig_type_none(self) -> None:
        assert self._tag(False, True, False, False, False) == 1

    def test_seam_fill_partial(self) -> None:
        assert self._tag(False, False, True, True, False) == 2

    def test_seam_partial_invalid(self) -> None:
        assert self._tag(False, False, True, False, False) == 3

    def test_seam_binder_assign(self) -> None:
        assert self._tag(False, False, False, False, False) == 4

    def test_seam_no_op(self) -> None:
        assert self._tag(False, False, False, False, True) == 5

    # ---- gate-off vs gate-on differential tests ----

    def _func_over_func(self) -> tuple[FuncDef, Any]:
        orig = FuncDef("old")
        defn = FuncDef("f")
        defn.original_def = orig
        return defn, self._any()

    def test_parity_func_over_func(self) -> None:
        self._assert_par(self._func_over_func)

    def test_parity_func_over_func_incompatible(self) -> None:
        self._assert_par(self._func_over_func, lambda: NoneType())

    def _orig_none(self) -> tuple[FuncDef, Any]:
        orig = Var("v")
        orig.type = None
        defn = FuncDef("f")
        defn.original_def = orig
        return defn, self._any()

    def test_parity_orig_none(self) -> None:
        self._assert_par(self._orig_none)

    def _fill_partial(self) -> tuple[FuncDef, Any]:

        orig = Var("v")
        orig.type = PartialType(None, orig)
        defn = FuncDef("f")
        defn.original_def = orig
        return defn, self._any()

    def test_parity_fill_partial(self) -> None:
        self._assert_par(self._fill_partial)

    def _partial_invalid(self) -> tuple[FuncDef, Any]:

        orig = Var("v")
        info = self._fake_info()
        orig.type = PartialType(info, orig)
        defn = FuncDef("f")
        defn.original_def = orig
        return defn, self._any()

    def test_parity_partial_invalid(self) -> None:
        self._assert_par(self._partial_invalid)

    def _binder_assign(self) -> tuple[FuncDef, Any]:
        orig = Var("v")
        orig.type = self._any()
        defn = FuncDef("f")
        defn.is_invalid_redefinition = False
        defn.original_def = orig
        return defn, self._any()

    def test_parity_binder_assign(self) -> None:
        self._assert_par(self._binder_assign)

    def _no_op(self) -> tuple[FuncDef, Any]:
        orig = Var("v")
        orig.type = self._any()
        defn = FuncDef("f")
        defn.is_invalid_redefinition = True
        defn.original_def = orig
        return defn, self._any()

    def test_parity_no_op(self) -> None:
        self._assert_par(self._no_op)

    def _fill_partial_decorator(self) -> tuple[FuncDef, Any]:

        inner = FuncDef("old")
        var = Var("v")
        var.type = PartialType(None, var)
        decorator = Decorator(inner, [], var)
        defn = FuncDef("f")
        defn.original_def = decorator
        return defn, self._any()

    def test_parity_fill_partial_decorator(self) -> None:
        self._assert_par(self._fill_partial_decorator)

    def _fake_info(self) -> Any:
        from mypy.nodes import Block, SymbolTable, TypeInfo as _TypeInfo

        defn = ClassDef("List", Block([]), None, [])
        defn.fullname = "builtins.list"
        info = _TypeInfo(SymbolTable(), defn, "builtins")
        return info


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeEnumNewSuite(Suite):
    """Parity for the Rust `check_enum_new` base-fold decision port.

    `TypeChecker.check_enum_new` (checker.py:3739-3766) folds over
    `defn.info.bases`. An enum base scans `mro[1:-1]` for a non-enum
    mixin exposing `__new__`; a non-enum base checks `__new__` directly;
    a second mixin raises the single-data-type-mixin error. The Rust
    classifier (`checker_functions.rs`) returns one SKIP/ADVANCE/CONFLICT
    tag per base and the shim applies `self.fail` while tracking
    `has_new`. Direct seam calls assert exact tags; the gate-off vs
    gate-on differential compares recorded message lists.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _node(self, fullname: str) -> SymbolTableNode:
        fn = FuncDef("__new__")
        fn._fullname = fullname
        return SymbolTableNode(MDEF, fn)

    def _info(self, name: str, *, is_enum: bool, has_new: bool = False) -> TypeInfo:
        info = self.fx.make_type_info(name)
        info.is_enum = is_enum
        if has_new:
            info.names["__new__"] = self._node(f"{name}.__new__")
        return info

    def _seam(self, bases: list[Instance]) -> list[int] | None:
        return _type_kernel.rust_classify_enum_new(bases)

    def _sub(self, bases: list[Instance]) -> Any:
        sub = self.fx.make_type_info("mod.Sub")
        sub.bases = bases
        sub.defn.info = sub
        return sub.defn

    def _run(self, bases: list[Instance]) -> tuple[Any, Any]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> list[tuple[str, object]]:
            chk = TypeChecker.__new__(TypeChecker)
            msgs: list[tuple[str, object]] = []
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                fail=lambda msg, ctx, code=None: msgs.append((str(msg), code))
            )
            chk.options = Options()
            chk.check_enum_new(self._sub(bases))
            return msgs

        return self._with_gate(False, check_one), self._with_gate(True, check_one)

    def test_seam_non_list_defers(self) -> None:
        assert self._seam("nope") is None  # type: ignore[arg-type]

    def test_seam_non_enum_no_new(self) -> None:
        base = self._info("mod.Plain", is_enum=False)
        assert self._seam([Instance(base, [])]) == [0]

    def test_seam_non_enum_has_new(self) -> None:
        base = self._info("mod.Mixin", is_enum=False, has_new=True)
        assert self._seam([Instance(base, [])]) == [1]

    def test_seam_enum_mro_has_new(self) -> None:
        enum = self._info("enum.Enum", is_enum=True)
        mixin = self._info("mod.Mixin", is_enum=False, has_new=True)
        enum.mro = [enum, mixin, self.fx.oi]
        assert self._seam([Instance(enum, [])]) == [1]

    def test_seam_enum_mro_all_enum(self) -> None:
        enum = self._info("enum.Enum", is_enum=True)
        enum2 = self._info("enum.Parent", is_enum=True)
        enum.mro = [enum, enum2, self.fx.oi]
        assert self._seam([Instance(enum, [])]) == [0]

    def test_seam_conflict(self) -> None:
        a = self._info("mod.A", is_enum=False, has_new=True)
        b = self._info("mod.B", is_enum=False, has_new=True)
        assert self._seam([Instance(a, []), Instance(b, [])]) == [1, 2]

    def test_parity_single_mixin(self) -> None:
        a = self._info("mod.A", is_enum=False, has_new=True)
        off, on = self._run([Instance(a, [])])
        assert_equal(on, off, "single non-enum mixin")

    def test_parity_two_mixins(self) -> None:
        a = self._info("mod.A", is_enum=False, has_new=True)
        b = self._info("mod.B", is_enum=False, has_new=True)
        off, on = self._run([Instance(a, []), Instance(b, [])])
        assert_equal(on, off, "two non-enum mixins")

    def test_parity_enum_plus_mixin(self) -> None:
        enum = self._info("enum.Enum", is_enum=True)
        mixin = self._info("mod.Mixin", is_enum=False, has_new=True)
        enum.mro = [enum, mixin, self.fx.oi]
        off, on = self._run([Instance(enum, [])])
        assert_equal(on, off, "enum base with mixin in mro")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeEnumBasesSuite(Suite):
    """Parity for the Rust `check_enum_bases` fold port.

    `TypeChecker.check_enum_bases` (checker.py:3850) folds over
    `defn.info.bases`; once an enum base is seen, a later non-enum
    mixin base is an error. The Rust classifier
    (`checker_functions.rs`) returns `(enum_base_idx, violating_idx)`
    and the shim applies `self.fail` with
    `enum_base.str_with_options`. Direct seam calls assert exact
    indices; the gate-off vs gate-on differential compares recorded
    message lists.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _info(self, name: str, *, is_enum: bool) -> TypeInfo:
        info = self.fx.make_type_info(name)
        info.is_enum = is_enum
        return info

    def _base(self, name: str, *, is_enum: bool) -> Instance:
        return Instance(self._info(name, is_enum=is_enum), [])

    def _seam(self, bases: list[Instance]) -> tuple[int, int] | None:
        return _type_kernel.rust_classify_enum_bases(bases)

    def _sub(self, bases: list[Instance]) -> Any:
        sub = self.fx.make_type_info("mod.Sub")
        sub.bases = bases
        sub.defn.info = sub
        return sub.defn

    def _run(self, bases: list[Instance]) -> tuple[Any, Any]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> list[tuple[str, object]]:
            chk = TypeChecker.__new__(TypeChecker)
            msgs: list[tuple[str, object]] = []
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                fail=lambda msg, ctx, code=None: msgs.append((str(msg), code))
            )
            chk.options = Options()
            chk.check_enum_bases(self._sub(bases))
            return msgs

        return self._with_gate(False, check_one), self._with_gate(True, check_one)

    def test_seam_non_list_defers(self) -> None:
        assert self._seam("nope") is None  # type: ignore[arg-type]

    def test_seam_no_enum(self) -> None:
        a = self._base("mod.A", is_enum=False)
        assert self._seam([a]) == (-1, -1)

    def test_seam_enum_only(self) -> None:
        e = self._base("enum.Enum", is_enum=True)
        assert self._seam([e]) == (0, -1)

    def test_seam_enum_then_nonenum(self) -> None:
        e = self._base("enum.Enum", is_enum=True)
        m = self._base("mod.Mixin", is_enum=False)
        assert self._seam([e, m]) == (0, 1)

    def test_seam_nonenum_enum_nonenum(self) -> None:
        m1 = self._base("mod.Mixin1", is_enum=False)
        e = self._base("enum.Enum", is_enum=True)
        m2 = self._base("mod.Mixin2", is_enum=False)
        assert self._seam([m1, e, m2]) == (1, 2)

    def test_parity_no_enum(self) -> None:
        a = self._base("mod.A", is_enum=False)
        off, on = self._run([a])
        assert_equal(on, off, "no enum mixin")

    def test_parity_enum_only(self) -> None:
        e = self._base("enum.Enum", is_enum=True)
        off, on = self._run([e])
        assert_equal(on, off, "enum only")

    def test_parity_enum_then_nonenum(self) -> None:
        e = self._base("enum.Enum", is_enum=True)
        m = self._base("mod.Mixin", is_enum=False)
        off, on = self._run([e, m])
        assert_equal(on, off, "enum then non-enum mixin")

    def test_parity_nonenum_enum_nonenum(self) -> None:
        m1 = self._base("mod.Mixin1", is_enum=False)
        e = self._base("enum.Enum", is_enum=True)
        m2 = self._base("mod.Mixin2", is_enum=False)
        off, on = self._run([m1, e, m2])
        assert_equal(on, off, "non-enum, enum, non-enum")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCanBeNarrowedWithLenSuite(Suite):
    """Parity for the Rust len-narrowing gate predicate (#1065).

    `TypeChecker.can_be_narrowed_with_len` (checker.py:9267) is consulted
    at the leaf of every `find_isinstance_check` conditional. The Rust
    port (`lennarrow.rs`) decides from the wire type + resolver snapshot
    and defers (None) on an unresolved MRO/alias; the shim falls through
    to the pure-Python body then. Direct seam calls assert the exact
    bool; the gate-off vs gate-on differential drives the real
    TypeChecker method.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._typeinfos: list[TypeInfo] = []
        self._rebuild_resolver()
        _set_native_checker_active(True)
        _set_native_checker_resolver(self._resolver)
        set_wire_typeinfo_map(
            {info.fullname: info for info in self._typeinfos + _base_infos(self.fx)}
        )

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)

    def _rebuild_resolver(self) -> None:
        import type_kernel as _tk

        from mypy.checker import _set_native_checker_resolver

        infos = self._typeinfos + _base_infos(self.fx)
        self._resolver = _tk.build_native_resolver(infos, [])
        _set_native_checker_resolver(self._resolver)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def _run(self, typ: Type) -> tuple[bool, bool]:
        from mypy.checker import TypeChecker

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.can_be_narrowed_with_len(typ)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, typ: Type, expected: bool | None = None, defers: bool = False) -> None:
        from mypy.checker import _serialize_type_for_checker

        off, on = self._run(typ)
        assert on == off, f"can_be_narrowed_with_len parity {typ!r}: off={off} on={on}"
        if expected is not None:
            assert on == expected, f"can_be_narrowed_with_len {typ!r}: {on} != {expected}"
        # The seam must engage (not defer) for the differential to mean
        # anything; alias/missing-snapshot cases assert deferral directly.
        result = _type_kernel.rust_can_be_narrowed_with_len(
            _serialize_type_for_checker(typ), self._resolver
        )
        if defers:
            assert result is None, f"Rust len gate expected defer for {typ!r}"
        else:
            assert result is not None, f"Rust len gate did not engage for {typ!r}"

    def _seam(self, typ: Type, resolver: Any) -> bool | None:
        from mypy.checker import _serialize_type_for_checker

        return _type_kernel.rust_can_be_narrowed_with_len(
            _serialize_type_for_checker(typ), resolver
        )

    def _info(self, fullname: str) -> TypeInfo:

        from mypy.wirefixup import set_wire_typeinfo_map

        info = self.fx.make_type_info(fullname)
        self._typeinfos.append(info)
        self._rebuild_resolver()
        set_wire_typeinfo_map({i.fullname: i for i in self._typeinfos + _base_infos(self.fx)})
        return info

    def test_seam_fixed_tuple(self) -> None:
        t = TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)
        assert self._seam(t, self._resolver) is True

    def test_seam_tuple_instance(self) -> None:
        assert self._seam(self.fx.std_tuple, self._resolver) is True

    def test_seam_non_tuple_instance(self) -> None:
        assert self._seam(self.fx.a, self._resolver) is False

    def test_seam_custom_len_false(self) -> None:
        info = self._info("mod.Len")
        node = FuncDef("__len__", [], None, None)
        node.info = info
        info.names["__len__"] = SymbolTableNode(MDEF, node)
        self._rebuild_resolver()
        assert self._seam(Instance(info, []), self._resolver) is False

    def test_seam_missing_snapshot_defers(self) -> None:
        import type_kernel as _tk

        empty = _tk.build_native_resolver([], [])
        assert self._seam(self.fx.std_tuple, empty) is None

    def test_seam_alias_defers(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.std_tuple, "mod.Alias", "mod", -1, -1)
        assert self._seam(TypeAliasType(alias, []), self._resolver) is None

    def test_parity_fixed_tuple(self) -> None:
        self._assert_par(TupleType([self.fx.a, self.fx.b], self.fx.std_tuple), True)

    def test_parity_tuple_instance(self) -> None:
        self._assert_par(self.fx.std_tuple, True)

    def test_parity_non_tuple(self) -> None:
        self._assert_par(self.fx.a, False)
        self._assert_par(AnyType(TypeOfAny.special_form), False)

    def test_parity_custom_len(self) -> None:
        info = self._info("mod.Len")
        node = FuncDef("__len__", [], None, None)
        node.info = info
        info.names["__len__"] = SymbolTableNode(MDEF, node)
        self._rebuild_resolver()
        self._assert_par(Instance(info, []), False)

    def test_parity_union_of_tuples(self) -> None:
        u = UnionType([TupleType([self.fx.a], self.fx.std_tuple), self.fx.std_tuple])
        self._assert_par(u, True)

    def test_parity_union_without_tuple(self) -> None:
        u = UnionType([self.fx.a, self.fx.b])
        self._assert_par(u, False)

    def test_parity_union_with_custom_len_item(self) -> None:
        # A custom-__len__ item poisons the whole union -> False.
        info = self._info("mod.Len")
        node = FuncDef("__len__", [], None, None)
        node.info = info
        info.names["__len__"] = SymbolTableNode(MDEF, node)
        self._rebuild_resolver()
        u = UnionType([Instance(info, []), self.fx.std_tuple])
        self._assert_par(u, False)

    def test_parity_alias_falls_back(self) -> None:
        # The seam defers on a TypeAliasType; the shim must fall through
        # to the pure-Python body, which expands the alias to tuple.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.std_tuple, "mod.Alias", "mod", -1, -1)
        self._assert_par(TypeAliasType(alias, []), True, defers=True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeRequiresUsageSuite(Suite):
    """Parity for the `rust_type_requires_usage` __await__ branch port.

    `mypy.checker.TypeChecker.type_requires_usage` decides whether an
    expression-statement's type requires a usage note: the
    `typing.Coroutine` branch (UNUSED_COROUTINE) and the
    `proper_type.type.get("__await__")` branch (UNUSED_AWAITABLE). The
    Rust seam mirrors both on the wire type plus the resolver's member
    snapshots; the awaitable branch is the newly-ported one (previously
    deferred). Toggling the checker-stmts gate off (pure Python) and on
    (Rust seam) must produce identical note/code results, and a direct
    seam call proves the Rust function engages rather than silently
    deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_resolver, _set_native_checker_stmts_active

        self.fx = TypeFixture()

        # A class with an __await__ method and one without; both with a
        # mro through builtins.object so the resolver snapshot carries the
        # member_info map (names).
        self.awaitablei = self.fx.make_type_info("mod.AwaitableImpl", mro=[self.fx.oi])
        self.plaini = self.fx.make_type_info("mod.Plain", mro=[self.fx.oi])
        self.awaitablei.names["__await__"] = SymbolTableNode(MDEF, FuncDef("__await__"))
        # Coroutine-like fullname: the typing.Coroutine branch keys on
        # the Instance's type_ref, so build an info with that fullname.
        self.coroi = self.fx.make_type_info("typing.Coroutine", mro=[self.fx.oi])

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
                self.fx.str_type_info,
                self.fx.bool_type_info,
                self.awaitablei,
                self.plaini,
                self.coroi,
            ]
        )
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active = _set_native_checker_stmts_active
        self._set_resolver = _set_native_checker_resolver
        self._set_active(True)
        self._set_resolver(self.resolver)

    def tearDown(self) -> None:
        self._set_active(False)
        self._set_resolver(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _assert_par(self, typ: Type, label: str) -> None:
        from mypy.checker import TypeChecker

        chk = TypeChecker.__new__(TypeChecker)
        off = self._with_gate(False, lambda: chk.type_requires_usage(typ))
        on = self._with_gate(True, lambda: chk.type_requires_usage(typ))
        assert_equal(on, off, f"type_requires_usage parity {label}")

    def _assert_engages(self, typ: Type, expected_code: int) -> None:

        self._assert_engages_with(typ, expected_code, self.resolver)

    def _assert_engages_with(self, typ: Type, expected_code: int, resolver: Any) -> None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_type_requires_usage(_serialize_type_for_checker(typ), resolver)
        assert (
            result == expected_code
        ), f"rust_type_requires_usage({typ}) = {result!r}, expected {expected_code!r}"

    def test_awaitable_class_returns_awaitable_note(self) -> None:
        # Instance with __await__ in names -> UNUSED_AWAITABLE.
        typ = Instance(self.awaitablei, [])
        self._assert_par(typ, "awaitable")
        self._assert_engages(typ, 1)

    def test_plain_class_no_note(self) -> None:
        # No __await__ anywhere in the mro -> Python returns None; the Rust
        # conclusion (Some(false)) also yields no note, so the shim's None
        # defers back into Python which agrees.
        typ = Instance(self.plaini, [])
        self._assert_par(typ, "plain")
        # Direct seam: Some(false) -> the shim returns None, so the seam
        # engages but produces no note. Assert the seam decided (Some)
        # rather than deferring (None).
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_type_requires_usage(
            _serialize_type_for_checker(typ), self.resolver
        )
        assert result is not None, "rust_type_requires_usage plain deferred"
        assert result == 2

    def test_coroutine_fullname_note(self) -> None:
        # Instance whose type_ref is typing.Coroutine -> UNUSED_COROUTINE.
        typ = Instance(self.coroi, [])
        self._assert_par(typ, "coroutine")
        self._assert_engages(typ, 0)

    def test_awaitable_inherited_note(self) -> None:
        # Subclass whose base has __await__: TypeInfo.get walks the mro.
        subi = self.fx.make_type_info("mod.Sub", mro=[self.awaitablei, self.fx.oi])
        type_infos = self.resolver_len_collect(subi)
        sub_resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(sub_resolver)
        try:
            typ = Instance(subi, [])
            self._assert_par(typ, "inherited awaitable")
            self._assert_engages_with(typ, 1, sub_resolver)
        finally:
            self._set_resolver(self.resolver)

    def test_alias_defers_to_python(self) -> None:
        # TypeAliasType: the wire format carries no alias target, so Rust
        # defers and Python expands via get_proper_type.
        alias_typ = self.fx.non_rec_alias(Instance(self.plaini, []))
        self._assert_par(alias_typ, "alias")

    def resolver_len_collect(self, *extra: TypeInfo) -> list[TypeInfo]:
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
                self.fx.str_type_info,
                self.fx.bool_type_info,
                self.awaitablei,
                self.plaini,
                self.coroi,
            ]
        )
        type_infos.extend(extra)
        return type_infos


class NativeTypeRequiresUsageTailSuite(NativeTypeRequiresUsageSuite):
    """Parity for the ported non-Instance tail of `rust_type_requires_usage`.

    Python's `type_requires_usage` yields no note for every proper type
    that is not an Instance. The Rust port now decides those cases
    directly (Some(2), skipping the Python body); gate-on and gate-off
    must agree, and the seam must engage rather than defer.
    """

    def test_bare_none_no_note(self) -> None:
        typ = NoneType()
        self._assert_par(typ, "NoneType")
        self._assert_engages(typ, 2)

    def test_any_no_note(self) -> None:
        typ = self.fx.anyt
        self._assert_par(typ, "AnyType")
        self._assert_engages(typ, 2)

    def test_union_no_note(self) -> None:
        typ = UnionType([self.fx.anyt, self.fx.nonet])
        self._assert_par(typ, "UnionType")
        self._assert_engages(typ, 2)

    def test_tuple_no_note(self) -> None:
        typ = TupleType([self.fx.anyt, self.fx.nonet], self.fx.std_tuple)
        self._assert_par(typ, "TupleType")
        self._assert_engages(typ, 2)

    def test_callable_no_note(self) -> None:
        typ = self.fx.callable(self.fx.anyt, self.fx.nonet)
        self._assert_par(typ, "CallableType")
        self._assert_engages(typ, 2)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMetaclassCompatibilitySuite(Suite):
    """Parity for the Rust `check_metaclass_compatibility` decision-head port.

    `TypeChecker.check_metaclass_compatibility` (checker.py:3918-3941) first
    exempts metaclasses, protocols, named tuples, enums, and TypedDicts,
    then flags a metaclass conflict when the class has no metaclass but a
    base does. The Rust classifier (`checker_functions.rs`) reads the live
    `TypeInfo` facts via PyO3 and returns a branch tag; the Python shim
    applies the `self.fail` (METACLASS code) and `explain_metaclass_conflict`
    + `self.note` side effects, and keeps the pure-Python body as fallback.

    Direct seam calls assert the exact tag for every branch; the gate-off vs
    gate-on differential drives the real TypeChecker method through a stub
    fail/note recorder and asserts identical (fail, note) pairs.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:

        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _typeinfo(
        self,
        *,
        fullname: str = "mod.A",
        is_metaclass: bool = False,
        is_protocol: bool = False,
        is_named_tuple: bool = False,
        is_enum: bool = False,
        typeddict_type: object = None,
        metaclass_type: object = None,
        bases: list[Any] | None = None,
    ) -> TypeInfo:
        from mypy.nodes import Block, SymbolTable, TypeInfo

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.is_protocol = is_protocol
        info.is_named_tuple = is_named_tuple
        info.is_enum = is_enum
        info.typeddict_type = typeddict_type  # type: ignore[assignment]
        info.metaclass_type = metaclass_type  # type: ignore[assignment]
        info.bases = list(bases) if bases is not None else []
        # Give info a minimal MRO so is_metaclass() (which calls has_base)
        # does not crash: [info] alone suffices for the non-metaclass cases.
        info.mro = [info]
        return info

    def _base_instance(self, base_type: TypeInfo) -> Any:
        from types import SimpleNamespace

        return SimpleNamespace(type=base_type)

    def _tag(self, typ: TypeInfo) -> int | None:
        return _type_kernel.rust_classify_metaclass_compat(typ)

    def test_seam_exempt_metaclass(self) -> None:
        # A TypeInfo that is a metaclass: pass. We approximate by giving it
        # `builtins.type` in its MRO so is_metaclass() returns True.
        t = self._typeinfo(fullname="builtins.type")
        t.mro.append(t)  # harmless; is_metaclass short-circuits on has_base
        # Force has_base to True by appending a fake base with the fullname.
        from types import SimpleNamespace

        fake_type = SimpleNamespace(fullname="builtins.type")
        t.mro = [t, fake_type]  # type: ignore[list-item]
        assert self._tag(t) == 0

    def test_seam_exempt_protocol(self) -> None:
        t = self._typeinfo(is_protocol=True)
        assert self._tag(t) == 0

    def test_seam_exempt_named_tuple(self) -> None:
        t = self._typeinfo(is_named_tuple=True)
        assert self._tag(t) == 0

    def test_seam_exempt_enum(self) -> None:
        t = self._typeinfo(is_enum=True)
        assert self._tag(t) == 0

    def test_seam_exempt_typeddict(self) -> None:
        from types import SimpleNamespace

        td = SimpleNamespace()  # non-None placeholder
        t = self._typeinfo(typeddict_type=td)
        assert self._tag(t) == 0

    def test_seam_conflict(self) -> None:
        from types import SimpleNamespace

        base_type = self._typeinfo(fullname="mod.Meta")
        base_type.metaclass_type = SimpleNamespace()  # type: ignore[assignment]
        base = self._base_instance(base_type)
        t = self._typeinfo(bases=[base])
        assert self._tag(t) == 1

    def test_seam_no_conflict_class_has_metaclass(self) -> None:
        from types import SimpleNamespace

        base_type = self._typeinfo(fullname="mod.Meta")
        base_type.metaclass_type = SimpleNamespace()  # type: ignore[assignment]
        base = self._base_instance(base_type)
        mc = SimpleNamespace()
        t = self._typeinfo(metaclass_type=mc, bases=[base])
        assert self._tag(t) == 0

    def test_seam_no_conflict_no_base_metaclass(self) -> None:
        base_type = self._typeinfo(fullname="mod.Base")
        base = self._base_instance(base_type)
        t = self._typeinfo(bases=[base])
        assert self._tag(t) == 0

    def test_seam_exempt_wins_over_conflict(self) -> None:
        from types import SimpleNamespace

        base_type = self._typeinfo(fullname="mod.Meta")
        base_type.metaclass_type = SimpleNamespace()  # type: ignore[assignment]
        base = self._base_instance(base_type)
        # Protocol + conflict-shape: exemption short-circuits.
        t = self._typeinfo(is_protocol=True, bases=[base])
        assert self._tag(t) == 0

    def _run(self, typ: TypeInfo) -> tuple[list[str], list[str]]:
        from mypy.checker import TypeChecker

        def check_one() -> tuple[list[str], list[str]]:
            chk = TypeChecker.__new__(TypeChecker)
            fails: list[str] = []
            notes: list[str] = []
            chk.fail = lambda msg, ctx, *, code=None: fails.append(msg)  # type: ignore[arg-type, assignment, method-assign, return-value]
            chk.note = lambda msg, ctx, *, code=None: notes.append(msg)  # type: ignore[assignment, method-assign, misc]
            chk.check_metaclass_compatibility(typ)
            return fails, notes

        return check_one()

    def _assert_par(self, typ: TypeInfo) -> None:
        off = self._with_gate(False, lambda: self._run(typ))
        on = self._with_gate(True, lambda: self._run(typ))
        assert_equal(on, off, f"check_metaclass_compatibility parity for {typ!r}")

    def test_parity_exempt_protocol(self) -> None:
        t = self._typeinfo(is_protocol=True)
        self._assert_par(t)

    def test_parity_exempt_enum(self) -> None:
        t = self._typeinfo(is_enum=True)
        self._assert_par(t)

    def test_parity_exempt_named_tuple(self) -> None:
        t = self._typeinfo(is_named_tuple=True)
        self._assert_par(t)

    def test_parity_conflict(self) -> None:
        from types import SimpleNamespace

        base_type = self._typeinfo(fullname="mod.Meta")
        base_type.metaclass_type = SimpleNamespace()  # type: ignore[assignment]
        base = self._base_instance(base_type)
        t = self._typeinfo(bases=[base])
        self._assert_par(t)

    def test_parity_no_conflict(self) -> None:
        base_type = self._typeinfo(fullname="mod.Base")
        base = self._base_instance(base_type)
        t = self._typeinfo(bases=[base])
        self._assert_par(t)

    def test_parity_no_conflict_class_has_metaclass(self) -> None:
        from types import SimpleNamespace

        base_type = self._typeinfo(fullname="mod.Meta")
        base_type.metaclass_type = SimpleNamespace()  # type: ignore[assignment]
        base = self._base_instance(base_type)
        mc = SimpleNamespace()
        t = self._typeinfo(metaclass_type=mc, bases=[base])
        self._assert_par(t)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMatchArgsSuite(Suite):
    """Parity for the Rust `check_match_args` predicate port (#986).

    `TypeChecker.check_match_args` (checker.py:3128) guards on
    `self.scope.active_class()`, then checks that `get_proper_type(typ)`
    is a `TupleType` whose every item is a string literal. If not, it emits
    a `LITERAL_REQ` note. The Rust seam (`checker_functions.rs`) reads one
    wire Type and returns `isinstance(TupleType) and
    all(is_string_literal(item))` as a bool; the active_class gate and the
    note emission stay in Python.

    Direct seam calls assert the bool and the None deferrals; the gate-off
    vs gate-on differential drives the real TypeChecker method through a
    stub note recorder and asserts identical note lists.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:

        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _bool(self, type_bytes: bytes) -> bool | None:
        return _type_kernel.rust_check_match_args(type_bytes)

    def _bytes_of(self, t: Type) -> bytes:
        from mypy.checker import _serialize_type_for_checker

        return _serialize_type_for_checker(t)

    def _class_scope(self) -> Any:
        from mypy.checker_shared import CheckerScope
        from mypy.nodes import Block, SymbolTable

        info = TypeInfo(SymbolTable(), ClassDef("C", Block([]), None, []), "mod")
        scope = CheckerScope.__new__(CheckerScope)
        scope.stack = [info]
        return scope

    def _module_scope(self) -> Any:
        from mypy.checker_shared import CheckerScope
        from mypy.nodes import MypyFile

        mod = MypyFile([], [], False, {})
        scope = CheckerScope.__new__(CheckerScope)
        scope.stack = [mod]
        return scope

    def _run(self, scope: Any, typ: Type) -> tuple[list[str], list[str]]:
        from mypy.checker import TypeChecker
        from mypy.options import Options

        def check_one() -> list[str]:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            chk.scope = scope
            notes: list[str] = []

            class _Msg:
                def note(self, msg: str, _ctx: Any, **_kw: Any) -> None:
                    notes.append(str(msg))

            chk.msg = _Msg()  # type: ignore[assignment]
            chk.check_match_args(Var("__match_args__"), typ, Var("__match_args__"))
            return notes

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, scope: Any, typ: Type) -> None:
        off, on = self._run(scope, typ)
        assert_equal(on, off, f"check_match_args parity for typ={typ!r}")

    def test_seam_ok_all_string_literals(self) -> None:
        fx = TypeFixture()
        tup = TupleType([fx.lit_str1, fx.lit_str2], fx.std_tuple)
        assert self._bool(self._bytes_of(tup)) is True

    def test_seam_ok_empty_tuple(self) -> None:
        fx = TypeFixture()
        tup = TupleType([], fx.std_tuple)
        assert self._bool(self._bytes_of(tup)) is True

    def test_seam_fail_not_tuple(self) -> None:
        fx = TypeFixture()
        assert self._bool(self._bytes_of(fx.str_type)) is False

    def test_seam_fail_non_literal_item(self) -> None:
        fx = TypeFixture()
        tup = TupleType([fx.lit_str1, fx.a], fx.std_tuple)
        assert self._bool(self._bytes_of(tup)) is False

    def test_module_scope_precedes_the_gate(self) -> None:
        # A module scope returns at checker.py:3425, before the seam gate is
        # read, so the arm comparison cannot fail; the empty note list on
        # this non-literal tuple pins the skip (a class scope emits LITERAL_REQ).
        fx = TypeFixture()
        notes, _ = self._run(self._module_scope(), TupleType([fx.a], fx.std_tuple))
        assert_equal(notes, [])

    def test_parity_ok_all_string_literals(self) -> None:
        fx = TypeFixture()
        tup = TupleType([fx.lit_str1, fx.lit_str2], fx.std_tuple)
        self._assert_par(self._class_scope(), tup)

    def test_parity_ok_empty_tuple(self) -> None:
        fx = TypeFixture()
        self._assert_par(self._class_scope(), TupleType([], fx.std_tuple))

    def test_parity_fail_not_tuple(self) -> None:
        fx = TypeFixture()
        self._assert_par(self._class_scope(), fx.str_type)

    def test_parity_fail_non_literal_item(self) -> None:
        fx = TypeFixture()
        tup = TupleType([fx.lit_str1, fx.a], fx.std_tuple)
        self._assert_par(self._class_scope(), tup)

    def test_parity_fail_any_item(self) -> None:
        fx = TypeFixture()
        tup = TupleType([AnyType(TypeOfAny.special_form)], fx.std_tuple)
        self._assert_par(self._class_scope(), tup)

    def test_parity_fail_lkv_string(self) -> None:
        fx = TypeFixture()
        # Instance with last_known_value of a string literal is a string
        # literal by is_string_literal.
        tup = TupleType([fx.lit_str1_inst], fx.std_tuple)
        self._assert_par(self._class_scope(), tup)

    def test_seam_defer_alias_item(self) -> None:
        fx = TypeFixture()
        from mypy.nodes import TypeAlias

        alias = TypeAlias(fx.str_type, "mod.A", "mod", -1, -1)
        tup = TupleType([TypeAliasType(alias, [])], fx.std_tuple)
        assert self._bool(self._bytes_of(tup)) is None

    def test_parity_alias_item_defers_to_python(self) -> None:
        fx = TypeFixture()
        from mypy.nodes import TypeAlias

        alias = TypeAlias(fx.str_type, "mod.A", "mod", -1, -1)
        tup = TupleType([TypeAliasType(alias, [])], fx.std_tuple)
        self._assert_par(self._class_scope(), tup)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFindIsinstanceHeadSuite(Suite):
    """Parity for the Rust `find_isinstance_check` dispatch head port (#1086).

    `TypeChecker.find_isinstance_check_helper` (checker.py:8418) dispatches
    on the builtin callee (isinstance / issubclass / callable / hasattr)
    before falling into the TypeGuard/TypeIs extraction block. The Rust seam
    (`checker_functions.rs`) reads the live callee via PyO3 (zero wire
    bytes) and returns the arm tag; the shim applies the arm bodies. Defers
    (None) on a RefExpr naming a TypeAlias (refers_to_fullname unwraps the
    alias target) or any unreadable fact.

    Direct seam calls assert the tag per arm; the gate-off vs gate-on
    differential drives the real `find_isinstance_check_helper` through a
    stub TypeChecker (a `_type_maps` list is all the machinery the covered
    arms need) and asserts identical (if_map, else_map) results.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _seam(self, callee: Any, args_len: int, literal_ok: bool) -> Any:
        return _type_kernel.rust_classify_find_isinstance_head(callee, args_len, literal_ok)

    def _builtin_ref(self, fullname: str) -> Any:
        ref = NameExpr(fullname.rsplit(".", 1)[-1])
        ref.fullname = fullname
        return ref

    def _var_expr(self) -> NameExpr:
        # A NameExpr bound to a Var classifies as LITERAL_TYPE via literal().
        expr = NameExpr("x")
        expr.node = Var("x")
        return expr

    def _call(self, callee: Any, args: list[Expression]) -> CallExpr:
        return CallExpr(callee, args, [ARG_POS] * len(args), [None] * len(args))

    def _checker(self) -> Any:
        from mypy.checker import TypeChecker

        return TypeChecker.__new__(TypeChecker)

    def _run(self, node: CallExpr) -> Any:
        from mypy.checker import TypeChecker

        def check_one() -> Any:
            chk = TypeChecker.__new__(TypeChecker)
            chk._type_maps = [{node: AnyType(TypeOfAny.unannotated)}]
            return TypeChecker.find_isinstance_check_helper(chk, node, in_boolean_context=False)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(str(on), str(off), f"find_isinstance head parity for {node.callee}")
        return on

    # Direct seam calls: one per arm tag.

    def test_seam_isinstance_arms(self) -> None:
        from mypy.checker import (
            NATIVE_ISINSTANCE_HEAD_ISINSTANCE_BAD_ARGS,
            NATIVE_ISINSTANCE_HEAD_ISINSTANCE_NARROW,
            NATIVE_ISINSTANCE_HEAD_ISINSTANCE_TAIL,
        )

        callee = self._builtin_ref("builtins.isinstance")
        assert self._seam(callee, 2, True) == NATIVE_ISINSTANCE_HEAD_ISINSTANCE_NARROW
        assert self._seam(callee, 1, True) == NATIVE_ISINSTANCE_HEAD_ISINSTANCE_BAD_ARGS
        assert self._seam(callee, 2, False) == NATIVE_ISINSTANCE_HEAD_ISINSTANCE_TAIL

    def test_seam_issubclass_arms(self) -> None:
        from mypy.checker import (
            NATIVE_ISINSTANCE_HEAD_ISSUBCLASS_BAD_ARGS,
            NATIVE_ISINSTANCE_HEAD_ISSUBCLASS_NARROW,
            NATIVE_ISINSTANCE_HEAD_ISSUBCLASS_TAIL,
        )

        callee = self._builtin_ref("builtins.issubclass")
        assert self._seam(callee, 2, True) == NATIVE_ISINSTANCE_HEAD_ISSUBCLASS_NARROW
        assert self._seam(callee, 1, True) == NATIVE_ISINSTANCE_HEAD_ISSUBCLASS_BAD_ARGS
        assert self._seam(callee, 2, False) == NATIVE_ISINSTANCE_HEAD_ISSUBCLASS_TAIL

    def test_seam_callable_arms(self) -> None:
        from mypy.checker import (
            NATIVE_ISINSTANCE_HEAD_CALLABLE_BAD_ARGS,
            NATIVE_ISINSTANCE_HEAD_CALLABLE_NARROW,
            NATIVE_ISINSTANCE_HEAD_CALLABLE_TAIL,
        )

        callee = self._builtin_ref("builtins.callable")
        assert self._seam(callee, 1, True) == NATIVE_ISINSTANCE_HEAD_CALLABLE_NARROW
        assert self._seam(callee, 2, True) == NATIVE_ISINSTANCE_HEAD_CALLABLE_BAD_ARGS
        assert self._seam(callee, 1, False) == NATIVE_ISINSTANCE_HEAD_CALLABLE_TAIL

    def test_seam_hasattr_arms(self) -> None:
        from mypy.checker import (
            NATIVE_ISINSTANCE_HEAD_HASATTR,
            NATIVE_ISINSTANCE_HEAD_HASATTR_BAD_ARGS,
        )

        callee = self._builtin_ref("builtins.hasattr")
        assert self._seam(callee, 2, True) == NATIVE_ISINSTANCE_HEAD_HASATTR
        assert self._seam(callee, 1, True) == NATIVE_ISINSTANCE_HEAD_HASATTR_BAD_ARGS

    def test_seam_typeguard_for_other_callees(self) -> None:
        from mypy.checker import NATIVE_ISINSTANCE_HEAD_TYPEGUARD

        assert self._seam(self._builtin_ref("mod.guard_fn"), 1, True) == (
            NATIVE_ISINSTANCE_HEAD_TYPEGUARD
        )
        member = MemberExpr(self._var_expr(), "guard_fn")
        assert self._seam(member, 1, True) == NATIVE_ISINSTANCE_HEAD_TYPEGUARD

    def test_seam_defers_on_alias_callee(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.str_type, "mod.A", "mod", -1, -1)
        ref = self._builtin_ref("mod.A")
        ref.node = alias
        assert self._seam(ref, 2, True) is None

    # Gate-off vs gate-on differential through the real method.

    def test_parity_bad_args_arms(self) -> None:
        # The arity error is reported elsewhere; all four builtins
        # short-circuit to ({}, {}).
        for fullname, args in [
            ("builtins.isinstance", 1),
            ("builtins.issubclass", 1),
            ("builtins.callable", 2),
            ("builtins.hasattr", 1),
        ]:
            node = self._call(self._builtin_ref(fullname), [self._var_expr()] * args)
            assert_equal(self._run(node), ({}, {}), f"bad args for {fullname}")

    def test_parity_tail_non_literal_arg(self) -> None:
        # A non-literal first argument skips narrowing and lands in the
        # boolean-context tail.
        inner = CallExpr(self._builtin_ref("mod.f"), [], [], [])
        for fullname, args in [("builtins.isinstance", 2), ("builtins.callable", 1)]:
            node = self._call(self._builtin_ref(fullname), [inner] * args)
            result = self._run(node)
            if_map, else_map = result
            assert set(if_map) == {node} and set(else_map) == {node}

    def test_parity_typeguard_tail_without_guard(self) -> None:
        # A non-builtin callee without TypeGuard/TypeIs falls to the tail.
        node = self._call(self._builtin_ref("mod.f"), [self._var_expr()])
        result = self._run(node)
        assert set(result[0]) == {node} and set(result[1]) == {node}

    def test_parity_typeguard_guarded_type_map(self) -> None:
        # A RefExpr callee with type_guard set narrows to TypeGuardedType.
        from mypy.types import TypeGuardedType

        callee = self._builtin_ref("mod.guard_fn")
        callee.type_guard = self.fx.str_type
        expr = self._var_expr()
        node = self._call(callee, [expr])
        expected = str(({expr: TypeGuardedType(self.fx.str_type)}, {}))
        assert_equal(str(self._run(node)), expected)

    def test_parity_alias_callee_defers_to_python(self) -> None:
        # The alias deferral re-runs the pure-Python head, which treats the
        # callee like any other non-builtin.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.str_type, "mod.A", "mod", -1, -1)
        callee = self._builtin_ref("mod.A")
        callee.node = alias
        node = self._call(callee, [self._var_expr()])
        result = self._run(node)
        assert set(result[0]) == {node} and set(result[1]) == {node}

    def test_true_false_literal_precedes_the_gate(self) -> None:
        # True/False literals short-circuit at checker.py:8637-8640, before
        # the seam gate at :8648 is read, so the arm comparison cannot fail
        # on this shape; the literal maps of one run are pinned instead.
        from mypy.checker import TypeChecker
        from mypy.types import UninhabitedType

        def run(node: Any) -> Any:
            chk = TypeChecker.__new__(TypeChecker)
            chk._type_maps = [{}]
            return TypeChecker.find_isinstance_check_helper(chk, node, in_boolean_context=False)

        true_ref = self._builtin_ref("builtins.True")
        assert_equal(run(true_ref), ({}, {true_ref: UninhabitedType()}))
        false_ref = self._builtin_ref("builtins.False")
        assert_equal(run(false_ref), ({false_ref: UninhabitedType()}, {}))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeEnumCheckSuite(Suite):
    """Parity for the Rust `check_enum` multi-arm classifier port.

    `TypeChecker.check_enum` (checker.py:3843) has three arms:
    (a) `__members__` override fail, (c) final-enum base loop,
    (b) stub-empty-enum fail+note. The Rust classifier
    (`checker_functions.rs`) returns `(tag, base_names)` where
    tag is a bit flag and base_names are the arm-(c) offending
    base fullnames. Direct seam calls assert exact tags and
    base-name lists; gate-off vs gate-on parity compares recorded
    fail/note message lists.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _info(
        self, name: str, *, is_enum: bool = True, mro: list[TypeInfo] | None = None
    ) -> TypeInfo:
        info = self.fx.make_type_info(name, mro=mro)
        info.is_enum = is_enum
        return info

    def _seam(
        self, info: TypeInfo, *, is_stub: bool = False, tree_fullname: str = "mod.sub"
    ) -> tuple[int, list[str]] | None:
        from mypy.semanal_enum import ENUM_BASES

        return _type_kernel.rust_classify_enum(info, is_stub, tree_fullname, list(ENUM_BASES))

    def _var_sym(self, has_explicit_value: bool) -> SymbolTableNode:
        var = Var("__members__")
        var.has_explicit_value = has_explicit_value
        return SymbolTableNode(MDEF, var)

    def _sub(self, info: TypeInfo) -> Any:
        info.defn.info = info
        return info.defn

    def _run(
        self, info: TypeInfo, *, is_stub: bool = False, tree_fullname: str = "mod.sub"
    ) -> tuple[Any, Any]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> list[tuple[str, object]]:
            chk = TypeChecker.__new__(TypeChecker)
            msgs: list[tuple[str, object]] = []
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                fail=lambda msg, ctx, code=None: msgs.append((str(msg), code))
            )
            chk.note = lambda msg, ctx, code=None: msgs.append(("NOTE: " + str(msg), code))  # type: ignore[method-assign, misc]
            chk.options = Options()
            chk.is_stub = is_stub
            chk.tree = SimpleNamespace(fullname=tree_fullname)  # type: ignore[assignment]
            defn = self._sub(info)
            chk.check_enum(defn)
            return msgs

        return self._with_gate(False, check_one), self._with_gate(True, check_one)

    # --- Direct seam tests ---

    def test_seam_no_arms(self) -> None:
        info = self._info("mod.Sub")
        result = self._seam(info)
        assert result is not None
        tag, base_names = result
        assert tag == 0
        assert base_names == []

    def test_seam_members_override(self) -> None:
        info = self._info("mod.Sub")
        info.names["__members__"] = self._var_sym(True)
        result = self._seam(info)
        assert result is not None
        tag, _ = result
        assert tag & 1 == 1

    def test_seam_members_not_var(self) -> None:
        info = self._info("mod.Sub")
        info.names["__members__"] = SymbolTableNode(MDEF, FuncDef("__members__"))
        result = self._seam(info)
        assert result is not None
        tag, _ = result
        assert tag == 0

    def test_seam_members_no_explicit_value(self) -> None:
        info = self._info("mod.Sub")
        info.names["__members__"] = self._var_sym(False)
        result = self._seam(info)
        assert result is not None
        tag, _ = result
        assert tag == 0

    def test_seam_members_in_enum_base(self) -> None:
        info = self._info("enum.Enum")
        info.names["__members__"] = self._var_sym(True)
        result = self._seam(info)
        assert result is not None
        tag, _ = result
        assert tag == 0

    def test_seam_final_enum_base(self) -> None:
        enum_base = self._info("mod.Parent", is_enum=True)
        info = self._info("mod.Sub", mro=[enum_base, self.fx.oi])
        result = self._seam(info)
        assert result is not None
        tag, base_names = result
        assert tag == 0
        assert base_names == ["mod.Parent"]

    def test_seam_enum_base_in_enum_bases(self) -> None:
        enum_base = self._info("enum.Enum", is_enum=True)
        info = self._info("mod.Sub", mro=[enum_base, self.fx.oi])
        result = self._seam(info)
        assert result is not None
        _, base_names = result
        assert base_names == []

    def test_seam_stub_empty(self) -> None:
        info = self._info("mod.Sub")
        result = self._seam(info, is_stub=True, tree_fullname="mod.sub")
        assert result is not None
        tag, _ = result
        assert tag & 2 == 2

    def test_seam_stub_not_empty(self) -> None:
        info = self._info("mod.Sub")
        # Add an enum member: a Var with has_explicit_value
        var = Var("MEMBER")
        var.has_explicit_value = True
        info.names["MEMBER"] = SymbolTableNode(MDEF, var)
        result = self._seam(info, is_stub=True)
        assert result is not None
        tag, _ = result
        assert tag == 0

    def test_seam_stub_in_enum_module(self) -> None:
        info = self._info("mod.Sub")
        result = self._seam(info, is_stub=True, tree_fullname="enum")
        assert result is not None
        tag, _ = result
        assert tag == 0

    def test_seam_stub_in_typeshed(self) -> None:
        info = self._info("mod.Sub")
        result = self._seam(info, is_stub=True, tree_fullname="_typeshed")
        assert result is not None
        tag, _ = result
        assert tag == 0

    def test_seam_all_three_arms(self) -> None:
        enum_base = self._info("mod.Parent", is_enum=True)
        info = self._info("mod.Sub", mro=[enum_base, self.fx.oi])
        info.names["__members__"] = self._var_sym(True)
        result = self._seam(info, is_stub=True)
        assert result is not None
        tag, base_names = result
        assert tag & 1 == 1
        assert tag & 2 == 2
        assert base_names == ["mod.Parent"]

    # --- Gate-off vs gate-on parity tests ---

    def test_parity_no_arms(self) -> None:
        info = self._info("mod.Sub")
        off, on = self._run(info)
        assert_equal(on, off, "no arms")

    def test_parity_members_override(self) -> None:
        info = self._info("mod.Sub")
        info.names["__members__"] = self._var_sym(True)
        off, on = self._run(info)
        assert_equal(on, off, "members override")

    def test_parity_final_enum_base(self) -> None:
        enum_base = self._info("mod.Parent", is_enum=True)
        # Give it enum_members so check_final_enum fires
        var = Var("MEMBER")
        var.has_explicit_value = True
        enum_base.names["MEMBER"] = SymbolTableNode(MDEF, var)
        info = self._info("mod.Sub", mro=[enum_base, self.fx.oi])
        off, on = self._run(info)
        assert_equal(on, off, "final enum base")

    def test_parity_stub_empty(self) -> None:
        info = self._info("mod.Sub")
        off, on = self._run(info, is_stub=True)
        assert_equal(on, off, "stub empty enum")

    def test_parity_all_three(self) -> None:
        enum_base = self._info("mod.Parent", is_enum=True)
        var = Var("MEMBER")
        var.has_explicit_value = True
        enum_base.names["MEMBER"] = SymbolTableNode(MDEF, var)
        info = self._info("mod.Sub", mro=[enum_base, self.fx.oi])
        info.names["__members__"] = self._var_sym(True)
        off, on = self._run(info, is_stub=True)
        assert_equal(on, off, "all three arms")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassPatternRangesSuite(Suite):
    """Parity for the Rust class-pattern type-range dispatch (issue #987).

    `PatternChecker.get_class_pattern_type_ranges` (checkpattern.py:794-832)
    dispatches on the proper type: union recursion, FunctionLike type object,
    the `typing.Callable` class-ref Var arm, TypeType, AnyType, and a fail
    tail. The Rust classifier (`checkpattern.rs`) walks the union on the
    wire and reads the class-ref facts via PyO3, returning one tag per leaf;
    the Python shim builds the TypeRanges from live nodes and reports the
    fail. Direct seam calls assert the tag lists; the gate-off vs gate-on
    differential drives the real PatternChecker method.
    """

    def setUp(self) -> None:
        from mypy.checkpattern import _set_native_checkpattern_active

        self._set_active = _set_native_checkpattern_active
        self.fx = TypeFixture()
        self.options = Options()

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _type_obj(self) -> FunctionLike:
        # A type-object callable: fallback builtins.type, ret Instance(A).
        return CallableType(
            [AnyType(TypeOfAny.unannotated)], [ARG_STAR], [None], self.fx.a, self.fx.type_type
        )

    def _callable_var(self) -> Var:
        v = Var("Callable")
        v._fullname = "typing.Callable"
        v.type = self.fx.function
        return v

    def _plain_var(self) -> Var:
        v = Var("x")
        v._fullname = "mod.x"
        v.type = self.fx.function
        return v

    def _pattern(self, node: Any) -> ClassPattern:
        from mypy.patterns import ClassPattern

        ref = NameExpr("Callable")
        ref.node = node
        return ClassPattern(ref, [], [], [])

    def _pc(self, records: list[str]) -> Any:
        from mypy.checkpattern import PatternChecker

        pc = PatternChecker.__new__(PatternChecker)
        pc.chk = SimpleNamespace(named_type=lambda name: self.fx.function)  # type: ignore[assignment]
        pc.msg = SimpleNamespace(fail=lambda msg, ctx: records.append(msg))  # type: ignore[assignment]
        pc.options = self.options
        return pc

    def _run(self, typ: Type, node: Any) -> tuple[object, object, list[str], list[str]]:
        records_off: list[str] = []
        records_on: list[str] = []

        def one(records: list[str]) -> object:
            return self._pc(records).get_class_pattern_type_ranges(typ, self._pattern(node))

        off = self._with_gate(False, lambda: one(records_off))
        on = self._with_gate(True, lambda: one(records_on))
        return off, on, records_off, records_on

    def _assert_par(self, typ: Type, node: Any = None) -> Any:
        off, on, off_rec, on_rec = self._run(typ, node)
        assert_equal(self._ranges(off), self._ranges(on), f"class pattern parity for {typ!r}")
        assert_equal(on_rec, off_rec, f"class pattern fail parity for {typ!r}")
        return on

    def _ranges(self, ranges: Any) -> list[tuple[str, bool]]:
        if ranges is None:
            return []
        return [(str(r.item), r.is_upper_bound) for r in ranges]

    # ----- direct seam calls -----

    def _seam(self, typ: Type, node: Any) -> Any:
        return _type_kernel.rust_classify_class_pattern_ranges(self._bytes_of(typ), node)

    def test_seam_union_preorder(self) -> None:
        tags = self._seam(
            UnionType.make_union([self._type_obj(), AnyType(TypeOfAny.unannotated)]), None
        )
        assert tags == [1, 4]

    def test_seam_type_type(self) -> None:
        assert self._seam(TypeType.make_normalized(self.fx.a), None) == [3]

    def test_seam_any(self) -> None:
        assert self._seam(AnyType(TypeOfAny.unannotated), None) == [4]

    def test_seam_callable_var_arm(self) -> None:
        assert self._seam(self.fx.a, self._callable_var()) == [2]

    def test_seam_fail_arm(self) -> None:
        assert self._seam(self.fx.a, None) == [0]

    def test_seam_alias_defers(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        assert self._seam(typ, None) is None

    def test_seam_non_metaclass_fallback_defers(self) -> None:
        # fallback.type.is_metaclass() needs the live TypeInfo: defer.
        call = CallableType(
            [AnyType(TypeOfAny.unannotated)], [ARG_STAR], [None], self.fx.a, self.fx.function
        )
        assert self._seam(call, None) is None

    def test_seam_uninhabited_ret_not_type_obj(self) -> None:
        call = CallableType(
            [AnyType(TypeOfAny.unannotated)],
            [ARG_STAR],
            [None],
            UninhabitedType(),
            self.fx.type_type,
        )
        assert self._seam(call, self._callable_var()) == [2]

    def test_seam_union_with_failing_leaf(self) -> None:
        tags = self._seam(UnionType.make_union([self.fx.a, AnyType(TypeOfAny.unannotated)]), None)
        assert tags == [0, 4]

    # ----- gate-off vs gate-on differentials -----

    def test_parity_type_obj(self) -> None:
        on = self._assert_par(self._type_obj())
        assert [(str(r.item), r.is_upper_bound) for r in on] == [("A", False)]

    def test_parity_any(self) -> None:
        on = self._assert_par(AnyType(TypeOfAny.unannotated))
        assert self._ranges(on) == [("Any", False)]

    def test_parity_type_type(self) -> None:
        on = self._assert_par(TypeType.make_normalized(self.fx.a))
        assert self._ranges(on) == [("A", True)]

    def test_parity_union(self) -> None:
        on = self._assert_par(
            UnionType.make_union([self._type_obj(), AnyType(TypeOfAny.unannotated)])
        )
        assert self._ranges(on) == [("A", False), ("Any", False)]

    def test_parity_callable_var_arm(self) -> None:
        on = self._assert_par(self.fx.a, self._callable_var())
        (rng,) = on
        item: ProperType = get_proper_type(rng.item)
        assert isinstance(item, CallableType)
        assert item.fallback.type.fullname == "builtins.function"
        assert rng.is_upper_bound is False

    def test_parity_fail_arm(self) -> None:
        on = self._assert_par(self.fx.a)
        assert on is None
        # The fail message names the offending type (checked via records).

    def test_parity_union_with_failing_leaf(self) -> None:
        on = self._assert_par(UnionType.make_union([self.fx.a, AnyType(TypeOfAny.unannotated)]))
        assert self._ranges(on) == [("Any", False)]

    def test_parity_alias_defers(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        # The Rust seam defers on the alias; both gates run the Python body,
        # which expands to Instance and reports CLASS_PATTERN_TYPE_REQUIRED.
        on = self._assert_par(typ)
        assert on is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRvalueCountSuite(Suite):
    """Parity for the Rust `check_rvalue_count_in_assignment` dispatch port.

    `TypeChecker.check_rvalue_count_in_assignment` (checker.py:5319)
    classifies the unpacking-arity decision: the variadic-unpack arm
    (star target required, too-many-targets, prefix/suffix asymmetry) and
    the plain star-count / exact-count arms. The Rust classifier
    (`checker_functions.rs`) returns a branch tag from the live lvalues
    list plus scalar counts; the Python shim applies the fail /
    wrong-number messages and keeps the pure-Python body as the fallback.

    Direct seam calls assert the exact tag for every branch; the gate-off
    vs gate-on differential drives the real TypeChecker method and asserts
    identical (return, messages) pairs.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _seam(self, lvalues: object, rvalue_count: int, rvalue_unpack: int | None) -> int | None:
        return _type_kernel.rust_classify_rvalue_count(lvalues, rvalue_count, rvalue_unpack)

    def _lv(self, n: int, star_at: int | None = None) -> list[NameExpr | StarExpr]:
        out: list[NameExpr | StarExpr] = []
        for i in range(n):
            if i == star_at:
                out.append(StarExpr(NameExpr("xs")))
            else:
                out.append(NameExpr(f"v{i}"))
        return out

    def _run(
        self, lvalues: list[NameExpr | StarExpr], rvalue_count: int, rvalue_unpack: int | None
    ) -> tuple[Any, Any]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> tuple[Any, Any]:
            chk = TypeChecker.__new__(TypeChecker)
            msgs: list[tuple[str, str]] = []
            chk.fail = lambda msg, ctx, code=None: msgs.append(  # type: ignore[method-assign, assignment, misc]
                ("fail", str(msg))
            )
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                wrong_number_values_to_unpack=lambda received, expected, ctx: msgs.append(
                    ("wrong_number", f"{received}:{expected}")
                )
            )
            result = chk.check_rvalue_count_in_assignment(
                cast("list[Expression]", lvalues),
                rvalue_count,
                Context(),
                rvalue_unpack=rvalue_unpack,
            )
            return result, msgs

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_parity(
        self,
        lvalues: list[NameExpr | StarExpr],
        rvalue_count: int,
        rvalue_unpack: int | None,
        label: str,
    ) -> None:
        off, on = self._run(lvalues, rvalue_count, rvalue_unpack)
        assert_equal(on, off, f"rvalue count parity: {label}")

    # --- direct seam tests ---

    def test_seam_non_list_defers(self) -> None:
        assert self._seam("nope", 1, None) is None

    def test_seam_variadic_no_star(self) -> None:
        assert self._seam(self._lv(2), 4, 2) == 1

    def test_seam_variadic_too_many(self) -> None:
        assert self._seam(self._lv(5, star_at=1), 4, 2) == 2

    def test_seam_variadic_asymmetric_suffix(self) -> None:
        # star first: left_suffix 3 > right_suffix 1.
        assert self._seam(self._lv(4, star_at=0), 5, 3) == 3

    def test_seam_variadic_asymmetric_prefix(self) -> None:
        # left_prefix 2 > right_prefix 1.
        assert self._seam(self._lv(4, star_at=2), 5, 1) == 3

    def test_seam_variadic_pass(self) -> None:
        assert self._seam(self._lv(4, star_at=1), 4, 1) == 0
        # Right side longer than left is fine.
        assert self._seam(self._lv(2, star_at=0), 4, 2) == 0

    def test_seam_star_count(self) -> None:
        # len - 1 = 3 > 2.
        assert self._seam(self._lv(4, star_at=0), 2, None) == 4
        assert self._seam(self._lv(3, star_at=0), 2, None) == 0

    def test_seam_exact_count(self) -> None:
        assert self._seam(self._lv(3), 2, None) == 5
        assert self._seam(self._lv(3), 3, None) == 0

    # --- gate-off vs gate-on differential tests ---

    def test_parity_variadic_no_star(self) -> None:
        self._assert_parity(self._lv(2), 2, 2, "variadic without star target")

    def test_parity_variadic_too_many(self) -> None:
        self._assert_parity(self._lv(5, star_at=1), 4, 2, "variadic too many targets")

    def test_parity_variadic_asymmetric(self) -> None:
        self._assert_parity(self._lv(4, star_at=0), 5, 3, "asymmetric suffix")
        self._assert_parity(self._lv(4, star_at=2), 5, 1, "asymmetric prefix")

    def test_parity_variadic_symmetric(self) -> None:
        self._assert_parity(self._lv(4, star_at=1), 4, 1, "symmetric variadic")
        self._assert_parity(self._lv(2, star_at=0), 4, 2, "short prefix/suffix")

    def test_parity_star_count(self) -> None:
        self._assert_parity(self._lv(4, star_at=0), 2, None, "star too few rvalues")
        self._assert_parity(self._lv(3, star_at=0), 2, None, "star exact count")

    def test_parity_exact_count(self) -> None:
        self._assert_parity(self._lv(3), 2, None, "plain too few rvalues")
        self._assert_parity(self._lv(3), 3, None, "plain exact count")

    def test_parity_message_contents(self) -> None:
        # The fail / wrong-number side effects carry the same messages.
        off, on = self._run(self._lv(2), 2, 2)
        assert_equal(on, off, "variadic no star message")
        assert on[1] == [("fail", "Variadic tuple unpacking requires a star target")]

        off, on = self._run(self._lv(3), 2, None)
        assert_equal(on, off, "exact count message")
        assert on[1] == [("wrong_number", "2:3")]


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeGetattrMethodSuite(Suite):
    """Parity for the Rust `check_getattr_method` 4-way dispatch head.

    `rust_classify_getattr_method` classifies module/getattribute/class/pass
    from the live `Scope` facts (`len(scope.stack) == 1`,
    `scope.active_class()`); the Python shim builds the fixed CallableType
    via `named_type`, runs `is_subtype`, and emits the messages. Direct seam
    calls assert the tag for every branch; the gate-off vs gate-on
    differential drives the real `check_getattr_method` through a minimal
    fake checker capturing fail/note records.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _assert_par(self, module: bool, name: str, ok: bool, cls: bool = False) -> None:
        """Run the real check through gate-off and gate-on checkers; compare."""
        typ = self._typ(name, ok, cls)
        off_scope = _FakeScope(module=module, cls=cls)
        off = self._with_gate(False, lambda: _run_check(off_scope, typ, name))
        on_scope = _FakeScope(module=module, cls=cls)
        self._set_active(True)
        on = _run_check(on_scope, typ, name)
        assert_equal(off, on, f"getattr-method parity module={module} cls={cls} {name}")
        has_fail = bool(off[0]) or bool(off[1])
        assert has_fail == expect_fail_check(module, name, ok, cls), (off, on)

    def _typ(self, name: str, ok: bool, cls: bool = False) -> object:
        return _make_getattr_sig(ok, cls)

    # -- direct seam calls -------------------------------------------------

    def test_seam_module_scope_getattribute(self) -> None:
        from mypy.checker import (  # type: ignore[attr-defined]
            NATIVE_GETATTR_METHOD_MODULE_GETATTRIBUTE,
            _rust_classify_getattr_method,
        )

        assert _rust_classify_getattr_method(_FakeScope(module=True), "__getattribute__") == (
            NATIVE_GETATTR_METHOD_MODULE_GETATTRIBUTE
        )

    def test_seam_module_scope_getattr(self) -> None:
        from mypy.checker import (  # type: ignore[attr-defined]
            NATIVE_GETATTR_METHOD_MODULE,
            _rust_classify_getattr_method,
        )

        assert _rust_classify_getattr_method(_FakeScope(module=True), "__getattr__") == (
            NATIVE_GETATTR_METHOD_MODULE
        )

    def test_seam_class_scope(self) -> None:
        from mypy.checker import (  # type: ignore[attr-defined]
            NATIVE_GETATTR_METHOD_CLASS,
            _rust_classify_getattr_method,
        )

        assert _rust_classify_getattr_method(
            _FakeScope(module=False, cls=True), "__getattr__"
        ) == (NATIVE_GETATTR_METHOD_CLASS)

    def test_seam_other_scope_pass(self) -> None:
        from mypy.checker import (  # type: ignore[attr-defined]
            NATIVE_GETATTR_METHOD_PASS,
            _rust_classify_getattr_method,
        )

        assert _rust_classify_getattr_method(
            _FakeScope(module=False, cls=False), "__getattr__"
        ) == (NATIVE_GETATTR_METHOD_PASS)

    # -- gate-off vs gate-on differentials ---------------------------------

    def test_parity_module_getattribute(self) -> None:
        self._assert_par(module=True, name="__getattribute__", ok=False)

    def test_parity_module_getattr_valid(self) -> None:
        self._assert_par(module=True, name="__getattr__", ok=True)

    def test_parity_module_getattr_invalid(self) -> None:
        self._assert_par(module=True, name="__getattr__", ok=False)

    def test_parity_class_scope_valid(self) -> None:
        self._assert_par(module=False, cls=True, name="__getattr__", ok=True)

    def test_parity_class_scope_invalid(self) -> None:
        self._assert_par(module=False, cls=True, name="__getattr__", ok=False)

    def test_parity_other_scope_noop(self) -> None:
        self._assert_par(module=False, cls=False, name="__getattr__", ok=True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTruthyTypeSuite(Suite):
    """Gate-off vs gate-on parity for check_for_truthy_type (issue #1010).

    Runs TypeChecker.check_for_truthy_type on a stub checker with a fail
    recorder and asserts identical message lists with the native
    classifier off and on, plus direct seam calls on the live types.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:

        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], list[str]]) -> list[str]:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _run(self, t: Type, expr: Any = None) -> tuple[list[str], list[str]]:
        from mypy.checker import TypeChecker
        from mypy.nodes import NameExpr
        from mypy.options import Options
        from mypy.state import state

        if expr is None:
            expr = NameExpr("x")

        def check_one() -> list[str]:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            fails: list[str] = []

            class _Msg:
                def fail(self, msg: str, _ctx: Any, **_kw: Any) -> None:
                    fails.append(str(msg))

            chk.msg = _Msg()  # type: ignore[assignment]
            chk.check_for_truthy_type(t, expr)
            return fails

        old = state.strict_optional
        state.strict_optional = True
        try:
            off = self._with_gate(False, check_one)
            on = self._with_gate(True, check_one)
        finally:
            state.strict_optional = old
        return off, on

    def _assert_par(self, t: Type, expr: Any = None) -> None:
        off, on = self._run(t, expr)
        assert_equal(on, off, f"check_for_truthy_type parity for typ={t!r}")

    def _tag(self, t: Type) -> int:
        proper = get_proper_type(t)
        result = _type_kernel.rust_classify_truthy_type(proper)
        assert result is not None
        return result

    def _bool_member_instance(self) -> Instance:
        from mypy.nodes import MDEF, SymbolTableNode, Var

        info = self.fx.make_type_info("WithBool")
        assert info is not None
        info.names["__bool__"] = SymbolTableNode(MDEF, Var("__bool__"))
        return Instance(info, [])

    def _len_member_instance(self) -> Instance:
        from mypy.nodes import MDEF, SymbolTableNode, Var

        info = self.fx.make_type_info("WithLen")
        assert info is not None
        info.names["__len__"] = SymbolTableNode(MDEF, Var("__len__"))
        return Instance(info, [])

    def _iterable_instance(self) -> Instance:
        info = self.fx.make_type_info("typing.Iterable")
        assert info is not None
        return Instance(info, [self.fx.o])

    # Direct seam calls: the five branch tags.

    def test_seam_other_instance(self) -> None:
        # fx.a has a fixture-added __bool__; class D does not.
        assert self._tag(self.fx.d) == 4

    def test_seam_skip_object(self) -> None:
        assert self._tag(self.fx.o) == 0

    def test_seam_skip_none(self) -> None:
        assert self._tag(self.fx.nonet) == 0

    def test_seam_skip_any(self) -> None:
        assert self._tag(self.fx.anyt) == 0

    def test_seam_lkv_instance_is_other(self) -> None:
        # The fixture's builtins.str TypeInfo has no __bool__ member, so
        # an Instance with last_known_value is still a truthy Instance.
        assert self._tag(self.fx.lit_str1_inst) == 4

    def test_seam_skip_bool_member(self) -> None:
        assert self._tag(self._bool_member_instance()) == 0

    def test_seam_skip_len_member(self) -> None:
        assert self._tag(self._len_member_instance()) == 0

    def test_seam_function(self) -> None:
        assert self._tag(self.fx.callable(self.fx.o, self.fx.nonet)) == 1

    def test_seam_union_of_callables(self) -> None:
        c = self.fx.callable(self.fx.o, self.fx.nonet)
        assert self._tag(UnionType([c, c])) == 2

    def test_seam_iterable(self) -> None:
        assert self._tag(self._iterable_instance()) == 3

    # Gate-off vs gate-on differentials on the captured fail messages.

    def test_parity_other_instance(self) -> None:
        # fx.a has a fixture-added __bool__; class D does not.
        self._assert_par(self.fx.d)

    def test_parity_skip_object(self) -> None:
        self._assert_par(self.fx.o)

    def test_parity_skip_none(self) -> None:
        self._assert_par(self.fx.nonet)

    def test_parity_skip_any(self) -> None:
        self._assert_par(self.fx.anyt)

    def test_parity_lkv_instance(self) -> None:
        self._assert_par(self.fx.lit_str1_inst)

    def test_parity_skip_bool_member(self) -> None:
        self._assert_par(self._bool_member_instance())

    def test_parity_skip_len_member(self) -> None:
        self._assert_par(self._len_member_instance())

    def test_parity_function(self) -> None:
        self._assert_par(self.fx.callable(self.fx.o, self.fx.nonet))

    def test_parity_union_of_callables(self) -> None:
        c = self.fx.callable(self.fx.o, self.fx.nonet)
        self._assert_par(UnionType([c, c]))

    def test_parity_union_with_non_truthy_item(self) -> None:
        c = self.fx.callable(self.fx.o, self.fx.nonet)
        self._assert_par(UnionType([c, self.fx.o]))

    def test_parity_iterable(self) -> None:
        self._assert_par(self._iterable_instance())

    def test_parity_member_expr_message(self) -> None:
        from mypy.nodes import MemberExpr, NameExpr

        member = MemberExpr(NameExpr("x"), "f")
        self._assert_par(self.fx.callable(self.fx.o, self.fx.nonet), member)

    def test_parity_call_expr_message(self) -> None:
        from mypy.nodes import ARG_POS, CallExpr, NameExpr

        call = CallExpr(NameExpr("f"), [], [ARG_POS], [None])
        self._assert_par(self.fx.callable(self.fx.o, self.fx.nonet), call)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckFinalSuite(Suite):
    """Parity for the Rust `check_final` decision-head port.

    `TypeChecker.check_final` (checker.py:5095) is, after the shim's
    flatten_lvalues / is_final_decl computation, a pure sequence of message
    decisions: the `final_without_value` gate (scalar Var / statement /
    class facts) and the per-lvalue final-assignment arbitration (RefExpr
    -> Var gate, the MRO walk over `cls.mro[1:]` looking up a final base
    Var, and the own `lv.node.is_final` check). The Rust port
    (`checker_functions.rs`) walks the live lvalues and MRO via PyO3 and
    returns `(without_value, [(name, info_is_none), ...])`; the Python shim
    applies the final_without_value / cant_assign_to_final emissions and
    keeps the pure-Python body as the fallback.

    Direct seam calls assert the exact (without_value, msgs) pairs; the
    gate-off vs gate-on differential drives the real TypeChecker method
    through a stub message recorder and asserts identical message lists.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _var(self, name: str, is_final: bool = False) -> Any:
        from mypy.nodes import Var

        v = Var(name)
        v.is_final = is_final
        return v

    def _ref(self, node: Any) -> Any:
        from mypy.nodes import NameExpr

        lv = NameExpr(node.name)
        lv.node = node
        return lv

    def _cls(self, names: dict[str, Any] | None = None, *, is_named_tuple: bool = False) -> Any:
        from types import SimpleNamespace

        return SimpleNamespace(
            fullname="mod.Base",
            mro=[SimpleNamespace(names={}), SimpleNamespace(names=names or {})],
            is_named_tuple=is_named_tuple,
        )

    def _seam(
        self,
        lvs: list[Any],
        is_final_decl: bool,
        cls: Any,
        is_stub: bool = False,
        s_type_is_none: bool = False,
        is_assignment_stmt: bool = True,
    ) -> Any:
        return _type_kernel.rust_classify_check_final(
            lvs, is_final_decl, cls, is_stub, s_type_is_none, is_assignment_stmt
        )

    def _run(
        self,
        lvs: list[Any],
        is_final_decl: bool,
        cls: Any,
        *,
        is_stub: bool = False,
        s_type: Any = None,
    ) -> tuple[list[str], list[str]]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker
        from mypy.nodes import TempNode

        def check_one() -> list[str]:
            chk = TypeChecker.__new__(TypeChecker)
            msgs: list[str] = []
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                final_without_value=lambda ctx: msgs.append("final_without_value"),
                cant_assign_to_final=lambda n, info_is_none, ctx: msgs.append(
                    f"cant_assign:{n}:{info_is_none}"
                ),
            )
            chk.is_stub = is_stub
            chk.scope = SimpleNamespace(active_class=lambda: cls)  # type: ignore[assignment]
            s = AssignmentStmt(lvs, TempNode(AnyType(TypeOfAny.explicit)), type=s_type)
            s.is_final_def = is_final_decl
            chk.check_final(s)
            return msgs

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(
        self,
        lvs: list[Any],
        is_final_decl: bool,
        cls: Any,
        *,
        is_stub: bool = False,
        s_type: Any = None,
    ) -> None:
        off, on = self._run(lvs, is_final_decl, cls, is_stub=is_stub, s_type=s_type)
        assert_equal(on, off, f"check_final parity for lvs={lvs!r} cls={cls!r}")

    def test_seam_non_refexpr_lvalue(self) -> None:
        from mypy.nodes import TempNode

        res = self._seam([TempNode(AnyType(TypeOfAny.explicit))], False, None)
        assert res == (False, [])

    def test_seam_node_none(self) -> None:
        from mypy.nodes import NameExpr

        lv = NameExpr("x")
        lv.node = None
        res = self._seam([lv], False, None)
        assert res == (False, [])

    def test_seam_non_var_node(self) -> None:
        from mypy.nodes import FuncDef

        res = self._seam([self._ref(FuncDef("f"))], False, None)
        assert res == (False, [])

    def test_seam_plain_assignment(self) -> None:
        # The fast no-final path: one lookup, no messages.
        res = self._seam([self._ref(self._var("x"))], False, None)
        assert res == (False, [])

    def test_seam_own_final(self) -> None:
        res = self._seam([self._ref(self._var("x", True))], False, None)
        assert res == (False, [("x", False)])

    def test_seam_own_final_decl_no_msg(self) -> None:
        res = self._seam([self._ref(self._var("x", True))], True, None)
        assert res == (False, [])

    def test_seam_base_final(self) -> None:
        from mypy.nodes import MDEF, SymbolTableNode

        sym = SymbolTableNode(MDEF, self._var("x", True))
        cls = self._cls({"x": sym})
        res = self._seam([self._ref(self._var("x"))], False, cls)
        assert res == (False, [("x", False)])

    def test_seam_base_final_decl_suppressed(self) -> None:
        from mypy.nodes import MDEF, SymbolTableNode

        sym = SymbolTableNode(MDEF, self._var("x", True))
        cls = self._cls({"x": sym})
        res = self._seam([self._ref(self._var("x", True))], True, cls)
        assert res == (False, [])

    def test_seam_base_and_own_both_msg(self) -> None:
        from mypy.nodes import MDEF, SymbolTableNode

        sym = SymbolTableNode(MDEF, self._var("x", True))
        cls = self._cls({"x": sym})
        res = self._seam([self._ref(self._var("x", True))], False, cls)
        assert res == (False, [("x", False), ("x", False)])

    def test_seam_base_method_not_var(self) -> None:
        from mypy.nodes import MDEF, FuncDef, SymbolTableNode

        sym = SymbolTableNode(MDEF, FuncDef("x"))
        cls = self._cls({"x": sym})
        res = self._seam([self._ref(self._var("x"))], False, cls)
        assert res == (False, [])

    def test_seam_base_var_no_info(self) -> None:
        from mypy.nodes import MDEF, SymbolTableNode

        # A fresh Var carries the VAR_NO_INFO FakeInfo, so `info is None`
        # is False and the message flag is False.
        base_final = self._var("x", True)
        sym = SymbolTableNode(MDEF, base_final)
        cls = self._cls({"x": sym})
        res = self._seam([self._ref(self._var("x"))], False, cls)
        assert res == (False, [("x", False)])

    def test_seam_final_without_value(self) -> None:
        v = self._var("x", True)
        v.final_unset_in_class = True
        v.final_set_in_init = False
        res = self._seam([self._ref(v)], True, self._cls(), s_type_is_none=False)
        assert res == (True, [])

    def test_seam_final_without_value_named_tuple(self) -> None:
        v = self._var("x", True)
        v.final_unset_in_class = True
        v.final_set_in_init = False
        res = self._seam(
            [self._ref(v)], True, self._cls(is_named_tuple=True), s_type_is_none=False
        )
        assert res == (False, [])

    def test_seam_final_without_value_stub(self) -> None:
        v = self._var("x", True)
        v.final_unset_in_class = True
        v.final_set_in_init = False
        res = self._seam([self._ref(v)], True, self._cls(), is_stub=True, s_type_is_none=False)
        assert res == (False, [])

    def test_seam_final_without_value_set_in_init(self) -> None:
        v = self._var("x", True)
        v.final_unset_in_class = True
        v.final_set_in_init = True
        res = self._seam([self._ref(v)], True, self._cls(), s_type_is_none=False)
        assert res == (False, [])

    def test_differential_plain_assignment(self) -> None:
        self._assert_par([self._ref(self._var("x"))], False, None)

    def test_differential_own_final(self) -> None:
        self._assert_par([self._ref(self._var("x", True))], False, None)

    def test_differential_own_final_decl(self) -> None:
        self._assert_par([self._ref(self._var("x", True))], True, None)

    def test_differential_base_final(self) -> None:
        from mypy.nodes import MDEF, SymbolTableNode

        sym = SymbolTableNode(MDEF, self._var("x", True))
        self._assert_par([self._ref(self._var("x"))], False, self._cls({"x": sym}))

    def test_differential_base_and_own(self) -> None:
        from mypy.nodes import MDEF, SymbolTableNode

        sym = SymbolTableNode(MDEF, self._var("x", True))
        self._assert_par([self._ref(self._var("x", True))], False, self._cls({"x": sym}))

    def test_differential_tuple_lvalues(self) -> None:
        from mypy.nodes import MDEF, SymbolTableNode, TupleExpr

        sym = SymbolTableNode(MDEF, self._var("x", True))
        cls = self._cls({"x": sym})
        lv1 = self._ref(self._var("x", True))
        lv2 = self._ref(self._var("y"))
        off, on = self._run([TupleExpr([lv1, lv2])], False, cls)
        assert_equal(on, off, "check_final tuple lvalues parity")

    def test_differential_final_without_value(self) -> None:
        v = self._var("x", True)
        v.final_unset_in_class = True
        v.final_set_in_init = False
        self._assert_par([self._ref(v)], True, self._cls(), s_type=AnyType(TypeOfAny.explicit))

    def test_differential_final_without_value_stub(self) -> None:
        v = self._var("x", True)
        v.final_unset_in_class = True
        v.final_set_in_init = False
        self._assert_par(
            [self._ref(v)], True, self._cls(), is_stub=True, s_type=AnyType(TypeOfAny.explicit)
        )


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMissingAnnotationsSuite(Suite):
    """Parity for the Rust `check_for_missing_annotations` head port (#1009).

    `TypeChecker.check_for_missing_annotations` (checker.py:2722-2771)
    arbitrates annotation completeness for every function def: the
    `show_untyped` gate, the `has_explicit_annotation` scan feeding
    `check_incomplete_defs`, the self/cls-only special case for an untyped
    def, and the per-site return/param Any-ness (including the generator /
    coroutine ret-type unwrapping). The Rust classifier
    (`checker_functions.rs`) turns those facts into `(tag, param_fail)`;
    the Python shim applies the fail/note side effects (the RETURN_UNTYPED
    note decision routes through the existing `rust_has_return_statement`
    seam) and keeps the pure-Python body as the fallback.

    Direct seam calls assert the exact tag for every branch; the gate-off
    vs gate-on differential drives the real TypeChecker method through a
    stub fail/note recorder and asserts identical message lists.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.geni = self.fx.make_type_info(
            "typing.Generator",
            mro=[self.fx.oi],
            typevars=["T", "T2", "T3"],
            variances=[COVARIANT] * 3,
        )
        self.coroi = self.fx.make_type_info(
            "typing.Coroutine",
            mro=[self.fx.oi],
            typevars=["T", "T2", "T3"],
            variances=[COVARIANT] * 3,
        )
        types_to_resolve = [
            self.fx.ai,
            self.fx.bi,
            self.fx.oi,
            self.fx.str_type_info,
            self.geni,
            self.coroi,
        ]
        set_wire_typeinfo_map({info.fullname: info for info in types_to_resolve})
        # Registered under a fullname distinct from the ghost "mod.A" used by
        # test_seam_defers_on_ghost_alias_return; the alias-return expansion
        # round (wave16) needs it in the snapshot.
        self.ret_alias = TypeAlias(self.fx.a, "mod.RA", "mod", -1, -1)
        self.resolver = _type_kernel.build_native_resolver(types_to_resolve, [self.ret_alias])
        self._set_active = _set_native_checker_active
        self._set_resolver = _set_native_checker_resolver
        self._set_active(True)
        self._set_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _fdef(
        self,
        typ: ProperType | None,
        arg_names: tuple[str | None, ...] = ("x",),
        is_generator: bool = False,
        is_coroutine: bool = False,
    ) -> FuncItem:
        from mypy.nodes import FuncDef

        arguments = []
        for name in arg_names:
            var = Var(name or "_")
            arg = Argument(var, None, None, ARG_POS)
            if name is None:
                arg.pos_only = True
            arguments.append(arg)
        # FuncDef (a concrete FuncItem subclass): production only ever
        # passes FuncDef into check_for_missing_annotations.
        f = FuncDef("f", arguments)
        f.type = typ
        f.is_generator = is_generator
        f.is_coroutine = is_coroutine
        return f

    def _callable(self, ret: Type, arg_types: list[Type]) -> CallableType:
        return CallableType(
            arg_types,
            [ARG_POS] * len(arg_types),
            [None] * len(arg_types),
            ret,
            self.fx.function,
            name="f",
        )

    def _run(
        self,
        fdef: FuncItem,
        *,
        is_typeshed_stub: bool = False,
        warn_incomplete_stub: bool = False,
        disallow_untyped_defs: bool = True,
        disallow_incomplete_defs: bool = False,
    ) -> tuple[list[str], list[str]]:
        from mypy.checker import TypeChecker

        def check_one() -> tuple[list[str], list[str]]:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            chk.options.warn_incomplete_stub = warn_incomplete_stub
            chk.options.disallow_untyped_defs = disallow_untyped_defs
            chk.options.disallow_incomplete_defs = disallow_incomplete_defs
            chk.is_typeshed_stub = is_typeshed_stub
            # Minimal lookup plumbing: the pure-Python fallback path calls
            # named_generic_type -> lookup_qualified -> self.modules when
            # unwrapping generator/coroutine declared return types.
            from mypy.nodes import MypyFile, SymbolTableNode

            mod = MypyFile([], [], False)
            mod.names = SymbolTable(
                {
                    "Generator": SymbolTableNode(GDEF, self.geni),
                    "Coroutine": SymbolTableNode(GDEF, self.coroi),
                }
            )
            chk.modules = {"typing": mod}
            msgs: list[str] = []
            notes: list[str] = []

            class _Msg:
                def fail(self, msg: str, _ctx: Any, **_kw: Any) -> None:
                    msgs.append(str(msg))

                def note(self, msg: str, _ctx: Any, **_kw: Any) -> None:
                    notes.append(str(msg))

            chk.msg = _Msg()  # type: ignore[assignment]
            chk.check_for_missing_annotations(fdef)
            return msgs, notes

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(on, off, f"check_for_missing_annotations parity for {fdef!r}")
        return off

    def _assert_par(
        self, fdef: FuncItem, expected_msgs: list[str], expected_notes: list[str], **kw: Any
    ) -> None:
        msgs, notes = self._run(fdef, **kw)
        assert msgs == expected_msgs, f"{msgs!r} != {expected_msgs!r}"
        assert notes == expected_notes, f"{notes!r} != {expected_notes!r}"

    def _gen(self, args: list[Type]) -> Instance:
        return Instance(self.geni, args)

    def _coro(self, args: list[Type]) -> Instance:
        return Instance(self.coroi, args)

    # -- direct seam calls --

    def _seam(self, fdef: FuncItem, **kw: Any) -> Any:
        from mypy.checker import _serialize_type_for_checker

        is_typeshed_stub = kw.get("is_typeshed_stub", False)
        warn_incomplete_stub = kw.get("warn_incomplete_stub", False)
        disallow_untyped_defs = kw.get("disallow_untyped_defs", True)
        disallow_incomplete_defs = kw.get("disallow_incomplete_defs", False)
        if fdef.type is None:
            type_tag, ret_bytes, arg_blobs = 0, None, []
        else:
            type_tag = 1
            assert isinstance(fdef.type, CallableType)
            ret_bytes = _serialize_type_for_checker(fdef.type.ret_type)
            arg_blobs = [_serialize_type_for_checker(t) for t in fdef.type.arg_types]
        return _type_kernel.rust_classify_missing_annotations(
            is_typeshed_stub,
            warn_incomplete_stub,
            disallow_untyped_defs,
            disallow_incomplete_defs,
            type_tag,
            len(fdef.arguments),
            list(fdef.arg_names),
            bool(fdef.is_generator),
            bool(fdef.is_coroutine),
            ret_bytes,
            arg_blobs,
            state.strict_optional,
            self.resolver,
        )

    def test_seam_untyped_no_args(self) -> None:
        res = self._seam(self._fdef(None, ()))
        assert res == (1, False)

    def test_seam_untyped_self_only(self) -> None:
        res = self._seam(self._fdef(None, ("self",)))
        assert res == (1, False)

    def test_seam_untyped_cls_only(self) -> None:
        res = self._seam(self._fdef(None, ("cls",)))
        assert res == (1, False)

    def test_seam_untyped_regular_args(self) -> None:
        res = self._seam(self._fdef(None, ("x",)))
        assert res == (2, False)
        res = self._seam(self._fdef(None, ("self", "x")))
        assert res == (2, False)

    def test_seam_unannotated_return_and_param(self) -> None:
        fdef = self._fdef(
            self._callable(AnyType(TypeOfAny.unannotated), [AnyType(TypeOfAny.unannotated)]),
            ("x",),
        )
        res = self._seam(fdef)
        assert res == (3, True)

    def test_seam_generator_unannotated_tr(self) -> None:
        fdef = self._fdef(
            self._callable(self._gen([self.fx.a, self.fx.a, AnyType(TypeOfAny.unannotated)]), []),
            (),
            is_generator=True,
        )
        res = self._seam(fdef)
        assert res == (3, False)

    def test_seam_defers_on_ghost_alias_return(self) -> None:
        # TypeAliasType has no resolved alias target on the wire, so the
        # classifier defers and the Python body handles the type.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        fdef = self._fdef(self._callable(TypeAliasType(alias, []), []), ())
        res = self._seam(fdef)
        assert res is None

    # -- alias-expansion round (wave16: the alias-return defer was lifted) --

    def test_seam_alias_return_expands(self) -> None:
        # A resolvable alias return expands instead of deferring: the ret
        # type is "annotated" (a plain Instance) after expansion.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.RA", "mod", -1, -1)
        fdef = self._fdef(self._callable(TypeAliasType(alias, []), []), ())
        res = self._seam_alias(fdef, alias)
        assert res == (0, False)

    def test_seam_alias_return_unannotated_args(self) -> None:
        # Expansion decides the ret site, so only the param fail bit fires.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.RA", "mod", -1, -1)
        fdef = self._fdef(
            self._callable(TypeAliasType(alias, []), [AnyType(TypeOfAny.unannotated)]), ("x",)
        )
        res = self._seam_alias(fdef, alias)
        assert res == (0, True)

    def test_alias_return_parity(self) -> None:
        # Gate-off expands the alias through get_proper_type; gate-on now
        # expands inside the seam. Both must produce the same messages.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.RA", "mod", -1, -1)
        fdef = self._fdef(
            self._callable(TypeAliasType(alias, []), [AnyType(TypeOfAny.unannotated)]), ("x",)
        )
        self._assert_par(
            fdef,
            ["Function is missing a type annotation for one or more parameters"],
            [],
            disallow_untyped_defs=True,
            disallow_incomplete_defs=True,
        )
        fdef2 = self._fdef(self._callable(TypeAliasType(alias, []), []), ())
        self._assert_par(fdef2, [], [])

    def _seam_alias(self, fdef: FuncItem, alias: Any) -> Any:
        # Same as _seam but with a resolver snapshot that can expand the
        # alias (the suite resolver is built without one).
        from mypy.checker import _serialize_type_for_checker

        resolver = _type_kernel.build_native_resolver(
            [self.fx.ai, self.fx.bi, self.fx.oi, self.fx.str_type_info], [alias]
        )
        type_tag = 1
        assert isinstance(fdef.type, CallableType)
        ret_bytes = _serialize_type_for_checker(fdef.type.ret_type)
        arg_blobs = [_serialize_type_for_checker(t) for t in fdef.type.arg_types]
        return _type_kernel.rust_classify_missing_annotations(
            False,
            False,
            True,
            False,
            type_tag,
            len(fdef.arguments),
            list(fdef.arg_names),
            bool(fdef.is_generator),
            bool(fdef.is_coroutine),
            ret_bytes,
            arg_blobs,
            state.strict_optional,
            resolver,
        )

    # -- gate-off vs gate-on differentials --

    def test_untyped_def_no_args(self) -> None:
        fdef = self._fdef(None, ())
        self._assert_par(
            fdef,
            ["Function is missing a return type annotation"],
            ['Use "-> None" if function does not return a value'],
        )

    def test_untyped_def_self_only(self) -> None:
        fdef = self._fdef(None, ("self",))
        self._assert_par(
            fdef,
            ["Function is missing a return type annotation"],
            ['Use "-> None" if function does not return a value'],
        )

    def test_untyped_def_cls_only(self) -> None:
        fdef = self._fdef(None, ("cls",))
        self._assert_par(
            fdef,
            ["Function is missing a return type annotation"],
            ['Use "-> None" if function does not return a value'],
        )

    def test_untyped_def_regular_args(self) -> None:
        fdef = self._fdef(None, ("x",))
        self._assert_par(fdef, ["Function is missing a type annotation"], [])
        fdef = self._fdef(None, ("self", "x"))
        self._assert_par(fdef, ["Function is missing a type annotation"], [])

    def test_untyped_def_generator_no_note(self) -> None:
        # A generator gets the fail but not the "-> None" note.
        fdef = self._fdef(None, (), is_generator=True)
        self._assert_par(fdef, ["Function is missing a return type annotation"], [])

    def test_untyped_def_with_return_statement_no_note(self) -> None:
        # A non-trivial return suppresses the note; the self-only case routes
        # through the return-annotation branch. Must be a FuncDef (production
        # only passes FuncDef; the AST serializer drops bare FuncItem bodies).
        from mypy.nodes import Block, FuncDef, ReturnStmt, StrExpr

        f = FuncDef(
            "f", [Argument(Var("self"), None, None, ARG_POS)], Block([ReturnStmt(StrExpr("x"))])
        )
        f.type = None
        self._assert_par(f, ["Function is missing a return type annotation"], [])

    def test_callable_unannotated_return(self) -> None:
        fdef = self._fdef(self._callable(AnyType(TypeOfAny.unannotated), [self.fx.a]), ("x",))
        self._assert_par(fdef, ["Function is missing a return type annotation"], [])

    def test_callable_unannotated_param(self) -> None:
        fdef = self._fdef(self._callable(self.fx.o, [AnyType(TypeOfAny.unannotated)]), ("x",))
        self._assert_par(
            fdef, ["Function is missing a type annotation for one or more parameters"], []
        )

    def test_callable_annotated_no_messages(self) -> None:
        fdef = self._fdef(self._callable(self.fx.o, [self.fx.o]), ("x",))
        self._assert_par(fdef, [], [], disallow_untyped_defs=True, disallow_incomplete_defs=True)

    def test_gate_off_all_unannotated_incomplete_only(self) -> None:
        # has_explicit_annotation is False when every site is unannotated
        # Any, so check_incomplete_defs alone leaves the gate off.
        fdef = self._fdef(
            self._callable(AnyType(TypeOfAny.unannotated), [AnyType(TypeOfAny.unannotated)]),
            ("x",),
        )
        self._assert_par(fdef, [], [], disallow_untyped_defs=False, disallow_incomplete_defs=True)

    def test_callable_generator_unannotated_tr(self) -> None:
        fdef = self._fdef(
            self._callable(self._gen([self.fx.a, self.fx.a, AnyType(TypeOfAny.unannotated)]), []),
            (),
            is_generator=True,
        )
        self._assert_par(fdef, ["Function is missing a return type annotation"], [])

    def test_callable_generator_annotated_tr(self) -> None:
        fdef = self._fdef(
            self._callable(self._gen([self.fx.a, self.fx.a, self.fx.str_type]), []),
            (),
            is_generator=True,
        )
        self._assert_par(fdef, [], [])

    def test_callable_coroutine_unannotated_tr(self) -> None:
        fdef = self._fdef(
            self._callable(self._coro([self.fx.a, self.fx.a, AnyType(TypeOfAny.unannotated)]), []),
            (),
            is_coroutine=True,
        )
        self._assert_par(fdef, ["Function is missing a return type annotation"], [])

    def test_callable_coroutine_annotated_tr(self) -> None:
        fdef = self._fdef(
            self._callable(self._coro([self.fx.a, self.fx.a, self.fx.str_type]), []),
            (),
            is_coroutine=True,
        )
        self._assert_par(fdef, [], [])

    def test_typeshed_stub_no_warn_noop(self) -> None:
        fdef = self._fdef(None, ())
        self._assert_par(fdef, [], [], is_typeshed_stub=True)
        fdef = self._fdef(
            self._callable(AnyType(TypeOfAny.unannotated), [AnyType(TypeOfAny.unannotated)]),
            ("x",),
        )
        self._assert_par(fdef, [], [], is_typeshed_stub=True)

    def test_typeshed_stub_warn_incomplete(self) -> None:
        fdef = self._fdef(self._callable(self.fx.o, [AnyType(TypeOfAny.unannotated)]), ("x",))
        self._assert_par(
            fdef,
            ["Function is missing a type annotation for one or more parameters"],
            [],
            is_typeshed_stub=True,
            warn_incomplete_stub=True,
        )


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeReturnStmtSuite(Suite):
    """Parity for the Rust `check_return_stmt` two-phase decision port.

    `TypeChecker.check_return_stmt` (checker.py:6546) is a two-phase seam:
    phase 1 picks the return-type variant (generator / coroutine / plain)
    and fires NO_RETURN_EXPECTED on a non-ambiguous UninhabitedType; the
    accept() call and its binder side effects stay in Python; phase 2
    classifies the post-accept arms (async-generator fail, warn_return_any
    gate, declared-None exemptions) and the empty-return arms. The shim
    applies the four distinct fail messages plus the
    incorrectly_returning_any note; the pure-Python classification tail is
    the fallback.

    Direct seam calls assert the variant tags, the phase-1 bool, the
    phase-2 tags, and the None deferrals on undecodable bytes; the gate-off
    vs gate-on differential drives the real TypeChecker method through a
    stub message recorder and asserts identical message lists.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _bytes_of(self, t: Type) -> bytes:
        from mypy.checker import _serialize_type_for_checker

        return _serialize_type_for_checker(t)

    def _defn(
        self,
        *,
        is_generator: bool = False,
        is_coroutine: bool = False,
        is_async_generator: bool = False,
        name: str = "f",
        lambda_: bool = False,
    ) -> Any:
        if lambda_:
            from mypy.nodes import LambdaExpr

            defn: Any = LambdaExpr.__new__(LambdaExpr)
        else:
            defn = SimpleNamespace()
        defn.is_generator = is_generator
        defn.is_coroutine = is_coroutine
        defn.is_async_generator = is_async_generator
        if not lambda_:
            # LambdaExpr.name is a read-only property (returns LAMBDA_NAME).
            defn.name = name
        return defn

    def _run(
        self,
        defn: Any,
        return_type: Type,
        *,
        expr_type: Type | None = None,
        warn_return_any: bool = False,
        deferred: bool = False,
    ) -> list[tuple[str, str]]:
        """Run check_return_stmt with stubbed checker state; empty return
        when expr_type is None, else a NameExpr whose accept returns
        expr_type."""
        from mypy.nodes import NameExpr

        s_expr: Any = None
        if expr_type is not None:
            s_expr = NameExpr("x")
        return self._run_with_expr_node(
            defn,
            s_expr,
            return_type,
            accept_result=expr_type,
            warn_return_any=warn_return_any,
            deferred=deferred,
        )

    def _run_with_expr_node(
        self,
        defn: Any,
        s_expr: Any,
        return_type: Type,
        *,
        accept_result: Type | None = None,
        warn_return_any: bool = False,
        deferred: bool = False,
    ) -> list[tuple[str, str]]:
        """Run check_return_stmt with an explicit expression node (None for
        an empty return) through a stubbed checker, gate-off vs gate-on."""
        from mypy.checker import TypeChecker
        from mypy.nodes import ReturnStmt

        def check_one() -> list[tuple[str, str]]:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            chk.options.warn_return_any = warn_return_any
            msgs: list[tuple[str, str]] = []
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                fail=lambda msg, ctx, **kw: msgs.append(("fail", str(msg))),
                incorrectly_returning_any=lambda typ, ctx: msgs.append(("warn_any", str(typ))),
            )
            chk.scope = SimpleNamespace(current_function=lambda: defn)  # type: ignore[assignment]
            chk.return_types = [return_type]
            chk.current_node_deferred = deferred
            chk.dynamic_funcs = []
            chk.check_subtype = lambda **kw: msgs.append(("subtype", str(kw["subtype"])))  # type: ignore[method-assign,assignment]
            if s_expr is not None:
                s: ReturnStmt = ReturnStmt(s_expr)
                chk._expr_checker = SimpleNamespace(  # type: ignore[assignment]
                    accept=lambda *args, **kwargs: accept_result
                )
            else:
                s = ReturnStmt(None)
            chk.check_return_stmt(s)
            return msgs

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(on, off, f"check_return_stmt parity for {defn!r}")
        return on

    # -- direct seam calls -------------------------------------------------

    def test_seam_variant_tags(self) -> None:
        assert _type_kernel.rust_classify_return_stmt_variant(False, False) == 3
        assert _type_kernel.rust_classify_return_stmt_variant(True, False) == 1
        assert _type_kernel.rust_classify_return_stmt_variant(False, True) == 2
        assert _type_kernel.rust_classify_return_stmt_variant(True, True) == 1

    def test_seam_pre_uninhabited(self) -> None:
        never = self._bytes_of(UninhabitedType())
        assert _type_kernel.rust_classify_return_stmt_pre(never, False) is True

    def test_seam_pre_uninhabited_ambiguous(self) -> None:
        ambiguous = self._bytes_of(UninhabitedType(ambiguous=True))
        assert _type_kernel.rust_classify_return_stmt_pre(ambiguous, False) is False

    def test_seam_pre_lambda_suppresses(self) -> None:
        never = self._bytes_of(UninhabitedType())
        assert _type_kernel.rust_classify_return_stmt_pre(never, True) is False

    def test_seam_pre_plain_types_proceed(self) -> None:
        assert (
            _type_kernel.rust_classify_return_stmt_pre(self._bytes_of(self.fx.str_type), False)
            is False
        )
        assert (
            _type_kernel.rust_classify_return_stmt_pre(self._bytes_of(NoneType()), False) is False
        )
        assert (
            _type_kernel.rust_classify_return_stmt_pre(
                self._bytes_of(AnyType(TypeOfAny.special_form)), False
            )
            is False
        )

    def test_seam_pre_defers_on_garbage(self) -> None:
        assert _type_kernel.rust_classify_return_stmt_pre(b"\xff\xff\xff", False) is None

    def test_seam_post_tags(self) -> None:
        post = _type_kernel.rust_classify_return_stmt_post
        ret_str = self._bytes_of(self.fx.str_type)
        ret_none = self._bytes_of(NoneType())
        ret_any = self._bytes_of(AnyType(TypeOfAny.unannotated))
        ret_obj = self._bytes_of(self.fx.o)
        typ_int = self._bytes_of(self.fx.a)
        typ_any = self._bytes_of(AnyType(TypeOfAny.unannotated))
        # Post-accept arms.
        assert (
            post(
                typ_any,
                ret_str,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                True,
            )
            == 4
        )
        assert (
            post(
                typ_any,
                ret_str,
                False,
                False,
                False,
                False,
                True,
                False,
                False,
                False,
                False,
                True,
            )
            == 3
        )
        assert (
            post(
                typ_any,
                ret_any,
                False,
                False,
                False,
                False,
                True,
                False,
                False,
                False,
                False,
                True,
            )
            == 4
        )
        assert (
            post(
                typ_any,
                ret_obj,
                False,
                False,
                False,
                False,
                True,
                False,
                False,
                False,
                False,
                True,
            )
            == 4
        )
        assert (
            post(
                typ_int,
                ret_none,
                False,
                False,
                False,
                True,
                False,
                False,
                False,
                False,
                False,
                True,
            )
            == 6
        )
        assert (
            post(
                typ_int,
                ret_none,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                True,
            )
            == 7
        )
        # Empty-return arms (typ_bytes None).
        assert (
            post(None, ret_any, False, True, False, False, False, False, False, False, False, True)
            == 8
        )
        assert (
            post(
                None, ret_none, False, False, False, False, False, False, False, False, False, True
            )
            == 8
        )
        assert (
            post(
                None, ret_str, False, False, False, False, False, False, False, False, False, True
            )
            == 9
        )

    def test_seam_post_defers_on_garbage(self) -> None:
        post = _type_kernel.rust_classify_return_stmt_post
        ret_str = self._bytes_of(self.fx.str_type)
        assert (
            post(
                self._bytes_of(self.fx.a),
                b"\xff\xff\xff",
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                True,
            )
            is None
        )
        assert (
            post(
                b"\xff\xff\xff",
                ret_str,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                True,
            )
            is None
        )

    # -- gate-off vs gate-on differentials ---------------------------------

    def test_parity_empty_return_checked_fail(self) -> None:
        msgs = self._run(self._defn(), self.fx.str_type)
        assert len(msgs) == 1 and msgs[0][0] == "fail"

    def test_parity_empty_return_none_decl(self) -> None:
        assert self._run(self._defn(), NoneType()) == []

    def test_parity_empty_return_any_decl(self) -> None:
        assert self._run(self._defn(), AnyType(TypeOfAny.unannotated)) == []

    def test_parity_empty_return_generator_any(self) -> None:
        assert self._run(self._defn(is_generator=True), AnyType(TypeOfAny.unannotated)) == []

    def test_parity_empty_return_coroutine_any(self) -> None:
        assert self._run(self._defn(is_coroutine=True), AnyType(TypeOfAny.unannotated)) == []

    def test_parity_no_return_expected(self) -> None:
        msgs = self._run(self._defn(), UninhabitedType())
        assert len(msgs) == 1 and msgs[0][0] == "fail"
        assert "does not return" in msgs[0][1]

    def test_parity_no_return_expected_ambiguous(self) -> None:
        # Ambiguous UninhabitedType falls through to the empty-return arms.
        msgs = self._run(self._defn(), UninhabitedType(ambiguous=True), expr_type=self.fx.a)
        assert len(msgs) == 1 and msgs[0][0] == "subtype"

    def test_parity_no_return_expected_lambda(self) -> None:
        # Lambda suppresses NO_RETURN_EXPECTED; empty return then hits the
        # checked-function fail.
        msgs = self._run(self._defn(lambda_=True), UninhabitedType())
        assert len(msgs) == 1 and msgs[0][0] == "fail"
        assert "return a value" in msgs[0][1] or "Never" not in msgs[0][1]

    def test_parity_async_generator_fail(self) -> None:
        msgs = self._run(
            self._defn(is_async_generator=True), self.fx.str_type, expr_type=self.fx.a
        )
        assert len(msgs) == 1 and msgs[0][0] == "fail"
        assert "async generator" in msgs[0][1]

    def test_parity_warn_return_any(self) -> None:
        msgs = self._run(
            self._defn(),
            self.fx.str_type,
            expr_type=AnyType(TypeOfAny.unannotated),
            warn_return_any=True,
        )
        assert len(msgs) == 1 and msgs[0][0] == "warn_any"

    def test_parity_warn_return_any_silent_ret_any(self) -> None:
        msgs = self._run(
            self._defn(),
            AnyType(TypeOfAny.unannotated),
            expr_type=AnyType(TypeOfAny.unannotated),
            warn_return_any=True,
        )
        assert msgs == []

    def test_parity_warn_return_any_silent_object(self) -> None:
        msgs = self._run(
            self._defn(), self.fx.o, expr_type=AnyType(TypeOfAny.unannotated), warn_return_any=True
        )
        assert msgs == []

    def test_parity_warn_return_any_silent_lambda(self) -> None:
        msgs = self._run(
            self._defn(lambda_=True),
            self.fx.str_type,
            expr_type=AnyType(TypeOfAny.unannotated),
            warn_return_any=True,
        )
        assert msgs == []

    def test_parity_warn_return_any_silent_deferred(self) -> None:
        msgs = self._run(
            self._defn(),
            self.fx.str_type,
            expr_type=AnyType(TypeOfAny.unannotated),
            warn_return_any=True,
            deferred=True,
        )
        assert msgs == []

    def test_parity_warn_return_any_off(self) -> None:
        msgs = self._run(self._defn(), self.fx.str_type, expr_type=AnyType(TypeOfAny.unannotated))
        assert msgs == []

    def test_parity_binary_magic_not_implemented_silent(self) -> None:
        from mypy.nodes import NameExpr

        # __add__ is a binary magic method; `return NotImplemented` is the
        # sanctioned escape hatch. The stub accept returns the AnyType that
        # NotImplemented rewrites to, so the warn must stay silent.
        expr = NameExpr("NotImplemented")
        expr.fullname = "builtins.NotImplemented"
        defn = self._defn(name="__add__")
        msgs = self._run_with_expr_node(
            defn,
            expr,
            AnyType(TypeOfAny.special_form),
            accept_result=AnyType(TypeOfAny.special_form),
            warn_return_any=True,
        )
        assert msgs == []

    def test_parity_binary_magic_plain_expr_warns(self) -> None:
        from mypy.nodes import NameExpr

        # Same binary-magic function, but the expression is not the
        # NotImplemented literal: the warn fires.
        defn = self._defn(name="__add__")
        msgs = self._run_with_expr_node(
            defn,
            NameExpr("x"),
            self.fx.str_type,
            accept_result=AnyType(TypeOfAny.unannotated),
            warn_return_any=True,
        )
        assert len(msgs) == 1 and msgs[0][0] == "warn_any"

    def test_parity_none_declared_none_value_ok(self) -> None:
        msgs = self._run(self._defn(), NoneType(), expr_type=NoneType())
        assert msgs == []

    def test_parity_none_declared_fail(self) -> None:
        msgs = self._run(self._defn(), NoneType(), expr_type=self.fx.a)
        assert len(msgs) == 1 and msgs[0][0] == "fail"

    def test_parity_none_declared_lambda_ok(self) -> None:
        msgs = self._run(self._defn(lambda_=True), NoneType(), expr_type=self.fx.a)
        assert msgs == []

    def test_parity_check_subtype(self) -> None:
        msgs = self._run(self._defn(), self.fx.str_type, expr_type=self.fx.a)
        assert len(msgs) == 1 and msgs[0][0] == "subtype"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeCheckRaiseSuite(Suite):
    """Parity for the Rust `type_check_raise` decision-head port (#1050).

    `TypeChecker.type_check_raise` (checker.py:6979) classifies the raised
    expression's proper type: DeletedType -> deleted_as_rvalue early
    return, else the BaseException subtype check (already native) plus a
    zero-arg `check_call` for FunctionLike types, plus the
    "did you mean NotImplementedError" fail when the type is a
    _NotImplementedType instance or the raised expression is a call of
    `builtins.NotImplemented`. The Rust seam decides only the tag from the
    wire type plus the callee fullname fact; Python applies every side
    effect. Direct seam calls assert the exact tag for every branch;
    toggling the checker gate off vs on must produce identical captured
    message lists.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _wire(self, t: Type) -> bytes:
        from mypy.checker import _serialize_type_for_checker

        return _serialize_type_for_checker(t)

    def _notimpl_info(self, name: str, module: str) -> Any:
        from mypy.nodes import Block, SymbolTable, TypeInfo

        class_def = ClassDef(name, Block([]), None, [])
        class_def.fullname = f"{module}.{name}"
        return TypeInfo(SymbolTable(), class_def, module)

    def _notimpl_instance(self) -> Instance:
        return Instance(self._notimpl_info("_NotImplementedType", "builtins"), [])

    def _call_of_notimpl(self) -> Any:
        from mypy.nodes import CallExpr, NameExpr

        callee = NameExpr("NotImplemented")
        callee.fullname = "builtins.NotImplemented"
        return CallExpr(callee, [], [], [])

    def _run(
        self, typ: Type, active: bool, e: Any | None = None, optional: bool = False
    ) -> list[tuple[str, str]]:
        from mypy.checker import TypeChecker
        from mypy.options import Options

        fx = TypeFixture()
        from mypy.nodes import NameExpr, RaiseStmt

        if e is None:
            e = NameExpr("ctx")

        def run_one() -> list[tuple[str, str]]:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            records: list[tuple[str, str]] = []

            class _Msg:
                def deleted_as_rvalue(self, t: Any, _ctx: Any) -> None:
                    records.append(("deleted_as_rvalue", str(t.source)))

            chk.msg = _Msg()  # type: ignore[assignment]
            chk.fail = lambda msg, _ctx, **_kw: records.append(  # type: ignore[assignment]
                ("fail", str(msg.value))
            )
            chk.named_type = lambda _name: fx.o  # type: ignore[assignment]
            chk.check_subtype = (  # type: ignore[method-assign]
                lambda _typ, _exp, _ctx, _msg, **_kw: records.append(  # type: ignore[assignment]
                    ("check_subtype", "call")
                )
            )
            chk._expr_checker = SimpleNamespace(  # type: ignore[assignment]
                accept=lambda _e: typ,
                check_call=lambda t, _args, _kinds, _ctx: records.append(("check_call", str(t))),
            )
            s = RaiseStmt(None, None)
            chk.type_check_raise(e, s, optional)
            return records

        return self._with_gate(active, run_one)

    def _assert_par(self, typ: Type, e: Any | None = None, optional: bool = False) -> None:
        off = self._run(typ, False, e, optional)
        on = self._run(typ, True, e, optional)
        assert_equal(on, off, f"type_check_raise parity for typ={typ!r}")

    # -- direct seam tests (all 3 tags + deferral) --

    def test_seam_deleted(self) -> None:
        from mypy.checker import NATIVE_RAISE_DELETED

        tag = _type_kernel.rust_classify_type_check_raise(self._wire(DeletedType("x")), None)
        assert tag == NATIVE_RAISE_DELETED, f"{tag}"

    def test_seam_deleted_beats_callee(self) -> None:
        from mypy.checker import NATIVE_RAISE_DELETED

        tag = _type_kernel.rust_classify_type_check_raise(
            self._wire(DeletedType("x")), "builtins.NotImplemented"
        )
        assert tag == NATIVE_RAISE_DELETED, f"{tag}"

    def test_seam_plain(self) -> None:
        from mypy.checker import NATIVE_RAISE_PLAIN

        fx = TypeFixture()
        tag = _type_kernel.rust_classify_type_check_raise(self._wire(fx.str_type), None)
        assert tag == NATIVE_RAISE_PLAIN, f"{tag}"

    def test_seam_plain_any(self) -> None:
        from mypy.checker import NATIVE_RAISE_PLAIN

        tag = _type_kernel.rust_classify_type_check_raise(
            self._wire(AnyType(TypeOfAny.special_form)), None
        )
        assert tag == NATIVE_RAISE_PLAIN, f"{tag}"

    def test_seam_notimpl_instance(self) -> None:
        from mypy.checker import NATIVE_RAISE_NOT_IMPLEMENTED

        tag = _type_kernel.rust_classify_type_check_raise(
            self._wire(self._notimpl_instance()), None
        )
        assert tag == NATIVE_RAISE_NOT_IMPLEMENTED, f"{tag}"

    def test_seam_notimpl_types_module_name(self) -> None:
        from mypy.checker import NATIVE_RAISE_NOT_IMPLEMENTED

        info = self._notimpl_info("NotImplementedType", "types")
        tag = _type_kernel.rust_classify_type_check_raise(self._wire(Instance(info, [])), None)
        assert tag == NATIVE_RAISE_NOT_IMPLEMENTED, f"{tag}"

    def test_seam_notimpl_callee_fact(self) -> None:
        from mypy.checker import NATIVE_RAISE_NOT_IMPLEMENTED

        fx = TypeFixture()
        tag = _type_kernel.rust_classify_type_check_raise(
            self._wire(fx.str_type), "builtins.NotImplemented"
        )
        assert tag == NATIVE_RAISE_NOT_IMPLEMENTED, f"{tag}"

    def test_seam_notimpl_error_subclass_is_plain(self) -> None:
        from mypy.checker import NATIVE_RAISE_PLAIN

        tag = _type_kernel.rust_classify_type_check_raise(
            self._wire(AnyType(TypeOfAny.special_form)), "builtins.NotImplementedError"
        )
        assert tag == NATIVE_RAISE_PLAIN, f"{tag}"

    def test_seam_defers_on_bad_wire(self) -> None:
        assert _type_kernel.rust_classify_type_check_raise(b"\xff\xff\xff", None) is None

    # -- gate off/on differential tests (all branches) --

    def test_par_deleted(self) -> None:
        off = self._run(DeletedType("x"), False)
        on = self._run(DeletedType("x"), True)
        assert off == on, f"deleted: off={off} on={on}"
        assert ("deleted_as_rvalue", "x") in off, f"expected delete: {off}"

    def test_par_valid_exception(self) -> None:
        fx = TypeFixture()
        off = self._run(fx.str_type, False)
        on = self._run(fx.str_type, True)
        assert off == on, f"valid: off={off} on={on}"
        assert ("check_subtype", "call") in off, f"expected subtype: {off}"

    def test_par_valid_exception_optional(self) -> None:
        # `raise e from None` path: optional=True adds NoneType to the
        # expected union; the tail behavior is unchanged.
        fx = TypeFixture()
        off = self._run(fx.str_type, False, optional=True)
        on = self._run(fx.str_type, True, optional=True)
        assert off == on, f"optional: off={off} on={on}"
        assert ("check_subtype", "call") in off, f"expected subtype: {off}"

    def test_par_function_like(self) -> None:
        fx = TypeFixture()
        call = fx.callable_type(fx.anyt)
        off = self._run(call, False)
        on = self._run(call, True)
        assert off == on, f"callable: off={off} on={on}"
        assert ("check_call", str(call)) in off, f"expected check_call: {off}"

    def test_par_notimpl_instance(self) -> None:
        typ = self._notimpl_instance()
        off = self._run(typ, False)
        on = self._run(typ, True)
        assert off == on, f"notimpl: off={off} on={on}"
        assert ("check_subtype", "call") in off
        assert len([r for r in off if r[0] == "fail"]) == 1, f"expected fail: {off}"

    def test_par_notimpl_callee(self) -> None:
        fx = TypeFixture()
        e = self._call_of_notimpl()
        off = self._run(fx.str_type, False, e=e)
        on = self._run(fx.str_type, True, e=e)
        assert off == on, f"callee: off={off} on={on}"
        assert len([r for r in off if r[0] == "fail"]) == 1, f"expected fail: {off}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSimpleAssignmentSuite(Suite):
    """Parity for `rust_classify_simple_assignment` (issue #1055).

    `TypeChecker.check_simple_assignment` (checker.py:6325-6436) dispatches
    the stub '...' initializer, the try_fallback gate (inferred Var or a
    union lvalue, and not simple_rvalue), the direct accept arm (no
    fallback / no lvalue type / TypedDict context), and the
    preferred/fallback selector for the two-context re-inference. The Rust
    seam decides only the tag from the wire proper lvalue type plus live
    scalar facts; Python applies the stub return, the accept /
    infer_rvalue_with_fallback_context branch bodies, and the shared
    need-annotation / widening / check_subtype tail. Direct seam calls
    assert the exact tag for every branch; toggling the checker gate off
    vs on must produce identical captured observations.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _wire(self, t: Type) -> bytes:
        from mypy.checker import _serialize_type_for_checker

        return _serialize_type_for_checker(t)

    # ---- direct seam tag tests ----

    def test_seam_stub(self) -> None:
        from mypy.checker import NATIVE_SA_STUB

        tag = _type_kernel.rust_classify_simple_assignment(None, True, True, True, True, False)
        assert tag == NATIVE_SA_STUB, f"{tag}"

    def test_seam_direct_no_lvalue(self) -> None:
        from mypy.checker import NATIVE_SA_DIRECT

        tag = _type_kernel.rust_classify_simple_assignment(None, False, False, False, False, False)
        assert tag == NATIVE_SA_DIRECT, f"{tag}"

    def test_seam_direct_via_simple_rvalue(self) -> None:
        from mypy.checker import NATIVE_SA_DIRECT

        fx = TypeFixture()
        tag = _type_kernel.rust_classify_simple_assignment(
            self._wire(Instance(fx.ai, [])), False, False, True, False, True
        )
        assert tag == NATIVE_SA_DIRECT, f"{tag}"

    def test_seam_direct_via_typeddict_context(self) -> None:
        from mypy.checker import NATIVE_SA_DIRECT

        fx = TypeFixture()
        td = TypedDictType({}, set(), set(), fx.a)
        tag = _type_kernel.rust_classify_simple_assignment(
            self._wire(td), False, False, True, False, False
        )
        assert tag == NATIVE_SA_DIRECT, f"{tag}"
        # A union with a TypedDictType item is also a TypedDict context.
        un = UnionType([Instance(fx.ai, []), td])
        tag = _type_kernel.rust_classify_simple_assignment(
            self._wire(un), False, False, True, False, False
        )
        assert tag == NATIVE_SA_DIRECT, f"{tag}"

    def test_seam_direct_union_simple_rvalue(self) -> None:
        from mypy.checker import NATIVE_SA_DIRECT

        fx = TypeFixture()
        un = UnionType([Instance(fx.ai, []), Instance(fx.bi, [])])
        tag = _type_kernel.rust_classify_simple_assignment(
            self._wire(un), False, False, False, False, True
        )
        assert tag == NATIVE_SA_DIRECT, f"{tag}"

    def test_seam_fallback_no_preferred(self) -> None:
        from mypy.checker import NATIVE_SA_FALLBACK_NO_PREFERRED

        fx = TypeFixture()
        # inferred Var that is not a function argument.
        tag = _type_kernel.rust_classify_simple_assignment(
            self._wire(Instance(fx.ai, [])), False, False, True, False, False
        )
        assert tag == NATIVE_SA_FALLBACK_NO_PREFERRED, f"{tag}"

    def test_seam_fallback_lvalue_preferred(self) -> None:
        from mypy.checker import NATIVE_SA_FALLBACK_LVALUE_PREFERRED

        fx = TypeFixture()
        # Union lvalue without an inferred Var.
        un = UnionType([Instance(fx.ai, []), Instance(fx.bi, [])])
        tag = _type_kernel.rust_classify_simple_assignment(
            self._wire(un), False, False, False, False, False
        )
        assert tag == NATIVE_SA_FALLBACK_LVALUE_PREFERRED, f"{tag}"
        # Function-argument inferred Var (is_argument).
        tag = _type_kernel.rust_classify_simple_assignment(
            self._wire(Instance(fx.ai, [])), False, False, True, True, False
        )
        assert tag == NATIVE_SA_FALLBACK_LVALUE_PREFERRED, f"{tag}"

    def test_seam_defers_on_bad_wire(self) -> None:
        assert (
            _type_kernel.rust_classify_simple_assignment(
                b"\xff\xff\xff", False, False, True, False, False
            )
            is None
        )

    # ---- gate-off vs gate-on differential tests ----

    def _run(
        self,
        lvalue_type: Type | None,
        rvalue: Expression,
        rvalue_type: Type,
        inferred: Var | None = None,
        is_stub: bool = False,
        lvalue: Expression | None = None,
    ) -> tuple[tuple[object, ...], tuple[object, ...]]:
        from mypy.checker import TypeChecker

        def check_one() -> tuple[object, ...]:
            obs: list[object] = []
            # `expr_checker` is a read-only property on TypeChecker, so the
            # mock runs on a dynamic subclass whose plain class attribute
            # shadows the property.
            chk_cls = cast(
                "type[TypeChecker]", type("_Chk", (TypeChecker,), {"expr_checker": None})
            )
            chk = chk_cls.__new__(chk_cls)
            chk.is_stub = is_stub
            chk.options = Options()

            def accept(
                rv: Expression, type_context: Type | None = None, always_allow_any: bool = False
            ) -> Type:
                obs.append(("accept", str(type_context), always_allow_any))
                return rvalue_type

            def fallback(
                lt: Type | None,
                rv: Expression,
                preferred: Type | None,
                fallback_context: Type | None,
                inf: Var | None,
                aaa: bool,
            ) -> Type:
                obs.append(("fallback", str(preferred), str(fallback_context)))
                return rvalue_type

            chk.expr_checker = SimpleNamespace(accept=accept)  # type: ignore[misc, assignment]
            chk.infer_rvalue_with_fallback_context = fallback  # type: ignore[method-assign, assignment]
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                need_annotation_for_var=lambda v, ctx, options: obs.append(("need_ann",)),
                deleted_as_rvalue=lambda t, ctx: obs.append(("del_rvalue",)),
                deleted_as_lvalue=lambda t, ctx: obs.append(("del_lvalue",)),
            )
            chk.check_subtype = lambda nt, lt, ctx, msg, d1, d2, notes=None: obs.append(  # type: ignore[method-assign, assignment]
                ("subtype", str(lt))
            )
            chk.widened_vars = []
            chk._globals_widened_in_func = []
            chk.binder = SimpleNamespace(  # type: ignore[assignment]
                put=lambda e, t: obs.append(("binder_put",)), declarations={}
            )
            # The widening block is only reached via a NameExpr lvalue.
            chk.refers_to_different_scope = lambda name: False  # type: ignore[method-assign]
            chk.can_widen_in_scope = lambda name, orig_type: True  # type: ignore[method-assign]
            chk.set_inferred_type = lambda inf, lv, t: obs.append(("set_inferred", str(t)))  # type: ignore[method-assign, assignment]
            chk.scope = SimpleNamespace(top_level_function=lambda: None)  # type: ignore[assignment]
            ret = chk.check_simple_assignment(
                lvalue_type, rvalue, NameExpr("ctx"), lvalue=lvalue, inferred=inferred
            )
            obs.append(("ret", str(ret[0]), str(ret[1])))
            return tuple(obs)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(
        self,
        lvalue_type: Type | None,
        rvalue: Expression,
        rvalue_type: Type,
        inferred: Var | None = None,
        is_stub: bool = False,
        lvalue: Expression | None = None,
    ) -> tuple[tuple[object, ...], tuple[object, ...]]:
        off = self._run(lvalue_type, rvalue, rvalue_type, inferred, is_stub, lvalue)[0]
        on = self._run(lvalue_type, rvalue, rvalue_type, inferred, is_stub, lvalue)[1]
        assert off == on, f"off={off} on={on}"
        return off, on

    def _nonsimple_call(self) -> CallExpr:
        # simple_rvalue() is False for a CallExpr whose callee node is not
        # a function definition.
        return CallExpr(NameExpr("f"), [], [], [])

    def test_stub_ellipsis_precedes_the_gate(self) -> None:
        # The stub '...' initializer returns at checker.py:6512, before the
        # `_rust_classify_simple_assignment` gate is read, so both gate states
        # run identical Python and `_assert_par` cannot fail on this shape.
        obs = self._run(
            Instance(TypeFixture().ai, []),
            EllipsisExpr(),
            AnyType(TypeOfAny.special_form),
            is_stub=True,
        )[0]
        assert obs == (("ret", "Any", "A"),), obs

    def test_par_direct_no_lvalue(self) -> None:
        fx = TypeFixture()
        off, on = self._assert_par(None, IntExpr(1), fx.anyt)
        assert ("accept", "None", False) in off, off

    def test_par_direct_any_lvalue(self) -> None:
        # An Any lvalue type disables always_allow_any on the accept call.
        fx = TypeFixture()
        off, on = self._assert_par(fx.anyt, IntExpr(1), fx.anyt)
        assert ("accept", "Any", False) in off, off
        assert ("subtype", "Any") in off, off

    def test_par_direct_simple_rvalue(self) -> None:
        fx = TypeFixture()
        lvalue = Instance(fx.ai, [])
        off, on = self._assert_par(lvalue, IntExpr(1), lvalue)
        assert ("accept", str(lvalue), True) in off, off
        assert not any(isinstance(o, tuple) and o[0] == "fallback" for o in off), off

    def test_par_direct_typeddict_context(self) -> None:
        fx = TypeFixture()
        td = TypedDictType({}, set(), set(), fx.a)
        off, on = self._assert_par(td, self._nonsimple_call(), fx.a, inferred=Var("v"))
        assert ("accept", str(td), True) in off, off
        assert not any(isinstance(o, tuple) and o[0] == "fallback" for o in off), off

    def test_par_fallback_no_preferred(self) -> None:
        fx = TypeFixture()
        lvalue = Instance(fx.ai, [])
        var = Var("v")
        var.type = None
        off, on = self._assert_par(lvalue, self._nonsimple_call(), lvalue, inferred=var)
        assert ("fallback", "None", str(lvalue)) in off, off

    def test_par_fallback_lvalue_preferred_argument(self) -> None:
        fx = TypeFixture()
        lvalue = Instance(fx.ai, [])
        var = Var("v")
        var.is_argument = True
        off, on = self._assert_par(lvalue, self._nonsimple_call(), lvalue, inferred=var)
        assert ("fallback", str(lvalue), "None") in off, off

    def test_par_fallback_union_lvalue(self) -> None:
        fx = TypeFixture()
        un = UnionType([Instance(fx.ai, []), Instance(fx.bi, [])])
        off, on = self._assert_par(un, self._nonsimple_call(), Instance(fx.ai, []))
        assert ("fallback", str(un), "None") in off, off
        assert ("subtype", str(un)) in off, off

    def test_par_need_annotation(self) -> None:
        fx = TypeFixture()
        lvalue = Instance(fx.ai, [])
        var = Var("v")
        var.type = None
        # An uninhabited inferred rvalue type fires need_annotation_for_var
        # and the SetNothingToAny fixup rewrites it to Any.
        off, on = self._assert_par(
            lvalue, self._nonsimple_call(), UninhabitedType(ambiguous=True), inferred=var
        )
        assert ("need_ann",) in off, off
        assert ("ret", "Any", str(lvalue)) in off, off

    def test_par_deleted_rvalue(self) -> None:
        fx = TypeFixture()
        lvalue = Instance(fx.ai, [])
        off, on = self._assert_par(lvalue, IntExpr(1), DeletedType("x"))
        assert ("del_rvalue",) in off, off

    def test_par_widen(self) -> None:
        fx = TypeFixture()
        int_inst = Instance(fx.ai, [])
        unrel_inst = Instance(fx.di, [])  # D is unrelated to A: not a proper subtype
        var = Var("x")
        var.type = int_inst
        lvalue = NameExpr("x")
        # The widened lvalue type becomes the union of the original and the
        # newly inferred type; the binder and set_inferred_type observe it.
        off, on = self._assert_par(
            int_inst, self._nonsimple_call(), unrel_inst, inferred=var, lvalue=lvalue
        )
        assert ("set_inferred", str(UnionType([int_inst, unrel_inst]))) in off, off
        assert ("subtype", str(UnionType([int_inst, unrel_inst]))) in off, off


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAllSupersGateSuite(Suite):
    """Parity for the Rust `check_compatibility_all_supers` gate-head port.

    `TypeChecker.check_compatibility_all_supers` (checker.py:4915-5008) opens
    with a pure scalar gate (isinstance(lvalue_node, Var), line equality or
    inferred-without-explicit-self-type, kind in (MDEF, None), non-empty
    bases) and its MRO tail skips bases under allow_incompatible_override
    (with the __slots__/builtins.object exception) or is_private. The Rust
    classifier (`checker_functions.rs`) decides the gate and the per-base
    skip list in one call; the per-base check bodies (message emission,
    is_writable_attribute), the node_type_from_base lookups, and the
    inferred-var type stash/restore stay in Python, and the pure-Python body
    remains the fallback.

    Direct seam calls assert the gate tag and the per-base skip list; the
    gate-off vs gate-on differential drives the real TypeChecker method
    through stubbed sub-checks and asserts identical event sequences.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _class_info(self, fullname: str, name: str) -> TypeInfo:
        from mypy.nodes import Block

        cdef = ClassDef(name=name, defs=Block([]))
        cdef.fullname = fullname
        return TypeInfo(SymbolTable(), cdef, fullname)

    def _lvalue(
        self,
        *,
        var_name: str = "attr",
        var_line: int = 1,
        lvalue_line: int = 1,
        is_inferred: bool = True,
        explicit_self_type: bool = False,
        lvalue_kind: int | None = MDEF,
        allow_incompatible_override: bool = False,
        base_fullnames: tuple[str, ...] = ("mod.Base",),
        base_has_name: bool = True,
    ) -> NameExpr:
        bases = [self._class_info(fn, fn.rsplit(".", 1)[-1]) for fn in base_fullnames]
        for base in bases:
            if base_has_name:
                base.names[var_name] = SymbolTableNode(MDEF, Var(var_name))
        cls = self._class_info("mod.C", "C")
        # TypeInfo.bases holds Instances; direct_base_classes() reads
        # Instance.type to recover the TypeInfo of each direct base.
        cls.bases = [Instance(base, []) for base in bases]
        cls.mro = [cls, *bases]
        var = Var(var_name)
        var.line = var_line
        var.is_inferred = is_inferred
        var.explicit_self_type = explicit_self_type
        var.allow_incompatible_override = allow_incompatible_override
        var.info = cls
        var.type = NoneType()
        lvalue = NameExpr(var_name)
        lvalue.line = lvalue_line
        lvalue.kind = lvalue_kind
        lvalue.node = var
        return lvalue

    def _tag(self, lvalue_node: Any, lvalue_line: int, lvalue_kind: int | None) -> Any:
        return _type_kernel.rust_classify_all_supers_gate(
            lvalue_node, lvalue_line, lvalue_kind, MDEF
        )

    def _run(
        self, lvalue: NameExpr, *, super_ok: bool = True, var: Var | None = None
    ) -> tuple[list[tuple[Any, ...]], list[tuple[Any, ...]]]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        if var is None:
            var = cast(Var, lvalue.node)
        cls = var.info
        attr_type = NoneType()

        def check_one() -> list[tuple[Any, ...]]:
            chk = TypeChecker.__new__(TypeChecker)
            events: list[tuple[Any, ...]] = []

            def classvar_super(node: Any, base: Any, base_node: Any) -> bool:
                events.append(("classvar", base.fullname))
                return True

            def final_super(node: Any, base: Any, base_node: Any) -> bool:
                events.append(("final", base.fullname))
                return True

            def super_check(
                compare_type: Any,
                rvalue: Any,
                base: Any,
                base_type: Any,
                base_node: Any,
                *,
                always_allow_covariant: bool,
            ) -> bool:
                events.append(("super", base.fullname, always_allow_covariant))
                return super_ok

            def node_type_from_base(
                name: str,
                base: Any,
                context: Any,
                *,
                setter_type: bool = False,
                is_class: bool = False,
                current_class: Any = None,
            ) -> tuple[Any, Any]:
                if setter_type:
                    return None, None
                if base is cls:
                    return attr_type, var
                tnode = base.names.get(name)
                return (attr_type, tnode.node) if tnode else (None, None)

            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                incompatible_setter_override=lambda *a, **k: events.append(("setter",))
            )
            chk.check_compatibility_classvar_super = classvar_super  # type: ignore[method-assign]
            chk.check_compatibility_final_super = final_super  # type: ignore[method-assign]
            chk.check_compatibility_super = super_check  # type: ignore[method-assign,assignment]
            chk.node_type_from_base = node_type_from_base  # type: ignore[method-assign]
            chk._expr_checker = SimpleNamespace(  # type: ignore[assignment]
                accept=lambda rv, ctx=None: attr_type
            )
            chk.check_compatibility_all_supers(lvalue, IntExpr(1))
            return events

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, lvalue: NameExpr, *, super_ok: bool = True) -> None:
        off, on = self._run(lvalue, super_ok=super_ok)
        assert_equal(on, off, f"check_compatibility_all_supers parity for {lvalue.node!r}")

    def test_seam_not_var(self) -> None:
        assert self._tag(None, 1, MDEF) == (0, [])

    def test_seam_gate_proceeds(self) -> None:
        lvalue = self._lvalue()
        assert self._tag(lvalue.node, lvalue.line, lvalue.kind) == (1, [1])

    def test_seam_line_mismatch_skips(self) -> None:
        lvalue = self._lvalue(var_line=2, lvalue_line=1, is_inferred=False)
        assert self._tag(lvalue.node, lvalue.line, lvalue.kind) == (0, [])

    def test_seam_inferred_without_self_type_proceeds(self) -> None:
        # Line mismatch but inferred without an explicit self annotation.
        lvalue = self._lvalue(var_line=2, lvalue_line=1, is_inferred=True)
        assert self._tag(lvalue.node, lvalue.line, lvalue.kind) == (1, [1])

    def test_seam_inferred_with_self_type_skips(self) -> None:
        lvalue = self._lvalue(var_line=2, lvalue_line=1, is_inferred=True, explicit_self_type=True)
        assert self._tag(lvalue.node, lvalue.line, lvalue.kind) == (0, [])

    def test_seam_kind_none_proceeds(self) -> None:
        # None for Vars defined via self.
        lvalue = self._lvalue(lvalue_kind=None)
        assert self._tag(lvalue.node, lvalue.line, lvalue.kind) == (1, [1])

    def test_seam_kind_gdef_skips(self) -> None:
        lvalue = self._lvalue(lvalue_kind=GDEF)
        assert self._tag(lvalue.node, lvalue.line, lvalue.kind) == (0, [])

    def test_seam_no_bases_skips(self) -> None:
        lvalue = self._lvalue(base_fullnames=())
        assert self._tag(lvalue.node, lvalue.line, lvalue.kind) == (0, [])

    def test_seam_per_base_skip_list(self) -> None:
        # allow_incompatible_override skips every base except the
        # __slots__/builtins.object exception.
        lvalue = self._lvalue(
            var_name="__slots__",
            allow_incompatible_override=True,
            base_fullnames=("builtins.object", "mod.Base"),
        )
        assert self._tag(lvalue.node, lvalue.line, lvalue.kind) == (1, [1, 0])

    def test_seam_private_name_all_skipped(self) -> None:
        lvalue = self._lvalue(var_name="__priv", base_fullnames=("mod.Base", "mod.Base2"))
        assert self._tag(lvalue.node, lvalue.line, lvalue.kind) == (1, [0, 0])

    def test_seam_defers_on_none_info(self) -> None:
        # An unreadable info attribute defers to the pure-Python body.
        var = Var("attr")
        var.line = 1
        var.is_inferred = True
        var.info = None  # type: ignore[assignment]
        assert self._tag(var, 1, MDEF) is None

    def test_parity_gate_pass(self) -> None:
        self._assert_par(self._lvalue())

    def test_parity_gate_line_mismatch_skips(self) -> None:
        self._assert_par(self._lvalue(var_line=2, lvalue_line=1, is_inferred=False))

    def test_parity_not_var(self) -> None:
        lvalue = self._lvalue()
        var = cast(Var, lvalue.node)
        lvalue.node = None
        off, on = self._run(lvalue, var=var)
        assert_equal(on, off, "check_compatibility_all_supers parity for node=None")

    def test_parity_annotated_var_line_match(self) -> None:
        # Explicit annotation: not inferred, gate passes via line equality,
        # and the inferred-var stash/restore is skipped.
        self._assert_par(self._lvalue(is_inferred=False))

    def test_parity_allow_incompatible_override(self) -> None:
        self._assert_par(self._lvalue(allow_incompatible_override=True))

    def test_parity_slots_object_exception(self) -> None:
        self._assert_par(
            self._lvalue(
                var_name="__slots__",
                allow_incompatible_override=True,
                base_fullnames=("builtins.object", "mod.Base"),
            )
        )

    def test_parity_private_name(self) -> None:
        self._assert_par(self._lvalue(var_name="__priv"))

    def test_parity_super_incompatible_breaks(self) -> None:
        self._assert_par(self._lvalue(), super_ok=False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsWritableAttributeSuite(Suite):
    """Parity for the Rust `is_writable_attribute` pure-predicate port (#1071).

    `TypeChecker.is_writable_attribute` (checker.py:10167) is a pure bool
    over a live `Node`: a `Var` is writable unless it is a read-only
    property; a property `OverloadedFuncDef` is writable when its first
    item (kept a `Decorator`, mirroring the Python assert) has a settable
    property var; everything else is not writable. The Rust port
    (`checker_functions.rs`) reads the live node via PyO3 and returns the
    bool directly, mirroring `rust_is_final_enum_value`. Direct seam calls
    assert the exact bool; the gate-off vs gate-on differential drives the
    real TypeChecker method.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _var(self, name: str, is_property: bool = False, is_settable: bool = False) -> Var:
        v = Var(name)
        v.is_property = is_property
        v.is_settable_property = is_settable
        return v

    def _decorator(self, name: str, is_settable: bool) -> Decorator:
        v = Var(name)
        v.is_property = True
        v.is_settable_property = is_settable
        return Decorator(FuncDef(name), [], v)

    def _overloaded(self, is_settable: bool, is_property: bool = True) -> Any:
        ofd = OverloadedFuncDef([self._decorator("prop", is_settable)])
        ofd.is_property = is_property
        return ofd

    def _seam(self, node: Any) -> Any:
        return _type_kernel.rust_is_writable_attribute(node)

    def _run(self, node: Any) -> tuple[bool, bool]:

        from mypy.checker import TypeChecker

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_writable_attribute(node)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, node: Any) -> None:
        off, on = self._run(node)
        assert_equal(on, off, f"is_writable_attribute parity for node={node!r}")

    def test_seam_var_plain(self) -> None:
        assert self._seam(self._var("attr")) is True

    def test_seam_var_readonly_property(self) -> None:
        assert self._seam(self._var("prop", is_property=True)) is False

    def test_seam_var_settable_property(self) -> None:
        assert self._seam(self._var("prop", is_property=True, is_settable=True)) is True

    def test_seam_overloaded_settable(self) -> None:
        assert self._seam(self._overloaded(is_settable=True)) is True

    def test_seam_overloaded_non_settable(self) -> None:
        assert self._seam(self._overloaded(is_settable=False)) is False

    def test_seam_overloaded_not_property(self) -> None:
        assert self._seam(self._overloaded(is_settable=False, is_property=False)) is False

    def test_seam_funcdef(self) -> None:
        assert self._seam(FuncDef("method")) is False

    def test_seam_non_node(self) -> None:
        # Anything that is not a Var / property OverloadedFuncDef: False.
        assert self._seam(self.fx.oi) is False

    def test_parity_var_plain(self) -> None:
        self._assert_par(self._var("attr"))

    def test_parity_var_readonly_property(self) -> None:
        self._assert_par(self._var("prop", is_property=True))

    def test_parity_var_settable_property(self) -> None:
        self._assert_par(self._var("prop", is_property=True, is_settable=True))

    def test_parity_overloaded_settable(self) -> None:
        self._assert_par(self._overloaded(is_settable=True))

    def test_parity_overloaded_non_settable(self) -> None:
        self._assert_par(self._overloaded(is_settable=False))

    def test_parity_overloaded_not_property(self) -> None:
        self._assert_par(self._overloaded(is_settable=False, is_property=False))

    def test_parity_funcdef(self) -> None:
        self._assert_par(FuncDef("method"))

    def test_parity_non_node(self) -> None:
        self._assert_par(self.fx.oi)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsDefinedInBaseClassSuite(Suite):
    """Parity for the Rust `is_defined_in_base_class` pure-predicate port (#1601).

    `TypeChecker.is_defined_in_base_class` (checker.py:10168) is a pure bool
    over a live `Var`: returns `False` when `var.info` is falsy, `True` when
    `var.info.fallback_to_any`, else walks `info.mro[1:]` and returns `True`
    if any base's `names.get(var.name)` is not None. The Rust port reads the
    live Var via PyO3 and returns the bool directly, mirroring
    `rust_is_final_enum_value`. Direct seam calls assert the exact bool;
    the gate-off vs gate-on differential drives the real TypeChecker method.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _make_typeinfo(self, name: str = "mod.X") -> TypeInfo:
        from mypy.nodes import Block

        defn = ClassDef(name, Block([]))
        info = TypeInfo(SymbolTable(), defn, name)
        defn.info = info
        info.mro = [info]
        return info

    def _make_var(self, name: str, info: TypeInfo | None = None) -> Var:
        v = Var(name)
        if info is not None:
            v.info = info
        return v

    def _add_base(self, info: TypeInfo, base: TypeInfo, member_name: str | None = None) -> None:
        """Add base to info.mro and optionally register a member in base.names."""
        info.mro.insert(1, base)
        if member_name is not None:
            st = SymbolTableNode(GDEF, Var(member_name))
            base.names[member_name] = st

    def _seam(self, var: Var) -> Any:
        return _type_kernel.rust_is_defined_in_base_class(var)

    def _run(self, var: Var) -> tuple[bool, bool]:
        from mypy.checker import TypeChecker

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_defined_in_base_class(var)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, var: Var) -> None:
        off, on = self._run(var)
        assert_equal(on, off, f"is_defined_in_base_class parity for var={var!r}")

    def test_seam_no_info(self) -> None:
        v = self._make_var("attr")
        assert self._seam(v) is False

    def test_seam_fallback_to_any(self) -> None:
        info = self._make_typeinfo()
        info.fallback_to_any = True
        v = self._make_var("attr", info)
        assert self._seam(v) is True

    def test_seam_empty_mro(self) -> None:
        info = self._make_typeinfo()
        v = self._make_var("attr", info)
        assert self._seam(v) is False

    def test_seam_name_in_base(self) -> None:
        info = self._make_typeinfo()
        base = self._make_typeinfo("mod.Base")
        self._add_base(info, base, member_name="attr")
        v = self._make_var("attr", info)
        assert self._seam(v) is True

    def test_seam_name_not_in_base(self) -> None:
        info = self._make_typeinfo()
        base = self._make_typeinfo("mod.Base")
        self._add_base(info, base, member_name="other")
        v = self._make_var("attr", info)
        assert self._seam(v) is False

    def test_seam_name_in_second_base(self) -> None:
        info = self._make_typeinfo()
        base1 = self._make_typeinfo("mod.B1")
        base2 = self._make_typeinfo("mod.B2")
        self._add_base(info, base1, member_name="other")
        self._add_base(info, base2, member_name="attr")
        v = self._make_var("attr", info)
        assert self._seam(v) is True

    def test_parity_no_info(self) -> None:
        self._assert_par(self._make_var("attr"))

    def test_parity_fallback_to_any(self) -> None:
        info = self._make_typeinfo()
        info.fallback_to_any = True
        self._assert_par(self._make_var("attr", info))

    def test_parity_empty_mro(self) -> None:
        info = self._make_typeinfo()
        self._assert_par(self._make_var("attr", info))

    def test_parity_name_in_base(self) -> None:
        info = self._make_typeinfo()
        base = self._make_typeinfo("mod.Base")
        self._add_base(info, base, member_name="attr")
        self._assert_par(self._make_var("attr", info))

    def test_parity_name_not_in_base(self) -> None:
        info = self._make_typeinfo()
        base = self._make_typeinfo("mod.Base")
        self._add_base(info, base, member_name="other")
        self._assert_par(self._make_var("attr", info))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsDefinitionSuite(Suite):
    """Production pins for the retired `is_definition` seam (#1603).

    `TypeChecker.is_definition` (checker.py:6180) is a pure bool over a
    live `Lvalue`: a `NameExpr` is a definition when `is_inferred_def` is
    set or its `node` is a `Var` with `type is None`; a `MemberExpr` is a
    definition when `is_inferred_def` is set; else `False`. The Rust port
    `rust_is_definition` was retired by #1739: the alias and shim are gone,
    the pyfunction stays registered for the direct-seam calls below, and
    the host reads no gate, so a gate-off vs gate-on differential cannot
    fail on any shape here. The production tests pin the exact bool of
    one run each; the retirement pin lives in
    `testtypes_native_retired_checker.py::NativeIsDefinitionRetiredSuite`.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._set_active(False)

    def _name_expr(self, name: str, is_inferred_def: bool = False, node: Any = None) -> NameExpr:
        ne = NameExpr(name)
        ne.is_inferred_def = is_inferred_def
        if node is not None:
            ne.node = node
        return ne

    def _member_expr(self, name: str, is_inferred_def: bool = False) -> MemberExpr:
        me = MemberExpr(NameExpr("obj"), name)
        me.is_inferred_def = is_inferred_def
        return me

    def _seam(self, node: Any) -> Any:
        return _type_kernel.rust_is_definition(node)

    def _assert_value(self, node: Any, expected: bool) -> None:
        from mypy.checker import TypeChecker

        chk = TypeChecker.__new__(TypeChecker)
        got = chk.is_definition(node)
        assert got is expected, f"is_definition({node!r}) -> {got}"

    def test_seam_name_expr_inferred_def(self) -> None:
        assert self._seam(self._name_expr("x", is_inferred_def=True)) is True

    def test_seam_name_expr_var_no_type(self) -> None:
        v = Var("x")
        v.type = None
        assert self._seam(self._name_expr("x", node=v)) is True

    def test_seam_name_expr_var_with_type(self) -> None:
        v = Var("x")
        v.type = self.fx.o
        assert self._seam(self._name_expr("x", node=v)) is False

    def test_seam_name_expr_no_node(self) -> None:
        assert self._seam(self._name_expr("x")) is False

    def test_seam_member_expr_inferred_def(self) -> None:
        assert self._seam(self._member_expr("attr", is_inferred_def=True)) is True

    def test_seam_member_expr_not_inferred(self) -> None:
        assert self._seam(self._member_expr("attr")) is False

    def test_seam_non_expr(self) -> None:
        assert self._seam(self.fx.oi) is False

    def test_value_name_expr_inferred_def(self) -> None:
        self._assert_value(self._name_expr("x", is_inferred_def=True), True)

    def test_value_name_expr_var_no_type(self) -> None:
        v = Var("x")
        v.type = None
        self._assert_value(self._name_expr("x", node=v), True)

    def test_value_name_expr_var_with_type(self) -> None:
        v = Var("x")
        v.type = self.fx.o
        self._assert_value(self._name_expr("x", node=v), False)

    def test_value_member_expr_inferred_def(self) -> None:
        self._assert_value(self._member_expr("attr", is_inferred_def=True), True)

    def test_value_member_expr_not_inferred(self) -> None:
        self._assert_value(self._member_expr("attr"), False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckExitReturnTypeSuite(Suite):
    """Parity for `rust_check_exit_return_type` (issue #1597).

    `TypeChecker.check__exit__return_type` (checker.py:3949-3972) emits
    `incorrect__exit__return` when an `__exit__` method always returns
    `False` but its declared return type contains `bool`. The Rust seam
    is a live-PyO3-object port: it reads `defn.type` (CallableType check),
    calls Python's `get_proper_type` + `has_bool_item` (both already
    native), calls `all_return_statements` (already native), and checks
    each return's `expr` is a `NameExpr` with `fullname ==
    "builtins.False"`. Direct seam calls assert the expected bool;
    gate-off vs gate-on differentials drive the real TypeChecker method
    through a stub message recorder and must agree.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _false_name(self) -> Any:
        from mypy.nodes import NameExpr

        n = NameExpr("False")
        n.fullname = "builtins.False"
        return n

    def _true_name(self) -> Any:
        from mypy.nodes import NameExpr

        n = NameExpr("True")
        n.fullname = "builtins.True"
        return n

    def _func(self, ret_type: Type, returns: list[Any]) -> Any:
        from mypy.nodes import Block, FuncDef, ReturnStmt

        stmts: list[Statement] = [ReturnStmt(expr) for expr in returns]
        fd = FuncDef("__exit__", [], Block(stmts))
        ct = self.fx.callable_type(ret_type)
        fd.type = ct
        return fd

    def _seam(self, defn: Any) -> Any:
        return _type_kernel.rust_check_exit_return_type(defn)

    def _run(self, defn: Any) -> list[tuple[str, str]]:
        from mypy.checker import TypeChecker

        def check_one() -> list[tuple[str, str]]:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            msgs: list[tuple[str, str]] = []
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                fail=lambda msg, ctx, **kw: msgs.append(("fail", str(msg))),
                incorrect__exit__return=lambda ctx: msgs.append(
                    (
                        "fail",
                        '"bool" is invalid as return type for "__exit__" that always returns False',
                    )
                ),
            )
            chk.check__exit__return_type(defn)
            return msgs

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(on, off, f"check__exit__return_type parity for {defn!r}")
        return on

    def test_seam_all_false_returns(self) -> None:
        fd = self._func(self.fx.bool_type, [self._false_name(), self._false_name()])
        assert self._seam(fd) is True

    def test_seam_mixed_returns(self) -> None:
        fd = self._func(self.fx.bool_type, [self._false_name(), self._true_name()])
        assert self._seam(fd) is False

    def test_seam_true_returns(self) -> None:
        fd = self._func(self.fx.bool_type, [self._true_name()])
        assert self._seam(fd) is False

    def test_seam_no_returns(self) -> None:
        fd = self._func(self.fx.bool_type, [])
        assert self._seam(fd) is False

    def test_seam_no_bool_item(self) -> None:
        fd = self._func(self.fx.str_type, [self._false_name()])
        assert self._seam(fd) is False

    def test_seam_none_type(self) -> None:
        from mypy.nodes import Block, FuncDef

        fd = FuncDef("__exit__", [], Block([]))
        fd.type = None
        assert self._seam(fd) is False

    def test_seam_non_callable_type(self) -> None:
        from mypy.nodes import Block, FuncDef

        fd = FuncDef("__exit__", [], Block([]))
        fd.type = self.fx.bool_type  # not a CallableType
        assert self._seam(fd) is False

    def test_seam_union_with_bool(self) -> None:
        from mypy.types import UnionType

        ret = UnionType([self.fx.bool_type, self.fx.nonet])
        fd = self._func(ret, [self._false_name()])
        assert self._seam(fd) is True

    def test_parity_all_false(self) -> None:
        fd = self._func(self.fx.bool_type, [self._false_name()])
        msgs = self._run(fd)
        assert msgs == [
            ("fail", '"bool" is invalid as return type for "__exit__" that always returns False')
        ]

    def test_parity_mixed(self) -> None:
        fd = self._func(self.fx.bool_type, [self._false_name(), self._true_name()])
        msgs = self._run(fd)
        assert msgs == []

    def test_parity_no_returns(self) -> None:
        fd = self._func(self.fx.bool_type, [])
        msgs = self._run(fd)
        assert msgs == []

    def test_parity_no_bool_item(self) -> None:
        fd = self._func(self.fx.str_type, [self._false_name()])
        msgs = self._run(fd)
        assert msgs == []

    def test_parity_none_type(self) -> None:
        from mypy.nodes import Block, FuncDef

        fd = FuncDef("__exit__", [], Block([]))
        fd.type = None
        msgs = self._run(fd)
        assert msgs == []

    def test_parity_non_callable_type(self) -> None:
        from mypy.nodes import Block, FuncDef

        fd = FuncDef("__exit__", [], Block([]))
        fd.type = self.fx.bool_type
        msgs = self._run(fd)
        assert msgs == []


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckFinalDeletableSuite(Suite):
    """Parity for the Rust `check_final_deletable` port (H1d).

    `TypeChecker.check_final_deletable` (checker.py:4053-4059) iterates
    `typ.deletable_attributes`, looks up each in `typ.names`, and emits
    `CANNOT_MAKE_DELETABLE_FINAL` for any whose `node` is a final `Var`.
    The Rust port (`checker_functions.rs`) reads the live `TypeInfo` via
    PyO3 and returns the list of offending attr names. Direct seam calls
    assert the exact list; gate-off vs gate-on parity drives the real
    TypeChecker method through a mock `fail` recorder.
    """

    def setUp(self) -> None:

        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _make_info(
        self, deletable: list[str], names: dict[str, SymbolTableNode] | None = None
    ) -> TypeInfo:
        from mypy.nodes import Block, SymbolTable, TypeInfo

        table = SymbolTable()
        info = TypeInfo(table, ClassDef("X", Block([])), "mod.X")
        info.deletable_attributes = list(deletable)
        if names:
            for key, val in names.items():
                table[key] = val
        return info

    def _var_node(self, name: str, is_final: bool = False) -> SymbolTableNode:
        v = Var(name)
        v.is_final = is_final
        return SymbolTableNode(GDEF, v)

    def _funcdef_node(self, name: str) -> SymbolTableNode:
        return SymbolTableNode(GDEF, FuncDef(name))

    def _seam(self, typ: TypeInfo) -> Any:
        return _type_kernel.rust_check_final_deletable(typ)

    def _run(self, typ: TypeInfo) -> tuple[list[Any], list[Any]]:

        from mypy.checker import TypeChecker

        fails: list[Any] = []

        class _StubChecker(TypeChecker):
            def fail(self, msg: Any, context: Any, *, code: Any = None) -> Any:
                fails.append((str(msg), context))

        def check_one() -> list[Any]:
            nonlocal fails
            fails = []
            chk = _StubChecker.__new__(_StubChecker)
            chk.check_final_deletable(typ)
            return list(fails)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, typ: TypeInfo) -> None:
        off, on = self._run(typ)
        assert_equal(on, off, f"check_final_deletable parity for typ={typ!r}")

    # --- direct seam tests ---

    def test_seam_empty_deletable(self) -> None:
        info = self._make_info([])
        assert self._seam(info) == []

    def test_seam_var_final(self) -> None:
        info = self._make_info(["attr"], {"attr": self._var_node("attr", is_final=True)})
        assert self._seam(info) == ["attr"]

    def test_seam_var_not_final(self) -> None:
        info = self._make_info(["attr"], {"attr": self._var_node("attr", is_final=False)})
        assert self._seam(info) == []

    def test_seam_missing_name(self) -> None:
        info = self._make_info(["attr"])
        assert self._seam(info) == []

    def test_seam_non_var_node(self) -> None:
        info = self._make_info(["attr"], {"attr": self._funcdef_node("attr")})
        assert self._seam(info) == []

    def test_seam_multiple_attrs(self) -> None:
        info = self._make_info(
            ["a", "b", "c"],
            {
                "a": self._var_node("a", is_final=False),
                "b": self._var_node("b", is_final=True),
                "c": self._var_node("c", is_final=True),
            },
        )
        assert self._seam(info) == ["b", "c"]

    # --- gate-off vs gate-on parity ---

    def test_parity_empty(self) -> None:
        self._assert_par(self._make_info([]))

    def test_parity_var_final(self) -> None:
        info = self._make_info(["attr"], {"attr": self._var_node("attr", is_final=True)})
        self._assert_par(info)

    def test_parity_var_not_final(self) -> None:
        info = self._make_info(["attr"], {"attr": self._var_node("attr", is_final=False)})
        self._assert_par(info)

    def test_parity_missing_name(self) -> None:
        self._assert_par(self._make_info(["attr"]))

    def test_parity_non_var(self) -> None:
        info = self._make_info(["attr"], {"attr": self._funcdef_node("attr")})
        self._assert_par(info)

    def test_parity_multiple(self) -> None:
        info = self._make_info(
            ["a", "b", "c"],
            {
                "a": self._var_node("a", is_final=False),
                "b": self._var_node("b", is_final=True),
                "c": self._var_node("c", is_final=True),
            },
        )
        self._assert_par(info)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeInferOperatorAssignmentSuite(Suite):
    """Parity for the Rust `infer_operator_assignment_method` port (#1079).

    `TypeChecker.infer_operator_assignment_method` (checker.py:11498) plus
    its helper `_find_inplace_method` decide `(True, "__i<rest>")` vs
    `(False, method)` for augmented assignments: an Instance (or a
    TypedDictType via its fallback) whose class has a readable inplace
    member, gated by `operators.ops_with_inplace_method`. The Rust port
    (`checker_functions.rs`) reads the live proper type via PyO3 and
    returns the 2-tuple; `get_proper_type` and the operator-set membership
    stay shim-side. Direct seam calls assert the exact tuple; the
    gate-off vs gate-on differential drives the real module function.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _info(self, name: str, members: tuple[str, ...] = ()) -> TypeInfo:
        from mypy.nodes import Block

        defn = ClassDef(name, Block([]), None, [])
        defn.fullname = "mod." + name
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = [info]
        for member in members:
            fn = FuncDef(
                member, [], None, CallableType([], [], [], self.fx.anyt, self.fx.function)
            )
            fn.info = info
            info.names[member] = SymbolTableNode(MDEF, fn)
        return info

    def _td(self, info: TypeInfo) -> TypedDictType:
        return TypedDictType({}, set(), set(), Instance(info, []))

    def _facts(self, operator: str) -> tuple[str, bool]:
        from mypy import operators

        return (operators.op_methods[operator], operator in operators.ops_with_inplace_method)

    def _seam(self, typ: Any, operator: str) -> Any:
        method, in_ops = self._facts(operator)
        return _type_kernel.rust_infer_operator_assignment_method(typ, method, in_ops)

    def _run(self, typ: Type, operator: str) -> tuple[tuple[bool, str], tuple[bool, str]]:
        from mypy.checker import infer_operator_assignment_method

        def call() -> tuple[bool, str]:
            return infer_operator_assignment_method(typ, operator)

        off = self._with_gate(False, call)
        on = self._with_gate(True, call)
        return off, on

    def _assert_par(self, typ: Type, operator: str, expected: tuple[bool, str]) -> None:
        off, on = self._run(typ, operator)
        assert_equal(on, off, f"opassign parity for {typ!r} operator={operator!r}")
        assert_equal(on, expected, f"opassign value for {typ!r} operator={operator!r}")

    def test_seam_instance_with_inplace(self) -> None:
        itype = Instance(self._info("C", ("__iadd__",)), [])
        assert self._seam(itype, "+") == (True, "__iadd__")

    def test_seam_instance_without_inplace(self) -> None:
        itype = Instance(self._info("C"), [])
        assert self._seam(itype, "+") == (False, "__add__")

    def test_seam_typeddict_fallback_with_inplace(self) -> None:
        td = self._td(self._info("TD", ("__imul__",)))
        assert self._seam(td, "*") == (True, "__imul__")

    def test_seam_typeddict_fallback_without_inplace(self) -> None:
        td = self._td(self._info("TD"))
        assert self._seam(td, "*") == (False, "__mul__")

    def test_seam_non_instance(self) -> None:
        assert self._seam(self.fx.anyt, "+") == (False, "__add__")
        assert self._seam(NoneType(), "+") == (False, "__add__")

    def test_seam_operator_not_in_inplace_set(self) -> None:
        # "in" has no inplace method even when a member is present.
        itype = Instance(self._info("C", ("__icontains__",)), [])
        assert self._seam(itype, "in") == (False, "__contains__")

    def test_parity_instance_with_inplace(self) -> None:
        self._assert_par(Instance(self._info("C", ("__iadd__",)), []), "+", (True, "__iadd__"))

    def test_parity_instance_without_inplace(self) -> None:
        self._assert_par(Instance(self._info("C"), []), "+", (False, "__add__"))

    def test_parity_typeddict_fallback_with_inplace(self) -> None:
        self._assert_par(self._td(self._info("TD", ("__imul__",))), "*", (True, "__imul__"))

    def test_parity_typeddict_fallback_without_inplace(self) -> None:
        self._assert_par(self._td(self._info("TD")), "*", (False, "__mul__"))

    def test_parity_non_instance_types(self) -> None:
        self._assert_par(self.fx.anyt, "+", (False, "__add__"))
        self._assert_par(NoneType(), "+", (False, "__add__"))
        self._assert_par(self.fx.nonet, "+", (False, "__add__"))

    def test_parity_operator_not_in_inplace_set(self) -> None:
        # Member present but the operator admits no inplace method.
        self._assert_par(
            Instance(self._info("C", ("__icontains__",)), []), "in", (False, "__contains__")
        )
        self._assert_par(Instance(self._info("C"), []), "==", (False, "__eq__"))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeComparisonNarrowingSuite(Suite):
    """Parity for the Rust comparison_type_narrowing operand front (#1087).

    `TypeChecker.comparison_type_narrowing_helper` (checker.py:8579) Step 1
    classifies each operand as narrowable or not: the `literal(expr) ==
    LITERAL_TYPE` gate, the None / NotImplemented / True / False / enum
    literal suppressions, and the two non-narrowable proper-type tests
    (`FunctionLike.is_type_obj()` and `TypeType` over a `TypeVarType`).
    Rust decides from wire types plus shim-side literal flags; alias
    operands expand through the alias snapshot like `get_proper_type`
    (#1235), and an alias ret-type expanding to `UninhabitedType` decides
    `is_type_obj() == False`. Missing alias snapshots still defer. The
    literal-hash bookkeeping, chain grouping (already native via
    `rust_group_comparison_operands`), and the narrowing arm bodies stay
    Python-side.

    Direct seam calls assert the per-operand bools and the None deferrals;
    the gate-off vs gate-on differential drives the real helper with
    `narrow_type_by_identity_equality` stubbed to capture
    (operator, chain indices, narrowable indices) and asserts identical
    captures and returns.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver

        self.fx = TypeFixture()
        self._infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.str_type_info,
            self.fx.type_typei,
            self.fx.functioni,
            self.fx.std_tuplei,
            self.fx.std_listi,
        ]
        self._resolver = _type_kernel.build_native_resolver(self._infos, [])
        _set_native_checker_resolver(self._resolver)
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_resolver

        self._set_active(False)
        _set_native_checker_resolver(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _no_flags(self) -> tuple[bool, bool, bool, bool, bool]:
        return (False, False, False, False, False)

    def _seam(
        self, kinds: list[int], flags: list[tuple[bool, bool, bool, bool, bool]], types: list[Type]
    ) -> Any:
        from mypy.checker import _serialize_type_for_checker

        return _type_kernel.rust_classify_comparison_operands(
            kinds, flags, [_serialize_type_for_checker(t) for t in types], self._resolver
        )

    def _var_expr(self, name: str) -> NameExpr:
        from mypy.nodes import NameExpr, Var

        expr = NameExpr(name)
        expr.node = Var(name)
        return expr

    def _comparison(self, operators: list[str], operands: list[Any]) -> Any:

        return ComparisonExpr(operators, operands)

    def _tvar(self) -> TypeVarType:
        return TypeVarType(
            "T", "mod.T", TypeVarId(1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )

    def _checker(self, expr_types: dict[Any, Type]) -> Any:
        chk = TypeChecker.__new__(TypeChecker)
        chk.options = Options()
        chk._type_maps = [dict(expr_types)]
        return chk

    def _run_front(self, chk: Any, node: Any) -> Any:
        calls: list[Any] = []

        def fake_narrow(
            operator: str,
            operands: Any = None,
            operand_types: Any = None,
            expr_indices: Any = None,
            narrowable_indices: Any = (),
        ) -> tuple[TypeMap, TypeMap]:
            calls.append(
                (operator, tuple(expr_indices or []), tuple(sorted(narrowable_indices or ())))
            )
            return ({}, {})

        chk.narrow_type_by_identity_equality = fake_narrow
        chk.find_tuple_len_narrowing = lambda _node: []
        result = chk.comparison_type_narrowing_helper(node)
        return calls, result

    def _assert_par(self, node: Any, expr_types: dict[Any, Any], note: str) -> None:
        def run(active: bool) -> Any:
            chk = self._checker(expr_types)
            return self._with_gate(active, lambda: self._run_front(chk, node))

        off = run(False)
        on = run(True)
        assert_equal(on, off, f"comparison_type_narrowing parity: {note}")

    # --- direct seam calls ---

    def test_seam_plain_instances_narrowable(self) -> None:
        fx = self.fx
        assert self._seam([1, 1], [self._no_flags()] * 2, [fx.str_type, fx.o]) == [True, True]

    def test_seam_kind_gate_suppresses(self) -> None:
        fx = self.fx
        # kind != LITERAL_TYPE: not narrowable, other facts never consulted.
        assert self._seam([0, 2], [self._no_flags()] * 2, [fx.str_type, fx.str_type]) == [
            False,
            False,
        ]

    def test_seam_literal_flags_suppress(self) -> None:
        fx = self.fx
        for flags in [
            (True, False, False, False, False),
            (False, True, False, False, False),
            (False, False, True, False, False),
            (False, False, False, True, False),
            (False, False, False, False, True),
        ]:
            assert self._seam([1], [flags], [fx.str_type]) == [False], flags

    def test_seam_type_object_not_narrowable(self) -> None:
        fx = self.fx
        assert self._seam([1], [self._no_flags()], [fx.callable_type(fx.a)]) == [False]

    def test_seam_plain_callable_narrowable(self) -> None:
        fx = self.fx
        assert self._seam([1], [self._no_flags()], [fx.callable(fx.a)]) == [True]

    def test_seam_overloaded_type_object_not_narrowable(self) -> None:
        fx = self.fx
        assert self._seam([1], [self._no_flags()], [Overloaded([fx.callable_type(fx.a)])]) == [
            False
        ]

    def test_seam_type_type_over_typevar_not_narrowable(self) -> None:
        assert self._seam([1], [self._no_flags()], [TypeType(self._tvar())]) == [False]

    def test_seam_type_type_over_instance_narrowable(self) -> None:
        fx = self.fx
        assert self._seam([1], [self._no_flags()], [TypeType(fx.str_type)]) == [True]

    def test_seam_alias_operand_defers(self) -> None:
        fx = self.fx
        alias_type = TypeAliasType(TypeAlias(Instance(fx.ai, []), "mod.AAlias", "mod", -1, -1), [])
        assert self._seam([1], [self._no_flags()], [alias_type]) is None

    # --- alias operands with a resolver snapshot decide natively (#1235) ---

    def _rebuild_resolver(self, aliases: list[Any]) -> None:
        from mypy.checker import _set_native_checker_resolver

        self._resolver = _type_kernel.build_native_resolver(self._infos, aliases)
        _set_native_checker_resolver(self._resolver)

    def test_seam_alias_operand_expands_to_instance(self) -> None:
        fx = self.fx
        alias = TypeAlias(Instance(fx.ai, []), "mod.CAlias", "mod", -1, -1)
        self._rebuild_resolver([alias])
        assert self._seam([1], [self._no_flags()], [TypeAliasType(alias, [])]) == [True]

    def test_seam_alias_operand_expands_to_type_object(self) -> None:
        fx = self.fx
        alias = TypeAlias(fx.callable_type(fx.a), "mod.DAlias", "mod", -1, -1)
        self._rebuild_resolver([alias])
        assert self._seam([1], [self._no_flags()], [TypeAliasType(alias, [])]) == [False]

    def test_seam_alias_ret_expands_to_uninhabited(self) -> None:
        fx = self.fx
        alias = TypeAlias(UninhabitedType(), "mod.NAlias", "mod", -1, -1)
        self._rebuild_resolver([alias])
        fn = fx.callable(fx.a)
        fn.ret_type = TypeAliasType(alias, [])
        assert self._seam([1], [self._no_flags()], [fn]) == [True]

    def test_seam_length_mismatch_defers(self) -> None:
        fx = self.fx
        assert self._seam([1, 1], [self._no_flags()], [fx.str_type]) is None
        assert self._seam([1], [self._no_flags()] * 2, [fx.str_type]) is None

    def test_seam_empty_operands(self) -> None:
        assert self._seam([], [], []) == []

    # --- gate-off vs gate-on differential through the real helper ---

    def test_parity_narrowable_chain(self) -> None:
        fx = self.fx
        e0, e1, e2 = self._var_expr("x0"), self._var_expr("x1"), self._var_expr("x2")
        node = self._comparison(["==", "<"], [e0, e1, e2])
        types = {e0: fx.str_type, e1: fx.o, e2: fx.a}
        self._assert_par(node, types, "narrowable chain")

    def test_parity_coalescing_chain(self) -> None:
        # same == x < y == same: the repeated operand groups the == chains.
        fx = self.fx
        e0, e1, e2 = self._var_expr("x0"), self._var_expr("x1"), self._var_expr("x2")
        node = self._comparison(["==", "<", "=="], [e0, e1, e2, e0])
        types = {e0: fx.str_type, e1: fx.o, e2: fx.a}
        self._assert_par(node, types, "coalescing chain")

    def test_parity_non_narrowable_literals(self) -> None:
        fx = self.fx
        from mypy.nodes import NameExpr

        e_true = NameExpr("True")
        e_true.fullname = "builtins.True"
        e_none = NameExpr("None")
        e_none.fullname = "builtins.None"
        e0 = self._var_expr("x0")
        node = self._comparison(["==", "==", "=="], [e_true, e0, e_none, e_true])
        types = {e_true: fx.o, e0: fx.str_type, e_none: fx.nonet}
        self._assert_par(node, types, "literal operands")

    def test_parity_type_object_operand(self) -> None:
        fx = self.fx
        e0, e_obj = self._var_expr("x0"), self._var_expr("C")
        node = self._comparison(["==", "=="], [e0, e_obj, e0])
        types = {e0: fx.str_type, e_obj: fx.callable_type(fx.a)}
        self._assert_par(node, types, "type-object operand")

    def test_parity_type_type_typevar_operand(self) -> None:
        fx = self.fx
        e0, e1 = self._var_expr("x0"), self._var_expr("x1")
        node = self._comparison(["==", "=="], [e0, e1, e0])
        types = {e0: fx.str_type, e1: TypeType(self._tvar())}
        self._assert_par(node, types, "TypeType over TypeVar operand")

    def test_parity_mixed_narrowable_and_not(self) -> None:
        fx = self.fx
        from mypy.nodes import NameExpr

        e0, e1, e_true = self._var_expr("x0"), self._var_expr("x1"), NameExpr("True")
        e_true.fullname = "builtins.True"
        node = self._comparison(["==", "<", "!="], [e0, e1, e_true, e0])
        types = {e0: fx.str_type, e1: fx.a, e_true: fx.o}
        self._assert_par(node, types, "mixed operands")

    def test_parity_alias_operand_defers(self) -> None:
        fx = self.fx
        e0, e1 = self._var_expr("x0"), self._var_expr("x1")
        node = self._comparison(["==", "=="], [e0, e1, e0])
        types = {
            e0: fx.str_type,
            e1: TypeAliasType(TypeAlias(Instance(fx.ai, []), "mod.BAlias", "mod", -1, -1), []),
        }
        self._assert_par(node, types, "alias operand")

    def test_parity_alias_operand_with_snapshot(self) -> None:
        # The alias is resolvable, so both gates decide narrowable from the
        # expanded Instance (#1235); parity of the recorded hash captures.
        fx = self.fx
        alias = TypeAlias(Instance(fx.ai, []), "mod.EAlias", "mod", -1, -1)
        self._rebuild_resolver([alias])
        e0, e1 = self._var_expr("x0"), self._var_expr("x1")
        node = self._comparison(["==", "=="], [e0, e1, e0])
        types = {e0: fx.str_type, e1: TypeAliasType(alias, [])}
        self._assert_par(node, types, "alias operand with snapshot")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeNarrowIdentityEqualitySuite(Suite):
    """Parity for the identity/equality narrowing seam (#387, #1126).

    `TypeChecker.narrow_type_by_identity_equality` (checker.py:8963) hands
    each non-custom-__eq__ operand pair to
    `_try_native_narrow_type_by_identity_equality`, which calls
    `rust_narrow_type_by_identity_equality` for `is` / `is not` / `==` /
    `!=`. Rust runs the caller's fallback pre-step (coerce_to_literal +
    try_expanding_sum_type_to_union) and the single-range
    `conditional_types(..., from_equality=True)` call through the
    cond_types::conditional_types_inner port, deferring (None) on anything
    it cannot decide (aliases, generic erasure, structural subtypes); the
    Python shim falls back to the pure-Python body then.

    Direct seam calls assert engagement per operator and the deferral
    shapes; the gate-off vs gate-on differential drives the real method on
    a TypeChecker stub (options only: the method needs no other checker
    state for NameExpr operands) and asserts identical typemaps.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_resolver, _set_native_checker_stmts_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._enum_info = self.fx.make_type_info("mod.Color")
        self._enum_info.is_enum = True
        # enum_members is computed from names: one Var member with an
        # explicit value makes this a single-member enum.
        from mypy.nodes import GDEF, SymbolTableNode, Var

        var = Var("RED", Instance(self._enum_info, []))
        var.has_explicit_value = True
        self._enum_info.names["RED"] = SymbolTableNode(GDEF, var)
        # _base_infos skips str_type_info (its attr name ends in "o");
        # include it so builtins.str snapshots resolve.
        self._infos = [self._enum_info, self.fx.str_type_info] + _base_infos(self.fx)
        self._resolver = _type_kernel.build_native_resolver(self._infos, [])
        # Enum/instance member reads go live through this map
        # (coerce_to_literal, expand_for_target).
        self._resolver.set_live_typeinfo_map({i.fullname: i for i in self._infos})
        _set_native_checker_resolver(self._resolver)
        self._set_active = _set_native_checker_stmts_active
        self._set_active(True)
        set_wire_typeinfo_map({i.fullname: i for i in self._infos})

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_resolver, _set_native_checker_stmts_active
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checker_stmts_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _operands(self) -> list[Any]:
        from mypy.nodes import NameExpr

        return [NameExpr("x"), NameExpr("y")]

    def _run(self, operator: str, operand_types: list[Type]) -> Any:
        from mypy.checker import TypeChecker

        operands = self._operands()

        def run_one() -> Any:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            return chk.narrow_type_by_identity_equality(
                operator, operands, operand_types, [0, 1], {0, 1}
            )

        off = self._with_gate(False, run_one)
        on = self._with_gate(True, run_one)
        return off, on

    def _seam(self, a: Type, b: Type, operator: str) -> Any:
        from mypy.checker import _serialize_type_for_checker

        return _type_kernel.rust_narrow_type_by_identity_equality(
            _serialize_type_for_checker(a),
            _serialize_type_for_checker(b),
            operator,
            True,
            self._resolver,
        )

    def _assert_par(
        self, operator: str, operand_types: list[Type], note: str, defers: bool = False
    ) -> None:
        off, on = self._run(operator, operand_types)
        assert on == off, f"narrow parity {operator} {note}: off={off!r} on={on!r}"
        # Engagement: the differential must not silently compare two
        # identical pure-Python runs. Alias/unsupported-operator cases
        # assert the deferral directly instead.
        result = self._seam(operand_types[0], operand_types[1], operator)
        if defers:
            assert result is None, f"seam expected defer ({operator} {note}): {result!r}"
        else:
            assert result is not None, f"seam did not engage ({operator} {note})"

    # --- direct seam calls ---

    def test_seam_all_operators_engage(self) -> None:
        fx = self.fx
        union = UnionType.make_union([fx.a, fx.nonet])
        for op in ("is", "is not", "==", "!="):
            assert self._seam(union, fx.a, op) is not None, op

    def test_seam_unsupported_operator_defers(self) -> None:
        fx = self.fx
        assert self._seam(fx.a, fx.a, "<") is None

    def test_seam_alias_defers(self) -> None:
        from mypy.nodes import TypeAlias

        fx = self.fx
        alias = TypeAliasType(TypeAlias(Instance(fx.ai, []), "mod.AAlias", "mod", -1, -1), [])
        for op in ("is", "=="):
            assert self._seam(alias, fx.a, op) is None, op

    def test_seam_single_member_enum_coerces(self) -> None:
        # The #1126 port: a single-member enum target coerces to
        # Literal["RED"] live (enum_members read via PyO3), so `x is y`
        # narrows x to the literal instead of the enum instance.
        enum = Instance(self._enum_info, [])
        for op in ("is", "is not", "==", "!="):
            assert self._seam(enum, enum, op) is not None, op

    # --- gate-off vs gate-on differentials ---

    def test_parity_equality_optional_against_base(self) -> None:
        fx = self.fx
        self._assert_par("==", [UnionType.make_union([fx.a, fx.nonet]), fx.a], "A | None == A")

    def test_parity_not_equality_optional_against_base(self) -> None:
        fx = self.fx
        self._assert_par("!=", [UnionType.make_union([fx.a, fx.nonet]), fx.a], "A | None != A")

    def test_parity_identity_optional_against_base(self) -> None:
        fx = self.fx
        union = UnionType.make_union([fx.a, fx.nonet])
        self._assert_par("is", [union, fx.a], "A | None is A")
        self._assert_par("is not", [union, fx.a], "A | None is not A")

    def test_parity_equality_wider_target(self) -> None:
        fx = self.fx
        self._assert_par("==", [fx.str_type, fx.o], "str == object")

    def test_parity_equality_none_target(self) -> None:
        fx = self.fx
        self._assert_par("==", [fx.a, fx.nonet], "A == None")

    def test_parity_identity_literal_target(self) -> None:
        fx = self.fx
        self._assert_par("is", [UnionType.make_union([fx.a, fx.nonet]), fx.lit1_inst], "literal")

    def test_parity_identity_single_member_enum(self) -> None:
        enum = Instance(self._enum_info, [])
        self._assert_par("is", [enum, enum], "Color is Color")

    def test_parity_equality_alias_defers(self) -> None:
        from mypy.nodes import TypeAlias

        fx = self.fx
        alias = TypeAliasType(TypeAlias(Instance(fx.ai, []), "mod.AAlias", "mod", -1, -1), [])
        self._assert_par("==", [alias, fx.a], "alias operand", defers=True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckAssignmentHeadSuite(Suite):
    """Direct-seam tests and production pins for `rust_classify_check_assignment` (#1090).

    `TypeChecker.check_assignment` (checker.py:4800) classifies a
    special-name front (NameExpr `__setattr__`/`__getattribute__`/
    `__getattr__` signature check, `__slots__` in a class body,
    `__match_args__` with an inferred Var, `__post_init__`, and the
    MemberExpr `__match_args__` fail) plus the `lvalue_type` branch
    (partial-None inference, member assignment when `kind is None`,
    check_simple_assignment tail, no-type fallthrough). Direct seam calls
    assert the exact tags via the registered pyfunction.

    The seam was retired by #1739: the call was removed from
    `check_assignment` (the inline isinstance chain at checker.py:4815-4837
    applies every arm), the alias and shim are gone, and only the
    `NATIVE_CA_*` tag constants survive. The host reads no gate, so a
    gate-off vs gate-on differential cannot fail on any shape here. The
    production tests below pin the captured observations of one run each;
    the retirement pin lives in
    `testtypes_native_retired_checker.py::NativeClassifyCheckAssignmentRetiredSuite`.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    # ---- direct seam tests ----

    def _name(self, name: str = "x", node: object | None = None) -> NameExpr:
        ne = NameExpr(name)
        ne.node = Var(name) if node is None else node  # type: ignore[assignment]
        return ne

    def _seam(
        self, lvalue: object, lvalue_type: Type | None, has_inferred: bool, active_class: bool
    ) -> tuple[int, int] | None:
        return _type_kernel.rust_classify_check_assignment(
            lvalue, lvalue_type, has_inferred, active_class
        )

    def test_seam_setattr_family(self) -> None:
        from mypy.checker import NATIVE_CA_BRANCH_NO_TYPE, NATIVE_CA_SPECIAL_SETATTR_SIG

        for name in ("__setattr__", "__getattribute__", "__getattr__"):
            tags = self._seam(self._name(name), None, False, False)
            assert tags == (
                NATIVE_CA_SPECIAL_SETATTR_SIG,
                NATIVE_CA_BRANCH_NO_TYPE,
            ), f"{name}: {tags}"

    def test_seam_slots(self) -> None:
        from mypy.checker import (
            NATIVE_CA_BRANCH_NO_TYPE,
            NATIVE_CA_SPECIAL_NONE,
            NATIVE_CA_SPECIAL_SLOTS,
        )

        tags = self._seam(self._name("__slots__"), None, False, True)
        assert tags == (NATIVE_CA_SPECIAL_SLOTS, NATIVE_CA_BRANCH_NO_TYPE), f"{tags}"
        # Without an active class no special check fires.
        tags = self._seam(self._name("__slots__"), None, False, False)
        assert tags == (NATIVE_CA_SPECIAL_NONE, NATIVE_CA_BRANCH_NO_TYPE), f"{tags}"

    def test_seam_match_args(self) -> None:
        from mypy.checker import (
            NATIVE_CA_BRANCH_NO_TYPE,
            NATIVE_CA_SPECIAL_MATCH_ARGS,
            NATIVE_CA_SPECIAL_NONE,
        )

        tags = self._seam(self._name("__match_args__"), None, True, False)
        assert tags == (NATIVE_CA_SPECIAL_MATCH_ARGS, NATIVE_CA_BRANCH_NO_TYPE), f"{tags}"
        # Without an inferred Var no special check fires.
        tags = self._seam(self._name("__match_args__"), None, False, False)
        assert tags == (NATIVE_CA_SPECIAL_NONE, NATIVE_CA_BRANCH_NO_TYPE), f"{tags}"

    def test_seam_post_init(self) -> None:
        from mypy.checker import NATIVE_CA_BRANCH_NO_TYPE, NATIVE_CA_SPECIAL_POST_INIT

        tags = self._seam(self._name("__post_init__"), None, False, False)
        assert tags == (NATIVE_CA_SPECIAL_POST_INIT, NATIVE_CA_BRANCH_NO_TYPE), f"{tags}"

    def test_seam_plain_name(self) -> None:
        from mypy.checker import NATIVE_CA_BRANCH_NO_TYPE, NATIVE_CA_SPECIAL_NONE

        tags = self._seam(self._name("x"), None, False, False)
        assert tags == (NATIVE_CA_SPECIAL_NONE, NATIVE_CA_BRANCH_NO_TYPE), f"{tags}"
        # A NameExpr with a falsy node never enters the special block.
        ne = self._name("x", node=None)
        tags = self._seam(ne, None, False, False)
        assert tags == (NATIVE_CA_SPECIAL_NONE, NATIVE_CA_BRANCH_NO_TYPE), f"{tags}"

    def test_seam_member_match_args(self) -> None:
        from mypy.checker import NATIVE_CA_BRANCH_NO_TYPE, NATIVE_CA_SPECIAL_MEMBER_MATCH_ARGS

        me = MemberExpr(NameExpr("b"), "__match_args__")
        tags = self._seam(me, None, False, False)
        assert tags == (NATIVE_CA_SPECIAL_MEMBER_MATCH_ARGS, NATIVE_CA_BRANCH_NO_TYPE), f"{tags}"

    def test_seam_branches(self) -> None:
        from mypy.checker import (
            NATIVE_CA_BRANCH_MEMBER,
            NATIVE_CA_BRANCH_NO_TYPE,
            NATIVE_CA_BRANCH_PARTIAL_NONE,
            NATIVE_CA_BRANCH_SIMPLE,
            NATIVE_CA_SPECIAL_NONE,
        )
        from mypy.nodes import GDEF

        # No lvalue type -> NO_TYPE.
        tags = self._seam(self._name("x"), None, False, False)
        assert tags == (NATIVE_CA_SPECIAL_NONE, NATIVE_CA_BRANCH_NO_TYPE), f"{tags}"
        # Partial-None lvalue type -> PARTIAL_NONE.
        pt = PartialType(None, Var("x"), None)
        tags = self._seam(self._name("x"), pt, False, False)
        assert tags == (NATIVE_CA_SPECIAL_NONE, NATIVE_CA_BRANCH_PARTIAL_NONE), f"{tags}"
        # Partial with a non-None class is not the partial-None arm.
        pt2 = PartialType(self.fx.std_tuplei, Var("x"), None)
        tags = self._seam(self._name("x"), pt2, False, False)
        assert tags == (NATIVE_CA_SPECIAL_NONE, NATIVE_CA_BRANCH_SIMPLE), f"{tags}"
        # Member with kind None -> MEMBER.
        me = MemberExpr(NameExpr("b"), "attr")
        tags = self._seam(me, Instance(self.fx.ai, []), False, False)
        assert tags == (NATIVE_CA_SPECIAL_NONE, NATIVE_CA_BRANCH_MEMBER), f"{tags}"
        # Member with a resolved kind -> SIMPLE.
        me2 = MemberExpr(NameExpr("b"), "attr")
        me2.kind = GDEF
        tags = self._seam(me2, Instance(self.fx.ai, []), False, False)
        assert tags == (NATIVE_CA_SPECIAL_NONE, NATIVE_CA_BRANCH_SIMPLE), f"{tags}"

    def test_seam_defers_on_unreadable_node_name(self) -> None:
        # A NameExpr whose node lacks `.name` defers (None) so the
        # pure-Python body re-runs.
        ne = self._name("x", node=SimpleNamespace())
        assert self._seam(ne, None, False, False) is None

    # ---- production pins (the seam is retired; see the docstring) ----

    def _run(
        self,
        lvalue: Lvalue,
        lvalue_type: Type | None,
        index_lvalue: Lvalue | None,
        inferred: Var | None,
        *,
        rvalue_type: Type | None = None,
        active_class: object | None = None,
        is_stub: bool = False,
    ) -> tuple[object, ...]:
        """One `check_assignment` run with the gate on, as production runs it."""
        from mypy.checker import TypeChecker

        rvalue: Expression = StrExpr("v")
        rt = rvalue_type if rvalue_type is not None else Instance(self.fx.ai, [])

        obs: list[object] = []
        # The mock surface below intentionally diverges from the real
        # TypeChecker signatures, so the instance is typed as Any.
        chk = cast(Any, TypeChecker.__new__(TypeChecker))
        chk.options = Options()
        chk.is_stub = is_stub
        chk.current_node_deferred = False
        chk.can_skip_diagnostics = True
        chk.var_decl_frames = {}
        chk._expr_checker = SimpleNamespace(
            accept=lambda expr, type_context=None, always_allow_any=False: rt
        )
        chk.scope = SimpleNamespace(active_class=lambda: active_class)
        chk.binder = SimpleNamespace(
            assign_type=lambda lv, rvt, lvt: obs.append(("assign",)),
            frames=[],
            put=lambda lv, t: obs.append(("put",)),
        )
        chk.msg = SimpleNamespace(
            concrete_only_assign=lambda lt, rv: obs.append(("concrete_only",))
        )
        chk.try_infer_partial_generic_type_from_assignment = lambda lv, rv, op: obs.append(
            ("partial_generic",)
        )
        chk.check_lvalue = lambda lv, rv=None: (lvalue_type, index_lvalue, inferred)
        chk.fail = lambda msg, ctx: obs.append(("fail", str(msg)))
        chk.check_setattr_method = lambda sig, lv: obs.append(("setattr",))
        chk.check_getattr_method = lambda sig, lv, name: obs.append(("getattr", name))
        chk.check_slots_definition = lambda typ, lv: obs.append(("slots_def",))
        chk.check_match_args = lambda typ, t, lv: obs.append(("match_args",))

        def _mock_member_assignment(
            lv: Any, it: Any, lt: Any, rv: Any, context: Any = None
        ) -> tuple[Any, Any, bool]:
            obs.append(("member",))
            return rt, lt, True

        def _mock_simple_assignment(
            lt: Any, rv: Any, context: Any = None, inferred: Any = None, lvalue: Any = None
        ) -> tuple[Any, Any]:
            obs.append(("simple",))
            return rt, lt

        chk.check_member_assignment = _mock_member_assignment
        chk.check_simple_assignment = _mock_simple_assignment
        chk.check_indexed_assignment = lambda il, rv, lv: obs.append(("indexed",))
        chk.get_variable_type_context = lambda inf, rv: None
        chk.infer_variable_type = lambda inf, lv, rvt, rv: obs.append(("infer_var",))
        chk.check_assignment_to_slots = lambda lv: obs.append(("slots",))
        chk.find_partial_types = lambda var: {var: None}
        chk.set_inferred_type = lambda var, lv, t: obs.append(("set_type", str(t)))
        chk.infer_partial_type = lambda var, lv, rvt: False
        chk.inference_error_fallback_type = lambda rvt: rvt
        chk.check_assignment_to_multiple_lvalues = lambda items, rv, ctx, ilt: obs.append(
            ("multiple",)
        )
        chk.check_assignment(lvalue, rvalue)
        return tuple(obs)

    def _assert_obs(
        self,
        lvalue: Lvalue,
        lvalue_type: Type | None,
        index_lvalue: Lvalue | None,
        inferred: Var | None,
        expected: tuple[object, ...],
        *,
        rvalue_type: Type | None = None,
        active_class: object | None = None,
    ) -> None:
        obs = self._run(
            lvalue,
            lvalue_type,
            index_lvalue,
            inferred,
            rvalue_type=rvalue_type,
            active_class=active_class,
        )
        assert obs == expected, f"check_assignment observations for lvalue={lvalue!r}: {obs}"

    def test_value_name_simple(self) -> None:
        self._assert_obs(
            self._name("x"),
            Instance(self.fx.ai, []),
            None,
            None,
            (("partial_generic",), ("simple",), ("assign",), ("slots",)),
        )

    def test_value_name_no_type(self) -> None:
        self._assert_obs(self._name("x"), None, None, None, (("partial_generic",), ("slots",)))

    def test_value_member(self) -> None:
        self._assert_obs(
            MemberExpr(NameExpr("b"), "attr"),
            Instance(self.fx.ai, []),
            None,
            None,
            (("partial_generic",), ("member",), ("assign",), ("slots",)),
        )

    def test_value_index(self) -> None:
        self._assert_obs(
            IndexExpr(NameExpr("a"), NameExpr("b")),
            None,
            IndexExpr(NameExpr("a"), NameExpr("c")),
            None,
            (("partial_generic",), ("indexed",), ("slots",)),
        )

    def test_value_inferred_tail(self) -> None:
        self._assert_obs(
            self._name("x"),
            None,
            None,
            Var("x"),
            (("partial_generic",), ("infer_var",), ("slots",)),
        )

    def test_value_setattr(self) -> None:
        self._assert_obs(
            self._name("__setattr__"),
            Instance(self.fx.ai, []),
            None,
            None,
            (("partial_generic",), ("setattr",), ("simple",), ("assign",), ("slots",)),
        )

    def test_value_getattr_family(self) -> None:
        self._assert_obs(
            self._name("__getattribute__"),
            None,
            None,
            None,
            (("partial_generic",), ("getattr", "__getattribute__"), ("slots",)),
        )

    def test_value_slots(self) -> None:
        self._assert_obs(
            self._name("__slots__"),
            None,
            None,
            None,
            (("partial_generic",), ("slots_def",), ("slots",)),
            active_class=SimpleNamespace(metadata={}),
        )

    def test_value_match_args_name(self) -> None:
        self._assert_obs(
            self._name("__match_args__"),
            None,
            None,
            Var("__match_args__"),
            (("partial_generic",), ("match_args",), ("infer_var",), ("slots",)),
        )

    def test_value_post_init(self) -> None:
        self._assert_obs(
            self._name("__post_init__"),
            None,
            None,
            None,
            (
                ("partial_generic",),
                ("fail", '"__post_init__" method must be an instance method'),
                ("slots",),
            ),
            active_class=SimpleNamespace(metadata={"dataclass"}),
        )

    def test_value_member_match_args(self) -> None:
        self._assert_obs(
            MemberExpr(NameExpr("b"), "__match_args__"),
            Instance(self.fx.ai, []),
            None,
            None,
            (
                ("partial_generic",),
                ("fail", 'Cannot assign to "__match_args__"'),
                ("member",),
                ("assign",),
                ("slots",),
            ),
        )

    def test_value_partial_none_return(self) -> None:
        self._assert_obs(
            self._name("x"),
            PartialType(None, Var("x"), None),
            None,
            None,
            (("partial_generic",),),
            rvalue_type=NoneType(),
        )

    def test_value_partial_none_infer(self) -> None:
        self._assert_obs(
            self._name("x"),
            PartialType(None, Var("x"), None),
            None,
            None,
            (("partial_generic",), ("set_type", "A | None"), ("assign",), ("slots",)),
        )

    def test_value_tuple(self) -> None:
        self._assert_obs(TupleExpr([NameExpr("a")]), None, None, None, (("multiple",),))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeConditionalStructuralFalseSuite(Suite):
    """Wave 46 (#1450): the structural-subtype fall-through in the Rust
    `conditional_types` port.

    Python's conditional_types uses `is_subtype(current, proposed)` in the
    structural branch only as an `if` gate: on False it falls through to the
    shared narrowing tail (equality erasure, overlap, restrict_subtype_away,
    avoid-widening). The seam used to defer the whole call on any answer
    except Some(true); a decided Some(false) now falls through exactly like
    Python, and only an undecided check defers. The protocol below carries a
    member `f` that class A lacks, so is_subtype(A, ProtoF) is a decided
    False. Toggling the checker gate off (pure Python) and on (Rust seam)
    must produce identical results; a direct seam call proves the seam
    engages instead of deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        # The builtins the fixture skips (name does not end in "i") and the
        # function TypeInfo fallback for CallableType shapes.
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        # A protocol carrying a member f that mod.A (fx.ai) lacks: a
        # non-conforming protocol target for the structural branch.
        self.proto_info = self.fx.make_type_info("mod.ProtoF", mro=[self.fx.oi])
        self.proto_info.is_protocol = True
        pinst = Instance(self.proto_info, [])
        node = FuncDef("f", [], None, None)
        node.info = self.proto_info
        node.type = CallableType([pinst], [ARG_POS], [None], self.fx.a, self.fx.function)
        node.line = 1
        node.column = 1
        self.proto_info.names["f"] = SymbolTableNode(MDEF, node)
        type_infos.append(self.proto_info)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._live_map = {info.fullname: info for info in type_infos}
        self.resolver.set_live_typeinfo_map(dict(self._live_map))
        _set_native_checker_active(True)
        _set_native_checker_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def test_structural_false_protocol_target_engages(self) -> None:
        # A does not implement ProtoF (it has no member f): the structural
        # is_subtype is a decided False, so the seam falls through to the
        # shared narrowing tail and returns a decision instead of None.
        from mypy.checker import (
            _serialize_type_for_checker,
            _serialize_type_ranges,
            conditional_types,
        )

        current = self.fx.a
        ranges = [TypeRange(Instance(self.proto_info, []), False)]
        off = self._with_gate(False, lambda: conditional_types(current, ranges, None))
        on = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal(
            (str(off[0]), str(off[1])),
            (str(on[0]), str(on[1])),
            f"structural-False parity {current} vs ProtoF",
        )
        result = _type_kernel.rust_conditional_types(
            _serialize_type_for_checker(current),
            _serialize_type_ranges(ranges),
            None,
            True,
            False,
            state.strict_optional,
            self.resolver,
        )
        assert (
            result is not None
        ), "structural-False conditional_types must engage (pre-wave-46 deferral)"
        assert (str(off[0]), str(off[1])) == (str(on[0]), str(on[1]))

    def test_structural_false_callable_current_parity(self) -> None:
        # proposed is a CallableType; current is a CallableType that is
        # neither a proper nor a structural subtype of it. Parity must hold
        # through the fall-through tail.
        from mypy.checker import conditional_types
        from mypy.types import CallableType

        current = CallableType([self.fx.str_type], [ARG_POS], [None], self.fx.a, self.fx.function)
        proposed = CallableType([self.fx.str_type], [ARG_POS], [None], self.fx.b, self.fx.function)
        ranges = [TypeRange(proposed, False)]
        off = self._with_gate(False, lambda: conditional_types(current, ranges, None))
        on = self._with_gate(True, lambda: conditional_types(current, ranges, None))
        assert_equal(
            (str(off[0]), str(off[1])),
            (str(on[0]), str(on[1])),
            f"structural-False callable parity {current} vs {proposed}",
        )


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeRangeSuite(Suite):
    """Gate-off vs gate-on parity for get_type_range_of_type (issue #1464 C1).

    Runs TypeChecker.get_type_range_of_type on a stub checker (named_type
    overridden so the REST is_subtype gate resolves) and asserts identical
    (item, is_upper_bound) results with the native classifier off and on,
    plus direct seam calls on the live proper types.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], Any]) -> Any:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _range(self, typ: Type) -> tuple[str | None, bool | None]:
        from mypy.checker import TypeChecker
        from mypy.options import Options

        chk = TypeChecker.__new__(TypeChecker)
        chk.options = Options()
        chk.named_type = lambda name: self.fx.type_type  # type: ignore[method-assign]
        tr = chk.get_type_range_of_type(typ)
        return (
            None if tr is None else str(tr.item),
            tr.is_upper_bound if tr is not None else None,
        )

    def _par(self, typ: Type) -> None:
        off = self._with_gate(False, lambda: self._range(typ))
        on = self._with_gate(True, lambda: self._range(typ))
        assert off == on, f"get_type_range_of_type parity for typ={typ!r}: {off} != {on}"

    def _seam(self, typ: Type) -> tuple[int, bool]:
        result = _type_kernel.rust_classify_type_range(get_proper_type(typ))
        assert result is not None
        return result

    def _union_instance(self, with_args: bool) -> Instance:
        info = self.fx.make_type_info("types.UnionType")
        assert info is not None
        args = [self.fx.a, self.fx.b] if with_args else []
        return Instance(info, args)

    def _special_form(self) -> Instance:
        info = self.fx.make_type_info("typing._SpecialForm")
        assert info is not None
        return Instance(info, [])

    def _final_instance(self) -> Instance:
        info = self.fx.make_type_info("FinalKlass")
        assert info is not None
        info.is_final = True
        return Instance(info, [])

    # --- direct seam: the eight leaf branch tags ---

    def test_seam_fn_typeobj(self) -> None:
        assert self._seam(self.fx.callable_type(self.fx.a, self.fx.a)) == (1, False)

    def test_seam_fn_rest(self) -> None:
        assert self._seam(self.fx.callable(self.fx.a, self.fx.o)) == (7, False)

    def test_seam_typetype_upper(self) -> None:
        assert self._seam(TypeType.make_normalized(self.fx.a)) == (2, True)

    def test_seam_typetype_none(self) -> None:
        assert self._seam(TypeType.make_normalized(self.fx.nonet)) == (2, False)

    def test_seam_typetype_final(self) -> None:
        assert self._seam(TypeType.make_normalized(self._final_instance())) == (2, False)

    def test_seam_any(self) -> None:
        assert self._seam(self.fx.anyt) == (3, False)

    def test_seam_builtins_type(self) -> None:
        assert self._seam(self.fx.type_type) == (4, False)

    def test_seam_types_union(self) -> None:
        assert self._seam(self._union_instance(True)) == (5, False)

    def test_seam_types_union_no_args_is_rest(self) -> None:
        assert self._seam(self._union_instance(False)) == (7, False)

    def test_seam_special_form(self) -> None:
        assert self._seam(self._special_form()) == (6, False)

    def test_seam_other_instance_is_rest(self) -> None:
        assert self._seam(self.fx.a) == (7, False)

    def test_seam_none_type_is_rest(self) -> None:
        assert self._seam(self.fx.nonet) == (7, False)

    def test_seam_defers_on_unreadable_instance_fullname(self) -> None:
        # A live Instance whose TypeInfo.fullname read raises AttributeError:
        # the seam must defer (None) instead of propagating, so the shim
        # re-runs the pure-Python body (issue #1466 pin).
        broken = Instance(_BrokenAttrInfo("mod.Broken", "fullname"), [])
        result = _type_kernel.rust_classify_type_range(broken)
        assert result is None

    def test_par_unreadable_instance_fullname(self) -> None:
        # Both gates raise the identical AttributeError: gate-off is the pure
        # body, gate-on defers the unreadable attribute to it (issue #1466).
        broken = Instance(_BrokenAttrInfo("mod.Broken", "fullname"), [])

        def run() -> BaseException | None:
            try:
                self._range(broken)
                return None
            except Exception as err:
                return err

        off = self._with_gate(False, run)
        on = run()
        assert isinstance(off, AttributeError), f"gate-off expected AttributeError: {off!r}"
        assert isinstance(on, AttributeError), f"gate-on expected AttributeError: {on!r}"
        assert str(off) == str(on), f"raise messages differ: {off!r} != {on!r}"

    def test_seam_repropagates_non_attribute_error(self) -> None:
        # A read that raises RuntimeError must NOT be swallowed: the seam
        # re-propagates it so genuine kernel bugs stay visible (issue #1466
        # error-class boundary pin).
        import pytest

        broken = Instance(_BrokenAttrInfo("mod.Broken", "fullname", RuntimeError), [])
        with pytest.raises(RuntimeError):
            _type_kernel.rust_classify_type_range(broken)

    def test_par_repropagates_non_attribute_error(self) -> None:
        # Both gates raise the identical RuntimeError: gate-off is the pure
        # body, gate-on re-propagates it from the seam (issue #1466).
        broken = Instance(_BrokenAttrInfo("mod.Broken", "fullname", RuntimeError), [])

        def run() -> BaseException | None:
            try:
                self._range(broken)
                return None
            except Exception as err:
                return err

        off = self._with_gate(False, run)
        on = run()
        assert isinstance(off, RuntimeError), f"gate-off expected RuntimeError: {off!r}"
        assert isinstance(on, RuntimeError), f"gate-on expected RuntimeError: {on!r}"
        assert str(off) == str(on), f"raise messages differ: {off!r} != {on!r}"

    # --- gate-off vs gate-on differentials on the captured range ---

    def test_par_fn_typeobj(self) -> None:
        self._par(self.fx.callable_type(self.fx.a, self.fx.a))

    def test_par_fn_rest(self) -> None:
        self._par(self.fx.callable(self.fx.a, self.fx.o))

    def test_par_typetype_upper(self) -> None:
        self._par(TypeType.make_normalized(self.fx.a))

    def test_par_typetype_none(self) -> None:
        self._par(TypeType.make_normalized(self.fx.nonet))

    def test_par_typetype_final(self) -> None:
        self._par(TypeType.make_normalized(self._final_instance()))

    def test_par_any(self) -> None:
        self._par(self.fx.anyt)

    def test_par_builtins_type(self) -> None:
        self._par(self.fx.type_type)

    def test_par_types_union(self) -> None:
        self._par(self._union_instance(True))

    def test_par_special_form(self) -> None:
        self._par(self._special_form())

    def test_par_other_instance(self) -> None:
        self._par(self.fx.a)

    def test_par_none_type(self) -> None:
        self._par(self.fx.nonet)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeNarrowWithLenAliasSuite(Suite):
    """Wave-61B: alias expansion for `narrow_with_len` (#1512).

    The wave-61 audit pinned 7 of 11 cold-self-check fallbacks to the
    entry `get_proper_type` on a `TypeAliasType` and 4 more to a union
    item the `can_be_narrowed_with_len` recursion could not expand. Both
    helpers now expand through the alias snapshot, mirroring Python's
    `get_proper_type`, and `can_be_narrowed_with_len` expands the type
    before the `custom_special_method` check (Python expands inside it).
    """

    def setUp(self) -> None:
        from mypy.checker import (
            _set_native_checker_active,
            _set_native_checker_resolver,
            _set_native_checker_types_active,
        )
        from mypy.nodes import TypeAlias
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._type_infos = _base_infos(self.fx)
        self.tup = TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)
        self.alias = TypeAlias(self.tup, "mod.TupAlias", "mod", -1, -1)
        self._resolver = _type_kernel.build_native_resolver(self._type_infos, [self.alias])
        _set_native_checker_active(True)
        _set_native_checker_types_active(True)
        _set_native_checker_resolver(self._resolver)
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})

    def tearDown(self) -> None:
        from mypy.checker import (
            _set_native_checker_active,
            _set_native_checker_resolver,
            _set_native_checker_types_active,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checker_active(False)
        _set_native_checker_types_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def _seam(self, typ: Type, op: str, size: int, resolver: Any | None = None) -> Any:
        from mypy.checker import _serialize_type_for_checker

        return _type_kernel.rust_narrow_with_len(
            _serialize_type_for_checker(typ),
            op,
            size,
            True,
            False,
            resolver if resolver is not None else self._resolver,
        )

    def test_seam_alias_engages(self) -> None:
        from mypy.checker import _deserialize_type_from_checker

        alias_t = TypeAliasType(self.alias, [])
        result = self._seam(alias_t, "==", 2)
        assert result is not None
        yes_bytes, no_bytes = result
        yes = _deserialize_type_from_checker(bytes(yes_bytes))
        no = _deserialize_type_from_checker(bytes(no_bytes))
        assert str(yes) == str(self.tup)
        assert isinstance(get_proper_type(no), UninhabitedType)

    def test_seam_defers_without_alias_snapshot(self) -> None:
        empty = _type_kernel.build_native_resolver([], [])
        assert self._seam(TypeAliasType(self.alias, []), "==", 2, empty) is None

    def test_seam_union_alias_item_engages(self) -> None:
        from mypy.checker import _deserialize_type_from_checker

        u = UnionType([TypeAliasType(self.alias, []), self.fx.a])
        result = self._seam(u, "==", 2)
        assert result is not None, "union item alias did not expand"
        yes_bytes, no_bytes = result
        assert _deserialize_type_from_checker(bytes(yes_bytes)) is not None
        assert _deserialize_type_from_checker(bytes(no_bytes)) is not None

    def test_can_be_narrowed_alias_engages(self) -> None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_can_be_narrowed_with_len(
            _serialize_type_for_checker(TypeAliasType(self.alias, [])), self._resolver
        )
        assert result is True

    def test_gate_parity_alias(self) -> None:
        chk = TypeChecker.__new__(TypeChecker)
        chk.options = Options()
        alias_t = TypeAliasType(self.alias, [])

        def run() -> tuple[Type, Type]:
            return chk.narrow_with_len(alias_t, "==", 2)

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert str(on[0]) == str(off[0])
        assert str(on[1]) == str(off[1])
        assert str(off[0]) == str(self.tup)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsValidDefaultDictPartialValueTypeSuite(Suite):
    """Parity for the Rust `is_valid_defaultdict_partial_value_type` port
    (H1g).

    `TypeChecker.is_valid_defaultdict_partial_value_type`
    (checker.py:6295-6318) checks whether a proper type can be used as the
    basis for a partial defaultdict value type. The Rust port
    (`checker_functions.rs`) decodes the wire type and returns the bool;
    the `old_type_inference` flag is passed from Python.

    Direct seam calls assert the bool and the None deferrals; the
    gate-off vs gate-on differential drives the real TypeChecker method
    through a stub checker with mock options.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _seam(self, t: Type, old_type_inference: bool = False) -> Any:
        from mypy.checker import _serialize_type_for_checker

        return _type_kernel.rust_is_valid_defaultdict_partial_value_type(
            _serialize_type_for_checker(t), old_type_inference
        )

    def _run(self, t: ProperType, old_type_inference: bool = False) -> tuple[bool, bool]:
        from unittest.mock import Mock

        from mypy.checker import TypeChecker

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Mock()
            chk.options.old_type_inference = old_type_inference
            return chk.is_valid_defaultdict_partial_value_type(t)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, t: ProperType, old_type_inference: bool = False) -> None:
        off, on = self._run(t, old_type_inference)
        assert_equal(on, off, f"parity for t={t!r} old={old_type_inference}")

    # --- direct seam tests ---

    def test_seam_no_args(self) -> None:
        assert self._seam(self.fx.o) is True

    def test_seam_one_arg_uninhabited(self) -> None:
        t = Instance(self.fx.gi, [self.fx.uninhabited])
        assert self._seam(t) is True

    def test_seam_one_arg_none(self) -> None:
        t = Instance(self.fx.gi, [self.fx.nonet])
        assert self._seam(t) is True

    def test_seam_one_arg_typevar_old(self) -> None:
        t = Instance(self.fx.gi, [self.fx.t])
        assert self._seam(t, old_type_inference=True) is True

    def test_seam_one_arg_typevar_new(self) -> None:
        t = Instance(self.fx.gi, [self.fx.t])
        assert self._seam(t, old_type_inference=False) is False

    def test_seam_one_arg_instance(self) -> None:
        t = Instance(self.fx.gi, [self.fx.a])
        assert self._seam(t) is False

    def test_seam_two_args(self) -> None:
        t = Instance(self.fx.hi, [self.fx.a, self.fx.a])
        assert self._seam(t) is False

    def test_seam_non_instance(self) -> None:
        assert self._seam(self.fx.nonet) is False

    def test_seam_type_alias_defers(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.str_type, "mod.A", "mod", -1, -1)
        t = TypeAliasType(alias, [])
        assert self._seam(t) is None

    # --- gate-off vs gate-on parity ---

    def test_parity_no_args(self) -> None:
        self._assert_par(self.fx.o)

    def test_parity_one_arg_uninhabited(self) -> None:
        t = Instance(self.fx.gi, [self.fx.uninhabited])
        self._assert_par(t)

    def test_parity_one_arg_none(self) -> None:
        t = Instance(self.fx.gi, [self.fx.nonet])
        self._assert_par(t)

    def test_parity_one_arg_typevar_old(self) -> None:
        t = Instance(self.fx.gi, [self.fx.t])
        self._assert_par(t, old_type_inference=True)

    def test_parity_one_arg_typevar_new(self) -> None:
        t = Instance(self.fx.gi, [self.fx.t])
        self._assert_par(t, old_type_inference=False)

    def test_parity_one_arg_instance(self) -> None:
        t = Instance(self.fx.gi, [self.fx.a])
        self._assert_par(t)

    def test_parity_two_args(self) -> None:
        t = Instance(self.fx.hi, [self.fx.a, self.fx.a])
        self._assert_par(t)

    def test_parity_non_instance(self) -> None:
        self._assert_par(self.fx.nonet)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsLenOfTupleSuite(Suite):
    """Parity for the Rust `is_len_of_tuple` AST-shape front (H1h).

    `TypeChecker.is_len_of_tuple` (checker.py:9497-9510) checks whether an
    expression is a `len(x)` call where x is a tuple or union of tuples.
    The Rust seam handles the early-return front (CallExpr, builtins.len,
    arg count == 1) and defers (None) to let Python run `literal()`,
    `has_type()`, and `can_be_narrowed_with_len()`. Direct seam calls
    assert the expected bool/None; gate-off vs gate-on differentials drive
    the real TypeChecker method through a minimal stub.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _len_call(self, arg: Expression | None = None) -> CallExpr:
        callee = NameExpr("len")
        callee.fullname = "builtins.len"
        if arg is None:
            arg = NameExpr("x")
        return CallExpr(callee, [arg], [ARG_POS], [None])

    def _seam(self, expr: Any) -> Any:
        return _type_kernel.rust_is_len_of_tuple(expr)

    def test_seam_not_call_expr(self) -> None:
        assert self._seam(NameExpr("x")) is False

    def test_seam_callee_not_refexpr(self) -> None:
        c = CallExpr(IntExpr(1), [NameExpr("x")], [ARG_POS], [None])
        assert self._seam(c) is False

    def test_seam_callee_wrong_fullname(self) -> None:
        callee = NameExpr("foo")
        callee.fullname = "builtins.foo"
        c = CallExpr(callee, [NameExpr("x")], [ARG_POS], [None])
        assert self._seam(c) is False

    def test_seam_wrong_arg_count(self) -> None:
        callee = NameExpr("len")
        callee.fullname = "builtins.len"
        c = CallExpr(callee, [NameExpr("x"), NameExpr("y")], [ARG_POS, ARG_POS], [None, None])
        assert self._seam(c) is False

    def test_seam_valid_len_call_defers(self) -> None:
        c = self._len_call()
        assert self._seam(c) is None

    def test_seam_zero_args(self) -> None:
        callee = NameExpr("len")
        callee.fullname = "builtins.len"
        c = CallExpr(callee, [], [], [])
        assert self._seam(c) is False

    def test_parity_not_call_expr(self) -> None:
        from mypy.checker import TypeChecker

        def check() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_len_of_tuple(NameExpr("x"))

        off = self._with_gate(False, check)
        on = self._with_gate(True, check)
        assert_equal(on, off, "is_len_of_tuple parity: not CallExpr")

    def test_parity_wrong_fullname(self) -> None:
        from mypy.checker import TypeChecker

        callee = NameExpr("foo")
        callee.fullname = "builtins.foo"
        c = CallExpr(callee, [NameExpr("x")], [ARG_POS], [None])

        def check() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_len_of_tuple(c)

        off = self._with_gate(False, check)
        on = self._with_gate(True, check)
        assert_equal(on, off, "is_len_of_tuple parity: wrong fullname")

    def test_parity_wrong_arg_count(self) -> None:
        from mypy.checker import TypeChecker

        callee = NameExpr("len")
        callee.fullname = "builtins.len"
        c = CallExpr(callee, [NameExpr("x"), NameExpr("y")], [ARG_POS, ARG_POS], [None, None])

        def check() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_len_of_tuple(c)

        off = self._with_gate(False, check)
        on = self._with_gate(True, check)
        assert_equal(on, off, "is_len_of_tuple parity: wrong arg count")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsAssignableSlotSuite(Suite):
    """Parity for the Rust `is_assignable_slot` port (H1i).

    `TypeChecker.is_assignable_slot` (checker.py:5534-5552) checks whether
    a lvalue is assignable as a slot (e.g. property). The Rust port
    (`checker_functions.rs`) reads the live lvalue `node` attr and the
    live proper type via PyO3, handling non-Union cases natively and
    deferring (`None`) on UnionType (Python recurses).

    Direct seam calls assert the bool and the None deferrals; the
    gate-off vs gate-on differential drives the real TypeChecker method
    through a stub checker.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _seam(self, lvalue: Any, typ: Any) -> Any:
        return _type_kernel.rust_is_assignable_slot(lvalue, typ)

    def _run(self, lvalue: Any, typ: Any) -> tuple[bool, bool]:
        from mypy.checker import TypeChecker

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_assignable_slot(lvalue, typ)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, lvalue: Any, typ: Any) -> None:
        off, on = self._run(lvalue, typ)
        assert_equal(on, off, f"parity for lvalue={lvalue!r} typ={typ!r}")

    def _make_lvalue(self, has_node: bool = False) -> Any:
        from mypy.nodes import NameExpr

        expr = NameExpr("x")
        if has_node:
            from mypy.nodes import Var

            expr.node = Var("x")
        return expr

    # --- direct seam tests ---

    def test_seam_definition(self) -> None:
        lv = self._make_lvalue(has_node=True)
        assert self._seam(lv, self.fx.a) is False

    def test_seam_none_type(self) -> None:
        lv = self._make_lvalue()
        assert self._seam(lv, None) is True

    def test_seam_any_type(self) -> None:
        lv = self._make_lvalue()
        assert self._seam(lv, AnyType(TypeOfAny.special_form)) is True

    def test_seam_instance_with_set(self) -> None:
        lv = self._make_lvalue()
        typ = Instance(self.fx.gi, [])
        assert self._seam(lv, typ) is False

    def test_seam_function_like(self) -> None:
        lv = self._make_lvalue()
        ct = self.fx.callable(self.fx.a, self.fx.a)
        assert self._seam(lv, ct) is True

    def test_seam_union_defers(self) -> None:
        lv = self._make_lvalue()
        u = UnionType([self.fx.a, self.fx.nonet])
        assert self._seam(lv, u) is None

    def test_seam_other_type_false(self) -> None:
        lv = self._make_lvalue()
        assert self._seam(lv, self.fx.nonet) is False

    # --- gate-off vs gate-on parity ---

    def test_parity_definition(self) -> None:
        lv = self._make_lvalue(has_node=True)
        self._assert_par(lv, self.fx.a)

    def test_parity_none(self) -> None:
        lv = self._make_lvalue()
        self._assert_par(lv, None)

    def test_parity_any(self) -> None:
        lv = self._make_lvalue()
        self._assert_par(lv, AnyType(TypeOfAny.special_form))

    def test_parity_instance_no_set(self) -> None:
        lv = self._make_lvalue()
        self._assert_par(lv, Instance(self.fx.gi, []))

    def test_parity_callable(self) -> None:
        lv = self._make_lvalue()
        ct = self.fx.callable(self.fx.a, self.fx.a)
        self._assert_par(lv, ct)

    def test_parity_union(self) -> None:
        lv = self._make_lvalue()
        u = UnionType([self.fx.a, self.fx.nonet])
        self._assert_par(lv, u)

    def test_parity_other(self) -> None:
        lv = self._make_lvalue()
        self._assert_par(lv, self.fx.nonet)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsNoopForReachabilitySuite(Suite):
    """Parity for the Rust `is_noop_for_reachability` port (H1j).

    `TypeChecker.is_noop_for_reachability` (checker.py:4658-4684)
    classifies a statement as a no-op for `--warn-unreachable`. The Rust
    port decides the AssertStmt / ReturnStmt / RaiseStmt branches and
    the non-CallExpr ExpressionStmt fallthrough natively; the
    ExpressionStmt+CallExpr branch defers (None) because it needs
    `self.expr_checker.accept()`.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _seam(self, stmt: Any) -> Any:
        return _type_kernel.rust_is_noop_for_reachability(stmt)

    def _make_false_expr(self) -> NameExpr:
        n = NameExpr("False")
        n.fullname = "builtins.False"
        return n

    def _make_not_implemented_expr(self) -> NameExpr:
        n = NameExpr("NotImplemented")
        n.fullname = "builtins.NotImplemented"
        return n

    def test_seam_assert_false(self) -> None:
        stmt = AssertStmt(self._make_false_expr())
        assert self._seam(stmt) is True

    def test_seam_assert_true(self) -> None:
        n = NameExpr("True")
        n.fullname = "builtins.True"
        stmt = AssertStmt(n)
        assert self._seam(stmt) is False

    def test_seam_assert_int_zero(self) -> None:
        stmt = AssertStmt(IntExpr(0))
        assert self._seam(stmt) is True

    def test_seam_return_not_implemented(self) -> None:
        stmt = ReturnStmt(self._make_not_implemented_expr())
        assert self._seam(stmt) is True

    def test_seam_return_none(self) -> None:
        stmt = ReturnStmt(None)
        assert self._seam(stmt) is False

    def test_seam_return_value(self) -> None:
        stmt = ReturnStmt(IntExpr(42))
        assert self._seam(stmt) is False

    def test_seam_raise(self) -> None:
        stmt = RaiseStmt(None, None)
        assert self._seam(stmt) is True

    def test_seam_expr_stmt_non_call(self) -> None:
        stmt = ExpressionStmt(IntExpr(1))
        assert self._seam(stmt) is False

    def test_seam_expr_stmt_call_defers(self) -> None:
        call = CallExpr(NameExpr("f"), [], [], [])
        stmt = ExpressionStmt(call)
        assert self._seam(stmt) is None

    def test_seam_other_stmt(self) -> None:
        stmt = PassStmt()
        assert self._seam(stmt) is False

    def test_parity_assert_false(self) -> None:
        from mypy.checker import TypeChecker

        stmt = AssertStmt(self._make_false_expr())

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_noop_for_reachability(stmt)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(on, off)

    def test_parity_return_not_implemented(self) -> None:
        from mypy.checker import TypeChecker

        stmt = ReturnStmt(self._make_not_implemented_expr())

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_noop_for_reachability(stmt)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(on, off)

    def test_parity_raise(self) -> None:
        from mypy.checker import TypeChecker

        stmt = RaiseStmt(None, None)

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_noop_for_reachability(stmt)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(on, off)

    def test_parity_pass_stmt(self) -> None:
        from mypy.checker import TypeChecker

        stmt = PassStmt()

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            return chk.is_noop_for_reachability(stmt)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(on, off)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsLiteralEnumSuite(Suite):
    """Parity for the Rust `is_literal_enum` port (checker.py:10586).

    `TypeChecker.is_literal_enum` returns True when a `Foo.A` member
    expression is an enum literal: the parent type is a type-object
    callable, the member type is an enum LiteralType, and the
    fallback TypeInfo matches `type_object()`. The Rust seam
    (`rust_is_literal_enum`) reads the two resolved proper types via
    PyO3. Direct seam calls assert the exact bool; the gate-off vs
    gate-on differential drives the real TypeChecker method.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self.fx = TypeFixture()
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _make_enum_info(self, name: str = "mod.Foo") -> TypeInfo:
        info = self.fx.make_type_info(name, mro=[self.fx.oi])
        info.is_enum = True
        return info

    def _make_type_obj_callable(self, enum_info: TypeInfo) -> CallableType:
        meta_info = self.fx.make_type_info("builtins.type")
        meta_info.fallback_to_any = True
        enum_inst = Instance(enum_info, [])
        return CallableType(
            [], [], [], enum_inst, Instance(meta_info, []), instance_type=enum_inst
        )

    def _seam(self, parent: Any, member: Any) -> Any:
        return _type_kernel.rust_is_literal_enum(parent, member)

    def _run(self, parent_type: Type, member_type: Type) -> tuple[bool, bool]:
        from mypy.checker import TypeChecker

        parent_expr = NameExpr("Foo")
        member_expr = MemberExpr(parent_expr, "A")
        chk = TypeChecker.__new__(TypeChecker)
        chk._type_maps = [{parent_expr: parent_type, member_expr: member_type}]

        def check() -> bool:
            chk2 = TypeChecker.__new__(TypeChecker)
            chk2._type_maps = [{parent_expr: parent_type, member_expr: member_type}]
            return chk2.is_literal_enum(member_expr)

        off = self._with_gate(False, check)
        on = self._with_gate(True, check)
        return off, on

    def test_seam_true(self) -> None:
        enum_info = self._make_enum_info()
        parent = self._make_type_obj_callable(enum_info)
        member = LiteralType(1, Instance(enum_info, []))
        assert self._seam(parent, member) is True

    def test_seam_false_not_functionlike(self) -> None:
        enum_info = self._make_enum_info()
        member = LiteralType(1, Instance(enum_info, []))
        assert self._seam(Instance(enum_info, []), member) is False

    def test_seam_false_not_literaltype(self) -> None:
        enum_info = self._make_enum_info()
        parent = self._make_type_obj_callable(enum_info)
        assert self._seam(parent, Instance(enum_info, [])) is False

    def test_seam_false_not_type_obj(self) -> None:
        enum_info = self._make_enum_info()
        non_type_callable = CallableType([], [], [], Instance(enum_info, []), self.fx.function)
        member = LiteralType(1, Instance(enum_info, []))
        assert self._seam(non_type_callable, member) is False

    def test_seam_false_not_enum_literal(self) -> None:
        enum_info = self._make_enum_info()
        parent = self._make_type_obj_callable(enum_info)
        member = LiteralType(1, Instance(self.fx.ai, []))
        assert self._seam(parent, member) is False

    def test_seam_false_wrong_fallback(self) -> None:
        enum_info = self._make_enum_info()
        other_info = self._make_enum_info("mod.Bar")
        other_info.is_enum = True
        parent = self._make_type_obj_callable(enum_info)
        member = LiteralType(1, Instance(other_info, []))
        assert self._seam(parent, member) is False

    def test_parity_true(self) -> None:
        enum_info = self._make_enum_info()
        parent = self._make_type_obj_callable(enum_info)
        member = LiteralType(1, Instance(enum_info, []))
        off, on = self._run(parent, member)
        assert_equal(on, off, "is_literal_enum parity: true case")
        assert on is True

    def test_false_not_member_expr_precedes_the_gate(self) -> None:
        # A bare NameExpr returns at checker.py:10717, before the seam gate
        # at :10727 is read, so both gate states run identical Python and the
        # arm comparison cannot fail on this shape. Pin one run instead.
        from mypy.checker import TypeChecker

        chk = TypeChecker.__new__(TypeChecker)
        chk._type_maps = [{}]
        assert chk.is_literal_enum(NameExpr("Foo")) is False

    def test_false_none_types_precede_the_gate(self) -> None:
        # Neither expression has a recorded type, so is_literal_enum
        # returns at checker.py:10722, before the seam gate is read; the
        # arm comparison cannot fail on this shape. Pin one run instead.
        from mypy.checker import TypeChecker

        member_expr = MemberExpr(NameExpr("Foo"), "A")
        chk = TypeChecker.__new__(TypeChecker)
        chk._type_maps = [{}]
        assert chk.is_literal_enum(member_expr) is False


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeUnboundReturnTypevarSuite(Suite):
    """Parity for the Rust `check_unbound_return_typevar` port (H1l).

    `TypeChecker.check_unbound_return_typevar` (checker.py:2827) fails
    when the return TypeVar is declared in `variables` but does not
    appear in any argument type. The Rust seam
    (`rust_classify_unbound_return_typevar`) decodes the wire
    `CallableType`, checks whether `ret_type` is a `TypeVarType`
    present in `variables`, and walks `arg_types` collecting all
    `TypeVarType` ids (mirroring `CollectArgTypeVarTypes`). Returns
    `Some(0)` = pass, `Some(1)` = fail + no note (upper bound is
    `builtins.object`), `Some(2)` = fail + note, `None` = defer.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _wire(self, t: Type) -> bytes:
        from mypy.checker import _serialize_type_for_checker

        return _serialize_type_for_checker(t)

    def _seam(self, typ: CallableType) -> int | None:
        return _type_kernel.rust_classify_unbound_return_typevar(self._wire(typ))

    def _run(self, typ: CallableType) -> tuple[list[str], list[str]]:
        from mypy.checker import TypeChecker
        from mypy.options import Options

        def run_one() -> list[str]:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            records: list[str] = []

            chk.fail = lambda msg, _ctx, **_kw: records.append(  # type: ignore[assignment]
                str(msg)
            )
            chk.note = lambda msg, context, **_kw: records.append(  # type: ignore[method-assign]
                str(msg)
            )
            chk.check_unbound_return_typevar(typ)
            return records

        off = self._with_gate(False, run_one)
        on = self._with_gate(True, run_one)
        return off, on

    def _make_tvar(self, name: str = "T", uid: int = -1) -> TypeVarType:
        return TypeVarType(
            name, name, TypeVarId(uid), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )

    def _make_callable(
        self, ret: Type, args: list[Type], variables: list[TypeVarType]
    ) -> CallableType:
        return CallableType(
            args,
            [ARG_POS] * len(args),
            [None] * len(args),
            ret,
            self.fx.function,
            name=None,
            variables=variables,
        )

    # -- direct seam tests --

    def test_seam_pass_ret_not_tvar(self) -> None:
        ct = self._make_callable(self.fx.a, [self.fx.a], [])
        assert self._seam(ct) == 0

    def test_seam_pass_tvar_not_in_variables(self) -> None:
        t = self._make_tvar()
        ct = self._make_callable(t, [self.fx.a], [])
        assert self._seam(ct) == 0

    def test_seam_pass_tvar_in_args(self) -> None:
        t = self._make_tvar()
        ct = self._make_callable(t, [t], [t])
        assert self._seam(ct) == 0

    def test_seam_pass_tvar_nested_in_args(self) -> None:
        t = self._make_tvar()
        ct = self._make_callable(t, [Instance(self.fx.oi, [t])], [t])
        assert self._seam(ct) == 0

    def test_seam_fail_object_bound(self) -> None:
        t = self._make_tvar()
        ct = self._make_callable(t, [self.fx.a], [t])
        assert self._seam(ct) == 1

    def test_seam_fail_non_object_bound(self) -> None:
        t = TypeVarType(
            "T", "T", TypeVarId(-1), [], self.fx.str_type, AnyType(TypeOfAny.from_omitted_generics)
        )
        ct = self._make_callable(t, [self.fx.a], [t])
        assert self._seam(ct) == 2

    def test_seam_defers_on_bad_wire(self) -> None:
        assert _type_kernel.rust_classify_unbound_return_typevar(b"\xff\xff\xff") is None

    # -- gate off/on parity --

    def test_parity_pass_not_tvar(self) -> None:
        ct = self._make_callable(self.fx.a, [self.fx.a], [])
        off, on = self._run(ct)
        assert_equal(on, off, "parity: non-tvar return")
        assert off == []

    def test_parity_pass_tvar_not_in_variables(self) -> None:
        t = self._make_tvar()
        ct = self._make_callable(t, [self.fx.a], [])
        off, on = self._run(ct)
        assert_equal(on, off, "parity: tvar not in variables")
        assert off == []

    def test_parity_pass_tvar_in_args(self) -> None:
        t = self._make_tvar()
        ct = self._make_callable(t, [t], [t])
        off, on = self._run(ct)
        assert_equal(on, off, "parity: tvar in args")
        assert off == []

    def test_parity_fail_object_bound(self) -> None:
        t = self._make_tvar()
        ct = self._make_callable(t, [self.fx.a], [t])
        off, on = self._run(ct)
        assert_equal(on, off, "parity: fail object bound")
        assert len(off) == 1
        assert "TypeVar should receive" in off[0]

    def test_parity_fail_non_object_bound(self) -> None:
        t = TypeVarType(
            "T", "T", TypeVarId(-1), [], self.fx.str_type, AnyType(TypeOfAny.from_omitted_generics)
        )
        ct = self._make_callable(t, [self.fx.a], [t])
        off, on = self._run(ct)
        assert_equal(on, off, "parity: fail non-object bound")
        assert len(off) == 2
        assert "TypeVar should receive" in off[0]
        assert "upper bound" in off[1]

    def test_parity_pass_tvar_nested_in_args(self) -> None:
        t = self._make_tvar()
        ct = self._make_callable(t, [Instance(self.fx.oi, [t])], [t])
        off, on = self._run(ct)
        assert_equal(on, off, "parity: tvar nested in args")
        assert off == []


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckUntypedAfterDecoratorSuite(Suite):
    """Parity for the Rust `check_untyped_after_decorator` port (H1o).

    `TypeChecker.check_untyped_after_decorator` (checker.py) is a gate +
    `has_any_type` check: if `disallow_any_decorated` is on, the file is not
    a stub, and the node is not deferred, it emits
    `untyped_decorated_function` when the decorated type contains Any.
    The Rust port takes the 3 gate bools as scalars, decodes the wire type,
    and runs `has_any_type_inner` (the same BoolTypeQuery the checkexpr
    seam uses). Direct seam calls assert the bool; the gate-off vs gate-on
    differential drives the real TypeChecker method through a stub checker
    with a mock msg recorder.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
        self._set_active(True)
        self.fx = TypeFixture()
        self.resolver = _type_kernel.build_native_resolver([], [])

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _seam(
        self,
        typ: Type,
        disallow_any_decorated: bool = True,
        is_stub: bool = False,
        current_node_deferred: bool = False,
    ) -> bool | None:
        from mypy.checker import _serialize_type_for_checker

        return _type_kernel.rust_check_untyped_after_decorator(
            disallow_any_decorated,
            is_stub,
            current_node_deferred,
            _serialize_type_for_checker(typ),
            self.resolver,
        )

    def _run(
        self,
        typ: Type,
        disallow_any_decorated: bool = True,
        is_stub: bool = False,
        current_node_deferred: bool = False,
    ) -> tuple[list[Any], list[Any]]:
        from types import SimpleNamespace

        from mypy.checker import TypeChecker

        def check_one() -> list[Any]:
            chk = TypeChecker.__new__(TypeChecker)
            fails: list[Any] = []
            chk.options = SimpleNamespace(  # type: ignore[assignment]
                disallow_any_decorated=disallow_any_decorated
            )
            chk.is_stub = is_stub
            chk.current_node_deferred = current_node_deferred
            chk.msg = SimpleNamespace(  # type: ignore[assignment]
                untyped_decorated_function=lambda t, ctx: fails.append(t)
            )
            func = FuncDef("f")
            chk.check_untyped_after_decorator(typ, func)
            return list(fails)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(
        self,
        typ: Type,
        disallow_any_decorated: bool = True,
        is_stub: bool = False,
        current_node_deferred: bool = False,
    ) -> None:
        off, on = self._run(typ, disallow_any_decorated, is_stub, current_node_deferred)
        assert_equal(on, off, f"check_untyped_after_decorator parity for typ={typ!r}")

    # --- direct seam tests ---

    def test_seam_any_type_fires(self) -> None:
        assert self._seam(AnyType(TypeOfAny.from_error)) is True

    def test_seam_special_form_any_does_not_fire(self) -> None:
        assert self._seam(AnyType(TypeOfAny.special_form)) is False

    def test_seam_plain_instance_does_not_fire(self) -> None:
        assert self._seam(self.fx.a) is False

    def test_seam_gate_disallow_off(self) -> None:
        assert self._seam(AnyType(TypeOfAny.from_error), disallow_any_decorated=False) is False

    def test_seam_gate_is_stub(self) -> None:
        assert self._seam(AnyType(TypeOfAny.from_error), is_stub=True) is False

    def test_seam_gate_deferred(self) -> None:
        assert self._seam(AnyType(TypeOfAny.from_error), current_node_deferred=True) is False

    def test_seam_callable_with_any_fires(self) -> None:
        any_unannotated = AnyType(TypeOfAny.unannotated)
        ct = CallableType([any_unannotated], [ARG_POS], [None], any_unannotated, self.fx.function)
        assert self._seam(ct) is True

    def test_seam_typed_callable_does_not_fire(self) -> None:
        ct = CallableType([self.fx.a], [ARG_POS], [None], self.fx.a, self.fx.function)
        assert self._seam(ct) is False

    # --- gate-off vs gate-on parity ---

    def test_parity_any_type(self) -> None:
        self._assert_par(AnyType(TypeOfAny.from_error))

    def test_parity_special_form_any(self) -> None:
        self._assert_par(AnyType(TypeOfAny.special_form))

    def test_parity_plain_instance(self) -> None:
        self._assert_par(self.fx.a)

    def test_parity_gate_disallow_off(self) -> None:
        self._assert_par(AnyType(TypeOfAny.from_error), disallow_any_decorated=False)

    def test_parity_gate_is_stub(self) -> None:
        self._assert_par(AnyType(TypeOfAny.from_error), is_stub=True)

    def test_parity_gate_deferred(self) -> None:
        self._assert_par(AnyType(TypeOfAny.from_error), current_node_deferred=True)

    def test_parity_typed_callable(self) -> None:
        ct = CallableType([self.fx.a], [ARG_POS], [None], self.fx.a, self.fx.function)
        self._assert_par(ct)

    def test_parity_callable_with_any(self) -> None:
        any_unannotated = AnyType(TypeOfAny.unannotated)
        ct = CallableType([any_unannotated], [ARG_POS], [None], any_unannotated, self.fx.function)
        self._assert_par(ct)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIncompatiblePropertyOverrideSuite(Suite):
    """Parity for the Rust `check_incompatible_property_override` port (H1m).

    `TypeChecker.check_incompatible_property_override` (checker.py:7800)
    walks `e.func.info.mro[1:]` looking for a base with a settable
    property of the same name that the read-only `e` overrides. The Rust
    port (checker_functions.rs) reads the live Decorator via PyO3 and
    returns Some(true) when the fail should fire, Some(false) otherwise,
    None on an unreadable attribute.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self._set_active = _set_native_checker_active
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

    def _make_info(self, name: str, mro: list[TypeInfo] | None = None) -> TypeInfo:
        return self.fx.make_type_info(name, mro=mro)

    def _put_property(self, info: TypeInfo, name: str, is_settable: bool) -> None:
        v = Var(name)
        v.is_property = True
        v.is_settable_property = is_settable
        dec = Decorator(FuncDef(name), [], v)
        ofd = OverloadedFuncDef([dec])
        ofd.is_property = True
        info.names[name] = SymbolTableNode(MDEF, ofd)

    def _decorator(self, name: str, info: TypeInfo, is_settable: bool = False) -> Decorator:
        v = Var(name)
        v.is_property = True
        v.is_settable_property = is_settable
        fdef = FuncDef(name)
        fdef.info = info
        return Decorator(fdef, [], v)

    def _seam(self, e: Decorator) -> Any:
        return _type_kernel.rust_check_incompatible_property_override(e)

    def _run(self, e: Decorator) -> tuple[list[str], list[str]]:
        from mypy.checker import TypeChecker

        def check_one() -> list[str]:
            records: list[str] = []
            chk = TypeChecker.__new__(TypeChecker)
            chk.fail = lambda msg, _ctx, **_kw: records.append(str(msg))  # type: ignore[assignment]
            chk.check_incompatible_property_override(e)
            return records

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, e: Decorator) -> None:
        off, on = self._run(e)
        assert_equal(on, off, f"check_incompatible_property_override parity for e={e!r}")

    def test_seam_readonly_overrides_settable(self) -> None:
        base = self._make_info("Base", mro=[self.fx.oi])
        self._put_property(base, "prop", is_settable=True)
        sub = self._make_info("Sub", mro=[base, self.fx.oi])
        e = self._decorator("prop", sub, is_settable=False)
        assert self._seam(e) is True

    def test_seam_settable_overrides_settable(self) -> None:
        base = self._make_info("Base", mro=[self.fx.oi])
        self._put_property(base, "prop", is_settable=True)
        sub = self._make_info("Sub", mro=[base, self.fx.oi])
        e = self._decorator("prop", sub, is_settable=True)
        assert self._seam(e) is False

    def test_seam_no_base_property(self) -> None:
        base = self._make_info("Base", mro=[self.fx.oi])
        sub = self._make_info("Sub", mro=[base, self.fx.oi])
        e = self._decorator("prop", sub, is_settable=False)
        assert self._seam(e) is False

    def test_seam_base_property_not_settable(self) -> None:
        base = self._make_info("Base", mro=[self.fx.oi])
        self._put_property(base, "prop", is_settable=False)
        sub = self._make_info("Sub", mro=[base, self.fx.oi])
        e = self._decorator("prop", sub, is_settable=False)
        assert self._seam(e) is False

    def test_seam_no_info(self) -> None:
        fdef = FuncDef("prop")
        v = Var("prop")
        v.is_property = True
        v.is_settable_property = False
        e = Decorator(fdef, [], v)
        assert self._seam(e) is False

    def test_seam_base_not_property(self) -> None:
        base = self._make_info("Base", mro=[self.fx.oi])
        v = Var("prop")
        base.names["prop"] = SymbolTableNode(MDEF, v)
        sub = self._make_info("Sub", mro=[base, self.fx.oi])
        e = self._decorator("prop", sub, is_settable=False)
        assert self._seam(e) is False

    def test_parity_readonly_overrides_settable(self) -> None:
        base = self._make_info("Base", mro=[self.fx.oi])
        self._put_property(base, "prop", is_settable=True)
        sub = self._make_info("Sub", mro=[base, self.fx.oi])
        e = self._decorator("prop", sub, is_settable=False)
        self._assert_par(e)

    def test_parity_settable_overrides_settable(self) -> None:
        base = self._make_info("Base", mro=[self.fx.oi])
        self._put_property(base, "prop", is_settable=True)
        sub = self._make_info("Sub", mro=[base, self.fx.oi])
        e = self._decorator("prop", sub, is_settable=True)
        self._assert_par(e)

    def test_parity_no_base_property(self) -> None:
        base = self._make_info("Base", mro=[self.fx.oi])
        sub = self._make_info("Sub", mro=[base, self.fx.oi])
        e = self._decorator("prop", sub, is_settable=False)
        self._assert_par(e)

    def test_parity_base_property_not_settable(self) -> None:
        base = self._make_info("Base", mro=[self.fx.oi])
        self._put_property(base, "prop", is_settable=False)
        sub = self._make_info("Sub", mro=[base, self.fx.oi])
        e = self._decorator("prop", sub, is_settable=False)
        self._assert_par(e)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCanWidenInScopeSuite(Suite):
    """Parity for `rust_can_widen_in_scope` (H1n).

    `TypeChecker.can_widen_in_scope` (checker.py:6702-6716) is a pure bool:
    returns False when `name.kind == GDEF`, the scope is inside a top-level
    function, and `get_proper_type(orig_type)` is not NoneType; else True.
    The Rust port reads live objects via PyO3 (zero wire bytes), mirroring
    `rust_is_writable_attribute`. Direct seam calls assert the exact bool;
    the gate-off vs gate-on differential drives the real TypeChecker method.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        self._set_active(True)

    def _set_active(self, active: bool) -> None:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Any) -> Any:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _name_expr(self, kind: int = 1) -> NameExpr:
        ne = NameExpr("x")
        ne.kind = kind
        return ne

    def _scope(self, in_function: bool = False) -> Any:
        from mypy.checker_shared import CheckerScope
        from mypy.nodes import Block

        module = MypyFile([], [])
        scope = CheckerScope(module)
        if in_function:
            fdef = FuncDef("f", [], Block([]))
            scope.stack.append(fdef)
        return scope

    def _seam(self, name: Any, orig_type: Any, scope: Any) -> bool | None:
        return _type_kernel.rust_can_widen_in_scope(name, orig_type, scope)

    def _run(self, name: Any, orig_type: Any, scope: Any) -> tuple[bool, bool]:
        from mypy.checker import TypeChecker

        def check_one() -> bool:
            chk = TypeChecker.__new__(TypeChecker)
            chk.scope = scope
            return chk.can_widen_in_scope(name, orig_type)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, name: Any, orig_type: Any, scope: Any) -> None:
        off, on = self._run(name, orig_type, scope)
        assert_equal(on, off, "can_widen_in_scope parity")

    # --- Direct seam calls ---

    def test_seam_gdef_in_function_non_none_type(self) -> None:
        """GDEF name in a function with non-None orig_type -> False."""
        name = self._name_expr(kind=1)
        scope = self._scope(in_function=True)
        assert self._seam(name, self.fx.anyt, scope) is False

    def test_seam_gdef_in_function_none_type(self) -> None:
        """GDEF name in a function with NoneType orig_type -> True."""
        name = self._name_expr(kind=1)
        scope = self._scope(in_function=True)
        assert self._seam(name, self.fx.nonet, scope) is True

    def test_seam_gdef_not_in_function(self) -> None:
        """GDEF name not in a function -> True."""
        name = self._name_expr(kind=1)
        scope = self._scope(in_function=False)
        assert self._seam(name, self.fx.anyt, scope) is True

    def test_seam_ldef(self) -> None:
        """LDEF name (not GDEF) -> True."""
        name = self._name_expr(kind=2)
        scope = self._scope(in_function=True)
        assert self._seam(name, self.fx.anyt, scope) is True

    def test_seam_mdef(self) -> None:
        """MDEF name -> True."""
        name = self._name_expr(kind=3)
        scope = self._scope(in_function=True)
        assert self._seam(name, self.fx.anyt, scope) is True

    # --- Gate-off vs gate-on parity ---

    def test_parity_gdef_in_function_non_none_type(self) -> None:
        name = self._name_expr(kind=1)
        scope = self._scope(in_function=True)
        self._assert_par(name, self.fx.anyt, scope)

    def test_parity_gdef_in_function_none_type(self) -> None:
        name = self._name_expr(kind=1)
        scope = self._scope(in_function=True)
        self._assert_par(name, self.fx.nonet, scope)

    def test_parity_gdef_not_in_function(self) -> None:
        name = self._name_expr(kind=1)
        scope = self._scope(in_function=False)
        self._assert_par(name, self.fx.anyt, scope)

    def test_parity_ldef(self) -> None:
        name = self._name_expr(kind=2)
        scope = self._scope(in_function=True)
        self._assert_par(name, self.fx.anyt, scope)

    def test_parity_mdef(self) -> None:
        name = self._name_expr(kind=3)
        scope = self._scope(in_function=True)
        self._assert_par(name, self.fx.anyt, scope)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativePatternCheckDriverSuite(Suite):
    """Parity for the #1633 PatternChecker driver-head classifiers.

    `visit_sequence_pattern` steps 1-3 (`rust_classify_sequence_pattern_head`),
    the fixed-tuple step-5 fold (`rust_classify_sequence_tuple_result`), the
    mapping `o.rest` branch (`rust_classify_mapping_rest`), the class-pattern
    alias gate and keyword arbitration (`rust_classify_class_pattern_alias_gate`,
    `rust_classify_class_pattern_keywords`), and the or-pattern match-type
    filter (`rust_filter_or_match_types`). Each shim keeps the verbatim Python
    head as its fallback; direct seam calls prove engagement and deferral,
    gate-off vs gate-on differentials prove the routing agrees, and the
    or/value/singleton tests compare full `PatternType` triples.

    What stays Python (per #1633): `accept` recursion, `conditional_types`
    narrowing, `TypeRange` construction from live TypeInfos, `msg.fail`
    emission, binder writes via `update_type_map`, and the
    `get_mapping_item_type` / `iterable_item_type` checker calls.
    """

    def setUp(self) -> None:
        from mypy.checkpattern import _set_native_checkpattern_active
        from mypy.subtypes import _set_native_subtype_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active = _set_native_checkpattern_active
        self._set_resolver = _set_native_subtype_resolver
        self._set_active(True)
        self.fx = TypeFixture()
        self.options = Options()
        anyt = AnyType(TypeOfAny.special_form)
        self.iter_info = self.fx.make_type_info("typing.Iterable", mro=[self.fx.oi])
        self.seq_info = self.fx.make_type_info("typing.Sequence", mro=[self.iter_info, self.fx.oi])
        self.map_info = self.fx.make_type_info("typing.Mapping", mro=[self.fx.oi])
        self.tup_seq_info = self.fx.make_type_info(
            "test.TupleSeq",
            mro=[self.seq_info, self.fx.oi],
            bases=[Instance(self.seq_info, [anyt])],
        )
        self.infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.str_type_info,
            self.fx.std_tuplei,
            self.fx.std_listi,
            self.fx.bool_type_info,
            self.fx.functioni,
            self.iter_info,
            self.seq_info,
            self.map_info,
            self.tup_seq_info,
        ]
        set_wire_typeinfo_map({i.fullname: i for i in self.infos})
        self.resolver = _type_kernel.build_native_resolver(self.infos, [])
        self._set_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _any(self) -> AnyType:
        return AnyType(TypeOfAny.special_form)

    def _named_type(self, name: str) -> Type:
        if name == "typing.Sequence":
            return Instance(self.seq_info, [self._any()])
        if name == "typing.Iterable":
            return Instance(self.iter_info, [self._any()])
        if name == "typing.Mapping":
            return Instance(self.map_info, [self._any(), self._any()])
        if name == "builtins.object":
            return self.fx.o
        raise AssertionError(f"unexpected named_type {name}")

    def _named_generic_type(self, name: str, args: list[Type]) -> Type:
        if name == "typing.Iterable":
            return Instance(self.iter_info, args)
        if name == "typing.Sequence":
            return Instance(self.seq_info, args)
        if name == "builtins.list":
            return Instance(self.fx.std_listi, args)
        raise AssertionError(f"unexpected named_generic_type {name}")

    def _pc(self) -> Any:
        from mypy.checkpattern import PatternChecker

        pc = PatternChecker.__new__(PatternChecker)
        pc.chk = SimpleNamespace(  # type: ignore[assignment]
            named_type=self._named_type,
            named_generic_type=self._named_generic_type,
            type_is_iterable=self._type_is_iterable,
        )
        pc.msg = SimpleNamespace(fail=lambda msg, ctx: None)  # type: ignore[assignment]
        pc.options = self.options
        pc.type_context = []
        pc.self_match_types = []
        pc.non_sequence_match_types = [Instance(self.fx.str_type_info, [])]
        return pc

    def _type_is_iterable(self, t: Type) -> bool:

        return bool(is_subtype(t, Instance(self.iter_info, [self._any()])))

    # ----- sequence head -----

    def _assert_seq_head_par(self, current: Type, star: int | None, required: int) -> int:
        off = self._with_gate(
            False, lambda: self._pc()._classify_sequence_head(current, star, required)
        )
        on = self._with_gate(
            True, lambda: self._pc()._classify_sequence_head(current, star, required)
        )
        assert_equal(on, off, f"sequence head parity for {current!r}")
        return int(on)

    def test_seq_head_cannot_match(self) -> None:
        from mypy.checkpattern import _SEQ_CANNOT_MATCH

        tag = self._assert_seq_head_par(self.fx.a, None, 1)
        assert_equal(tag, _SEQ_CANNOT_MATCH)

    def test_seq_head_union(self) -> None:
        from mypy.checkpattern import _SEQ_UNION

        seq = Instance(self.seq_info, [self._any()])
        tag = self._assert_seq_head_par(UnionType.make_union([seq, self.fx.o]), None, 1)
        assert_equal(tag, _SEQ_UNION)

    def test_seq_head_any(self) -> None:
        from mypy.checkpattern import _SEQ_ANY

        tag = self._assert_seq_head_par(AnyType(TypeOfAny.special_form), None, 2)
        assert_equal(tag, _SEQ_ANY)

    def test_seq_head_iterable(self) -> None:
        from mypy.checkpattern import _SEQ_ITERABLE

        tag = self._assert_seq_head_par(Instance(self.seq_info, [self._any()]), None, 1)
        assert_equal(tag, _SEQ_ITERABLE)

    def test_seq_head_other(self) -> None:
        from mypy.checkpattern import _SEQ_OTHER

        tag = self._assert_seq_head_par(self.fx.o, None, 1)
        assert_equal(tag, _SEQ_OTHER)

    def _seq_tuple(self, items: list[Type]) -> TupleType:
        return TupleType(items, Instance(self.tup_seq_info, [self._any()]))

    def test_seq_head_fixed_ok(self) -> None:
        from mypy.checkpattern import _SEQ_FIXED_OK

        tag = self._assert_seq_head_par(self._seq_tuple([self.fx.a, self.fx.a]), None, 2)
        assert_equal(tag, _SEQ_FIXED_OK)

    def test_seq_head_fixed_sizes(self) -> None:
        from mypy.checkpattern import _SEQ_FIXED_TOO_FEW, _SEQ_FIXED_TOO_MANY

        assert_equal(
            self._assert_seq_head_par(self._seq_tuple([self.fx.a]), None, 2), _SEQ_FIXED_TOO_FEW
        )
        assert_equal(
            self._assert_seq_head_par(self._seq_tuple([self.fx.a, self.fx.a]), None, 1),
            _SEQ_FIXED_TOO_MANY,
        )
        # A star absorbs the surplus.
        from mypy.checkpattern import _SEQ_FIXED_OK

        assert_equal(
            self._assert_seq_head_par(self._seq_tuple([self.fx.a, self.fx.a]), 0, 1), _SEQ_FIXED_OK
        )

    def test_seq_head_variadic(self) -> None:
        from mypy.checkpattern import _SEQ_VARIADIC_OK, _SEQ_VARIADIC_TOO_MANY

        tup = TupleType(
            [self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self._any()]))],
            Instance(self.tup_seq_info, [self._any()]),
        )
        assert_equal(self._assert_seq_head_par(tup, None, 0), _SEQ_VARIADIC_TOO_MANY)
        assert_equal(self._assert_seq_head_par(tup, None, 1), _SEQ_VARIADIC_OK)

    def test_seq_head_alias_defers_to_python(self) -> None:
        from mypy.checkpattern import _SEQ_CANNOT_MATCH

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        # No alias snapshot installed: the seam defers, the live
        # get_proper_type fallback expands to A and decides.
        assert self._seam_seq_head(typ, None, 1) is None
        assert_equal(self._assert_seq_head_par(typ, None, 1), _SEQ_CANNOT_MATCH)

    def test_seq_head_alias_engages_with_snapshot(self) -> None:
        from mypy.checkpattern import _SEQ_CANNOT_MATCH

        alias = TypeAlias(self.fx.a, "mod.Snap", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        resolver = _type_kernel.build_native_resolver(self.infos, [alias])
        non_seq = UnionType.make_union([Instance(self.fx.str_type_info, [])])
        seq = Instance(self.seq_info, [self._any()])
        tag = _type_kernel.rust_classify_sequence_pattern_head(
            self._bytes_of(typ),
            None,
            1,
            self._bytes_of(non_seq),
            self._bytes_of(seq),
            self._bytes_of(Instance(self.iter_info, [self._any()])),
            resolver,
        )
        assert_equal(tag, _SEQ_CANNOT_MATCH)

    def _seam_seq_head(self, current: Type, star: int | None, required: int) -> Any:
        non_seq = UnionType.make_union([Instance(self.fx.str_type_info, [])])
        seq = Instance(self.seq_info, [self._any()])
        return _type_kernel.rust_classify_sequence_pattern_head(
            self._bytes_of(current),
            star,
            required,
            self._bytes_of(non_seq),
            self._bytes_of(seq),
            self._bytes_of(Instance(self.iter_info, [self._any()])),
            self.resolver,
        )

    def test_seq_head_resolver_missing_falls_back(self) -> None:
        from mypy.checkpattern import _SEQ_CANNOT_MATCH

        self._set_resolver(None)
        try:
            tag = self._with_gate(
                True, lambda: self._pc()._classify_sequence_head(self.fx.a, None, 1)
            )
        finally:
            self._set_resolver(self.resolver)
        assert_equal(tag, _SEQ_CANNOT_MATCH)

    # ----- sequence tuple result -----

    def _assert_seq_result_par(
        self, new: list[Type], rest: list[Type]
    ) -> tuple[bool, int, int, list[bool]]:
        off = self._with_gate(False, lambda: self._pc()._classify_sequence_tuple_result(new, rest))
        on = self._with_gate(True, lambda: self._pc()._classify_sequence_tuple_result(new, rest))
        assert_equal(on, off, "sequence tuple-result parity")
        return cast("tuple[bool, int, int, list[bool]]", on)

    def test_seq_result_all(self) -> None:
        from mypy.checkpattern import _SEQ_REST_ALL

        never = UninhabitedType()
        new, tag, idx, mask = self._assert_seq_result_par([never, self.fx.a], [never, never])
        assert new is True
        assert_equal((tag, idx, mask), (_SEQ_REST_ALL, -1, [True, True]))

    def test_seq_result_single(self) -> None:
        from mypy.checkpattern import _SEQ_REST_SINGLE

        never = UninhabitedType()
        new, tag, idx, mask = self._assert_seq_result_par(
            [self.fx.a, self.fx.b], [never, self.fx.a]
        )
        assert new is False
        assert_equal((tag, idx, mask), (_SEQ_REST_SINGLE, 1, [True, False]))

    def test_seq_result_keep(self) -> None:
        from mypy.checkpattern import _SEQ_REST_KEEP

        new, tag, idx, mask = self._assert_seq_result_par(
            [self.fx.a, self.fx.b], [self.fx.a, self.fx.b]
        )
        assert new is False
        assert_equal((tag, idx, mask), (_SEQ_REST_KEEP, -1, [False, False]))

    def test_seq_result_alias_defers(self) -> None:
        alias = TypeAlias(self.fx.a, "mod.R", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        assert (
            _type_kernel.rust_classify_sequence_tuple_result(
                [self._bytes_of(self.fx.a)], [self._bytes_of(typ)], self.resolver
            )
            is None
        )
        # Parity still holds through the live-alias fallback.
        self._assert_seq_result_par([self.fx.a], [typ])

    # ----- mapping rest -----

    def _assert_map_rest_par(self, current: Type) -> int:
        mapping = Instance(self.map_info, [self._any(), self._any()])
        off = self._with_gate(False, lambda: self._pc()._classify_mapping_rest(current, mapping))
        on = self._with_gate(True, lambda: self._pc()._classify_mapping_rest(current, mapping))
        assert_equal(on, off, f"mapping rest parity for {current!r}")
        return int(on)

    def test_map_rest_instance(self) -> None:
        from mypy.checkpattern import _MAP_INSTANCE

        assert_equal(
            self._assert_map_rest_par(Instance(self.map_info, [self._any(), self._any()])),
            _MAP_INSTANCE,
        )

    def test_map_rest_dict_fallback(self) -> None:
        from mypy.checkpattern import _MAP_DICT_FALLBACK

        assert_equal(self._assert_map_rest_par(self.fx.a), _MAP_DICT_FALLBACK)

    def test_map_rest_alias_defers(self) -> None:
        from mypy.checkpattern import _MAP_DICT_FALLBACK

        alias = TypeAlias(self.fx.a, "mod.M", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        mapping = Instance(self.map_info, [self._any(), self._any()])
        assert (
            _type_kernel.rust_classify_mapping_rest(
                self._bytes_of(typ), self._bytes_of(mapping), self.resolver
            )
            is None
        )
        assert_equal(self._assert_map_rest_par(typ), _MAP_DICT_FALLBACK)

    # ----- class alias gate -----

    def _assert_alias_gate_par(self, node: Any) -> bool:
        off = self._with_gate(False, lambda: self._pc()._is_generic_type_alias(node))
        on = self._with_gate(True, lambda: self._pc()._is_generic_type_alias(node))
        assert_equal(on, off, "class alias-gate parity")
        return bool(on)

    def test_alias_gate_none_node(self) -> None:
        assert _type_kernel.rust_classify_class_pattern_alias_gate(None) is False
        assert self._assert_alias_gate_par(None) is False

    def test_alias_gate_plain_var(self) -> None:
        v = Var("x")
        assert _type_kernel.rust_classify_class_pattern_alias_gate(v) is False
        assert self._assert_alias_gate_par(v) is False

    def test_alias_gate_generic_alias(self) -> None:
        alias = TypeAlias(self.fx.a, "mod.G", "mod", -1, -1)
        assert _type_kernel.rust_classify_class_pattern_alias_gate(alias) is True
        assert self._assert_alias_gate_par(alias) is True

    def test_alias_gate_no_args_alias(self) -> None:
        alias = TypeAlias(self.fx.a, "mod.N", "mod", -1, -1, no_args=True)
        assert _type_kernel.rust_classify_class_pattern_alias_gate(alias) is False
        assert self._assert_alias_gate_par(alias) is False

    # ----- class keywords -----

    def _assert_kw_par(
        self, names: list[str | None], n: int, keys: list[str]
    ) -> list[tuple[int, int]]:
        off = self._with_gate(
            False, lambda: self._pc()._classify_class_pattern_keywords(names, n, keys)
        )
        on = self._with_gate(
            True, lambda: self._pc()._classify_class_pattern_keywords(names, n, keys)
        )
        assert_equal(on, off, "class keywords parity")
        return cast("list[tuple[int, int]]", on)

    def test_kw_too_many(self) -> None:
        from mypy.checkpattern import _CLS_KW_TOO_MANY

        assert_equal(self._assert_kw_par(["x"], 2, ["a"]), [(_CLS_KW_TOO_MANY, -1)])
        assert_equal(
            _type_kernel.rust_classify_class_pattern_keywords(["x"], 2, ["a"]),
            [(_CLS_KW_TOO_MANY, -1)],
        )

    def test_kw_matches_positional(self) -> None:
        from mypy.checkpattern import _CLS_KW_MATCHES_POSITIONAL

        assert_equal(
            self._assert_kw_par(["x", "y"], 2, ["y", "a"]), [(_CLS_KW_MATCHES_POSITIONAL, 0)]
        )

    def test_kw_duplicate(self) -> None:
        from mypy.checkpattern import _CLS_KW_DUPLICATE

        assert_equal(self._assert_kw_par(["x"], 1, ["a", "a"]), [(_CLS_KW_DUPLICATE, 1)])

    def test_kw_mixed_order(self) -> None:
        from mypy.checkpattern import _CLS_KW_DUPLICATE, _CLS_KW_MATCHES_POSITIONAL

        # Both violations reported in key order, no early break.
        assert_equal(
            self._assert_kw_par(["x", "y"], 2, ["y", "a", "a"]),
            [(_CLS_KW_MATCHES_POSITIONAL, 0), (_CLS_KW_DUPLICATE, 2)],
        )

    def test_kw_ok_empty(self) -> None:
        assert_equal(self._assert_kw_par(["x"], 1, ["a", "b"]), [])
        assert_equal(self._assert_kw_par([], 0, []), [])

    # ----- or filter -----

    def _or_types(self, ts: list[Type]) -> list[Any]:
        from mypy.checkpattern import PatternType

        return [PatternType(t, UninhabitedType(), {}) for t in ts]

    def _assert_or_filter_par(self, ts: list[Type]) -> list[int]:
        pts = self._or_types(ts)
        off = self._with_gate(False, lambda: self._pc()._filter_or_match_types(pts))
        on = self._with_gate(True, lambda: self._pc()._filter_or_match_types(pts))
        assert_equal(on, off, "or-filter parity")
        return cast("list[int]", on)

    def test_or_filter_indices(self) -> None:
        never = UninhabitedType()
        assert_equal(self._assert_or_filter_par([self.fx.a, never, self.fx.b]), [0, 2])
        assert_equal(
            _type_kernel.rust_filter_or_match_types(
                [self._bytes_of(t) for t in [self.fx.a, never]], self.resolver
            ),
            [0],
        )

    def test_or_filter_all_uninhabited(self) -> None:
        assert_equal(self._assert_or_filter_par([UninhabitedType()]), [])

    def test_or_filter_alias_defers(self) -> None:
        alias = TypeAlias(self.fx.a, "mod.O", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        assert (
            _type_kernel.rust_filter_or_match_types([self._bytes_of(typ)], self.resolver) is None
        )
        assert_equal(self._assert_or_filter_par([self.fx.a, typ]), [0, 1])

    # ----- full PatternType differentials -----

    def _triple(self, pt: Any) -> tuple[str, str, list[tuple[str, str]]]:
        return (
            str(pt.type),
            str(pt.rest_type),
            sorted((str(e), str(t)) for e, t in pt.captures.items()),
        )

    def _capture(self, name: str) -> Any:
        from mypy.nodes import NameExpr

        v = Var(name)
        e = NameExpr(name)
        e.node = v
        return e

    def test_or_visit_patterntype_parity(self) -> None:
        from mypy.patterns import AsPattern, OrPattern, Pattern

        e1 = self._capture("x")
        e2 = self._capture("x")
        pats: list[Pattern] = [AsPattern(None, e1), AsPattern(None, e2)]

        def run() -> Any:
            pc = self._pc()
            pc.chk = SimpleNamespace(
                named_type=self._named_type,
                named_generic_type=self._named_generic_type,
                type_is_iterable=self._type_is_iterable,
                conditional_types_with_intersection=(lambda t, ranges, ctx, default: (t, default)),
            )
            pc.type_context.append(self.fx.a)
            try:
                return pc.visit_or_pattern(OrPattern(pats))
            finally:
                pc.type_context.pop()

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert_equal(self._triple(on), self._triple(off), "or-visit triple parity")
        # Both alternatives capture x: the union is inhabited.
        assert str(on.type) == "A"

    def test_value_visit_patterntype(self) -> None:
        # visit_value_pattern (checkpattern.py:308-320) reads no checkpattern
        # gate: its PatternType comes from the stubbed accept and narrow
        # helpers, so a gate differential cannot fail; the triple is pinned.
        from mypy.patterns import ValuePattern

        lit = self._capture("v")

        def run() -> Any:
            pc = self._pc()
            pc.chk = SimpleNamespace(
                named_type=self._named_type,
                named_generic_type=self._named_generic_type,
                type_is_iterable=self._type_is_iterable,
                expr_checker=SimpleNamespace(accept=lambda e: self.fx.a),
                narrow_type_by_identity_equality=lambda *a, **k: ({}, {}),
            )
            pc.type_context.append(self.fx.a)
            try:
                return pc.visit_value_pattern(ValuePattern(lit))
            finally:
                pc.type_context.pop()

        assert_equal(self._triple(run()), ("A", "A", []))

    def test_singleton_visit_patterntype_parity(self) -> None:
        from mypy.patterns import SingletonPattern

        def run() -> Any:
            pc = self._pc()
            pc.chk = SimpleNamespace(
                named_type=self._named_type,
                named_generic_type=self._named_generic_type,
                type_is_iterable=self._type_is_iterable,
                expr_checker=SimpleNamespace(infer_literal_expr_type=lambda v, n: self.fx.a),
                conditional_types_with_intersection=(lambda t, ranges, ctx, default: (t, default)),
            )
            pc.type_context.append(self.fx.a)
            try:
                return pc.visit_singleton_pattern(SingletonPattern(True))
            finally:
                pc.type_context.pop()

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert_equal(self._triple(on), self._triple(off), "singleton-visit triple parity")
        assert_equal(self._triple(on), ("A", "A", []))


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeStmtDriverSuite(Suite):
    """Parity for `rust_classify_range_int_gate` and
    `rust_classify_match_subject_head` (issue #1634).

    Two statement-driver decision heads ported as live-PyO3-object
    classifiers (zero wire bytes):

    * `rust_classify_range_int_gate` mirrors the 5-part entry gate of
      `TypeChecker.analyze_range_native_int_type` (checker.py:7639-7644):
      CallExpr + RefExpr callee + fullname == "builtins.range" + 1-3
      args + all ARG_POS. Returns 1 if gate passes, 0 if fails, None
      on unreadable attribute.

    * `rust_classify_match_subject_head` mirrors the 3-way head of
      `TypeChecker._make_named_statement_for_match` (checker.py:8097-8111):
      `can_put_directly(subject)` (isinstance + `literal() > LITERAL_NO`)
      then `subject_dummy is not None` then create-dummy. Returns 0 =
      DIRECT, 1 = HAS_DUMMY, 2 = MAKE_DUMMY, None = defer.

    Direct seam calls assert the expected tag; gate-off vs gate-on
    differentials drive the real TypeChecker methods and must agree.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active

        self.fx = TypeFixture()
        self._set_active = _set_native_checker_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    # ------------------------------------------------------------------
    # range-int-gate direct seam
    # ------------------------------------------------------------------

    def _seam_range(self, expr: Any) -> Any:
        return _type_kernel.rust_classify_range_int_gate(expr)

    def _call_expr(
        self, fullname: str | None, args: list[Any], kinds: list[Any] | None = None
    ) -> Any:
        from mypy.nodes import ARG_POS, CallExpr

        callee = RefExpr()
        if fullname is not None:
            callee.fullname = fullname
        if kinds is None:
            kinds = [ARG_POS] * len(args)
        ce = CallExpr(callee, args, kinds, [None] * len(args))
        return ce

    def test_seam_range_not_call_expr(self) -> None:
        from mypy.nodes import IntExpr

        assert self._seam_range(IntExpr(1)) == 0

    def test_seam_range_callee_not_ref(self) -> None:
        from mypy.nodes import CallExpr, IntExpr

        ce = CallExpr(IntExpr(1), [], [], [])
        assert self._seam_range(ce) == 0

    def test_seam_range_wrong_fullname(self) -> None:
        from mypy.nodes import IntExpr

        ce = self._call_expr("builtins.len", [IntExpr(0)])
        assert self._seam_range(ce) == 0

    def test_seam_range_zero_args(self) -> None:
        ce = self._call_expr("builtins.range", [])
        assert self._seam_range(ce) == 0

    def test_seam_range_four_args(self) -> None:
        from mypy.nodes import IntExpr

        ce = self._call_expr("builtins.range", [IntExpr(0), IntExpr(1), IntExpr(2), IntExpr(3)])
        assert self._seam_range(ce) == 0

    def test_seam_range_one_arg_pos(self) -> None:
        from mypy.nodes import IntExpr

        ce = self._call_expr("builtins.range", [IntExpr(0)])
        assert self._seam_range(ce) == 1

    def test_seam_range_three_args_pos(self) -> None:
        from mypy.nodes import IntExpr

        ce = self._call_expr("builtins.range", [IntExpr(0), IntExpr(1), IntExpr(2)])
        assert self._seam_range(ce) == 1

    def test_seam_range_non_positional(self) -> None:
        from mypy.nodes import IntExpr

        ce = self._call_expr("builtins.range", [IntExpr(0)], kinds=[2])
        assert self._seam_range(ce) == 0

    def test_seam_range_none_fullname(self) -> None:
        from mypy.nodes import ARG_POS, CallExpr, IntExpr

        callee = RefExpr()
        ce = CallExpr(callee, [IntExpr(0)], [ARG_POS], [None])
        assert self._seam_range(ce) == 0

    # ------------------------------------------------------------------
    # range-int-gate parity through real method
    # ------------------------------------------------------------------

    def _run_range(self, expr: Any) -> Type | None:
        from mypy.checker import TypeChecker
        from mypy.nodes import CallExpr
        from mypy.types import AnyType, TypeOfAny

        def check_one() -> Type | None:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()
            chk.msg = None  # type: ignore[assignment]
            type_map: dict[Any, Type] = {}
            if isinstance(expr, CallExpr):
                for arg in expr.args:
                    type_map[arg] = AnyType(TypeOfAny.special_form)
            chk._type_maps = [type_map]
            return chk.analyze_range_native_int_type(expr)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(on, off, f"analyze_range_native_int_type parity for {expr!r}")
        return on

    def test_parity_range_not_range(self) -> None:
        from mypy.nodes import IntExpr

        assert self._run_range(IntExpr(1)) is None

    def test_parity_range_one_arg_no_native_int(self) -> None:
        from mypy.nodes import IntExpr

        ce = self._call_expr("builtins.range", [IntExpr(0)])
        assert self._run_range(ce) is None

    def test_parity_range_non_positional(self) -> None:
        from mypy.nodes import IntExpr

        ce = self._call_expr("builtins.range", [IntExpr(0)], kinds=[2])
        assert self._run_range(ce) is None

    def test_parity_range_wrong_fullname(self) -> None:
        from mypy.nodes import IntExpr

        ce = self._call_expr("builtins.len", [IntExpr(0)])
        assert self._run_range(ce) is None

    # ------------------------------------------------------------------
    # match-subject-head direct seam
    # ------------------------------------------------------------------

    def _seam_match(self, subject: Any, subject_dummy_is_none: bool) -> Any:
        return _type_kernel.rust_classify_match_subject_head(subject, subject_dummy_is_none)

    def test_seam_match_nameexpr_literal_yes(self) -> None:
        from mypy.nodes import NameExpr, Var

        v = Var("x")
        v.is_final = True
        v.final_value = 1
        n = NameExpr("x")
        n.node = v
        assert self._seam_match(n, True) == 0

    def test_seam_match_nameexpr_literal_type(self) -> None:
        from mypy.nodes import NameExpr, Var

        v = Var("x")
        v.is_final = False
        n = NameExpr("x")
        n.node = v
        assert self._seam_match(n, True) == 0

    def test_seam_match_nameexpr_no_node(self) -> None:
        from mypy.nodes import NameExpr

        n = NameExpr("x")
        n.node = None
        assert self._seam_match(n, True) == 0

    def test_seam_match_memberexpr_literal_yes(self) -> None:
        from mypy.nodes import MemberExpr, NameExpr, Var

        v = Var("x")
        v.is_final = True
        v.final_value = 1
        n = NameExpr("x")
        n.node = v
        m = MemberExpr(n, "attr")
        assert self._seam_match(m, True) == 0

    def test_seam_match_indexexpr_literal_yes(self) -> None:
        from mypy.nodes import IndexExpr, NameExpr, Var

        v = Var("x")
        v.is_final = True
        v.final_value = 1
        base = NameExpr("x")
        base.node = v
        idx = IntExpr(0)
        ie = IndexExpr(base, idx)
        assert self._seam_match(ie, True) == 0

    def test_seam_match_indexexpr_non_literal_index(self) -> None:
        from mypy.nodes import CallExpr, IndexExpr, NameExpr

        base = NameExpr("x")
        base.node = None
        idx = CallExpr(RefExpr(), [], [], [])
        ie = IndexExpr(base, idx)
        assert self._seam_match(ie, True) == 2

    def test_seam_match_non_direct_expr(self) -> None:
        from mypy.nodes import CallExpr

        ce = CallExpr(RefExpr(), [], [], [])
        assert self._seam_match(ce, True) == 2

    def test_seam_match_has_dummy(self) -> None:
        from mypy.nodes import CallExpr

        ce = CallExpr(RefExpr(), [], [], [])
        assert self._seam_match(ce, False) == 1

    def test_seam_match_intexpr_make_dummy(self) -> None:
        from mypy.nodes import IntExpr

        assert self._seam_match(IntExpr(1), True) == 2

    # ------------------------------------------------------------------
    # match-subject-head parity through real method
    # ------------------------------------------------------------------

    def _run_match(self, subject: Any, has_dummy: bool) -> Any:
        from mypy.checker import TypeChecker
        from mypy.nodes import MatchStmt

        def check_one() -> Any:
            chk = TypeChecker.__new__(TypeChecker)
            chk.options = Options()

            class _Binder:
                @staticmethod
                def can_put_directly(expr: Any) -> bool:
                    from mypy.literals import literal
                    from mypy.nodes import LITERAL_NO, IndexExpr, MemberExpr, NameExpr

                    return (
                        isinstance(expr, (IndexExpr, MemberExpr, NameExpr))
                        and literal(expr) > LITERAL_NO
                    )

            chk.binder = _Binder()  # type: ignore[assignment]
            chk._unique_dummy_names = {}  # type: ignore[attr-defined]
            chk._unique_id = 0
            s = MatchStmt(subject, [], [], [])
            if has_dummy:
                from mypy.nodes import NameExpr, Var

                dummy = NameExpr("_match_dummy")
                dummy.node = Var("_match_dummy")
                s.subject_dummy = dummy
            return chk._make_named_statement_for_match(s, subject)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(
            type(on).__name__,
            type(off).__name__,
            f"_make_named_statement_for_match parity: on={on!r} off={off!r}",
        )
        return on

    def test_parity_match_nameexpr_direct(self) -> None:
        from mypy.nodes import NameExpr, Var

        v = Var("x")
        v.is_final = True
        v.final_value = 1
        n = NameExpr("x")
        n.node = v
        result = self._run_match(n, has_dummy=False)
        assert result is n

    def test_parity_match_memberexpr_direct(self) -> None:
        from mypy.nodes import MemberExpr, NameExpr, Var

        v = Var("x")
        v.is_final = True
        v.final_value = 1
        base = NameExpr("x")
        base.node = v
        m = MemberExpr(base, "attr")
        result = self._run_match(m, has_dummy=False)
        assert result is m

    def test_parity_match_non_direct_make_dummy(self) -> None:
        from mypy.nodes import CallExpr

        ce = CallExpr(RefExpr(), [], [], [])
        result = self._run_match(ce, has_dummy=False)
        assert type(result).__name__ == "NameExpr"

    def test_parity_match_non_direct_has_dummy(self) -> None:
        from mypy.nodes import CallExpr

        ce = CallExpr(RefExpr(), [], [], [])
        result = self._run_match(ce, has_dummy=True)
        assert type(result).__name__ == "NameExpr"

    def test_parity_match_intexpr_make_dummy(self) -> None:
        from mypy.nodes import IntExpr

        result = self._run_match(IntExpr(1), has_dummy=False)
        assert type(result).__name__ == "NameExpr"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeShouldReportUnreachableIssuesSuite(Suite):
    """Parity for `rust_should_report_unreachable_issues` (H1d, #1672).

    `TypeChecker.should_report_unreachable_issues` (checker.py:4702) is a
    conjunction over `in_checked_function` (checker.py:10235) plus
    `options.warn_unreachable`, `current_node_deferred`, and the binder's
    suppression flag. The Rust port reads the live checker and
    short-circuits in the same order, so `None` (defer) is reachable only
    through an unreadable attribute.
    """

    def setUp(self) -> None:
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _set_active(self, active: bool) -> None:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)

    def _with_gate(self, active: bool, fn: Callable[[], bool]) -> bool:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _stub(
        self,
        *,
        check_untyped_defs: bool = False,
        dynamic_funcs: list[bool] | None = None,
        warn_unreachable: bool = True,
        deferred: bool = False,
        suppressed: bool = False,
    ) -> _H1dCheckerStub:
        options = Options()
        options.check_untyped_defs = check_untyped_defs
        options.warn_unreachable = warn_unreachable
        return _H1dCheckerStub(
            options,
            [] if dynamic_funcs is None else dynamic_funcs,
            deferred,
            _H1dBinderStub(suppressed),
        )

    def _seam(self, chk: Any) -> bool | None:
        return _type_kernel.rust_should_report_unreachable_issues(chk)

    def _run(self, chk: Any) -> tuple[bool, bool]:
        from mypy.checker import TypeChecker

        def check_one() -> bool:
            real = TypeChecker.__new__(TypeChecker)
            real.options = chk.options
            real.dynamic_funcs = chk.dynamic_funcs
            real.current_node_deferred = chk.current_node_deferred
            real.binder = chk.binder
            return real.should_report_unreachable_issues()

        return self._with_gate(False, check_one), self._with_gate(True, check_one)

    def _assert_par(self, chk: Any) -> None:
        off, on = self._run(chk)
        assert_equal(on, off, "should_report_unreachable_issues parity")

    # --- Direct seam calls ---

    def test_seam_check_untyped_defs_but_warn_off(self) -> None:
        """check_untyped_defs makes in_checked_function True; warn off -> False."""
        chk = self._stub(check_untyped_defs=True, dynamic_funcs=[True], warn_unreachable=False)
        assert self._seam(chk) is False

    def test_seam_empty_dynamic_funcs(self) -> None:
        """`not []` is True, so an empty stack is in a checked function."""
        assert self._seam(self._stub(dynamic_funcs=[])) is True

    def test_seam_last_dynamic_func_false(self) -> None:
        """`not dynamic_funcs[-1]` is True for a False top of stack."""
        assert self._seam(self._stub(dynamic_funcs=[True, False])) is True

    def test_seam_last_dynamic_func_true(self) -> None:
        """`not dynamic_funcs[-1]` is False for a True top of stack."""
        assert self._seam(self._stub(dynamic_funcs=[True])) is False

    def test_seam_node_deferred(self) -> None:
        """`current_node_deferred` short-circuits to False."""
        assert self._seam(self._stub(deferred=True)) is False

    def test_seam_warnings_suppressed(self) -> None:
        """A suppressing binder frame short-circuits to False."""
        assert self._seam(self._stub(suppressed=True)) is False

    def test_seam_unreadable_checker_defers(self) -> None:
        """An object without `options` defers rather than raising."""
        assert self._seam(object()) is None

    # --- Gate-off vs gate-on parity ---

    def test_parity_warn_off(self) -> None:
        self._assert_par(self._stub(check_untyped_defs=True, warn_unreachable=False))

    def test_parity_empty_dynamic_funcs(self) -> None:
        self._assert_par(self._stub(dynamic_funcs=[]))

    def test_parity_last_dynamic_func_false(self) -> None:
        self._assert_par(self._stub(dynamic_funcs=[True, False]))

    def test_parity_last_dynamic_func_true(self) -> None:
        self._assert_par(self._stub(dynamic_funcs=[True]))

    def test_parity_node_deferred(self) -> None:
        self._assert_par(self._stub(deferred=True))

    def test_parity_warnings_suppressed(self) -> None:
        self._assert_par(self._stub(suppressed=True))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRefersToDifferentScopeSuite(Suite):
    """Parity for `rust_refers_to_different_scope` (H1d, #1672).

    `TypeChecker.refers_to_different_scope` (checker.py:6762) is a pure
    bool over the live `NameExpr.kind`, the live `Scope`, and the enclosing
    `MypyFile.fullname`. `RefExpr.kind` values are LDEF=0, GDEF=1, MDEF=2
    (mypy/nodes.py:215-217).
    """

    def setUp(self) -> None:
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _set_active(self, active: bool) -> None:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)

    def _with_gate(self, active: bool, fn: Callable[[], bool]) -> bool:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _name(self, kind: int | None, fullname: str = "x", *, readable: bool = True) -> Any:
        if not readable:
            return object()
        name = NameExpr("x")
        name.kind = kind
        name._fullname = fullname
        return name

    def _checker(self, module_name: str, *, in_function: bool) -> _H1dScopeCheckerStub:
        from mypy.checker_shared import CheckerScope

        scope = CheckerScope(_h1d_module(module_name))
        if in_function:
            scope.stack.append(FuncDef("f", [], Block([])))
        return _H1dScopeCheckerStub(scope, _h1d_module(module_name))

    def _seam(self, name: Any, chk: _H1dScopeCheckerStub) -> bool | None:
        return _type_kernel.rust_refers_to_different_scope(name, chk.scope, chk.tree)

    def _run(self, name: Any, chk: _H1dScopeCheckerStub) -> tuple[bool, bool]:
        from mypy.checker import TypeChecker

        def check_one() -> bool:
            real = TypeChecker.__new__(TypeChecker)
            real.scope = chk.scope
            real.tree = chk.tree
            return real.refers_to_different_scope(name)

        return self._with_gate(False, check_one), self._with_gate(True, check_one)

    def _assert_par(self, name: Any, chk: _H1dScopeCheckerStub) -> None:
        off, on = self._run(name, chk)
        assert_equal(on, off, "refers_to_different_scope parity")

    # --- Direct seam calls ---

    def test_seam_ldef_is_local(self) -> None:
        """LDEF is never a different scope, even inside a function."""
        chk = self._checker("m", in_function=True)
        assert self._seam(self._name(0), chk) is False

    def test_seam_unbound_kind_in_function(self) -> None:
        """`kind is None` inside a function is a different scope."""
        chk = self._checker("m", in_function=True)
        assert self._seam(self._name(None), chk) is True

    def test_seam_mdef_in_function(self) -> None:
        """MDEF inside a function is a different scope."""
        chk = self._checker("m", in_function=True)
        assert self._seam(self._name(2), chk) is True

    def test_seam_unbound_kind_outside_function(self) -> None:
        """`kind is None` at module scope is not a different scope."""
        chk = self._checker("m", in_function=False)
        assert self._seam(self._name(None), chk) is False

    def test_seam_gdef_other_module(self) -> None:
        """GDEF whose parent module differs from the tree is foreign."""
        chk = self._checker("m", in_function=True)
        assert self._seam(self._name(1, "other.mod.x"), chk) is True

    def test_seam_gdef_same_module(self) -> None:
        """GDEF whose parent module is the tree is local."""
        chk = self._checker("m", in_function=True)
        assert self._seam(self._name(1, "m.x"), chk) is False

    def test_seam_gdef_no_dot_matches_empty_tree(self) -> None:
        """`rpartition(".")[0]` is "" for a dot-free fullname."""
        chk = self._checker("", in_function=False)
        assert self._seam(self._name(1, "x"), chk) is False

    def test_seam_gdef_outside_function(self) -> None:
        """A module-scope GDEF reads its own module as the scope."""
        chk = self._checker("m", in_function=False)
        assert self._seam(self._name(1, "m.x"), chk) is False

    def test_seam_unreadable_name_defers(self) -> None:
        """A name without `kind` defers rather than raising."""
        chk = self._checker("m", in_function=True)
        assert self._seam(self._name(0, readable=False), chk) is None

    # --- Gate-off vs gate-on parity ---

    def test_parity_ldef_is_local(self) -> None:
        self._assert_par(self._name(0), self._checker("m", in_function=True))

    def test_parity_unbound_kind_in_function(self) -> None:
        self._assert_par(self._name(None), self._checker("m", in_function=True))

    def test_parity_mdef_in_function(self) -> None:
        self._assert_par(self._name(2), self._checker("m", in_function=True))

    def test_parity_gdef_other_module(self) -> None:
        self._assert_par(self._name(1, "other.mod.x"), self._checker("m", in_function=True))

    def test_parity_gdef_same_module(self) -> None:
        self._assert_par(self._name(1, "m.x"), self._checker("m", in_function=True))

    def test_parity_gdef_outside_function(self) -> None:
        self._assert_par(self._name(1, "m.x"), self._checker("m", in_function=False))

    def test_parity_unbound_kind_outside_function(self) -> None:
        self._assert_par(self._name(None), self._checker("m", in_function=False))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFlattenLvaluesSuite(Suite):
    """Parity for `rust_flatten_lvalues` (H1d, #1672).

    `TypeChecker.flatten_lvalues` (checker.py:5947) is a pure recursive read
    over the live lvalue expressions. The Python body is not `elif`-chained:
    a `TupleExpr` / `ListExpr` contributes its flattened `items` *and*
    itself, and a `StarExpr` is appended unwrapped.
    """

    def setUp(self) -> None:
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _set_active(self, active: bool) -> None:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)

    def _with_gate(self, active: bool, fn: Callable[[], list[Expression]]) -> list[Expression]:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _seam(self, lvalues: Any) -> list[Expression] | None:
        return _type_kernel.rust_flatten_lvalues(lvalues)

    def _run(self, lvalues: list[Expression]) -> tuple[list[str], list[str]]:
        from mypy.checker import TypeChecker

        def check_one() -> list[Expression]:
            real = TypeChecker.__new__(TypeChecker)
            return real.flatten_lvalues(lvalues)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return _h1d_names(off), _h1d_names(on)

    def _assert_par(self, lvalues: list[Expression]) -> None:
        off, on = self._run(lvalues)
        assert_equal(on, off, "flatten_lvalues parity")
        seam = self._seam(lvalues)
        assert seam is not None
        assert_equal(_h1d_names(seam), off, "flatten_lvalues seam parity")

    # --- Direct seam calls ---

    def test_seam_empty(self) -> None:
        assert self._seam([]) == []

    def test_seam_flat_names_preserve_identity(self) -> None:
        a, b = NameExpr("a"), NameExpr("b")
        assert self._seam([a, b]) == [a, b]

    def test_seam_tuple_keeps_itself_and_unwraps_items(self) -> None:
        a, b = NameExpr("a"), NameExpr("b")
        tup = TupleExpr([a, b])
        assert self._seam([tup]) == [a, b, tup]

    def test_seam_list_expr_keeps_itself(self) -> None:
        a = NameExpr("a")
        lst = ListExpr([a])
        assert self._seam([lst]) == [a, lst]

    def test_seam_nested_tuple_order(self) -> None:
        a, b, c = NameExpr("a"), NameExpr("b"), NameExpr("c")
        inner = TupleExpr([a, b])
        outer = TupleExpr([inner, c])
        assert self._seam([outer]) == [a, b, inner, c, outer]

    def test_seam_star_expr_is_unwrapped(self) -> None:
        a = NameExpr("a")
        star = StarExpr(a)
        assert self._seam([star]) == [a]

    def test_seam_star_expr_inside_tuple(self) -> None:
        a, b = NameExpr("a"), NameExpr("b")
        star = StarExpr(a)
        tup = TupleExpr([star, b])
        assert self._seam([tup]) == [a, b, tup]

    def test_seam_unreadable_input_defers(self) -> None:
        """A non-sequence input defers rather than raising."""
        assert self._seam(42) is None

    # --- Gate-off vs gate-on parity ---

    def test_parity_empty(self) -> None:
        self._assert_par([])

    def test_parity_flat_names(self) -> None:
        self._assert_par([NameExpr("a"), NameExpr("b")])

    def test_parity_tuple(self) -> None:
        self._assert_par([TupleExpr([NameExpr("a"), NameExpr("b")])])

    def test_parity_list_expr(self) -> None:
        self._assert_par([ListExpr([NameExpr("a")])])

    def test_parity_nested_tuple(self) -> None:
        inner = TupleExpr([NameExpr("a"), NameExpr("b")])
        self._assert_par([TupleExpr([inner, NameExpr("c")])])

    def test_parity_star_expr(self) -> None:
        self._assert_par([StarExpr(NameExpr("a"))])

    def test_parity_star_expr_inside_tuple(self) -> None:
        tup = TupleExpr([StarExpr(NameExpr("a")), NameExpr("b")])
        self._assert_par([tup])


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeLiteralIntExprSuite(Suite):
    """Parity for `rust_literal_int_expr` (H1d, #1672).

    `TypeChecker.literal_int_expr` (checker.py:9772) scans the live
    `_type_maps` stack (`has_type` + `lookup_type`, innermost map first),
    coerces to a literal, and returns the value only when the proper type is
    a `LiteralType` whose `value` passes `isinstance(v, int)` (which accepts
    `bool`). The seam returns `(flag, value)`; flag 1 means literal int.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _set_active(self, active: bool) -> None:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)

    def _with_gate(self, active: bool, fn: Callable[[], int | None]) -> int | None:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _literal(self, value: Any) -> LiteralType:
        return LiteralType(value, self.fx.a)

    def _seam(self, type_maps: Any, expr: Any) -> tuple[int, Any] | None:
        return _type_kernel.rust_literal_int_expr(type_maps, expr)

    def _run(self, type_maps: Any, expr: Any) -> tuple[int | None, int | None]:
        from mypy.checker import TypeChecker

        def check_one() -> int | None:
            real = TypeChecker.__new__(TypeChecker)
            real._type_maps = type_maps
            return real.literal_int_expr(expr)

        return self._with_gate(False, check_one), self._with_gate(True, check_one)

    def _assert_par(self, type_maps: Any, expr: Any) -> None:
        off, on = self._run(type_maps, expr)
        assert_equal(on, off, "literal_int_expr parity")
        seam = self._seam(type_maps, expr)
        assert seam is not None
        flag, value = seam
        assert_equal(value if flag else None, off, "literal_int_expr seam parity")

    # --- Direct seam calls ---

    def test_seam_no_type_maps(self) -> None:
        assert self._seam([], NameExpr("x")) == (0, None)

    def test_seam_absent_expression(self) -> None:
        expr = NameExpr("x")
        assert self._seam([{}], expr) == (0, None)

    def test_seam_literal_int(self) -> None:
        expr = NameExpr("x")
        assert self._seam([{expr: self._literal(3)}], expr) == (1, 3)

    def test_seam_literal_bool_counts_as_int(self) -> None:
        """`isinstance(True, int)` is True, so a bool literal is a literal int."""
        expr = NameExpr("x")
        flag, value = self._seam([{expr: self._literal(True)}], expr) or (0, None)
        assert flag == 1
        assert value is True

    def test_seam_literal_str_is_not_int(self) -> None:
        expr = NameExpr("x")
        assert self._seam([{expr: self._literal("s")}], expr) == (0, None)

    def test_seam_last_known_value_instance(self) -> None:
        """`coerce_to_literal` turns a last-known-value Instance into a literal."""
        expr = NameExpr("x")
        inst = Instance(self.fx.a.type, [], last_known_value=self._literal(5))
        assert self._seam([{expr: inst}], expr) == (1, 5)

    def test_seam_plain_instance(self) -> None:
        expr = NameExpr("x")
        assert self._seam([{expr: self.fx.a}], expr) == (0, None)

    def test_seam_any_type(self) -> None:
        expr = NameExpr("x")
        assert self._seam([{expr: self.fx.anyt}], expr) == (0, None)

    def test_seam_union_defers(self) -> None:
        """A union is not read live; the seam defers to the Python body."""
        expr = NameExpr("x")
        union = UnionType.make_union([self._literal(3), self.fx.anyt])
        assert self._seam([{expr: union}], expr) is None

    def test_seam_single_item_union_defers(self) -> None:
        """A one-item union can collapse to a literal, so it must defer."""
        expr = NameExpr("x")
        assert self._seam([{expr: UnionType([self._literal(3)])}], expr) is None

    def _alias_type(self) -> TypeAliasType:
        """`TypeAliasType` wrapping an int literal, for the deferral test."""
        alias = TypeAlias(self._literal(3), "mod.A", "mod", 1, 1)
        return TypeAliasType(alias, [])

    def test_seam_type_alias_defers(self) -> None:
        """`get_proper_type` is not identity for an alias, so the seam defers."""
        expr = NameExpr("x")
        assert self._seam([{expr: self._alias_type()}], expr) is None

    def test_parity_type_alias_defers(self) -> None:
        """The alias still resolves to the int through the Python body."""
        expr = NameExpr("x")
        self._assert_par_deferring([{expr: self._alias_type()}], expr)
        assert self._run([{expr: self._alias_type()}], expr) == (3, 3)

    def test_seam_enum_like_instance_is_not_an_int_literal(self) -> None:
        """An Instance without last-known-value never yields an int literal."""
        expr = NameExpr("x")
        assert self._seam([{expr: self.fx.a}], expr) == (0, None)

    def test_seam_innermost_map_wins(self) -> None:
        """`lookup_type` scans reversed, so the last map shadows outer ones."""
        expr = NameExpr("x")
        maps = [{expr: self._literal(3)}, {expr: self.fx.anyt}]
        assert self._seam(maps, expr) == (0, None)

    def test_seam_older_map_used_when_inner_misses(self) -> None:
        expr = NameExpr("x")
        maps = [{expr: self._literal(7)}, {}]
        assert self._seam(maps, expr) == (1, 7)

    def test_seam_identity_keyed(self) -> None:
        """Two distinct `NameExpr("x")` nodes are different keys."""
        stored, queried = NameExpr("x"), NameExpr("x")
        assert self._seam([{stored: self._literal(9)}], queried) == (0, None)

    def test_seam_big_int_is_exact(self) -> None:
        expr = NameExpr("x")
        big = 10**30
        assert self._seam([{expr: self._literal(big)}], expr) == (1, big)

    def test_seam_unreadable_maps_defers(self) -> None:
        assert self._seam(42, NameExpr("x")) is None

    # --- Gate-off vs gate-on parity ---

    def test_parity_absent_expression(self) -> None:
        self._assert_par([{}], NameExpr("x"))

    def test_parity_literal_int(self) -> None:
        expr = NameExpr("x")
        self._assert_par([{expr: self._literal(3)}], expr)

    def test_parity_literal_bool(self) -> None:
        expr = NameExpr("x")
        self._assert_par([{expr: self._literal(True)}], expr)

    def test_parity_literal_str(self) -> None:
        expr = NameExpr("x")
        self._assert_par([{expr: self._literal("s")}], expr)

    def test_parity_last_known_value_instance(self) -> None:
        expr = NameExpr("x")
        inst = Instance(self.fx.a.type, [], last_known_value=self._literal(5))
        self._assert_par([{expr: inst}], expr)

    def test_parity_plain_instance(self) -> None:
        expr = NameExpr("x")
        self._assert_par([{expr: self.fx.a}], expr)

    def test_parity_innermost_map_wins(self) -> None:
        expr = NameExpr("x")
        self._assert_par([{expr: self._literal(3)}, {expr: self.fx.anyt}], expr)

    def test_parity_older_map_used_when_inner_misses(self) -> None:
        expr = NameExpr("x")
        self._assert_par([{expr: self._literal(7)}, {}], expr)

    def _assert_par_deferring(self, type_maps: Any, expr: Any) -> None:
        """Differential for a head the seam deliberately defers on."""
        off, on = self._run(type_maps, expr)
        assert_equal(on, off, "literal_int_expr deferring parity")
        assert self._seam(type_maps, expr) is None

    def test_parity_union_defers(self) -> None:
        expr = NameExpr("x")
        union = UnionType.make_union([self._literal(3), self.fx.anyt])
        self._assert_par_deferring([{expr: union}], expr)

    def test_parity_single_item_union_defers(self) -> None:
        """The collapsing one-item union still resolves to the int literal."""
        expr = NameExpr("x")
        self._assert_par_deferring([{expr: UnionType([self._literal(3)])}], expr)
        assert self._run([{expr: UnionType([self._literal(3)])}], expr) == (3, 3)


# Moved from mypy/test/testtypes_native_types.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeDetachCallableSuite(Suite):
    """Parity for the Rust `detach_callable` port (mypy.checker).

    `detach_callable` ensures a callable's `variables` include the class type
    variables it uses. The Rust port reads the callable's `variables` and the
    incoming `class_type_vars` list off the wire and returns the concatenated
    per-variable blobs; the Python shim decodes them and rebuilds the
    callable via `copy_modified` on the live object. Toggling the checker
    gate off (pure Python) and on (Rust seam) must produce identical results
    (`str` and `len(variables)`), and a direct seam call proves the Rust
    function engages rather than silently deferring. The empty-`class_type_vars`
    fast path never reaches Rust.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active
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
        self._set_active = _set_native_checker_active
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

    def _callable(self, variables: list[TypeVarType]) -> CallableType:
        return CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.anyt,
            self.fx.function,
            variables=variables,
        )

    def _assert_par(self, typ: CallableType, class_type_vars: list[TypeVarLikeType]) -> None:
        from mypy.checker import detach_callable

        off = self._with_gate(False, lambda: detach_callable(typ, class_type_vars))
        assert isinstance(off, CallableType)
        on = self._with_gate(True, lambda: detach_callable(typ, class_type_vars))
        assert isinstance(on, CallableType)
        assert_equal(str(on), str(off), f"detach_callable str parity {typ}")
        assert_equal(
            len(on.variables), len(off.variables), f"detach_callable len(variables) parity {typ}"
        )

    def _assert_engages(self, typ: CallableType, class_type_vars: list[TypeVarType]) -> None:
        from mypy.checker import _serialize_type_for_checker, _serialize_type_list

        result = _type_kernel.rust_detach_callable(
            _serialize_type_for_checker(typ), _serialize_type_list(class_type_vars)
        )
        assert result is not None, "Rust detach_callable did not engage"

    def test_non_generic_empty_class_vars(self) -> None:
        from mypy.checker import detach_callable

        # Fast path: empty class_type_vars returns typ unchanged, no Rust call.
        c = self._callable([])
        result = self._with_gate(True, lambda: detach_callable(c, []))
        assert isinstance(result, CallableType)
        assert result is c, "empty class_type_vars must return typ unchanged"

    def test_non_generic_class_vars_extended(self) -> None:
        from mypy.checker import detach_callable

        c = self._callable([])
        self._assert_par(c, [self.fx.t])
        on = self._with_gate(True, lambda: detach_callable(c, [self.fx.t]))
        assert isinstance(on, CallableType)
        assert_equal(len(on.variables), 1)
        self._assert_engages(c, [self.fx.t])

    def test_generic_class_extended(self) -> None:
        from mypy.checker import detach_callable

        c = self._callable([self.fx.t])
        self._assert_par(c, [self.fx.s])
        on = self._with_gate(True, lambda: detach_callable(c, [self.fx.s]))
        assert isinstance(on, CallableType)
        assert_equal(len(on.variables), 2)
        self._assert_engages(c, [self.fx.s])

    def test_multiple_class_vars(self) -> None:
        from mypy.checker import detach_callable

        c = self._callable([self.fx.t])
        self._assert_par(c, [self.fx.s, self.fx.u])
        on = self._with_gate(True, lambda: detach_callable(c, [self.fx.s, self.fx.u]))
        assert isinstance(on, CallableType)
        assert_equal(len(on.variables), 3)
        self._assert_engages(c, [self.fx.s, self.fx.u])


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeExpandCallableVariantsSuite(Suite):
    """Parity for the Rust `expand_callable_variants` port (mypy.checker).

    `expand_callable_variants` expands a generic callable over every
    combination of its type variables' values (or upper bound). The Rust
    port reads the callable off the wire, substitutes each combination via
    `expand_type_inner`, and returns the variant list as a wire type-list;
    the Python shim rebuilds each variant on the live callable so non-wire
    fields survive. Toggling the checker gate off (pure Python) and on
    (Rust seam) must produce identical results (`str` and `len`), and a
    direct seam call proves the Rust function engages rather than silently
    deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active
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
        self._set_active = _set_native_checker_active
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

    def _tvar(self, raw_id: int, values: list[Type]) -> TypeVarType:
        return TypeVarType("V", "__main__.V", TypeVarId(raw_id), values, self.fx.o, self.fx.o)

    def _callable(
        self,
        variables: list[TypeVarType],
        arg_types: list[Type] | None = None,
        ret_type: Type | None = None,
    ) -> CallableType:
        args = arg_types if arg_types is not None else [self.fx.a, self.fx.b]
        kinds = [ARG_POS] * len(args)
        return CallableType(
            args,
            kinds,
            [None] * len(args),
            ret_type if ret_type is not None else self.fx.anyt,
            self.fx.function,
            variables=variables,
        )

    def _assert_par(self, c: CallableType) -> None:
        from mypy.checker import expand_callable_variants

        off = self._with_gate(False, lambda: expand_callable_variants(c))
        on = self._with_gate(True, lambda: expand_callable_variants(c))
        assert_equal(len(on), len(off), f"expand_callable_variants len parity {c}")
        assert_equal(
            [str(v) for v in on], [str(v) for v in off], f"expand_callable_variants str parity {c}"
        )

    def _assert_engages(self, c: CallableType) -> None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_expand_callable_variants(
            _serialize_type_for_checker(c), state.strict_optional
        )
        assert result is not None, "Rust expand_callable_variants did not engage"

    def test_non_generic_fast_path(self) -> None:
        from mypy.checker import expand_callable_variants

        c = self._callable([])
        self._assert_par(c)
        on = self._with_gate(True, lambda: expand_callable_variants(c))
        assert_equal(len(on), 1)
        self._assert_engages(c)

    def test_plain_generic_two_vars(self) -> None:
        from mypy.checker import expand_callable_variants

        # Both vars are value-less, so each substitutes its upper bound (o).
        c = self._callable(
            [self.fx.t, self.fx.s], arg_types=[self.fx.gt, self.fx.t], ret_type=self.fx.t
        )
        self._assert_par(c)
        on = self._with_gate(True, lambda: expand_callable_variants(c))
        assert_equal(len(on), 1)
        self._assert_engages(c)

    def test_single_tvar_with_values(self) -> None:
        from mypy.checker import expand_callable_variants

        # V in {A, B} yields two variants.
        v = self._tvar(10, [self.fx.a, self.fx.b])
        c = self._callable([v], arg_types=[v], ret_type=self.fx.a)
        self._assert_par(c)
        on = self._with_gate(True, lambda: expand_callable_variants(c))
        assert_equal(len(on), 2)
        self._assert_engages(c)

    def test_two_tvars_product(self) -> None:
        from mypy.checker import expand_callable_variants

        # U in {C}, V in {A, B}: two variants over the cartesian product.
        u = self._tvar(11, [self.fx.c])
        v = self._tvar(12, [self.fx.a, self.fx.b])
        c = self._callable([u, v], arg_types=[u, v], ret_type=self.fx.a)
        self._assert_par(c)
        on = self._with_gate(True, lambda: expand_callable_variants(c))
        assert_equal(len(on), 2)
        self._assert_engages(c)

    def test_self_type_with_other_var(self) -> None:
        from mypy.checker import expand_callable_variants

        # Self (raw_id 0) expands to its upper bound A, plus T substitutes o.
        self_var = TypeVarType("Self", "__main__.Self", TypeVarId(0), [], self.fx.a, self.fx.o)
        c = self._callable(
            [self_var, self.fx.t], arg_types=[self_var, self.fx.gt], ret_type=self.fx.a
        )
        self._assert_par(c)
        on = self._with_gate(True, lambda: expand_callable_variants(c))
        assert_equal(len(on), 1)
        self._assert_engages(c)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeBuiltinItemTypeSuite(Suite):
    """Parity for the Rust `builtin_item_type` port (mypy.checker).

    `builtin_item_type` extracts the element type of a builtin container
    (list, dict, set, frozenset, dict_keys, KeysView, Tuple, TypedDict)
    so the checker can narrow optional types in `x in (...)`. The Rust
    port speaks the #1101 decided-None protocol: positive cases return
    the element type, shapes whose Python answer is None (non-container
    instances, unparameterized containers, Any first args, tuples with
    Any items, TypedDicts without a Mapping base, other types) return a
    decided no-result instead of deferring. Only wire-unsupported shapes
    (TypeAliasType, non-tuple unpacks, missing snapshots, encode/decode
    failures) defer to the pure-Python path. Toggling the checker gate
    off (Python) and on (Rust) must produce identical results; a direct
    seam call proves the Rust function engages rather than silently
    deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.int_info = self.fx.make_type_info("builtins.int")
        self.mapping_info = self.fx.make_type_info(
            "typing.Mapping",
            mro=[self.fx.str_type_info, self.fx.oi],
            typevars=["K", "V"],
            variances=[CONTRAVARIANT, COVARIANT],
            bases=[self.fx.str_type, self.fx.o],
        )
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        # str_type_info does not end in "i"; snapshot it explicitly (the
        # TypedDict/Mapping cases need it for wire fixup).
        type_infos.extend([self.fx.str_type_info, self.int_info, self.mapping_info])
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_checker_active(True)
        _set_native_checker_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def _assert_par(self, t: Type) -> None:
        from mypy.checker import builtin_item_type

        off = self._with_gate(False, lambda: builtin_item_type(t))
        on = self._with_gate(True, lambda: builtin_item_type(t))
        assert_equal(str(on), str(off), f"builtin_item_type parity {t}")

    def _assert_engages(self, t: Type) -> None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_builtin_item_type(
            _serialize_type_for_checker(t), state.strict_optional, self.resolver
        )
        assert result is not None, f"Rust builtin_item_type did not engage for {t}"
        decided, _value = result
        assert decided, f"Rust builtin_item_type deferred (undecided) for {t}"

    def _assert_decided_none(self, t: Type) -> None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_builtin_item_type(
            _serialize_type_for_checker(t), state.strict_optional, self.resolver
        )
        assert result is not None, f"Rust builtin_item_type did not engage for {t}"
        decided, value = result
        assert decided and value is None, f"expected decided-None for {t}, got {result!r}"

    def test_list_instance(self) -> None:
        from mypy.checker import builtin_item_type

        t = Instance(self.fx.std_listi, [Instance(self.int_info, [])])
        self._assert_par(t)
        result = self._with_gate(True, lambda: builtin_item_type(t))
        assert_equal(str(result), "builtins.int")
        self._assert_engages(t)

    def test_unparameterized_container(self) -> None:
        from mypy.checker import builtin_item_type

        t = Instance(self.fx.std_listi, [])
        self._assert_par(t)
        assert self._with_gate(True, lambda: builtin_item_type(t)) is None
        self._assert_decided_none(t)

    def test_any_first_arg_decided_none(self) -> None:
        # Rust decides Any first arg as None (Python's answer), not a defer.
        from mypy.checker import builtin_item_type

        t = Instance(self.fx.std_listi, [self.fx.anyt])
        self._assert_par(t)
        assert self._with_gate(True, lambda: builtin_item_type(t)) is None
        self._assert_decided_none(t)

    def test_non_container_instance(self) -> None:
        from mypy.checker import builtin_item_type

        t = Instance(self.fx.ai, [])
        self._assert_par(t)
        assert builtin_item_type(t) is None
        self._assert_decided_none(t)

    def test_tuple_items(self) -> None:
        from mypy.checker import builtin_item_type

        t = TupleType([Instance(self.int_info, []), self.fx.str_type], self.fx.std_tuple)
        self._assert_par(t)
        result = self._with_gate(True, lambda: builtin_item_type(t))
        assert_equal(str(result), "builtins.int | builtins.str")
        self._assert_engages(t)

    def test_tuple_unpack(self) -> None:
        # An UnpackType item normalizes through its upper_bound
        # (a builtins.tuple instance); the tuple's item is the arg.
        from mypy.checker import builtin_item_type

        tv = TypeVarTupleType(
            "Ts",
            "Ts",
            TypeVarId(1),
            self.fx.std_tuple.copy_modified(args=[Instance(self.int_info, [])]),
            self.fx.std_tuple.copy_modified(args=[self.fx.o]),
            self.fx.anyt,
        )
        t = TupleType([UnpackType(tv), self.fx.str_type], self.fx.std_tuple)
        self._assert_par(t)
        result = self._with_gate(True, lambda: builtin_item_type(t))
        assert_equal(str(result), "builtins.int | builtins.str")
        self._assert_engages(t)

    def test_typed_dict(self) -> None:
        from mypy.checker import builtin_item_type

        td_fallback = Instance(self.mapping_info, [self.fx.str_type, self.fx.o])
        t = TypedDictType({"x": self.fx.o}, {"x"}, set(), td_fallback)
        self._assert_par(t)
        result = self._with_gate(True, lambda: builtin_item_type(t))
        assert_equal(str(result), "builtins.str")
        self._assert_engages(t)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeBuiltinItemAliasSuite(Suite):
    """Wave-61B: alias expansion for `builtin_item_type` (#1512).

    The wave-61 audit pinned all 17 cold-self-check fallbacks to alias
    first args / tuple items. Python checks `get_proper_type(args[0])`
    for Any and returns the arg; the only consumer immediately calls
    `get_proper_type` on the result, so expanding the alias in Rust is
    the same result through the public interface. The suite compares
    `get_proper_type(result)` gate-off vs gate-on and asserts the direct
    seam decides instead of deferring.
    """

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.nodes import TypeAlias
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._type_infos = _base_infos(self.fx)
        self.alias = TypeAlias(self.fx.a, "mod.ItemAlias", "mod", -1, -1)
        self.any_alias = TypeAlias(AnyType(TypeOfAny.explicit), "mod.AnyItemAlias", "mod", -1, -1)
        self._resolver = _type_kernel.build_native_resolver(
            self._type_infos, [self.alias, self.any_alias]
        )
        _set_native_checker_active(True)
        _set_native_checker_resolver(self._resolver)
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})

    def tearDown(self) -> None:
        from mypy.checker import _set_native_checker_active, _set_native_checker_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checker_active(False)
        _set_native_checker_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checker import _set_native_checker_active

        _set_native_checker_active(active)
        try:
            return fn()
        finally:
            _set_native_checker_active(True)

    def _seam(self, typ: Type) -> tuple[bool, bytes | None] | None:
        from mypy.checker import _serialize_type_for_checker

        result = _type_kernel.rust_builtin_item_type(
            _serialize_type_for_checker(typ), True, self._resolver
        )
        if result is None:
            return None
        decided, value = result
        assert decided
        return decided, None if value is None else bytes(value)

    def _parity_str(self, typ: Type) -> str | None:
        from mypy.checker import builtin_item_type

        off = self._with_gate(False, lambda: builtin_item_type(typ))
        on = self._with_gate(True, lambda: builtin_item_type(typ))
        off_p = None if off is None else get_proper_type(off)
        on_p = None if on is None else get_proper_type(on)
        assert str(on_p) == str(off_p), f"builtin_item_type parity {typ!r}: {off_p!r} vs {on_p!r}"
        return None if off_p is None else str(off_p)

    def test_seam_instance_alias_returns_expanded_target(self) -> None:
        from mypy.checker import _deserialize_type_from_checker

        tp = Instance(self.fx.std_listi, [TypeAliasType(self.alias, [])])
        result = self._seam(tp)
        assert result is not None
        _decided, value = result
        assert value is not None
        decoded = get_proper_type(_deserialize_type_from_checker(value))
        assert isinstance(decoded, Instance)
        assert str(decoded) == str(self.fx.a)

    def test_seam_alias_to_any_decided_none(self) -> None:
        tp = Instance(self.fx.std_listi, [TypeAliasType(self.any_alias, [])])
        result = self._seam(tp)
        assert result == (True, None)

    def test_seam_tuple_alias_item_engages(self) -> None:
        tp = TupleType([TypeAliasType(self.alias, []), self.fx.b], self.fx.std_tuple)
        result = self._seam(tp)
        assert result is not None
        _decided, value = result
        assert value is not None

    def test_seam_defers_without_alias_snapshot(self) -> None:
        from mypy.checker import _serialize_type_for_checker

        empty = _type_kernel.build_native_resolver([], [])
        tp = Instance(self.fx.std_listi, [TypeAliasType(self.alias, [])])
        assert (
            _type_kernel.rust_builtin_item_type(_serialize_type_for_checker(tp), True, empty)
            is None
        )

    def test_gate_parity_instance_alias(self) -> None:
        tp = Instance(self.fx.std_listi, [TypeAliasType(self.alias, [])])
        assert self._parity_str(tp) == str(self.fx.a)

    def test_gate_parity_alias_to_any(self) -> None:
        tp = Instance(self.fx.std_listi, [TypeAliasType(self.any_alias, [])])
        assert self._parity_str(tp) is None

    def test_gate_parity_tuple_alias_item(self) -> None:
        tp = TupleType([TypeAliasType(self.alias, []), self.fx.b], self.fx.std_tuple)
        assert self._parity_str(tp) is not None
