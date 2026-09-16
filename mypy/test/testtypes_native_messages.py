"""Native seam suites for the messages area (split from testtypes.py, #1677)."""

from __future__ import annotations

try:
    from librt.internal import WriteBuffer as _WriteBuffer
    import type_kernel as _type_kernel
except ImportError:
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _type_kernel = None  # type: ignore[assignment]

from collections.abc import Callable
from mypy.nodes import (
    ARG_NAMED,
    ARG_POS,
    Argument,
    Block,
    ClassDef,
    Context,
    FuncDef,
    IntExpr,
    MDEF,
    NameExpr,
    ReturnStmt,
    SymbolTable,
    SymbolTableNode,
    TypeAlias,
    TypeInfo,
    Var,
)
from mypy.options import Options
from mypy.test.helpers import Suite, assert_equal
from mypy.test.typefixture import TypeFixture
from mypy.types import (
    AnyType,
    CallableType,
    DeletedType,
    Instance,
    NoneType,
    TupleType,
    Type,
    TypeAliasType,
    TypeOfAny,
    TypeVarId,
    TypeVarType,
    TypedDictType,
    UninhabitedType,
    UnionType,
    UnpackType,
)
from typing import Any
from unittest import skipUnless

from mypy.test.testtypes import (
    T,
    _NATIVE_WIRE_ENABLED,
    _is_type_info,
)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSuggestionsSuite(Suite):
    """Parity suite for M27: did-you-mean suggestion ranking and formatting.

    Verifies `best_matches` and `pretty_seq` produce identical results
    between the pure-Python difflib path and the Rust Ratcliff-Obershelp
    path when the `native_type_kernel` gate is on.
    """

    def setUp(self) -> None:
        from mypy.messages import _set_native_suggestions_active

        self._setter = _set_native_suggestions_active

    def _check_best_matches(self, current: str, options: list[str], n: int) -> None:
        from mypy.messages import best_matches

        self._setter(False)
        py_result = best_matches(current, options, n)
        self._setter(True)
        rs_result = best_matches(current, options, n)
        assert_equal(rs_result, py_result, f"best_matches({current!r}, {options!r}, {n})")

    def _check_pretty_seq(self, args: list[str], conjunction: str) -> None:
        from mypy.messages import pretty_seq

        self._setter(False)
        py_result = pretty_seq(args, conjunction)
        self._setter(True)
        rs_result = pretty_seq(args, conjunction)
        assert_equal(rs_result, py_result, f"pretty_seq({args!r}, {conjunction!r})")

    def test_best_matches_identical(self) -> None:
        self._check_best_matches("foo", ["foo", "bar", "baz"], 3)

    def test_best_matches_fuzzy(self) -> None:
        self._check_best_matches("helo", ["hello", "help", "hero", "hola", "foo"], 3)

    def test_best_matches_empty_current(self) -> None:
        self._check_best_matches("", ["a", "b"], 3)

    def test_best_matches_no_matches(self) -> None:
        self._check_best_matches("xyz", ["foo", "bar", "baz"], 3)

    def test_best_matches_many_options(self) -> None:
        options = [f"test{i}" for i in range(60)]
        options[30] = "test"
        self._check_best_matches("test", options, 3)

    def test_best_matches_tie_breaking(self) -> None:
        self._check_best_matches("abc", ["abd", "abe", "abf", "abg"], 2)

    def test_best_matches_partial_match(self) -> None:
        self._check_best_matches("append", ["apend", "append", "prepend", "foo"], 3)

    def test_best_matches_case_sensitive(self) -> None:
        self._check_best_matches("Sequence", ["sequence", "Sequence", "SEQUENCE"], 3)

    def test_best_matches_n_larger_than_results(self) -> None:
        self._check_best_matches("foo", ["foo", "foobar"], 10)

    def test_pretty_seq_single(self) -> None:
        self._check_pretty_seq(["a"], "or")

    def test_pretty_seq_pair(self) -> None:
        self._check_pretty_seq(["a", "b"], "or")

    def test_pretty_seq_triple(self) -> None:
        self._check_pretty_seq(["a", "b", "c"], "or")

    def test_pretty_seq_quad(self) -> None:
        self._check_pretty_seq(["a", "b", "c", "d"], "and")

    def test_pretty_seq_empty(self) -> None:
        self._setter(False)
        from mypy.messages import pretty_seq

        # Python crashes on empty, so only test Rust returns ""
        self._setter(True)
        assert_equal(pretty_seq([], "or"), "")

    def test_pretty_seq_and_conjunction(self) -> None:
        self._check_pretty_seq(["x", "y", "z"], "and")

@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMessagesSuite(Suite):
    """Parity tests for the Rust message formatting (M21).

    Each test builds a NativeTypeResolver from the live Python TypeInfo
    graph, serializes a `Type` via `Type.write(WriteBuffer)`, and asserts
    that `rust_format_type_bare(bytes, resolver, ...)` matches the
    pure-Python `format_type_bare(typ, options, ...)`. The corpus targets
    the hot type-formatter output that appears in error messages.
    """

    def setUp(self) -> None:
        from mypy.options import Options
        from mypy.test.typefixture import TypeFixture as _TypeFixture

        self.fx = _TypeFixture()
        self.options = Options()
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

    def assert_format_par(self, t: Type, verbosity: int = 0) -> None:
        from mypy.messages import format_type_bare

        expected = format_type_bare(t, self.options, verbosity=verbosity)
        actual = _type_kernel.rust_format_type_bare(
            self._bytes_of(t), self.resolver, verbosity, False, True
        )
        self.assertIsNotNone(actual, f"rust format_type_bare({t!r}) returned None")
        assert_equal(actual, expected, f"format_type_bare({t!r}) = {{}} ({{}} expected)")

    def assert_format_quoted_par(self, t: Type) -> None:
        from mypy.messages import format_type

        expected = format_type(t, self.options)
        actual = _type_kernel.rust_format_type(self._bytes_of(t), self.resolver, 0, False, True)
        self.assertIsNotNone(actual, f"rust format_type({t!r}) returned None")
        assert_equal(actual, expected, f"format_type({t!r}) = {{}} ({{}} expected)")

    def test_pure_helpers(self) -> None:
        # Pure string helpers, no resolver needed.
        assert_equal(_type_kernel.rust_quote_type_string("int"), '"int"')
        assert_equal(_type_kernel.rust_quote_type_string("Module"), "Module")
        assert_equal(_type_kernel.rust_capitalize("hello"), "Hello")
        assert_equal(_type_kernel.rust_capitalize(""), "")
        assert_equal(_type_kernel.rust_pretty_seq(["a", "b"], "or"), '"a" or "b"')
        assert_equal(_type_kernel.rust_format_string_list(["a", "b", "c"]), "a, b and c")
        assert_equal(_type_kernel.rust_format_item_name_list(["a", "b"]), '("a", "b")')
        assert_equal(
            _type_kernel.rust_wrong_type_arg_count(1, 1, "0", "List"),
            '"List" expects 1 type argument, but none given',
        )
        assert_equal(_type_kernel.rust_strip_quotes('"x"'), "x")
        assert_equal(_type_kernel.rust_extract_type('"__getitem__" of list'), "list")
        assert_equal(_type_kernel.rust_variance_string(1), "covariant")
        assert_equal(_type_kernel.rust_variance_string(2), "contravariant")
        assert_equal(_type_kernel.rust_variance_string(0), "invariant")

    def test_any(self) -> None:

        t = AnyType(TypeOfAny.special_form)
        self.assert_format_par(t)
        self.assert_format_quoted_par(t)

    def test_none(self) -> None:
        self.assert_format_par(NoneType())
        self.assert_format_quoted_par(NoneType())

    def test_uninhabited(self) -> None:
        self.assert_format_par(UninhabitedType())

    def test_instance_singletons(self) -> None:
        self.assert_format_par(self.fx.str_type)
        self.assert_format_par(self.fx.function)
        self.assert_format_par(self.fx.bool_type)
        self.assert_format_par(self.fx.o)
        self.assert_format_par(self.fx.a)
        self.assert_format_par(self.fx.b)
        self.assert_format_par(self.fx.o)

    def test_instance_generic(self) -> None:
        self.assert_format_par(self.fx.ga)
        self.assert_format_par(self.fx.gb)
        self.assert_format_par(self.fx.gt)
        self.assert_format_par(self.fx.lsta)
        self.assert_format_par(self.fx.lstb)

    def test_instance_tuple(self) -> None:
        self.assert_format_par(self.fx.std_tuple)

    def test_literal_int(self) -> None:
        self.assert_format_par(self.fx.lit1)
        self.assert_format_par(self.fx.lit2)
        self.assert_format_par(self.fx.lit4)

    def test_literal_str(self) -> None:
        self.assert_format_par(self.fx.lit_str1)
        self.assert_format_par(self.fx.lit_str2)
        self.assert_format_par(self.fx.lit_str3)

    def test_literal_bool(self) -> None:
        self.assert_format_par(self.fx.lit_false)
        self.assert_format_par(self.fx.lit_true)

    def test_type_type(self) -> None:
        self.assert_format_par(self.fx.type_a)
        self.assert_format_par(self.fx.type_b)
        self.assert_format_par(self.fx.type_any)

    def test_union_none(self) -> None:
        # Optional[T]: should format as `T | None`.
        u = UnionType.make_union([self.fx.a, NoneType()])
        self.assert_format_par(u)
        self.assert_format_quoted_par(u)

    def test_union_multi(self) -> None:
        u = UnionType.make_union([self.fx.a, self.fx.b, self.fx.o])
        self.assert_format_par(u)

    def test_union_literals(self) -> None:
        # Coalesced Literal[1, 2] | None.
        u = UnionType.make_union([self.fx.lit1, self.fx.lit2, NoneType()])
        self.assert_format_par(u)

    def test_callable_simple(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b], [ARG_POS, ARG_POS], [None, None], self.fx.o, self.fx.function
        )
        self.assert_format_par(c)

    def test_callable_named(self) -> None:
        # Named-arg callables use pretty_callable, which needs FuncDef
        # data not present in the wire format, so the Rust path defers
        # (returns None) and Python formats instead.
        c = CallableType([self.fx.a], [ARG_NAMED], ["x"], self.fx.o, self.fx.function)
        actual = _type_kernel.rust_format_type_bare(
            self._bytes_of(c), self.resolver, 0, False, True
        )
        self.assertIsNone(actual, "named-arg callable should defer to Python")

    def test_ellipsis_callable(self) -> None:
        c = CallableType([], [], [], self.fx.o, self.fx.function, is_ellipsis_args=True)
        self.assert_format_par(c)

    def test_tuple_type(self) -> None:
        t = TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)
        self.assert_format_par(t)

    def test_type_var(self) -> None:
        self.assert_format_par(self.fx.t)
        self.assert_format_par(self.fx.tf)

    def test_unpack(self) -> None:
        u = UnpackType(self.fx.a)
        self.assert_format_par(u)

    def test_typeddict_anonymous(self) -> None:
        from mypy.types import TypedDictType

        # Create a TypeInfo for typing._TypedDict so the anonymity check
        # treats the fallback as anonymous.
        fb_ti = self.fx.make_type_info("typing._TypedDict")
        fb = Instance(fb_ti, [])
        t = TypedDictType({"x": self.fx.a, "y": self.fx.b}, {"x"}, set(), fb)
        self.assert_format_par(t)

    def test_deleted_type(self) -> None:
        from mypy.types import DeletedType

        t = DeletedType("var")
        self.assert_format_par(t)

    def test_append_invariance_notes_par(self) -> None:
        from mypy.messages import append_invariance_notes

        for arg_t, exp_t in [
            (self.fx.lsta, self.fx.lsta),  # List[A] vs List[A]: list note fires
            (self.fx.lsta, self.fx.lstb),  # A not subtype of B: no note appended
        ]:
            expected = append_invariance_notes([], arg_t, exp_t)
            actual = _type_kernel.rust_append_invariance_notes(
                self._bytes_of(arg_t), self._bytes_of(exp_t), self.resolver
            )
            self.assertIsNotNone(actual, f"rust invariance notes deferred for {arg_t} vs {exp_t}")
            assert_equal(actual, expected, f"invariance notes mismatch {arg_t} vs {exp_t}")

        # Non-list/dict pairs with empty args (a class with no type vars):
        # decided natively as no-op instead of deferring on the args scan.
        empty = Instance(self.fx.make_type_info("mod.X"), [])
        expected = append_invariance_notes([], empty, empty)
        actual = _type_kernel.rust_append_invariance_notes(
            self._bytes_of(empty), self._bytes_of(empty), self.resolver
        )
        self.assertIsNotNone(actual, "rust deferred on an empty-args non-list/dict pair")
        assert_equal(actual, expected, "empty-args non-list/dict mismatch")

    def test_append_numbers_notes_par(self) -> None:
        from mypy.messages import append_numbers_notes

        num_t = Instance(self.fx.make_type_info("numbers.Complex"), [])
        for exp_t in [num_t, self.fx.str_type]:
            expected = append_numbers_notes([], self.fx.a, exp_t)
            actual = _type_kernel.rust_append_numbers_notes(self._bytes_of(exp_t))
            self.assertIsNotNone(actual, f"rust numbers notes deferred for {exp_t}")
            assert_equal(actual, expected, f"numbers notes mismatch for {exp_t}")

    def test_append_union_note_par(self) -> None:
        from mypy.messages import append_union_note

        # 11 items clears MAX_UNION_ITEMS so the non-matching tail can fire.
        items = [
            self.fx.a,
            self.fx.b,
            self.fx.c,
            self.fx.d,
            self.fx.e,
            self.fx.f,
            self.fx.e2,
            self.fx.e3,
            self.fx.o,
            self.fx.str_type,
            self.fx.bool_type,
        ]
        arg_u = UnionType.make_union(items)
        exp_u = UnionType.make_union([self.fx.a, self.fx.b])
        expected = append_union_note(
            [],
            arg_u,  # type: ignore[arg-type]
            exp_u,  # type: ignore[arg-type]
            self.options,
        )
        actual = _type_kernel.rust_append_union_note(
            self._bytes_of(arg_u),
            self._bytes_of(exp_u),
            self.resolver,
            self.options.use_star_unpack(),
        )
        if actual is not None:
            assert_equal(actual, expected, "union note mismatch")

    def test_pretty_callable_par(self) -> None:
        from mypy.messages import pretty_callable

        # Definition-free callable, the exact scope of the Rust gate.
        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.o,
            self.fx.function,
            name="f",
        )
        expected = pretty_callable(c, self.options)
        actual = _type_kernel.rust_pretty_callable(
            self._bytes_of(c), self.resolver, False, self.options.use_star_unpack()
        )
        self.assertIsNotNone(actual, "rust pretty_callable deferred on a plain callable")
        assert_equal(actual, expected, "pretty_callable mismatch")

@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMessagesDeferralSuite(Suite):
    """Gate-on/off differential for the messages.rs deferral ports.

    `rust_format_type_distinctly` mirrors `format_type_distinctly`, including
    the `min_verbosity` decision for a Callable/Callable pair where `right` has
    named args: Python bumps verbosity to 1 iff
    `is_subtype(left, right, ignore_pos_arg_names=True)`, and the Rust seam now
    computes that natively (previously it deferred the whole branch whenever
    `right` had any named args).

    Each test compares the gate-off (pure-Python) and gate-on (Rust seam) results
    of `format_type_distinctly` for representative pairs and asserts parity. Named-
    arg callables now render through the Rust `pretty_callable` port when the
    shim proves the wire-dropped `definition` cannot change the output
    (`_pretty_wire_safe`); definition-dependent callables still defer to the
    pure-Python formatter, and the differential guards both paths.
    """

    def setUp(self) -> None:
        from mypy.messages import _set_native_messages_active, _set_native_messages_resolver
        from mypy.options import Options
        from mypy.test.typefixture import TypeFixture as _TypeFixture

        self.fx = _TypeFixture()
        self.options = Options()
        self._set_active = _set_native_messages_active
        self._set_resolver = _set_native_messages_resolver
        self._buf = _WriteBuffer()
        type_infos = [self.fx.oi, self.fx.ai, self.fx.bi, self.fx.ci, self.fx.functioni]
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(self.resolver)
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
        self._buf = _WriteBuffer()
        t.write(self._buf)
        return self._buf.getvalue()

    def _assert_distinctly_par(self, *types: Type) -> None:
        from mypy.messages import format_type_distinctly

        off = self._with_gate(
            False, lambda: format_type_distinctly(*types, options=self.options, bare=True)
        )
        on = self._with_gate(
            True, lambda: format_type_distinctly(*types, options=self.options, bare=True)
        )
        assert_equal(on, off, f"format_type_distinctly parity {types}")

    def test_positional_callables(self) -> None:
        # Plain positional callables: the common formattable path (no
        # pretty_callable detour), both gates must agree.
        left = CallableType(
            [self.fx.a, self.fx.b], [ARG_POS, ARG_POS], [None, None], self.fx.o, self.fx.function
        )
        right = CallableType([self.fx.c], [ARG_POS], [None], self.fx.o, self.fx.function)
        self._assert_distinctly_par(left, right)

    def test_callable_pair_named_arg_non_subtype(self) -> None:
        # left arg A, right arg B (unrelated): is_subtype is False, so the
        # min_verbosity decision resolves to 0 natively. Previously the seam
        # deferred this whole branch; gate-on must still match gate-off.
        left = CallableType([self.fx.a], [ARG_POS], [None], self.fx.o, self.fx.function)
        right = CallableType([self.fx.b], [ARG_NAMED], ["x"], self.fx.o, self.fx.function)
        self._assert_distinctly_par(left, right)

    def test_callable_pair_named_arg_subtype(self) -> None:
        # Identical object args with a named arg: is_subtype(left, right,
        # ignore_pos_arg_names=True) is True, so min_verbosity resolves to 1
        # natively (matching Python's verbosity bump).
        left = CallableType([self.fx.o], [ARG_NAMED], ["x"], self.fx.o, self.fx.function)
        right = CallableType([self.fx.o], [ARG_NAMED], ["x"], self.fx.o, self.fx.function)
        self._assert_distinctly_par(left, right)

    def test_single_instance_pair(self) -> None:
        # Non-callable pair: the decision is trivially verbosity 0 natively.
        self._assert_distinctly_par(self.fx.a, self.fx.b)

    def test_mixed_callable_instance(self) -> None:
        # One callable, one instance: no verbosity bump, must stay in parity.
        c = CallableType([self.fx.a], [ARG_POS], [None], self.fx.o, self.fx.function)
        self._assert_distinctly_par(c, self.fx.b)

    def _named_arg_callable(self) -> CallableType:
        return CallableType([self.fx.a], [ARG_NAMED], ["x"], self.fx.o, self.fx.function)

    def test_named_arg_callable_engages_native_pretty(self) -> None:
        # A named/optional signature takes the pretty path. Without a
        # definition the wire is lossless, so the Rust pretty_callable port
        # renders it and the shim must not defer.
        from mypy.messages import format_type_bare

        c = self._named_arg_callable()
        off = self._with_gate(False, lambda: format_type_bare(c, self.options))
        on = self._with_gate(True, lambda: format_type_bare(c, self.options))
        assert_equal(on, off)
        raw = _type_kernel.rust_format_type_bare(
            self._bytes_of(c), self.resolver, 0, False, True, False, True
        )
        assert raw is not None, "Rust pretty path did not engage"

    def test_definition_dependent_callable_defers(self) -> None:
        # name=None means pretty_callable falls back to definition.name; the
        # wire cannot carry it, so the shim marks the tree unsafe and Rust
        # keeps deferring (parity through the pure-Python formatter).
        from mypy.messages import _pretty_wire_safe, format_type_bare
        from mypy.nodes import ARG_POS, Argument, Block, FuncDef, Var

        fdef = FuncDef("f", [Argument(Var("x"), None, None, ARG_POS)], Block([]))
        c = self._named_arg_callable()
        c.definition = fdef
        assert not _pretty_wire_safe(c)
        off = self._with_gate(False, lambda: format_type_bare(c, self.options))
        on = self._with_gate(True, lambda: format_type_bare(c, self.options))
        assert_equal(on, off)
        raw = _type_kernel.rust_format_type_bare(
            self._bytes_of(c), self.resolver, 0, False, True, False, False
        )
        assert raw is None, "Rust rendered a definition-dependent pretty callable"

    def test_definition_wire_safe_callable_engages(self) -> None:
        # With a callable name present and no extra special first parameter,
        # the definition is irrelevant to pretty_callable, so the native
        # pretty path stays parity-safe and engages.
        from mypy.messages import _pretty_wire_safe
        from mypy.nodes import ARG_POS, Argument, Block, FuncDef, Var

        fdef = FuncDef("f", [Argument(Var("x"), None, None, ARG_POS)], Block([]))
        c = self._named_arg_callable()
        c.name = "f"
        c.definition = fdef
        assert _pretty_wire_safe(c)
        raw = _type_kernel.rust_format_type_bare(
            self._bytes_of(c), self.resolver, 0, False, True, False, True
        )
        assert raw is not None, "Rust pretty path did not engage for a safe definition"

    def _def_dependent(self, name: str, arg_names: list[str], call_name: str | None) -> CallableType:
        from mypy.nodes import ARG_POS, Argument, Block, FuncDef, Var

        fdef = FuncDef(
            name, [Argument(Var(a), None, None, ARG_POS) for a in arg_names], Block([])
        )
        c = self._named_arg_callable()
        c.name = call_name
        c.definition = fdef
        return c

    def test_definition_name_hint_engages(self) -> None:
        # name=None means pretty_callable takes the function name from the
        # definition; the shim hands Rust that scalar instead of deferring.
        from mypy.messages import format_type_distinctly

        c = self._def_dependent("f", ["x"], None)
        off = self._with_gate(
            False, lambda: format_type_distinctly(c, options=self.options, bare=True)
        )
        on = self._with_gate(
            True, lambda: format_type_distinctly(c, options=self.options, bare=True)
        )
        assert_equal(on, off)
        raw = _type_kernel.rust_format_type_distinctly(
            [self._bytes_of(c)], self.resolver, True, True, False, True, [("f", None)]
        )
        assert raw == list(off), "Rust name-hint render diverged"

    def test_definition_first_arg_hint_engages(self) -> None:
        # The definition declares an extra leading parameter, so
        # pretty_callable prepends `self`; the hint carries it.
        from mypy.messages import format_type_distinctly

        c = self._def_dependent("g", ["self", "x"], None)
        off = self._with_gate(
            False, lambda: format_type_distinctly(c, options=self.options, bare=True)
        )
        on = self._with_gate(
            True, lambda: format_type_distinctly(c, options=self.options, bare=True)
        )
        assert_equal(on, off)
        raw = _type_kernel.rust_format_type_distinctly(
            [self._bytes_of(c)], self.resolver, True, True, False, True, [("g", "self")]
        )
        assert raw == list(off), "Rust first-arg-hint render diverged"

    def test_nested_definition_dependent_callable_defers(self) -> None:
        # A definition-dependent callable nested under another type has no
        # live cursor in Rust: the shim must keep the pure-Python fallback.
        from mypy.messages import _pretty_wire_safe, format_type_distinctly

        c = self._def_dependent("f", ["x"], None)
        outer = Instance(self.fx.std_listi, [c])
        assert not _pretty_wire_safe(outer)
        off = self._with_gate(
            False, lambda: format_type_distinctly(outer, options=self.options, bare=True)
        )
        on = self._with_gate(
            True, lambda: format_type_distinctly(outer, options=self.options, bare=True)
        )
        assert_equal(on, off)
        raw = _type_kernel.rust_format_type_distinctly(
            [self._bytes_of(outer)], self.resolver, True, True, False, False, [None]
        )
        assert raw is None, "nested definition-dependent render must defer"

    def test_recursive_alias_pretty_walk_terminates(self) -> None:
        # Issue #1532: pin every expansion result (blocking id reuse) so
        # the alias-identity cut must bound the walk; without it the
        # walk terminates only by allocation luck.
        import mypy.messages as messages_mod
        from mypy.messages import _pretty_wire_safe, _unsafe_pretty_callables

        A, _target = self.fx.def_alias_2(self.fx.a)
        assert A.is_recursive
        real = messages_mod.get_proper_type  # type: ignore[attr-defined]
        retained: list[Type] = []
        calls = 0

        def pinned(t: Type) -> Type:
            nonlocal calls
            calls += 1
            if calls > 64:
                raise AssertionError("pretty walk re-expanded a recursive alias")
            out = real(t)
            retained.append(out)
            return out

        messages_mod.get_proper_type = pinned  # type: ignore[attr-defined, assignment]
        try:
            calls = 0
            assert _pretty_wire_safe(A) is True
            safe_calls = calls
            calls = 0
            assert _unsafe_pretty_callables(A) == []
            unsafe_calls = calls
        finally:
            messages_mod.get_proper_type = real  # type: ignore[attr-defined]
        assert safe_calls <= 8, f"safe walk expanded {safe_calls} times"
        assert unsafe_calls <= 8, f"unsafe walk expanded {unsafe_calls} times"

    def test_recursive_alias_format_parity(self) -> None:
        # End-to-end pin: gate-on formatting of a recursive alias (alias
        # registered in the resolver snapshot) terminates and matches the
        # pure-Python formatter.
        from mypy.messages import format_type
        from mypy.wirefixup import set_wire_alias_map

        A, _target = self.fx.def_alias_2(self.fx.a)
        alias_node = A.alias
        assert alias_node is not None
        type_infos = [self.fx.oi, self.fx.ai, self.fx.bi, self.fx.ci, self.fx.functioni]
        set_wire_alias_map({alias_node.fullname: alias_node})
        self._set_resolver(_type_kernel.build_native_resolver(type_infos, [alias_node]))
        try:
            off = self._with_gate(False, lambda: format_type(A, self.options))
            on = self._with_gate(True, lambda: format_type(A, self.options))
        finally:
            set_wire_alias_map(None)
            self._set_resolver(self.resolver)
        assert_equal(on, off, "recursive alias format parity")

@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFindTypeOverlapsSuite(Suite):
    """Parity tests for the Rust find_type_overlaps port (mypy.messages).

    find_type_overlaps(*types) returns the set of fullnames whose short
    names collide across the given types (plus the typing.* injections for
    TYPES_FOR_UNIMPORTED_HINTS). The Rust seam (rust_find_type_overlaps)
    decodes the wire blobs and computes the same set; the Python body stays
    untouched and runs when the gate is off or the seam defers.

    Each test compares the pure-Python result (gate off) with the seam
    result (gate on) and asserts they are identical.
    """

    def setUp(self) -> None:
        from mypy.messages import _set_native_messages_active

        self.fx = TypeFixture()
        self._set_active = _set_native_messages_active
        self._set_active(True)
        self._buf = _WriteBuffer()

    def tearDown(self) -> None:
        self._set_active(False)

    def _bytes_of(self, t: Type) -> bytes:
        self._buf = _WriteBuffer()
        t.write(self._buf)
        return self._buf.getvalue()

    def _dup_a(self) -> Instance:
        # A realistic TypeInfo for class A in module other: defn.name "A"
        # (short name), fullname "other.A". make_type_info("other.A") cannot
        # produce this shape, so build the TypeInfo directly.
        from mypy.nodes import Block, ClassDef, SymbolTable, TypeInfo

        class_def = ClassDef("A", Block([]), None, [])
        class_def.fullname = "other.A"
        info = TypeInfo(SymbolTable(), class_def, "other")
        info.mro = [info]
        info.bases = []
        return Instance(info, [])

    def _with_gate(self, active: bool, fn: Callable[[], T]) -> T:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def assert_overlap_par(self, *types: Type) -> None:
        from mypy.messages import find_type_overlaps

        off = self._with_gate(False, lambda: find_type_overlaps(*types))
        on = self._with_gate(True, lambda: find_type_overlaps(*types))
        assert_equal(on, off, f"find_type_overlaps parity {types}")
        assert_equal(set(on), set(off), f"find_type_overlaps set parity {types}")

    def _assert_engages(self, *types: Type) -> None:
        result = _type_kernel.rust_find_type_overlaps([self._bytes_of(t) for t in types])
        assert result is not None, f"rust find_type_overlaps did not engage for {types}"

    def test_overlapping_pair(self) -> None:
        # Two classes with the same short name in different modules.
        self.assert_overlap_par(self.fx.a, self._dup_a())
        self._assert_engages(self.fx.a, self._dup_a())

    def test_disjoint_pair(self) -> None:
        # Different short names, no overlap.
        self.assert_overlap_par(self.fx.a, self.fx.b)

    def test_three_types_one_overlap(self) -> None:
        # A collides with another A; C is disjoint; only A fullnames return.
        self.assert_overlap_par(self.fx.a, self._dup_a(), self.fx.c)
        self._assert_engages(self.fx.a, self._dup_a(), self.fx.c)

    def test_no_overlap(self) -> None:
        # Three disjoint names: empty set.
        self.assert_overlap_par(self.fx.a, self.fx.b, self.fx.c)

    def test_typing_injection(self) -> None:
        # `List` also in TYPES_FOR_UNIMPORTED_HINTS: typing.List is injected
        # into the short-name group when a user List collides.
        from mypy.nodes import Block, ClassDef, SymbolTable, TypeInfo

        class_def = ClassDef("List", Block([]), None, [])
        class_def.fullname = "m.List"
        info = TypeInfo(SymbolTable(), class_def, "m")
        info.mro = [info]
        info.bases = []
        lst2 = Instance(info, [])
        self.assert_overlap_par(lst2, lst2)
        self._assert_engages(lst2)

    def test_typevar_scoped_name(self) -> None:
        # Same short TypeVar name, different namespaces: both scoped names
        # are returned.
        id1 = TypeVarId(1, namespace="mod1")
        id2 = TypeVarId(2, namespace="mod2")
        tv1 = TypeVarType("T", "T", id1, [], self.fx.o, self.fx.o)
        tv2 = TypeVarType("T", "T", id2, [], self.fx.o, self.fx.o)
        self.assert_overlap_par(tv1, tv2)
        self._assert_engages(tv1, tv2)

    def test_union_inner_overlap(self) -> None:
        # Overlap nested inside a union arg is detected.
        u = UnionType.make_union([self.fx.a, self._dup_a()])
        self.assert_overlap_par(u)
        self._assert_engages(u)

    def test_alias_defers_to_python(self) -> None:
        # TypeAliasType: the wire carries no alias node, so the Rust seam
        # defers and the pure-Python body runs (gate-on == gate-off).
        from mypy.messages import find_type_overlaps
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.TA", "mod", -1, -1)
        alias_type = TypeAliasType(alias, [])
        # The seam must decline (None) on the alias wire form.
        assert (
            _type_kernel.rust_find_type_overlaps([self._bytes_of(alias_type)]) is None
        ), "rust should defer on TypeAliasType"
        off = self._with_gate(False, lambda: find_type_overlaps(alias_type))
        on = self._with_gate(True, lambda: find_type_overlaps(alias_type))
        assert_equal(on, off, "alias find_type_overlaps parity")

@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeInferredTypeNoteSuite(Suite):
    """Gate-on/off differential for the make_inferred_type_note port (#982).

    `rust_make_inferred_type_note` mirrors the pure bool decision of
    `mypy.messages.make_inferred_type_note` (messages.py:3770-3800): two
    same-fullname generic Instances whose every per-arg `is_subtype` result
    is true, in a `ReturnStmt` whose expression is an inferred `Var` via a
    `NameExpr`. Python keeps note emission. Each test compares gate-off
    (pure-Python) and gate-on (Rust decision + Python emission) results,
    plus direct seam calls proving engagement.
    """

    def setUp(self) -> None:
        from mypy.messages import _set_native_messages_active
        from mypy.test.typefixture import TypeFixture as _TypeFixture

        self.fx = _TypeFixture()
        self._set_active = _set_native_messages_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    @staticmethod
    def _return_ctx(name: str = "x", inferred: bool = True) -> Context:
        from mypy.nodes import ReturnStmt

        expr = NameExpr(name)
        var = Var(name)
        var.is_inferred = inferred
        expr.node = var
        return ReturnStmt(expr)

    def _note(self, context: Context, subtype: Type, supertype: Type) -> str:
        from mypy.messages import make_inferred_type_note

        return make_inferred_type_note(context, subtype, supertype, "list[A]")

    def _assert_par(self, context: Context, subtype: Type, supertype: Type) -> None:
        self._set_active(False)
        off = self._note(context, subtype, supertype)
        self._set_active(True)
        on = self._note(context, subtype, supertype)
        assert_equal(on, off, f"inferred type note mismatch {subtype} vs {supertype}")

    def test_note_fires(self) -> None:
        # List[B] returned where list[A] is expected, B <: A, inferred var:
        # the note fires and both gates produce the identical message.
        ctx = self._return_ctx("x")
        expected = 'Perhaps you need a type annotation for "x"? Suggestion: list[A]'
        self._assert_par(ctx, self.fx.lstb, self.fx.lsta)
        self._set_active(True)
        assert_equal(self._note(ctx, self.fx.lstb, self.fx.lsta), expected)

    def test_var_not_inferred(self) -> None:
        self._assert_par(self._return_ctx("x", inferred=False), self.fx.lstb, self.fx.lsta)

    def test_context_not_return_stmt(self) -> None:
        self._assert_par(NameExpr("x"), self.fx.lstb, self.fx.lsta)

    def test_expr_not_name_expr(self) -> None:
        from mypy.nodes import ReturnStmt

        self._assert_par(ReturnStmt(IntExpr(1)), self.fx.lstb, self.fx.lsta)

    def test_node_not_var(self) -> None:
        from mypy.nodes import ReturnStmt

        expr = NameExpr("x")
        expr.node = self.fx.a  # type: ignore[assignment]
        self._assert_par(ReturnStmt(expr), self.fx.lstb, self.fx.lsta)

    def test_different_fullnames(self) -> None:
        # Both generic Instances but different classes: no note.
        self._assert_par(self._return_ctx(), self.fx.ga, self.fx.gb)

    def test_arg_not_subtype(self) -> None:
        # List[A] vs List[B]: A is not a subtype of B.
        self._assert_par(self._return_ctx(), self.fx.lsta, self.fx.lstb)

    def test_non_instance_types(self) -> None:
        # AnyType and non-generic Instances never produce the note.
        self._assert_par(self._return_ctx(), AnyType(TypeOfAny.special_form), self.fx.lsta)
        self._assert_par(self._return_ctx(), self.fx.a, self.fx.b)

    def test_direct_seam_engagement(self) -> None:
        subtype = self.fx.lstb
        supertype = self.fx.lsta
        buf = _WriteBuffer()
        subtype.write(buf)
        sub_bytes = buf.getvalue()
        buf = _WriteBuffer()
        supertype.write(buf)
        sup_bytes = buf.getvalue()
        fires = _type_kernel.rust_make_inferred_type_note(
            sub_bytes, sup_bytes, [True], self._return_ctx("y")
        )
        self.assertTrue(fires, "seam should decide True for List[B] vs List[A] in return")
        not_fires = _type_kernel.rust_make_inferred_type_note(
            sub_bytes, sup_bytes, [False], self._return_ctx("y")
        )
        self.assertFalse(not_fires, "seam should decide False on a false per-arg result")
        self.assertFalse(
            _type_kernel.rust_make_inferred_type_note(sub_bytes, sup_bytes, [True], NameExpr("y")),
            "seam should decide False for a non-ReturnStmt context",
        )

@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeHasNoAttrSuite(Suite):
    """Gate-on/off differential for rust_classify_has_no_attr (issue #1006).

    Rust arbitrates the `Messages.has_no_attr` dispatch head
    (mypy/messages.py:364-569) from scalar facts and returns a
    (tag, op, matches) triple; Python applies the fail / note /
    unsupported_left_operand side effects and all formatting. Each
    differential test runs has_no_attr with the gate off (pure-Python
    body) and on (the shim) and asserts identical messages and codes.
    Direct seam tests pin every tag.
    """

    def setUp(self) -> None:
        from mypy.messages import _set_native_messages_active

        self.fx = TypeFixture()
        self.options = Options()
        self._set_active = _set_native_messages_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _make_builder(self) -> tuple[Any, list[tuple[str, Any]]]:
        from mypy import errorcodes as codes
        from mypy.errors import Errors
        from mypy.messages import MessageBuilder

        errors = Errors(self.options)
        builder = MessageBuilder(errors, {})
        captured: list[tuple[str, Any]] = []
        builder.fail = lambda msg, ctx, code=None, **kw: captured.append((msg, code))  # type: ignore[method-assign,assignment]
        builder.note = lambda msg, ctx, offset=0, code=None, **kw: captured.append((msg, code))  # type: ignore[method-assign]

        def _left_operand(op: str, typ: Type, ctx: Context) -> None:
            captured.append(("__left__:" + op, codes.OPERATOR))

        builder.unsupported_left_operand = _left_operand  # type: ignore[method-assign,assignment]
        return builder, captured

    def _assert_par(
        self,
        original_type: Type,
        typ: Type,
        member: str,
        module_symbol_table: SymbolTable | None = None,
        disable_type_names: bool = False,
    ) -> tuple[Any, list[tuple[str, Any]]]:
        results = []
        for active in (False, True):
            self._set_active(active)
            builder, captured = self._make_builder()
            if disable_type_names:
                builder._disable_type_names = [True]
            code = builder.has_no_attr(original_type, typ, member, Context(), module_symbol_table)
            results.append((code, captured))
        self._set_active(True)
        assert_equal(results[1], results[0], f"has_no_attr({original_type}, {member!r}) parity")
        return results[0]

    def _seam(
        self,
        member: str,
        is_instance: bool = False,
        is_function_like: bool = False,
        is_type_obj: bool = False,
        is_union: bool = False,
        is_typevar: bool = False,
        typevar_bound_is_union: bool = False,
        has_readable_member: bool = False,
        instance_fullname: str = "",
        are_type_names_disabled: bool = False,
        instance_has_names: bool = False,
        module_private: bool = False,
        instance_names: list[str] | None = None,
        module_public_names: list[str] | None = None,
    ) -> tuple[int, str, list[str]]:
        return _type_kernel.rust_classify_has_no_attr(
            member,
            is_instance,
            is_function_like,
            is_type_obj,
            is_union,
            is_typevar,
            typevar_bound_is_union,
            has_readable_member,
            instance_fullname,
            are_type_names_disabled,
            instance_has_names,
            module_private,
            instance_names or [],
            module_public_names or [],
        )

    def _info(self, fullname: str, names: dict[str, str]) -> Instance:
        # Build a live TypeInfo with the given member names (all Vars).
        from mypy.nodes import Block, ClassDef

        short = fullname.split(".")[-1]
        class_def = ClassDef(short, Block([]), None, [])
        class_def.fullname = fullname
        info = TypeInfo(SymbolTable(), class_def, fullname.split(".")[0])
        info.mro = [info]
        info.bases = []
        for name in names:
            info.names[name] = SymbolTableNode(MDEF, Var(name, self.fx.a))
        return Instance(info, [])

    def _module_table(self, entries: dict[str, bool]) -> SymbolTable:
        # SymbolTableNode.module_public defaults to None (falsy); only the
        # entries listed as public get an explicit True.
        table = SymbolTable()
        for name, public in entries.items():
            node = SymbolTableNode(MDEF, Var(name, self.fx.a))
            node.module_public = public
            table[name] = node
        return table

    # -- direct seam tests ------------------------------------------------

    def test_seam_not_assignable(self) -> None:
        assert self._seam("x", is_instance=True, has_readable_member=True)[0] == 0

    def test_seam_operators(self) -> None:
        assert self._seam("__contains__")[0] == 1
        tag, op, _ = self._seam("__add__")
        assert (tag, op) == (2, "+")
        assert self._seam("__neg__")[0] == 3
        assert self._seam("__pos__")[0] == 4
        assert self._seam("__invert__")[0] == 5

    def test_seam_index_and_call(self) -> None:
        assert self._seam("__getitem__", is_function_like=True, is_type_obj=True)[0] == 6
        assert self._seam("__getitem__", is_function_like=True)[0] == 7
        assert self._seam("__getitem__")[0] == 7
        assert self._seam("__setitem__")[0] == 8
        assert (
            self._seam("__call__", is_instance=True, instance_fullname="builtins.function")[0] == 9
        )
        assert self._seam("__call__")[0] == 10

    def test_seam_suggestions(self) -> None:
        # COMMON_MISTAKES["add"] = ("append", "extend").
        tag, _, matches = self._seam(
            "add", is_instance=True, instance_has_names=True, instance_names=["append"]
        )
        assert (tag, matches) == (12, ["append"])
        # Matches may also come from public module names.
        tag, _, matches = self._seam(
            "appnd",
            is_instance=True,
            instance_has_names=True,
            instance_names=["apend"],
            module_public_names=["append"],
        )
        assert tag == 12
        assert "append" in matches and "apend" in matches

    def test_seam_tail_tags(self) -> None:
        assert (
            self._seam("x", is_instance=True, instance_has_names=True, module_private=True)[0]
            == 11
        )
        assert (
            self._seam("zz", is_instance=True, instance_has_names=True, instance_names=["a"])[0]
            == 13
        )
        assert self._seam("x", is_instance=True, instance_has_names=False)[0] == 16
        # With type names enabled, union and typevar shapes land in the
        # plain else-branch; their own messages need the names-disabled
        # mode.
        assert self._seam("x", is_union=True)[0] == 16
        assert self._seam("x", is_typevar=True, typevar_bound_is_union=True)[0] == 16
        assert self._seam("x", is_typevar=True, typevar_bound_is_union=False)[0] == 16
        # Disabled: union item, typevar union bound, silent typevar, plain.
        assert self._seam("x", is_union=True, are_type_names_disabled=True)[0] == 14
        assert (
            self._seam(
                "x", is_typevar=True, typevar_bound_is_union=True, are_type_names_disabled=True
            )[0]
            == 15
        )
        assert (
            self._seam(
                "x", is_typevar=True, typevar_bound_is_union=False, are_type_names_disabled=True
            )[0]
            == 17
        )
        assert self._seam("zz", is_instance=True, are_type_names_disabled=True)[0] == 16

    # -- gate-off vs gate-on differentials ---------------------------------

    def test_not_assignable(self) -> None:
        inst = self._info("mod.C", {"x": "var"})
        code, captured = self._assert_par(inst, inst, "x")
        assert code is None
        assert captured == [('Member "x" is not assignable', None)]

    def test_contains(self) -> None:
        from mypy import errorcodes as codes

        code, captured = self._assert_par(self.fx.a, self.fx.a, "__contains__")
        assert code == codes.OPERATOR
        assert captured[0][0] == 'Unsupported right operand type for in ("A")'

    def test_binary_op_method(self) -> None:
        from mypy import errorcodes as codes

        code, captured = self._assert_par(self.fx.a, self.fx.a, "__add__")
        assert code == codes.OPERATOR
        assert captured == [("__left__:+", codes.OPERATOR)]

    def test_unary_operators(self) -> None:
        from mypy import errorcodes as codes

        code, captured = self._assert_par(self.fx.a, self.fx.a, "__neg__")
        assert code == codes.OPERATOR
        assert captured[0][0] == 'Unsupported operand type for unary - ("A")'
        self._assert_par(self.fx.a, self.fx.a, "__pos__")
        self._assert_par(self.fx.a, self.fx.a, "__invert__")

    def test_getitem_type_obj(self) -> None:
        # A class object: FunctionLike with is_type_obj() True.
        type_obj = CallableType([], [], [], self.fx.a, self.fx.type_type)
        code, captured = self._assert_par(type_obj, type_obj, "__getitem__")
        assert code is None
        assert "not generic and not indexable" in captured[0][0]

    def test_getitem_not_indexable(self) -> None:
        from mypy import errorcodes as codes

        code, captured = self._assert_par(self.fx.a, self.fx.a, "__getitem__")
        assert code == codes.INDEX
        assert captured[0][0] == 'Value of type "A" is not indexable'

    def test_setitem(self) -> None:
        from mypy import errorcodes as codes

        code, captured = self._assert_par(self.fx.a, self.fx.a, "__setitem__")
        assert code == codes.INDEX
        assert "Unsupported target for indexed assignment" in captured[0][0]

    def test_call(self) -> None:
        from mypy import errorcodes as codes

        code, captured = self._assert_par(self.fx.a, self.fx.a, "__call__")
        assert code == codes.OPERATOR
        assert captured[0][0] == '"A" not callable'
        code, captured = self._assert_par(self.fx.function, self.fx.function, "__call__")
        assert code == codes.OPERATOR
        assert captured[0][0] == "Cannot call function of unknown type"

    def test_module_private_export(self) -> None:
        from mypy import errorcodes as codes

        inst = self._info("other.mod.C", {"a": "var"})
        table = self._module_table({"x": False})
        code, captured = self._assert_par(inst, inst, "x", table)
        assert code == codes.ATTR_DEFINED
        assert 'does not explicitly export attribute "x"' in captured[0][0]

    def test_suggestion_message(self) -> None:
        from mypy import errorcodes as codes

        inst = self._info("m.C", {"append": "var"})
        code, captured = self._assert_par(inst, inst, "add")
        assert code == codes.ATTR_DEFINED
        assert captured[0][0] == '"C" has no attribute "add"; maybe "append"?'

    def test_plain_attribute(self) -> None:
        from mypy import errorcodes as codes

        inst = self._info("m.C", {"append": "var"})
        code, captured = self._assert_par(inst, inst, "zz")
        assert code == codes.ATTR_DEFINED
        assert captured[0][0] == '"C" has no attribute "zz"'

    def test_union_item(self) -> None:
        from mypy import errorcodes as codes

        u = UnionType([self.fx.a, self.fx.nonet])
        # The union-item message only fires with type names disabled; the
        # not-disabled tail produces the plain attr-defined message.
        code, captured = self._assert_par(u, self.fx.a, "x", disable_type_names=True)
        assert code == codes.UNION_ATTR
        assert captured[0][0] == 'Item "A" of "A | None" has no attribute "x"'
        code, captured = self._assert_par(u, self.fx.a, "x")
        assert code == codes.ATTR_DEFINED
        assert captured[0][0] == '"A | None" has no attribute "x"'

    def test_union_none_swap(self) -> None:
        from mypy import errorcodes as codes

        # The None-swap needs typ_format == '"object"'; find_type_overlaps
        # forces fullnames here ("builtins.object"), so the swap does not
        # trigger under this fixture and both gate sides render fullnames.
        u = UnionType([self.fx.o, self.fx.nonet])
        code, captured = self._assert_par(u, self.fx.o, "x", disable_type_names=True)
        assert code == codes.UNION_ATTR
        assert (
            captured[0][0]
            == 'Item "builtins.object" of "builtins.object | None" has no attribute "x"'
        )

    def test_typevar_union_bound(self) -> None:
        from mypy import errorcodes as codes

        tv = TypeVarType(
            "T",
            "T",
            TypeVarId(1),
            [],
            UnionType([self.fx.a, self.fx.nonet]),
            AnyType(TypeOfAny.from_omitted_generics),
        )
        code, captured = self._assert_par(tv, self.fx.a, "x", disable_type_names=True)
        assert code == codes.UNION_ATTR
        assert captured[0][0] == (
            'Item "A" of the upper bound "A | None" of type variable "T" has no attribute "x"'
        )

    def test_typevar_plain_bound_silent(self) -> None:
        # Only the names-disabled tail is silent for a non-union bound; the
        # not-disabled tail produces the plain attr-defined message.
        tv = TypeVarType(
            "T", "T", TypeVarId(1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )
        code, captured = self._assert_par(tv, self.fx.a, "x", disable_type_names=True)
        assert code is None
        assert captured == []
        code, captured = self._assert_par(tv, self.fx.a, "x")
        assert captured[0][0] == '"T" has no attribute "x"'

    def test_type_names_disabled(self) -> None:
        # With type names disabled the tail switches to the disabled
        # dispatch chain; a suggestion-capable Instance gets the plain
        # message instead of the suggestion.
        inst = self._info("m.C", {"append": "var"})
        code, captured = self._assert_par(inst, inst, "add", disable_type_names=True)
        assert code is not None and code.code == "attr-defined"
        assert captured[0][0] == '"C" has no attribute "add"'

@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFormatAliasTopSuite(Suite):
    """Gate-on/off differential for the fmt:alias_top retirement (#1447).

    `rust_format_type_bare` mirrors `format_type_bare` (messages.py:3298).
    The TypeAliasType arm of the formatter now expands non-recursive
    aliases through the alias snapshots (`get_proper_type`, line 3015)
    instead of deferring every alias; recursive aliases (Python renders
    the live alias name), aliases missing from the snapshot, and
    unexpandable shapes (variadic aliases / substitution walls) still
    defer, so the formatting stays byte-identical in both gates.
    """

    def setUp(self) -> None:
        from mypy.messages import _set_native_messages_active, _set_native_messages_resolver
        from mypy.options import Options
        from mypy.test.typefixture import TypeFixture as _TypeFixture

        self.fx = _TypeFixture()
        self.options = Options()
        self._set_active = _set_native_messages_active
        self._set_resolver = _set_native_messages_resolver
        self._rebuild_resolver([])
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)
        self._set_resolver(None)

    def _rebuild_resolver(self, aliases: list[Any]) -> None:
        type_infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                type_infos.append(value)
        self.resolver = _type_kernel.build_native_resolver(type_infos, aliases)
        self._set_resolver(self.resolver)

    def _with_gate(self, active: bool, fn: Callable[[], Any]) -> Any:
        self._set_active(active)
        try:
            return fn()
        finally:
            self._set_active(True)

    def _seam_bare(self, typ: Type, verbosity: int = 0) -> str | None:
        from mypy.messages import _serialize_type_for_messages

        return _type_kernel.rust_format_type_bare(
            _serialize_type_for_messages(typ),
            self.resolver,
            verbosity,
            False,
            self.options.use_star_unpack(),
        )

    def _assert_format_par(self, typ: Type, verbosity: int = 0) -> None:
        from mypy.messages import format_type_bare

        off = self._with_gate(False, lambda: format_type_bare(typ, self.options, verbosity))
        on = format_type_bare(typ, self.options, verbosity)
        assert_equal(on, off, f"format_type_bare(alias) parity {typ}")

    def _make_alias(
        self, target: Type, fullname: str = "__main__.A", *, alias_tvars: list[Any] | None = None
    ) -> tuple[TypeAliasType, TypeAlias]:
        from mypy.nodes import TypeAlias as _TypeAlias

        node = _TypeAlias(target, fullname, "__main__", -1, -1, alias_tvars=alias_tvars)
        return TypeAliasType(node, []), node

    def test_non_recursive_expansion_parity(self) -> None:
        # A = list[A]; the formatter expands to the target Instance and
        # both gates emit "list[A]" byte-identically.
        from mypy.messages import format_type_bare

        alias, node = self._make_alias(Instance(self.fx.std_listi, [self.fx.a]))
        self._rebuild_resolver([node])
        self._assert_format_par(alias)
        expected = self._with_gate(False, lambda: format_type_bare(alias, self.options))
        assert_equal(self._seam_bare(alias), expected, "direct seam mismatch")

    def test_generic_alias_substitution_parity(self) -> None:
        # A[T] = list[T]; A[A] -> list[A] with the wire arg substituted.
        from mypy.messages import format_type_bare

        alias, node = self._make_alias(
            Instance(self.fx.std_listi, [self.fx.t]), alias_tvars=[self.fx.t]
        )
        self._rebuild_resolver([node])
        applied = TypeAliasType(node, [self.fx.a])
        self._assert_format_par(applied)
        expected = self._with_gate(False, lambda: format_type_bare(applied, self.options))
        assert_equal(self._seam_bare(applied), expected, "direct seam mismatch")

    def test_chain_alias_parity(self) -> None:
        # A = B; B = list[A]: the chain resolves through both snapshots
        # before formatting ("list[A]").
        from mypy.messages import format_type_bare

        b_target = Instance(self.fx.std_listi, [self.fx.a])
        _, b_node = self._make_alias(b_target, "__main__.B")
        b_ref = TypeAliasType(b_node, [])
        a, a_node = self._make_alias(b_ref)
        self._rebuild_resolver([a_node, b_node])
        self._assert_format_par(a)
        expected = self._with_gate(False, lambda: format_type_bare(a, self.options))
        assert_equal(self._seam_bare(a), expected, "direct seam mismatch")

    def test_verbosity_fullname_parity(self) -> None:
        # verbosity >= 2 prints the expanded type with fullnames; the
        # alias arm must pass verbosity through to the recursive format.
        alias, node = self._make_alias(Instance(self.fx.std_listi, [self.fx.a]))
        self._rebuild_resolver([node])
        self._assert_format_par(alias, verbosity=2)

    def test_recursive_alias_defers(self) -> None:
        # A = list[A]: is_recursive is flagged on the wire; the seam has
        # no display-name channel and must defer (Python renders "A").
        from mypy.messages import format_type_bare

        inner = TypeAliasType(None, [])
        alias = TypeAlias(Instance(self.fx.std_listi, [inner]), "__main__.A", "__main__", -1, -1)
        inner.alias = alias
        alias._is_recursive = True
        self._rebuild_resolver([alias])
        assert self._seam_bare(inner) is None, "recursive alias must defer"
        assert_equal(
            self._with_gate(False, lambda: format_type_bare(inner, self.options)),
            format_type_bare(inner, self.options),
            "recursive alias parity",
        )

    def test_missing_snapshot_defers(self) -> None:
        # The alias node is not in the resolver snapshot; the seam cannot
        # expand and must defer (Python renders from the live node).
        from mypy.messages import format_type_bare

        alias, _ = self._make_alias(Instance(self.fx.std_listi, [self.fx.a]))
        self._rebuild_resolver([])
        assert self._seam_bare(alias) is None, "missing-snapshot alias must defer"
        assert_equal(
            self._with_gate(False, lambda: format_type_bare(alias, self.options)),
            format_type_bare(alias, self.options),
            "missing-snapshot alias parity",
        )

    def test_alias_within_union_parity(self) -> None:
        # A non-recursive alias nested inside a union argument formats
        # through the same arm during the recursive format.
        alias, node = self._make_alias(Instance(self.fx.std_listi, [self.fx.a]))
        self._rebuild_resolver([node])
        self._assert_format_par(UnionType([self.fx.a, alias]))
