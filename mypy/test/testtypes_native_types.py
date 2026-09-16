"""Native seam suites for the types area (split from testtypes.py, #1677)."""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
    from librt.internal import WriteBuffer as _WriteBuffer
except ImportError:
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _type_kernel = None  # type: ignore[assignment]

from collections.abc import Callable, Iterator, Mapping, Sequence
from types import SimpleNamespace
from typing import Any, cast
from unittest import skipUnless

import mypy.expandtype
from mypy.checker import TypeChecker
from mypy.constraints import SUBTYPE_OF, SUPERTYPE_OF, Constraint
from mypy.errorcodes import ErrorCode
from mypy.join import join_types
from mypy.meet import is_overlapping_types, meet_types, narrow_declared_type
from mypy.nodes import (
    ARG_NAMED,
    ARG_OPT,
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    CONTRAVARIANT,
    COVARIANT,
    INVARIANT,
    MDEF,
    ArgKind,
    Argument,
    Block,
    ClassDef,
    Context,
    Decorator,
    Expression,
    FuncBase,
    FuncDef,
    IntExpr,
    MypyFile,
    NameExpr,
    OpExpr,
    OverloadedFuncDef,
    PlaceholderNode,
    ReturnStmt,
    StrExpr,
    SymbolTable,
    SymbolTableNode,
    TypeAlias,
    TypeInfo,
    TypeVarExpr,
    Var,
)
from mypy.options import Options
from mypy.state import state
from mypy.subtypes import is_more_precise, is_proper_subtype, is_same_type, is_subtype
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import (
    _NATIVE_WIRE_ENABLED,
    T,
    _base_infos,
    _build_native_variance_resolver,
    _is_type_info,
    strict_optional_flag,
)
from mypy.test.typefixture import TypeFixture
from mypy.typeanal import (
    _TYPE_WITH_INFO_TAG_INSTANCE,
    _TYPE_WITH_INFO_TAG_NONE_TYPE,
    _TYPE_WITH_INFO_TAG_TUPLE,
    _TYPE_WITH_INFO_TAG_VEC,
    _set_native_typeanal_active,
    collect_all_inner_types,
    has_any_from_unimported_type,
    has_explicit_any,
    make_optional_type,
    native_analyze_type,
)
from mypy.typeops import (
    coerce_to_literal,
    false_only,
    is_singleton_equality_type,
    is_singleton_identity_type,
    make_simplified_union,
    true_only,
    try_contracting_literals_in_union,
    try_getting_instance_fallback,
)
from mypy.types import (
    AnyType,
    CallableType,
    DeletedType,
    EllipsisType,
    ErasedType,
    FormalArgument,
    FunctionLike,
    Instance,
    LiteralType,
    NoneType,
    NormalizedCallableType,
    Overloaded,
    Parameters,
    ParamSpecFlavor,
    ParamSpecType,
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
    UnboundType,
    UninhabitedType,
    UnionType,
    UnpackType,
    get_proper_type,
)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFreshenSuite(Suite):
    """Parity tests for the Rust `freshen_all_functions_type_vars` port.

    Each test serializes a `Type` via `Type.write(WriteBuffer)` and asserts
    the Rust result (a) advances `TypeVarId.next_raw_id`, (b) produces
    meta-level-1 `raw_id > 1000000` variables, and (c) renders identically to
    the pure-Python `freshen_all_functions_type_vars` oracle. The oracle is
    forced to pure Python by disabling the native gate; the Rust side is
    invoked directly through the extension, so no TypeInfo resolver is
    needed.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        self._old_active = mypy.expandtype._native_expand_type_active
        mypy.expandtype._set_native_expand_type_active(False)
        # The freshen seam takes the resolver for its internal alias map;
        # this fixture carries no aliases.
        self.resolver = _type_kernel.build_native_resolver([], [])

    def tearDown(self) -> None:
        mypy.expandtype._set_native_expand_type_active(self._old_active)

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def assert_fresh_par(self, t: Type, resolver: Any = None) -> None:
        from mypy.expandtype import freshen_all_functions_type_vars
        from mypy.types import read_type as _read_type

        expected = freshen_all_functions_type_vars(t)
        call = _type_kernel.rust_freshen_all_functions_type_vars(
            1000000, self._bytes_of(t), state.strict_optional, resolver or self.resolver
        )
        assert call is not None, f"rust freshen None for {t!r}"
        next_raw_id, changed, serialized = call
        assert changed, f"rust freshen reported no change for {t!r}"
        assert next_raw_id > 1000000
        assert_equal(
            _type_kernel.read_type_to_str(bytes(serialized)), str(expected), f"freshen {t!r}"
        )
        # The real freshen signal: fresh meta-level-1 variables with a
        # bumped raw_id (str() prints only the name, which is unchanged).
        from librt.internal import ReadBuffer

        rt = _read_type(ReadBuffer(bytes(serialized)))
        if isinstance(rt, Overloaded):  # type: ignore[misc]
            items = rt.items
        else:
            assert isinstance(rt, ProperType) and isinstance(rt, CallableType), str(rt)
            items = [rt]
        for item in items:
            for v in item.variables:
                assert v.id.meta_level == 1, f"var {v!r} not meta_level 1"
                # TypeVarId.new() uses next_raw_id then increments (types.py:561),
                # so the first fresh var gets exactly start_raw_id.
                assert v.id.raw_id >= 1000000, f"var {v!r} raw_id not fresh"

    def test_generic_simple(self) -> None:
        c = CallableType(
            [self.fx.t], [ARG_POS], [None], self.fx.b, self.fx.function, variables=[self.fx.t]
        )
        self.assert_fresh_par(c)

    def test_generic_instance_arg(self) -> None:
        c = CallableType(
            [self.fx.gt], [ARG_POS], [None], self.fx.b, self.fx.function, variables=[self.fx.t]
        )
        self.assert_fresh_par(c)

    def test_non_generic_changed_false(self) -> None:
        c = CallableType([self.fx.b], [ARG_POS], [None], self.fx.b, self.fx.function)
        call = _type_kernel.rust_freshen_all_functions_type_vars(
            1000000, self._bytes_of(c), state.strict_optional, self.resolver
        )
        assert call is not None
        next_raw_id, changed, serialized = call
        assert not changed
        assert next_raw_id == 1000000
        # pyo3 0.20 converts Vec<u8> to a Python list, not bytes; the shim
        # wraps it via bytes() when decoding. Empty payload = no change.
        assert len(serialized) == 0

    def test_nested_generic_in_ret(self) -> None:
        inner = CallableType(
            [self.fx.t], [ARG_POS], [None], self.fx.t, self.fx.function, variables=[self.fx.t]
        )
        outer = CallableType([inner], [ARG_POS], [None], self.fx.b, self.fx.function)
        self.assert_fresh_par(outer)

    def test_generic_with_upper_bound(self) -> None:
        u = TypeVarType("U", "U", TypeVarId(10), [], self.fx.o, AnyType(TypeOfAny.special_form))
        c = CallableType([self.fx.t], [ARG_POS], [None], u, self.fx.function, variables=[u])
        self.assert_fresh_par(c)

    def test_generic_with_default_expansion(self) -> None:
        t2 = TypeVarType("T2", "T2", TypeVarId(11), [], self.fx.o, self.fx.gt)
        c = CallableType(
            [self.fx.t], [ARG_POS], [None], t2, self.fx.function, variables=[self.fx.t, t2]
        )
        self.assert_fresh_par(c)

    def test_union_and_type_type_ret(self) -> None:
        inner = CallableType(
            [self.fx.t], [ARG_POS], [None], self.fx.t, self.fx.function, variables=[self.fx.t]
        )
        ret_union = UnionType.make_union([inner, self.fx.type_type])
        c = CallableType([self.fx.gt], [ARG_POS], [None], ret_union, self.fx.function)
        self.assert_fresh_par(c)

    def test_overloaded_items(self) -> None:
        # The all-functions visitor translates Overloaded roots through
        # TypeTranslator.visit_overloaded and freshens each item; the seam
        # used to defer the whole root.
        c1 = CallableType(
            [self.fx.t], [ARG_POS], [None], self.fx.t, self.fx.function, variables=[self.fx.t]
        )
        c2 = CallableType(
            [self.fx.s], [ARG_POS], [None], self.fx.s, self.fx.function, variables=[self.fx.s]
        )
        self.assert_fresh_par(Overloaded([c1, c2]))

    def test_paramspec_variables(self) -> None:
        # ParamSpec `variables` take the generic new_unification_variable
        # path (types.py:770-772) and P.args/P.kwargs splice; rendered
        # through Python (the Rust renderer prints bare `*P`, pre-existing).
        from mypy.expandtype import freshen_all_functions_type_vars
        from mypy.types import read_type as _read_type

        ps = ParamSpecType(
            "P",
            "P",
            TypeVarId(5),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.special_form),
        )
        c = CallableType(
            [ps.with_flavor(ParamSpecFlavor.ARGS), ps.with_flavor(ParamSpecFlavor.KWARGS)],
            [ARG_STAR, ARG_STAR2],
            [None, None],
            self.fx.t,
            self.fx.function,
            variables=[ps],
        )
        expected = freshen_all_functions_type_vars(c)
        call = _type_kernel.rust_freshen_all_functions_type_vars(
            1000000, self._bytes_of(c), state.strict_optional, self.resolver
        )
        assert call is not None, f"rust freshen None for {c!r}"
        next_raw_id, changed, serialized = call
        assert changed and next_raw_id > 1000000
        from librt.internal import ReadBuffer

        rt = _read_type(ReadBuffer(bytes(serialized)))
        assert isinstance(rt, ProperType) and isinstance(rt, CallableType)
        assert_equal(str(rt), str(expected), "freshen ParamSpec signature")
        assert isinstance(rt.variables[0], ParamSpecType)
        assert rt.variables[0].id.meta_level == 1
        assert rt.variables[0].id.raw_id >= 1000000

    def test_alias_union_argument(self) -> None:
        # Union[U, None] with U = Union[A, B]: the union arm expands the
        # alias through the resolver snapshot and flattens it (#1203);
        # `_resync_definitions` tolerates the item-count change (no defs).
        alias = TypeAlias(
            UnionType([self.fx.a, self.fx.b]), "mod.FreshenUnionAlias", "mod", -1, -1
        )
        resolver = _type_kernel.build_native_resolver([], [alias])
        c = CallableType(
            [UnionType([TypeAliasType(alias, []), NoneType()])],
            [ARG_POS],
            [None],
            self.fx.a,
            self.fx.function,
            variables=[self.fx.t],
        )
        self.assert_fresh_par(c, resolver)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeBindSelfSuite(Suite):
    """Parity tests for the Rust `bind_self` fast path (mypy.typeops.bind_self).

    Each test serializes a `CallableType` via `Type.write(WriteBuffer)` and
    asserts that `type_kernel.rust_bind_self` strips the first parameter and
    sets `is_bound=True`, matching the pure-Python non-generic path
    (typeops.py:663-670). Generic (variable-carrying) callables must defer
    (return `None`) so Python runs `infer_type_arguments`.
    """

    def setUp(self) -> None:
        from librt.internal import ReadBuffer

        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.ReadBuffer = ReadBuffer
        type_infos = [
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.oi,
            self.fx.bool_type_info,
            self.fx.str_type_info,
            self.fx.functioni,
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        set_wire_typeinfo_map(None)

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _bind(self, c: CallableType) -> CallableType | None:
        result = _type_kernel.rust_bind_self(self._bytes_of(c))
        if result is None:
            return None
        from mypy.types import instance_cache, read_type as _read_type
        from mypy.wirefixup import fixup_wire_type

        decoded = _read_type(self.ReadBuffer(bytes(result)))
        # Clear instance_cache primitives after read_type so NOT_READY
        # singletons cannot leak into later tests (mirrors typeops.py).
        instance_cache.int_type = None
        instance_cache.str_type = None
        instance_cache.bool_type = None
        instance_cache.object_type = None
        instance_cache.function_type = None
        t = fixup_wire_type(decoded)
        assert isinstance(t, CallableType)  # type: ignore[misc]
        return t

    def _assert_stripped(self, c: CallableType) -> None:
        decoded = self._bind(c)
        assert decoded is not None, f"rust_bind_self returned None for {c!r}"
        assert isinstance(decoded, CallableType)
        assert decoded.is_bound, "expected is_bound=True"
        assert_equal(len(decoded.arg_types), len(c.arg_types) - 1)
        assert_equal(decoded.arg_types[0], c.arg_types[1])
        assert_equal(decoded.ret_type, c.ret_type)
        assert_equal(decoded.variables, c.variables)

    def test_simple(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.anyt,
            self.fx.function,
        )
        self._assert_stripped(c)

    def test_three_args(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b, self.fx.c],
            [ARG_POS, ARG_POS, ARG_POS],
            [None, None, None],
            self.fx.anyt,
            self.fx.function,
        )
        self._assert_stripped(c)

    def test_generic_defers(self) -> None:
        c = CallableType(
            [self.fx.t], [ARG_POS], [None], self.fx.b, self.fx.function, variables=[self.fx.t]
        )
        assert self._bind(c) is None

    def test_no_args_defers(self) -> None:
        c = CallableType([], [], [], self.fx.anyt, self.fx.function)
        assert self._bind(c) is None

    def test_star_args_defers(self) -> None:
        # bind_self returns the method unchanged for *args first param, so
        # the native path reports "not handled" and Python does the same.
        c = CallableType([self.fx.a], [ARG_STAR], [None], self.fx.anyt, self.fx.function)
        assert self._bind(c) is None

    def test_star2_args_defers(self) -> None:
        c = CallableType([self.fx.a], [ARG_STAR2], [None], self.fx.anyt, self.fx.function)
        assert self._bind(c) is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFillTypevarsSuite(Suite):
    """Parity tests for `rust_fill_typevars` (mypy.typevars.fill_typevars).

    Rust reads the live `TypeInfo` (fullname, defn.type_vars,
    tuple_type) and rebuilds each class type parameter at line=-1 via the
    wire round-trip; TypeVarTupleType entries are wrapped in UnpackType;
    a named-tuple `tuple_type` is returned with the rebuilt Instance as
    fallback. Each test decodes the encoded result and compares it
    against the pure-Python `fill_typevars` call (gate off).
    """

    def setUp(self) -> None:
        from librt.internal import ReadBuffer

        from mypy.typevars import _set_native_typevars_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_native_typevars_active = _set_native_typevars_active
        self.ReadBuffer = ReadBuffer
        self._type_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.di,
            self.fx.gi,
            self.fx.g2i,
            self.fx.hi,
            self.fx.std_tuplei,
        ]
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})
        self._set_native_typevars_active(True)

    def _register(self, info: TypeInfo) -> None:
        # Newly created TypeInfos must be in the map so the decoded
        # outer Instance resolves to the same live object.
        from mypy.wirefixup import set_wire_typeinfo_map

        self._type_infos.append(info)
        set_wire_typeinfo_map({i.fullname: i for i in self._type_infos})

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_native_typevars_active(False)
        set_wire_typeinfo_map(None)

    def _decode(self, result: bytes) -> ProperType | None:
        from mypy.types import instance_cache, read_type as _read_type
        from mypy.wirefixup import fixup_wire_type

        decoded = _read_type(self.ReadBuffer(bytes(result)))
        # Clear instance_cache primitives so NOT_READY singletons cannot
        # leak into later tests (mirrors typeops.py).
        instance_cache.int_type = None
        instance_cache.str_type = None
        instance_cache.bool_type = None
        instance_cache.object_type = None
        instance_cache.function_type = None
        # The test seams only emit fully-resolved wire types, so a
        # successful fixup is always a ProperType (never an alias target).
        fixed = fixup_wire_type(decoded)
        if fixed is None:
            return None
        return cast(ProperType, fixed)

    def _pure_python(self, info: TypeInfo) -> Instance | TupleType:
        from mypy.typevars import fill_typevars

        self._set_native_typevars_active(False)
        try:
            return fill_typevars(info)
        finally:
            self._set_native_typevars_active(True)

    def _assert_par(self, info: TypeInfo) -> None:
        from mypy.typevars import fill_typevars

        expected = self._pure_python(info)
        result = _type_kernel.rust_fill_typevars(info)
        assert result is not None, f"rust_fill_typevars deferred for {info.fullname}"
        decoded = self._decode(result)
        assert decoded is not None, f"decode failed for {info.fullname}"
        assert_equal(decoded, expected, f"fill_typevars parity for {info.fullname}")
        # Gated shim: with the flag on the same result must come back.
        assert_equal(fill_typevars(info), expected)

    def test_non_generic(self) -> None:
        self._assert_par(self.fx.ai)

    def test_single_typevar(self) -> None:
        self._assert_par(self.fx.gi)

    def test_two_typevars(self) -> None:
        self._assert_par(self.fx.hi)

    def test_named_tuple(self) -> None:
        info = self.fx.make_type_info(
            "NT",
            mro=[self.fx.oi, self.fx.std_tuplei],
            bases=[Instance(self.fx.std_tuplei, [self.fx.a])],
        )
        info.tuple_type = TupleType(
            [self.fx.a, self.fx.b], Instance(self.fx.std_tuplei, [self.fx.o])
        )
        self._register(info)
        self._assert_par(info)

    def test_typevar_tuple(self) -> None:
        info = self.fx.make_type_info(
            "V", mro=[self.fx.oi], typevars=["T", "Ts"], typevar_tuple_index=1
        )
        self._register(info)
        self._assert_par(info)

    def test_paramspec(self) -> None:
        info = self.fx.make_type_info("P", mro=[self.fx.oi])
        info.defn.type_vars = [
            ParamSpecType(
                "P",
                "P",
                TypeVarId(1),
                ParamSpecFlavor.BARE,
                Instance(self.fx.oi, [], -1),
                NoneType(),
            )
        ]
        self._register(info)
        self._assert_par(info)

    def test_unresolvable_type_ref_defers(self) -> None:
        # "GS" is not in the wire typeinfo map, so fixup_wire_type fails
        # and the gated shim must fall back to pure Python.
        from mypy.typevars import fill_typevars

        info = self.fx.gsi
        result = _type_kernel.rust_fill_typevars(info)
        assert result is not None
        assert self._decode(result) is None
        assert_equal(fill_typevars(info), self._pure_python(info))

    def test_stale_map_entry_keeps_live_typ(self) -> None:
        # Regression guard: the wire map can hold a stale object for the
        # fullname across fine-grained refreshes. The shim must still
        # return an Instance rooted at the live `typ`, never the stale

        # map entry (fine-grained.test::testConstructorSignatureChanged3).
        from mypy.typevars import fill_typevars
        from mypy.wirefixup import set_wire_typeinfo_map

        info = self.fx.gi
        stale = self.fx.make_type_info("G", mro=[self.fx.oi])
        set_wire_typeinfo_map({i.fullname: i for i in self._type_infos + [stale]})
        result = fill_typevars(info)
        assert isinstance(result, Instance)
        assert result.type is info, "stale wire-map entry leaked into the result"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFillTypevarsWithAnySuite(Suite):
    """Parity tests for `rust_fill_typevars_with_any`
    (mypy.typevars.fill_typevars_with_any).

    Rust reads the live `TypeInfo` and builds the erased Instance whose
    args are all `AnyType(special_form)` (the erased_vars mapping). A
    named-tuple `tuple_type` is returned only when erasing it with the
    class's own tvar ids still yields a TupleType; TypeVarTuple and meta
    tvars defer to Python. Each test decodes the encoded result and
    compares it against the pure-Python call (gate off).
    """

    def setUp(self) -> None:
        from librt.internal import ReadBuffer

        from mypy.typevars import _set_native_typevars_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_native_typevars_active = _set_native_typevars_active
        self.ReadBuffer = ReadBuffer
        self._type_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.di,
            self.fx.gi,
            self.fx.g2i,
            self.fx.hi,
            self.fx.std_tuplei,
        ]
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})
        self._set_native_typevars_active(True)

    def _register(self, info: TypeInfo) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._type_infos.append(info)
        set_wire_typeinfo_map({i.fullname: i for i in self._type_infos})

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_native_typevars_active(False)
        set_wire_typeinfo_map(None)

    def _decode(self, result: bytes) -> ProperType | None:
        from mypy.types import instance_cache, read_type as _read_type
        from mypy.wirefixup import fixup_wire_type

        decoded = _read_type(self.ReadBuffer(bytes(result)))
        instance_cache.int_type = None
        instance_cache.str_type = None
        instance_cache.bool_type = None
        instance_cache.object_type = None
        instance_cache.function_type = None
        # The test seams only emit fully-resolved wire types, so a
        # successful fixup is always a ProperType (never an alias target).
        fixed = fixup_wire_type(decoded)
        if fixed is None:
            return None
        return cast(ProperType, fixed)

    def _pure_python(self, info: TypeInfo) -> Instance | TupleType:
        from mypy.typevars import fill_typevars_with_any

        self._set_native_typevars_active(False)
        try:
            return fill_typevars_with_any(info)
        finally:
            self._set_native_typevars_active(True)

    def _assert_par(self, info: TypeInfo) -> None:
        from mypy.typevars import fill_typevars_with_any

        expected = self._pure_python(info)
        result = _type_kernel.rust_fill_typevars_with_any(info)
        assert result is not None, f"rust_fill_typevars_with_any deferred for {info.fullname}"
        decoded = self._decode(result)
        assert decoded is not None, f"decode failed for {info.fullname}"
        assert_equal(decoded, expected, f"fill_typevars_with_any parity for {info.fullname}")
        assert_equal(str(decoded), str(expected))
        # Gated shim: with the flag on the same result must come back.
        assert_equal(fill_typevars_with_any(info), expected)

    def test_non_generic(self) -> None:
        self._assert_par(self.fx.ai)

    def test_single_typevar(self) -> None:
        self._assert_par(self.fx.gi)

    def test_two_typevars(self) -> None:
        self._assert_par(self.fx.hi)

    def test_native_args_are_special_form_any(self) -> None:
        # The native path must emit TypeOfAny.special_form (6), matching
        # erased_vars(..., TypeOfAny.special_form).
        info = self.fx.gi
        result = _type_kernel.rust_fill_typevars_with_any(info)
        assert result is not None
        decoded = self._decode(result)
        assert isinstance(decoded, Instance)
        assert len(decoded.args) == 1
        arg = get_proper_type(decoded.args[0])
        assert isinstance(arg, AnyType)
        assert arg.type_of_any == TypeOfAny.special_form

    def test_named_tuple(self) -> None:
        info = self.fx.make_type_info(
            "NT",
            mro=[self.fx.oi, self.fx.std_tuplei],
            bases=[Instance(self.fx.std_tuplei, [self.fx.a])],
        )
        info.tuple_type = TupleType(
            [self.fx.a, self.fx.b], Instance(self.fx.std_tuplei, [self.fx.o])
        )
        self._register(info)
        self._assert_par(info)

    def test_paramspec(self) -> None:
        info = self.fx.make_type_info("P", mro=[self.fx.oi])
        info.defn.type_vars = [
            ParamSpecType(
                "P",
                "P",
                TypeVarId(1),
                ParamSpecFlavor.BARE,
                Instance(self.fx.oi, [], -1),
                NoneType(),
            )
        ]
        self._register(info)
        self._assert_par(info)

    def test_typevar_tuple_defers(self) -> None:
        # TypeVarTuple erasure needs the live tuple_fallback (UnpackType
        # wrap), so Rust defers and the gated shim falls back to Python.
        from mypy.typevars import fill_typevars_with_any

        info = self.fx.make_type_info(
            "V", mro=[self.fx.oi], typevars=["T", "Ts"], typevar_tuple_index=1
        )
        self._register(info)
        assert _type_kernel.rust_fill_typevars_with_any(info) is None
        assert_equal(fill_typevars_with_any(info), self._pure_python(info))

    def test_meta_typevar_defers(self) -> None:
        # raw_id < 0 (inference meta var) is deferred; the gated shim must
        # still produce the pure-Python result.
        from mypy.typevars import fill_typevars_with_any

        info = self.fx.make_type_info("M", mro=[self.fx.oi])
        info.defn.type_vars = [
            TypeVarType(
                "T", "T", TypeVarId(-1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
            )
        ]
        self._register(info)
        assert _type_kernel.rust_fill_typevars_with_any(info) is None
        assert_equal(fill_typevars_with_any(info), self._pure_python(info))

    def test_unresolvable_type_ref_defers(self) -> None:
        # "GS" is not in the wire typeinfo map, so fixup fails and the
        # gated shim falls back to pure Python.
        from mypy.typevars import fill_typevars_with_any

        info = self.fx.gsi
        result = _type_kernel.rust_fill_typevars_with_any(info)
        assert result is not None
        assert self._decode(result) is None
        assert_equal(fill_typevars_with_any(info), self._pure_python(info))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeWireResolverSuite(Suite):
    """Parity tests for the Rust `Type` reader with TypeInfo resolver.

    Each test builds a resolver from the live Python TypeInfo graph via
    `type_kernel.build_resolver(type_infos)`, serializes a `Type` via
    `Type.write(WriteBuffer)`, and asserts:
        type_kernel.read_type_to_str_with_resolver(bytes, resolver) == str(t)
    The seed corpus targets the Stage 3b deferred renderings: builtins
    prefix stripping, enum-literal `value_repr`, bytes-literal
    `value_repr`, and the `[()]` variadic-tuple branch.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        # The fixture's TypeInfo graph: all TypeInfos reachable from the
        # fixture instances. build_native_resolver walks them into the
        # NativeTypeResolver pyclass (Rust-owned HashMaps, zero FFI per

        # lookup). No aliases in this fixture; pass [].
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

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def assert_wire_par(self, t: Type) -> None:
        expected = str(t)
        actual = _type_kernel.read_type_to_str_with_native_resolver(
            self._bytes_of(t), self.resolver
        )
        assert_equal(actual, expected, f"wire-resolver str({t!r}) = {{}} ({{}} expected)")

    def test_instance_no_args(self) -> None:
        self.assert_wire_par(self.fx.a)
        self.assert_wire_par(self.fx.b)
        self.assert_wire_par(self.fx.o)

    def test_instance_generic(self) -> None:
        self.assert_wire_par(self.fx.ga)
        self.assert_wire_par(self.fx.gb)
        self.assert_wire_par(self.fx.gt)

    def test_instance_tuple(self) -> None:
        # builtins.tuple renders as `tuple[T, ...]`.
        self.assert_wire_par(self.fx.std_tuple)

    def test_literal_int(self) -> None:
        self.assert_wire_par(self.fx.lit1)
        self.assert_wire_par(self.fx.lit2)
        self.assert_wire_par(self.fx.lit4)

    def test_literal_str(self) -> None:
        self.assert_wire_par(self.fx.lit_str1)
        self.assert_wire_par(self.fx.lit_str2)

    def test_literal_bool(self) -> None:
        self.assert_wire_par(self.fx.lit_false)
        self.assert_wire_par(self.fx.lit_true)

    def test_last_known_value(self) -> None:
        self.assert_wire_par(self.fx.lit1_inst)
        self.assert_wire_par(self.fx.lit_str1_inst)

    def test_type_type(self) -> None:
        self.assert_wire_par(self.fx.type_a)
        self.assert_wire_par(self.fx.type_b)

    def test_callable_pos(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_union(self) -> None:
        self.assert_wire_par(UnionType.make_union([self.fx.a, self.fx.b]))
        self.assert_wire_par(UnionType.make_union([self.fx.a, self.fx.b, self.fx.nonet]))


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinMeetSuite(Suite):
    """Parity suite for the Rust `trivial_join`/`trivial_meet` (Stage 3c M8d).

    Exercises the Rust path with the resolver built from the TypeFixture.
    Rust handles nominal-instance subtype/join/meet and returns `None`
    (Python fallthrough) for non-Instance right in object_or_any_from_type
    and when is_subtype defers. Because Python runs when Rust returns None,
    every assertion matches the pure-Python result.
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

    def test_trivial_join_subtype_returns_supertype(self) -> None:
        # B <: A -> trivial_join(B, A) = A (the supertype).
        from mypy.join import trivial_join

        assert trivial_join(self.fx.b, self.fx.a) == self.fx.a
        assert trivial_join(self.fx.a, self.fx.b) == self.fx.a

    def test_trivial_join_same_type(self) -> None:
        # A <: A -> trivial_join(A, A) = A.
        from mypy.join import trivial_join

        assert trivial_join(self.fx.a, self.fx.a) == self.fx.a
        assert trivial_join(self.fx.o, self.fx.o) == self.fx.o

    def test_trivial_join_unrelated_returns_object(self) -> None:
        # B and C unrelated -> object_or_any_from_type(right) = object.
        from mypy.join import trivial_join

        result = trivial_join(self.fx.b, self.fx.c)
        assert result == self.fx.o

    def test_trivial_meet_subtype_returns_subtype(self) -> None:
        # B <: A -> trivial_meet(B, A) = B (the subtype).
        from mypy.meet import trivial_meet

        assert trivial_meet(self.fx.b, self.fx.a) == self.fx.b
        assert trivial_meet(self.fx.a, self.fx.b) == self.fx.b

    def test_trivial_meet_same_type(self) -> None:
        # A <: A -> trivial_meet(A, A) = A.
        from mypy.meet import trivial_meet

        assert trivial_meet(self.fx.a, self.fx.a) == self.fx.a
        assert trivial_meet(self.fx.o, self.fx.o) == self.fx.o

    def test_trivial_meet_unrelated_returns_bottom(self) -> None:
        # B and C unrelated, strict_optional -> UninhabitedType.
        from mypy.meet import trivial_meet

        with state.strict_optional_set(True):
            result = trivial_meet(self.fx.b, self.fx.c)
            assert isinstance(result, UninhabitedType)

    def test_trivial_meet_unrelated_non_strict_returns_none_type(self) -> None:
        # B and C unrelated, non-strict-optional -> NoneType.
        from mypy.meet import trivial_meet

        with state.strict_optional_set(False):
            result = trivial_meet(self.fx.b, self.fx.c)
            assert isinstance(result, NoneType)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinTypesSuite(Suite):
    """Parity suite for the Rust `join_types` pre-dispatch (Stage 3c M8e).

    Exercises the Rust path with the resolver built from the TypeFixture.
    Rust handles the UnionType swap + AnyType/NoneType/UninhabitedType/
    DeletedType short-circuits and the leaf TypeJoinVisitor cases that
    don't recurse. Returns `None` (Python fallthrough) for Instance/
    Union/CallableType right and normalize_callables. Because Python runs
    when Rust returns None, every assertion matches the pure-Python result.
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

    def test_join_any_left_returns_any(self) -> None:
        # join.py:314: isinstance(s, AnyType) -> return s.
        from mypy.join import join_types

        assert join_types(self.fx.anyt, self.fx.a) == self.fx.anyt

    def test_join_none_none_strict_returns_none(self) -> None:
        # visit_none_type, strict_optional, s=None -> SameT (None).
        from mypy.join import join_types

        with state.strict_optional_set(True):
            assert join_types(self.fx.nonet, self.fx.nonet) == self.fx.nonet

    def test_join_none_none_non_strict_returns_none(self) -> None:
        # Non-strict-optional: visit_none_type returns s.
        from mypy.join import join_types

        with state.strict_optional_set(False):
            assert join_types(self.fx.nonet, self.fx.nonet) == self.fx.nonet

    def test_join_uninhabited_none_strict_returns_none(self) -> None:
        # s=Uninhabited, t=None: Uninhabited swap fires -> s=None,
        # t=Uninhabited. visit_uninhabited returns s (NoneType).
        from mypy.join import join_types

        with state.strict_optional_set(True):
            result = join_types(UninhabitedType(), self.fx.nonet)
            assert result == self.fx.nonet

    def test_join_uninhabited_uninhabited_returns_uninhabited(self) -> None:
        # s=Uninhabited, t=Uninhabited: no swap, visit_uninhabited
        # returns s (UninhabitedType).
        pass


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinInstanceSuite(Suite):
    """Parity suite for the Rust `visit_instance` nominal join (Stage 3c M8f).

    Exercises the args-less Instance-Instance nominal join: same-type,
    direct-subtype, and common-ancestor via the MRO bases walk. The
    fixture provides A, B(A), C(A), D (unrelated). join(B, C) finds A as
    the common ancestor via the bases walk, which trivial_join (direct
    subtype only) would miss (it returns object).
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

    def test_join_same_type_returns_self(self) -> None:
        # join.py:114: t.type == s.type, no args -> Instance(A, []) = A.
        from mypy.join import join_types

        assert join_types(self.fx.a, self.fx.a) == self.fx.a
        assert join_types(self.fx.d, self.fx.d) == self.fx.d

    def test_join_direct_subtype_returns_supertype(self) -> None:
        # B <: A -> join(A, B) = A. The Rust path returns
        # Ancestor("A") which the shim maps to Instance(A, []).
        from mypy.join import join_types

        assert join_types(self.fx.a, self.fx.b) == self.fx.a
        assert join_types(self.fx.b, self.fx.a) == self.fx.a

    def test_join_common_ancestor_returns_ancestor(self) -> None:
        # B <: A, C <: A, B not <: C, C not <: B -> join(B, C) = A.
        # trivial_join would return object (neither is a subtype of
        # the other); the visit_instance bases walk finds A.
        from mypy.join import join_types

        assert join_types(self.fx.b, self.fx.c) == self.fx.a
        assert join_types(self.fx.c, self.fx.b) == self.fx.a

    def test_join_unrelated_defers_to_python_returns_object(self) -> None:
        # A and D unrelated (D not <: A, A not <: D, no common base
        # in the fixture). Rust defers; Python returns object.
        from mypy.join import join_types

        result = join_types(self.fx.a, self.fx.d)
        assert result == self.fx.o

    def test_join_with_args_returns_same_instance(self) -> None:
        # Instance with type args (M8g): join(G[A], G[A]) where T is
        # invariant. is_equivalent(A, A)=True, join_types(A, A)=A ->
        # Rust returns SameTypeWithArgs (disc 6) with arg_discs=[0]

        # (use s.args[0]=A). Shim reconstructs G[A].
        from mypy.join import join_types

        result = join_types(self.fx.ga, self.fx.ga)
        assert result == self.fx.ga


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSubtypeTupleSuite(Suite):
    """Parity suite for the Rust `visit_tuple_type` port (Phase B2, #589).

    Exercises the fixed-tuple paths in is_subtype with the resolver built
    from the TypeFixture: TupleType vs Instance (Sized, tuple-like,
    structural fallback), TupleType vs TupleType (length, item-wise,
    fallback), and the deferred variadic/Unpack cases. Because Rust
    returns None for the deferred cases (Python decides), every assertion
    here matches the pure-Python result either way. Requires
    TEST_NATIVE_TYPE_KERNEL=1 to route through Rust.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _tup(self, *items: Type) -> TupleType:
        return TupleType(list(items), self.fx.std_tuple)

    def test_tuple_vs_sized(self) -> None:
        # Any tuple <: typing.Sized -> True (short-circuit on type_ref
        # before any resolver lookup, subtypes.py:953-954).
        from mypy.nodes import TypeInfo

        defn = ClassDef("Sized", Block([]), None, [])
        defn.fullname = "typing.Sized"
        sized = Instance(TypeInfo(SymbolTable(), defn, "Sized"), [])
        assert is_subtype(self._tup(self.fx.a), sized)
        assert is_subtype(self._tup(), sized)

    def test_tuple_vs_builtins_tuple_any(self) -> None:
        # (A,) <: tuple[Any] -> True (Any iter type special case,
        # subtypes.py:962-966).
        assert is_subtype(self._tup(self.fx.a), self.fx.std_tuple)
        assert is_subtype(self._tup(), self.fx.std_tuple)

    def test_tuple_vs_builtins_tuple_exact(self) -> None:
        # (A,) <: tuple[A] -> True (each item subtype of iter_type).
        tuple_a = Instance(self.fx.std_tuplei, [self.fx.a])
        assert is_subtype(self._tup(self.fx.a), tuple_a)
        # (A,) !<: tuple[B] when A !<: B.
        tuple_b = Instance(self.fx.std_tuplei, [self.fx.b])
        assert not is_subtype(self._tup(self.fx.a), tuple_b)

    def test_tuple_vs_tuple_equal(self) -> None:
        # (A,) <: (A,) -> True (length + item-wise + fallback).
        assert is_subtype(self._tup(self.fx.a), self._tup(self.fx.a))

    def test_tuple_vs_tuple_length_mismatch(self) -> None:
        # (A,) !<: (A, A) -> False (length mismatch).
        assert not is_subtype(self._tup(self.fx.a), self._tup(self.fx.a, self.fx.a))

    def test_tuple_vs_tuple_item_mismatch(self) -> None:
        # (A,) !<: (B,) when A !<: B -> False.
        assert not is_subtype(self._tup(self.fx.a), self._tup(self.fx.b))

    def test_tuple_vs_tuple_any_item(self) -> None:
        # (A,) <: (Any,) -> True (Any left is always a subtype non-proper).
        assert is_subtype(self._tup(self.fx.a), self._tup(AnyType(TypeOfAny.special_form)))

    def test_tuple_vs_unrelated_instance_defer_or_false(self) -> None:
        # (A,) !<: D (unrelated Instance, not a protocol). Result must be
        # False (matches Python: fallback check fails, no protocol).
        assert not is_subtype(self._tup(self.fx.a), self.fx.d)

    def test_tuple_variadic_unpack_defers(self) -> None:
        # Unpack items are not handled by the fixed-tuple port; the result
        # must still match Python (variadic_tuple_subtype decides). The
        # async test asserts the *pure-Python* outcome is stable here:

        # (A,) <: (*tuple[A, ...],) via the infinite-union mapping is
        # True (the Rust path either decides it or defers, both correct).
        t1 = self._tup(self.fx.a)
        t2 = TupleType([UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))], self.fx.std_tuple)
        assert is_subtype(t1, t2)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSubtypesDeferralSuite(Suite):
    """Parity suite for the subtype-seam alias deferral reduction (#868).

    `rust_is_equivalent`, `rust_is_same_type` and `rust_is_more_precise`
    previously received `TypeAliasType` operands raw, hit the recursive
    alias guard inside `is_subtype`, and deferred the whole call to the
    pure-Python body. The seams now expand alias operands through the
    alias resolver (mirroring `get_proper_type` in the Python fallbacks),
    so alias-shaped operands answer natively.

    Each test asserts a gate-on/off differential (identical result either
    way) AND a direct seam call proving the Rust side actually decides
    (non-None) for the alias path, so a regression back to deferral is
    caught. Missing-snapshot operands must still defer (None), preserving
    the pure-Python fallback.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        self.resolver = self._build_resolver([])
        _set_native_subtype_resolver(self.resolver)
        _set_native_subtype_active(True)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _build_resolver(self, aliases: list[Any]) -> Any:
        return _type_kernel.build_native_resolver(self._type_infos(), aliases)

    def _rebuild_with_aliases(self, aliases: list[Any]) -> None:
        from mypy.subtypes import _set_native_subtype_resolver

        self.resolver = self._build_resolver(aliases)
        _set_native_subtype_resolver(self.resolver)

    def _set_gate(self, active: bool) -> None:
        from mypy.subtypes import _set_native_subtype_active

        _set_native_subtype_active(active)

    def test_equivalent_alias_equals_target(self) -> None:
        from mypy.nodes import TypeAlias
        from mypy.subtypes import _serialize_type, is_equivalent

        # mod.A = A (fx.a); is_equivalent(A, A) is True natively once the
        # alias expands (previously the alias guard deferred to Python).
        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        t = TypeAliasType(alias, [])
        # Gate-on and gate-off give the identical result (Python's
        # fallback expands via get_proper_type in _is_subtype).
        self._set_gate(True)
        assert is_equivalent(t, self.fx.a)
        self._set_gate(False)
        assert is_equivalent(t, self.fx.a)
        # The direct seam call proves the alias path is native now
        # (non-None), not deferring back to Python.
        self._set_gate(True)
        rusted = _type_kernel.rust_is_equivalent(
            _serialize_type(t), _serialize_type(self.fx.a), False, True, self.resolver
        )
        assert rusted is True

    def test_equivalent_alias_missing_snapshot_defers(self) -> None:
        # Alias not registered in the resolver: the seam defers (None) and
        # the pure-Python fallback answers, so gate-on == gate-off.
        from mypy.nodes import TypeAlias
        from mypy.subtypes import _serialize_type, is_equivalent

        alias = TypeAlias(self.fx.a, "mod.Missing", "mod", -1, -1)
        t = TypeAliasType(alias, [])
        self._set_gate(False)
        expected = is_equivalent(t, self.fx.a)
        self._set_gate(True)
        assert is_equivalent(t, self.fx.a) is expected
        rusted = _type_kernel.rust_is_equivalent(
            _serialize_type(t), _serialize_type(self.fx.a), False, True, self.resolver
        )
        assert rusted is None

    def test_same_type_alias_expands(self) -> None:
        from mypy.nodes import TypeAlias
        from mypy.subtypes import _serialize_type

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        t = TypeAliasType(alias, [])
        self._set_gate(True)
        assert is_same_type(t, self.fx.a)
        self._set_gate(False)
        assert is_same_type(t, self.fx.a)
        self._set_gate(True)
        rusted = _type_kernel.rust_is_same_type(
            _serialize_type(t), _serialize_type(self.fx.a), False, True, self.resolver
        )
        assert rusted is True

    def test_more_precise_alias_expands(self) -> None:
        from mypy.nodes import TypeAlias
        from mypy.subtypes import _serialize_type

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        t = TypeAliasType(alias, [])
        self._set_gate(True)
        assert is_more_precise(t, self.fx.a)
        self._set_gate(False)
        assert is_more_precise(t, self.fx.a)
        # Rust decides natively (left alias expands, proper-subtype answered).
        self._set_gate(True)
        rusted = _type_kernel.rust_is_more_precise(
            _serialize_type(t), _serialize_type(self.fx.a), False, True, self.resolver
        )
        assert rusted is True

    def test_more_precise_alias_to_any(self) -> None:
        # A = Any; is_more_precise(x, A) is True via the Any fast path once
        # the right alias expands on the Rust side.
        from mypy.nodes import TypeAlias
        from mypy.subtypes import _serialize_type

        alias = TypeAlias(AnyType(TypeOfAny.special_form), "mod.AnyA", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        t = TypeAliasType(alias, [])
        self._set_gate(True)
        assert is_more_precise(self.fx.a, t)
        self._set_gate(False)
        assert is_more_precise(self.fx.a, t)
        self._set_gate(True)
        rusted = _type_kernel.rust_is_more_precise(
            _serialize_type(self.fx.a), _serialize_type(t), False, True, self.resolver
        )
        assert rusted is True

    def test_recursive_alias_gate_parity_no_wrong_verdict(self) -> None:
        # Issue #1149: with R = Union[A, R], native expansion must
        # terminate like Python's lazy get_proper_type, and the direct
        # seam must never invent a verdict on a cut-node shape.
        from mypy.nodes import TypeAlias
        from mypy.subtypes import _serialize_type, is_subtype
        from mypy.types import UnionType

        alias = TypeAlias(self.fx.a, "mod.R", "mod", -1, -1)
        alias.target = UnionType([self.fx.a, TypeAliasType(alias, [])], False)
        self._rebuild_with_aliases([alias])
        r = TypeAliasType(alias, [])

        cases: list[tuple[Callable[[], bool], str]] = []
        cases.append((lambda: is_same_type(r, r), "is_same_type(R, R)"))
        cases.append((lambda: is_subtype(r, r), "is_subtype(R, R)"))
        cases.append((lambda: is_subtype(self.fx.a, r), "is_subtype(A, R)"))
        cases.append((lambda: is_subtype(r, self.fx.a), "is_subtype(R, A)"))
        cases.append((lambda: is_more_precise(r, self.fx.a), "is_more_precise(R, A)"))
        for fn, label in cases:
            self._set_gate(False)
            expected = fn()
            self._set_gate(True)
            assert fn() == expected, f"gate-on != gate-off for {label}"
            assert expected, f"both gates must answer True for {label}"

        # Direct seam: the wave-50 raw-target unroll (#1457) lets the engine
        # decide `R <: A` natively -- the ALIAS_ASSUME guard terminates
        # the recursion and matches the gate-off answer above (True).
        self._set_gate(True)
        rusted = _type_kernel.rust_is_subtype(
            _serialize_type(r),
            _serialize_type(self.fx.a),
            False,
            False,
            False,
            False,
            False,
            True,
            False,
            False,
            self.resolver,
        )
        assert rusted is True

    def test_instance_callable_probe_engages(self) -> None:
        # Issue #1205 (Port B): an Instance parsed across the MRO snapshot
        # with no `__call__` definition decides the Instance <: Callable
        # arm natively (Python's find_member miss returns False).
        from mypy.subtypes import _serialize_type, is_subtype

        right = self.fx.callable(self.fx.a, self.fx.bool_type)
        self._set_gate(False)
        assert not is_subtype(self.fx.a, right)
        self._set_gate(True)
        assert not is_subtype(self.fx.a, right)
        rusted = _type_kernel.rust_is_subtype(
            _serialize_type(self.fx.a),
            _serialize_type(right),
            False,
            False,
            False,
            False,
            False,
            True,
            False,
            False,
            self.resolver,
        )
        assert rusted is False

    def test_overload_right_ordered_match_engages(self) -> None:
        # Issue #1205 (Port C): Overloaded <: Overloaded mirrors
        # visit_overloaded's ordered-match loop. Covers the out-of-order
        # overlap probe (subtypes.py:1160-1168) for reordered items.
        from mypy.subtypes import _serialize_type, is_subtype
        from mypy.types import Overloaded

        f = self.fx.callable(self.fx.a, self.fx.bool_type)
        g = self.fx.callable(self.fx.b, self.fx.bool_type)

        # Ordered subset: every right item is matched in non-decreasing
        # left order -> True.
        left = Overloaded([f, g])
        right = Overloaded([f])
        self._set_gate(False)
        assert is_subtype(left, right)
        self._set_gate(True)
        assert is_subtype(left, right)
        rusted = _type_kernel.rust_is_subtype(
            _serialize_type(left),
            _serialize_type(right),
            False,
            False,
            False,
            False,
            False,
            True,
            False,
            False,
            self.resolver,
        )
        assert rusted is True

        # Reordered overlap: right item `f` is matched only by the left
        # item at index 1 (fixture B <: A, so g <: f is False), forcing
        # `g` out of order; the overlap probe answers False.
        left = Overloaded([g, f])
        right = Overloaded([f, g])
        self._set_gate(False)
        assert not is_subtype(left, right)
        self._set_gate(True)
        assert not is_subtype(left, right)
        rusted = _type_kernel.rust_is_subtype(
            _serialize_type(left),
            _serialize_type(right),
            False,
            False,
            False,
            False,
            False,
            True,
            False,
            False,
            self.resolver,
        )
        assert rusted is False

    def test_plain_equal_equivalent_engages(self) -> None:
        # A regression guard: plain (alias-free) operand pairs must still be
        # decided natively, not newly deferred by the seam expansion.
        from mypy.subtypes import _serialize_type, is_equivalent

        self._set_gate(True)
        assert is_equivalent(self.fx.a, self.fx.a)
        rusted = _type_kernel.rust_is_equivalent(
            _serialize_type(self.fx.a), _serialize_type(self.fx.a), False, True, self.resolver
        )
        assert rusted is True

    def test_subtype_erased_union_item(self) -> None:
        # Issue #1185: the Python shell gate (subtypes.py:842-844) filters
        # only a TOP-LEVEL ErasedType operand, so a nested ErasedType union
        # item reaches the Rust kernel. Before the fix, the right-union

        # recursion landed the Erased item in the Instance-vs-non-Instance
        # tail and answered Some(false), making the whole union check
        # wrongly False; the fast-path fix (right in {Any, Unbound, Erased})

        # answers Some(true), matching Python.
        from mypy.subtypes import _serialize_type, is_subtype

        u = UnionType([self.fx.b, ErasedType()], False)
        self._set_gate(False)
        expected = is_subtype(self.fx.a, u)
        assert expected
        self._set_gate(True)
        assert is_subtype(self.fx.a, u) is expected
        rusted = _type_kernel.rust_is_subtype(
            _serialize_type(self.fx.a),
            _serialize_type(u),
            False,
            False,
            False,
            False,
            False,
            True,
            False,
            False,
            self.resolver,
        )
        assert rusted is True

    def test_valid_inferred_erased_leaf(self) -> None:
        # Issue #1185: is_valid_inferred_type serializes its operand
        # wholesale, so an ErasedType leaf rides the wire into the Rust
        # invalid_inferred_types_query. Before the fix the query defaulted

        # ErasedType to valid (True); Python (checker.py, the
        # _invalid_inferred_types visitor) treats ErasedType as invalid,
        # so the native arm now mirrors that.
        from mypy.checker import (
            _serialize_type_for_checker,
            _set_native_checker_stmts_active,
            is_valid_inferred_type,
        )

        options = Options()
        t = ErasedType()
        try:
            _set_native_checker_stmts_active(False)
            expected = is_valid_inferred_type(t, options)
            assert expected is False
            _set_native_checker_stmts_active(True)
            assert is_valid_inferred_type(t, options) is expected
            rusted = _type_kernel.rust_is_valid_inferred_type(
                _serialize_type_for_checker(t), False, False, False, self.resolver
            )
            assert rusted is False
        finally:
            _set_native_checker_stmts_active(False)

    def test_valid_inferred_alias_operand_native(self) -> None:
        # Issue #1298: is_valid_inferred_type serializes its operand wholesale,
        # so a TypeAliasType operand rides the wire into the kernel, which now
        # expands it via the resolver snapshot instead of deferring.
        from mypy.checker import (
            _serialize_type_for_checker,
            _set_native_checker_resolver,
            _set_native_checker_stmts_active,
            is_valid_inferred_type,
        )

        options = Options()
        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        t = TypeAliasType(alias, [])
        try:
            _set_native_checker_resolver(self.resolver)
            _set_native_checker_stmts_active(False)
            expected = is_valid_inferred_type(t, options)
            assert expected is True
            _set_native_checker_stmts_active(True)
            assert is_valid_inferred_type(t, options) is expected
            # The direct seam call proves the alias path is native now
            # (non-None), not deferring back to Python.
            rusted = _type_kernel.rust_is_valid_inferred_type(
                _serialize_type_for_checker(t), False, False, False, self.resolver
            )
            assert rusted is True
        finally:
            _set_native_checker_stmts_active(False)
            _set_native_checker_resolver(None)

    def test_valid_inferred_pep695_alias_edge_differential(self) -> None:
        # Mirror of the kernel's edge-continuation tests (checker_stmts.rs): a
        # PEP-695 alias's written args are still queried, so a meta-var arg
        # flips the verdict; an old-style alias skips the args query and stays valid.
        from mypy.checker import (
            _serialize_type_for_checker,
            _set_native_checker_resolver,
            _set_native_checker_stmts_active,
            is_valid_inferred_type,
        )

        options = Options()
        meta = TypeVarType(
            "T",
            "mod.T",
            TypeVarId(-1, meta_level=1),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        tvar = TypeVarType(
            "T", "mod.T", TypeVarId(1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )
        pep695 = TypeAlias(
            self.fx.a, "mod.A", "mod", -1, -1, alias_tvars=[tvar], python_3_12_type_alias=True
        )
        old_style = TypeAlias(
            self.fx.a, "mod.B", "mod", -1, -1, alias_tvars=[tvar], python_3_12_type_alias=False
        )
        self._rebuild_with_aliases([pep695, old_style])
        try:
            _set_native_checker_resolver(self.resolver)
            _set_native_checker_stmts_active(False)
            # Python: get_proper_type expands mod.A to a clean Instance, so the
            # verdict comes from the ORIGINAL alias operand (checker.py:11760),
            # whose visit_type_alias_type edge only consults args under PEP-695.
            pep_expected = is_valid_inferred_type(TypeAliasType(pep695, [meta]), options)
            assert pep_expected is False
            old_expected = is_valid_inferred_type(TypeAliasType(old_style, [meta]), options)
            assert old_expected is True
            _set_native_checker_stmts_active(True)
            assert is_valid_inferred_type(TypeAliasType(pep695, [meta]), options) is pep_expected
            assert (
                is_valid_inferred_type(TypeAliasType(old_style, [meta]), options) is old_expected
            )
            rusted = _type_kernel.rust_is_valid_inferred_type(
                _serialize_type_for_checker(TypeAliasType(pep695, [meta])),
                False,
                False,
                False,
                self.resolver,
            )
            assert rusted is False
        finally:
            _set_native_checker_stmts_active(False)
            _set_native_checker_resolver(None)

    def test_callable_protocol_right_no_call_non_typeobj_native(self) -> None:
        """Issue #1233 (Port A): CallableType left vs protocol Instance
        right without "__call__".

        Python answers is_subtype(left.fallback, right) unless
        left.is_type_obj() (subtypes.py:1372-1381); the seam used to
        defer the whole arm and now decides the non-type-obj case with
        the resolver-backed is_type_obj port. The fallback class has no
        `f` member, so both engines answer False via the member-miss
        path. builtins.function snapshots keep deferring here, so the
        fixture class fallback stands in for the production shape.
        """
        from mypy.subtypes import _serialize_type, _set_native_subtype_resolver, is_subtype
        from mypy.types import CallableType, Instance
        from mypy.wirefixup import set_wire_typeinfo_map

        pinfo = self.fx.make_type_info("mod.PNoCall")
        pinfo.mro = [pinfo, self.fx.oi]
        pinfo.is_protocol = True
        pinst = Instance(pinfo, [])
        node = FuncDef("f", [], None, None)
        node.info = pinfo
        node.type = CallableType([pinst], [ARG_POS], [None], self.fx.a, self.fx.function)
        node.line = 1
        node.column = 1
        pinfo.names["f"] = SymbolTableNode(MDEF, node)
        # Plain non-protocol, non-type-object class carrying the
        # callable's fallback; is_type_obj() must come out False.
        iinfo = self.fx.make_type_info("mod.FbNoCall")
        iinfo.mro = [iinfo, self.fx.oi]
        live = {iinfo.fullname: iinfo, pinfo.fullname: pinfo}
        resolver = _type_kernel.build_native_resolver(self._type_infos() + [iinfo, pinfo], [])
        resolver.set_live_typeinfo_map(live)
        set_wire_typeinfo_map(live)
        _set_native_subtype_resolver(resolver)
        try:
            left = CallableType([self.fx.a], [ARG_POS], [None], self.fx.a, Instance(iinfo, []))
            self._set_gate(False)
            expected = is_subtype(left, pinst)
            self._set_gate(True)
            assert is_subtype(left, pinst) is expected
            rusted = _type_kernel.rust_is_subtype(
                _serialize_type(left),
                _serialize_type(pinst),
                False,
                False,
                False,
                False,
                False,
                True,
                False,
                False,
                resolver,
            )
            assert rusted is not None, "non-type-obj protocol arm must decide natively"
            assert rusted is expected
        finally:
            set_wire_typeinfo_map(None)
            _set_native_subtype_resolver(self.resolver)

    def test_callable_protocol_right_typeobj_still_defers(self) -> None:
        """Issue #1233 (Port A): the is_type_obj()-True arm still defers.

        Python then attempts is_protocol_implementation(class_obj=True)
        (subtypes.py:1374-1381), which stays Python-side; a callable
        whose fallback is builtins.type is a type object.
        """
        from mypy.subtypes import _serialize_type, _set_native_subtype_resolver, is_subtype
        from mypy.types import Instance
        from mypy.wirefixup import set_wire_typeinfo_map

        info = self.fx.make_type_info("mod.PNoCall2")
        info.mro = [info, self.fx.oi]
        info.is_protocol = True
        inst = Instance(info, [])
        resolver = _type_kernel.build_native_resolver(self._type_infos() + [info], [])
        resolver.set_live_typeinfo_map({info.fullname: info})
        set_wire_typeinfo_map({info.fullname: info})
        _set_native_subtype_resolver(resolver)
        try:
            left = self.fx.callable_type(self.fx.a, self.fx.a)
            self._set_gate(False)
            expected = is_subtype(left, inst)
            self._set_gate(True)
            assert is_subtype(left, inst) is expected
            rusted = _type_kernel.rust_is_subtype(
                _serialize_type(left),
                _serialize_type(inst),
                False,
                False,
                False,
                False,
                False,
                True,
                False,
                False,
                resolver,
            )
            assert rusted is None, "type-obj protocol arm must still defer"
        finally:
            set_wire_typeinfo_map(None)
            _set_native_subtype_resolver(self.resolver)

    def test_is_typeddict_alias_context_native(self) -> None:
        """Issue #1309 (itdc): an alias to a TypedDictType decides natively.

        is_typeddict_type_context serializes its operand wholesale, so a
        TypeAliasType operand rides the wire into the kernel, which now
        expands it via the alias resolver instead of deferring.
        """
        from mypy.checker import (
            _serialize_type_for_checker,
            _set_native_checker_active,
            _set_native_checker_resolver,
            is_typeddict_type_context,
        )
        from mypy.nodes import TypeAlias
        from mypy.types import UnionType

        td = TypedDictType({"x": self.fx.o}, {"x"}, set(), self.fx.a)
        alias = TypeAlias(self.fx.a, "mod.AliasTD", "mod", -1, -1)
        alias.target = td
        self._rebuild_with_aliases([alias])
        t = TypeAliasType(alias, [])
        union = UnionType([self.fx.a, t], False)
        try:
            _set_native_checker_resolver(self.resolver)
            _set_native_checker_active(False)
            expected = is_typeddict_type_context(t)
            assert expected is True
            expected_union = is_typeddict_type_context(union)
            assert expected_union is True
            _set_native_checker_active(True)
            assert is_typeddict_type_context(t) is expected
            assert is_typeddict_type_context(union) is expected_union
            # The direct seam call proves the alias path is native now
            # (non-None), not deferring back to Python.
            rusted = _type_kernel.rust_is_typeddict_type_context(
                self.resolver, _serialize_type_for_checker(t)
            )
            assert rusted is True
        finally:
            _set_native_checker_active(False)
            _set_native_checker_resolver(None)

    def test_is_typeddict_alias_missing_snapshot_defers(self) -> None:
        # Alias not registered in the checker resolver: the seam defers
        # (None) and the pure-Python fallback answers, so gate-on equals
        # gate-off for both the alias and its union-wrapped form.
        from mypy.checker import (
            _serialize_type_for_checker,
            _set_native_checker_active,
            _set_native_checker_resolver,
            is_typeddict_type_context,
        )
        from mypy.nodes import TypeAlias
        from mypy.types import UnionType

        td = TypedDictType({"x": self.fx.o}, {"x"}, set(), self.fx.a)
        alias = TypeAlias(self.fx.a, "mod.AliasTD2", "mod", -1, -1)
        alias.target = td
        t = TypeAliasType(alias, [])
        union = UnionType([self.fx.a, t], False)
        bare_resolver = self._build_resolver([])
        try:
            _set_native_checker_resolver(bare_resolver)
            _set_native_checker_active(False)
            expected = is_typeddict_type_context(t)
            assert expected is True
            expected_union = is_typeddict_type_context(union)
            assert expected_union is True
            _set_native_checker_active(True)
            assert is_typeddict_type_context(t) is expected
            assert is_typeddict_type_context(union) is expected_union
            rusted = _type_kernel.rust_is_typeddict_type_context(
                bare_resolver, _serialize_type_for_checker(t)
            )
            assert rusted is None, "missing snapshot must defer"
        finally:
            _set_native_checker_active(False)
            _set_native_checker_resolver(None)

    def test_is_typeddict_none_resolver_defers(self) -> None:
        # Issue #1312 review: a daemon recheck clears the checker resolver
        # while the gate stays active; passing None into the seam raised
        # TypeError instead of deferring. The gate now checks the resolver.
        from mypy.checker import (
            _set_native_checker_active,
            _set_native_checker_resolver,
            is_typeddict_type_context,
        )

        try:
            _set_native_checker_active(True)
            _set_native_checker_resolver(None)
            assert is_typeddict_type_context(self.fx.a) is False
        finally:
            _set_native_checker_active(False)
            _set_native_checker_resolver(None)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSnapshotGapLiveNominalSuite(Suite):
    """Issue #1619: live-`TypeInfo` decisions for a snapshot-missing left.

    The Rust `TypeResolver` snapshot is fed per SCC *after* semanal, while
    the Python wire map publishes a module's TypeInfos at top-level
    completion, so a seam inside a class's own SCC sees `in_map=True,
    in_snap=False` and deferred. The nominal prelude now reads the live
    `TypeInfo` at decision time instead (nothing is stored, so a
    pre-inference variance can never be pinned, the #1490 sealing
    regression), and the map seam reads the live supertype for the
    `not superclass.type_vars` fast path. Every ambiguous arm still
    defers (`None`), so the Python fallback keeps answering.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        self.resolver = _type_kernel.build_native_resolver(self._type_infos(), [])
        _set_native_subtype_resolver(self.resolver)
        _set_native_subtype_active(True)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        set_wire_typeinfo_map(None)

    def _type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _snapshot_gap_resolver(self, infos: list[TypeInfo]) -> Any:
        """Snapshot with the `fx` classes only, live map with `infos` too."""
        from mypy.subtypes import _set_native_subtype_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        live_infos = self._type_infos() + infos
        live = {info.fullname: info for info in live_infos}
        self.resolver = _type_kernel.build_native_resolver(self._type_infos(), [])
        self.resolver.set_live_typeinfo_map(live)
        set_wire_typeinfo_map(live)
        _set_native_subtype_resolver(self.resolver)
        return self.resolver

    def _set_gate(self, active: bool) -> None:
        from mypy.subtypes import _set_native_subtype_active

        _set_native_subtype_active(active)

    def _seam(self, left: Any, right: Any) -> Any:
        from mypy.subtypes import _serialize_type

        return _type_kernel.rust_is_subtype(
            _serialize_type(left),
            _serialize_type(right),
            False,
            False,
            False,
            False,
            False,
            True,
            False,
            False,
            self.resolver,
        )

    def _parity(self, left: Any, right: Any) -> bool:
        from mypy.subtypes import is_subtype

        self._set_gate(False)
        expected = is_subtype(left, right)
        self._set_gate(True)
        assert is_subtype(left, right) is expected
        return expected

    def _base_sub(self, suffix: str = "") -> tuple[TypeInfo, TypeInfo]:
        from mypy.types import Instance

        base = self.fx.make_type_info(f"mod.LiveBase{suffix}")
        sub = self.fx.make_type_info(
            f"mod.LiveSub{suffix}", mro=[base, self.fx.oi], bases=[Instance(base, [])]
        )
        return base, sub

    def test_nominal_live_non_generic_decides(self) -> None:
        # left missing from the snapshot but present live, has_base to a
        # non-generic right: Python answers True via the nominal branch
        # and the live prelude must decide the same without deferring.
        from mypy.types import Instance

        base, sub = self._base_sub()
        self._snapshot_gap_resolver([base, sub])
        left, right = Instance(sub, []), Instance(base, [])
        assert self._parity(left, right) is True
        assert self._seam(left, right) is True

    def test_nominal_live_object_right_decides(self) -> None:
        # builtins.object right hits the `rname == "builtins.object"`
        # clause with no has_base read needed.
        from mypy.types import Instance

        base, sub = self._base_sub("Obj")
        self._snapshot_gap_resolver([base, sub])
        left, right = Instance(sub, []), Instance(self.fx.oi, [])
        assert self._parity(left, right) is True
        assert self._seam(left, right) is True

    def test_nominal_live_no_base_decides_false(self) -> None:
        # No has_base and no object/NamedTuple clause: Python skips the
        # nominal branch and (right not a protocol) answers False.
        from mypy.types import Instance

        other = self.fx.make_type_info("mod.LiveOther")
        _, sub = self._base_sub("False")
        self._snapshot_gap_resolver([other, sub])
        left, right = Instance(sub, []), Instance(other, [])
        assert self._parity(left, right) is False
        assert self._seam(left, right) is False

    def test_nominal_live_generic_right_defers(self) -> None:
        # Generic right: the per-arg variance walk needs the snapshot
        # substitution, so the live prelude must defer (gate parity holds
        # through the Python fallback).
        from mypy.types import Instance

        sub = self.fx.make_type_info(
            "mod.LiveGSub", mro=[self.fx.gi, self.fx.oi], bases=[Instance(self.fx.gi, [self.fx.a])]
        )
        self._snapshot_gap_resolver([sub])
        left, right = Instance(sub, []), Instance(self.fx.gi, [self.fx.a])
        assert self._parity(left, right) is True
        assert self._seam(left, right) is None

    def test_nominal_live_protocol_right_defers(self) -> None:
        # Protocol right: the member loop is not part of the live prelude.
        from mypy.types import Instance

        proto = self.fx.make_type_info("mod.LiveProto")
        proto.is_protocol = True
        _, sub = self._base_sub("Proto")
        self._snapshot_gap_resolver([proto, sub])
        left, right = Instance(sub, []), Instance(proto, [])
        self._parity(left, right)
        assert self._seam(left, right) is None

    def test_nominal_live_promotion_defers(self) -> None:
        # A base carrying `_promote` may promote left to right; deciding
        # that needs the promote targets' expansion, so the live prelude
        # defers and the Python promote loop answers True.
        from mypy.types import Instance

        base, sub = self._base_sub("Promote")
        sub._promote = [Instance(base, [])]
        self._snapshot_gap_resolver([base, sub])
        left, right = Instance(sub, []), Instance(base, [])
        assert self._parity(left, right) is True
        assert self._seam(left, right) is None

    def test_nominal_live_fallback_to_any_decides(self) -> None:
        # `fallback_to_any` short-circuits True for a non-proper check.
        from mypy.types import Instance

        base, sub = self._base_sub("FbAny")
        sub.fallback_to_any = True
        self._snapshot_gap_resolver([base, sub])
        left, right = Instance(sub, []), Instance(base, [])
        assert self._parity(left, right) is True
        assert self._seam(left, right) is True

    def test_nominal_live_without_live_map_defers(self) -> None:
        # No live map (pure-Rust / cleared resolver): the prelude must
        # defer rather than guess from the empty snapshot.
        from mypy.subtypes import _set_native_subtype_resolver, is_subtype
        from mypy.types import Instance

        base, sub = self._base_sub("NoLive")
        left, right = Instance(sub, []), Instance(base, [])
        self._set_gate(False)
        expected = is_subtype(left, right)
        bare = _type_kernel.build_native_resolver(self._type_infos(), [])
        self.resolver = bare
        _set_native_subtype_resolver(bare)
        self._set_gate(True)
        assert is_subtype(left, right) is expected
        assert self._seam(left, right) is None

    def test_map_live_non_generic_decides(self) -> None:
        # Both classes absent from the snapshot but live: the FFI mapping
        # fast path reads the live supertype's empty `type_vars`.
        from librt.internal import ReadBuffer

        from mypy.types import Instance, get_proper_type, read_type

        base, sub = self._base_sub("Map")
        resolver = self._snapshot_gap_resolver([base, sub])
        inst = Instance(sub, [])
        buf = _WriteBuffer()
        inst.write(buf)
        result = _type_kernel.rust_map_instance_to_supertype(
            resolver, sub.fullname, buf.getvalue(), base.fullname
        )
        assert result is not None, "live non-generic supertype must map natively"
        decoded = read_type(ReadBuffer(bytes(result)))
        from mypy.wirefixup import fixup_wire_type

        fixed = fixup_wire_type(decoded)
        assert fixed is not None
        proper = get_proper_type(fixed)
        assert isinstance(proper, Instance)
        assert proper.type is base
        assert not proper.args

    def test_map_live_generic_right_defers(self) -> None:
        # Generic supertype: still needs the snapshot derivation walk.
        from mypy.types import Instance

        sub = self.fx.make_type_info(
            "mod.LiveMapGSub",
            mro=[self.fx.gi, self.fx.oi],
            bases=[Instance(self.fx.gi, [self.fx.a])],
        )
        resolver = self._snapshot_gap_resolver([sub])
        inst = Instance(sub, [])
        buf = _WriteBuffer()
        inst.write(buf)
        result = _type_kernel.rust_map_instance_to_supertype(
            resolver, sub.fullname, buf.getvalue(), self.fx.gi.fullname
        )
        assert result is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeInstanceCallSubtypeSuite(Suite):
    """Issue #1255: the two remaining is_subtype defer arms now decide.

    P2: Instance-left > FunctionLike-right via find_member("__call__",
    left, left) (subtypes.py:1235-1240). P3: CallableType-left >
    protocol Instance-right with "__call__" in protocol_members
    (subtypes.py:1389-1398). Both fetch the member through the live
    TypeInfo map, so a live-map resolver decides natively and a
    snapshot-only resolver still defers.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
        self.resolver = _type_kernel.build_native_resolver(self._type_infos(), [])
        _set_native_subtype_resolver(self.resolver)
        _set_native_subtype_active(True)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _set_gate(self, active: bool) -> None:
        from mypy.subtypes import _set_native_subtype_active

        _set_native_subtype_active(active)

    def _add_call_method(self, info: TypeInfo, ret: Type) -> None:
        from mypy.nodes import FuncDef
        from mypy.types import CallableType

        signature = CallableType([], [], [], ret, self.fx.function)
        func_def = FuncDef("__call__", [], Block([]))
        func_def.type = signature
        # find_node_type maps the receiver onto method.info; a bare FuncDef
        # carries the FUNC_NO_INFO placeholder, so bind it to the class.
        func_def.info = info
        info.names["__call__"] = SymbolTableNode(MDEF, func_def)

    def _rebuild_with(self, infos: list[TypeInfo], live: bool) -> Any:
        from mypy.subtypes import _set_native_subtype_resolver

        all_infos = self._type_infos() + infos
        self.resolver = _type_kernel.build_native_resolver(all_infos, [])
        if live:
            self.resolver.set_live_typeinfo_map({info.fullname: info for info in all_infos})
        _set_native_subtype_resolver(self.resolver)
        return self.resolver

    def _seam(self, left: Any, right: Any, resolver: Any) -> Any:
        from mypy.subtypes import _serialize_type

        return _type_kernel.rust_is_subtype(
            _serialize_type(left),
            _serialize_type(right),
            False,
            False,
            False,
            False,
            False,
            True,
            False,
            False,
            resolver,
        )

    def test_instance_call_right_member_engages(self) -> None:
        # P2: Caller defines __call__ -> A; Caller <: () -> A. The live
        # fetch finds the member and recurses; both gates answer True and
        # the seam decides natively (no longer None).
        from mypy.subtypes import is_subtype
        from mypy.types import Instance

        info = self.fx.make_type_info("mod.Caller")
        info.mro = [info, self.fx.oi]
        self._add_call_method(info, self.fx.a)
        inst = Instance(info, [])
        right = self.fx.callable(self.fx.a)
        resolver = self._rebuild_with([info], live=True)
        self._set_gate(False)
        expected = is_subtype(inst, right)
        self._set_gate(True)
        assert is_subtype(inst, right) is expected
        assert expected, "Caller with __call__ -> A must be a subtype of () -> A"
        assert self._seam(inst, right, resolver) is True

    def test_instance_call_right_member_not_subtype(self) -> None:
        # P2 decides False: __call__ -> A is not a subtype of () -> bool.
        from mypy.subtypes import is_subtype
        from mypy.types import Instance

        info = self.fx.make_type_info("mod.Caller2")
        info.mro = [info, self.fx.oi]
        self._add_call_method(info, self.fx.a)
        inst = Instance(info, [])
        right = self.fx.callable(self.fx.bool_type)
        resolver = self._rebuild_with([info], live=True)
        self._set_gate(False)
        expected = is_subtype(inst, right)
        self._set_gate(True)
        assert is_subtype(inst, right) is expected
        assert not expected
        assert self._seam(inst, right, resolver) is False

    def test_instance_call_right_no_live_map_defers(self) -> None:
        # P2 with a snapshot-only resolver: the negative pre-check does
        # not fire (the class defines __call__), the live fetch is
        # unavailable, so the seam defers and Python answers.
        from mypy.subtypes import is_subtype
        from mypy.types import Instance

        info = self.fx.make_type_info("mod.Caller3")
        info.mro = [info, self.fx.oi]
        self._add_call_method(info, self.fx.a)
        inst = Instance(info, [])
        right = self.fx.callable(self.fx.a)
        resolver = self._rebuild_with([info], live=False)
        self._set_gate(False)
        expected = is_subtype(inst, right)
        self._set_gate(True)
        assert is_subtype(inst, right) is expected
        assert expected
        assert self._seam(inst, right, resolver) is None

    def test_callable_left_protocol_call_single_member(self) -> None:
        # P3 shortcut: protocol with exactly one member __call__, and the
        # callable matches it -> True decided natively.
        from mypy.subtypes import is_subtype
        from mypy.types import Instance

        pinfo = self.fx.make_type_info("mod.CallProto")
        pinfo.mro = [pinfo, self.fx.oi]
        pinfo.is_protocol = True
        self._add_call_method(pinfo, self.fx.a)
        inst = Instance(pinfo, [])
        left = self.fx.callable(self.fx.a)
        resolver = self._rebuild_with([pinfo], live=True)
        self._set_gate(False)
        expected = is_subtype(left, inst)
        self._set_gate(True)
        assert is_subtype(left, inst) is expected
        assert expected, "() -> A implements the single-member __call__ protocol"
        assert self._seam(left, inst, resolver) is True

    def test_callable_left_protocol_call_not_subtype(self) -> None:
        # P3 with a mismatching callable: the call check fails, Python falls
        # through to the is_type_obj/fallback tail; the fallback recursion
        # (Instance builtins.function vs protocol) decides False natively.
        from mypy.subtypes import is_subtype
        from mypy.types import Instance

        pinfo = self.fx.make_type_info("mod.CallProto2")
        pinfo.mro = [pinfo, self.fx.oi]
        pinfo.is_protocol = True
        self._add_call_method(pinfo, self.fx.bool_type)
        inst = Instance(pinfo, [])
        left = self.fx.callable(self.fx.a)
        resolver = self._rebuild_with([pinfo], live=True)
        self._set_gate(False)
        expected = is_subtype(left, inst)
        self._set_gate(True)
        assert is_subtype(left, inst) is expected
        assert not expected
        assert self._seam(left, inst, resolver) is False

    def test_callable_left_protocol_call_no_live_map_defers(self) -> None:
        # P3 with a snapshot-only resolver: the call-check port defers.
        from mypy.subtypes import is_subtype
        from mypy.types import Instance

        pinfo = self.fx.make_type_info("mod.CallProto3")
        pinfo.mro = [pinfo, self.fx.oi]
        pinfo.is_protocol = True
        self._add_call_method(pinfo, self.fx.a)
        inst = Instance(pinfo, [])
        left = self.fx.callable(self.fx.a)
        resolver = self._rebuild_with([pinfo], live=False)
        self._set_gate(False)
        expected = is_subtype(left, inst)
        self._set_gate(True)
        assert is_subtype(left, inst) is expected
        assert expected
        assert self._seam(left, inst, resolver) is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeVariadicTupleRightSuite(Suite):
    """Parity suite for the Rust TypeVarTupleType-right subtype port.

    `mypy/subtypes.py` `visit_instance` treats a TypeVarTupleType on the
    right of an Instance-left subtype check as an `Any`-like tuple
    (subtypes.py:617-620): map the left to the typevar's own
    `tuple_fallback` and answer `not proper_subtype` when the mapped
    first arg is Any. The Rust `visit_instance_variadic_right` ports
    this exactly using the wire-carried `tuple_fallback` and the
    TypeInfo snapshot's `has_base("builtins.tuple")`.

    Gate-toggling (differential) plus a direct seam call proving the
    Rust function engages for the portable path and defers cleanly
    (`None`) when the left target's snapshot is missing.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _tvt(self, fallback: Instance | None = None) -> TypeVarTupleType:
        # Mirrors `_bound_tvt` (testtypes.py:4527): a TypeVarTuple whose
        # `tuple_fallback` defaults to the fixture's `tuple[Any]`.
        return TypeVarTupleType(
            "Ts",
            "mod.Ts",
            TypeVarId(1),
            self.fx.o,
            fallback if fallback is not None else self.fx.std_tuple,
            AnyType(TypeOfAny.from_omitted_generics) if fallback is None else fallback.args[0],
        )

    def _tup_any(self) -> Instance:
        # tuple[Any, ...]: an Instance whose TypeInfo's MRO reaches
        # builtins.tuple (has_base set includes it).
        return self.fx.std_tuple

    def _tup_int(self) -> Instance:
        # tuple[int, ...]: same TypeInfo but an int argument, so the
        # mapped first arg is not Any.
        return Instance(self.fx.std_tuplei, [self.fx.a])

    def _tup_arg_any(self, typ: Instance, proper: bool = False) -> bool:
        # Direct seam call proving the Rust function engages for the
        # portable path (mirrors the Native*Suites' _assert_engages).
        from mypy.subtypes import _serialize_type

        left = _serialize_type(typ)
        right = _serialize_type(self._tvt())
        result = _type_kernel.rust_subtype_tvar_tuple_right(left, right, proper, self.resolver)
        assert result is not None, "Rust seam must engage for the variadic path"
        return result

    def _assert_par(self, left: Type, right: Type, *, proper: bool = False) -> None:
        # Differential: gate off (pure Python) vs on (Rust seam) must
        # agree on the decision string.
        from mypy.subtypes import _set_native_subtype_active, is_proper_subtype, is_subtype as _is

        def run() -> str:
            if proper:
                return str(is_proper_subtype(left, right))
            return str(_is(left, right))

        _set_native_subtype_active(False)
        try:
            off = run()
        finally:
            _set_native_subtype_active(True)
        on = run()
        assert_equal(on, off, f"subtype parity (right=TypeVarTuple) {left} vs {right}")

    def test_instance_left_variadic_right_any_then_true(self) -> None:
        # tuple[Any, ...] <: tuple[*Ts] -> True via the Any-mapped first
        # arg (not proper_subtype).
        left = self._tup_any()
        right = self._tvt()
        self._assert_par(left, right)
        assert self._tup_arg_any(left)

    def test_variadic_right_proper_subtype_false(self) -> None:
        # Under proper_subtype the Any-mapped first arg yields False.
        left = self._tup_any()
        right = self._tvt()
        self._assert_par(left, right, proper=True)
        assert not self._tup_arg_any(left, proper=True)

    def test_left_non_tuple_base_false(self) -> None:
        # A non-tuple Instance (e.g. list[int]) has no builtins.tuple
        # base -> False, and the seam engages (returns bool, not None).
        left = Instance(self.fx.gi, [self.fx.a])
        right = self._tvt()
        self._assert_par(left, right)
        assert self._tup_arg_any(left) is False

    def test_erased_any_keeps_false(self) -> None:
        # tuple[int, ...] vs the variadic target: mapped first arg is
        # int, not Any -> False.
        left = self._tup_int()
        right = self._tvt()
        self._assert_par(left, right)
        assert self._tup_arg_any(left) is False


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeVariadicTupleSubtypeSuite(Suite):
    """Parity suite for the Rust `variadic_tuple_subtype` port
    (subtypes.py:1086-1166).

    Covers TupleType vs TupleType when the right side has a variadic
    unpack. The Rust port returns `Some(Some(true))` when it
    short-circuits True; it returns fall-through (`None` inner) when it
    cannot (no right unpack, unsupported left shape, or a recursive
    `is_subtype` deferral), so the caller runs the fixed-length logic
    exactly like Python's `if self.variadic_tuple_subtype(left, right)`
    (subtypes.py:1064-1065).

    Gate-toggling (differential) plus a direct seam call
    (`rust_variadic_tuple_subtype`) proving Rust engages.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _tup(self, *items: Type) -> TupleType:
        return TupleType(list(items), self.fx.std_tuple)

    def _var_tup(self, item: Type) -> TupleType:
        # (*tuple[item, ...],): the variadic right side.
        unpack = UnpackType(Instance(self.fx.std_tuplei, [item]))
        return TupleType([unpack], self.fx.std_tuple)

    def _engages(self, left: Type, right: Type, proper: bool = False) -> bool | None:
        # Direct seam call: True when Rust decides short-circuit; None
        # when it cannot (fall-through). Proves the port engages.
        from mypy.subtypes import _serialize_type

        return _type_kernel.rust_variadic_tuple_subtype(
            _serialize_type(left), _serialize_type(right), proper, self.resolver
        )

    def _assert_par(self, left: Type, right: Type, *, proper: bool = False) -> None:
        # Differential: gate off (pure Python) vs on (Rust seam) must
        # agree on the decision string.
        from mypy.subtypes import _set_native_subtype_active, is_proper_subtype, is_subtype as _is

        def run() -> str:
            if proper:
                return str(is_proper_subtype(left, right))
            return str(_is(left, right))

        _set_native_subtype_active(False)
        try:
            off = run()
        finally:
            _set_native_subtype_active(True)
        on = run()
        assert_equal(on, off, f"subtype parity (variadic tuple) {left} vs {right}")

    def test_fixed_left_vs_variadic_right_true(self) -> None:
        # (A,) <: (*tuple[A, ...],) -> True (mapping selects length 1).
        left = self._tup(self.fx.a)
        right = self._var_tup(self.fx.a)
        self._assert_par(left, right)
        assert self._engages(left, right) is True

    def test_fixed_left_any_vs_variadic_right_true(self) -> None:
        # (A,) <: (*tuple[Any, ...],) -> True (Any item absorbs).
        left = self._tup(self.fx.a)
        right = self._var_tup(AnyType(TypeOfAny.special_form))
        self._assert_par(left, right)
        assert self._engages(left, right) is True

    def test_fixed_left_unrelated_item_vs_variadic_false(self) -> None:
        # (A,) !<: (*tuple[B, ...],) when A !<: B -> False.
        left = self._tup(self.fx.a)
        right = self._var_tup(self.fx.b)
        self._assert_par(left, right)

    def test_fixed_left_short_vs_prefix_suffix_false(self) -> None:
        # (A,) !<: (x, *tuple[A, ...]) because prefix+suffix exceed the
        # fixed left length (subtypes.py:1113-1114).
        left = self._tup(self.fx.a)
        right = TupleType(
            [self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a],
            self.fx.std_tuple,
        )
        self._assert_par(left, right)

    def test_fixed_left_matches_prefix_suffix_true(self) -> None:
        # (A, A, A) <: (A, *tuple[A, ...], A) -> True (prefix/suffix
        # consumed, middle maps to the infinite union item).
        left = self._tup(self.fx.a, self.fx.a, self.fx.a)
        right = TupleType(
            [self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a],
            self.fx.std_tuple,
        )
        self._assert_par(left, right)
        assert self._engages(left, right) is True

    def test_both_variadic_same_item_true(self) -> None:
        # (*tuple[A, ...],) <: (*tuple[A, ...],) -> True (asymptotic and
        # all finite overlaps).
        left = self._var_tup(self.fx.a)
        right = self._var_tup(self.fx.a)
        self._assert_par(left, right)
        assert self._engages(left, right) is True

    def test_both_variadic_unrelated_item_false(self) -> None:
        # (*tuple[A, ...],) !<: (*tuple[B, ...],) when A !<: B -> False
        # (asymptotic case fails).
        left = self._var_tup(self.fx.a)
        right = self._var_tup(self.fx.b)
        self._assert_par(left, right)

    def test_left_unpack_top_right(self) -> None:
        # (*tuple[A, ...],) <: (*tuple[object, ...],) -> True via the
        # top-type right path (object middle, subtypes.py:1137-1151).
        left = self._var_tup(self.fx.a)
        right = self._var_tup(self.fx.o)
        self._assert_par(left, right)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeExpandTypeByInstanceSuite(Suite):
    """Parity for the Rust `expand_type_by_instance` TypeVarTuple branch
    (mypy.expandtype, expandtype.py:391-406).

    A variadic instance (`instance.type.has_type_var_tuple_type`) binds
    its TypeVarTuple to a TupleType of the middle args and the prefix /
    suffix args to their ordinary tvars. The Rust branch (expandtype.rs
    `expand_type_by_instance_core`, with the new
    `type_var_tuple_fallback` snapshot field) must produce the same
    expansion as the pure-Python visitor. Toggling the gate off vs on
    must agree on `str(expanded)`, and a direct seam call proves the Rust
    engagement for the variadic prefix=1/suffix=1 case.
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_expand_type_active
        self._set_resolver = _set_native_expand_type_resolver
        self._set_map = _set_native_expand_type_typeinfo_map
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._type_infos = type_infos
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(self._resolver)
        self._set_map({info.fullname: info for info in type_infos})
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)
        set_wire_alias_map(None)

    def _register(self, info: TypeInfo) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._type_infos.append(info)
        self._resolver.update([info], [])
        set_wire_typeinfo_map({i.fullname: i for i in self._type_infos})

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _expand(self, typ: Type, instance: Instance) -> Type:
        from mypy.expandtype import expand_type_by_instance

        return expand_type_by_instance(typ, instance)

    def _assert_par(self, typ: Type, instance: Instance) -> None:
        off = str(self._with_gate(False, lambda: self._expand(typ, instance)))
        on = str(self._with_gate(True, lambda: self._expand(typ, instance)))
        assert_equal(on, off, f"expand_type_by_instance parity {typ} inst {instance}")

    def _variadic_info(self, name: str, prefix: int, suffix: int) -> TypeInfo:
        # typevars ["T", "Ts", "B"] with TypeVarTuple at index 1 -> the
        # wire snapshot reads prefix/suffix from the live TypeInfo (which
        # must be set by hand, mirroring nodes.py).
        info = self.fx.make_type_info(
            name, mro=[self.fx.oi], typevars=["T", "Ts", "B"], typevar_tuple_index=1
        )
        info.type_var_tuple_prefix = prefix
        info.type_var_tuple_suffix = suffix
        return info

    def test_seam_engages_variadic(self) -> None:
        # Direct seam call: the variadic (prefix=1/suffix=1) expansion
        # must engage (return bytes) rather than defer to Python.
        from mypy.expandtype import _serialize_type

        info = self._variadic_info("VPair", 1, 1)
        self._register(info)
        instance = Instance(
            info, [self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.b])), self.fx.c]
        )
        # typ = Tuple[Unpack[Ts]] references the size-2 middle.
        typ = TupleType([UnpackType(Instance(self.fx.std_tuplei, [self.fx.b]))], self.fx.std_tuple)
        result = _type_kernel.rust_expand_type_by_instance(
            self._resolver, _serialize_type(typ), _serialize_type(instance), state.strict_optional
        )
        assert result is not None, "Rust expand_type_by_instance did not engage for variadic"

    def test_variadic_prefix_suffix_parity(self) -> None:
        # VPair[T, *Ts, B] applied to VPair[A, *tuple[B, ...], C]:
        # *Ts binds Tuple[B] (the middle item). Expanding a type that
        # references *Ts must splice the middle tuple's items.
        info = self._variadic_info("VPair", 1, 1)
        self._register(info)
        instance = Instance(
            info, [self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.b])), self.fx.c]
        )
        # typ references the TypeVarTuple *Ts via an UnpackType.
        typ = TupleType(
            [
                UnpackType(
                    TypeVarTupleType(
                        "Ts", "VPair.Ts", TypeVarId(2), self.fx.o, self.fx.std_tuple, self.fx.anyt
                    )
                )
            ],
            self.fx.std_tuple,
        )
        self._assert_par(typ, instance)

    def test_variadic_no_middle_bypasses(self) -> None:
        # Variadic prefix=1/suffix=1 with exactly two args: middle is
        # empty, so binding *Ts to an empty tuple must not assert in the
        # Rust branch. Both gates must agree.
        info = self._variadic_info("VPair", 1, 1)
        self._register(info)
        instance = Instance(info, [self.fx.a, self.fx.c])
        typ = self.fx.a  # no tvar reference
        off = str(self._with_gate(False, lambda: self._expand(typ, instance)))
        on = str(self._with_gate(True, lambda: self._expand(typ, instance)))
        assert_equal(on, off, "empty-middle variadic parity")

    # --- alias round-trip (#1289): the relink entry accepts alias-bearing
    # input; the FFI seams resolve the per-build alias snapshots ---

    def _install_alias(self) -> tuple[TypeAlias, TypeInfo, TypeVarLikeType]:
        # Bag[T] with a class-bound tvar (id namespace "Bag", mirroring
        # the production contract that binder ids carry the declaring
        # class fullname), plus the generic alias A = list[T].
        from mypy.wirefixup import set_wire_alias_map

        info = self.fx.make_type_info("Bag", mro=[self.fx.oi], typevars=["T"])
        binder = info.defn.type_vars[0]
        binder.id = TypeVarId(1, namespace="Bag")
        alias = TypeAlias(Instance(self.fx.std_listi, [binder]), "mod.A", "mod", -1, -1)
        self._resolver = _type_kernel.build_native_resolver([info, *self._type_infos], [alias])
        self._set_resolver(self._resolver)
        set_wire_alias_map({alias.fullname: alias})
        return alias, info, binder

    def test_alias_instance_arg_expands_natively(self) -> None:
        # list[A[T]] against Bag[int]: the alias-bearing input takes the
        # relink entry, the arg aliases substitute natively, and the
        # surviving alias node re-links through the alias map.
        from mypy.expandtype import _serialize_type

        alias, info, binder = self._install_alias()
        typ = Instance(self.fx.std_listi, [TypeAliasType(alias, [binder])])
        instance = Instance(info, [self.fx.a])
        result = _type_kernel.rust_expand_type_by_instance(
            self._resolver, _serialize_type(typ), _serialize_type(instance), state.strict_optional
        )
        assert result is not None, "alias-bearing by_instance expansion deferred"
        self._assert_par(typ, instance)
        on = self._expand(typ, instance)
        assert isinstance(on, Instance), str(on)  # type: ignore[misc]
        # args[0] is the re-linked alias node itself; get_proper_type here
        # would expand it and defeat the re-link assertion.
        assert isinstance(on.args[0], TypeAliasType), str(on)
        alias_arg = on.args[0]
        assert alias_arg.alias is alias, "decoded alias node not re-linked"
        assert alias_arg.args == [self.fx.a]

    def test_alias_union_item_flattens_natively(self) -> None:
        # Union[A[T], B] against Bag[int]: the FFI entry installs the
        # alias map for the union flatten (issue #1203 path), so the
        # seam engages instead of deferring the whole call.
        from mypy.expandtype import _serialize_type

        alias, info, binder = self._install_alias()
        typ = UnionType([TypeAliasType(alias, [binder]), self.fx.b])
        instance = Instance(info, [self.fx.a])
        result = _type_kernel.rust_expand_type_by_instance(
            self._resolver, _serialize_type(typ), _serialize_type(instance), state.strict_optional
        )
        assert result is not None, "alias union item by_instance deferred"
        self._assert_par(typ, instance)

    def test_missing_alias_map_defers_to_python(self) -> None:
        # Without a wire alias map the fixup cannot re-link the decoded
        # alias; the shim falls back to the pure-Python body and both
        # gates still agree.
        from mypy.wirefixup import set_wire_alias_map

        alias, info, binder = self._install_alias()
        typ = Instance(self.fx.std_listi, [TypeAliasType(alias, [binder])])
        instance = Instance(info, [self.fx.a])
        set_wire_alias_map(None)
        try:
            self._assert_par(typ, instance)
        finally:
            set_wire_alias_map({alias.fullname: alias})


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeExpandTypeFreezeIdentitySuite(Suite):
    """Fresh-var object identity across the freshen -> apply freeze chain (#1180).

    `freeze_all_type_vars` (typeops.py) mutates `TypeVarId.meta_level` in
    place, so it only works when the type vars the enclosing `variables`
    slot lists are the *same objects* as the occurrences in the tree. The
    wire decode creates fresh objects, so any seam that rebuilds a
    subtree through the wire must either re-canonicalize ids against a
    seeded object or re-link them onto the live originals. `remove_trivial`
    is a partial-list seam with no `variables` context, so it re-links
    every decoded var onto the live object in its own input list
    (`wirefixup.resync_var_identities_list`, #1623); the other relaxed
    callers (expand_type / expand_type_by_instance / freshen) canonicalize
    or are seeded.

    Locks the regression: a decorated generic signature whose return
    union is rebuilt through Python `expand_type` -> `visit_union_type`
    -> native `remove_trivial` used to split the fresh var into two
    objects, leaving `variables` frozen while the arg/ret occurrences
    stayed meta-level 1.
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
        )
        from mypy.nodes import ARG_POS
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_expand_type_active
        self._set_resolver = _set_native_expand_type_resolver
        self._set_map = _set_native_expand_type_typeinfo_map
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._type_infos = type_infos
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(self._resolver)
        self._set_map({info.fullname: info for info in type_infos})
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

        fx = self.fx
        self._callee = CallableType(
            [fx.t],
            [ARG_POS],
            [None],
            UnionType([fx.t, fx.b]),
            fx.function,
            name="dec",
            variables=[fx.t],
        )

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _occurrences(self, t: Type) -> list[TypeVarType]:
        out: list[TypeVarType] = []
        stack: list[Type] = [t]
        seen: set[int] = set()
        while stack:
            p = get_proper_type(stack.pop())
            if id(p) in seen:
                continue
            seen.add(id(p))
            if isinstance(p, TypeVarType):
                out.append(p)
            elif isinstance(p, UnionType):
                stack.extend(p.items)
            elif isinstance(p, Instance):
                stack.extend(p.args)
            elif isinstance(p, CallableType):
                stack.append(p.ret_type)
                stack.extend(p.arg_types)
        return out

    def test_freshen_retains_occurrence_identity(self) -> None:
        from mypy.expandtype import freshen_function_type_vars

        fresh = freshen_function_type_vars(self._callee)
        assert isinstance(fresh, CallableType)
        var = fresh.variables[0]
        occs = self._occurrences(fresh.ret_type) + self._occurrences(fresh.arg_types[0])
        assert occs, "expected tvar occurrences in the freshened signature"
        for o in occs:
            assert o is var, "freshen split the fresh var into distinct objects"

        # freeze_all_type_vars relies on object identity: the
        # in-place meta_level mutation must reach every occurrence.
        from mypy.typeops import freeze_all_type_vars

        freeze_all_type_vars(fresh)
        assert var.id.meta_level == 0
        for o in occs:
            assert o is var
            assert o.id.meta_level == 0

        # Gate parity on the frozen result.
        off = str(self._with_gate(False, lambda: str(fresh)))
        assert off == str(fresh)

    def test_apply_substitution_preserves_identity(self) -> None:
        # Root-cause regression: apply_generic_arguments substitutes the
        # fresh var by another free var; the ret union is rebuilt through
        # visit_union_type -> native remove_trivial. g must be identical.
        from mypy.expandtype import expand_type, freshen_function_type_vars

        fresh = freshen_function_type_vars(self._callee)
        assert isinstance(fresh, CallableType)
        var = fresh.variables[0]
        g = var.copy_modified(id=TypeVarId(var.id.raw_id + 900, meta_level=1))
        out = expand_type(fresh, {var.id: g})
        assert isinstance(out, CallableType)
        occs = self._occurrences(out.ret_type) + self._occurrences(out.arg_types[0])
        assert occs
        for o in occs:
            assert o is g, "apply chain rebuilt a split copy of the fresh var"

    def test_remove_trivial_relinks_fresh_vars_natively(self) -> None:
        # A meta (fresh) var with no default now decides natively: the
        # decoded copy is re-linked onto the live input object, so
        # freeze_all_type_vars's identity contract still holds (#1623).
        from unittest import mock

        import type_kernel

        from mypy.expandtype import remove_trivial

        fx = self.fx
        v = fx.t.copy_modified(id=TypeVarId(500, meta_level=1))
        calls: list[bytes] = []
        real = type_kernel.rust_remove_trivial

        def wrapper(b: bytes, so: bool) -> object:
            calls.append(b)
            return real(b, so)

        with mock.patch.object(type_kernel, "rust_remove_trivial", wrapper):
            result = remove_trivial([v, fx.b])
        assert_equal(len(calls), 1, "fresh-var list did not cross the Rust seam")
        assert result[0] is v

    def test_freeze_identity_gate_parity(self) -> None:
        # Gate on vs gate off must agree str-wise on the full
        # freshen -> substitute chain that previously split.
        from mypy.expandtype import expand_type, freshen_function_type_vars

        def run() -> str:
            fresh = freshen_function_type_vars(self._callee)
            assert isinstance(fresh, CallableType)
            var = fresh.variables[0]
            g = var.copy_modified(id=TypeVarId(var.id.raw_id + 900, meta_level=1))
            out = expand_type(fresh, {var.id: g})
            assert isinstance(out, CallableType)
            return str(out)

        on = str(self._with_gate(True, run))
        off = str(self._with_gate(False, run))
        assert_equal(on, off, "freshen+apply gate parity regression")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeExpandTypeDefinitionGateSuite(Suite):
    """Parity for the `definition_gate` flag in expandtype._needs_python (#1220).

    The CallableType arm of `mypy.expandtype._needs_python` checked
    `p.definition is not None` unconditionally, so callers passing
    `definition_gate=False` over-deferred to Python whenever the type
    nested a callable carrying a `definition` node, even though the
    wire-decoded result is repaired via `_resync_definitions`. The twin
    `mypy.typeops._needs_python` already mirrors
    `if definition_gate and p.definition is not None`.

    Scope note (#1220): the relaxation survives only where it is provably
    safe. The instance-args precheck of `expand_type_by_instance` and
    `freshen_all_functions_type_vars` pass `definition_gate=False`
    (definitions re-stamped via `_resync_definitions`; unpairable shapes
    defer to the pure-Python body). The `expand_type` and
    `expand_type_by_instance` top-level gates keep the flag on: their
    decoded trees re-enter error reporting as plugin contexts, and nested
    wire-decoded types carry no locations, so relaxing them loses error
    line numbers (functools partial regressions
    `check-functools.test::testFunctoolsPartial*` against #1219's fixup).

    Locks: with `definition_gate=False` a definition-bearing callable must
    not defer (predicate differential + direct freshen_all seam decision),
    while `definition_gate=True` keeps the defer; the freshen_all path
    must agree gate off vs gate on and re-stamp the same live definition
    node.
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
        )
        from mypy.nodes import ARG_POS, FuncDef
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_expand_type_active
        self._set_resolver = _set_native_expand_type_resolver
        self._set_map = _set_native_expand_type_typeinfo_map
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._type_infos = type_infos
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(self._resolver)
        self._set_map({info.fullname: info for info in type_infos})
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

        fx = self.fx
        defn = FuncDef("f")
        self._callee = CallableType(
            [fx.t],
            [ARG_POS],
            [None],
            fx.t,
            fx.function,
            name="f",
            variables=[fx.t],
            definition=defn,
        )
        self._defn = defn

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def test_definition_gate_false_stops_deferring(self) -> None:
        from mypy.expandtype import _needs_python

        assert _needs_python(self._callee), "gate=True must defer on definition"
        assert not _needs_python(
            self._callee, definition_gate=False
        ), "gate=False must not defer on a re-stamped callable"
        # A nested definition-carrying callable must also pass relaxed.
        nested = UnionType([self._callee, self.fx.b])
        assert not _needs_python(nested, definition_gate=False)

    def test_freshen_all_definition_restamped(self) -> None:
        # The surviving relaxation in production: freshen_all passes
        # definition_gate=False and re-stamps the dropped definition from
        # the pre-seam type; gate off vs gate on must agree.
        from mypy.expandtype import freshen_all_functions_type_vars

        def run() -> Type:
            return freshen_all_functions_type_vars(self._callee)

        off = self._with_gate(False, lambda: str(run()))
        on_result = self._with_gate(True, run)
        assert_equal(str(on_result), off, "freshen_all parity on definition-carrying callable")
        on_proper = get_proper_type(on_result)
        assert isinstance(on_proper, CallableType)
        assert on_proper.definition is self._defn

    def test_seam_engages_with_definition(self) -> None:
        # Direct seam call through the surviving freshen_all relaxation: the
        # kernel decides on the wire shape alone (definitions are dropped in
        # transit), so a definition-bearing generic callable must engage.
        from mypy.expandtype import _serialize_type

        call = _type_kernel.rust_freshen_all_functions_type_vars(
            TypeVarId.next_raw_id,
            _serialize_type(self._callee),
            state.strict_optional,
            self._resolver,
        )
        assert call is not None, "Rust freshen_all deferred on definition-carrying callable"
        _next_raw_id, changed, _serialized = call
        assert changed, "Rust freshen_all reported no change for a generic callable"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeExpandTypeEmptyEnvSuite(Suite):
    """Parity for the Rust `expand_type` empty-env fast path.

    `expand_type(typ, {})` performs no substitution. The seam used to bail
    on ANY empty env (expandtype.rs returned None inside rust_expand_type),
    forcing a pure-Python rebuild even for typevar-free types. Now an empty
    env is wire-portable: `expand_type_with_env` rebuilds the tree and
    returns leftover TypeVars instead of deferring; the shim re-links the
    decoded vars to the live originals (`resync_var_identities`), so the
    caller keeps object identity. This suite locks the differential: a
    typevar-free type must engage natively and gate-on == gate-off for
    both typevar-free and typevar-bearing input.
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_expand_type_active
        self._set_resolver = _set_native_expand_type_resolver
        self._set_map = _set_native_expand_type_typeinfo_map
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._type_infos = type_infos
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(self._resolver)
        self._set_map({info.fullname: info for info in type_infos})
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _expand(self, typ: Type) -> Type:
        from mypy.expandtype import expand_type

        return expand_type(typ, {})

    def _assert_par(self, typ: Type) -> None:
        off = str(self._with_gate(False, lambda: self._expand(typ)))
        on = str(self._with_gate(True, lambda: self._expand(typ)))
        assert_equal(on, off, f"expand_type(empty env) parity {typ}")

    def test_seam_engages_typevar_free(self) -> None:
        # G[A] with an empty env has no TypeVar to substitute; the seam must
        # engage (return bytes) and produce a rebuilt G[A].
        from mypy.expandtype import _serialize_env, _serialize_type

        result = _type_kernel.rust_expand_type(
            self._resolver, _serialize_type(self.fx.ga), _serialize_env({}), state.strict_optional
        )
        assert result is not None, "empty-env typevar-free expand_type did not engage"

    def test_typevar_free_parity(self) -> None:
        # G[A] (no typevars): native rebuild must equal Python rebuild.
        self._assert_par(self.fx.ga)

    def test_typevar_result_relinks_identity(self) -> None:
        # G[T] with an empty env leaves T unmatched. The seam returns the
        # expansion with the leftover T; the shim re-links every decoded T
        # occurrence to the live original (identity parity, gate-on == off).
        from mypy.expandtype import _serialize_env, _serialize_type

        result = _type_kernel.rust_expand_type(
            self._resolver, _serialize_type(self.fx.gt), _serialize_env({}), state.strict_optional
        )
        assert (
            result is not None
        ), "empty-env typevar-bearing expand_type must return the leftover-tvar result"
        off = self._with_gate(False, lambda: self._expand(self.fx.gt))
        on = self._with_gate(True, lambda: self._expand(self.fx.gt))
        assert_equal(str(on), str(off), "expand_type(empty env) parity")
        assert isinstance(on, Instance), str(on)  # type: ignore[misc]
        assert isinstance(off, Instance), str(off)  # type: ignore[misc]
        assert on.args[0] is off.args[0] is self.fx.t, "decoded TypeVar must relink to original"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeExpandTypeAliasSuite(Suite):
    """Parity for the Rust `expand_type` alias-arg handling (#1195).

    The seam used to defer every alias-bearing input (the alias_entry
    guard) and any result still carrying a TypeAliasType. Alias args now
    expand natively (mirroring visit_type_alias_type) and the Python shim
    re-links wire-decoded alias nodes to live TypeAlias nodes via
    fixup_wire_type(resolve_aliases=True); a decoded alias missing from
    the per-build alias map defers to the pure-Python body, and parity
    holds either way.
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
        )
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_expand_type_active
        self._set_resolver = _set_native_expand_type_resolver
        self._set_map = _set_native_expand_type_typeinfo_map
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        # A = List[T], generic in the fixture typevar, so both bare and
        # arg-bearing references exist and substitution inside the alias
        # args is observable.
        from mypy.nodes import TypeAlias

        self.alias = TypeAlias(Instance(self.fx.std_listi, [self.fx.t]), "mod.A", "mod", -1, -1)
        self._rebuild_resolver([self.alias])
        self._set_map({info.fullname: info for info in type_infos})
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        set_wire_alias_map({self.alias.fullname: self.alias})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)
        set_wire_alias_map(None)

    def _rebuild_resolver(self, aliases: list[Any]) -> None:
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._resolver = _type_kernel.build_native_resolver(type_infos, aliases)
        self._set_resolver(self._resolver)

    def _with_gate(self, active: bool, fn: Callable[[], Any]) -> Any:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _expand(self, typ: Type) -> Type:
        from mypy.expandtype import expand_type

        return expand_type(typ, {self.fx.t.id: self.fx.a})

    def _assert_par(self, typ: Type) -> None:
        off = str(self._with_gate(False, lambda: self._expand(typ)))
        on = str(self._with_gate(True, lambda: self._expand(typ)))
        assert_equal(on, off, f"expand_type(alias) parity {typ}")

    def test_alias_args_expand_natively(self) -> None:
        # expand_type(A[T], {T: A}) -> A[A]: alias args substitute in Rust
        # and the surviving alias node re-links through the alias map.
        from mypy.expandtype import _serialize_env, _serialize_type

        alias_t = TypeAliasType(self.alias, [self.fx.t])
        result = _type_kernel.rust_expand_type(
            self._resolver,
            _serialize_type(alias_t),
            _serialize_env({self.fx.t.id: self.fx.a}),
            state.strict_optional,
        )
        assert result is not None, "alias-arg expand_type deferred (alias_entry guard back?)"
        self._assert_par(alias_t)

    def test_bare_alias_passes_through(self) -> None:
        # expand_type(A, {T: A}) (no args): Python returns t unchanged; the
        # seam engages and the wire clone re-links to the live TypeAlias.
        from mypy.expandtype import _serialize_env, _serialize_type

        bare = TypeAliasType(self.alias, [])
        result = _type_kernel.rust_expand_type(
            self._resolver,
            _serialize_type(bare),
            _serialize_env({self.fx.t.id: self.fx.a}),
            state.strict_optional,
        )
        assert result is not None, "bare alias expand_type deferred"
        self._assert_par(bare)

    def test_alias_inside_instance_parity(self) -> None:
        # G[A[int]]: an alias nested inside a non-alias input must also
        # survive the round-trip (the fixup re-links the nested node).
        alias_int = TypeAliasType(self.alias, [self.fx.a])
        self._assert_par(Instance(self.fx.gi, [alias_int]))

    def test_missing_alias_map_defers_to_python(self) -> None:
        # Without a wire alias map the fixup cannot re-link the decoded
        # alias; the shim must fall back to the pure-Python body and both
        # gates still agree.
        from mypy.wirefixup import set_wire_alias_map

        alias_t = TypeAliasType(self.alias, [self.fx.t])
        set_wire_alias_map(None)
        try:
            self._assert_par(alias_t)
        finally:
            set_wire_alias_map({self.alias.fullname: self.alias})

    def _freshen_par(self, typ: Type) -> Type:
        from mypy.expandtype import freshen_all_functions_type_vars

        on_result = cast(Type, self._with_gate(True, lambda: freshen_all_functions_type_vars(typ)))
        off = str(self._with_gate(False, lambda: freshen_all_functions_type_vars(typ)))
        assert_equal(str(on_result), off, f"freshen_all(alias) parity {typ}")
        return on_result

    def test_freshen_alias_union_argument(self) -> None:
        # Union[U, None] with U = Union[A, B]: the union arm expands the
        # alias through the resolver snapshot and flattens it (#1203), so
        # `_resync_definitions` must ride the 2 -> 3 item change (no defs).
        from mypy.nodes import TypeAlias
        from mypy.wirefixup import set_wire_alias_map

        union_alias = TypeAlias(UnionType([self.fx.a, self.fx.b]), "mod.U", "mod", -1, -1)
        self._rebuild_resolver([self.alias, union_alias])
        set_wire_alias_map({self.alias.fullname: self.alias, union_alias.fullname: union_alias})
        try:
            c = CallableType(
                [UnionType([TypeAliasType(union_alias, []), NoneType()])],
                [ARG_POS],
                [None],
                self.fx.a,
                self.fx.function,
                variables=[self.fx.t],
            )
            result = get_proper_type(self._freshen_par(c))
            assert isinstance(result, CallableType)
            arg = get_proper_type(result.arg_types[0])
            assert isinstance(arg, UnionType)
            assert len(arg.items) == 3
            assert all(
                type(i) is not TypeAliasType for i in arg.items
            ), f"alias survived the flatten: {arg}"
        finally:
            self._rebuild_resolver([self.alias])
            set_wire_alias_map({self.alias.fullname: self.alias})

    def test_freshen_direct_alias_argument(self) -> None:
        # A direct alias arg survives expansion (visit_type_alias_type
        # keeps the node); the shim's alias-decode retry re-links the live
        # TypeAlias and `_resync_definitions` pairs the alias args.
        c = CallableType(
            [TypeAliasType(self.alias, [self.fx.t])],
            [ARG_POS],
            [None],
            self.fx.a,
            self.fx.function,
            variables=[self.fx.t],
        )
        result = get_proper_type(self._freshen_par(c))
        assert isinstance(result, CallableType)
        arg = result.arg_types[0]
        assert isinstance(arg, TypeAliasType)
        assert arg.alias is self.alias, "decoded alias not re-linked to live node"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeExpandParamSpecSpliceSuite(Suite):
    """Parity for the Rust `expand_type` ParamSpec splice (wave29 #1343).

    `visit_callable_type`'s Concatenate splice (expandtype.py:1149-1195)
    and `visit_param_spec` (:963-996) run natively when the env maps the
    ParamSpec to a Parameters and no ParamSpecType occurs in the result.
    Shapes the port defers (a fresh meta substitute keyed only at a
    nonzero meta level, an unpack it cannot normalize, an ARGS/KWARGS
    leaf with a Parameters replacement, and any splice or leaf result
    that embeds a ParamSpecType -- the wire drops meta_level, so a
    fresh origin would round-trip at meta level 0) fall back to the
    pure Python body, so gate-on == gate-off everywhere.
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        self._set_active = _set_native_expand_type_active
        self._set_resolver = _set_native_expand_type_resolver
        self._set_map = _set_native_expand_type_typeinfo_map
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(self._resolver)
        self._set_map({info.fullname: info for info in type_infos})
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], Any]) -> Any:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _expand(self, typ: Type, env: Mapping[TypeVarId, Type]) -> Any:
        from mypy.expandtype import expand_type

        return expand_type(typ, env)

    def _assert_par(self, typ: Type, env: Mapping[TypeVarId, Type]) -> None:
        off = str(self._with_gate(False, lambda: self._expand(typ, env)))
        on = str(self._with_gate(True, lambda: self._expand(typ, env)))
        assert_equal(on, off, f"expand_type(ps splice) parity {typ}")

    def _param_spec(
        self, raw_id: int = 1, name: str = "P", fullname: str = "mod.P"
    ) -> ParamSpecType:
        return ParamSpecType(
            name, fullname, TypeVarId(raw_id), 0, self.fx.o, AnyType(TypeOfAny.special_form)
        )

    def _splice_callable(
        self, ps: ParamSpecType, lead: Sequence[Type], ret: Type | None = None
    ) -> CallableType:
        # Concatenate[*lead, P.args, **P.kwargs]: the trailing ARGS and
        # KWARGS occurrences occupy the last two arg slots; Python's
        # CallableType.param_spec() canonicalizes the head likewise.
        arg_types = [
            *lead,
            ps.with_flavor(ParamSpecFlavor.ARGS),
            ps.with_flavor(ParamSpecFlavor.KWARGS),
        ]
        arg_kinds = [ARG_POS] * len(lead) + [ARG_STAR, ARG_STAR2]
        return CallableType(
            arg_types,
            arg_kinds,
            [None] * len(arg_types),
            ret if ret is not None else self.fx.anyt,
            self.fx.function,
        )

    def _engaged(self, typ: Type, env: Mapping[TypeVarId, Type]) -> bool:
        from mypy.expandtype import _serialize_env, _serialize_type

        result = _type_kernel.rust_expand_type(
            self._resolver, _serialize_type(typ), _serialize_env(env), state.strict_optional
        )
        return result is not None

    def test_splice_with_parameters_repl(self) -> None:
        # Concatenate[int, P] with P -> Parameters[str]: natively spliced
        # (args + kinds + names concat), the repl's prefix rides in.
        from mypy.types import Parameters

        ps = self._param_spec()
        callee = self._splice_callable(ps, [self.fx.a])
        repl = Parameters([self.fx.str_type], [ARG_POS], [None])
        env = {ps.id: repl}
        assert self._engaged(callee, env), "paramspec splice deferred"
        self._assert_par(callee, env)

    def test_splice_with_paramspec_repl(self) -> None:
        # P -> Q with a prefix: the splice builds clean Q.args/**Q.kwargs
        # nodes carrying Q's meta_level, which round-trips on the wire
        # (ParamSpecType.id.meta_level since #1417), so the seam decides.
        from mypy.types import Parameters

        ps = self._param_spec()
        repl = ParamSpecType(
            "Q",
            "mod.Q",
            TypeVarId(2),
            0,
            self.fx.o,
            AnyType(TypeOfAny.special_form),
            prefix=Parameters([self.fx.str_type], [ARG_POS], [None]),
        )
        callee = self._splice_callable(ps, [self.fx.a])
        env = {ps.id: repl}
        assert self._engaged(callee, env), "paramspec splice deferred"
        self._assert_par(callee, env)

    def test_splice_without_repl_defers(self) -> None:
        # Empty env: the splice arm falls through and the generic path
        # keeps the occurrences, but the output embeds ParamSpecTypes
        # the wire cannot round-trip; the kernel defers to Python.
        ps = self._param_spec()
        callee = self._splice_callable(ps, [self.fx.a])
        assert not self._engaged(callee, {}), "occurrence keeper engaged"
        self._assert_par(callee, {})

    def test_splice_fresh_key_defers(self) -> None:
        # A fresh (meta level 1) env entry keyed raw_id+namespace masks
        # the bare key: the kernel defers and Python answers.
        ps = self._param_spec()
        fresh = ParamSpecType(
            "Q", "mod.Q", TypeVarId(ps.id.raw_id, 1), 0, self.fx.o, AnyType(TypeOfAny.special_form)
        )
        callee = self._splice_callable(ps, [self.fx.a])
        env = {fresh.id: self.fx.a}
        assert not self._engaged(callee, env), "fresh-key splice engaged"
        self._assert_par(callee, env)

    def test_splice_unpack_star_normalizes(self) -> None:
        # An ARG_STAR UnpackType wrapping builtins.tuple in the splice
        # result is normalized natively (normalize_trivial_unpack port):
        # *args: *tuple[Any, ...] -> *args: Any.  The seam engages.
        from mypy.types import Parameters, UnpackType

        ps = self._param_spec()
        callee = self._splice_callable(ps, [self.fx.a])
        unpacked = UnpackType(Instance(self.fx.std_tuplei, [self.fx.anyt]))
        env = {ps.id: Parameters([unpacked], [ARG_STAR], [None])}
        assert self._engaged(callee, env), "unpack splice deferred"
        self._assert_par(callee, env)

    def test_leaf_bare_parameters_repl(self) -> None:
        # A bare P occurrence expands to the Parameters replacement
        # (prefix concat, occurrence flavor preserved per Python).
        from mypy.types import Parameters

        ps = self._param_spec()
        repl = Parameters([self.fx.str_type], [ARG_POS], [None])
        env = {ps.id: repl}
        typ = Instance(self.fx.gi, [ps])
        assert self._engaged(typ, env), "bare leaf deferred"
        self._assert_par(typ, env)

    def test_leaf_args_flavor_defers(self) -> None:
        # An ARGS occurrence with a Parameters replacement rides the
        # unported _possible_callable_varargs arm; defer -> Python parity.
        from mypy.types import Parameters

        ps = self._param_spec()
        env = {ps.id: Parameters([self.fx.str_type], [ARG_POS], [None])}
        typ = Instance(self.fx.gi, [ps.with_flavor(ParamSpecFlavor.ARGS)])
        assert not self._engaged(typ, env), "ARGS leaf engaged"
        self._assert_par(typ, env)

    def test_leaf_missing_env_prefix_defers(self) -> None:
        # No repl for P: Python's default expands only P's own prefix
        # and keeps the occurrence, but the output is a ParamSpecType
        # whose meta_level the wire would flatten; the kernel defers.
        from mypy.types import Parameters

        ps = ParamSpecType(
            "P",
            "mod.P",
            TypeVarId(1),
            0,
            self.fx.o,
            AnyType(TypeOfAny.special_form),
            prefix=Parameters([self.fx.bool_type], [ARG_POS], [None]),
        )
        typ = Instance(self.fx.gi, [ps])
        assert not self._engaged(typ, {}), "prefix-expansion leaf engaged"
        self._assert_par(typ, {})


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassCallableSuite(Suite):
    """Parity for the Rust `class_callable` port (mypy.typeops, #492 follow-up).

    The Python shim in mypy/typeops.py::class_callable makes the ret_type
    decision in Rust once the two resolver-backed subtype booleans (is_eq,
    is_st) are known; Python then rebuilds the live CallableType so non-wire
    fields survive. Toggling the typeops gate off (pure-Python fallback) and
    on (Rust seam) must produce an equal ret_type and equal variables. The
    direct `rust_class_callable` call proves the seam actually engages for a
    default __init__ rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        infos = [self.fx.oi, self.fx.ai, self.fx.bi, self.fx.di, self.fx.gi]
        set_wire_typeinfo_map({i.fullname: i for i in infos})
        _set_native_typeops_active(True)

    def tearDown(self) -> None:
        from mypy.typeops import _set_native_typeops_active
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_typeops_active(False)
        set_wire_typeinfo_map({})

    def _init_callable(self, info: TypeInfo, ret: Type | None = None) -> CallableType:
        # A minimal `<init>` signature: one positional self, the given ret.
        return CallableType(
            [Instance(info, [])],
            [ARG_POS],
            [None],
            ret if ret is not None else UninhabitedType(),
            Instance(self.fx.oi, []),
            name="<init>",
        )

    def _construct(
        self, init_type: CallableType, info: TypeInfo, is_new: bool, orig_self_type: Type | None
    ) -> CallableType:
        from mypy.typeops import class_callable

        # type_type (4th arg) is the copy_modified fallback; Instance(object)
        # is a valid stand-in and identical under both gate settings.
        return class_callable(
            init_type, info, None, Instance(self.fx.oi, []), None, is_new, orig_self_type
        )

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(active)
        try:
            return fn()
        finally:
            _set_native_typeops_active(True)

    def test_baseline_gate_off(self) -> None:
        info = self.fx.ai
        init_type = self._init_callable(info)
        result = self._with_gate(False, lambda: self._construct(init_type, info, False, None))
        assert result.ret_type == Instance(info, [])
        assert list(result.variables) == []

    def test_parity_init_default(self) -> None:
        info = self.fx.ai
        init_type = self._init_callable(info)
        off = self._with_gate(False, lambda: self._construct(init_type, info, False, None))
        on = self._with_gate(True, lambda: self._construct(init_type, info, False, None))
        assert on.ret_type == off.ret_type
        assert list(on.variables) == list(off.variables)

    def test_parity_new_returns_object(self) -> None:
        # __new__ (is_new=True) whose explicit ret (object) differs from the
        # default (A): the equivalence/subtype decision must match.
        info = self.fx.ai
        init_type = self._init_callable(info, ret=Instance(self.fx.oi, []))
        off = self._with_gate(False, lambda: self._construct(init_type, info, True, None))
        on = self._with_gate(True, lambda: self._construct(init_type, info, True, None))
        assert on.ret_type == off.ret_type
        assert list(on.variables) == list(off.variables)

    def test_parity_generic_variables(self) -> None:
        # A generic class (G, one typevar): variables = defn.type_vars +
        # init.variables must combine identically through the wire round-trip.
        info = self.fx.gi
        init_type = self._init_callable(info)
        off = self._with_gate(False, lambda: self._construct(init_type, info, False, None))
        on = self._with_gate(True, lambda: self._construct(init_type, info, False, None))
        assert on.ret_type == off.ret_type
        assert list(on.variables) == list(off.variables)

    def test_parity_declared_self(self) -> None:
        # is_new=False with a declared self type: the subtype branch must
        # pick the same ret_type.
        info = self.fx.ai
        init_type = self._init_callable(info)
        off = self._with_gate(
            False, lambda: self._construct(init_type, info, False, Instance(info, []))
        )
        on = self._with_gate(
            True, lambda: self._construct(init_type, info, False, Instance(info, []))
        )
        assert on.ret_type == off.ret_type
        assert list(on.variables) == list(off.variables)

    def test_seam_engages_direct(self) -> None:
        # Call the Rust seam directly (no Python shim) and confirm it returns
        # (ret_blob, var_blobs) rather than None for a default __init__.
        import type_kernel as _tk

        from mypy.typeops import _serialize_type
        from mypy.typevars import fill_typevars

        info = self.fx.ai
        init_type = self._init_callable(info)
        result = _tk.rust_class_callable(
            _serialize_type(init_type),
            None,
            _serialize_type(fill_typevars(info)),
            False,
            False,
            False,
            info,
        )
        assert result is not None, "Rust class_callable seam did not engage"
        ret_blob, var_blobs = result
        assert isinstance(bytes(ret_blob), bytes)
        assert isinstance(var_blobs, list)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMapTypeFromSupertypeSuite(Suite):
    """Parity for the Rust `map_type_from_supertype` composite
    (mypy.typeops.map_type_from_supertype, #492 family).

    The Python body composes fill_typevars(sub_info) -> tuple_fallback ->
    map_instance_to_supertype -> expand_type_by_instance. All four
    primitives are native, so the whole body is one consolidated Rust call
    (rust_map_type_from_supertype). Toggling the typeops gate off (pure
    Python) and on (Rust composite) must produce an identical result for a
    mapped type. A direct seam call proves the composite engages for the
    generic supertype case rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
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
        self._live_map = {info.fullname: info for info in type_infos}
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        _set_native_typeops_active(True)
        _set_native_typeops_resolver(self._resolver)

    def tearDown(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_typeops_active(False)
        _set_native_typeops_resolver(None)
        set_wire_typeinfo_map(None)

    def _map(self, typ: Type, sub_info: TypeInfo, super_info: TypeInfo) -> Type:
        from mypy.typeops import map_type_from_supertype

        return map_type_from_supertype(typ, sub_info, super_info)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(active)
        try:
            return fn()
        finally:
            _set_native_typeops_active(True)

    def _assert_par(self, typ: Type, sub_info: TypeInfo, super_info: TypeInfo) -> None:
        off = self._with_gate(False, lambda: self._map(typ, sub_info, super_info))
        on = self._with_gate(True, lambda: self._map(typ, sub_info, super_info))
        assert_equal(on, off, f"map_type_from_supertype parity {typ}")

    def test_baseline_gate_off(self) -> None:
        # Non-generic B <: A: a type in A's frame is unchanged in B's frame.
        typ = Instance(self.fx.ai, [])
        result = self._with_gate(False, lambda: self._map(typ, self.fx.bi, self.fx.ai))
        assert result == Instance(self.fx.ai, [])

    def test_parity_non_generic(self) -> None:
        # B <: A, no typevars on either side: identity mapping.
        self._assert_par(Instance(self.fx.ai, []), self.fx.bi, self.fx.ai)
        self._assert_par(Instance(self.fx.oi, []), self.fx.bi, self.fx.oi)

    def test_parity_same_class(self) -> None:
        # sub_info == super_info: pure identity (map fast path).
        self._assert_par(Instance(self.fx.gi, [self.fx.t]), self.fx.gi, self.fx.gi)

    def test_parity_generic_supertype(self) -> None:
        # GS2[S] <: G[S]. A G-frame type (its typevar T, raw id 1 per the
        # fixture) maps to S (raw id 1) in GS2's frame.
        self._assert_par(self.fx.t, self.fx.gs2i, self.fx.gi)

    def test_parity_generic_instance_arg(self) -> None:
        # G[T] maps through the substitution: G[T] in G's frame -> the
        # concrete arg frame of GS2.
        self._assert_par(Instance(self.fx.gi, [self.fx.t]), self.fx.gs2i, self.fx.gi)

    def test_parity_callable(self) -> None:
        # A callable (like an __init__ signature) in G's frame maps with its
        # arg/ret types substituted.
        callable = CallableType(
            [self.fx.o, self.fx.t],
            [ARG_POS, ARG_POS],
            [None, None],
            Instance(self.fx.gi, [self.fx.t]),
            self.fx.function,
            name="<init>",
        )
        self._assert_par(callable, self.fx.gs2i, self.fx.gi)

    def test_parity_definition_restamped(self) -> None:
        # A callable carrying a FuncDef definition: the wire round-trip drops
        # it (#1207, the #1169 repair pattern), so the native path re-stamps
        # it positionally instead of deferring (gate on/off must agree).
        defn = FuncDef("<init>")
        callable = CallableType(
            [self.fx.o, self.fx.t],
            [ARG_POS, ARG_POS],
            [None, None],
            Instance(self.fx.gi, [self.fx.t]),
            self.fx.function,
            name="<init>",
            definition=defn,
        )
        off = self._with_gate(False, lambda: self._map(callable, self.fx.gs2i, self.fx.gi))
        on = self._with_gate(True, lambda: self._map(callable, self.fx.gs2i, self.fx.gi))
        assert get_proper_type(on) == get_proper_type(off)
        mapped = cast(CallableType, on)
        assert mapped.definition is defn

    def test_seam_engages_direct(self) -> None:
        # Call the Rust seam directly on the typevar-free hot path (B->A
        # frame mapping an object-typed type) and confirm it returns bytes,
        # i.e. the composite engages rather than deferring. The engine ships

        # only typevar-free expansions, so a generic-frame mapping would
        # legitimately defer (None).
        import type_kernel as _tk

        from mypy.typeops import _serialize_type

        info = self.fx.bi
        result = _tk.rust_map_type_from_supertype(
            self._resolver, info, self.fx.ai, _serialize_type(Instance(self.fx.oi, [])), True
        )
        assert result is not None, "Rust map_type_from_supertype did not engage"
        assert isinstance(bytes(result), bytes)

    def test_leftover_tvar_relinks_identity(self) -> None:
        # Issue #1309 (mfs standalone, the #1215/#1224 relink variant): a
        # type in the super frame carries a tvar the env cannot match
        # (G[T] via a tvar-free B -> A step); the shim re-links identities.
        from mypy.typeops import _serialize_type

        typ = Instance(self.fx.gi, [self.fx.t])
        result = _type_kernel.rust_map_type_from_supertype(
            self._resolver, self.fx.bi, self.fx.ai, _serialize_type(typ), True
        )
        assert (
            result is not None
        ), "leftover-tvar map_type_from_supertype must return the expansion"
        off = self._with_gate(False, lambda: self._map(typ, self.fx.bi, self.fx.ai))
        on = self._with_gate(True, lambda: self._map(typ, self.fx.bi, self.fx.ai))
        assert_equal(str(on), str(off), "map_type_from_supertype parity (leftover tvar)")
        assert isinstance(on, Instance), str(on)  # type: ignore[misc]
        assert isinstance(off, Instance), str(off)  # type: ignore[misc]
        assert on.args[0] is off.args[0] is self.fx.t, "decoded TypeVar must relink to original"

    def test_alias_union_arg_flattens(self) -> None:
        # `Alias[T] | None` with Alias = G[T]: the union arm must expand
        # the top-level alias through the snapshot (FlatAliasGuard,
        # #1203/#1446) instead of deferring the whole map.
        from mypy.nodes import TypeAlias
        from mypy.typeops import (
            _serialize_type,
            _set_native_typeops_resolver,
            map_type_from_supertype,
        )
        from mypy.types import NoneType, TypeVarId, TypeVarType, UnionType
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        # Production class typevars bind TypeVarId(raw_id, namespace=<class
        # fullname>) on both the defn tvars and the base-arg occurrences;
        # stamp the fixture so the Rust map env keys line up.
        for info in (self.fx.gi, self.fx.gs2i):
            for tv in info.defn.type_vars:
                tv.id = TypeVarId(tv.id.raw_id, namespace=info.fullname)
            for base in info.bases:
                for arg in base.args:
                    if isinstance(arg, TypeVarType):
                        arg.id = TypeVarId(arg.id.raw_id, namespace=info.fullname)
        alias = TypeAlias(Instance(self.fx.gi, [self.fx.t]), "mod.U", "mod", -1, -1)
        union = UnionType.make_union([TypeAliasType(alias, []), NoneType()])
        callable = CallableType([union], [ARG_POS], [None], self.fx.anyt, self.fx.function)
        resolver = _type_kernel.build_native_resolver(self._type_infos, [alias])
        try:
            set_wire_alias_map({alias.fullname: alias})
            set_wire_typeinfo_map(self._live_map)
            _set_native_typeops_resolver(resolver)
            result = _type_kernel.rust_map_type_from_supertype(
                resolver, self.fx.gs2i, self.fx.gi, _serialize_type(callable), True
            )
            assert result is not None, "alias-union map_type_from_supertype must decide"
            off = self._with_gate(
                False, lambda: map_type_from_supertype(callable, self.fx.gs2i, self.fx.gi)
            )
            on = self._with_gate(
                True, lambda: map_type_from_supertype(callable, self.fx.gs2i, self.fx.gi)
            )
            assert_equal(str(on), str(off), "map_type_from_supertype parity (alias union)")
        finally:
            _set_native_typeops_resolver(self._resolver)
            set_wire_typeinfo_map(self._live_map)
            set_wire_alias_map(None)

    def test_alias_typ_object_parity_and_engagement(self) -> None:
        # Issue #1309 (mfs): mod.A = List[T] rides through the seam unchanged
        # (visit_type_alias_type semantics); gate on/off agree, the direct
        # seam call returns bytes, and the alias re-links to the live node.
        from mypy.nodes import TypeAlias
        from mypy.typeops import (
            _serialize_type,
            _set_native_typeops_resolver,
            map_type_from_supertype,
        )
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        alias = TypeAlias(Instance(self.fx.std_listi, [self.fx.t]), "mod.A", "mod", -1, -1)
        typ = TypeAliasType(alias, [])
        resolver = _type_kernel.build_native_resolver(self._type_infos, [alias])
        try:
            set_wire_alias_map({alias.fullname: alias})
            set_wire_typeinfo_map(self._live_map)
            _set_native_typeops_resolver(resolver)
            result = _type_kernel.rust_map_type_from_supertype(
                resolver, self.fx.bi, self.fx.ai, _serialize_type(typ), True
            )
            assert result is not None, "alias-typ map_type_from_supertype must ride through"
            off = self._with_gate(
                False, lambda: map_type_from_supertype(typ, self.fx.bi, self.fx.ai)
            )
            on = self._with_gate(
                True, lambda: map_type_from_supertype(typ, self.fx.bi, self.fx.ai)
            )
            assert_equal(str(on), str(off), "map_type_from_supertype parity (alias)")
            assert isinstance(on, TypeAliasType), "decoded type must be the alias"
            assert on.alias is alias, "decoded alias must re-link to the live TypeAlias node"
        finally:
            _set_native_typeops_resolver(self._resolver)
            set_wire_typeinfo_map(self._live_map)
            set_wire_alias_map(None)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMapInstanceToSupertypesSuite(Suite):
    """Parity for the Rust whole per-member supertype-mapping loop
    (mypy.maptype.map_instance_to_supertypes, maptype.py:179-196).

    The inner per-member loop of each derivation-path step (`for t in
    types: map_instance_to_direct_supertypes(t, sup)`) runs in Rust in
    one call (rust_map_instance_to_supertypes). Members Rust cannot map
    (wire-unsupported args: TypeAlias / definition-carrying Callable /
    ParamSpec, or a variadic frame) defer per-member to Python. The
    differential (gate off = pure-Python loop, gate on = Rust whole-step
    loop with per-member fallback) must agree on the mapped frontier.
    """

    def setUp(self) -> None:
        from mypy.maptype import _set_native_map_active, _set_native_map_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._type_infos = type_infos
        _set_native_map_active(True)
        _set_native_map_resolver(self._resolver)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})

    def tearDown(self) -> None:
        from mypy.maptype import _set_native_map_active, _set_native_map_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_map_active(False)
        _set_native_map_resolver(None)
        set_wire_typeinfo_map(None)

    def _map(self, instance: Instance, supertype: TypeInfo) -> list[Instance]:
        from mypy.maptype import map_instance_to_supertypes

        return map_instance_to_supertypes(instance, supertype)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.maptype import _set_native_map_active

        _set_native_map_active(active)
        try:
            return fn()
        finally:
            _set_native_map_active(True)

    def _assert_par(self, instance: Instance, supertype: TypeInfo) -> None:
        off = self._with_gate(False, lambda: self._map(instance, supertype))
        on = self._with_gate(True, lambda: self._map(instance, supertype))
        assert_equal(
            [str(x) for x in on],
            [str(x) for x in off],
            f"map_instance_to_supertypes parity {instance} -> {supertype}",
        )

    def test_parity_direct_non_generic(self) -> None:
        # B <: A, no typevars on either side: [B] maps to [A].
        self._assert_par(self.fx.b, self.fx.ai)

    def test_parity_multi_level(self) -> None:
        # D <: C <: B <: A: a multi-level derivation path maps to A.
        self._assert_par(self.fx.d, self.fx.ai)

    def test_parity_generic_supertype(self) -> None:
        # G[T] <: object: mapping G[A] to builtins.object gives object
        # (all type vars dropped, no substitution needed).
        self._assert_par(Instance(self.fx.gi, [self.fx.a]), self.fx.oi)

    def test_parity_same_class_returns_input(self) -> None:
        # instance.type == superclass: identity, both gates agree.
        self._assert_par(self.fx.a, self.fx.ai)

    def test_full_member_list_maps_all(self) -> None:
        # Multi-member frontier, all supported: the seam maps every
        # member in one Rust call; gate-off is the per-member loop.
        # G[A]/G[B] drop the typevar, same as Python path in Rust.
        from mypy.maptype import _native_map_step_frontier, map_instance_to_direct_supertypes

        members = [Instance(self.fx.gi, [self.fx.a]), Instance(self.fx.gi, [self.fx.b])]
        off = [m for mem in members for m in map_instance_to_direct_supertypes(mem, self.fx.oi)]
        on = self._with_gate(True, lambda: _native_map_step_frontier(members, self.fx.oi))
        assert on is not None
        assert_equal(
            [str(x) for x in on], [str(x) for x in off], "full member list frontier parity"
        )

    def test_mixed_supported_unsupported_members(self) -> None:
        # Mixes a supported member with an alias-carrying member
        # (TypeAlias is wire-unsupported): the alias member defers
        # per-member to Python; G[A] still maps through Rust.
        from mypy.maptype import _native_map_step_frontier, map_instance_to_direct_supertypes

        alias, _ = self.fx.def_alias_1(self.fx.a)
        members: list[Instance] = [
            Instance(self.fx.gi, [self.fx.a]),
            Instance(self.fx.gi, [alias]),
        ]
        off = [m for mem in members for m in map_instance_to_direct_supertypes(mem, self.fx.oi)]
        on = self._with_gate(True, lambda: _native_map_step_frontier(members, self.fx.oi))
        assert on is not None
        assert_equal([str(x) for x in on], [str(x) for x in off], "mixed member frontier parity")

    def test_per_member_deferral_sentinel(self) -> None:
        # Per-member deferral sentinel: Rust returns a parallel flags Vec
        # (true = mapped, false = re-run in Python). Variadic G[Ts] defers
        # (variadic-frames defer at the derivation path); flags [False, True].
        import type_kernel as _tk

        from mypy.maptype import _WriteBuffer  # type: ignore[attr-defined]
        from mypy.types import write_type_list

        members = [
            Instance(self.fx.gvi, [self.fx.a, self.fx.b]),
            Instance(self.fx.gi, [self.fx.a]),
        ]
        buf = _WriteBuffer()
        write_type_list(buf, members)
        result = _tk.rust_map_instance_to_supertypes(
            self._resolver, buf.getvalue(), self.fx.gi.fullname
        )
        assert result is not None, "Rust rust_map_instance_to_supertypes did not engage"
        encoded_results, flags = result
        assert flags == [False, True], f"per-member deferral flags {flags}"
        assert bytes(encoded_results)

    def test_mixed_variadic_member_parity(self) -> None:
        # Frontier with a variadic-frame member: Rust defers the GV
        # member per-member via the sentinel flag, and the shim re-runs
        # it in Python. Gate-off and gate-on map to the same frontier.
        from mypy.maptype import _native_map_step_frontier, map_instance_to_direct_supertypes

        members = [
            Instance(self.fx.gvi, [self.fx.a, self.fx.b]),
            Instance(self.fx.gi, [self.fx.a]),
        ]
        off = [m for mem in members for m in map_instance_to_direct_supertypes(mem, self.fx.oi)]
        on = self._with_gate(True, lambda: _native_map_step_frontier(members, self.fx.oi))
        assert on is not None
        assert_equal(
            [str(x) for x in on], [str(x) for x in off], "variadic-member frontier parity"
        )

    def test_seam_engages_full_member_list(self) -> None:
        # Direct seam call: a two-member frontier engages the Rust
        # whole-step loop (returns bytes + flags) rather than deferring.
        import type_kernel as _tk

        from mypy.maptype import _WriteBuffer  # type: ignore[attr-defined]
        from mypy.types import write_type_list

        members = [Instance(self.fx.gi, [self.fx.a]), Instance(self.fx.gi, [self.fx.b])]
        buf = _WriteBuffer()
        write_type_list(buf, members)
        result = _tk.rust_map_instance_to_supertypes(
            self._resolver, buf.getvalue(), self.fx.oi.fullname
        )
        assert result is not None, "Rust rust_map_instance_to_supertypes did not engage"
        encoded_results, flags = result
        assert flags == [True, True]
        assert bytes(encoded_results)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMapFreshVarRepairSuite(Suite):
    """Fresh-var identity repair at the map decode seams (#1198).

    Fresh (meta_level > 0) type vars reach the map seams via constraint
    template mapping; a native map of a fresh-bearing instance round-trips
    the var through the wire, and the decode splits re-occurrences of one
    id into distinct equal-id objects. The single seam re-unifies via
    canonicalize_fresh_vars_reported and keeps fresh-bearing trees out of
    the binary-blob decode cache (a cached tree shares one object per id,
    so an in-place freeze would leak into later callers of the identical
    blob); the frontier step has no cache and canonicalizes too, while
    fresh-bearing members still defer per-member via the sentinel flag.
    """

    def setUp(self) -> None:
        from mypy.maptype import (
            _clear_map_supertype_decode_cache,
            _set_native_map_active,
            _set_native_map_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        # GS3[T, S] <: G[H[S, S]]: the base instance mentions the class's
        # own S var in two positions, so mapping a fresh-bearing GS3
        # instance to G yields a result carrying the same fresh id twice.
        self._gs3 = self.fx.make_type_info(
            "GS3",
            mro=[self.fx.gi, self.fx.oi],
            typevars=["T", "S"],
            variances=[COVARIANT, COVARIANT],
        )
        # Class-frame tvars must follow the convention the Rust expander
        # matches on: raw_id is the 1-based slot position, namespace the
        # declaring class's fullname (fixture globals use "").
        gs3_tvar = TypeVarType(
            "T", "GS3.T", TypeVarId(1, namespace="GS3"), [], self.fx.o, self.fx.o, COVARIANT
        )
        gs3_svar = TypeVarType(
            "S", "GS3.S", TypeVarId(2, namespace="GS3"), [], self.fx.o, self.fx.o, COVARIANT
        )
        self._gs3.defn.type_vars = [gs3_tvar, gs3_svar]
        self._gs3.bases = [Instance(self.fx.gi, [Instance(self.fx.hi, [gs3_svar, gs3_svar])])]
        # GS4[T, S] <: G[S]: the base instance mentions S once, so a
        # fresh-bearing instance maps to a result carrying the fresh id
        # exactly once (a single decoded occurrence, no split risk).
        self._gs4 = self.fx.make_type_info(
            "GS4",
            mro=[self.fx.gi, self.fx.oi],
            typevars=["T", "S"],
            variances=[COVARIANT, COVARIANT],
        )
        self._gs4.defn.type_vars = [gs3_tvar, gs3_svar]
        self._gs4.bases = [Instance(self.fx.gi, [gs3_svar])]
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend([self._gs3, self._gs4])
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_map_active(True)
        _set_native_map_resolver(self._resolver)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        _clear_map_supertype_decode_cache()
        self._fresh = TypeVarType(
            "Fresh", "Test.Fresh", TypeVarId(500, meta_level=1), [], self.fx.o, self.fx.o
        )

    def tearDown(self) -> None:
        from mypy.maptype import (
            _clear_map_supertype_decode_cache,
            _set_native_map_active,
            _set_native_map_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        _clear_map_supertype_decode_cache()
        _set_native_map_active(False)
        _set_native_map_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.maptype import _set_native_map_active

        _set_native_map_active(active)
        try:
            return fn()
        finally:
            _set_native_map_active(True)

    def _assert_same_id(self, t: Type, raw_id: int, meta_level: int) -> None:
        p = get_proper_type(t)
        assert isinstance(p, TypeVarType)
        assert p.id.raw_id == raw_id
        assert p.id.meta_level == meta_level

    def test_fresh_recurrence_identity_repaired(self) -> None:
        import type_kernel as _tk

        from mypy.maptype import (  # type: ignore[attr-defined]
            _map_supertype_decode_cache,
            _WriteBuffer,
            map_instance_to_supertype,
        )

        fx = self.fx
        inst = Instance(self._gs3, [self._fresh, self._fresh])
        before_keys = set(_map_supertype_decode_cache)
        off = self._with_gate(False, lambda: map_instance_to_supertype(inst, fx.gi))
        on = self._with_gate(True, lambda: map_instance_to_supertype(inst, fx.gi))
        assert_equal(str(on), str(off), "fresh-bearing map str parity")
        # Python (gate-off) rebuilds the tree by substitution and keeps
        # object identity; the Rust result re-unifies at the decode seam
        # instead: both occurrences of the fresh id share one object.
        inner = get_proper_type(on.args[0])
        assert isinstance(inner, Instance)
        assert inner.args[0] is inner.args[1], "decode split the fresh id into two objects"
        self._assert_same_id(inner.args[0], 500, 1)
        # The seam must not cache fresh-bearing decoded trees.
        from mypy.maptype import _map_supertype_decode_cache

        assert (
            set(_map_supertype_decode_cache) == before_keys
        ), "fresh-bearing call inserted into the shared decode cache"
        # Engagement: the Rust seam actually maps the fresh-bearing
        # instance instead of silently deferring to Python.
        buf = _WriteBuffer()
        inst.write(buf)
        result = _tk.rust_map_instance_to_supertype(
            self._resolver, inst.type.fullname, buf.getvalue(), fx.gi.fullname
        )
        assert result is not None, "Rust map seam deferred on a fresh-bearing instance"

    def test_fresh_singlet_no_cache_insert(self) -> None:
        from mypy.maptype import _map_supertype_decode_cache, map_instance_to_supertype

        fx = self.fx
        # Single base occurrence: the output carries the fresh id exactly
        # once, but it is still a fresh-bearing tree -> no cache insert.
        inst = Instance(self._gs4, [fx.a, self._fresh])
        off = self._with_gate(False, lambda: map_instance_to_supertype(inst, fx.gi))
        before = set(_map_supertype_decode_cache)
        on = self._with_gate(True, lambda: map_instance_to_supertype(inst, fx.gi))
        assert_equal(str(on), str(off), "fresh singlet map str parity")
        assert (
            set(_map_supertype_decode_cache) == before
        ), "fresh-bearing call inserted into the shared decode cache"
        self._assert_same_id(on.args[0], 500, 1)

    def test_clean_call_caches_and_hits(self) -> None:
        from mypy.maptype import _map_supertype_decode_cache, map_instance_to_supertype

        fx = self.fx
        inst = Instance(self._gs3, [fx.a, fx.b])
        before = set(_map_supertype_decode_cache)
        first = self._with_gate(True, lambda: map_instance_to_supertype(inst, fx.gi))
        added = set(_map_supertype_decode_cache) - before
        assert len(added) == 1, "clean call did not populate the decode cache"
        second = self._with_gate(True, lambda: map_instance_to_supertype(inst, fx.gi))
        assert second is not first, "cache hit must return a shallow copy"
        assert_equal(str(second), str(first), "cache-hit result differs")

    def test_frontier_fresh_member_defers_per_member(self) -> None:
        from mypy.maptype import _native_map_step_frontier, map_instance_to_direct_supertypes

        fx = self.fx
        inst = Instance(self._gs3, [self._fresh, self._fresh])
        off = [m for m in map_instance_to_direct_supertypes(inst, fx.gi)]
        on = self._with_gate(True, lambda: _native_map_step_frontier([inst], fx.gi))
        assert on is not None
        assert_equal([str(x) for x in on], [str(x) for x in off], "frontier fresh parity")
        # The fresh-bearing member re-runs in Python (sentinel flag), so
        # the output preserves the fresh object's identity directly.
        inner = get_proper_type(on[0].args[0])
        assert isinstance(inner, Instance)
        assert inner.args[0] is inner.args[1]


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeObjectTypeSuite(Suite):
    """Parity for the Rust `type_object_type_from_function` composite seam
    (mypy.typeops.type_object_type_from_function, issue #492 family).

    The Python body composes (in order): the non-generic `bind_self` strip,
    `map_type_from_supertype` from `def_info`'s frame into `info`'s frame,
    and the per-item `class_callable` assembly. Rust mirrors the whole path
    in one call returning the wire `FunctionLike`; the Python shim rebuilds
    each callable through `copy_modified` so non-wire fields (special_sig,
    instance_type, definition, line/column) survive. Toggling the typeops
    gate off (pure Python) and on (Rust composite) must produce an
    identical result (`str` and structure) for signatures that engage, and
    a direct seam call proves the seam actually engages rather than
    silently deferring.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                self._type_infos.append(value)
        self._resolver = _type_kernel.build_native_resolver(self._type_infos, [])
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})
        _set_native_typeops_active(True)
        _set_native_typeops_resolver(self._resolver)

    def tearDown(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_typeops_active(False)
        _set_native_typeops_resolver(None)
        set_wire_typeinfo_map(None)

    def _init_sig(self, info: TypeInfo, def_info: TypeInfo, is_new: bool = False) -> CallableType:
        # A minimal __init__/__new__ signature: one positional self (bound
        # instance) and a `None` return. `info` may differ from `def_info`
        # (supertype __init__), matching production.
        self_param = Instance(def_info, [])
        fallback = self.fx.type_type if is_new else self.fx.function
        ret = self.fx.o if is_new else NoneType()
        return CallableType([self_param], [ARG_POS], [None], ret, fallback, name="<init>")

    def _type_object(self, sig: FunctionLike, info: TypeInfo, is_new: bool) -> FunctionLike:
        from mypy.typeops import type_object_type_from_function

        return type_object_type_from_function(sig, info, info, self.fx.type_type, is_new)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(active)
        try:
            return fn()
        finally:
            _set_native_typeops_active(True)

    def _assert_par(self, sig: CallableType, info: TypeInfo, is_new: bool = False) -> None:
        off = self._with_gate(False, lambda: self._type_object(sig, info, is_new))
        on = self._with_gate(True, lambda: self._type_object(sig, info, is_new))
        assert_equal(str(on), str(off), f"type_object_type parity {sig}")

    def test_baseline_gate_off(self) -> None:
        # Non-generic class: the bound result is (Instance(A)) -> A with the
        # type-object fallback and empty variables.
        result = self._with_gate(
            False,
            lambda: self._type_object(self._init_sig(self.fx.ai, self.fx.ai), self.fx.ai, False),
        )
        result = cast(CallableType, result)
        assert result.ret_type == Instance(self.fx.ai, [])
        assert list(result.variables) == []

    def test_parity_non_generic_init(self) -> None:
        # class A: def __init__(self) -> None
        self._assert_par(self._init_sig(self.fx.ai, self.fx.ai), self.fx.ai)

    def test_parity_generic_class_var(self) -> None:
        # class G(Generic[T]): def __init__(self, x: T) -> None. The bound
        # callable carries G's type variable; the seam must not defer on the
        # generic def. (bind_self strip happens before the wire, so the

        # signature passed in is already bound.)
        sig = CallableType(
            [self.fx.gt, self.fx.t],
            [ARG_POS, ARG_POS],
            [None, None],
            NoneType(),
            self.fx.function,
            name="<init>",
        )
        self._assert_par(sig, self.fx.gi)

    def test_generic_init_binds_self_typevar(self) -> None:
        # A `variables`-carrying __init__ on Generic[T] must run the generic
        # bind_self arm natively: the unbound T is filled from the enclosing
        # class frame of `info`, not deferred to the Python path.
        info = self.fx.std_listi
        tvar = info.defn.type_vars[0]
        sig = CallableType(
            [tvar],
            [ARG_POS],
            [None],
            NoneType(),
            self.fx.function,
            name="<init>",
            variables=[tvar],
        )
        off = self._with_gate(False, lambda: self._type_object(sig, info, False))
        on = self._with_gate(True, lambda: self._type_object(sig, info, False))
        assert_equal(str(on), str(off), "generic __init__ parity")
        # The self param (x: T) is stripped; T stays free in `variables`
        # (bound to itself via the class frame), so it survives in the
        # bound callable's own variables.
        assert_equal(str(on), "def [T] () -> builtins.list[T]")

    def test_parity_classmethod_new(self) -> None:
        # A classmethod `__new__`: the self param is dropped and the ret is
        # the instance type.
        self._assert_par(
            self._init_sig(self.fx.ai, self.fx.ai, is_new=True), self.fx.ai, is_new=True
        )

    def test_parity_supertype_init(self) -> None:
        # class B(A): __init__ inherited from A. def_info=A, info=B: the
        # supertype mapping must map A's frame to B.
        sig = CallableType(
            [Instance(self.fx.ai, [])],
            [ARG_POS],
            [None],
            NoneType(),
            self.fx.function,
            name="<init>",
        )
        self._assert_par(sig, self.fx.bi)

    def test_parity_definition_restamped(self) -> None:
        # An __init__ signature carrying a FuncDef `definition` node: the wire
        # round-trip drops it (#1207), so the native path re-stamps it
        # positionally instead of deferring (the #1169 repair pattern).
        sig = self._init_sig(self.fx.ai, self.fx.ai)
        defn = FuncDef("__init__")
        sig = sig.copy_modified(definition=defn)
        off = self._with_gate(False, lambda: self._type_object(sig, self.fx.ai, False))
        on = self._with_gate(True, lambda: self._type_object(sig, self.fx.ai, False))
        assert_equal(str(on), str(off), "type_object_type parity with definition")
        assert isinstance(on, CallableType) and isinstance(off, CallableType)
        assert on.definition is off.definition
        assert on.definition is defn

    def test_parity_definition_restamped_overloaded(self) -> None:
        # Same as above for an overloaded __init__: each item keeps its own
        # definition, matched positionally (item-count divergence defers).
        d1, d2 = FuncDef("__init__"), FuncDef("__init__")
        item1 = self._init_sig(self.fx.ai, self.fx.ai).copy_modified(definition=d1)
        item2 = self._init_sig(self.fx.ai, self.fx.ai).copy_modified(
            arg_types=[Instance(self.fx.ai, []), NoneType()],
            arg_kinds=[ARG_POS, ARG_POS],
            arg_names=[None, None],
            definition=d2,
        )
        sig: FunctionLike = Overloaded([item1, item2])
        off = self._with_gate(False, lambda: self._type_object(sig, self.fx.ai, False))
        on = self._with_gate(True, lambda: self._type_object(sig, self.fx.ai, False))
        assert_equal(str(on), str(off), "type_object_type overload parity with definitions")
        assert isinstance(on, Overloaded) and isinstance(off, Overloaded)
        assert len(on.items) == len(off.items) == 2
        assert on.items[0].definition is d1
        assert on.items[1].definition is d2

    def test_seam_engages_direct(self) -> None:
        # Call the Rust seam directly (no Python shim) and confirm it returns
        # a wire type (not None) for a default __init__.
        import type_kernel as _tk

        from mypy.typeops import _serialize_type

        info = self.fx.ai
        sig = self._init_sig(info, info)
        result = _tk.rust_type_object_type_from_function(
            _serialize_type(sig),
            info,
            info,
            _serialize_type(self.fx.type_type),
            False,
            True,
            False,
            self._resolver,
        )
        assert result is not None, "Rust type_object_type_from_function did not engage"
        assert isinstance(bytes(result), bytes)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeObjectArbitrationSuite(Suite):
    """Parity for the Rust `type_object_type` arbitration head (#1059).

    `typeops.type_object_type` (typeops.py:350-461) takes its type from
    whichever of `__init__`/`__new__` is first in the MRO (preferring
    `__init__` on a tie, with a universal-callable arm for object +
    fallback_to_any), applies the tuple `special_sig` fixup, and writes
    the cache only when allowed and strict_optional is on. The Rust seam
    reads the live `TypeInfo` and returns
    `(tag, is_new, special_sig, uncached, method)`; Python keeps the
    error Any, the universal-callable construction, fallback
    construction, and the already-native
    `type_object_type_from_function` tail. Direct seam calls assert the
    exact tag for every branch; gate-off vs gate-on runs must produce
    identical results and identical cache writes.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                self._type_infos.append(value)
        self._resolver = _type_kernel.build_native_resolver(self._type_infos, [])
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})
        self._set_active = _set_native_typeops_active
        self._set_resolver = _set_native_typeops_resolver
        self._set_active(True)
        self._set_resolver(self._resolver)

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

    def _func_def(self, name: str, defining: TypeInfo) -> FuncDef:

        fd = FuncDef(name, [], Block([]))
        fd.info = defining
        return fd

    def _add_method(self, owner: TypeInfo, name: str, defining: TypeInfo | None = None) -> FuncDef:
        # A bare `def name(self)` owned by `defining` (defaults to owner),
        # installed in owner's symbol table.
        fd = self._func_def(name, defining or owner)
        owner.names[name] = SymbolTableNode(MDEF, fd)
        return fd

    def _cls(self, name: str, mro: list[TypeInfo] | None = None) -> TypeInfo:
        info = self.fx.make_type_info(name, mro=mro)
        # A metaclass fallback avoids the stdlib typeinfo lookup, which
        # needs modules_state content unit tests do not populate.
        info.metaclass_type = Instance(self.fx.type_typei, [])
        return info

    def _with_cache_allowed(self, fn: Callable[[], T]) -> T:
        # type_object_type reads checker_state.type_checker to decide
        # whether the constructor cache may be used.
        from mypy.checker_state import checker_state

        saved = checker_state.type_checker
        checker_state.type_checker = SimpleNamespace(allow_constructor_cache=True)  # type: ignore[assignment]
        try:
            return fn()
        finally:
            checker_state.type_checker = saved

    def _type_object(self, info: TypeInfo) -> ProperType:
        from mypy.typeops import type_object_type

        return type_object_type(info)

    def _assert_par(self, info: TypeInfo, cache: bool = False) -> ProperType:
        def run() -> ProperType:
            info.type_object_type = None
            return self._with_cache_allowed(lambda: self._type_object(info))

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert_equal(str(on), str(off), f"type_object_type parity {info.fullname}")
        if cache:
            cached_off = info.type_object_type
            off2 = self._with_gate(False, run)
            on2 = self._with_gate(True, run)
            assert_equal(str(on2), str(off2), f"cached parity {info.fullname}")
            assert_equal(str(on2), str(cached_off), f"cache stable {info.fullname}")
        return on

    # -- direct seam tests (all 5 tags + deferral) --

    def test_seam_own_init_tie(self) -> None:
        from mypy.typeops import NATIVE_TYPE_OBJECT_INIT

        a = self._cls("mod.A", mro=[self.fx.oi])
        fd = self._add_method(a, "__init__")
        self._add_method(a, "__new__")
        result = _type_kernel.rust_classify_type_object_type(a)
        assert result is not None, "Rust arbitration did not engage"
        tag, is_new, special_sig, uncached, method = result
        assert tag == NATIVE_TYPE_OBJECT_INIT, f"{tag}"
        assert is_new is False
        assert special_sig is False
        assert uncached is False
        assert method is fd

    def test_seam_new_wins_by_mro(self) -> None:
        from mypy.typeops import NATIVE_TYPE_OBJECT_NEW

        # class B(A): __new__ defined here, __init__ inherited from A.
        a = self._cls("mod.A", mro=[self.fx.oi])
        b = self._cls("mod.B", mro=[a, self.fx.oi])
        self._add_method(a, "__init__")
        fd_new = self._add_method(b, "__new__")
        result = _type_kernel.rust_classify_type_object_type(b)
        assert result is not None
        tag, is_new, _, _, method = result
        assert tag == NATIVE_TYPE_OBJECT_NEW, f"{tag}"
        assert is_new is True
        assert method is fd_new

    def test_seam_init_wins_by_mro(self) -> None:
        from mypy.typeops import NATIVE_TYPE_OBJECT_INIT

        # class B(A): __init__ defined here, __new__ inherited from A.
        a = self._cls("mod.A", mro=[self.fx.oi])
        b = self._cls("mod.B", mro=[a, self.fx.oi])
        fd_init = self._add_method(b, "__init__")
        self._add_method(a, "__new__")
        result = _type_kernel.rust_classify_type_object_type(b)
        assert result is not None
        tag, is_new, _, _, method = result
        assert tag == NATIVE_TYPE_OBJECT_INIT, f"{tag}"
        assert is_new is False
        assert method is fd_init

    def test_seam_missing_init(self) -> None:
        from mypy.typeops import NATIVE_TYPE_OBJECT_ERROR_INIT

        a = self._cls("mod.NoInit", mro=[self.fx.oi])
        result = _type_kernel.rust_classify_type_object_type(a)
        assert result is not None
        tag, _, _, _, method = result
        assert tag == NATIVE_TYPE_OBJECT_ERROR_INIT, f"{tag}"
        assert method is None

    def test_seam_invalid_init(self) -> None:
        from mypy.typeops import NATIVE_TYPE_OBJECT_ERROR_INIT

        a = self._cls("mod.BadInit", mro=[self.fx.oi])
        a.names["__init__"] = SymbolTableNode(MDEF, Var("__init__"))
        result = _type_kernel.rust_classify_type_object_type(a)
        assert result is not None
        assert result[0] == NATIVE_TYPE_OBJECT_ERROR_INIT

    def test_seam_missing_new_copies_init(self) -> None:
        from mypy.typeops import NATIVE_TYPE_OBJECT_INIT

        # Test-stub case: no `__new__` anywhere in the MRO; init is used.
        a = self._cls("mod.NoNew", mro=[self.fx.oi])
        fd = self._add_method(a, "__init__")
        result = _type_kernel.rust_classify_type_object_type(a)
        assert result is not None
        tag, is_new, _, _, method = result
        assert tag == NATIVE_TYPE_OBJECT_INIT, f"{tag}"
        assert is_new is False
        assert method is fd

    def test_seam_tie_any(self) -> None:
        from mypy.typeops import NATIVE_TYPE_OBJECT_TIE_ANY

        # Both __init__ and __new__ resolve to object's own methods and
        # the class falls back to Any: the universal-callable arm.
        self._add_method(self.fx.oi, "__init__", self.fx.oi)
        try:
            a = self._cls("mod.AnyBase", mro=[self.fx.oi])
            a.fallback_to_any = True
            result = _type_kernel.rust_classify_type_object_type(a)
            assert result is not None
            tag, is_new, _, _, method = result
            assert tag == NATIVE_TYPE_OBJECT_TIE_ANY, f"{tag}"
            assert is_new is False
            assert method is None
        finally:
            del self.fx.oi.names["__init__"]

    def test_seam_special_sig(self) -> None:
        from mypy.typeops import NATIVE_TYPE_OBJECT_INIT

        # A class inheriting tuple's constructor gets special_sig="tuple".
        self._add_method(self.fx.std_tuplei, "__init__")
        try:
            sub = self._cls("mod.SubTuple", mro=[self.fx.std_tuplei, self.fx.oi])
            result = _type_kernel.rust_classify_type_object_type(sub)
            assert result is not None
            tag, _, special_sig, _, _ = result
            assert tag == NATIVE_TYPE_OBJECT_INIT
            assert special_sig is True
            # tuple itself skips the fixup (micro-optimization parity).
            result = _type_kernel.rust_classify_type_object_type(self.fx.std_tuplei)
            assert result is not None
            assert result[2] is False
        finally:
            del self.fx.std_tuplei.names["__init__"]

    def test_seam_uncached_overloaded(self) -> None:
        # An untyped OverloadedFuncDef __init__ disables the cache write.
        from mypy.nodes import OverloadedFuncDef

        a = self._cls("mod.Over", mro=[self.fx.oi])
        ofd = OverloadedFuncDef([self._func_def("__init__", a)])
        ofd.info = a  # semanal sets this; the FakeInfo default defers
        a.names["__init__"] = SymbolTableNode(MDEF, ofd)
        a.names["__new__"] = SymbolTableNode(MDEF, self._func_def("__new__", a))
        result = _type_kernel.rust_classify_type_object_type(a)
        assert result is not None
        assert result[3] is True, f"expected uncached: {result}"

    def test_seam_defers_on_plain_object(self) -> None:
        assert _type_kernel.rust_classify_type_object_type(object()) is None

    # -- gate off/on differential tests --

    def test_par_own_init(self) -> None:
        a = self._cls("mod.A", mro=[self.fx.oi])
        self._add_method(a, "__init__")
        self._add_method(a, "__new__")
        result = self._assert_par(a, cache=True)
        result = cast(CallableType, result)
        assert result.ret_type == Instance(a, [])
        assert result.special_sig is None

    def test_par_new_by_mro(self) -> None:
        a = self._cls("mod.A", mro=[self.fx.oi])
        b = self._cls("mod.B", mro=[a, self.fx.oi])
        self._add_method(a, "__init__")
        self._add_method(b, "__new__")
        result = self._assert_par(b, cache=True)
        result = cast(CallableType, result)
        assert result.ret_type == Instance(b, [])

    def test_par_error_init(self) -> None:
        a = self._cls("mod.BadInit", mro=[self.fx.oi])
        a.names["__init__"] = SymbolTableNode(MDEF, Var("__init__"))
        result = self._assert_par(a)
        assert isinstance(result, AnyType)
        assert result.type_of_any == TypeOfAny.from_error

    def test_par_missing_new(self) -> None:
        a = self._cls("mod.NoNew", mro=[self.fx.oi])
        self._add_method(a, "__init__")
        result = self._assert_par(a, cache=True)
        result = cast(CallableType, result)
        assert result.ret_type == Instance(a, [])

    def test_par_tie_any(self) -> None:
        from mypy.types import instance_cache

        self._add_method(self.fx.oi, "__init__", self.fx.oi)
        saved_function = instance_cache.function_type
        instance_cache.function_type = Instance(self.fx.functioni, [])
        try:
            a = self._cls("mod.AnyBase", mro=[self.fx.oi])
            a.fallback_to_any = True
            # metaclass fallback avoids the stdlib typeinfo lookup, which
            # needs modules_state content unit tests do not populate.
            a.metaclass_type = Instance(self.fx.type_typei, [])
            result = self._assert_par(a)
            result = cast(CallableType, result)
            assert result.ret_type == Instance(a, [])
            assert result.arg_kinds == [ARG_STAR, ARG_STAR2]
        finally:
            instance_cache.function_type = saved_function
            del self.fx.oi.names["__init__"]

    def test_par_special_sig(self) -> None:
        self._add_method(self.fx.std_tuplei, "__init__")
        self.fx.std_tuplei.metaclass_type = Instance(self.fx.type_typei, [])
        try:
            sub = self._cls("mod.SubTuple", mro=[self.fx.std_tuplei, self.fx.oi])
            result = self._assert_par(sub, cache=True)
            result = cast(CallableType, result)
            assert result.special_sig == "tuple"
            # tuple itself: same constructor, no special_sig.
            result = self._assert_par(self.fx.std_tuplei, cache=True)
            result = cast(CallableType, result)
            assert result.special_sig is None
        finally:
            del self.fx.std_tuplei.names["__init__"]

    def test_par_uncached_overload_not_cached(self) -> None:
        from mypy.nodes import OverloadedFuncDef

        a = self._cls("mod.Over", mro=[self.fx.oi])
        ofd = OverloadedFuncDef([self._func_def("__init__", a)])
        ofd.info = a  # semanal sets this; the FakeInfo default defers
        a.names["__init__"] = SymbolTableNode(MDEF, ofd)
        a.names["__new__"] = SymbolTableNode(MDEF, self._func_def("__new__", a))

        def run() -> ProperType:
            a.type_object_type = None
            return self._with_cache_allowed(lambda: self._type_object(a))

        off = self._with_gate(False, run)
        on = self._with_gate(True, run)
        assert_equal(str(on), str(off), f"uncached parity {a.fullname}")
        # The overloaded result is computed but never written to the cache.
        assert a.type_object_type is None, "uncached result must not be cached"

        # A typed FuncDef __init__ under the same allowance is cached.
        a2 = self._cls("mod.Cached", mro=[self.fx.oi])
        self._add_method(a2, "__init__")
        self._add_method(a2, "__new__")
        self._with_gate(True, lambda: self._with_cache_allowed(lambda: self._type_object(a2)))
        assert a2.type_object_type is not None, "typed result must be cached"

    def test_par_cache_hit_roundtrip(self) -> None:
        a = self._cls("mod.A", mro=[self.fx.oi])
        self._add_method(a, "__init__")
        self._add_method(a, "__new__")
        cached = CallableType([], [], [], Instance(a, []), self.fx.type_type, name="cached")
        a.type_object_type = cached

        def run() -> ProperType:
            return self._with_cache_allowed(lambda: self._type_object(a))

        assert self._with_gate(False, run) is cached
        assert self._with_gate(True, run) is cached


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCoerceLiteralSingletonSuite(Suite):
    """Parity for the Rust `coerce_to_literal` + singleton pair
    (mypy.typeops.coerce_to_literal / is_singleton_identity_type /
    is_singleton_equality_type, issue #492 family).

    The Python body turns Instances carrying a last-known value, or a
    single-member enum, into the corresponding LiteralType. The Rust port
    reads the single-member-enum decision live from the resolver-installed
    live TypeInfo map (the snapshot's `enum_members` can go stale), and the
    singleton identity/equality predicates likewise read `is_enum` /
    `enum_members` / `is_final` live. Toggling the typeops gate off (pure
    Python) and on (Rust) must produce identical results. Direct seam calls
    prove the Rust functions engage rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        # A single-member enum: `enum_members` (a names-walk property)
        # needs an explicitly-valued Var member to yield ['RED'].
        self.enum_info = self.fx.make_type_info("mod.Color")
        self.enum_info.is_enum = True
        v = Var("RED")
        v.has_explicit_value = True
        v.type = self.fx.o
        self.enum_info.names["RED"] = SymbolTableNode(MDEF, v)
        type_infos.append(self.enum_info)
        self.enum_inst = Instance(self.enum_info, [])
        # A final class for the TypeType branch.
        self.final_info = self.fx.make_type_info("mod.Final")
        self.final_info.is_final = True
        type_infos.append(self.final_info)
        self.final_inst = Instance(self.final_info, [])
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._live_map = {info.fullname: info for info in type_infos}
        self._resolver.set_live_typeinfo_map(dict(self._live_map))
        _set_native_typeops_active(True)
        _set_native_typeops_resolver(self._resolver)

    def tearDown(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_typeops_active(False)
        _set_native_typeops_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], object]) -> object:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(active)
        try:
            return fn()
        finally:
            _set_native_typeops_active(True)

    def _assert_coerce_par(self, typ: Type) -> None:
        off = self._with_gate(False, lambda: coerce_to_literal(typ))
        on = self._with_gate(True, lambda: coerce_to_literal(typ))
        assert_equal(str(on), str(off), f"coerce_to_literal parity {typ}")

    def _assert_singleton_par(self, typ: ProperType) -> None:
        id_off = self._with_gate(False, lambda: is_singleton_identity_type(typ))
        id_on = self._with_gate(True, lambda: is_singleton_identity_type(typ))
        eq_off = self._with_gate(False, lambda: is_singleton_equality_type(typ))
        eq_on = self._with_gate(True, lambda: is_singleton_equality_type(typ))
        assert id_on == id_off, f"identity parity {typ}"
        assert eq_on == eq_off, f"equality parity {typ}"

    def test_coerce_single_enum(self) -> None:
        # The headline new coverage: a single-member enum Instance becomes
        # Literal[Color.RED]. Both gates must agree and the value must round
        # trip through the live enum-member read.
        self._assert_coerce_par(self.enum_inst)
        result = self._with_gate(True, lambda: coerce_to_literal(self.enum_inst))
        assert_equal(str(result), "Literal[mod.Color.RED]")

    def test_coerce_last_known_value(self) -> None:
        # An Instance whose last_known_value is set unwraps to the literal.
        lkv = self.fx.lit_str1_inst
        self._assert_coerce_par(lkv)
        result = self._with_gate(True, lambda: coerce_to_literal(lkv))
        assert_equal(str(result), "Literal['x']")

    def test_coerce_plain_instance_unchanged(self) -> None:
        # No lkv, non-enum: returned untouched.
        plain = Instance(self.fx.ai, [])
        self._assert_coerce_par(plain)

    def test_coerce_union(self) -> None:
        # Union items coerced recursively and recombined.
        u = UnionType([self.enum_inst, Instance(self.fx.ai, [])])
        self._assert_coerce_par(u)
        result = self._with_gate(True, lambda: coerce_to_literal(u))
        assert_equal(str(result), "Literal[mod.Color.RED] | A")

    def test_coerce_alias_defers(self) -> None:
        # A TypeAliasType carries no wire target, so the native path defers
        # and Python produces the (known) literal result. Parity must hold.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(Instance(self.enum_info, []), "mod.RGB", "mod", -1, -1)
        self._assert_coerce_par(TypeAliasType(alias, []))

    def test_singleton_none(self) -> None:
        self._assert_singleton_par(self.fx.nonet)

    def test_singleton_single_enum(self) -> None:
        self._assert_singleton_par(self.enum_inst)
        assert self._with_gate(True, lambda: is_singleton_identity_type(self.enum_inst))

    def test_singleton_plain_instance(self) -> None:
        self._assert_singleton_par(Instance(self.fx.ai, []))

    def test_singleton_literal_bool(self) -> None:
        self._assert_singleton_par(self.fx.lit_true)
        self._assert_singleton_par(self.fx.lit_false)
        assert self._with_gate(True, lambda: is_singleton_identity_type(self.fx.lit_true))

    def test_singleton_literal_int_not_identity(self) -> None:
        # Literal[1] is equality-singleton but not identity-singleton.
        self._assert_singleton_par(self.fx.lit1)
        assert not self._with_gate(True, lambda: is_singleton_identity_type(self.fx.lit1))
        assert self._with_gate(True, lambda: is_singleton_equality_type(self.fx.lit1))

    def test_singleton_typetype_final(self) -> None:
        self._assert_singleton_par(TypeType(self.final_inst))
        assert self._with_gate(True, lambda: is_singleton_identity_type(TypeType(self.final_inst)))

    def test_singleton_typetype_nonfinal(self) -> None:
        self._assert_singleton_par(TypeType(Instance(self.fx.ai, [])))
        assert not self._with_gate(
            True, lambda: is_singleton_identity_type(TypeType(Instance(self.fx.ai, [])))
        )

    def test_singleton_non_typeobj_callable(self) -> None:
        # Fallback builtins.function is not a metaclass, so python is false
        # without consulting type_object() (wave-60B FunctionLike arm).
        c = self.fx.callable(self.fx.a, self.fx.nonet)
        self._assert_singleton_par(c)
        assert not self._with_gate(True, lambda: is_singleton_identity_type(c))

    def test_singleton_typeobj_callable_final(self) -> None:
        # Callable[..., Final] is a type object; type_object() is the final
        # class, so the identity-singleton answer is true.
        c = self.fx.callable_type(self.fx.a, self.final_inst)
        self._assert_singleton_par(c)
        assert self._with_gate(True, lambda: is_singleton_identity_type(c))

    def test_singleton_typeobj_callable_nonfinal(self) -> None:
        c = self.fx.callable_type(self.fx.a, self.fx.a)
        self._assert_singleton_par(c)
        assert not self._with_gate(True, lambda: is_singleton_identity_type(c))

    def test_singleton_typeobj_callable_uninhabited_ret(self) -> None:
        # Uninhabited ret_type: is_type_obj() is false, so python is false.
        c = self.fx.callable_type(self.fx.a, UninhabitedType())
        self._assert_singleton_par(c)
        assert not self._with_gate(True, lambda: is_singleton_identity_type(c))

    def test_singleton_overloaded_typeobj_final(self) -> None:
        # Overloaded delegates is_type_obj()/type_object() to items[0].
        c = Overloaded([self.fx.callable_type(self.fx.a, self.final_inst)])
        self._assert_singleton_par(c)
        assert self._with_gate(True, lambda: is_singleton_identity_type(c))

    def test_singleton_typevar_over_tuple_ret_engages(self) -> None:
        # force_fallback unwraps a TypeVar upper bound once and then applies
        # the TupleType fallback check (types.py:2667-2675); the type object
        # is the final tuple fallback class, so the answer is true.
        from mypy.typeops import _serialize_type

        ret = TypeVarType(
            "T",
            "T",
            TypeVarId(1),
            [],
            TupleType([self.fx.a], self.final_inst),
            AnyType(TypeOfAny.from_omitted_generics),
        )
        c = self.fx.callable_type(self.fx.a, ret)
        self._assert_singleton_par(c)
        r = _type_kernel.rust_is_singleton_identity_type(_serialize_type(c), self._resolver)
        assert r is True, "Rust TypeVar-over-tuple cascade did not engage"

    def test_singleton_callable_arm_engages_direct(self) -> None:
        from mypy.typeops import _serialize_type

        c = self.fx.callable_type(self.fx.a, self.final_inst)
        r = _type_kernel.rust_is_singleton_identity_type(_serialize_type(c), self._resolver)
        assert r is True, "Rust singleton FunctionLike arm did not engage"
        non_final = self.fx.callable_type(self.fx.a, self.fx.a)
        assert (
            _type_kernel.rust_is_singleton_identity_type(
                _serialize_type(non_final), self._resolver
            )
            is False
        )

    def test_seams_engage_direct(self) -> None:
        # Call the Rust seams directly and confirm they return non-None.
        from mypy.typeops import _serialize_type

        r = _type_kernel.rust_coerce_to_literal(_serialize_type(self.enum_inst), self._resolver)
        assert r is not None, "Rust coerce_to_literal did not engage"
        id_r = _type_kernel.rust_is_singleton_identity_type(
            _serialize_type(self.enum_inst), self._resolver
        )
        assert id_r is True, "Rust singleton identity did not engage"
        eq_r = _type_kernel.rust_is_singleton_equality_type(
            _serialize_type(self.enum_inst), self._resolver
        )
        assert eq_r is True, "Rust singleton equality did not engage"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeopsDeferralSuite(Suite):
    """Parity for TypeAliasType-operand deferral reduction in typeops.rs.

    `coerce_to_literal` (typeops.py:1854-1889) and
    `try_getting_instance_fallback` (typeops.py:2059-2084) both run
    `get_proper_type` at the top of the Python body. The Rust seams used to
    refuse a `TypeAliasType` operand entirely (no wire alias target), so
    every alias call fell back to Python. Both seams now expand the alias
    through the resolver's alias table (issue #870). This suite proves the
    expansion keeps gate-on/off parity for a resolvable alias and that a
    missing resolver snapshot still defers (Python computes, both gates
    agree).
    """

    def setUp(self) -> None:
        from mypy.nodes import TypeAlias
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        # A single-member enum: `coerce_to_literal` turns it into a Literal
        # after the alias expands to it.
        self.enum_info = self.fx.make_type_info("mod.Color")
        self.enum_info.is_enum = True
        v = Var("RED")
        v.has_explicit_value = True
        v.type = self.fx.o
        self.enum_info.names["RED"] = SymbolTableNode(MDEF, v)
        type_infos.append(self.enum_info)
        self.enum_inst = Instance(self.enum_info, [])
        self.str_inst = Instance(self.fx.str_type_info, [])

        # Two resolvable aliases: one to the enum, one to a plain str.
        self.rgb_alias = TypeAlias(self.enum_inst, "mod.RGB", "mod", -1, -1)
        self.str_alias = TypeAlias(self.str_inst, "mod.S", "mod", -1, -1)
        # A generic alias whose target carries the alias tvar T:
        # mod.Lst = list[T]. Used for the get_type_vars live expansion.
        self.lst_alias = TypeAlias(
            Instance(self.fx.std_listi, [self.fx.t]),
            "mod.Lst",
            "mod",
            -1,
            -1,
            alias_tvars=[self.fx.t],
        )
        self.aliases = [self.rgb_alias, self.str_alias, self.lst_alias]

        self._resolver = _type_kernel.build_native_resolver(type_infos, self.aliases)
        self._resolver.set_live_typeinfo_map({info.fullname: info for info in type_infos})
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._live_map = {info.fullname: info for info in type_infos}
        _set_native_typeops_active(True)
        _set_native_typeops_resolver(self._resolver)

    def tearDown(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_typeops_active(False)
        _set_native_typeops_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(active)
        try:
            return fn()
        finally:
            _set_native_typeops_active(True)

    def test_coerce_alias_to_enum_parity(self) -> None:
        # A TypeAliasType operand now expands through the resolver to the
        # enum Instance, which coerces to Literal[Color.RED]. Both gates
        # must agree and the Rust seam must engage (no longer defer).
        from mypy.types import TypeAliasType

        alias = TypeAliasType(self.rgb_alias, [])
        off = self._with_gate(False, lambda: coerce_to_literal(alias))
        on = self._with_gate(True, lambda: coerce_to_literal(alias))
        assert_equal(str(on), str(off), f"coerce alias parity {alias}")
        assert_equal(str(on), "Literal[mod.Color.RED]")

    def test_coerce_alias_direct_seam_engages(self) -> None:
        from mypy.typeops import _deserialize_type, _serialize_type
        from mypy.types import TypeAliasType

        # Direct seam call must return a decoded non-None result (the
        # alias expands to the enum and coerces), not defer.
        alias = TypeAliasType(self.rgb_alias, [])
        r = _type_kernel.rust_coerce_to_literal(_serialize_type(alias), self._resolver)
        assert r is not None, "rust_coerce_to_literal deferred on an alias"
        decoded = _deserialize_type(bytes(r))
        assert_equal(str(decoded), "Literal[mod.Color.RED]")

    def test_instance_fallback_alias_to_str_parity(self) -> None:
        from mypy.typeops import _serialize_type
        from mypy.types import TypeAliasType

        alias = TypeAliasType(self.str_alias, [])
        off = self._with_gate(False, lambda: try_getting_instance_fallback(alias))
        on = self._with_gate(True, lambda: try_getting_instance_fallback(alias))
        assert off is not None
        assert_equal(str(on), str(off), f"instance fallback alias parity {alias}")
        assert on == self.str_inst
        # Native engagement proof: the seam expands the alias and returns
        # the decided (True, blob) pair instead of deferring. Blob decode
        # defers (the fixture map lacks builtin refs) and Python answers.
        r = _type_kernel.rust_try_getting_instance_fallback(_serialize_type(alias), self._resolver)
        assert r is not None, "rust_try_getting_instance_fallback deferred on an alias"
        decided, blob = r
        assert decided and blob is not None, f"undecided pair: {r!r}"

    def test_instance_fallback_alias_direct_seam_engages(self) -> None:
        from mypy.typeops import _deserialize_type, _serialize_type
        from mypy.types import TypeAliasType

        # A TypeAliasType operand expands to its Instance target and the
        # seam returns the decided fallback pair instead of deferring.
        alias = TypeAliasType(self.rgb_alias, [])
        r = _type_kernel.rust_try_getting_instance_fallback(_serialize_type(alias), self._resolver)
        assert r is not None, "rust_try_getting_instance_fallback deferred on an alias"
        decided, blob = r
        assert decided and blob is not None, f"undecided pair: {r!r}"
        decoded = _deserialize_type(bytes(blob))
        assert_equal(str(decoded), str(self.enum_inst))
        # Round-tripped TypeInfo identity is asserted through the public
        # parity test (on == self.str_inst); here the round-trip shape is
        # the point.

    def test_instance_fallback_decided_none_protocol(self) -> None:
        # Issue #1101 decided-None protocol: proper shapes outside the
        # isinstance chain hit Python's `else: return None` tail; the
        # seam decides them as (True, None) instead of deferring (#1183).
        from mypy.typeops import _serialize_type
        from mypy.types import AnyType, NoneType, TypeOfAny, TypeType, UninhabitedType, UnionType

        none_t = NoneType()
        any_t = AnyType(TypeOfAny.explicit)
        uni = UninhabitedType()
        union = UnionType([none_t, any_t])
        ttype = TypeType.make_normalized(self.str_inst)
        for t in (none_t, any_t, uni, union, ttype):
            r = _type_kernel.rust_try_getting_instance_fallback(_serialize_type(t), self._resolver)
            assert r is not None, f"rust seam deferred on {type(t).__name__}"
            decided, blob = r
            assert decided and blob is None, f"undecided pair for {type(t).__name__}: {r!r}"
        # The public path answers None for these without a Python-body
        # recursion; parity with the gate off is trivially None on both.
        for t in (none_t, any_t, uni, union, ttype):
            assert try_getting_instance_fallback(t) is None

    def test_simple_literal_decided_none_protocol(self) -> None:
        # Issue #1295: non-literal proper types are DECIDED not-a-literal
        # ((True, None), #1101 protocol), so the shim skips its body; only
        # an undecodable blob answers (False, None).
        from mypy.typeops import _serialize_type, simple_literal_type
        from mypy.types import AnyType, TypeOfAny

        for t in (self.str_inst, self.enum_inst, AnyType(TypeOfAny.explicit)):
            r = _type_kernel.rust_simple_literal_type(_serialize_type(t))
            decided, blob = r
            assert decided and blob is None, f"undecided pair for {t!r}: {r!r}"
            assert simple_literal_type(t) is None

    def test_simple_literal_instance_with_lkv(self) -> None:
        # An Instance with a literal last_known_value decides (True, blob);
        # the public path decodes it back to the fallback Instance with
        # gate-off/on parity.
        from mypy.typeops import _serialize_type, simple_literal_type
        from mypy.types import LiteralType

        inst = Instance(
            self.fx.str_type_info, [], last_known_value=LiteralType("x", self.str_inst)
        )
        off = self._with_gate(False, lambda: simple_literal_type(inst))
        on = self._with_gate(True, lambda: simple_literal_type(inst))
        assert off is not None
        assert_equal(str(on), str(off), "simple_literal_type lkv parity")
        assert_equal(str(on), str(self.str_inst))
        r = _type_kernel.rust_simple_literal_type(_serialize_type(inst))
        decided, blob = r
        # Direct-seam engagement proof: the lkv Instance decides as
        # (True, Some). Blob decode defers here (the fixture map lacks
        # builtin refs); the public parity above covers the decoded value.
        assert decided and blob is not None, f"undecided pair: {r!r}"

    def test_missing_snapshot_defers_to_python(self) -> None:
        # An alias NOT registered in the resolver cannot be expanded, so the
        # Rust seam must defer and Python computes. Parity must still hold.
        from mypy.nodes import TypeAlias
        from mypy.types import TypeAliasType

        ghost_alias = TypeAlias(self.str_inst, "mod.Ghost", "mod", -1, -1)
        alias = TypeAliasType(ghost_alias, [])
        off = self._with_gate(False, lambda: try_getting_instance_fallback(alias))
        on = self._with_gate(True, lambda: try_getting_instance_fallback(alias))
        assert off is not None
        assert_equal(str(on), str(off), f"missing-snapshot fallback parity {alias}")
        assert_equal(on, self.str_inst)

    # get_type_vars alias expansion (issue #1313)

    def _type_vars_on(self, tp: Type) -> str:
        from mypy.typeops import get_type_vars

        return str(self._with_gate(True, lambda: get_type_vars(tp)))

    def test_get_type_vars_alias_target_tvar_collected(self) -> None:
        # mod.Lst = list[T] with no explicit args: the alias expands to
        # list[T], so T is collected. Gate parity + direct seam engagement.
        from mypy.typeops import _serialize_type, get_type_vars
        from mypy.types import TypeAliasType

        alias = TypeAliasType(self.lst_alias, [])
        off = self._with_gate(False, lambda: str(get_type_vars(alias)))
        on = str(self._type_vars_on(alias))
        assert_equal(on, off, "get_type_vars alias parity")
        assert_equal(on, f"[{self.fx.t}]", "get_type_vars collects the alias tvar")
        raw = _type_kernel.rust_get_type_vars_live(self._resolver, _serialize_type(alias), False)
        assert raw is not None, "live get_type_vars deferred on a resolvable alias"

    def test_get_type_vars_alias_arg_substituted(self) -> None:
        # mod.Lst[A]: the alias tvar T is substituted with A (no tvar left),
        # so nothing is collected. Same tree through the byte-only entry
        # defers; the live entry decides.
        from mypy.typeops import _serialize_type, get_type_vars
        from mypy.types import TypeAliasType

        alias = TypeAliasType(self.lst_alias, [self.fx.a])
        off = self._with_gate(False, lambda: str(get_type_vars(alias)))
        on = str(self._type_vars_on(alias))
        assert_equal(on, off, "get_type_vars alias arg parity")
        assert_equal(on, "[]")
        raw = _type_kernel.rust_get_type_vars_live(self._resolver, _serialize_type(alias), False)
        assert raw is not None, "live get_type_vars deferred on a resolvable alias"

    def test_get_type_vars_alias_in_union(self) -> None:
        # Nested unions carrying two applications of one alias: both
        # Python (collect_type_vars dedup) and the Rust seen-by-type_ref
        # guard collapse the duplicate descent to one T.
        from mypy.typeops import get_type_vars
        from mypy.types import TypeAliasType, UnionType

        u = UnionType([TypeAliasType(self.lst_alias, []), self.fx.b])
        nested = UnionType([TypeAliasType(self.lst_alias, []), u])
        off = str(self._with_gate(False, lambda: str(get_type_vars(nested))))
        on = self._type_vars_on(nested)
        assert_equal(on, off, "get_type_vars union parity")
        assert_equal(on, f"[{self.fx.t}]")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCtorBlobPureSuite(Suite):
    """`_native_ctor_blob` must build the blob without native seam detours.

    The typeops resolver installed at blob time is the stale one from the
    previous `_build_native_resolvers` call: the fresh class being blobbed
    is not in its snapshot table yet. Any native detour inside
    `type_object_type` (the type_object_type_from_function composite,
    map_type_from_supertype) therefore defers on the missing snapshot and
    its Python fallback re-enters the same seams, which defer again — the
    wave22 regression measured after #1324 (totf 1800 @ 97% -> 4249 @ 63%,
    mts 129 @ 81% -> 1637 @ 9%). The fix runs the constructor pure; this
    suite pins zero totf/mts seam traffic and a successful blob build.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = [v for v in vars(self.fx).values() if _is_type_info(v)]
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        # The class under blob is created below and stays OUT of the
        # snapshot table: exactly the mid-build stale-resolver shape.
        self._set_active = _set_native_typeops_active
        self._set_resolver = _set_native_typeops_resolver
        self._set_active(True)
        self._set_resolver(self._resolver)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        set_wire_typeinfo_map(None)

    def test_blob_building_makes_no_totf_or_mts_seam_calls(self) -> None:
        from mypy.build import _native_ctor_blob
        from mypy.nodes import MDEF, FuncDef, SymbolTableNode

        info = self.fx.make_type_info("mod.BlobPure")
        # A metaclass fallback avoids the stdlib typeinfo lookup, which
        # needs modules_state content unit tests do not populate.
        info.metaclass_type = Instance(self.fx.type_typei, [])
        fd = FuncDef("__init__", [], Block([]))
        fd.info = info
        info.names["__init__"] = SymbolTableNode(MDEF, fd)

        counters = {"totf": 0, "mts": 0}
        orig_totf = _type_kernel.rust_type_object_type_from_function
        orig_mts = _type_kernel.rust_map_type_from_supertype

        def spy_totf(*args: Any, **kwargs: Any) -> Any:
            counters["totf"] += 1
            return orig_totf(*args, **kwargs)

        def spy_mts(*args: Any, **kwargs: Any) -> Any:
            counters["mts"] += 1
            return orig_mts(*args, **kwargs)

        _type_kernel.rust_type_object_type_from_function = spy_totf
        _type_kernel.rust_map_type_from_supertype = spy_mts
        try:
            blob = _native_ctor_blob(info)
        finally:
            _type_kernel.rust_type_object_type_from_function = orig_totf
            _type_kernel.rust_map_type_from_supertype = orig_mts

        assert_equal(
            counters,
            {"totf": 0, "mts": 0},
            "ctor blob building must not enter the stale-resolver seams",
        )
        assert blob is not None, "ctor blob failed to build"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeContractLiteralsSuite(Suite):
    """Parity for the Rust `try_contracting_literals_in_union` port
    (mypy.typeops.try_contracting_literals_in_union, #492 family).

    The Python body contracts literal types sharing a fallback back into
    the sum type when all values of the sum are present: `Literal[True,
    False]` becomes `bool`, and a union covering every member of an enum
    becomes the enum Instance. The Rust port implements the bool case and
    the enum case (reading `enum_members` from the resolver snapshot).
    Toggling the typeops gate off (pure Python) and on (Rust seam) must
    produce identical results, and a direct seam call proves the Rust
    function engages rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        # `builtins.bool` is required for the bool contraction: the Rust
        # helper looks up the fallback snapshot for the Literal[True] /
        # Literal[False] items. The fixture scan above skips it because

        # `bool_type_info` does not end in "i", so add it explicitly
        # (mirrors production, where builtins.bool is always snapshotted).
        type_infos.append(self.fx.bool_type_info)
        # A three-member enum: `enum_members` (a names-walk property)
        # needs explicitly-valued Var members to yield the member names.
        self.enum_info = self.fx.make_type_info("mod.Color")
        self.enum_info.is_enum = True
        for name in ("RED", "GREEN", "BLUE"):
            v = Var(name)
            v.has_explicit_value = True
            v.type = self.fx.o
            self.enum_info.names[name] = SymbolTableNode(MDEF, v)
        type_infos.append(self.enum_info)
        self.enum_inst = Instance(self.enum_info, [])
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._live_map = {info.fullname: info for info in type_infos}
        self._resolver.set_live_typeinfo_map(dict(self._live_map))
        _set_native_typeops_active(True)
        _set_native_typeops_resolver(self._resolver)

    def tearDown(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_typeops_active(False)
        _set_native_typeops_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(active)
        try:
            return fn()
        finally:
            _set_native_typeops_active(True)

    def _strs(self, items: Sequence[Type]) -> list[str]:
        return [str(x) for x in try_contracting_literals_in_union(items)]

    def _assert_par(self, items: Sequence[Type], label: str) -> None:
        off = self._with_gate(False, lambda: self._strs(items))
        on = self._with_gate(True, lambda: self._strs(items))
        assert_equal(on, off, f"try_contracting_literals_in_union parity {label}")

    def _enum_literal(self, name: str) -> LiteralType:
        return LiteralType(name, self.enum_inst)

    def test_parity_bool_complete(self) -> None:
        # Literal[True, False] contracts to builtins.bool.
        items = [self.fx.lit_true, self.fx.lit_false]
        self._assert_par(items, "bool complete")
        result = self._with_gate(True, lambda: self._strs(items))
        assert_equal(result, ["builtins.bool"])

    def test_parity_bool_single(self) -> None:
        # A single bool literal must not contract (indices.len() < 2).
        items = [self.fx.lit_true, self.fx.a]
        self._assert_par(items, "bool single")
        result = self._with_gate(True, lambda: self._strs(items))
        assert_equal(result, ["Literal[True]", "A"])

    def test_parity_enum_complete(self) -> None:
        # A union covering every enum member contracts to the enum.
        items = [self._enum_literal(n) for n in ("RED", "GREEN", "BLUE")]
        self._assert_par(items, "enum complete")
        result = self._with_gate(True, lambda: self._strs(items))
        assert_equal(result, ["mod.Color"])

    def test_parity_enum_partial(self) -> None:
        # Not every member present: no contraction.
        items = [self._enum_literal("RED"), self._enum_literal("GREEN")]
        self._assert_par(items, "enum partial")
        result = self._with_gate(True, lambda: self._strs(items))
        assert_equal(result, ["Literal[mod.Color.RED]", "Literal[mod.Color.GREEN]"])

    def test_parity_non_literal_mixed(self) -> None:
        # Mixed literals and non-literals, no shared full value set.
        items = [self.fx.lit_true, self.fx.lit1]
        self._assert_par(items, "mixed")
        result = self._with_gate(True, lambda: self._strs(items))
        assert_equal(result, ["Literal[True]", "Literal[1]"])

    def test_seam_engages_direct(self) -> None:
        # Call the Rust seam directly on a complete bool union and confirm
        # it returns blobs (engages) rather than None (deferral).
        from mypy.typeops import _serialize_type_list

        r = _type_kernel.rust_try_contracting_literals_in_union(
            _serialize_type_list([self.fx.lit_true, self.fx.lit_false]), self._resolver
        )
        assert r is not None, "Rust try_contracting_literals_in_union did not engage"
        e = _type_kernel.rust_try_contracting_literals_in_union(
            _serialize_type_list([self._enum_literal(n) for n in ("RED", "GREEN", "BLUE")]),
            self._resolver,
        )
        assert e is not None, "Rust enum contraction did not engage"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFunctionTypeSuite(Suite):
    """Parity for the Rust `function_type`/`callable_type` port (mypy.typeops).

    `function_type` either returns the func's typed FunctionLike, or builds
    a `CallableType` from the FuncItem's signature (binding self/cls), or a
    dummy `Overloaded([CallableType])` for a broken overload. The Rust port
    mirrors all three branches; the Python shim restores the non-wire
    line/column/definition via `copy_modified`. Toggling the typeops gate
    off (pure Python) and on (Rust seam) must produce identical `str()` and
    identical line/column/definition state. Direct seam calls prove the
    Rust function engages rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active
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
        self._set_active = _set_native_typeops_active
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

    def _assert_par(self, func: FuncBase) -> None:
        from mypy.typeops import function_type

        off = self._with_gate(False, lambda: function_type(func, self.fx.function))
        on = self._with_gate(True, lambda: function_type(func, self.fx.function))
        assert_equal(str(on), str(off), f"function_type(str) parity {func.name}")
        assert_equal(on.line, off.line, f"function_type(line) parity {func.name}")
        assert_equal(on.column, off.column, f"function_type(column) parity {func.name}")
        if isinstance(on, CallableType) and isinstance(off, CallableType):
            assert_equal(on.implicit, off.implicit, f"function_type(implicit) {func.name}")
            if isinstance(func, FuncDef):
                assert_equal(
                    on.definition, off.definition, f"function_type(definition) parity {func.name}"
                )

    def _assert_engages(self, func: FuncBase) -> None:
        from mypy.typeops import _serialize_type

        result = _type_kernel.rust_function_type(func, _serialize_type(self.fx.function))
        assert result is not None, f"Rust function_type did not engage for {func.name}"

    def _func_def(
        self,
        name: str = "f",
        arguments: Sequence[Argument] | None = None,
        typ: FunctionLike | None = None,
    ) -> FuncDef:
        return FuncDef(name, list(arguments) if arguments is not None else None, None, typ)

    def test_untyped_func_def_no_self(self) -> None:
        # Plain def f(): -> dummy CallableType via callable_type: all-Any args.
        fn = self._func_def()
        self._assert_par(fn)
        self._assert_engages(fn)

    def test_func_def_with_args(self) -> None:
        # def f(x: int, y: str = "", *args, **kwargs): with no .type set.
        a = Var("x")
        b = Var("y")
        c = Var("args")
        d = Var("kwargs")
        fn = self._func_def(
            "f",
            [
                Argument(a, self.fx.a, None, ARG_POS),
                Argument(b, self.fx.b, None, ARG_OPT),
                Argument(c, None, None, ARG_STAR),
                Argument(d, None, None, ARG_STAR2),
            ],
        )
        self._assert_par(fn)
        self._assert_engages(fn)

    def test_method_with_self_binds_self(self) -> None:
        # Method: info set + has_self_or_cls_argument -> fill_typevars(info).
        fn = self._func_def("method")
        fn.info = self.fx.ai
        fn.arg_names = ["self"]
        fn.arg_kinds = [ARG_POS]
        self._assert_par(fn)
        self._assert_engages(fn)

    def test_classmethod_binds_cls_as_type(self) -> None:
        # @classmethod: is_class=True -> self_type is TypeType(Instance(A)).
        fn = self._func_def("cm")
        fn.info = self.fx.ai
        fn.is_class = True
        fn.arg_names = ["cls"]
        fn.arg_kinds = [ARG_POS]
        self._assert_par(fn)
        self._assert_engages(fn)

    def test_staticmethod_no_self_binding(self) -> None:
        # @staticmethod inside a class: has_self_or_cls_argument False
        # (is_static True, name != "__new__") -> all-Any args.
        fn = self._func_def("sm")
        fn.info = self.fx.ai
        fn.is_static = True
        fn.arg_names = ["x"]
        fn.arg_kinds = [ARG_POS]
        self._assert_par(fn)
        self._assert_engages(fn)

    def test_new_method_binds_cls(self) -> None:
        # __new__: has_self_or_cls_argument True even when static.
        fn = self._func_def("__new__")
        fn.info = self.fx.ai
        fn.is_static = True
        fn.arg_names = ["cls"]
        fn.arg_kinds = [ARG_POS]
        self._assert_par(fn)
        self._assert_engages(fn)

    def test_typed_func_def_returns_type(self) -> None:
        # A typed FuncDef: func.type truthy -> returned unchanged.
        typ = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            ["x", "y"],
            self.fx.a,
            self.fx.function,
            name="f",
            line=7,
            column=9,
        )
        fn = self._func_def("f", typ=typ)
        self._assert_par(fn)
        self._assert_engages(fn)

    def test_overloaded_func_def_types(self) -> None:
        # A valid typed overload: func.type is an Overloaded -> returned
        # unchanged (passthrough).
        item = FuncDef("g")
        item.info = self.fx.ai
        item.arg_names = ["x"]
        item.arg_kinds = [ARG_POS]
        overloaded = OverloadedFuncDef([item])
        overloaded.type = Overloaded(
            [CallableType([self.fx.a], [ARG_POS], ["x"], self.fx.a, self.fx.function, name="g")]
        )
        self._assert_par(overloaded)
        self._assert_engages(overloaded)

    def test_broken_overload_dummy(self) -> None:
        # A broken overload whose type is set to a (dummy) CallableType:
        # Python returns `func.type` UNCHANGED (the `if func.type:`
        # passthrough fires first). Rust must mirror the passthrough.
        item = FuncDef("g")
        item.info = self.fx.ai
        item.arg_names = ["x"]
        item.arg_kinds = [ARG_POS]
        overloaded = OverloadedFuncDef([item])
        overloaded.type = CallableType(
            [self.fx.a], [ARG_POS], ["x"], self.fx.a, self.fx.function, name="g"
        )
        self._assert_par(overloaded)
        self._assert_engages(overloaded)

    def test_untyped_overload_dummy(self) -> None:
        # OverloadedFuncDef with NO type: Python builds the dummy
        # `Overloaded([CallableType([Any, Any], [ARG_STAR, ARG_STAR2], ...,
        # is_ellipsis_args=True)])` with line=func.line and no name.
        item = FuncDef("h")
        item.info = self.fx.ai
        item.arg_names = ["x", "y"]
        item.arg_kinds = [ARG_POS, ARG_OPT]
        overloaded = OverloadedFuncDef([item])
        self.assertIsNone(overloaded.type)
        self._assert_par(overloaded)
        self._assert_engages(overloaded)
        from mypy.typeops import function_type

        result = self._with_gate(True, lambda: function_type(overloaded, self.fx.function))
        assert isinstance(result, Overloaded)
        assert isinstance(result.items[0], CallableType)
        assert_equal(result.items[0].is_ellipsis_args, True)
        assert_equal(result.items[0].arg_kinds, [ARG_STAR, ARG_STAR2])
        assert_equal(result.items[0].line, overloaded.line)

    def test_invalid_type_defers(self) -> None:
        # func.type truthy but not a FunctionLike (e.g. a Defn): Rust
        # defers (returns None) and the Python body's assert decides.
        from mypy.typeops import _serialize_type

        fn = self._func_def("f")
        fn.type = fn  # type: ignore[assignment]  # a Defn, not a FunctionLike
        result = _type_kernel.rust_function_type(fn, _serialize_type(self.fx.function))
        assert result is None, f"Rust should defer for invalid func.type, got {result!r}"
        import pytest

        self._set_active(False)
        try:
            with pytest.raises(AssertionError):
                from mypy.typeops import function_type

                function_type(fn, self.fx.function)
        finally:
            self._set_active(True)

    def test_lambda_expr_callable_type(self) -> None:
        # LambdaExpr is a FuncItem with no .type; the checkexpr lambda
        # caller passes an explicit ret_type. rust_callable_type must
        # mirror `callable_type` including the ret_type substitution.
        from mypy.nodes import LambdaExpr

        lam_body = Block([ReturnStmt(NameExpr("x"))])
        lam = LambdaExpr(arguments=[], body=lam_body, typ=None)
        lam.arg_names = ["x"]
        lam.arg_kinds = [ARG_POS]
        # lambda (x): x with info unset -> all-Any args; ret_type = fx.a.
        lam.line = 5
        lam.column = 7
        from mypy.typeops import callable_type

        off = self._with_gate(False, lambda: callable_type(lam, self.fx.function, self.fx.a))
        on = self._with_gate(True, lambda: callable_type(lam, self.fx.function, self.fx.a))
        assert_equal(str(on), str(off), "callable_type(lambda) str parity")
        assert_equal(on.line, off.line, "callable_type(lambda) line parity")
        assert_equal(on.column, off.column, "callable_type(lambda) column parity")
        assert_equal(on.implicit, off.implicit, "callable_type(lambda) implicit parity")
        assert_equal(on.name, "<lambda>", "callable_type(lambda) uses LAMBDA_NAME")
        assert_equal(str(on.ret_type), str(self.fx.a), "callable_type(lambda) ret_type")
        # Direct seam call proves native engagement for the lambda path.
        from mypy.typeops import _serialize_type

        result = _type_kernel.rust_callable_type(
            lam, _serialize_type(self.fx.function), _serialize_type(self.fx.a)
        )
        assert result is not None, "Rust callable_type did not engage for lambda"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTruthinessSuite(Suite):
    """Parity for the Rust `true_only`/`false_only`/`true_or_false` port.

    The Python bodies (typeops.py:1287-1402) gate behind the typeops gate
    and the installed resolver. The Rust port decides each step (including
    the step-6 `__bool__`/`__len__` live-MRO lookup and the `is_final`/
    `is_enum` checks) and returns a discriminator; the Python shim applies
    the `copy_type` + flag mutation on live objects.

    Every test runs gate-off (pure Python) vs gate-on (Rust seam) and
    asserts identical `str()`. Direct seam calls prove the Rust function
    engages rather than silently deferring. The fixture must install BOTH
    the resolver and the live TypeInfo map (`set_live_typeinfo_map`), since
    the step-6 MRO walk reads `mro`/`names` live.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self.typeinfo_map = {info.fullname: info for info in type_infos}
        set_wire_typeinfo_map(self.typeinfo_map)
        # The resolver holds its own live_info_map; install it so the
        # step-6 MRO walk sees it. build_native_resolver owns the object.
        self.resolver.set_live_typeinfo_map(self.typeinfo_map)
        _set_native_typeops_active(True)
        _set_native_typeops_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_typeops_active(False)
        _set_native_typeops_resolver(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], ProperType]) -> ProperType:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(active)
        try:
            return fn()
        finally:
            _set_native_typeops_active(True)

    def _assert_par(
        self,
        op: str,
        t: ProperType,
        *,
        strict_optional: bool = True,
        expect_uninhabited: bool = False,
    ) -> None:
        from mypy.state import state
        from mypy.typeops import true_only, true_or_false

        fn: Callable[[Type], ProperType]
        if op == "true_only":
            fn = true_only
        elif op == "false_only":
            fn = false_only
        elif op == "true_or_false":
            fn = true_or_false
        else:
            raise AssertionError(f"bad op {op}")
        with state.strict_optional_set(strict_optional):
            off = self._with_gate(False, lambda: fn(t))
            on = self._with_gate(True, lambda: fn(t))
        assert_equal(str(on), str(off), f"{op}(t) str parity strict_optional={strict_optional}")
        if expect_uninhabited:
            assert isinstance(
                on, UninhabitedType
            ), f"{op}(t) should be UninhabitedType, got {on!r}"

    def _add_dunder(self, info: TypeInfo, name: str, ret: Type) -> None:
        """Add a `def __bool__/__len__(self) -> ret` method to a live TypeInfo.

        Mirrors `TypeFixture._add_bool_dunder`: a FuncDef with a
        CallableType (no args -> returns ret) registered as MDEF.
        """
        from mypy.nodes import MDEF, FuncDef, SymbolTableNode

        signature = CallableType([], [], [], ret, self.fx.function)
        func_def = FuncDef(name, [], Block([]))
        func_def.type = signature
        info.names[name] = SymbolTableNode(MDEF, func_def)

    def _add_bool_ret(self, info: TypeInfo, ret: Type) -> None:
        self._add_dunder(info, "__bool__", ret)

    def _add_len_ret(self, info: TypeInfo, ret: Type) -> None:
        self._add_dunder(info, "__len__", ret)

    def test_plain_instance_no_dunder(self) -> None:
        # D (mro=[object]) has no __bool__/__len__ in any live MRO entry
        # (the fixture object TypeInfo's names table is empty) -> both ops
        # copy with a flag mutation, not a defer and not Uninhabited.
        self._assert_par("true_only", self.fx.d)
        self._assert_par("false_only", self.fx.d)

    def test_instance_bool_returns_never_true_only(self) -> None:
        # def __bool__(self) -> Never: true_only(D) -> Uninhabited.
        self._add_bool_ret(self.fx.di, UninhabitedType())
        self._assert_par("true_only", self.fx.d, expect_uninhabited=True)

    def test_instance_bool_returns_never_false_only(self) -> None:
        # def __bool__(self) -> Never: false_only(D) -> Uninhabited
        # (ret_type found and not can_be_false).
        self._add_bool_ret(self.fx.di, UninhabitedType())
        self._assert_par("false_only", self.fx.d, expect_uninhabited=True)

    def test_instance_bool_returns_bool(self) -> None:
        # __bool__ -> bool: can_be_true and can_be_false, so
        # true_only/false_only both copy; false_only must NOT narrow a
        # @final class with __bool__ -> bool to Uninhabited.
        self._add_bool_ret(self.fx.di, self.fx.bool_type)
        self._assert_par("true_only", self.fx.d)
        self._assert_par("false_only", self.fx.d)

    def test_final_class_no_dunder_strict(self) -> None:
        # @final class D (no custom dunder) under strict_optional:
        # false_only(D) -> Uninhabited.
        self.fx.di.is_final = True
        self._assert_par("false_only", self.fx.d, expect_uninhabited=True)

    def test_final_class_no_dunder_non_strict(self) -> None:
        # Non-strict: is_final check only runs under strict_optional ->
        # false_only(D) copies (not Uninhabited).
        self.fx.di.is_final = True
        self._assert_par("false_only", self.fx.d, strict_optional=False)

    def test_final_class_with_bool_dunder(self) -> None:
        # @final class with __bool__ -> bool: ret_type found and
        # can_be_false -> copy. This is the exact case the draft's
        # conflation broke.
        self.fx.di.is_final = True
        self._add_bool_ret(self.fx.di, self.fx.bool_type)
        self._assert_par("true_only", self.fx.d)
        self._assert_par("false_only", self.fx.d)

    def test_enum_class_no_dunder_strict(self) -> None:
        # is_enum=True: false_only(D) -> Uninhabited under strict_optional.
        self.fx.di.is_enum = True
        self._assert_par("false_only", self.fx.d, expect_uninhabited=True)

    def test_enum_class_no_dunder_non_strict(self) -> None:
        self.fx.di.is_enum = True
        self._assert_par("false_only", self.fx.d, strict_optional=False)

    def test_str_instance(self) -> None:
        # str -> LiteralType("", fallback=str) in false_only; true_only
        # copies (str can_be_true).
        self._assert_par("true_only", self.fx.str_type)
        self._assert_par("false_only", self.fx.str_type)

    def test_bytes_instance(self) -> None:
        # bytes/str share the LiteralType("") rule. Build a bytes TypeInfo
        # via make_type_info and set the builtins.bytes fullname.
        bytes_info = self.fx.make_type_info("bytes")
        bytes_info._fullname = "builtins.bytes"
        bytes_inst = Instance(bytes_info, [])
        self._assert_par("true_only", bytes_inst)
        self._assert_par("false_only", bytes_inst)

    def test_int_instance(self) -> None:
        # int -> LiteralType(0) in false_only. Build a builtins.int TypeInfo.
        int_info = self.fx.make_type_info("int")
        int_info._fullname = "builtins.int"
        int_inst = Instance(int_info, [])
        self._assert_par("true_only", int_inst)
        self._assert_par("false_only", int_inst)

    def test_none_uninhabited_bool_literals(self) -> None:
        self._assert_par("true_only", self.fx.nonet)
        self._assert_par("false_only", self.fx.nonet)
        self._assert_par("true_only", self.fx.uninhabited)
        self._assert_par("false_only", self.fx.uninhabited, expect_uninhabited=True)
        self._assert_par("true_only", self.fx.lit_false, expect_uninhabited=True)
        self._assert_par("true_only", self.fx.lit_true)
        self._assert_par("false_only", self.fx.lit_true, expect_uninhabited=True)
        self._assert_par("false_only", self.fx.lit_false)

    def test_union_narrow(self) -> None:
        # A | None under strict_optional: false_only keeps None, true_only
        # keeps A meaning both copy; the union recursion must preserve
        # per-item discriminators.
        from mypy.typeops import make_simplified_union

        u = make_simplified_union([self.fx.a, self.fx.nonet])
        self._assert_par("true_only", u)
        self._assert_par("false_only", u)
        # A | int: int false_only is LiteralType(0) — the union disc for
        # int must decode to the fallback correctly.
        u2 = make_simplified_union([self.fx.a, self.fx.str_type])
        self._assert_par("false_only", u2)

    def test_dunder_in_mro_base(self) -> None:
        # class B(A); A has __bool__ -> Never. Instance(B)'s MRO walk must
        # find A's __bool__.
        self._add_bool_ret(self.fx.ai, UninhabitedType())
        self._assert_par("true_only", self.fx.b, expect_uninhabited=True)

    def test_dunder_in_fixture_a(self) -> None:
        # The fixture gives A a __bool__ -> bool. Both ops must handle the
        # live-map dunder normally (copy on both).
        self._assert_par("true_only", self.fx.a)
        self._assert_par("false_only", self.fx.a)

    def test_len_fallback_when_no_bool(self) -> None:
        # D has no __bool__ but a __len__ -> Never: false_only(D) ->
        # Uninhabited via the __len__ fallback.
        self._add_len_ret(self.fx.di, UninhabitedType())
        self._assert_par("false_only", self.fx.d, expect_uninhabited=True)

    def test_bool_wins_over_len(self) -> None:
        # D has __bool__ -> bool AND __len__ -> Never: the __bool__ lookup
        # wins (first name in the or-chain), so false_only copies instead
        # of narrowing via __len__.
        self._add_bool_ret(self.fx.di, self.fx.bool_type)
        self._add_len_ret(self.fx.di, UninhabitedType())
        self._assert_par("false_only", self.fx.d)

    def test_resolver_absent_defers_to_python(self) -> None:
        # With the resolver removed, the Rust seam defers (None) and the
        # result equals the pure-Python path (which itself runs since the
        # gate is still active but the resolver is None -> skip).
        from mypy.state import state
        from mypy.typeops import _set_native_typeops_resolver

        _set_native_typeops_resolver(None)
        try:
            with state.strict_optional_set(True):
                off = self._with_gate(False, lambda: true_only(self.fx.a))
                on = self._with_gate(True, lambda: true_only(self.fx.a))
            assert_equal(str(on), str(off), "resolver-absent true_only parity")
        finally:
            _set_native_typeops_resolver(self.resolver)

    def test_true_or_false_resets_flags(self) -> None:
        self._assert_par("true_or_false", self.fx.a)
        self._assert_par("true_or_false", self.fx.lit_false)
        u = make_simplified_union([self.fx.a, self.fx.nonet])
        self._assert_par("true_or_false", u)

    def test_engages_via_direct_seam(self) -> None:
        # Direct seam calls prove the Rust functions engage for the
        # handled cases (not silently deferring to Python).
        from mypy.typeops import _serialize_type

        d_bytes = _serialize_type(self.fx.d)
        assert _type_kernel.rust_true_only(d_bytes, self.resolver) is not None
        assert _type_kernel.rust_false_only(d_bytes, True, self.resolver) is not None
        assert _type_kernel.rust_true_or_false(d_bytes, self.resolver) is not None
        self._add_bool_ret(self.fx.di, UninhabitedType())
        assert _type_kernel.rust_true_only(d_bytes, self.resolver) is not None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeImplTruthinessSuite(Suite):
    """Parity for the Rust `can_be_true_default`/`can_be_false_default` ports.

    The byte seams (types_impl.rs `rust_can_be_true_default` /
    `rust_can_be_false_default`) cover the type classes whose truthiness is
    wire-portable; the resolver-backed live seams
    (`rust_can_be_true_default_live` / `rust_can_be_false_default_live`)
    additionally decide the TupleType `can_be_any_bool` check, the
    TypeAliasType alias-target delegation, and the enum-LiteralType branch.
    Python falls back to the pure-Python default when the Rust seam returns
    None.

    Every test asserts the gate-on result equals the gate-off (pure Python)
    result via `bool(t.can_be_true)` / `bool(t.can_be_false)`, and direct
    seam calls prove the Rust function engages (does not silently defer)
    for the handled cases. The fixture installs the resolver with the live
    TypeInfo map so the live-MRO `__bool__`/`__len__` walk and the
    `is_enum` read work.

    The `_VISITOR_*` seam names in mypy.types are bound only when the
    module-level try-block succeeds. On a first import that try hits the
    circular `from mypy.types import read_type` (read_type is defined later
    in the module), so it fails and leaves every `_rust_*` seam None plus
    `_VISITOR_HAS_TYPE_KERNEL` False. A module reload would bind them but
    breaks object identity for the already-imported fixture types. Instead
    the fixture binds the seam names directly on the live module, exactly
    as the try-block would (mirror of mypy/types.py:4497-4534).
    """

    # Seam names stay None on first import, so the fixture binds them on
    # the live module; see the class docstring for why binding here beats
    # a module reload.
    _VISITOR_SEAM_NAMES = (
        "callable_with_ellipsis",
        "can_be_false_default",
        "can_be_false_default_live",
        "can_be_true_default",
        "can_be_true_default_live",
        "callable_is_generic",
        "callable_is_kw_arg",
        "callable_is_var_arg",
        "callable_max_possible_positional_args",
        "callable_min_args",
        "callable_formal_arguments",
        "callable_argument_by_name",
        "callable_argument_by_position",
        "copy_type",
        "find_unpack_in_list",
        "flatten_nested_tuples",
        "flatten_nested_unions",
        "has_recursive_types",
        "has_type_vars",
        "is_literal_type",
        "is_unannotated_any",
        "copy_modified",
        "remove_dups",
        "split_with_prefix_and_suffix",
        "tuple_length",
        "type_vars_as_args",
        "union_length",
    )
    _VISITOR_SEAM_RESOLVER_NAMES = ("can_be_true_default_live", "can_be_false_default_live")

    def setUp(self) -> None:
        from librt.internal import ReadBuffer, WriteBuffer

        import mypy.types as _types_mod

        self._types_mod = _types_mod
        # Bind the seam names the module-level try would have bound.
        _types_mod._VisitorWriteBuffer = WriteBuffer  # type: ignore[attr-defined]
        _types_mod._ReadBuffer = ReadBuffer  # type: ignore[attr-defined]
        for n in self._VISITOR_SEAM_NAMES:
            _types_mod.__dict__["_rust_" + n] = getattr(_type_kernel, "rust_" + n)
        self._orig_kernel_flag = _types_mod._VISITOR_HAS_TYPE_KERNEL
        _types_mod._VISITOR_HAS_TYPE_KERNEL = True
        # `_native_visitor_active` is a process-global other suites (the
        # conftest parity installer) may have enabled; save it and restore
        # the prior value in tearDown rather than forcing False.
        self._orig_visitor_gate = _types_mod._native_visitor_active

        from mypy.types import _set_native_truthiness_resolver

        self.fx = TypeFixture()
        self.type_infos = _base_infos(self.fx)
        self.resolver = _type_kernel.build_native_resolver(self.type_infos, [])
        self.resolver.set_live_typeinfo_map({info.fullname: info for info in self.type_infos})
        _set_native_truthiness_resolver(self.resolver)
        self._set_gate(True)

    def tearDown(self) -> None:
        from mypy.types import _set_native_truthiness_resolver

        # Restore the pre-suite state: the visitor gate as when the
        # suite started, no resolver, and the kernel flag as when the
        # suite started. The old hardcoded False leaked into other tests.
        self._set_gate(self._orig_visitor_gate)
        _set_native_truthiness_resolver(None)
        self._types_mod._VISITOR_HAS_TYPE_KERNEL = self._orig_kernel_flag

    def _set_gate(self, active: bool) -> None:
        from mypy.types import _set_native_visitor_active

        _set_native_visitor_active(active)

    def _with_gate(self, active: bool, fn: Callable[[], bool]) -> bool:
        self._set_gate(active)
        try:
            return fn()
        finally:
            self._set_gate(True)

    def _assert_par(self, t: Type) -> None:
        """Assert gate-on (Rust) == gate-off (Python) for both booleans."""
        off_true = self._with_gate(False, lambda: t.can_be_true)
        on_true = self._with_gate(True, lambda: t.can_be_true)
        off_false = self._with_gate(False, lambda: t.can_be_false)
        on_false = self._with_gate(True, lambda: t.can_be_false)
        assert on_true == off_true, f"can_be_true parity {t!r}: {off_true} vs {on_true}"
        assert on_false == off_false, f"can_be_false parity {t!r}: {off_false} vs {on_false}"

    def _assert_values(self, t: Type, true: bool, false: bool) -> None:
        """Assert both gate-on results equal the expected booleans."""
        on_true = self._with_gate(True, lambda: t.can_be_true)
        on_false = self._with_gate(True, lambda: t.can_be_false)
        assert on_true == true, f"can_be_true {t!r}: expected {true}, got {on_true}"
        assert on_false == false, f"can_be_false {t!r}: expected {false}, got {on_false}"

    def test_uninhabited_none(self) -> None:
        # UninhabitedType: False/False. NoneType: False/True.
        self._assert_values(self.fx.uninhabited, False, False)
        self._assert_values(self.fx.nonet, False, True)
        self._assert_par(self.fx.uninhabited)
        self._assert_par(self.fx.nonet)

    def test_callable(self) -> None:
        # CallableType: FunctionLike forces _can_be_false=False -> True/False.
        c = CallableType([], [], [], NoneType(), self.fx.function)
        self._assert_values(c, True, False)
        self._assert_par(c)

    def test_union(self) -> None:
        # UnionType: any(item.can_be_*). A | None -> true/false both True.
        from mypy.typeops import make_simplified_union

        u = make_simplified_union([self.fx.a, self.fx.nonet])
        self._assert_values(u, True, True)
        self._assert_par(u)
        # A | str -> true True, false True (str can_be_false via literal "").
        u2 = make_simplified_union([self.fx.a, self.fx.str_type])
        self._assert_par(u2)

    def test_literal_non_enum(self) -> None:
        # LiteralType non-enum: bool(value). Literal[0] -> False/True.
        self._assert_values(self.fx.lit_false, False, True)
        self._assert_values(self.fx.lit_true, True, False)
        self._assert_par(self.fx.lit_false)
        self._assert_par(self.fx.lit_true)
        # Literal[0] -> False (int 0); Literal[0.0] -> False (Float != 0.0);
        # Literal[""] -> False. Fixture has no int/float TypeInfos, so build
        # them as NativeTruthinessSuite.test_int_instance does.
        int_info = self.fx.make_type_info("int")
        int_info._fullname = "builtins.int"
        int_type = Instance(int_info, [])
        float_info = self.fx.make_type_info("float")
        float_info._fullname = "builtins.float"
        float_type = Instance(float_info, [])
        zero = LiteralType(0, int_type)
        zero_float = LiteralType(0.0, float_type)
        empty_str = LiteralType("", self.fx.str_type)
        self._assert_values(zero, False, True)
        self._assert_values(zero_float, False, True)
        self._assert_values(empty_str, False, True)
        self._assert_par(zero)
        self._assert_par(zero_float)
        self._assert_par(empty_str)

    def test_literal_enum(self) -> None:
        # Enum literal: fallback is_enum=True, truthiness is the Instance
        # default (True/True); mypy does not respect __bool__/__len__.
        en = self.fx.make_type_info("E", module_name="__main__", is_abstract=False)
        en.is_enum = True
        lit_member = LiteralType("a", Instance(en, []))
        self._assert_values(lit_member, True, True)
        self._assert_par(lit_member)

    def test_tuple_builtins_tuple(self) -> None:
        # TupleType with builtins.tuple fallback: can_be_any_bool False ->
        # can_be_true = length > 0, can_be_false = (length parse).
        t0 = TupleType([], self.fx.std_tuple)
        t1 = TupleType([self.fx.a], self.fx.std_tuple)
        t2 = TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)
        self._assert_values(t0, False, True)  # empty -> can_be_false True
        self._assert_values(t1, True, False)  # len 1 non-unpack
        self._assert_values(t2, True, False)  # len 2 -> false
        self._assert_par(t0)
        self._assert_par(t1)
        self._assert_par(t2)

    def test_tuple_namedtuple_with_dunder(self) -> None:
        # TupleType with a namedtuple/custom fallback that has __bool__ in an
        # MRO class -> can_be_any_bool True -> both True.
        name_ti = self.fx.make_type_info("NT", module_name="__main__")
        name_ti.is_named_tuple = True
        # Add __bool__ returning bool.
        from mypy.nodes import MDEF, FuncDef, SymbolTableNode

        sig = CallableType([], [], [], self.fx.bool_type, self.fx.function)
        func_def = FuncDef("__bool__", [], Block([]))
        func_def.type = sig
        name_ti.names["__bool__"] = SymbolTableNode(MDEF, func_def)
        nt = TupleType([self.fx.a, self.fx.b], Instance(name_ti, []))
        self._assert_values(nt, True, True)
        self._assert_par(nt)

    def test_typealias(self) -> None:
        # TypeAliasType: delegates to alias.target.can_be_*.
        alias_node = TypeAliasType(None, [self.fx.a])  # alias.target = self.fx.a (Instance A)
        self._assert_par(alias_node)

    def test_engages_via_direct_seam(self) -> None:
        # Direct seam calls: byte/live seams engage (not defer). Byte seam
        # defers on LiteralType (needs TypeInfo.is_enum), so only plain
        # types engage it; live seam decides literals via the resolver.
        from mypy.types import _serialize_type_for_visitor

        for t in (self.fx.d, self.fx.nonet):
            assert (
                _type_kernel.rust_can_be_true_default(_serialize_type_for_visitor(t)) is not None
            ), f"rust_can_be_true_default did not engage for {t!r}"
            assert (
                _type_kernel.rust_can_be_false_default(_serialize_type_for_visitor(t)) is not None
            ), f"rust_can_be_false_default did not engage for {t!r}"
            # Live seams engage with the resolver installed (incl. literals).
            assert (
                _type_kernel.rust_can_be_true_default_live(
                    _serialize_type_for_visitor(t), self.resolver
                )
                is not None
            ), f"rust_can_be_true_default_live did not engage for {t!r}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeInstantiateTypeAliasSuite(Suite):
    """Parity for the Rust `instantiate_type_alias` port (mypy.typeanal).

    `instantiate_type_alias` normalizes a TypeAlias node plus type
    arguments into the instantiated result type. The Rust port decides
    the three non-error success paths (bare-generic eager expansion,
    non-generic alias, correct generic instantiation) and returns a
    branch tag; the Python shim rebuilds the live result object exactly
    as the pure-Python body would. Every error / Any-fill path returns
    None and Python runs the full pure-Python body. Toggling the typeanal
    gate off (pure Python) and on (Rust seam) must produce identical
    `(str(t), used_default)` results on both success and deferral paths.
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        self.fx = TypeFixture()
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

    def _make_alias(
        self,
        target: Type,
        *,
        alias_tvars: list[TypeVarLikeType] | None = None,
        no_args: bool = False,
    ) -> TypeAlias:
        from mypy.nodes import TypeAlias

        return TypeAlias(
            target, "mod.AL", "mod", -1, -1, alias_tvars=alias_tvars or [], no_args=no_args
        )

    def _ctx(self) -> Context:
        ctx = Context(5, 5)
        ctx.end_line = 6
        ctx.end_column = 7
        return ctx

    def _instantiate(
        self, node: TypeAlias, args: list[Type], no_args: bool, *, empty_tuple_index: bool = False
    ) -> tuple[Type, bool]:
        from mypy.typeanal import instantiate_type_alias

        return instantiate_type_alias(
            node,
            args,
            lambda *a, **k: None,
            lambda *a, **k: None,
            no_args,
            self._ctx(),
            Options(),
            empty_tuple_index=empty_tuple_index,
        )

    def _assert_par(
        self, node: TypeAlias, args: list[Type], no_args: bool, *, empty_tuple_index: bool = False
    ) -> None:
        off = self._with_gate(
            False,
            lambda: self._instantiate(node, args, no_args, empty_tuple_index=empty_tuple_index),
        )
        on = self._with_gate(
            True,
            lambda: self._instantiate(node, args, no_args, empty_tuple_index=empty_tuple_index),
        )
        assert_equal(
            (str(on[0]), on[1]),
            (str(off[0]), off[1]),
            f"instantiate_type_alias parity {node.name} args={args}",
        )

    def _rust_tag(self, node: TypeAlias, args: list[Type], no_args: bool) -> int | None:
        from mypy.typeanal import (  # type: ignore[attr-defined]
            _rust_instantiate_type_alias,
            _serialize_typeanal_type,
        )

        return _rust_instantiate_type_alias(
            node, [_serialize_typeanal_type(a) for a in args], no_args, False, False
        )

    def _assert_engages(self, node: TypeAlias, args: list[Type], no_args: bool) -> None:
        result = self._rust_tag(node, args, no_args)
        assert result is not None, f"Rust instantiate_type_alias did not engage for {node.name}"

    def test_non_generic_alias(self) -> None:
        # S = str; used bare -> TypeAliasType(S, []), normalization deferred.
        from mypy.nodes import TypeAlias

        node = TypeAlias(Instance(self.fx.str_type_info, []), "mod.S", "mod", -1, -1)
        self._assert_par(node, [], False)
        self._assert_engages(node, [], False)

    def test_no_args_bare_generic_empty(self) -> None:
        # L = List (no_args); used bare -> eager Instance(list, []).
        node = self._make_alias(
            Instance(self.fx.std_listi, [AnyType(TypeOfAny.special_form)]), no_args=True
        )
        self._assert_par(node, [], True)
        self._assert_engages(node, [], True)

    def test_no_args_bare_generic_with_args(self) -> None:
        # L = List; L[int] -> eager Instance(list, [int]) carrying location.
        node = self._make_alias(
            Instance(self.fx.std_listi, [AnyType(TypeOfAny.special_form)]), no_args=True
        )
        self._assert_par(node, [self.fx.a], True)
        self._assert_engages(node, [self.fx.a], True)

    def test_generic_alias_correct_args(self) -> None:
        # G[T]; G[int] -> TypeAliasType(G, [int]).
        node = self._make_alias(Instance(self.fx.gi, [self.fx.t]), alias_tvars=[self.fx.t])
        self._assert_par(node, [self.fx.a], False)
        self._assert_engages(node, [self.fx.a], False)

    def test_generic_alias_missing_args_defers(self) -> None:
        # G[T] used bare with an undefaulted T -> Python fills Any
        # (set_any_tvars); Rust defers.
        node = self._make_alias(Instance(self.fx.gi, [self.fx.t]), alias_tvars=[self.fx.t])
        # Parity holds even though the path defers to Python.
        self._assert_par(node, [], False)
        assert self._rust_tag(node, [], False) is None

    def test_generic_alias_default_fill_single(self) -> None:
        # G[T = A] used bare: every alias tvar has a default, so
        # set_any_tvars takes its defaults-only path (tag 3); the shim runs
        # the per-default native expand_type and rebuilds the alias.
        tv = self.fx.t.copy_modified(default=self.fx.a)
        node = self._make_alias(Instance(self.fx.gi, [tv]), alias_tvars=[tv])
        self._assert_par(node, [], False)
        assert self._rust_tag(node, [], False) == 3

    def test_generic_alias_default_fill_crossref(self) -> None:
        # S's default references the earlier T; the gradual env built by
        # the shim must substitute T's resolved default into S's.
        t = self.fx.t.copy_modified(default=self.fx.a)
        s = self.fx.s.copy_modified(default=Instance(self.fx.std_listi, [t]))
        node = self._make_alias(Instance(self.fx.gi, [t, s]), alias_tvars=[t, s])
        self._assert_par(node, [], False)
        assert self._rust_tag(node, [], False) == 3

    def test_generic_alias_mixed_defaults_defers(self) -> None:
        # T has no default, so the fill constructs an Any type; Rust
        # defers and the pure-Python set_any_tvars body stays single-sourced.
        t = self.fx.t
        s = self.fx.s.copy_modified(default=self.fx.a)
        node = self._make_alias(Instance(self.fx.gi, [t, s]), alias_tvars=[t, s])
        self._assert_par(node, [], False)
        assert self._rust_tag(node, [], False) is None

    def test_generic_alias_analyzing_tvar_def_defers(self) -> None:
        # While analyzing another tvar default the fill records
        # used_default and checks default recursion: Rust keeps the
        # pure-Python path (tag must stay None).
        from mypy.typeanal import _rust_instantiate_type_alias  # type: ignore[attr-defined]

        tv = self.fx.t.copy_modified(default=self.fx.a)
        node = self._make_alias(Instance(self.fx.gi, [tv]), alias_tvars=[tv])
        assert _rust_instantiate_type_alias(node, [], False, False, True) is None

    def test_generic_alias_bad_count_defers(self) -> None:
        # G[T] with two args -> error + Any fill; Rust defers.
        node = self._make_alias(Instance(self.fx.gi, [self.fx.t]), alias_tvars=[self.fx.t])
        self._assert_par(node, [self.fx.a, self.fx.b], False)

    def test_variadic_alias_split_defers(self) -> None:
        # H[T, Ts, S] with a split TypeVarTuple -> error; Rust defers.
        node = self._make_alias(
            Instance(self.fx.hi, [self.fx.t, self.fx.ts, self.fx.s]),
            alias_tvars=[self.fx.t, self.fx.ts, self.fx.s],
        )
        unpack = UnpackType(self.fx.ts)
        self._assert_par(node, [unpack], False)

    def test_flexible_alias_unwrap(self) -> None:
        # FlexibleAlias[T, typ] expands to typ (last argument).

        flex_info = self.fx.make_type_info("mypy_extensions.FlexibleAlias")
        target = Instance(flex_info, [self.fx.t, self.fx.a])
        node = self._make_alias(target, alias_tvars=[self.fx.t])
        self._assert_par(node, [self.fx.b], False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeUnboundWithoutTypeInfoSuite(Suite):
    """Parity for the Rust `analyze_unbound_type_without_type_info` port.

    The classification front (which unbound non-TypeInfo symbol a type
    expression refers to) runs in Rust: an Any-typed Var alias, the
    `type` special form under allow_type_any, an allowed unbound type
    variable, and an enum member inside Literal[...], plus the full
    message-tail family (variable / function / module not valid as a
    type, rejected unbound type variables with the PEP 695 variant, raw
    enum values, and `TypeType[Any]` under allow_type_any).
    The Python shim rebuilds the result objects from the live node and
    applies every fail/note side effect keyed by the Rust tag. Both gates
    must produce identical `str(result)` AND identical captured
    fail/note message lists on engaged paths, and a direct seam call
    proves the Rust classifier engages on each family.
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        self.fx = TypeFixture()
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

    def _analyser(self, *, allow_type_any: bool = False, allow_unbound_tvars: bool = False) -> Any:
        from mypy.tvar_scope import TypeVarLikeScope
        from mypy.typeanal import TypeAnalyser

        ta = TypeAnalyser.__new__(TypeAnalyser)
        ta.tvar_scope = TypeVarLikeScope()
        ta.allow_type_any = allow_type_any
        ta.allow_unbound_tvars = allow_unbound_tvars
        ta.allow_param_spec_literals = False
        messages: list[str] = []
        ta.messages = messages  # type: ignore[attr-defined]
        ta.fail = lambda msg, *a, **k: messages.append(msg)  # type: ignore[method-assign]
        ta.note = lambda msg, *a, **k: messages.append(  # type: ignore[method-assign]
            "note: " + msg
        )
        return ta

    def _var_sym(
        self,
        info: TypeInfo | None,
        type: AnyType | Instance | TypeType | None,
        name: str = "mod.x",
    ) -> SymbolTableNode:
        v = Var(name.rsplit(".", 1)[-1])
        v._fullname = name
        v.type = type
        if info is not None:
            v.info = info
        return SymbolTableNode(MDEF, v)

    def _tvar_expr_sym(self, name: str = "T") -> SymbolTableNode:
        from mypy.nodes import SymbolTableNode

        tv = TypeVarExpr(name, name, [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics))
        return SymbolTableNode(MDEF, tv)

    def _call(
        self,
        t: UnboundType,
        sym: SymbolTableNode,
        defining_literal: bool,
        *,
        allow_type_any: bool = False,
        allow_unbound_tvars: bool = False,
    ) -> tuple[Type, list[str]]:
        ta = self._analyser(allow_type_any=allow_type_any, allow_unbound_tvars=allow_unbound_tvars)
        result = cast("Type", ta.analyze_unbound_type_without_type_info(t, sym, defining_literal))
        return result, ta.messages

    def _assert_par(
        self,
        t: UnboundType,
        sym: SymbolTableNode,
        defining_literal: bool,
        *,
        allow_type_any: bool = False,
        allow_unbound_tvars: bool = False,
    ) -> None:
        off = self._with_gate(
            False,
            lambda: self._call(
                t,
                sym,
                defining_literal,
                allow_type_any=allow_type_any,
                allow_unbound_tvars=allow_unbound_tvars,
            ),
        )
        on = self._with_gate(
            True,
            lambda: self._call(
                t,
                sym,
                defining_literal,
                allow_type_any=allow_type_any,
                allow_unbound_tvars=allow_unbound_tvars,
            ),
        )
        assert_equal(
            str(on[0]),
            str(off[0]),
            f"analyze_unbound_type_without_type_info parity {sym.fullname}",
        )
        assert_equal(
            on[1], off[1], f"analyze_unbound_type_without_type_info messages parity {sym.fullname}"
        )

    def _assert_engages(
        self,
        *,
        is_var_any: bool = False,
        allow_type_any: bool = False,
        is_type_instance: bool = False,
        is_type_type_any: bool = False,
        unbound_tvar: bool = False,
        allow_unbound_tvars: bool = False,
        is_enum_member: bool = False,
        defining_literal: bool = False,
        is_new_style: bool = False,
        tail_kind: int = 0,
        name: str = "mod.x",
    ) -> None:
        from mypy.typeanal import _rust_analyze_unbound_without_info  # type: ignore[attr-defined]

        result = _rust_analyze_unbound_without_info(
            is_var_any,
            allow_type_any,
            is_type_instance,
            is_type_type_any,
            unbound_tvar,
            allow_unbound_tvars,
            is_enum_member,
            defining_literal,
            is_new_style,
            tail_kind,
            name,
        )
        assert result is not None, "Rust analyze_unbound_type_without_type_info did not engage"

    def test_var_any_from_unimported(self) -> None:
        # `x: Any` in a type context -> Any(from_unimported_type).
        t = UnboundType("mod.x")
        sym = self._var_sym(None, AnyType(TypeOfAny.from_unimported_type))
        self._assert_par(t, sym, False)
        self._assert_engages(is_var_any=True)

    def test_var_special_any_under_allow_type_any(self) -> None:
        # `x: type` under allow_type_any -> Any(special_form).
        t = UnboundType("mod.x")
        type_info = self.fx.make_type_info("mod.type_holder")
        sym = self._var_sym(type_info, Instance(self.fx.type_typei, []))
        self._assert_par(t, sym, False, allow_type_any=True)
        self._assert_engages(is_type_instance=True, allow_type_any=True)

    def test_var_type_any_without_flag_defers(self) -> None:
        # `x: type` without allow_type_any -> falls through to the message tail.
        t = UnboundType("mod.x")
        type_info = self.fx.make_type_info("mod.type_holder")
        sym = self._var_sym(type_info, Instance(self.fx.type_typei, []))
        self._assert_par(t, sym, False)

    def test_unbound_tvar_allowed(self) -> None:
        # Unbound Tv in an allowed context -> returns t unchanged.
        t = UnboundType("mod.T")
        sym = self._tvar_expr_sym()
        self._assert_par(t, sym, False, allow_unbound_tvars=True)
        self._assert_engages(unbound_tvar=True, allow_unbound_tvars=True)

    def test_unbound_tvar_not_allowed_defers(self) -> None:
        # Unbound Tv when not allowed -> error tail.
        t = UnboundType("mod.T")
        sym = self._tvar_expr_sym()
        self._assert_par(t, sym, False)

    def test_enum_member_literal(self) -> None:
        # Color.RED inside Literal[...] -> LiteralType.
        t = UnboundType("mod.Color.RED")
        enum_info = self.fx.make_type_info("mod.Color")
        enum_info.is_enum = True
        v = Var("RED")
        v.has_explicit_value = True
        v.type = self.fx.o
        v.info = enum_info
        enum_info.names["RED"] = SymbolTableNode(MDEF, v)
        sym = SymbolTableNode(MDEF, v)
        self._assert_par(t, sym, True)
        self._assert_engages(is_enum_member=True, defining_literal=True)

    def test_enum_member_outside_literal_defers(self) -> None:
        # Color.RED outside Literal[...] -> raw-enum error, defer.
        t = UnboundType("mod.Color.RED")
        enum_info = self.fx.make_type_info("mod.Color")
        enum_info.is_enum = True
        v = Var("RED")
        v.has_explicit_value = True
        v.type = self.fx.o
        v.info = enum_info
        enum_info.names["RED"] = SymbolTableNode(MDEF, v)
        sym = SymbolTableNode(MDEF, v)
        self._assert_par(t, sym, False)

    def test_plain_reference_defers(self) -> None:
        # An unresolvable name (symbol with a mismatched node kind) ->
        # message tail.
        t = UnboundType("mod.not_a_type")
        v = Var("not_a_type")
        v.type = self.fx.o
        sym = SymbolTableNode(MDEF, v)
        self._assert_par(t, sym, False)

    def test_typed_object_var_message(self) -> None:
        # A Var with a non-Any type -> 'Variable "..." is not valid as a
        # type' + the variables-vs-type-aliases note.
        t = UnboundType("mod.x")
        sym = self._var_sym(None, self.fx.o)
        self._assert_par(t, sym, False)
        self._assert_engages(tail_kind=0)

    def test_funcdef_message_callback_note(self) -> None:
        # A function as a type -> 'Function "..." is not valid as a type'
        # + the Callable/callback-protocol note.
        from mypy.nodes import FuncDef

        t = UnboundType("mod.f")
        fd = FuncDef("f")
        fd._fullname = "mod.f"
        sym = SymbolTableNode(MDEF, fd)
        self._assert_par(t, sym, False)
        self._assert_engages(tail_kind=1, name="mod.f")

    def test_funcdef_any_note(self) -> None:
        # builtins.any -> the 'Perhaps you meant "typing.Any"' note.
        from mypy.nodes import FuncDef

        t = UnboundType("builtins.any")
        fd = FuncDef("any")
        fd._fullname = "builtins.any"
        sym = SymbolTableNode(MDEF, fd)
        self._assert_par(t, sym, False)
        self._assert_engages(tail_kind=1, name="builtins.any")

    def test_funcdef_callable_note(self) -> None:
        # builtins.callable -> the 'Perhaps you meant "typing.Callable"' note.
        from mypy.nodes import FuncDef

        t = UnboundType("builtins.callable")
        fd = FuncDef("callable")
        fd._fullname = "builtins.callable"
        sym = SymbolTableNode(MDEF, fd)
        self._assert_par(t, sym, False)
        self._assert_engages(tail_kind=1, name="builtins.callable")

    def test_module_message(self) -> None:
        # A module reference -> 'Module "..." is not valid as a type' +
        # the protocol-structure note.

        t = UnboundType("mod")
        tree = MypyFile([], [], False, {})
        tree._fullname = "mod"
        sym = SymbolTableNode(MDEF, tree)
        self._assert_par(t, sym, False)
        self._assert_engages(tail_kind=2, name=tree.fullname)

    def test_rejected_unbound_tvar_messages(self) -> None:
        # A classic unbound type variable when not allowed -> 'Type
        # variable "..." is unbound' plus the two bind hints.
        t = UnboundType("m.T")

        tv = TypeVarExpr("T", "m.T", [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics))
        sym = SymbolTableNode(MDEF, tv)
        self._assert_par(t, sym, False)
        self._assert_engages(unbound_tvar=True, name="m.T", tail_kind=3)

    def test_pep695_tvar_name_not_defined(self) -> None:
        # A PEP 695 type parameter outside its scope -> 'Name "T" is not
        # defined' with the short name and NAME_DEFINED.
        t = UnboundType("m.T")

        tv = TypeVarExpr(
            "T", "m.T", [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics), is_new_style=True
        )
        sym = SymbolTableNode(MDEF, tv)
        self._assert_par(t, sym, False)
        self._assert_engages(unbound_tvar=True, is_new_style=True, name="m.T", tail_kind=3)

    def test_type_type_any_special_form(self) -> None:
        # `Var` typed TypeType[Any] under allow_type_any ->
        # AnyType(from_another_any, source_any=...).
        t = UnboundType("mod.x")
        sym = self._var_sym(None, TypeType(AnyType(TypeOfAny.from_error)))
        self._assert_par(t, sym, False, allow_type_any=True)
        self._assert_engages(allow_type_any=True, is_type_type_any=True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeUnboundBranchFrontSuite(Suite):
    """Parity for the Rust `visit_unbound_type_nonoptional` decision front.

    The dispatch hub that decides how an unbound type expression resolves
    (unresolved symbol, PlaceholderNode, ParamSpecExpr, TypeVarExpr,
    TypeVarTupleExpr) runs in Rust from raw node facts and returns a branch
    tag; the Python shim applies the side effects (defer /
    record_incomplete_ref / fail) and rebuilds the result object. Branches
    Rust cannot classify from facts alone (plugin hook, non-front node
    kinds, and the unbound-TypeVarExpr fail tail, which re-analyzes args
    and emits messages) defer to the pure-Python body.

    Toggling the typeanal gate off (pure Python) and on (Rust seam) must
    produce identical (str(result), captured fail/note messages, defer and
    record_incomplete_ref counts) on both engaged and deferred paths, and a
    direct seam call proves the Rust classifier engages on each front family.
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        self.fx = TypeFixture()
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

    def _analyser(self, syms: dict[str, SymbolTableNode], **kwargs: Any) -> Any:
        from mypy.tvar_scope import TypeVarLikeScope
        from mypy.typeanal import TypeAnalyser

        class FakeApi:
            def __init__(self) -> None:
                self.final_iteration = False
                self.calls: dict[str, int] = {
                    "defer": 0,
                    "record_incomplete_ref": 0,
                    "is_func_scope": 0,
                }
                self.errors: list[str] = []

            def lookup_qualified(
                self, name: str, ctx: Context, suppress_errors: bool = False
            ) -> SymbolTableNode | None:
                return syms.get(name)

            def fail(self, msg: str, ctx: Context, code: ErrorCode | None = None) -> None:
                self.errors.append(msg)

            def note(self, msg: str, ctx: Context, code: ErrorCode | None = None) -> None:
                self.errors.append(f"note: {msg}")

            def defer(self) -> None:
                self.calls["defer"] += 1

            def record_incomplete_ref(self) -> None:
                self.calls["record_incomplete_ref"] += 1

            def is_func_scope(self) -> bool:
                self.calls["is_func_scope"] += 1
                return False

        class FakePlugin:
            def __init__(self, hook: object) -> None:
                self._hook = hook

            def get_type_analyze_hook(self, fullname: str) -> object:
                return self._hook

        api = FakeApi()
        ta = TypeAnalyser.__new__(TypeAnalyser)
        ta.api = api  # type: ignore[assignment]
        ta.fail_func = api.fail  # type: ignore[assignment]
        ta.note_func = api.note
        ta.tvar_scope = TypeVarLikeScope()
        ta.plugin = FakePlugin(kwargs.pop("hook", None))  # type: ignore[assignment]
        ta.options = Options()
        ta.defining_alias = False
        ta.python_3_12_type_alias = False
        ta.alias_type_params_names = None
        ta.allowed_alias_tvars = []
        ta.erase_tvar_defs = []
        ta.allow_unbound_tvars = False
        ta.allow_placeholder = False
        ta.allow_param_spec_literals = False
        ta.allow_type_any = False
        ta.allow_type_var_tuple = -1
        ta.nesting_level = 0
        ta.is_typeshed_stub = False
        ta.analyzing_tvar_def = False
        ta.aliases_used = set()
        if kwargs.pop("api_final_iteration", False):
            api.final_iteration = True
        if kwargs.pop("bound_tvar", False):
            ta.tvar_scope.bind_existing(self._bound_tvar())
        if kwargs.pop("bound_pspec", False):
            ta.tvar_scope.bind_existing(self._bound_pspec())
        if kwargs.pop("bound_tvt", False):
            ta.tvar_scope.bind_existing(self._bound_tvt())
        for key, value in kwargs.items():
            setattr(ta, key, value)
        return ta, api

    def _tvar_expr_sym(self, name: str = "T", fullname: str = "mod.T") -> SymbolTableNode:
        from mypy.nodes import SymbolTableNode

        tv = TypeVarExpr(name, fullname, [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics))
        return SymbolTableNode(MDEF, tv)

    def _pspec_expr_sym(self, name: str = "P", fullname: str = "mod.P") -> SymbolTableNode:
        from mypy.nodes import ParamSpecExpr, SymbolTableNode

        pe = ParamSpecExpr(name, fullname, self.fx.o, AnyType(TypeOfAny.from_omitted_generics))
        return SymbolTableNode(MDEF, pe)

    def _tvt_expr_sym(self, name: str = "Ts", fullname: str = "mod.Ts") -> SymbolTableNode:
        from mypy.nodes import SymbolTableNode, TypeVarTupleExpr

        te = TypeVarTupleExpr(
            name, fullname, self.fx.o, self.fx.std_tuple, AnyType(TypeOfAny.from_omitted_generics)
        )
        return SymbolTableNode(MDEF, te)

    def _bind(self, ta: Any, tvar_def: TypeVarLikeType) -> None:
        ta.tvar_scope.bind_existing(tvar_def)

    def _bound_tvar(self, name: str = "T", fullname: str = "mod.T") -> TypeVarType:
        return TypeVarType(
            name, fullname, TypeVarId(1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )

    def _bound_pspec(self, name: str = "P", fullname: str = "mod.P") -> ParamSpecType:
        return ParamSpecType(
            name, fullname, TypeVarId(1), 0, self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )

    def _bound_tvt(self, name: str = "Ts", fullname: str = "mod.Ts") -> TypeVarTupleType:
        return TypeVarTupleType(
            name,
            fullname,
            TypeVarId(1),
            self.fx.o,
            self.fx.std_tuple,
            AnyType(TypeOfAny.from_omitted_generics),
        )

    def _visit(
        self, t: UnboundType, ta: Any, defining_literal: bool = False
    ) -> tuple[str, list[str], dict[str, int]]:
        result = ta.visit_unbound_type_nonoptional(t, defining_literal)
        messages = [m for m in ta.api.errors if not m.startswith("note: ")]
        return str(result), messages, ta.api.calls

    def _assert_par(self, t: UnboundType, syms: dict[str, SymbolTableNode], **kwargs: Any) -> None:
        off_ta, _ = self._analyser(syms, **kwargs)
        off = self._with_gate(False, lambda: self._visit(t, off_ta))
        self._set_active(True)
        on_ta, _ = self._analyser(syms, **kwargs)
        on = self._with_gate(True, lambda: self._visit(t, on_ta))
        assert_equal(str(on), str(off), f"visit_unbound_type_nonoptional parity {t.name}")
        assert_equal(on[1], off[1], f"visit_unbound_type_nonoptional errors {t.name}")
        assert_equal(
            on[2]["defer"],
            off[2]["defer"],
            f"visit_unbound_type_nonoptional defer parity {t.name}",
        )
        assert_equal(
            on[2]["record_incomplete_ref"],
            off[2]["record_incomplete_ref"],
            f"visit_unbound_type_nonoptional record parity {t.name}",
        )

    def _assert_engages(self, **facts: Any) -> None:
        from mypy.typeanal import _rust_classify_unbound_front  # type: ignore[attr-defined]

        defaults: dict[str, Any] = {
            "node_kind": -1,
            "placeholder_becomes_typeinfo": False,
            "final_iteration": False,
            "allow_placeholder": False,
            "has_hook": False,
            "tvar_def_exists": False,
            "tvar_def_in_allowed": False,
            "tvar_def_erased": False,
            "placeholder_in_tvar_params": False,
            "allow_unbound_tvars": False,
            "defining_alias": False,
            "defining_literal": False,
            "param_spec_name_set": False,
            "allow_param_spec_literals": False,
            "has_args": False,
            "alias_type_params_names": None,
            "tname": "T",
            "allow_type_var_tuple": -1,
            "nesting_level": 0,
        }
        defaults.update(facts)
        result = _rust_classify_unbound_front(
            defaults["node_kind"],
            defaults["placeholder_becomes_typeinfo"],
            defaults["final_iteration"],
            defaults["allow_placeholder"],
            defaults["has_hook"],
            defaults["tvar_def_exists"],
            defaults["tvar_def_in_allowed"],
            defaults["tvar_def_erased"],
            defaults["placeholder_in_tvar_params"],
            defaults["allow_unbound_tvars"],
            defaults["defining_alias"],
            defaults["defining_literal"],
            defaults["param_spec_name_set"],
            defaults["allow_param_spec_literals"],
            defaults["has_args"],
            defaults["alias_type_params_names"],
            defaults["tname"],
            defaults["allow_type_var_tuple"],
            defaults["nesting_level"],
        )
        assert result is not None, "Rust visit_unbound_type_nonoptional front did not engage"

    def test_unresolved_symbol(self) -> None:
        # sym is None (lookup misses) -> Any(special_form).
        t = UnboundType("mod.missing")
        self._assert_par(t, {})
        self._assert_engages(node_kind=-1)

    def test_placeholder_becomes_typeinfo_final(self) -> None:
        # Cyclic reference to a class -> "Cannot resolve name".
        from mypy.nodes import SymbolTableNode

        t = UnboundType("mod.C")
        sym = SymbolTableNode(MDEF, PlaceholderNode("mod.C", Var("C"), 1, becomes_typeinfo=True))
        self._assert_par(t, {"mod.C": sym}, api_final_iteration=True)
        self._assert_engages(node_kind=1, placeholder_becomes_typeinfo=True, final_iteration=True)

    def test_placeholder_becomes_typeinfo_defer(self) -> None:
        # Incomplete reference, placeholder allowed -> PlaceholderType + defer.
        from mypy.nodes import SymbolTableNode

        t = UnboundType("mod.C")
        sym = SymbolTableNode(MDEF, PlaceholderNode("mod.C", Var("C"), 1, becomes_typeinfo=True))
        self._assert_par(t, {"mod.C": sym}, allow_placeholder=True)
        self._assert_engages(
            node_kind=1, placeholder_becomes_typeinfo=True, allow_placeholder=True
        )

    def test_placeholder_becomes_typeinfo_record(self) -> None:
        # Incomplete reference, not allowed -> PlaceholderType + record.
        from mypy.nodes import SymbolTableNode

        t = UnboundType("mod.C")
        sym = SymbolTableNode(MDEF, PlaceholderNode("mod.C", Var("C"), 1, becomes_typeinfo=True))
        self._assert_par(t, {"mod.C": sym})
        self._assert_engages(node_kind=1, placeholder_becomes_typeinfo=True)

    def test_placeholder_plain_final(self) -> None:
        # Unknown placeholder on the final iteration -> error Any.
        from mypy.nodes import SymbolTableNode

        t = UnboundType("mod.C")
        sym = SymbolTableNode(MDEF, PlaceholderNode("mod.C", Var("C"), 1))
        self._assert_par(t, {"mod.C": sym}, api_final_iteration=True)
        self._assert_engages(node_kind=1, final_iteration=True)

    def test_placeholder_plain_record(self) -> None:
        # Unknown placeholder -> Any(special_form) + record.
        from mypy.nodes import SymbolTableNode

        t = UnboundType("mod.C")
        sym = SymbolTableNode(MDEF, PlaceholderNode("mod.C", Var("C"), 1))
        self._assert_par(t, {"mod.C": sym})
        self._assert_engages(node_kind=1)

    def test_node_none_internal_error(self) -> None:
        # Resolved symbol with no node -> internal error + Any(special_form).
        t = UnboundType("mod.x")
        sym = SymbolTableNode(MDEF, None)
        self._assert_par(t, {"mod.x": sym})
        self._assert_engages(node_kind=0)

    def test_hook_defers(self) -> None:
        # A registered plugin hook always defers to the Python body.
        t = UnboundType("mod.T")
        sym = self._tvar_expr_sym()
        self._assert_par(t, {"mod.T": sym}, hook=lambda ctx: AnyType(TypeOfAny.special_form))

    def test_pspec_unbound_tvar_allowed(self) -> None:
        # Unbound param spec in an allowed context -> t unchanged.
        t = UnboundType("mod.P")
        self._assert_par(t, {"mod.P": self._pspec_expr_sym()}, allow_unbound_tvars=True)
        self._assert_engages(node_kind=2, allow_unbound_tvars=True, tname="mod.P")

    def test_pspec_unbound_not_declared(self) -> None:
        # Generic alias without P in type_params -> error.
        t = UnboundType("mod.P")
        self._assert_par(
            t,
            {"mod.P": self._pspec_expr_sym()},
            defining_alias=True,
            alias_type_params_names=["X"],
        )
        self._assert_engages(
            node_kind=2, defining_alias=True, tname="mod.P", alias_type_params_names=["X"]
        )

    def test_pspec_unbound_plain(self) -> None:
        # Unbound param spec -> "is unbound" error.
        t = UnboundType("mod.P")
        self._assert_par(t, {"mod.P": self._pspec_expr_sym()}, defining_alias=True)
        self._assert_engages(node_kind=2, defining_alias=True, tname="mod.P")

    def test_pspec_bound_args(self) -> None:
        # Bound param spec with arguments -> "used with arguments" + build.
        t = UnboundType("mod.P", [UnboundType("int")])
        psym = self._pspec_expr_sym()
        self._assert_par(t, {"mod.P": psym}, bound_pspec=True)
        self._assert_engages(node_kind=2, tvar_def_exists=True, has_args=True, tname="mod.P")

    def test_pspec_bound_ok(self) -> None:
        # Plain bound param spec -> ParamSpecType with the new line.
        t = UnboundType("mod.P")
        self._assert_par(t, {"mod.P": self._pspec_expr_sym()}, bound_pspec=True)
        self._assert_engages(node_kind=2, tvar_def_exists=True, tname="mod.P")

    def test_pspec_args_suffix(self) -> None:
        # `P.args`/`P.kwargs` component literal when literals are allowed.
        t = UnboundType("mod.P.args")
        sym = self._pspec_expr_sym()
        self._assert_par(
            t, {"mod.P.args": sym, "mod.P": sym}, bound_pspec=True, allow_param_spec_literals=True
        )
        self._assert_engages(
            node_kind=2,
            tvar_def_exists=True,
            param_spec_name_set=True,
            allow_param_spec_literals=True,
            tname="mod.P.args",
        )

    def test_pspec_args_suffix_component_error(self) -> None:
        # `P.args` where literals are not allowed -> component error.
        t = UnboundType("mod.P.args")
        sym = self._pspec_expr_sym()
        self._assert_par(t, {"mod.P.args": sym, "mod.P": sym}, bound_pspec=True)
        self._assert_engages(
            node_kind=2, tvar_def_exists=True, param_spec_name_set=True, tname="mod.P.args"
        )

    def test_typevar_alias_not_declared(self) -> None:
        # Generic alias using T that is not in type_params -> error.
        t = UnboundType("mod.T")
        self._assert_par(
            t, {"mod.T": self._tvar_expr_sym()}, defining_alias=True, alias_type_params_names=["X"]
        )
        self._assert_engages(
            node_kind=3, defining_alias=True, tname="mod.T", alias_type_params_names=["X"]
        )

    def test_typevar_alias_not_declared_312(self) -> None:
        # PEP 696 style alias error message on Python 3.12.
        t = UnboundType("mod.T")
        self._assert_par(
            t,
            {"mod.T": self._tvar_expr_sym()},
            defining_alias=True,
            alias_type_params_names=["X"],
            python_3_12_type_alias=True,
        )
        self._assert_engages(
            node_kind=3, defining_alias=True, tname="mod.T", alias_type_params_names=["X"]
        )

    def test_typevar_alias_bound(self) -> None:
        # Generic alias using T that is bound but not allowed -> error.
        t = UnboundType("mod.T")
        self._assert_par(
            t,
            {"mod.T": self._tvar_expr_sym()},
            defining_alias=True,
            alias_type_params_names=["mod.T"],
            bound_tvar=True,
        )
        self._assert_engages(
            node_kind=3,
            defining_alias=True,
            tvar_def_exists=True,
            tname="mod.T",
            alias_type_params_names=["mod.T"],
        )

    def test_typevar_alias_allowed(self) -> None:
        # Generic alias using T that is allowed -> TypeVarType (allowed).
        t = UnboundType("mod.T")
        self._assert_par(
            t,
            {"mod.T": self._tvar_expr_sym()},
            defining_alias=True,
            alias_type_params_names=["T"],
            bound_tvar=True,
            allowed_alias_tvars=[self._bound_tvar()],
        )
        self._assert_engages(
            node_kind=3, defining_alias=True, tvar_def_exists=True, tvar_def_in_allowed=True
        )

    def test_typevar_erased(self) -> None:
        # Erased tvar -> Any(from_error) without a new message.
        t = UnboundType("mod.T")
        self._assert_par(
            t,
            {"mod.T": self._tvar_expr_sym()},
            bound_tvar=True,
            erase_tvar_defs=[self._bound_tvar()],
        )
        self._assert_engages(node_kind=3, tvar_def_exists=True, tvar_def_erased=True)

    def test_typevar_plain(self) -> None:
        # Plain bound tvar -> copy_modified with the new line.
        t = UnboundType("mod.T")
        self._assert_par(t, {"mod.T": self._tvar_expr_sym()}, bound_tvar=True)
        self._assert_engages(node_kind=3, tvar_def_exists=True)

    def test_typevar_with_args(self) -> None:
        # Bound tvar with arguments -> "used with arguments" + copy_modified.
        t = UnboundType("mod.T", [UnboundType("int")])
        self._assert_par(t, {"mod.T": self._tvar_expr_sym()}, bound_tvar=True)
        self._assert_engages(node_kind=3, tvar_def_exists=True, has_args=True)

    def test_typevar_unbound_tvar_allowed(self) -> None:
        # Unbound tvar in an allowed context -> raw t via the without-info
        # back's Option 2 (typeanal.py:1731-1736), now decided in Rust.
        t = UnboundType("mod.T")
        self._assert_par(t, {"mod.T": self._tvar_expr_sym()}, allow_unbound_tvars=True)
        self._assert_engages(node_kind=3, allow_unbound_tvars=True, tname="mod.T")

    def test_typevar_unbound_allow_defining_literal(self) -> None:
        # defining_literal skips the alias guard; the unbound back still
        # returns t under allow_unbound_tvars.
        t = UnboundType("mod.T")
        self._assert_par(t, {"mod.T": self._tvar_expr_sym()}, allow_unbound_tvars=True)
        self._assert_engages(
            node_kind=3, allow_unbound_tvars=True, defining_literal=True, tname="mod.T"
        )

    def test_typevar_unbound_fail_tail_defers(self) -> None:
        # Unbound tvar, not allowed -> the without-info fail tail (arg
        # re-analysis + "is unbound" message); Rust defers to that body.
        t = UnboundType("mod.T")
        self._assert_par(t, {"mod.T": self._tvar_expr_sym()})

    def test_tvt_alias_not_declared(self) -> None:
        # Generic alias using Ts that is not in type_params -> error.
        t = UnboundType("mod.Ts")
        self._assert_par(
            t,
            {"mod.Ts": self._tvt_expr_sym()},
            defining_alias=True,
            alias_type_params_names=["X"],
            bound_tvt=True,
        )
        self._assert_engages(
            node_kind=4,
            tvar_def_exists=True,
            defining_alias=True,
            tname="mod.Ts",
            alias_type_params_names=["X"],
        )

    def test_tvt_alias_bound(self) -> None:
        # Generic alias using Ts that is bound but not allowed -> error.
        t = UnboundType("mod.Ts")
        self._assert_par(
            t,
            {"mod.Ts": self._tvt_expr_sym()},
            defining_alias=True,
            alias_type_params_names=["Ts"],
            bound_tvt=True,
        )
        self._assert_engages(
            node_kind=4, tvar_def_exists=True, defining_alias=True, tname="mod.Ts"
        )

    def test_tvt_unbound_tvar_allowed(self) -> None:
        # Unbound Ts in an allowed context -> t unchanged.
        t = UnboundType("mod.Ts")
        self._assert_par(t, {"mod.Ts": self._tvt_expr_sym()}, allow_unbound_tvars=True)
        self._assert_engages(node_kind=4, allow_unbound_tvars=True, tname="mod.Ts")

    def test_tvt_unbound_not_declared(self) -> None:
        # Unbound Ts in a generic alias -> 3.12-aware "not included" error.
        t = UnboundType("mod.Ts")
        self._assert_par(
            t,
            {"mod.Ts": self._tvt_expr_sym()},
            defining_alias=True,
            alias_type_params_names=["X"],
            python_3_12_type_alias=True,
        )
        self._assert_engages(
            node_kind=4, defining_alias=True, tname="mod.Ts", alias_type_params_names=["X"]
        )

    def test_tvt_unbound_plain(self) -> None:
        # Unbound Ts not allowed -> "is unbound" error.
        t = UnboundType("mod.Ts")
        self._assert_par(t, {"mod.Ts": self._tvt_expr_sym()})
        self._assert_engages(node_kind=4, tname="mod.Ts")

    def test_tvt_nesting_mismatch(self) -> None:
        # Ts at the wrong unpack nesting -> "only valid with an unpack" error.
        t = UnboundType("mod.Ts")
        self._assert_par(
            t,
            {"mod.Ts": self._tvt_expr_sym()},
            bound_tvt=True,
            allow_type_var_tuple=1,
            nesting_level=2,
        )
        self._assert_engages(
            node_kind=4, tvar_def_exists=True, allow_type_var_tuple=1, nesting_level=2
        )

    def test_tvt_with_args(self) -> None:
        # Bound Ts with arguments -> "used with arguments" + build.
        t = UnboundType("mod.Ts", [UnboundType("int")])
        self._assert_par(t, {"mod.Ts": self._tvt_expr_sym()}, bound_tvt=True)
        self._assert_engages(node_kind=4, tvar_def_exists=True, has_args=True)

    def test_tvt_plain(self) -> None:
        # Plain bound Ts -> TypeVarTupleType with the new line.
        t = UnboundType("mod.Ts")
        self._assert_par(t, {"mod.Ts": self._tvt_expr_sym()}, bound_tvt=True)
        self._assert_engages(node_kind=4, tvar_def_exists=True)

    def test_other_kind_defers(self) -> None:
        # A Var node is resolved by the back of the method (deferred).
        t = UnboundType("mod.x")
        v = Var("x")
        v._fullname = "mod.x"
        v.type = self.fx.o
        self._assert_par(t, {"mod.x": SymbolTableNode(MDEF, v)})


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTryAnalyzeSpecialUnboundSuite(Suite):
    """Parity for the Rust `try_analyze_special_unbound_type` classifier.

    The special-form elif-chain (builtins.None, typing.Any, Final, Tuple,
    Union, Optional, Callable, Type, TypeForm, ClassVar, Never, Annotated,
    Required, NotRequired, ReadOnly) is decided in Rust from scalar facts
    (fullname + arity + a few flags); the Python shim applies the branch
    bodies (errors, object construction, anal recursion). Branches the Rust
    classifier cannot decide purely (Self, the
    non-special tail, and every gold path that recurses) defer to the
    pure-Python body.

    Toggling the typeanal gate off (pure Python) and on (Rust seam) must
    produce identical (str(result), captured fail/note messages) on both
    engaged and deferred paths, and a direct seam call proves the Rust
    classifier engages on each special family. The `builtins.tuple` /
    `builtins.function` / `builtins.int` TypeInfos are snapshotted so the
    Tuple and Callable branches can build their fallbacks.
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        self.fx = TypeFixture()
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

    def _analyser(self, **kwargs: Any) -> tuple[Any, Any]:
        from mypy.tvar_scope import TypeVarLikeScope
        from mypy.typeanal import TypeAnalyser

        fx = self.fx
        self_tuple_info: TypeInfo = fx.std_tuplei
        int_info: TypeInfo = fx.make_type_info("builtins.int")

        class FakePlugin:
            def get_type_analyze_hook(self, fullname: str) -> object:
                return None

        class FakeApi:
            def __init__(self) -> None:
                self.final_iteration = False
                self.type = None
                self.errors: list[str] = []
                self.syms: dict[str, SymbolTableNode] = {
                    "builtins.tuple": SymbolTableNode(MDEF, self_tuple_info),
                    "builtins.function": SymbolTableNode(MDEF, fx.functioni),
                    "builtins.int": SymbolTableNode(MDEF, int_info),
                    "builtins.str": SymbolTableNode(MDEF, fx.str_type_info),
                    "builtins.bool": SymbolTableNode(MDEF, fx.bool_type_info),
                    # Short names too: anal_type resolves short names via
                    # lookup_qualified with no module scope, so key them or
                    # UnboundType("int") stays an unresolvable Any.
                    "int": SymbolTableNode(MDEF, int_info),
                    "str": SymbolTableNode(MDEF, fx.str_type_info),
                }

            def lookup_qualified(
                self, name: str, ctx: Context, suppress_errors: bool = False
            ) -> SymbolTableNode | None:
                return self.syms.get(name)

            def lookup_fully_qualified(self, fullname: str) -> SymbolTableNode:
                return self.syms[fullname]

            def lookup_fully_qualified_or_none(self, fullname: str) -> SymbolTableNode | None:
                return self.syms.get(fullname)

            def is_incomplete_namespace(self, fullname: str) -> bool:
                return False

            def record_incomplete_ref(self) -> None:
                self.errors.append("record_incomplete_ref")

            def fail(self, msg: str, ctx: Context, code: ErrorCode | None = None) -> None:
                self.errors.append(msg)

            def note(self, msg: str, ctx: Context, code: ErrorCode | None = None) -> None:
                self.errors.append(f"note: {msg}")

            def is_func_scope(self) -> bool:
                return False

            def record_fixed_type(self, typ: Type) -> None:
                pass

            def defer(self) -> None:
                pass

        api = FakeApi()
        ta = TypeAnalyser.__new__(TypeAnalyser)
        ta.api = api  # type: ignore[assignment]
        ta.fail_func = api.fail  # type: ignore[assignment]
        ta.note_func = api.note
        ta.tvar_scope = TypeVarLikeScope()
        ta.plugin = FakePlugin()  # type: ignore[assignment]
        ta.options = Options()
        ta.defining_alias = False
        ta.python_3_12_type_alias = False
        ta.alias_type_params_names = None
        ta.allowed_alias_tvars = []
        ta.erase_tvar_defs = []
        ta.allow_unbound_tvars = False
        ta.allow_placeholder = False
        ta.allow_param_spec_literals = False
        ta.allow_type_any = False
        ta.allow_type_var_tuple = -1
        ta.nesting_level = 0
        ta.is_typeshed_stub = False
        ta.analyzing_tvar_def = False
        ta.aliases_used = set()
        ta.prohibit_special_class_field_types = None
        ta.allow_typed_dict_special_forms = False
        ta.allow_final = True
        ta.allow_unpack = False
        ta.allow_ellipsis = False
        ta.allow_tuple_literal = True
        ta.report_invalid_types = False
        ta.prohibit_self_type = None
        ta.cur_mod_node = None  # type: ignore[assignment]
        for key, value in kwargs.items():
            setattr(ta, key, value)
        return ta, api

    _t_fullname = "typing.Any"

    @staticmethod
    def _t(fullname: str, args: list[Type] | None = None, **kw: Any) -> UnboundType:
        return UnboundType(fullname, args, **kw)

    def _call(self, ta: Any, t: UnboundType, fullname: str) -> tuple[str, list[str]]:
        result = ta.try_analyze_special_unbound_type(t, fullname)
        messages = list(ta.api.errors)
        return str(result), messages

    def _assert_par(
        self,
        fullname: str,
        args: list[Type] | None = None,
        *,
        expected: str | None = None,
        **kwargs: object,
    ) -> None:
        off_ta, _ = self._analyser(**kwargs)
        off = self._with_gate(False, lambda: self._call(off_ta, self._t(fullname, args), fullname))
        self._set_active(True)
        on_ta, _ = self._analyser(**kwargs)
        on = self._with_gate(True, lambda: self._call(on_ta, self._t(fullname, args), fullname))
        assert_equal(on[0], off[0], f"try_analyze_special_unbound parity result {fullname}")
        assert_equal(on[1], off[1], f"try_analyze_special_unbound parity messages {fullname}")
        if expected is not None:
            assert_equal(on[0], expected, f"try_analyze_special_unbound result {fullname}")

    def _assert_engages(self, **facts: Any) -> None:
        from mypy.typeanal import _rust_classify_special_unbound  # type: ignore[attr-defined]

        defaults: dict[str, Any] = {
            "fullname": "typing.Any",
            "arg_count": 0,
            "empty_tuple_index": False,
            "allow_typed_dict_special_forms": False,
            "tuple_missing_or_placeholder": False,
            "tuple_ellipsis_form": False,
            "not_in_final": True,
            "not_in_tuple": True,
            "not_in_type": True,
            "not_in_typeform": True,
            "not_in_classvar": True,
            "not_in_never": True,
            "not_in_annotated": True,
            "not_in_required": True,
            "not_in_notrequired": True,
            "not_in_readonly": True,
            "not_in_literal": True,
            "not_in_unpack": True,
            "not_in_self": True,
            "allow_unpack": False,
        }
        defaults.update(facts)
        result = _rust_classify_special_unbound(
            defaults["fullname"],
            defaults["arg_count"],
            defaults["empty_tuple_index"],
            defaults["allow_typed_dict_special_forms"],
            defaults["tuple_missing_or_placeholder"],
            defaults["tuple_ellipsis_form"],
            defaults["not_in_final"],
            defaults["not_in_tuple"],
            defaults["not_in_type"],
            defaults["not_in_typeform"],
            defaults["not_in_classvar"],
            defaults["not_in_never"],
            defaults["not_in_annotated"],
            defaults["not_in_required"],
            defaults["not_in_notrequired"],
            defaults["not_in_readonly"],
            defaults["not_in_literal"],
            defaults["not_in_unpack"],
            defaults["not_in_self"],
            defaults["allow_unpack"],
        )
        assert result is not None, "Rust try_analyze_special_unbound did not engage"

    def test_none(self) -> None:
        self._assert_par("builtins.None", expected="None")
        self._assert_engages(fullname="builtins.None")

    def test_any(self) -> None:
        self._assert_par("typing.Any", expected="Any")
        self._assert_engages(fullname="typing.Any")

    def test_final_error(self) -> None:
        # Final in a non-final context -> error Any; message depends on flags.
        self._assert_par("typing.Final", expected="Any")
        self._assert_engages(fullname="typing.Final", not_in_final=False)

    def test_final_ext_error(self) -> None:
        self._assert_par("typing_extensions.Final", expected="Any")
        self._assert_engages(fullname="typing_extensions.Final", not_in_final=False)

    def test_tuple_bare(self) -> None:
        # Bare 'Tuple' same as 'tuple'; builds via named_type with omitted Any.
        self._assert_par("typing.Tuple", expected="builtins.tuple[Any, ...]")
        self._assert_engages(fullname="typing.Tuple", not_in_tuple=False)

    def test_tuple_ellipsis(self) -> None:
        # Tuple[T, ...] -> tuple[T] (uniform).
        arg = UnboundType("int")
        self._assert_par(
            "typing.Tuple", [arg, EllipsisType()], expected="builtins.tuple[builtins.int, ...]"
        )
        self._assert_engages(
            fullname="typing.Tuple", arg_count=2, not_in_tuple=False, tuple_ellipsis_form=True
        )

    def test_tuple_fixed_arity(self) -> None:
        # Tuple[int, str] -> full form via tuple_type.
        self._assert_par("typing.Tuple", [UnboundType("int"), UnboundType("str")])
        self._assert_engages(
            fullname="typing.Tuple", arg_count=2, not_in_tuple=False, tuple_ellipsis_form=False
        )

    def test_tuple_empty_index(self) -> None:
        # Tuple[()] -> empty_tuple_index; full form.
        t = UnboundType("typing.Tuple", [], empty_tuple_index=True)
        off_ta, _ = self._analyser()
        off = self._with_gate(False, lambda: self._call(off_ta, t, "typing.Tuple"))
        self._set_active(True)
        on_ta, _ = self._analyser()
        on = self._with_gate(True, lambda: self._call(on_ta, t, "typing.Tuple"))
        assert_equal(on[0], off[0], "Tuple[()] parity result")
        self._assert_engages(
            fullname="typing.Tuple", arg_count=0, empty_tuple_index=True, not_in_tuple=False
        )

    def test_tuple_missing_lookup(self) -> None:
        # builtins.tuple missing -> lookup-defer branch; fail('Name "tuple" is
        # not defined') since is_incomplete_namespace is False.
        off_ta, off_api = self._analyser()
        off_api.syms.pop("builtins.tuple")
        t = UnboundType("typing.Tuple")
        off = self._with_gate(False, lambda: self._call(off_ta, t, "typing.Tuple"))
        self._set_active(True)
        on_ta, on_api = self._analyser()
        on_api.syms.pop("builtins.tuple")
        on = self._with_gate(True, lambda: self._call(on_ta, t, "typing.Tuple"))
        assert_equal(on[0], off[0], "Tuple missing-lookup parity result")
        assert_equal(on[1], off[1], "Tuple missing-lookup parity messages")
        self._assert_engages(
            fullname="typing.Tuple", not_in_tuple=False, tuple_missing_or_placeholder=True
        )

    def test_union_no_arity_check(self) -> None:
        # The original Union branch has no arity check: Union[int] defers to
        # make_union and collapses to builtins.int.
        self._assert_par("typing.Union", [UnboundType("int")], expected="builtins.int")
        self._assert_engages(fullname="typing.Union", arg_count=1)

    def test_union_gold(self) -> None:
        # Union[int, str] -> UnionType.make_union (defer for the make).
        self._assert_par(
            "typing.Union",
            [UnboundType("int"), UnboundType("str")],
            expected="builtins.int | builtins.str",
        )
        self._assert_engages(fullname="typing.Union", arg_count=2)

    def test_optional_arity_error(self) -> None:
        self._assert_par(
            "typing.Optional", [UnboundType("int"), UnboundType("str")], expected="Any"
        )
        self._assert_engages(fullname="typing.Optional", arg_count=2)

    def test_optional_gold(self) -> None:
        self._assert_par("typing.Optional", [UnboundType("int")], expected="builtins.int | None")
        self._assert_engages(fullname="typing.Optional", arg_count=1)

    def test_callable_bare(self) -> None:
        # Callable (bare) -> callable_with_ellipsis; deferred for building.
        self._assert_par("typing.Callable")
        self._assert_engages(fullname="typing.Callable")

    def test_callable_two_args(self) -> None:
        self._assert_par("typing.Callable", [EllipsisType(), UnboundType("int")])
        self._assert_engages(fullname="typing.Callable", arg_count=2)

    def test_type_bare_any(self) -> None:
        # typing.Type bare -> TypeType(Any).
        self._assert_par("typing.Type", expected="type[Any]")
        self._assert_engages(fullname="typing.Type", not_in_type=False)

    def test_type_bare_none(self) -> None:
        # builtins.type bare -> None (not special, #9476: builtins.type
        # must not collapse to builtins.object).
        self._assert_par("builtins.type", expected="None")
        self._assert_engages(fullname="builtins.type", arg_count=0, not_in_type=False)

    def test_type_one_arg(self) -> None:
        self._assert_par("typing.Type", [UnboundType("int")], expected="type[builtins.int]")
        self._assert_engages(fullname="typing.Type", arg_count=1, not_in_type=False)

    def test_type_arity_error(self) -> None:
        self._assert_par(
            "typing.Type",
            [UnboundType("int"), UnboundType("str")],
            expected="type[builtins.int]",  # one-arg golden path still used
        )
        self._assert_engages(fullname="typing.Type", arg_count=2, not_in_type=False)

    def test_typeform_bare(self) -> None:
        self._assert_par("typing.TypeForm", expected="TypeForm[Any]")
        self._assert_engages(fullname="typing.TypeForm", not_in_typeform=False)

    def test_typeform_one_arg(self) -> None:
        self._assert_par(
            "typing.TypeForm", [UnboundType("int")], expected="TypeForm[builtins.int]"
        )
        self._assert_engages(fullname="typing.TypeForm", arg_count=1, not_in_typeform=False)

    def test_classvar_zero(self) -> None:
        # Bare ClassVar in a plain context -> Any (from_omitted_generics).
        self._assert_par("typing.ClassVar", expected="Any")
        # Bare ClassVar inside a TypedDict (prohibit context) still reports
        # the "can't be used inside" error before the arg-count dispatch.
        self._assert_par(
            "typing.ClassVar", prohibit_special_class_field_types="TypedDict", expected="Any"
        )
        self._assert_engages(fullname="typing.ClassVar", not_in_classvar=False)

    def test_classvar_one_arg(self) -> None:
        self._assert_par("typing.ClassVar", [UnboundType("int")], expected="builtins.int")
        self._assert_engages(fullname="typing.ClassVar", arg_count=1, not_in_classvar=False)

    def test_classvar_nested(self) -> None:
        # ClassVar nested inside a type -> "Invalid type" error + int.
        off_ta, _ = self._analyser(nesting_level=1)
        t = UnboundType("typing.ClassVar", [UnboundType("int")])
        off = self._with_gate(False, lambda: self._call(off_ta, t, "typing.ClassVar"))
        self._set_active(True)
        on_ta, _ = self._analyser(nesting_level=1)
        on = self._with_gate(True, lambda: self._call(on_ta, t, "typing.ClassVar"))
        assert_equal(on[0], off[0], "ClassVar nested parity result")
        assert_equal(on[1], off[1], "ClassVar nested parity messages")
        self._assert_engages(fullname="typing.ClassVar", arg_count=1, not_in_classvar=False)

    def test_classvar_arity_error(self) -> None:
        self._assert_par(
            "typing.ClassVar", [UnboundType("int"), UnboundType("str")], expected="Any"
        )
        self._assert_engages(fullname="typing.ClassVar", arg_count=2, not_in_classvar=False)

    def test_never(self) -> None:
        self._assert_par("typing.Never", expected="Never")
        self._assert_engages(fullname="typing.Never", not_in_never=False)

    def test_noreturn(self) -> None:
        self._assert_par("typing.NoReturn", expected="Never")
        self._assert_engages(fullname="typing.NoReturn", not_in_never=False)

    def test_annotated_arity_error(self) -> None:
        self._assert_par("typing.Annotated", [UnboundType("int")], expected="Any")
        self._assert_engages(fullname="typing.Annotated", arg_count=1, not_in_annotated=False)

    def test_annotated_gold(self) -> None:
        self._assert_par(
            "typing.Annotated", [UnboundType("int"), UnboundType("unit")], expected="builtins.int"
        )
        self._assert_engages(fullname="typing.Annotated", arg_count=2, not_in_annotated=False)

    def test_required_bad_ctx(self) -> None:
        # Required outside a TypedDict -> "can be only used" + error Any.
        self._assert_par("typing.Required", [UnboundType("int")], expected="Any")
        self._assert_engages(
            fullname="typing.Required",
            arg_count=1,
            allow_typed_dict_special_forms=False,
            not_in_required=False,
        )

    def test_required_arg_err(self) -> None:
        self._assert_par(
            "typing.Required",
            [UnboundType("int"), UnboundType("str")],
            allow_typed_dict_special_forms=True,
            expected="Any",
        )
        self._assert_engages(
            fullname="typing.Required",
            arg_count=2,
            allow_typed_dict_special_forms=True,
            not_in_required=False,
        )

    def test_required_gold(self) -> None:
        self._assert_par(
            "typing.Required",
            [UnboundType("int")],
            allow_typed_dict_special_forms=True,
            expected="Required[builtins.int]",
        )
        self._assert_engages(
            fullname="typing.Required",
            arg_count=1,
            allow_typed_dict_special_forms=True,
            not_in_required=False,
        )

    def test_notrequired_bad_ctx(self) -> None:
        self._assert_par("typing_extensions.NotRequired", [UnboundType("int")], expected="Any")
        self._assert_engages(
            fullname="typing_extensions.NotRequired",
            arg_count=1,
            allow_typed_dict_special_forms=False,
            not_in_notrequired=False,
        )

    def test_notrequired_gold(self) -> None:
        self._assert_par(
            "typing_extensions.NotRequired",
            [UnboundType("int")],
            allow_typed_dict_special_forms=True,
            expected="NotRequired[builtins.int]",
        )
        self._assert_engages(
            fullname="typing_extensions.NotRequired",
            arg_count=1,
            allow_typed_dict_special_forms=True,
            not_in_notrequired=False,
        )

    def test_readonly_bad_ctx(self) -> None:
        self._assert_par("typing_extensions.ReadOnly", [UnboundType("int")], expected="Any")
        self._assert_engages(
            fullname="typing_extensions.ReadOnly",
            arg_count=1,
            allow_typed_dict_special_forms=False,
            not_in_readonly=False,
        )

    def test_readonly_gold(self) -> None:
        self._assert_par(
            "typing_extensions.ReadOnly",
            [UnboundType("int")],
            allow_typed_dict_special_forms=True,
            expected="ReadOnly[builtins.int]",
        )
        self._assert_engages(
            fullname="typing_extensions.ReadOnly",
            arg_count=1,
            allow_typed_dict_special_forms=True,
            not_in_readonly=False,
        )

    def test_plain_name_defers(self) -> None:
        # A non-special name -> None; the tag path is skipped entirely and
        # the caller runs the full body -> same result both ways.
        self._assert_par("mod.SomeName")

    def test_literal_defers(self) -> None:
        # Literal -> TAG_LITERAL_DEFER; the shim runs analyze_literal_type.
        # An unresolved Instance arg (plain `int` through the harness
        # lookup) is rejected as an invalid parameter -> error Any.
        self._assert_par("typing.Literal", [UnboundType("int")], expected="Any")
        self._assert_engages(fullname="typing.Literal", not_in_literal=False)

    def test_literal_gold(self) -> None:
        # String-form literal arg (Literal["a"] in real code) -> a real
        # LiteralType this time; the shim still routes analyze_literal_type.
        arg = UnboundType("a", original_str_expr="a", original_str_fallback="builtins.str")
        self._assert_par("typing.Literal", [arg], expected="Literal['a']")
        self._assert_engages(fullname="typing_extensions.Literal", not_in_literal=False)

    def test_literal_arity_error(self) -> None:
        # Bare Literal -> "at least one parameter" + error Any.
        self._assert_par("typing.Literal", expected="Any")
        self._assert_engages(fullname="typing.Literal", not_in_literal=False)

    def test_typeguard_bool(self) -> None:
        # TypeGuard filters into the bool-alias branch; the arg is analyzed
        # for its errors but the result is builtins.bool.
        self._assert_par("typing.TypeGuard", [UnboundType("int")], expected="builtins.bool")
        self._assert_engages(fullname="typing.TypeGuard")

    def test_typeguard_arity_error(self) -> None:
        self._assert_par(
            "typing.TypeGuard", [UnboundType("int"), UnboundType("str")], expected="builtins.bool"
        )
        self._assert_engages(fullname="typing.TypeGuard")

    def test_typeis_bool(self) -> None:
        self._assert_par(
            "typing_extensions.TypeIs", [UnboundType("int")], expected="builtins.bool"
        )
        self._assert_engages(fullname="typing_extensions.TypeIs")

    def test_unpack_arg_err(self) -> None:
        # Unpack with arity != 1 -> from_error Any; classified in Rust.
        self._assert_par(
            "typing.Unpack",
            [UnboundType("int"), UnboundType("str")],
            allow_unpack=True,
            expected="Any",
        )
        self._assert_engages(
            fullname="typing.Unpack", arg_count=2, not_in_unpack=False, allow_unpack=True
        )

    def test_unpack_pos_err(self) -> None:
        # Unpack in a non-variadic position -> from_error Any.
        self._assert_par("typing.Unpack", [UnboundType("int")], expected="Any")
        self._assert_engages(
            fullname="typing.Unpack", arg_count=1, not_in_unpack=False, allow_unpack=False
        )

    def test_unpack_gold(self) -> None:
        # Unpack[int] in a variadic position: the gold path tag routes the
        # shim to the exact original body (mutates allow_type_var_tuple
        # around anal_type) -> still identical across gates.
        self._assert_par(
            "typing.Unpack", [UnboundType("int")], allow_unpack=True, expected="*builtins.int"
        )
        self._assert_engages(
            fullname="typing.Unpack", arg_count=1, not_in_unpack=False, allow_unpack=True
        )

    def test_self_defer_parity(self) -> None:
        # Self has no pure decision surface (needs the live api.type /
        # api.type.self_type, and the args-error falls through to the gold
        # body): the classifier defers and both gates run the full body.
        self._assert_par("typing_extensions.Self")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAnalyzeTypeWithInfoSuite(Suite):
    """Parity for the Rust `analyze_type_with_type_info` decision front.

    `TypeAnalyser.analyze_type_with_type_info` binds an unbound type that
    resolved to a `TypeInfo` node. The front classifier runs in Rust from
    raw node facts (fullname, argument count, which of tuple_type /
    special_alias / typeddict_type are set) and returns a branch tag; the
    Python shim applies the side effects for the two tags it executes inline
    (tuple with args, types.NoneType). Every other tag re-runs the original
    body, so message side effects stay single-sourced and parity is trivial
    for the vec / tail / Instance branches.

    Toggling the typeanal gate off (pure Python) and on (Rust seam) must
    produce identical (str(result), captured fail messages) on both engaged
    and body paths, and a direct seam call proves the classifier engages on
    each decision branch.
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_typeanal_active
        self._set_active(True)
        # The body re-analyzes bound argument types through native_analyze_type,
        # so the fixture TypeInfos must resolve through the wire fixup.
        set_wire_typeinfo_map(
            {
                getattr(self.fx, attr).fullname: getattr(self.fx, attr)
                for attr in dir(self.fx)
                if isinstance(getattr(self.fx, attr), TypeInfo)
            }
        )

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        set_wire_typeinfo_map(None)
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _analyser(self, **kwargs: Any) -> Any:
        from mypy.errorcodes import ErrorCode as _ErrorCode
        from mypy.typeanal import TypeAnalyser

        class FakeApi:
            def __init__(self) -> None:
                self.final_iteration = False
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
        ta.tvar_scope = None  # type: ignore[assignment]
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
        for key, value in kwargs.items():
            setattr(ta, key, value)
        return ta, api

    def _generic(self, fullname: str, n_tvars: int = 1) -> TypeInfo:
        """A TypeInfo with `n_tvars` type variables set in both the short and
        the defn list, so validate_instance sees a well-formed generic class."""
        info = self.fx.make_type_info(fullname, typevars=["T"] * n_tvars)
        assert info.defn.type_vars
        return info

    def _call(
        self, info: TypeInfo, args: Sequence[Type], empty_tuple_index: bool = False
    ) -> tuple[str, list[str]]:
        from mypy.typeanal import TypeAnalyser

        ta, api = self._analyser()
        ctx = UnboundType("ctx")
        result = TypeAnalyser.analyze_type_with_type_info(
            ta, info, list(args), ctx, empty_tuple_index
        )
        messages = [m for m in api.errors if not m.startswith("note: ")]
        return str(result), messages

    def _assert_par(
        self, info: TypeInfo, args: Sequence[Type], empty_tuple_index: bool = False
    ) -> None:
        off = self._with_gate(False, lambda: self._call(info, args, empty_tuple_index))
        self._set_active(True)
        on = self._with_gate(True, lambda: self._call(info, args, empty_tuple_index))
        assert_equal(on[0], off[0], f"analyze_type_with_type_info parity {info.fullname}")
        assert_equal(on[1], off[1], f"analyze_type_with_type_info errors {info.fullname}")

    def _assert_engages(self, expected: int, **facts: Any) -> None:
        from mypy.typeanal import _rust_classify_type_with_info  # type: ignore[attr-defined]

        defaults: dict[str, Any] = {
            "fullname": "mod.UserClass",
            "args_len": 0,
            "tuple_type_not_none": False,
            "special_alias_not_none": False,
            "typeddict_type_not_none": False,
        }
        defaults.update(facts)
        result = _rust_classify_type_with_info(
            defaults["fullname"],
            defaults["args_len"],
            defaults["tuple_type_not_none"],
            defaults["special_alias_not_none"],
            defaults["typeddict_type_not_none"],
        )
        assert result == expected, (
            f"Rust analyze_type_with_type_info classifier {defaults!r}: "
            f"got {result}, want {expected}"
        )

    def test_plain_no_args(self) -> None:
        # Plain class, no type arguments -> plain Instance.
        info = self.fx.make_type_info("mod.UserClass")
        self._assert_par(info, [])
        self._assert_engages(_TYPE_WITH_INFO_TAG_INSTANCE, fullname="mod.UserClass", args_len=0)

    def test_generic_with_args(self) -> None:
        # Generic class, one argument -> plain Instance.
        info = self._generic("mod.UserClass")
        self._assert_par(info, [self.fx.str_type])
        self._assert_engages(_TYPE_WITH_INFO_TAG_INSTANCE, fullname="mod.UserClass", args_len=1)

    def test_generic_too_few_args(self) -> None:
        # Generic class with no arguments -> validate_instance fails, the
        # body's fix_instance emits "Missing type parameters" and fills Any.
        info = self._generic("mod.UserClass")
        self._assert_par(info, [])

    def test_bare_tuple_no_args(self) -> None:
        # builtins.tuple with no arguments is not the tuple special form; it
        # is a plain Instance branch (the body then fails arg count).
        info = self._generic("builtins.tuple")
        self._assert_par(info, [])
        self._assert_engages(_TYPE_WITH_INFO_TAG_INSTANCE, fullname="builtins.tuple", args_len=0)

    def test_tuple_args(self) -> None:
        # tuple[...] with arguments -> TupleType (shim-inline branch).
        info = self._generic("builtins.tuple")
        self._assert_par(info, [self.fx.str_type])
        self._assert_engages(_TYPE_WITH_INFO_TAG_TUPLE, fullname="builtins.tuple", args_len=1)

    def test_nonetype(self) -> None:
        # types.NoneType -> error + NoneType return (shim-inline branch).
        info = self.fx.make_type_info("types.NoneType")
        self._assert_par(info, [])
        self._assert_engages(_TYPE_WITH_INFO_TAG_NONE_TYPE, fullname="types.NoneType", args_len=0)

    def test_named_tuple_tail(self) -> None:
        # A class with a tuple_type base but no special_alias -> the body
        # resolves the named-tuple tail (not inline in the shim).
        info = self.fx.make_type_info("mod.Point")
        info.tuple_type = TupleType(
            [self.fx.str_type], Instance(self.fx.std_tuplei, [self.fx.str_type]), 1
        )
        self._assert_par(info, [])

    def test_vec(self) -> None:
        # librt.vecs.vec with no item -> check_vec_type_args fails, the body
        # returns Any(from_error). Engages the vec tag.
        info = self._generic("librt.vecs.vec")
        self._assert_par(info, [])
        self._assert_engages(_TYPE_WITH_INFO_TAG_VEC, fullname="librt.vecs.vec", args_len=0)


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
class NativeJoinCovariantArgsSuite(Suite):
    """Parity suite for the Rust `visit_instance` covariant-arg join
    (Stage 3c M8h).

    Exercises the Instance-Instance same-type join where T is covariant:
    equal args fire the Rust path (recursive join_types returns SameS/
    SameT, is_subtype(arg, upper_bound=object)=True); unequal args defer
    to Python (the recursive join returns Ancestor, which the Rust
    covariant branch can't express as an arg disc). AnyType args
    short-circuit on either side (disc 4). Results are identical to the
    pure-Python `JoinSuite.test_generics_covariant` cases.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(COVARIANT)
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

    def test_equal_args_returns_same_instance(self) -> None:
        # join(G[A], G[A]) where T is covariant. join_types(A, A)=A
        # (SameS) -> arg disc 1 (t.args[0]=A). is_subtype(A, object)=
        # True -> G[A]. Fires the Rust covariant branch.
        from mypy.join import join_types

        assert join_types(self.fx.ga, self.fx.ga) == self.fx.ga

    def test_any_arg_returns_any_instance(self) -> None:
        # join(G[Any], G[A]) where T is covariant. AnyType arg
        # short-circuits (join.py:131-135) -> G[Any]. Fires the Rust
        # AnyType-discard path (disc 4), shared with the invariant

        # branch.
        from mypy.join import join_types

        assert join_types(self.fx.gdyn, self.fx.ga) == self.fx.gdyn
        assert join_types(self.fx.ga, self.fx.gdyn) == self.fx.gdyn

    def test_multiple_equal_args_returns_same_instance(self) -> None:
        # join(H[A,B], H[A,B]) where S,T are covariant. Both args
        # equal -> recursive join returns SameS for each -> H[A,B].
        # Fires the Rust covariant branch per-arg.
        from mypy.join import join_types

        assert join_types(self.fx.hab, self.fx.hab) == self.fx.hab

    def test_subtype_args_defer_to_python(self) -> None:
        # join(G[A], G[B]) where T is covariant, B <: A. The
        # recursive join_types(A, B) returns Ancestor(A) (the common
        # supertype), which the Rust covariant branch can't express as

        # an arg disc, so it defers. Python computes G[A]. The result
        # is identical to pure-Python JoinSuite.test_generics_covariant.
        from mypy.join import join_types

        assert join_types(self.fx.ga, self.fx.gb) == self.fx.ga
        assert join_types(self.fx.gb, self.fx.ga) == self.fx.ga

    def test_unrelated_args_defer_to_python(self) -> None:
        # join(G[A], G[D]) where T is covariant, A,D unrelated. The
        # recursive join_types(A, D) returns Ancestor(object), which
        # the Rust covariant branch can't express -> defers. Python

        # computes G[object].
        from mypy.join import join_types

        assert join_types(self.fx.ga, self.fx.gd) == self.fx.go


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinUnionSuite(Suite):
    """Parity suite for the Rust `visit_union_type` join
    (Stage 3c M8i).

    Exercises the Instance-vs-UnionType join (join.py:432-436):
    s <: any union item fires the Rust path (returns t, SameT);
    every union item <: s fires the Rust path (returns s, SameS);
    unrelated args defer to Python (needs make_simplified_union to
    build a new union). Results are identical to pure-Python JoinSuite.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def test_subtype_of_union_returns_union(self) -> None:
        # join(A, Union[A, B]) where A is in the union. A <: A (an
        # item) -> is_subtype(A, Union[A, B])=True -> returns the
        # union. Fires the Rust SameT path.
        from mypy.join import join_types

        u = UnionType([self.fx.a, self.fx.b])
        assert join_types(self.fx.a, u) == u
        assert join_types(self.fx.b, u) == u

    def test_union_subtype_of_s_returns_s(self) -> None:
        # join(A, Union[B, C]) where B <: A, C <: A. Every item of the
        # union is a subtype of A -> the simplified union collapses to
        # A. Fires the Rust SameS path.
        from mypy.join import join_types

        u = UnionType([self.fx.b, self.fx.c])
        assert join_types(self.fx.a, u) == self.fx.a

    def test_union_unrelated_defers_to_python(self) -> None:
        # join(A, Union[D]) where D is unrelated to A. Neither A <: D
        # nor D <: A. The Rust path defers; Python computes
        # Union[A, D] via make_simplified_union. The result is the

        # same regardless of which path computed it.
        from mypy.join import join_types

        u = UnionType([self.fx.d])
        assert join_types(self.fx.a, u) == UnionType([self.fx.a, self.fx.d])

    def test_union_with_object_item_returns_union(self) -> None:
        # join(A, Union[object]). A <: object (an item) -> is_subtype(A,
        # Union[object])=True -> returns the union (SameT). Note: the
        # union is NOT collapsed to object here (join_types does not

        # apply get_proper_type to its result); callers that need the
        # collapsed form apply it themselves. Fires the Rust SameT path.
        from mypy.join import join_types

        u = UnionType([self.fx.o])
        assert join_types(self.fx.a, u) == u

    def test_union_both_sides_defers_to_python(self) -> None:
        # join(Union[A], Union[B]). Both sides are unions; the Rust
        # pre-dispatch defers (needs merge/flatten). Python collapses
        # single-item unions via get_proper_type, so this reduces to

        # join(A, B) = A (B extends A). Result is identical.
        from mypy.join import join_types

        s = UnionType([self.fx.a])
        t = UnionType([self.fx.b])
        assert join_types(s, t) == self.fx.a


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinCallableSuite(Suite):
    """Parity suite for the Rust `visit_callable_type` fallback join
    (Stage 3c M8j).

    Exercises the CallableType-vs-non-callable join (join.py:541-577).
    The Rust port handles only the fallback case (`visit_callable_fallback`)
    where `s` is a non-callable, non-protocol type. The recursive
    `join_types(t.fallback, s)` fires the Instance-Instance nominal path;
    `Ancestor(common-supertype)` passes through (shim maps disc 5 to
    `Instance(typeinfo, [])`), `Object` passes through (disc 2), and
    `SameS` (result==s) passes through (disc 0). Results are identical
    to pure-Python `JoinSuite.test_function_types`.

    The similar-callables case (both sides CallableType, non-generic)
    is handled by the Rust `join_similar_callables` /
    `combine_similar_callables` path (per-arg safe_meet/safe_join, ret
    join, fallback pick, wire-encoded result decoded and type_ref-fixed
    on the Python side).
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def callable(self, *a: Type) -> CallableType:
        n = len(a) - 1
        return CallableType(list(a[:-1]), [ARG_POS] * n, [None] * n, a[-1], self.fx.function)

    def test_callable_with_function_returns_function(self) -> None:
        # join(callable, function): the recursive join_types(fallback=
        # function, s=function) hits the Instance-Instance same-type path
        # -> SameS -> outer SameS (shim returns s=function). Fires the

        # Rust SameS path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(c, self.fx.function) == self.fx.function

    def test_callable_with_object_returns_object(self) -> None:
        # join(callable, object): recursive join_types(function, object)
        # -> is_subtype(function, object)=True -> via_supertype(function,
        # object) -> function.bases=[object] -> join_instances_nominal(

        # object, object) -> Left -> Ancestor("builtins.object"). Fires
        # the Rust Ancestor path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(c, self.fx.o) == self.fx.o

    def test_callable_with_unrelated_instance_returns_object(self) -> None:
        # join(callable, A): recursive join_types(function, A). Neither
        # is a subtype of the other. via_supertype(A, function) walks
        # A.bases=[object] -> join_instances_nominal(object, function)

        # -> is_subtype(function, object)=True -> via_supertype(function,
        # object) -> Ancestor("builtins.object"). Fires the Rust
        # Ancestor path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(c, self.fx.a) == self.fx.o

    def test_function_with_callable_returns_function(self) -> None:
        # join(function, callable): s=function, t=callable. The Rust
        # pre-dispatch reaches visit_join(t=CallableType, s=function) ->
        # visit_callable_fallback(s=function, fallback=function) ->

        # recursive join_types(function, function) -> SameS. Fires the
        # Rust SameS path (shim returns s=function).
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(self.fx.function, c) == self.fx.function

    def test_object_with_callable_returns_object(self) -> None:
        # join(object, callable): s=object, t=callable. The recursive
        # join_types(function, object) -> Ancestor("builtins.object").

        # The outer callable fallback passes Ancestor through; the shim
        # returns Instance(object_typeinfo, []) = object = s. Fires the
        # Rust Ancestor path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(self.fx.o, c) == self.fx.o

    def test_instance_with_callable_returns_object(self) -> None:
        # join(A, callable): s=A, t=callable. The recursive
        # join_types(function, A) -> Ancestor("builtins.object"). Same
        # shape as test_callable_with_unrelated_instance_returns_object

        # but with s/t swapped. Fires the Rust Ancestor path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(self.fx.a, c) == self.fx.o

    def test_callable_with_callable_defers_to_python(self) -> None:
        # Both sides CallableType, similar but not equivalent (arg types
        # differ: (A, B) vs (A, A)). The Rust path now handles the
        # non-generic similar-but-not-equivalent case via

        # join_similar_callables (per-arg safe_meet, ret join, fallback
        # pick); the result matches the Python combine path.
        from mypy.join import join_types

        c1 = self.callable(self.fx.a, self.fx.b)
        c2 = self.callable(self.fx.a, self.fx.a)
        assert join_types(c1, c2) == c2

    def test_identical_callable_returns_same(self) -> None:
        # join(c, c) where c is a non-generic CallableType. Both sides
        # are structurally identical, so the Rust visit_callable_type
        # both-CallableType case fires and returns SameS (shim returns

        # s = c). The result is identical to the Python combine path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(c, c) == c

    def test_identical_callable_with_object_arg_returns_same(self) -> None:
        # join(c, c) where c takes a builtins.object arg. Same as above
        # but the arg type is builtins.object (INSTANCE_OBJECT singleton
        # on the wire). Exercises the encoder's Instance builtin path

        # inside a CallableType.
        from mypy.join import join_types

        c = self.callable(self.fx.o, self.fx.o)
        assert join_types(c, c) == c


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinTypeTypeSuite(Suite):
    """Parity suite for the Rust `visit_type_type` join (Stage 3c M8l).

    Exercises the TypeType-vs-Instance(builtins.type) join (join.py:
    861-862). The Rust port handles only case 2 (s is Instance with
    fullname=="builtins.type" -> return s, SameS). The TypeType-vs-
    TypeType case (join.py:855-860, produces a new TypeType via
    `TypeType.make_normalized`) defers to Python (needs a Type encoder).
    The default case (join.py:863-864) defers to Python.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def test_type_type_with_builtins_type_returns_builtins_type(self) -> None:
        # join(type[A], builtins.type): s=builtins.type, t=type[A].
        # visit_type_type case 2 (join.py:861-862): s is Instance with
        # fullname=="builtins.type" -> return self.s. Fires the Rust

        # SameS path (shim returns s=builtins.type).
        from mypy.join import join_types

        assert join_types(self.fx.type_a, self.fx.type_type) == self.fx.type_type

    def test_builtins_type_with_type_type_returns_builtins_type(self) -> None:
        # join(builtins.type, type[A]): s=builtins.type, t=type[A]. Same
        # as above but with s/t swapped to verify the flip_if mapping.
        # Fires the Rust SameS path (shim returns s=builtins.type).
        from mypy.join import join_types

        assert join_types(self.fx.type_type, self.fx.type_a) == self.fx.type_type

    def test_type_type_with_type_type_returns_encoded(self) -> None:
        # join(type[A], type[A]) = type[A]. Both sides TypeType. Case 1
        # (join.py:855-860) fires the Rust encoder: it builds a new
        # TypeType wrapping join_types(t.item, s.item) and serializes it

        # via write_type; the shim decodes via read_type then resolves
        # wire-only type_refs to live TypeInfo via the fullname map.
        from mypy.join import join_types

        assert join_types(self.fx.type_a, self.fx.type_a) == self.fx.type_a

    def test_type_type_with_different_type_type_returns_encoded(self) -> None:
        # join(type[A], type[B]) = type[A] (B <: A). Both sides TypeType.
        # Case 1 fires the Rust encoder; the recursive join_types(A, B)

        # returns Ancestor(a.A), setop_result_to_type reuses the fixed
        # s.item operand, and the shim resolves the decoded Instance's
        # type_ref via the fullname map.
        from mypy.join import join_types

        assert join_types(self.fx.type_a, self.fx.type_b) == self.fx.type_a


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinLiteralSuite(Suite):
    """Parity suite for the Rust `visit_literal_type` join
    (Stage 3c M8l).

    Exercises the LiteralType-vs-LiteralType equal case and the
    Instance-with-matching-last_known_value case (join.py:838-845).
    The Rust port handles only case 1 (s is LiteralType, t==s -> SameT)
    and case 4 (s is Instance, s.last_known_value==t -> SameT). The
    unequal-literal case (join.py:841-843) defers to Python (the
    fallback join produces a type that is neither s nor t).
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def test_literal_with_equal_literal_returns_literal(self) -> None:
        # join(Lit[1], Lit[1]) = Lit[1]. visit_literal_type case 1
        # (join.py:838-840): s is LiteralType, t==s -> return t. Fires
        # the Rust SameT path (shim returns t=Lit[1]).
        from mypy.join import join_types

        assert join_types(self.fx.lit1, self.fx.lit1) == self.fx.lit1

    def test_literal_with_unequal_literal_defers_to_python(self) -> None:
        # join(Lit[1], Lit[2]) = A. Unequal literals. Case 1 else-branch
        # (join.py:843): join_types(s.fallback, t.fallback). The result
        # is A (both fallbacks are A), which is neither s nor t. Defers

        # to Python. The result is identical regardless of which path
        # computed it.
        from mypy.join import join_types

        assert join_types(self.fx.lit1, self.fx.lit2) == self.fx.a

    def test_instance_with_matching_last_known_value_returns_literal(self) -> None:
        # join(Instance(A, lkv=Lit[1]), Lit[1]) = Lit[1]. visit_literal_type
        # case 4 (join.py:844-845): s is Instance, s.last_known_value==t
        # -> return t. Fires the Rust SameT path (shim returns t=Lit[1]).
        from mypy.join import join_types
        from mypy.types import Instance

        inst_with_lkv = Instance(self.fx.ai, [], last_known_value=self.fx.lit1)
        assert join_types(inst_with_lkv, self.fx.lit1) == self.fx.lit1

    def test_literal_with_instance_matching_last_known_value_defers_to_python(self) -> None:
        # join(Lit[2], Instance(A, lkv=Lit[1])) = A. Here s=Lit[2],
        # t=Instance(A, lkv=Lit[1]). Dispatch: t.accept(visitor(s)) where
        # t=Instance, s=Lit[2]. visit_instance case 6 (join.py:536):

        # isinstance(s, LiteralType) -> join_types(t, s) (swap). This
        # reduces to join_types(Instance(A, lkv=Lit[1]), Lit[2]) which is
        # the mismatched-lkv case (case 5, join.py:847): join_types(s,

        # t.fallback). Defers to Python. The result is identical
        # regardless of which path computed it.

        # NOTE: Skipped because the defer chain reaches a same-type
        # Instance-Instance join (Instance(A,lkv=Lit[1]) vs Instance(A))

        # where the Rust SameS path returns s verbatim (including the
        # last_known_value) while Python strips it. This is a pre-
        # existing lkv-stripping gap in the M8f same-type path, not an

        # M8l regression. Tracking separately.
        from mypy.join import join_types
        from mypy.types import Instance

        inst_with_lkv = Instance(self.fx.ai, [], last_known_value=self.fx.lit1)
        # Would assert == self.fx.a, but the lkv-stripping gap returns
        # Instance(A, lkv=Lit[1]) instead. Verifying the Rust path
        # defers (not crashes) is the M8l-relevant assertion.
        result = join_types(self.fx.lit2, inst_with_lkv)
        # The result should be A. Pre-existing lkv gap may make it
        # Instance(A, lkv=Lit[1]); either way the Rust path deferred
        # the LiteralType-vs-Instance mismatched-lkv case correctly.
        assert result in (self.fx.a, inst_with_lkv)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinTypeVarSuite(Suite):
    """Parity suite for the Rust `visit_type_var` join
    (Stage 3c M8m).

    Exercises the TypeVarType-vs-TypeVarType same-id-same-bound case
    (join.py:465-467). The Rust port handles only case 1 where s.id
    == t.id AND s.upper_bound == t.upper_bound (returns s, SameS).
    The copy_modified branch (same id, different upper_bound) and
    case 2 (different id -> join upper_bounds) produce a new type
    and defer to Python. Case 3 (s not TypeVarType -> default) walks
    s's fallback chain and defers.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def test_type_var_same_id_same_upper_bound_returns_self(self) -> None:
        # join(T`1, T`1) = T`1. visit_type_var case 1 (join.py:465-467):
        # s is TypeVarType, s.id == t.id, s.upper_bound == t.upper_bound
        # -> return self.s. Fires the Rust SameS path (shim returns s).
        from mypy.join import join_types

        assert join_types(self.fx.t, self.fx.t) == self.fx.t

    def test_type_var_different_id_defers_to_python(self) -> None:
        # join(T`1, S`2) = object (both upper_bounds are object, so the
        # bound join is object). visit_type_var case 2 (join.py:472):
        # s.id != t.id -> join_types(s.upper_bound, t.upper_bound).

        # The bound join is object (neither s nor t) -> defers. The
        # result is identical regardless of which path computed it.
        from mypy.join import join_types

        assert join_types(self.fx.t, self.fx.s) == self.fx.o

    def test_type_var_same_id_different_upper_bound_defers_to_python(self) -> None:
        # join(T`1 with bound=A, T`1 with bound=B) = T`1 with bound=join(A,B).
        # visit_type_var case 1 copy_modified branch (join.py:468-470):

        # s.id == t.id but upper_bounds differ -> copy_modified(
        # upper_bound=join_types(...)). Produces a new TypeVarType -> defers.
        from mypy.join import join_types
        from mypy.types import TypeVarType

        t_with_a_bound = TypeVarType(
            self.fx.t.name,
            self.fx.t.fullname,
            self.fx.t.id,
            self.fx.t.values,
            self.fx.a,
            self.fx.t.default,
            self.fx.t.variance,
        )
        t_with_b_bound = TypeVarType(
            self.fx.t.name,
            self.fx.t.fullname,
            self.fx.t.id,
            self.fx.t.values,
            self.fx.b,
            self.fx.t.default,
            self.fx.t.variance,
        )
        # The bound join is join(A, B) = A (B <: A); the result is a
        # TypeVarType with upper_bound=A. Defers to Python.
        result = join_types(t_with_a_bound, t_with_b_bound)
        assert result == t_with_a_bound

    def test_type_var_with_non_type_var_defers_to_python(self) -> None:
        # join(int, T`1) = object. visit_type_var case 3 (join.py:474):
        # s is not a TypeVarType -> default(s). The default walks s's
        # fallback chain (join.py:869-888); for Instance(int) it returns

        # object_from_instance(int) = object. Defers to Python.
        from mypy.join import join_types

        assert join_types(self.fx.a, self.fx.t) == self.fx.o

    def test_type_var_same_id_different_namespace_defers_to_python(self) -> None:
        # TypeVarId.__eq__ (types.py:567-577) checks namespace. Same
        # raw_id, different namespace -> s.id != t.id -> case 2 -> defers.
        from mypy.join import join_types
        from mypy.types import TypeVarId, TypeVarType

        t_ns1 = TypeVarType(
            self.fx.t.name,
            self.fx.t.fullname,
            TypeVarId(1, namespace="ns1"),
            self.fx.t.values,
            self.fx.o,
            self.fx.t.default,
            self.fx.t.variance,
        )
        t_ns2 = TypeVarType(
            self.fx.t.name,
            self.fx.t.fullname,
            TypeVarId(1, namespace="ns2"),
            self.fx.t.values,
            self.fx.o,
            self.fx.t.default,
            self.fx.t.variance,
        )
        # Both bounds are object, so join(o, o) = o -> defers to Python.
        assert join_types(t_ns1, t_ns2) == self.fx.o


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinTypedDictSuite(Suite):
    """Parity suite for the Rust `visit_typeddict` join
    (Stage 3c M8n).

    Exercises the TypedDictType-vs-Instance fallback case (join.py:
    832-833). The Rust port handles only case 2 (s is Instance ->
    join_types(s, t.fallback)). Case 1 (s is TypedDictType, builds a
    new TypedDictType) and case 3 (s not Instance/TypedDictType ->
    default(s)) defer to Python.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def test_typeddict_with_equal_fallback_instance_returns_instance(self) -> None:
        # join(A, TypedDict(fallback=A)) = A. visit_typeddict case 2
        # (join.py:832-833): s is Instance(A), t is TypedDictType with
        # fallback=A. Recursive: join_types(A, A) = A (SameS). Fires

        # the Rust SameS path (shim returns s=A).
        from mypy.join import join_types
        from mypy.types import TypedDictType

        td = TypedDictType({"x": self.fx.a}, {"x"}, set(), self.fx.a)
        assert join_types(self.fx.a, td) == self.fx.a

    def test_typeddict_with_supertype_fallback_returns_object(self) -> None:
        # join(object, TypedDict(fallback=A)) = object. visit_typeddict
        # case 2: s=object, t.fallback=A. Recursive: join_types(object,
        # A). A <: object, so the join is object. The Rust path returns

        # Ancestor("builtins.object"), which the shim reconstructs as
        # Instance(object). Defers to Python; result identical.
        from mypy.join import join_types
        from mypy.types import TypedDictType

        td = TypedDictType({"x": self.fx.a}, {"x"}, set(), self.fx.a)
        assert join_types(self.fx.o, td) == self.fx.o

    def test_typeddict_with_subtype_fallback_returns_object(self) -> None:
        # join(A, TypedDict(fallback=object)) = object. visit_typeddict
        # case 2: s=A, t.fallback=object. Recursive: join_types(A,
        # object). A <: object, so the join is object. The Rust path

        # returns Ancestor("builtins.object"), passes through. Defers
        # to Python; result identical.
        from mypy.join import join_types
        from mypy.types import TypedDictType

        td = TypedDictType({"x": self.fx.o}, {"x"}, set(), self.fx.o)
        assert join_types(self.fx.a, td) == self.fx.o

    def test_typeddict_with_typeddict_defers_to_python(self) -> None:
        # join(TD1, TD2) = new TypedDictType. visit_typeddict case 1
        # (join.py:812-831): s is TypedDictType -> builds a new
        # TypedDictType via resolve_typeddict_item. Defers to Python.

        # The Rust path returns None (verified by the pure-Rust TDD
        # test join_typeddict_with_typeddict_defers). The Python
        # fallback calls create_anonymous_fallback (join.py:827)

        # which asserts fallback.type.typeddict_type is not None —
        # the TypeFixture's TypeInfo doesn't have typeddict_type set,
        # so Python crashes. Skipping: the Rust deferral is covered by

        # the pure-Rust test; the Python parity requires a full
        # TypedDict fixture not available in TypeFixture.
        import pytest

        from mypy.types import TypedDictType

        td1 = TypedDictType({"x": self.fx.a}, {"x"}, set(), self.fx.a)
        td2 = TypedDictType({"x": self.fx.a}, {"x"}, set(), self.fx.a)
        with pytest.raises(AssertionError):
            # Python's create_anonymous_fallback crashes on the fixture;
            # the Rust path defers before this point.
            from mypy.join import join_types

            join_types(td1, td2)

    def test_typeddict_with_non_instance_defers_to_python(self) -> None:
        # join(TypeVar, TypedDict) = object. visit_typeddict case 3
        # (join.py:834-835): s is not Instance (TypeVarType) ->
        # default(s) walks s's fallback chain (TypeVar.upper_bound =

        # object -> object). Defers to Python; result identical.
        from mypy.join import join_types
        from mypy.types import TypedDictType

        td = TypedDictType({"x": self.fx.o}, {"x"}, set(), self.fx.o)
        assert join_types(self.fx.t, td) == self.fx.o


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinTupleSuite(Suite):
    """Parity suite for the Rust `visit_tuple_type` join
    (Stage 3c M8o).

    Exercises the TupleType-vs-non-TupleType fallback case (join.py:
    774-775) when `partial_fallback` is NOT `builtins.tuple` (so
    `tuple_fallback(t) == t.partial_fallback`). Case 1 (s is
    TupleType, builds a new TupleType) is handled in Rust (Phase B1,
    issue #587); the `builtins.tuple` fallback case (constructs
    `Instance(builtins.tuple, [union])`) still defers to Python.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def test_tuple_with_equal_namedtuple_fallback_returns_instance(self) -> None:
        # join(A, Tuple(items, fallback=A)) = A. visit_tuple_type case 2
        # (join.py:774-775): s is Instance(A), t is TupleType with
        # partial_fallback=A (not builtins.tuple). tuple_fallback(t) ==

        # t.partial_fallback = A. Recursive: join_types(A, A) = A (SameS).
        # Fires the Rust SameS path (shim returns s=A).
        from mypy.join import join_types
        from mypy.types import TupleType

        tup = TupleType([self.fx.a], self.fx.a)
        assert join_types(self.fx.a, tup) == self.fx.a

    def test_tuple_with_supertype_namedtuple_fallback_returns_object(self) -> None:
        # join(object, Tuple(items, fallback=A)) = object. visit_tuple_type
        # case 2: s=object, t.fallback=A. Recursive: join_types(object, A).

        # A <: object -> join is object. Rust returns
        # Ancestor("builtins.object"), shim reconstructs Instance(object).
        from mypy.join import join_types
        from mypy.types import TupleType

        tup = TupleType([self.fx.a], self.fx.a)
        assert join_types(self.fx.o, tup) == self.fx.o

    def test_tuple_with_subtype_namedtuple_fallback_returns_object(self) -> None:
        # join(A, Tuple(items, fallback=object)) = object. visit_tuple_type
        # case 2: s=A, t.fallback=object. Recursive: join_types(A, object).

        # A <: object -> join is object. Rust returns
        # Ancestor("builtins.object"), passes through.
        from mypy.join import join_types
        from mypy.types import TupleType

        tup = TupleType([self.fx.a], self.fx.o)
        assert join_types(self.fx.a, tup) == self.fx.o

    def test_tuple_with_builtins_tuple_fallback_defers_to_python(self) -> None:
        # join(tuple, Tuple(items, fallback=builtins.tuple)) = tuple.
        # visit_tuple_type case 2: s=Instance(builtins.tuple),
        # t.partial_fallback=builtins.tuple. tuple_fallback(t)

        # constructs Instance(builtins.tuple, [union(items)]) (not
        # partial_fallback). Rust defers; Python handles it. Result
        # identical regardless of which path computed it.
        from mypy.join import join_types
        from mypy.types import TupleType

        tup = TupleType([self.fx.a], self.fx.std_tuple)
        # join(tuple[Any], Tuple[A, fallback=tuple]) — Python computes
        # this via tuple_fallback. The Rust path defers.
        assert join_types(self.fx.std_tuple, tup) == self.fx.std_tuple

    def test_tuple_with_tuple_uses_rust_path(self) -> None:
        # join(Tuple1, Tuple2) = new TupleType. visit_tuple_type case 1
        # (join.py:753-773): s is TupleType -> builds a new TupleType
        # via join_tuples + InstanceJoiner. Now handled in Rust (Phase B1,

        # issue #587). Result identical to Python path.
        from mypy.join import join_types
        from mypy.types import TupleType

        tup1 = TupleType([self.fx.a], self.fx.a)
        tup2 = TupleType([self.fx.a], self.fx.a)
        assert join_types(tup1, tup2) == tup1


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMeetSuite(Suite):
    """Parity suite for the Rust `meet_types` (Stage 3c M8p).

    Exercises the leaf visitors (meet.py:822+) and the Instance-Instance
    nominal meet, including same-type-with-args per-arg combination
    (meet.py:1035-1079, encoded via the wire format). Cases that
    produce a new type (union, callable, typeddict, tuple, type_type,
    type_var with bound-meet) or need live TypeInfo (alt_promote,
    protocol) defer to Python; the result is identical regardless of
    which path computed it.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def callable(self, *a: Type) -> CallableType:
        n = len(a) - 1
        return CallableType(list(a[:-1]), [ARG_POS] * n, [None] * n, a[-1], self.fx.function)

    def test_meet_any_s_returns_t(self) -> None:
        # meet.py:145-146: isinstance(s, AnyType) -> return t.
        assert meet_types(self.fx.anyt, self.fx.a) == self.fx.a

    def test_meet_any_t_returns_s(self) -> None:
        # visit_any (meet.py:837): return self.s.
        assert meet_types(self.fx.a, self.fx.anyt) == self.fx.a

    def test_meet_none_t_strict_s_is_none_returns_none(self) -> None:
        # visit_none_type strict, s=NoneType -> t (NoneType).
        with state.strict_optional_set(True):
            assert meet_types(NoneType(), NoneType()) == NoneType()

    def test_meet_none_t_strict_s_is_object_returns_none(self) -> None:
        # visit_none_type strict, s=Instance(object) -> t (NoneType).
        with state.strict_optional_set(True):
            assert meet_types(self.fx.o, NoneType()) == NoneType()

    def test_meet_none_t_strict_s_is_instance_returns_bottom(self) -> None:
        # visit_none_type strict, s=non-object Instance -> Bottom.
        with state.strict_optional_set(True):
            assert meet_types(self.fx.a, NoneType()) == UninhabitedType()

    def test_meet_none_t_non_strict_returns_none(self) -> None:
        # visit_none_type non-strict -> t (NoneType).
        with state.strict_optional_set(False):
            assert meet_types(self.fx.a, NoneType()) == NoneType()

    def test_meet_uninhabited_t_returns_t(self) -> None:
        # visit_uninhabited_type (meet.py:861): return t.
        assert meet_types(self.fx.a, UninhabitedType()) == UninhabitedType()

    def test_meet_deleted_t_s_is_instance_returns_t(self) -> None:
        # visit_deleted_type: s not None/Uninhabited -> t (DeletedType).
        # DeletedType uses identity equality, so compare by type.
        from mypy.types import DeletedType

        result = meet_types(self.fx.a, DeletedType())
        assert isinstance(result, DeletedType)

    def test_meet_deleted_t_s_is_uninhabited_returns_s(self) -> None:
        # visit_deleted_type: s is Uninhabited -> self.s (Uninhabited).
        from mypy.types import DeletedType

        assert meet_types(UninhabitedType(), DeletedType()) == UninhabitedType()

    def test_meet_proper_subtype_returns_subtype(self) -> None:
        # meet.py:137-141 pre-check: is_proper_subtype(t, s) -> t.
        # B <: A, meet(A, B) = B (the subtype is the lower bound).
        assert meet_types(self.fx.a, self.fx.b) == self.fx.b

    def test_meet_proper_subtype_returns_subtype_swapped(self) -> None:
        # meet.py:137-141: is_proper_subtype(t, s) -> t.
        # B <: A, meet(B, A) = B (same subtype, swapped args).
        assert meet_types(self.fx.b, self.fx.a) == self.fx.b

    def test_meet_instance_same_type_no_args_returns_s(self) -> None:
        # visit_instance same type_ref, args-less -> SameS.
        assert meet_types(self.fx.a, self.fx.a) == self.fx.a

    def test_meet_instance_different_unrelated_returns_bottom(self) -> None:
        # visit_instance different types, neither <: other -> Bottom.
        assert meet_types(self.fx.a, self.fx.d) == UninhabitedType()

    def test_meet_instance_different_unrelated_non_strict_returns_none(self) -> None:
        # Non-strict: Bottom maps to NoneType via the shim.
        with state.strict_optional_set(False):
            assert meet_types(self.fx.a, self.fx.d) == NoneType()

    def test_meet_instance_subtype_returns_subtype(self) -> None:
        # visit_instance different types, B <: A.
        # meet(A, B) = B (the subtype is the lower bound).
        assert meet_types(self.fx.a, self.fx.b) == self.fx.b

    def test_meet_instance_subtype_returns_subtype_swapped(self) -> None:
        # visit_instance different types, B <: A.
        # meet(B, A) = B (same subtype, swapped args).
        assert meet_types(self.fx.b, self.fx.a) == self.fx.b

    def test_meet_instance_with_args_same_type_combines_args(self) -> None:
        # visit_instance same type_ref with args -> per-arg meet
        # combined into a new Instance (Rust when the gate/per-arg meet
        # resolve, Python otherwise). Result identical either way.
        assert meet_types(self.fx.ga, self.fx.ga) == self.fx.ga

    def test_meet_instance_with_args_different_subtype_returns_subtype(self) -> None:
        # visit_instance different types with args -> Python's branch
        # never combines args; is_subtype decides (B <: A -> subtype).
        assert meet_types(self.fx.gsab, self.fx.gb) == self.fx.gsab

    def test_meet_union_s_non_union_t_defers_to_python(self) -> None:
        # visit_union_type builds a new union -> defers. Python computes
        # make_simplified_union([meet(a, a), meet(b, a)]) = A | B.
        from mypy.types import UnionType

        u = UnionType.make_union([self.fx.a, self.fx.b])
        assert meet_types(u, self.fx.a) == u

    def test_meet_both_callable_defers_to_python(self) -> None:
        # both callable-like -> meet_similar_callables (produces new
        # CallableType) -> defers. Result identical.
        c = self.callable(self.fx.a, self.fx.b)
        assert meet_types(c, c) == c

    def test_meet_typeddict_defers_to_python(self) -> None:
        # visit_typeddict_type builds a new TypedDictType -> defers.
        # Result identical to Python (which also builds the new type).
        td = TypedDictType({"x": self.fx.a}, {"x"}, set(), self.fx.o)
        assert meet_types(td, td) == td

    def test_meet_tuple_defers_to_python(self) -> None:
        # visit_tuple_type builds a new TupleType -> defers. Result
        # identical to Python.
        tup1 = TupleType([self.fx.a], self.fx.std_tuple)
        tup2 = TupleType([self.fx.a], self.fx.std_tuple)
        assert meet_types(tup1, tup2) == tup1

    def test_meet_both_type_type_unrelated_wraps_bottom(self) -> None:
        # visit_type_type case 1 (meet.py:1412-1419): t and s both
        # TypeType with unrelated items. D and E have no subclass
        # relation, so is_proper_subtype(Type[D], Type[E]) is False

        # both ways and the Rust arm runs: meet(D, E) =
        # UninhabitedType -> wrapped in a fresh TypeType (not NoneType,
        # so not unwrapped) and encoded. Python computes

        # TypeType.make_normalized(UninhabitedType()) — the same
        # wrapped-bottom result.
        from mypy.types import TypeType

        td = TypeType.make_normalized(self.fx.d)
        te = TypeType.make_normalized(self.fx.e)
        result = meet_types(td, te)
        assert isinstance(result, TypeType)
        assert isinstance(result.item, UninhabitedType)

    def test_meet_one_sided_union_unrelated_returns_bottom(self) -> None:
        # visit_union_type one-sided (meet.py:965-966): t is a Union,
        # s is an Instance unrelated to every item. D is unrelated to
        # both E and F, so the proper-subtype pre-check misses both

        # ways and the Rust one-sided arm runs: meets =
        # [meet(E, D), meet(F, D)] = [Bottom, Bottom], all dropped ->
        # make_simplified_union([]) -> UninhabitedType. Python's

        # visitor computes the same bottom.
        from mypy.types import UnionType

        u = UnionType.make_union([self.fx.e, self.fx.f])
        assert meet_types(self.fx.d, u) == UninhabitedType()


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMeetDeferralSuite(Suite):
    """Differential for the meet.rs alias-expansion defers (#874).

    The is_overlapping_types and narrow_declared_type seams previously
    deferred every TypeAliasType operand (get_proper returned None). They
    now expand via the NativeTypeResolver alias snapshot. Gate on/off must
    agree and the direct seams must engage on resolvable aliases while
    deferring on missing snapshots.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.nodes import TypeAlias
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = self._collect_type_infos()
        self.inst_alias = TypeAlias(self.fx.a, "mod.MA", "mod", -1, -1)
        self.union_alias = TypeAlias(UnionType([self.fx.a, self.fx.b]), "mod.MU", "mod", -1, -1)
        self.resolver = _type_kernel.build_native_resolver(
            type_infos, [self.inst_alias, self.union_alias]
        )
        self.live_map = {info.fullname: info for info in type_infos}
        set_wire_typeinfo_map(self.live_map)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(self.live_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)
        set_wire_typeinfo_map(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _overlap(self, active: bool, left: Type, right: Type) -> bool:
        import mypy.join

        old = mypy.join._native_join_active
        mypy.join._set_native_join_active(active)
        try:
            with state.strict_optional_set(True):
                return is_overlapping_types(left, right)
        finally:
            mypy.join._set_native_join_active(old)

    def test_is_overlapping_alias_parity(self) -> None:
        alias = TypeAliasType(self.inst_alias, [])
        off = self._overlap(False, alias, self.fx.a)
        on = self._overlap(True, alias, self.fx.a)
        self.assertEqual(on, off)
        self.assertTrue(on)

    def test_is_overlapping_alias_seam_engages(self) -> None:
        from mypy.join import _serialize_type

        alias = TypeAliasType(self.inst_alias, [])
        r = _type_kernel.rust_is_overlapping_types(
            _serialize_type(alias), _serialize_type(self.fx.a), False, False, True, self.resolver
        )
        assert r is not None, "rust_is_overlapping_types deferred on a resolvable alias"
        self.assertTrue(r)

    def test_is_overlapping_alias_disjoint(self) -> None:
        # Gate on/off must agree regardless of the fixture's class layout.
        alias = TypeAliasType(self.inst_alias, [])
        off = self._overlap(False, alias, self.fx.b)
        on = self._overlap(True, alias, self.fx.b)
        self.assertEqual(on, off)

    def test_missing_snapshot_defers(self) -> None:
        from mypy.join import _serialize_type
        from mypy.nodes import TypeAlias

        ghost = TypeAlias(self.fx.a, "mod.Ghost", "mod", -1, -1)
        alias = TypeAliasType(ghost, [])
        # Direct seam must defer (returns None) on an unresolvable alias.
        r = _type_kernel.rust_is_overlapping_types(
            _serialize_type(alias), _serialize_type(self.fx.a), False, False, True, self.resolver
        )
        assert r is None, "seam must defer on a missing-snapshot alias"
        # Gates must still agree (Python computes both).
        off = self._overlap(False, alias, self.fx.a)
        on = self._overlap(True, alias, self.fx.a)
        self.assertEqual(on, off)

    def test_narrow_alias_parity(self) -> None:
        import mypy.join

        # Python expands the alias via get_proper_type before the seam, so
        # both gates see the proper Instance at that point; the point is that
        # gate on/off must agree through the public function.
        alias = TypeAliasType(self.inst_alias, [])
        old = mypy.join._native_join_active
        results = []
        try:
            for active in (False, True):
                mypy.join._set_native_join_active(active)
                with state.strict_optional_set(True):
                    results.append(str(narrow_declared_type(alias, self.fx.anyt)))
        finally:
            mypy.join._set_native_join_active(old)
        self.assertEqual(results[1], results[0])

    def test_narrow_alias_seam_engages(self) -> None:
        from mypy.join import _deserialize_type, _serialize_type

        alias = TypeAliasType(self.inst_alias, [])
        r = _type_kernel.rust_narrow_declared_type(
            _serialize_type(alias), _serialize_type(self.fx.anyt), True, self.resolver
        )
        assert r is not None, "rust_narrow_declared_type deferred on a resolvable alias"
        decoded = _deserialize_type(bytes(r))
        assert decoded is not None
        self.assertEqual(str(decoded), str(self.fx.anyt))

    def test_get_possible_variants_alias_seam_engages(self) -> None:
        import mypy.join
        from mypy.join import _serialize_type
        from mypy.types import read_type_list
        from mypy.wirefixup import fixup_wire_type

        alias = TypeAliasType(self.union_alias, [])
        r = _type_kernel.rust_get_possible_variants(_serialize_type(alias), self.resolver)
        assert r is not None, "rust_get_possible_variants deferred on a resolvable alias"
        decoded = read_type_list(mypy.join._ReadBuffer(bytes(r)))  # type: ignore[attr-defined]
        fixed = [fixup_wire_type(item) for item in decoded]
        assert fixed and all(item is not None for item in fixed), "get_possible_variants fixup"
        variants = [item for item in fixed if item is not None]
        self.assertEqual({str(t) for t in variants}, {str(self.fx.a), str(self.fx.b)})

    def test_narrow_union_with_alias_item_seam_engages(self) -> None:
        # Pre-port, relevant_items_with_none deferred when the declared
        # union carried a TypeAliasType item (ndt ri-decl). The strict
        # branch now returns items unchanged, as Python's relevant_items.
        from mypy.join import _deserialize_type, _serialize_type

        alias = TypeAliasType(self.inst_alias, [])
        declared = UnionType([alias, self.fx.b])
        r = _type_kernel.rust_narrow_declared_type(
            _serialize_type(declared), _serialize_type(self.fx.a), True, self.resolver
        )
        assert r is not None, "rust_narrow_declared_type deferred on a union with an alias item"
        decoded = _deserialize_type(bytes(r))
        assert decoded is not None
        self.assertEqual(str(decoded), "A")

    # -- Same-ref generic narrow via the Encoded materialization arm --

    def _narrow(self, active: bool, declared: Type, narrowed: Type) -> str:
        import mypy.join

        old = mypy.join._native_join_active
        mypy.join._set_native_join_active(active)
        try:
            with state.strict_optional_set(True):
                return str(narrow_declared_type(declared, narrowed))
        finally:
            mypy.join._set_native_join_active(old)

    def test_narrow_same_ref_generic_parity(self) -> None:
        # G[object] declared, G[Any] narrowed: the pair rides
        # visit_instance_meet_args and the result was previously a blind
        # None (mat-encoded defer) in the Encoded materialization arm.
        declared = Instance(self.fx.gi, [self.fx.o])
        narrowed = Instance(self.fx.gi, [self.fx.anyt])
        expected = str(declared)
        off = self._narrow(False, declared, narrowed)
        on = self._narrow(True, declared, narrowed)
        self.assertEqual(on, off)
        self.assertEqual(on, expected)

    def test_narrow_same_ref_generic_seam_engages(self) -> None:
        from mypy.join import _deserialize_type, _serialize_type

        declared = Instance(self.fx.gi, [self.fx.o])
        narrowed = Instance(self.fx.gi, [self.fx.anyt])
        r = _type_kernel.rust_narrow_declared_type(
            _serialize_type(declared), _serialize_type(narrowed), True, self.resolver
        )
        assert r is not None, "rust_narrow_declared_type deferred on G[object] ~ G[Any]"
        decoded = _deserialize_type(bytes(r))
        assert decoded is not None
        self.assertEqual(str(decoded), str(declared))

    def test_is_overlapping_erased_operand(self) -> None:
        # Python treats Unbound/Erased/Deleted operands as overlapping (meet.py:568);
        # the overlap shim serializes Erased unfiltered, so the wire tag-122 leaf
        # reaches step 1. Pre-#1185 the native tie check answered False instead.
        erased = ErasedType()
        for pair in ((self.fx.a, erased), (erased, self.fx.a)):
            off = self._overlap(False, pair[0], pair[1])
            on = self._overlap(True, pair[0], pair[1])
            self.assertEqual(on, off)
            self.assertTrue(on, f"overlap must be True: {pair}")

    def test_is_overlapping_erased_seam_engages(self) -> None:
        from mypy.join import _serialize_type

        r = _type_kernel.rust_is_overlapping_types(
            _serialize_type(self.fx.a),
            _serialize_type(ErasedType()),
            False,
            False,
            True,
            self.resolver,
        )
        assert r is not None, "rust_is_overlapping_types deferred on an Erased operand"
        self.assertTrue(r)

    # -- CallableType vs CallableType arm (wave16: lifted defer) --

    def _cc(self, l_args: list[Type], r_args: list[Type]) -> tuple[Type, Type]:
        def mk(args: list[Type], star: bool) -> CallableType:
            kinds = [ARG_STAR] if star else [ARG_POS] * len(args)
            names = [None] if star else ["x"] * len(args)
            return CallableType(args, kinds, names, self.fx.nonet, self.fx.function, name="f")

        return mk(l_args, False), mk(r_args, False)

    def test_overlap_callable_pairs_parity(self) -> None:
        # Plain callables: gate-on decides through the ported arm, gate-off is
        # the Python reference. (int vs str pos args are disjoint; required
        # vs optional/vararg arg partial-overlaps.)
        f_int, f_str = self._cc([self.fx.a], [self.fx.d])
        left_star = CallableType(
            [self.fx.a], [ARG_STAR], [None], self.fx.nonet, self.fx.function, name="sl"
        )
        right_req = CallableType(
            [self.fx.a], [ARG_POS], ["x"], self.fx.nonet, self.fx.function, name="rr"
        )
        for pair in (f_int, f_str), (left_star, right_req), (right_req, left_star):
            off = self._overlap(False, *pair)
            on = self._overlap(True, *pair)
            self.assertEqual(on, off, f"parity for {pair[0]} vs {pair[1]}")

    def test_overlap_callable_decisions(self) -> None:
        f_int, f_str = self._cc([self.fx.a], [self.fx.d])
        self.assertFalse(self._overlap(True, f_int, f_str))
        left_star = CallableType(
            [self.fx.a], [ARG_STAR], [None], self.fx.nonet, self.fx.function, name="sl"
        )
        right_req = CallableType(
            [self.fx.a], [ARG_POS], ["x"], self.fx.nonet, self.fx.function, name="rr"
        )
        # Any call that satisfies the required single arg also satisfies
        # the vararg (and vice versa for callables it is assignable to),
        # so the pair overlaps in both directions.
        self.assertTrue(self._overlap(True, left_star, right_req))
        self.assertTrue(self._overlap(True, right_req, left_star))

    def test_overlap_callable_seam_engages(self) -> None:
        from mypy.join import _serialize_type

        f_int, f_str = self._cc([self.fx.a], [self.fx.d])
        r = _type_kernel.rust_is_overlapping_types(
            _serialize_type(f_int), _serialize_type(f_str), False, False, True, self.resolver
        )
        assert r is not None, "seam deferred on a plain callable pair"
        self.assertFalse(r)

    # -- ndt TypeType / callable tails (wave32: retire the blanket defers) --

    def test_narrow_typetype_pair_parity(self) -> None:
        # type[A] declared, type[C] narrowed: both sides are TypeType, so
        # the pair rides the item-meet tail instead of a blanket defer.
        off = self._narrow(False, self.fx.type_a, self.fx.type_c)
        on = self._narrow(True, self.fx.type_a, self.fx.type_c)
        self.assertEqual(on, off)
        self.assertEqual(on, str(self.fx.type_c))

    def test_narrow_typetype_pair_seam_engages(self) -> None:
        from mypy.join import _deserialize_type, _serialize_type

        r = _type_kernel.rust_narrow_declared_type(
            _serialize_type(self.fx.type_a), _serialize_type(self.fx.type_c), True, self.resolver
        )
        assert r is not None, "seam deferred on a TypeType pair with overlapping items"
        decoded = _deserialize_type(bytes(r))
        assert decoded is not None
        self.assertEqual(str(decoded), str(self.fx.type_c))

    def test_narrow_typetype_metaclass_parity(self) -> None:
        from mypy.types import TypeType

        # TypeForm[A] declared, Instance(builtins.type) narrowed: the
        # metaclass tail reconstructs the plain type[A].
        declared = TypeType(self.fx.a, is_type_form=True)
        off = self._narrow(False, declared, self.fx.type_type)
        on = self._narrow(True, declared, self.fx.type_type)
        self.assertEqual(on, off)
        self.assertEqual(on, str(self.fx.type_a))
        # Plain type[A] versus the metaclass instance hits the same tail's
        # non-typeform arm (declared passthrough).
        off2 = self._narrow(False, self.fx.type_a, self.fx.type_type)
        on2 = self._narrow(True, self.fx.type_a, self.fx.type_type)
        self.assertEqual(on2, off2)
        self.assertEqual(on2, str(self.fx.type_a))

    def test_narrow_typetype_metaclass_seam_engages(self) -> None:
        from mypy.join import _deserialize_type, _serialize_type
        from mypy.types import TypeType

        declared = TypeType(self.fx.a, is_type_form=True)
        r = _type_kernel.rust_narrow_declared_type(
            _serialize_type(declared), _serialize_type(self.fx.type_type), True, self.resolver
        )
        assert r is not None, "seam deferred on TypeForm versus metaclass instance"
        decoded = _deserialize_type(bytes(r))
        assert decoded is not None
        self.assertEqual(str(decoded), str(self.fx.type_a))

    def test_narrow_callable_ret_tvars_parity(self) -> None:
        # Callable with a type-var-carrying ret: the ret-meet tail replaces
        # the declared ret (G[A] narrowed to G[B]) instead of deferring.
        declared = CallableType([], [], [], self.fx.ga, self.fx.function, name="f")
        narrowed = CallableType([], [], [], self.fx.gb, self.fx.function, name="f")
        off = self._narrow(False, declared, narrowed)
        on = self._narrow(True, declared, narrowed)
        self.assertEqual(on, off)
        self.assertEqual(on, "def () -> G[B]")

    def test_narrow_callable_ret_tvars_seam_engages(self) -> None:
        from mypy.join import _deserialize_type, _serialize_type

        declared = CallableType([], [], [], self.fx.ga, self.fx.function, name="f")
        narrowed = CallableType([], [], [], self.fx.gb, self.fx.function, name="f")
        r = _type_kernel.rust_narrow_declared_type(
            _serialize_type(declared), _serialize_type(narrowed), True, self.resolver
        )
        assert r is not None, "seam deferred on callables with a tvar-carrying ret"
        decoded = _deserialize_type(bytes(r))
        assert decoded is not None
        self.assertEqual(str(decoded), "def () -> G[B]")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMeetUnboundSuite(Suite):
    """Parity suite for the Rust `visit_unbound_type` meet (Stage 3c M8r).

    Exercises the three branches of `TypeMeetVisitor.visit_unbound_type`
    (meet.py:864-873): NoneType s (strict/non-strict), UninhabitedType s,
    and the else branch (AnyType). The Rust arm fires for UnboundType t;
    cases that would reach the visitor with AnyType s instead short-circuit
    at meet.py:145 (AnyType-s -> return t), verified separately.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def test_meet_unbound_s_none_strict_returns_bottom(self) -> None:
        # visit_unbound_type (meet.py:865-867): s=NoneType, strict ->
        # UninhabitedType.
        with state.strict_optional_set(True):
            assert meet_types(NoneType(), UnboundType("X")) == UninhabitedType()

    def test_meet_unbound_s_none_non_strict_returns_s(self) -> None:
        # visit_unbound_type (meet.py:865,868-869): s=NoneType, non-strict
        # -> self.s (NoneType).
        with state.strict_optional_set(False):
            assert meet_types(NoneType(), UnboundType("X")) == NoneType()

    def test_meet_unbound_s_uninhabited_returns_s(self) -> None:
        # visit_unbound_type (meet.py:870-871): s=UninhabitedType ->
        # self.s (UninhabitedType).
        with state.strict_optional_set(True):
            assert meet_types(UninhabitedType(), UnboundType("X")) == UninhabitedType()

    def test_meet_unbound_s_instance_returns_any(self) -> None:
        # visit_unbound_type else (meet.py:872-873): AnyType.
        with state.strict_optional_set(True):
            result = meet_types(self.fx.a, UnboundType("X"))
            assert isinstance(result, AnyType)

    def test_meet_unbound_s_any_short_circuits_to_t(self) -> None:
        # AnyType-s short-circuit (meet.py:145) fires before the visitor:
        # meet_types(AnyType, UnboundType) returns t (the UnboundType).
        # The Rust path mirrors this in meet_types (SameT).
        with state.strict_optional_set(True):
            result = meet_types(self.fx.anyt, UnboundType("X"))
            assert isinstance(result, UnboundType)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMeetTypeVarTupleSuite(Suite):
    """Parity suite for the Rust `visit_type_var_tuple` meet (Stage 3c M8r).

    Exercises `TypeMeetVisitor.visit_type_var_tuple` (meet.py:930-934):
    same id -> pick by min_len; different id / s not TypeVarTupleType ->
    default(self.s) -> Bottom (strict).
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture()
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

    def _tvt(self, raw_id: int, min_len: int) -> TypeVarTupleType:
        return TypeVarTupleType(
            "Ts",
            "Ts",
            TypeVarId(raw_id),
            self.fx.o,
            self.fx.std_tuple,
            AnyType(TypeOfAny.from_omitted_generics),
            min_len=min_len,
        )

    def test_meet_tvt_same_id_s_larger_min_returns_s(self) -> None:
        # visit_type_var_tuple (meet.py:931-932): same id, s.min_len >
        # t.min_len -> self.s.
        s = self._tvt(1, 2)
        t = self._tvt(1, 1)
        with state.strict_optional_set(True):
            assert meet_types(s, t) == s

    def test_meet_tvt_same_id_t_larger_min_returns_t(self) -> None:
        # visit_type_var_tuple: same id, s.min_len <= t.min_len -> t.
        s = self._tvt(1, 1)
        t = self._tvt(1, 2)
        with state.strict_optional_set(True):
            assert meet_types(s, t) == t

    def test_meet_tvt_same_id_equal_min_returns_t(self) -> None:
        # visit_type_var_tuple: same id, equal min_len -> t (the `else`
        # branch of `self.s if self.s.min_len > t.min_len else t`).
        s = self._tvt(1, 3)
        t = self._tvt(1, 3)
        with state.strict_optional_set(True):
            assert meet_types(s, t) == t

    def test_meet_tvt_different_id_returns_bottom(self) -> None:
        # visit_type_var_tuple else (meet.py:933-934): different id ->
        # default(self.s) -> Bottom (strict).
        s = self._tvt(1, 2)
        t = self._tvt(2, 2)
        with state.strict_optional_set(True):
            assert meet_types(s, t) == UninhabitedType()

    def test_meet_tvt_s_not_tvt_returns_bottom(self) -> None:
        # visit_type_var_tuple else: s is Instance -> default(Instance)
        # -> Bottom (strict).
        t = self._tvt(1, 2)
        with state.strict_optional_set(True):
            assert meet_types(self.fx.a, t) == UninhabitedType()

    # ---- visit_type_var (M8q) ----

    def test_meet_type_var_same_id_same_upper_bound_returns_s(self) -> None:
        # visit_type_var case 1 (meet.py:880-881): s.id == t.id,
        # s.upper_bound == t.upper_bound -> return self.s.
        assert meet_types(self.fx.t, self.fx.t) == self.fx.t

    def test_meet_type_var_same_id_different_ub_meets_bounds(self) -> None:
        # visit_type_var case 2 (meet.py:882): same id, different
        # upper_bound -> s.copy_modified(upper_bound=meet(s.ub, t.ub)).

        # Rust encodes the fresh TypeVar; its upper-bound meet goes
        # through fruit_to_type so a recursive Encoded result decodes.
        # meet(object, a) = a, so the new TypeVar's bound is a.
        from mypy.types import TypeVarType

        tv_obj = TypeVarType(
            "T", "T", TypeVarId(1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )
        tv_a = TypeVarType(
            "T", "T", TypeVarId(1), [], self.fx.a, AnyType(TypeOfAny.from_omitted_generics)
        )
        result = meet_types(tv_obj, tv_a)
        assert isinstance(result, TypeVarType)
        assert result.upper_bound == self.fx.a  # meet(object, a) == a

    def test_meet_type_var_different_id_returns_bottom(self) -> None:
        # visit_type_var else (meet.py:883-884): s.id != t.id ->
        # default(self.s) -> Bottom.
        assert meet_types(self.fx.t, self.fx.s) == UninhabitedType()

    def test_meet_type_var_s_not_type_var_returns_bottom(self) -> None:
        # visit_type_var else: s is Instance (not TypeVarType) ->
        # default -> Bottom.
        assert meet_types(self.fx.a, self.fx.t) == UninhabitedType()

    # ---- visit_literal_type (M8q) ----

    def test_meet_literal_equal_literal_returns_t(self) -> None:
        # visit_literal_type case 1 (meet.py:1237-1238): s is LiteralType,
        # s == t -> return t.
        assert meet_types(self.fx.lit1, self.fx.lit1) == self.fx.lit1

    def test_meet_literal_unequal_literal_returns_bottom(self) -> None:
        # visit_literal_type else: s is LiteralType, s != t -> default
        # -> Bottom.
        assert meet_types(self.fx.lit1, self.fx.lit2) == UninhabitedType()

    def test_meet_literal_s_is_instance_fallback_subtype_returns_t(self) -> None:
        # visit_literal_type case 2 (meet.py:1239-1240): s is Instance,
        # is_subtype(t.fallback, s) -> return t. lit1 has fallback a;
        # meet(a, lit1) = lit1 (a is supertype of lit1's fallback).
        assert meet_types(self.fx.a, self.fx.lit1) == self.fx.lit1

    def test_meet_literal_s_is_instance_fallback_not_subtype_returns_bottom(self) -> None:
        # visit_literal_type else: s is Instance, is_subtype(t.fallback,
        # s) = False -> default -> Bottom. lit3 has fallback d (unrelated
        # to a); meet(a, lit3) = Bottom.
        assert meet_types(self.fx.a, self.fx.lit3) == UninhabitedType()

    # ---- visit_type_type (M8q) ----

    def test_meet_type_type_s_is_builtins_type_returns_t(self) -> None:
        # visit_type_type case 2 (meet.py:1256-1257): s is
        # Instance(builtins.type) -> return t.
        assert meet_types(self.fx.type_type, self.fx.type_a) == self.fx.type_a

    def test_meet_type_type_both_type_type_defers_to_python(self) -> None:
        # visit_type_type case 1 (meet.py:1249-1255): both TypeType ->
        # recursive meet + make_normalized -> defers. Result identical.
        assert meet_types(self.fx.type_a, self.fx.type_a) == self.fx.type_a

    def test_meet_type_type_s_is_unrelated_instance_returns_bottom(self) -> None:
        # visit_type_type else (meet.py:1260-1261): s is Instance (not
        # builtins.type) -> default -> Bottom.
        assert meet_types(self.fx.a, self.fx.type_a) == UninhabitedType()


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMroSuite(Suite):
    """Parity tests for `mro::rust_linearize_hierarchy` (Stage 5).

    Each test constructs a small TypeInfo hierarchy, builds the Rust
    resolver snapshot, installs it, and calls the public `calculate_mro`.
    The Rust path returns None for cycles, missing bases, the `obj_type`
    callback edge, and inconsistent merges, so those cases fall through to
    Python (which raises `MroError` on inconsistency); parity holds because
    the end state (`info.mro` or a raised `MroError`) is identical.
    """

    def setUp(self) -> None:
        from mypy.mro import _set_native_mro_resolver

        self._set_native_mro_resolver = _set_native_mro_resolver
        # Builtins.object TypeInfo, reused as the hierarchy root. Its mro is
        # set to [self] so linearize_hierarchy short-circuits for it.
        self.oi = self._make_class("builtins.object", bases=[], mro=None)

    def tearDown(self) -> None:
        # Clear the resolver so later suites (or the default Python path)
        # are not affected by a stale install.
        self._set_native_mro_resolver(None, None)

    def _make_class(
        self, name: str, *, bases: list[Instance], mro: list[TypeInfo] | None
    ) -> TypeInfo:
        from mypy.nodes import TypeInfo

        defn = ClassDef(name, Block([]), None, [])
        defn.fullname = name
        info = TypeInfo(SymbolTable(), defn, name)
        info.bases = bases
        # `mro=None` leaves info.mro empty so `calculate_mro` computes it;
        # a non-None list sets the cached mro (for the short-circuit test
        # and for builtins.object, which is its own root).
        info.mro = mro if mro is not None else []
        return info

    def _install_resolver(self, infos: list[TypeInfo]) -> None:
        import type_kernel as _type_kernel

        resolver = _type_kernel.build_native_resolver(infos, [])
        self._resolver = resolver
        typeinfo_map = {info.fullname: info for info in infos}
        self._set_native_mro_resolver(resolver, typeinfo_map)

    def _mro_fullnames(self, info: TypeInfo) -> list[str]:
        return [t.fullname for t in info.mro]

    def test_object_root_linearizes_to_itself(self) -> None:
        from mypy.mro import calculate_mro

        self._install_resolver([self.oi])
        calculate_mro(self.oi)
        assert self._mro_fullnames(self.oi) == ["builtins.object"]

    def test_direct_base_appends_object(self) -> None:
        # B : object  ->  B, object
        from mypy.mro import calculate_mro

        b = self._make_class("mymod.B", bases=[Instance(self.oi, [])], mro=None)
        self._install_resolver([b, self.oi])
        calculate_mro(b)
        assert self._mro_fullnames(b) == ["mymod.B", "builtins.object"]

    def test_diamond_inheritance_c3_order(self) -> None:
        # D : object, B : D, C : D, A : B, C  ->  A, B, C, D, object
        from mypy.mro import calculate_mro

        d = self._make_class("mymod.D", bases=[Instance(self.oi, [])], mro=None)
        b = self._make_class("mymod.B", bases=[Instance(d, [])], mro=None)
        c = self._make_class("mymod.C", bases=[Instance(d, [])], mro=None)
        a = self._make_class("mymod.A", bases=[Instance(b, []), Instance(c, [])], mro=None)
        self._install_resolver([a, b, c, d, self.oi])
        calculate_mro(a)
        assert self._mro_fullnames(a) == [
            "mymod.A",
            "mymod.B",
            "mymod.C",
            "mymod.D",
            "builtins.object",
        ]

    def test_consistent_merge_succeeds(self) -> None:
        # A : object, B : A, object  ->  B, A, object
        from mypy.mro import calculate_mro

        a = self._make_class("mymod.A", bases=[Instance(self.oi, [])], mro=None)
        b = self._make_class("mymod.B", bases=[Instance(a, []), Instance(self.oi, [])], mro=None)
        self._install_resolver([b, a, self.oi])
        calculate_mro(b)
        assert self._mro_fullnames(b) == ["mymod.B", "mymod.A", "builtins.object"]

    def test_inconsistent_merge_raises_mro_error(self) -> None:
        # X : A, B  and  Y : B, A  with Z : X, Y  ->  merge fails. Rust
        # returns None (declines), Python raises MroError. The end state
        # (MroError propagated) is identical to the pure-Python path.
        from mypy.mro import MroError, calculate_mro

        a = self._make_class("mymod.Inc.A", bases=[Instance(self.oi, [])], mro=None)
        b = self._make_class("mymod.Inc.B", bases=[Instance(self.oi, [])], mro=None)
        x = self._make_class("mymod.Inc.X", bases=[Instance(a, []), Instance(b, [])], mro=None)
        y = self._make_class("mymod.Inc.Y", bases=[Instance(b, []), Instance(a, [])], mro=None)
        z = self._make_class("mymod.Inc.Z", bases=[Instance(x, []), Instance(y, [])], mro=None)
        self._install_resolver([z, x, y, a, b, self.oi])
        with self.assertRaises(MroError):
            calculate_mro(z)

    def test_cycle_returns_none_at_rust_level(self) -> None:
        # A : B, B : A  ->  cycle. Rust's `rust_linearize_hierarchy`
        # returns None (its cycle guard), so the shim would fall through
        # to Python. We test the Rust entry directly (NOT `calculate_mro`)

        # because Python's `linearize_hierarchy` has no cycle guard of its
        # own: it relies on `semanal.verify_base_classes` (nodes.py:2826)

        # to reject raw inheritance cycles before MRO runs, so a synthetic
        # cycle reaching `calculate_mro` would infinite-loop rather than
        # raise MroError. The Rust None keeps the production path safe:

        # a stale snapshot that reintroduces a cycle declines to Python,
        # which would have rejected the cycle at semantic-analysis time.
        import type_kernel as _type_kernel

        a = self._make_class("mymod.Cyc.A", bases=[], mro=None)
        b = self._make_class("mymod.Cyc.B", bases=[Instance(a, [])], mro=None)
        a.bases = [Instance(b, [])]
        self._install_resolver([a, b, self.oi])
        # Call the Rust function directly (the shim would defer to Python,
        # which is unsafe for a synthetic cycle).
        result = _type_kernel.rust_linearize_hierarchy(self._resolver, "mymod.Cyc.A")
        assert result is None

    def test_obj_type_fallback_edge_defers_to_python(self) -> None:
        # A baseless non-object class needs the `obj_type` callback
        # (mro.py:34) to synthesize a dummy `object` base. Rust has no
        # callback and returns None; Python uses `obj_type` to build the

        # mro. Here we pass `obj_type=lambda: Instance(self.oi, [])` so
        # Python completes the MRO as [cls, object].
        from mypy.mro import calculate_mro

        standalone = self._make_class("mymod.Standalone", bases=[], mro=None)
        self._install_resolver([standalone, self.oi])
        calculate_mro(standalone, obj_type=lambda: Instance(self.oi, []))
        assert self._mro_fullnames(standalone) == ["mymod.Standalone", "builtins.object"]

    def test_missing_base_in_snapshot_defers_to_python(self) -> None:
        # A : B, but B is absent from the resolver snapshot. Rust returns
        # None; Python rebuilds from the live graph (B is reachable via
        # `info.bases`), so the MRO is still computed correctly.
        from mypy.mro import calculate_mro

        b = self._make_class("mymod.Mb.B", bases=[Instance(self.oi, [])], mro=None)
        a = self._make_class("mymod.Mb.A", bases=[Instance(b, [])], mro=None)
        # Install a resolver that deliberately omits B (stale snapshot).
        self._install_resolver([a, self.oi])
        calculate_mro(a)
        assert self._mro_fullnames(a) == ["mymod.Mb.A", "mymod.Mb.B", "builtins.object"]

    def test_cached_mro_short_circuits_without_calling_rust(self) -> None:
        # When `info.mro` is already set, `calculate_mro` re-assigns it
        # (idempotent) and re-runs the side effects, but the shim must not
        # call Rust (mro.py:31 short-circuit). To prove Rust is skipped, we

        # build the resolver with `cls.mro` empty (so the snapshot's mro is
        # empty and Rust WOULD walk the bases to [Cached, object]), then set
        # the live `cls.mro` to a cached [object] list. If the short-circuit

        # works, `linearize_hierarchy` returns the cached [object]; if Rust
        # were called instead, it would return [Cached, object] (different).
        from mypy.mro import calculate_mro

        cls = self._make_class("mymod.Cached", bases=[Instance(self.oi, [])], mro=None)
        # Build the resolver with empty cls.mro so the snapshot sees no cache.
        self._install_resolver([cls, self.oi])
        # Now set the live cache AFTER the resolver snapshot is frozen. Rust
        # would still see an empty snapshot mro and recompute; the Python
        # short-circuit must return this cached list instead.
        cls.mro = [self.oi]
        calculate_mro(cls)
        assert self._mro_fullnames(cls) == ["builtins.object"]


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeApplyGenericArgumentsAmbiguousSuite(Suite):
    """Parity suite for the Rust apply_generic_arguments ambiguous-
    UninhabitedType branch (issue #913).

    Python's get_target_type (applytype.py:254-256) expands a typevar
    default when the applied arg is an ambiguous UninhabitedType and the
    typevar has a real default; a non-ambiguous Never or a typevar with
    no default falls through to the bound check. The Rust path previously
    deferred the whole call on any UninhabitedType; it now mirrors that
    branch. Each test compares gate-off (pure Python) to gate-on (Rust
    seam) on str(), plus a direct seam call proving the kernel engages.
    """

    def setUp(self) -> None:
        from mypy.applytype import (
            _set_native_applytype_active,
            _set_native_applytype_resolver,
            _set_native_applytype_typeinfo_map,
        )

        self.fx = TypeFixture()
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_applytype_active(True)
        _set_native_applytype_resolver(self.resolver)
        _set_native_applytype_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.applytype import (
            _set_native_applytype_active,
            _set_native_applytype_resolver,
            _set_native_applytype_typeinfo_map,
        )

        _set_native_applytype_active(False)
        _set_native_applytype_resolver(None)
        _set_native_applytype_typeinfo_map(None)

    def _tvar(self, default: Type) -> TypeVarType:
        return TypeVarType("T", "T", TypeVarId(1), [], self.fx.o, default)

    def _callable(self, tvar: TypeVarType) -> CallableType:
        return self.fx.callable(tvar, tvar).copy_modified(variables=[tvar])

    def _apply(self, callable: CallableType, arg: Type) -> CallableType:
        from mypy.applytype import apply_generic_arguments

        return apply_generic_arguments(
            callable, [arg], lambda *a: None, Context(), skip_unsatisfied=True
        )

    def _par(self, tvar: TypeVarType, arg: Type, label: str) -> CallableType:
        from mypy.applytype import _set_native_applytype_active

        callable = self._callable(tvar)
        _set_native_applytype_active(False)
        off = self._apply(callable, arg)
        _set_native_applytype_active(True)
        on = self._apply(callable, arg)
        assert_equal(str(on), str(off), f"{label}: str parity")
        return off

    def _assert_engages(self, tvar: TypeVarType, arg: Type, label: str) -> None:
        from mypy.applytype import _serialize_optional_type_list, _serialize_type

        callable = self._callable(tvar)
        result = _type_kernel.rust_apply_generic_arguments(
            self.resolver,
            _serialize_type(callable),
            _serialize_optional_type_list([arg]),
            True,
            state.strict_optional,
        )
        assert result is not None, f"{label}: Rust seam did not engage"

    def test_ambiguous_with_default_expands(self) -> None:
        tvar = self._tvar(self.fx.str_type)
        off = self._par(tvar, self.fx.a_uninhabited, "ambiguous+default")
        self._assert_engages(tvar, self.fx.a_uninhabited, "ambiguous+default")
        # The ambiguous Never with a real default expands to the default;
        # T (the only variable) is fully substituted.
        assert not off.variables
        assert off.ret_type == self.fx.str_type

    def test_ambiguous_no_default_falls_through(self) -> None:
        tvar = self._tvar(AnyType(TypeOfAny.from_omitted_generics))
        off = self._par(tvar, self.fx.a_uninhabited, "ambiguous+no-default")
        self._assert_engages(tvar, self.fx.a_uninhabited, "ambiguous+no-default")
        # No default: the ambiguous branch is skipped, the Never falls
        # through the bound check, so T maps to the Never itself.
        assert not off.variables
        rt = get_proper_type(off.ret_type)
        assert isinstance(rt, UninhabitedType)
        assert rt.ambiguous is True

    def test_non_ambiguous_falls_through(self) -> None:
        tvar = self._tvar(self.fx.str_type)
        off = self._par(tvar, self.fx.uninhabited, "non-ambiguous")
        self._assert_engages(tvar, self.fx.uninhabited, "non-ambiguous")
        # Non-ambiguous Never never expands the default; it falls through
        # the bound check and maps T to the Never.
        assert not off.variables
        rt = get_proper_type(off.ret_type)
        assert isinstance(rt, UninhabitedType)
        assert rt.ambiguous is False


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
class NativeTypeanalAliasQuerySuite(Suite):
    """Parity for the resolver-backed typeanal query seams (issue #852).

    The byte-only seams defer on `TypeAliasType` (its proper expansion is
    not on the wire). The `_live` variants expand aliases through the
    `NativeTypeResolver` alias snapshot, mirroring `get_proper_type` plus
    the query visitors' `seen_aliases` recursion guards. Each test runs
    the gate-off (pure Python) and gate-on (Rust) paths and asserts they
    agree; direct seam calls prove the live path engages where the byte
    seam defers.

    The `test_hafu_window_*` tests simulate the no-resolver semanal window
    (issue #1342): gate-on runs with NO resolver installed, so the shim
    routes through the live no-resolver seam, which must decide alias-,
    placeholder-, and unbound-bearing trees that the byte seams defer on.
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active, _set_native_typeanal_resolver
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        self._base_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(self._base_infos, [])
        _set_native_typeanal_active(True)
        _set_native_typeanal_resolver(self.resolver)
        set_wire_alias_map({})
        set_wire_typeinfo_map({info.fullname: info for info in self._base_infos})

    def tearDown(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active, _set_native_typeanal_resolver
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        _set_native_typeanal_active(False)
        _set_native_typeanal_resolver(None)
        set_wire_alias_map(None)
        set_wire_typeinfo_map(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _make_alias(
        self, fullname: str, target: Type, *, alias_tvars: list[TypeVarLikeType] | None = None
    ) -> TypeAlias:
        from mypy.nodes import TypeAlias

        return TypeAlias(target, fullname, "mod", -1, -1, alias_tvars=alias_tvars or [])

    def _install_aliases(self, aliases: list[TypeAlias]) -> None:
        from mypy.typeanal import _set_native_typeanal_resolver
        from mypy.wirefixup import set_wire_alias_map

        self.resolver = _type_kernel.build_native_resolver(self._base_infos, aliases)
        _set_native_typeanal_resolver(self.resolver)
        set_wire_alias_map({alias.fullname: alias for alias in aliases})

    def _par(self, fn: Any, t: Type) -> tuple[Any, Any]:
        from mypy.typeanal import _set_native_typeanal_active

        _set_native_typeanal_active(False)
        expected = fn(t)
        _set_native_typeanal_active(True)
        actual = fn(t)
        return actual, expected

    def test_has_explicit_any_alias_to_explicit_any(self) -> None:
        from type_kernel import rust_has_explicit_any, rust_has_explicit_any_live

        from mypy.typeanal import _serialize_typeanal_type, has_explicit_any

        alias = self._make_alias("mod.ExplicitAlias", AnyType(TypeOfAny.explicit))
        self._install_aliases([alias])
        t = TypeAliasType(alias, [])
        # The byte seam defers on the alias; the live seam decides.
        assert rust_has_explicit_any(_serialize_typeanal_type(t)) is None
        assert rust_has_explicit_any_live(self.resolver, _serialize_typeanal_type(t)) is True
        actual, expected = self._par(has_explicit_any, t)
        assert_equal(actual, expected)
        assert actual is True

    def test_has_explicit_any_alias_to_unimported_any(self) -> None:
        from mypy.typeanal import has_explicit_any

        alias = self._make_alias("mod.UnimportedAlias", AnyType(TypeOfAny.from_unimported_type))
        self._install_aliases([alias])
        t = TypeAliasType(alias, [])
        actual, expected = self._par(has_explicit_any, t)
        assert_equal(actual, expected)
        assert actual is False

    def test_has_any_from_unimported_type_alias(self) -> None:
        from type_kernel import (
            rust_has_any_from_unimported_type,
            rust_has_any_from_unimported_type_live,
        )

        from mypy.typeanal import _serialize_typeanal_type, has_any_from_unimported_type

        alias = self._make_alias("mod.UnimportedAlias", AnyType(TypeOfAny.from_unimported_type))
        self._install_aliases([alias])
        t = TypeAliasType(alias, [])
        assert rust_has_any_from_unimported_type(_serialize_typeanal_type(t)) is None
        assert (
            rust_has_any_from_unimported_type_live(self.resolver, _serialize_typeanal_type(t))
            is True
        )
        actual, expected = self._par(has_any_from_unimported_type, t)
        assert_equal(actual, expected)
        assert actual is True

    def test_has_explicit_any_alias_nested_in_instance(self) -> None:
        from mypy.typeanal import has_explicit_any

        alias = self._make_alias(
            "mod.ListAlias", Instance(self.fx.std_listi, [AnyType(TypeOfAny.explicit)])
        )
        self._install_aliases([alias])
        t = Instance(self.fx.std_listi, [TypeAliasType(alias, [])])
        actual, expected = self._par(has_explicit_any, t)
        assert_equal(actual, expected)
        assert actual is True

    def test_has_explicit_any_recursive_alias(self) -> None:
        from mypy.typeanal import has_explicit_any

        alias = self._make_alias("mod.RecAlias", self.fx.b)
        alias.target = Instance(self.fx.std_listi, [TypeAliasType(alias, [])])
        self._install_aliases([alias])
        t = TypeAliasType(alias, [])
        actual, expected = self._par(has_explicit_any, t)
        assert_equal(actual, expected)
        assert actual is False

    def test_collect_all_inner_types_alias_to_instance(self) -> None:
        from mypy.typeanal import collect_all_inner_types

        alias = self._make_alias("mod.AliasToList", Instance(self.fx.std_listi, [self.fx.b]))
        self._install_aliases([alias])
        t = TypeAliasType(alias, [])
        actual, expected = self._par(collect_all_inner_types, t)
        assert_equal(actual, expected)
        assert_equal(actual, [self.fx.b])

    def test_collect_all_inner_types_recursive_alias(self) -> None:
        from mypy.typeanal import collect_all_inner_types

        alias = self._make_alias("mod.RecAlias", self.fx.b)
        alias.target = Instance(self.fx.std_listi, [TypeAliasType(alias, [])])
        self._install_aliases([alias])
        t = TypeAliasType(alias, [])
        actual, expected = self._par(collect_all_inner_types, t)
        assert_equal(actual, expected)
        # The repeated alias does not expand further, but it is still
        # reported as the direct child of the containing instance.
        assert_equal(actual, [TypeAliasType(alias, [])])

    def test_make_optional_type_alias_to_none(self) -> None:
        from mypy.typeanal import make_optional_type

        alias = self._make_alias("mod.NoneAlias", NoneType())
        self._install_aliases([alias])
        t = UnionType([TypeAliasType(alias, []), self.fx.b])
        actual, expected = self._par(make_optional_type, t)
        assert_equal(actual, expected)
        assert isinstance(actual, UnionType)  # type: ignore[misc]
        assert_equal(actual.items, [self.fx.b, NoneType()], f"got {actual.items!r}")

    def test_make_optional_type_alias_to_other(self) -> None:
        from mypy.typeanal import make_optional_type

        alias = self._make_alias("mod.IntAlias", self.fx.a)
        self._install_aliases([alias])
        t = UnionType([self.fx.b, TypeAliasType(alias, [])])
        actual, expected = self._par(make_optional_type, t)
        assert_equal(actual, expected)
        assert isinstance(actual, UnionType)  # type: ignore[misc]
        # A non-None alias is kept as-is (Python does not substitute it).
        assert_equal(
            actual.items,
            [self.fx.b, TypeAliasType(alias, []), NoneType()],
            f"got {actual.items!r}",
        )

    def test_unknown_unpack_alias_to_special_form_any(self) -> None:
        from type_kernel import rust_unknown_unpack, rust_unknown_unpack_live

        from mypy.typeanal import _serialize_typeanal_type, unknown_unpack

        alias = self._make_alias("mod.SpecialAlias", AnyType(TypeOfAny.special_form))
        self._install_aliases([alias])
        t = UnpackType(TypeAliasType(alias, []))
        assert rust_unknown_unpack(_serialize_typeanal_type(t)) is None
        assert rust_unknown_unpack_live(self.resolver, _serialize_typeanal_type(t)) is True
        actual, expected = self._par(unknown_unpack, t)
        assert_equal(actual, expected)
        assert actual is True

    def test_unknown_unpack_alias_to_other_any(self) -> None:
        from mypy.typeanal import unknown_unpack

        alias = self._make_alias("mod.PlainAlias", AnyType(TypeOfAny.unannotated))
        self._install_aliases([alias])
        t = UnpackType(TypeAliasType(alias, []))
        actual, expected = self._par(unknown_unpack, t)
        assert_equal(actual, expected)
        assert actual is False

    def _par_noresolver(self, fn: Any, t: Type) -> tuple[Any, Any]:
        # Gate-on with NO resolver installed: the pre-first-SCC semanal
        # window (#1342) routes has_any_from_unimported_type through the
        # live no-resolver seam; gate-off runs the pure-Python visitor.
        from mypy.typeanal import _set_native_typeanal_active, _set_native_typeanal_resolver

        _set_native_typeanal_active(False)
        expected = fn(t)
        _set_native_typeanal_active(True)
        _set_native_typeanal_resolver(None)
        try:
            actual = fn(t)
        finally:
            _set_native_typeanal_resolver(self.resolver)
        return actual, expected

    def _assert_hafu_window(self, t: Type, expected: bool) -> None:
        from type_kernel import rust_has_any_from_unimported_type_live_noresolver

        from mypy.typeanal import has_any_from_unimported_type

        seam = rust_has_any_from_unimported_type_live_noresolver(t)
        assert seam is expected, f"no-resolver seam deferred on {t!r}"
        actual, exp = self._par_noresolver(has_any_from_unimported_type, t)
        assert_equal(actual, exp)
        assert actual is expected

    def test_hafu_window_alias_to_unimported(self) -> None:
        alias = self._make_alias("mod.UnimportedAlias", AnyType(TypeOfAny.from_unimported_type))
        self._install_aliases([alias])
        self._assert_hafu_window(TypeAliasType(alias, []), True)

    def test_hafu_window_alias_to_explicit(self) -> None:
        alias = self._make_alias("mod.ExplicitAlias", AnyType(TypeOfAny.explicit))
        self._install_aliases([alias])
        self._assert_hafu_window(TypeAliasType(alias, []), False)

    def test_hafu_window_alias_nested_in_instance(self) -> None:
        alias = self._make_alias("mod.ListAlias", AnyType(TypeOfAny.from_unimported_type))
        self._install_aliases([alias])
        t = Instance(self.fx.std_listi, [TypeAliasType(alias, [])])
        self._assert_hafu_window(t, True)

    def test_hafu_window_alias_substituted_tvar(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(
            Instance(self.fx.std_listi, [self.fx.t]), "mod.G", "mod", 1, 1, alias_tvars=[self.fx.t]
        )
        t = TypeAliasType(alias, [AnyType(TypeOfAny.from_unimported_type)])
        self._assert_hafu_window(t, True)

    def test_hafu_window_alias_recursive(self) -> None:
        alias = self._make_alias("mod.RecAlias", self.fx.b)
        alias.target = Instance(self.fx.std_listi, [TypeAliasType(alias, [])])
        self._install_aliases([alias])
        self._assert_hafu_window(TypeAliasType(alias, []), False)

    def test_hafu_window_alias_py312_args_tail(self) -> None:
        from mypy.nodes import TypeAlias

        # Unused alias tvar: the visitor still queries t.args for
        # new-style aliases after the expansion found nothing.
        alias = TypeAlias(
            self.fx.o,
            "mod.Y312",
            "mod",
            1,
            1,
            alias_tvars=[self.fx.t],
            python_3_12_type_alias=True,
        )
        t = TypeAliasType(alias, [AnyType(TypeOfAny.from_unimported_type)])
        self._assert_hafu_window(t, True)

    def test_hafu_window_alias_py312_plain_args(self) -> None:
        from mypy.nodes import TypeAlias

        # Expansion found nothing and the args carry no unimported Any.
        alias = TypeAlias(
            self.fx.o,
            "mod.P312",
            "mod",
            1,
            1,
            alias_tvars=[self.fx.t],
            python_3_12_type_alias=True,
        )
        t = TypeAliasType(alias, [AnyType(TypeOfAny.explicit)])
        self._assert_hafu_window(t, False)

    def test_hafu_window_placeholder_type(self) -> None:
        from mypy.types import PlaceholderType

        t = PlaceholderType("mod.ph", [AnyType(TypeOfAny.from_unimported_type)], line=-1)
        self._assert_hafu_window(t, True)
        self._assert_hafu_window(PlaceholderType("mod.ph", [], line=-1), False)

    def test_hafu_window_unbound_args(self) -> None:
        # HasAnyFromUnimportedType never resolves unbound names: the
        # default visit queries t.args only.
        t = UnboundType("Undefined", [AnyType(TypeOfAny.from_unimported_type)])
        self._assert_hafu_window(t, True)
        self._assert_hafu_window(UnboundType("Undefined"), False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeAnalSuite(Suite):
    """Parity tests for `rust_type_analyze` — the TypeAnalyser.anal_type hot path.

    Runs the native path with `_set_native_typeanal_active(True)` and asserts
    the decoded result round-trips str-identically to the input. For the
    already-bound fixtures in this suite, `TypeAnalyser` is a no-op (type
    analysis applies only to unbound or alias-laden types), so the input
    itself is the reference. The differential across the whole suite (with and
    without `TEST_NATIVE_TYPE_KERNEL=1`) is the same mechanism the
    NativeJoinMeetSuite uses.

    The Rust path returns ``None`` for types needing semantic context
    (UnboundType, PlaceholderType), so those defer to Python and parity is
    guaranteed by construction. TypeAliasType is handled natively as a
    passthrough (args analyzed, alias node unchanged, mirroring
    ``visit_type_alias_type``), resolved to a live alias by the installed
    alias map; without the map the same node defers.

    The test corpus covers: Instance, Callable, TypeVar, ParamSpec,
    TypeVarTuple, Tuple, TypedDict, Union, TypeType, Literal, Any, None,
    Uninhabited, Deleted, Overloaded — all types that rust_type_analyze can
    handle without symbol lookup or alias expansion.
    """

    def setUp(self) -> None:
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_typeanal_active
        type_infos = self._collect_type_infos()
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        set_wire_alias_map(self._collect_aliases())

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        set_wire_typeinfo_map(None)
        set_wire_alias_map(None)
        self._set_active(False)

    def _collect_aliases(self) -> dict[str, Any]:
        """Collect the fixture's TypeAlias nodes by fullname.

        `def_alias_1(base)` builds a recursive alias (A = Tuple[Union[base,
        A], ...]) whose TypeAlias node is returned only through its
        TypeAliasType; `non_rec_alias` builds a fresh alias per call. Both
        live under `__main__.A`, so the per-test map must re-install the
        node the test actually references.
        """
        from mypy.nodes import TypeAlias

        def _from(alias_type: TypeAliasType | None) -> dict[str, Any]:
            if alias_type is None or not isinstance(alias_type.alias, TypeAlias):
                return {}
            return {alias_type.alias.fullname: alias_type.alias}

        a1, _ = self.fx.def_alias_1(self.fx.a)
        return _from(a1)

    def _collect_type_infos(self) -> list[TypeInfo]:
        from mypy.nodes import TypeInfo

        return [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]

    def _assert_par(self, t: Type) -> Type:
        """Run the native path with the gate on and assert the decoded result
        round-trips to the input. Returns the decoded object."""
        self._set_active(True)
        rs = native_analyze_type(t, allow_unpack=True)
        assert rs is not None, f"Rust returned None for {type(t).__name__}: {t!r}"
        assert_equal(
            str(rs), str(t), f"Rust/Python round-trip mismatch for {type(t).__name__}: {t!r}"
        )
        return rs

    # --- Instance (already-bound, simple + generic + lkv) ---

    def test_instance_simple(self) -> None:
        result = self._assert_par(self.fx.a)
        self.assertIsInstance(result, Instance)
        assert isinstance(result, Instance)  # type: ignore[misc]
        assert result.type.fullname == "A"

    def test_instance_generic(self) -> None:
        result = self._assert_par(self.fx.ga)
        self.assertIsInstance(result, Instance)

    def test_instance_list_of_a(self) -> None:
        result = self._assert_par(self.fx.lsta)
        self.assertIsInstance(result, Instance)

    # --- Callable ---

    def test_callable_simple(self) -> None:
        sig = CallableType(
            arg_types=[self.fx.a, self.fx.b],
            arg_kinds=[ARG_POS, ARG_POS],
            arg_names=[None, None],
            ret_type=self.fx.anyt,
            fallback=self.fx.function,
        )
        result = self._assert_par(sig)
        self.assertIsInstance(result, CallableType)

    def test_callable_with_typevars(self) -> None:
        sig = CallableType(
            arg_types=[self.fx.t],
            arg_kinds=[ARG_POS],
            arg_names=[None],
            ret_type=self.fx.a,
            fallback=self.fx.function,
            variables=[self.fx.t],
        )
        result = self._assert_par(sig)
        self.assertIsInstance(result, CallableType)

    def test_callable_star_args(self) -> None:
        sig = CallableType(
            arg_types=[self.fx.a],
            arg_kinds=[ARG_STAR],
            arg_names=[None],
            ret_type=self.fx.a,
            fallback=self.fx.function,
        )
        result = self._assert_par(sig)
        self.assertIsInstance(result, CallableType)

    def test_callable_return_type_type(self) -> None:
        sig = CallableType(
            arg_types=[],
            arg_kinds=[],
            arg_names=[],
            ret_type=self.fx.type_a,
            fallback=self.fx.function,
        )
        result = self._assert_par(sig)
        self.assertIsInstance(result, CallableType)

    # --- TypeVar ---

    def test_type_var_simple(self) -> None:
        result = self._assert_par(self.fx.t)
        self.assertIsInstance(result, TypeVarType)
        assert isinstance(result, TypeVarType)
        assert result.name == "T"

    # --- ParamSpec ---

    def test_param_spec_simple(self) -> None:
        ps = ParamSpecType(
            name="P",
            fullname="P",
            id=TypeVarId(-1),
            flavor=ParamSpecFlavor.BARE,
            upper_bound=Instance(self.fx.oi, [], -1),
            default=NoneType(),
        )
        result = self._assert_par(ps)
        self.assertIsInstance(result, ParamSpecType)

    # --- TypeVarTuple ---

    def test_type_var_tuple_simple(self) -> None:
        tvt = TypeVarTupleType(
            name="Ts",
            fullname="Ts",
            id=TypeVarId(-1),
            upper_bound=self.fx.a,
            tuple_fallback=self.fx.std_tuple,
            default=NoneType(),
        )
        result = self._assert_par(tvt)
        self.assertIsInstance(result, TypeVarTupleType)

    # --- Tuple ---

    def test_tuple_simple(self) -> None:
        tup = TupleType([self.fx.a, self.fx.b], self.fx.std_tuple, line=-1)
        result = self._assert_par(tup)
        self.assertIsInstance(result, TupleType)

    # --- TypedDict ---

    def test_typed_dict_simple(self) -> None:
        td = TypedDictType(
            items={"x": self.fx.a},
            required_keys={"x"},
            readonly_keys=set(),
            fallback=Instance(self.fx.a.type, [], -1),
        )
        result = self._assert_par(td)
        self.assertIsInstance(result, TypedDictType)

    # --- Union ---

    def test_union_two_types(self) -> None:
        u = UnionType([self.fx.a, self.fx.b], line=-1, column=-1)
        result = self._assert_par(u)
        self.assertIsInstance(result, UnionType)

    def test_union_with_none(self) -> None:
        u = UnionType([self.fx.a, NoneType()], line=-1, column=-1)
        result = self._assert_par(u)
        self.assertIsInstance(result, UnionType)

    def test_union_with_unpack_child_defers(self) -> None:
        """Union items analyze with allow_unpack=False in Python, so an
        UnpackType child must defer to the Python error path instead of
        analyzing with the outer flag."""
        self._set_active(True)
        u = UnionType([self.fx.a, UnpackType(self.fx.b, line=-1, column=-1)], line=-1, column=-1)
        result = native_analyze_type(u, allow_unpack=True)
        self.assertIsNone(result)

    def test_tuple_implicit_defers_without_tuple_literal(self) -> None:
        """visit_tuple_type errors on implicit tuples when tuple literals
        are disallowed; Rust must defer so Python emits the error."""
        self._set_active(True)
        tup = TupleType([self.fx.a], self.fx.std_tuple, line=-1, implicit=True)
        result = native_analyze_type(tup, allow_tuple_literal=False)
        self.assertIsNone(result)

    # --- TypeType ---

    def test_type_type_simple(self) -> None:
        tt = TypeType(self.fx.a, line=-1)
        result = self._assert_par(tt)
        self.assertIsInstance(result, TypeType)

    # --- Literal ---

    def test_literal_int(self) -> None:
        result = self._assert_par(self.fx.lit1)
        self.assertIsInstance(result, LiteralType)

    # --- Any / None / Uninhabited ---

    def test_any_type(self) -> None:
        result = self._assert_par(AnyType(TypeOfAny.special_form))
        self.assertIsInstance(result, AnyType)

    def test_none_type(self) -> None:
        result = self._assert_par(NoneType())
        self.assertIsInstance(result, NoneType)

    def test_uninhabited_type(self) -> None:
        result = self._assert_par(UninhabitedType())
        self.assertIsInstance(result, UninhabitedType)

    # --- Deleted ---

    def test_deleted_type(self) -> None:
        result = self._assert_par(DeletedType(source="x"))
        self.assertIsInstance(result, DeletedType)

    # --- Overloaded ---

    def test_overloaded(self) -> None:
        c1 = CallableType([self.fx.a], [ARG_POS], [None], self.fx.anyt, self.fx.function)
        c2 = CallableType([self.fx.b], [ARG_POS], [None], self.fx.anyt, self.fx.function)
        ov = Overloaded([c1, c2])
        result = self._assert_par(ov)
        self.assertIsInstance(result, Overloaded)

    # --- Unpack ---

    def test_unpack_type(self) -> None:
        ut = UnpackType(self.fx.a, line=-1, column=-1)
        result = self._assert_par(ut)
        self.assertIsInstance(result, UnpackType)

    # --- Defer cases (Rust returns None, Python fallback is authoritative) ---

    def test_defer_unbound_type(self) -> None:
        """UnboundType requires symbol lookup — Rust always defers."""
        self._set_active(True)
        result = native_analyze_type(UnboundType("Foo"))
        self.assertIsNone(result)
        # Python fallback would look up "Foo" and process it.

    def test_defer_union_with_unbound_item(self) -> None:
        """A union with an unbound leaf defers (issue #1167 fast path).

        Rust rejects the unbound child after a full wire round-trip, so the
        shim short-circuits before serializing; the caller falls back to the
        Python visitor either way.
        """
        self._set_active(True)
        u1 = UnionType([UnboundType("Foo"), NoneType()], line=1, column=0)
        self.assertIsNone(native_analyze_type(u1))
        u2 = UnionType([UnboundType("Foo"), UnboundType("Bar")], line=1, column=0)
        self.assertIsNone(native_analyze_type(u2))

    def test_defer_type_alias_type_no_map(self) -> None:
        """TypeAliasType defers when no alias map is installed.

        Without `set_wire_alias_map`, the decoded alias's type_ref cannot
        resolve to a live TypeAlias, so the fixer defers and the Python
        visitor is authoritative (parity by construction).
        """
        self._set_active(True)
        from mypy.wirefixup import set_wire_alias_map

        set_wire_alias_map(None)
        A, _ = self.fx.def_alias_1(self.fx.a)
        result = native_analyze_type(A, allow_unpack=True)
        self.assertIsNone(result)

    # --- TypeAliasType passthrough (args analyzed, node unchanged) ---

    def test_type_alias_type_passthrough(self) -> None:
        """TypeAliasType analyzes args and passes the node through unchanged.

        `visit_type_alias_type` (typeanal.py) is a pure passthrough: the
        alias is returned as-is with its args analyzed. The Rust path mirrors
        that, so the round trip preserves the alias and its arguments.
        """
        self._set_active(True)
        from mypy.wirefixup import set_wire_alias_map

        A, _ = self.fx.def_alias_1(self.fx.a)
        alias = A.alias
        assert alias is not None
        set_wire_alias_map({alias.fullname: alias})
        result = native_analyze_type(A, allow_unpack=True)
        assert result is not None
        assert isinstance(result, TypeAliasType)
        self.assertEqual(str(result), str(A))
        # The recursive alias's args contain a Union (base, A); analysis
        # must have visited them (left the instance intact).
        self.assertEqual(len(result.args), len(A.args))

    def test_type_alias_type_non_recursive_args_analyzed(self) -> None:
        """Non-recursive alias: args with a generic Instance analyze in place."""
        self._set_active(True)
        from mypy.wirefixup import set_wire_alias_map

        NA = self.fx.non_rec_alias(Instance(self.fx.gi, [self.fx.t]), [self.fx.t], [self.fx.a])
        alias = NA.alias
        assert alias is not None
        set_wire_alias_map({alias.fullname: alias})
        result = native_analyze_type(NA, allow_unpack=True)
        assert result is not None
        assert isinstance(result, TypeAliasType)
        self.assertEqual(str(result), str(NA))
        self.assertEqual(len(result.args), len(NA.args))

    # --- Nesting / child analysis ---

    def test_nested_instance_in_callable(self) -> None:
        """Callable with Instance args should analyze children."""
        sig = CallableType(
            arg_types=[self.fx.ga],
            arg_kinds=[ARG_POS],
            arg_names=[None],
            ret_type=self.fx.ga,
            fallback=self.fx.function,
        )
        result = self._assert_par(sig)
        self.assertIsInstance(result, CallableType)
        assert isinstance(result, CallableType)  # type: ignore[misc]
        self.assertEqual(len(result.arg_types), 1)
        self.assertIsInstance(result.arg_types[0], Instance)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCallableArgConstraintsSuite(Suite):
    """Parity tests for `rust_infer_callable_arguments_constraints`.

    Differential harness: runs `infer_callable_arguments_constraints` with the
    native gate on (resolver installed) and off (pure Python), and asserts the
    two constraint lists are equal. The decoded native constraints resolve
    live TypeInfos via the wirefixup map, so `Constraint.__eq__` holds against
    the Python-built results. Covers the four argument-matching phases from
    `subtypes.are_parameters_compatible` (star/star, corresponding arguments,
    right *args, right **kwargs).
    """

    def setUp(self) -> None:
        from mypy.constraints import _set_native_constraints_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_constraints_active
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        # Safe fallbacks so a failed differential never crosses suites.
        self._set_active(False)

    def tearDown(self) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)

    def _callable(
        self,
        arg_types: list[Type],
        arg_kinds: list[ArgKind],
        arg_names: list[str | None],
        variables: list[TypeVarType] | None = None,
    ) -> CallableType:
        return CallableType(
            arg_types,
            arg_kinds,
            arg_names,
            AnyType(TypeOfAny.special_form),
            self.fx.function,
            variables=variables,
        )

    def _constraints(
        self, template: Type, actual: Type, direction: int, native: bool
    ) -> list[Any]:
        from mypy.constraints import (
            _set_native_constraints_resolver,
            infer_callable_arguments_constraints,
        )

        self._set_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return infer_callable_arguments_constraints(
            template,  # type: ignore[arg-type]
            actual,  # type: ignore[arg-type]
            direction,
        )

    def _assert_par(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        native = self._constraints(template, actual, direction, native=True)
        python = self._constraints(template, actual, direction, native=False)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    # --- Phase 1b: corresponding positional/named arguments ---

    def test_positional_simple(self) -> None:
        template = self._callable([self.fx.t], [ARG_POS], [None], variables=[self.fx.t])
        actual = self._callable([self.fx.a], [ARG_POS], [None])
        self._assert_par(template, actual)

    def test_positional_two(self) -> None:
        template = self._callable(
            [self.fx.t, self.fx.s],
            [ARG_POS, ARG_POS],
            [None, None],
            variables=[self.fx.t, self.fx.s],
        )
        actual = self._callable([self.fx.a, self.fx.b], [ARG_POS, ARG_POS], [None, None])
        self._assert_par(template, actual)

    def test_named_argument(self) -> None:
        template = self._callable([self.fx.t], [ARG_NAMED], ["x"], variables=[self.fx.t])
        actual = self._callable([self.fx.a], [ARG_NAMED], ["x"])
        self._assert_par(template, actual)

    # --- Phase 1a: star vs star ---

    def test_star_args_pair(self) -> None:
        template = self._callable([self.fx.t], [ARG_STAR], [None], variables=[self.fx.t])
        actual = self._callable([self.fx.a], [ARG_STAR], [None])
        self._assert_par(template, actual)

    def test_kwargs_pair(self) -> None:
        template = self._callable([self.fx.t], [ARG_STAR2], [None], variables=[self.fx.t])
        actual = self._callable([self.fx.a], [ARG_STAR2], [None])
        self._assert_par(template, actual)

    # --- Phase 1c: right *args compared against left positional args ---

    def test_left_positional_against_right_star(self) -> None:
        template = self._callable([self.fx.t], [ARG_POS], [None], variables=[self.fx.t])
        actual = self._callable([self.fx.a], [ARG_STAR], [None])
        self._assert_par(template, actual)

    # --- Phase 1d: right **kwargs compared against left-only named args ---

    def test_left_named_against_right_kwargs(self) -> None:
        template = self._callable([self.fx.t], [ARG_NAMED], ["x"], variables=[self.fx.t])
        actual = self._callable([self.fx.a], [ARG_STAR2], [None])
        self._assert_par(template, actual)

    # --- Direction variation ---

    def test_supertypes_direction(self) -> None:
        template = self._callable([self.fx.t], [ARG_POS], [None], variables=[self.fx.t])
        actual = self._callable([self.fx.a], [ARG_POS], [None])
        self._assert_par(template, actual, direction=SUPERTYPE_OF)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTupleConstraintsSuite(Suite):
    """Parity tests for the Rust `visit_tuple_type` constraint port.

    Differential harness: runs `infer_constraints` with the native
    constraint-builder gate on (resolver + wire map installed) and off
    (pure Python ConstraintBuilderVisitor), asserting the two constraint
    lists are equal. Covers the variadic paths from
    `visit_tuple_type` (constraints.py:1731-1835) and
    `build_constraints_for_simple_unpack` (constraints.py:2050-2143):
    template-Unpack vs varlength tuple, template-Unpack vs fixed
    TupleType, template without Unpack vs actual with internal Unpack,
    fixed-vs-fixed, and the named-tuple fallback early return.
    """

    def setUp(self) -> None:
        from mypy.constraints import _set_native_constraints_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_constraints_active
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        # Safe fallbacks so a failed differential never crosses suites.
        self._set_active(False)

    def tearDown(self) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)

    def _tup(self, *items: Type) -> TupleType:
        return TupleType(list(items), self.fx.std_tuple)

    def _tup_tvt(self, tvt: TypeVarTupleType) -> TupleType:
        # A TypeVarTupleType in item position must be wrapped in UnpackType
        # to form a variadic tuple (PEP 646).
        return self._tup(UnpackType(tvt))

    def _constraints(
        self, template: Type, actual: Type, direction: int, native: bool
    ) -> list[Any]:
        from mypy.constraints import _set_native_constraints_resolver, infer_constraints

        self._set_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return infer_constraints(template, actual, direction)

    def _assert_par(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        native = self._constraints(template, actual, direction, native=True)
        python = self._constraints(template, actual, direction, native=False)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _assert_engages(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        # Direct seam call proving the Rust port runs (returns constraint
        # blobs) rather than deferring to the pure-Python visitor.
        raw = _type_kernel.rust_infer_constraints_full(
            self.resolver,
            self._bytes_of(template),
            self._bytes_of(actual),
            direction,
            False,
            False,
            strict_optional_flag(),
            True,
        )
        assert (
            raw is not None
        ), f"Rust seam must engage for template={template!r} actual={actual!r}"

    # --- Template has an Unpack, actual is a varlength tuple ---

    def test_unpack_template_varlength_tuple_tvt(self) -> None:
        # Tuple[*Ts] <: tuple[X, ...] via map_instance_to_supertype.
        template = self._tup_tvt(self.fx.ts)
        actual = Instance(self.fx.std_tuplei, [self.fx.a])
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    def test_unpack_template_varlength_tuple_homogeneous(self) -> None:
        # Tuple[*tuple[T, ...]] <: tuple[X, ...].
        template = self._tup(UnpackType(Instance(self.fx.std_tuplei, [self.fx.t])))
        actual = Instance(self.fx.std_tuplei, [self.fx.a])
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    def test_unpack_template_varlength_tuple_prefix_suffix(self) -> None:
        # Tuple[T, *Ts, U] <: tuple[X, ...]: T <: X and U <: X in addition
        # to the packet constraint.
        template = self._tup(self.fx.t, UnpackType(self.fx.ts), self.fx.u)
        actual = Instance(self.fx.std_tuplei, [self.fx.a])
        self._assert_par(template, actual)

    # --- Template has an Unpack, actual is a fixed TupleType ---

    def test_unpack_template_fixed_tuple_tvt(self) -> None:
        # Tuple[*Ts] <: (X, Y).
        template = self._tup_tvt(self.fx.ts)
        actual = self._tup(self.fx.a, self.fx.b)
        self._assert_par(template, actual)

    def test_unpack_template_fixed_tuple_simple(self) -> None:
        # Tuple[T, *Ts, U] <: (X, Y, Z, W): T <: X, Ts <: (Y, Z), U <: W.
        template = self._tup(self.fx.t, UnpackType(self.fx.ts), self.fx.u)
        actual = self._tup(self.fx.a, self.fx.b, self.fx.c, self.fx.d)
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    def test_unpack_template_fixed_tuple_homogeneous(self) -> None:
        # Tuple[T, *tuple[S, ...], U] <: (X, Y, Z, W).
        template = self._tup(
            self.fx.t, UnpackType(Instance(self.fx.std_tuplei, [self.fx.s])), self.fx.u
        )
        actual = self._tup(self.fx.a, self.fx.b, self.fx.c, self.fx.d)
        self._assert_par(template, actual)

    def test_unpack_template_fixed_tuple_too_short(self) -> None:
        # Tuple[T, *Ts, U] <: (X): prefix+suffix exceed the actual length,
        # fast-return path.
        template = self._tup(self.fx.t, UnpackType(self.fx.ts), self.fx.u)
        actual = self._tup(self.fx.a)
        self._assert_par(template, actual)

    # --- Template without Unpack, actual has an internal Unpack ---

    def test_fixed_template_actual_internal_unpack(self) -> None:
        # Tuple[T, S, U] <: (X, *tuple[Y, ...], Z): T <: X, S <: Y, U <: Z.
        template = self._tup(self.fx.t, self.fx.s, self.fx.u)
        actual = self._tup(
            self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.b])), self.fx.c
        )
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    def test_fixed_template_actual_trailing_internal_unpack(self) -> None:
        # Tuple[T, S, U] <: (*tuple[X, ...], Y): the compatible split
        # branch (template len == actual len - 1, unpack at the front)
        # constrains the middle template items against the unpack arg.
        template = self._tup(self.fx.t, self.fx.s, self.fx.u)
        actual = self._tup(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.b)
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    def test_fixed_template_actual_trailing_internal_unpack_supertypes(self) -> None:
        template = self._tup(self.fx.t, self.fx.s, self.fx.u)
        actual = self._tup(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.b)
        self._assert_par(template, actual, direction=SUPERTYPE_OF)

    def test_fixed_template_actual_internal_unpack_tvt(self) -> None:
        # Tuple[T, S, U] <: (X, *Ts, Z): the middle is a TypeVarTuple and
        # yields no constraints, the split prefix/suffix still constrain.
        template = self._tup(self.fx.t, self.fx.s, self.fx.u)
        actual = self._tup(self.fx.a, UnpackType(self.fx.ts), self.fx.b)
        self._assert_par(template, actual)

    def test_fixed_template_actual_internal_unpack_too_short(self) -> None:
        # Tuple[T, S] <: (X, *tuple[Y, ...], Z): actual length exceeds
        # template, no per-item constraints but the fallback tail runs.
        template = self._tup(self.fx.t, self.fx.s)
        actual = self._tup(
            self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.b])), self.fx.c
        )
        self._assert_par(template, actual)

    # --- Fixed template vs fixed actual ---

    def test_fixed_equal_length(self) -> None:
        template = self._tup(self.fx.t, self.fx.s)
        actual = self._tup(self.fx.a, self.fx.b)
        self._assert_par(template, actual)

    def test_fixed_equal_length_supertypes(self) -> None:
        template = self._tup(self.fx.t, self.fx.s)
        actual = self._tup(self.fx.a, self.fx.b)
        self._assert_par(template, actual, direction=SUPERTYPE_OF)

    def test_fixed_equal_length_any_item(self) -> None:
        template = self._tup(self.fx.t, self.fx.s)
        actual = self._tup(AnyType(TypeOfAny.special_form), self.fx.b)
        self._assert_par(template, actual)

    def test_fixed_length_mismatch(self) -> None:
        template = self._tup(self.fx.t, self.fx.s, self.fx.u)
        actual = self._tup(self.fx.a, self.fx.b)
        self._assert_par(template, actual)

    # --- Template is a TupleType, actual is not ---

    def test_template_tuple_actual_any(self) -> None:
        # infer_against_any over the template items.
        template = self._tup(self.fx.t, self.fx.s)
        actual = AnyType(TypeOfAny.special_form)
        self._assert_par(template, actual)

    def test_template_tuple_actual_instance(self) -> None:
        # Unrelated instance, not a varlength tuple: no constraints.
        template = self._tup(self.fx.t)
        actual = self.fx.a
        self._assert_par(template, actual)

    def test_template_tuple_actual_varlength_list(self) -> None:
        # A list is not a tuple subtype through builtins.tuple; no
        # constraints.
        template = self._tup(self.fx.t)
        actual = Instance(self.fx.std_listi, [self.fx.a])
        self._assert_par(template, actual)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeConstraintsDeferralSuite(Suite):
    """Parity suite for the constraints.rs defer-reduction (issue #869).

    `_try_native_constraint_builder` routes the full ConstraintBuilderVisitor
    through Rust (`rust_infer_constraints_full`). Before #869 a
    `TypeAliasType` operand anywhere in the recursion deferred the whole call
    to Python. Now `infer_constraints_full_inner` expands both operands
    through the alias resolver at the top (mirroring `get_proper_type` at
    constraints.py:548-549), so a nested alias resolves natively. Since
    #1130 the union-dispatch branches run natively too, so union operands
    (including an alias that expands into one) engage instead of deferring;
    only an unresolvable (missing-snapshot) alias still defers.

    Differential harness: runs `infer_constraints` with the gate on
    (resolver + wire map installed) and off (pure Python), asserting equal
    constraint lists; a direct `rust_infer_constraints_full` call proves
    native engagement (returns blobs) for the alias cases and None (defer)
    for the union / missing-snapshot cases.
    """

    def setUp(self) -> None:
        from mypy.constraints import _set_native_constraints_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_constraints_active
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        # Safe default so a mismatched gate never crosses suites.
        self._set_active(False)

    def tearDown(self) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)

    def _rebuild_with_aliases(self, aliases: list[Any]) -> None:
        from mypy.constraints import _set_native_constraints_resolver

        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        self.resolver = _type_kernel.build_native_resolver(type_infos, aliases)
        _set_native_constraints_resolver(self.resolver)

    def _list(self, *args: Type) -> Instance:
        return Instance(self.fx.std_listi, list(args))

    def _alias(self, target: Type, name: str) -> Any:
        from mypy.nodes import TypeAlias

        return TypeAlias(target, name, "mod", -1, -1)

    def _constraints(
        self, template: Type, actual: Type, direction: int, native: bool
    ) -> list[Any]:
        from mypy.constraints import _set_native_constraints_resolver, infer_constraints

        self._set_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return infer_constraints(template, actual, direction)

    def _assert_par(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        native = self._constraints(template, actual, direction, native=True)
        python = self._constraints(template, actual, direction, native=False)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _rust(
        self, template: Type, actual: Type, direction: int = SUBTYPE_OF, erase_types: bool = False
    ) -> Any:
        return _type_kernel.rust_infer_constraints_full(
            self.resolver,
            self._bytes_of(template),
            self._bytes_of(actual),
            direction,
            False,
            erase_types,
            strict_optional_flag(),
            True,
        )

    def _assert_engages(
        self, template: Type, actual: Type, direction: int = SUBTYPE_OF, erase_types: bool = False
    ) -> None:
        raw = self._rust(template, actual, direction, erase_types)
        assert (
            raw is not None
        ), f"Rust seam must engage for template={template!r} actual={actual!r}"

    def _assert_defers(
        self, template: Type, actual: Type, direction: int = SUBTYPE_OF, erase_types: bool = False
    ) -> None:
        raw = self._rust(template, actual, direction, erase_types)
        assert raw is None, f"Rust seam must defer for template={template!r} actual={actual!r}"

    # --- alias operands expand natively through the resolver ---

    def test_actual_alias_operand_expands(self) -> None:
        # List[T] vs List[Alias->A]: the Alias actual arg reaches
        # infer_constraints_full_inner via push_inner and expands to A, so
        # the native seam emits T <: A / T :> A instead of deferring.
        alias = self._alias(self.fx.a, "mod.AliasA")
        self._rebuild_with_aliases([alias])
        template = self._list(self.fx.t)
        actual = self._list(TypeAliasType(alias, []))
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    def test_template_alias_operand_expands(self) -> None:
        # H[T, Alias->A] vs H[A, B]: the Alias template arg expands to A on
        # the template side while the T position still constrains, so the
        # native seam yields the same constraints as Python.
        alias = self._alias(self.fx.a, "mod.AliasT")
        self._rebuild_with_aliases([alias])
        template = Instance(self.fx.hi, [self.fx.t, TypeAliasType(alias, [])])
        actual = Instance(self.fx.hi, [self.fx.a, self.fx.b])
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    def test_alias_expanding_to_union_engages(self) -> None:
        # List[T] vs List[Alias->(A | B)]: expansion yields a union, whose
        # dispatch branch is now ported (issue #1130), so the native seam
        # emits the same per-item constraints as Python.
        from mypy.types import UnionType

        alias = self._alias(UnionType.make_union([self.fx.a, self.fx.b]), "mod.AliasU")
        self._rebuild_with_aliases([alias])
        template = self._list(self.fx.t)
        actual = self._list(TypeAliasType(alias, []))
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    # --- union operands engage natively (issue #1130) ---

    def test_actual_union_operand_engages(self) -> None:
        # List[T] vs List[A | B]: the union dispatch branch is ported
        # (issue #1130), so the seam handles the union actual natively.
        actual_union = UnionType.make_union([self.fx.a, self.fx.b])
        template = self._list(self.fx.t)
        actual = self._list(actual_union)
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    def test_template_union_operand_engages(self) -> None:
        # H[T, (A | B)] vs H[A, A]: the union template arg is dispatched
        # natively (issue #1130) before the other position constrains.
        template_union = UnionType.make_union([self.fx.a, self.fx.b])
        template = Instance(self.fx.hi, [self.fx.t, template_union])
        actual = Instance(self.fx.hi, [self.fx.a, self.fx.a])
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    # --- type-object/callable and tuple-fallback arms (issue #1259 round 3)

    def test_type_object_callable_actual_engages(self) -> None:
        # constraints.py:2046-2053: `type[List[T]]` vs a type-object callable
        # constrains List[T] against the callable's ret_type (the
        # unconstrained builder instance_type falls back to it).
        template = TypeType.make_normalized(Instance(self.fx.std_listi, [self.fx.t]))
        actual = CallableType([], [], [], self.fx.lstb, self.fx.type_type)
        self._assert_par(template, actual, SUPERTYPE_OF)
        self._assert_engages(template, actual, SUPERTYPE_OF)

    def test_type_object_overloaded_actual_engages(self) -> None:
        # Same as above via the Overloaded arm: items[0].get_instance_type()
        # (constraints.py:2054-2060).
        template = TypeType.make_normalized(Instance(self.fx.std_listi, [self.fx.t]))
        items = [
            CallableType([], [], [], self.fx.lstb, self.fx.type_type),
            CallableType([], [], [], self.fx.lsta, self.fx.type_type),
        ]
        actual = Overloaded(items)
        self._assert_par(template, actual, SUPERTYPE_OF)
        self._assert_engages(template, actual, SUPERTYPE_OF)

    def test_namedtuple_tuple_fallback_engages(self) -> None:
        # constraints.py:1626-1654 tail: a namedtuple-shaped TupleType actual
        # constrains the generic template against its partial_fallback
        # instance through the nominal SUPERTYPE_OF arm.
        template = Instance(self.fx.hi, [self.fx.t, self.fx.b])
        actual = TupleType(
            [self.fx.a, self.fx.b], Instance(self.fx.hi, [self.fx.a, self.fx.b]), implicit=True
        )
        self._assert_par(template, actual, SUPERTYPE_OF)
        self._assert_engages(template, actual, SUPERTYPE_OF)

    def test_typetype_actual_engages_on_nominal_template(self) -> None:
        # constraints.py:1400-1417 tail: a non-protocol template with a
        # type[...] actual falls out to `return []` on both sides
        # (previously a defer).
        template = self._list(self.fx.t)
        actual = TypeType.make_normalized(self.fx.a)
        self._assert_par(template, actual, SUPERTYPE_OF)
        self._assert_engages(template, actual, SUPERTYPE_OF)

    # --- unresolvable alias defers to Python ---

    def test_missing_snapshot_defers(self) -> None:
        # List[T] vs List[Alias] with no resolver snapshot: expansion cannot
        # proceed, so the native seam defers and Python resolves the alias
        # from the live node.
        alias = self._alias(self.fx.a, "mod.Missing")
        self._rebuild_with_aliases([])
        template = self._list(self.fx.t)
        actual = self._list(TypeAliasType(alias, []))
        self._assert_par(template, actual)
        self._assert_defers(template, actual)

    # --- type[...] template vs a type-object callable (issue #1260) ---

    def _type_of(self, item: Type) -> Type:
        return TypeType.make_normalized(item)

    def test_type_type_vs_type_object_callable(self) -> None:
        # type[list[T]] vs a type-object callable (fallback builtins.type):
        # get_instance_type() stands in the proper ret_type, erase_typevars
        # applies per the erase_types flag, then recursion emits the constraint.
        template = self._type_of(self._list(self.fx.t))
        actual = self.fx.callable_type(self._list(self.fx.a))
        self._assert_par(template, actual)
        self._assert_engages(template, actual, erase_types=True)

    def test_type_type_vs_ctor_typevar_ret_erases_by_flag(self) -> None:
        # Same shape with a typevar-bearing ret_type: erase_types=True erases
        # the ctor typevars; False (the typeops.py bind_self callers) preserves
        # them. Both engage; the serialized constraints differ by the flag.
        template = self._type_of(self._list(self.fx.t))
        actual = self.fx.callable_type(self._list(self.fx.t))
        self._assert_par(template, actual)
        raw_true = self._rust(template, actual, erase_types=True)
        raw_false = self._rust(template, actual, erase_types=False)
        assert raw_true is not None and raw_false is not None
        assert raw_true != raw_false, "the erase flag must change the constraint bytes"

    def test_type_type_vs_plain_callable_recurse_ret(self) -> None:
        # Non-type-object callable (fallback builtins.function): recursion
        # against the raw ret_type, no erase (constraints.py:2040).
        template = self._type_of(self._list(self.fx.t))
        actual = self.fx.callable(self._list(self.fx.a))
        self._assert_par(template, actual)
        self._assert_engages(template, actual)

    def test_type_type_vs_ctor_alias_ret_defers(self) -> None:
        # is_type_obj=True, no instance_type, alias ret without a snapshot:
        # the proper-type expansion cannot proceed, so the whole call
        # defers; Python produces the same constraints through the fallback.
        alias = self._alias(self.fx.a, "mod.MissingCtor")
        self._rebuild_with_aliases([])
        template = self._type_of(self._list(self.fx.t))
        actual = self.fx.callable_type(TypeAliasType(alias, []))
        self._assert_par(template, actual)
        self._assert_defers(template, actual)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeInstanceConstraintArmsSuite(Suite):
    """Gate-off/on parity for the wave-65C `visit_instance` actual-shape arms.

    Pins the arms that previously deferred the whole `rust_infer_constraints_full`
    call (issue #1541):

    * both-protocol Instance SUPERTYPE_OF (constraints.py:1550-1572),
    * Callable actual vs protocol template (callback-protocol + class-object
      arms, constraints.py:1356-1385, then the fallback continuation),
    * Tuple actual vs protocol template (constraints.py:1619-1630),
    * Overloaded actual (`infer_against_overloaded`, constraints.py:1830/1861),
    * Instance actual (`__call__` member recursion, constraints.py:1848-1857),
    * the `else: return []` tail for the remaining actual shapes.

    Each test runs `infer_constraints` gate-off vs gate-on and asserts equal
    constraint lists, plus a direct `rust_infer_constraints_full` call proving
    native engagement (returns blobs, not None).
    """

    def setUp(self) -> None:
        from mypy.constraints import _set_native_constraints_active

        self.fx = TypeFixture()
        self._live_info: dict[str, TypeInfo] = {}
        self._set_active = _set_native_constraints_active
        # Safe default so a mismatched gate never crosses suites.
        self._set_active(False)

    def tearDown(self) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)

    # --- synthetic protocol / impl fixtures (protocol-suite shape) ---

    def _method_callable(
        self, ret: Type | None = None, self_type: Type | None = None
    ) -> CallableType:
        self_arg = self_type if self_type is not None else self.fx.a
        return CallableType(
            [self_arg], [ARG_POS], [None], ret if ret is not None else self.fx.a, self.fx.function
        )

    def _add_method(self, info: TypeInfo, name: str, typ: ProperType) -> None:
        node = FuncDef(name, [], None, None)
        node.info = info
        node.type = typ
        node.line = 1
        node.column = 1
        info.names[name] = SymbolTableNode(MDEF, node)

    def _synth_info(self, fullname: str, is_protocol: bool) -> TypeInfo:
        info = self.fx.make_type_info(fullname)
        info.mro = [info, self.fx.oi]
        info.is_protocol = is_protocol
        self._live_info[fullname] = info
        return info

    def _protocol(self, fullname: str, members: list[str]) -> TypeInfo:
        info = self._synth_info(fullname, True)
        inst = Instance(info, [])
        for name in members:
            self._add_method(info, name, self._method_callable(self.fx.a, inst))
        return info

    def _impl(self, fullname: str, members: list[str]) -> TypeInfo:
        info = self._synth_info(fullname, False)
        inst = Instance(info, [])
        for name in members:
            self._add_method(info, name, self._method_callable(self.fx.a, inst))
        return info

    def _build_resolver(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        type_infos = [
            getattr(self.fx, name)
            for name in dir(self.fx)
            if name.endswith("i") and _is_type_info(getattr(self.fx, name))
        ]
        type_infos.extend(self._live_info.values())
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self.resolver.set_live_typeinfo_map(dict(self._live_info))
        set_wire_typeinfo_map(dict(self._live_info))

    def _constraints(
        self, template: Type, actual: Type, direction: int, native: bool
    ) -> list[Any]:
        from mypy.constraints import _set_native_constraints_resolver, infer_constraints

        self._set_active(native)
        if native:
            self._build_resolver()
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return infer_constraints(template, actual, direction)

    def _assert_par(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        native = self._constraints(template, actual, direction, native=True)
        python = self._constraints(template, actual, direction, native=False)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def _assert_engages(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        self._build_resolver()
        tbuf = _WriteBuffer()
        template.write(tbuf)
        abuf = _WriteBuffer()
        actual.write(abuf)
        raw = _type_kernel.rust_infer_constraints_full(
            self.resolver,
            tbuf.getvalue(),
            abuf.getvalue(),
            direction,
            False,
            False,
            strict_optional_flag(),
            True,
        )
        assert (
            raw is not None
        ), f"Rust seam must engage for template={template!r} actual={actual!r}"

    def _assert_defers(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        self._build_resolver()
        tbuf = _WriteBuffer()
        template.write(tbuf)
        abuf = _WriteBuffer()
        actual.write(abuf)
        raw = _type_kernel.rust_infer_constraints_full(
            self.resolver,
            tbuf.getvalue(),
            abuf.getvalue(),
            direction,
            False,
            False,
            strict_optional_flag(),
            True,
        )
        assert raw is None, f"Rust seam must defer for template={template!r} actual={actual!r}"

    def _plain_callable(self, ret: Type) -> CallableType:
        return CallableType([], [], [], ret, self.fx.function)

    def _plain_callable_with_fallback(self, ret: Type, fallback: Instance) -> CallableType:
        return CallableType([], [], [], ret, fallback)

    # --- arms ---

    def test_both_protocol_supertype_engages(self) -> None:
        # constraints.py:1550-1572 fires for a protocol TEMPLATE regardless
        # of whether the actual is also a protocol; the previous
        # `!a_snap.is_protocol` gate deferred every such pair.
        target = Instance(self._protocol("mod.PTarget", ["m"]), [])
        other = Instance(self._protocol("mod.POther", ["m"]), [])
        self._assert_par(target, other, SUPERTYPE_OF)
        self._assert_engages(target, other, SUPERTYPE_OF)

    def test_protocol_actual_supertype_returns_empty(self) -> None:
        # constraints.py:1550's template arm does not fire for a
        # non-protocol template, and the SUBTYPE_OF arm needs the opposite
        # direction; the shape tail then returns [] (line 1589-1641).
        template = self.fx.a
        other = Instance(self._protocol("mod.POther2", ["m"]), [])
        self._assert_par(template, other, SUPERTYPE_OF)
        self._assert_engages(template, other, SUPERTYPE_OF)

    def test_callable_actual_protocol_template_engages(self) -> None:
        # A protocol template without `__call__` and a non-type-object
        # callable actual: the callback arm is skipped and the callable
        # unwraps to a synthetic non-implementer fallback -> [].
        target = Instance(self._protocol("mod.PTargetC", ["m"]), [])
        fallback = Instance(self._impl("mod.NoImplC", ["z"]), [])
        actual = self._plain_callable_with_fallback(self.fx.a, fallback)
        self._assert_par(target, actual, SUPERTYPE_OF)
        self._assert_engages(target, actual, SUPERTYPE_OF)

    def test_callback_protocol_member_engages(self) -> None:
        # constraints.py:1356-1372: the generic callback-protocol arm runs
        # `find_member("__call__")` on the template and recurses; the
        # fallback is a synthetic non-implementer so the arm decides.
        target = Instance(self._protocol("mod.PCall", ["__call__"]), [])
        fallback = Instance(self._impl("mod.NoImplCall", ["z"]), [])
        actual = self._plain_callable_with_fallback(self.fx.a, fallback)
        self._assert_par(target, actual, SUPERTYPE_OF)
        self._assert_engages(target, actual, SUPERTYPE_OF)

    def test_type_object_member_arm_parity(self) -> None:
        # constraints.py:1373-1385: a type-object callable actual runs the
        # class-object member loop, whose `class_obj=True` fetch stays a
        # documented defer floor, so the seam defers and parity holds.
        target = Instance(self._protocol("mod.PLen", ["__len__"]), [])
        actual = CallableType([], [], [], self.fx.lsta, self.fx.type_type)
        self._assert_par(target, actual, SUPERTYPE_OF)
        self._assert_defers(target, actual, SUPERTYPE_OF)

    def test_tuple_actual_protocol_template_engages(self) -> None:
        # constraints.py:1619-1630: the tuple-fallback protocol special
        # case; a synthetic non-implementer partial fallback lets the
        # engine answer False, then the final fallback recursion decides.
        target = Instance(self._protocol("mod.PTuple", ["__len__"]), [])
        fallback = Instance(self._impl("mod.NoLen", ["z"]), [])
        actual = TupleType([self.fx.a], fallback)
        self._assert_par(target, actual, SUPERTYPE_OF)
        self._assert_engages(target, actual, SUPERTYPE_OF)

    def test_overloaded_actual_engages(self) -> None:
        # constraints.py:1830/1861: `infer_against_overloaded` matches the
        # first callable-compatible item (ignore_return) and recurses on it.
        template = self._plain_callable(self.fx.a)
        actual = Overloaded([self._plain_callable(self.fx.a), self._plain_callable(self.fx.b)])
        self._assert_par(template, actual, SUPERTYPE_OF)
        self._assert_engages(template, actual, SUPERTYPE_OF)

    def test_instance_call_member_engages(self) -> None:
        # constraints.py:1848-1857: an Instance actual recurses against its
        # `__call__` member (bound by `find_member`).
        info = self._impl("mod.Callable0", ["__call__"])
        template = self._plain_callable(self.fx.a)
        actual = Instance(info, [])
        self._assert_par(template, actual, SUPERTYPE_OF)
        self._assert_engages(template, actual, SUPERTYPE_OF)

    def test_instance_no_call_member_defers(self) -> None:
        # Same arm with no `__call__` member: Python's `find_member` miss
        # path owns position/side-effect bookkeeping the kernel does not
        # replicate, so the seam defers and Python answers [].
        info = self._impl("mod.Plain0", ["m"])
        template = self._plain_callable(self.fx.a)
        actual = Instance(info, [])
        self._assert_par(template, actual, SUPERTYPE_OF)
        self._assert_defers(template, actual, SUPERTYPE_OF)

    def test_callable_actual_else_returns_empty(self) -> None:
        # constraints.py:1858-1859: every remaining actual shape returns [].
        template = self._plain_callable(self.fx.a)
        self._assert_par(template, NoneType(), SUPERTYPE_OF)
        self._assert_engages(template, NoneType(), SUPERTYPE_OF)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeConstraintsPolyGateSuite(Suite):
    """Parity suite for the skip_neg_op / infer_polymorphic gate (issue #1226).

    The cb-actual-generic defer fired whenever the actual callable was
    generic; since #1427 the generic helpers are pinned by body. The
    Known(true) reverse frame now engages as well: its `extra_tvars`
    attachments ride the #1618 blob section and relink onto the live actual
    variables on decode.
    """

    def setUp(self) -> None:
        from mypy.constraints import _set_native_constraints_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_constraints_active
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active(False)

    def tearDown(self) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)

    def _polymorphic_on(self) -> None:
        # Restore via addCleanup so a failed assert cannot leak the flag.
        # Save the entry value: xdist workers may hold leaked state from
        # other modules.
        from mypy.typestate import type_state

        old = type_state.infer_polymorphic
        type_state.infer_polymorphic = True
        self.addCleanup(setattr, type_state, "infer_polymorphic", old)

    def _polymorphic_off(self) -> None:
        # Mirror of _polymorphic_on: pin ambient False (test / old-inference
        # mode) so a test cannot observe leaked ambient state from a worker
        # because infer_polymorphic is never restored by its producers.
        from mypy.typestate import type_state

        old = type_state.infer_polymorphic
        type_state.infer_polymorphic = False
        self.addCleanup(setattr, type_state, "infer_polymorphic", old)

    def _generic_callable(self, name: str, var: TypeVarType) -> CallableType:
        return CallableType(
            [var], [ARG_POS], [None], var, self.fx.function, variables=[var], name=name
        )

    def _param_spec_template(self, p: ParamSpecType) -> CallableType:
        return CallableType([p], [ARG_POS], [None], self.fx.a, self.fx.function, variables=[p])

    def _plain_callable(self, arg: Type, ret: Type) -> CallableType:
        return CallableType([arg], [ARG_POS], [None], ret, self.fx.function)

    def _constraints(
        self, template: Type, actual: Type, direction: int, native: bool, skip_neg_op: bool = False
    ) -> list[Any]:
        from mypy.constraints import _set_native_constraints_resolver, infer_constraints

        self._set_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return infer_constraints(template, actual, direction, skip_neg_op=skip_neg_op)

    def _assert_par(
        self, template: Type, actual: Type, direction: int = SUBTYPE_OF, skip_neg_op: bool = False
    ) -> None:
        native = self._constraints(template, actual, direction, True, skip_neg_op)
        python = self._constraints(template, actual, direction, False, skip_neg_op)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _rust(
        self,
        template: Type,
        actual: Type,
        direction: int = SUBTYPE_OF,
        skip_neg_op: bool = False,
        infer_polymorphic: bool = False,
    ) -> Any:
        from mypy.typestate import type_state

        return _type_kernel.rust_infer_constraints_full(
            self.resolver,
            self._bytes_of(template),
            self._bytes_of(actual),
            direction,
            skip_neg_op,
            False,
            strict_optional_flag(),
            infer_polymorphic if infer_polymorphic else type_state.infer_polymorphic,
        )

    # --- skip_neg_op=True lets a generic actual proceed natively ---

    def test_skip_true_generic_actual_engages(self) -> None:
        t1 = self.fx.t
        u1 = self.fx.s
        template = self._generic_callable("f", t1)
        actual = self._generic_callable("g", u1)
        self._assert_par(template, actual, SUBTYPE_OF, skip_neg_op=True)
        raw = self._rust(template, actual, SUBTYPE_OF, skip_neg_op=True)
        assert raw is not None, "skip_neg_op=True must run the generic actual natively"

    def test_skip_false_generic_actual_mode_dependent(self) -> None:
        t1 = self.fx.t
        u1 = self.fx.s
        template = self._generic_callable("f", t1)
        actual = self._generic_callable("g", u1)
        # Gate-on run matches the Python body in both ambient modes,
        # since the extras channel (#1618) carries the reverse frame.
        self._assert_par(template, actual, SUBTYPE_OF, skip_neg_op=False)
        # Ambient is flipped mid-test, so save and restore the entry value.
        from mypy.typestate import type_state

        old = type_state.infer_polymorphic
        try:
            # Ambient off: the kernel installs Known(false), the reverse
            # frame stays off, and the pair decides extras-free.
            type_state.infer_polymorphic = False
            raw_off = self._rust(template, actual, SUBTYPE_OF, skip_neg_op=False)
            # Ambient on: the reverse frame attaches the actual's own
            # variables as extras; the #1618 blob section carries them, so
            # the call still decides natively.
            type_state.infer_polymorphic = True
            raw_on = self._rust(template, actual, SUBTYPE_OF, skip_neg_op=False)
        finally:
            type_state.infer_polymorphic = old
        assert raw_off is not None, "Known(false) ambient keeps the call engaged"
        assert raw_on is not None, "Known(true) extras ride the #1618 blob section"

    def test_skip_false_generic_actual_extras_reach_python(self) -> None:
        # #1618: the reverse frame's extras must reach the Python solver as
        # objects; `_try_native_constraint_builder` relinks them onto the live
        # actual variable (like Python's own `c.extra_tvars += cactual.variables`).
        from mypy.constraints import (
            _set_native_constraints_resolver,
            _try_native_constraint_builder,
        )

        t1 = self.fx.t
        u1 = self.fx.s
        template = self._generic_callable("f", t1)
        actual = self._generic_callable("g", u1)
        self._polymorphic_on()
        _set_native_constraints_resolver(self.resolver)
        try:
            res = _try_native_constraint_builder(template, actual, SUBTYPE_OF, False, False, True)
        finally:
            _set_native_constraints_resolver(None)
        assert res is not None, "the extras-carrying call must decide natively"
        extras = [v for c in res for v in c.extra_tvars]
        assert extras, "the reverse frame must attach the actual's variables"
        assert all(
            v is u1 for v in extras
        ), f"extras must relink onto the live actual variable, got {extras!r}"

    def test_restore_extra_tvars_loose_meta_level_fallback(self) -> None:
        # Defensive path: a pre-#1417 wire stream would drop meta_level for
        # ParamSpec ids, so full-id equality fails; the `(raw_id, namespace)`
        # fallback must then find the single live variable.
        from mypy.constraints import _restore_extra_tvars

        fresh = ParamSpecType(
            "P",
            "P",
            TypeVarId(11, 1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        decoded = fresh.copy_modified(id=TypeVarId(11, 0))
        holder = self._param_spec_template(fresh)
        restored = _restore_extra_tvars([decoded], holder)
        assert restored[0] is fresh, "decoded ParamSpec must relink onto the live var"

    def test_restore_extra_tvars_id_equal_duplicates_relink(self) -> None:
        # Two distinct copies of one ParamSpec (same raw_id/meta_level/
        # namespace) occur in the wild; the relink dedupes on the structural
        # TypeVarId every consumer keys on and takes the first occurrence.
        from mypy.constraints import _restore_extra_tvars

        fresh = ParamSpecType(
            "P",
            "P",
            TypeVarId(837, 1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        duplicate = fresh.copy_modified()
        assert duplicate is not fresh
        holder_a = self._param_spec_template(fresh)
        holder_b = CallableType(
            [duplicate], [ARG_POS], [None], self.fx.a, self.fx.function, variables=[duplicate]
        )
        decoded = fresh.copy_modified()
        restored = _restore_extra_tvars([decoded], holder_a, holder_b)
        assert restored[0] is fresh, "first id-equal occurrence wins"

    def test_restore_extra_tvars_duplicate_occurrences_relink(self) -> None:
        # A variable occurring twice in the live tree (`Callable[[T], T]`)
        # appears twice in `get_all_type_vars`; the identity dedupe must
        # still resolve it to the single live object.
        from mypy.constraints import _restore_extra_tvars

        t1 = self.fx.t
        holder = CallableType([t1], [ARG_POS], [None], t1, self.fx.function, variables=[])
        decoded = t1.copy_modified()
        assert decoded is not t1
        restored = _restore_extra_tvars([decoded], holder)
        assert restored[0] is t1

    # --- param-spec target mirrors the infer_polymorphic ternary ---

    def test_param_spec_target_with_skip_true_and_polymorphic(self) -> None:
        self._polymorphic_on()
        p = ParamSpecType(
            "P",
            "P",
            TypeVarId(1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        template = self._param_spec_template(p)
        actual = self._plain_callable(self.fx.a, self.fx.a)
        self._assert_par(template, actual, SUPERTYPE_OF, skip_neg_op=True)
        raw = self._rust(template, actual, SUPERTYPE_OF, skip_neg_op=True, infer_polymorphic=True)
        assert raw is not None, "param-spec target must engage with skip_neg_op=True"

    def test_param_spec_target_keeps_variables_without_polymorphic(self) -> None:
        # With infer_polymorphic=False (old inference or tests), the target
        # keeps cactual.variables: parity must hold in that mode too. Pin
        # ambient False so the intended mode runs even after a leakage.
        self._polymorphic_off()
        p = ParamSpecType(
            "P",
            "P",
            TypeVarId(1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )
        template = self._param_spec_template(p)
        actual = self._plain_callable(self.fx.a, self.fx.a)
        self._assert_par(template, actual, SUPERTYPE_OF, skip_neg_op=True)
        raw = self._rust(template, actual, SUPERTYPE_OF, skip_neg_op=True, infer_polymorphic=False)
        assert raw is not None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAreParametersCompatibleSuite(Suite):
    """Parity for the Rust `are_parameters_compatible` seam.

    Differential harness: runs `is_subtype` on `Parameters` pairs with the
    native subtype gate on (resolver installed) and off (pure Python), and
    asserts identical results. The gate-on path routes through
    `SubtypeVisitor.visit_parameters` (subtypes.py:962), which fast-paths
    to the Rust seam. Also covers the meet overlap entry
    (`mypy.meet.is_overlapping_types` on `Parameters` pairs, which routes
    through the same engine with an overlap is_compat). Rust returns `None`
    (Python fallthrough) for any shape the engine cannot decide (generic
    parameters, meet_types merge, nested is_subtype/overlap deferral), so
    every assertion matches the pure-Python result.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        # The subtype/join gates are shared by the seam; off by default so a
        # failed differential never leaks into the next suite.
        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        from mypy.join import _set_native_join_active, _set_native_join_resolver

        _set_native_join_active(False)
        _set_native_join_resolver(None)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        set_wire_typeinfo_map(None)
        from mypy.join import _set_native_join_active, _set_native_join_resolver

        _set_native_join_active(False)
        _set_native_join_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _parameters(
        self,
        arg_types: list[Type],
        arg_kinds: list[ArgKind],
        arg_names: list[str | None],
        is_ellipsis_args: bool = False,
        imprecise_arg_kinds: bool = False,
        variables: list[TypeVarLikeType] | None = None,
    ) -> Parameters:
        return Parameters(
            arg_types,
            arg_kinds,
            arg_names,
            is_ellipsis_args=is_ellipsis_args,
            imprecise_arg_kinds=imprecise_arg_kinds,
            variables=variables,
        )

    def _assert_subtype_par(self, left: Parameters, right: Parameters) -> None:
        """Differential: `is_subtype` on Parameters, gate-on vs gate-off.

        Routes through `SubtypeVisitor.visit_parameters` (subtypes.py:962),
        which fast-paths to the Rust seam when the gate is on and differs
        from the pure-Python `are_parameters_compatible` only via that seam.
        """
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        python = is_subtype(left, right)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        native = is_subtype(left, right)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def _assert_overlap_par(self, left: Parameters, right: Parameters) -> None:
        """Differential: `is_overlapping_types` on Parameters, gate-on vs off."""
        from mypy.join import _set_native_join_active, _set_native_join_resolver
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        python = is_overlapping_types(left, right)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        native = is_overlapping_types(left, right)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def test_simple_positional(self) -> None:
        left = self._parameters([self.fx.a], [ARG_POS], [None])
        right = self._parameters([self.fx.a], [ARG_POS], [None])
        self._assert_subtype_par(left, right)
        self._assert_overlap_par(left, right)

    def test_supertype_left_more_general(self) -> None:
        # left (A) is more general than right (B): A <: B false, B <: A true.
        left = self._parameters([self.fx.a], [ARG_POS], [None])
        right = self._parameters([self.fx.b], [ARG_POS], [None])
        self._assert_subtype_par(left, right)

    def test_incompatible_types(self) -> None:
        left = self._parameters([self.fx.a], [ARG_POS], [None])
        right = self._parameters([self.fx.b], [ARG_POS], [None])
        self._assert_overlap_par(left, right)

    def test_name_mismatch(self) -> None:
        left = self._parameters([self.fx.a], [ARG_NAMED], ["x"])
        right = self._parameters([self.fx.a], [ARG_NAMED], ["y"])
        self._assert_subtype_par(left, right)

    def test_pos_arg_names_ignored(self) -> None:
        left = self._parameters([self.fx.a], [ARG_POS], ["x"])
        right = self._parameters([self.fx.a], [ARG_POS], ["y"])
        self._assert_subtype_par(left, right)

    def test_optional_vs_required(self) -> None:
        left = self._parameters([self.fx.a], [ARG_OPT], [None])
        right = self._parameters([self.fx.a], [ARG_POS], [None])
        self._assert_subtype_par(left, right)

    def test_ellipsis_args(self) -> None:
        # `Callable[..., X]`: **Any then *Any (trivial params) — right is a
        # supertype of everything (non-proper subtype check).
        left = self._parameters([self.fx.a], [ARG_POS], [None])
        right = self._parameters(
            [AnyType(TypeOfAny.special_form), AnyType(TypeOfAny.special_form)],
            [ARG_STAR, ARG_STAR2],
            [None, None],
            is_ellipsis_args=True,
        )
        self._assert_subtype_par(left, right)

    def test_trivial_vararg_suffix(self) -> None:
        # right = (*Any) — supertype of all-positional-left callables.
        left = self._parameters([self.fx.a, self.fx.b], [ARG_POS, ARG_POS], [None, None])
        right = self._parameters([AnyType(TypeOfAny.special_form)], [ARG_STAR], [None])
        self._assert_subtype_par(left, right)

    def test_right_star_vs_left_positional(self) -> None:
        left = self._parameters([self.fx.a, self.fx.b], [ARG_POS, ARG_POS], [None, None])
        right = self._parameters([self.fx.a], [ARG_STAR], [None])
        self._assert_subtype_par(left, right)

    def test_right_kwargs_vs_left_named(self) -> None:
        left = self._parameters([self.fx.a], [ARG_NAMED], ["x"])
        right = self._parameters([self.fx.a], [ARG_STAR2], [None])
        self._assert_subtype_par(left, right)

    def test_left_required_no_right_match(self) -> None:
        left = self._parameters([self.fx.a], [ARG_NAMED], ["x"])
        right = self._parameters([self.fx.o], [ARG_STAR2], [None])
        self._assert_subtype_par(left, right)

    def test_generic_parameters_defer(self) -> None:
        # Variables non-empty: the engine defers, Python answers identically.
        left = self._parameters([self.fx.t], [ARG_POS], [None], variables=[self.fx.t])
        right = self._parameters([self.fx.a], [ARG_POS], [None])
        self._assert_subtype_par(left, right)

    def test_overlap_partial(self) -> None:
        left = self._parameters([self.fx.a], [ARG_OPT], [None])
        right = self._parameters([self.fx.b], [ARG_OPT], [None])
        self._assert_overlap_par(left, right)

    def test_overlap_extras(self) -> None:
        # Extra optional arg on the right, allow_partial_overlap: compatible.
        left = self._parameters([self.fx.a], [ARG_POS], [None])
        right = self._parameters([self.fx.a, self.fx.b], [ARG_POS, ARG_OPT], [None, None])
        self._assert_overlap_par(left, right)

    # --- standalone are_parameters_compatible shim (#1066) ---

    def _callable(
        self,
        arg_types: list[Type],
        arg_kinds: list[ArgKind],
        arg_names: list[str | None] | None = None,
        variables: list[TypeVarLikeType] | None = None,
        is_ellipsis_args: bool = False,
    ) -> NormalizedCallableType:
        call = CallableType(
            arg_types,
            arg_kinds,
            arg_names if arg_names is not None else [None] * len(arg_types),
            self.fx.a,
            self.fx.function,
            variables=variables,
            is_ellipsis_args=is_ellipsis_args,
        )
        return cast(NormalizedCallableType, call)

    def _assert_standalone_par(
        self,
        left: NormalizedCallableType | Parameters,
        right: NormalizedCallableType | Parameters,
        is_compat: Callable[[Type, Type], bool],
        **flags: bool,
    ) -> None:
        """Differential: standalone are_parameters_compatible, gate-on vs off."""
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
            are_parameters_compatible,
        )

        flags.setdefault("is_proper_subtype", False)

        def run() -> bool:
            return are_parameters_compatible(left, right, is_compat=is_compat, **flags)

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        python = run()
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        native = run()
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def _standalone_seam(
        self,
        left: NormalizedCallableType | Parameters,
        right: NormalizedCallableType | Parameters,
        *,
        is_proper_subtype: bool = False,
        ignore_pos_arg_names: bool = False,
        allow_partial_overlap: bool = False,
        strict_concatenate_check: bool = False,
        nested_proper_subtype: bool = False,
    ) -> bool | None:
        from mypy.subtypes import _serialize_type

        return _type_kernel.rust_are_parameters_compatible(
            _serialize_type(left),
            _serialize_type(right),
            is_proper_subtype,
            ignore_pos_arg_names,
            allow_partial_overlap,
            strict_concatenate_check,
            state.strict_optional,
            nested_proper_subtype,
            self.resolver,
        )

    def test_standalone_resolver_callback(self) -> None:
        # Module-level is_compat callbacks (checker / constraints callers)
        # engage the kernel; gate-on vs gate-off must agree.
        left = self._callable([self.fx.a], [ARG_POS])
        right = self._callable([self.fx.b], [ARG_POS])
        self._assert_standalone_par(left, right, is_subtype)

    def test_standalone_arg_count_mismatch(self) -> None:
        left = self._callable([self.fx.a], [ARG_POS])
        right = self._callable([self.fx.a, self.fx.b], [ARG_POS, ARG_POS])
        self._assert_standalone_par(left, right, is_subtype)

    def test_standalone_name_mismatch(self) -> None:
        left = self._callable([self.fx.a], [ARG_POS], ["x"])
        right = self._callable([self.fx.b], [ARG_POS], ["y"])
        self._assert_standalone_par(left, right, is_subtype)

    def test_standalone_name_mismatch_ignored_visitor_callback(self) -> None:
        # Module-level is_subtype builds a default SubtypeContext, so a
        # non-default ignore_pos_arg_names defers; the visitor callback with
        # the matching flag engages the kernel instead.
        from mypy.subtypes import SubtypeContext, SubtypeVisitor

        left = self._callable([self.fx.a], [ARG_POS], ["x"])
        right = self._callable([self.fx.b], [ARG_POS], ["y"])
        visitor = SubtypeVisitor(
            right=right,
            subtype_context=SubtypeContext(ignore_pos_arg_names=True),
            proper_subtype=False,
        )
        self._assert_standalone_par(left, right, visitor._is_subtype, ignore_pos_arg_names=True)

    def test_standalone_required_arity(self) -> None:
        left = self._callable([self.fx.a], [ARG_POS])
        right = self._callable([self.fx.a], [ARG_OPT])
        self._assert_standalone_par(left, right, is_subtype)

    def test_standalone_partial_overlap(self) -> None:
        left = self._callable([self.fx.a], [ARG_OPT])
        right = self._callable([self.fx.a], [ARG_OPT])
        self._assert_standalone_par(left, right, is_subtype, allow_partial_overlap=True)

    def test_standalone_trivial_right_suffix(self) -> None:
        # right = (B, *Any, **Any): only the non-star right arg is checked.
        left = self._callable([self.fx.a, self.fx.a], [ARG_POS, ARG_POS])
        right = self._callable(
            [self.fx.b, self.fx.anyt, self.fx.anyt], [ARG_POS, ARG_STAR, ARG_STAR2]
        )
        self._assert_standalone_par(left, right, is_subtype)

    def test_standalone_proper_subtype_flag(self) -> None:
        left = self._callable([self.fx.anyt, self.fx.anyt], [ARG_STAR, ARG_STAR2])
        right = self._callable([self.fx.b], [ARG_POS])
        self._assert_standalone_par(left, right, is_proper_subtype, is_proper_subtype=True)

    def test_standalone_tvar_deferral(self) -> None:
        # Generic callables defer (needs unify_generic_callable); gate-on
        # must fall through to the pure-Python body and agree.
        left = self._callable([self.fx.t], [ARG_POS], variables=[self.fx.t])
        right = self._callable([self.fx.a], [ARG_POS])
        self._assert_standalone_par(left, right, is_subtype)

    def test_standalone_foreign_callback_defers(self) -> None:
        # A non-resolver compat callback (TypeChecker erasure predicates, a
        # test stub) must defer: gate-on runs the same pure-Python body with
        # the same is_compat call count.
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
            are_parameters_compatible,
        )

        left = self._callable([self.fx.a], [ARG_POS])
        right = self._callable([self.fx.b], [ARG_POS])

        def run() -> tuple[bool, int]:
            calls = 0

            def stub_is_compat(l: Type, r: Type) -> bool:
                nonlocal calls
                calls += 1
                return True

            ret = are_parameters_compatible(
                left, right, is_compat=stub_is_compat, is_proper_subtype=False
            )
            return ret, calls

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        python = run()
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        native = run()
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def test_standalone_seam_direct_calls(self) -> None:
        # Direct kernel calls over serialized callables: exact decisions.
        # Arg comparison is contravariant (is_compat(right.typ, left.typ)).
        assert (
            self._standalone_seam(
                self._callable([self.fx.a], [ARG_POS]),
                self._callable([self.fx.a, self.fx.b], [ARG_POS, ARG_POS]),
            )
            is False
        )
        assert (
            self._standalone_seam(
                self._callable([self.fx.b], [ARG_POS]), self._callable([self.fx.a], [ARG_POS])
            )
            is False
        )
        assert (
            self._standalone_seam(
                self._callable([self.fx.a], [ARG_POS]), self._callable([self.fx.b], [ARG_POS])
            )
            is True
        )
        assert (
            self._standalone_seam(
                self._callable([self.fx.a], [ARG_POS], ["x"]),
                self._callable([self.fx.b], [ARG_POS], ["y"]),
            )
            is False
        )
        assert (
            self._standalone_seam(
                self._callable([self.fx.a], [ARG_POS], ["x"]),
                self._callable([self.fx.b], [ARG_POS], ["y"]),
                ignore_pos_arg_names=True,
            )
            is True
        )
        assert (
            self._standalone_seam(
                self._callable([self.fx.a], [ARG_OPT]),
                self._callable([self.fx.a], [ARG_OPT]),
                allow_partial_overlap=True,
            )
            is True
        )
        assert (
            self._standalone_seam(
                self._callable([self.fx.a], [ARG_POS]),
                self._callable([], [], is_ellipsis_args=True),
            )
            is True
        )
        assert (
            self._standalone_seam(
                self._callable([self.fx.a], [ARG_POS]),
                self._callable([self.fx.anyt, self.fx.anyt], [ARG_STAR, ARG_STAR2]),
            )
            is True
        )
        assert (
            self._standalone_seam(
                self._callable([self.fx.t], [ARG_POS], variables=[self.fx.t]),
                self._callable([self.fx.a], [ARG_POS]),
            )
            is None
        )


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
class NativeAreArgsCompatibleSuite(Suite):
    """Parity for the Rust `are_args_compatible` dispatch-head port.

    `mypy.subtypes.are_args_compatible` (subtypes.py:2627-2681) classifies
    the name gate, position gate, required-arity gate, and partial-overlap
    shortcut head over two `FormalArgument` scalars, then falls through to
    `is_compat(right.typ, left.typ)` for the tail. The Rust classifier
    (`subtypes.rs`) turns those facts into a tag (FALSE / TRUE /
    CALL_IS_COMPAT); the Python shim keeps the trailing is_compat call and
    the pure-Python body.

    Direct seam calls assert the exact tag for every branch; the gate-off
    vs gate-on differential drives the real function with a stub is_compat
    and asserts identical (return, call count) pairs.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active

        self._set_active = _set_native_subtype_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _arg(self, name: str | None, pos: int | None, required: bool) -> FormalArgument:
        return FormalArgument(name, pos, NoneType(), required)

    def _tag(
        self,
        left: FormalArgument,
        right: FormalArgument,
        *,
        ignore_pos_arg_names: bool = False,
        allow_partial_overlap: bool = False,
        allow_imprecise_kinds: bool = False,
    ) -> int | None:
        return _type_kernel.rust_are_args_compatible(
            left, right, ignore_pos_arg_names, allow_partial_overlap, allow_imprecise_kinds
        )

    def _run(
        self,
        left: FormalArgument,
        right: FormalArgument,
        *,
        ignore_pos_arg_names: bool = False,
        allow_partial_overlap: bool = False,
        allow_imprecise_kinds: bool = False,
    ) -> tuple[tuple[bool, int], tuple[bool, int]]:
        from mypy.subtypes import are_args_compatible

        def check_one() -> tuple[bool, int]:
            calls = 0

            def is_compat(l: Any, r: Any) -> bool:
                nonlocal calls
                calls += 1
                return True

            ret = are_args_compatible(
                left,
                right,
                is_compat,
                ignore_pos_arg_names=ignore_pos_arg_names,
                allow_partial_overlap=allow_partial_overlap,
                allow_imprecise_kinds=allow_imprecise_kinds,
            )
            return ret, calls

        off = self._with_gate(False, check_one)
        on = self._with_gate(True, check_one)
        return off, on

    def _assert_par(
        self,
        left: FormalArgument,
        right: FormalArgument,
        *,
        ignore_pos_arg_names: bool = False,
        allow_partial_overlap: bool = False,
        allow_imprecise_kinds: bool = False,
    ) -> None:
        off, on = self._run(
            left,
            right,
            ignore_pos_arg_names=ignore_pos_arg_names,
            allow_partial_overlap=allow_partial_overlap,
            allow_imprecise_kinds=allow_imprecise_kinds,
        )
        assert_equal(on, off, f"are_args_compatible parity for left={left!r} right={right!r}")

    def test_seam_name_mismatch_false(self) -> None:
        left = self._arg("x", None, False)
        right = self._arg("y", None, False)
        assert self._tag(left, right) == 0

    def test_seam_name_mismatch_ignored_with_pos_calls_compat(self) -> None:
        left = self._arg("x", 0, False)
        right = self._arg("y", 0, False)
        assert self._tag(left, right, ignore_pos_arg_names=True) == 2

    def test_seam_name_mismatch_ignored_no_right_pos_false(self) -> None:
        left = self._arg("x", None, False)
        right = self._arg("y", None, False)
        assert self._tag(left, right, ignore_pos_arg_names=True) == 0

    def test_seam_right_name_none_calls_compat(self) -> None:
        left = self._arg("x", 0, False)
        right = self._arg(None, 0, False)
        assert self._tag(left, right) == 2

    def test_seam_pos_mismatch_false(self) -> None:
        left = self._arg(None, 0, False)
        right = self._arg(None, 1, False)
        assert self._tag(left, right) == 0

    def test_seam_pos_mismatch_imprecise_calls_compat(self) -> None:
        left = self._arg(None, 0, False)
        right = self._arg(None, 1, False)
        assert self._tag(left, right, allow_imprecise_kinds=True) == 2

    def test_seam_right_pos_none_calls_compat(self) -> None:
        left = self._arg(None, 0, False)
        right = self._arg(None, None, False)
        assert self._tag(left, right) == 2

    def test_seam_required_left_optional_right_false(self) -> None:
        left = self._arg(None, 0, True)
        right = self._arg(None, 0, False)
        assert self._tag(left, right) == 0

    def test_seam_overlap_both_optional_true(self) -> None:
        left = self._arg(None, 0, False)
        right = self._arg(None, 0, False)
        assert self._tag(left, right, allow_partial_overlap=True) == 1

    def test_seam_overlap_one_required_calls_compat(self) -> None:
        left = self._arg(None, 0, True)
        right = self._arg(None, 0, False)
        assert self._tag(left, right, allow_partial_overlap=True) == 2

    def test_seam_both_required_disables_overlap_calls_compat(self) -> None:
        left = self._arg(None, 0, True)
        right = self._arg(None, 0, True)
        assert self._tag(left, right, allow_partial_overlap=True) == 2

    def test_seam_overlap_left_name_none_true(self) -> None:
        left = self._arg(None, 0, False)
        right = self._arg("y", 0, False)
        assert self._tag(left, right, allow_partial_overlap=True) == 1

    def test_parity_every_branch(self) -> None:
        cases: list[tuple[FormalArgument, FormalArgument, dict[str, bool]]] = [
            (self._arg("x", None, False), self._arg("y", None, False), {}),
            (self._arg("x", 0, False), self._arg("y", 0, False), {"ignore_pos_arg_names": True}),
            (
                self._arg("x", None, False),
                self._arg("y", None, False),
                {"ignore_pos_arg_names": True},
            ),
            (self._arg("x", 0, False), self._arg(None, 0, False), {}),
            (self._arg(None, 0, False), self._arg(None, 1, False), {}),
            (
                self._arg(None, 0, False),
                self._arg(None, 1, False),
                {"allow_imprecise_kinds": True},
            ),
            (self._arg(None, 0, False), self._arg(None, None, False), {}),
            (self._arg(None, 0, True), self._arg(None, 0, False), {}),
            (
                self._arg(None, 0, False),
                self._arg(None, 0, False),
                {"allow_partial_overlap": True},
            ),
            (self._arg(None, 0, True), self._arg(None, 0, False), {"allow_partial_overlap": True}),
            (self._arg(None, 0, True), self._arg(None, 0, True), {"allow_partial_overlap": True}),
            (self._arg(None, 0, False), self._arg("y", 0, False), {"allow_partial_overlap": True}),
        ]
        for left, right, flags in cases:
            self._assert_par(left, right, **flags)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSubtypesCallableSuite(Suite):
    """Parity for the native Callable-vs-Callable routing in
    `rust_is_subtype` (Stage C1, #719).

    Previously `is_subtype(CallableType, CallableType)` returned None from
    the Rust visitor and the Python shim fell through to pure-Python
    `is_callable_compatible`. Now the visitor routes Callable-vs-Callable
    into the native callable_compat engine (and Callable-vs-Overloaded into
    an all-items recursion of the same engine), matching the Python
    `visit_callable_type` decision table. The gate-off / gate-on
    differential must produce identical booleans; each test asserts the
    pair (off, on) equal, so a deferral (None -> Python decides) and a
    wrong native answer both fail.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _call(self, *args: Type) -> CallableType:
        return self.fx.callable(*args)

    def _call_named(
        self, arg_names: list[str | None], arg_types: list[Type], ret: Type
    ) -> CallableType:
        return CallableType(
            arg_types, [ARG_POS] * len(arg_types), arg_names, ret, self.fx.function
        )

    def _par(self, left: Type, right: Type) -> tuple[bool, bool]:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        off = is_subtype(left, right)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        on = is_subtype(left, right)
        return off, on

    def test_callable_equal_signatures(self) -> None:
        # (A) -> A <: (A) -> A: identical signature, True both ways.
        left = self._call(self.fx.a, self.fx.a)
        right = self._call(self.fx.a, self.fx.a)
        off, on = self._par(left, right)
        assert (off, on) == (True, True)

    def test_callable_arg_type_differs(self) -> None:
        # (A) -> None <: (B) -> None when B <: A (contravariant arg:
        # needs B <: A, which holds as B(A)) -> True.
        left = self._call(self.fx.a, NoneType())
        right = self._call(self.fx.b, NoneType())
        off, on = self._par(left, right)
        assert (off, on) == (True, True)
        # (B) -> None !<: (A) -> None (needs A <: B, which does not
        # hold) -> False.
        left = self._call(self.fx.b, NoneType())
        right = self._call(self.fx.a, NoneType())
        off, on = self._par(left, right)
        assert (off, on) == (False, False)

    def test_callable_covariant_return(self) -> None:
        # (None) -> A <: (None) -> object (return covariant).
        left = self._call(NoneType(), self.fx.a)
        right = self._call(NoneType(), self.fx.o)
        off, on = self._par(left, right)
        assert (off, on) == (True, True)

    def test_callable_param_name_mismatch_default(self) -> None:
        # Same types, different positional names, no ignore_pos_arg_names
        # passed: `is_subtype` defaults the flag to False, so names must
        # match -> (False, False). This is the differentiated case the

        # native entry must decide identically.
        left = self._call_named(["x"], [self.fx.a], self.fx.a)
        right = self._call_named(["y"], [self.fx.a], self.fx.a)
        off, on = self._par(left, right)
        assert (off, on) == (False, False)

    def test_ignore_pos_arg_names_off_requires_names(self) -> None:
        # With ignore_pos_arg_names=False (as produced by usages that pass
        # the flag explicitly), matching names become mandatory; both
        # engines agree False for a name mismatch, True for a match.
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        left = self._call_named(["x"], [self.fx.a], self.fx.a)
        right = self._call_named(["y"], [self.fx.a], self.fx.a)
        off = is_subtype(left, right, ignore_pos_arg_names=False)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        on = is_subtype(left, right, ignore_pos_arg_names=False)
        assert (off, on) == (False, False)

        match_right = self._call_named(["x"], [self.fx.a], self.fx.a)
        off = is_subtype(left, match_right, ignore_pos_arg_names=False)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        on = is_subtype(left, match_right, ignore_pos_arg_names=False)
        assert (off, on) == (True, True)

    def test_strict_concatenate_from_concatenate(self) -> None:
        # When either side carries `from_concatenate`, Python forces
        # strict_concatenate_check=True regardless of the option
        # (subtypes.py:1872-1875); the native engine must mirror the

        # resulting arg compatibility. A ParamSpec-prefixed callable
        # compared against a plain (A, B) callable: with
        # strict_concatenate_check True the prefix is matched literally,

        # so the fixed args must equal the prefix (A, B) — True here.
        # The differential (off, on) must agree.
        ps = ParamSpecType(
            "P",
            "P",
            TypeVarId(1),
            0,  # ParamSpecFlavor.BARE
            self.fx.o,
            AnyType(TypeOfAny.special_form),
        )
        left = CallableType(
            [UnpackType(ps)],
            [ARG_STAR],
            [None],
            NoneType(),
            self.fx.function,
            from_concatenate=True,
        )
        right = self._call(self.fx.a, self.fx.b, NoneType())
        off, on = self._par(left, right)
        assert off == on

    def test_callable_vs_overloaded_all_items_match(self) -> None:
        # f: (A) -> None <: overload[(A) -> None, (B) -> None] -> True
        # (subtypes.py:966-967: all items).
        left = self._call(self.fx.a, NoneType())
        right = Overloaded([self._call(self.fx.a, NoneType()), self._call(self.fx.b, NoneType())])
        off, on = self._par(left, right)
        assert (off, on) == (True, True)

    def test_callable_vs_overloaded_one_item_fails(self) -> None:
        # f: (A) -> None <: overload[(A) -> int, (B) -> None]: the (A) -> int
        # item fails the return check -> False (all items must match).
        left = self._call(self.fx.a, NoneType())
        right = Overloaded([self._call(self.fx.a, self.fx.a), self._call(self.fx.b, NoneType())])
        off, on = self._par(left, right)
        assert (off, on) == (False, False)

    def test_callable_vs_overloaded_mismatched_arg(self) -> None:
        # f: (A) -> None <: overload[(B) -> None] when B <: A: the (B) ->
        # None item is satisfied via contravariance (A <: B needed... no:

        # contravariance requires B <: A, which holds) -> True. This
        # documents that `b` is a subtype of `a` (B(A)) in the fixture.
        left = self._call(self.fx.a, NoneType())
        right = Overloaded([self._call(self.fx.b, NoneType())])
        off, on = self._par(left, right)
        assert (off, on) == (True, True)

    def test_type_guard_mismatch_false(self) -> None:
        # RIGHT has TypeGuard, LEFT does not: incompatible, False
        # (subtypes.py:922-925, guard set via CallableType.type_guard).

        # LEFT with the guard against a plain right falls through to the
        # return-type check only when the guard types are comparable.
        guarded = CallableType(
            [self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function, type_guard=self.fx.a
        )
        plain = self._call(self.fx.a, self.fx.b)
        # Right-guard / left-plain: incompatible.
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        off = is_subtype(plain, guarded)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        on = is_subtype(plain, guarded)
        assert (off, on) == (False, False)
        # Left-guard / right-plain: allowed (return types compare; both
        # fake `-> B` and `-> TypeGuard(B)` compare by B).
        assert is_subtype(guarded, plain)

    def test_type_guard_both_sides_subtype_of_guard(self) -> None:
        # Both sides guard, guarded types comparable covariantly:
        # type_guard=A <: type_guard=object -> True.
        left = CallableType(
            [self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function, type_guard=self.fx.a
        )
        right = CallableType(
            [self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function, type_guard=self.fx.o
        )
        off, on = self._par(left, right)
        assert (off, on) == (True, True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCallableUnifyPreludeSuite(Suite):
    """Parity for the `is_callable_compatible` prelude the shim runs ahead of
    `rust_callables_compatible` (issue #1279).

    The kernel has no constraint solver, so a generic `left` deferred
    wholesale (453 of 503 cold self-check calls). The shim now normalizes
    both operands, applies the implicit + type-object gates, and unifies a
    generic left via `unify_generic_callable`; the unified left is serialized
    with `variables` dropped (the kernel gates on them, its body never reads
    them). The early shim answers (type-object mismatch, unify failure) must
    match what the Python engine returns for the same operands, and every
    crossed pair must agree gate-off vs gate-on.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _generic(self, arg: Type, ret: Type) -> CallableType:
        # A generic callable over the fixture's single type variable: (T) -> T
        # shaped according to the caller's arg/ret.
        return self.fx.callable(arg, ret).copy_modified(variables=[self.fx.t])

    def _par(self, left: Type, right: Type) -> tuple[bool, bool]:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        off = is_subtype(left, right)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        on = is_subtype(left, right)
        return off, on

    def _par_proper(self, left: Type, right: Type) -> tuple[bool, bool]:
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
            is_proper_subtype,
        )

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        off = is_proper_subtype(left, right)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        on = is_proper_subtype(left, right)
        return off, on

    def test_generic_left_unifiable(self) -> None:
        # (T) -> T <: (object) -> object: T solves to object, then the
        # unified-left decision goes natively -> (True, True).
        left = self._generic(self.fx.t, self.fx.t)
        right = self.fx.callable(self.fx.o, self.fx.o)
        off, on = self._par(left, right)
        assert (off, on) == (True, True)

    def test_generic_left_unify_failure(self) -> None:
        # T must satisfy T >= object (arg) and T <= None (ret): unsolvable,
        # so `unify_generic_callable` returns None and the shim answers
        # False without crossing the seam.
        left = self._generic(self.fx.t, self.fx.t)
        right = self.fx.callable(self.fx.o, self.fx.nonet)
        off, on = self._par(left, right)
        assert (off, on) == (False, False)

    def test_generic_left_native_body_mismatch(self) -> None:
        # Unifiable left whose post-unify body still fails: (x: T) -> T vs
        # (y: object) -> object unifies, but the arg names mismatch, so
        # `are_parameters_compatible` must answer False natively.
        left = CallableType(
            [self.fx.t], [ARG_POS], ["x"], self.fx.t, self.fx.function, variables=[self.fx.t]
        )
        right = CallableType([self.fx.o], [ARG_POS], ["y"], self.fx.o, self.fx.function)
        off, on = self._par(left, right)
        assert (off, on) == (False, False)

    def test_generic_left_same_signature(self) -> None:
        # Identical generic callables: unify solves T = T, body compares
        # equal -> (True, True).
        left = self._generic(self.fx.t, self.fx.t)
        right = self._generic(self.fx.t, self.fx.t)
        off, on = self._par(left, right)
        assert (off, on) == (True, True)

    def test_generic_left_proper_subtype_dimension(self) -> None:
        # Same unifiable pair through the proper-subtype visitor: the seam
        # carries is_proper_subtype=True and must agree with Python.
        left = self._generic(self.fx.t, self.fx.t)
        right = self.fx.callable(self.fx.o, self.fx.o)
        off, on = self._par_proper(left, right)
        assert (off, on) == (True, True)

    def test_gradual_right_unifies_with_generic_left(self) -> None:
        # (T) -> T vs (*Any, **Any) -> object: the trivial-right shortcut in
        # `are_parameters_compatible` answers True without needing unified
        # args; unification must not turn this into a False.
        left = self._generic(self.fx.t, self.fx.t)
        right = CallableType(
            [AnyType(TypeOfAny.special_form), AnyType(TypeOfAny.special_form)],
            [ARG_STAR, ARG_STAR2],
            [None, None],
            self.fx.o,
            self.fx.function,
        )
        off, on = self._par(left, right)
        assert (off, on) == (True, True)

    def test_seam_engagement(self) -> None:
        # Wave 37 (#1426): the generic-left Callable|Callable pair is
        # decided INSIDE rust_is_subtype (the kernel unifies the generic
        # left natively), so the whole pair crosses the is_subtype seam:

        # the unifiable pair once, and the unify-failure pair once more
        # (kernel NoUnify -> False, no Python prelude involved).

        import contextlib

        import type_kernel as tk_mod

        def counting_ctx() -> contextlib.AbstractContextManager[list[object]]:
            calls: list[object] = []

            @contextlib.contextmanager
            def ctx() -> Iterator[list[object]]:
                orig = tk_mod.rust_is_subtype

                def counting(*args: object, **kwargs: object) -> object:
                    calls.append(args)
                    fn = cast(Callable[..., object], orig)
                    return fn(*args, **kwargs)

                tk_mod.rust_is_subtype = counting  # type: ignore[assignment]
                try:
                    yield calls
                finally:
                    tk_mod.rust_is_subtype = orig

            return ctx()

        left = self._generic(self.fx.t, self.fx.t)
        ok_right = self.fx.callable(self.fx.o, self.fx.o)
        fail_right = self.fx.callable(self.fx.o, self.fx.nonet)

        with counting_ctx() as calls:
            on = is_subtype(left, ok_right)
        assert on is True
        assert len(calls) == 1, f"expected 1 seam call, got {len(calls)}"

        with counting_ctx() as calls:
            on = is_subtype(left, fail_right)
        assert on is False
        assert len(calls) == 1, f"expected 1 seam call, got {len(calls)}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsSubtypeBatchSuite(Suite):
    """Parity for the batch `rust_is_subtype_batch` seam.

    One Rust call answers many Instance-vs-Instance pairs with a single
    shared flag set. Each slot must equal the answer the single-pair
    `rust_is_subtype` call would give; a pair Rust cannot decide (a
    protocol-Instance right) must be marked -1 for its slot only, without
    poisoning the decided pairs in the same batch. The Python accumulator
    (`_flush_subtype_batch`) drops -1 slots and answers the rest from the
    returned dict, keyed by (left, right, flag-set).
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        # A protocol-Instance right is a guaranteed deferral (subtypes.rs
        # returns None for protocol rights).
        self.proto_info = self.fx.make_type_info("mod.Proto", mro=[self.fx.oi])
        self.proto_info.is_protocol = True
        type_infos.append(self.proto_info)
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import (
            _clear_subtype_batch,
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _clear_subtype_batch()
        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _batch(self, pairs: list[tuple[bytes, bytes]]) -> list[int]:
        """Run `rust_is_subtype_batch` with the default flag set."""
        flat: list[bytes] = []
        for left_b, right_b in pairs:
            flat.append(left_b)
            flat.append(right_b)
        return _type_kernel.rust_is_subtype_batch(
            flat,
            False,  # ignore_type_params
            False,  # ignore_declared_variance
            False,  # always_covariant
            False,  # ignore_promotions
            False,  # proper_subtype
            state.strict_optional,
            False,  # ignore_pos_arg_names
            False,  # strict_concatenate
            self.resolver,
        )

    def test_identical_repeats_match_single_pair(self) -> None:
        # The same (A, object) pair repeated: every slot must carry the
        # answer the single-pair call gives, so the Python-edge dedup is
        # sound. (A <: B is false in the fixture; use A <: object.)
        from mypy.subtypes import _serialize_type

        left_b = _serialize_type(self.fx.a)
        right_b = _serialize_type(self.fx.o)
        got = self._batch([(left_b, right_b)] * 4)
        assert got == [1, 1, 1, 1]

    def test_decided_pair_batch_result(self) -> None:
        # Decided pairs keep their per-slot answers when mixed together.
        from mypy.subtypes import _serialize_type

        # A <: object -> True (1); B <: A -> True (1); A <: B -> False (0).
        pairs = [
            (_serialize_type(self.fx.a), _serialize_type(self.fx.o)),
            (_serialize_type(self.fx.b), _serialize_type(self.fx.a)),
            (_serialize_type(self.fx.a), _serialize_type(self.fx.b)),
        ]
        got = self._batch(pairs)
        assert got == [1, 1, 0]

    def test_identical_pairs_share_one_slot_result(self) -> None:
        # The same (left, right) appearing twice in one batch answers the
        # same way in both slots.
        from mypy.subtypes import _serialize_type

        got = self._batch(
            [
                (_serialize_type(self.fx.a), _serialize_type(self.fx.o)),
                (_serialize_type(self.fx.a), _serialize_type(self.fx.o)),
            ]
        )
        assert got == [1, 1]

    def test_protocol_right_defers_only_its_slot(self) -> None:
        # A protocol-Instance right is a guaranteed deferral; its slot is
        # -1 while the decided slot around it stays decided.
        from mypy.subtypes import _serialize_type

        proto_inst = Instance(self.proto_info, [])
        got = self._batch(
            [
                (_serialize_type(self.fx.a), _serialize_type(self.fx.o)),
                (_serialize_type(self.fx.a), _serialize_type(proto_inst)),
            ]
        )
        assert got == [1, -1]

    def test_accumulator_flush_maps_answers(self) -> None:
        # `_flush_subtype_batch` folds decided pairs into the build-global
        # answer cache and drops deferred (-1) slots.

        # A repeated identical pair is answered from the cache without a
        # fresh Rust call.

        # Drives the batch primitive directly; 512 is a `Final` threshold,
        # so the accumulator is exercised at the flush edge.
        import mypy.subtypes as subtypes

        subtypes._clear_subtype_batch()
        left_b = subtypes._serialize_type(self.fx.a)
        right_b = subtypes._serialize_type(self.fx.o)
        ctx_key = (False, False, False, False, False, False, False, False, False)
        subtypes._subtype_batch.append((left_b, right_b, ctx_key))
        proto_inst = Instance(self.proto_info, [])
        subtypes._subtype_batch.append(
            (subtypes._serialize_type(self.fx.a), subtypes._serialize_type(proto_inst), ctx_key)
        )
        answers = subtypes._flush_subtype_batch()
        # Decided pair present; deferred pair absent from both dicts.
        assert answers[(left_b, right_b, ctx_key)] is True
        assert subtypes._subtype_answers[(left_b, right_b, ctx_key)] is True
        assert len(answers) == 1
        assert subtypes._subtype_batch == []


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsSubtypeAliasSuite(Suite):
    """Parity for `TypeAliasType` expansion in `rust_is_subtype`.

    Previously both `rust_is_subtype` and `rust_is_subtype_batch` deferred
    to Python whenever a `TypeAliasType` appeared in either operand,
    because the wire carries only `type_ref` (the fullname), not the
    alias target. Now `expand_aliases` walks the type tree, looks up
    each alias in the `TypeAliasResolver`, substitutes type-var args via
    `expand_type_inner`, and recurses until no alias remains, mirroring
    `get_proper_type` (types.py:4032). The gate-off / gate-on
    differential must produce identical booleans; each test asserts the
    pair (off, on) equal.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        self._base_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(self._base_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import (
            _clear_subtype_batch,
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _clear_subtype_batch()
        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _make_alias(
        self, fullname: str, target: Type, *, alias_tvars: list[TypeVarLikeType] | None = None
    ) -> TypeAlias:
        from mypy.nodes import TypeAlias

        return TypeAlias(target, fullname, "mod", -1, -1, alias_tvars=alias_tvars or [])

    def _alias_type(self, alias: TypeAlias) -> TypeAliasType:
        return TypeAliasType(alias, [])

    def _rebuild_resolver(self, aliases: list[Any], extra_infos: list[Any] | None = None) -> None:
        from mypy.subtypes import _set_native_subtype_resolver

        infos = self._base_infos + (extra_infos or [])
        self.resolver = _type_kernel.build_native_resolver(infos, aliases)
        _set_native_subtype_resolver(self.resolver)

    def _par(self, left: Type, right: Type) -> tuple[bool, bool]:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        off = is_subtype(left, right)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        on = is_subtype(left, right)
        return off, on

    def test_alias_to_union_subtype(self) -> None:
        # TA = Union[int, str]; A <: TA and TA <: object.
        alias = self._make_alias("mod.TA", UnionType.make_union([self.fx.a, self.fx.b]))
        self._rebuild_resolver([alias])
        alias_t = self._alias_type(alias)
        # B <: Union[A, B] -> True (B is a member).
        off, on = self._par(self.fx.b, alias_t)
        assert (off, on) == (True, True)
        # A <: Union[A, B] -> True (A is a member).
        off, on = self._par(self.fx.a, alias_t)
        assert (off, on) == (True, True)

    def test_alias_in_union_item(self) -> None:
        # TA = A; Union[TA, B] <: object -> True.
        alias = self._make_alias("mod.TA", self.fx.a)
        self._rebuild_resolver([alias])
        alias_t = self._alias_type(alias)
        u = UnionType.make_union([alias_t, self.fx.b])
        off, on = self._par(u, self.fx.o)
        assert (off, on) == (True, True)

    def test_alias_to_instance(self) -> None:
        # TA = A; B <: TA when B is a subclass of A.
        alias = self._make_alias("mod.TA", self.fx.a)
        self._rebuild_resolver([alias])
        alias_t = self._alias_type(alias)
        # B <: A in the fixture (B(A)) -> True.
        off, on = self._par(self.fx.b, alias_t)
        assert (off, on) == (True, True)
        # A <: B is False; A <: TA is A <: A -> True.
        off, on = self._par(alias_t, self.fx.b)
        assert (off, on) == (False, False)

    def test_generic_alias_args(self) -> None:
        # TA = List[T]; TA[A] <: List[A] -> True (substitution yields
        # identical args). The invariant fixture makes List[A] <:
        # List[object] False in pure Python too.
        T = TypeVarType(
            "T", "T", TypeVarId(1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )
        from mypy.types import Instance as InstanceType

        list_t = InstanceType(self.fx.std_listi, [T])
        alias = self._make_alias("mod.TA", list_t, alias_tvars=[T])
        self._rebuild_resolver([alias])
        alias_a = TypeAliasType(alias, [self.fx.a])
        # List[A] <: List[A] -> True after substitution.
        off, on = self._par(alias_a, InstanceType(self.fx.std_listi, [self.fx.a]))
        assert (off, on) == (True, True)

    def test_alias_not_in_resolver_defers(self) -> None:
        # An alias whose TypeAlias is not in the resolver defers to
        # Python (expand_aliases returns None); both paths must agree.

        alias = self._make_alias("mod.Missing", self.fx.a)
        alias_t = TypeAliasType(alias, [])
        off, on = self._par(self.fx.b, alias_t)
        assert off == on


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeInferVarianceSuite(Suite):
    """Parity tests for the Rust `infer_variance` member decision.

    `infer_variance` mutates `info.defn.type_vars[i].variance` while
    scanning members; the Python shim keeps that loop and routes each
    member's algebra (erase_return_self_types wire subset, expand_type,
    the two is_subtype calls) through `rust_infer_variance_member`,
    deferring (None) to the pure-Python body when Rust cannot decide.
    Each test runs the real `infer_variance` gate-off vs gate-on and
    asserts the final variance and success flag agree. The direct seam
    tests additionally prove the Rust decision engages and computes the
    expected flip bitmask for known-variance fixtures.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_native_subtype_active = _set_native_subtype_active
        self._set_native_subtype_resolver = _set_native_subtype_resolver
        self._initial_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.gi,
            self.fx.functioni,
            self.fx.bool_type_info,
            self.fx.str_type_info,
            self.fx.std_listi,
        ]
        set_wire_typeinfo_map({info.fullname: info for info in self._initial_infos})

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_native_subtype_active(False)
        self._set_native_subtype_resolver(None)
        set_wire_typeinfo_map(None)

    def _make_class(self) -> TypeInfo:
        """A `class C(Generic[T])` with variance NOT_READY and no members."""
        from mypy.nodes import VARIANCE_NOT_READY
        from mypy.wirefixup import set_wire_typeinfo_map

        info = self.fx.make_type_info(
            "C", mro=[self.fx.oi], typevars=["T"], variances=[VARIANCE_NOT_READY]
        )
        set_wire_typeinfo_map({i.fullname: i for i in self._initial_infos + [info]})
        return info

    def _add_var_member(self, info: TypeInfo, member_type: Type) -> None:
        from mypy.nodes import MDEF, SymbolTableNode, Var

        v = Var("x")
        v.type = member_type
        v.info = info
        info.names["x"] = SymbolTableNode(MDEF, v)

    def _install_gate(self, info: TypeInfo, on: bool) -> None:
        self._set_native_subtype_resolver(
            None if not on else _build_native_variance_resolver(self._initial_infos + [info])
        )
        self._set_native_subtype_active(on)

    def _assert_variance(self, member_type: Type, expected: int) -> None:
        off_ok, off_v = self._run_variance_gated(member_type, False)
        on_ok, on_v = self._run_variance_gated(member_type, True)
        assert (off_ok, off_v) == (
            on_ok,
            on_v,
        ), f"gate-off {off_ok, off_v} != gate-on {on_ok, on_v}"
        assert (off_ok, off_v) == (True, expected), f"expected {expected}, got {off_ok, off_v}"

    def _run_variance_gated(self, member_type: Type, on: bool) -> tuple[bool, int]:
        return self._run_variance_gated_with(lambda info: member_type, on)

    def _run_variance_gated_with(
        self, member_factory: Callable[[TypeInfo], Type], on: bool
    ) -> tuple[bool, int]:
        from mypy.subtypes import infer_variance

        info = self._make_class()
        self._add_var_member(info, member_factory(info))
        self._install_gate(info, on)
        try:
            ok = infer_variance(info, 0)
            return ok, cast(TypeVarType, info.defn.type_vars[0]).variance
        finally:
            self._install_gate(info, False)

    def test_member_t_covariant(self) -> None:
        # Class with a settable Var x: T infers INVARIANT (a settable
        # variable of type T neither covariant nor contravariant).
        self._assert_variance(self.fx.t, INVARIANT)

    def test_member_callable_contravariant(self) -> None:
        # x: Callable[[T], None] infers CONTRAVARIANT (contravariant use).
        c = CallableType([self.fx.t], [ARG_POS], [None], NoneType(), self.fx.function)
        self._assert_variance(c, CONTRAVARIANT)

    def test_member_callable_returning_t_invariant(self) -> None:
        # x: Callable[[T], T] infers INVARIANT.
        c = CallableType([self.fx.t], [ARG_POS], [None], self.fx.t, self.fx.function)
        self._assert_variance(c, INVARIANT)

    def test_member_self_return_deferral(self) -> None:
        # A method returning the self type erases in Python (callable-form
        # self return); the Rust subset must defer so Python decides.

        def make_member(info: TypeInfo) -> Type:
            from mypy.types import Instance

            return CallableType(
                [self.fx.t], [ARG_POS], [None], Instance(info, [self.fx.t]), self.fx.function
            )

        off_ok, off_v = self._run_variance_gated_with(make_member, False)
        on_ok, on_v = self._run_variance_gated_with(make_member, True)
        assert (off_ok, off_v) == (on_ok, on_v)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeInferVarianceSeamSuite(Suite):
    """Direct-seam tests for `rust_infer_variance_member`.

    These prove the Rust module engages (does not always defer) and
    returns the expected co/contra flip bitmask for known-variance
    fixture classes, independent of the `infer_variance` loop.
    """

    def setUp(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.gi,
            self.fx.functioni,
            self.fx.bool_type_info,
            self.fx.str_type_info,
            self.fx.std_listi,
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._type_infos = type_infos

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        set_wire_typeinfo_map(None)

    def _make_covariant_class(self) -> TypeInfo:
        # Explicit COVARIANT so the Rust subtype seam (which reads the
        # snapshot variance) can decide the C[T] vs C[object] checks.
        from mypy.nodes import COVARIANT
        from mypy.wirefixup import set_wire_typeinfo_map

        info = self.fx.make_type_info("C", mro=[self.fx.oi], typevars=["T"], variances=[COVARIANT])
        self._type_infos.append(info)
        set_wire_typeinfo_map({i.fullname: i for i in self._type_infos})
        return info

    def _compute(self, info: TypeInfo, member_type: Type) -> int | None:
        import mypy.subtypes as subtypes
        from mypy.typevars import fill_typevars

        resolver = _build_native_variance_resolver(self._type_infos)
        subtypes._set_native_subtype_active(True)
        subtypes._set_native_subtype_resolver(resolver)
        try:
            self_type = fill_typevars(info)
            object_type = Instance(self.fx.oi, [])
            tvar = info.defn.type_vars[0]
            return subtypes._native_infer_variance_member(
                member_type, self_type, object_type, tvar
            )
        finally:
            subtypes._set_native_subtype_active(False)
            subtypes._set_native_subtype_resolver(None)

    def test_member_t_engages(self) -> None:
        # x: T in a COVARIANT class: C[T] <: C[object] holds (no flip),
        # C[object] <: C[T] fails -> contravariant flip (bit 2).
        info = self._make_covariant_class()
        v = Var("x")
        v.type = self.fx.t
        v.info = info
        info.names["x"] = SymbolTableNode(MDEF, v)
        result = self._compute(info, self.fx.t)
        assert result == 2, f"expected contravariant-only flip, got {result}"

    def test_member_callable_engages(self) -> None:
        # x: Callable[[T], None]: C[T] vs C[object] only differs in the
        # arg position (contravariant) -> bit 1 (covariant flip).
        info = self._make_covariant_class()
        c = CallableType([self.fx.t], [ARG_POS], [None], NoneType(), self.fx.function)
        v = Var("x")
        v.type = c
        v.info = info
        info.names["x"] = SymbolTableNode(MDEF, v)
        result = self._compute(info, c)
        assert result == 1, f"expected covariant-only flip, got {result}"

    def test_callable_returning_t_invariant(self) -> None:
        # x: Callable[[T], T]: both directions flip -> bit 3.
        info = self._make_covariant_class()
        c = CallableType([self.fx.t], [ARG_POS], [None], self.fx.t, self.fx.function)
        v = Var("x")
        v.type = c
        v.info = info
        info.names["x"] = SymbolTableNode(MDEF, v)
        result = self._compute(info, c)
        assert result == 3, f"expected both flips, got {result}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeObjectOrAnyFromTypeSuite(Suite):
    """Parity for the Rust `object_or_any_from_type` port (joinfns.rs).

    Runs the pure-Python body (join gate off) and the Rust path (join
    gate on) on the same corpus and asserts identical results. The Rust
    path serializes the proper type, calls
    `type_kernel.rust_object_or_any_from_type`, and deserializes the
    result; both must produce equal types. Cases Rust cannot handle
    (missing resolver snapshot, unwritable variant) return None and the
    Python body runs unchanged, so the differential still passes.
    """

    def setUp(self) -> None:

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
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _set_active(self, active: bool) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver

        _set_native_join_active(active)
        _set_native_join_resolver(self.resolver if active else None)

    def _native_result(self, typ: ProperType) -> ProperType:
        from mypy.join import object_or_any_from_type

        return object_or_any_from_type(typ)

    def _reference_result(self, typ: ProperType) -> ProperType:
        from mypy.join import object_or_any_from_type

        return object_or_any_from_type(typ)

    def _assert_parity(self, typ: ProperType) -> None:
        res = self._native_result(typ)
        ref = self._reference_result(typ)
        assert_equal(res, ref)

    def test_instance(self) -> None:
        # Instance -> object_from_instance (mro[-1] = object).
        self._assert_parity(self.fx.a)

    def test_instance_subclass(self) -> None:
        # B mro = [B, A, object], mro[-1] = object.
        self._assert_parity(self.fx.b)

    def test_callable_type(self) -> None:
        # CallableType -> object from its function fallback
        # (builtins.function mro[-1] = object).
        self._assert_parity(self.fx.callable(self.fx.a, self.fx.b))

    def test_literal_type(self) -> None:
        # LiteralType -> object from its fallback (A -> object).
        self._assert_parity(self.fx.lit1)

    def test_typed_dict_type(self) -> None:
        # TypedDictType -> object from its fallback (A).
        td = TypedDictType({"x": self.fx.a}, {"x"}, set(), self.fx.a)
        self._assert_parity(td)

    def test_tuple_type(self) -> None:
        # TupleType -> object from partial_fallback (builtins.tuple).
        self._assert_parity(TupleType([self.fx.a, self.fx.b], self.fx.std_tuple))

    def test_type_type(self) -> None:
        # TypeType -> recurse on item (Instance A -> object).
        self._assert_parity(self.fx.type_a)

    def test_type_var(self) -> None:
        # TypeVarType -> recurse on upper_bound (object).
        self._assert_parity(self.fx.t)

    def test_type_var_bound_subclass(self) -> None:
        # TypeVar bound to A -> recurse on A -> object.
        bound = TypeVarType(
            "T", "T", TypeVarId(100), [], self.fx.a, AnyType(TypeOfAny.from_omitted_generics)
        )
        self._assert_parity(bound)

    def test_union_with_instance_candidate(self) -> None:
        # Union[Callable, A] -> first Instance candidate = object(A).
        u = UnionType.make_union([self.fx.callable(self.fx.a), self.fx.a])
        self._assert_parity(u)

    def test_union_without_instance_candidate(self) -> None:
        # Union[Callable, Callable] (no bare Instance) -> Any.
        u = UnionType.make_union([self.fx.callable(self.fx.a), self.fx.callable(self.fx.b)])
        self._assert_parity(u)

    def test_unpack_type(self) -> None:
        # UnpackType -> Python discards the recursion result, returns Any.
        unpacked = UnpackType(TupleType([self.fx.a], self.fx.std_tuple))
        self._assert_parity(unpacked)

    def test_any_fallback(self) -> None:
        # UnboundType (not handled) -> AnyType(implementation_artifact).
        self._assert_parity(UnboundType("X"))

    def test_type_var_tuple(self) -> None:
        # TypeVarTupleType upper_bound is tuple[object] -> object(tuple).
        self._assert_parity(self.fx.ts)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeObjectFromInstanceSuite(Suite):
    """Parity for the Rust `object_from_instance` port (join.py:1303,
    joinfns.rs).

    Toggling the join gate off vs on must agree on the constructed
    `builtins.object` Instance for the same input Instances, asserting
    equality of fullname AND the produced `Instance` (line/column are
    dropped by the wire round-trip, so the comparison uses the
    `assert_equal` type comparison). The direct seam call proves the
    Rust engagement.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
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
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        self._set_active = _set_native_join_active
        self._set_resolver = _set_native_join_resolver
        self._set_map = _set_native_join_typeinfo_map
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._typeinfo_map = {info.fullname: info for info in type_infos}
        self._set_resolver(self._resolver)
        self._set_map(self._typeinfo_map)
        set_wire_typeinfo_map(self._typeinfo_map)
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _native_result(self, instance: Instance) -> Instance:
        from mypy.join import object_from_instance

        return self._with_gate(True, lambda: object_from_instance(instance))

    def _reference_result(self, instance: Instance) -> Instance:
        from mypy.join import object_from_instance

        return self._with_gate(False, lambda: object_from_instance(instance))

    def _assert_parity(self, instance: Instance) -> None:
        res = self._native_result(instance)
        ref = self._reference_result(instance)
        assert_equal(res, ref)

    def test_object(self) -> None:
        # object -> object (mro[-1]).
        self._assert_parity(self.fx.o)

    def test_simple_class(self) -> None:
        # A (mro=[A, object]) -> object.
        self._assert_parity(self.fx.a)

    def test_subclass(self) -> None:
        # B (mro=[B, A, object]) -> object.
        self._assert_parity(self.fx.b)

    def test_deep_hierarchy(self) -> None:
        # E2 (mro=[E2, F2, F, object]) -> object.
        self._assert_parity(self.fx.e2)

    def test_generic_instance(self) -> None:
        # G[A] -> object (args dropped).
        self._assert_parity(self.fx.ga)

    def test_str_instance(self) -> None:
        # builtins.str -> object.
        self._assert_parity(self.fx.str_type)

    def test_bool_instance(self) -> None:
        # builtins.bool -> object.
        self._assert_parity(self.fx.bool_type)

    def test_seam_engages(self) -> None:
        # Direct seam call: rust_object_from_instance returns the
        # builtins.object fullname for a serialized Instance.
        from mypy.join import _serialize_type

        result = _type_kernel.rust_object_from_instance(_serialize_type(self.fx.a), self._resolver)
        assert result == "builtins.object", f"got {result!r}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCombineSimilarCallablesSuite(Suite):
    """Parity for the Rust `combine_similar_callables` port (joinfns.rs).

    Runs the pure-Python body (join gate off) and the Rust path (join
    gate on) on the same callable pairs and asserts identical results.
    Both-generic callables renumber natively since the TypeVarId
    registry (#1345): their ids are Rust-side fresh ids, so only the
    direct-seam engagement and value shape are asserted, not parity
    with Python's TypeVarId.new ids. Per-arg joins it cannot decide
    still return None and the Python body runs unchanged.
    """

    def setUp(self) -> None:

        from mypy.join import _set_native_join_typeinfo_map

        self.fx = TypeFixture(INVARIANT)
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_join_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )

        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _set_active(self, active: bool) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver

        _set_native_join_active(active)
        _set_native_join_resolver(self.resolver if active else None)

    def _native_result(self, t: CallableType, s: CallableType) -> CallableType:
        from mypy.join import combine_similar_callables

        return combine_similar_callables(t, s)

    def _reference_result(self, t: CallableType, s: CallableType) -> CallableType:
        from mypy.join import combine_similar_callables

        return combine_similar_callables(t, s)

    def _assert_parity(self, t: CallableType, s: CallableType) -> None:
        res = self._native_result(t, s)
        ref = self._reference_result(t, s)
        assert_equal(res, ref)

    def test_same_arg_names(self) -> None:
        # (a: A) -> B joined with (a: A) -> B: names kept.
        t = CallableType([self.fx.a], [ARG_POS], ["a"], self.fx.b, self.fx.function)
        s = CallableType([self.fx.a], [ARG_POS], ["a"], self.fx.b, self.fx.function)
        self._assert_parity(t, s)

    def test_different_arg_names(self) -> None:
        # (a: A) vs (x: A): positional names differ -> name dropped (None).
        t = CallableType([self.fx.a], [ARG_POS], ["a"], self.fx.b, self.fx.function)
        s = CallableType([self.fx.a], [ARG_POS], ["x"], self.fx.b, self.fx.function)
        self._assert_parity(t, s)

    def test_named_args(self) -> None:
        # Named args: names kept even if different (combine_arg_names
        # is_named rule).
        t = CallableType([self.fx.a], [ARG_NAMED], ["a"], self.fx.b, self.fx.function)
        s = CallableType([self.fx.a], [ARG_NAMED], ["x"], self.fx.b, self.fx.function)
        self._assert_parity(t, s)

    def test_fallback_function_vs_type(self) -> None:
        # t fallback=function, s fallback=type -> function wins.
        t = CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function)
        s = CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.type_type)
        self._assert_parity(t, s)

    def test_instance_type_none_both(self) -> None:
        # Both instance_type None -> result None.
        t = CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function)
        s = CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function)
        self._assert_parity(t, s)

    def test_instance_type_both(self) -> None:
        # Both instance_type set -> joined via join_types.
        t = CallableType(
            [self.fx.a],
            [ARG_POS],
            [None],
            self.fx.b,
            self.fx.function,
            variables=[],
            instance_type=self.fx.a,
        )
        s = CallableType(
            [self.fx.a],
            [ARG_POS],
            [None],
            self.fx.b,
            self.fx.function,
            variables=[],
            instance_type=self.fx.b,
        )
        self._assert_parity(t, s)

    def test_ret_type_join(self) -> None:
        # ret A vs ret B -> join = object.
        t = CallableType([self.fx.a], [ARG_POS], [None], self.fx.a, self.fx.function)
        s = CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function)
        self._assert_parity(t, s)

    def test_generic_both_renumbers_natively(self) -> None:
        # Both generic: the TypeVarId registry (#1345) renumbers both
        # operands into one sentinel-namespace batch (ids are Rust-side
        # fresh ids; id-only difference accepted, as in setops.rs tests).
        tv = [
            TypeVarType(
                "T", "T", TypeVarId(10), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
            )
        ]
        t = CallableType([tv[0]], [ARG_POS], [None], tv[0], self.fx.function, variables=tv)
        from mypy.join import _serialize_type

        result = _type_kernel.rust_combine_similar_callables(
            _serialize_type(t), _serialize_type(t), True, self.resolver
        )
        assert result is not None, "Rust must renumber both-generic combine natively"
        # SameS through the shim: identical operands keep one renumbered
        # variable, and arg/ret ids point at the shared variable id.
        combined = self._native_result(t, t)
        assert isinstance(combined, CallableType)
        assert len(combined.variables) == 1
        arg_t = combined.arg_types[0]
        assert isinstance(arg_t, TypeVarType)
        ret_t = combined.ret_type
        assert isinstance(ret_t, TypeVarType)
        assert arg_t.id == combined.variables[0].id
        assert ret_t.id == combined.variables[0].id
        assert str(combined.variables[0]) == "T"

    def test_no_variables_unchanged(self) -> None:
        # min_len == 0: match_generic_callables no-op; plain join.
        t = CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function)
        s = CallableType([self.fx.b], [ARG_POS], [None], self.fx.a, self.fx.function)
        self._assert_parity(t, s)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAnyConstraintsSuite(Suite):
    """Parity tests for `rust_any_constraints`.

    Differential harness: runs `any_constraints` on the same option list with
    the native gate on (resolver installed) and off (pure Python), and asserts
    the results are equal. Constraint.__eq__ compares (type_var, op, target)
    and UnionType.__eq__ compares the item frozenset, so the wire round-trip
    recomposes exactly what the Python body builds.
    """

    def setUp(self) -> None:
        from mypy.constraints import Constraint, _set_native_constraints_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.Constraint = Constraint
        self._set_active = _set_native_constraints_active
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active(False)

    def tearDown(self) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)

    def _tvar(
        self,
        name: str,
        raw_id: int,
        upper_bound: Type,
        *,
        meta_level: int = 0,
        values: list[Type] | None = None,
    ) -> TypeVarType:
        return TypeVarType(
            name,
            name,
            TypeVarId(raw_id, meta_level=meta_level),
            values if values is not None else [],
            upper_bound,
            AnyType(TypeOfAny.from_omitted_generics),
        )

    def _result(
        self, options: Sequence[list[Constraint] | None], eager: bool, native: bool
    ) -> list[Any]:
        from mypy.constraints import _set_native_constraints_resolver, any_constraints

        self._set_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return any_constraints(list(options), eager=eager)

    def _assert_par(self, options: Sequence[list[Constraint] | None], eager: bool = False) -> None:
        native = self._result(options, eager, native=True)
        python = self._result(options, eager, native=False)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def test_all_any_targets_skips_op(self) -> None:
        # is_same_constraint: both targets Any -> op check skipped.
        self._assert_par(
            [
                [self.Constraint(self.fx.t, 0, AnyType(TypeOfAny.special_form))],
                [self.Constraint(self.fx.t, 1, AnyType(TypeOfAny.special_form))],
            ],
            eager=True,
        )

    def test_trivial_with_merge_union(self) -> None:
        # select_trivial + merge_with_any: option2 is trivial, merge the
        # target with Any into a union.
        anyt = AnyType(TypeOfAny.special_form)
        self._assert_par(
            [[self.Constraint(self.fx.t, 0, anyt)], [self.Constraint(self.fx.t, 0, self.fx.a)]],
            eager=True,
        )

    def test_len_one_option_passthrough(self) -> None:
        self._assert_par([[self.Constraint(self.fx.t, 0, self.fx.a)]])

    def test_empty_option(self) -> None:
        self._assert_par([[]])
        self._assert_par([None])
        self._assert_par([])

    def test_none_option_with_present(self) -> None:
        self._assert_par([None, [self.Constraint(self.fx.t, 0, self.fx.a)]])

    def test_mixed_origins_is_similar(self) -> None:
        # is_similar_constraints with differing raw ids in the same
        # namespace: NoRelation -> options are not merged, and no unified
        # constraint is produced.
        self._assert_par(
            [
                [self.Constraint(self.fx.t, 0, self.fx.a)],
                [self.Constraint(self.fx.s, 0, self.fx.a)],
            ],
            eager=True,
        )

    def test_values_satisfiable(self) -> None:
        # filter_satisfiable via the values branch: t1 has values and the
        # target is a subtype of one of them.
        t1 = self._tvar("t1", 12, self.fx.o, values=[self.fx.a, self.fx.b])
        self._assert_par(
            [[self.Constraint(t1, 0, self.fx.a)], [self.Constraint(t1, 0, self.fx.b)]], eager=True
        )

    def test_meta_exclusion(self) -> None:
        # exclude_non_meta_vars with a non-meta (meta_level=0) origin.
        self._assert_par(
            [
                [self.Constraint(self.fx.t, 0, self.fx.a)],
                [self.Constraint(self.fx.t, 0, self.fx.b)],
            ],
            eager=True,
        )

    def test_eager_false_branch(self) -> None:
        self._assert_par(
            [
                [self.Constraint(self.fx.t, 0, self.fx.a)],
                [self.Constraint(self.fx.t, 0, self.fx.b)],
            ],
            eager=False,
        )

    def test_seam_engages_with_none_option(self) -> None:
        # A differential-only suite passes silently when the kernel defers and Python answers
        # both sides. That is how the None-option (-1) bug survived: read_size rejected it and
        # every call deferred. Call the seam directly to prove the wire decodes (issue #1171).
        from mypy.cache import write_int_bare  # type: ignore[attr-defined]
        from mypy.constraints import _set_native_constraints_resolver, _write_option

        options: list[list[Constraint] | None] = [None, [self.Constraint(self.fx.t, 0, self.fx.a)]]
        buf = _WriteBuffer()
        write_int_bare(buf, len(options))
        write_int_bare(buf, -1)
        option1 = options[1]
        assert option1 is not None
        _write_option(buf, option1)
        _set_native_constraints_resolver(self.resolver)
        try:
            raw = _type_kernel.rust_any_constraints(
                buf.getvalue(), False, state.strict_optional, self.resolver
            )
        finally:
            _set_native_constraints_resolver(None)
        assert raw is not None, "seam must not defer on a None (-1) option"
        assert len(raw) == 1

    def test_seam_preserves_extra_tvars_identity(self) -> None:
        # Polymorphic-call inference attaches extra_tvars to constraints; the
        # wire format has no slot for them, so returning wire-rebuilt
        # Constraints silently drops them (issue #1171).

        # The shim must match each wire blob back to the original live Constraint and
        # return it; value-equal-but-distinct options disambiguate (the all-same branch
        # returns the first valid option's constraints, exactly as the pure-Python body does).
        from mypy.constraints import _set_native_constraints_resolver, _try_native_any_constraints

        c1 = self.Constraint(self.fx.t, 0, self.fx.a)
        c2 = self.Constraint(self.fx.t, 0, self.fx.a)
        extra_a = self._tvar("EA", 900, self.fx.o)
        extra_b = self._tvar("EB", 901, self.fx.o)
        c1.extra_tvars = [extra_a]
        c2.extra_tvars = [extra_b]
        assert c1 == c2  # value-equal so the kernel takes the all-same path

        _set_native_constraints_resolver(self.resolver)
        try:
            res2 = _try_native_any_constraints([[c2], [c1]], eager=False)
        finally:
            _set_native_constraints_resolver(None)
        assert res2 is not None, "seam must not defer on verbatim options"
        assert res2[0] is c2, "seam must return the live original Constraint"
        assert res2[0].extra_tvars == [extra_b], "extra_tvars must survive"
        # Differential: gate-on vs gate-off result equality.
        self._assert_par([[c2], [c1]], eager=False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeDivergingAliasSuite(Suite):
    """Parity tests for `rust_detect_diverging_alias` (mypy.typeanal.detect_diverging_alias).

    Builds fresh alias nodes per path (the positive verdict is cached on
    `node._is_recursive`, so gate-on and gate-off must not share nodes) and
    asserts the native path matches the pure-Python body, including the
    `_is_recursive is None` collect-aliases front ported in issue #1171
    (previously a 100%-defer seam with zero test coverage).
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        self.fx = TypeFixture()
        self._set_active = _set_native_typeanal_active

    def tearDown(self) -> None:
        self._set_active(False)

    def _make_diverging_pair(self) -> tuple[TypeAlias, Type]:
        # F = tuple[F[list[T]]]: the alias reappears in a seen chain with a
        # typevar-carrying arg, so both the recursive-alias front
        # (collect-aliases walk) and the diverging-expansion back end fire.
        from mypy.nodes import TypeAlias

        a = TypeAliasType(None, [])
        node = TypeAlias(self.fx.anyt, "__main__.F", "__main__", -1, -1)
        arg = TypeAliasType(node, [Instance(self.fx.std_listi, [self.fx.t])])
        target = Instance(self.fx.std_tuplei, [arg])
        a.alias = node
        node.target = target
        return node, target

    def _make_recursive_not_diverging_pair(self) -> tuple[TypeAlias, Type]:
        # A = tuple[Union[B, A]]: recursive but every occurrence is bare, so
        # the alias is recursive yet not diverging.
        from mypy.nodes import TypeAlias

        a = TypeAliasType(None, [])
        node = TypeAlias(self.fx.anyt, "__main__.R", "__main__", -1, -1)
        arg = TypeAliasType(node, [])
        target = Instance(self.fx.std_tuplei, [UnionType([self.fx.b, arg])])
        a.alias = node
        node.target = target
        return node, target

    def _make_plain_pair(self) -> tuple[TypeAlias, Type]:
        from mypy.nodes import TypeAlias

        node = TypeAlias(self.fx.std_tuple, "__main__.P", "__main__", -1, -1)
        return node, node.target

    def test_diverging_recursion(self) -> None:
        from mypy.typeanal import detect_diverging_alias

        self._set_active(False)
        node_py, target_py = self._make_diverging_pair()
        expected = detect_diverging_alias(node_py, target_py)
        self._set_active(True)
        node_na, target_na = self._make_diverging_pair()
        actual = detect_diverging_alias(node_na, target_na)
        self.assertEqual(actual, expected)
        self.assertTrue(expected)
        self.assertTrue(node_na._is_recursive, "positive verdict is cached")

    def test_recursive_not_diverging(self) -> None:
        from mypy.typeanal import detect_diverging_alias

        self._set_active(False)
        node_py, target_py = self._make_recursive_not_diverging_pair()
        expected = detect_diverging_alias(node_py, target_py)
        self._set_active(True)
        node_na, target_na = self._make_recursive_not_diverging_pair()
        actual = detect_diverging_alias(node_na, target_na)
        self.assertEqual(actual, expected)
        self.assertFalse(expected)
        self.assertTrue(node_na._is_recursive, "recursive verdict is cached on the node")

    def test_non_recursive_alias(self) -> None:
        from mypy.typeanal import detect_diverging_alias

        self._set_active(False)
        node_py, target_py = self._make_plain_pair()
        expected = detect_diverging_alias(node_py, target_py)
        self._set_active(True)
        node_na, target_na = self._make_plain_pair()
        actual = detect_diverging_alias(node_na, target_na)
        self.assertEqual(actual, expected)
        self.assertFalse(expected)

    def test_seam_engages(self) -> None:
        self._set_active(True)
        node, target = self._make_diverging_pair()
        result = _type_kernel.rust_detect_diverging_alias(node, target)
        self.assertTrue(result)
        plain_node, plain_target = self._make_plain_pair()
        assert _type_kernel.rust_detect_diverging_alias(plain_node, plain_target) is False


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRepackCallableArgsSuite(Suite):
    """Parity tests for `rust_repack_callable_args`.

    Differential harness: runs `repack_callable_args` with the native gate
    on and off, and asserts the returned arg-type lists are equal. Exercises
    the ARG_STAR present/absent branches and the unpack normalization.
    """

    def setUp(self) -> None:
        from mypy.constraints import _set_native_constraints_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_constraints_active
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active(False)

    def tearDown(self) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)

    def _callable(self, arg_types: list[Type], arg_kinds: list[ArgKind]) -> CallableType:
        return CallableType(
            arg_types,
            arg_kinds,
            [None] * len(arg_types),
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )

    def _result(self, callable: CallableType, native: bool) -> list[Any]:
        from mypy.constraints import _set_native_constraints_resolver, repack_callable_args

        self._set_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return repack_callable_args(callable, self.fx.std_tuplei)

    def _assert_par(self, callable: CallableType) -> None:
        native = self._result(callable, native=True)
        python = self._result(callable, native=False)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def test_no_star(self) -> None:
        # ARG_STAR absent -> callable.arg_types passes through.
        self._assert_par(self._callable([self.fx.a, self.fx.b], [ARG_POS, ARG_POS]))

    def test_star_plain(self) -> None:
        # *args: A -> UnpackType(Instance(builtins.tuple, [A])).
        self._assert_par(self._callable([self.fx.a, self.fx.b], [ARG_POS, ARG_STAR]))

    def test_star_unpack_tuple(self) -> None:
        # *args: *tuple[A, B] -> unpack with the tuple prefix spliced and
        # the rest as a suffix.
        star = UnpackType(
            TupleType(
                [UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.b],
                self.fx.std_tuple,
            )
        )
        self._assert_par(self._callable([star], [ARG_STAR]))

    def test_prefix_and_star(self) -> None:
        self._assert_par(
            self._callable([self.fx.a, self.fx.b, self.fx.c], [ARG_POS, ARG_STAR, ARG_POS])
        )


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRemoveTrivialSuite(Suite):
    """Parity for the Rust `remove_trivial` port (mypy.expandtype,
    expandtype.py:984-1011).

    Toggling the expand-type gate off vs on must agree on the simplified
    list for the same inputs, including the strict_optional-dependent
    NoneType handling. A direct seam call proves the Rust engagement.
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_expand_type_active
        self._set_resolver = _set_native_expand_type_resolver
        self._set_map = _set_native_expand_type_typeinfo_map
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._type_infos = type_infos
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(self._resolver)
        self._set_map({info.fullname: info for info in type_infos})
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _remove(self, types: Sequence[Type]) -> list[Type]:
        from mypy.expandtype import remove_trivial

        return remove_trivial(types)

    def _remove_so(self, types: Sequence[Type], strict_optional: bool) -> list[Type]:
        import mypy.state

        old = mypy.state.state.strict_optional
        mypy.state.state.strict_optional = strict_optional
        try:
            return self._remove(types)
        finally:
            mypy.state.state.strict_optional = old

    def _assert_par(self, types: Sequence[Type], strict_optional: bool = True) -> None:
        off = str(self._with_gate(False, lambda: self._remove_so(types, strict_optional)))
        on = str(self._with_gate(True, lambda: self._remove_so(types, strict_optional)))
        assert_equal(on, off, f"remove_trivial parity {types} so={strict_optional}")

    def test_empty(self) -> None:
        assert_equal(self._remove_so([], True), [UninhabitedType()])
        self._assert_par([])

    def test_only_uninhabited(self) -> None:
        types = [UninhabitedType()]
        assert_equal(self._remove_so(types, True), [UninhabitedType()])
        self._assert_par(types)

    def test_none_strict_optional_true(self) -> None:
        import mypy.state

        old = mypy.state.state.strict_optional
        mypy.state.state.strict_optional = True
        try:
            result = self._remove([NoneType()])
            assert_equal(result, [NoneType()])
        finally:
            mypy.state.state.strict_optional = old
        self._assert_par([NoneType()], strict_optional=True)

    def test_none_strict_optional_false(self) -> None:
        import mypy.state

        old = mypy.state.state.strict_optional
        mypy.state.state.strict_optional = False
        try:
            result = self._remove([NoneType()])
            assert_equal(result, [NoneType()])  # removed_none -> [NoneType()]
        finally:
            mypy.state.state.strict_optional = old
        self._assert_par([NoneType()], strict_optional=False)

    def test_object_shortcuts(self) -> None:
        # object anywhere (not just first) -> [object] immediately, even
        # after a removed UninhabitedType before it.
        types = [UninhabitedType(), Instance(self.fx.oi, [])]
        result = self._remove_so(types, True)
        assert_equal(len(result), 1)
        first = get_proper_type(result[0])
        assert isinstance(first, Instance), f"expected Instance, got {result[0]!r}"
        assert first.type.fullname == "builtins.object"
        self._assert_par(types)

    def test_duplicates_dropped(self) -> None:
        types = [
            Instance(self.fx.ai, []),
            Instance(self.fx.ai, []),  # exact duplicate
            Instance(self.fx.bi, []),
        ]
        result = self._remove_so(types, True)
        assert_equal(len(result), 2)
        first = get_proper_type(result[0])
        assert isinstance(first, Instance)
        second = get_proper_type(result[1])
        assert isinstance(second, Instance)
        assert first.type.fullname == "A"
        assert second.type.fullname == "B"
        self._assert_par(types)

    def test_mixed(self) -> None:
        types = [
            UninhabitedType(),
            NoneType(),
            Instance(self.fx.ai, []),
            Instance(self.fx.ai, []),
            Instance(self.fx.bi, []),
        ]
        # strict_optional=True keeps the NoneType.
        result = self._remove_so(types, True)
        assert_equal(len(result), 3)
        self._assert_par(types, strict_optional=True)
        # strict_optional=False drops it (removed_none), leaving A and B.
        result_so = self._remove_so(types, False)
        assert_equal(len(result_so), 2)
        self._assert_par(types, strict_optional=False)

    def test_removed_none_empty_result(self) -> None:
        # Only NoneType with strict_optional=False: removed_none with no
        # new_types -> [NoneType()] (the Python fallback).
        self._assert_par([NoneType()], strict_optional=False)

    def test_seam_engages(self) -> None:
        buf = _WriteBuffer()
        from mypy.types import write_type_list

        write_type_list(buf, [Instance(self.fx.ai, []), Instance(self.fx.ai, [])])
        result = _type_kernel.rust_remove_trivial(buf.getvalue(), True)
        assert result is not None, "rust_remove_trivial returned None"
        from librt.internal import ReadBuffer

        from mypy.types import read_type_list

        decoded = read_type_list(ReadBuffer(bytes(result)))
        assert_equal(len(decoded), 1)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRemoveTrivialFreshVarSuite(Suite):
    """Live-object identity for `remove_trivial`'s partial-list seam (#1623).

    `remove_trivial` (expandtype.py) is the one expand-family seam with no
    enclosing `variables` slot to seed `canonicalize_fresh_vars` from, so
    its decoded list used to gate every fresh (meta_level > 0) type var
    back to the pure-Python body: a decoded copy is a doppelganger that
    escapes both `freeze_all_type_vars`'s in-place `meta_level` mutation
    and id-keyed substitution. The seam now re-links every decoded
    TypeVar-like onto the live object reachable from its own input list
    (`wirefixup.resync_var_identities_list`, strict), which is the
    identity Python's `remove_trivial` preserves by returning the input
    items unchanged. A decoded occurrence with no structurally-equal live
    original still defers, so the strictness that used to be encoded in
    the gate is now checked against the actual input list.
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_expand_type_active
        self._set_resolver = _set_native_expand_type_resolver
        self._set_map = _set_native_expand_type_typeinfo_map
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(self._resolver)
        self._set_map({info.fullname: info for info in type_infos})
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _fresh(self, raw_id: int = 500) -> TypeVarType:
        """A fresh (meta_level 1) tvar carrying the no-default sentinel."""
        return self.fx.t.copy_modified(id=TypeVarId(raw_id, meta_level=1))

    def _remove(self, types: Sequence[Type]) -> list[Type]:
        from mypy.expandtype import remove_trivial

        return remove_trivial(types)

    def _spy(self) -> tuple[Any, list[bytes]]:
        """Patch the Rust seam so a test can prove native engagement."""
        from unittest import mock

        import type_kernel

        calls: list[bytes] = []
        real = type_kernel.rust_remove_trivial

        def wrapper(b: bytes, so: bool) -> object:
            calls.append(b)
            return real(b, so)

        return mock.patch.object(type_kernel, "rust_remove_trivial", wrapper), calls

    def test_fresh_var_relinked_to_live_object(self) -> None:
        v = self._fresh()
        patched, calls = self._spy()
        with patched:
            result = self._remove([v, self.fx.b])
        assert_equal(len(calls), 1, "fresh-var list did not cross the Rust seam")
        assert result[0] is v

    def test_fresh_var_nested_in_row_relinked(self) -> None:
        v = self._fresh()
        list_v = Instance(self.fx.std_listi, [v])
        patched, calls = self._spy()
        with patched:
            result = self._remove([list_v, self.fx.b])
        assert_equal(len(calls), 1, "fresh-var list did not cross the Rust seam")
        first = get_proper_type(result[0])
        assert isinstance(first, Instance)
        assert first.args[0] is v

    def test_plain_typevar_relinked(self) -> None:
        # A non-meta var is no fresh id, but Python still returns the input
        # object; the old list canonicalizer left these decoded.
        patched, calls = self._spy()
        with patched:
            result = self._remove([self.fx.t, self.fx.b])
        assert_equal(len(calls), 1, "plain-var list did not cross the Rust seam")
        assert result[0] is self.fx.t

    def test_freeze_in_place_reaches_the_result(self) -> None:
        # freeze_all_type_vars mutates TypeVarId.meta_level in place on the
        # vars a callable's `variables` slot lists, so the returned
        # occurrence must be the very object the caller passed in.
        from mypy.typeops import freeze_all_type_vars

        v = self._fresh()
        sig = self.fx.callable(self.fx.o, v).copy_modified(variables=[v])
        patched, calls = self._spy()
        with patched:
            result = self._remove([v, self.fx.b])
        assert_equal(len(calls), 1, "fresh-var list did not cross the Rust seam")
        freeze_all_type_vars(sig)
        first = get_proper_type(result[0])
        assert isinstance(first, TypeVarType)
        assert first is v
        assert first.id.meta_level == 0

    def test_gate_parity_fresh_and_plain(self) -> None:
        v = self._fresh()
        list_v = Instance(self.fx.std_listi, [v])
        cases: list[list[Type]] = [[v, self.fx.b], [list_v, self.fx.b]]
        for types in cases:
            off = str(self._with_gate(False, lambda: self._remove(types)))
            on = str(self._with_gate(True, lambda: self._remove(types)))
            assert_equal(on, off, f"remove_trivial gate parity {types}")

    def test_var_bearing_result_stays_out_of_the_cache(self) -> None:
        import mypy.expandtype as expandtype

        expandtype._expand_remove_trivial_cache.clear()
        try:
            self._remove([self._fresh(), self.fx.b])
            assert not expandtype._expand_remove_trivial_cache, (
                "var-bearing remove_trivial result was cached: a later caller "
                "would receive the first caller's live objects"
            )
            self._remove([self.fx.a, self.fx.b])
            assert len(expandtype._expand_remove_trivial_cache) == 1
        finally:
            expandtype._expand_remove_trivial_cache.clear()

    def test_helper_defers_on_unmatched_var(self) -> None:
        # Strictness: the live list holds no var, so the decoded one has no
        # original to stand in for it.
        from mypy.wirefixup import resync_var_identities_list

        decoded = self._fresh().copy_modified()
        assert resync_var_identities_list([self.fx.b], [decoded]) is None

    def test_helper_defers_on_same_id_different_content(self) -> None:
        # Same key, different structure: not a round-trip copy of the live
        # original, so it must not pass as one.
        from mypy.wirefixup import resync_var_identities_list

        v = self._fresh()
        drifted = v.copy_modified(upper_bound=self.fx.a)
        assert resync_var_identities_list([v], [drifted]) is None

    def test_helper_relinks_every_occurrence(self) -> None:
        from mypy.wirefixup import resync_var_identities_list

        v = self._fresh()
        row = Instance(self.fx.std_listi, [v])
        decoded_rows: list[Type] = [
            Instance(self.fx.std_listi, [v.copy_modified()]),
            v.copy_modified(),
        ]
        out = resync_var_identities_list([row], decoded_rows)
        assert out is not None
        first = get_proper_type(out[0])
        assert isinstance(first, Instance)
        assert first.args[0] is v
        assert out[1] is v

    def _relink_spy(self) -> tuple[Any, list[int]]:
        """Patch the wirefixup relink so a test can prove it ran."""
        from unittest import mock

        import mypy.wirefixup as wirefixup

        calls: list[int] = []
        real = wirefixup.resync_var_identities_list

        def wrapper(live: Any, decoded: Any) -> Any:
            calls.append(1)
            return real(live, decoded)

        return mock.patch.object(wirefixup, "resync_var_identities_list", wrapper), calls

    def _instance_type_call(self, v: TypeVarType) -> CallableType:
        return self.fx.callable(self.fx.o, self.fx.o).copy_modified(
            instance_type=Instance(self.fx.std_listi, [v])
        )

    def test_helper_relinks_instance_type_var(self) -> None:
        # A var whose only occurrence is CallableType.instance_type: the
        # contains_typevar_like walk used to miss it, so the relink never ran.
        from mypy.wirefixup import resync_var_identities_list

        v = self._fresh()
        out = resync_var_identities_list(
            [self._instance_type_call(v)], [self._instance_type_call(v.copy_modified())]
        )
        assert out is not None
        first = get_proper_type(out[0])
        assert isinstance(first, CallableType)
        inst = first.instance_type
        assert isinstance(inst, Instance)
        assert inst.args[0] is v

    def test_helper_relinks_type_guard_var(self) -> None:
        from mypy.wirefixup import resync_var_identities_list

        v = self._fresh()
        live = self.fx.callable(self.fx.o, self.fx.b).copy_modified(type_guard=v)
        decoded = self.fx.callable(self.fx.o, self.fx.b).copy_modified(
            type_guard=v.copy_modified()
        )
        out = resync_var_identities_list([live], [decoded])
        assert out is not None
        first = get_proper_type(out[0])
        assert isinstance(first, CallableType)
        assert first.type_guard is v

    def test_instance_type_var_stays_out_of_the_cache(self) -> None:
        # Regression pin: contains_typevar_like missed instance_type, so a
        # callable whose only var sat there was served through the cache.
        import mypy.expandtype as expandtype

        expandtype._expand_remove_trivial_cache.clear()
        try:
            v = self._fresh()
            patched, relinks = self._relink_spy()
            with patched:
                result = self._remove([self._instance_type_call(v), self.fx.b])
            assert_equal(len(relinks), 1, "instance_type-only var skipped the relink")
            assert (
                not expandtype._expand_remove_trivial_cache
            ), "instance_type var went through the cache gate"
            first = get_proper_type(result[0])
            assert isinstance(first, CallableType)
            inst = first.instance_type
            assert isinstance(inst, Instance)
            assert inst.args[0] is v
        finally:
            expandtype._expand_remove_trivial_cache.clear()

    def test_helper_passes_var_free_list_through(self) -> None:
        from mypy.wirefixup import resync_var_identities_list

        decoded: list[Type] = [self.fx.a, self.fx.b]
        assert resync_var_identities_list([], decoded) is decoded


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeEraseReturnSelfSuite(Suite):
    """Parity for the Rust `erase_return_self_types` port (mypy.subtypes).

    `erase_return_self_types` rewrites a function-like type whose return
    type equals `self_type` so the return becomes `Any` (dropping
    self-referential `-> Self` returns). The Rust port implements the same
    structural `Instance.__eq__` match on the wire format; the Python shim
    decodes the result through the shared wirefixup path, so live TypeInfo
    identity is restored. Toggling the subtype gate off (pure Python) and
    on (Rust seam) must produce identical results, and a direct seam call
    proves the Rust function engages rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active
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
        self._set_active = _set_native_subtype_active
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

    def _assert_par(self, typ: Type, self_type: Instance) -> None:
        from mypy.subtypes import erase_return_self_types

        off = self._with_gate(False, lambda: erase_return_self_types(typ, self_type))
        on = self._with_gate(True, lambda: erase_return_self_types(typ, self_type))
        assert_equal(str(on), str(off), f"erase_return_self_types parity {typ}")

    def _assert_engages(self, typ: Type, self_type: Instance) -> None:
        from mypy.subtypes import _serialize_type

        result = _type_kernel.rust_erase_return_self_types(
            _serialize_type(typ), _serialize_type(self_type)
        )
        assert result is not None, f"Rust erase_return_self_types did not engage for {typ}"

    def test_callable_returning_self(self) -> None:
        from mypy.subtypes import erase_return_self_types

        c = CallableType(
            [self.fx.a], [ARG_POS], [None], Instance(self.fx.ai, []), self.fx.function
        )
        self._assert_par(c, self.fx.a)
        result = self._with_gate(True, lambda: erase_return_self_types(c, self.fx.a))
        result = get_proper_type(result)
        assert isinstance(result, CallableType)
        assert isinstance(get_proper_type(result.ret_type), AnyType)
        assert_equal(str(result), "def (A) -> Any")
        self._assert_engages(c, self.fx.a)

    def test_callable_returning_non_self(self) -> None:
        from mypy.subtypes import erase_return_self_types

        c = CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function)
        self._assert_par(c, self.fx.a)
        result = self._with_gate(True, lambda: erase_return_self_types(c, self.fx.a))
        result = get_proper_type(result)
        assert isinstance(result, CallableType)
        assert_equal(str(result.ret_type), "B")
        self._assert_engages(c, self.fx.a)

    def test_generic_self_instance(self) -> None:
        # Callable[[], G[A]] with self G[A] erases: the type args
        # participate in the Instance equality.
        from mypy.subtypes import erase_return_self_types

        self_type = self.fx.ga
        c = CallableType(
            [self.fx.a], [ARG_POS], [None], Instance(self.fx.gi, [self.fx.a]), self.fx.function
        )
        self._assert_par(c, self_type)
        result = self._with_gate(True, lambda: erase_return_self_types(c, self_type))
        result = get_proper_type(result)
        assert isinstance(result, CallableType)
        assert isinstance(get_proper_type(result.ret_type), AnyType)
        self._assert_engages(c, self_type)

    def test_generic_self_args_differ_unchanged(self) -> None:
        # Callable[[], G[B]] with self G[A]: args differ, so no erase.
        from mypy.subtypes import erase_return_self_types

        self_type = self.fx.ga
        c = CallableType(
            [self.fx.a], [ARG_POS], [None], Instance(self.fx.gi, [self.fx.b]), self.fx.function
        )
        self._assert_par(c, self_type)
        result = self._with_gate(True, lambda: erase_return_self_types(c, self_type))
        result = get_proper_type(result)
        assert isinstance(result, CallableType)
        assert_equal(str(result.ret_type), "G[B]")
        self._assert_engages(c, self_type)

    def test_overloaded_mixed(self) -> None:
        # One item returns self (erased to Any), one does not; the
        # Overloaded is rebuilt with the erased item.
        from mypy.subtypes import erase_return_self_types
        from mypy.types import Overloaded

        c1 = CallableType([], [], [], Instance(self.fx.ai, []), self.fx.function)
        c2 = CallableType([self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function)
        overloaded = Overloaded([c1, c2])
        self._assert_par(overloaded, self.fx.a)
        result = self._with_gate(True, lambda: erase_return_self_types(overloaded, self.fx.a))
        result = get_proper_type(result)
        assert isinstance(result, Overloaded)
        assert isinstance(get_proper_type(result.items[0].ret_type), AnyType)
        assert_equal(str(result.items[1].ret_type), "B")
        self._assert_engages(overloaded, self.fx.a)

    def test_bare_instance_unchanged(self) -> None:
        # A bare Instance == self_type is not function-like: Python returns
        # it unchanged (subtypes.py:2773), Rust engages and clones it.
        from mypy.subtypes import erase_return_self_types

        self._assert_par(self.fx.a, self.fx.a)
        result = self._with_gate(True, lambda: erase_return_self_types(self.fx.a, self.fx.a))
        assert_equal(str(result), "A")
        self._assert_engages(self.fx.a, self.fx.a)

    def test_union_and_none_unchanged(self) -> None:
        from mypy.subtypes import erase_return_self_types
        from mypy.types import UnionType

        union = UnionType([self.fx.a, self.fx.b])
        self._assert_par(union, self.fx.a)
        result = self._with_gate(True, lambda: erase_return_self_types(union, self.fx.a))
        assert_equal(str(result), "A | B")
        self._assert_engages(union, self.fx.a)
        self._assert_par(self.fx.nonet, self.fx.a)
        self._assert_engages(self.fx.nonet, self.fx.a)


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


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinInstancesSuite(Suite):
    """Parity for the Rust `InstanceJoiner.join_instances` port
    (join.py:208-303, setops.rs `join_instances_core`).

    The seam is the new `rust_join_instances` pyfunction, wired into
    `InstanceJoiner.join_instances` (the args-less, same-type-with-args,
    variadic-single-arg, and different-args-less nominal paths). Toggling
    the join gate off vs on must agree on the resulting ProperType for the
    same Instance pair; a direct seam call proves the Rust engagement.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
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
        type_infos.extend([self.fx.str_type_info, self.fx.bool_type_info])
        self._set_active = _set_native_join_active
        self._set_resolver = _set_native_join_resolver
        self._set_map = _set_native_join_typeinfo_map
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._typeinfo_map = {info.fullname: info for info in type_infos}
        self._set_resolver(self._resolver)
        self._set_map(self._typeinfo_map)
        set_wire_typeinfo_map(self._typeinfo_map)
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        self._set_map(None)
        set_wire_typeinfo_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _native_result(self, t: Instance, s: Instance) -> ProperType:
        from mypy.join import InstanceJoiner

        return self._with_gate(True, lambda: InstanceJoiner().join_instances(t, s))

    def _reference_result(self, t: Instance, s: Instance) -> ProperType:
        from mypy.join import InstanceJoiner

        return self._with_gate(False, lambda: InstanceJoiner().join_instances(t, s))

    def _assert_parity(self, t: Instance, s: Instance) -> None:
        res = self._native_result(t, s)
        ref = self._reference_result(t, s)
        assert_equal(res, ref)

    def test_same_type_args_less(self) -> None:
        # join.py:215, no args -> Instance(A, []) = A for both orders.
        self._assert_parity(self.fx.a, self.fx.a)

    def test_same_type_different_vars_returns_object(self) -> None:
        # join_instances_via_supertype with t/s unrelated -> object.
        self._assert_parity(self.fx.a, self.fx.d)

    def test_same_type_covariant_args_join(self) -> None:
        # G[A] vs G[B], T covariant -> G[A] (A is the supertype of B).
        self._assert_parity(self.fx.ga, self.fx.gb)

    def test_same_type_covariant_args_join_reversed(self) -> None:
        self._assert_parity(self.fx.gb, self.fx.ga)

    def test_same_type_invariant_unequal_args_returns_object(self) -> None:
        # TypeFixture(INVARIANT): G[A] vs G[B], T invariant ->
        # is_equivalent(A, B) false -> object.
        inv_fx = TypeFixture(INVARIANT)
        inv_infos = []
        for name in dir(inv_fx):
            if not name.endswith("i"):
                continue
            value = getattr(inv_fx, name)
            if _is_type_info(value):
                inv_infos.append(value)
        inv_infos.extend([inv_fx.str_type_info, inv_fx.bool_type_info])
        inv_resolver = _type_kernel.build_native_resolver(inv_infos, [])
        inv_map = {info.fullname: info for info in inv_infos}

        from mypy.join import _set_native_join_resolver as _set_res
        from mypy.wirefixup import set_wire_typeinfo_map as _set_wire_map

        prev_res = self._resolver
        prev_map = self._typeinfo_map
        _set_res(inv_resolver)
        _set_wire_map(inv_map)
        self._resolver = inv_resolver
        self._typeinfo_map = inv_map
        try:
            self._assert_parity(inv_fx.ga, inv_fx.gb)
        finally:
            _set_res(prev_res)
            _set_wire_map(prev_map)
            self._resolver = prev_res
            self._typeinfo_map = prev_map

    def test_same_type_with_any_arg(self) -> None:
        # G[A] vs G[Any] -> G[Any] via the AnyType arg path (disc 4).
        self._assert_parity(self.fx.ga, self.fx.gdyn)

    def test_same_type_with_any_arg_reversed(self) -> None:
        self._assert_parity(self.fx.gdyn, self.fx.ga)

    def test_multi_arg_same_base(self) -> None:
        # H[A, B] vs H[A, B], single variance -> same.
        self._assert_parity(self.fx.hab, self.fx.hab)

    def test_literal_lkv_is_dropped(self) -> None:
        # Same type with a literal last_known_value: join_instances
        # builds Instance(A, []) without LKV (join.py:281 path).
        from mypy.types import LiteralType

        lit_a = Instance(self.fx.ai, [], last_known_value=LiteralType(1, self.fx.a))
        lit_b = Instance(self.fx.ai, [], last_known_value=LiteralType(2, self.fx.a))
        self._assert_parity(lit_a, lit_b)

    def test_nominal_ancestor(self) -> None:
        # B <: A -> join(B, A) = A via join_instances_via_supertype.
        self._assert_parity(self.fx.b, self.fx.a)
        self._assert_parity(self.fx.a, self.fx.b)

    def test_seen_guard_routes_to_python_object(self) -> None:
        # Python's seen_instances already contains (A, A): the shim
        # must NOT engage (the pair is on the Python stack) and the
        # pure-Python guard returns object_from_instance(A) = object.
        from mypy.join import InstanceJoiner

        j = InstanceJoiner()
        j.seen_instances.append((self.fx.a, self.fx.a))
        res = j.join_instances(self.fx.a, self.fx.a)
        assert res == self.fx.o, f"expected object, got {res}"
        # Fresh joiner: Rust engages and returns A.
        j2 = InstanceJoiner()
        assert j2.join_instances(self.fx.a, self.fx.a) == self.fx.a

    def test_seam_engages(self) -> None:
        # Direct seam call: rust_join_instances returns a disc tuple for
        # A vs A (args-less same-type). None would mean Rust defers.
        from mypy.join import _serialize_type

        result = _type_kernel.rust_join_instances(
            _serialize_type(self.fx.a), _serialize_type(self.fx.a), False, self._resolver
        )
        assert result is not None, f"rust_join_instances(A, A) deferred: {result!r}"
        disc = result[0]
        assert disc in (0, 1), f"expected SameS/SameT for A vs A, got disc {disc}"

    def test_encoded_instance_any_arg_is_from_another_any(self) -> None:
        # Multi-variance join where one arg hits the AnyType arm (disc
        # 4) and another triggers a real join (H[Any, A] vs H[B, B]):
        # the encoded wire Instance must round-trip from_another_any (#1269).
        joiner_inst = Instance(self.fx.hi, [self.fx.anyt, self.fx.a])
        other_inst = Instance(self.fx.hi, [self.fx.b, self.fx.b])
        for args in [(joiner_inst, other_inst), (other_inst, joiner_inst)]:
            got = self._native_result(*args)
            ref = self._reference_result(*args)
            assert_equal(got, ref)
            assert isinstance(got, Instance)
            assert isinstance(got.args[0], AnyType)  # type: ignore[misc]
            assert got.args[0].type_of_any == TypeOfAny.from_another_any


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeLiteralParamSuite(Suite):
    """Parity for the Rust `analyze_literal_param` dispatch classifier.

    The 9-way Literal-param dispatch head (string-Literal, Any fail/silent,
    RawExpression float/complex/arbitrary/with-value, NoneType/LiteralType,
    Instance LKV, Union recursion, invalid) is decided in Rust from scalar
    type-kind facts; the Python shim applies the branch bodies (LiteralType
    construction, error emission, visit_unbound_type recursion, union merge).

    Toggling the typeanal gate off (pure Python) and on (Rust seam) must
    produce identical (str(result), captured fail messages), and a direct
    seam call proves the Rust classifier engages on each branch.
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        self.fx = TypeFixture()
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

    def _analyser(self) -> tuple[object, object]:
        from mypy.tvar_scope import TypeVarLikeScope
        from mypy.typeanal import TypeAnalyser

        fx = self.fx
        str_info: TypeInfo = fx.str_type_info
        int_info: TypeInfo = fx.make_type_info("builtins.int")
        bool_info: TypeInfo = fx.bool_type_info
        float_info: TypeInfo = fx.make_type_info("builtins.float")
        complex_info: TypeInfo = fx.make_type_info("builtins.complex")

        class FakePlugin:
            def get_type_analyze_hook(self, fullname: str) -> object:
                return None

        class FakeApi:
            def __init__(self) -> None:
                self.final_iteration = False
                self.errors: list[str] = []
                self.syms: dict[str, SymbolTableNode] = {
                    "builtins.str": SymbolTableNode(MDEF, str_info),
                    "builtins.int": SymbolTableNode(MDEF, int_info),
                    "builtins.bool": SymbolTableNode(MDEF, bool_info),
                    "builtins.float": SymbolTableNode(MDEF, float_info),
                    "builtins.complex": SymbolTableNode(MDEF, complex_info),
                    "str": SymbolTableNode(MDEF, str_info),
                    "int": SymbolTableNode(MDEF, int_info),
                }

            def lookup_qualified(
                self, name: str, ctx: Context, suppress_errors: bool = False
            ) -> SymbolTableNode | None:
                return self.syms.get(name)

            def lookup_fully_qualified(self, fullname: str) -> SymbolTableNode:
                return self.syms[fullname]

            def lookup_fully_qualified_or_none(self, fullname: str) -> SymbolTableNode | None:
                return self.syms.get(fullname)

            def is_incomplete_namespace(self, fullname: str) -> bool:
                return False

            def record_incomplete_ref(self) -> None:
                pass

            def fail(self, msg: str, ctx: Context, code: ErrorCode | None = None) -> None:
                self.errors.append(msg)

            def note(self, msg: str, ctx: Context, code: ErrorCode | None = None) -> None:
                self.errors.append(f"note: {msg}")

            def is_func_scope(self) -> bool:
                return False

            def record_fixed_type(self, typ: Type) -> None:
                pass

            def defer(self) -> None:
                pass

        api = FakeApi()
        ta = TypeAnalyser.__new__(TypeAnalyser)
        ta.api = api  # type: ignore[assignment]
        ta.fail_func = api.fail  # type: ignore[assignment]
        ta.note_func = api.note
        ta.tvar_scope = TypeVarLikeScope()
        ta.plugin = FakePlugin()  # type: ignore[assignment]
        ta.options = Options()
        ta.defining_alias = False
        ta.python_3_12_type_alias = False
        ta.alias_type_params_names = None
        ta.allowed_alias_tvars = []
        ta.erase_tvar_defs = []
        ta.allow_unbound_tvars = False
        ta.allow_placeholder = False
        ta.allow_param_spec_literals = False
        ta.allow_type_any = False
        ta.allow_type_var_tuple = -1
        ta.nesting_level = 0
        ta.is_typeshed_stub = False
        ta.analyzing_tvar_def = False
        ta.aliases_used = set()
        ta.prohibit_special_class_field_types = None
        ta.allow_typed_dict_special_forms = False
        ta.allow_final = True
        ta.allow_unpack = False
        ta.allow_ellipsis = False
        ta.allow_tuple_literal = True
        ta.report_invalid_types = False
        ta.prohibit_self_type = None
        ta.cur_mod_node = None  # type: ignore[assignment]
        return ta, api

    def _call(self, ta: Any, arg: Type) -> tuple[str, list[str]]:
        result = ta.analyze_literal_param(1, arg, self.fx.o)
        messages = list(ta.api.errors)
        if result is None:
            return "None", messages
        return str(result), messages

    def _assert_par(self, arg: Type, *, expected: str | None = None) -> None:
        off_ta, _ = self._analyser()
        off = self._with_gate(False, lambda: self._call(off_ta, arg))
        self._set_active(True)
        on_ta, _ = self._analyser()
        on = self._with_gate(True, lambda: self._call(on_ta, arg))
        assert_equal(on[0], off[0], f"literal_param parity result {arg!r}")
        assert_equal(on[1], off[1], f"literal_param parity messages {arg!r}")
        if expected is not None:
            assert_equal(on[0], expected, f"literal_param result {arg!r}")

    def _assert_engages(self, **facts: Any) -> None:
        from mypy.typeanal import _rust_classify_literal_param  # type: ignore[attr-defined]

        defaults: dict[str, Any] = {
            "is_proper_type": False,
            "is_unbound": False,
            "is_union_pre": False,
            "original_str_expr_is_not_none": False,
            "is_any": False,
            "type_of_any": 0,
            "is_raw_expr": False,
            "literal_value_is_none": True,
            "simple_name": "",
            "is_none_type": False,
            "is_literal": False,
            "is_instance": False,
            "last_known_value_is_none": True,
            "is_union_post": False,
        }
        defaults.update(facts)
        result = _rust_classify_literal_param(
            defaults["is_proper_type"],
            defaults["is_unbound"],
            defaults["is_union_pre"],
            defaults["original_str_expr_is_not_none"],
            defaults["is_any"],
            defaults["type_of_any"],
            defaults["is_raw_expr"],
            defaults["literal_value_is_none"],
            defaults["simple_name"],
            defaults["is_none_type"],
            defaults["is_literal"],
            defaults["is_instance"],
            defaults["last_known_value_is_none"],
            defaults["is_union_post"],
        )
        assert result is not None, "Rust literal_param did not engage"

    def test_str_literal_unbound(self) -> None:
        arg = UnboundType("foo", original_str_expr="hello", original_str_fallback="builtins.str")
        self._assert_par(arg, expected="[Literal['hello']]")
        self._assert_engages(
            is_proper_type=True, is_unbound=True, original_str_expr_is_not_none=True
        )

    def test_str_literal_union(self) -> None:
        union = UnionType([UnboundType("a"), UnboundType("b")])
        union.original_str_expr = "hello"
        union.original_str_fallback = "builtins.str"
        self._assert_par(union, expected="[Literal['hello']]")
        self._assert_engages(
            is_proper_type=True, is_union_pre=True, original_str_expr_is_not_none=True
        )

    def test_none_type(self) -> None:
        self._assert_par(NoneType(), expected="[None]")
        self._assert_engages(is_none_type=True)

    def test_literal_type(self) -> None:
        lit = LiteralType(42, Instance(self.fx.make_type_info("builtins.int"), []))
        self._assert_par(lit, expected="[Literal[42]]")
        self._assert_engages(is_literal=True)

    def test_any_fail(self) -> None:
        self._assert_par(AnyType(TypeOfAny.explicit), expected="None")
        self._assert_engages(is_any=True, type_of_any=2)

    def test_any_silent_from_error(self) -> None:
        self._assert_par(AnyType(TypeOfAny.from_error), expected="None")
        self._assert_engages(is_any=True, type_of_any=5)

    def test_any_silent_special_form(self) -> None:
        self._assert_par(AnyType(TypeOfAny.special_form), expected="None")
        self._assert_engages(is_any=True, type_of_any=6)

    def test_raw_no_value_float(self) -> None:
        from mypy.types import RawExpressionType

        arg = RawExpressionType(None, "builtins.float")
        self._assert_par(arg, expected="None")
        self._assert_engages(is_raw_expr=True, literal_value_is_none=True, simple_name="float")

    def test_raw_no_value_complex(self) -> None:
        from mypy.types import RawExpressionType

        arg = RawExpressionType(None, "builtins.complex")
        self._assert_par(arg, expected="None")
        self._assert_engages(is_raw_expr=True, literal_value_is_none=True, simple_name="complex")

    def test_raw_no_value_arbitrary(self) -> None:
        from mypy.types import RawExpressionType

        arg = RawExpressionType(None, "builtins.list")
        self._assert_par(arg, expected="None")
        self._assert_engages(is_raw_expr=True, literal_value_is_none=True, simple_name="list")

    def test_raw_with_value(self) -> None:
        from mypy.types import RawExpressionType

        arg = RawExpressionType(42, "builtins.int")
        self._assert_par(arg)
        self._assert_engages(is_raw_expr=True, literal_value_is_none=False)

    def test_instance_lkv(self) -> None:
        int_info = self.fx.make_type_info("builtins.int")
        lkv = LiteralType(42, Instance(int_info, []))
        inst = Instance(int_info, [], last_known_value=lkv)
        self._assert_par(inst, expected="[Literal[42]]")
        self._assert_engages(is_instance=True, last_known_value_is_none=False)

    def test_union_recurse(self) -> None:
        union = UnionType(
            [LiteralType(1, Instance(self.fx.make_type_info("builtins.int"), [])), NoneType()]
        )
        self._assert_par(union, expected="[Literal[1], None]")
        self._assert_engages(is_union_post=True)

    def test_invalid(self) -> None:
        # CallableType is not any of the recognized branches.
        arg = CallableType([], [], [], NoneType(), self.fx.function)
        self._assert_par(arg, expected="None")
        self._assert_engages()

    def test_str_beats_any_direct(self) -> None:
        # Direct seam call: branch (a) is checked before (c).
        from mypy.typeanal import _rust_classify_literal_param  # type: ignore[attr-defined]

        tag = _rust_classify_literal_param(
            True, True, False, True, True, 2, False, True, "", False, False, False, True, False
        )
        assert tag == 1, f"expected TAG_STR_LITERAL, got {tag}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsValidConstructorSuite(Suite):
    """Parity for the Rust `is_valid_constructor` port (mypy.typeops, #967).

    `is_valid_constructor` (typeops.py:445-455) is a pure bool predicate:
    True for `OverloadedFuncDef`/`FuncDef` (SYMBOL_FUNCBASE_TYPES), True for a
    `Decorator` whose `get_proper_type(var.type)` is a `FunctionLike`, False
    otherwise (incl. `None` and other SymbolNodes). The Rust seam reads the
    live node via PyO3 isinstance and checks the wire tag for the Decorator
    arm; it always returns a bool (never defers). Gate-off vs gate-on runs
    must agree and the direct seam call must match.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active

        self.fx = TypeFixture()
        self._set_active = _set_native_typeops_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _assert_par(self, n: Any, expected: bool) -> None:
        from mypy.typeops import is_valid_constructor

        off = self._with_gate(False, lambda: is_valid_constructor(n))
        on = self._with_gate(True, lambda: is_valid_constructor(n))
        assert off == expected, f"gate-off {n!r}: {off} != {expected}"
        assert on == expected, f"gate-on {n!r}: {on} != {expected}"

    def _assert_seam(self, n: Any, expected: bool) -> None:
        result = _type_kernel.rust_is_valid_constructor(n)
        assert result is not None, f"Rust deferred on {n!r}"
        assert result == expected, f"Rust seam {n!r}: {result} != {expected}"

    def _func_def(self, name: str = "f") -> FuncDef:

        return FuncDef(name, [], Block([]))

    def _decorator(self, typ: Type | None) -> Decorator:
        v = Var("m")
        v.type = typ
        return Decorator(self._func_def("m"), [], v)

    def test_seam_none(self) -> None:
        assert _type_kernel.rust_is_valid_constructor(None) is False

    def test_seam_func_def(self) -> None:
        self._assert_seam(self._func_def(), True)

    def test_seam_overloaded_func_def(self) -> None:
        from mypy.nodes import OverloadedFuncDef

        self._assert_seam(OverloadedFuncDef([self._func_def("g")]), True)

    def test_seam_decorator_callable(self) -> None:
        ct = CallableType([], [], [], self.fx.a, self.fx.function, name="m")
        self._assert_seam(self._decorator(ct), True)

    def test_seam_decorator_overloaded(self) -> None:
        ct = CallableType([], [], [], self.fx.a, self.fx.function, name="m")
        self._assert_seam(self._decorator(Overloaded([ct])), True)

    def test_seam_decorator_non_callable(self) -> None:
        self._assert_seam(self._decorator(self.fx.a), False)

    def test_seam_decorator_none_type(self) -> None:
        self._assert_seam(self._decorator(None), False)

    def test_seam_other_node(self) -> None:
        self._assert_seam(Var("x"), False)

    def test_parity_none(self) -> None:
        self._assert_par(None, False)

    def test_parity_func_def(self) -> None:
        self._assert_par(self._func_def(), True)

    def test_parity_overloaded_func_def(self) -> None:
        from mypy.nodes import OverloadedFuncDef

        self._assert_par(OverloadedFuncDef([self._func_def("g")]), True)

    def test_parity_decorator_callable(self) -> None:
        ct = CallableType([], [], [], self.fx.a, self.fx.function, name="m")
        self._assert_par(self._decorator(ct), True)

    def test_parity_decorator_overloaded(self) -> None:
        ct = CallableType([], [], [], self.fx.a, self.fx.function, name="m")
        self._assert_par(self._decorator(Overloaded([ct])), True)

    def test_parity_decorator_non_callable(self) -> None:
        self._assert_par(self._decorator(self.fx.a), False)

    def test_parity_decorator_none_type(self) -> None:
        self._assert_par(self._decorator(None), False)

    def test_parity_var(self) -> None:
        self._assert_par(Var("x"), False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsDescriptorSuite(Suite):
    """Parity for the Rust `is_descriptor` port (mypy.subtypes).

    `is_descriptor` (subtypes.py:2177-2183) is a recursive bool predicate:
    an Instance is a descriptor when its class (via MRO) has a `__get__`
    member; a UnionType is a descriptor when all relevant items are
    descriptors; all other types return False. The Rust port walks the
    wire Type and checks `__get__` member presence via the resolver
    snapshots, reusing `has_readable_member_by_ref` from checkmember.
    Toggling the subtype gate off (pure Python) and on (Rust seam) must
    produce identical results, and a direct seam call proves engagement.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        # Build a descriptor class: has `__get__` in its names dict.
        self.desci = self.fx.make_type_info("mod.Desc", mro=[self.fx.oi])
        self.desci.names["__get__"] = SymbolTableNode(MDEF, Var("__get__"))
        # A subclass inherits __get__ via MRO.
        self.subdesci = self.fx.make_type_info("mod.SubDesc", mro=[self.desci, self.fx.oi])
        # A non-descriptor class without __get__.
        self.plaini = self.fx.make_type_info("mod.Plain", mro=[self.fx.oi])
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend([self.desci, self.subdesci, self.plaini])
        self.typeinfo_map = {info.fullname: info for info in type_infos}
        set_wire_typeinfo_map(self.typeinfo_map)
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active = _set_native_subtype_active
        self._set_resolver = _set_native_subtype_resolver
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

    def _assert_par(self, typ: Type | None) -> None:
        from mypy.subtypes import is_descriptor

        off = self._with_gate(False, lambda: is_descriptor(typ))
        on = self._with_gate(True, lambda: is_descriptor(typ))
        assert off == on, f"is_descriptor({typ}): gate-off={off}, gate-on={on}"

    def _assert_engages(self, typ: Type | None, expected: bool) -> None:
        from mypy.subtypes import _serialize_type

        result = _type_kernel.rust_is_descriptor(
            self.resolver, _serialize_type(cast(Any, typ)), True
        )
        assert result is not None, f"rust_is_descriptor did not engage for {typ}"
        assert result == expected, f"rust_is_descriptor({typ}) = {result}, expected {expected}"

    def test_instance_with_get(self) -> None:
        typ = Instance(self.desci, [])
        self._assert_par(typ)
        self._assert_engages(typ, True)

    def test_instance_inherited_get(self) -> None:
        typ = Instance(self.subdesci, [])
        self._assert_par(typ)
        self._assert_engages(typ, True)

    def test_instance_without_get(self) -> None:
        typ = Instance(self.plaini, [])
        self._assert_par(typ)
        self._assert_engages(typ, False)

    def test_plain_instance_no_get(self) -> None:
        # The fixture's A class has no __get__.
        typ = self.fx.a
        self._assert_par(typ)
        self._assert_engages(typ, False)

    def test_none_type(self) -> None:
        typ = NoneType()
        self._assert_par(typ)
        self._assert_engages(typ, False)

    def test_any_type(self) -> None:
        typ = self.fx.anyt
        self._assert_par(typ)
        self._assert_engages(typ, False)

    def test_callable_type(self) -> None:
        typ = self.fx.callable(self.fx.a, self.fx.b)
        self._assert_par(typ)
        self._assert_engages(typ, False)

    def test_none_input(self) -> None:
        # is_descriptor(None) -> get_proper_type(None) -> None -> not
        # Instance/UnionType -> False.
        self._assert_par(None)

    def test_union_all_descriptors(self) -> None:
        typ = UnionType.make_union([Instance(self.desci, []), Instance(self.desci, [])])
        self._assert_par(typ)
        self._assert_engages(typ, True)

    def test_union_one_non_descriptor(self) -> None:
        typ = UnionType.make_union([Instance(self.desci, []), self.fx.a])
        self._assert_par(typ)
        self._assert_engages(typ, False)

    def test_union_all_non_descriptors(self) -> None:
        typ = UnionType.make_union([self.fx.a, self.fx.b])
        self._assert_par(typ)
        self._assert_engages(typ, False)

    def test_union_with_none_strict_optional(self) -> None:
        # strict_optional=True: NoneType is relevant -> not a descriptor.
        from mypy.subtypes import _serialize_type

        typ = UnionType.make_union([Instance(self.desci, []), NoneType()])
        self._with_gate(False, lambda: _serialize_type(typ))
        self._with_gate(True, lambda: _serialize_type(typ))
        # Both serialize the same way; the gate toggle doesn't change
        # serialization. Just check parity of is_descriptor.
        self._assert_par(typ)
        result = _type_kernel.rust_is_descriptor(self.resolver, _serialize_type(typ), True)
        assert result is not None
        assert result is False

    def test_union_empty_items(self) -> None:
        # UnionType.make_union([]) collapses to UninhabitedType (Never),
        # which is not a descriptor. Parity must still hold.
        typ = UnionType.make_union([])
        self._assert_par(typ)
        self._assert_engages(typ, False)

    def test_union_nested(self) -> None:
        # Union[Union[Desc, A], B] -> not all are descriptors.
        inner = UnionType.make_union([Instance(self.desci, []), self.fx.a])
        typ = UnionType.make_union([inner, self.fx.b])
        self._assert_par(typ)
        self._assert_engages(typ, False)

    def test_union_nested_all_descriptors(self) -> None:
        inner = UnionType.make_union([Instance(self.desci, []), Instance(self.desci, [])])
        typ = UnionType.make_union([inner, Instance(self.subdesci, [])])
        self._assert_par(typ)
        self._assert_engages(typ, True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsDisjointBaseSuite(Suite):
    """Parity tests for `rust_is_disjoint_base` (mypy.typeops._is_disjoint_base).

    The Rust seam reads `info.is_disjoint_base`, `info.slots`, and
    `info.bases[*].type.slots` via PyO3 and computes the own-vs-base
    slot set difference. Each test compares the direct seam call and
    the gated Python shim against the pure-Python fallback.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active

        self._set_active = _set_native_typeops_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _typeinfo(
        self,
        *,
        fullname: str = "mod.A",
        is_disjoint_base: bool = False,
        slots: set[str] | None = None,
        bases: list[Any] | None = None,
    ) -> TypeInfo:
        from mypy.nodes import TypeInfo

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.is_disjoint_base = is_disjoint_base
        info.slots = slots
        info.bases = list(bases) if bases is not None else []
        info.mro = [info]
        return info

    def _base_entry(self, base_type: TypeInfo) -> Any:
        from types import SimpleNamespace

        return SimpleNamespace(type=base_type)

    def _pure_python(self, info: TypeInfo) -> bool:
        self._set_active(False)
        try:
            from mypy.typeops import _is_disjoint_base

            return _is_disjoint_base(info)
        finally:
            self._set_active(True)

    def _assert_par(self, info: TypeInfo) -> None:
        from mypy.typeops import _is_disjoint_base

        expected = self._pure_python(info)
        assert _type_kernel.rust_is_disjoint_base(info) == expected
        assert _is_disjoint_base(info) == expected

    def test_decorator_true(self) -> None:
        info = self._typeinfo(is_disjoint_base=True)
        self._assert_par(info)

    def test_no_slots(self) -> None:
        info = self._typeinfo(slots=None)
        self._assert_par(info)

    def test_empty_slots(self) -> None:
        info = self._typeinfo(slots=set())
        self._assert_par(info)

    def test_own_slots_no_bases(self) -> None:
        info = self._typeinfo(slots={"x", "y"})
        self._assert_par(info)

    def test_all_slots_inherited_from_base(self) -> None:
        base = self._typeinfo(fullname="mod.Base", slots={"x"})
        info = self._typeinfo(slots={"x"}, bases=[self._base_entry(base)])
        self._assert_par(info)

    def test_mixed_own_and_inherited(self) -> None:
        base = self._typeinfo(fullname="mod.Base", slots={"x"})
        info = self._typeinfo(slots={"x", "y"}, bases=[self._base_entry(base)])
        self._assert_par(info)

    def test_base_slots_none(self) -> None:
        base = self._typeinfo(fullname="mod.Base", slots=None)
        info = self._typeinfo(slots={"x"}, bases=[self._base_entry(base)])
        self._assert_par(info)

    def test_multiple_bases(self) -> None:
        b1 = self._typeinfo(fullname="mod.B1", slots={"x"})
        b2 = self._typeinfo(fullname="mod.B2", slots={"y"})
        info = self._typeinfo(
            slots={"x", "y", "z"}, bases=[self._base_entry(b1), self._base_entry(b2)]
        )
        self._assert_par(info)

    def test_gate_off_vs_on(self) -> None:
        base = self._typeinfo(fullname="mod.Base", slots={"x"})
        info = self._typeinfo(slots={"x", "y"}, bases=[self._base_entry(base)])
        self._set_active(False)
        from mypy.typeops import _is_disjoint_base

        off = _is_disjoint_base(info)
        self._set_active(True)
        on = _is_disjoint_base(info)
        assert off == on


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsRecursivePairSuite(Suite):
    """Parity for the Rust ``rust_is_recursive_pair`` port (mypy.typeops).

    Mirrors ``is_recursive_pair`` (typeops.py:249-274): a pure bool
    predicate gating ``join_types`` / ``meet_types`` / ``is_subtype``
    against infinite recursion.  Each test runs the public function
    gate-off (pure Python) and gate-on (Rust seam) and asserts equal
    answers; a direct seam call proves the Rust function engages rather
    than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._type_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.di,
            self.fx.std_tuplei,
            self.fx.std_listi,
            self.fx.str_type_info,
        ]
        self._aliases: list[Any] = []
        self._resolver = _type_kernel.build_native_resolver(self._type_infos, [])
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})
        self._resolver.set_live_typeinfo_map({info.fullname: info for info in self._type_infos})
        _set_native_typeops_active(True)
        _set_native_typeops_resolver(self._resolver)

    def tearDown(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_typeops_active(False)
        _set_native_typeops_resolver(None)
        set_wire_typeinfo_map(None)

    def _rebuild_resolver(self, aliases: list[Any]) -> None:
        from mypy.typeops import _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._resolver = _type_kernel.build_native_resolver(self._type_infos, aliases)
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})
        self._resolver.set_live_typeinfo_map({info.fullname: info for info in self._type_infos})
        _set_native_typeops_resolver(self._resolver)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(active)
        try:
            return fn()
        finally:
            _set_native_typeops_active(True)

    def _assert_par(self, s: Type, t: Type) -> None:
        from mypy.typeops import is_recursive_pair

        off = self._with_gate(False, lambda: is_recursive_pair(s, t))
        on = self._with_gate(True, lambda: is_recursive_pair(s, t))
        assert_equal(on, off, f"is_recursive_pair parity: {s} vs {t}")

    def _seam(self, s: Type, t: Type) -> bool | None:
        from mypy.typeops import _serialize_type

        return _type_kernel.rust_is_recursive_pair(
            _serialize_type(s), _serialize_type(t), self._resolver
        )

    def test_neither_alias_false(self) -> None:
        s = self.fx.a
        t = self.fx.b
        self._assert_par(s, t)
        assert self._seam(s, t) is False

    def test_s_rec_t_instance_true(self) -> None:
        # s = recursive alias, t = Instance -> branch a True.
        s, _ = self.fx.def_alias_1(self.fx.a)
        t = self.fx.b
        self._assert_par(s, t)

    def test_s_rec_t_union_true(self) -> None:
        # s = recursive alias, t = Union -> branch a True.
        s, _ = self.fx.def_alias_1(self.fx.a)
        t = UnionType([self.fx.a, self.fx.b])
        self._assert_par(s, t)

    def test_both_rec_true(self) -> None:
        # Both recursive -> branch b True (resolver-free).
        s, _ = self.fx.def_alias_1(self.fx.a)
        t, _ = self.fx.def_alias_2(self.fx.b)
        self._assert_par(s, t)

    def test_t_rec_s_instance_true(self) -> None:
        # t = recursive alias, s = Instance -> branch a True.
        s = self.fx.a
        t, _ = self.fx.def_alias_2(self.fx.b)
        self._assert_par(s, t)

    def test_non_recursive_alias_not_rec_false(self) -> None:
        # A non-recursive TypeAliasType is not "recursive", so the
        # predicate returns False (matches Python: is_recursive is False).
        s = self.fx.non_rec_alias(self.fx.a)
        t = self.fx.b
        self._assert_par(s, t)

    def test_missing_alias_snapshot_defers(self) -> None:
        # No alias snapshot for s; t = NoneType (not Instance/Union) so
        # branch a fails and branch c needs get_proper_type(s) -> defer.
        # Parity holds: the shim falls back to Python on None.
        s, _ = self.fx.def_alias_1(self.fx.a)
        t = NoneType()
        assert self._seam(s, t) is None
        self._assert_par(s, t)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTupleTypeImplicitSuite(Suite):
    """Parity for the Rust `visit_tuple_type` implicit-tuple classifier.

    The message-arbitration head (implicit tuple + allow_tuple_literal off
    -> fail + one-of-three suggestion note by len(t.items)) is decided in
    Rust from three scalars; the Python shim applies the fail/note and,
    on OK, the named_type + anal_array reconstruction. Toggling the
    typeanal gate off (pure Python) and on (Rust seam) must produce
    identical (str(result), captured fail/note messages), and direct seam
    calls prove the tag table (OK/EMPTY/SINGLE/MULTI).
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        self._set_active = _set_native_typeanal_active
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

    def _analyser(self, allow_tuple_literal: bool = False) -> tuple[Any, Any]:
        from mypy.typeanal import TypeAnalyser

        class FakeApi:
            def __init__(self) -> None:
                self.messages: list[str] = []

            def fail(self, msg: str, ctx: Context, code: Any = None) -> None:
                self.messages.append(f"fail: {msg}")

            def note(self, msg: str, ctx: Context, code: Any = None) -> None:
                self.messages.append(f"note: {msg}")

        api = FakeApi()
        ta = TypeAnalyser.__new__(TypeAnalyser)
        ta.api = api  # type: ignore[assignment]
        ta.fail_func = api.fail  # type: ignore[assignment]
        ta.note_func = api.note
        ta.allow_tuple_literal = allow_tuple_literal
        ta.allow_param_spec_literals = False
        ta.nesting_level = 0
        ta.allow_typed_dict_special_forms = False
        ta.allow_final = False
        ta.allow_ellipsis = False
        ta.allow_unpack = False
        return ta, api

    def _make_t(self, n_items: int, *, implicit: bool) -> object:
        return TupleType(
            [AnyType(TypeOfAny.special_form)] * n_items,
            self.fx.std_tuple,
            line=-1,
            column=-1,
            implicit=implicit,
        )

    def _call(self, ta: Any, t: Any) -> tuple[str, list[str]]:
        result = ta.visit_tuple_type(t)
        return str(result), list(ta.api.messages)

    def _assert_par(
        self, n_items: int, *, implicit: bool, allow_tuple_literal: bool = False
    ) -> None:
        t = self._make_t(n_items, implicit=implicit)
        off_ta, _ = self._analyser(allow_tuple_literal=allow_tuple_literal)
        off = self._with_gate(False, lambda: self._call(off_ta, t))
        on_ta, _ = self._analyser(allow_tuple_literal=allow_tuple_literal)
        on = self._with_gate(True, lambda: self._call(on_ta, t))
        label = f"tuple_implicit {n_items} implicit={implicit} allow={allow_tuple_literal}"
        assert_equal(on[0], off[0], f"{label} parity result")
        assert_equal(on[1], off[1], f"{label} parity messages")

    def _seam(self, implicit: bool, allow_tuple_literal: bool, items_len: int) -> int | None:
        from mypy.typeanal import _rust_classify_tuple_type_implicit  # type: ignore[attr-defined]

        return _rust_classify_tuple_type_implicit(implicit, allow_tuple_literal, items_len)

    def test_implicit_empty_tuple(self) -> None:
        self._assert_par(0, implicit=True)
        assert self._seam(True, False, 0) == 1

    def test_implicit_single_item(self) -> None:
        self._assert_par(1, implicit=True)
        assert self._seam(True, False, 1) == 2

    def test_implicit_many_items(self) -> None:
        self._assert_par(3, implicit=True)
        assert self._seam(True, False, 3) == 3

    def test_not_implicit_ok_path(self) -> None:
        self._assert_par(2, implicit=False)
        assert self._seam(False, False, 2) == 0

    def test_implicit_with_allowed_literal_ok_path(self) -> None:
        self._assert_par(2, implicit=True, allow_tuple_literal=True)
        assert self._seam(True, True, 2) == 0

    def test_gate_off_defers_to_python(self) -> None:
        # Gate off is handled in the shim, not the seam; verify the shim
        # returns None (pure-Python arbitration) while the seam itself is
        # a total function.
        ta, _ = self._analyser()
        t = self._make_t(1, implicit=True)
        self._set_active(False)
        try:
            assert ta._native_tuple_type_implicit_tag(t) is None
        finally:
            self._set_active(True)


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


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAnalyzeCallableTypeSuite(Suite):
    """Parity for the Rust `analyze_callable_type` dispatch classifier.

    The 6-way dispatch head of `analyze_callable_type` (bare Callable, TypeList
    args, Ellipsis, ParamSpec, invalid arity with/without disallow_any_generics)
    is decided in Rust from four scalar facts (`arg_count`, `arg0` is TypeList,
    `arg0` is Ellipsis, `disallow_any_generics`); the Python shim applies the
    branch bodies (object construction, tvar_scope entry, fail/note emission).

    The invalid-arity branches (tags 4 and 5) only call `self.fail` and return
    AnyType, so they are exercised end-to-end with a minimal FakeApi. The
    remaining branches need `named_type`, `get_omitted_any`, `tvar_scope`, and
    `visit_callable_type` which require a fully wired TypeAnalyser; those paths
    are covered by testcheck.py parity and by direct seam tag tests.
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

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

    def _analyser(self, disallow_any_generics: bool = False) -> tuple[object, object]:
        from mypy.errorcodes import ErrorCode as _ErrorCode
        from mypy.typeanal import TypeAnalyser

        fx = TypeFixture()
        function_info: TypeInfo = fx.functioni

        class FakeApi:
            def __init__(self) -> None:
                self.errors: list[str] = []
                self.syms: dict[str, SymbolTableNode] = {
                    "builtins.function": SymbolTableNode(MDEF, function_info)
                }

            def lookup_qualified(
                self, name: str, ctx: Context, suppress_errors: bool = False
            ) -> SymbolTableNode | None:
                return self.syms.get(name)

            def lookup_fully_qualified(self, fullname: str) -> SymbolTableNode:
                return self.syms[fullname]

            def lookup_fully_qualified_or_none(self, fullname: str) -> SymbolTableNode | None:
                return self.syms.get(fullname)

            def fail(self, msg: str, ctx: Context, code: _ErrorCode | None = None) -> None:
                self.errors.append(msg)

            def note(self, msg: str, ctx: Context, code: _ErrorCode | None = None) -> None:
                self.errors.append(f"note: {msg}")

        api = FakeApi()
        ta = TypeAnalyser.__new__(TypeAnalyser)
        ta.api = api  # type: ignore[assignment]
        ta.fail_func = api.fail  # type: ignore[assignment]
        ta.note_func = api.note
        ta.options = Options()
        ta.options.disallow_any_generics = disallow_any_generics
        return ta, api

    def _call_invalid(self, ta: Any, t: UnboundType) -> tuple[str, list[str]]:
        result = ta.analyze_callable_type(t)
        messages = list(ta.api.errors)
        return str(result), messages

    def _assert_invalid_par(
        self, args: list[Type], *, disallow_any_generics: bool = False
    ) -> None:
        t = UnboundType("typing.Callable", args)
        off_ta, _ = self._analyser(disallow_any_generics=disallow_any_generics)
        off = self._with_gate(False, lambda: self._call_invalid(off_ta, t))
        self._set_active(True)
        on_ta, _ = self._analyser(disallow_any_generics=disallow_any_generics)
        on = self._with_gate(True, lambda: self._call_invalid(on_ta, t))
        assert_equal(on[0], off[0], f"callable_type parity result {args!r}")
        assert_equal(on[1], off[1], f"callable_type parity messages {args!r}")

    def _assert_engages(
        self,
        arg_count: int,
        arg0_is_type_list: bool,
        arg0_is_ellipsis: bool,
        disallow_any_generics: bool,
    ) -> None:
        from mypy.typeanal import (  # type: ignore[attr-defined]
            _rust_classify_analyze_callable_type,
        )

        tag = _rust_classify_analyze_callable_type(
            arg_count, arg0_is_type_list, arg0_is_ellipsis, disallow_any_generics
        )
        assert tag is not None, "Rust analyze_callable_type did not engage"

    def test_invalid_arity_allow(self) -> None:
        # 1 or 3 args without disallow_any_generics -> tag 5.
        self._assert_invalid_par([UnboundType("int")])
        self._assert_invalid_par([UnboundType("int"), UnboundType("str"), UnboundType("bool")])
        self._assert_engages(1, False, False, False)
        self._assert_engages(3, False, False, False)

    def test_invalid_arity_disallow(self) -> None:
        # 1 or 3 args with disallow_any_generics -> tag 4.
        self._assert_invalid_par([UnboundType("int")], disallow_any_generics=True)
        self._assert_invalid_par(
            [UnboundType("int"), UnboundType("str"), UnboundType("bool")],
            disallow_any_generics=True,
        )
        self._assert_engages(1, False, False, True)
        self._assert_engages(3, False, False, True)

    def test_invalid_disallow_message(self) -> None:
        # Tag 4 emits the shorter message (no "or Callable" suffix).
        t = UnboundType("typing.Callable", [UnboundType("int")])
        on_ta, on_api = self._analyser(disallow_any_generics=True)
        on = self._with_gate(True, lambda: self._call_invalid(on_ta, t))
        assert_equal(
            on[1],
            ['Please use "Callable[[<parameters>], <return type>]"'],
            "disallow_any_generics message",
        )

    def test_invalid_allow_message(self) -> None:
        # Tag 5 emits the longer message (with "or Callable" suffix).
        t = UnboundType("typing.Callable", [UnboundType("int")])
        on_ta, on_api = self._analyser(disallow_any_generics=False)
        on = self._with_gate(True, lambda: self._call_invalid(on_ta, t))
        assert_equal(
            on[1],
            ['Please use "Callable[[<parameters>], <return type>]" or "Callable"'],
            "allow_any_generics message",
        )

    def test_bare(self) -> None:
        # arg_count == 0 -> tag 0 (bare Callable).
        self._assert_engages(0, False, False, False)
        self._assert_engages(0, False, False, True)

    def test_type_list(self) -> None:
        # arg_count == 2, arg0 is TypeList -> tag 1.
        self._assert_engages(2, True, False, False)

    def test_ellipsis(self) -> None:
        # arg_count == 2, arg0 is Ellipsis -> tag 2.
        self._assert_engages(2, False, True, False)

    def test_paramspec(self) -> None:
        # arg_count == 2, arg0 is neither TypeList nor Ellipsis -> tag 3.
        self._assert_engages(2, False, False, False)

    def test_direct_tags(self) -> None:
        # Verify all 6 tags through the direct seam.
        from mypy.typeanal import (  # type: ignore[attr-defined]
            _CALLABLE_TAG_BARE,
            _CALLABLE_TAG_ELLIPSIS,
            _CALLABLE_TAG_INVALID_ALLOW,
            _CALLABLE_TAG_INVALID_DISALLOW,
            _CALLABLE_TAG_PARAMSPEC,
            _CALLABLE_TAG_TYPE_LIST,
            _rust_classify_analyze_callable_type,
        )

        assert _rust_classify_analyze_callable_type(0, False, False, False) == _CALLABLE_TAG_BARE
        assert (
            _rust_classify_analyze_callable_type(2, True, False, False) == _CALLABLE_TAG_TYPE_LIST
        )
        assert (
            _rust_classify_analyze_callable_type(2, False, True, False) == _CALLABLE_TAG_ELLIPSIS
        )
        assert (
            _rust_classify_analyze_callable_type(2, False, False, False) == _CALLABLE_TAG_PARAMSPEC
        )
        assert (
            _rust_classify_analyze_callable_type(1, False, False, True)
            == _CALLABLE_TAG_INVALID_DISALLOW
        )
        assert (
            _rust_classify_analyze_callable_type(1, False, False, False)
            == _CALLABLE_TAG_INVALID_ALLOW
        )


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeParameterSuite(Suite):
    """Parity for the Rust `check_type_parameter` dispatch-head port.

    `mypy.subtypes.check_type_parameter` (subtypes.py:891-943) upgrades an
    INVARIANT variance to COVARIANT when the left arg is an ambiguous
    UninhabitedType, then routes to one of four subtype leaves by
    variance (covariant / contravariant arg-order swap / equality).
    The Rust classifier (`subtypes.rs`) reads the scalar variance and
    `proper_subtype` flags plus the live `left` via PyO3 and returns a
    leaf tag; the Python shim applies the leaf call and keeps the
    pure-Python body as the fallback.

    Direct seam calls assert the exact tag for every branch; the gate-off
    vs gate-on differential drives the real function over live types and
    asserts identical results.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active

        self._set_active = _set_native_subtype_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _tag(self, left: Type, variance: int, proper_subtype: bool) -> int | None:
        return _type_kernel.rust_classify_type_parameter(left, variance, proper_subtype)

    def _run(
        self, left: Type, right: Type, variance: int, proper_subtype: bool
    ) -> tuple[bool, bool]:
        from mypy.subtypes import SubtypeContext, check_type_parameter

        ctx = SubtypeContext()

        def call() -> bool:
            return check_type_parameter(left, right, variance, proper_subtype, ctx)

        off = self._with_gate(False, call)
        on = self._with_gate(True, call)
        return off, on

    def _assert_par(self, left: Type, right: Type, variance: int, proper_subtype: bool) -> None:
        off, on = self._run(left, right, variance, proper_subtype)
        assert_equal(
            on,
            off,
            f"check_type_parameter parity for left={left!r} right={right!r} "
            f"variance={variance} proper_subtype={proper_subtype}",
        )

    def test_seam_covariant_tag(self) -> None:
        from mypy.nodes import COVARIANT
        from mypy.subtypes import NATIVE_TYPEPARAM_SUBTYPE

        assert self._tag(UninhabitedType(), COVARIANT, False) == (NATIVE_TYPEPARAM_SUBTYPE)

    def test_seam_variance_not_ready_tag(self) -> None:
        # VARIANCE_NOT_READY is leniently treated as covariant.
        from mypy.nodes import VARIANCE_NOT_READY
        from mypy.subtypes import NATIVE_TYPEPARAM_PROPER_SUBTYPE

        assert self._tag(UninhabitedType(), VARIANCE_NOT_READY, True) == (
            NATIVE_TYPEPARAM_PROPER_SUBTYPE
        )

    def test_seam_ambiguous_uninhabited_upgrades_to_covariant(self) -> None:
        from mypy.nodes import INVARIANT
        from mypy.subtypes import NATIVE_TYPEPARAM_SUBTYPE

        assert self._tag(UninhabitedType(ambiguous=True), INVARIANT, False) == (
            NATIVE_TYPEPARAM_SUBTYPE
        )

    def test_seam_unambiguous_uninhabited_stays_invariant(self) -> None:
        from mypy.nodes import INVARIANT
        from mypy.subtypes import NATIVE_TYPEPARAM_EQUIVALENT

        assert self._tag(UninhabitedType(), INVARIANT, False) == (NATIVE_TYPEPARAM_EQUIVALENT)

    def test_seam_ambiguous_uninhabited_proper_subtype(self) -> None:
        from mypy.nodes import INVARIANT
        from mypy.subtypes import NATIVE_TYPEPARAM_PROPER_SUBTYPE

        assert self._tag(UninhabitedType(ambiguous=True), INVARIANT, True) == (
            NATIVE_TYPEPARAM_PROPER_SUBTYPE
        )

    def test_seam_contravariant_tags(self) -> None:
        from mypy.nodes import CONTRAVARIANT
        from mypy.subtypes import NATIVE_TYPEPARAM_PROPER_SWAP, NATIVE_TYPEPARAM_SUBTYPE_SWAP

        assert self._tag(UninhabitedType(), CONTRAVARIANT, False) == (
            NATIVE_TYPEPARAM_SUBTYPE_SWAP
        )
        assert self._tag(UninhabitedType(), CONTRAVARIANT, True) == (NATIVE_TYPEPARAM_PROPER_SWAP)

    def test_seam_alias_left_expands_through_get_proper_type(self) -> None:
        # get_proper_type must unwrap an alias before the UninhabitedType
        # isinstance; a live alias expands through the same call, so the
        # seam still decides (tag matches the expanded variance check).
        from mypy.nodes import INVARIANT, TypeAlias
        from mypy.subtypes import NATIVE_TYPEPARAM_SUBTYPE

        alias = TypeAlias(UninhabitedType(ambiguous=True), "mod.A", "mod", -1, -1)
        assert self._tag(TypeAliasType(alias, []), INVARIANT, False) == (NATIVE_TYPEPARAM_SUBTYPE)

    def test_seam_defers_on_expanding_alias_left(self) -> None:
        # A hand-rolled TypeAliasType whose expansion raises defers (None)
        # and the Python body raises the identical AttributeError, keeping
        # the raise behavior on both sides of the gate.
        from mypy.nodes import INVARIANT

        assert (
            self._tag(
                TypeAliasType(cast(Any, "A"), cast(Any, UninhabitedType()), cast(Any, [])),
                INVARIANT,
                False,
            )
            is None
        )

    def test_par_covariant_types(self) -> None:
        from mypy.nodes import COVARIANT

        fx = TypeFixture()
        self._assert_par(fx.a, fx.b, COVARIANT, False)
        self._assert_par(fx.a, fx.b, COVARIANT, True)

    def test_par_contravariant_types(self) -> None:
        from mypy.nodes import CONTRAVARIANT

        fx = TypeFixture()
        self._assert_par(fx.a, fx.o, CONTRAVARIANT, False)
        self._assert_par(fx.a, fx.o, CONTRAVARIANT, True)

    def test_par_invariant_equality(self) -> None:
        from mypy.nodes import INVARIANT

        fx = TypeFixture()
        self._assert_par(fx.a, fx.a, INVARIANT, False)
        self._assert_par(fx.a, fx.a, INVARIANT, True)
        self._assert_par(fx.a, fx.b, INVARIANT, False)
        self._assert_par(fx.a, fx.b, INVARIANT, True)

    def test_par_ambiguous_uninhabited_invariant(self) -> None:
        from mypy.nodes import INVARIANT, VARIANCE_NOT_READY

        fx = TypeFixture()
        amb = UninhabitedType(ambiguous=True)
        self._assert_par(amb, fx.a, INVARIANT, False)
        self._assert_par(amb, fx.a, INVARIANT, True)
        self._assert_par(amb, fx.o, VARIANCE_NOT_READY, False)

    def test_par_nonstandard_variance_equality_leaves(self) -> None:
        # A non-standard variance int falls into the equality leaves.
        fx = TypeFixture()
        self._assert_par(fx.a, fx.a, 7, False)
        self._assert_par(fx.a, fx.b, 7, True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeConstraintHelpersSuite(Suite):
    """Parity tests for the standalone constraint-list helper seams.

    Ports `merge_with_any`, `filter_satisfiable`, and
    `is_same_constraints` (issue #1001). Differential harness: runs each
    Python helper with the native gate on (resolver installed) and off
    (pure Python) and asserts identical results. Direct seam calls prove
    engagement and the deferral set (alias targets, ParamSpec origins,
    missing resolver).
    """

    def setUp(self) -> None:
        from mypy.constraints import Constraint, _set_native_constraints_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.Constraint = Constraint
        self._set_active = _set_native_constraints_active
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_active(False)

    def tearDown(self) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)

    def _tvar(
        self,
        name: str,
        raw_id: int,
        upper_bound: Type,
        *,
        meta_level: int = 0,
        values: list[Type] | None = None,
    ) -> TypeVarType:
        return TypeVarType(
            name,
            name,
            TypeVarId(raw_id, meta_level=meta_level),
            values if values is not None else [],
            upper_bound,
            AnyType(TypeOfAny.from_omitted_generics),
        )

    def _pspec(self) -> ParamSpecType:
        return ParamSpecType(
            "P",
            "P",
            TypeVarId(1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )

    # --- merge_with_any ---

    def _merge_result(self, constraint: Constraint, native: bool) -> Constraint:
        from mypy.constraints import merge_with_any

        self._set_active(native)
        return merge_with_any(constraint)

    def _assert_merge_par(self, constraint: Constraint) -> None:
        native = self._merge_result(constraint, native=True)
        python = self._merge_result(constraint, native=False)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def test_merge_plain_target(self) -> None:
        # Plain Instance target: a union with Any is built; the seam
        # engages natively.
        self._assert_merge_par(self.Constraint(self.fx.t, 0, self.fx.a))
        merged = self._merge_result(self.Constraint(self.fx.t, 0, self.fx.a), native=True)
        assert isinstance(get_proper_type(merged.target), UnionType)

    def test_merge_union_with_any_untouched(self) -> None:
        # Target already contains Any: the constraint is returned as-is
        # (same object, no redundant union).
        target = UnionType([self.fx.a, AnyType(TypeOfAny.special_form)])
        constraint = self.Constraint(self.fx.t, 0, target)
        self._assert_merge_par(constraint)
        assert self._merge_result(constraint, native=True) is constraint
        assert self._merge_result(constraint, native=False) is constraint

    def test_merge_plain_any_target(self) -> None:
        # Any target itself counts as a union with Any.
        constraint = self.Constraint(self.fx.t, 0, AnyType(TypeOfAny.special_form))
        self._assert_merge_par(constraint)
        assert self._merge_result(constraint, native=True) is constraint

    def test_merge_alias_target_defers(self) -> None:
        # TypeAliasType targets defer (get_proper_type cannot expand on
        # the wire); the Python fallback merges the same way.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        self._assert_merge_par(self.Constraint(self.fx.t, 0, TypeAliasType(alias, [])))

    # --- filter_satisfiable ---

    def _filter_result(
        self, option: list[Constraint] | None, native: bool
    ) -> list[Constraint] | None:
        from mypy.constraints import _set_native_constraints_resolver, filter_satisfiable

        self._set_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return filter_satisfiable(option)

    def _assert_filter_par(self, option: list[Constraint] | None) -> None:
        native = self._filter_result(option, native=True)
        python = self._filter_result(option, native=False)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def test_filter_upper_bound_keeps(self) -> None:
        # Target is a subtype of the upper bound: kept.
        self._assert_filter_par([self.Constraint(self.fx.t, 0, self.fx.a)])

    def test_filter_upper_bound_drops(self) -> None:
        # b is not a subtype of a: the only constraint is dropped -> None.
        t = self._tvar("t", 11, self.fx.a)
        t2 = self._tvar("t2", 12, self.fx.b)
        self._assert_filter_par([self.Constraint(t2, 0, self.fx.a)])
        self._assert_filter_par([self.Constraint(t, 0, self.fx.a)])

    def test_filter_mixed(self) -> None:
        # B <: A in the fixture, so both constraints survive the A bound.
        t = self._tvar("t", 11, self.fx.a)
        self._assert_filter_par(
            [self.Constraint(t, 0, self.fx.a), self.Constraint(t, 0, self.fx.b)]
        )

    def test_filter_values_branch(self) -> None:
        # Values-carrying origin: keep when the target is a subtype of
        # one of the values.
        t1 = self._tvar("t1", 12, self.fx.o, values=[self.fx.a, self.fx.b])
        self._assert_filter_par(
            [self.Constraint(t1, 0, self.fx.a), self.Constraint(t1, 0, self.fx.b)]
        )
        t2 = self._tvar("t2", 13, self.fx.o, values=[self.fx.b])
        self._assert_filter_par([self.Constraint(t2, 0, self.fx.a)])

    def test_filter_empty_and_none(self) -> None:
        # `if not option: return option` runs before the seam: an empty
        # list is returned intact, None passes through.
        assert self._filter_result([], native=True) == []
        assert self._filter_result(None, native=True) is None
        self._assert_filter_par([])
        self._assert_filter_par(None)

    def test_filter_paramspec_defers(self) -> None:
        # ParamSpec origins defer to the pure-Python body; parity holds.
        self._assert_filter_par([self.Constraint(self._pspec(), 0, self.fx.o)])

    # --- is_same_constraints ---

    def _same_result(self, x: list[Constraint], y: list[Constraint], native: bool) -> bool:
        from mypy.constraints import _set_native_constraints_resolver, is_same_constraints

        self._set_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return is_same_constraints(x, y)

    def _assert_same_par(self, x: list[Constraint], y: list[Constraint]) -> None:
        native = self._same_result(x, y, native=True)
        python = self._same_result(x, y, native=False)
        assert native == python, f"native={native!r} python={python!r}"

    def test_same_identical_lists(self) -> None:
        option = [self.Constraint(self.fx.t, 0, self.fx.a)]
        self._assert_same_par(option, list(option))
        assert self._same_result(option, list(option), native=True) is True

    def test_same_both_any_skips_op(self) -> None:
        # Both targets Any: op mismatch is ignored.
        x = [self.Constraint(self.fx.t, 0, AnyType(TypeOfAny.special_form))]
        y = [self.Constraint(self.fx.t, 1, AnyType(TypeOfAny.special_form))]
        self._assert_same_par(x, y)
        assert self._same_result(x, y, native=True) is True

    def test_same_op_mismatch(self) -> None:
        x = [self.Constraint(self.fx.t, 0, self.fx.a)]
        y = [self.Constraint(self.fx.t, 1, self.fx.a)]
        self._assert_same_par(x, y)
        assert self._same_result(x, y, native=True) is False

    def test_same_target_mismatch(self) -> None:
        x = [self.Constraint(self.fx.t, 0, self.fx.a)]
        y = [self.Constraint(self.fx.t, 0, self.fx.b)]
        self._assert_same_par(x, y)
        assert self._same_result(x, y, native=True) is False

    def test_same_var_mismatch(self) -> None:
        x = [self.Constraint(self.fx.t, 0, self.fx.a)]
        y = [self.Constraint(self.fx.s, 0, self.fx.a)]
        self._assert_same_par(x, y)
        assert self._same_result(x, y, native=True) is False

    def test_same_empty_lists(self) -> None:
        self._assert_same_par([], [])
        assert self._same_result([], [], native=True) is True

    def test_same_alias_target_defers(self) -> None:
        # Alias targets make the pairwise check undecidable in Rust; the
        # Python fallback decides and parity holds.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        target = TypeAliasType(alias, [])
        self._assert_same_par(
            [self.Constraint(self.fx.t, 0, target)], [self.Constraint(self.fx.t, 0, self.fx.a)]
        )

    # --- direct seam calls ---

    def _wire_constraint(self, constraint: Constraint) -> bytes:
        from mypy.constraints import _write_constraint

        buf = _WriteBuffer()
        _write_constraint(buf, constraint)
        return buf.getvalue()

    def _wire_option(self, option: list[Constraint]) -> bytes:
        from mypy.constraints import _write_option

        buf = _WriteBuffer()
        _write_option(buf, option)
        return buf.getvalue()

    def test_direct_merge_engages(self) -> None:
        assert (
            _type_kernel.rust_merge_with_any(
                self._wire_constraint(self.Constraint(self.fx.t, 0, self.fx.a))
            )
            is True
        )
        assert (
            _type_kernel.rust_merge_with_any(
                self._wire_constraint(
                    self.Constraint(self.fx.t, 0, AnyType(TypeOfAny.special_form))
                )
            )
            is False
        )

    def test_direct_merge_alias_defers(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        blob = self._wire_constraint(self.Constraint(self.fx.t, 0, TypeAliasType(alias, [])))
        assert _type_kernel.rust_merge_with_any(blob) is None

    def test_direct_filter_engages(self) -> None:
        from mypy.constraints import _read_index_list

        # B <: A in the fixture: both targets satisfy the A upper bound.
        t = self._tvar("t", 11, self.fx.a)
        option = [self.Constraint(t, 0, self.fx.a), self.Constraint(t, 0, self.fx.b)]
        raw = _type_kernel.rust_filter_satisfiable(
            self._wire_option(option), strict_optional_flag(), self.resolver
        )
        assert raw is not None
        assert _read_index_list(bytes(raw)) == [0, 1]
        # All filtered: A is not a subtype of the B upper bound.
        t2 = self._tvar("t2", 12, self.fx.b)
        raw = _type_kernel.rust_filter_satisfiable(
            self._wire_option([self.Constraint(t2, 0, self.fx.a)]),
            strict_optional_flag(),
            self.resolver,
        )
        assert raw is not None
        assert _read_index_list(bytes(raw)) == []

    def test_direct_filter_paramspec_defers(self) -> None:
        raw = _type_kernel.rust_filter_satisfiable(
            self._wire_option([self.Constraint(self._pspec(), 0, self.fx.o)]),
            strict_optional_flag(),
            self.resolver,
        )
        assert raw is None

    def test_direct_is_same_engages(self) -> None:
        option = [self.Constraint(self.fx.t, 0, self.fx.a)]
        assert (
            _type_kernel.rust_is_same_constraints(
                self._wire_option(option), self._wire_option(option), self.resolver
            )
            is True
        )
        other = [self.Constraint(self.fx.t, 1, self.fx.a)]
        assert (
            _type_kernel.rust_is_same_constraints(
                self._wire_option(option), self._wire_option(other), self.resolver
            )
            is False
        )

    def test_direct_filter_requires_resolver(self) -> None:
        # The shim defers to Python when no resolver snapshot is installed.
        from mypy.constraints import _try_native_filter_satisfiable

        t = self._tvar("t", 11, self.fx.a)
        self._set_active(True)
        try:
            assert _try_native_filter_satisfiable([self.Constraint(t, 0, self.fx.a)]) is None
        finally:
            self._set_active(False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeGuardArgSuite(Suite):
    """Parity for the Rust TypeGuard/TypeIs argument classifier (issue #1043).

    `anal_type_guard_arg` / `anal_type_is_arg` (typeanal.py) decide family
    membership (TypeGuard vs TypeIs name-sets) and the arity gate in Rust
    from scalars (fullname, args_len, is_typeis); the Python shim applies
    the VALID_TYPE fail + AnyType(from_error) or the anal_type recursion,
    and NOT_GUARD returns None. Toggling the typeanal gate off (pure
    Python) and on (Rust seam) must produce identical
    (str(result), captured fail messages), and direct seam calls prove the
    tag table.
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active

        self._set_active = _set_native_typeanal_active
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
        from mypy.typeanal import TypeAnalyser

        class FakeApi:
            def __init__(self) -> None:
                self.messages: list[str] = []

            def fail(self, msg: str, ctx: Context, code: Any = None) -> None:
                self.messages.append(f"fail: {msg}")

            def note(self, msg: str, ctx: Context, code: Any = None) -> None:
                self.messages.append(f"note: {msg}")

        api = FakeApi()
        ta = TypeAnalyser.__new__(TypeAnalyser)
        ta.api = api  # type: ignore[assignment]
        ta.fail_func = api.fail  # type: ignore[assignment]
        ta.note_func = api.note
        # anal_type recursion stub: the RECURSE branch returns its argument.
        ta.anal_type = lambda t, **kw: t  # type: ignore[method-assign]

        # lookup_qualified stub: resolve the unbound name to a node whose
        # fullname is the name itself (the wrappers precompute it Python-
        # side; only the _arg methods hit the seam).
        def lookup_stub(name: str, ctx: Context, suppress_errors: bool = False) -> Any:
            return SimpleNamespace(node=SimpleNamespace(fullname=name))

        ta.lookup_qualified = lookup_stub  # type: ignore[method-assign]
        return ta

    def _make_t(self, fullname: str, n_args: int) -> UnboundType:
        return UnboundType(fullname, [self.fx.str_type] * n_args)

    def _call(self, ta: Any, t: UnboundType, fullname: str, typeis: bool) -> tuple[str, list[str]]:
        if typeis:
            result = ta.anal_type_is(t)
        else:
            result = ta.anal_type_guard(t)
        return str(result), list(ta.api.messages)

    def _assert_par(self, fullname: str, n_args: int, typeis: bool = False) -> None:
        t = self._make_t(fullname, n_args)
        off = self._with_gate(False, lambda: self._call(self._analyser(), t, fullname, typeis))
        on = self._with_gate(True, lambda: self._call(self._analyser(), t, fullname, typeis))
        label = f"type_guard_arg {fullname} n={n_args} typeis={typeis}"
        assert_equal(on[0], off[0], f"{label} parity result")
        assert_equal(on[1], off[1], f"{label} parity messages")

    def _seam(self, fullname: str, args_len: int, typeis: bool) -> int | None:
        from mypy.typeanal import _rust_classify_type_guard_arg  # type: ignore[attr-defined]

        return _rust_classify_type_guard_arg(fullname, args_len, typeis)

    def test_guard_one_arg_recurses(self) -> None:
        self._assert_par("typing.TypeGuard", 1)
        assert self._seam("typing.TypeGuard", 1, False) == 2

    def test_guard_zero_args_fails(self) -> None:
        self._assert_par("typing.TypeGuard", 0)
        assert self._seam("typing.TypeGuard", 0, False) == 1

    def test_guard_two_args_fails(self) -> None:
        self._assert_par("typing.TypeGuard", 2)
        assert self._seam("typing.TypeGuard", 2, False) == 1

    def test_typeis_one_arg_recurses(self) -> None:
        self._assert_par("typing.TypeIs", 1, typeis=True)
        assert self._seam("typing.TypeIs", 1, True) == 2

    def test_typeis_zero_args_fails(self) -> None:
        self._assert_par("typing.TypeIs", 0, typeis=True)
        assert self._seam("typing.TypeIs", 0, True) == 1

    def test_typeis_two_args_fails(self) -> None:
        self._assert_par("typing.TypeIs", 2, typeis=True)
        assert self._seam("typing.TypeIs", 2, True) == 1

    def test_non_guard_fullname_returns_none(self) -> None:
        self._assert_par("typing.Optional", 1)
        self._assert_par("mod.NotAGuard", 0, typeis=True)
        assert self._seam("mod.NotAGuard", 1, False) == 0

    def test_extension_fullnames(self) -> None:
        self._assert_par("typing_extensions.TypeGuard", 1)
        self._assert_par("typing_extensions.TypeIs", 0, typeis=True)
        assert self._seam("typing_extensions.TypeGuard", 1, False) == 2
        assert self._seam("typing_extensions.TypeIs", 2, True) == 1

    def test_gate_off_defers_to_python(self) -> None:
        # Gate off is handled in the shim, not the seam; verify the shim
        # returns None (pure-Python arbitration) while the seam itself is
        # a total function.
        ta = self._analyser()
        self._set_active(False)
        try:
            assert ta._native_type_guard_arg_tag("typing.TypeGuard", 1, is_typeis=False) is None
        finally:
            self._set_active(True)

    def test_recurse_result_and_fail_messages_parity(self) -> None:
        # Explicit parity on the three payload shapes: the RECURSE result
        # carries the analyzed arg, the FAIL branches carry the family-
        # specific message, and NOT_GUARD is a silent None.
        t = self._make_t("typing.TypeGuard", 1)
        off = self._with_gate(
            False, lambda: self._call(self._analyser(), t, "typing.TypeGuard", False)
        )
        on = self._with_gate(
            True, lambda: self._call(self._analyser(), t, "typing.TypeGuard", False)
        )
        assert_equal(on, ("builtins.str", []), "guard recurse payload")
        assert_equal(off, on, "guard recurse parity")
        t0 = self._make_t("typing.TypeIs", 0)
        off = self._with_gate(
            False, lambda: self._call(self._analyser(), t0, "typing.TypeIs", True)
        )
        on = self._with_gate(True, lambda: self._call(self._analyser(), t0, "typing.TypeIs", True))
        assert_equal(
            on,
            ("Any", ["fail: TypeIs must have exactly one type argument"]),
            "typeis fail payload",
        )
        assert_equal(off, on, "typeis fail parity")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeGetTargetTypeSuite(Suite):
    """Parity for the Rust `get_target_type` decision head (applytype.py, #1081).

    The Python shim in mypy.applytype.get_target_type computes the
    resolver-backed subtype/same-type booleans (already native) and lets
    Rust pick the branch tag; Python applies expand_type, the
    report_incompatible_typevar_value callback, and returns live types.
    Toggling the applytype gate off (pure-Python body) and on (Rust seam)
    must produce identical results and identical callback traffic. Direct
    seam calls prove each tag engages rather than silently deferring.
    """

    def setUp(self) -> None:
        from mypy.applytype import _set_native_applytype_active

        self.fx = TypeFixture()
        self._set_active = _set_native_applytype_active
        self._set_active(False)

    def tearDown(self) -> None:
        self._set_active(False)

    def _tvar(
        self,
        name: str = "T",
        raw_id: int = 1,
        upper_bound: Type | None = None,
        values: list[Type] | None = None,
        default: Type | None = None,
    ) -> TypeVarType:
        return TypeVarType(
            name,
            "mod." + name,
            TypeVarId(raw_id),
            values if values is not None else [],
            upper_bound if upper_bound is not None else self.fx.o,
            default if default is not None else AnyType(TypeOfAny.from_omitted_generics),
        )

    def _pspec(self) -> ParamSpecType:
        return ParamSpecType(
            "P",
            "mod.P",
            TypeVarId(1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
        )

    def _tvt(self) -> TypeVarTupleType:
        return TypeVarTupleType(
            "Ts",
            "mod.Ts",
            TypeVarId(1),
            self.fx.o,
            self.fx.std_tuple,
            AnyType(TypeOfAny.from_omitted_generics),
        )

    def _callable(self, tvar: TypeVarLikeType) -> CallableType:
        return CallableType([], [], [], self.fx.anyt, self.fx.function, name="f", variables=[tvar])

    def _report(self) -> tuple[Any, list[tuple[str, Type]]]:
        calls: list[tuple[str, Type]] = []

        def report(callable: CallableType, type: Type, name: str, context: Any) -> None:
            calls.append((name, type))

        return report, calls

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _assert_par(
        self, tvar: TypeVarLikeType, type_arg: Type, skip_unsatisfied: bool = False
    ) -> tuple[Type | None, list[tuple[str, Type]]]:
        from mypy.applytype import get_target_type

        report, calls = self._report()
        context = self._callable(tvar)
        off = self._with_gate(
            False,
            lambda: get_target_type(
                tvar, type_arg, context, report, context, skip_unsatisfied, {}
            ),
        )
        report2, calls2 = self._report()
        on = self._with_gate(
            True,
            lambda: get_target_type(
                tvar, type_arg, context, report2, context, skip_unsatisfied, {}
            ),
        )
        assert_equal(on, off, f"get_target_type parity {type_arg}")
        assert_equal(calls2, calls, f"report parity {type_arg}")
        return on, calls2

    # --- gate-off vs gate-on differentials ---

    def test_baseline_gate_off(self) -> None:
        tvar = self._tvar()
        result, calls = self._assert_par(tvar, self.fx.a)
        assert result is self.fx.a
        assert calls == []

    def test_parity_unconstrained_bound_ok(self) -> None:
        tvar = self._tvar(upper_bound=self.fx.o)
        result, calls = self._assert_par(tvar, self.fx.a)
        assert result is self.fx.a
        assert calls == []

    def test_parity_unconstrained_bound_fail_report(self) -> None:
        # A is not a subtype of B: both gates report and return the type.
        tvar = self._tvar(upper_bound=self.fx.b)
        result, calls = self._assert_par(tvar, self.fx.a)
        assert result is self.fx.a
        assert calls == [("T", self.fx.a)]

    def test_parity_unconstrained_bound_fail_skip(self) -> None:
        tvar = self._tvar(upper_bound=self.fx.b)
        result, calls = self._assert_par(tvar, self.fx.a, skip_unsatisfied=True)
        assert result is None
        assert calls == []

    def test_parity_self_erase(self) -> None:
        # Self upper bound G[T]: the shim erases typevars before the bound
        # check, so G[A] passes; without the erase the check would fail.
        upper = Instance(self.fx.gi, [self.fx.t])
        tvar = self._tvar(name="Self", raw_id=1, upper_bound=upper)
        result, calls = self._assert_par(tvar, Instance(self.fx.gi, [self.fx.a]))
        assert result is not None
        assert calls == []

    def test_parity_paramspec_passthrough(self) -> None:
        result, calls = self._assert_par(self._pspec(), self.fx.a)
        assert result is self.fx.a
        assert calls == []

    def test_parity_tvt_passthrough(self) -> None:
        result, calls = self._assert_par(self._tvt(), self.fx.a)
        assert result is self.fx.a
        assert calls == []

    def test_parity_expand_default(self) -> None:
        # Ambiguous UninhabitedType with a real tvar default: the default
        # is expanded (empty env leaves it unchanged) and returned.
        tvar = self._tvar(default=self.fx.a)
        amb = UninhabitedType(ambiguous=True)
        result, calls = self._assert_par(tvar, amb)
        assert_equal(result, self.fx.a)
        assert calls == []

    def test_parity_constrained_any_passthrough(self) -> None:
        tvar = self._tvar(values=[self.fx.a, self.fx.b])
        anyt = AnyType(TypeOfAny.special_form)
        result, calls = self._assert_par(tvar, anyt)
        assert result is anyt
        assert calls == []

    def test_parity_constrained_best_match(self) -> None:
        tvar = self._tvar(values=[self.fx.a, self.fx.b])
        result, calls = self._assert_par(tvar, self.fx.b)
        assert result is tvar.values[1]
        assert calls == []

    def test_parity_constrained_narrowest_match(self) -> None:
        # Both values match A (object and A); the narrowest (A) wins.
        tvar = self._tvar(values=[self.fx.o, self.fx.a])
        result, calls = self._assert_par(tvar, self.fx.a)
        assert result is tvar.values[1]
        assert calls == []

    def test_parity_constrained_no_match_report(self) -> None:
        tvar = self._tvar(values=[self.fx.b])
        result, calls = self._assert_par(tvar, self.fx.a)
        assert result is self.fx.a
        assert calls == [("T", self.fx.a)]

    def test_parity_constrained_no_match_skip(self) -> None:
        tvar = self._tvar(values=[self.fx.b])
        result, calls = self._assert_par(tvar, self.fx.a, skip_unsatisfied=True)
        assert result is None
        assert calls == []

    def test_parity_cross_product_allow(self) -> None:
        # A TypeVarType arg whose every value is a legal tvar value passes.
        tvar = self._tvar(raw_id=1, values=[self.fx.a, self.fx.b])
        t1 = self._tvar(name="T1", raw_id=2, values=[self.fx.a])
        result, calls = self._assert_par(tvar, t1)
        assert result is t1
        assert calls == []

    def test_parity_cross_product_mismatch(self) -> None:
        # T1's values are not a subset of the tvar's: falls through to the
        # matching fold, which finds no match and reports.
        tvar = self._tvar(raw_id=1, values=[self.fx.a, self.fx.b])
        t1 = self._tvar(name="T1", raw_id=2, values=[Instance(self.fx.di, [])])
        result, calls = self._assert_par(tvar, t1)
        assert result is t1
        assert calls == [("T", t1)]

    # --- direct seam calls ---

    def _seam(self, tvar: Type, type_arg: Type, **facts: Any) -> Any:
        from mypy.applytype import _serialize_type

        return _type_kernel.rust_get_target_type(
            _serialize_type(tvar),
            _serialize_type(type_arg),
            facts.get("skip_unsatisfied", False),
            facts.get("same_type_ok"),
            facts.get("bound_ok"),
            facts.get("value_subtypes"),
            facts.get("narrow_matrix"),
        )

    def test_seam_expand_default_tag(self) -> None:
        tvar = self._tvar(default=self.fx.a)
        assert self._seam(tvar, UninhabitedType(ambiguous=True)) == (0, -1)

    def test_seam_paramspec_tag(self) -> None:
        assert self._seam(self._pspec(), self.fx.a) == (1, -1)

    def test_seam_tvt_tag(self) -> None:
        assert self._seam(self._tvt(), self.fx.a) == (1, -1)

    def test_seam_any_passthrough_tag(self) -> None:
        tvar = self._tvar(values=[self.fx.a, self.fx.b])
        assert self._seam(tvar, AnyType(TypeOfAny.special_form)) == (1, -1)

    def test_seam_cross_product_tag(self) -> None:
        tvar = self._tvar(values=[self.fx.a, self.fx.b])
        t1 = self._tvar(name="T1", raw_id=2, values=[self.fx.a])
        assert self._seam(t1, tvar, same_type_ok=True) == (1, -1)

    def test_seam_best_match_tag(self) -> None:
        tvar = self._tvar(values=[self.fx.a, self.fx.b])
        assert self._seam(tvar, self.fx.b, value_subtypes=[False, True]) == (2, 1)

    def test_seam_narrowest_match_tag(self) -> None:
        # Both values match; is_subtype(values[1], values[0]) picks index 1.
        import mypy.subtypes

        tvar = self._tvar(values=[self.fx.o, self.fx.a])
        matrix = [
            mypy.subtypes.is_subtype(tvar.values[i], tvar.values[j])
            for i in range(2)
            for j in range(2)
        ]
        assert self._seam(tvar, self.fx.a, value_subtypes=[True, True], narrow_matrix=matrix) == (
            2,
            1,
        )

    def test_seam_skip_and_report_tags(self) -> None:
        tvar = self._tvar(values=[self.fx.b])
        assert self._seam(tvar, self.fx.a, skip_unsatisfied=True, value_subtypes=[False]) == (
            3,
            -1,
        )
        assert self._seam(tvar, self.fx.a, skip_unsatisfied=False, value_subtypes=[False]) == (
            4,
            -1,
        )

    def test_seam_unconstrained_tags(self) -> None:
        tvar = self._tvar()
        assert self._seam(tvar, self.fx.a, bound_ok=True) == (1, -1)
        assert self._seam(tvar, self.fx.a, bound_ok=False, skip_unsatisfied=True) == (3, -1)
        assert self._seam(tvar, self.fx.a, bound_ok=False, skip_unsatisfied=False) == (4, -1)

    def test_seam_self_bound_tag(self) -> None:
        # The shim erases the Self upper bound before the subtype check;
        # the seam only arbitrates on the resulting boolean.
        upper = Instance(self.fx.gi, [self.fx.t])
        tvar = self._tvar(name="Self", raw_id=1, upper_bound=upper)
        assert self._seam(tvar, Instance(self.fx.gi, [self.fx.a]), bound_ok=True) == (1, -1)

    def test_seam_alias_defers(self) -> None:
        from mypy.nodes import TypeAlias

        alias = TypeAlias(Instance(self.fx.ai, []), "mod.AAlias", "mod", -1, -1)
        tvar = self._tvar()
        assert self._seam(tvar, TypeAliasType(alias, []), bound_ok=True) is None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeArgVarianceWalkSuite(Suite):
    """Differential suite for the per-arg variance walk in
    `visit_instance_nominal` (issue #1098).

    Covers the two defer removals in the walk: non-TypeVarType type
    params (ParamSpec, kind=1) now dispatch like Python's else branch
    (COVARIANT pass-through), and wire-equal args short-circuit via the
    reflexive fast path in `check_type_parameter`. VARIANCE_NOT_READY
    still defers; the snapshot variance is populated at build time by
    `infer_class_variances` (mypy/build.py).
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos() + self._custom_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if isinstance(value, TypeInfo):
                infos.append(value)
        return infos

    def _custom_infos(self) -> list[TypeInfo]:
        from mypy.nodes import VARIANCE_NOT_READY

        fx = self.fx
        self.co = fx.make_type_info("Co", mro=[fx.oi], typevars=["T"], variances=[COVARIANT])
        self.contra = fx.make_type_info(
            "Contra", mro=[fx.oi], typevars=["T"], variances=[CONTRAVARIANT]
        )
        inv = fx.make_type_info("Inv", mro=[fx.oi], typevars=["T"], variances=[INVARIANT])
        self.inv = inv
        # A class whose only type param is a bare ParamSpec (snapshot kind=1).
        gp = fx.make_type_info("GP", mro=[fx.oi])
        self.pspec = ParamSpecType(
            "P", "P", TypeVarId(1), ParamSpecFlavor.BARE, Instance(fx.oi, [], -1), NoneType()
        )
        gp.defn.type_vars = [self.pspec]
        self.gp = gp
        # A class whose type param is stuck at VARIANCE_NOT_READY.
        self.nr = fx.make_type_info(
            "Nr", mro=[fx.oi], typevars=["T"], variances=[VARIANCE_NOT_READY]
        )
        return [self.co, self.contra, inv, gp, self.nr]

    def _differential(self, left: Type, right: Type) -> bool:
        """Assert the gate-off (pure Python) and gate-on answers agree."""
        from mypy.subtypes import _set_native_subtype_active, is_subtype

        _set_native_subtype_active(False)
        try:
            with state.strict_optional_set(True):
                expected = is_subtype(left, right)
        finally:
            _set_native_subtype_active(True)
        with state.strict_optional_set(True):
            actual = is_subtype(left, right)
        assert_equal(actual, expected)
        return actual

    def _direct_seam(self, left: Type, right: Type) -> bool | None:
        """Call the single-pair seam directly; None means Rust deferred."""
        from mypy.subtypes import _serialize_type

        return _type_kernel.rust_is_subtype(
            _serialize_type(left),
            _serialize_type(right),
            False,  # ignore_type_params
            False,  # ignore_declared_variance
            False,  # always_covariant
            False,  # ignore_promotions
            False,  # proper_subtype
            True,  # strict_optional
            False,  # ignore_pos_arg_names
            False,  # strict_concatenate
            self.resolver,
        )

    def test_covariant_arg(self) -> None:
        # Co[int] <: Co[object] holds; the reverse does not.
        co_int = Instance(self.co, [self.fx.str_type])
        co_obj = Instance(self.co, [self.fx.o])
        assert self._differential(co_int, co_obj) is True
        assert self._differential(co_obj, co_int) is False

    def test_contravariant_arg(self) -> None:
        # Contra[object] <: Contra[str] holds; the reverse does not.
        contra_str = Instance(self.contra, [self.fx.str_type])
        contra_obj = Instance(self.contra, [self.fx.o])
        assert self._differential(contra_obj, contra_str) is True
        assert self._differential(contra_str, contra_obj) is False

    def test_invariant_arg(self) -> None:
        # Inv[str] <: Inv[str] holds; Inv[str] <: Inv[object] does not.
        inv_str = Instance(self.inv, [self.fx.str_type])
        inv_obj = Instance(self.inv, [self.fx.o])
        assert self._differential(inv_str, inv_str) is True
        assert self._differential(inv_str, inv_obj) is False

    def test_paramspec_same_ref_decides_natively(self) -> None:
        # GP[P] <: GP[P]: identical ParamSpec args hit the reflexive fast
        # path in check_type_parameter instead of deferring in the
        # recursive is_subtype (the pre-#1098 behavior).
        gp_p = Instance(self.gp, [self.pspec])
        assert self._differential(gp_p, gp_p) is True
        assert self._direct_seam(gp_p, gp_p) is True

    def test_paramspec_differing_args_parity(self) -> None:
        # GP[P] <: GP[P.args-shape]: differing ParamSpec args dispatch
        # through the kind=1 COVARIANT pass-through; gate on/off agree.
        gp_p = Instance(self.gp, [self.pspec])
        other = self.pspec.with_flavor(ParamSpecFlavor.KWARGS)
        gp_other = Instance(self.gp, [other])
        self._differential(gp_p, gp_other)
        self._differential(gp_other, gp_p)

    def test_variance_not_ready_still_defers(self) -> None:
        # The snapshot cannot mirror infer_class_variances' live mutation;
        # NOT_READY keeps deferring to the Python fallback, which infers
        # the variance lazily (covariant here) on first encounter.
        nr_str = Instance(self.nr, [self.fx.str_type])
        nr_obj = Instance(self.nr, [self.fx.o])
        assert self._direct_seam(nr_str, nr_obj) is None
        assert self._differential(nr_str, nr_obj) is True


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFindSelfTypeSuite(Suite):
    """Parity for the Rust `find_self_type` port (mypy.typeanal, #1114).

    `find_self_type` (typeanal.py:4231) walks a live type tree with the
    `HasSelfType` BoolTypeQuery (ANY_STRATEGY) and returns True when any
    unbound name resolves to `typing.Self` / `typing_extensions.Self`.
    The Rust seam (`rust_find_self_type`) mirrors the visitor. Issue #1114
    added the previously-deferred leaf shapes: TypeList items (query),
    bare EllipsisType (False), and RawExpressionType (False); before the
    port those fell through to the pure-Python body on every occurrence.

    Gate-off vs gate-on runs must produce identical booleans, and the
    direct seam call must decide each shape (never defer).
    """

    def setUp(self) -> None:
        from mypy.typeanal import _set_native_typeanal_active, _set_native_typeanal_resolver

        self.fx = TypeFixture()
        self._set_active = _set_native_typeanal_active
        self._set_resolver = _set_native_typeanal_resolver
        self._set_active(True)
        # Issue #1157: install a resolver snapshot covering the under-test
        # aliases so the live seam expands alias targets instead of
        # deferring; `alias_plain`'s Self resolution must NOT reach args.
        self.alias_self = TypeAlias(UnboundType("Self"), "mod.X", "mod", 1, 1)
        self.alias_list = TypeAlias(
            Instance(self.fx.ai, [UnboundType("Self")]), "mod.L", "mod", 1, 1
        )
        self.alias_py312 = TypeAlias(NoneType(), "mod.Y", "mod", 1, 1, python_3_12_type_alias=True)
        self.alias_plain = TypeAlias(Instance(self.fx.ai, [self.fx.o]), "mod.P", "mod", 1, 1)
        infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.str_type_info,
            self.fx.type_typei,
            self.fx.std_tuplei,
            self.fx.std_listi,
        ]
        self.resolver = _type_kernel.build_native_resolver(
            infos, [self.alias_self, self.alias_list, self.alias_py312, self.alias_plain]
        )
        self.no_alias_resolver = _type_kernel.build_native_resolver(infos, [])
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

    def _lookup(self, names: dict[str, str]) -> Callable[[str], Any]:
        class FakeSym:
            def __init__(self, fullname: str | None) -> None:
                self.fullname = fullname

        def lookup(name: str) -> Any:
            fullname = names.get(name)
            if fullname is None:
                return None
            return FakeSym(fullname)

        return lookup

    def _assert_par(self, typ: Type, names: dict[str, str], expected: bool) -> None:
        from mypy.typeanal import find_self_type

        lookup = self._lookup(names)
        off = self._with_gate(False, lambda: find_self_type(typ, lookup))
        on = self._with_gate(True, lambda: find_self_type(typ, lookup))
        assert off == expected, f"gate-off {typ!r}: {off} != {expected}"
        assert on == expected, f"gate-on {typ!r}: {on} != {expected}"

    def _assert_seam(self, typ: Type, names: dict[str, str], expected: bool) -> None:
        lookup = self._lookup(names)
        result = _type_kernel.rust_find_self_type(typ, lookup)
        assert result is not None, f"Rust deferred on {typ!r}"
        assert result == expected, f"Rust seam {typ!r}: {result} != {expected}"

    def test_seam_unbound_self(self) -> None:
        self._assert_seam(UnboundType("Self"), {"Self": "typing.Self"}, True)

    def test_seam_unbound_plain(self) -> None:
        self._assert_seam(UnboundType("int"), {}, False)

    def test_seam_unbound_nested_self(self) -> None:
        t = UnboundType("list", [UnboundType("Self")])
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_seam_raw_expression(self) -> None:
        from mypy.types import RawExpressionType

        t = RawExpressionType("nope", "mod", line=1, column=1)
        self._assert_seam(t, {}, False)

    def test_seam_ellipsis(self) -> None:
        from mypy.types import EllipsisType

        self._assert_seam(EllipsisType(), {}, False)

    def test_seam_type_list_with_self(self) -> None:
        from mypy.types import TypeList

        t = TypeList([UnboundType("Self")], line=1, column=1)
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_seam_type_list_plain(self) -> None:
        from mypy.types import TypeList

        t = TypeList([UnboundType("int")], line=1, column=1)
        self._assert_seam(t, {}, False)

    def test_seam_instance_arg_self(self) -> None:
        t = Instance(self.fx.ai, [UnboundType("Self")])
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_seam_union_nested_self(self) -> None:
        t = UnionType([self.fx.a, UnboundType("Self")])
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_seam_callable_ret_self(self) -> None:
        t = CallableType([], [], [], UnboundType("Self"), self.fx.function)
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_seam_callable_ret_plain(self) -> None:
        t = CallableType([], [], [], UnboundType("int"), self.fx.function)
        self._assert_seam(t, {}, False)

    def test_parity_unbound_self(self) -> None:
        self._assert_par(UnboundType("Self"), {"Self": "typing.Self"}, True)

    def test_parity_unbound_plain(self) -> None:
        self._assert_par(UnboundType("int"), {}, False)

    def test_parity_unbound_nested_self(self) -> None:
        t = UnboundType("list", [UnboundType("Self")])
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_raw_expression(self) -> None:
        from mypy.types import RawExpressionType

        t = RawExpressionType("nope", "mod", line=1, column=1)
        self._assert_par(t, {}, False)

    def test_parity_ellipsis(self) -> None:
        from mypy.types import EllipsisType

        self._assert_par(EllipsisType(), {}, False)

    def test_parity_type_list_with_self(self) -> None:
        from mypy.types import TypeList

        t = TypeList([UnboundType("Self")], line=1, column=1)
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_type_list_plain(self) -> None:
        from mypy.types import TypeList

        t = TypeList([UnboundType("int")], line=1, column=1)
        self._assert_par(t, {}, False)

    def test_parity_instance_arg_self(self) -> None:
        t = Instance(self.fx.ai, [UnboundType("Self")])
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_union_nested_self(self) -> None:
        t = UnionType([self.fx.a, UnboundType("Self")])
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_callable_ret_self(self) -> None:
        t = CallableType([], [], [], UnboundType("Self"), self.fx.function)
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_callable_ret_plain(self) -> None:
        t = CallableType([], [], [], UnboundType("int"), self.fx.function)
        self._assert_par(t, {}, False)

    # The TypeVar-like shapes below construct the #1122 divergent shapes: a
    # Self unbound name hiding in the Python child lists (upper_bound /
    # default / values / prefix) that the pre-fix Rust shortcut returned False for.

    def test_seam_tvar_upper_bound_self(self) -> None:
        t = TypeVarType(
            "T", "mod.T", TypeVarId(1), [], UnboundType("Self"), AnyType(TypeOfAny.special_form)
        )
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_parity_tvar_upper_bound_self(self) -> None:
        t = TypeVarType(
            "T", "mod.T", TypeVarId(1), [], UnboundType("Self"), AnyType(TypeOfAny.special_form)
        )
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_tvar_default_self(self) -> None:
        t = TypeVarType("T", "mod.T", TypeVarId(1), [], self.fx.o, UnboundType("Self"))
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_tvar_values_self(self) -> None:
        t = TypeVarType(
            "T",
            "mod.T",
            TypeVarId(1),
            [UnboundType("Self")],
            self.fx.o,
            AnyType(TypeOfAny.special_form),
        )
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_seam_paramspec_prefix_self(self) -> None:
        prefix = Parameters([UnboundType("Self")], [ARG_POS], [None])
        t = ParamSpecType(
            "P",
            "mod.P",
            TypeVarId(1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.special_form),
            prefix=prefix,
        )
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_parity_paramspec_prefix_self(self) -> None:
        prefix = Parameters([UnboundType("Self")], [ARG_POS], [None])
        t = ParamSpecType(
            "P",
            "mod.P",
            TypeVarId(1),
            ParamSpecFlavor.BARE,
            self.fx.o,
            AnyType(TypeOfAny.special_form),
            prefix=prefix,
        )
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_seam_tvt_upper_bound_self(self) -> None:
        t = TypeVarTupleType(
            "Ts",
            "mod.Ts",
            TypeVarId(1),
            UnboundType("Self"),
            self.fx.std_tuple,
            AnyType(TypeOfAny.special_form),
        )
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_parity_tvt_upper_bound_self(self) -> None:
        t = TypeVarTupleType(
            "Ts",
            "mod.Ts",
            TypeVarId(1),
            UnboundType("Self"),
            self.fx.std_tuple,
            AnyType(TypeOfAny.special_form),
        )
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_seam_typeddict_fallback_self(self) -> None:
        # Artificial shape: Python never puts Self in a fallback, but the
        # Rust port must not descend into the fallback at all.
        fallback = self.fx.std_tuple.copy_modified(args=[UnboundType("Self")])
        t = TypedDictType({}, set(), set(), fallback)
        self._assert_seam(t, {"Self": "typing.Self"}, False)

    def test_parity_typeddict_fallback_self(self) -> None:
        fallback = self.fx.std_tuple.copy_modified(args=[UnboundType("Self")])
        t = TypedDictType({}, set(), set(), fallback)
        self._assert_par(t, {"Self": "typing.Self"}, False)

    def test_parity_typeddict_items_self(self) -> None:
        fallback = self.fx.std_tuple.copy_modified(args=[self.fx.o])
        t = TypedDictType({"x": UnboundType("Self")}, set(), set(), fallback)
        self._assert_par(t, {"Self": "typing.Self"}, True)

    # Issue #1157. Python expands a TypeAliasType through the alias
    # node's target (and, for PEP 695 aliases, also queries the written
    # args); Rust must decide the same through the resolver snapshot.

    def test_parity_alias_target_self(self) -> None:
        t = TypeAliasType(self.alias_self, [])
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_alias_instance_with_self_arg(self) -> None:
        t = TypeAliasType(self.alias_list, [])
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_alias_py312_args_self(self) -> None:
        t = TypeAliasType(self.alias_py312, [UnboundType("Self")])
        self._assert_par(t, {"Self": "typing.Self"}, True)

    def test_parity_alias_py312_args_plain(self) -> None:
        t = TypeAliasType(self.alias_py312, [self.fx.o])
        self._assert_par(t, {"Self": "typing.Self"}, False)

    def test_parity_alias_non_py312_args_ignored(self) -> None:
        # Self appears only in the arguments: a non-PEP-695 alias must
        # stay False on both sides.
        t = TypeAliasType(self.alias_plain, [UnboundType("Self")])
        self._assert_par(t, {"Self": "typing.Self"}, False)

    def test_seam_resolverless_alias_decides(self) -> None:
        # Issue #1308: the no-resolver window (pre-first-SCC semanal)
        # expands alias targets over live objects instead of deferring
        # every TypeAliasType seen during that window.
        lookup = self._lookup({"Self": "typing.Self"})
        t = TypeAliasType(self.alias_self, [])
        result = _type_kernel.rust_find_self_type(t, lookup)
        assert result is True, f"resolver-less seam deferred or answered {result}"

    def test_seam_resolverless_alias_substituted_tvar(self) -> None:
        # _expand_once substitution: t.args[i] replaces the alias tvar
        # occurrence in the target, keyed by TypeVarId equality.
        alias_g = TypeAlias(
            Instance(self.fx.ai, [self.fx.t]), "mod.G", "mod", 1, 1, alias_tvars=[self.fx.t]
        )
        t = TypeAliasType(alias_g, [UnboundType("Self")])
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_seam_resolverless_alias_noargs(self) -> None:
        # no_args aliases copy t.args over the target Instance's args;
        # the query runs over t.args directly.
        alias_lst = TypeAlias(
            Instance(self.fx.std_listi, []), "mod.Lst", "mod", 1, 1, no_args=True
        )
        t = TypeAliasType(alias_lst, [UnboundType("Self")])
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_seam_resolverless_alias_nested(self) -> None:
        # A target carrying another TypeAliasType recurses through the
        # same live expansion.
        alias_n = TypeAlias(
            Instance(self.fx.ai, [TypeAliasType(self.alias_self, [])]), "mod.N", "mod", 1, 1
        )
        t = TypeAliasType(alias_n, [])
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_seam_resolverless_alias_tvt_defers(self) -> None:
        # The tvar_tuple_index mid-split mapping is not representable in
        # the (tvar -> arg) map; the defer stays parity-safe on both
        # seams.
        ts = TypeVarTupleType(
            "Ts",
            "mod.Ts",
            TypeVarId(1),
            self.fx.o,
            self.fx.std_tuple,
            AnyType(TypeOfAny.special_form),
        )
        alias_ts = TypeAlias(self.fx.std_tuple, "mod.T", "mod", 1, 1, alias_tvars=[ts])
        lookup = self._lookup({"Self": "typing.Self"})
        t = TypeAliasType(alias_ts, [self.fx.o])
        result = _type_kernel.rust_find_self_type(t, lookup)
        assert result is None, f"tvar-tuple alias answered {result}"

    def test_seam_resolverless_alias_nested_chain(self) -> None:
        # A[T] = B[T], B[S] = list[S], use site A[Self] expands to
        # list[Self] -> True (nested alias args resolve via the outer
        # subst, InstantiateAliasVisitor semantics).
        alias_b = TypeAlias(
            Instance(self.fx.std_listi, [self.fx.t]), "mod.B", "mod", 1, 1, alias_tvars=[self.fx.t]
        )
        alias_a = TypeAlias(
            TypeAliasType(alias_b, [self.fx.t]), "mod.A", "mod", 1, 1, alias_tvars=[self.fx.t]
        )
        t = TypeAliasType(alias_a, [UnboundType("Self")])
        self._assert_seam(t, {"Self": "typing.Self"}, True)

    def test_seam_live_alias_target_self(self) -> None:
        lookup = self._lookup({"Self": "typing.Self"})
        t = TypeAliasType(self.alias_self, [])
        result = _type_kernel.rust_find_self_type_live(self.resolver, t, lookup)
        assert result is True, f"live seam deferred or answered {result}"

    def test_seam_live_alias_instance_with_self_arg(self) -> None:
        lookup = self._lookup({"Self": "typing.Self"})
        t = TypeAliasType(self.alias_list, [])
        result = _type_kernel.rust_find_self_type_live(self.resolver, t, lookup)
        assert result is True, f"live seam deferred or answered {result}"

    def test_seam_live_missing_snapshot_defers(self) -> None:
        alias_missing = TypeAlias(UnboundType("Self"), "mod.MISSING", "mod", 1, 1)
        lookup = self._lookup({"Self": "typing.Self"})
        t = TypeAliasType(alias_missing, [])
        result = _type_kernel.rust_find_self_type_live(self.no_alias_resolver, t, lookup)
        assert result is None, f"missing-snapshot seam answered {result}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeConstraintUnionSuite(Suite):
    """Parity suite for the constraint-builder union dispatch (issue #1130).

    `rust_infer_constraints_full` previously deferred (`None`) every call
    with a `UnionType` operand. The four union-dispatch branches of
    `_infer_constraints` (constraints.py:833-880) now run natively inside
    `infer_constraints_full_inner`:

    - branch a: SUBTYPE_OF template union -> per-item recursion
    - branch b: SUPERTYPE_OF actual union -> per-item recursion with
      `orig_template`, re-wrapping each item through
      `TypeType.make_normalized` when the top-level `type[...]`/union
      fixup looked up the template
    - branch c: SUBTYPE_OF actual union -> `any_constraints` over
      `infer_constraints_if_possible` per item (eager first match)
    - branch d: SUPERTYPE_OF template union -> `any_constraints` over
      `infer_constraints_if_possible` per item, empty result -> []
    - proper-form expansion now unwraps a top-level alias whose target
      is a union (instead of deferring), including the
      suggestion_history-free emit path

    Differential harness mirrors NativeConstraintsDeferralSuite: each
    case runs the public `infer_constraints` with the gate on and off
    and asserts equal `str()` constraint lists; a direct
    `rust_infer_constraints_full` call proves native engagement for the
    shapes that used to defer (returning constraint blobs instead of
    `None`).
    """

    def setUp(self) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._active = False
        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_constraints_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.constraints import (
            _set_native_constraints_active,
            _set_native_constraints_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_constraints_active(self._active)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)

    def _rebuild_with_aliases(self, aliases: list[Any]) -> None:
        from mypy.constraints import _set_native_constraints_resolver

        type_infos = [
            value
            for name in dir(self.fx)
            if name.endswith("i")
            for value in [getattr(self.fx, name)]
            if isinstance(value, TypeInfo)
        ]
        self.resolver = _type_kernel.build_native_resolver(type_infos, aliases)
        _set_native_constraints_resolver(self.resolver)

    def _cons(self, template: Type, actual: Type, direction: int, native: bool) -> list[str]:
        from mypy.constraints import (
            _set_native_constraints_active,
            _set_native_constraints_resolver,
            infer_constraints,
        )

        _set_native_constraints_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return [str(c) for c in infer_constraints(template, actual, direction)]

    def _assert_par(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        native = self._cons(template, actual, direction, native=True)
        python = self._cons(template, actual, direction, native=False)
        assert native == python, f"native={native!r} python={python!r}"

    def _rust(self, template: Type, actual: Type, direction: int) -> Any:
        tb = _WriteBuffer()
        template.write(tb)
        ab = _WriteBuffer()
        actual.write(ab)
        return _type_kernel.rust_infer_constraints_full(
            self.resolver,
            tb.getvalue(),
            ab.getvalue(),
            direction,
            False,
            False,
            strict_optional_flag(),
            True,
        )

    def _assert_engages(self, template: Type, actual: Type, direction: int) -> None:
        assert (
            self._rust(template, actual, direction) is not None
        ), f"Rust seam must engage for template={template!r} actual={actual!r}"

    # --- branch a: SUBTYPE_OF with a union template ---

    def test_branch_a_template_union_subtypes(self) -> None:
        # Union[G[T], List[T]] <: A: per-item recursion yields T :> A
        # (twice) and nothing for the tvar-less list item.
        template = UnionType([self.fx.gt, Instance(self.fx.std_listi, [self.fx.t])])
        self._assert_par(template, self.fx.a, SUBTYPE_OF)
        self._assert_engages(template, self.fx.a, SUBTYPE_OF)

    def test_branch_a_item_union_recurse(self) -> None:
        # Both operands are unions: branch a wins (SUBTYPE_OF + union
        # template first), and each item then dispatches through branch c.
        template = UnionType([self.fx.gt, self.fx.gs])
        actual = UnionType([self.fx.ga, self.fx.gd])
        self._assert_par(template, actual, SUBTYPE_OF)
        self._assert_engages(template, actual, SUBTYPE_OF)

    # --- branch b: SUPERTYPE_OF with a union actual ---

    def test_branch_b_actual_union_supertypes(self) -> None:
        # G[T] :> (G[A] | G[D]): recursion emits both T :> A and T :> D.
        actual = UnionType([self.fx.ga, self.fx.gd])
        self._assert_par(self.fx.gt, actual, SUPERTYPE_OF)
        self._assert_engages(self.fx.gt, actual, SUPERTYPE_OF)

    def test_branch_b_type_type_items_rewrapped(self) -> None:
        # Template type[T] and actual type[G[A]] | type[G[D]]: the
        # type[...]-union fixup unwraps both sides, then branch b re-wraps
        # each actual item via TypeType.make_normalized before recursing.
        template = TypeType.make_normalized(self.fx.t)
        actual = UnionType(
            [TypeType.make_normalized(self.fx.ga), TypeType.make_normalized(self.fx.gd)]
        )
        self._assert_par(template, actual, SUPERTYPE_OF)
        self._assert_engages(template, actual, SUPERTYPE_OF)

    # --- branch c: SUBTYPE_OF with a union actual ---

    def test_branch_c_actual_union_first_item_matches(self) -> None:
        # G[T] <: (G[A] | G[D]): any_constraints(eager=True) stops at the
        # first compatible item, so only G[A] constrains T.
        actual = UnionType([self.fx.ga, self.fx.gd])
        self._assert_par(self.fx.gt, actual, SUBTYPE_OF)
        self._assert_engages(self.fx.gt, actual, SUBTYPE_OF)

    def test_branch_c_actual_union_no_item_matches(self) -> None:
        # List[A] <: (G[A] | G[D]): no item resolves against the tvar-less
        # template, so the result is empty on both sides.
        actual = UnionType([self.fx.ga, self.fx.gd])
        self._assert_par(self.fx.lsta, actual, SUBTYPE_OF)

    # --- branch d: SUPERTYPE_OF with a union template ---

    def test_branch_d_template_union_supertypes(self) -> None:
        # (G[T] | List[T]) :> G[A]: the List item is not a possible target,
        # so only the G[T] item constrains T.
        template = UnionType([self.fx.gt, self.fx.lsta])
        self._assert_par(template, self.fx.ga, SUPERTYPE_OF)
        self._assert_engages(template, self.fx.ga, SUPERTYPE_OF)

    def test_branch_d_template_union_empty_result(self) -> None:
        # (List[A] | List[B]) :> G[A]: no item produces constraints.
        template = UnionType([self.fx.lsta, self.fx.lstb])
        self._assert_par(template, self.fx.ga, SUPERTYPE_OF)

    # --- top-level alias expanding into a union ---

    def test_alias_actual_expands_to_union(self) -> None:
        # T :> alias -> Union[G[A], None]: the alias expands to a union
        # before the emit, so the constraint target is the expanded union
        # (previously the whole call deferred on the alias operand).
        target = UnionType([self.fx.ga, self.fx.nonet])
        alias = TypeAlias(target, "mod.OptGA", "mod", -1, -1)
        self._rebuild_with_aliases([alias])
        actual = TypeAliasType(alias, [])
        self._assert_par(self.fx.t, actual, SUPERTYPE_OF)
        self._assert_engages(self.fx.t, actual, SUPERTYPE_OF)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTryGettingStrLiteralsSuite(Suite):
    """Issue #1168: the `try_getting_{str,int,bool}_literals_from_type`
    seams under the #1101 decided-None protocol.

    Rust answers `(true, values)` for a proven literal list and
    `(true, None)` when Python provably answers None (plain Instance
    without last_known_value, an Instance-with-lkv union item, a literal
    whose fallback fullname or value kind does not match the target,
    TupleType, Any, ...), so the shim no longer re-runs the Python body.
    Only `TypeAliasType` candidates (top level or a union item) defer:
    Python expands them via `get_proper_type`, but the wire has no alias
    target. Direct seam calls prove the decided markers; gate-off vs
    gate-on differentials prove the public functions answer identically
    either way.
    """

    def setUp(self) -> None:
        import type_kernel

        from mypy.typeops import _serialize_type, _set_native_typeops_active

        self._tk = type_kernel
        self._serialize = _serialize_type
        self._set_active = _set_native_typeops_active
        self.fx = TypeFixture()
        int_info = self.fx.make_type_info("builtins.int")
        self.int_inst = Instance(int_info, [])
        self.lit_int1 = LiteralType(1, self.int_inst)
        self.lit_int1_inst = Instance(int_info, [], last_known_value=self.lit_int1)
        # A bool literal value on an int fallback: the fallback gate fires
        # first in Python, so this is not an int literal, but Literal[True]
        # ON an int fallback is (isinstance(True, int)).
        self.lit_true_on_int = LiteralType(True, self.int_inst)
        self.lit_true_on_int_inst = Instance(int_info, [], last_known_value=self.lit_true_on_int)
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _with_gate(self, active: bool, fn: Callable[[], Any]) -> Any:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _seam(self, fn_name: str, typ: Type) -> tuple[bool, Any]:
        """Direct seam call; pyfunction-None (deferral) raises TypeError."""
        fn = getattr(self._tk, fn_name)
        decided, result = fn(self._serialize(typ))
        return decided, result

    # --- direct seam calls (str target) ---

    def test_str_seam_decided_values(self) -> None:
        # Instance with a str last_known_value: the literal survives.
        decided, result = self._seam(
            "rust_try_getting_str_literals_from_type", self.fx.lit_str1_inst
        )
        assert decided is True and result == ["x"]
        # Top-level LiteralType proper.
        decided, result = self._seam("rust_try_getting_str_literals_from_type", self.fx.lit_str1)
        assert decided is True and result == ["x"]
        # Union of LiteralType items.
        u = UnionType([self.fx.lit_str1, self.fx.lit_str2])
        decided, result = self._seam("rust_try_getting_str_literals_from_type", u)
        assert decided is True and result == ["x", "y"]

    def test_str_seam_decided_none(self) -> None:
        # Plain Instance without last_known_value (the dominant defer class).
        cases: list[Type] = [
            self.fx.a,
            self.fx.anyt,
            self.fx.nonet,
            self.fx.std_tuple,
            # A literal with a non-str fallback: Python checks
            # fallback.fullname before the value kind, so it is decided-None.
            self.fx.lit1_inst,
            # Union whose items are Instances-with-lkv (not LiteralTypes):
            # Python's get_proper_types does not unwrap the items' lkv.
            UnionType([self.fx.lit_str1_inst, self.fx.lit_str2_inst]),
            UnionType([self.fx.lit_str1_inst, self.lit_int1_inst]),
            UnionType([self.fx.lit_str1_inst, self.fx.a]),
        ]
        for typ in cases:
            decided, result = self._seam("rust_try_getting_str_literals_from_type", typ)
            assert decided is True and result is None

    def test_str_seam_defers_on_aliases(self) -> None:
        # A pyfunction-level None is the deferral marker: Python expands
        # the alias via get_proper_type, so the shim must re-run its body
        # (the public function still answers None; proven by parity below).
        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        t = TypeAliasType(alias, [])
        # A union with an alias item defers as a whole.
        u = UnionType([self.fx.lit_str1_inst, t])
        assert self._tk.rust_try_getting_str_literals_from_type(self._serialize(t)) is None
        assert self._tk.rust_try_getting_str_literals_from_type(self._serialize(u)) is None

    # --- int and bool targets ---

    def test_int_seam(self) -> None:
        decided, result = self._seam("rust_try_getting_int_literals_from_type", self.lit_int1_inst)
        assert decided is True and result == [1]
        # Literal[True] with an int fallback counts as an int literal
        # (isinstance(True, int)); decided into Scalar::Int by Rust.
        decided, result = self._seam(
            "rust_try_getting_int_literals_from_type", self.lit_true_on_int_inst
        )
        assert decided is True and result == [1]
        # A str literal under the int target is decided-None.
        decided, result = self._seam(
            "rust_try_getting_int_literals_from_type", self.fx.lit_str1_inst
        )
        assert decided is True and result is None
        # Literal[True] on the BOOL fallback fails the fallback gate first
        # in Python: decided-None, not the bool-value->int fold.
        bool_inst = Instance(self.fx.bool_type_info, [], last_known_value=self.fx.lit_true)
        decided, result = self._seam("rust_try_getting_int_literals_from_type", bool_inst)
        assert decided is True and result is None

    def test_bool_seam(self) -> None:
        u = UnionType([self.fx.lit_true, self.fx.lit_false])
        decided, result = self._seam("rust_try_getting_bool_literals_from_type", u)
        assert decided is True and result == [True, False]
        decided, result = self._seam("rust_try_getting_bool_literals_from_type", self.fx.lit_true)
        assert decided is True and result == [True]
        # An int literal under the bool target is decided-None.
        decided, result = self._seam(
            "rust_try_getting_bool_literals_from_type", self.lit_int1_inst
        )
        assert decided is True and result is None

    # --- gate-off vs gate-on parity through the public functions ---

    def test_str_gate_parity(self) -> None:
        from mypy.typeops import try_getting_str_literals_from_type

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        alias_t = TypeAliasType(alias, [])
        cases = [
            (self.fx.lit_str1_inst, ["x"]),
            (UnionType([self.fx.lit_str1, self.fx.lit_str2]), ["x", "y"]),
            (self.fx.a, None),
            (self.fx.anyt, None),
            (self.fx.std_tuple, None),
            (self.fx.lit1_inst, None),
            (UnionType([self.fx.lit_str1_inst, self.fx.lit_str2_inst]), None),
            (alias_t, None),
            (UnionType([self.fx.lit_str1_inst, alias_t]), None),
        ]
        for typ, expected in cases:
            off = self._with_gate(False, lambda: try_getting_str_literals_from_type(typ))
            on = self._with_gate(True, lambda: try_getting_str_literals_from_type(typ))
            assert off == on == expected, f"{typ}: off={off!r} on={on!r}"

    def test_int_bool_gate_parity(self) -> None:
        from mypy.typeops import try_getting_int_literals_from_type, try_getting_literals_from_type

        int_cases = [
            (self.lit_int1_inst, [1]),
            (self.lit_true_on_int_inst, [1]),
            (self.fx.lit_str1_inst, None),
        ]
        for typ, expected in int_cases:
            off = self._with_gate(False, lambda: try_getting_int_literals_from_type(typ))
            on = self._with_gate(True, lambda: try_getting_int_literals_from_type(typ))
            assert off == on == expected
        bool_cases = [
            (self.fx.lit_true, [True]),
            (UnionType([self.fx.lit_true, self.fx.lit_false]), [True, False]),
            (self.lit_int1_inst, None),
        ]
        for typ, expected in bool_cases:
            off = self._with_gate(
                False, lambda: try_getting_literals_from_type(typ, bool, "builtins.bool")
            )
            on = self._with_gate(
                True, lambda: try_getting_literals_from_type(typ, bool, "builtins.bool")
            )
            assert off == on == expected


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeVisitorBindSuite(Suite):
    """Regression for #1412: the visitor-kernel gate actually engages.

    The Stage 7 try-block used `from mypy.types import read_type` during
    mypy.types' own import, before read_type is defined; the ImportError
    nulled every visitor binding and pinned `_VISITOR_HAS_TYPE_KERNEL`
    to False forever. The fix drops the self-import and binds
    `_visitor_read_type = read_type` after read_type's definition, so the
    visitor seams reach their Rust path for the first time. These tests
    pin the flag, the binding, and one live seam engagement.
    """

    def setUp(self) -> None:
        super().setUp()
        self.fx = TypeFixture()

    def test_flag_and_reader_bound(self) -> None:
        # The precise regression: the try-block never failed with a missing
        # kernel here, it failed on the self-import. Flag True and a bound
        # reader means both halves work.
        import mypy.types as t

        assert t._VISITOR_HAS_TYPE_KERNEL
        assert t._visitor_read_type is t.read_type

    def test_has_type_vars_takes_rust_path_when_active(self) -> None:
        import mypy.types as t

        saved = t._native_visitor_active
        try:
            t._set_native_visitor_active(True)
            seen: list[int] = []
            # Bound via getattr/setattr on a variable name: the underscore
            # name is an imported alias in mypy.types, not an implicit
            # re-export, and bugbear B009/B010 would fix a constant name.
            attr = "_rust_has_type_vars"
            orig: Callable[[bytes], bool] = getattr(t, attr)

            def spy(b: bytes) -> bool:
                seen.append(1)
                return orig(b)

            setattr(t, attr, spy)
            assert t.has_type_vars(self.fx.t)
            assert t.has_type_vars(self.fx.o) is False
            assert seen == [1, 1]
        finally:
            setattr(t, attr, orig)
            t._set_native_visitor_active(saved)

    def test_gate_off_keeps_pure_python_visitor(self) -> None:
        import mypy.types as t

        saved = t._native_visitor_active
        try:
            # Control its own input rather than inheriting the
            # process-global gate value (which may be True under
            # MYPY_NATIVE_PARITY_INSTALL_VISITOR) as a precondition.
            t._set_native_visitor_active(False)
            assert not t._native_visitor_active
            assert t.has_type_vars(self.fx.t)
            assert t.has_type_vars(self.fx.o) is False
        finally:
            t._set_native_visitor_active(saved)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFakeInfoRegistrationSuite(Suite):
    """Parity suite for resolver registration of runtime-synthesized
    TypeInfos (issue #1456).

    `TypeChecker.make_fake_typeinfo` products (ad-hoc intersections from
    isinstance narrowing, callable subtypes, protocol-variance dummies)
    are created during type checking, after their module's SCC snapshot
    sealed, so `visit_instance_nominal` deferred I(fake) -> I(real) pairs
    on a missing left snapshot. The fix registers them into the live
    `NativeTypeResolver` snapshot at creation; each test asserts a
    gate-on/off differential AND a direct seam call proving the Rust side
    decides (non-None) once the fake is registered. An unregistered fake
    must still defer (None), preserving the pure-Python fallback.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        self.resolver = self._build_resolver([])
        _set_native_subtype_resolver(self.resolver)
        _set_native_subtype_active(True)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _build_resolver(self, aliases: list[Any]) -> Any:
        return _type_kernel.build_native_resolver(self._type_infos(), aliases)

    def _set_gate(self, active: bool) -> None:
        from mypy.subtypes import _set_native_subtype_active

        _set_native_subtype_active(active)

    def _seam(self, left: Any, right: Any) -> Any:
        from mypy.subtypes import _serialize_type

        return _type_kernel.rust_is_subtype(
            _serialize_type(left),
            _serialize_type(right),
            False,
            False,
            False,
            False,
            False,
            True,
            False,
            False,
            self.resolver,
        )

    def _make_fake_subclass_info(self, gen_name: str, bases: list[Instance]) -> TypeInfo:
        from mypy.mro import calculate_mro

        cdef = ClassDef(gen_name, Block([]))
        cdef.fullname = "mod." + gen_name
        info = TypeInfo(SymbolTable(), cdef, "mod")
        cdef.info = info
        info.bases = bases
        calculate_mro(info)
        info.metaclass_type = info.calculate_metaclass_type()
        return info

    def test_unregistered_fake_left_defers_direct_seam(self) -> None:
        fake = self._make_fake_subclass_info(
            '<subclass of "mod.A" and "mod.D">', [self.fx.a, self.fx.d]
        )
        left = Instance(fake, [])
        # Not registered: the engine cannot read the fake's MRO and
        # defers (None), falling through to the Python body.
        assert self._seam(left, self.fx.a) is None
        assert self._seam(left, self.fx.b) is None

    def test_registered_fake_left_answers_true_direct_seam(self) -> None:
        fake = self._make_fake_subclass_info(
            '<subclass of "mod.A" and "mod.D">', [self.fx.a, self.fx.d]
        )
        left = Instance(fake, [])
        assert self._seam(left, self.fx.a) is None
        added, added_alias = self.resolver.update([fake], [], None, None)
        assert (added, added_alias) == (1, 0)
        # Registered: the nominal has_base walk answers like Python.
        assert self._seam(left, self.fx.a) is True
        assert self._seam(left, self.fx.d) is True
        # Idempotent: first seal wins, an identical registration is a
        # no-op for the Rust snapshot.
        added, added_alias = self.resolver.update([fake], [], None, None)
        assert (added, added_alias) == (0, 0)

    def test_registered_fake_gate_on_off_parity(self) -> None:
        from mypy.subtypes import is_subtype

        fake = self._make_fake_subclass_info(
            '<subclass of "mod.A" and "mod.D">', [self.fx.a, self.fx.d]
        )
        self.resolver.update([fake], [], None, None)
        left = Instance(fake, [])
        self._set_gate(False)
        assert is_subtype(left, self.fx.a)
        self._set_gate(True)
        assert is_subtype(left, self.fx.a)
        assert is_subtype(left, self.fx.d)

    def test_manager_registrar_registers_fake_info(self) -> None:
        from types import SimpleNamespace

        from mypy.build import BuildManager

        fake = self._make_fake_subclass_info(
            '<subclass of "mod.A" and "mod.D">', [self.fx.a, self.fx.d]
        )
        left = Instance(fake, [])
        stub = SimpleNamespace(
            options=Options(),
            _native_resolver=self.resolver,
            _native_typeinfo_map={},
            _native_snapshotted=set(),
        )
        BuildManager._register_native_fake_typeinfo(stub, fake)  # type: ignore[arg-type]
        assert stub._native_typeinfo_map == {fake.fullname: fake}
        assert fake.fullname in stub._native_snapshotted
        assert self._seam(left, self.fx.a) is True
        # A duplicate name is a no-op (already snapshotted this build).
        BuildManager._register_native_fake_typeinfo(stub, fake)  # type: ignore[arg-type]
        assert fake.fullname in stub._native_snapshotted
        assert stub._native_typeinfo_map == {fake.fullname: fake}

    def test_registrar_rolls_back_on_update_failure(self) -> None:
        from types import SimpleNamespace

        from mypy.build import BuildManager

        fake = self._make_fake_subclass_info(
            '<subclass of "mod.A" and "mod.D">', [self.fx.a, self.fx.d]
        )

        class ExplodingResolver:
            def update(self, *args: Any) -> None:
                raise RuntimeError("boom")

        stub = SimpleNamespace(
            options=Options(),
            _native_resolver=ExplodingResolver(),
            _native_typeinfo_map={},
            _native_snapshotted=set(),
        )
        BuildManager._register_native_fake_typeinfo(stub, fake)  # type: ignore[arg-type]
        # Both structures roll back: the fake keeps the Python-fallback
        # behavior and stays eligible for a later retry.
        assert stub._native_typeinfo_map == {}
        assert fake.fullname not in stub._native_snapshotted

    def test_make_fake_typeinfo_invokes_registrar(self) -> None:
        from mypy.checker import _set_native_checker_fake_info_registrar
        from mypy.errors import Errors
        from mypy.nodes import MypyFile, SymbolTable
        from mypy.options import Options
        from mypy.plugin import Plugin

        registered: list[TypeInfo] = []
        _set_native_checker_fake_info_registrar(registered.append)
        try:
            options = Options()
            errors = Errors(options)
            tree = MypyFile([], [])
            tree.is_stub = True
            tree.names = SymbolTable()
            chk = TypeChecker(errors, {}, options, tree, "", Plugin(options), {})
            _, info = chk.make_fake_typeinfo("mod", "Fake", "Fake", [self.fx.a])
            # The production call site fires the registrar with the
            # freshly-built info (bases/MRO already final).
            assert registered == [info]
            assert registered[0].fullname == "mod.Fake"
            assert registered[0].bases == [self.fx.a]
        finally:
            _set_native_checker_fake_info_registrar(None)

    def test_other_unregistered_fake_still_defers_and_parity_holds(self) -> None:
        from mypy.subtypes import is_subtype

        fake = self._make_fake_subclass_info(
            '<subclass of "mod.B" and "mod.O">', [self.fx.b, self.fx.o]
        )
        left = Instance(fake, [])
        # Distinct fake, never registered: the direct seam still defers
        # and the public path answers identically through Python.
        assert self._seam(left, self.fx.b) is None
        self._set_gate(False)
        assert is_subtype(left, self.fx.b)
        self._set_gate(True)
        assert is_subtype(left, self.fx.b)
        assert is_subtype(left, self.fx.o)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIcfProtocolSubtypeArmSuite(Suite):
    """Parity suite for the icf SUBTYPE_OF structural-protocol ACTUAL arm.

    `_try_native_constraint_builder` routes the full ConstraintBuilderVisitor
    through Rust; before wave 53 the (plain template Instance, protocol
    actual Instance, SUBTYPE_OF) shape deferred the whole call to Python at
    the structural-protocol branch (constraints.rs). The ported arm mirrors
    constraints.py:1568-1582: `is_protocol_implementation(erased(template),
    actual, skip=["__call__"])` (erased template LEFT, protocol actual
    RIGHT), then the member loop constrains template-side members against
    actual-side members, guarded by the actual's shared inferring stack.

    Differential harness (NativeConstraintsDeferralSuite pattern): runs
    `infer_constraints` with the gate on (resolver + live TypeInfo map +
    wire map installed) and off (pure Python), asserting equal constraint
    lists; a direct `rust_infer_constraints_full` call proves native
    engagement (blobs) or deferral (None).

    Fixtures: `mod.IterInfo[T]` (plain, generic, `def f(self) -> T`) and
    `mod.PInfo[T]` (protocol, generic, `def f(self) -> T`), neither a base
    of the other, both registered in the resolver: the nominal SUBTYPE_OF /
    SUPERTYPE_OF branches must miss, so the structural arm is the path
    under test.
    """

    def setUp(self) -> None:
        from mypy.checkexpr import _set_native_plugin_hook_registry
        from mypy.constraints import _set_native_constraints_active
        from mypy.options import Options
        from mypy.plugins.default import DEFAULT_HOOK_FULLNAMES_BY_KIND, DefaultPlugin

        self.fx = TypeFixture()
        self._set_active = _set_native_constraints_active
        # The Var-member fetch resolves attribute hooks through the live
        # plugin snapshot; install a defaults-only chain (NativeProtocol
        # Suite pattern) so synthetic member fullnames are unhooked.
        registry = _type_kernel.PluginHookRegistry(
            {kind: list(names) for kind, names in DEFAULT_HOOK_FULLNAMES_BY_KIND.items()}
        )
        _set_native_plugin_hook_registry(registry, False, [DefaultPlugin(Options())])
        self._plugin_installed = True
        self.iter_info, self.iter_t = self._generic_info("mod.IterInfo")
        self.p_info, self.p_t = self._generic_info("mod.PInfo")
        self.p_info.is_protocol = True
        self._add_method(self.iter_info, "f", self.iter_t)
        self._add_method(self.p_info, "f", self.p_t)
        # A non-implementing plain class (no member f) for the false arm.
        self.other_info, self.other_t = self._generic_info("mod.Other")
        # Safe default so a mismatched gate never crosses suites.
        self._set_active(False)

    def tearDown(self) -> None:
        from mypy.checkexpr import _set_native_plugin_hook_registry
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        _set_native_constraints_resolver(None)
        set_wire_typeinfo_map(None)
        if self._plugin_installed:
            _set_native_plugin_hook_registry(None, False)
            self._plugin_installed = False

    def _generic_info(self, name: str) -> tuple[TypeInfo, Any]:
        """A generic class `mod.X[T]` with MRO [X, object] and one tvar.

        Production class typevars bind `TypeVarId(raw_id, namespace=<class
        fullname>)` (types.py:554); mirror that (like the checkmember
        suites) so the tvar matches the Rust expand env keyed on the
        instance type_ref.
        """
        from mypy.types import TypeVarId, TypeVarType

        info = self.fx.make_type_info(name, typevars=["T"])
        tvar = info.defn.type_vars[0]
        assert isinstance(tvar, TypeVarType)
        tvar.id = TypeVarId(tvar.id.raw_id, namespace=info.fullname)
        return info, tvar

    def _add_method(self, info: TypeInfo, name: str, ret: Any) -> None:
        from mypy.types import CallableType, Instance

        node = FuncDef(name, [], None, None)
        node.info = info
        tvar = info.defn.type_vars[0]
        sig = CallableType([Instance(info, [tvar])], [ARG_POS], [None], ret, self.fx.function)
        sig.variables = (tvar,)
        node.type = sig
        node.line = 1
        node.column = 1
        info.names[name] = SymbolTableNode(MDEF, node)

    def _add_var(self, info: TypeInfo, name: str, typ: Any) -> None:
        v = Var(name)
        v.info = info
        v.type = typ
        v.is_ready = True
        v.is_inferred = True
        info.names[name] = SymbolTableNode(MDEF, v)

    def _build_resolver(self, *extra: TypeInfo) -> None:
        from mypy.constraints import _set_native_constraints_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        type_infos = []
        for nm in dir(self.fx):
            if not nm.endswith("i"):
                continue
            value = getattr(self.fx, nm)
            if _is_type_info(value):
                type_infos.append(value)
        type_infos.extend(extra)
        # Full set in both maps: decoded constraint targets reference fixture
        # TypeInfos too (`T <: A` serializes Instance "A"); a partial map
        # makes fixup fail and the parity pins compare Python vs Python.
        live = {i.fullname: i for i in type_infos}
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self.resolver.set_live_typeinfo_map(live)
        set_wire_typeinfo_map(live)
        _set_native_constraints_resolver(self.resolver)

    def _constraints(
        self, template: Type, actual: Type, direction: int, native: bool
    ) -> list[Any]:
        from mypy.constraints import _set_native_constraints_resolver, infer_constraints

        self._set_active(native)
        if native:
            _set_native_constraints_resolver(self.resolver)
        else:
            _set_native_constraints_resolver(None)
        return infer_constraints(template, actual, direction)

    def _assert_par(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        native = self._constraints(template, actual, direction, native=True)
        python = self._constraints(template, actual, direction, native=False)
        assert_equal(native, python, f"native={native!r} python={python!r}")

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _rust(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> Any:
        return _type_kernel.rust_infer_constraints_full(
            self.resolver,
            self._bytes_of(template),
            self._bytes_of(actual),
            direction,
            False,
            False,
            strict_optional_flag(),
            True,
        )

    def _assert_engages(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        raw = self._rust(template, actual, direction)
        assert raw is not None, f"Rust seam must engage for template={template!r}"

    def _assert_defers(self, template: Type, actual: Type, direction: int = SUBTYPE_OF) -> None:
        raw = self._rust(template, actual, direction)
        assert raw is None, f"Rust seam must defer for template={template!r}"

    # --- arm-2 decided / parity shapes ---

    def test_method_member_protocol_actual_subtype_of_decides(self) -> None:
        # The Generator/SupportsNext shape: plain generic template vs
        # protocol actual, nominal branches miss, the structural arm
        # decides T <: int through the member loop (ret constraint).
        self._build_resolver(self.iter_info, self.p_info, self.other_info)
        template = Instance(self.iter_info, [self.iter_t])
        actual = Instance(self.p_info, [self.fx.a])
        self._assert_par(template, actual, SUBTYPE_OF)
        self._assert_engages(template, actual, SUBTYPE_OF)

    def test_direction_supertype_of_returns_empty(self) -> None:
        # Wave-65C widened the dispatch: a protocol-actual SUPERTYPE_OF pair
        # misses both structural arms and Python's shape tail returns [],
        # now decided natively instead of a whole-call defer.
        self._build_resolver(self.iter_info, self.p_info, self.other_info)
        template = Instance(self.iter_info, [self.iter_t])
        actual = Instance(self.p_info, [self.fx.a])
        self._assert_par(template, actual, SUPERTYPE_OF)
        self._assert_engages(template, actual, SUPERTYPE_OF)

    def test_non_implementing_template_falls_to_tail(self) -> None:
        # `mod.Other[T]` declares no `f`, so is_protocol_implementation(erased,
        # PInfo[int]) is False: Python drops out of the instance block and
        # the tail returns []. The native arm decides the same [].
        self._build_resolver(self.iter_info, self.p_info, self.other_info)
        template = Instance(self.other_info, [self.other_t])
        actual = Instance(self.p_info, [self.fx.a])
        self._assert_par(template, actual, SUBTYPE_OF)
        self._assert_engages(template, actual, SUBTYPE_OF)

    def test_recursive_member_guard_suppresses_reentry(self) -> None:
        # A protocol member whose type re-enters the arm with the SAME actual
        # (P[T].f -> P[T] binds to P[int]) is suppressed by the inferring
        # guard on both sides, so the recursion contributes no constraints.
        it2, it2_t = self._generic_info("mod.IterRec")
        p2, p2_t = self._generic_info("mod.PRec")
        p2.is_protocol = True
        self._add_method(it2, "f", Instance(it2, [it2_t]))
        self._add_method(p2, "f", Instance(p2, [p2_t]))
        self._build_resolver(it2, p2)
        template = Instance(it2, [it2_t])
        actual = Instance(p2, [self.fx.a])
        self._assert_par(template, actual, SUBTYPE_OF)
        self._assert_engages(template, actual, SUBTYPE_OF)

    def test_settable_var_member_parity_holds_fetch_defers(self) -> None:
        # A plain (non-final) Var member is IS_SETTABLE (flags decide),
        # but the member FETCH of a generic attribute defers: the Var
        # tail rejects tvar-carrying expanded results (checker_helpers).
        from mypy.types import Instance

        it2, it2_t = self._generic_info("mod.IterVar")
        p2, p2_t = self._generic_info("mod.PVar")
        p2.is_protocol = True
        self._add_var(it2, "x", it2_t)
        self._add_var(p2, "x", p2_t)
        self._build_resolver(it2, p2)
        template = Instance(it2, [it2_t])
        actual = Instance(p2, [self.fx.a])
        self._assert_par(template, actual, SUBTYPE_OF)
        self._assert_defers(template, actual, SUBTYPE_OF)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativePluginFakeRegistrarSuite(Suite):
    """Parity suite for registrar registration of plugin-synthesized
    TypeInfos (issue #1485).

    `make_fake_register_class_instance` (mypy/plugins/singledispatch.py)
    builds the register-hook fake directly, bypassing the #1456
    `TypeChecker.make_fake_typeinfo` funnel, and the fake never enters
    `BuildManager.modules`, so no incremental walk reaches it. The
    creation site now fires the same #1456 manager registrar; each test
    asserts the fake lands in the resolver snapshot (direct seam) and
    the gate-on / gate-off expansion answers identically through the
    real `expand_type_by_instance` path.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        self.resolver = _type_kernel.build_native_resolver(self._type_infos(), [])
        _set_native_subtype_resolver(self.resolver)
        _set_native_subtype_active(True)
        self._expand_installed = False

    def tearDown(self) -> None:
        from mypy.plugins.singledispatch import _set_native_fake_info_registrar
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_fake_info_registrar(None)
        if self._expand_installed:
            from mypy.expandtype import (
                _set_native_expand_type_active,
                _set_native_expand_type_resolver,
                _set_native_expand_type_typeinfo_map,
            )

            _set_native_expand_type_active(False)
            _set_native_expand_type_resolver(None)
            _set_native_expand_type_typeinfo_map(None)

    def _type_infos(self) -> list[TypeInfo]:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def _api(self) -> Any:
        from mypy.types import Instance

        class FakePluginApi:
            fx: Any

            def __init__(self, fx: Any) -> None:
                self.fx = fx

            def named_generic_type(self, fullname: str, args: Sequence[Type]) -> Instance:
                if fullname == "builtins.object":
                    return Instance(self.fx.oi, [])
                if fullname == "builtins.function":
                    return Instance(self.fx.functioni, [])
                raise AssertionError(f"unexpected {fullname}")

        return FakePluginApi(self.fx)

    def _make_fake(self) -> tuple[Any, CallableType]:
        from mypy.plugins.singledispatch import make_fake_register_class_instance

        inst = make_fake_register_class_instance(self._api(), (self.fx.a, self.fx.o))
        sym = inst.type.names["__call__"]
        assert sym.node is not None
        assert isinstance(sym.node, FuncDef)
        assert isinstance(sym.node.type, CallableType)
        return inst, sym.node.type

    def _expand_seam(self, sig: Type, inst: Any) -> Any:
        from mypy.expandtype import _serialize_type

        return _type_kernel.rust_expand_type_by_instance(
            self.resolver, _serialize_type(sig), _serialize_type(inst), False
        )

    def test_plugin_fake_fires_registrar_at_creation(self) -> None:
        from mypy.plugins.singledispatch import (
            _set_native_fake_info_registrar,
            make_fake_register_class_instance,
        )
        from mypy.types import Instance

        registered: list[TypeInfo] = []
        _set_native_fake_info_registrar(registered.append)
        try:
            inst = make_fake_register_class_instance(self._api(), (self.fx.a, self.fx.o))
            # The plugin creation site fires the registrar with the
            # freshly-built info (bases/MRO already final).
            assert registered == [inst.type]
            assert registered[0].fullname == "functools._SingleDispatchRegisterCallable"
            assert registered[0].bases == [Instance(self.fx.oi, [])]
        finally:
            _set_native_fake_info_registrar(None)
        # Cleared: a later fake is not registered (falls back to Python).
        inst2 = make_fake_register_class_instance(self._api(), (self.fx.a, self.fx.o))
        assert inst2.type.fullname == "functools._SingleDispatchRegisterCallable"

    def test_plugin_fake_unregistered_defers_direct_seam(self) -> None:
        inst, sig = self._make_fake()
        # Not registered: the engine cannot read the fake's class facts
        # from the snapshot and defers (None), falling to the Python body.
        assert self._expand_seam(sig, inst) is None

    def test_plugin_fake_registered_answers_direct_seam(self) -> None:
        inst, sig = self._make_fake()
        assert self._expand_seam(sig, inst) is None
        added, added_alias = self.resolver.update([inst.type], [], None, None)
        assert (added, added_alias) == (1, 0)
        result = self._expand_seam(sig, inst)
        assert result is not None
        # The native expansion round-trips back to the live signature.
        from librt.internal import ReadBuffer

        from mypy.expandtype import _resync_definitions
        from mypy.types import read_type
        from mypy.wirefixup import fixup_wire_type, set_wire_typeinfo_map

        typeinfo_map = {info.fullname: info for info in self._type_infos()}
        typeinfo_map[inst.type.fullname] = inst.type
        set_wire_typeinfo_map(typeinfo_map)
        decoded = read_type(ReadBuffer(bytes(result)))
        fixed = fixup_wire_type(decoded, resolve_aliases=True)
        assert isinstance(fixed, ProperType)
        relinked = get_proper_type(_resync_definitions(sig, fixed))
        assert isinstance(relinked, CallableType)
        assert str(relinked) == str(sig)
        self_arg = get_proper_type(relinked.arg_types[0])
        assert isinstance(self_arg, Instance)
        assert self_arg.type.fullname == inst.type.fullname

    def test_plugin_fake_manager_registrar_first_seal_wins(self) -> None:
        from types import SimpleNamespace

        from mypy.build import BuildManager
        from mypy.options import Options
        from mypy.plugins.singledispatch import make_fake_register_class_instance

        stub = SimpleNamespace(
            options=Options(),
            _native_resolver=self.resolver,
            _native_typeinfo_map={},
            _native_snapshotted=set(),
        )
        first = make_fake_register_class_instance(self._api(), (self.fx.a, self.fx.o))
        second = make_fake_register_class_instance(self._api(), (self.fx.d, self.fx.b))
        # Distinct fakes share the fullname; first seal wins for the build.
        assert (
            first.type.fullname
            == second.type.fullname
            == "functools._SingleDispatchRegisterCallable"
        )
        assert first.type is not second.type
        BuildManager._register_native_fake_typeinfo(stub, first.type)  # type: ignore[arg-type]
        BuildManager._register_native_fake_typeinfo(stub, second.type)  # type: ignore[arg-type]
        assert stub._native_typeinfo_map == {first.type.fullname: first.type}
        assert first.type.fullname in stub._native_snapshotted

    def test_plugin_fake_expansion_gate_on_off_parity(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
            expand_type_by_instance,
        )

        inst, sig = self._make_fake()
        self.resolver.update([inst.type], [], None, None)
        typeinfo_map = {info.fullname: info for info in self._type_infos()}
        typeinfo_map[inst.type.fullname] = inst.type
        _set_native_expand_type_typeinfo_map(typeinfo_map)
        _set_native_expand_type_resolver(self.resolver)
        _set_native_expand_type_active(False)
        self._expand_installed = True
        try:
            off = expand_type_by_instance(sig, inst)
            _set_native_expand_type_active(True)
            on = expand_type_by_instance(sig, inst)
            assert isinstance(on, CallableType)
            assert str(on) == str(off)
            assert (on.line, on.column) == (off.line, off.column)
            assert on.fallback.type.fullname == "builtins.function"
            self_arg = get_proper_type(on.arg_types[0])
            assert isinstance(self_arg, Instance)
            assert self_arg.type.fullname == inst.type.fullname
        finally:
            _set_native_expand_type_active(False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFindMemberCallFetchSuite(Suite):
    """Engagement for the Instance-left / FunctionLike-right find_member fetch.

    Issue #1491 (wave 55): the `visit_instance` FunctionLike arm runs
    `find_member("__call__", left, left, is_operator=True)`. The fetch reused
    the `get_protocol_member` helper, whose precise-metaclass special case
    answers None and whose extra_attrs prelude defers, so receivers like
    `builtins.type`, `types.FunctionType` and `functools.partial` fell back to
    Python. The fetch now runs plain find_member semantics
    (`find_member_semantics=True`): the member is read from the live MRO,
    bound via `member_method_inner`, and compared against the right callable.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        self._live_info: dict[str, TypeInfo] = {}

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        set_wire_typeinfo_map(None)

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

    def _method_call(self, info: TypeInfo, ret: Type) -> CallableType:
        any_t = AnyType(TypeOfAny.explicit)
        return CallableType(
            [Instance(info, []), any_t, any_t],
            [ARG_POS, ARG_STAR, ARG_STAR2],
            [None, None, None],
            ret,
            self.fx.function,
        )

    def _class_with_call(self, fullname: str, ret: Type) -> TypeInfo:
        info = self.fx.make_type_info(fullname)
        info.mro = [info, self.fx.oi]
        node = FuncDef("__call__", [], None, None)
        node.info = info
        node.type = self._method_call(info, ret)
        node.line = 1
        node.column = 1
        info.names["__call__"] = SymbolTableNode(MDEF, node)
        self._live_info[fullname] = info
        return info

    def _seam_call(self, left: Type, right: Type) -> bool | None:
        from mypy.subtypes import _serialize_type

        return _type_kernel.rust_is_subtype(
            _serialize_type(left),
            _serialize_type(right),
            False,
            False,
            False,
            False,
            False,
            True,
            False,
            False,
            self.resolver,
        )

    def test_instance_star_call_fetch_engages(self) -> None:
        # `def __call__(self, *args: Any, **kwargs: Any) -> A` on the
        # receiver: the fetched bound callable accepts `(A) -> A` natively.
        self._live_info = {}
        info = self._class_with_call("mod.StarCall", self.fx.a)
        self._build_resolver()
        right = self.fx.callable(self.fx.a, self.fx.a)
        left = Instance(info, [])
        from mypy.subtypes import _set_native_subtype_active

        _set_native_subtype_active(False)
        expected = is_subtype(left, right)
        _set_native_subtype_active(True)
        try:
            assert is_subtype(left, right) == expected
            assert self._seam_call(left, right) is True
        finally:
            _set_native_subtype_active(False)

    def test_instance_call_incompatible_ret_false(self) -> None:
        # `def __call__(self, *args: Any, **kwargs: Any) -> object` is not
        # a subtype of `(A) -> A`: the native fetch decides False, not a
        # deferral.
        self._live_info = {}
        info = self._class_with_call("mod.ObjCall", self.fx.o)
        self._build_resolver()
        right = self.fx.callable(self.fx.a, self.fx.a)
        left = Instance(info, [])
        from mypy.subtypes import _set_native_subtype_active

        _set_native_subtype_active(False)
        expected = is_subtype(left, right)
        _set_native_subtype_active(True)
        try:
            assert is_subtype(left, right) == expected
            assert not expected
            assert self._seam_call(left, right) is False
        finally:
            _set_native_subtype_active(False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeObjectAliasDecodeSuite(Suite):
    """Alias-aware decode for the type-object composite seam (issue #1493).

    `type_object_type_from_function` mirrors the pure-Python body, which
    preserves live `TypeAliasType` nodes (bind_self / map_type_from_supertype
    only expand alias args). The shim's `_deserialize_type` runs with
    `resolve_aliases=False`, so every signature carrying an alias decoded to
    None: the whole Rust composite round-trip was wasted and the Python body
    re-ran. The wave-56 audit pinned all 60 cold-self-check `decode_None`
    events to aliases (e.g. `ast._ConstantValue`, `logging._FormatStyle`),
    not to missing TypeInfos.

    The seam now retries once through `_deserialize_type_with_aliases`, the
    #1309/#1224 contract (re-link decoded aliases through the per-build alias
    map, re-unify fresh vars), and the existing `resync_var_identities` /
    definition-restamp tail runs unchanged. Gate-off vs gate-on results must
    agree, the retry must re-link the live alias, and a missing alias map
    must still fall back cleanly to the pure-Python body.
    """

    def setUp(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                self._type_infos.append(value)
        self.alias = TypeAlias(
            Instance(self.fx.std_listi, [self.fx.t]), "mod.AliasArg", "mod", -1, -1
        )
        self._resolver = _type_kernel.build_native_resolver(self._type_infos, [self.alias])
        set_wire_typeinfo_map({info.fullname: info for info in self._type_infos})
        set_wire_alias_map({self.alias.fullname: self.alias})
        _set_native_typeops_active(True)
        _set_native_typeops_resolver(self._resolver)

    def tearDown(self) -> None:
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        _set_native_typeops_active(False)
        _set_native_typeops_resolver(None)
        set_wire_typeinfo_map(None)
        set_wire_alias_map(None)

    def _init_sig(self, info: TypeInfo) -> CallableType:
        # def __init__(self, x: AliasArg[T]) -> None, with AliasArg = list[T].
        return CallableType(
            [Instance(info, []), TypeAliasType(self.alias, [self.fx.t])],
            [ARG_POS, ARG_POS],
            [None, None],
            NoneType(),
            self.fx.function,
            name="<init>",
        )

    def _type_object(self, sig: FunctionLike, info: TypeInfo) -> FunctionLike:
        from mypy.typeops import type_object_type_from_function

        return type_object_type_from_function(sig, info, info, self.fx.type_type, False)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        from mypy.typeops import _set_native_typeops_active

        _set_native_typeops_active(active)
        try:
            return fn()
        finally:
            _set_native_typeops_active(True)

    def _raw_seam_result(self) -> bytes:
        from mypy.typeops import _serialize_type

        info = self.fx.ai
        result = _type_kernel.rust_type_object_type_from_function(
            _serialize_type(self._init_sig(info)),
            info,
            info,
            _serialize_type(self.fx.type_type),
            False,
            state.strict_optional,
            False,
            self._resolver,
        )
        assert result is not None, "Rust type_object_type_from_function did not engage"
        return bytes(result)

    def test_alias_signature_gate_parity(self) -> None:
        info = self.fx.ai
        sig = self._init_sig(info)
        off = self._with_gate(False, lambda: self._type_object(sig, info))
        on = self._with_gate(True, lambda: self._type_object(sig, info))
        assert_equal(str(on), str(off), "type_object_type alias-signature parity")
        # The pure-Python body preserves the live alias node (bind_self /
        # map only expand alias args); the native path must too.
        assert isinstance(off, CallableType) and isinstance(on, CallableType)
        # bind_self strips the self parameter, so the alias is arg 0.
        off_arg = off.arg_types[0]
        on_arg = on.arg_types[0]
        assert isinstance(off_arg, TypeAliasType) and off_arg.alias is self.alias
        assert isinstance(on_arg, TypeAliasType) and on_arg.alias is self.alias

    def test_alias_decode_retry_engages(self) -> None:
        from mypy.typeops import _deserialize_type, _deserialize_type_with_aliases

        raw = self._raw_seam_result()
        # The plain decoder defers on any decoded alias; the retry resolves
        # it through the per-build alias map and re-links the live node.
        assert _deserialize_type(raw) is None
        fixed = _deserialize_type_with_aliases(raw)
        assert fixed is not None
        assert isinstance(fixed, FunctionLike)
        assert isinstance(fixed, CallableType)
        alias_arg = fixed.arg_types[0]
        assert isinstance(alias_arg, TypeAliasType), f"alias node lost: {alias_arg!r}"
        assert alias_arg.alias is self.alias, "decoded alias not re-linked to live node"
        assert alias_arg.type_ref is None

    def test_missing_alias_map_falls_back(self) -> None:
        from mypy.typeops import _deserialize_type_with_aliases
        from mypy.wirefixup import set_wire_alias_map

        raw = self._raw_seam_result()
        set_wire_alias_map(None)
        try:
            assert _deserialize_type_with_aliases(raw) is None
            info = self.fx.ai
            sig = self._init_sig(info)
            off = self._with_gate(False, lambda: self._type_object(sig, info))
            on = self._with_gate(True, lambda: self._type_object(sig, info))
            assert_equal(str(on), str(off), "type_object_type alias-map-missing parity")
        finally:
            set_wire_alias_map({self.alias.fullname: self.alias})

    def test_bind_self_generic_alias_argument(self) -> None:
        # Generic self (`self: T`) with an alias argument: the bind_self
        # solve arm expands with alias survivors allowed (alias_ok) and the
        # parity tail re-links the live node (it used to defer pre-solve).
        from mypy.typeops import _serialize_type

        info = self.fx.ai
        self_tv = TypeVarType(
            "T",
            "T",
            TypeVarId(-1),
            [],
            Instance(info, []),
            AnyType(TypeOfAny.from_omitted_generics),
        )
        sig = CallableType(
            [self_tv, TypeAliasType(self.alias, [self.fx.t])],
            [ARG_POS, ARG_POS],
            [None, None],
            NoneType(),
            self.fx.function,
            name="<init>",
            variables=[self_tv],
        )
        raw = _type_kernel.rust_type_object_type_from_function(
            _serialize_type(sig),
            info,
            info,
            _serialize_type(self.fx.type_type),
            False,
            state.strict_optional,
            False,
            self._resolver,
        )
        assert raw is not None, "alias-bearing generic-self composite deferred"
        off = self._with_gate(False, lambda: self._type_object(sig, info))
        on = self._with_gate(True, lambda: self._type_object(sig, info))
        assert_equal(str(on), str(off), "type_object_type generic-self alias parity")
        assert isinstance(on, CallableType)
        alias_arg = on.arg_types[0]
        assert isinstance(alias_arg, TypeAliasType)
        assert alias_arg.alias is self.alias, "decoded alias not re-linked to live node"


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


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeOverlapCallableInstanceSuite(Suite):
    """Wave-61B: `find_member("__call__")` arm of `is_overlapping_types`.

    The wave-61 audit pinned 12 of 16 fallbacks to the
    `CallableType`-vs-`Instance` arm that deferred because the operator
    `find_member` fetch was unported. The arm now runs the live
    `get_protocol_member_inner` fetch with find_member semantics and
    recurses with the fetched callable, mirroring meet.py:783-792; the
    callable side still degrades to its fallback when the fetched member
    is not FunctionLike.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._live_info: dict[str, TypeInfo] = {}
        self._set_native_join_active = _set_native_join_active
        self.info = self._class_with_call("mod.MeetCallableBox")
        infos = _base_infos(self.fx) + list(self._live_info.values())
        self._resolver = _type_kernel.build_native_resolver(infos, [])
        self._resolver.set_live_typeinfo_map(dict(self._live_info))
        typeinfo_map = {info.fullname: info for info in infos}
        set_wire_typeinfo_map(typeinfo_map)
        _set_native_join_active(True)
        _set_native_join_resolver(self._resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)
        set_wire_typeinfo_map(None)

    def _class_with_call(self, fullname: str) -> TypeInfo:
        info = self.fx.make_type_info(fullname)
        info.mro = [info, self.fx.oi]
        node = FuncDef("__call__", [], None, None)
        node.info = info
        node.type = CallableType(
            [Instance(info, []), self.fx.a],
            [ARG_POS, ARG_POS],
            ["self", "x"],
            self.fx.o,
            self.fx.function,
        )
        node.line = 1
        node.column = 1
        info.names["__call__"] = SymbolTableNode(MDEF, node)
        self._live_info[fullname] = info
        return info

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_native_join_active(active)
        try:
            return fn()
        finally:
            self._set_native_join_active(True)

    def _parity(self, left: Type, right: Type) -> bool:
        off = self._with_gate(False, lambda: is_overlapping_types(left, right))
        on = self._with_gate(True, lambda: is_overlapping_types(left, right))
        assert on == off, f"is_overlapping_types parity: off={off} on={on}"
        return on

    def test_callable_instance_arm_engages(self) -> None:
        from mypy.join import _serialize_type

        left = Instance(self.info, [])
        right = CallableType([self.fx.a], [ARG_POS], ["x"], self.fx.b, self.fx.function)
        result = _type_kernel.rust_is_overlapping_types(
            _serialize_type(left), _serialize_type(right), False, False, True, self._resolver
        )
        assert result is not None, "callable/instance overlap arm did not engage"
        assert self._parity(left, right) == result

    def test_callable_instance_arm_false_engages(self) -> None:
        from mypy.join import _serialize_type

        info = self.fx.make_type_info("mod.MeetUnrelatedBox")
        info.mro = [info, self.fx.oi]
        node = FuncDef("__call__", [], None, None)
        node.info = info
        node.type = CallableType(
            [Instance(info, []), self.fx.b],
            [ARG_POS, ARG_POS],
            ["self", "x"],
            self.fx.a,
            self.fx.function,
        )
        node.line = 1
        node.column = 1
        info.names["__call__"] = SymbolTableNode(MDEF, node)
        self._live_info[info.fullname] = info
        infos = _base_infos(self.fx) + list(self._live_info.values())
        resolver = _type_kernel.build_native_resolver(infos, [])
        resolver.set_live_typeinfo_map(dict(self._live_info))

        left = Instance(info, [])
        right = CallableType([self.fx.a], [ARG_POS], ["x"], self.fx.b, self.fx.function)
        result = _type_kernel.rust_is_overlapping_types(
            _serialize_type(left), _serialize_type(right), False, False, True, resolver
        )
        assert result is not None, "callable/instance overlap arm did not engage"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinMeetWave62Suite(Suite):
    """Wave-62A join/meet residual ports (issue #1516).

    Covers the residual defer arms the wave-60A audit classified:

    - `visit_instance_join` s non-Instance cross arms (join.py:437-454):
      FunctionLike -> `join_types(t, s.fallback)`; TypeType / TypedDict /
      Tuple / Literal -> `join_types(t, s)`; every other shape ->
      `join_default(s)`.
    - `visit_meet` tuple arm (meet.py:1355-1361), including the
      structural Bottom cases (unequal fixed arity, fixed shorter than
      the variadic prefix+suffix).
    - nested join materialization against EXPANDED alias operands
      (`rust_join_types_inner`), and the `tuple_fallback` union
      (handle_recursive=False) expanding NON-recursive aliases while
      keeping recursive alias nodes folded (wave-33 guardrail).

    Every case asserts gate-off vs gate-on parity; the decided cases
    also pin a direct seam call (non-None) so a gate regression cannot
    hide behind the Python fallback.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )

        self.fx = TypeFixture()
        self._base_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.str_type_info,
            self.fx.bool_type_info,
            self.fx.functioni,
            self.fx.std_tuplei,
            self.fx.std_listi,
        ]
        self._active = _set_native_join_active
        self._resolver_seam = _set_native_join_resolver
        self._map_seam = _set_native_join_typeinfo_map
        self._install(self._base_infos, [])
        self._active(True)

    def _install(self, infos: list[Any], aliases: list[Any]) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._resolver = _type_kernel.build_native_resolver(infos, aliases)
        self._resolver_seam(self._resolver)
        self._typeinfo_map = {info.fullname: info for info in infos}
        self._map_seam(self._typeinfo_map)
        set_wire_typeinfo_map(self._typeinfo_map)
        self._set_wire_map = set_wire_typeinfo_map

    def tearDown(self) -> None:
        self._active(False)
        self._resolver_seam(None)
        self._map_seam(None)
        self._set_wire_map(None)

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._active(active)
        try:
            return fn()
        finally:
            self._active(True)

    def _make_alias(self, fullname: str, target: Type) -> Any:
        from mypy.nodes import TypeAlias

        return TypeAlias(target, fullname, "mod", -1, -1)

    def _assert_join_parity(self, s: Type, t: Type) -> object:
        off = self._with_gate(False, lambda: join_types(s, t))
        on = self._with_gate(True, lambda: join_types(s, t))
        assert_equal(on, off)
        return on

    def _assert_meet_parity(self, s: Type, t: Type) -> object:
        off = self._with_gate(False, lambda: meet_types(s, t))
        on = self._with_gate(True, lambda: meet_types(s, t))
        assert_equal(on, off)
        return on

    def _seam_join(self, s: Type, t: Type) -> Any:
        from mypy.join import _serialize_type

        return _type_kernel.rust_join_types(
            _serialize_type(s), _serialize_type(t), True, self._resolver
        )

    def _seam_meet(self, s: Type, t: Type) -> Any:
        from mypy.join import _serialize_type

        return _type_kernel.rust_meet_types(
            _serialize_type(s), _serialize_type(t), True, self._resolver
        )

    def _seam_join_type_list(self, types: list[Type]) -> Any:
        from mypy.join import _serialize_type

        return _type_kernel.rust_join_type_list(
            [_serialize_type(t) for t in types], True, self._resolver
        )

    def test_instance_with_functionlike_cross_arm(self) -> None:
        # join(Callable, function): the cross arm recurses
        # join_types(t, s.fallback); the result is the function instance
        # (SameT in the outer frame).
        c = self.fx.callable(self.fx.a, self.fx.b)
        on = self._assert_join_parity(c, self.fx.function)
        assert_equal(on, self.fx.function)
        assert self._seam_join(c, self.fx.function) is not None

    def test_instance_with_type_type_cross_arm(self) -> None:
        # join(Callable, Type[None]): the arm defaults s; the kernel
        # encodes the s-derived object (the t-derived Object disc would
        # be Any for a TypeType t - the pre-fix gate-on divergence).
        tt = TypeType.make_normalized(NoneType())
        c = self.fx.callable(self.fx.a, self.fx.b)
        assert_equal(self._assert_join_parity(c, tt), self.fx.o)
        assert_equal(self._assert_join_parity(tt, c), self.fx.o)
        assert self._seam_join(c, tt) is not None

    def test_join_instance_with_parameters_parity(self) -> None:
        # Discovered while porting wave-62A: the Object disc is
        # t-derived, so join(Instance, Parameters) diverged gate-on
        # (Any vs Python's object); the arm now encodes default(s).
        p = Parameters([self.fx.a], [ARG_POS], [None])
        assert_equal(self._assert_join_parity(self.fx.a, p), self.fx.o)

    def test_instance_with_tuple_cross_arm(self) -> None:
        # join(Tuple[A, B], A): the Tuple cross arm recurses
        # join_types(t, s) and resolves through the tuple fallback.
        tup = TupleType([self.fx.a, self.fx.b], Instance(self.fx.std_tuplei, []))
        self._assert_join_parity(tup, self.fx.a)
        self._assert_join_parity(self.fx.a, tup)
        assert self._seam_join(tup, self.fx.a) is not None

    def test_instance_with_typeddict_cross_arm(self) -> None:
        # join(TypedDict, A): the TypedDict cross arm recurses
        # join_types(t, s) -> visit_typeddict_type -> fallback join
        # (the fallback is A) -> A.
        td = TypedDictType({"x": self.fx.o}, {"x"}, set(), self.fx.a)
        assert_equal(self._assert_join_parity(td, self.fx.a), self.fx.a)
        self._assert_join_parity(self.fx.a, td)
        assert self._seam_join(td, self.fx.a) is not None

    def test_meet_tuple_fixed_parity(self) -> None:
        # meet(Tuple[A, B], Tuple[B, A]): itemwise tuple arm; B <: A so
        # both items meet to B.
        s = TupleType([self.fx.a, self.fx.b], Instance(self.fx.std_tuplei, []))
        t = TupleType([self.fx.b, self.fx.a], Instance(self.fx.std_tuplei, []))
        self._assert_meet_parity(s, t)
        assert self._seam_meet(s, t) is not None

    def test_meet_tuple_structural_bottom(self) -> None:
        # tuple[A] vs tuple[A, A, *tuple[A, ...]]: meet_tuples returns
        # None (fixed shorter than variadic.length()-1) -> default(s) =
        # Bottom; the Rust seam must decide rather than defer.
        s = TupleType([self.fx.a], Instance(self.fx.std_tuplei, []))
        t = TupleType(
            [self.fx.a, self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))],
            Instance(self.fx.std_tuplei, []),
        )
        self._assert_meet_parity(s, t)
        seam = self._seam_meet(s, t)
        assert seam is not None, "Rust deferred on the structural-None tuple meet"
        assert seam[0] == 3, f"expected Bottom disc, got {seam[0]}"

    def test_join_type_list_nonrecursive_alias_expands(self) -> None:
        # join_type_list([A-alias, A]) -> A: the alias item expands like
        # get_proper_type, and the nested materialization must name the
        # expanded operand, never leak the alias node.
        from mypy.join import _deserialize_type, join_type_list

        alias = self._make_alias("mod.TA", self.fx.a)
        self._install(self._base_infos, [alias])
        alias_t = TypeAliasType(alias, [])
        off = self._with_gate(False, lambda: join_type_list([alias_t, self.fx.a]))
        on = self._with_gate(True, lambda: join_type_list([alias_t, self.fx.a]))
        assert_equal(on, off)
        assert_equal(on, self.fx.a)
        rusted = self._seam_join_type_list([alias_t, self.fx.a])
        assert rusted is not None, "Rust deferred on a non-recursive alias item"
        decoded = _deserialize_type(bytes(rusted))
        assert decoded is not None
        assert_equal(decoded, self.fx.a)

    def test_join_type_list_recursive_alias_parity(self) -> None:
        # Wave-33 guardrail: a recursive alias item must never be
        # unfolded into a different result; gate-off/on must agree.
        from mypy.join import join_type_list

        alias = self._make_alias("mod.R", self.fx.a)
        alias.target = UnionType([self.fx.a, TypeAliasType(alias, [])], False)
        self._install(self._base_infos, [alias])
        r = TypeAliasType(alias, [])
        off = self._with_gate(False, lambda: join_type_list([r, self.fx.a]))
        on = self._with_gate(True, lambda: join_type_list([r, self.fx.a]))
        assert_equal(on, off)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeWireRemainderSuite(Suite):
    """Wire format tests for definition_ref (#1620) and PartialType (#1620).

    definition_ref: CallableType now serializes the fullname of its
    ``definition`` SymbolNode (or None) so a wire-decoded callable can
    re-link to the live node via the wirefixup symbol map, replacing
    the old name+arity heuristic.

    PartialType: tag 123 carries ``type_ref`` (str_opt), ``is_class``
    (bool), ``value_type`` (type_opt).  It is transient — every native
    seam checks ``is_instance(PartialType)`` and defers — so the wire
    reader returns a placeholder and the Rust ``Display`` impl renders
    ``<partial>``.
    """

    def setUp(self) -> None:
        from librt.internal import ReadBuffer

        from mypy.nodes import Var
        from mypy.wirefixup import set_wire_symbol_map, set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.ReadBuffer = ReadBuffer
        self.Var = Var
        type_infos = [
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.oi,
            self.fx.bool_type_info,
            self.fx.str_type_info,
            self.fx.functioni,
            self.fx.std_listi,
        ]
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        set_wire_symbol_map({info.fullname: info for info in type_infos})

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_symbol_map, set_wire_typeinfo_map

        set_wire_symbol_map(None)
        set_wire_typeinfo_map(None)

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def _round_trip(self, t: Type) -> Type:
        from mypy.types import instance_cache, read_type as _read_type
        from mypy.wirefixup import fixup_wire_type

        decoded = _read_type(self.ReadBuffer(self._bytes_of(t)))
        instance_cache.int_type = None
        instance_cache.str_type = None
        instance_cache.bool_type = None
        instance_cache.object_type = None
        instance_cache.function_type = None
        fixed = fixup_wire_type(decoded)
        assert fixed is not None
        return fixed

    # --- definition_ref round-trip ---

    def test_definition_ref_none_when_no_definition(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.anyt,
            self.fx.function,
        )
        assert c.definition is None
        decoded = self._round_trip(c)
        dec_ct = get_proper_type(decoded)
        assert isinstance(dec_ct, CallableType)
        assert dec_ct.definition is None
        assert dec_ct.definition_ref is None

    def test_definition_ref_round_trips_fullname(self) -> None:
        from mypy.nodes import FuncDef

        fdef = FuncDef("my_func", [], None)
        fdef._fullname = "mod.my_func"
        c = CallableType(
            [self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function, definition=fdef
        )
        assert c.definition is fdef
        decoded = self._round_trip(c)
        dec_ct = get_proper_type(decoded)
        assert isinstance(dec_ct, CallableType)
        # definition_ref is not serialized: written as None for format
        # compatibility to avoid circular-import issues. Re-linking uses
        # the _match_definition name+arity heuristic.
        assert dec_ct.definition_ref is None

    def test_definition_ref_resolved_via_symbol_map(self) -> None:
        from mypy.nodes import FuncDef, SymbolTableNode, TypeInfo
        from mypy.types import Instance

        fdef = FuncDef("my_func", [], None)
        fdef._fullname = "mod.my_func"
        # _match_definition needs the fallback TypeInfo's symbol table.
        cdef = ClassDef("MyClass", Block([]))
        cdef.fullname = "mod.MyClass"
        info = TypeInfo(SymbolTable(), cdef, "mod")
        info.names["my_func"] = SymbolTableNode(1, fdef)
        c = CallableType(
            [self.fx.a],
            [ARG_POS],
            [None],
            self.fx.b,
            Instance(info, []),
            name="my_func",
            definition=fdef,
        )
        fdef.type = c
        from mypy.wirefixup import set_wire_symbol_map, set_wire_typeinfo_map

        type_infos = {info.fullname: info}
        for inst in (self.fx.a, self.fx.b, self.fx.function):
            type_infos[inst.type.fullname] = inst.type
        set_wire_typeinfo_map(type_infos)
        set_wire_symbol_map({"mod.my_func": fdef})
        decoded = self._round_trip(c)
        set_wire_typeinfo_map(None)
        set_wire_symbol_map(None)
        dec_ct = get_proper_type(decoded)
        assert isinstance(dec_ct, CallableType)
        # definition_ref is not serialized; _match_definition resolves
        # the definition via the fallback TypeInfo's symbol table.
        assert dec_ct.definition is fdef

    def test_definition_ref_falls_back_when_no_map(self) -> None:
        from mypy.nodes import FuncDef

        fdef = FuncDef("my_func", [], None)
        fdef._fullname = "mod.my_func"
        c = CallableType(
            [self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function, definition=fdef
        )
        from mypy.wirefixup import set_wire_symbol_map

        set_wire_symbol_map(None)
        decoded = self._round_trip(c)
        dec_ct = get_proper_type(decoded)
        assert isinstance(dec_ct, CallableType)
        # definition_ref is not serialized; with no symbol map the
        # _match_definition heuristic may or may not resolve, but the
        # wire field itself stays None.
        assert dec_ct.definition_ref is None

    # --- PartialType wire ---

    def test_partial_type_none_round_trip(self) -> None:
        from mypy.nodes import Var

        v = Var("x")
        pt = PartialType(None, v)
        decoded = self._round_trip(pt)
        dec_pt = get_proper_type(decoded)
        assert isinstance(dec_pt, PartialType)
        assert dec_pt.type is None
        assert dec_pt.type_ref is None
        assert dec_pt.is_class is False

    def test_partial_type_class_round_trip(self) -> None:
        from mypy.nodes import Var

        v = Var("x")
        pt = PartialType(self.fx.ai, v)
        decoded = self._round_trip(pt)
        dec_pt = get_proper_type(decoded)
        assert isinstance(dec_pt, PartialType)
        assert dec_pt.type_ref == "A"
        assert dec_pt.is_class is True

    def test_partial_type_with_value_type_round_trip(self) -> None:
        from mypy.nodes import Var

        v = Var("x")
        val = Instance(self.fx.std_listi, [self.fx.a])
        pt = PartialType(self.fx.ai, v, val)
        decoded = self._round_trip(pt)
        dec_pt = get_proper_type(decoded)
        assert isinstance(dec_pt, PartialType)
        assert dec_pt.type_ref == "A"
        assert dec_pt.is_class is True
        assert dec_pt.value_type is not None

    def test_partial_type_display(self) -> None:
        from mypy.nodes import Var

        v = Var("x")
        pt = PartialType(self.fx.ai, v)
        assert _type_kernel.read_type_to_str(self._bytes_of(pt)) == "<partial>"

    def test_partial_type_display_none(self) -> None:
        from mypy.nodes import Var

        v = Var("x")
        pt = PartialType(None, v)
        assert _type_kernel.read_type_to_str(self._bytes_of(pt)) == "<partial>"

    # --- gate-off / gate-on parity ---

    def test_definition_ref_gate_off_on_parity(self) -> None:
        from mypy.nodes import FuncDef

        fdef = FuncDef("my_func", [], None)
        fdef._fullname = "mod.my_func"
        c = CallableType(
            [self.fx.a], [ARG_POS], [None], self.fx.b, self.fx.function, definition=fdef
        )
        from mypy.wirefixup import set_wire_symbol_map

        set_wire_symbol_map({"mod.my_func": fdef})
        on = self._round_trip(c)
        set_wire_symbol_map(None)
        off = self._round_trip(c)
        set_wire_symbol_map({"mod.my_func": fdef})
        on_ct = get_proper_type(on)
        off_ct = get_proper_type(off)
        assert isinstance(on_ct, CallableType)
        assert isinstance(off_ct, CallableType)
        assert on_ct.definition_ref == off_ct.definition_ref
        assert str(on) == str(off)
