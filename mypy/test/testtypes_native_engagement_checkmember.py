"""Native engagement suites for the checkmember area (`mypy/checkmember.py`).

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
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from collections.abc import Callable
from types import SimpleNamespace
from typing import Any, cast
from unittest import skipUnless

from mypy.nodes import (
    ARG_POS,
    COVARIANT,
    MDEF,
    ArgKind,
    ClassDef,
    Context,
    Decorator,
    FuncDef,
    NameExpr,
    SymbolNode,
    SymbolTableNode,
    TypeInfo,
    Var,
)
from mypy.state import state
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import (
    _HAS_TYPE_KERNEL,
    _NATIVE_WIRE_ENABLED,
    T,
    _base_infos,
    _is_type_info,
)
from mypy.test.typefixture import TypeFixture
from mypy.types import (
    AnyType,
    CallableType,
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
    TypeOfAny,
    TypeType,
    TypeVarId,
    TypeVarLikeType,
    TypeVarType,
    UninhabitedType,
    UnionType,
    get_proper_type,
)


# Moved from mypy/test/testtypes_native_checker.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeProtocolMemberDeferSuite(Suite):
    """Engagement for the `get_protocol_member_inner` defer closures
    ported in issue #1121.

    The protocol-right member loop (is_protocol_implementation_inner)
    previously went back to Python whenever the looked-up member was a
    Decorator node (typeshed `@property` protocol members like
    ``SupportsIndex.__index__`` / ``Sized.__len__``) or was defined on a
    base class of the receiver (the same-class guard of
    member_method_inner). Both shapes are now decided natively:

    * Decorator nodes unwrap to ``.var``; the var's callable type runs
      the same bind + map + expand tail as a plain method, and a property
      member yields the getter's return type (checkmember.py:1966-1982).
      Static methods still defer (analyze_var skips the self bind for
      them, which member_method_inner's strip would get wrong).
    * Base-class-defined members run the same tail with the
      allow_subclass_receiver flag; `map_instance_to_supertype` re-maps
      the receiver to the defining class (the pure-Python
      `analyze_instance_member_access` maps via `method.info`).

    Each test calls the `rust_is_protocol_implementation` seam directly
    (the deferred path needs a live checker_state, mirroring
    NativeProtocolImplementationSuite) and asserts the decided answer.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_plugin_hook_registry
        from mypy.options import Options
        from mypy.plugins.default import DEFAULT_HOOK_FULLNAMES_BY_KIND, DefaultPlugin

        self.fx = TypeFixture()
        self._live_info: dict[str, Any] = {}
        self.resolver: Any = None
        # The decorator arm resolves attribute hooks through the live
        # ChainedPlugin snapshot; install a defaults-only chain so the
        # synthetic member fullnames are provably unhooked.
        import type_kernel as _type_kernel

        registry = _type_kernel.PluginHookRegistry(
            {kind: list(names) for kind, names in DEFAULT_HOOK_FULLNAMES_BY_KIND.items()}
        )
        _set_native_plugin_hook_registry(registry, False, [DefaultPlugin(Options())])

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_plugin_hook_registry

        _set_native_plugin_hook_registry(None, False)

    def _protocol_info(self, fullname: str) -> Any:
        """Build and register a protocol TypeInfo (plain MRO)."""
        info = self.fx.make_type_info(fullname)
        info.mro = [info, self.fx.oi]
        info.is_protocol = True
        self._live_info[fullname] = info
        return info

    def _impl_info(self, fullname: str) -> Any:
        """Build and register a non-protocol TypeInfo (plain MRO)."""
        info = self.fx.make_type_info(fullname)
        info.mro = [info, self.fx.oi]
        self._live_info[fullname] = info
        return info

    def _property_decorator(self, name: str, ret_type: Any, owner: Any) -> Any:
        """A ``@property def name(self) -> ret`` Decorator node owned by
        `owner`, with the getter bound against the owner's instance."""
        from mypy.nodes import Block, Decorator, FuncDef, Var
        from mypy.types import CallableType, Instance

        fd = FuncDef(name, [], Block([]))
        fd.info = owner
        v = Var(name)
        v.info = owner
        v.is_property = True
        v.is_initialized_in_class = True
        v.is_ready = True
        v.is_inferred = False
        v.type = CallableType([Instance(owner, [])], [ARG_POS], [None], ret_type, self.fx.function)
        return Decorator(fd, [], v)

    def _staticmethod_decorator(self, name: str, ret_type: Any, owner: Any) -> Any:
        """A ``@staticmethod def name(...) -> ret`` Decorator node."""
        from mypy.nodes import Block, Decorator, FuncDef, Var
        from mypy.types import CallableType

        fd = FuncDef(name, [], Block([]))
        fd.info = owner
        v = Var(name)
        v.info = owner
        v.is_staticmethod = True
        v.is_initialized_in_class = True
        v.is_ready = True
        v.is_inferred = False
        v.type = CallableType([self.fx.a], [ARG_POS], [None], ret_type, self.fx.function)
        return Decorator(fd, [], v)

    def _method_callable(self, ret: Any, self_type: Any) -> Any:
        from mypy.types import CallableType

        return CallableType([self_type], [ARG_POS], [None], ret, self.fx.function)

    def _base_impl(self, fullname: str, base: Any, member: str, func_types: dict[str, Any]) -> Any:
        """An implementing Info that inherits `member` from `base`."""
        from mypy.types import Instance

        info = self.fx.make_type_info(fullname)
        base.mro = [base, info, self.fx.oi]
        info.mro = [info, base, self.fx.oi]
        info.bases = [Instance(base, [])]
        node = FuncDef(member, [], None)
        node.info = base
        node.type = func_types[member]
        node.line = 1
        node.column = 1
        base.names[member] = SymbolTableNode(MDEF, node)
        self._live_info[fullname] = info
        self._live_info[base.fullname] = base
        return info

    def _build_resolver(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend(list(self._live_info.values()))
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self.resolver.set_live_typeinfo_map(dict(self._live_info))
        set_wire_typeinfo_map(dict(self._live_info))

    def _seam_call(self, left: Any, right: Any) -> bool | None:
        from mypy.subtypes import _serialize_type

        return _type_kernel.rust_is_protocol_implementation(
            _serialize_type(left),
            _serialize_type(right),
            [],
            False,
            False,
            False,
            False,
            False,  # proper_subtype
            True,  # strict_optional
            False,
            False,
            self.resolver,
        )

    def test_property_protocol_member_engages(self) -> None:
        """A protocol whose member is a `@property` Decorator (the
        ``Sized.__len__`` shape) must be decided natively: the sup lookup
        unwraps to the getter and compares its return type against the
        implementation's property value."""
        from mypy.types import Instance

        self._live_info = {}
        p_info = self._protocol_info("mod.P")
        i_info = self._impl_info("mod.I")
        p_info.names["attr"] = SymbolTableNode(
            MDEF, self._property_decorator("attr", self.fx.a, p_info)
        )
        i_info.names["attr"] = SymbolTableNode(
            MDEF, self._property_decorator("attr", self.fx.a, i_info)
        )
        self._build_resolver()
        result = self._seam_call(Instance(i_info, []), Instance(p_info, []))
        assert result is True, f"property-protocol must be decided True, got {result!r}"

    def test_property_protocol_ret_mismatch_false(self) -> None:
        """A property whose getter returns a different type is not an
        implementation: the member VALUE (ret type) is compared, not the
        bound getter callable."""
        from mypy.types import Instance

        self._live_info = {}
        p_info = self._protocol_info("mod.P")
        i_info = self._impl_info("mod.I")
        p_info.names["attr"] = SymbolTableNode(
            MDEF, self._property_decorator("attr", self.fx.a, p_info)
        )
        i_info.names["attr"] = SymbolTableNode(
            MDEF, self._property_decorator("attr", self.fx.o, i_info)
        )
        self._build_resolver()
        result = self._seam_call(Instance(i_info, []), Instance(p_info, []))
        assert result is False, f"wrong-return property must not implement, got {result!r}"

    def test_staticmethod_member_still_defers(self) -> None:
        """A `@staticmethod` member still defers: analyze_var skips the
        self bind for static methods, which the bind tail would get wrong."""
        from mypy.types import Instance

        self._live_info = {}
        p_info = self._protocol_info("mod.P")
        i_info = self._impl_info("mod.I")
        p_info.names["f"] = SymbolTableNode(
            MDEF, self._staticmethod_decorator("f", self.fx.a, p_info)
        )
        i_info.names["f"] = SymbolTableNode(
            MDEF, self._staticmethod_decorator("f", self.fx.a, i_info)
        )
        self._build_resolver()
        result = self._seam_call(Instance(i_info, []), Instance(p_info, []))
        assert result is None, f"staticmethod member must defer, got {result!r}"

    def test_base_class_member_engages(self) -> None:
        """An inherited member (defining class is a base of the receiver)
        must be decided natively: map_instance_to_supertype re-maps the
        receiver to the defining class before the bind."""
        from mypy.types import Instance

        self._live_info = {}
        base = self._impl_info("mod.Base")
        p_info = self._protocol_info("mod.P")
        i = self._base_impl(
            "mod.Sub", base, "f", {"f": self._method_callable(self.fx.a, Instance(base, []))}
        )
        fnode = FuncDef("f", [], None)
        fnode.info = p_info
        fnode.type = self._method_callable(self.fx.a, Instance(p_info, []))
        p_info.names["f"] = SymbolTableNode(MDEF, fnode)
        self._build_resolver()
        result = self._seam_call(Instance(i, []), Instance(p_info, []))
        assert result is True, f"inherited member must be decided True, got {result!r}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckMemberSuite(Suite):
    """Parity tests for the M20 checkmember Rust port.

    Verifies that `rust_bind_self_fast` agrees with the Python
    `bind_self_fast` for CallableType and Overloaded, including the
    keep-unchanged semantics for *args/**kwargs and empty-arg callables,
    and the deferral (None return) for non-callable types. Also verifies
    `rust_instance_fallback`, `rust_has_operator`, `rust_meta_has_operator`,
    and `rust_defined_in_superclass` against the Python originals.
    """

    def setUp(self) -> None:
        import type_kernel as _tk
        from librt.internal import WriteBuffer

        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self._tk = _tk
        self.WriteBuffer = WriteBuffer
        self._set_active = _set_native_checkmember_active
        self._set_resolver = _set_native_checkmember_resolver
        self.fx = TypeFixture()
        # Production class typevars bind TypeVarId(raw_id, namespace=<class
        # fullname>) (types.py:554). Mirror that here so fixture tvars
        # match the Rust expand env keyed on the instance type_ref.
        for info in (self.fx.gi, self.fx.g2i, self.fx.hi):
            for tv in info.defn.type_vars:
                tv.id = TypeVarId(tv.id.raw_id, namespace=info.fullname)
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
        buf = self.WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _make_callable(self, arg_kinds: list[ArgKind], ret: Type | None = None) -> CallableType:

        return CallableType(
            arg_types=[self.fx.o],
            arg_kinds=arg_kinds,
            arg_names=["self"],
            ret_type=ret or self.fx.o,
            fallback=self.fx.function,
            is_bound=False,
        )

    def test_bind_self_fast_plain_callable(self) -> None:
        from mypy.checkmember import bind_self_fast
        from mypy.nodes import ARG_POS

        method = self._make_callable([ARG_POS])
        result = bind_self_fast(method)
        assert isinstance(result, CallableType)
        assert result.is_bound is True
        assert len(result.arg_types) == 0
        assert len(result.arg_kinds) == 0
        assert len(result.arg_names) == 0

    def test_bind_self_fast_preserves_ret_type(self) -> None:
        from mypy.checkmember import bind_self_fast
        from mypy.nodes import ARG_POS

        method = self._make_callable([ARG_POS], ret=self.fx.a)
        result = bind_self_fast(method)
        assert isinstance(result, CallableType)
        assert result.ret_type == self.fx.a

    def test_bind_self_fast_preserves_variables(self) -> None:
        from mypy.checkmember import bind_self_fast
        from mypy.nodes import ARG_POS

        method = CallableType(
            arg_types=[self.fx.o],
            arg_kinds=[ARG_POS],
            arg_names=["self"],
            ret_type=self.fx.t,
            fallback=self.fx.function,
            variables=[self.fx.t],
            is_bound=False,
        )
        result = bind_self_fast(method)
        assert isinstance(result, CallableType)
        assert len(result.variables) == 1

    def test_bind_self_fast_overloaded(self) -> None:
        from mypy.checkmember import bind_self_fast
        from mypy.nodes import ARG_POS

        item1 = self._make_callable([ARG_POS])
        item2 = self._make_callable([ARG_POS])
        overloaded = Overloaded([item1, item2])
        result = bind_self_fast(overloaded)
        assert isinstance(result, Overloaded)
        assert len(result.items) == 2
        for item in result.items:
            assert item.is_bound is True

    def test_bind_self_fast_defers_star_args(self) -> None:
        from mypy.checkmember import bind_self_fast
        from mypy.nodes import ARG_STAR

        method = self._make_callable([ARG_STAR])
        result = bind_self_fast(method)
        assert isinstance(result, CallableType)
        assert result.is_bound is False
        assert len(result.arg_types) == 1

    def test_bind_self_fast_defers_star2(self) -> None:
        from mypy.checkmember import bind_self_fast
        from mypy.nodes import ARG_STAR2

        method = self._make_callable([ARG_STAR2])
        result = bind_self_fast(method)
        assert isinstance(result, CallableType)
        assert result.is_bound is False
        assert len(result.arg_types) == 1

    def test_bind_self_fast_rust_round_trip_plain_callable(self) -> None:
        # New semantics: plain callable comes back bound through the wire.
        from mypy.checkmember import _deserialize_type_for_checkmember
        from mypy.nodes import ARG_POS

        method = self._make_callable([ARG_POS])
        rust_bytes = self._tk.rust_bind_self_fast(self._bytes_of(method))
        assert rust_bytes is not None
        decoded = _deserialize_type_for_checkmember(bytes(rust_bytes))
        assert isinstance(decoded, CallableType)
        assert decoded.is_bound is True
        assert len(decoded.arg_types) == 0

    def test_bind_self_fast_rust_keeps_star_unchanged(self) -> None:
        # *args / **kwargs return the method unchanged (matches Python).
        from mypy.checkmember import _deserialize_type_for_checkmember
        from mypy.nodes import ARG_STAR

        method = self._make_callable([ARG_STAR])
        rust_bytes = self._tk.rust_bind_self_fast(self._bytes_of(method))
        assert rust_bytes is not None
        decoded = _deserialize_type_for_checkmember(bytes(rust_bytes))
        assert isinstance(decoded, CallableType)
        assert decoded.is_bound is False
        assert len(decoded.arg_types) == 1

    def test_bind_self_fast_rust_returns_none_for_non_callable(self) -> None:
        rust_bytes = self._tk.rust_bind_self_fast(self._bytes_of(self.fx.a))
        assert rust_bytes is None

    def test_classify_member_access_instance(self) -> None:
        # Instance -> MA_INSTANCE (0).
        code = self._tk.rust_classify_member_access(self.resolver, self._bytes_of(self.fx.a))
        assert code == 0

    def test_classify_member_access_none(self) -> None:
        code = self._tk.rust_classify_member_access(self.resolver, self._bytes_of(NoneType()))
        assert code == 8

    def test_classify_member_access_union(self) -> None:
        u = UnionType([self.fx.a, self.fx.b])
        code = self._tk.rust_classify_member_access(self.resolver, self._bytes_of(u))
        assert code == 2

    def test_classify_member_access_type_type(self) -> None:
        tt = TypeType(self.fx.a)
        code = self._tk.rust_classify_member_access(self.resolver, self._bytes_of(tt))
        assert code == 4

    def test_classify_member_access_deleted(self) -> None:
        from mypy.types import DeletedType

        dt = DeletedType("x")
        code = self._tk.rust_classify_member_access(self.resolver, self._bytes_of(dt))
        assert code == 10

    def test_classify_member_access_uninhabited(self) -> None:
        ui = UninhabitedType()
        code = self._tk.rust_classify_member_access(self.resolver, self._bytes_of(ui))
        assert code == 11

    def test_bind_self_fast_preserves_name(self) -> None:
        from mypy.checkmember import bind_self_fast
        from mypy.nodes import ARG_POS

        method = CallableType(
            arg_types=[self.fx.o],
            arg_kinds=[ARG_POS],
            arg_names=["self"],
            ret_type=self.fx.o,
            fallback=self.fx.function,
            name="my_method",
            is_bound=False,
        )
        result = bind_self_fast(method)
        assert isinstance(result, CallableType)
        assert result.name == "my_method"

    def test_bind_self_fast_round_trip_multiple_args(self) -> None:
        from mypy.checkmember import bind_self_fast
        from mypy.nodes import ARG_POS

        method = CallableType(
            arg_types=[self.fx.o, self.fx.a, self.fx.b],
            arg_kinds=[ARG_POS, ARG_POS, ARG_POS],
            arg_names=["self", "x", "y"],
            ret_type=self.fx.a,
            fallback=self.fx.function,
            is_bound=False,
        )
        result = bind_self_fast(method)
        assert isinstance(result, CallableType)
        assert result.is_bound is True
        assert len(result.arg_types) == 2
        assert result.arg_names == ["x", "y"]

    def test_has_operator_parity_present(self) -> None:
        from mypy.checkmember import has_operator

        self._set_active(False)
        try:
            expected = has_operator(self.fx.a, "__bool__")
        finally:
            self._set_active(True)
        actual = has_operator(self.fx.a, "__bool__")
        assert actual is True
        assert actual == expected

    def test_has_operator_parity_missing(self) -> None:
        from mypy.checkmember import has_operator

        self._set_active(False)
        try:
            expected = has_operator(self.fx.a, "__add__")
        finally:
            self._set_active(True)
        actual = has_operator(self.fx.a, "__add__")
        assert actual is False
        assert actual == expected

    def test_has_operator_rust_answers_meta_any(self) -> None:
        from mypy.checkmember import has_operator
        from mypy.types import AnyType, TypeOfAny

        # AnyType: Python says True; Rust mirrors it without needing a
        # superclass lookup, so this resolves with no resolver floor.
        assert has_operator(AnyType(TypeOfAny.special_form), "__eq__") is True

    def test_meta_has_operator_gate_retired(self) -> None:
        # #1668 retired this gate. The old pin asserted the Rust
        # default-metaclass answer; the Python body needs live builtins
        # state, which TypeFixture lacks, so only AnyType survives here.
        from mypy import checkmember
        from mypy.checkmember import meta_has_operator

        assert not hasattr(checkmember, "_rust_meta_has_operator")
        assert meta_has_operator(AnyType(TypeOfAny.special_form), "__call__") is True
        # The Rust pyfunction stays registered for direct-seam tests.
        assert (
            self._tk.rust_meta_has_operator(
                self.resolver, self._bytes_of(AnyType(TypeOfAny.special_form)), "__call__"
            )
            is True
        )

    def test_instance_fallback_parity(self) -> None:
        from mypy.checkmember import instance_fallback

        # The gate is retired (#1668), so both sides below are the Python
        # body; the Rust seam is exercised directly instead.
        for t, expected in ((self.fx.a, self.fx.a), (self.fx.lit1, self.fx.a)):
            actual = instance_fallback(t)
            assert isinstance(actual, Instance)
            assert actual is expected
            assert self._tk.rust_instance_fallback(self._bytes_of(t)) is not None

    def test_instance_fallback_rust_tuple_parity(self) -> None:
        from mypy.checkmember import _deserialize_type_for_checkmember, instance_fallback
        from mypy.types import TupleType

        cases = [
            # builtins tuple: Python rebuilds the args (join union) while Rust
            # returns the partial fallback unchanged, so only the class must
            # match.
            (TupleType([self.fx.a, self.fx.b], self.fx.std_tuple), self.fx.std_tuplei),
            # non-builtins fallback: both return the partial fallback unchanged.
            (TupleType([self.fx.a], Instance(self.fx.ai, [])), self.fx.ai),
        ]
        for t, expected_info in cases:
            actual = instance_fallback(t)
            assert isinstance(actual, Instance)
            assert actual.type is expected_info
            rust_bytes = self._tk.rust_instance_fallback(self._bytes_of(t))
            assert rust_bytes is not None
            decoded = _deserialize_type_for_checkmember(bytes(rust_bytes))
            assert isinstance(decoded, Instance)
            assert decoded.type is expected_info

    def test_defined_in_superclass_parity(self) -> None:
        # member_info is captured eagerly when the resolver is built, so a
        # fresh resolver is needed after mutating ai.names.
        from mypy.checkmember import defined_in_superclass
        from mypy.nodes import MDEF, SymbolTableNode, Var

        v = Var("x")
        v.type = self.fx.a
        v.has_explicit_value = True
        self.fx.ai.names["x"] = SymbolTableNode(MDEF, v)
        try:
            local = self._tk.build_native_resolver([self.fx.bi, self.fx.ai, self.fx.oi], [])
            self._set_resolver(local)
            self._set_active(False)
            try:
                expected = defined_in_superclass(self.fx.bi, "x")
                expected_missing = defined_in_superclass(self.fx.bi, "zzz")
            finally:
                self._set_active(True)
            assert defined_in_superclass(self.fx.bi, "x") is True
            assert defined_in_superclass(self.fx.bi, "x") == expected
            assert defined_in_superclass(self.fx.bi, "zzz") is False
            assert expected_missing is False
        finally:
            self._set_resolver(self.resolver)
            del self.fx.ai.names["x"]

    def _member_access_expected(
        self, instance: Instance, method: TypeInfo, signature: CallableType
    ) -> Type:
        # Python baseline for the M20 seam tail:
        # map_instance_to_supertype -> expand_type_by_instance -> freeze
        # (checkmember.py:502-504). Runs with the native checkmember gate

        # off so it always exercises the pure-Python expand path.
        from mypy.expandtype import expand_type_by_instance
        from mypy.maptype import map_instance_to_supertype

        self._set_active(False)
        try:
            mapped = map_instance_to_supertype(instance, method)
            expanded = expand_type_by_instance(signature, mapped)
            from mypy.typeops import freeze_all_type_vars

            freeze_all_type_vars(expanded)
            return expanded
        finally:
            self._set_active(True)

    def _rust_member_access(
        self, instance: Type, signature: Type, method_fullname: str
    ) -> bytes | None:
        result = self._tk.rust_analyze_instance_member_access(
            self.resolver,
            self._bytes_of(instance),
            self._bytes_of(signature),
            method_fullname,
            False,
            False,
        )
        return bytes(result) if result is not None else None

    def _assert_parity_callable(self, decoded: Type, expected: Type) -> None:
        # Structural parity for the M20 seam: decoded comes back from the wire
        # with a process-global builtins.function TypeInfo (see
        # instance_cache.function_type in mypy/typeops.py), which is not the

        # TypeFixture() TypeInfo under test. Comparing the whole CallableType
        # by __eq__ would then fail on fallback identity alone, so compare
        # every field except fallback and verify fallback by name only.
        assert isinstance(decoded, CallableType) and isinstance(expected, CallableType)  # type: ignore[misc]
        assert decoded.name == expected.name
        assert decoded.is_ellipsis_args == expected.is_ellipsis_args
        assert decoded.type_guard == expected.type_guard
        assert decoded.type_is == expected.type_is
        assert decoded.arg_types == expected.arg_types
        assert decoded.arg_names == expected.arg_names
        assert decoded.arg_kinds == expected.arg_kinds
        assert decoded.ret_type == expected.ret_type
        assert decoded.fallback.type.fullname == expected.fallback.type.fullname

    def test_instance_member_access_parity_plain(self) -> None:
        # Non-generic static method: member type is just the ret type.
        from mypy.checkmember import _deserialize_type_for_checkmember

        sig = CallableType(
            arg_types=[], arg_kinds=[], arg_names=[], ret_type=self.fx.a, fallback=self.fx.function
        )
        expected = self._member_access_expected(self.fx.ga, self.fx.gi, sig)
        rust_bytes = self._rust_member_access(self.fx.ga, sig, "G")
        assert rust_bytes is not None
        decoded = _deserialize_type_for_checkmember(rust_bytes)
        assert isinstance(decoded, ProperType)
        self._assert_parity_callable(decoded, expected)
        assert isinstance(decoded, CallableType)
        assert decoded.ret_type == self.fx.a

    def test_instance_member_access_parity_expands(self) -> None:
        # Generic: G[A].foo() -> G[A] substitutes T`1 into the ret type.
        # Use a namespaced tvar (production keys env on the declaring class
        # fullname, types.py:554) so both sides agree.
        from mypy.checkmember import _deserialize_type_for_checkmember

        ns_t = TypeVarType(
            "T",
            "__main__.T",
            TypeVarId(1, namespace="G"),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        ret = Instance(self.fx.gi, [ns_t])
        sig = CallableType(
            arg_types=[], arg_kinds=[], arg_names=[], ret_type=ret, fallback=self.fx.function
        )
        expected = self._member_access_expected(self.fx.ga, self.fx.gi, sig)
        rust_bytes = self._rust_member_access(self.fx.ga, sig, "G")
        assert rust_bytes is not None
        decoded = _deserialize_type_for_checkmember(rust_bytes)
        assert isinstance(decoded, ProperType)
        self._assert_parity_callable(decoded, expected)
        assert isinstance(decoded, CallableType)
        assert decoded.ret_type == self.fx.ga

    def test_instance_member_access_parity_sibling_class_maps(self) -> None:
        # G2[A].foo() where foo is defined on G2 maps the args onto G2[A]
        # even though the instance type is generic.
        from mypy.checkmember import _deserialize_type_for_checkmember

        sig = CallableType(
            arg_types=[], arg_kinds=[], arg_names=[], ret_type=self.fx.a, fallback=self.fx.function
        )
        instance = Instance(self.fx.g2i, [self.fx.a])
        expected = self._member_access_expected(instance, self.fx.g2i, sig)
        rust_bytes = self._rust_member_access(instance, sig, "G2")
        assert rust_bytes is not None
        decoded = _deserialize_type_for_checkmember(rust_bytes)
        assert isinstance(decoded, ProperType)
        self._assert_parity_callable(decoded, expected)
        assert isinstance(decoded, CallableType)
        assert decoded.ret_type == self.fx.a

    def test_instance_member_access_rust_none_for_overloaded(self) -> None:
        item = CallableType(
            arg_types=[], arg_kinds=[], arg_names=[], ret_type=self.fx.a, fallback=self.fx.function
        )
        assert self._rust_member_access(self.fx.ga, Overloaded([item]), "G") is None

    def test_instance_member_access_rust_none_for_non_instance(self) -> None:
        # A non-Instance left operand is not handled: defer.
        sig = CallableType(
            arg_types=[], arg_kinds=[], arg_names=[], ret_type=self.fx.a, fallback=self.fx.function
        )
        result = self._tk.rust_analyze_instance_member_access(
            self.resolver, self._bytes_of(self.fx.o), self._bytes_of(sig), "G", False, False
        )
        assert result is None

    def test_instance_member_access_rust_none_for_unknown_class(self) -> None:
        # A method fullname with no resolver floor entry defers.
        sig = CallableType(
            arg_types=[], arg_kinds=[], arg_names=[], ret_type=self.fx.a, fallback=self.fx.function
        )
        result = self._tk.rust_analyze_instance_member_access(
            self.resolver,
            self._bytes_of(self.fx.ga),
            self._bytes_of(sig),
            "NoSuchClass",
            False,
            False,
        )
        assert result is None

    def _rust_member_method(
        self,
        instance: Type,
        signature: Type,
        method_fullname: str,
        self_type: Type,
        name: str = "foo",
    ) -> bytes | None:
        result = self._tk.rust_analyze_member_method(
            self.resolver,
            self._bytes_of(instance),
            self._bytes_of(signature),
            method_fullname,
            self._bytes_of(self_type),
            name,
            False,
            False,
        )
        return bytes(result) if result is not None else None

    def test_member_method_parity_binds_and_expands(self) -> None:
        # Plain non-trivial method on the defining class: Rust binds self
        # and expands class type vars. G[T].foo(self: G[T]) -> G[T].
        from mypy.checkmember import _deserialize_type_for_checkmember

        ns_t = TypeVarType(
            "T",
            "__main__.T",
            TypeVarId(1, namespace="G"),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        ret = Instance(self.fx.gi, [ns_t])
        sig = CallableType(
            arg_types=[Instance(self.fx.gi, [ns_t]), self.fx.a],
            arg_kinds=[ARG_POS, ARG_POS],
            arg_names=["self", "x"],
            ret_type=ret,
            fallback=self.fx.function,
        )
        rust_bytes = self._rust_member_method(self.fx.ga, sig, "G", self.fx.ga)
        assert rust_bytes is not None
        decoded = _deserialize_type_for_checkmember(rust_bytes)
        assert isinstance(decoded, ProperType)
        assert isinstance(decoded, CallableType)
        # self stripped, is_bound set, G[A] substituted into ret type.
        assert decoded.arg_types == [self.fx.a]
        assert decoded.is_bound
        assert decoded.ret_type == self.fx.ga

    def test_member_method_parity_plain(self) -> None:
        # Non-generic method on the defining class G: receiver G[A] with a
        # concrete self annotation passes the self-arg filter; the shim
        # restores the non-wire fields.
        from mypy.checkmember import _deserialize_type_for_checkmember

        sig = CallableType(
            arg_types=[self.fx.ga, self.fx.b],
            arg_kinds=[ARG_POS, ARG_POS],
            arg_names=["self", "x"],
            ret_type=self.fx.a,
            fallback=self.fx.function,
        )
        rust_bytes = self._rust_member_method(self.fx.ga, sig, "G", self.fx.ga)
        assert rust_bytes is not None
        decoded = _deserialize_type_for_checkmember(rust_bytes)
        assert isinstance(decoded, ProperType)
        assert isinstance(decoded, CallableType)
        assert decoded.arg_types == [self.fx.b]
        assert decoded.is_bound
        assert decoded.ret_type == self.fx.a

    def test_member_method_rust_none_for_classmethod(self) -> None:
        # classmethod true -> defer to Python (needs TypeType wrapping).
        sig = CallableType(
            arg_types=[self.fx.a],
            arg_kinds=[ARG_POS],
            arg_names=["self"],
            ret_type=self.fx.a,
            fallback=self.fx.function,
        )
        result = self._tk.rust_analyze_member_method(
            self.resolver,
            self._bytes_of(self.fx.ga),
            self._bytes_of(sig),
            "G",
            self._bytes_of(self.fx.ga),
            "foo",
            False,
            True,
        )
        assert result is None

    def test_member_method_rust_none_for_foreign_self(self) -> None:
        # self_type not the defining class -> check_self_arg could filter:
        # defer.
        sig = CallableType(
            arg_types=[self.fx.a],
            arg_kinds=[ARG_POS],
            arg_names=["self"],
            ret_type=self.fx.a,
            fallback=self.fx.function,
        )
        result = self._tk.rust_analyze_member_method(
            self.resolver,
            self._bytes_of(self.fx.ga),
            self._bytes_of(sig),
            "G",
            self._bytes_of(self.fx.o),
            "foo",
            False,
            False,
        )
        assert result is None

    def test_member_method_rust_none_for_bindable_noncallable(self) -> None:
        # A non-callable signature cannot be bound: defer.
        result = self._tk.rust_analyze_member_method(
            self.resolver,
            self._bytes_of(self.fx.ga),
            self._bytes_of(self.fx.a),
            "G",
            self._bytes_of(self.fx.ga),
            "foo",
            False,
            False,
        )
        assert result is None

    def test_instance_member_access_trivial_self_binds(self) -> None:
        # Trivial-self method: Rust binds the first arg and marks is_bound.
        from mypy.checkmember import _deserialize_type_for_checkmember

        sig = CallableType(
            arg_types=[self.fx.a, self.fx.o],
            arg_kinds=[ARG_POS, ARG_POS],
            arg_names=["self", "x"],
            ret_type=self.fx.a,
            fallback=self.fx.function,
        )
        rust_bytes = self._tk.rust_analyze_instance_member_access(
            self.resolver, self._bytes_of(self.fx.ga), self._bytes_of(sig), "G", False, True
        )
        assert rust_bytes is not None
        decoded = _deserialize_type_for_checkmember(bytes(rust_bytes))
        # Bound: first arg dropped, is_bound set.
        expected = CallableType(
            arg_types=[self.fx.o],
            arg_kinds=[ARG_POS],
            arg_names=["x"],
            ret_type=self.fx.a,
            fallback=self.fx.function,
        )
        assert isinstance(decoded, CallableType)
        assert decoded.arg_types == expected.arg_types
        assert decoded.arg_kinds == expected.arg_kinds
        assert decoded.arg_names == expected.arg_names
        assert decoded.is_bound
        assert decoded.ret_type == expected.ret_type

    def test_instance_member_access_trivial_self_expands(self) -> None:
        # Trivial-self generic method: Rust expands then binds. Fixture
        # tvars carry namespace "" while production/Rust key on the
        # instance type_ref, so use a namespaced G signature.
        from mypy.checkmember import _deserialize_type_for_checkmember

        ns_t = TypeVarType(
            "T",
            "__main__.T",
            TypeVarId(1, namespace="G"),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        sig = CallableType(
            arg_types=[self.fx.a, ns_t],
            arg_kinds=[ARG_POS, ARG_POS],
            arg_names=["self", "x"],
            ret_type=Instance(self.fx.gi, [ns_t]),
            fallback=self.fx.function,
        )
        rust_bytes = self._tk.rust_analyze_instance_member_access(
            self.resolver, self._bytes_of(self.fx.ga), self._bytes_of(sig), "G", False, True
        )
        assert rust_bytes is not None
        decoded = _deserialize_type_for_checkmember(bytes(rust_bytes))
        assert isinstance(decoded, CallableType)
        # t binds to A (instance arg of G[A]): arg x: T -> A, ret G[T] -> G[A].
        assert decoded.arg_types == [self.fx.a]
        assert decoded.is_bound
        assert decoded.ret_type == self.fx.ga

    def test_expand_without_binding_alias_survives(self) -> None:
        # Wave16: an alias-carrying method signature no longer defers. The
        # alias arg survives expansion (fixup relinks it through the wire
        # alias map), so gate-on must match gate-off.
        import contextlib
        from types import SimpleNamespace

        from mypy.checkmember import (
            MemberContext,
            _deserialize_type_for_checkmember,
            expand_without_binding,
        )
        from mypy.nodes import TypeAlias
        from mypy.wirefixup import set_wire_alias_map

        alias = TypeAlias(self.fx.a, "mod.WA", "mod", -1, -1)
        infos = [self.fx.oi, self.fx.ai, self.fx.bi, self.fx.gi, self.fx.functioni]
        resolver = self._tk.build_native_resolver(infos, [alias])
        var = Var("m")
        var.info = self.fx.gi
        # Method type of G with a G[T] parameter (T bound to the class
        # namespace like a real method): expansion substitutes the
        # receiver's arg (the alias) into the parameter type.
        ns_t = self.fx.gi.defn.type_vars[0]
        typ = CallableType(
            arg_types=[Instance(self.fx.gi, [ns_t])],
            arg_kinds=[ARG_POS],
            arg_names=["self"],
            ret_type=Instance(self.fx.gi, [ns_t]),
            fallback=self.fx.function,
        )
        itype = Instance(self.fx.gi, [TypeAliasType(alias, [])])
        original_itype = Instance(self.fx.gi, [self.fx.a])
        mx = MemberContext(
            is_lvalue=False,
            is_super=False,
            is_operator=False,
            original_type=typ,
            context=NameExpr("x"),
            chk=cast(
                Any,
                SimpleNamespace(
                    msg=SimpleNamespace(
                        fail=lambda *a: None,
                        note=lambda *a: None,
                        filter_errors=lambda *a, **kw: contextlib.nullcontext(),
                        disable_type_names=lambda: contextlib.nullcontext(),
                    )
                ),
            ),
            self_type=None,
        )

        def run() -> str:
            got = expand_without_binding(typ, var, itype, original_itype, mx)
            return str(getattr(got, "arg_types", [None])[0])

        set_wire_alias_map({alias.fullname: alias})
        self._set_resolver(resolver)
        try:
            results = {}
            for active in (False, True):
                self._set_active(active)
                try:
                    results[active] = run()
                finally:
                    self._set_active(True)
        finally:
            set_wire_alias_map(None)
            self._set_resolver(self.resolver)
        assert_equal(results[True], results[False], "alias-carrying expansion parity")
        assert results[False] == "G[A]", results[False]

        # Direct seam: decide on the alias input instead of deferring.
        from mypy.checkmember import _serialize_type_for_checkmember
        from mypy.expandtype import expand_type_by_instance

        self._set_resolver(resolver)
        set_wire_alias_map({alias.fullname: alias})
        try:
            seam = self._tk.rust_expand_without_binding(
                _serialize_type_for_checkmember(typ),
                _serialize_type_for_checkmember(itype),
                False,
                False,
                TypeVarId.next_raw_id,
                state.strict_optional,
                resolver,
            )
            assert seam is not None, "seam deferred on an alias-carrying signature"
            _, _, wire_bytes = seam
            decoded = _deserialize_type_for_checkmember(bytes(wire_bytes), freeze=True)
            assert decoded is not None
            expected = expand_type_by_instance(typ, itype)
            assert isinstance(decoded, CallableType)
            assert isinstance(expected, CallableType)
            assert str(decoded.arg_types[0]) == str(expected.arg_types[0])
        finally:
            set_wire_alias_map(None)
            self._set_resolver(self.resolver)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMemberAccessDispatchSuite(Suite):
    """Parity tests for the #805 method-branch dispatch seam.

    `rust_analyze_instance_member_dispatch` replaces the whole method
    branch of `analyze_instance_member_access` (checkmember.py:634-775):
    freshen, the static/trivial-self tail, and the non-trivial tail.
    Rust reads live node flags and the serialized `method.type`,
    freshens against the shared raw-id counter, and returns
    (next_raw_id, changed, wire bytes). Deferred (None) cases fall
    through to the pure-Python branch. The union seam returns per-item
    results that the Python shim joins via `make_simplified_union`.
    """

    def setUp(self) -> None:
        import type_kernel as _tk
        from librt.internal import WriteBuffer

        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self._tk = _tk
        self.WriteBuffer = WriteBuffer
        self._set_active = _set_native_checkmember_active
        self._set_resolver = _set_native_checkmember_resolver
        self.fx = TypeFixture()
        for info in (self.fx.gi, self.fx.g2i, self.fx.hi):
            for tv in info.defn.type_vars:
                tv.id = TypeVarId(tv.id.raw_id, namespace=info.fullname)
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
        # Live-node read path (build.py:1518).
        self.resolver.set_live_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_resolver(self.resolver)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self.resolver.set_live_typeinfo_map(None)
        set_wire_typeinfo_map(None)
        self._set_resolver(None)
        self._set_active(False)

    def _bytes_of(self, t: Type) -> bytes:
        buf = self.WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _add_method(
        self,
        info: TypeInfo,
        name: str,
        sig: CallableType | Overloaded,
        *,
        is_static: bool = False,
        is_trivial_self: bool = False,
        is_class: bool = False,
        is_final: bool = False,
    ) -> FuncDef:
        from mypy.nodes import MDEF, FuncDef, SymbolTableNode

        fn = FuncDef(name, [], None, None)
        fn.type = sig
        fn.info = info
        fn.is_static = is_static
        fn.is_trivial_self = is_trivial_self
        fn.is_class = is_class
        fn.is_final = is_final
        info.names[name] = SymbolTableNode(MDEF, fn)
        return fn

    def _sig(
        self,
        arg_types: list[Type],
        arg_kinds: list[ArgKind],
        arg_names: list[str | None],
        ret: Type,
        variables: list[TypeVarLikeType] | None = None,
    ) -> CallableType:
        return CallableType(
            arg_types=arg_types,
            arg_kinds=arg_kinds,
            arg_names=arg_names,
            ret_type=ret,
            fallback=self.fx.function,
            variables=variables or [],
            is_bound=False,
        )

    def _dispatch(
        self,
        instance: Instance,
        name: str,
        self_type: Type,
        *,
        preserve: bool = False,
        start_raw_id: int | None = None,
        override: str | None = None,
    ) -> tuple[int, bool, Type | None]:
        from mypy.checkmember import _deserialize_type_for_checkmember

        start = TypeVarId.next_raw_id if start_raw_id is None else start_raw_id
        result = self._tk.rust_analyze_instance_member_dispatch(
            self.resolver,
            self._bytes_of(instance),
            name,
            override,
            self._bytes_of(self_type),
            False,
            preserve,
            start,
            False,
        )
        if result is None:
            return (start, False, None)
        next_raw_id, changed, wire_bytes = result
        decoded = _deserialize_type_for_checkmember(bytes(wire_bytes))
        return (next_raw_id, changed, decoded)

    def _raw_dispatch(self, instance: Instance, name: str) -> tuple[int, bool, bytes] | None:
        return self._tk.rust_analyze_instance_member_dispatch(
            self.resolver,
            self._bytes_of(instance),
            name,
            None,
            self._bytes_of(instance),
            False,
            False,
            100,
            False,
        )

    def test_dispatch_static_non_generic_preserves(self) -> None:
        from mypy.nodes import ARG_POS

        self._add_method(
            self.fx.ai, "m", self._sig([self.fx.o], [ARG_POS], ["x"], self.fx.a), is_static=True
        )
        _next_raw_id, changed, decoded = self._dispatch(self.fx.a, "m", self.fx.a)
        decoded = get_proper_type(decoded)
        assert changed is False
        assert isinstance(decoded, CallableType)
        assert decoded.ret_type == self.fx.a
        assert decoded.is_bound is False

    def test_dispatch_trivial_self_binds(self) -> None:
        from mypy.nodes import ARG_POS

        self._add_method(
            self.fx.ai,
            "m",
            self._sig([self.fx.o, self.fx.a], [ARG_POS, ARG_POS], ["self", "x"], self.fx.a),
            is_trivial_self=True,
        )
        _next_raw_id, changed, decoded = self._dispatch(self.fx.a, "m", self.fx.a)
        decoded = get_proper_type(decoded)
        assert changed is False
        assert isinstance(decoded, CallableType)
        assert decoded.is_bound is True
        assert decoded.arg_types == [self.fx.a]
        assert decoded.ret_type == self.fx.a

    def test_dispatch_non_trivial_same_class_binds(self) -> None:
        from mypy.nodes import ARG_POS

        self._add_method(
            self.fx.ai,
            "m",
            self._sig([self.fx.o, self.fx.a], [ARG_POS, ARG_POS], ["self", "x"], self.fx.a),
        )
        _next_raw_id, changed, decoded = self._dispatch(self.fx.a, "m", self.fx.a)
        decoded = get_proper_type(decoded)
        assert changed is False
        assert isinstance(decoded, CallableType)
        assert decoded.is_bound is True
        assert decoded.arg_types == [self.fx.a]
        assert decoded.ret_type == self.fx.a

    def test_dispatch_static_overloaded_expands(self) -> None:
        from mypy.nodes import ARG_POS

        item1 = self._sig([self.fx.o], [ARG_POS], ["x"], self.fx.a)
        item2 = self._sig([self.fx.a], [ARG_POS], ["x"], self.fx.b)
        self._add_method(self.fx.ai, "m", Overloaded([item1, item2]), is_static=True)
        _next_raw_id, changed, decoded = self._dispatch(self.fx.a, "m", self.fx.a)
        decoded = get_proper_type(decoded)
        assert changed is False
        assert isinstance(decoded, Overloaded)
        assert len(decoded.items) == 2
        assert decoded.items[0].ret_type == self.fx.a
        assert decoded.items[1].ret_type == self.fx.b
        assert decoded.items[0].is_bound is False

    def test_dispatch_freshens_generic_method(self) -> None:
        from mypy.nodes import ARG_POS

        ns_t = TypeVarType(
            "T",
            "__main__.T",
            TypeVarId(1, namespace="G"),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        self._add_method(
            self.fx.ai,
            "m",
            self._sig([self.fx.o], [ARG_POS], ["x"], ns_t, variables=[ns_t]),
            is_static=True,
        )
        old = TypeVarId.next_raw_id
        try:
            next_raw_id, changed, decoded = self._dispatch(
                self.fx.a, "m", self.fx.a, start_raw_id=old
            )
            decoded = get_proper_type(decoded)
            assert changed is True
            assert next_raw_id == old + 1
            assert isinstance(decoded, CallableType)
            rt = get_proper_type(decoded.ret_type)
            assert isinstance(rt, TypeVarType)
            assert rt.id.raw_id == old
            # Python's freeze_all_type_vars (checkmember.py:933) reifies the
            # freshened variable to meta_level 0 before the return; the Rust
            # tail ports that freeze, so parity expects 0 here.
            assert rt.id.meta_level == 0
        finally:
            TypeVarId.next_raw_id = old

    def test_dispatch_defers_unknown_name(self) -> None:
        assert self._raw_dispatch(self.fx.a, "zzz") is None

    def test_dispatch_defers_decorator(self) -> None:
        from mypy.nodes import MDEF, Decorator, FuncDef, SymbolTableNode, Var

        fn = FuncDef("m", [], None, None)
        fn.type = self._sig([], [], [], self.fx.a)
        self.fx.ai.names["m"] = SymbolTableNode(MDEF, Decorator(fn, [], Var("m")))
        assert self._raw_dispatch(self.fx.a, "m") is None

    def test_dispatch_defers_var_node(self) -> None:
        from mypy.nodes import MDEF, SymbolTableNode, Var

        self.fx.ai.names["m"] = SymbolTableNode(MDEF, Var("m"))
        assert self._raw_dispatch(self.fx.a, "m") is None

    def test_dispatch_completes_subclass_receiver_nongeneric(self) -> None:
        from mypy.nodes import ARG_POS

        self._add_method(self.fx.ai, "m", self._sig([self.fx.o], [ARG_POS], ["x"], self.fx.a))
        # Non-generic signature: Python's bind_self takes the strip path, so
        # the subclass receiver completes in Rust (filter + map + expand +
        # strip), mirroring the Python tail.
        _next_raw_id, _changed, decoded = self._dispatch(self.fx.c, "m", self.fx.c)
        decoded = get_proper_type(decoded)
        assert isinstance(decoded, CallableType)
        assert decoded.ret_type == self.fx.a
        assert decoded.is_bound is True
        assert decoded.arg_types == []

    def test_dispatch_completes_subclass_receiver_generic(self) -> None:
        from mypy.nodes import ARG_POS

        self._add_method(
            self.fx.ai,
            "m",
            self._sig([self.fx.o], [ARG_POS], ["x"], self.fx.a, variables=[self.fx.t]),
        )
        # Issue #1214: the generic bind branch is decided by the bind plan.
        # The self param carries no method tvar (T is declared but unused
        # there), so the plan is the empty strip and the receiver completes.
        _next_raw_id, _changed, decoded = self._dispatch(self.fx.c, "m", self.fx.c)
        decoded = get_proper_type(decoded)
        assert isinstance(decoded, CallableType)
        assert decoded.ret_type == self.fx.a
        assert decoded.is_bound is True
        assert decoded.arg_types == []

    def test_dispatch_defers_subclass_receiver_tvar_self(self) -> None:
        from mypy.nodes import ARG_POS

        self._add_method(
            self.fx.ai,
            "m",
            self._sig([self.fx.t], [ARG_POS], ["x"], self.fx.a, variables=[self.fx.t]),
        )
        # The self param IS the method typevar: the bind plan is bare-Self
        # and a subclass receiver still completes (Python solves T := the
        # receiver). A defer only fires for a receiver carrying type vars.
        _next_raw_id, _changed, decoded = self._dispatch(self.fx.c, "m", self.fx.c)
        decoded = get_proper_type(decoded)
        assert isinstance(decoded, CallableType)
        assert decoded.ret_type == self.fx.a
        assert decoded.is_bound is True
        assert decoded.arg_types == []

    def test_dispatch_defers_non_final_init(self) -> None:
        from mypy.nodes import ARG_POS

        self._add_method(
            self.fx.ai, "__init__", self._sig([self.fx.o], [ARG_POS], ["self"], NoneType())
        )
        assert self._raw_dispatch(self.fx.a, "__init__") is None

    def test_union_seam_per_item(self) -> None:
        from mypy.checkmember import _deserialize_type_for_checkmember

        self._add_method(self.fx.ai, "m", self._sig([], [], [], self.fx.a), is_static=True)
        self._add_method(self.fx.bi, "m", self._sig([], [], [], self.fx.b), is_static=True)
        union = UnionType([self.fx.a, self.fx.b])
        result = self._tk.rust_analyze_union_member_access(
            self.resolver, self._bytes_of(union), "m", False, False, False, True, 100, False
        )
        assert result is not None
        _next_raw_id, changed, per_item = result
        assert changed is False
        assert len(per_item) == 2
        decoded = []
        for item in per_item:
            assert item is not None
            decoded.append(_deserialize_type_for_checkmember(bytes(item)))
        assert isinstance(decoded[0], CallableType) and isinstance(decoded[1], CallableType)
        assert decoded[0].ret_type == self.fx.a
        assert decoded[1].ret_type == self.fx.b

    def test_union_seam_defers_for_lvalue(self) -> None:
        union = UnionType([self.fx.a, self.fx.b])
        result = self._tk.rust_analyze_union_member_access(
            self.resolver, self._bytes_of(union), "m", True, False, False, True, 100, False
        )
        assert result is None

    def test_union_seam_none_slot_for_methodless_item(self) -> None:
        # A union item without a dispatch context (a bare callable:
        # fallback recursion into an Instance defers) arrives as a
        # None slot instead of discarding whole-union work.
        from mypy.checkmember import _deserialize_type_for_checkmember

        self._add_method(self.fx.ai, "m2", self._sig([], [], [], self.fx.a), is_static=True)
        bare_callable = self._sig([], [], [], self.fx.a)
        union = UnionType([self.fx.a, bare_callable])
        result = self._tk.rust_analyze_union_member_access(
            self.resolver, self._bytes_of(union), "m2", False, False, False, True, 100, False
        )
        assert result is not None
        _next_raw_id, _changed, per_item = result
        assert len(per_item) == 2
        assert isinstance(per_item[0], (bytes, list)) and per_item[1] is None
        decoded = _deserialize_type_for_checkmember(bytes(per_item[0]))
        assert isinstance(decoded, CallableType)
        assert decoded.ret_type == self.fx.a

    def test_none_seam_bool_callable(self) -> None:
        from mypy.checkmember import _deserialize_type_for_checkmember

        none_bytes = self._bytes_of(NoneType())
        for strict in (False, True):
            result = self._tk.rust_analyze_none_member_access(
                self.resolver, "__bool__", none_bytes, None, False, False, False, 100, strict
            )
            assert result is not None
            next_raw_id, changed, wire_bytes = result
            assert (next_raw_id, changed) == (100, False)
            decoded = _deserialize_type_for_checkmember(bytes(wire_bytes))
            assert isinstance(decoded, CallableType)
            assert decoded.is_bound is False
            ret = get_proper_type(decoded.ret_type)
            assert isinstance(ret, LiteralType)
            assert ret.value is False
            assert ret.fallback.type.fullname == "builtins.bool"

    def test_none_seam_object_method_binds(self) -> None:
        # None.foo with a method on builtins.object: recursion rides the
        # live dispatch; self_type = the object instance passes the
        # same-class guard and binds the method.
        from mypy.checkmember import _deserialize_type_for_checkmember
        from mypy.nodes import ARG_POS

        self._add_method(
            self.fx.oi,
            "obj_m",
            self._sig([self.fx.o, self.fx.a], [ARG_POS, ARG_POS], ["self", "x"], self.fx.a),
        )
        result = self._tk.rust_analyze_none_member_access(
            self.resolver,
            "obj_m",
            self._bytes_of(NoneType()),
            self._bytes_of(self.fx.o),
            False,
            False,
            False,
            100,
            False,
        )
        assert result is not None
        _next_raw_id, changed, wire_bytes = result
        decoded = _deserialize_type_for_checkmember(bytes(wire_bytes))
        assert isinstance(decoded, CallableType)
        assert decoded.is_bound is True
        assert decoded.arg_types == [self.fx.a]

    def test_none_seam_defers(self) -> None:
        # lvalue / super defer (per-item lvalue/super semantics stay
        # Python-side), an absent self_type defers, and an unknown name
        # defers through the object dispatch.
        none_bytes = self._bytes_of(NoneType())
        self_bytes = self._bytes_of(self.fx.o)
        base = (self.resolver, "obj_m", none_bytes)
        assert (
            self._tk.rust_analyze_none_member_access(
                *base, self_bytes, True, False, False, 100, False
            )
            is None
        )
        assert (
            self._tk.rust_analyze_none_member_access(
                *base, self_bytes, False, True, False, 100, False
            )
            is None
        )
        assert (
            self._tk.rust_analyze_none_member_access(*base, None, False, False, False, 100, False)
            is None
        )
        self._add_method(self.fx.ai, "unrelated", self._sig([], [], [], self.fx.a))
        assert (
            self._tk.rust_analyze_none_member_access(
                self.resolver, "unrelated", none_bytes, self_bytes, False, False, False, 100, False
            )
            is None
        )

    def test_union_shim_fills_none_slot(self) -> None:
        # End-to-end shim: union [A(has __bool__), None] with
        # strict_optional=True. The None slot fills via
        # _analyze_member_access, re-entering the native __bool__ seam.
        from types import SimpleNamespace

        from mypy import state as mypy_state
        from mypy.checkmember import MemberContext, analyze_union_member_access

        class _NoopCtx:
            def __enter__(self) -> None:
                pass

            def __exit__(self, *a: object) -> None:
                pass

        self._add_method(self.fx.ai, "__bool__", self._sig([], [], [], self.fx.a), is_static=True)
        union = UnionType([self.fx.a, NoneType()])
        chk = SimpleNamespace(
            named_type=lambda n: (
                self.fx.bool_type
                if n == "builtins.bool"
                else self.fx.function if n == "builtins.function" else Instance(self.fx.oi, [])
            ),
            msg=SimpleNamespace(
                filter_errors=lambda *a, **kw: _NoopCtx(),
                disable_type_names=lambda: _NoopCtx(),
                fail=lambda *a, **kw: None,
            ),
        )
        mx = MemberContext(
            is_lvalue=False,
            is_super=False,
            is_operator=False,
            original_type=union,
            context=NameExpr("x"),
            chk=cast(Any, chk),
        )
        old_strict = mypy_state.state.strict_optional
        try:
            mypy_state.state.strict_optional = True
            result = analyze_union_member_access("__bool__", union, mx)
        finally:
            mypy_state.state.strict_optional = old_strict
        # The shim fills the None slot via the NoneType __bool__ native
        # path and joins: [A.__bool__ callable, None.__bool__ callable].
        result = get_proper_type(result)
        assert isinstance(result, UnionType)
        assert len(result.items) == 2
        first = get_proper_type(result.items[0])
        assert isinstance(first, CallableType)
        assert first.ret_type == self.fx.a
        second = get_proper_type(result.items[1])
        assert isinstance(second, CallableType)
        second_ret = get_proper_type(second.ret_type)
        assert isinstance(second_ret, LiteralType)
        assert second_ret.value is False


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckmemberDeferralSuite(Suite):
    """Reduction of `analyze_member_access_inner` fallback-recursion defers (#872).

    The general member-access seam previously deferred every TupleType /
    LiteralType / CallableType / TypeVar fallback that resolved to an
    Instance (`memacc:tuple_instance_fb` and friends, the largest
    wire-portable defer class in checkmember.rs). It now routes rvalue,
    non-super Instance fallbacks through the native method branch, so
    e.g. `(a, b).method(...)` type-checks without falling back to Python.

    Each test calls the seam directly: a TupleType-with-method-fallback
    must answer natively (non-None, bound method); lvalue/super operands,
    and fallbacks without a dispatch context, must still defer (None).
    """

    def setUp(self) -> None:
        import type_kernel as _tk
        from librt.internal import WriteBuffer

        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self._tk = _tk
        self.WriteBuffer = WriteBuffer
        self.fx = TypeFixture()
        type_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.str_type_info,
            self.fx.functioni,
        ]
        self.resolver = _tk.build_native_resolver(type_infos, [])
        self.resolver.set_live_typeinfo_map({info.fullname: info for info in type_infos})
        _set_native_checkmember_resolver(self.resolver)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        _set_native_checkmember_active(True)

    def tearDown(self) -> None:
        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.resolver.set_live_typeinfo_map(None)
        set_wire_typeinfo_map(None)
        _set_native_checkmember_resolver(None)
        _set_native_checkmember_active(False)

    def _bytes_of(self, t: Type) -> bytes:
        buf = self.WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _add_method(
        self, info: TypeInfo, name: str, sig: CallableType, is_trivial_self: bool = False
    ) -> None:
        from mypy.nodes import MDEF, FuncDef, SymbolTableNode

        fn = FuncDef(name, [], None, None)
        fn.type = sig
        fn.info = info
        fn.is_trivial_self = is_trivial_self
        info.names[name] = SymbolTableNode(MDEF, fn)

    def _general(
        self,
        typ: Type,
        name: str,
        *,
        is_lvalue: bool = False,
        is_super: bool = False,
        start: int = 100,
    ) -> tuple[int, bool, Type | None]:
        from mypy.checkmember import _deserialize_type_for_checkmember

        result = self._tk.rust_analyze_member_access(
            self.resolver,
            name,
            self._bytes_of(typ),
            self._bytes_of(typ),
            is_lvalue,
            is_super,
            False,
            False,
            False,
            start,
            False,
        )
        if result is None:
            return (start, False, None)
        next_raw_id, changed, wire_bytes = result
        decoded = _deserialize_type_for_checkmember(bytes(wire_bytes))
        return (next_raw_id, changed, decoded)

    def test_tuple_fallback_routes_and_binds(self) -> None:
        # class A gains a trivial-self method m(self, x: int) -> A; a
        # TupleType with fallback A must recurse into the native method
        # branch and return the bound signature (self stripped).
        from mypy.nodes import ARG_POS
        from mypy.types import TupleType

        self._add_method(
            self.fx.ai,
            "m",
            CallableType(
                [self.fx.o, self.fx.a],
                [ARG_POS, ARG_POS],
                ["self", "x"],
                self.fx.a,
                self.fx.function,
            ),
            is_trivial_self=True,
        )
        tup = TupleType([self.fx.a], self.fx.a)
        _next_raw_id, changed, decoded = self._general(tup, "m")
        decoded = get_proper_type(decoded)
        assert changed is False
        assert isinstance(decoded, CallableType)
        assert decoded.is_bound is True
        assert decoded.arg_types == [self.fx.a]
        assert decoded.ret_type == self.fx.a

    def test_tuple_fallback_lvalue_defers(self) -> None:
        # An lvalue access must defer (Python owns lvalue semantics).
        from mypy.nodes import ARG_POS
        from mypy.types import TupleType

        self._add_method(
            self.fx.ai,
            "m",
            CallableType([self.fx.o], [ARG_POS], ["self"], self.fx.a, self.fx.function),
            is_trivial_self=True,
        )
        tup = TupleType([self.fx.a], self.fx.a)
        assert self._general(tup, "m", is_lvalue=True)[2] is None
        assert self._general(tup, "m", is_super=True)[2] is None

    def test_tuple_fallback_no_method_defers(self) -> None:
        # A fallback Instance with no matching method defers (Python
        # reports the missing-attribute diagnostic); the recursion still
        # routes without guessing.
        from mypy.types import TupleType

        tup = TupleType([self.fx.a], self.fx.a)
        assert self._general(tup, "missing_name")[2] is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeTypeMemberAccessSuite(Suite):
    """Parity for the Rust `analyze_type_type_member_access` dispatch port.

    `rust_classify_type_type_member_access` classifies the 9-way dispatch
    head of `mypy.checkmember.analyze_type_type_member_access`
    (checkmember.py:965) plus a nested 4-way sub-dispatch on
    `get_proper_type(typ.item.upper_bound)` for the TypeVarType arm. Rust
    returns a branch tag; the Python shim applies the terminal branches
    (`_analyze_member_access`, `filter_errors`, `tuple_fallback`,
    `TypeType.make_normalized`, `metaclass_type`).

    Direct seam calls assert the exact tag for every branch; the
    gate-off vs gate-on differential drives the real
    `analyze_type_type_member_access` through a mock MemberContext and
    asserts identical (result, path) pairs.
    """

    def setUp(self) -> None:
        from mypy.checkmember import _set_native_checkmember_active

        self._set_active = _set_native_checkmember_active
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

    def _tag(self, typ: TypeType) -> int | None:
        return _type_kernel.rust_classify_type_type_member_access(typ)

    def _typevar(self, upper_bound: Type) -> TypeVarType:
        return TypeVarType(
            "T",
            "T",
            TypeVarId(1),
            [],
            upper_bound,
            AnyType(TypeOfAny.from_omitted_generics),
            COVARIANT,
        )

    def test_seam_item_instance(self) -> None:
        tt = TypeType(self.fx.a)
        assert self._tag(tt) == 1

    def test_seam_item_any(self) -> None:
        tt = TypeType(AnyType(TypeOfAny.special_form))
        assert self._tag(tt) == 2

    def test_seam_tv_ub_instance(self) -> None:
        tt = TypeType(self._typevar(self.fx.a))
        assert self._tag(tt) == 3

    def test_seam_tv_ub_union(self) -> None:
        tt = TypeType(self._typevar(UnionType([self.fx.a, self.fx.b])))
        assert self._tag(tt) == 4

    def test_seam_tv_ub_tuple(self) -> None:
        tt = TypeType(self._typevar(TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)))
        assert self._tag(tt) == 5

    def test_seam_tv_ub_any(self) -> None:
        tt = TypeType(self._typevar(AnyType(TypeOfAny.special_form)))
        assert self._tag(tt) == 6

    def test_seam_tv_ub_other(self) -> None:
        tt = TypeType(self._typevar(NoneType()))
        assert self._tag(tt) == 7

    def test_seam_item_tuple(self) -> None:
        tt = TypeType(TupleType([self.fx.a, self.fx.b], self.fx.std_tuple))
        assert self._tag(tt) == 8

    def test_seam_item_func_typeobj(self) -> None:
        from mypy.types import CallableType

        sig = CallableType([], [], [], self.fx.a, self.fx.type_type)
        tt = TypeType(sig)
        assert self._tag(tt) == 9

    def test_seam_item_func_not_typeobj(self) -> None:
        from mypy.types import CallableType

        sig = CallableType([], [], [], self.fx.a, self.fx.function)
        tt = TypeType(sig)
        assert self._tag(tt) == 10

    def test_seam_item_type_type_instance(self) -> None:
        tt = TypeType(cast(Any, TypeType(self.fx.a)))
        assert self._tag(tt) == 11

    def test_seam_item_type_type_other(self) -> None:
        tt = TypeType(cast(Any, TypeType(NoneType())))
        assert self._tag(tt) == 12

    def test_seam_none(self) -> None:
        tt = TypeType(NoneType())
        assert self._tag(tt) == 0

    def _make_mx(self, typ: TypeType) -> Any:
        """Build a minimal MemberContext for the tail path."""
        from types import SimpleNamespace

        from mypy.checkmember import MemberContext

        class _FilterErrorsCtx:
            def __enter__(self) -> None:
                pass

            def __exit__(self, *a: object) -> None:
                pass

        msg = SimpleNamespace(
            filter_errors=lambda *a, **kw: _FilterErrorsCtx(),
            disable_type_names=lambda: _FilterErrorsCtx(),
            fail=lambda *a, **kw: None,
        )
        chk = SimpleNamespace(
            named_type=lambda n: self.fx.type_type,
            msg=msg,
            handle_cannot_determine_type=lambda *a, **kw: None,
        )
        return MemberContext(
            is_lvalue=False,
            is_super=False,
            is_operator=False,
            original_type=typ,
            context=NameExpr("x"),
            chk=cast(Any, chk),
        )

    def _run_fn(self, typ: TypeType, name: str = "foo") -> tuple[str, list[str]]:
        """Run analyze_type_type_member_access with mocked tail calls.

        Returns (result_str, call_log) where call_log records which
        tail functions were invoked.
        """
        import mypy.checkmember as cm

        calls: list[str] = []

        orig_ama = cm._analyze_member_access
        orig_aca = cm.analyze_class_attribute_access
        orig_tf = cm.tuple_fallback  # type: ignore[attr-defined]

        def mock_ama(name: Any, typ: Any, mx: Any, override_info: Any = None) -> Any:
            fb = str(typ)
            calls.append(f"ama:{fb}")
            return AnyType(TypeOfAny.special_form)

        def mock_aca(item: Any, name: Any, mx: Any, **kw: Any) -> Any:
            calls.append(f"aca:{item}")
            return None

        def mock_tf(typ: Any) -> Any:
            calls.append(f"tf:{typ}")
            return self.fx.o

        cm._analyze_member_access = mock_ama
        cm.analyze_class_attribute_access = mock_aca  # type: ignore[assignment]
        cm.tuple_fallback = mock_tf  # type: ignore[attr-defined]
        try:
            mx = self._make_mx(typ)
            result = cm.analyze_type_type_member_access(name, typ, mx, None)
            return str(result), sorted(calls)
        finally:
            cm._analyze_member_access = orig_ama
            cm.analyze_class_attribute_access = orig_aca
            cm.tuple_fallback = orig_tf  # type: ignore[attr-defined]

    def _assert_par(self, typ: TypeType) -> None:
        off = self._with_gate(False, lambda: self._run_fn(typ))
        on = self._with_gate(True, lambda: self._run_fn(typ))
        assert_equal(on, off, f"analyze_type_type_member_access parity for {typ}")

    def test_par_item_instance(self) -> None:
        self._assert_par(TypeType(self.fx.a))

    def test_par_item_any(self) -> None:
        self._assert_par(TypeType(AnyType(TypeOfAny.special_form)))

    def test_par_tv_ub_instance(self) -> None:
        self._assert_par(TypeType(self._typevar(self.fx.a)))

    def test_par_tv_ub_union(self) -> None:
        self._assert_par(TypeType(self._typevar(UnionType([self.fx.a, self.fx.b]))))

    def test_par_tv_ub_tuple(self) -> None:
        self._assert_par(
            TypeType(self._typevar(TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)))
        )

    def test_par_tv_ub_any(self) -> None:
        self._assert_par(TypeType(self._typevar(AnyType(TypeOfAny.special_form))))

    def test_par_tv_ub_other(self) -> None:
        self._assert_par(TypeType(self._typevar(NoneType())))

    def test_par_item_tuple(self) -> None:
        self._assert_par(TypeType(TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)))

    def test_par_item_func_typeobj(self) -> None:
        from mypy.types import CallableType

        sig = CallableType([], [], [], self.fx.a, self.fx.type_type)
        self._assert_par(TypeType(sig))

    def test_par_item_func_not_typeobj(self) -> None:
        from mypy.types import CallableType

        sig = CallableType([], [], [], self.fx.a, self.fx.function)
        self._assert_par(TypeType(sig))

    def test_par_item_type_type_instance(self) -> None:
        self._assert_par(TypeType(cast(Any, TypeType(self.fx.a))))

    def test_par_item_type_type_other(self) -> None:
        self._assert_par(TypeType(cast(Any, TypeType(NoneType()))))

    def test_par_none(self) -> None:
        self._assert_par(TypeType(NoneType()))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeDescriptorHeadSuite(Suite):
    """Parity for the Rust `analyze_descriptor_access` head (mypy.checkmember).

    The seam decides the pure guards of `analyze_descriptor_access`
    (checkmember.py:1376-1421): a non-Instance proper type and an
    Instance with no readable `__get__`/`__set__` for the access kind
    return `orig_descriptor_type` (tag 0, the live object is returned by
    the shim); a UnionType maps item-wise through the same decision and
    joins via `make_simplified_union` (tag 1). A `__get__`-bearing
    Instance (the checker-state tail: bound `__get__` lookup, expand,
    `transform_callee_type`, `check_call`, `warn_deprecated`) and the
    lvalue `__set__` assign path defer (`None`). The decided branches
    never touch `mx.chk`, so the gate-off vs gate-on differential runs
    through the real `analyze_descriptor_access` with a stub MemberContext.

    The guards run *before* the gate, so the shapes they answer (a plain
    or setter-only Instance on a non-lvalue read) never reach the seam:
    `test_plain_instance*` and `test_setter_instance*` pin those values
    directly instead of comparing two arms of code that cannot differ.
    """

    def setUp(self) -> None:
        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        # A descriptor class with a readable __get__.
        self.desci = self.fx.make_type_info("mod.Desc", mro=[self.fx.oi])
        self.desci.names["__get__"] = SymbolTableNode(MDEF, Var("__get__"))
        # A settable-only descriptor class (readable __set__).
        self.setteri = self.fx.make_type_info("mod.Setter", mro=[self.fx.oi])
        self.setteri.names["__set__"] = SymbolTableNode(MDEF, Var("__set__"))
        # A get+set descriptor class.
        self.getseti = self.fx.make_type_info("mod.GetSet", mro=[self.fx.oi])
        self.getseti.names["__get__"] = SymbolTableNode(MDEF, Var("__get__"))
        self.getseti.names["__set__"] = SymbolTableNode(MDEF, Var("__set__"))
        # A plain class without __get__/__set__.
        self.plaini = self.fx.make_type_info("mod.Plain", mro=[self.fx.oi])
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend([self.desci, self.setteri, self.getseti, self.plaini])
        self.typeinfo_map = {info.fullname: info for info in type_infos}
        set_wire_typeinfo_map(self.typeinfo_map)
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active = _set_native_checkmember_active
        self._set_resolver = _set_native_checkmember_resolver
        self._set_active(True)
        self._set_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        self._set_resolver(self.resolver if active else None)
        try:
            return fn()
        finally:
            self._set_active(True)
            self._set_resolver(self.resolver)

    def _seam(self, typ: ProperType, is_lvalue: bool) -> tuple[int, bytes] | None:
        from mypy.checkmember import _serialize_type_for_checkmember

        result = _type_kernel.rust_analyze_descriptor_access(
            self.resolver, _serialize_type_for_checkmember(typ), is_lvalue, True
        )
        if result is None:
            return None
        tag, wire = result
        return tag, bytes(wire)

    def _make_mx(self, is_lvalue: bool) -> Any:
        from types import SimpleNamespace

        from mypy.checkmember import MemberContext
        from mypy.options import Options

        chk = SimpleNamespace(msg=SimpleNamespace(fail=lambda *a, **kw: None, options=Options()))
        return MemberContext(
            is_lvalue=is_lvalue,
            is_super=False,
            is_operator=False,
            original_type=self.fx.o,
            context=NameExpr("x"),
            chk=cast(Any, chk),
        )

    def _assert_par(self, typ: Type, is_lvalue: bool = False) -> None:
        from mypy.checkmember import analyze_descriptor_access

        def run() -> str:
            mx = self._make_mx(is_lvalue)
            return str(analyze_descriptor_access(typ, mx))

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert off == on, f"descriptor head({typ}, lv={is_lvalue}): off={off}, on={on}"

    # --- direct seam tag tests ---

    def test_seam_non_descriptor_instance_orig(self) -> None:
        typ = Instance(self.plaini, [])
        assert self._seam(typ, False) == (0, b"")
        assert self._seam(typ, True) == (0, b"")

    def test_seam_get_descriptor_defers(self) -> None:
        typ = Instance(self.desci, [])
        assert self._seam(typ, False) is None

    def test_seam_get_descriptor_lvalue_defers(self) -> None:
        # Lvalue + readable __get__ (no __set__) needs the heavy tail.
        typ = Instance(self.desci, [])
        assert self._seam(typ, True) is None

    def test_seam_set_descriptor_lvalue_defers(self) -> None:
        # Lvalue + readable __set__ routes to analyze_descriptor_assign.
        typ = Instance(self.setteri, [])
        assert self._seam(typ, True) is None

    def test_seam_set_descriptor_non_lvalue_orig(self) -> None:
        # Non-lvalue access only checks __get__; a __set__-only class
        # passes through.
        typ = Instance(self.setteri, [])
        assert self._seam(typ, False) == (0, b"")

    def test_seam_getset_descriptor_defers(self) -> None:
        typ = Instance(self.getseti, [])
        assert self._seam(typ, False) is None
        assert self._seam(typ, True) is None

    def test_seam_non_instance_orig(self) -> None:
        # CallableType / NoneType / TupleType are all answered by the
        # checkmember.py:1416 non-Instance guard, both access kinds.
        sig = CallableType([], [], [], self.fx.a, self.fx.function)
        for typ in (sig, NoneType()):
            assert self._seam(typ, False) == (0, b"")
            assert self._seam(typ, True) == (0, b"")

    def test_seam_missing_snapshot_defers(self) -> None:
        # A class with no resolver snapshot defers (member presence is
        # unreadable).
        from mypy.checkmember import _serialize_type_for_checkmember

        result = _type_kernel.rust_analyze_descriptor_access(
            _type_kernel.build_native_resolver([], []),
            _serialize_type_for_checkmember(Instance(self.plaini, [])),
            False,
            True,
        )
        assert result is None

    # --- gate-off vs gate-on differentials through the real function ---

    def test_parity_non_instance_callable(self) -> None:
        self._assert_par(self.fx.callable(self.fx.a, self.fx.b))

    def test_parity_non_instance_callable_lvalue(self) -> None:
        self._assert_par(self.fx.callable(self.fx.a, self.fx.b), is_lvalue=True)

    def test_parity_non_instance_none(self) -> None:
        self._assert_par(NoneType())

    def test_plain_instance_returns_original_object(self) -> None:
        # The plain-Instance guards precede the gate, so no arm reaches the
        # seam: pin the guard's own result instead of comparing two arms of
        # identical Python.
        from mypy.checkmember import analyze_descriptor_access

        typ = Instance(self.plaini, [])
        result = analyze_descriptor_access(typ, self._make_mx(False))
        assert result is typ, f"plain instance round-tripped: {result!r}"
        assert str(result) == "mod.Plain"

    def test_plain_instance_lvalue_returns_original_object(self) -> None:
        from mypy.checkmember import analyze_descriptor_access

        typ = Instance(self.plaini, [])
        result = analyze_descriptor_access(typ, self._make_mx(True))
        assert result is typ, f"plain instance lvalue round-tripped: {result!r}"
        assert str(result) == "mod.Plain"

    def test_setter_instance_non_lvalue_returns_original_object(self) -> None:
        from mypy.checkmember import analyze_descriptor_access

        typ = Instance(self.setteri, [])
        result = analyze_descriptor_access(typ, self._make_mx(False))
        assert result is typ, f"setter-only instance round-tripped: {result!r}"
        assert str(result) == "mod.Setter"

    def test_parity_union_plain_items(self) -> None:
        typ = UnionType.make_union([Instance(self.plaini, []), NoneType()])
        self._assert_par(typ)

    def test_parity_union_plain_items_lvalue(self) -> None:
        typ = UnionType.make_union([Instance(self.plaini, []), self.fx.a])
        self._assert_par(typ, is_lvalue=True)

    def test_parity_union_with_descriptor_item(self) -> None:
        # One __get__-bearing item defers the whole union; Python's
        # per-item recursion runs unchanged (its item-level gate also
        # defers there), so both gates agree.
        typ = UnionType.make_union([Instance(self.plaini, []), Instance(self.desci, [])])
        self._assert_par(typ)

    def test_parity_union_with_setter_item_lvalue(self) -> None:
        typ = UnionType.make_union([Instance(self.plaini, []), Instance(self.setteri, [])])
        self._assert_par(typ, is_lvalue=True)

    def test_parity_nested_union(self) -> None:
        inner = UnionType.make_union([Instance(self.plaini, []), self.fx.a])
        typ = UnionType.make_union([inner, NoneType()])
        self._assert_par(typ)

    def test_non_instance_returns_original_object(self) -> None:
        # Tag 0 must return the live orig_descriptor_type object, not a
        # round-tripped copy, so the shape has to reach the gate: a plain
        # Instance is answered by the guards before it.
        from mypy.checkmember import analyze_descriptor_access

        typ = NoneType()
        result = analyze_descriptor_access(typ, self._make_mx(False))
        assert result is typ, f"tag 0 round-tripped: {result!r}"
        assert str(result) == "None"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsInstanceVarSuite(Suite):
    """Parity for the Rust `is_instance_var` port (mypy.checkmember).

    `is_instance_var` (checkmember.py:1502-1511) is a pure boolean
    conjunction over a live `Var`: `var.name in var.info.names`,
    `var.info.names[var.name].node is var`, `not var.is_classvar`, and
    `not var.is_inferred`. The Rust seam reads the live attrs via PyO3
    and returns a plain bool on every well-formed Var; it defers
    (None) only when an attribute is unreadable (e.g. a Var whose
    `info` is the FakeInfo placeholder). The production gate retired in
    #1739 (`testtypes_native_retired_checkmember.py` pins that
    `is_instance_var` loads no `rust_*` name), so the Python-predicate
    asserts below pin the exact bool per shape and `_assert_seam` calls
    the still-registered pyfunction directly.
    """

    def setUp(self) -> None:
        from mypy.checkmember import _set_native_checkmember_active

        self._set_active = _set_native_checkmember_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _typeinfo(self, fullname: str = "mod.A") -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = [info]
        return info

    def _make_var(
        self,
        name: str = "x",
        *,
        is_classvar: bool = False,
        is_inferred: bool = False,
        node: Var | None = None,
        info: TypeInfo | None = None,
    ) -> Var:
        var = Var(name)
        if info is None:
            info = self._typeinfo()
        var.info = info
        var.is_classvar = is_classvar
        var.is_inferred = is_inferred
        registered = node if node is not None else var
        info.names[name] = SymbolTableNode(MDEF, registered)
        return var

    def _assert_value(self, var: Var, expected: bool) -> None:
        from mypy.checkmember import is_instance_var

        assert is_instance_var(var) == expected, f"{var.name}: {is_instance_var(var)}"

    def _assert_seam(self, var: Var, expected: bool) -> None:
        result = _type_kernel.rust_is_instance_var(var)
        assert result is not None, f"Rust deferred on {var!r}"
        assert result == expected, f"Rust seam: {result} != {expected}"

    def test_true_instance_var(self) -> None:
        var = self._make_var("x")
        self._assert_value(var, True)
        self._assert_seam(var, True)

    def test_name_not_in_names_false(self) -> None:
        var = self._make_var("x")
        var.info.names.pop("x")
        self._assert_value(var, False)
        self._assert_seam(var, False)

    def test_node_not_var_false(self) -> None:
        other = Var("x")
        var = self._make_var("x", node=other)
        self._assert_value(var, False)
        self._assert_seam(var, False)

    def test_is_classvar_false(self) -> None:
        var = self._make_var("x", is_classvar=True)
        self._assert_value(var, False)
        self._assert_seam(var, False)

    def test_is_inferred_false(self) -> None:
        var = self._make_var("x", is_inferred=True)
        self._assert_value(var, False)
        self._assert_seam(var, False)

    def test_seam_defers_on_fake_info(self) -> None:
        # A Var whose `info` is the FakeInfo placeholder: any attr read
        # on info raises AssertionError, so the Rust seam defers (None).
        # The Python predicate would raise too, so seam-only here.
        var = Var("x")
        result = _type_kernel.rust_is_instance_var(var)
        assert result is None, f"expected defer, got {result}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckFinalMemberSuite(Suite):
    """Parity for the Rust `check_final_member` MRO fold (mypy.checkmember).

    `check_final_member` (checkmember.py:1360) walks `info.mro` looking
    for a final Var/FuncDef/OverloadedFuncDef/Decorator under `name`
    and emits `cant_assign_to_final` for each final entry. The Rust
    seam reads the live TypeInfo via PyO3 (zero wire bytes) and folds
    the whole MRO into one bool (True = some entry is final); the
    Python shim keeps the emission. Gate-off vs gate-on runs must
    record identical emissions and the direct seam call must engage
    (non-None) on every well-formed MRO.
    """

    class _RecordingMsg:
        def __init__(self) -> None:
            self.calls: list[str] = []

        def cant_assign_to_final(self, name: str, attr_assign: bool, ctx: object) -> None:
            self.calls.append(name)

    def setUp(self) -> None:
        from mypy.checkmember import _set_native_checkmember_active

        self._set_active = _set_native_checkmember_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _typeinfo(self, fullname: str = "mod.A") -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        return info

    def _chain(self, *fullnames: str) -> list[TypeInfo]:
        infos = [self._typeinfo(fn) for fn in fullnames]
        infos[0].mro = list(infos)
        return infos

    def _install(self, info: TypeInfo, name: str, node: SymbolNode | None) -> None:
        info.names[name] = SymbolTableNode(MDEF, node)

    def _assert_par(self, info: TypeInfo, name: str, expected: list[str]) -> None:
        from mypy.checkmember import check_final_member

        def run() -> list[str]:
            msg = self._RecordingMsg()
            check_final_member(name, info, cast(Any, msg), cast(Context, None))
            return msg.calls

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert off == expected, f"gate-off: {off} != {expected}"
        assert on == expected, f"gate-on: {on} != {expected}"

    def _assert_seam(self, info: TypeInfo, name: str, expected: bool) -> None:
        result = _type_kernel.rust_check_final_member(info, name)
        assert result is not None, f"Rust deferred on {info.fullname}.{name}"
        assert result == expected, f"Rust seam: {result} != {expected}"

    def test_final_var_in_base(self) -> None:
        a, b = self._chain("mod.A", "mod.B")
        final_var = Var("x")
        final_var.is_final = True
        self._install(b, "x", final_var)
        self._assert_par(a, "x", ["x"])
        self._assert_seam(a, "x", True)

    def test_non_final_var_in_base(self) -> None:
        a, b = self._chain("mod.A", "mod.B")
        self._install(b, "x", Var("x"))
        self._assert_par(a, "x", [])
        self._assert_seam(a, "x", False)

    def test_final_in_own_names(self) -> None:
        a, _b = self._chain("mod.A", "mod.B")
        final_var = Var("x")
        final_var.is_final = True
        self._install(a, "x", final_var)
        self._assert_par(a, "x", ["x"])
        self._assert_seam(a, "x", True)

    def test_final_in_later_base(self) -> None:
        a, _b, c = self._chain("mod.A", "mod.B", "mod.C")
        final_var = Var("x")
        final_var.is_final = True
        self._install(c, "x", final_var)
        self._assert_par(a, "x", ["x"])
        self._assert_seam(a, "x", True)

    def test_overridden_by_non_final_still_final(self) -> None:
        a, b, c = self._chain("mod.A", "mod.B", "mod.C")
        self._install(b, "x", Var("x"))
        final_var = Var("x")
        final_var.is_final = True
        self._install(c, "x", final_var)
        self._assert_par(a, "x", ["x"])
        self._assert_seam(a, "x", True)

    def test_name_not_in_mro(self) -> None:
        a, b = self._chain("mod.A", "mod.B")
        self._assert_par(a, "x", [])
        self._assert_seam(a, "x", False)

    def test_final_method_emits(self) -> None:
        from mypy.nodes import Block

        a, b = self._chain("mod.A", "mod.B")
        fd = FuncDef("m", [], Block([]))
        fd.is_final = True
        self._install(b, "m", fd)
        self._assert_par(a, "m", ["m"])
        self._assert_seam(a, "m", True)

    def test_non_final_node_kind_skipped(self) -> None:
        a, b = self._chain("mod.A", "mod.B")
        self._install(b, "x", self._typeinfo("mod.Other"))
        self._assert_par(a, "x", [])
        self._assert_seam(a, "x", False)

    def test_none_node_skipped(self) -> None:
        a, b = self._chain("mod.A", "mod.B")
        self._install(b, "x", None)
        self._assert_par(a, "x", [])
        self._assert_seam(a, "x", False)

    def test_seam_defers_on_non_typeinfo(self) -> None:
        # A non-TypeInfo first arg has no `mro`, so the Rust seam
        # defers (None) instead of answering.
        result = _type_kernel.rust_check_final_member(cast(TypeInfo, Var("x")), "x")
        assert result is None, f"expected defer, got {result}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAnalyzeVarSuite(Suite):
    """Parity tests for `rust_classify_analyze_var` (mypy.checkmember.analyze_var).

    `analyze_var`'s decision head (checkmember.py:1771) reduces to a
    single outcome tag: SETTER / GETTER / PARTIAL / NOT_READY /
    ENUM_LITERAL / UNBOUND_ANY. Rust reads the live Var scalars via PyO3
    (settable-property flag, setter/getter type None-ness + PartialType
    kind, is_ready, is_initialized_in_class, is_instance_var,
    info.fullname, info.is_enum, info.enum_members) and gates on the
    resolver handling the receiver's map_instance_to_supertype. Python
    applies the tagged branch's side effects in `_apply_analyze_var_tag`;
    a None tag (fake info, undecodable wire, snapshot miss) falls back
    to the pure-Python body.

    Direct seam calls assert the exact tag per branch; the `test_value_*`
    pins drive the real `analyze_var` through a mock checker and assert the
    exact result plus recorded side effects per shape.

    The production gate retired in #1739: `analyze_var` loads no `rust_*`
    name (pinned by `NativeCheckmemberRetiredSeamsSuite` in
    `testtypes_native_retired_checkmember.py`), so there is no gate left to
    toggle and the former gate-off vs gate-on comparison is replaced by the
    value pin it used to hold on both arms.
    """

    _UNSET: object = object()

    def setUp(self) -> None:
        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active = _set_native_checkmember_active
        self._set_resolver = _set_native_checkmember_resolver
        self._set_active(True)
        self.info = self._typeinfo("mod.A")
        self.enum_info = self._typeinfo("mod.E")
        self.enum_info.is_enum = True
        self._live_map = {"mod.A": self.info, "mod.E": self.enum_info}
        self.resolver = _type_kernel.build_native_resolver(list(self._live_map.values()), [])
        self.resolver.set_live_typeinfo_map(dict(self._live_map))
        set_wire_typeinfo_map(dict(self._live_map))
        self._set_resolver(self.resolver)

    def tearDown(self) -> None:
        self._set_resolver(None)
        self._set_active(False)

    def _typeinfo(self, fullname: str = "mod.A") -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = [info]
        return info

    def _make_var(
        self,
        name: str = "x",
        *,
        typ: Any = _UNSET,
        setter_type: CallableType | None = None,
        is_ready: bool = True,
        is_property: bool = False,
        is_settable_property: bool = False,
        is_initialized_in_class: bool = False,
        has_explicit_value: bool = False,
        info: TypeInfo | None = None,
    ) -> Var:
        var = Var(name)
        if typ is not self._UNSET:
            var.type = typ
        if info is None:
            info = self.info
        var.info = info
        var.is_ready = is_ready
        var.setter_type = setter_type
        var.is_property = is_property
        var.is_settable_property = is_settable_property
        var.is_initialized_in_class = is_initialized_in_class
        var.has_explicit_value = has_explicit_value
        info.names[name] = SymbolTableNode(MDEF, var)
        return var

    def _seam(
        self,
        var: Var,
        itype: Instance,
        *,
        is_lvalue: bool = False,
        no_deferral: bool = False,
        is_operator: bool = False,
        resolver: Any = _UNSET,
    ) -> int | None:
        from mypy.checkmember import _serialize_type_for_checkmember

        return _type_kernel.rust_classify_analyze_var(
            var.name,
            var,
            _serialize_type_for_checkmember(itype),
            is_lvalue,
            no_deferral,
            is_operator,
            self.resolver if resolver is self._UNSET else resolver,
        )

    def _mx_and_calls(
        self, itype: Instance, is_lvalue: bool = False
    ) -> tuple[Any, list[tuple[Any, ...]]]:
        from types import SimpleNamespace

        from mypy.checkmember import MemberContext

        calls: list[tuple[Any, ...]] = []
        msg = SimpleNamespace(
            read_only_property=lambda *a: calls.append(("read_only_property",)),
            cant_assign_to_classvar=lambda *a: calls.append(("cant_assign_to_classvar",)),
            cant_assign_to_method=lambda *a: calls.append(("cant_assign_to_method",)),
        )

        def _partial(typ: object, lv: bool, var: Var, ctx: object) -> Any:
            calls.append(("partial", str(typ)))
            return AnyType(TypeOfAny.special_form)

        chk = SimpleNamespace(
            msg=msg,
            plugin=SimpleNamespace(get_attribute_hook=lambda fullname: None),
            handle_cannot_determine_type=lambda name, ctx: calls.append(("not_ready", name)),
            handle_partial_var_type=_partial,
        )
        mx = MemberContext(
            is_lvalue=is_lvalue,
            is_super=False,
            is_operator=False,
            original_type=itype,
            context=NameExpr("A"),
            chk=cast(Any, chk),
        )
        return mx, calls

    def _assert_value(
        self,
        name: str,
        var: Var,
        itype: Instance,
        expected: tuple[str, list[tuple[Any, ...]]],
        *,
        is_lvalue: bool = False,
    ) -> None:
        """`analyze_var`'s result and recorded side effects on the live path.

        The head gate this suite was written around (`rust_classify_analyze_var`)
        retired in #1739, so a gate-off vs gate-on comparison of `analyze_var`
        only re-read the same Python. The expected tuple is the value both arms
        produced under the parity assertion this replaced; the retirement pin
        `NativeCheckmemberRetiredSeamsSuite` re-checks it against the gate off.
        """
        from mypy.checkmember import analyze_var

        mx, calls = self._mx_and_calls(itype, is_lvalue)
        result = analyze_var(name, var, itype, mx)
        got = (str(result), calls)
        assert got == expected, f"{name}: {got!r} != {expected!r}"

    # --- direct seam tag tests ---

    def test_seam_getter_plain(self) -> None:
        var = self._make_var("x", typ=Instance(self.info, []))
        assert self._seam(var, Instance(self.info, [])) == 1
        assert self._seam(var, Instance(self.info, []), is_lvalue=True) == 1

    def test_seam_setter_lvalue(self) -> None:
        setter = CallableType([], [], [], AnyType(TypeOfAny.special_form), Instance(self.info, []))
        var = self._make_var(
            "prop", typ=Instance(self.info, []), setter_type=setter, is_settable_property=True
        )
        assert self._seam(var, Instance(self.info, []), is_lvalue=True) == 0
        assert self._seam(var, Instance(self.info, []), is_lvalue=False) == 1

    def test_seam_setter_falls_back_to_getter_type(self) -> None:
        # A settable property read as an lvalue with no setter type and
        # a ready var falls back to var.type: the tag is still SETTER
        # (the setter path was taken; the typ fallback is the shim's job).
        var = self._make_var("x", typ=Instance(self.info, []), is_settable_property=True)
        assert self._seam(var, Instance(self.info, []), is_lvalue=True) == 0

    def test_seam_partial(self) -> None:

        inner = Var("x")
        var = self._make_var("x", typ=PartialType(None, inner))
        assert self._seam(var, Instance(self.info, [])) == 2

    def test_seam_partial_beats_enum(self) -> None:

        inner = Var("RED")
        var = Var("RED", PartialType(None, inner))
        var.info = self.enum_info
        var.is_ready = True
        var.has_explicit_value = True
        self.enum_info.names["RED"] = SymbolTableNode(MDEF, var)
        assert self._seam(var, Instance(self.enum_info, [])) == 2

    def test_seam_not_ready(self) -> None:
        var = self._make_var("x", typ=None, is_ready=False)
        assert self._seam(var, Instance(self.info, [])) == 3
        assert self._seam(var, Instance(self.info, []), no_deferral=True) == 5

    def test_seam_unbound_any_ready_no_type(self) -> None:
        var = self._make_var("x", typ=None, is_ready=True)
        assert self._seam(var, Instance(self.info, [])) == 5

    def test_seam_enum_literal(self) -> None:
        var = Var("RED", Instance(self.enum_info, []))
        var.info = self.enum_info
        var.is_ready = True
        var.has_explicit_value = True
        self.enum_info.names["RED"] = SymbolTableNode(MDEF, var)
        assert self._seam(var, Instance(self.enum_info, []), is_lvalue=False) == 4
        # An lvalue enum access keeps the getter path (the literal arm
        # discards the head result, which the lvalue body computed).
        assert self._seam(var, Instance(self.enum_info, []), is_lvalue=True) == 1

    def test_seam_enum_name_value_excluded(self) -> None:
        for name in ("name", "value"):
            var = Var(name, Instance(self.enum_info, []))
            var.info = self.enum_info
            var.is_ready = True
            var.has_explicit_value = True
            self.enum_info.names[name] = SymbolTableNode(MDEF, var)
            assert self._seam(var, Instance(self.enum_info, []), is_lvalue=False) == 1

    def test_seam_not_ready_beats_enum(self) -> None:
        # The not-ready callback is a head-body side effect, so an
        # unready enum member must not collapse into ENUM_LITERAL.
        var = Var("RED")
        var.info = self.enum_info
        var.is_ready = False
        var.has_explicit_value = True
        self.enum_info.names["RED"] = SymbolTableNode(MDEF, var)
        assert self._seam(var, Instance(self.enum_info, [])) == 3

    def test_seam_defers_on_snapshot_miss(self) -> None:
        var = self._make_var("x", typ=Instance(self.info, []))
        empty_resolver = _type_kernel.build_native_resolver([], [])
        assert self._seam(var, Instance(self.info, []), resolver=empty_resolver) is None

    def test_seam_defers_on_fake_info(self) -> None:
        var = Var("x")
        assert self._seam(var, Instance(self.info, [])) is None

    # --- value + side-effect pins through real analyze_var ---

    def test_value_getter(self) -> None:
        var = self._make_var("x", typ=Instance(self.info, []))
        self._assert_value("x", var, Instance(self.info, []), ("mod.A", []))

    def test_value_read_only_property_lvalue(self) -> None:
        var = self._make_var("x", typ=Instance(self.info, []), is_property=True)
        self._assert_value(
            "x", var, Instance(self.info, []), ("mod.A", [("read_only_property",)]), is_lvalue=True
        )

    def test_value_setter_lvalue(self) -> None:
        setter = CallableType([], [], [], AnyType(TypeOfAny.special_form), Instance(self.info, []))
        var = self._make_var(
            "x", typ=Instance(self.info, []), setter_type=setter, is_settable_property=True
        )
        self._assert_value(
            "x", var, Instance(self.info, []), ("def () -> Any", []), is_lvalue=True
        )

    def test_value_partial(self) -> None:
        # A PartialType var returns from the handler before any surviving
        # checkmember gate, so the retired comparison could not fail here.
        inner = Var("x")
        var = Var("x", PartialType(None, inner))
        var.info = self.info
        var.is_ready = True
        self.info.names["x"] = SymbolTableNode(MDEF, var)
        self._assert_value(
            "x", var, Instance(self.info, []), ("Any", [("partial", "<partial None>")])
        )

    def test_value_not_ready(self) -> None:
        var = self._make_var("x", typ=None, is_ready=False)
        self._assert_value("x", var, Instance(self.info, []), ("Any", [("not_ready", "x")]))

    def test_value_unbound_any(self) -> None:
        var = self._make_var("x", typ=None, is_ready=True)
        self._assert_value("x", var, Instance(self.info, []), ("Any", []))

    def test_value_enum_literal(self) -> None:
        var = Var("RED", Instance(self.enum_info, []))
        var.info = self.enum_info
        var.is_ready = True
        var.has_explicit_value = True
        self.enum_info.names["RED"] = SymbolTableNode(MDEF, var)
        self._assert_value("RED", var, Instance(self.enum_info, []), ("Literal[mod.E.RED]?", []))

    def test_value_enum_member_bind_tail(self) -> None:
        # Real enum members are class-body assignments: is_inferred=True
        # (so is_instance_var is False) and is_initialized_in_class=True.
        # The enum-literal wrap happens in the tail after the getter arm.
        var = Var("RED", Instance(self.enum_info, []))
        var.info = self.enum_info
        var.is_ready = True
        var.is_inferred = True
        var.is_initialized_in_class = True
        var.has_explicit_value = True
        self.enum_info.names["RED"] = SymbolTableNode(MDEF, var)
        self._assert_value("RED", var, Instance(self.enum_info, []), ("Literal[mod.E.RED]?", []))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMemberVarDispatchSuite(Suite):
    """Parity tests for the var arm of the member-access dispatch
    (issue #1234, `mypy.checkmember.analyze_member_var_access`).

    The var arm reduces to a live-fact decision chain: a plain `Var` or a
    non-deprecated `Decorator` head (unwrapped to `.var`), the
    `expand_without_binding` expansion, the class-var `call_type`
    arbitration for a single unbound CallableType, the trivial-self bind
    path, the `plugin_hook_known_absent` fast path, and the descriptor
    pass-through. Everything else defers (`None`) to the pure-Python
    body; in particular a property-bearing `call_type` defers because
    Python's `expand_and_bind_callable` runs the property-extract tail
    (returns the getter ret_type or the setter arg on an lvalue), which
    the bound-callable arm does not mirror.

    Direct seam calls assert the exact result per branch; the gate-off
    vs gate-on differential drives the real `analyze_union_member_access`
    (the fill path for a deferred union item is the plain member access,
    so a property item exercises the fallback end-to-end).
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_plugin_hook_registry
        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active = _set_native_checkmember_active
        self._set_resolver = _set_native_checkmember_resolver
        self._set_plugin_hook = _set_native_plugin_hook_registry
        self._set_plugin_hook(SimpleNamespace(has_hook_for=lambda kind, fullname: False), False)
        self._set_active(True)
        self.info = self._typeinfo("mod.A")
        self.base = self._typeinfo("mod.Base")
        self.info.bases = [Instance(self.base, [])]
        self.info.mro = [self.info, self.base]
        self.prop_info = self._typeinfo("mod.P")
        # Wire decode fixup needs the fallbacks of the __bool__ callable
        # (builtins.bool/function) resolvable to live TypeInfos.
        self.bool_info = self._typeinfo("builtins.bool")
        self.function_info = self._typeinfo("builtins.function")
        self._live_map = {
            "mod.A": self.info,
            "mod.Base": self.base,
            "mod.P": self.prop_info,
            "builtins.bool": self.bool_info,
            "builtins.function": self.function_info,
        }
        self.resolver = _type_kernel.build_native_resolver(list(self._live_map.values()), [])
        self.resolver.set_live_typeinfo_map(dict(self._live_map))
        set_wire_typeinfo_map(dict(self._live_map))
        self._set_resolver(self.resolver)

    def tearDown(self) -> None:
        self._set_plugin_hook(None, False)
        self._set_resolver(None)
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _typeinfo(self, fullname: str = "mod.A") -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = [info]
        return info

    def _register_var(
        self,
        info: TypeInfo,
        name: str,
        typ: Type | None,
        *,
        is_property: bool = False,
        is_initialized_in_class: bool = False,
        is_inferred: bool = False,
        is_staticmethod: bool = False,
        is_settable_property: bool = False,
        setter_type: CallableType | None = None,
        is_ready: bool = True,
    ) -> Var:
        var = Var(name, typ)
        var.info = info
        var.is_property = is_property
        var.is_initialized_in_class = is_initialized_in_class
        var.is_inferred = is_inferred
        var.is_staticmethod = is_staticmethod
        var.is_settable_property = is_settable_property
        var.setter_type = setter_type
        var.is_ready = is_ready
        info.names[name] = SymbolTableNode(MDEF, var)
        return var

    def _register_decorator(
        self,
        info: TypeInfo,
        name: str,
        typ: FunctionLike,
        *,
        is_property: bool = False,
        is_initialized_in_class: bool = False,
        is_inferred: bool = False,
        deprecated: str | None = None,
        is_trivial_self: bool = True,
    ) -> Decorator:
        from mypy.nodes import Block, FuncDef

        func = FuncDef(name, [], Block([]), typ)
        func.info = info
        func.is_trivial_self = is_trivial_self
        func.deprecated = deprecated
        var = Var(name, typ)
        var.info = info
        var.is_property = is_property
        var.is_initialized_in_class = is_initialized_in_class
        var.is_inferred = is_inferred
        node = Decorator(func, [], var)
        info.names[name] = SymbolTableNode(MDEF, node)
        return node

    def _seam(
        self,
        instance: Instance,
        name: str,
        *,
        is_operator: bool = False,
        is_self: bool = False,
        start_raw_id: int = 100,
        plugin: object | None = None,
    ) -> tuple[int, bool, Type] | None:
        from mypy.checkmember import _serialize_type_for_checkmember

        result = _type_kernel.rust_analyze_member_access(
            self.resolver,
            name,
            _serialize_type_for_checkmember(instance),
            _serialize_type_for_checkmember(instance),
            False,  # is_lvalue
            False,  # is_super
            is_operator,
            is_self,
            False,  # preserve_type_var_ids
            start_raw_id,
            True,  # strict_optional
            plugin,
        )
        if result is None:
            return None
        next_raw_id, changed, wire_bytes = result
        from mypy.checkmember import _deserialize_type_for_checkmember

        decoded = _deserialize_type_for_checkmember(bytes(wire_bytes), freeze=True)
        assert decoded is not None
        return next_raw_id, changed, decoded

    # --- direct seam tests ---

    def test_seam_plain_var_engages(self) -> None:
        self._register_var(self.info, "x", Instance(self.base, []))
        result = self._seam(Instance(self.info, []), "x")
        assert result is not None, "expected native result for a plain var"
        _, _, decoded = result
        assert str(decoded) == "mod.Base"

    def test_seam_property_callable_native_tail(self) -> None:
        # The regression shape: a property getter CallableType under the
        # class-var call_type gate binds and then applies the ret_type tail.
        # The whole path is native now and returns "mod.Base", like Python.
        getter = CallableType(
            [Instance(self.base, [])],
            [ArgKind.ARG_POS],
            ["self"],
            Instance(self.base, []),
            Instance(self.info, []),
            name=None,
        )
        self._register_var(
            self.info,
            "x",
            getter,
            is_property=True,
            is_initialized_in_class=True,
            is_inferred=True,
        )
        result = self._seam(Instance(self.info, []), "x")
        assert result is not None
        _, _, decoded = result
        assert str(decoded) == "mod.Base"

    def test_seam_property_not_in_gate_engages(self) -> None:
        # is_property with the call_type gate not entered (is_inferred True
        # with is_operator access) is covered by the union fill differential;
        # a property var outside the gate class path still passes through.
        getter = CallableType(
            [Instance(self.base, [])],
            [ArgKind.ARG_POS],
            ["self"],
            Instance(self.base, []),
            Instance(self.info, []),
            name=None,
        )
        self._register_var(self.info, "x", getter, is_property=True)
        result = self._seam(Instance(self.info, []), "x")
        assert result is not None
        _, _, decoded = result
        assert str(decoded) == "def (self: mod.Base) -> mod.Base"

    def test_seam_nontrivial_bind_defers(self) -> None:
        # An unbound callable class var with is_trivial_self False hits
        # the non-trivial bind path, which defers; a non-callable class
        # var in the gate is a no-op re-expansion and answers natively.
        sig = CallableType(
            [Instance(self.base, [])],
            [ArgKind.ARG_POS],
            ["self"],
            Instance(self.base, []),
            Instance(self.base, []),
            is_bound=False,
        )
        self._register_var(self.base, "x", sig, is_initialized_in_class=True, is_inferred=True)
        assert self._seam(Instance(self.info, []), "x") is None

    def test_seam_init_defers(self) -> None:
        self._register_var(self.base, "__init__", Instance(self.base, []))
        assert self._seam(Instance(self.info, []), "__init__") is None

    def test_seam_non_var_node_defers(self) -> None:
        from mypy.nodes import SymbolTableNode

        self.info.names["x"] = SymbolTableNode(MDEF, self.base)
        assert self._seam(Instance(self.info, []), "x") is None

    def test_seam_deprecated_decorator_defers(self) -> None:
        getter = CallableType(
            [Instance(self.base, [])],
            [ArgKind.ARG_POS],
            ["self"],
            Instance(self.base, []),
            Instance(self.info, []),
            name=None,
        )
        self._register_decorator(
            self.info,
            "x",
            getter,
            is_property=True,
            is_initialized_in_class=True,
            deprecated="gone",
        )
        assert self._seam(Instance(self.info, []), "x") is None

    def test_seam_union_item_slots(self) -> None:
        from mypy.checkmember import (
            _deserialize_type_for_checkmember,
            _serialize_type_for_checkmember,
        )
        from mypy.types import UnionType

        self._register_var(self.info, "x", Instance(self.base, []))
        getter = CallableType(
            [Instance(self.prop_info, [])],
            [ArgKind.ARG_POS],
            ["self"],
            Instance(self.base, []),
            Instance(self.prop_info, []),
            name=None,
        )
        self._register_var(
            self.prop_info,
            "x",
            getter,
            is_property=True,
            is_initialized_in_class=True,
            is_inferred=True,
        )
        union = UnionType([Instance(self.info, []), Instance(self.prop_info, [])])
        result = _type_kernel.rust_analyze_union_member_access(
            self.resolver,
            _serialize_type_for_checkmember(union),
            "x",
            False,
            False,
            False,
            False,
            100,
            True,
        )
        assert result is not None
        _, changed, slots = result
        assert not changed
        assert len(slots) == 2
        assert slots[0] is not None
        plain = _deserialize_type_for_checkmember(bytes(slots[0]), freeze=True)
        assert str(plain) == "mod.Base"
        # The property item is native since the var-prop-bind port; the tail
        # returns the getter ret_type, matching the pure-Python per-item
        # loop (the unionproperty regression keeps a value assertion).
        assert slots[1] is not None
        prop = _deserialize_type_for_checkmember(bytes(slots[1]), freeze=True)
        assert str(prop) == "mod.Base"

    def test_none_seam_bool_fixed(self) -> None:
        from mypy.checkmember import (
            _deserialize_type_for_checkmember,
            _serialize_type_for_checkmember,
        )

        result = _type_kernel.rust_analyze_none_member_access(
            self.resolver,
            "__bool__",
            _serialize_type_for_checkmember(NoneType()),
            None,
            False,
            False,
            False,
            100,
            True,
        )
        assert result is not None
        _, _, wire_bytes = result
        decoded = _deserialize_type_for_checkmember(bytes(wire_bytes), freeze=True)
        assert str(decoded) == "def () -> Literal[False]"

    def test_none_seam_object_miss_defers(self) -> None:
        # builtins.object is not in the live map, so the dispatch defers.
        from mypy.checkmember import _serialize_type_for_checkmember

        result = _type_kernel.rust_analyze_none_member_access(
            self.resolver,
            "foo",
            _serialize_type_for_checkmember(NoneType()),
            None,
            False,
            False,
            False,
            100,
            True,
        )
        assert result is None

    def test_seam_var_hook_chain_decisions(self) -> None:
        # The var hook gate defers only when a hook provably applies; the
        # live plugin chain is authoritative when the registry fast path
        # cannot prove absence (user plugins, issue #1288).
        class _NoAttrHook:
            def get_attribute_hook(self, fullname: str) -> None:
                return None

        class _WithAttrHook:
            def get_attribute_hook(self, fullname: str) -> Callable[[Any], Any]:
                return lambda ctx: ctx.default_attr_type

        self._register_var(self.info, "x", Instance(self.base, []))
        present = SimpleNamespace(has_hook_for=lambda kind, fullname: True)
        self._set_plugin_hook(present, False)
        try:
            # Chain answers "no hook": the native path proceeds.
            result = self._seam(Instance(self.info, []), "x", plugin=_NoAttrHook())
            assert result is not None
            _, _, decoded = result
            assert str(decoded) == "mod.Base"
            # Chain says a hook applies: defer so Python applies it.
            assert self._seam(Instance(self.info, []), "x", plugin=_WithAttrHook()) is None
            # No plugin handle: defer (the pre-#1288 behavior).
            assert self._seam(Instance(self.info, []), "x", plugin=None) is None
        finally:
            self._set_plugin_hook(
                SimpleNamespace(has_hook_for=lambda kind, fullname: False), False
            )

    # --- gate differentials ---

    def _union_mx_and_calls(self, itype: Instance) -> tuple[Any, list[tuple[Any, ...]]]:
        from contextlib import nullcontext
        from types import SimpleNamespace

        from mypy.checkmember import MemberContext

        calls: list[tuple[Any, ...]] = []
        msg = SimpleNamespace(
            read_only_property=lambda *a: calls.append(("read_only_property",)),
            cant_assign_to_classvar=lambda *a: calls.append(("cant_assign_to_classvar",)),
            cant_assign_to_method=lambda *a: calls.append(("cant_assign_to_method",)),
            disable_type_names=lambda: nullcontext(),
        )
        chk = SimpleNamespace(
            msg=msg,
            plugin=SimpleNamespace(get_attribute_hook=lambda fullname: None),
            handle_cannot_determine_type=lambda name, ctx: calls.append(("not_ready", name)),
            warn_deprecated=lambda *a: calls.append(("warn_deprecated",)),
            get_final_context=lambda: None,
        )
        mx = MemberContext(
            is_lvalue=False,
            is_super=False,
            is_operator=False,
            original_type=itype,
            context=NameExpr("A"),
            chk=cast(Any, chk),
        )
        return mx, calls

    def _assert_union_differential(self, union: UnionType, name: str) -> None:
        from mypy.checkmember import analyze_union_member_access

        def run() -> tuple[str, list[tuple[Any, ...]]]:
            mx, calls = self._union_mx_and_calls(Instance(self.info, []))
            result = analyze_union_member_access(name, union, mx)
            # Native contention of non-diagnostic calls (warn_deprecated)
            # is inherent: a natively-mapped item makes one fewer Python
            # member-access call. Compare result and recorded diagnostics.
            return str(result), [c for c in calls if c[0] != "warn_deprecated"]

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert off == on, f"gate mismatch: off={off!r} on={on!r}"

    def test_differential_union_with_property_item(self) -> None:
        # The rooted unionproperty regression: a property item must produce
        # the same narrowed result with and without the native union map
        # (Python fills the property slot either way; plain items go native).
        self._register_var(self.info, "x", Instance(self.base, []))
        getter = CallableType(
            [Instance(self.prop_info, [])],
            [ArgKind.ARG_POS],
            ["self"],
            Instance(self.base, []),
            Instance(self.prop_info, []),
            name=None,
        )
        self._register_var(
            self.prop_info,
            "x",
            getter,
            is_property=True,
            is_initialized_in_class=True,
            is_inferred=True,
        )
        union = UnionType([Instance(self.info, []), Instance(self.prop_info, [])])
        self._assert_union_differential(union, "x")

    def test_differential_union_plain_items(self) -> None:
        self._register_var(self.info, "x", Instance(self.base, []))
        self._register_var(self.base, "x", Instance(self.base, []))
        union = UnionType([Instance(self.info, []), Instance(self.base, [])])
        self._assert_union_differential(union, "x")


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeAmaResidualSuite(Suite):
    """Wave 46a residual deferrals on the ama seam (#1449).

    Retires the var-arm is_self blanket defer and the enum head gate.
    is_self narrows to a pure self_type test: a typevar-like proper
    self_type substitutes mx.self_type (supported_self_type with
    instances/callables disallowed), any other annotation leaves the
    type unchanged. Enum vars run the full arm plus the literal-wrap /
    nonmember-unwrap tail with a resolver-snapshot membership test.

    Direct seam calls assert the exact result per branch; the
    `test_member_access_*` pins assert the exact result the real
    `_analyze_member_access` produces for the same shapes through a stub
    MemberContext.

    The ama gate retired in #1823 (`_analyze_member_access` carries no gate
    and loads no `rust_*` name; pinned by `NativeAnalyzeMemberAccessRetiredSuite`
    in `testtypes_native_retired_checkmember.py`). The is_self shape's two
    arms could not differ: the gate-on arm's sole native call is the
    descriptor head's tag-0 path, which returns the original object by
    construction. The enum shape's arms differed only in
    `expand_without_binding`, a different live seam that
    `NativeAnalyzeVarSuite::test_value_enum_literal` exercises on the same
    shape. Both former differentials are value pins now, and the Rust
    pyfunction stays registered and is called directly above.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_plugin_hook_registry
        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active = _set_native_checkmember_active
        self._set_resolver = _set_native_checkmember_resolver
        self._set_plugin_hook = _set_native_plugin_hook_registry
        self._set_plugin_hook(SimpleNamespace(has_hook_for=lambda kind, fullname: False), False)
        self._set_active(True)
        self.info = self._typeinfo("mod.A")
        self.base = self._typeinfo("mod.Base")
        self.info.bases = [Instance(self.base, [])]
        self.info.mro = [self.info, self.base]
        self.enum_info = self._typeinfo("mod.E")
        self.enum_info.is_enum = True
        # enum_members is a computed property over names, and the resolver
        # snapshot captures it at build time: seed the members first.
        for member in ("RED", "GREEN", "BLUE"):
            var = Var(member, Instance(self.enum_info, []))
            var.info = self.enum_info
            var.is_initialized_in_class = True
            var.has_explicit_value = True
            self.enum_info.names[member] = SymbolTableNode(MDEF, var)
        self.nonmember_info = self._typeinfo("enum.nonmember")
        self.str_info = self._typeinfo("builtins.str")
        self.bool_info = self._typeinfo("builtins.bool")
        self.function_info = self._typeinfo("builtins.function")
        self.object_info = self._typeinfo("builtins.object")
        # A self-reference TypeVar on mod.A (var.info.self_type) plus an
        # unrelated typevar standing in for mx.self_type; both ride the
        # wire with the same upper_bound so expansions stay decidable.
        self.info_inst = Instance(self.info, [])
        self.self_tvar = TypeVarType(
            "Self", "Self", TypeVarId(100), [], self.info_inst, AnyType(TypeOfAny.special_form)
        )
        self.info.self_type = self.self_tvar
        self.other_tvar = TypeVarType(
            "S", "S", TypeVarId(200), [], self.info_inst, AnyType(TypeOfAny.special_form)
        )
        self._live_map = {
            "mod.A": self.info,
            "mod.Base": self.base,
            "mod.E": self.enum_info,
            "enum.nonmember": self.nonmember_info,
            "builtins.str": self.str_info,
            "builtins.bool": self.bool_info,
            "builtins.function": self.function_info,
            "builtins.object": self.object_info,
        }
        self.resolver = _type_kernel.build_native_resolver(list(self._live_map.values()), [])
        self.resolver.set_live_typeinfo_map(dict(self._live_map))
        set_wire_typeinfo_map(dict(self._live_map))
        self._set_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_plugin_hook(None, False)
        self._set_resolver(None)
        set_wire_typeinfo_map(None)
        self._set_active(False)

    def _typeinfo(self, fullname: str = "mod.A") -> TypeInfo:
        from mypy.nodes import Block, SymbolTable

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = [info]
        return info

    def _register_var(
        self, info: TypeInfo, name: str, typ: Type | None, *, is_initialized_in_class: bool = False
    ) -> None:
        var = Var(name, typ)
        var.info = info
        var.is_initialized_in_class = is_initialized_in_class
        info.names[name] = SymbolTableNode(MDEF, var)

    def _seam(
        self,
        instance: Instance,
        name: str,
        *,
        is_operator: bool = False,
        is_self: bool = False,
        self_type: Type | None = None,
        start_raw_id: int = 100,
    ) -> Type | None:
        from mypy.checkmember import (
            _deserialize_type_for_checkmember,
            _serialize_type_for_checkmember,
        )

        result = _type_kernel.rust_analyze_member_access(
            self.resolver,
            name,
            _serialize_type_for_checkmember(instance),
            _serialize_type_for_checkmember(self_type or instance),
            False,  # is_lvalue
            False,  # is_super
            is_operator,
            is_self,
            False,  # preserve_type_var_ids
            start_raw_id,
            True,  # strict_optional
            None,  # plugin (registry stub proves absence)
        )
        if result is None:
            return None
        _, changed, wire_bytes = result
        del changed
        decoded = _deserialize_type_for_checkmember(bytes(wire_bytes), freeze=True)
        assert decoded is not None
        return decoded

    # --- direct seam tests: is_self substitute-vs-skip gate ---

    def test_seam_is_self_typevar_substitutes(self) -> None:
        # A typevar-like self_type is supported: the Self reference in the
        # var type is substituted with mx.self_type.
        self._register_var(self.info, "x", self.self_tvar)
        decoded = self._seam(Instance(self.info, []), "x", is_self=True, self_type=self.other_tvar)
        assert decoded is not None, "is_self with a typevar self_type must be native"
        assert str(decoded) == "S"

    def test_seam_is_self_instance_skips_substitution(self) -> None:
        # An Instance self_type is not supported (allow_instances=False):
        # the Self reference stays even though var.info.self_type is set.
        self._register_var(self.info, "x", self.self_tvar)
        decoded = self._seam(
            Instance(self.info, []), "x", is_self=True, self_type=Instance(self.info, [])
        )
        assert decoded is not None, "is_self with an instance self_type must be native"
        assert str(decoded) == "Self"

    def test_seam_is_self_plain_var(self) -> None:
        # The dominant shape: self.attr on a NamedTuple / plain class where
        # the var does not reference Self. No substitution, plain expand.
        self._register_var(self.info, "x", Instance(self.base, []), is_initialized_in_class=True)
        decoded = self._seam(
            Instance(self.info, []), "x", is_self=True, self_type=Instance(self.info, [])
        )
        assert decoded is not None, "is_self plain var access must be native"
        assert str(decoded) == "mod.Base"

    def test_seam_is_self_alias_defers(self) -> None:
        # An alias self_type has no wire target, so the substitute-vs-skip
        # decision defers and Python re-runs with live state.
        from mypy.nodes import TypeAlias as NodeAlias

        alias_node = NodeAlias(AnyType(TypeOfAny.special_form), "mod.X", "mod", 0, 0)
        alias_type = TypeAliasType(alias_node, [])
        self._register_var(self.info, "x", Instance(self.base, []))
        assert self._seam(Instance(self.info, []), "x", is_self=True, self_type=alias_type) is None

    # --- direct seam tests: enum tail ---

    def test_seam_enum_member_wraps_literal(self) -> None:
        decoded = self._seam(Instance(self.enum_info, []), "RED")
        assert decoded is not None, "enum member access must be native"
        proper = get_proper_type(decoded)
        assert isinstance(proper, Instance)
        assert proper.last_known_value is not None
        lkv = proper.last_known_value
        assert isinstance(lkv, LiteralType)
        assert lkv.value == "RED"
        assert str(decoded) == "Literal[mod.E.RED]?"

    def test_seam_enum_name_value_skip_wrap(self) -> None:
        # name/value members do not wrap: the result stays the var type.
        self._register_var(self.enum_info, "value", Instance(self.str_info, []))
        decoded = self._seam(Instance(self.enum_info, []), "value")
        assert decoded is not None, "enum name/value access must be native"
        assert str(decoded) == "str"

    def test_seam_enum_nonmember_unwraps(self) -> None:
        # A member typed `enum.nonmember[X]` unwraps to X on access.
        self._register_var(
            self.enum_info,
            "x",
            Instance(self.nonmember_info, [Instance(self.base, [])]),
            is_initialized_in_class=True,
        )
        decoded = self._seam(Instance(self.enum_info, []), "x")
        assert decoded is not None, "enum nonmember access must be native"
        assert str(decoded) == "mod.Base"

    def test_seam_enum_live_miss_defers(self) -> None:
        # The enum tail reads enum_members from the live info; a class
        # absent from the live map defers to Python.
        ghost = self._typeinfo("mod.Ghost")
        self._register_var(ghost, "y", Instance(self.base, []))
        assert self._seam(Instance(ghost, []), "y") is None

    # --- value pins through the real function ---

    def _stub_mx(
        self, itype: Instance, *, is_self: bool = False, self_type: Type | None = None
    ) -> Any:
        from contextlib import nullcontext

        from mypy.checkmember import MemberContext

        msg = SimpleNamespace(
            has_no_attr=lambda *a, **kw: 0,
            filter_errors=lambda *a, **kw: nullcontext(),
            cant_assign_to_method=lambda *a, **kw: None,
            read_only_property=lambda *a, **kw: None,
            cant_assign_to_classvar=lambda *a, **kw: None,
        )
        chk = SimpleNamespace(
            warn_deprecated=lambda *a, **kw: None,
            plugin=SimpleNamespace(get_attribute_hook=lambda fullname: None),
            module_refs=set(),
            msg=msg,
            handle_partial_var_type=lambda *a, **kw: AnyType(TypeOfAny.special_form),
            expr_checker=SimpleNamespace(
                analyze_static_reference=lambda *a, **kw: AnyType(TypeOfAny.special_form)
            ),
            scope=SimpleNamespace(active_self_type=lambda: itype),
            checking_missing_await=False,
            checking_await_set=nullcontext(),
            get_precise_awaitable_type=lambda *a, **kw: None,
        )
        return MemberContext(
            is_lvalue=False,
            is_super=False,
            is_operator=False,
            original_type=itype,
            context=NameExpr("x"),
            chk=cast(Any, chk),
            self_type=self_type,
            is_self=is_self,
        )

    def test_member_access_is_self_substitution(self) -> None:
        from mypy.checkmember import _analyze_member_access

        self._register_var(self.info, "x", self.self_tvar)
        mx = self._stub_mx(self.info_inst, is_self=True, self_type=self.other_tvar)
        assert str(_analyze_member_access("x", self.self_tvar, mx)) == "S"

    def test_member_access_enum_member_literal(self) -> None:
        from mypy.checkmember import _analyze_member_access

        receiver = Instance(self.enum_info, [])
        mx = self._stub_mx(receiver)
        assert str(_analyze_member_access("RED", receiver, mx)) == "Literal[mod.E.RED]?"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAddClassTvarsFreeSuite(Suite):
    """Wave-61B: free-result expansion in `add_class_tvars` (#1512).

    The audit pinned all 8 cold-self-check fallbacks to
    `expand_type_by_instance_core_alias` returning None: a bound
    classmethod signature carrying its own fresh TypeVar (e.g.
    `dict.fromkeys`) left that var unbound after substituting the
    receiver's class vars. Python's `expand_type_by_instance` never
    defers there; the following `freeze_all_type_vars` (already run by
    the shim on the decoded result) reifies the leftover var. The
    CallableType arm now uses the free variant, matching the IAMA member
    tail. Gate-off vs gate-on must produce the same rendered type.
    """

    def setUp(self) -> None:
        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._type_infos = _base_infos(self.fx)
        self._resolver = _type_kernel.build_native_resolver(self._type_infos, [])
        _set_native_checkmember_active(True)
        _set_native_checkmember_resolver(self._resolver)
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})
        # Production class tvars carry the class fullname as namespace;
        # make_type_info leaves it empty, and the Rust env keys class tvars
        # by (raw_id, class fullname). Namespace the fixture tvar.
        self.ns_t = self.fx.gi.defn.type_vars[0]
        self.ns_t.id.namespace = self.fx.gi.fullname
        self.s = TypeVarType(
            "S", "S", TypeVarId(50), [], self.fx.o, AnyType(TypeOfAny.special_form)
        )
        self.method = CallableType(
            [Instance(self.fx.gi, [self.ns_t]), self.s],
            [ARG_POS, ARG_POS],
            ["cls", "x"],
            Instance(self.fx.gi, [self.ns_t]),
            self.fx.function,
            variables=[self.s],
        )
        self.isuper = Instance(self.fx.gi, [self.fx.a])

    def tearDown(self) -> None:
        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_checkmember_active(False)
        _set_native_checkmember_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.checkmember import _set_native_checkmember_active

        _set_native_checkmember_active(active)
        try:
            return fn()
        finally:
            _set_native_checkmember_active(True)

    def _mx(self) -> Any:
        import contextlib

        from mypy.checkmember import MemberContext

        return MemberContext(
            is_lvalue=False,
            is_super=False,
            is_operator=False,
            original_type=self.method,
            context=NameExpr("x"),
            chk=cast(
                Any,
                SimpleNamespace(
                    msg=SimpleNamespace(
                        fail=lambda *a: None,
                        note=lambda *a: None,
                        filter_errors=lambda *a, **kw: contextlib.nullcontext(),
                        disable_type_names=lambda: contextlib.nullcontext(),
                    )
                ),
            ),
            self_type=self.isuper,
        )

    def test_seam_free_expansion_engages(self) -> None:
        from mypy.checkmember import (
            _deserialize_type_for_checkmember,
            _serialize_type_for_checkmember,
            add_class_tvars,
        )
        from mypy.typeops import freeze_all_type_vars

        expected = self._with_gate(
            False,
            lambda: add_class_tvars(
                self.method, self.isuper, True, self._mx(), is_trivial_self=True
            ),
        )
        result = _type_kernel.rust_add_class_tvars(
            self._resolver,
            _serialize_type_for_checkmember(self.method),
            _serialize_type_for_checkmember(self.isuper),
            True,
            True,
            False,
            b"",
            TypeVarId.next_raw_id,
            True,
        )
        assert result is not None, "free expansion deferred on a leftover method tvar"
        _next_raw_id, _changed, wire_bytes = result
        decoded = _deserialize_type_for_checkmember(bytes(wire_bytes), freeze=True)
        assert decoded is not None
        freeze_all_type_vars(decoded)
        assert str(decoded) == str(expected)

    def test_gate_parity_leftover_method_tvar(self) -> None:
        from mypy.checkmember import add_class_tvars

        mx = self._mx()

        def run() -> Type:
            return add_class_tvars(self.method, self.isuper, True, mx, is_trivial_self=True)

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert str(on) == str(off), f"add_class_tvars parity: {off!r} vs {on!r}"
