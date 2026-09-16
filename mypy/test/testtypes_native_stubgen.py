"""Native seam suites for the stubgen area (split from testtypes.py, #1677)."""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from collections.abc import Callable
from mypy.checker import TypeChecker
from mypy.nodes import (
    ArgKind,
    BytesExpr,
    COVARIANT,
    CallExpr,
    DictExpr,
    EllipsisExpr,
    Expression,
    IndexExpr,
    IntExpr,
    ListExpr,
    MemberExpr,
    NameExpr,
    OpExpr,
    SetExpr,
    SliceExpr,
    StarExpr,
    StrExpr,
    TemplateStrExpr,
    TupleExpr,
    TypeInfo,
    UnaryExpr,
)
from mypy.state import state
from mypy.test.helpers import Suite, assert_equal
from mypy.test.typefixture import TypeFixture
from mypy.types import Instance, Type, UnionType
from typing import Any
from unittest import skipUnless

from mypy.test.testtypes import (
    T,
    _NATIVE_WIRE_ENABLED,
)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeStubgenRenderSuite(Suite):
    """Parity tests for the stubgen AliasPrinter render port (native).

    Each test builds an AST expression node, renders it via the Python
    `AliasPrinter` (`mypy.stubgen`) and via `type_kernel.rust_stubgen_render`,
    and asserts exact equality. The corpus mirrors the scratch harness in
    `misc/stubgen_kernel_check.py`. Nodes the Rust path does not handle
    return `None` and defer to Python (strangler-fig per-call gate), so only
    the handled node kinds are exercised here.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk

    # --- node constructors (mirror stubgen_kernel_check.py helpers) ---
    def _n(self, name: str) -> NameExpr:
        return NameExpr(name)

    def _m(self, expr: Expression, attr: str) -> MemberExpr:
        return MemberExpr(expr, attr)

    def _idx(self, base: Expression, index: Expression) -> IndexExpr:
        return IndexExpr(base, index)

    def _unary(self, op: str, operand: Expression) -> UnaryExpr:
        return UnaryExpr(op, operand)

    def _tup(self, *items: Expression) -> TupleExpr:
        return TupleExpr(list(items))

    def _s(self, v: str) -> StrExpr:
        return StrExpr(v)

    def _i(self, v: int) -> IntExpr:
        return IntExpr(v)

    def _b(self, v: bytes) -> BytesExpr:
        return BytesExpr(v)  # type: ignore[arg-type]

    def _lst(self, *items: Expression) -> ListExpr:
        return ListExpr(list(items))

    def _st(self, *items: Expression) -> SetExpr:
        return SetExpr(list(items))

    def _d(self, *kvs: tuple[StrExpr, Expression]) -> DictExpr:
        return DictExpr(list(kvs))

    def _call(
        self,
        callee: Expression,
        args: list[Expression],
        arg_names: list[str | None] | None = None,
        arg_kinds: list[ArgKind] | None = None,
    ) -> CallExpr:
        node = CallExpr(callee, args, arg_kinds or [], arg_names or [])
        return node

    def _op(self, op: str, left: Expression, right: Expression) -> OpExpr:
        return OpExpr(op, left, right)

    def _slc(self, begin: Expression | None, end: Expression | None) -> SliceExpr:
        return SliceExpr(begin, end, None)

    def _star(self, expr: Expression) -> StarExpr:
        return StarExpr(expr)

    def _ell(self) -> EllipsisExpr:
        return EllipsisExpr()

    def _tmpl(self, s: str) -> TemplateStrExpr:
        return TemplateStrExpr([StrExpr(s)])

    def _assert_render(self, expr: Expression) -> None:
        from mypy.stubgen import AliasPrinter, ASTStubGenerator

        gen = ASTStubGenerator()
        printer = AliasPrinter(gen)
        expected = expr.accept(printer)
        actual = self._tk.rust_stubgen_render(expr)
        assert actual is not None, f"Rust returned None for {expr!r}"
        assert_equal(
            actual, expected, f"stubgen render mismatch: Python={expected!r} Rust={actual!r}"
        )

    # --- NameExpr ---
    def test_name_int(self) -> None:
        self._assert_render(self._n("int"))

    def test_name_any(self) -> None:
        self._assert_render(self._n("Any"))

    def test_name_foo(self) -> None:
        self._assert_render(self._n("Foo"))

    # --- MemberExpr ---
    def test_member_foo_bar(self) -> None:
        self._assert_render(self._m(self._n("Foo"), "bar"))

    def test_member_nested(self) -> None:
        self._assert_render(self._m(self._m(self._n("pkg"), "sub"), "class"))

    # --- IndexExpr (generics) ---
    def test_index_list_int(self) -> None:
        self._assert_render(self._idx(self._n("list"), self._n("int")))

    def test_index_optional_str(self) -> None:
        self._assert_render(self._idx(self._n("Optional"), self._n("str")))

    def test_index_list_tuple(self) -> None:
        self._assert_render(
            self._idx(
                self._n("list"),
                self._idx(self._n("tuple"), self._tup(self._n("str"), self._n("int"))),
            )
        )

    def test_index_callable(self) -> None:
        self._assert_render(
            self._idx(
                self._n("Callable"),
                self._tup(self._lst(self._n("int"), self._n("str")), self._n("bool")),
            )
        )

    # --- UnaryExpr ---
    def test_unary_neg(self) -> None:
        self._assert_render(self._unary("-", self._i(5)))

    def test_unary_not(self) -> None:
        self._assert_render(self._unary("not", self._n("True")))

    # --- TupleExpr ---
    def test_tuple_two(self) -> None:
        self._assert_render(self._tup(self._n("int"), self._n("str")))

    def test_tuple_one(self) -> None:
        self._assert_render(self._tup(self._n("int")))

    def test_tuple_empty(self) -> None:
        self._assert_render(self._tup())

    # --- StrExpr ---
    def test_str_literal(self) -> None:
        self._assert_render(self._s("hello"))

    def test_str_forward_ref(self) -> None:
        self._assert_render(self._s("Foo"))

    def test_str_with_quote(self) -> None:
        self._assert_render(self._s("it's"))

    def test_str_backslash(self) -> None:
        self._assert_render(self._s("a\\b"))

    # --- IntExpr / BytesExpr ---
    def test_int_pos(self) -> None:
        self._assert_render(self._i(42))

    def test_int_neg(self) -> None:
        self._assert_render(self._i(-1))

    def test_bytes_literal(self) -> None:
        self._assert_render(self._b(b"test"))

    # --- ListExpr / SetExpr / DictExpr ---
    def test_list_expr(self) -> None:
        self._assert_render(self._lst(self._n("int"), self._n("str")))

    def test_set_expr(self) -> None:
        self._assert_render(self._st(self._n("int"), self._n("str")))

    def test_dict_expr(self) -> None:
        self._assert_render(self._d((self._s("key"), self._n("value"))))

    # --- OpExpr ---
    def test_op_add(self) -> None:
        self._assert_render(self._op("+", self._n("int"), self._n("int")))

    # --- SliceExpr ---
    def test_slice_basic(self) -> None:
        self._assert_render(self._slc(self._n("int"), self._n("int")))

    def test_slice_no_start(self) -> None:
        self._assert_render(self._slc(None, self._n("int")))

    # --- StarExpr / EllipsisExpr / TemplateStrExpr ---
    def test_star_expr(self) -> None:
        self._assert_render(self._star(self._n("int")))

    def test_ellipsis(self) -> None:
        self._assert_render(self._ell())

    def test_template_str(self) -> None:
        self._assert_render(self._tmpl("test"))

@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeGeneratorReturnTypeSuite(Suite):
    """Parity for the checker generator/coroutine return-type cluster (generators.rs, #434).

    The six `TypeChecker` methods (`is_generator_return_type`,
    `is_async_generator_return_type`, `get_generator_yield_type`,
    `get_generator_receive_type`, `get_generator_return_type`,
    `get_coroutine_return_type`) are Rust-ported behind the
    `_native_checker_types_active` + `_native_checker_resolver` gate. This
    suite locks in the parity: calling the real methods on a bare checker
    (built via `TypeChecker.__new__`) with the gate off (pure Python) and on
    (Rust seam) must give identical results, the direct Rust seams must
    engage (return a decision) rather than silently defer on the decided
    cases, and TypeAliasType inputs must defer to Python (the wire carries no
    alias target; Python expands via `get_proper_type`).

    #855 deferral audit: every remaining defer in `generators.rs` is
    non-wire-portable. `get_proper_or_defer` / `encode_type` defer on
    TypeAliasType (no alias target on the wire; a poisoned alias across the
    seam crashes, see checker_stmts.rs); `decode_type` defers on wire decode
    failure; `get_coroutine_return_type_inner`'s non-Instance tail defers
    because Python asserts Instance; the union arms defer through
    `make_simplified_union` (setops-owned: alias flatten, recursive
    `is_subtype` None, literal contraction); the `is_*` classification
    propagates those `is_subtype` defers (TypeAliasType operands, missing
    snapshots, protocol/variadic/named-tuple rights, ParamSpec/TVT-kinded
    nominal args, `map_instance_to_supertype` failures, FunctionLike rights,
    TypeType metaclass paths). No portable sites remain; the generator
    cluster is fully native.
    """

    # typing parametrization: Generator/Coroutine take 3 covariant params,
    # AsyncGenerator 2, Awaitable 1; AwaitableGenerator is matched by the
    # exact `type_ref` string, so it needs no resolver snapshot.
    _GEN_PARAMS = 3
    _AGEN_PARAMS = 2
    _AWAIT_PARAMS = 1
    _CORO_PARAMS = 3

    def setUp(self) -> None:
        from mypy.checker import _set_native_checker_resolver, _set_native_checker_types_active
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.geni = self.fx.make_type_info(
            "typing.Generator",
            mro=[self.fx.oi],
            typevars=["T", "T2", "T3"],
            variances=[COVARIANT] * self._GEN_PARAMS,
        )
        self.ageni = self.fx.make_type_info(
            "typing.AsyncGenerator",
            mro=[self.fx.oi],
            typevars=["T", "T2"],
            variances=[COVARIANT] * self._AGEN_PARAMS,
        )
        self.awaiti = self.fx.make_type_info(
            "typing.Awaitable",
            mro=[self.fx.oi],
            typevars=["T"],
            variances=[COVARIANT] * self._AWAIT_PARAMS,
        )
        self.coroi = self.fx.make_type_info(
            "typing.Coroutine",
            mro=[self.fx.oi],
            typevars=["T", "T2", "T3"],
            variances=[COVARIANT] * self._CORO_PARAMS,
        )
        # AwaitableGenerator: no type vars; matched by fullname string alone.
        self.awgi = self.fx.make_type_info("typing.AwaitableGenerator", mro=[self.fx.oi])

        types_to_resolve: list[TypeInfo] = [
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.oi,
            self.fx.str_type_info,
            self.geni,
            self.ageni,
            self.awaiti,
            self.coroi,
            self.awgi,
        ]
        self.typeinfo_map = {info.fullname: info for info in types_to_resolve}
        set_wire_typeinfo_map(self.typeinfo_map)
        self.resolver = _type_kernel.build_native_resolver(types_to_resolve, [])
        self._set_active = _set_native_checker_types_active
        self._set_resolver = _set_native_checker_resolver
        self._set_active(True)
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

    def _chk(self) -> TypeChecker:
        from mypy.checker import TypeChecker

        chk = TypeChecker.__new__(TypeChecker)
        infos = {
            "typing.Awaitable": self.awaiti,
            "typing.Generator": self.geni,
            "typing.AsyncGenerator": self.ageni,
        }

        def lookup_typeinfo(fullname: str) -> TypeInfo:
            if fullname not in infos:
                raise KeyError(fullname)
            return infos[fullname]

        # Feedback loop: the gate-off run uses the same fixtures, so the
        # Python classifier, the shims, and the wire resolver all agree.
        chk.lookup_typeinfo = lookup_typeinfo  # type: ignore[method-assign]
        return chk

    def _gen(self, args: list[Type]) -> Instance:
        return Instance(self.geni, args)

    def _assert_par(self, typ: Type, is_coroutine: bool, label: str) -> None:
        chk = self._chk()
        # is_generator_return_type
        off = self._with_gate(False, lambda: chk.is_generator_return_type(typ, is_coroutine))
        on = self._with_gate(True, lambda: chk.is_generator_return_type(typ, is_coroutine))
        assert_equal(on, off, f"is_generator_return_type parity {label}")
        # async generator classification
        off = self._with_gate(False, lambda: chk.is_async_generator_return_type(typ))
        on = self._with_gate(True, lambda: chk.is_async_generator_return_type(typ))
        assert_equal(on, off, f"is_async_generator_return_type parity {label}")
        # the three type extractors
        for meth in (
            "get_generator_yield_type",
            "get_generator_receive_type",
            "get_generator_return_type",
        ):
            off = self._with_gate(False, lambda: getattr(chk, meth)(typ, is_coroutine))
            on = self._with_gate(True, lambda: getattr(chk, meth)(typ, is_coroutine))
            assert_equal(str(on), str(off), f"{meth} parity {label}")

    def _assert_par_coroutine(self, typ: Type, label: str) -> None:
        chk = self._chk()
        off = self._with_gate(False, lambda: chk.get_coroutine_return_type(typ))
        on = self._with_gate(True, lambda: chk.get_coroutine_return_type(typ))
        assert_equal(str(on), str(off), f"get_coroutine_return_type parity {label}")

    def _assert_engages_bool(
        self, seam: str, typ: Type, is_coroutine: bool, expected: bool
    ) -> None:
        from mypy.checker import _serialize_type_for_checker

        # rust_is_async_generator_return_type takes just (typ_bytes,
        # strict_optional, resolver); the other classifiers take the
        # is_coroutine flag too.
        if seam == "rust_is_async_generator_return_type":
            args: tuple[object, ...] = (_serialize_type_for_checker(typ),)
        else:
            args = (_serialize_type_for_checker(typ), is_coroutine)
        result = getattr(_type_kernel, seam)(*args, state.strict_optional, self.resolver)
        assert (
            result == expected
        ), f"{seam}({typ}, coroutine={is_coroutine}) = {result!r}, expected {expected!r}"

    def _assert_engages_type(
        self, seam: str, typ: Type, is_coroutine: bool, expected: str
    ) -> None:
        from mypy.checker import _deserialize_type_from_checker, _serialize_type_for_checker

        if seam == "rust_get_coroutine_return_type":
            # Pure Type-in/Type-out: no classification, no resolver.
            blob = _type_kernel.rust_get_coroutine_return_type(_serialize_type_for_checker(typ))
        else:
            blob = getattr(_type_kernel, seam)(
                _serialize_type_for_checker(typ),
                is_coroutine,
                state.strict_optional,
                self.resolver,
            )
        assert blob is not None, f"{seam}({typ}) deferred"
        result = str(_deserialize_type_from_checker(bytes(blob)))
        assert_equal(result, expected, f"{seam}({typ})")

    def test_generator_result_type_par(self) -> None:
        # Generator[int, str, bool]: ty=int, tc=str, tr=bool. All three
        # extractors engage (same type_ref nominal decision).
        typ = self._gen([self.fx.a, self.fx.b, self.fx.str_type])
        self._assert_par(typ, False, "generator-result")
        self._assert_engages_type("rust_get_generator_yield_type", typ, False, str(self.fx.a))
        self._assert_engages_type("rust_get_generator_receive_type", typ, False, str(self.fx.b))
        self._assert_engages_type(
            "rust_get_generator_return_type", typ, False, str(self.fx.str_type)
        )

    def test_generator_any_args_par(self) -> None:
        # Generator[Any, Any, Any]: subclass matching is decidable and the
        # extractors return the Any args via the args-not-empty branches.
        typ = self._gen([self.fx.anyt, self.fx.anyt, self.fx.anyt])
        self._assert_par(typ, False, "generator-any")
        self._assert_engages_type("rust_get_generator_yield_type", typ, False, "Any")
        self._assert_engages_type("rust_get_generator_receive_type", typ, False, "Any")
        self._assert_engages_type("rust_get_generator_return_type", typ, False, "Any")

    def test_awaitable_coroutine_par(self) -> None:
        # Awaitable[int] (coroutine): ty=Any, tc=Any, tr=int.
        typ = Instance(self.awaiti, [self.fx.a])
        self._assert_par(typ, True, "awaitable-coroutine")
        self._assert_engages_type("rust_get_generator_yield_type", typ, True, "Any")
        self._assert_engages_type("rust_get_generator_receive_type", typ, True, "Any")
        self._assert_engages_type("rust_get_generator_return_type", typ, True, str(self.fx.a))

    def test_awaitable_generator_matches_par(self) -> None:
        # AwaitableGenerator is the exact-fullname branch; both coroutine and
        # non-coroutine classify it as a generator return.
        typ = Instance(self.awgi, [self.fx.a])
        self._assert_par(typ, False, "awaitable-generator")
        self._assert_par(typ, True, "awaitable-generator-coroutine")

    def test_async_generator_par(self) -> None:
        # yield=int, receive=str; the one-sided `tr` tail (no async
        # alternative, checker.py:1541) yields Any(from_error) for a bare
        # AsyncGenerator while receive reads args[1].
        typ = Instance(self.ageni, [self.fx.a, self.fx.str_type])
        self._assert_par(typ, False, "async-generator")
        self._assert_engages_type("rust_get_generator_yield_type", typ, False, str(self.fx.a))
        self._assert_engages_type(
            "rust_get_generator_receive_type", typ, False, str(self.fx.str_type)
        )
        self._assert_engages_type("rust_get_generator_return_type", typ, False, "Any")

    def test_non_generator_any_from_error(self) -> None:
        # A plain class is not a generator return; the extractors fall to
        # Any(from_error). The classification probes need the right-side
        # snapshots (builtins.A, typing.Generator, ...) to decide.
        typ = Instance(self.fx.ai, [])
        self._assert_par(typ, False, "plain-A")
        self._assert_engages_type("rust_get_generator_yield_type", typ, False, "Any")
        self._assert_engages_type("rust_get_generator_receive_type", typ, False, "Any")
        self._assert_engages_type("rust_get_generator_return_type", typ, False, "Any")

    def test_union_yield_recurses(self) -> None:
        # Union[Generator[int, Any, Any], str]: the Generator branch yields
        # int, the non-generator branch Any(from_error), simplified back to
        # Union[int, Any] (portable recursive make_simplified_union).
        typ = UnionType([self._gen([self.fx.a, self.fx.anyt, self.fx.anyt]), self.fx.str_type])
        self._assert_par(typ, False, "union-yield")
        from mypy.checker import _deserialize_type_from_checker, _serialize_type_for_checker

        blob = _type_kernel.rust_get_generator_yield_type(
            _serialize_type_for_checker(typ), False, state.strict_optional, self.resolver
        )
        assert blob is not None, "rust_get_generator_yield_type(union) deferred"
        result = str(_deserialize_type_from_checker(bytes(blob)))
        assert_equal(result, "A | Any", "union yield")

    def test_alias_defers_to_python(self) -> None:
        # TypeAliasType: the wire has no alias target, so every generators.rs
        # seam defers and Python expands via `get_proper_type`. Gate-off and
        # gate-on must still agree.
        alias_typ = self.fx.non_rec_alias(self._gen([self.fx.a, self.fx.b, self.fx.str_type]))
        self._assert_par(alias_typ, False, "alias")
        from mypy.checker import _serialize_type_for_checker

        for seam in (
            "rust_is_generator_return_type",
            "rust_is_async_generator_return_type",
            "rust_get_generator_yield_type",
            "rust_get_generator_receive_type",
            "rust_get_generator_return_type",
        ):
            # rust_is_async_generator_return_type has no is_coroutine param.
            if seam == "rust_is_async_generator_return_type":
                args: tuple[object, ...] = (_serialize_type_for_checker(alias_typ),)
            else:
                args = (_serialize_type_for_checker(alias_typ), False)
            result = getattr(_type_kernel, seam)(*args, state.strict_optional, self.resolver)
            assert result is None, f"{seam}(alias) should defer, got {result!r}"
        # get_coroutine_return_type: pure Type-in/Type-out, deferred on the
        # alias target just like the five classifier/extractor seams.
        assert (
            _type_kernel.rust_get_coroutine_return_type(_serialize_type_for_checker(alias_typ))
            is None
        ), "rust_get_coroutine_return_type(alias) should defer"

    def test_coroutine_return_type_par(self) -> None:
        # Coroutine[Any, Any, int]: tr = args[2]. get_coroutine_return_type
        # is pure Type-in/Type-out (no classification, no resolver).
        typ = Instance(self.coroi, [self.fx.anyt, self.fx.anyt, self.fx.a])
        self._assert_par_coroutine(typ, "coroutine-return")
        self._assert_engages_type("rust_get_coroutine_return_type", typ, True, str(self.fx.a))

@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeStubgenPrinterSuite(Suite):
    """Parity for the stubgen printer/collector seams (#1636).

    Covers `rust_stubgen_get_qualified_name`,
    `rust_stubgen_str_type_tag` and `rust_stubgen_str_default`
    with direct seam calls plus gate-off/on differentials
    through the real `mypy.stubgen` entry points.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        import mypy.stubgen as _sg

        self._tk = _tk
        self._sg = _sg

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        old = self._sg._HAS_NATIVE_STUBGEN
        self._sg._HAS_NATIVE_STUBGEN = active
        try:
            return fn()
        finally:
            self._sg._HAS_NATIVE_STUBGEN = old

    def _gen(self) -> Any:
        from mypy.stubgen import ASTStubGenerator

        return ASTStubGenerator()

    # --- get_qualified_name ---
    def test_qualified_name_direct(self) -> None:
        assert_equal(self._tk.rust_stubgen_get_qualified_name(NameExpr("x")), "x")
        nested = MemberExpr(MemberExpr(NameExpr("a"), "b"), "c")
        assert_equal(self._tk.rust_stubgen_get_qualified_name(nested), "a.b.c")

    def test_qualified_name_error_marker(self) -> None:
        assert_equal(self._tk.rust_stubgen_get_qualified_name(IntExpr(1)), "<ERROR>")
        callee = MemberExpr(CallExpr(NameExpr("f"), [], [], []), "attr")
        assert_equal(self._tk.rust_stubgen_get_qualified_name(callee), "<ERROR>.attr")

    def test_qualified_name_parity(self) -> None:
        nodes: list[Expression] = [
            NameExpr("x"),
            MemberExpr(MemberExpr(NameExpr("a"), "b"), "c"),
            IntExpr(1),
        ]
        for node in nodes:
            off = self._with_gate(False, lambda: self._sg.get_qualified_name(node))
            on = self._with_gate(True, lambda: self._sg.get_qualified_name(node))
            assert_equal(on, off, "qualified_name parity")

    # --- str_type_tag ---
    def test_str_type_tags_direct(self) -> None:
        from mypy.nodes import ComplexExpr, FloatExpr

        tk = self._tk
        assert_equal(tk.rust_stubgen_str_type_tag(IntExpr(1), True), 0)
        assert_equal(tk.rust_stubgen_str_type_tag(StrExpr("x"), True), 1)
        assert_equal(tk.rust_stubgen_str_type_tag(BytesExpr(b"x"), True), 2)  # type: ignore[arg-type]
        assert_equal(tk.rust_stubgen_str_type_tag(FloatExpr(1.5), True), 3)
        assert_equal(tk.rust_stubgen_str_type_tag(ComplexExpr(complex(0, 1)), True), 4)
        assert_equal(tk.rust_stubgen_str_type_tag(NameExpr("True"), True), 5)
        assert_equal(tk.rust_stubgen_str_type_tag(NameExpr("foo"), True), 6)
        assert_equal(tk.rust_stubgen_str_type_tag(NameExpr("foo"), False), 7)

    def test_str_type_unwrap_direct(self) -> None:
        tk = self._tk
        assert_equal(tk.rust_stubgen_str_type_tag(UnaryExpr("-", IntExpr(5)), True), 0)
        nested = UnaryExpr("-", UnaryExpr("-", IntExpr(5)))
        assert_equal(tk.rust_stubgen_str_type_tag(nested, True), 0)
        assert_equal(tk.rust_stubgen_str_type_tag(UnaryExpr("~", IntExpr(5)), True), 6)
        assert_equal(
            tk.rust_stubgen_str_type_tag(UnaryExpr("not", NameExpr("True")), True), 5
        )
        assert_equal(
            tk.rust_stubgen_str_type_tag(UnaryExpr("not", NameExpr("x")), True), 6
        )
        # Two-phase unwrap: `not` never falls into the math phase.
        not_plus = UnaryExpr("not", UnaryExpr("+", IntExpr(1)))
        assert_equal(tk.rust_stubgen_str_type_tag(not_plus, True), 6)
        neg_not = UnaryExpr("-", UnaryExpr("not", NameExpr("False")))
        assert_equal(tk.rust_stubgen_str_type_tag(neg_not, True), 6)

    def test_str_type_op_complex_direct(self) -> None:
        from mypy.nodes import ComplexExpr

        tk = self._tk
        op = OpExpr("+", IntExpr(1), ComplexExpr(complex(0, 1)))
        assert_equal(tk.rust_stubgen_str_type_tag(op, True), 4)
        plain = OpExpr("-", IntExpr(1), IntExpr(2))
        assert_equal(tk.rust_stubgen_str_type_tag(plain, True), 6)
        assert_equal(tk.rust_stubgen_str_type_tag(plain, False), 7)

    def test_str_type_parity(self) -> None:
        from mypy.nodes import ComplexExpr, FloatExpr

        nodes: list[Expression] = [
            IntExpr(1),
            StrExpr("x"),
            BytesExpr(b"x"),  # type: ignore[arg-type]
            FloatExpr(1.5),
            ComplexExpr(complex(0, 1)),
            UnaryExpr("-", IntExpr(5)),
            UnaryExpr("~", IntExpr(5)),
            UnaryExpr("not", NameExpr("True")),
            OpExpr("+", IntExpr(1), ComplexExpr(complex(0, 1))),
            OpExpr("-", IntExpr(1), IntExpr(2)),
            NameExpr("True"),
            NameExpr("foo"),
        ]
        for node in nodes:
            for inc in (True, False):
                off = self._with_gate(
                    False, lambda: self._gen().get_str_type_of_node(node, can_be_incomplete=inc)
                )
                on = self._with_gate(
                    True, lambda: self._gen().get_str_type_of_node(node, can_be_incomplete=inc)
                )
                assert_equal(on, off, "str_type parity")

    def test_str_type_incomplete_alias_parity(self) -> None:
        from mypy.stubgen import ASTStubGenerator

        for active in (False, True):
            gen = ASTStubGenerator()
            gen.set_defined_names({"Incomplete"})
            old = self._sg._HAS_NATIVE_STUBGEN
            self._sg._HAS_NATIVE_STUBGEN = active
            try:
                got = gen.get_str_type_of_node(NameExpr("foo"))
            finally:
                self._sg._HAS_NATIVE_STUBGEN = old
            if active:
                assert_equal(got, "_Incomplete")
            else:
                assert_equal(got, "_Incomplete")

    # --- str_default ---
    def test_str_default_direct(self) -> None:
        tk = self._tk
        assert_equal(tk.rust_stubgen_str_default(NameExpr("None")), ("None", True))
        assert_equal(tk.rust_stubgen_str_default(IntExpr(5)), ("5", True))
        assert_equal(tk.rust_stubgen_str_default(UnaryExpr("-", IntExpr(5))), ("-5", True))
        assert_equal(tk.rust_stubgen_str_default(StrExpr("it's")), ('"it\'s"', True))
        assert_equal(tk.rust_stubgen_str_default(TupleExpr([IntExpr(1)])), ("(1,)", True))
        assert_equal(tk.rust_stubgen_str_default(ListExpr([IntExpr(1)])), ("[1]", True))
        assert_equal(tk.rust_stubgen_str_default(SetExpr([])), ("...", False))
        assert_equal(tk.rust_stubgen_str_default(NameExpr("x")), ("...", False))

    def test_str_default_bytes_backslash(self) -> None:
        tk = self._tk
        node = BytesExpr(b"a\\b")  # type: ignore[arg-type]
        off = self._with_gate(False, lambda: self._gen().get_str_default_of_node(node))
        on_raw = tk.rust_stubgen_str_default(BytesExpr(b"a\\b"))  # type: ignore[arg-type]
        assert on_raw is not None
        assert_equal(tuple(on_raw), off, "bytes backslash parity")

    def test_str_default_parity(self) -> None:
        from mypy.nodes import FloatExpr

        nodes: list[Expression] = [
            NameExpr("None"),
            NameExpr("x"),
            IntExpr(5),
            FloatExpr(1.5),
            UnaryExpr("-", IntExpr(5)),
            UnaryExpr("-", UnaryExpr("-", IntExpr(5))),
            StrExpr("a\nb"),
            BytesExpr(b"\x00\xff"),  # type: ignore[arg-type]
            TupleExpr([]),
            TupleExpr([IntExpr(1)]),
            TupleExpr([IntExpr(1), StrExpr("x")]),
            TupleExpr([NameExpr("x")]),
            ListExpr([]),
            ListExpr([IntExpr(1)]),
            SetExpr([IntExpr(1)]),
            SetExpr([]),
            DictExpr([]),
            DictExpr([(StrExpr("k"), IntExpr(1))]),
            OpExpr("+", IntExpr(1), IntExpr(2)),
        ]
        for node in nodes:
            off = self._with_gate(False, lambda: self._gen().get_str_default_of_node(node))
            on = self._with_gate(True, lambda: self._gen().get_str_default_of_node(node))
            assert_equal(on, off, "str_default parity")

    def test_str_default_dict_none_key_parity(self) -> None:
        node = DictExpr([(None, IntExpr(1))])
        off = self._with_gate(False, lambda: self._gen().get_str_default_of_node(node))
        on = self._with_gate(True, lambda: self._gen().get_str_default_of_node(node))
        assert_equal(on, off, "dict none-key parity")
        assert_equal(off, ("...", False))
