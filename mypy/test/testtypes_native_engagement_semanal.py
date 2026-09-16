"""Native engagement suites for the semanal area (`mypy/semanal.py` and `mypy/symtable.py`).

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
from unittest import TestCase, skipUnless

from mypy.nodes import (
    ARG_NAMED,
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    GDEF,
    INVARIANT,
    MDEF,
    ArgKind,
    Argument,
    AssignmentStmt,
    Block,
    CallExpr,
    ClassDef,
    Decorator,
    Expression,
    FuncDef,
    IndexExpr,
    IntExpr,
    MemberExpr,
    NameExpr,
    OpExpr,
    OverloadedFuncDef,
    PassStmt,
    PlaceholderNode,
    StarExpr,
    StrExpr,
    SymbolTable,
    SymbolTableNode,
    TupleExpr,
    TypeAlias,
    TypeInfo,
    TypeVarExpr,
    UnaryExpr,
    Var,
)
from mypy.options import Options
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import (
    _HAS_TYPE_KERNEL,
    _NATIVE_SEMANAL_LOOKUP_ENABLED,
    _NATIVE_WIRE_ENABLED,
    T,
    _FakeNode,
    _FakeTypeInfo,
    _is_type_info,
)
from mypy.test.typefixture import TypeFixture
from mypy.typeanal import (
    collect_all_inner_types,
    has_any_from_unimported_type,
    has_explicit_any,
    make_optional_type,
)
from mypy.typeops import make_simplified_union, true_only
from mypy.types import (
    AnyType,
    CallableType,
    ErasedType,
    Instance,
    NoneType,
    PartialType,
    ProperType,
    TupleType,
    Type,
    TypeAliasType,
    TypedDictType,
    TypeOfAny,
    TypeType,
    TypeVarId,
    TypeVarType,
    UnboundType,
    UnionType,
    UnpackType,
    get_proper_type,
)


# Moved from mypy/test/testtypes_native_checker.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsBaseClassSuite(Suite):
    """Parity for `rust_is_base_class` (H1p).

    `SemanticAnalyzer.is_base_class` (semanal.py:3831-3844) is a pure
    graph walk on `TypeInfo.bases` — no wire bytes. Returns True if `t`
    is reachable from `s` via the base-class graph (excluding MRO).
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_active

        self._set_active = _set_native_semanal_active
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

    def _make_info(self, name: str, bases: list[TypeInfo] | None = None) -> TypeInfo:
        if bases:
            base_instances = [Instance(b, []) for b in bases]
            return self.fx.make_type_info(name, mro=[*bases, self.fx.oi], bases=base_instances)
        return self.fx.make_type_info(name)

    def _seam(self, t: TypeInfo, s: TypeInfo) -> bool | None:
        return _type_kernel.rust_is_base_class(t, s)

    def _run(self, t: TypeInfo, s: TypeInfo) -> tuple[bool, bool]:
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> bool:
            sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
            return sa.is_base_class(t, s)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, t: TypeInfo, s: TypeInfo) -> None:
        off, on = self._run(t, s)
        assert_equal(on, off, f"is_base_class parity for t={t.fullname} s={s.fullname}")

    def test_seam_direct_base(self) -> None:
        t = self._make_info("Base")
        s = self._make_info("Sub", bases=[t])
        assert self._seam(t, s) is True

    def test_seam_transitive_base(self) -> None:
        t = self._make_info("Grandparent")
        mid = self._make_info("Parent", bases=[t])
        s = self._make_info("Child", bases=[mid])
        assert self._seam(t, s) is True

    def test_seam_not_base(self) -> None:
        t = self._make_info("A")
        s = self._make_info("B")
        assert self._seam(t, s) is False

    def test_seam_self_is_base(self) -> None:
        t = self._make_info("Self")
        assert self._seam(t, t) is True

    def test_seam_cycle_safe(self) -> None:
        a = self._make_info("A")
        b = self._make_info("B", bases=[a])
        a.bases.append(Instance(b, []))
        assert self._seam(b, a) is True

    def test_parity_direct_base(self) -> None:
        t = self._make_info("Base")
        s = self._make_info("Sub", bases=[t])
        self._assert_par(t, s)

    def test_parity_transitive_base(self) -> None:
        t = self._make_info("Grandparent")
        mid = self._make_info("Parent", bases=[t])
        s = self._make_info("Child", bases=[mid])
        self._assert_par(t, s)

    def test_parity_not_base(self) -> None:
        t = self._make_info("A")
        s = self._make_info("B")
        self._assert_par(t, s)

    def test_parity_self(self) -> None:
        t = self._make_info("Self")
        self._assert_par(t, t)


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeIsOverloadedItemSuite(Suite):
    """Parity for `rust_is_overloaded_item` (H1q).

    `SemanticAnalyzer.is_overloaded_item` (semanal.py:8328-8339) is a pure
    isinstance + identity check, no wire bytes.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_active

        self._set_active = _set_native_semanal_active
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

    def _make_overload(self, funcs: list[FuncDef]) -> OverloadedFuncDef:
        ovl = OverloadedFuncDef(funcs)  # type: ignore[arg-type]
        return ovl

    def _make_func(self, name: str = "f") -> FuncDef:
        from mypy.nodes import Block

        return FuncDef(name, [], Block([]))

    def _make_decorator(self, name: str = "f") -> Decorator:
        func = self._make_func(name)
        var = Var(name)
        return Decorator(func, [], var)

    def _seam(self, node: Any, statement: Any) -> bool | None:
        return _type_kernel.rust_is_overloaded_item(node, statement)

    def _run(self, node: Any, statement: Any) -> tuple[bool, bool]:
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> bool:
            sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
            return sa.is_overloaded_item(node, statement)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, node: Any, statement: Any) -> None:
        off, on = self._run(node, statement)
        assert_equal(on, off, "is_overloaded_item parity")

    def test_seam_item_match(self) -> None:
        f = self._make_func("f")
        ovl = self._make_overload([f])
        assert self._seam(ovl, f) is True

    def test_seam_item_no_match(self) -> None:
        f1 = self._make_func("f")
        f2 = self._make_func("g")
        ovl = self._make_overload([f1])
        assert self._seam(ovl, f2) is False

    def test_seam_decorator_item_match(self) -> None:
        dec = self._make_decorator("f")
        ovl = OverloadedFuncDef([dec])
        assert self._seam(ovl, dec.func) is True

    def test_seam_impl_match(self) -> None:
        f = self._make_func("f")
        ovl = OverloadedFuncDef([])
        ovl.impl = f
        assert self._seam(ovl, f) is True

    def test_seam_impl_decorator_match(self) -> None:
        dec = self._make_decorator("f")
        ovl = OverloadedFuncDef([])
        ovl.impl = dec
        assert self._seam(ovl, dec.func) is True

    def test_seam_not_overloaded(self) -> None:
        f = self._make_func("f")
        assert self._seam(f, f) is False

    def test_seam_not_funcdef(self) -> None:
        f = self._make_func("f")
        ovl = self._make_overload([f])
        assert self._seam(ovl, ovl) is False

    def test_parity_item_match(self) -> None:
        f = self._make_func("f")
        ovl = self._make_overload([f])
        self._assert_par(ovl, f)

    def test_parity_no_match(self) -> None:
        f1 = self._make_func("f")
        f2 = self._make_func("g")
        ovl = self._make_overload([f1])
        self._assert_par(ovl, f2)

    def test_parity_impl(self) -> None:
        f = self._make_func("f")
        ovl = OverloadedFuncDef([])
        ovl.impl = f
        self._assert_par(ovl, f)

    def test_parity_not_overloaded(self) -> None:
        f = self._make_func("f")
        self._assert_par(f, f)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsSelfMemberRefSuite(Suite):
    """Parity for `rust_is_self_member_ref` (H1r).

    `SemanticAnalyzer.is_self_member_ref` (semanal.py:6007) is a pure
    isinstance + attribute check: returns True when memberexpr.expr is a
    NameExpr whose .node is a Var with is_self=True.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_active

        self._set_active = _set_native_semanal_active
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

    def _make_name_expr(self, name: str = "self") -> NameExpr:
        return NameExpr(name)

    def _make_self_var(self) -> Var:
        var = Var("self")
        var.is_self = True
        return var

    def _make_member_expr(self, expr: Expression, name: str = "attr") -> MemberExpr:
        return MemberExpr(expr, name)

    def _seam(self, memberexpr: MemberExpr) -> bool | None:
        return _type_kernel.rust_is_self_member_ref(memberexpr)

    def _run(self, memberexpr: MemberExpr) -> tuple[bool, bool]:
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> bool:
            sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
            return sa.is_self_member_ref(memberexpr)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, memberexpr: MemberExpr) -> None:
        off, on = self._run(memberexpr)
        assert_equal(on, off, "is_self_member_ref parity")

    def test_seam_self_var(self) -> None:
        ne = self._make_name_expr("self")
        ne.node = self._make_self_var()
        me = self._make_member_expr(ne, "x")
        assert self._seam(me) is True

    def test_seam_non_self_var(self) -> None:
        ne = self._make_name_expr("other")
        v = Var("other")
        v.is_self = False
        ne.node = v
        me = self._make_member_expr(ne, "x")
        assert self._seam(me) is False

    def test_seam_node_none(self) -> None:
        ne = self._make_name_expr("self")
        ne.node = None
        me = self._make_member_expr(ne, "x")
        assert self._seam(me) is False

    def test_seam_node_not_var(self) -> None:
        ne = self._make_name_expr("self")
        ne.node = self.fx.oi  # TypeInfo, not Var
        me = self._make_member_expr(ne, "x")
        assert self._seam(me) is False

    def test_seam_expr_not_nameexpr(self) -> None:
        inner = MemberExpr(NameExpr("x"), "y")
        me = self._make_member_expr(inner, "z")
        assert self._seam(me) is False

    def test_parity_self_var(self) -> None:
        ne = self._make_name_expr("self")
        ne.node = self._make_self_var()
        me = self._make_member_expr(ne, "x")
        self._assert_par(me)

    def test_parity_non_self_var(self) -> None:
        ne = self._make_name_expr("other")
        v = Var("other")
        v.is_self = False
        ne.node = v
        me = self._make_member_expr(ne, "x")
        self._assert_par(me)

    def test_parity_node_none(self) -> None:
        ne = self._make_name_expr("self")
        ne.node = None
        me = self._make_member_expr(ne, "x")
        self._assert_par(me)

    def test_parity_expr_not_nameexpr(self) -> None:
        inner = MemberExpr(NameExpr("x"), "y")
        me = self._make_member_expr(inner, "z")
        self._assert_par(me)

    def test_parity_node_not_var(self) -> None:
        ne = self._make_name_expr("self")
        ne.node = self.fx.oi
        me = self._make_member_expr(ne, "x")
        self._assert_par(me)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsTypeLikeSuite(Suite):
    """Parity for `rust_is_type_like` (H1s).

    `SemanticAnalyzer.is_type_like` (semanal.py:8310) is a pure
    isinstance check: True for TypeInfo, TypeAlias, or PlaceholderNode
    with becomes_typeinfo=True.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_active

        self._set_active = _set_native_semanal_active
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

    def _seam(self, node: Any) -> bool | None:
        return _type_kernel.rust_is_type_like(node)

    def _run(self, node: Any) -> tuple[bool, bool]:
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> bool:
            sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
            return sa.is_type_like(node)

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, node: Any) -> None:
        off, on = self._run(node)
        assert_equal(on, off, "is_type_like parity")

    def test_seam_typeinfo(self) -> None:
        assert self._seam(self.fx.oi) is True

    def test_seam_typealias(self) -> None:
        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        assert self._seam(alias) is True

    def test_seam_placeholder_becomes(self) -> None:
        ph = PlaceholderNode("mod.C", Var("C"), 1, becomes_typeinfo=True)
        assert self._seam(ph) is True

    def test_seam_placeholder_not_becomes(self) -> None:
        ph = PlaceholderNode("mod.C", Var("C"), 1, becomes_typeinfo=False)
        assert self._seam(ph) is False

    def test_seam_var(self) -> None:
        assert self._seam(Var("x")) is False

    def test_seam_none(self) -> None:
        assert self._seam(None) is False

    def test_parity_typeinfo(self) -> None:
        self._assert_par(self.fx.oi)

    def test_parity_typealias(self) -> None:
        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        self._assert_par(alias)

    def test_parity_placeholder_becomes(self) -> None:
        ph = PlaceholderNode("mod.C", Var("C"), 1, becomes_typeinfo=True)
        self._assert_par(ph)

    def test_parity_var(self) -> None:
        self._assert_par(Var("x"))


# Moved from mypy/test/testtypes_native_types.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeWireFixupSuite(Suite):
    """Parity suite for the wire round-trip fixup (#156).

    Serialization to wire format loses live TypeInfo references: decoded
    Instances carry only a type_ref fullname string and a FakeInfo
    placeholder. Every kernel returning a wire-decoded Type must run it
    through mypy.wirefixup before it re-enters the type graph. The four
    gates re-enabled here (typeops, semanal, erase_typevars, copy_type)
    were disabled in #155 because unfixed decode crashed production.
    """

    def setUp(self) -> None:
        from mypy.applytype import _set_native_applytype_typeinfo_map
        from mypy.copytype import _set_native_copy_active
        from mypy.erasetype import _set_native_erase_typevars_active
        from mypy.semanal import _set_native_semanal_active
        from mypy.typeops import _set_native_typeops_active

        self.fx = TypeFixture(INVARIANT)
        typeinfos = []
        for name in dir(self.fx):
            if name.endswith("i"):
                value = getattr(self.fx, name)
                if _is_type_info(value):
                    typeinfos.append(value)
        typeinfo_map = {info.fullname: info for info in typeinfos}
        # Installs the shared wirefixup map as well.
        _set_native_applytype_typeinfo_map(typeinfo_map)
        self._py: list[tuple[str, Any]] = [
            ("typeops", _set_native_typeops_active),
            ("semanal", _set_native_semanal_active),
            ("erase", _set_native_erase_typevars_active),
            ("copy", _set_native_copy_active),
        ]
        for _, setter in self._py:
            setter(True)

    def tearDown(self) -> None:
        from mypy.applytype import _set_native_applytype_typeinfo_map

        for _, setter in self._py:
            setter(False)
        _set_native_applytype_typeinfo_map(None)

    def _assert_no_fake_info(self, t: Type) -> None:
        from mypy.wirefixup import check_no_fake_info

        assert check_no_fake_info(t), "wire decode leaked a FakeInfo-bearing Type"

    def test_make_simplified_union_fixes_up_instances(self) -> None:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(False)
        expected = make_simplified_union([self.fx.b, self.fx.c])
        _set_native_typeops_active(True)
        actual = make_simplified_union([self.fx.b, self.fx.c])
        self._assert_no_fake_info(actual)
        assert_equal(actual, expected)
        assert isinstance(actual, UnionType)

    def test_true_only_fixes_up_literal_fallback(self) -> None:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(False)
        expected = true_only(self.fx.a)
        _set_native_typeops_active(True)
        actual = true_only(self.fx.a)
        self._assert_no_fake_info(actual)
        assert_equal(actual, expected)

    def test_make_any_non_explicit_fixes_up(self) -> None:
        from mypy.semanal import _set_native_semanal_active, make_any_non_explicit

        _set_native_semanal_active(False)
        expected = make_any_non_explicit(AnyType(TypeOfAny.explicit))
        _set_native_semanal_active(True)
        actual = make_any_non_explicit(AnyType(TypeOfAny.explicit))
        self._assert_no_fake_info(actual)
        assert_equal(actual, expected)
        assert isinstance(actual, AnyType)  # type: ignore[misc]
        assert actual.type_of_any == TypeOfAny.special_form

    def test_replace_implicit_first_type_fixes_up(self) -> None:
        from mypy.semanal import _set_native_semanal_active, replace_implicit_first_type

        sig = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.anyt,
            self.fx.function,
        )
        _set_native_semanal_active(False)
        expected = replace_implicit_first_type(sig, self.fx.o)
        _set_native_semanal_active(True)
        actual = replace_implicit_first_type(sig, self.fx.o)
        self._assert_no_fake_info(actual)
        assert_equal(actual, expected)
        assert isinstance(actual, CallableType)
        assert_equal(actual.arg_types, [self.fx.o, self.fx.b])

    def test_erase_typevars_fixes_up_typevars_in_instance(self) -> None:
        from mypy.erasetype import _set_native_erase_typevars_active, erase_typevars

        generic = Instance(self.fx.gi, [self.fx.s1])
        _set_native_erase_typevars_active(False)
        expected = erase_typevars(generic)
        _set_native_erase_typevars_active(True)
        actual = erase_typevars(generic)
        self._assert_no_fake_info(actual)
        assert_equal(actual, expected)
        assert isinstance(actual, Instance)  # type: ignore[misc]

    def test_replace_meta_vars_erased_type_target(self) -> None:
        # replace_meta_vars round-trips an ErasedType replacement (checkexpr
        # passes ErasedType() as the target during inference). A meta-var
        # TypeVar must be replaced by the target; a class typevar untouched.
        from mypy.erasetype import _set_native_erase_typevars_active, replace_meta_vars
        from mypy.types import TypeVarId

        meta = TypeVarType(
            "T",
            "T",
            TypeVarId(-1, meta_level=1),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        target = ErasedType()
        typ = Instance(self.fx.gi, [meta])

        _set_native_erase_typevars_active(False)
        expected = replace_meta_vars(typ, target)
        _set_native_erase_typevars_active(True)
        actual = replace_meta_vars(typ, target)
        self._assert_no_fake_info(actual)
        # ErasedType has no __eq__ (identity), and the native path returns a
        # fresh wire-decoded instance, so compare the semantic content.
        assert_equal(actual.serialize(), expected.serialize())
        assert isinstance(actual, Instance)  # type: ignore[misc]
        assert isinstance(actual.args[0], ErasedType)

    def test_replace_meta_vars_fixes_up_alias_args(self) -> None:
        # Issue #1298: replace_meta_vars round-trips literals nested in a
        # TypeAliasType. The Rust kernel recurses into the alias args and keeps
        # the alias node; Python decode re-links it to the live TypeAlias.
        from mypy.erasetype import _set_native_erase_typevars_active, replace_meta_vars
        from mypy.wirefixup import set_wire_alias_map

        meta = TypeVarType(
            "T",
            "T",
            TypeVarId(-1, meta_level=1),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        alias = TypeAlias(Instance(self.fx.std_listi, [self.fx.a]), "mod.A", "mod", -1, -1)
        target = self.fx.a
        typ = TypeAliasType(alias, [meta])
        set_wire_alias_map({alias.fullname: alias})
        try:
            _set_native_erase_typevars_active(False)
            expected = replace_meta_vars(typ, target)
            _set_native_erase_typevars_active(True)
            actual = replace_meta_vars(typ, target)
            self._assert_no_fake_info(actual)
            assert_equal(actual, expected)
            assert isinstance(actual, TypeAliasType)
            assert actual.alias is alias, "decoded alias node not re-linked"
            assert isinstance(get_proper_type(actual.args[0]), Instance)
        finally:
            set_wire_alias_map(None)

    def test_erase_typevars_fixes_up_alias_args(self) -> None:
        # Issue #1298: erase_typevars now recurses into TypeAliasType args
        # too (the meta var is erased to Any and the alias node survives,
        # re-linked to the live node on decode).
        from mypy.erasetype import _set_native_erase_typevars_active, erase_typevars
        from mypy.wirefixup import set_wire_alias_map

        meta = TypeVarType(
            "T",
            "T",
            TypeVarId(-1, meta_level=1),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        alias = TypeAlias(Instance(self.fx.std_listi, [self.fx.a]), "mod.A", "mod", -1, -1)
        typ = TypeAliasType(alias, [meta])
        set_wire_alias_map({alias.fullname: alias})
        try:
            _set_native_erase_typevars_active(False)
            expected = erase_typevars(typ)
            _set_native_erase_typevars_active(True)
            actual = erase_typevars(typ)
            self._assert_no_fake_info(actual)
            assert_equal(actual, expected)
            assert isinstance(actual, TypeAliasType)
            assert actual.alias is alias, "decoded alias node not re-linked"
            assert isinstance(get_proper_type(actual.args[0]), AnyType)
        finally:
            set_wire_alias_map(None)

    def test_copy_type_fixes_up_instance(self) -> None:
        from mypy.copytype import _set_native_copy_active, copy_type

        _set_native_copy_active(False)
        expected = copy_type(self.fx.b)
        _set_native_copy_active(True)
        actual = copy_type(self.fx.b)
        self._assert_no_fake_info(actual)
        assert_equal(actual, expected)
        assert actual is not self.fx.b

    def test_has_explicit_any_fixes_up(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        _set_native_typeanal_active(False)
        expected = has_explicit_any(AnyType(TypeOfAny.explicit))
        _set_native_typeanal_active(True)
        actual = has_explicit_any(AnyType(TypeOfAny.explicit))
        assert_equal(actual, expected)
        assert actual is True

    def test_has_explicit_any_nested_in_instance(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active, has_explicit_any

        t = Instance(self.fx.std_listi, [AnyType(TypeOfAny.explicit)])
        _set_native_typeanal_active(False)
        expected = has_explicit_any(t)
        _set_native_typeanal_active(True)
        actual = has_explicit_any(t)
        assert_equal(actual, expected)
        assert actual is True

    def test_has_explicit_any_false_for_unimported(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active, has_explicit_any

        t = AnyType(TypeOfAny.from_unimported_type)
        _set_native_typeanal_active(False)
        expected = has_explicit_any(t)
        _set_native_typeanal_active(True)
        actual = has_explicit_any(t)
        assert_equal(actual, expected)
        assert actual is False

    def test_has_explicit_any_false_for_typeddict(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active, has_explicit_any

        td = TypedDictType({"x": self.fx.a}, {"x"}, set(), Instance(self.fx.std_listi, []))
        _set_native_typeanal_active(False)
        expected = has_explicit_any(td)
        _set_native_typeanal_active(True)
        actual = has_explicit_any(td)
        assert_equal(actual, expected)
        assert actual is False

    def test_has_any_from_unimported_type_fixes_up(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        _set_native_typeanal_active(False)
        expected = has_any_from_unimported_type(AnyType(TypeOfAny.from_unimported_type))
        _set_native_typeanal_active(True)
        actual = has_any_from_unimported_type(AnyType(TypeOfAny.from_unimported_type))
        assert_equal(actual, expected)
        assert actual is True

    def test_collect_all_inner_types_fixes_up(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        t = make_simplified_union([self.fx.b, self.fx.c])
        _set_native_typeanal_active(False)
        expected = collect_all_inner_types(t)
        _set_native_typeanal_active(True)
        actual = collect_all_inner_types(t)
        for item in actual:
            self._assert_no_fake_info(item)
        assert_equal(actual, expected)
        assert_equal(actual, [self.fx.b, self.fx.c])

    def test_collect_all_inner_types_instance_args(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        t = Instance(self.fx.std_listi, [self.fx.b])
        _set_native_typeanal_active(False)
        expected = collect_all_inner_types(t)
        _set_native_typeanal_active(True)
        actual = collect_all_inner_types(t)
        for item in actual:
            self._assert_no_fake_info(item)
        assert_equal(actual, expected)
        assert_equal(actual, [self.fx.b])

    def test_collect_all_inner_types_leaf_is_empty(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        _set_native_typeanal_active(False)
        expected = collect_all_inner_types(self.fx.b)
        _set_native_typeanal_active(True)
        actual = collect_all_inner_types(self.fx.b)
        assert_equal(actual, expected)
        assert_equal(actual, [])

    def test_make_optional_type_fixes_up(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        _set_native_typeanal_active(False)
        expected = make_optional_type(self.fx.b)
        _set_native_typeanal_active(True)
        actual = make_optional_type(self.fx.b)
        self._assert_no_fake_info(actual)
        assert_equal(actual, expected)
        assert isinstance(actual, UnionType)  # type: ignore[misc]
        assert_equal(actual.items, [self.fx.b, NoneType()], f"got {actual.items!r}")

    def test_make_optional_type_union_strips_none(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        t = make_simplified_union([self.fx.b, self.fx.c])
        union_none = UnionType([t, NoneType()])
        _set_native_typeanal_active(False)
        expected = make_optional_type(union_none)
        _set_native_typeanal_active(True)
        actual = make_optional_type(union_none)
        self._assert_no_fake_info(actual)
        assert_equal(actual, expected)
        assert isinstance(actual, UnionType)  # type: ignore[misc]
        assert NoneType() in actual.items
        # Optional[B|C] == B|C|None, and None is absorbed.
        assert_equal(actual.items, [self.fx.b, self.fx.c, NoneType()], f"got {actual.items!r}")

    def test_unknown_unpack_false_for_plain_instance(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active, unknown_unpack

        _set_native_typeanal_active(False)
        expected = unknown_unpack(Instance(self.fx.std_listi, [self.fx.a]))
        _set_native_typeanal_active(True)
        actual = unknown_unpack(Instance(self.fx.std_listi, [self.fx.a]))
        assert_equal(actual, expected)
        assert actual is False

    def test_unknown_unpack_true_for_special_form_any(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active, unknown_unpack

        t = UnpackType(AnyType(TypeOfAny.special_form))
        _set_native_typeanal_active(False)
        expected = unknown_unpack(t)
        _set_native_typeanal_active(True)
        actual = unknown_unpack(t)
        assert_equal(actual, expected)
        assert actual is True

    def test_unknown_unpack_false_for_other_any(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active, unknown_unpack

        t = UnpackType(AnyType(TypeOfAny.unannotated))
        _set_native_typeanal_active(False)
        expected = unknown_unpack(t)
        _set_native_typeanal_active(True)
        actual = unknown_unpack(t)
        assert_equal(actual, expected)
        assert actual is False

    def test_unknown_unpack_falls_back_to_python_on_alias(self) -> None:
        # The Rust kernel defers on a TypeAliasType unpack target (its
        # proper expansion is not on the wire); both paths must agree.
        from mypy.nodes import TypeAlias
        from mypy.typeanal import _set_native_typeanal_active, unknown_unpack
        from mypy.types import TypeAliasType

        alias = TypeAlias(AnyType(TypeOfAny.special_form), "m.A", "m", -1, -1)
        t = UnpackType(TypeAliasType(alias, []))
        _set_native_typeanal_active(False)
        expected = unknown_unpack(t)
        _set_native_typeanal_active(True)
        actual = unknown_unpack(t)
        assert_equal(actual, expected)
        assert actual is True

    def test_erase_typevars_replacement_any_is_special_form(self) -> None:
        # Native erase_typevars replaces an erased TypeVar with a wire Any;
        # its type_of_any must decode as TypeOfAny.special_form (#1262).
        from mypy.erasetype import _set_native_erase_typevars_active, erase_typevars

        _set_native_erase_typevars_active(False)
        expected = erase_typevars(self.fx.t)
        _set_native_erase_typevars_active(True)
        actual = erase_typevars(self.fx.t)
        self._assert_no_fake_info(actual)
        assert_equal(actual, expected)
        assert isinstance(actual, AnyType)  # type: ignore[misc]
        assert actual.type_of_any == TypeOfAny.special_form


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSimpleLiteralTypeSuite(Suite):
    """Parity for the Rust `analyze_simple_literal_type` dispatch port.

    The 5-way dispatch head (semanal.py:4720-4749) decides the type-name
    tag from function_stack truthiness and the constant-fold value kind;
    the Python shim folds via the already-native constant_fold_expr, then
    applies named_type_or_none and, when is_final, the LiteralType
    last_known_value construction.

    Direct seam calls assert the exact tag (and the unknown-kind deferral);
    the gate-off vs gate-on differential drives the real SemanticAnalyzer
    method through a stub named_type_or_none and asserts identical results.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_visitor_active

        self.fx = TypeFixture()
        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _tag(self, function_stack: bool, kind: int, is_final: bool = False) -> int | None:
        from mypy.semanal import _rust_classify_simple_literal_type  # type: ignore[attr-defined]

        return _rust_classify_simple_literal_type(function_stack, kind, "__main__", is_final)

    def _analyzer(self, function_stack: list[object] | None = None) -> object:
        from mypy.semanal import SemanticAnalyzer

        fx = self.fx
        infos = {
            "builtins.bool": fx.bool_type_info,
            "builtins.int": fx.make_type_info("builtins.int"),
            "builtins.str": fx.str_type_info,
            "builtins.float": fx.make_type_info("builtins.float"),
        }

        def named_type_or_none(name: str) -> Instance | None:
            info = infos.get(name)
            return Instance(info, []) if info is not None else None

        sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
        sa.function_stack = function_stack if function_stack is not None else []  # type: ignore[assignment]
        sa.cur_mod_id = "__main__"
        sa.named_type_or_none = named_type_or_none  # type: ignore[method-assign, assignment]
        return sa

    def _call(self, sa: Any, rvalue: Expression, is_final: bool) -> str:
        result = sa.analyze_simple_literal_type(rvalue, is_final)
        if result is None:
            return "None"
        return str(result)

    def _assert_par(self, rvalue: Expression, is_final: bool, expected: str | None = None) -> None:
        off_sa = self._analyzer()
        off = self._with_gate(False, lambda: self._call(off_sa, rvalue, is_final))
        self._set_active(True)
        on_sa = self._analyzer()
        on = self._with_gate(True, lambda: self._call(on_sa, rvalue, is_final))
        assert_equal(on, off, f"simple_literal_type parity {rvalue!r} final={is_final}")
        if expected is not None:
            assert_equal(on, expected, f"simple_literal_type result {rvalue!r}")

    def _final_var_ref(self, fullname: str) -> NameExpr:
        v = Var("X")
        v.is_final = True
        v._fullname = fullname
        v.final_value = 5
        e = NameExpr("X")
        e.node = v
        return e

    def test_seam_tags(self) -> None:
        assert self._tag(True, 0) == 0
        assert self._tag(False, 0) == 0  # fold returned None
        assert self._tag(False, 1) == 0  # complex
        assert self._tag(False, 2) == 1  # builtins.bool
        assert self._tag(False, 3) == 2  # builtins.int
        assert self._tag(False, 4) == 3  # builtins.str
        assert self._tag(False, 5) == 4  # builtins.float

    def test_seam_unknown_kind_defers(self) -> None:
        assert self._tag(False, 99) is None
        assert self._tag(False, -1) is None

    def test_parity_int(self) -> None:
        self._assert_par(IntExpr(42), False, "builtins.int")

    def test_parity_int_final(self) -> None:
        self._assert_par(IntExpr(42), True)

    def test_parity_str(self) -> None:
        self._assert_par(StrExpr("x"), False, "builtins.str")

    def test_parity_str_final(self) -> None:
        self._assert_par(StrExpr("x"), True)

    def test_parity_float(self) -> None:
        from mypy.nodes import FloatExpr

        self._assert_par(FloatExpr(1.5), False, "builtins.float")

    def test_parity_complex_returns_none(self) -> None:
        from mypy.nodes import ComplexExpr

        self._assert_par(ComplexExpr(1j), False, "None")

    def test_parity_bool(self) -> None:
        self._assert_par(NameExpr("True"), False, "builtins.bool")

    def test_parity_bool_final(self) -> None:
        self._assert_par(NameExpr("True"), True)

    def test_parity_fold_failure_returns_none(self) -> None:
        self._assert_par(OpExpr("+", IntExpr(1), StrExpr("x")), False, "None")

    def test_parity_folded_op_final(self) -> None:
        self._assert_par(OpExpr("+", IntExpr(1), IntExpr(2)), True)

    def test_parity_final_var_ref_current_module(self) -> None:
        self._assert_par(self._final_var_ref("__main__.X"), False, "builtins.int")

    def test_parity_final_var_ref_other_module(self) -> None:
        self._assert_par(self._final_var_ref("other.X"), False, "None")

    def test_parity_inside_function(self) -> None:
        off_sa = self._analyzer(function_stack=[object()])
        off = self._with_gate(False, lambda: self._call(off_sa, IntExpr(42), False))
        self._set_active(True)
        on_sa = self._analyzer(function_stack=[object()])
        on = self._with_gate(True, lambda: self._call(on_sa, IntExpr(42), False))
        assert_equal(on, off, "simple_literal_type parity inside function")
        assert_equal(on, "None", "simple_literal_type inside function")


# Moved from mypy/test/testtypes_native_semanal.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeExpressionClassifySuite(Suite):
    """Parity for the Rust `try_parse_as_type_expression` classifier port.

    The bail-out front (which StrExpr/IndexExpr/OpExpr values cannot
    possibly be type expressions, semanal.py:8945-9021) runs in Rust:
    Python gathers scalar structural facts from the live AST node and
    Rust branches on them. `Some(0)` sets `as_type = None` and returns;
    `Some(1)` and `None` fall into the pure front, which then cannot
    bail. Toggling the semanal-visitor gate off (pure Python) and on
    (Rust) must produce identical `as_type` and full-parse counters on
    both engaged and deferred paths, and direct seam calls prove the
    Rust classifier engages. Identifier-string StrExprs defer because
    classifying them needs `self.lookup`.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        from mypy.semanal import _set_native_semanal_visitor_active

        self._set_active = _set_native_semanal_visitor_active
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

    def _analyser(self) -> Any:
        from contextlib import nullcontext

        from mypy.errors import Errors
        from mypy.semanal import SemanticAnalyzer

        sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
        sa.type_expression_parse_count = 0
        sa.type_expression_full_parse_success_count = 0
        sa.type_expression_full_parse_failure_count = 0
        sa.lookup = lambda name, typ, suppress_errors=True: None  # type: ignore[method-assign, assignment]
        sa.tvar_scope = None  # type: ignore[assignment]
        sa.errors = Errors(Options())
        sa.isolated_error_analysis = lambda: nullcontext()  # type: ignore[method-assign, return-value, assignment]
        sa.expr_to_analyzed_type = lambda expr: self.fx.o  # type: ignore[method-assign, assignment, misc]
        return sa

    def _run(self, node_factory: Callable[[], Expression]) -> tuple[str, int, int]:
        sa = self._analyser()
        expr = node_factory()
        sa.try_parse_as_type_expression(expr)
        # NameExpr/MemberExpr never receive `as_type` (the method returns
        # before assigning it); represent that as a fixed sentinel so both
        # gates compare equal.
        as_type = str(expr.as_type) if hasattr(expr, "as_type") else "NO_AS_TYPE_ATTR"
        return (
            as_type,
            sa.type_expression_full_parse_success_count,
            sa.type_expression_full_parse_failure_count,
        )

    def _assert_par(self, node_factory: Callable[[], Expression]) -> None:
        off = self._with_gate(False, lambda: self._run(node_factory))
        on = self._with_gate(True, lambda: self._run(node_factory))
        assert_equal(on, off, "try_parse_as_type_expression parity")

    def _assert_engages(self, args: tuple[object, ...], expected: int | None) -> None:
        result = self._tk.rust_classify_type_expression(*args)
        assert_equal(result, expected, "rust_classify_type_expression")

    # --- NameExpr / MemberExpr: never reach Rust (defer to TypeChecker) ---

    def test_name_expr_defers(self) -> None:
        self._assert_par(lambda: NameExpr("x"))

    def test_member_expr_defers(self) -> None:
        self._assert_par(lambda: MemberExpr(NameExpr("m"), "attr"))

    # --- StrExpr bail-outs ---

    def test_sentence_nontype(self) -> None:
        # A sentence-like string matches _MULTIPLE_WORDS_NONTYPE_RE.
        self._assert_par(lambda: StrExpr("sentence like this"))

    def test_identifier_string_unresolved(self) -> None:
        # 'a' is an identifier that refers to nothing; Rust defers, the
        # Python lookup returns None, both gates bail with as_type=None.
        self._assert_par(lambda: StrExpr("a"))

    def test_two_words_nontype(self) -> None:
        self._assert_par(lambda: StrExpr("foo bar"))

    def test_quote_no_bracket_nontype(self) -> None:
        # Quoted strings are valid only inside Literal[...]/Annotated[...].
        self._assert_par(lambda: StrExpr("'a'"))

    def test_quote_with_bracket_maybe(self) -> None:
        self._assert_par(lambda: StrExpr("Literal['a']"))

    def test_whitespace_nontype(self) -> None:
        self._assert_par(lambda: StrExpr("   "))

    def test_short_nontype(self) -> None:
        self._assert_par(lambda: StrExpr("?"))

    # --- IndexExpr bail-outs ---

    def test_index_var_base_nontype(self) -> None:
        def make() -> IndexExpr:
            n = NameExpr("x")
            v = Var("x")
            v._fullname = "mod.x"
            n.node = v
            return IndexExpr(n, StrExpr("int"))

        self._assert_par(make)

    def test_index_typeinfo_base_maybe(self) -> None:
        # A TypeInfo base is not a Var: the IndexExpr proceeds to the tail.
        def make() -> IndexExpr:
            n = NameExpr("C")
            n.node = self.fx.oi
            return IndexExpr(n, StrExpr("int"))

        self._assert_par(make)

    # --- OpExpr bail-outs ---

    def test_op_union_maybe(self) -> None:
        # '|' proceeds to the tail; the expr_to_analyzed_type stub yields Any.
        self._assert_par(lambda: OpExpr("|", NameExpr("X"), NameExpr("Y")))

    def test_op_minus_nontype(self) -> None:
        self._assert_par(lambda: OpExpr("-", NameExpr("X"), NameExpr("Y")))

    # --- Direct Rust seam calls ---

    def test_engages_sentence(self) -> None:
        self._assert_engages(
            (
                (0,),
                "sentence like this",
                False,
                False,
                False,
                False,
                True,
                0,
                False,
                False,
                False,
                False,
            ),
            0,
        )

    def test_engages_identifier_defers(self) -> None:
        self._assert_engages(
            ((0,), "a", True, False, False, False, False, 0, False, False, False, False), None
        )

    def test_engages_two_words(self) -> None:
        self._assert_engages(
            ((0,), "foo bar", False, False, False, False, True, 0, False, False, False, False), 0
        )

    def test_engages_quote_bracket(self) -> None:
        self._assert_engages(
            ((0,), "Literal[1]", False, True, True, False, False, 0, False, False, False, False), 1
        )

    def test_engages_whitespace(self) -> None:
        self._assert_engages(
            ((0,), " ", False, False, False, True, False, 0, False, False, False, False), 0
        )

    def test_engages_short(self) -> None:
        self._assert_engages(
            ((0,), "?", False, False, False, False, False, 0, False, False, False, False), 0
        )

    def test_engages_op_pipe(self) -> None:
        self._assert_engages(
            ((2,), None, False, False, False, False, False, 0, False, False, False, True), 1
        )

    def test_engages_op_minus(self) -> None:
        self._assert_engages(
            ((2,), None, False, False, False, False, False, 0, False, False, False, False), 0
        )

    def test_engages_index_var(self) -> None:
        self._assert_engages(
            ((1,), None, False, False, False, False, False, 0, True, True, False, False), 0
        )

    def test_engages_index_member_leftmost_not_name(self) -> None:
        self._assert_engages(
            ((1,), None, False, False, False, False, False, 1, False, False, False, False), 0
        )

    def test_engages_index_other(self) -> None:
        self._assert_engages(
            ((1,), None, False, False, False, False, False, 2, False, False, False, False), 0
        )

    def test_engages_index_non_var_maybe(self) -> None:
        self._assert_engages(
            ((1,), None, False, False, False, False, False, 0, True, False, False, False), 1
        )

    def test_engages_bad_tags_defers(self) -> None:
        self._assert_engages(
            ((0, 0), None, False, False, False, False, False, 0, False, False, False, False), None
        )
        self._assert_engages(
            ((), None, False, False, False, False, False, 0, False, False, False, False), None
        )


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCanPossiblyBeTypeFormSuite(Suite):
    """Parity for the Rust `rust_can_possibly_be_type_form` port.

    Mirrors the structural branch of
    `SemanticAnalyzer.can_possibly_be_type_form` (semanal.py:4040-4067).
    The Python shim precomputes the resolver-backed `is_pep_613` fact
    (it needs `self.lookup_qualified`) and passes it as a scalar; Rust
    decides the whole branch natively, including the annotated-var
    rejection that previously deferred. Toggling the semanal-visitor gate
    off (pure Python) and on (Rust seam) must produce identical results;
    direct seam calls prove the Rust classifier engages.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        from mypy.semanal import _set_native_semanal_visitor_active

        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _analyser(self, typealias_fullname: str | None) -> Any:
        from mypy.semanal import SemanticAnalyzer

        sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
        if typealias_fullname is not None:
            from types import SimpleNamespace

            sym = SymbolTableNode(
                MDEF, SimpleNamespace(fullname=typealias_fullname)  # type: ignore[arg-type]
            )
        else:
            sym = None
        sa.lookup = lambda name, ctx, suppress_errors=True: sym  # type: ignore[method-assign]
        return sa

    def _assert_par(
        self, node_factory: Callable[[], AssignmentStmt], typealias_fullname: str | None = None
    ) -> None:
        def run() -> bool:
            sa = self._analyser(typealias_fullname)
            result = sa.can_possibly_be_type_form(node_factory())
            assert isinstance(result, bool)
            return result

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert_equal(on, off, "can_possibly_be_type_form parity")

    def _assert_engages(self, s: AssignmentStmt, is_pep_613: bool, expected: bool | None) -> None:
        result = self._tk.rust_can_possibly_be_type_form(s, is_pep_613)
        assert_equal(result, expected, "rust_can_possibly_be_type_form")

    # --- Annotated variable: `x: int = 1` -> False (previously deferred) ---

    def test_annotated_var_false(self) -> None:
        def make() -> AssignmentStmt:
            return AssignmentStmt([NameExpr("x")], IntExpr(1), type=UnboundType("int"))

        self._assert_par(make)
        self._assert_engages(make(), False, False)

    # --- Unannotated subscript: `X = Foo[Bar]` -> True ---

    def test_unannotated_index_true(self) -> None:
        def make() -> AssignmentStmt:
            return AssignmentStmt([NameExpr("X")], IndexExpr(NameExpr("Foo"), NameExpr("Bar")))

        self._assert_par(make)
        self._assert_engages(make(), False, True)

    # --- PEP 613: `X: TypeAlias = List[int]` -> True (previously deferred) ---

    def test_pep_613_true(self) -> None:
        def make() -> AssignmentStmt:
            return AssignmentStmt(
                [NameExpr("X")],
                IndexExpr(NameExpr("List"), NameExpr("int")),
                type=UnboundType("TypeAlias"),
            )

        typealias = "typing.TypeAlias"
        self._assert_par(make, typealias_fullname=typealias)
        self._assert_engages(make(), True, True)

    # --- Plain call rvalue: `x = foo()` -> False ---

    def test_call_rvalue_false(self) -> None:
        def make() -> AssignmentStmt:
            callee = NameExpr("foo")
            callee.fullname = "mod.foo"
            return AssignmentStmt([NameExpr("x")], CallExpr(callee, [], [], []))

        self._assert_par(make)
        self._assert_engages(make(), False, False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeDecoratorClassifySuite(Suite):
    """Parity tests for the Rust decorator classification (Issue #348).

    Each test builds a list of decorator expressions (NameExpr/MemberExpr/
    CallExpr), classifies them via `type_kernel.rust_classify_decorators`, and
    asserts the tags match what the Python classifier loop in
    `semanal.visit_decorator` would produce for the same decorators. The
    branch order and name sets mirror semanal.py:1831-1897 exactly.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        from mypy.types import (
            DATACLASS_TRANSFORM_NAMES,
            DEPRECATED_TYPE_NAMES,
            FINAL_DECORATOR_NAMES,
            OVERRIDE_DECORATOR_NAMES,
            TYPE_CHECK_ONLY_NAMES,
        )

        self._abstract_names = "abc.abstractmethod"
        self._awaitable_names = ("asyncio.coroutines.coroutine", "types.coroutine")
        self._static_names = "builtins.staticmethod"
        self._class_names = "builtins.classmethod"
        self._override_names = OVERRIDE_DECORATOR_NAMES
        self._property_names = (
            "builtins.property",
            "abc.abstractproperty",
            "functools.cached_property",
            "enum.property",
            "types.DynamicClassAttribute",
        )
        self._abstract_property_names = "abc.abstractproperty"
        self._cached_property_names = "functools.cached_property"
        self._no_type_check_names = "typing.no_type_check"
        self._final_names = FINAL_DECORATOR_NAMES
        self._type_check_only_names = TYPE_CHECK_ONLY_NAMES
        self._dataclass_transform_names = DATACLASS_TRANSFORM_NAMES
        self._deprecated_names = DEPRECATED_TYPE_NAMES

    # --- node constructors ---
    def _name(self, fullname: str) -> NameExpr:
        node = NameExpr(fullname.rsplit(".", 1)[-1])
        node.fullname = fullname
        return node

    def _member(self, base: str, attr: str) -> MemberExpr:
        node = MemberExpr(self._name(base), attr)
        node.fullname = f"{base}.{attr}"
        return node

    def _call(self, callee: NameExpr, args: list[Expression]) -> CallExpr:
        kinds = [ARG_POS] * len(args)
        return CallExpr(callee, args, kinds, [None] * len(args))

    def _classify(self, decorators: list[Expression]) -> list[str]:
        result = self._tk.rust_classify_decorators(
            decorators,
            (
                self._abstract_names,
                self._awaitable_names,
                self._static_names,
                self._class_names,
                self._override_names,
                self._property_names,
                self._abstract_property_names,
                self._cached_property_names,
                self._no_type_check_names,
                self._final_names,
                self._type_check_only_names,
                self._dataclass_transform_names,
                self._deprecated_names,
            ),
        )
        assert result is not None, f"Rust returned None for {decorators!r}"
        return result

    def _assert_tags(self, decorators: list[Expression], expected: list[str]) -> None:
        actual = self._classify(decorators)
        assert_equal(actual, expected, f"decorator classification mismatch: {actual!r}")

    def test_abstract(self) -> None:
        self._assert_tags([self._name("abc.abstractmethod")], ["abstract"])

    def test_awaitable(self) -> None:
        self._assert_tags([self._name("types.coroutine")], ["awaitable"])
        self._assert_tags([self._name("asyncio.coroutines.coroutine")], ["awaitable"])

    def test_static(self) -> None:
        self._assert_tags([self._name("builtins.staticmethod")], ["static"])

    def test_class(self) -> None:
        self._assert_tags([self._name("builtins.classmethod")], ["class"])

    def test_override(self) -> None:
        self._assert_tags([self._name("typing.override")], ["override"])
        self._assert_tags([self._name("typing_extensions.override")], ["override"])

    def test_property(self) -> None:
        self._assert_tags([self._name("builtins.property")], ["property"])
        self._assert_tags([self._name("enum.property")], ["property"])
        self._assert_tags([self._name("types.DynamicClassAttribute")], ["property"])

    def test_abstract_property(self) -> None:
        self._assert_tags([self._name("abc.abstractproperty")], ["abstract_property"])

    def test_cached_property(self) -> None:
        self._assert_tags([self._name("functools.cached_property")], ["cached_property"])

    def test_no_type_check(self) -> None:
        self._assert_tags([self._name("typing.no_type_check")], ["no_type_check"])

    def test_final(self) -> None:
        self._assert_tags([self._name("typing.final")], ["final"])
        self._assert_tags([self._name("typing_extensions.final")], ["final"])

    def test_type_check_only(self) -> None:
        self._assert_tags([self._name("typing.type_check_only")], ["type_check_only"])
        self._assert_tags([self._name("typing_extensions.type_check_only")], ["type_check_only"])

    def test_dataclass_transform(self) -> None:
        self._assert_tags(
            [self._call(self._name("typing.dataclass_transform"), [])], ["dataclass_transform"]
        )

    def test_deprecated(self) -> None:
        self._assert_tags(
            [self._call(self._name("warnings.deprecated"), [StrExpr("msg")])], ["deprecated"]
        )

    def test_other(self) -> None:
        self._assert_tags([self._name("some.random.decorator")], ["other"])
        # MemberExpr with an unrecognized name.
        self._assert_tags([self._member("mod", "decorator")], ["other"])

    def test_multiple(self) -> None:
        decorators = [
            self._name("abc.abstractmethod"),
            self._name("builtins.staticmethod"),
            self._name("builtins.classmethod"),
            self._name("builtins.property"),
            self._name("functools.cached_property"),
            self._name("abc.abstractproperty"),
            self._name("typing.final"),
            self._name("typing.no_type_check"),
            self._name("typing.override"),
            self._name("some.random.decorator"),
            self._member("mod", "decorator"),
            self._call(self._name("warnings.deprecated"), [StrExpr("msg")]),
            self._call(self._name("typing.dataclass_transform"), []),
            self._name("types.coroutine"),
            self._name("enum.property"),
            self._name("types.DynamicClassAttribute"),
            self._name("typing_extensions.type_check_only"),
        ]
        expected = [
            "abstract",
            "static",
            "class",
            "property",
            "cached_property",
            "abstract_property",
            "final",
            "no_type_check",
            "override",
            "other",
            "other",
            "deprecated",
            "dataclass_transform",
            "awaitable",
            "property",
            "property",
            "type_check_only",
        ]
        self._assert_tags(decorators, expected)


@skipUnless(
    _NATIVE_SEMANAL_LOOKUP_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext"
)
class NativeLookupSuite(Suite):
    """Parity tests for the Rust name-resolution decision (Issue #419).

    Drives `type_kernel.rust_lookup` on constructed scope stacks and asserts
    the returned decision + node matches what Python's `SemanticAnalyzer.
    _lookup` (semanal.py) would produce for the same scopes. This is a direct
    exercise of the seam, independent of the build-manager gate, so it runs
    whenever the `type_kernel` extension is importable (the golden scope-stack
    walk order in Rust is verified against the mirror steps below).
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        from mypy.nodes import SymbolTableNode, Var

        self._tk = _tk
        self._SymbolTableNode = SymbolTableNode
        self._Var = Var

    def _node(self, fullname: str) -> Any:
        v = self._Var(fullname.rsplit(".", 1)[-1])
        v._fullname = fullname
        return self._SymbolTableNode(0, v)

    def _call(self, name: str, **kw: Any) -> Any:
        return self._tk.rust_lookup(name, **kw)

    def test_global_decl_found(self) -> None:
        g = {"x": self._node("m.x")}
        r = self._call(
            "x",
            global_decls={"x"},
            globals=g,
            nonlocal_decls=set(),
            locals=[],
            type_names=None,
            is_func_scope=True,
        )
        assert r is not None and r[0] == "found" and r[1] is g["x"]

    def test_global_decl_undeclared(self) -> None:
        r = self._call(
            "x",
            global_decls={"x"},
            globals={},
            nonlocal_decls=set(),
            locals=[None],
            type_names=None,
            is_func_scope=True,
        )
        assert r == ("global_undeclared", None)

    def test_nonlocal_found_in_enclosing(self) -> None:
        inner = {"y": self._node("m.f.y")}
        r = self._call(
            "y",
            global_decls=set(),
            globals={},
            nonlocal_decls={"y"},
            locals=[None, inner, None],
            type_names=None,
            is_func_scope=True,
        )
        # reversed(self.locals[:-1]) -> skips last (current) scope.
        assert r is not None and r[0] == "found" and r[1] is inner["y"]

    def test_nonlocal_undeclared(self) -> None:
        r = self._call(
            "y",
            global_decls=set(),
            globals={},
            nonlocal_decls={"y"},
            locals=[None, None, None],
            type_names=None,
            is_func_scope=True,
        )
        assert r == ("nonlocal_undeclared", None)

    def test_local_found(self) -> None:
        inner = {"a": self._node("m.f.a")}
        r = self._call(
            "a",
            global_decls=set(),
            globals={},
            nonlocal_decls=set(),
            locals=[None, inner],
            type_names=None,
            is_func_scope=True,
        )
        assert r is not None and r[0] == "found" and r[1] is inner["a"]

    def test_global_scope_found(self) -> None:
        g = {"b": self._node("m.b")}
        r = self._call(
            "b",
            global_decls=set(),
            globals=g,
            nonlocal_decls=set(),
            locals=[None],
            type_names=None,
            is_func_scope=False,
        )
        assert r is not None and r[0] == "found" and r[1] is g["b"]

    def test_builtin_found(self) -> None:
        builtin_names = {"len": self._node("builtins.len")}
        builtins_mypyfile = _FakeNode(builtin_names)
        b_entry = self._SymbolTableNode(0, builtins_mypyfile)  # type: ignore[arg-type]
        g = {"__builtins__": b_entry}
        r = self._call(
            "len",
            global_decls=set(),
            globals=g,
            nonlocal_decls=set(),
            locals=[None],
            type_names=None,
            is_func_scope=False,
        )
        assert r is not None and r[0] == "found" and r[1] is builtin_names["len"]

    def test_builtin_private(self) -> None:
        builtin_names = {"_private": self._node("builtins._private")}
        builtins_mypyfile = _FakeNode(builtin_names)
        b_entry = self._SymbolTableNode(0, builtins_mypyfile)  # type: ignore[arg-type]
        g = {"__builtins__": b_entry}
        r = self._call(
            "_private",
            global_decls=set(),
            globals=g,
            nonlocal_decls=set(),
            locals=[None],
            type_names=None,
            is_func_scope=False,
        )
        assert r == ("builtin_private", None)

    def test_not_found(self) -> None:
        r = self._call(
            "nope",
            global_decls=set(),
            globals={},
            nonlocal_decls=set(),
            locals=[None],
            type_names=None,
            is_func_scope=False,
        )
        assert r == ("not_found", None)

    def test_qualname_synthesize_at_class_scope(self) -> None:
        # Not in class namespace -> Rust says synthesize.
        from mypy.nodes import SymbolTable

        tn: SymbolTable = SymbolTable()
        r = self._call(
            "__qualname__",
            global_decls=set(),
            globals={},
            nonlocal_decls=set(),
            locals=[None],
            type_names=tn,
            is_func_scope=False,
        )
        assert r == ("synthesize_qualname", None)

    def test_class_attr_falls_back(self) -> None:
        # Class attr present -> Rust falls back (None) for active gating.
        tn = {"attr": self._node("m.C.attr")}
        r = self._call(
            "attr",
            global_decls=set(),
            globals={},
            nonlocal_decls=set(),
            locals=[None],
            type_names=tn,
            is_func_scope=False,
        )
        assert r is None
        # Non-class-scope (is_func_scope) -> class stack ignored.
        r = self._call(
            "attr",
            global_decls=set(),
            globals={},
            nonlocal_decls=set(),
            locals=[None],
            type_names=tn,
            is_func_scope=True,
        )
        assert r == ("not_found", None)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeLookupQualifiedVarSuite(Suite):
    """Parity tests for the non-Any Var branch of `rust_lookup_qualified`.

    Python's `lookup_qualified` (semanal.py:7747-7762) walks a non-Any Var
    first sym into the else branch: `nextsym = None`, then reports
    "name not defined" (unless suppress_errors) and returns None. The Rust
    seam answers this case with `RESULT_NOT_FOUND = -1` so the Python shim
    runs the identical `name_not_defined` + return-None path it already
    uses for missing TypeInfo members. Any-typed Vars stay deferred (the
    Python else branch constructs `implicit_symbol`), and MypyFile /
    TypeAlias / ParamSpecExpr / PlaceholderNode first syms keep deferring
    (they need `get_module_symbol` / alias-target resolution / special
    arg handling / unchanged-sym return). Direct seam exercise, independent
    of the build-manager gate.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        from mypy.nodes import SymbolTableNode, TypeInfo, Var

        self.fx = TypeFixture()
        self._tk = _tk
        self._SymbolTableNode = SymbolTableNode
        self._TypeInfo = TypeInfo
        self._Var = Var
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        self.resolver = _tk.build_native_resolver(type_infos, [])

    def _call(self, name: str, kind: int, fullname: str, is_any: bool) -> Any:
        return self._tk.rust_lookup_qualified(self.resolver, name, kind, fullname, is_any)

    def test_var_non_any_reports_not_found(self) -> None:
        # Non-Any Var first sym: Rust answers RESULT_NOT_FOUND so the
        # Python shim emits "name not defined" + return None.
        r = self._call("mod.x.y", 4, "mod.x", False)
        assert r == (-1, "")

    def test_var_any_still_defers(self) -> None:
        # Any-typed Var needs `implicit_symbol` construction in Python.
        r = self._call("mod.x.y", 4, "mod.x", True)
        assert r is None

    def test_mypyfile_still_defers(self) -> None:
        # MypyFile needs get_module_symbol; keep deferring.
        r = self._call("mod.sub.x", 1, "mod.sub", False)
        assert r is None

    def test_typealias_still_defers(self) -> None:
        # TypeAlias needs alias-target resolution; keep deferring.
        r = self._call("alias.x", 3, "mod.alias", False)
        assert r is None

    def test_paramspec_still_defers(self) -> None:
        # ParamSpecExpr needs the args/kwargs special check; keep deferring.
        r = self._call("P.args", 5, "mod.P", False)
        assert r is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeLookupQualifiedNestedSuite(Suite):
    """Parity tests for nested-TypeInfo dot-chains in `rust_lookup_qualified`.

    `_collect_type_infos` (build.py) now recurses into nested class
    namespaces (issue #806 D2), so `Outer.Inner` and
    `Outer.Inner.Deep` TypeInfos are in the resolver snapshot and the
    native dot-chain walk can resolve `Outer.Inner.x` / `Outer.Inner.Deep`
    instead of deferring at the second part. Direct seam exercise on a
    resolver built from the fixture's nested classes, independent of the
    build-manager gate.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        from mypy.nodes import SymbolTableNode, TypeInfo, Var

        self.fx = TypeFixture()
        self._tk = _tk
        self._SymbolTableNode = SymbolTableNode
        self._TypeInfo = TypeInfo
        self._Var = Var
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        # Include the explicitly-nested fixtures (out, out.I, out.I.Deep),
        # which mirror what `_collect_type_infos` now descends into.
        for name in ("out", "out_I", "out_I_Deep"):
            if hasattr(self.fx, name):
                type_infos.append(getattr(self.fx, name))
        self.resolver = _tk.build_native_resolver(type_infos, [])

    def _call(self, name: str, kind: int, fullname: str, is_any: bool) -> Any:
        return self._tk.rust_lookup_qualified(self.resolver, name, kind, fullname, is_any)

    def test_nested_typeinfo_dot_chain(self) -> None:
        # `out.I` resolves as a TypeInfo member of `out`.
        r = self._call("out.I", 0, "out", False)
        assert r is not None
        assert r[0] == 0
        assert r[1] == "out.I"

    def test_double_nested_typeinfo_dot_chain(self) -> None:
        # `out.I.Deep` resolves through two nesting levels, from the
        # top-level entry.
        r = self._call("out.I.Deep", 0, "out", False)
        assert r is not None
        assert r[0] == 0
        assert r[1] == "out.I.Deep"

    def test_nested_missing_member_not_found(self) -> None:
        # `out.I.missing`: not in the MRO of out.I -> positively not found.
        r = self._call("out.I.missing", 0, "out", False)
        assert r is not None
        assert r[0] == -1


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCleanUpBasesSuite(Suite):
    """Parity for the Rust base-class classification front (semanal_bases.rs).

    `clean_up_bases_and_infer_type_variables` removes `Generic[...]` /
    `Protocol[...]` base declarations. The Rust seam classifies each
    UnboundType base as KEEP / GENERIC / PROTOCOL_GENERIC / BARE_PROTOCOL,
    mirroring the branch order of `analyze_class_typevar_declaration`
    (semanal.py:2817-2843) and the bare-Protocol removal
    (semanal.py:2768-2774). Assertions on the four tags cover the full
    decision table.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk

    def _classify(
        self, fullname: str | None, in_protocol_names: bool, has_args: bool
    ) -> int | None:
        return self._tk.rust_clean_up_bases(fullname, in_protocol_names, has_args)

    def test_plain_base_kept(self) -> None:
        assert self._classify("mod.Bar", False, False) == 1  # KEEP

    def test_generic_base_with_args_kept(self) -> None:
        # `class B(A[int])`: A[int] is not a Generic/Protocol declaration.
        assert self._classify("mod.A", False, True) == 1  # KEEP

    def test_bare_generic_removed(self) -> None:
        # Bare `Generic` still declares no tvars and is removed.
        assert self._classify("typing.Generic", False, False) == 2  # GENERIC

    def test_generic_with_args_removed(self) -> None:
        assert self._classify("typing.Generic", False, True) == 2  # GENERIC

    def test_protocol_with_args_declares_tvars(self) -> None:
        assert self._classify("typing.Protocol", True, True) == 3  # PROTOCOL_GENERIC

    def test_bare_protocol_removed(self) -> None:
        assert self._classify("typing_extensions.Protocol", True, False) == 4  # BARE_PROTOCOL

    def test_unresolved_symbol_kept(self) -> None:
        # Missing node / failed lookup: neither declaration fires.
        assert self._classify(None, False, True) == 1
        assert self._classify(None, False, False) == 1

    def test_non_protocol_name_kept(self) -> None:
        assert self._classify("mod.NotProtocol", False, True) == 1  # KEEP


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassDecoratorCommonSuite(Suite):
    """Parity for the Rust class-decorator classifier (Phase E1, #897).

    Exercises `type_kernel.rust_classify_class_decorator` directly on
    constructed decorators and asserts the (tag, msg) pairs match the
    pure-Python branch order of `analyze_class_decorator_common`
    (semanal.py:2741-2752). A gate-on/off differential on the method
    asserts parity of the side effects (flag writes, fails, deprecation).
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        from mypy.types import (
            DEPRECATED_TYPE_NAMES,
            DISJOINT_BASE_DECORATOR_NAMES,
            FINAL_DECORATOR_NAMES,
            TYPE_CHECK_ONLY_NAMES,
        )

        self._name_sets = (
            FINAL_DECORATOR_NAMES,
            DISJOINT_BASE_DECORATOR_NAMES,
            TYPE_CHECK_ONLY_NAMES,
            DEPRECATED_TYPE_NAMES,
        )

    def _name(self, fullname: str) -> NameExpr:
        node = NameExpr(fullname.rsplit(".", 1)[-1])
        node.fullname = fullname
        return node

    def _member(self, base: str, attr: str) -> MemberExpr:
        node = MemberExpr(self._name(base), attr)
        node.fullname = f"{base}.{attr}"
        return node

    def _call(self, callee: Expression, args: list[Expression]) -> CallExpr:
        kinds = [ARG_POS] * len(args)
        return CallExpr(callee, args, kinds, [None] * len(args))

    def _classify(self, decorator: Expression) -> tuple[str, str | None]:
        result = self._tk.rust_classify_class_decorator(decorator, self._name_sets)
        assert result is not None, f"Rust returned None for {decorator!r}"
        return result

    def test_final(self) -> None:
        assert self._classify(self._name("typing.final")) == ("final", None)
        assert self._classify(self._name("typing_extensions.final")) == ("final", None)

    def test_disjoint_base(self) -> None:
        assert self._classify(self._name("typing.disjoint_base")) == ("disjoint_base", None)
        assert self._classify(self._name("typing_extensions.disjoint_base")) == (
            "disjoint_base",
            None,
        )

    def test_type_check_only(self) -> None:
        assert self._classify(self._name("typing.type_check_only")) == ("type_check_only", None)
        assert self._classify(self._name("typing_extensions.type_check_only")) == (
            "type_check_only",
            None,
        )

    def test_deprecated(self) -> None:
        deco = self._call(self._name("warnings.deprecated"), [StrExpr("msg")])
        assert self._classify(deco) == ("deprecated", "msg")

    def test_none(self) -> None:
        assert self._classify(self._name("some.random.decorator")) == ("none", None)
        assert self._classify(self._member("mod", "decorator")) == ("none", None)

    def test_name_set_mismatch_defers(self) -> None:
        assert self._tk.rust_classify_class_decorator(self._name("typing.final"), ()) is None  # type: ignore[arg-type]

    def _run_method(
        self, decorator: Expression, *, is_protocol: bool = False, typeddict: bool = False
    ) -> tuple[bool, bool, bool, str | None, list[str]]:
        from mypy import semanal
        from mypy.nodes import Block, SymbolTable, TypeInfo

        class _Analyzer:
            def __init__(self) -> None:
                self.failures: list[str] = []

            def fail(self, msg: str, _ctx: object) -> None:
                self.failures.append(msg)

            @staticmethod
            def get_deprecated(expr: Expression) -> str | None:
                return semanal.SemanticAnalyzer.get_deprecated(expr)

        defn = ClassDef("A", Block([]), None, [])
        defn.fullname = "mod.A"
        info = TypeInfo(SymbolTable(), defn, "mod")
        info.is_protocol = is_protocol
        info.typeddict_type = object() if typeddict else None  # type: ignore[assignment]
        defn.info = info
        analyzer = _Analyzer()
        semanal.SemanticAnalyzer.analyze_class_decorator_common(analyzer, defn, decorator)  # type: ignore[arg-type]
        return (
            info.is_final,
            info.is_disjoint_base,
            info.is_type_check_only,
            info.deprecated,
            analyzer.failures,
        )

    def _assert_method_parity(self, decorator: Expression, **kwargs: bool) -> None:
        from mypy import semanal

        old = semanal._native_semanal_visitor_active
        try:
            semanal._native_semanal_visitor_active = False
            off = self._run_method(decorator, **kwargs)
            semanal._native_semanal_visitor_active = True
            on = self._run_method(decorator, **kwargs)
        finally:
            semanal._native_semanal_visitor_active = old
        assert_equal(on, off, f"analyze_class_decorator_common parity for {decorator!r}")

    def test_method_final_parity(self) -> None:
        self._assert_method_parity(self._name("typing.final"))

    def test_method_disjoint_base_plain_parity(self) -> None:
        self._assert_method_parity(self._name("typing.disjoint_base"))

    def test_method_disjoint_base_protocol_parity(self) -> None:
        self._assert_method_parity(self._name("typing.disjoint_base"), is_protocol=True)

    def test_method_disjoint_base_typeddict_parity(self) -> None:
        self._assert_method_parity(self._name("typing.disjoint_base"), typeddict=True)

    def test_method_type_check_only_parity(self) -> None:
        self._assert_method_parity(self._name("typing.type_check_only"))

    def test_method_deprecated_parity(self) -> None:
        deco = self._call(self._name("warnings.deprecated"), [StrExpr("gone")])
        self._assert_method_parity(deco)

    def test_method_none_parity(self) -> None:
        self._assert_method_parity(self._name("some.random.decorator"))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCompatMetaclassHelperSuite(Suite):
    """Parity for the Rust compat-helper classifiers (#914, #917).

    Exercises `type_kernel.rust_classify_with_metaclass` and
    `rust_classify_add_metaclass` directly on the scalar facts and asserts a
    gate-on/off differential on `infer_metaclass_and_bases_from_compat_helpers`
    (semanal.py): both the base-side `six.with_metaclass(M, B1, ...)` head
    and the decorator-side `@six.add_metaclass(M)` loop decide in Rust while
    Python applies the side effects.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        from mypy.semanal import _set_native_semanal_visitor_active

        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _name(self, fullname: str) -> NameExpr:
        node = NameExpr(fullname.rsplit(".", 1)[-1])
        node.fullname = fullname
        return node

    def _call(self, callee: Expression, args: list[Expression]) -> CallExpr:
        kinds = [ARG_POS] * len(args)
        return CallExpr(callee, args, kinds, [None] * len(args))

    def test_seam_engages(self) -> None:
        assert self._tk.rust_classify_with_metaclass("six.with_metaclass", 2, True) == 1

    def test_decision_table(self) -> None:
        for name in (
            "six.with_metaclass",
            "future.utils.with_metaclass",
            "past.utils.with_metaclass",
        ):
            assert self._tk.rust_classify_with_metaclass(name, 2, True) == 1
        assert self._tk.rust_classify_with_metaclass("six.with_metaclass", 0, True) == 0
        assert self._tk.rust_classify_with_metaclass("six.with_metaclass", 2, False) == 0
        assert self._tk.rust_classify_with_metaclass("six.add_metaclass", 1, True) == 0
        assert self._tk.rust_classify_with_metaclass("mod.NotWithMeta", 2, True) == 0
        assert self._tk.rust_classify_with_metaclass(None, 2, True) == 0

    def test_add_metaclass_seam_engages(self) -> None:
        assert self._tk.rust_classify_add_metaclass("six.add_metaclass", 1, True) == 1

    def test_add_metaclass_decision_table(self) -> None:
        assert self._tk.rust_classify_add_metaclass("six.add_metaclass", 1, True) == 1
        assert self._tk.rust_classify_add_metaclass("six.add_metaclass", 0, True) == 0
        assert self._tk.rust_classify_add_metaclass("six.add_metaclass", 2, True) == 0
        assert self._tk.rust_classify_add_metaclass("six.add_metaclass", 1, False) == 0
        assert self._tk.rust_classify_add_metaclass("six.with_metaclass", 1, True) == 0
        assert self._tk.rust_classify_add_metaclass("mod.Other", 1, True) == 0
        assert self._tk.rust_classify_add_metaclass(None, 1, True) == 0

    def test_gate_off_defers(self) -> None:
        from mypy import semanal

        callee = self._name("six.with_metaclass")
        call_expr = self._call(
            callee, [self._name("mod.M"), self._name("mod.B1"), self._name("mod.B2")]
        )
        old = semanal._native_semanal_visitor_active
        try:
            semanal._native_semanal_visitor_active = False
            assert semanal._native_with_metaclass_classification(call_expr) is None
        finally:
            semanal._native_semanal_visitor_active = old

    def test_add_metaclass_gate_off_defers(self) -> None:
        from mypy import semanal

        callee = self._name("six.add_metaclass")
        call_expr = self._call(callee, [self._name("mod.M")])
        old = semanal._native_semanal_visitor_active
        try:
            semanal._native_semanal_visitor_active = False
            assert semanal._native_add_metaclass_classification(call_expr) is None
        finally:
            semanal._native_semanal_visitor_active = old

    def _run_method(
        self, base_expr: CallExpr | None = None, decorators: list[Expression] | None = None
    ) -> tuple[list[str | None], str | None, list[str]]:
        from mypy import semanal
        from mypy.nodes import Block, SymbolTable, TypeInfo

        class _Analyzer:
            def __init__(self) -> None:
                self.failures: list[str] = []

            def fail(self, msg: str, _ctx: object, *, code: object = None) -> None:
                self.failures.append(msg)

            def analyze_type_expr(self, expr: Expression) -> None:
                pass

            def visit_name_expr(self, expr: NameExpr) -> None:
                pass

        defn = ClassDef("A", Block([]), None, [])
        defn.fullname = "mod.A"
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        defn.base_type_exprs = [base_expr] if base_expr is not None else []
        defn.decorators = decorators or []
        analyzer = _Analyzer()
        semanal.SemanticAnalyzer.infer_metaclass_and_bases_from_compat_helpers(analyzer, defn)  # type: ignore[arg-type]
        fullnames = [getattr(b, "fullname", None) for b in defn.base_type_exprs]
        metaclass = getattr(defn.metaclass, "fullname", None)
        return (fullnames, metaclass, analyzer.failures)

    def _assert_method_parity(
        self, base_expr: CallExpr | None = None, decorators: list[Expression] | None = None
    ) -> None:
        from mypy import semanal

        old = semanal._native_semanal_visitor_active
        try:
            semanal._native_semanal_visitor_active = False
            off = self._run_method(base_expr, decorators)
            semanal._native_semanal_visitor_active = True
            on = self._run_method(base_expr, decorators)
        finally:
            semanal._native_semanal_visitor_active = old
        assert_equal(on, off, "infer_metaclass_and_bases_from_compat_helpers parity")

    def test_method_with_metaclass_parity(self) -> None:
        callee = self._name("six.with_metaclass")
        base_expr = self._call(
            callee, [self._name("mod.M"), self._name("mod.B1"), self._name("mod.B2")]
        )
        self._assert_method_parity(base_expr)

    def test_method_not_with_metaclass_parity(self) -> None:
        callee = self._name("mod.Other")
        base_expr = self._call(
            callee, [self._name("mod.M"), self._name("mod.B1"), self._name("mod.B2")]
        )
        self._assert_method_parity(base_expr)

    def _decorator(self, fullname: str, args: list[Expression], kinds: list[ArgKind]) -> CallExpr:
        return CallExpr(self._name(fullname), args, kinds, [None] * len(args))

    def test_method_add_metaclass_parity(self) -> None:
        decorator = self._decorator("six.add_metaclass", [self._name("mod.M")], [ARG_POS])
        self._assert_method_parity(decorators=[decorator])

    def test_method_add_metaclass_not_matched_parity(self) -> None:
        decorator = self._decorator("mod.Other", [self._name("mod.M")], [ARG_POS])
        self._assert_method_parity(decorators=[decorator])

    def test_method_add_metaclass_wrong_arity_parity(self) -> None:
        decorator = self._decorator(
            "six.add_metaclass", [self._name("mod.M"), self._name("mod.B")], [ARG_POS, ARG_POS]
        )
        self._assert_method_parity(decorators=[decorator])

    def test_method_add_metaclass_non_positional_parity(self) -> None:
        decorator = self._decorator("six.add_metaclass", [self._name("mod.M")], [ARG_NAMED])
        self._assert_method_parity(decorators=[decorator])


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMagicBaseSuite(Suite):
    """Parity for the magic-base skip and core-builtin gate classifiers
    (semanal_bases.rs). Direct seam calls only; the Python shim keeps the
    continue / side effects.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk

    def _is_magic(self, base_expr: Expression) -> bool:
        from mypy.types import TPDICT_NAMES, TYPED_NAMEDTUPLE_NAMES

        return self._tk.rust_is_magic_base(base_expr, TYPED_NAMEDTUPLE_NAMES, TPDICT_NAMES)

    def test_magic_namedtuple_refexpr(self) -> None:
        for fullname in (
            "typing.NamedTuple",
            "typing_extensions.NamedTuple",
            "typing.TypedDict",
            "typing_extensions.TypedDict",
            "mypy_extensions.TypedDict",
        ):
            node = NameExpr(fullname.rsplit(".", 1)[-1])
            node.fullname = fullname
            assert self._is_magic(node) is True, fullname
        node = NameExpr("Foo")
        node.fullname = "mod.Foo"
        assert self._is_magic(node) is False

    def test_magic_tpdict_callexpr(self) -> None:
        callee = NameExpr("TypedDict")
        callee.fullname = "typing.TypedDict"
        call = CallExpr(callee, [StrExpr("X")], [ARG_POS], [None])
        assert self._is_magic(call) is True
        callee = NameExpr("NamedTuple")
        callee.fullname = "typing.NamedTuple"
        call = CallExpr(callee, [StrExpr("X")], [ARG_POS], [None])
        assert self._is_magic(call) is False

    def test_core_builtin(self) -> None:
        from mypy.semanal import CORE_BUILTIN_CLASSES

        assert (
            self._tk.rust_is_core_builtin_class("builtins", "object", CORE_BUILTIN_CLASSES) is True
        )
        assert self._tk.rust_is_core_builtin_class("mod", "object", CORE_BUILTIN_CLASSES) is False
        assert (
            self._tk.rust_is_core_builtin_class("builtins", "Foo", CORE_BUILTIN_CLASSES) is False
        )


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFunctionSignatureSuite(Suite):
    """Parity for the Rust `check_function_signature` count-arbitration port.

    `SemanticAnalyzer.check_function_signature` (semanal.py:2072) compares
    `len(sig.arg_types)` against `len(fdef.arguments)` and picks one of
    three branches: too few (extend sig.arg_types with dummy Any + fail),
    too many (fail blocker=True), or ok (no-op). The Rust classifier
    (`semanal_checks.rs`) turns the two counts into a branch tag; the Python
    shim applies the side effects and keeps the pure-Python body as the
    fallback.

    Direct seam calls assert the exact tag for every branch; the gate-off
    vs gate-on differential drives the real SemanticAnalyzer method through
    a stub fail recorder and asserts identical (fail records, sig length)
    pairs.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_visitor_active

        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _tag(self, sig_len: int, args_len: int) -> int | None:
        return _type_kernel.rust_classify_function_signature(sig_len, args_len)

    def _fdef(self, n_args: int, sig_arg_types: list[Type]) -> FuncDef:
        fx = TypeFixture()
        ret = AnyType(TypeOfAny.special_form)
        sig = CallableType(
            list(sig_arg_types),
            [ARG_POS] * len(sig_arg_types),
            [None] * len(sig_arg_types),
            ret,
            fx.function,
        )
        args = [Argument(Var(f"a{i}"), None, None, ARG_POS) for i in range(n_args)]
        fdef = FuncDef("f", args, None, sig)
        return fdef

    def _run(self, n_args: int, sig_arg_types: list[Type]) -> tuple[object, object]:
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> tuple[object, object]:
            sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
            msgs: list[tuple[str, bool]] = []
            sa.fail = lambda msg, ctx, serious=False, blocker=False, code=None: msgs.append(  # type: ignore[method-assign, misc]
                (str(msg), blocker)
            )
            fdef = self._fdef(n_args, list(sig_arg_types))
            sa.check_function_signature(fdef)
            sig = fdef.type
            assert isinstance(sig, CallableType)
            return (list(msgs), len(sig.arg_types))

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, n_args: int, sig_arg_types: list[Type]) -> None:
        off, on = self._run(n_args, sig_arg_types)
        assert_equal(
            on, off, f"check_function_signature parity n_args={n_args} sig={sig_arg_types}"
        )

    def test_seam_ok(self) -> None:
        assert self._tag(0, 0) == 0
        assert self._tag(3, 3) == 0

    def test_seam_too_few(self) -> None:
        assert self._tag(0, 1) == 1
        assert self._tag(2, 5) == 1

    def test_seam_too_many(self) -> None:
        assert self._tag(1, 0) == 2
        assert self._tag(5, 2) == 2

    def test_parity_ok(self) -> None:
        fx = TypeFixture()
        self._assert_par(2, [fx.a, fx.b])

    def test_parity_too_few(self) -> None:
        fx = TypeFixture()
        self._assert_par(3, [fx.a, fx.b])

    def test_parity_too_many(self) -> None:
        fx = TypeFixture()
        self._assert_par(1, [fx.a, fx.b])

    def test_parity_zero_zero(self) -> None:
        self._assert_par(0, [])


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFixedArgsSuite(Suite):
    """Parity for the Rust `check_fixed_args` arbitration port.

    `SemanticAnalyzer.check_fixed_args` (semanal.py:6962) checks two gaps:
    `len(expr.args) != numargs` (wrong count) and
    `expr.arg_kinds != [ARG_POS]*numargs` (wrong kinds), returning bool.
    The Rust classifier (`semanal_checks.rs`) turns those into a tag; the
    Python shim applies the `self.fail` side effect per the tag.

    Direct seam calls assert the exact tag for every branch; the gate-off
    vs gate-on differential drives the real method through a stub message
    recorder and asserts identical (return, messages) pairs.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        from mypy.semanal import _set_native_semanal_visitor_active

        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _call(self, n_args: int, kinds: list[ArgKind]) -> CallExpr:
        from mypy.nodes import NameExpr

        callee = NameExpr("f")
        args: list[Expression] = [NameExpr(f"a{i}") for i in range(n_args)]
        return CallExpr(callee, args, kinds, [None] * n_args)

    def _seam(self, n_args: int, kinds: list[ArgKind], numargs: int) -> int | None:
        return self._tk.rust_classify_fixed_args(n_args, [k.value for k in kinds], numargs)

    def _run(
        self, n_args: int, kinds: list[ArgKind], numargs: int, name: str = "f"
    ) -> tuple[bool, list[str]]:
        from mypy import semanal

        def check_one() -> tuple[bool, list[str]]:
            call = self._call(n_args, kinds)
            failures: list[str] = []

            class _Analyzer:
                def fail(self, msg: str, ctx: object, *, code: object = None) -> None:
                    failures.append(str(msg))

            analyzer = _Analyzer()
            ret = semanal.SemanticAnalyzer.check_fixed_args(analyzer, call, numargs, name)  # type: ignore[arg-type]
            return ret, failures

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on  # type: ignore[return-value]

    def _assert_par(
        self, n_args: int, kinds: list[ArgKind], numargs: int, name: str = "f"
    ) -> None:
        off, on = self._run(n_args, kinds, numargs, name)
        assert_equal(on, off, f"check_fixed_args parity n_args={n_args} kinds={kinds}")

    def test_seam_ok(self) -> None:
        assert self._seam(2, [ARG_POS, ARG_POS], 2) == 0
        assert self._seam(0, [], 0) == 0
        assert self._seam(1, [ARG_POS], 1) == 0

    def test_seam_wrong_count(self) -> None:
        assert self._seam(1, [ARG_POS], 2) == 1
        assert self._seam(3, [ARG_POS, ARG_POS, ARG_POS], 2) == 1

    def test_seam_wrong_kinds(self) -> None:
        assert self._seam(2, [ARG_POS, ARG_NAMED], 2) == 2
        assert self._seam(2, [ARG_NAMED, ARG_POS], 2) == 2
        assert self._seam(2, [ARG_NAMED, ARG_NAMED], 2) == 2

    def test_parity_ok(self) -> None:
        self._assert_par(2, [ARG_POS, ARG_POS], 2)
        self._assert_par(0, [], 0)
        self._assert_par(1, [ARG_POS], 1, name="g")

    def test_parity_wrong_count(self) -> None:
        self._assert_par(1, [ARG_POS], 2)
        self._assert_par(3, [ARG_POS, ARG_POS, ARG_POS], 2)

    def test_parity_wrong_kinds(self) -> None:
        self._assert_par(2, [ARG_POS, ARG_NAMED], 2)
        self._assert_par(2, [ARG_STAR, ARG_POS], 2)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSemanalVisitorAuditSuite(Suite):
    """Parity for the semanal_visitor deferral-audit decisions (Issue #850).

    The #850 audit walked every `None` site in `crates/type_kernel/src/
    semanal_visitor.rs` and classified each as wire-portable or not. Result:
    no portable sites remain. The 26 defer sites are either output-shape
    defers (attribute-downcast / non-matching AST shape), where `None`
    exactly equals the Python fallback and the seam is semantically complete
    (e.g. `rust_parse_bool` on a non-`builtins.True`/`builtins.False`
    NameExpr, `rust_is_type_ref` on a non-RefExpr), or deep-analyzer defers
    that need live `SemanticAnalyzer` state (`self.lookup` / `self.fail` /
    `is_none_alias` / module re-export visibility / `__getattr__` Var
    synthesis). Those are not wire-portable and stay in Python.

    This suite locks in the decided subset (the gate-only pure-classifier
    seams, which the Python shims let decide without a pre-gate return) by a
    gate-off vs gate-on differential on real `is_type_ref` /
    `can_possibly_be_type_form` / `can_possibly_be_typevarlike_declaration`
    calls, plus direct seam-engagement asserts on the AST-shape defer
    boundaries (engagement asserts documented against the Rust contract for
    each seam). `rust_can_be_type_alias` is exercised at gate-off for the
    deep `is_none_alias` defer and pure-Python fallback on OpExpr unions and
    `CallExpr` aliases (`type(None)`, `None | X`).
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        from mypy.semanal import _set_native_semanal_visitor_active

        self._tk = _tk
        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)
        self._make_sa()

    def tearDown(self) -> None:
        self._set_active(False)

    def _make_sa(self) -> None:
        # A real SemanticAnalyzer built via `__new__` with only the state the
        # seam methods read (NativeTypeExpressionSuite precedent,
        # testtypes.py:16705); the differential compares Rust-vs-true decisions.
        from contextlib import nullcontext

        from mypy.errors import Errors
        from mypy.options import Options
        from mypy.semanal import SemanticAnalyzer

        sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
        sa._is_stub_file = False
        sa.globals = SymbolTable()
        sa.lookup = lambda name, typ, suppress_errors=True: None  # type: ignore[method-assign, assignment]
        sa.is_pep_613 = lambda s: False  # type: ignore[method-assign]
        sa.is_none_alias = lambda node: False  # type: ignore[method-assign]
        sa.errors = Errors(Options())
        sa.isolated_error_analysis = lambda: nullcontext()  # type: ignore[method-assign, return-value, assignment]
        self.sa = sa

    # --- node builders ---

    def _name(self, fullname: str) -> NameExpr:
        node = NameExpr(fullname.rsplit(".", 1)[-1])
        node.fullname = fullname
        return node

    def _member(self, base: str, attr: str) -> MemberExpr:
        node = MemberExpr(self._name(base), attr)
        node.fullname = f"{base}.{attr}"
        return node

    def _instance(self, fullname: str, is_enum: bool = False) -> Any:
        # A minimal TypeInfo stand-in carrying just the fields the Rust seam
        # reads (fullname, is_enum).
        return _FakeTypeInfo(fullname, is_enum)

    def _assign(
        self, lvalues: list[Any], rvalue: Expression, unanalyzed: object = None
    ) -> AssignmentStmt:
        s = AssignmentStmt(lvalues, rvalue)
        s.unanalyzed_type = unanalyzed  # type: ignore[assignment]
        return s

    def _call(
        self, callee: Any, args: list[Expression], kinds: list[ArgKind] | None = None
    ) -> CallExpr:
        if kinds is None:
            kinds = [ARG_POS] * len(args)
        return CallExpr(callee, args, kinds, [None] * len(args))

    # --- differential helpers (gate-off vs gate-on) ---

    def _par(self, fn: Callable[[], object], label: str) -> None:
        self._set_active(False)
        try:
            off = fn()
        finally:
            self._set_active(True)
        on = fn()
        assert_equal(on, off, f"semanal_visitor differential {label}")

    # --- is_type_ref (bare) ---

    def test_is_type_ref_non_refexpr(self) -> None:
        # Non-RefExprs are decided (false) by the Rust seam; Python's
        # isinstance gate falls to False identically.
        self._par(lambda: self.sa.is_type_ref(IntExpr(1), bare=True), "is_type_ref-int")
        self._par(lambda: self.sa.is_type_ref(StrExpr("x"), bare=True), "is_type_ref-str")
        self._par(
            lambda: self.sa.is_type_ref(UnaryExpr("-", IntExpr(1)), bare=True), "is_type_ref-unary"
        )

    def test_is_type_ref_typevarlike_node(self) -> None:
        # A NameExpr whose node is a TypeVarLikeExpr: the seam decides Some(false).
        # Python's true-path also calls `self.fail` (live analyzer state),
        # so this is a seam-engagement assert only, not a differential.
        from mypy.types import AnyType, TypeOfAny

        rv = self._name("m.T")
        rv.node = TypeVarExpr(
            "T", "m.T", [], AnyType(TypeOfAny.special_form), AnyType(TypeOfAny.special_form)
        )
        assert self._tk.rust_is_type_ref(rv, True) is False

    def test_is_type_ref_valid_refs_bare(self) -> None:
        # bare=True: typing.Any / Tuple / Callable are valid.
        for fullname in ("typing.Any", "typing.Tuple", "typing.Callable"):
            rv = self._name(fullname)
            rv.node = self._instance(fullname)
            self._par(
                lambda rv=rv: self.sa.is_type_ref(rv, bare=True),  # type: ignore[misc]
                f"is_type_ref-{fullname}",
            )

    def test_is_type_ref_valid_refs_not_bare(self) -> None:
        # bare=False: the type_constructors set (Union, Optional, Type, ...).
        # Reuse the AutoExpectTypeFixture-like minimal TypeInfo for these.
        for fullname in (
            "typing.Union",
            "typing.Optional",
            "typing.Type",
            "typing.Literal",
            "typing_extensions.Literal",
            "typing.Annotated",
            "typing_extensions.Annotated",
        ):
            rv = self._name(fullname)
            rv.node = self._instance(fullname)
            self._par(
                lambda rv=rv: self.sa.is_type_ref(rv, bare=False),  # type: ignore[misc]
                f"is_type_ref-{fullname}",
            )

    def test_is_type_ref_never_var(self) -> None:
        # A Var node whose fullname is a NEVER_NAME: decided true.
        from mypy.nodes import Var

        rv = self._name("typing.NoReturn")
        v = Var("NoReturn")
        v._fullname = "typing.NoReturn"
        rv.node = v
        self._par(lambda: self.sa.is_type_ref(rv, bare=True), "is_type_ref-never")

    def test_is_type_ref_plain_var_not_never(self) -> None:
        # A plain Var (non-Never): decided false.
        from mypy.nodes import Var

        rv = self._name("m.x")
        v = Var("x")
        v._fullname = "m.x"
        rv.node = v
        self._par(lambda: self.sa.is_type_ref(rv, bare=True), "is_type_ref-plain-var")

    def test_is_type_ref_node_none_defers(self) -> None:
        # The final `None` is the NameExpr/MemberExpr lookup case, which needs
        # self.lookup; audit classified it deep-analyzer (not wire-portable).
        # Direct seam: refexpr with node=None -> returns None (defer).
        rv = self._name("m.x")
        assert self._tk.rust_is_type_ref(rv, True) is None
        assert self._tk.rust_is_type_ref(rv, False) is None
        # MemberExpr with node=None also defers.
        me = self._member("m", "attr")
        assert self._tk.rust_is_type_ref(me, True) is None

    # --- can_possibly_be_type_form ---

    def test_type_form_non_assignment_defers(self) -> None:
        # Non-AssignmentStmt input: the seam returns None (defer).
        assert self._tk.rust_can_possibly_be_type_form(IntExpr(1), False) is None  # type: ignore[arg-type]

    def test_type_form_positive_decided(self) -> None:
        # Lvalue NameExpr, rvalue IndexExpr, no unanalyzed annotation:
        # decided True by both paths.
        s = self._assign(
            [self._name("Alias")], IndexExpr(self._name("typing.Union"), StrExpr("x"))
        )
        self._par(lambda: self.sa.can_possibly_be_type_form(s), "form-index")

    def test_type_form_unanalyzed_not_pep613(self) -> None:
        # unanalyzed_type set and is_pep_613 False -> decided False.
        s = self._assign(
            [self._name("Alias")], IndexExpr(self._name("typing.Union"), StrExpr("x")), object()
        )
        self._par(lambda: self.sa.can_possibly_be_type_form(s), "form-unanalyzed")

    # --- can_possibly_be_typevarlike_declaration ---

    def test_typevarlike_positive_decided(self) -> None:
        # lvalue NameExpr + rvalue CallExpr with NameExpr callee + known
        # TYPE_VAR_LIKE fullname: decided True by both paths. The Python
        # true-path calls `ref.accept(self)` (no-op) then reads fullname.
        s = self._assign(
            [self._name("T")], self._call(self._name("typing.TypeVar"), [StrExpr("T")])
        )
        self._par(lambda: self.sa.can_possibly_be_typevarlike_declaration(s), "tvl-call")

    def test_typevarlike_unknown_callee_decided_false(self) -> None:
        # NameExpr callee whose fullname is NOT in TYPE_VAR_LIKE_NAMES:
        # both paths decide False. MemberExpr-callee is excluded; it would
        # need an analyzed callee fullname on the Python side.
        s = self._assign(
            [self._name("T")], self._call(self._name("m.TypeVarNotSpecial"), [StrExpr("T")])
        )
        self._par(lambda: self.sa.can_possibly_be_typevarlike_declaration(s), "tvl-unknown")

    # --- can_be_type_alias (gate-off pure-Python fallback) ---

    def test_can_be_type_alias_call_expr(self) -> None:
        # `call = type(None)`: the seam returns Some(false), which the shim never
        # trusts, so gate-on and gate-off results are identical (both run the
        # true path; is_none_alias is False on the stub -> False).
        from mypy.semanal import _rust_can_be_type_alias  # type: ignore[attr-defined]

        call = self._call(self._name("builtins.type"), [self._name("builtins.None")])
        assert _rust_can_be_type_alias(call, False, False) is False
        self._par(lambda: self.sa.can_be_type_alias(call), "can_be_type_alias type(None)")

    def test_can_be_type_alias_op_union_defer(self) -> None:
        # `None | X`: the OpExpr arm needs is_none_alias (deep analyzer), so the
        # seam defers on the union; Python evaluates is_none_alias (False on
        # the stub) and both agree (False) through the recursion.
        from mypy.semanal import _rust_can_be_type_alias  # type: ignore[attr-defined]

        op = OpExpr("|", self._name("builtins.None"), self._name("m.X"))
        r = _rust_can_be_type_alias(op, False, False)
        assert r in (None, False), f"union seam result: {r!r}"
        self._par(lambda: self.sa.can_be_type_alias(op), "can_be_type_alias None|X")

    # --- check_typevarlike_name ---

    def test_check_typevarlike_name_valid(self) -> None:
        from mypy.semanal import _rust_check_typevarlike_name  # type: ignore[attr-defined]

        call = self._call(self._name("typing.TypeVar"), [StrExpr("T")])
        r = _rust_check_typevarlike_name(call, "T")
        # ArgKind is a plain Enum, not IntEnum, so pyo3's `extract::<i64>` always
        # fails and the seam defers; the shim only trusts `Some`, so behavior
        # is preserved but the seam stays deferred (audit finding from #850).
        assert r is None, f"check_typevarlike_name valid: {r!r}"

    def test_check_typevarlike_name_mismatch_defers(self) -> None:
        from mypy.semanal import _rust_check_typevarlike_name  # type: ignore[attr-defined]

        call = self._call(self._name("typing.TypeVar"), [StrExpr("U")])
        r = _rust_check_typevarlike_name(call, "T")
        # Same plain-Enum arg_kinds defer: the mismatch decision is never
        # produced; Python emits the error itself.
        assert r is None, f"mismatch: {r!r}"

    def test_check_typevarlike_name_not_call_defers(self) -> None:
        from mypy.semanal import _rust_check_typevarlike_name  # type: ignore[attr-defined]

        assert _rust_check_typevarlike_name(IntExpr(1), "T") is None  # type: ignore[arg-type]

    # --- parse_bool ---

    def test_parse_bool_true(self) -> None:
        assert self.sa.parse_bool(self._name("builtins.True")) is True

    def test_parse_bool_false(self) -> None:
        assert self.sa.parse_bool(self._name("builtins.False")) is False

    def test_parse_bool_other_defers(self) -> None:
        # A NameExpr with a non-bool fullname, or a non-NameExpr, defers to
        # Python's parse_bool (which returns None for non-bool literals).
        assert self.sa.parse_bool(self._name("m.x")) is None
        assert self.sa.parse_bool(IntExpr(1)) is None

    # --- var_is_typing_special_form (staticmethod, drives the seam) ---

    def test_var_is_typing_special_form(self) -> None:
        from mypy.nodes import Var
        from mypy.semanal import SemanticAnalyzer

        for fullname, expected in (
            ("typing.Callable", True),
            ("typing.Union", True),
            ("typing_extensions.TypeGuard", True),
            ("typing.Literal", True),
            ("typing.something_else", False),
        ):
            v = Var(fullname.rsplit(".", 1)[-1])
            v._fullname = fullname
            assert SemanticAnalyzer.var_is_typing_special_form(v) is expected, fullname

    def test_var_is_typing_special_form_nonvar_false(self) -> None:
        # A non-Var object -> Rust returns false; the staticmethod's Python
        # fallback would raise AttributeError, caught by the seam wrapper.
        assert self._tk.rust_var_is_typing_special_form(IntExpr(1)) is False

    # --- refers_to_fullname differential (module-level gated seam) ---

    def test_refers_to_fullname(self) -> None:
        from mypy.semanal import refers_to_fullname

        n = self._name("builtins.int")
        self._par(lambda: refers_to_fullname(n, "builtins.int"), "refers_to_fullname-hit")
        self._par(lambda: refers_to_fullname(n, "builtins.str"), "refers_to_fullname-miss")
        # MemberExpr form.
        m = self._member("m", "attr")
        self._par(lambda: refers_to_fullname(m, "m.attr"), "refers_to_fullname-member")

    def test_refers_to_fullname_nonrefexpr(self) -> None:
        from mypy.semanal import refers_to_fullname

        # Non-RefExprs: both paths return False.
        self._par(lambda: refers_to_fullname(IntExpr(1), "builtins.int"), "refers_to_fullname-int")

    # --- retired-predicate direct seams (#1698): shims retired, pyfunctions
    # stay registered, and these direct tests keep them exercised. ---

    def test_retired_is_trivial_body_direct(self) -> None:
        assert self._tk.rust_is_trivial_body(Block([PassStmt()])) is True
        assign = self._assign([NameExpr("x")], IntExpr(1))
        assert self._tk.rust_is_trivial_body(Block([assign])) is False

    def test_retired_is_valid_replacement_direct(self) -> None:
        old = SymbolTableNode(GDEF, PlaceholderNode("mod.x", Var("t"), 1))
        new = SymbolTableNode(GDEF, Var("y"))
        assert self._tk.rust_is_valid_replacement(old, new) is True
        assert self._tk.rust_is_valid_replacement(new, old) is False

    def test_retired_is_same_symbol_direct(self) -> None:
        v = Var("a")
        assert self._tk.rust_is_same_symbol(v, v) is True
        assert self._tk.rust_is_same_symbol(Var("a"), Var("b")) is False

    def test_retired_refers_to_fullname_direct(self) -> None:
        n = self._name("builtins.int")
        assert self._tk.rust_refers_to_fullname(n, "builtins.int") is True
        assert self._tk.rust_refers_to_fullname(n, "builtins.str") is False

    def test_retired_refers_to_class_or_function_direct(self) -> None:
        assert self._tk.rust_refers_to_class_or_function(self._name("x")) is False


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeDecoratedFunctionIsMethodSuite(Suite):
    """Parity for the Rust `check_decorated_function_is_method` predicate port.

    `SemanticAnalyzer.check_decorated_function_is_method`
    (semanal.py:2256-2258) is a single bool conjunction:
    `not self.type or self.is_func_scope()`. When true the decorator is
    used outside a method context and `self.fail` fires; when false the
    function is a method and nothing happens. The Rust seam
    (`semanal_checks.rs`) reads live analyzer state (`self.type`,
    `is_func_scope()`) via PyO3 and returns the negation: `Some(true)` =
    method (no-op), `Some(false)` = non-method (fail), `None` = defer.

    Direct seam calls assert the exact decision for every branch; the
    gate-off vs gate-on differential drives the real method through a
    stub fail recorder and asserts identical (fail) pairs.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_active

        self._set_active = _set_native_semanal_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _analyzer(self, *, has_type: bool, func_scope: bool) -> Any:
        from types import SimpleNamespace

        ns = SimpleNamespace()
        ns.type = object() if has_type else None
        ns.is_func_scope = lambda: func_scope
        ns._fail = []
        ns.fail = lambda msg, ctx: ns._fail.append(msg)
        return ns

    def _seam(self, *, has_type: bool, func_scope: bool) -> bool | None:
        ns = self._analyzer(has_type=has_type, func_scope=func_scope)
        return _type_kernel.rust_check_decorated_function_is_method(ns)

    def _run(self, *, has_type: bool, func_scope: bool) -> tuple[list[str], list[str]]:
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> list[str]:
            ns = self._analyzer(has_type=has_type, func_scope=func_scope)
            SemanticAnalyzer.check_decorated_function_is_method(
                ns, "abstractmethod", None  # type: ignore[arg-type]
            )
            fails: list[str] = ns._fail
            return fails

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, *, has_type: bool, func_scope: bool) -> None:
        off, on = self._run(has_type=has_type, func_scope=func_scope)
        assert_equal(
            on,
            off,
            f"check_decorated_function_is_method parity "
            f"(has_type={has_type}, func_scope={func_scope})",
        )

    def test_seam_method_in_class_body(self) -> None:
        assert self._seam(has_type=True, func_scope=False) is True

    def test_seam_not_method_outside_class(self) -> None:
        assert self._seam(has_type=False, func_scope=False) is False

    def test_seam_not_method_in_func_scope(self) -> None:
        assert self._seam(has_type=True, func_scope=True) is False

    def test_seam_not_method_outside_class_and_func(self) -> None:
        assert self._seam(has_type=False, func_scope=True) is False

    def test_parity_method_no_fail(self) -> None:
        self._assert_par(has_type=True, func_scope=False)

    def test_parity_outside_class_fails(self) -> None:
        self._assert_par(has_type=False, func_scope=False)

    def test_parity_func_scope_fails(self) -> None:
        self._assert_par(has_type=True, func_scope=True)

    def test_parity_both_fail(self) -> None:
        self._assert_par(has_type=False, func_scope=True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeLvalueValiditySuite(Suite):
    """Parity for the Rust `check_lvalue_validity` dispatch-head port.

    `SemanticAnalyzer.check_lvalue_validity` (semanal.py:5445) is a 2-way
    scalar dispatch on node kind: TypeVarExpr -> fail "Invalid assignment
    target", TypeInfo -> fail CANNOT_ASSIGN_TO_TYPE, else pass. The Rust
    classifier (`semanal_bases.rs`) reads the live `node` via PyO3
    `is_instance` and returns a branch tag; the Python shim applies the
    `self.fail(...)` side effects.

    Direct seam calls assert the exact tag for every branch; the gate-off
    vs gate-on differential drives the real SemanticAnalyzer method
    through a stub fail recorder and asserts identical message lists.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_visitor_active

        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _tag(self, node: Any) -> int:
        return _type_kernel.rust_classify_lvalue_validity(node)

    def _typeinfo(self) -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        class_def = ClassDef("C", Block([]), None, [])
        class_def.fullname = "mod.C"
        return TypeInfo(SymbolTable(), class_def, "mod")

    def _run(self, node: Any) -> list[str]:
        from mypy.nodes import TempNode
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> list[str]:
            sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
            fails: list[str] = []
            sa.fail = lambda msg, ctx, **kw: fails.append(str(msg))  # type: ignore[method-assign]
            sa.check_lvalue_validity(node, TempNode(AnyType(TypeOfAny.special_form)))
            return fails

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on  # type: ignore[return-value]

    def _assert_par(self, node: Any) -> None:
        off, on = self._run(node)
        assert_equal(on, off, f"check_lvalue_validity parity for node={node!r}")

    def test_seam_typevar_expr(self) -> None:

        tv = TypeVarExpr(
            "T", "T", [], AnyType(TypeOfAny.special_form), AnyType(TypeOfAny.from_omitted_generics)
        )
        assert self._tag(tv) == 1

    def test_seam_typeinfo(self) -> None:
        assert self._tag(self._typeinfo()) == 2

    def test_seam_pass(self) -> None:
        v = Var("x")
        assert self._tag(v) == 0
        assert self._tag(None) == 0

    def test_parity_typevar_expr(self) -> None:

        tv = TypeVarExpr(
            "T", "T", [], AnyType(TypeOfAny.special_form), AnyType(TypeOfAny.from_omitted_generics)
        )
        self._assert_par(tv)

    def test_parity_typeinfo(self) -> None:
        self._assert_par(self._typeinfo())

    def test_parity_var(self) -> None:
        self._assert_par(Var("x"))

    def test_parity_none(self) -> None:
        self._assert_par(None)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeShouldWaitRhsSuite(Suite):
    """Parity for the Rust `should_wait_rhs` predicate port.

    `SemanticAnalyzer.should_wait_rhs` (semanal.py:4179) decides whether an
    assignment rvalue must be deferred (placeholder not yet a typeinfo). It
    dispatches on the rvalue node kind with a bounded descent through
    `IndexExpr.base` and `CallExpr.callee`. The Rust seam
    (`semanal_checks.rs`) reads `final_iteration` and the node-kind
    isinstance tags via PyO3, performs the symbol lookups through the real
    `lookup` / `lookup_qualified` methods (resolver-seam pattern), and
    returns the bool; `None` defers to the pure-Python body.

    Direct seam calls assert the exact decision for every branch; the
    gate-off vs gate-on differential drives the real method through a stub
    analyzer with recording lookups and asserts identical results and
    identical lookup traffic.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_active

        self._set_active = _set_native_semanal_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _sym(self, becomes_typeinfo: bool = False) -> SymbolTableNode:
        ph = PlaceholderNode("m.ph", IntExpr(0), 1, becomes_typeinfo=becomes_typeinfo)
        return SymbolTableNode(MDEF, ph)

    def _analyzer(
        self,
        *,
        final_iteration: bool = False,
        sym: SymbolTableNode | None = None,
        qual_sym: SymbolTableNode | None = None,
    ) -> Any:
        from mypy.semanal import SemanticAnalyzer

        ns = SimpleNamespace()
        ns.final_iteration = final_iteration
        calls: list[tuple[str, Any, bool]] = []
        ns.calls = calls

        def lookup(name: str, ctx: Any) -> SymbolTableNode | None:
            calls.append(("lookup", name, False))
            return sym

        def lookup_qualified(
            name: str, ctx: Any, suppress_errors: bool = False
        ) -> SymbolTableNode | None:
            calls.append(("lookup_qualified", name, suppress_errors))
            return qual_sym

        ns.lookup = lookup
        ns.lookup_qualified = lookup_qualified
        # The pure-Python body recurses through self.should_wait_rhs on
        # IndexExpr.base / CallExpr.callee; bind it so the unbound-method
        # differential works on the stub.
        ns.should_wait_rhs = lambda rv: SemanticAnalyzer.should_wait_rhs(ns, rv)  # type: ignore[arg-type]
        return ns

    def _rv(self, kind: str) -> Any:
        if kind == "name":
            return NameExpr("x")
        if kind == "member":
            return MemberExpr(NameExpr("x"), "y")
        if kind == "member_deep":
            return MemberExpr(MemberExpr(NameExpr("a"), "b"), "c")
        if kind == "member_unchainable":
            return MemberExpr(IntExpr(0), "y")
        if kind == "index":
            return IndexExpr(NameExpr("x"), IntExpr(0))
        if kind == "index_nonref":
            return IndexExpr(IntExpr(0), IntExpr(0))
        if kind == "call":
            return CallExpr(NameExpr("x"), [], [], [])
        if kind == "call_nonref":
            return CallExpr(IntExpr(0), [], [], [])
        assert kind == "other"
        return IntExpr(3)

    def _seam(self, rv: Any, **kwargs: Any) -> bool | None:
        ns = self._analyzer(**kwargs)
        return _type_kernel.rust_should_wait_rhs(ns, rv)

    def _run(self, kind: str, **kwargs: Any) -> list[Any]:
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> list[Any]:
            rv = self._rv(kind)
            ns = self._analyzer(**kwargs)
            result = SemanticAnalyzer.should_wait_rhs(ns, rv)
            return [result, ns.calls]

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return [off, on]

    def _assert_par(self, kind: str, expected: bool, **kwargs: Any) -> None:
        off, on = self._run(kind, **kwargs)
        assert_equal(on, off, f"should_wait_rhs parity (rv={kind}, args={kwargs})")
        assert on[0] == expected, f"should_wait_rhs (rv={kind}) == {on[0]}, want {expected}"

    # Direct seam calls: every branch.

    def test_seam_name_placeholder_waits(self) -> None:
        assert self._seam(self._rv("name"), sym=self._sym(False)) is True

    def test_seam_name_placeholder_typeinfo_no_wait(self) -> None:
        assert self._seam(self._rv("name"), sym=self._sym(True)) is False

    def test_seam_name_missing_symbol_no_wait(self) -> None:
        assert self._seam(self._rv("name"), sym=None) is False

    def test_seam_member_qualified_placeholder_waits(self) -> None:
        assert self._seam(self._rv("member"), qual_sym=self._sym(False)) is True

    def test_seam_member_deep_chain_waits(self) -> None:
        assert self._seam(self._rv("member_deep"), qual_sym=self._sym(False)) is True

    def test_seam_member_unchainable_no_wait(self) -> None:
        # get_member_expr_fullname returns None for IntExpr.base: no lookup.
        assert self._seam(self._rv("member_unchainable"), qual_sym=self._sym(False)) is False

    def test_seam_index_descends_to_ref_base(self) -> None:
        assert self._seam(self._rv("index"), sym=self._sym(False)) is True

    def test_seam_index_nonref_base_no_wait(self) -> None:
        assert self._seam(self._rv("index_nonref"), sym=self._sym(False)) is False

    def test_seam_call_descends_to_ref_callee(self) -> None:
        assert self._seam(self._rv("call"), sym=self._sym(False)) is True

    def test_seam_call_nonref_callee_no_wait(self) -> None:
        assert self._seam(self._rv("call_nonref"), sym=self._sym(False)) is False

    def test_seam_other_kind_no_wait(self) -> None:
        assert self._seam(self._rv("other"), sym=self._sym(False)) is False

    def test_seam_final_iteration_always_false(self) -> None:
        assert self._seam(self._rv("name"), sym=self._sym(False), final_iteration=True) is False
        assert self._seam(self._rv("index"), sym=self._sym(False), final_iteration=True) is False

    # Parity differential: gate-off vs gate-on, results + lookup traffic.

    def test_parity_name_placeholder(self) -> None:
        self._assert_par("name", True, sym=self._sym(False))

    def test_parity_name_placeholder_typeinfo(self) -> None:
        self._assert_par("name", False, sym=self._sym(True))

    def test_parity_name_missing(self) -> None:
        self._assert_par("name", False, sym=None)

    def test_parity_member_placeholder(self) -> None:
        self._assert_par("member", True, qual_sym=self._sym(False))

    def test_parity_member_deep_placeholder(self) -> None:
        self._assert_par("member_deep", True, qual_sym=self._sym(False))

    def test_parity_member_unchainable(self) -> None:
        self._assert_par("member_unchainable", False, qual_sym=self._sym(False))

    def test_parity_index_ref_base(self) -> None:
        self._assert_par("index", True, sym=self._sym(False))

    def test_parity_index_nonref_base(self) -> None:
        self._assert_par("index_nonref", False, sym=self._sym(False))

    def test_parity_call_ref_callee(self) -> None:
        self._assert_par("call", True, sym=self._sym(False))

    def test_parity_call_nonref_callee(self) -> None:
        self._assert_par("call_nonref", False, sym=self._sym(False))

    def test_parity_other(self) -> None:
        self._assert_par("other", False, sym=self._sym(False))

    def test_parity_final_iteration(self) -> None:
        self._assert_par("name", False, sym=self._sym(False), final_iteration=True)
        self._assert_par("index", False, sym=self._sym(False), final_iteration=True)

    # Lookup traffic: the Rust seam must ride the real lookup methods.

    def test_lookup_traffic_name(self) -> None:
        off, on = self._run("name", sym=None)
        assert on[1] == [("lookup", "x", False)]

    def test_lookup_traffic_member_qualified(self) -> None:
        off, on = self._run("member_deep", qual_sym=None)
        assert on[1] == [("lookup_qualified", "a.b.c", True)]

    def test_lookup_traffic_member_unchainable_skips_lookup(self) -> None:
        off, on = self._run("member_unchainable", qual_sym=self._sym(False))
        assert on[1] == []

    def test_lookup_traffic_index_descends_not_lookups(self) -> None:
        # IndexExpr descends into the RefExpr base; the lookup happens one
        # level down (short lookup, not qualified).
        off, on = self._run("index", sym=None)
        assert on[1] == [("lookup", "x", False)]


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeConfigureBasesSuite(Suite):
    """Parity for the Rust `configure_base_classes` classifier + MRO tail port.

    `SemanticAnalyzer.configure_base_classes` (semanal.py:3348-3397) runs one
    ProperType isinstance chain per base (tuple / instance / Any / TypedDict
    / else) plus the disallow_any_unimported and check_for_explicit_any
    predicates, then folds verify_base_classes +
    verify_duplicate_base_classes into a 3-way MRO tail (dummy / any /
    proceed). Rust (`semanal_bases.rs`) owns both decisions; Python keeps
    configure_tuple_base_class, every fail, info.fallback_to_any, info.bases,
    the implicit-object append, and the mro writes. Direct seam calls assert
    the exact tags and flag fold; the gate-off vs gate-on differential drives
    the real SemanticAnalyzer method through stub recorders and asserts
    identical observations.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_visitor_active

        self.fx = TypeFixture()
        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _ser(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    # Movement (a): direct seam tag tests.

    def _classify(
        self,
        bases: list[Type],
        is_newtypes: list[bool],
        disallow_subclassing_any: bool = False,
        disallow_any_unimported: bool = False,
        disallow_any_explicit: bool = False,
        is_typeshed_stub_file: bool = False,
    ) -> list[tuple[int, bool, bool]] | None:
        return _type_kernel.rust_classify_configure_bases(
            [self._ser(b) for b in bases],
            is_newtypes,
            disallow_subclassing_any,
            disallow_any_unimported,
            disallow_any_explicit,
            is_typeshed_stub_file,
        )

    def test_seam_engages_instance(self) -> None:
        from mypy.semanal import _CONFIGURE_INSTANCE, _CONFIGURE_INSTANCE_NEWTYPE_FAIL

        fx = TypeFixture()
        result = self._classify([fx.str_type], [False])
        assert result == [(_CONFIGURE_INSTANCE, False, False)]
        result = self._classify([Instance(fx.oi, [])], [True])
        assert result == [(_CONFIGURE_INSTANCE_NEWTYPE_FAIL, False, False)]

    def test_seam_tuple_and_typeddict_and_invalid(self) -> None:
        from mypy.semanal import (
            _CONFIGURE_INVALID_BASE,
            _CONFIGURE_TUPLE,
            _CONFIGURE_TYPEDDICT_FALLBACK,
        )

        fx = TypeFixture()
        result = self._classify([TupleType([fx.a], fx.o)], [False])
        assert result == [(_CONFIGURE_TUPLE, False, False)]
        result = self._classify([TypedDictType({}, set(), set(), fx.str_type)], [False])
        assert result == [(_CONFIGURE_TYPEDDICT_FALLBACK, False, False)]
        result = self._classify([NoneType()], [False])
        assert result == [(_CONFIGURE_INVALID_BASE, False, False)]

    def test_seam_any_ok_and_fail(self) -> None:
        from mypy.semanal import _CONFIGURE_ANY_FAIL, _CONFIGURE_ANY_OK

        fx = TypeFixture()
        result = self._classify([fx.anyt], [False])
        assert result == [(_CONFIGURE_ANY_OK, False, False)]
        result = self._classify([fx.anyt], [False], disallow_subclassing_any=True)
        assert result == [(_CONFIGURE_ANY_FAIL, False, False)]

    def test_seam_unimported_and_explicit_flag_fold(self) -> None:
        fx = TypeFixture()
        unimported = AnyType(TypeOfAny.from_unimported_type)
        # Instance arg carries the unimported Any; option gates the flag.
        plain = Instance(fx.oi, [fx.a])
        carrying = Instance(fx.oi, [unimported])
        result = self._classify([carrying], [False], disallow_any_unimported=True)
        assert result is not None and result[0][1] is True
        result = self._classify([carrying], [False])
        assert result == [(2, False, False)]
        result = self._classify([plain], [False], disallow_any_unimported=True)
        assert result == [(2, False, False)]
        # check_for_explicit_any: disallow_any_explicit and not typeshed stub.
        # A bare explicit Any base is the ANY_OK (4) arm.
        explicit = AnyType(TypeOfAny.explicit)
        result = self._classify([explicit], [False], disallow_any_explicit=True)
        assert result == [(4, False, True)]
        result = self._classify(
            [explicit], [False], disallow_any_explicit=True, is_typeshed_stub_file=True
        )
        assert result == [(4, False, False)]
        # The unimported walk descends into tuples too.
        result = self._classify(
            [TupleType([unimported], fx.o)], [False], disallow_any_unimported=True
        )
        assert result is not None and result[0][0] == 1 and result[0][1] is True

    def test_seam_defers_on_garbage(self) -> None:
        result = _type_kernel.rust_classify_configure_bases(
            [b"\x63not-a-type"], [False], False, False, False, False
        )
        assert result is None

    # Movement (b)/(c): gate-off vs gate-on differential via the shim.

    def _info(self, name: str = "A") -> TypeInfo:
        from mypy.nodes import Block

        defn = ClassDef(name, Block([]), None, [])
        defn.fullname = f"mod.{name}"
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro.append(info)
        return info

    def _run_case(
        self,
        make_bases: Callable[[TypeInfo], list[tuple[Any, Expression]]],
        gate: bool,
        options: Options | None = None,
    ) -> tuple[object, object, object, object]:
        from types import SimpleNamespace

        from mypy.semanal import SemanticAnalyzer

        def check_one() -> tuple[object, object, object, object]:
            fx = self.fx
            info = self._info()
            defn = info.defn
            records: list[object] = []
            sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
            sa.options = options if options is not None else Options()
            sa._is_typeshed_stub_file = False
            sa.fail = lambda msg, ctx, serious=False, blocker=False, code=None: records.append(  # type: ignore[method-assign, misc]
                ("fail", str(msg), blocker)
            )
            sa.msg = SimpleNamespace(  # type: ignore[assignment]
                unimported_type_becomes_any=lambda prefix, t, ctx: records.append(
                    ("unimported", prefix, str(t))
                ),
                explicit_any=lambda ctx: records.append(("explicit",)),
            )
            sa.object_type = lambda: fx.o  # type: ignore[method-assign]
            sa.get_name_repr_of_expr = lambda e: getattr(e, "name", None)  # type: ignore[method-assign, assignment]
            sa.configure_tuple_base_class = lambda defn, base: base.partial_fallback  # type: ignore[method-assign]
            sa.set_dummy_mro = lambda info: records.append(("dummy_mro",))  # type: ignore[method-assign]
            sa.set_any_mro = lambda info: records.append(("any_mro",))  # type: ignore[method-assign]
            sa.calculate_class_mro = lambda defn, obj: records.append(("mro",))  # type: ignore[method-assign, assignment, misc]
            sa.configure_base_classes(defn, make_bases(info))
            return (
                list(records),
                info.fallback_to_any,
                info.bad_mro,
                [str(b) for b in info.bases],
            )

        return self._with_gate(gate, check_one)

    def _assert_par(
        self,
        make_bases: Callable[[TypeInfo], list[tuple[Any, Expression]]],
        options: Options | None = None,
    ) -> None:
        off = self._run_case(make_bases, gate=False, options=options)
        on = self._run_case(make_bases, gate=True, options=options)
        assert_equal(on, off, f"configure_base_classes parity bases={make_bases}")

    def test_parity_plain_instance(self) -> None:
        fx = self.fx
        self._assert_par(lambda info: [(fx.str_type, NameExpr("B"))])

    def test_parity_implicit_object(self) -> None:
        self._assert_par(lambda info: [])

    def test_parity_newtype_fail(self) -> None:
        nt = TypeInfo(SymbolTable(), self._info("NT").defn, "mod")
        nt.is_newtype = True
        self._assert_par(lambda info: [(Instance(nt, []), NameExpr("NT"))])

    def test_parity_any_ok(self) -> None:
        fx = self.fx
        self._assert_par(lambda info: [(fx.a, NameExpr("Whatevs"))])

    def test_parity_any_fail_named(self) -> None:
        fx = self.fx
        options = Options()
        options.disallow_subclassing_any = True
        self._assert_par(lambda info: [(fx.a, NameExpr("Whatevs"))], options)

    def test_parity_any_fail_member_expr(self) -> None:
        fx = self.fx
        options = Options()
        options.disallow_subclassing_any = True
        expr = MemberExpr(NameExpr("m"), "attr")
        self._assert_par(lambda info: [(fx.a, expr)], options)

    def test_parity_typeddict_fallback(self) -> None:
        fx = self.fx
        self._assert_par(
            lambda info: [(TypedDictType({}, set(), set(), fx.str_type), NameExpr("TD"))]
        )

    def test_parity_tuple_base(self) -> None:
        fx = self.fx
        self._assert_par(lambda info: [(TupleType([fx.a], fx.o), NameExpr("tup"))])

    def test_parity_invalid_base_named_and_anon(self) -> None:
        self._assert_par(lambda info: [(NoneType(), NameExpr("Bad"))])
        self._assert_par(lambda info: [(NoneType(), IntExpr(1))])

    def test_parity_unimported_any_flag(self) -> None:
        fx = self.fx
        options = Options()
        options.disallow_any_unimported = True
        carrying = Instance(fx.oi, [AnyType(TypeOfAny.from_unimported_type)])
        self._assert_par(lambda info: [(carrying, NameExpr("B"))], options)

    def test_parity_explicit_any_flag(self) -> None:
        fx = self.fx
        options = Options()
        options.disallow_any_explicit = True
        self._assert_par(lambda info: [(fx.a, NameExpr("B"))], options)

    def test_parity_cycle_dummy_mro(self) -> None:
        self._assert_par(lambda info: [(Instance(info, []), NameExpr("A"))])

    def test_parity_duplicate_base_any_mro(self) -> None:
        def make(info: TypeInfo) -> list[tuple[ProperType, Expression]]:
            b = self._info("B")
            return [(Instance(b, []), NameExpr("B")), (Instance(b, []), NameExpr("B"))]

        self._assert_par(make)

    def test_parity_clean_mro_proceed(self) -> None:
        def make(info: TypeInfo) -> list[tuple[ProperType, Expression]]:
            b = self._info("B")
            return [(Instance(b, []), NameExpr("B"))]

        self._assert_par(make)

    # Movement (c): deferral audit — the gated shim falls back to Python.

    def test_seam_mro_defers_on_non_typeinfo(self) -> None:
        from types import SimpleNamespace

        from mypy.semanal import _CONFIGURE_MRO_PROCEED

        # A well-formed clean TypeInfo decides PROCEED...
        info = self._info()
        b = self._info("B")
        info.bases = [Instance(b, [])]
        result = _type_kernel.rust_classify_configure_mro(info)
        assert result == (_CONFIGURE_MRO_PROCEED, [], None)
        # ...while an unreadable object defers (None) to the Python verify.
        assert _type_kernel.rust_classify_configure_mro(SimpleNamespace()) is None  # type: ignore[arg-type]
        assert _type_kernel.rust_classify_configure_mro(info.defn) is None  # type: ignore[arg-type]

    def test_seam_mro_direct_tags(self) -> None:
        from mypy.semanal import _CONFIGURE_MRO_ANY, _CONFIGURE_MRO_DUMMY, _CONFIGURE_MRO_PROCEED

        # Self-cycle: info lists itself as a base -> DUMMY with the cyclic index.
        info = self._info()
        info.bases = [Instance(info, [])]
        result = _type_kernel.rust_classify_configure_mro(info)
        assert result == (_CONFIGURE_MRO_DUMMY, [0], None)
        # No bases at all -> proceed (implicit object is a Python-side append).
        result = _type_kernel.rust_classify_configure_mro(self._info())
        assert result == (_CONFIGURE_MRO_PROCEED, [], None)
        info2 = self._info("C")
        b = self._info("B")
        info2.bases = [Instance(b, []), Instance(b, [])]
        result = _type_kernel.rust_classify_configure_mro(info2)
        assert result == (_CONFIGURE_MRO_ANY, [], "B")

    def test_shim_falls_back_when_kernel_symbol_missing(self) -> None:
        import mypy.semanal as semanal_mod

        fx = self.fx
        make: Callable[[TypeInfo], list[tuple[Any, Expression]]] = lambda info: [
            (fx.str_type, NameExpr("B"))
        ]
        saved = semanal_mod._rust_classify_configure_bases  # type: ignore[attr-defined]
        semanal_mod._rust_classify_configure_bases = None  # type: ignore[attr-defined, assignment]
        try:
            off = self._run_case(make, gate=False)
            on = self._run_case(make, gate=True)
        finally:
            semanal_mod._rust_classify_configure_bases = saved  # type: ignore[attr-defined]
        assert_equal(on, off)

    def test_shim_falls_back_when_garbage_blob(self) -> None:
        # An undecodable wire blob must defer the whole seam: the Rust
        # classifier returns None and the shim re-runs the pure body.
        import mypy.semanal as semanal_mod

        fx = self.fx
        saved = semanal_mod._serialize_semanal_type
        semanal_mod._serialize_semanal_type = lambda t: b"\x63garbage"
        try:
            off = self._run_case(lambda info: [(fx.str_type, NameExpr("B"))], gate=False)
            on = self._run_case(lambda info: [(fx.str_type, NameExpr("B"))], gate=True)
        finally:
            semanal_mod._serialize_semanal_type = saved
        assert_equal(on, off)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeDeclaredMetaclassSuite(Suite):
    """Parity for the Rust metaclass resolution decision heads (issue #1037).

    `SemanticAnalyzer.get_declared_metaclass` (semanal.py:3767) runs a
    strictly sequential gate chain (dynamic / name-error / Any-Var /
    placeholder / alias-unwrap / invalid / not-a-metaclass / ok); Rust owns
    the classification and Python applies the four fails plus the
    fill_typevars construction. `SemanticAnalyzer.recalculate_metaclass`
    (semanal.py:3863) folds the protocol-MRO scan and the enum scan into one
    exclusive 4-way tag; Python keeps named_type_or_none, is_enum, and the
    generic-enum fail. Direct seam calls assert the exact tags; the gate-off
    vs gate-on differential drives the real SemanticAnalyzer methods through
    stub recorders and asserts identical observations.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_visitor_active

        self.fx = TypeFixture()
        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _bare_info(self, fullname: str) -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        name = fullname.rsplit(".", 1)[-1]
        defn = ClassDef(name, Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        return info

    def _metaclass_info(self, name: str = "M") -> TypeInfo:
        # A minimal TypeInfo that passes is_metaclass(): its mro carries a
        # fake "builtins.type" class.
        type_info = self._bare_info("builtins.type")
        info = self._bare_info(f"mod.{name}")
        info.mro = [info, type_info]
        return info

    # Movement (a): direct seam tag tests.

    def test_seam_declared_engages_ok(self) -> None:
        from mypy.semanal import _META_OK

        meta = self._metaclass_info()
        result = _type_kernel.rust_classify_declared_metaclass("M", meta, None, meta)
        assert result == _META_OK

    def test_seam_declared_dynamic_and_name_error(self) -> None:
        from mypy.semanal import _META_DYNAMIC, _META_NAME_ERROR

        assert (
            _type_kernel.rust_classify_declared_metaclass(None, None, None, None) == _META_DYNAMIC
        )
        assert (
            _type_kernel.rust_classify_declared_metaclass("M", None, None, None)
            == _META_NAME_ERROR
        )

    def test_seam_declared_any_var(self) -> None:
        from mypy.nodes import Var
        from mypy.semanal import _META_ANY, _META_INVALID

        fx = self.fx
        v = Var("M")
        v.type = fx.anyt
        # Issue #1663: the seam takes the live proper type, not wire bytes.
        result = _type_kernel.rust_classify_declared_metaclass("M", v, fx.anyt, v)
        assert result == _META_ANY
        # A Var symbol whose type is not Any falls into the invalid arm.
        v.type = fx.str_type
        result = _type_kernel.rust_classify_declared_metaclass("M", v, fx.str_type, v)
        assert result == _META_INVALID

    def test_seam_declared_placeholder_and_invalid(self) -> None:
        from mypy.nodes import NameExpr, PlaceholderNode
        from mypy.semanal import _META_DEFER, _META_INVALID, _META_NOT_METACLASS

        p = PlaceholderNode("mod.M", NameExpr("M"), 1)
        assert _type_kernel.rust_classify_declared_metaclass("M", p, None, p) == _META_DEFER
        plain = self._bare_info("mod.NotMeta")
        result = _type_kernel.rust_classify_declared_metaclass("M", plain, None, plain)
        assert result == _META_NOT_METACLASS
        tuple_named = self._metaclass_info()
        tuple_named.tuple_type = TupleType([self.fx.a], self.fx.o)
        result = _type_kernel.rust_classify_declared_metaclass("M", tuple_named, None, tuple_named)
        assert result == _META_INVALID

    def test_seam_declared_defers(self) -> None:
        from mypy.nodes import Var
        from mypy.semanal import _META_INVALID

        # Issue #1663: the seam reads the live proper type, so a non-`Any`
        # live type decides the invalid arm instead of deferring.
        v = Var("M")
        v.type = self.fx.a
        assert _type_kernel.rust_classify_declared_metaclass("M", v, self.fx.a, v) == _META_INVALID
        # Unreadable sym/meta objects defer too.
        assert _type_kernel.rust_classify_declared_metaclass("M", v, None, None) is None

    def test_seam_recalculate_direct_tags(self) -> None:
        from mypy.semanal import (
            _RECALC_ABCMETA,
            _RECALC_ENUM_GENERIC_FAIL,
            _RECALC_IS_ENUM,
            _RECALC_OK,
        )

        info = self._bare_info("mod.A")
        info.mro.append(info)
        assert _type_kernel.rust_classify_recalculate_metaclass(info.defn) == _RECALC_OK
        # A protocol in the mro with no/default metaclass -> ABCMeta tag.
        proto = self._bare_info("mod.P")
        proto.is_protocol = True
        info.mro = [info, proto]
        assert _type_kernel.rust_classify_recalculate_metaclass(info.defn) == _RECALC_ABCMETA
        # An enum-metaclass tag, split on defn.type_vars.
        enum_meta = self._bare_info("mod.EnumMeta2")
        fake_enum_base = self._bare_info("enum.EnumMeta")
        enum_meta.mro = [enum_meta, fake_enum_base]
        info.mro = [info]
        info.metaclass_type = Instance(enum_meta, [])
        assert _type_kernel.rust_classify_recalculate_metaclass(info.defn) == _RECALC_IS_ENUM
        info.defn.type_vars = [self.fx.a]  # type: ignore[list-item]
        result = _type_kernel.rust_classify_recalculate_metaclass(info.defn)
        assert result == _RECALC_ENUM_GENERIC_FAIL

    def test_seam_recalculate_defers(self) -> None:
        from types import SimpleNamespace

        assert _type_kernel.rust_classify_recalculate_metaclass(SimpleNamespace()) is None
        assert _type_kernel.rust_classify_recalculate_metaclass(self._bare_info("mod.A")) is None

    # Movement (b)/(c): gate-off vs gate-on differential via the shims.

    def _run_declared(
        self,
        metaclass_expr: Any,
        sym_node: Any,
        gate: bool,
        disallow_subclassing_any: bool = False,
    ) -> tuple[object, object, object, object]:
        from mypy.nodes import SymbolTableNode
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> tuple[object, object, object, object]:
            records: list[object] = []
            sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
            sa.options = Options()
            sa.options.disallow_subclassing_any = disallow_subclassing_any
            sa.fail = lambda msg, ctx, serious=False, blocker=False, code=None: records.append(  # type: ignore[method-assign, misc]
                ("fail", str(msg))
            )
            sym = SymbolTableNode(0, sym_node) if sym_node is not None else None

            def lookup(name: str, ctx: Any) -> SymbolTableNode | None:
                return sym

            sa.lookup_qualified = lookup  # type: ignore[method-assign, assignment]
            result = sa.get_declared_metaclass("C", metaclass_expr)
            inst = result[0]
            return (str(inst) if inst is not None else None, result[1], result[2], list(records))

        return self._with_gate(gate, check_one)

    def _assert_par_declared(
        self, metaclass_expr: Any, sym_node: Any, disallow_subclassing_any: bool = False
    ) -> None:
        off = self._run_declared(
            metaclass_expr, sym_node, gate=False, disallow_subclassing_any=disallow_subclassing_any
        )
        on = self._run_declared(
            metaclass_expr, sym_node, gate=True, disallow_subclassing_any=disallow_subclassing_any
        )
        assert_equal(on, off, f"get_declared_metaclass parity expr={metaclass_expr}")

    def test_parity_declared_none_expr(self) -> None:
        self._assert_par_declared(None, None)

    def test_parity_declared_valid_class(self) -> None:
        meta = self._metaclass_info()
        self._assert_par_declared(NameExpr("M"), meta)
        self._assert_par_declared(MemberExpr(NameExpr("m"), "Meta"), meta)

    def test_parity_declared_dynamic(self) -> None:
        self._assert_par_declared(IntExpr(1), None)

    def test_parity_declared_lookup_miss(self) -> None:
        self._assert_par_declared(NameExpr("M"), None)

    def test_parity_declared_any_var(self) -> None:
        v = Var("M")
        v.type = self.fx.a
        self._assert_par_declared(NameExpr("M"), v)
        self._assert_par_declared(NameExpr("M"), v, disallow_subclassing_any=True)

    def test_parity_declared_placeholder(self) -> None:
        p = PlaceholderNode("mod.M", NameExpr("M"), 1)
        self._assert_par_declared(NameExpr("M"), p)

    def test_parity_declared_alias_unwrap(self) -> None:
        meta = self._metaclass_info()
        ta = TypeAlias(Instance(meta, []), "mod.meta_alias", "mod", 1, 0)
        self._assert_par_declared(NameExpr("meta_alias"), ta)
        # A PEP 695 style alias does not unwrap -> invalid metaclass.
        ta_312 = TypeAlias(
            Instance(meta, []), "mod.meta_p312", "mod", 1, 0, python_3_12_type_alias=True
        )
        self._assert_par_declared(NameExpr("meta_p312"), ta_312)

    def test_parity_declared_non_class_var(self) -> None:
        v = Var("M")
        v.type = self.fx.str_type
        self._assert_par_declared(NameExpr("M"), v)

    def test_parity_declared_not_metaclass(self) -> None:
        plain = self._bare_info("mod.NotMeta")
        self._assert_par_declared(NameExpr("M"), plain)

    def _run_recalculate(
        self,
        mro_protocols: bool,
        declared: Instance | None,
        enum_meta: TypeInfo | None,
        type_vars: list[Any] | None,
        gate: bool,
    ) -> tuple[object, object, object, object]:
        from mypy.semanal import SemanticAnalyzer

        def check_one() -> tuple[object, object, object, object]:
            records: list[object] = []
            info = self._bare_info("mod.A")
            if mro_protocols:
                proto = self._bare_info("mod.P")
                proto.is_protocol = True
                info.mro = [info, proto]
            else:
                info.mro = [info]
            enum_base = None
            if enum_meta is not None:
                enum_base = self._bare_info("enum.EnumMeta")
                enum_meta.mro = [enum_meta, enum_base]
            abc_meta = self._bare_info("abc.ABCMeta")
            defn = info.defn
            if type_vars is not None:
                defn.type_vars = type_vars
            sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
            sa.fail = lambda msg, ctx, serious=False, blocker=False, code=None: records.append(  # type: ignore[method-assign, misc]
                ("fail", str(msg))
            )

            def named_type_or_none(fullname: str, args: Any = None) -> Instance | None:
                return Instance(abc_meta, [])

            sa.named_type_or_none = named_type_or_none  # type: ignore[method-assign]
            sa.recalculate_metaclass(defn, declared)
            meta_t = info.metaclass_type
            return (
                str(meta_t) if meta_t is not None else None,
                info.is_enum,
                info.declared_metaclass is declared,
                list(records),
            )

        return self._with_gate(gate, check_one)

    def _assert_par_recalculate(
        self,
        mro_protocols: bool = False,
        declared: Instance | None = None,
        enum_meta: TypeInfo | None = None,
        type_vars: list[Any] | None = None,
    ) -> None:
        off = self._run_recalculate(mro_protocols, declared, enum_meta, type_vars, gate=False)
        on = self._run_recalculate(mro_protocols, declared, enum_meta, type_vars, gate=True)
        assert_equal(on, off, "recalculate_metaclass parity")

    def test_parity_recalculate_plain(self) -> None:
        self._assert_par_recalculate()

    def test_parity_recalculate_protocol_abcmeta(self) -> None:
        self._assert_par_recalculate(mro_protocols=True)

    def test_parity_recalculate_enum(self) -> None:
        self._assert_par_recalculate(enum_meta=self._bare_info("mod.EM"))

    def test_parity_recalculate_enum_generic_fail(self) -> None:
        self._assert_par_recalculate(enum_meta=self._bare_info("mod.EM"), type_vars=[self.fx.a])

    def test_parity_recalculate_declared_instance(self) -> None:
        self._assert_par_recalculate(declared=Instance(self._bare_info("mod.M"), []))

    # Movement (d)-adjacent: deferral audits through the gated shims.

    def test_shim_falls_back_when_seam_missing(self) -> None:
        import mypy.semanal as semanal_mod

        meta = self._metaclass_info()
        saved_d = semanal_mod._rust_classify_declared_metaclass  # type: ignore[attr-defined]
        saved_r = semanal_mod._rust_classify_recalculate_metaclass  # type: ignore[attr-defined]
        semanal_mod._rust_classify_declared_metaclass = None  # type: ignore[assignment, attr-defined]
        semanal_mod._rust_classify_recalculate_metaclass = None  # type: ignore[assignment, attr-defined]
        try:
            off = self._run_declared(NameExpr("M"), meta, gate=False)
            on = self._run_declared(NameExpr("M"), meta, gate=True)
            assert_equal(on, off)
            roff = self._run_recalculate(
                mro_protocols=True, declared=None, enum_meta=None, type_vars=None, gate=False
            )
            ron = self._run_recalculate(
                mro_protocols=True, declared=None, enum_meta=None, type_vars=None, gate=True
            )
            assert_equal(ron, roff)
        finally:
            semanal_mod._rust_classify_declared_metaclass = saved_d  # type: ignore[attr-defined]
            semanal_mod._rust_classify_recalculate_metaclass = saved_r  # type: ignore[attr-defined]

    def test_shim_declared_does_not_serialize(self) -> None:
        # Issue #1663: the seam reads the live Var type. `get_declared_metaclass`
        # must stay a parity path even when the serializer is unusable.
        import mypy.semanal as semanal_mod

        v = Var("M")
        v.type = self.fx.a
        saved = semanal_mod._serialize_semanal_type
        calls: list[Type] = []

        # A recording spy, not a raise: the shim swallows AssertionError as a
        # deferral signal, so a raising stub would hide a reintroduced call.
        def spy(t: Type) -> bytes:
            calls.append(t)
            return saved(t)

        semanal_mod._serialize_semanal_type = spy
        try:
            off = self._run_declared(NameExpr("M"), v, gate=False)
            on = self._run_declared(NameExpr("M"), v, gate=True)
        finally:
            semanal_mod._serialize_semanal_type = saved
        assert calls == [], f"get_declared_metaclass serialized {len(calls)} types"
        assert_equal(on, off)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativePrepareMethodSignatureSuite(Suite):
    """Parity for the Rust `prepare_method_signature` dispatch-head port.

    `SemanticAnalyzer.prepare_method_signature` (semanal.py:1543) walks a
    branch chain over the method signature: the `__new__` is_static write,
    the `__init_subclass__`/`__class_getitem__` is_class write, the
    Any-self trivial/replace arms, the Self-vs-explicit-annotation
    redundant/conflict fails, and the static-method-with-Self fail. The
    Rust classifier (`semanal_checks.rs`) returns
    (set_is_static, set_is_class, tag) from live FuncDef facts plus the
    wire-serialized first argument and the shim-precomputed
    `is_expected_self_type` bool; the Python shim applies every write and
    error emission and keeps the pure-Python body as the fallback.

    Direct seam calls assert the exact (flags, tag) tuple for every branch;
    the gate-off vs gate-on differential drives the real
    `prepare_method_signature` through a stub fail recorder and asserts
    identical (fail records, flags, str(func.type)) observations.
    """

    def setUp(self) -> None:
        from mypy.semanal import _set_native_semanal_visitor_active

        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _info(self) -> TypeInfo:
        from mypy.nodes import Block

        defn = ClassDef("C", Block([]), None, [])
        defn.fullname = "mod.C"
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro.append(info)

        info.self_type = Instance(info, [])  # type: ignore[assignment]
        return info

    def _fdef(
        self,
        name: str,
        arg_types: list[Type],
        unanalyzed_arg0: Type | None = None,
        is_static: bool = False,
    ) -> FuncDef:
        fx = TypeFixture()
        sig = CallableType(
            list(arg_types),
            [ARG_POS] * len(arg_types),
            [None] * len(arg_types),
            AnyType(TypeOfAny.special_form),
            fx.function,
        )
        args = [Argument(Var(f"a{i}"), None, None, ARG_POS) for i in range(len(arg_types))]
        fdef = FuncDef(name, args, None, sig)
        if is_static:
            fdef.is_static = True
        if unanalyzed_arg0 is not None:
            fdef.unanalyzed_type = CallableType(
                [unanalyzed_arg0] + list(arg_types[1:]),
                [ARG_POS] * len(arg_types),
                [None] * len(arg_types),
                AnyType(TypeOfAny.special_form),
                fx.function,
            )
        return fdef

    def _tag(
        self,
        fdef: FuncDef,
        self_type: Type | None,
        unanalyzed_kind: int,
        expected_self: bool | None,
        has_self_type: bool,
    ) -> tuple[bool, bool, int] | None:
        assert _type_kernel is not None
        # Issue #1663: the seam takes the live proper type, not wire bytes.
        return _type_kernel.rust_classify_method_signature(
            fdef, self_type, unanalyzed_kind, expected_self, has_self_type
        )

    def _run(
        self,
        name: str,
        arg0: Type,
        has_self_type: bool,
        unanalyzed_arg0: Type | None = None,
        is_static: bool = False,
        expected: bool = False,
    ) -> tuple[object, object]:
        from types import SimpleNamespace

        from mypy.semanal import SemanticAnalyzer

        def check_one() -> tuple[object, ...]:
            msgs = []
            info = self._info()
            sa = SimpleNamespace(
                type=info,
                fail=lambda msg, ctx, serious=False, blocker=None, code=None: msgs.append(
                    (str(msg), code)
                ),
                is_expected_self_type=lambda typ, is_classmethod: expected,
                class_type=lambda t: TypeType.make_normalized(t),
            )
            sa._native_prepare_method_signature = lambda func, i, hst, ft: (
                SemanticAnalyzer._native_prepare_method_signature(
                    sa, func, i, hst, ft  # type: ignore[arg-type]
                )
            )
            fdef = self._fdef(name, [arg0], unanalyzed_arg0, is_static)
            SemanticAnalyzer.prepare_method_signature(sa, fdef, info, has_self_type)  # type: ignore[arg-type]
            return (
                list(msgs),
                fdef.is_static,
                fdef.is_class,
                fdef.is_trivial_self,
                str(fdef.type),
            )

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(
        self,
        name: str,
        arg0: Type,
        has_self_type: bool,
        unanalyzed_arg0: Type | None = None,
        is_static: bool = False,
        expected: bool = False,
    ) -> None:
        off, on = self._run(name, arg0, has_self_type, unanalyzed_arg0, is_static, expected)
        assert_equal(on, off, f"prepare_method_signature parity name={name} arg0={arg0}")

    def test_seam_any_self_trivial(self) -> None:
        fdef = self._fdef("m", [AnyType(TypeOfAny.unannotated)])

        assert self._tag(fdef, AnyType(TypeOfAny.unannotated), 0, None, False) == (False, False, 1)

    def test_seam_any_self_replace(self) -> None:
        fdef = self._fdef("m", [AnyType(TypeOfAny.unannotated)])

        assert self._tag(fdef, AnyType(TypeOfAny.unannotated), 0, None, True) == (False, False, 0)

    def test_seam_new_static(self) -> None:
        static_fdef = self._fdef("__new__", [])
        static_fdef.is_static = True
        assert self._tag(static_fdef, None, 0, None, False) == (True, False, 5)

    def test_seam_new_with_any_self(self) -> None:
        fdef = self._fdef("__new__", [AnyType(TypeOfAny.unannotated)])

        assert self._tag(fdef, AnyType(TypeOfAny.unannotated), 0, None, False) == (True, False, 1)

    def test_seam_class_special_write(self) -> None:
        fx = TypeFixture()
        fdef = self._fdef("__init_subclass__", [fx.a])
        assert self._tag(fdef, fx.a, 0, None, False) == (False, True, 5)

    def test_seam_static_self_fail(self) -> None:
        fx = TypeFixture()
        fdef = self._fdef("m", [fx.a], is_static=True)
        assert self._tag(fdef, None, 0, None, True) == (False, False, 4)

    def test_seam_redundant_and_conflict(self) -> None:
        fx = TypeFixture()
        fdef = self._fdef("m", [fx.str_type], unanalyzed_arg0=fx.str_type)

        assert self._tag(fdef, fx.str_type, 2, True, True) == (False, False, 2)
        assert self._tag(fdef, fx.str_type, 2, False, True) == (False, False, 3)

    def test_seam_ok_tails(self) -> None:
        fx = TypeFixture()
        fdef = self._fdef("m", [fx.a])

        assert self._tag(fdef, fx.a, 0, None, False) == (False, False, 5)
        assert self._tag(fdef, fx.a, 1, None, True) == (False, False, 5)
        assert self._tag(fdef, fx.a, 0, None, True) == (False, False, 5)

    def test_seam_defers(self) -> None:
        fx = TypeFixture()
        fdef = self._fdef("m", [fx.a])

        assert self._tag(fdef, None, 0, None, False) is None
        assert self._tag(fdef, fx.a, 2, None, True) is None

    def test_parity_trivial_self(self) -> None:
        self._assert_par("m", AnyType(TypeOfAny.unannotated), False)

    def test_parity_any_self_replace(self) -> None:
        self._assert_par("m", AnyType(TypeOfAny.unannotated), True)

    def test_parity_new_static_and_replace(self) -> None:
        self._assert_par("__new__", AnyType(TypeOfAny.unannotated), False)
        self._assert_par("__new__", AnyType(TypeOfAny.unannotated), True)

    def test_parity_class_special(self) -> None:
        self._assert_par("__init_subclass__", AnyType(TypeOfAny.unannotated), False)
        self._assert_par("__class_getitem__", AnyType(TypeOfAny.unannotated), True)

    def test_parity_static_self_fail(self) -> None:
        fx = TypeFixture()
        self._assert_par("m", fx.a, True, is_static=True)

    def test_parity_redundant_self(self) -> None:
        fx = TypeFixture()
        self._assert_par("m", fx.str_type, True, unanalyzed_arg0=fx.str_type, expected=True)
        off, on = self._run("m", fx.str_type, True, unanalyzed_arg0=fx.str_type, expected=True)
        on_msgs = on[0]  # type: ignore[index]
        assert isinstance(on_msgs, list) and len(on_msgs) == 1
        assert on_msgs[0][0] == 'Redundant "Self" annotation for the first method argument'
        assert on_msgs[0][1] is not None

    def test_parity_explicit_self_conflict(self) -> None:
        fx = TypeFixture()
        self._assert_par("m", fx.str_type, True, unanalyzed_arg0=fx.str_type, expected=False)

    def test_parity_plain_method_ok(self) -> None:
        fx = TypeFixture()
        self._assert_par("m", fx.a, False)

    def test_parity_unanalyzed_arg0_any_ok(self) -> None:
        fx = TypeFixture()
        self._assert_par("m", fx.a, True, unanalyzed_arg0=AnyType(TypeOfAny.unannotated))

    def test_parity_unanalyzed_not_callable_ok(self) -> None:
        fx = TypeFixture()
        self._assert_par("m", fx.str_type, True, unanalyzed_arg0=None)

    def test_fallback_when_expected_self_raises(self) -> None:
        from mypy.semanal import SemanticAnalyzer

        fx = TypeFixture()

        def check_one() -> tuple[object, ...]:
            info = self._info()

            def boom(typ: Type, is_classmethod: bool) -> bool:
                raise RuntimeError("boom")

            sa = SimpleNamespace(
                type=info,
                fail=lambda msg, ctx, serious=False, blocker=None, code=None: None,
                is_expected_self_type=boom,
                class_type=lambda t: TypeType.make_normalized(t),
            )
            sa._native_prepare_method_signature = lambda func, i, hst, ft: (
                SemanticAnalyzer._native_prepare_method_signature(
                    sa, func, i, hst, ft  # type: ignore[arg-type]
                )
            )
            fdef = self._fdef("m", [fx.str_type], unanalyzed_arg0=fx.str_type)

            try:
                SemanticAnalyzer.prepare_method_signature(sa, fdef, info, True)  # type: ignore[arg-type]
            except RuntimeError as exc:
                return ("raised", str(exc))
            return (fdef.is_static, fdef.is_class, fdef.is_trivial_self, str(fdef.type))

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        assert_equal(on, off, "prepare_method_signature deferral parity")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRemoveUnpackKwargsSuite(Suite):
    """Parity for the Rust `remove_unpack_kwargs` arbitration port (#1044).

    `SemanticAnalyzer.remove_unpack_kwargs` (semanal.py:1586) arbitrates a
    `**kw: Unpack[TypedDict]` signature: PASSTHROUGH when the last kind is
    not ARG_STAR2 or the last type is not an UnpackType, NOT_TD_FAIL when
    the Unpack target is not a TypedDict, OVERLAP_FAIL on the sorted
    param/TypedDict-key overlap (minus the trailing kwargs name), and OK
    (rewrite with the TypedDict + unpack_kwargs=True). The Rust classifier
    (`semanal_checks.rs`) decides the tag from the live CallableType's
    arg_kinds/arg_names plus one wire serialization of the last arg type;
    Python applies both fails and all rewrites. Direct seam calls assert
    the exact (tag, names) tuple for every branch; the gate-off vs gate-on
    differential drives the real method through a stub fail recorder and
    asserts identical (str(result), unpack_kwargs, fail messages).
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        self.fx = TypeFixture()
        from mypy.semanal import _set_native_semanal_visitor_active

        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _td(self, keys: list[str]) -> TypedDictType:
        return TypedDictType(
            {k: self.fx.o for k in keys}, set(keys), set(), Instance(self.fx.ai, [])
        )

    def _callable(
        self, arg_names: list[str | None], last_type: Type, arg_kinds: list[ArgKind] | None = None
    ) -> CallableType:
        if arg_kinds is None:
            arg_kinds = [ARG_POS] * (len(arg_names) - 1) + [ARG_STAR2] if arg_names else []
        arg_types: list[Type] = [self.fx.a] * len(arg_names)
        if arg_names:
            arg_types[-1] = last_type
        return CallableType(arg_types, arg_kinds, list(arg_names), self.fx.a, self.fx.function)

    def _wire(self, t: Type) -> bytes | None:
        from mypy.semanal import _serialize_semanal_type

        try:
            return _serialize_semanal_type(t)
        except (AssertionError, NotImplementedError, ValueError, TypeError):
            return None

    def _seam(
        self, arg_names: list[str | None], last_type: Type, wire: bytes | None = None
    ) -> tuple[int, list[str]] | None:
        typ = self._callable(arg_names, last_type)
        # Mirror the shim: serialize the last arg type unless the caller
        # explicitly passes None to test the missing-wire deferral.
        if wire is None and arg_names:
            wire = self._wire(last_type)
        return self._tk.rust_classify_remove_unpack_kwargs(typ, wire)

    def _run(
        self, arg_names: list[str | None], last_type: Type
    ) -> tuple[tuple[str, bool, list[str]], tuple[str, bool, list[str]]]:
        from mypy import semanal

        def check_one() -> tuple[str, bool, list[str]]:
            typ = self._callable(arg_names, last_type)
            failures: list[str] = []

            class _Analyzer:
                def fail(self, msg: str, ctx: object, *, code: object = None) -> None:
                    failures.append(str(msg))

            ret = semanal.SemanticAnalyzer.remove_unpack_kwargs(
                _Analyzer(),  # type: ignore[arg-type]
                object(),  # type: ignore[arg-type]
                typ,
            )
            return str(ret), ret.unpack_kwargs, failures

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(self, arg_names: list[str | None], last_type: Type, label: str) -> None:
        off, on = self._run(arg_names, last_type)
        assert_equal(on, off, f"remove_unpack_kwargs parity {label}")

    # --- direct seam calls ---

    def test_seam_passthrough(self) -> None:
        assert self._seam([], self.fx.a, None) == (0, [])
        plain = self._callable(["x", "kw"], self.fx.a)
        plain.arg_kinds = [ARG_POS, ARG_POS]
        assert self._tk.rust_classify_remove_unpack_kwargs(plain, None) == (0, [])
        assert self._seam(["x", "kw"], self.fx.a) == (0, [])

    def test_seam_not_td_fail(self) -> None:
        assert self._seam(["x", "kw"], UnpackType(self.fx.a)) == (1, [])

    def test_seam_overlap_fail(self) -> None:
        # Overlap is param names ∩ TD keys ("z" is only a TD key, "kw" only
        # a param name), minus the trailing kwargs name, sorted.
        assert self._seam(["a", "kw"], UnpackType(self._td(["z", "a"]))) == (2, ["a"])

    def test_seam_ok(self) -> None:
        assert self._seam(["x", "kw"], UnpackType(self._td(["y"]))) == (3, [])
        # The 'kwargs' key is OK: it equals arg_names[-1] and is discarded.
        assert self._seam(["a", "kw"], UnpackType(self._td(["kw"]))) == (3, [])

    def test_seam_defers_on_missing_wire(self) -> None:
        # last kind is ARG_STAR2 but serialization failed: defer (None).
        typ = self._callable(["x", "kw"], UnpackType(self._td(["y"])))
        assert self._tk.rust_classify_remove_unpack_kwargs(typ, None) is None

    # --- gate-off vs gate-on differential ---

    def test_parity_passthrough(self) -> None:
        self._assert_par(["x", "kw"], self.fx.a, "not unpack")
        self._assert_par([], self.fx.a, "empty kinds")

    def test_parity_not_td(self) -> None:
        self._assert_par(["x", "kw"], UnpackType(self.fx.a), "not typeddict")

    def test_parity_overlap(self) -> None:
        self._assert_par(["a", "kw"], UnpackType(self._td(["z", "a"])), "overlap")
        # Order of the formatted names is sorted across both gates.
        self._assert_par(["b", "a", "kw"], UnpackType(self._td(["b", "a"])), "two overlap")

    def test_parity_ok(self) -> None:
        self._assert_par(["x", "kw"], UnpackType(self._td(["y"])), "ok")
        # TypedDict key named 'kwargs' is fine.
        self._assert_par(["a", "kw"], UnpackType(self._td(["kw"])), "kwargs key")

    def test_gate_on_engages(self) -> None:
        from mypy import semanal

        typ = self._callable(["x", "kw"], UnpackType(self._td(["y"])))
        decided = self._tk.rust_classify_remove_unpack_kwargs(typ, self._wire(typ.arg_types[-1]))
        assert decided is not None, "direct seam returned None"

        class _FailRecorder:
            def fail(self, msg: str, ctx: object, *, code: object = None) -> None:
                pass

        ret = semanal.SemanticAnalyzer.remove_unpack_kwargs(
            _FailRecorder(),  # type: ignore[arg-type]
            object(),  # type: ignore[arg-type]
            typ,
        )
        assert ret.unpack_kwargs
        assert isinstance(get_proper_type(ret.arg_types[-1]), TypedDictType)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeDecidedNoneSuite(Suite):
    """Issue #1101: the binder/constant_fold seams return a (decided, value)
    wire answer so a genuine no-result answer skips the Python walk.

    Direct seam calls prove the decided marker (including decided-None);
    gate-off vs gate-on differentials prove the public functions
    (`mypy.binder.get_declaration`, `mypy.constant_fold.constant_fold_expr`)
    answer identically either way.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        import mypy.binder as _binder_mod
        import mypy.constant_fold as _fold_mod

        self._binder_mod = _binder_mod
        self._fold_mod = _fold_mod
        self.fx = TypeFixture()

    # --- binder.get_declaration decided-None ---

    def _decided_none_exprs(self) -> list[tuple[str, Expression]]:
        """Live expressions whose get_declaration answer is None."""

        exprs: list[tuple[str, Expression]] = []
        # Non-RefExpr.
        exprs.append(("IntExpr", IntExpr(42)))
        # RefExpr with no node.
        exprs.append(("NameExpr no node", NameExpr("x")))
        # Var with no type yet (the common decided-None).
        v = Var("y")
        e = NameExpr("y")
        e.node = v
        exprs.append(("Var without type", e))
        # Var with a PartialType.
        pv = Var("p")
        pv.type = PartialType(None, pv)
        e = NameExpr("p")
        e.node = pv
        exprs.append(("Var with PartialType", e))
        # Node neither Var nor TypeInfo.
        fn = FuncDef("f")
        e = NameExpr("f")
        e.node = fn
        exprs.append(("FuncDef node", e))
        return exprs

    def test_binder_seam_signals_decided_none(self) -> None:
        for label, e in self._decided_none_exprs():
            decided, rust = self._tk.rust_get_declaration(e)
            assert decided is True, f"{label}: Rust did not decide ({decided!r})"
            assert rust is None, f"{label}: expected decided-None, got {rust!r}"

    def test_binder_gate_off_on_parity(self) -> None:
        from mypy.binder import get_declaration

        # Decided-None cases.
        for label, e in self._decided_none_exprs():
            old = self._binder_mod._HAS_RUST_BINDER
            try:
                self._binder_mod._HAS_RUST_BINDER = False
                py = get_declaration(e)  # type: ignore[arg-type]
                self._binder_mod._HAS_RUST_BINDER = True
                native = get_declaration(e)  # type: ignore[arg-type]
            finally:
                self._binder_mod._HAS_RUST_BINDER = old
            assert py is None and native is None, f"{label}: {py!r} vs {native!r}"

    def test_binder_gate_off_on_parity_decided_value(self) -> None:
        from mypy.binder import get_declaration

        # Var with a declared type.
        v = Var("x", self.fx.a)
        e = NameExpr("x")
        e.node = v
        # TypeInfo node.
        e2 = NameExpr("A")
        e2.node = self.fx.ai
        for expr in (e, e2):
            old = self._binder_mod._HAS_RUST_BINDER
            try:
                self._binder_mod._HAS_RUST_BINDER = False
                py = get_declaration(expr)
                self._binder_mod._HAS_RUST_BINDER = True
                native = get_declaration(expr)
            finally:
                self._binder_mod._HAS_RUST_BINDER = old
            assert py is not None and native is not None
            assert_equal(str(native), str(py), "gate-on must match gate-off")
        decided, rust = self._tk.rust_get_declaration(e2)
        assert decided is True
        assert_equal(str(rust), str(TypeType(self.fx.a)), "TypeInfo -> TypeType")

    # --- constant_fold decided-None ---

    def test_fold_seam_signals_decided_none(self) -> None:
        # Un-foldable expressions: a name that is not True/False with no
        # final Var node, an OpExpr with un-foldable operands, a unary op on
        # an un-foldable operand, and a node kind the fold never handles.
        name = NameExpr("unknown")
        op = OpExpr("+", NameExpr("a"), IntExpr(3))
        un = UnaryExpr("-", NameExpr("b"))
        # A Var is not an Expression, but the seam accepts it at runtime;
        # the entry is off the stub's declared type on purpose.
        cases: list[tuple[str, Expression]] = [
            ("NameExpr", name),
            ("OpExpr", op),
            ("UnaryExpr", un),
            ("Var", Var("v")),  # type: ignore[list-item]
        ]
        for label, expr in cases:
            decided, rust = self._tk.rust_constant_fold_expr(expr, "mod")
            assert decided is True, f"{label}: Rust did not decide ({decided!r})"
            assert rust is None, f"{label}: expected decided-None, got {rust!r}"

    def test_fold_gate_off_on_parity(self) -> None:
        from mypy.constant_fold import constant_fold_expr

        # A final Var bound in the current module.
        kv = Var("K")
        kv.is_final = True
        kv._fullname = "mod.K"
        kv.final_value = 42
        final_name = NameExpr("K")
        final_name.node = kv
        # A final Var in another module (not bound).
        ov = Var("K")
        ov.is_final = True
        ov._fullname = "other.K"
        ov.final_value = 42
        other_name = NameExpr("K")
        other_name.node = ov

        cases: list[Expression] = [
            IntExpr(7),
            StrExpr("s"),
            NameExpr("True"),
            NameExpr("False"),
            final_name,
            other_name,
            NameExpr("no_node"),
            OpExpr("+", IntExpr(1), IntExpr(2)),
            OpExpr("+", NameExpr("a"), IntExpr(3)),
            UnaryExpr("-", IntExpr(4)),
            UnaryExpr("-", NameExpr("b")),
        ]
        for expr in cases:
            old = self._fold_mod._native_constant_fold_active
            try:
                self._fold_mod._set_native_constant_fold_active(False)
                py = constant_fold_expr(expr, "mod")
                self._fold_mod._set_native_constant_fold_active(True)
                native = constant_fold_expr(expr, "mod")
            finally:
                self._fold_mod._set_native_constant_fold_active(old)
            assert py == native, f"{expr!r}: gate-off {py!r} vs gate-on {native!r}"

        # Direct seam decided answers for the foldable leaves.
        decided, val = self._tk.rust_constant_fold_expr(IntExpr(7), "mod")
        assert decided is True and val == 7
        decided, val = self._tk.rust_constant_fold_expr(NameExpr("True"), "mod")
        assert decided is True and val is True


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeBinderFrameSuite(Suite):
    """Parity tests for the H1b native binder frame-stack store.

    The Rust thread-local store mirrors the unreachable / suppress
    flags on the Python frame stack, with O(1) cached-count queries
    replacing the Python ``any(f.unreachable for f in self.frames)``
    scan. Direct seam tests drive the pyfunctions; the gate-off vs
    gate-on differential proves the shim produces identical results
    through the real ``ConditionalTypeBinder`` interface.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        self._k = kernel
        self._k.rust_binder_reset()

    def tearDown(self) -> None:
        self._k.rust_binder_reset()

    def test_option_default_off_and_not_cache_affecting(self) -> None:
        from mypy.options import OPTIONS_AFFECTING_CACHE, Options

        assert Options().native_binder is False
        assert "native_binder" not in OPTIONS_AFFECTING_CACHE

    def test_new_push_pop_frame_count(self) -> None:
        self._k.rust_binder_new()
        # new() starts with 1 frame (matching Python's __init__)
        assert self._k.rust_binder_frame_count() == 1
        self._k.rust_binder_push_frame()
        assert self._k.rust_binder_frame_count() == 2
        self._k.rust_binder_push_frame()
        assert self._k.rust_binder_frame_count() == 3
        self._k.rust_binder_pop_frame()
        assert self._k.rust_binder_frame_count() == 2
        self._k.rust_binder_pop_frame()
        assert self._k.rust_binder_frame_count() == 1
        # pop_frame on the last frame is a no-op (guards against empty)
        self._k.rust_binder_pop_frame()
        assert self._k.rust_binder_frame_count() == 1

    def test_set_unreachable_and_query(self) -> None:
        self._k.rust_binder_new()
        self._k.rust_binder_push_frame()
        assert self._k.rust_binder_is_unreachable() is False
        self._k.rust_binder_set_unreachable()
        assert self._k.rust_binder_is_unreachable() is True

    def test_unreachable_persists_across_nested_frames(self) -> None:
        self._k.rust_binder_new()
        self._k.rust_binder_push_frame()
        self._k.rust_binder_set_unreachable()
        self._k.rust_binder_push_frame()
        assert self._k.rust_binder_is_unreachable() is True
        self._k.rust_binder_pop_frame()
        assert self._k.rust_binder_is_unreachable() is True
        self._k.rust_binder_pop_frame()
        assert self._k.rust_binder_is_unreachable() is False

    def test_suppress_unreachable_warnings(self) -> None:
        self._k.rust_binder_new()
        self._k.rust_binder_push_frame()
        assert self._k.rust_binder_is_unreachable_warning_suppressed() is False
        self._k.rust_binder_suppress_unreachable_warnings()
        assert self._k.rust_binder_is_unreachable_warning_suppressed() is True

    def test_set_top_unreachable_can_clear(self) -> None:
        self._k.rust_binder_new()
        self._k.rust_binder_push_frame()
        self._k.rust_binder_set_unreachable()
        assert self._k.rust_binder_is_unreachable() is True
        self._k.rust_binder_set_top_unreachable(False)
        assert self._k.rust_binder_is_unreachable() is False

    def test_reset_clears_store(self) -> None:
        self._k.rust_binder_new()
        self._k.rust_binder_push_frame()
        self._k.rust_binder_set_unreachable()
        self._k.rust_binder_suppress_unreachable_warnings()
        self._k.rust_binder_reset()
        # reset drops the store; lazy init creates a fresh 1-frame store
        assert self._k.rust_binder_frame_count() == 1
        assert self._k.rust_binder_is_unreachable() is False
        assert self._k.rust_binder_is_unreachable_warning_suppressed() is False

    # --- gate-off vs gate-on differential through real binder ---

    def _make_binder(self, native: bool) -> Any:
        from mypy.options import Options

        opts = Options()
        opts.native_binder = native
        from mypy.binder import ConditionalTypeBinder

        return ConditionalTypeBinder(opts)

    def test_gate_off_vs_on_is_unreachable(self) -> None:
        off = self._make_binder(False)
        on = self._make_binder(True)
        for b in (off, on):
            b.push_frame()
            assert b.is_unreachable() is False
            b.unreachable()
            assert b.is_unreachable() is True
            b.push_frame()
            assert b.is_unreachable() is True
            # pop_frame(False, 0) calls update_from_options([])
            # which sets frames[-1].unreachable = not [] = True
            b.pop_frame(False, 0)
            assert b.is_unreachable() is True
            # Pop the original unreachable frame; update_from_options([])
            # sets the bottom frame unreachable too.
            b.pop_frame(False, 0)
            assert b.is_unreachable() is True

    def test_gate_off_vs_on_suppress(self) -> None:
        off = self._make_binder(False)
        on = self._make_binder(True)
        for b in (off, on):
            b.push_frame()
            assert b.is_unreachable_warning_suppressed() is False
            b.suppress_unreachable_warnings()
            assert b.is_unreachable_warning_suppressed() is True

    def test_gate_off_vs_on_update_from_options(self) -> None:
        off = self._make_binder(False)
        on = self._make_binder(True)
        for b in (off, on):
            b.push_frame()
            b.unreachable()
            assert b.is_unreachable() is True
            # update_from_options([]) sets frames[-1].unreachable = not [] = True
            b.update_from_options([])
            assert b.is_unreachable() is True
            # update_from_options with a non-empty frames list clears it:
            # frames[-1].unreachable = not frames = not [frame] = False
            from mypy.binder import Frame

            b.update_from_options([Frame(99)])
            assert b.is_unreachable() is False


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRemoveUnpackKwargsLiveSuite(Suite):
    """Parity for the live-object remove_unpack_kwargs seam (#1663).

    The Python shim no longer serializes `typ.arg_types[-1]`: Rust reads
    the live `CallableType` and resolves the unpack target through
    `mypy.types.get_proper_type`, so a `TypeAliasType` target is expanded
    instead of deferred. The trailing-kind guard keeps non-`**kw` calls
    off the FFI and the wire pyfunction stays registered for direct-seam
    tests. Measured on the cold self-check: 28,807 calls / 7,970 wire
    bytes before; the wire payload is gone after.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        self.fx = TypeFixture()
        from mypy.semanal import _set_native_semanal_visitor_active

        self._set_active = _set_native_semanal_visitor_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _td(self, keys: list[str]) -> TypedDictType:
        return TypedDictType(
            {k: self.fx.o for k in keys}, set(keys), set(), Instance(self.fx.ai, [])
        )

    def _callable(
        self, arg_names: list[str | None], last_type: Type, arg_kinds: list[ArgKind] | None = None
    ) -> CallableType:
        if arg_kinds is None:
            arg_kinds = [ARG_POS] * (len(arg_names) - 1) + [ARG_STAR2] if arg_names else []
        arg_types: list[Type] = [self.fx.a] * len(arg_names)
        if arg_names:
            arg_types[-1] = last_type
        return CallableType(arg_types, arg_kinds, list(arg_names), self.fx.a, self.fx.function)

    def _run_shim(self, typ: CallableType) -> tuple[CallableType, list[str]]:
        from mypy import semanal

        failures: list[str] = []

        class _Analyzer:
            def fail(self, msg: str, ctx: object, *, code: object = None) -> None:
                failures.append(str(msg))

        ret = semanal.SemanticAnalyzer.remove_unpack_kwargs(
            _Analyzer(),  # type: ignore[arg-type]
            object(),  # type: ignore[arg-type]
            typ,
        )
        return ret, failures

    def test_live_tags_match_wire(self) -> None:
        from mypy.semanal import _serialize_semanal_type

        cases: list[tuple[list[str | None], Type]] = [
            (["x", "kw"], self.fx.a),
            (["x", "kw"], UnpackType(self.fx.a)),
            (["x", "kw"], UnpackType(self._td(["y"]))),
            (["a", "kw"], UnpackType(self._td(["z", "a"]))),
        ]
        for names, last in cases:
            typ = self._callable(names, last)
            live = self._tk.rust_classify_remove_unpack_kwargs_live(typ)
            wire = self._tk.rust_classify_remove_unpack_kwargs(
                typ, _serialize_semanal_type(typ.arg_types[-1])
            )
            assert live == wire, (names, last)

    def test_live_resolves_alias_target(self) -> None:
        from mypy.semanal import _serialize_semanal_type

        td = self._td(["z", "a"])
        alias = TypeAlias(td, "m.Alias", "m", 1, 1)
        typ = self._callable(["a", "kw"], UnpackType(TypeAliasType(alias, [])))
        assert self._tk.rust_classify_remove_unpack_kwargs_live(typ) == (2, ["a"])
        # The wire variant cannot resolve the alias and defers.
        wire = self._tk.rust_classify_remove_unpack_kwargs(
            typ, _serialize_semanal_type(typ.arg_types[-1])
        )
        assert wire is None

    def test_live_defers_on_unreadable_object(self) -> None:
        assert self._tk.rust_classify_remove_unpack_kwargs_live(cast(Any, object())) is None

    def test_shim_does_not_serialize(self) -> None:
        from mypy import semanal

        calls: list[Type] = []
        orig = semanal._serialize_semanal_type

        def spy(t: Type) -> bytes:
            calls.append(t)
            return orig(t)

        semanal._serialize_semanal_type = spy
        try:
            typ = self._callable(["a", "kw"], UnpackType(self._td(["z", "a"])))
            ret, failures = self._run_shim(typ)
            ok = self._callable(["x", "kw"], UnpackType(self._td(["y"])))
            ret_ok, failures_ok = self._run_shim(ok)
        finally:
            semanal._serialize_semanal_type = orig
        assert calls == []
        assert ret.unpack_kwargs is False
        assert failures == ['Overlap between parameter names and ** TypedDict items: "a"']
        assert ret_ok.unpack_kwargs is True
        assert failures_ok == []

    def test_shim_skips_ffi_without_star2(self) -> None:
        from mypy import semanal

        calls: list[object] = []
        # Dynamic form: the alias is private to semanal and not re-exported,
        # so the self-check (implicit_reexport=False) rejects direct access.
        orig = getattr(semanal, "_rust_classify_remove_unpack_kwargs_live")  # noqa: B009

        def spy(typ: CallableType) -> Any:
            calls.append(typ)
            return orig(typ)

        setattr(semanal, "_rust_classify_remove_unpack_kwargs_live", spy)  # noqa: B010
        try:
            plain = self._callable(["x", "kw"], self.fx.a, [ARG_POS, ARG_POS])
            ret_plain, _ = self._run_shim(plain)
            empty = self._callable([], self.fx.a)
            ret_empty, _ = self._run_shim(empty)
            unpack = self._callable(["x", "kw"], UnpackType(self._td(["y"])))
            ret_unpack, _ = self._run_shim(unpack)
        finally:
            setattr(semanal, "_rust_classify_remove_unpack_kwargs_live", orig)  # noqa: B010
        assert calls == [unpack]
        assert ret_plain is plain
        assert ret_empty is empty
        assert ret_unpack.unpack_kwargs is True


class FlattenLvaluesContractTestCase(TestCase):
    """Pins the semanal contract of the shared flattener (#1688).

    Unlike the checker variant (parity-pinned in NativeFlattenLvaluesSuite),
    the semanal variant drops TupleExpr/ListExpr containers and keeps
    StarExpr nodes as-is.
    """

    def test_semanal_drops_containers(self) -> None:
        from mypy.semanal import flatten_lvalues

        a, b = NameExpr("a"), NameExpr("b")
        tup = TupleExpr([a, b])
        assert flatten_lvalues([tup], unwrap_star=False) == [a, b]

    def test_semanal_keeps_star_expr(self) -> None:
        from mypy.semanal import flatten_lvalues

        a = NameExpr("a")
        star = StarExpr(a)
        assert flatten_lvalues([star], unwrap_star=False) == [star]

    def test_semanal_method_delegates(self) -> None:
        from mypy.semanal import SemanticAnalyzer, flatten_lvalues

        a, b = NameExpr("a"), NameExpr("b")
        sa = SemanticAnalyzer.__new__(SemanticAnalyzer)
        assert sa.flatten_lvalues([TupleExpr([a, b])]) == flatten_lvalues(
            [TupleExpr([a, b])], unwrap_star=False
        )


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassifyMemberResolutionSuite(Suite):
    """Direct pins for rust_classify_member_resolution (#1723).

    The visit_member_expr shim is retired; the pyfunction stays
    registered, and these tests keep it exercised.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk

    def _info(self, name: str = "C") -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        class_def = ClassDef(name, Block([]), None, [])
        class_def.fullname = f"mod.{name}"
        return TypeInfo(SymbolTable(), class_def, "mod")

    def _bound(self, node: Any, attr: str) -> MemberExpr:
        base = NameExpr("b")
        base.node = node
        return MemberExpr(base, attr)

    def _call(self, expr: MemberExpr) -> tuple[str | None, Any]:
        from mypy.nodes import MemberExpr, MypyFile, RefExpr, TypeAlias, TypeInfo

        return self._tk.rust_classify_member_resolution(
            expr, MemberExpr, RefExpr, MypyFile, TypeInfo, TypeAlias
        )

    def test_class_member_hit(self) -> None:
        info = self._info()
        inner = self._info("Inner")
        info.names["attr"] = SymbolTableNode(GDEF, inner)
        code, sym = self._call(self._bound(info, "attr"))
        assert code == "member"
        assert sym is not None

    def test_var_member_decided_negative(self) -> None:
        info = self._info()
        info.names["attr"] = SymbolTableNode(GDEF, Var("v"))
        code, sym = self._call(self._bound(info, "attr"))
        assert code == "none"
        assert sym is None

    def test_unbound_base_defers(self) -> None:
        code, sym = self._call(self._bound(None, "attr"))
        assert code is None
        assert sym is None
