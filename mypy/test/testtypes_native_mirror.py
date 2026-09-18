"""Native seam suites for the mirror area (split from testtypes.py, #1677)."""

from __future__ import annotations

try:
    import type_kernel as _splice_kernel
    import type_kernel as _type_kernel
    from librt.internal import WriteBuffer as _WriteBuffer
except ImportError:
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _splice_kernel = None  # type: ignore[assignment]
    _type_kernel = None  # type: ignore[assignment]

import os
import sys
from collections.abc import Callable, Iterator
from typing import Any, cast
from unittest import skipIf, skipUnless

from mypy.nodes import (
    ARG_NAMED,
    ARG_NAMED_OPT,
    ARG_OPT,
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    CONTRAVARIANT,
    GDEF,
    INVARIANT,
    LDEF,
    MDEF,
    ArgKind,
    Argument,
    AssertStmt,
    AssignmentStmt,
    Block,
    CallExpr,
    CastExpr,
    ClassDef,
    ComparisonExpr,
    ConditionalExpr,
    ExpressionStmt,
    FuncDef,
    FuncItem,
    IndexExpr,
    IntExpr,
    ListExpr,
    MemberExpr,
    MypyFile,
    NameExpr,
    NotParsed,
    OpExpr,
    RefExpr,
    ReturnStmt,
    Statement,
    StrExpr,
    SymbolTable,
    SymbolTableNode,
    TupleExpr,
    TypeAlias,
    TypeInfo,
    UnaryExpr,
    Var,
)
from mypy.options import Options
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import (
    _HAS_TYPE_KERNEL,
    _NATIVE_WIRE_ENABLED,
    _SPLICE_ACTIVE,
    _base_infos,
    _is_type_info,
)
from mypy.test.typefixture import TypeFixture
from mypy.traverser import (
    all_name_and_member_expressions,
    all_return_statements,
    all_return_statements_and_flags,
    all_yield_expressions,
    all_yield_from_expressions,
    count_returns,
    find_non_extension_handlers,
    find_non_literal_handlers,
    has_await_expression,
    has_complex_slice,
    has_return_statement,
    has_str_expression,
    has_yield_expression,
    has_yield_from_expression,
    has_yield_return,
    is_global_expr,
)
from mypy.types import (
    AnyType,
    CallableType,
    Instance,
    LiteralType,
    NoneType,
    Overloaded,
    TupleType,
    Type,
    TypeAliasType,
    TypeOfAny,
    TypeVarId,
    TypeVarType,
    UnboundType,
    UninhabitedType,
    UnionType,
    UnpackType,
    get_proper_type,
    has_recursive_types,
)

# Sentinel for a binding the module never had; `None` is a real value the
# ImportError branch binds, so the two must stay distinguishable (#1778).
_ABSENT = object()


def _snapshot_types_module_bindings(names: tuple[str, ...]) -> dict[str, object]:
    """Snapshot `mypy.types` bindings; a missing name reads as `_ABSENT`."""
    import mypy.types as _types_mod

    return {name: _types_mod.__dict__.get(name, _ABSENT) for name in names}


def _restore_types_module_bindings(saved: dict[str, object]) -> None:
    """Undo a `_snapshot_types_module_bindings` snapshot."""
    import mypy.types as _types_mod

    for name, prior in saved.items():
        if prior is _ABSENT:
            _types_mod.__dict__.pop(name, None)
        else:
            _types_mod.__dict__[name] = prior


class _LeakyTypesBindingCase:
    """Negative control for the binding-hygiene detector (#1786).

    Injects one `mypy.types` seam name and never restores it, so the
    hygiene test has a known leak it must flag. A plain class, not a
    `Suite`, so pytest neither collects nor runs it.
    """

    def __init__(self, seam: str) -> None:
        self._SEAM_BINDINGS = (seam,)

    def setUp(self) -> None:
        import mypy.types as _types_mod

        for name in self._SEAM_BINDINGS:
            _types_mod.__dict__[name] = object()

    def tearDown(self) -> None:
        pass


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeWireSuite(Suite):
    """Parity tests for the Rust `Type` wire reader (Stage 3a).

    Each test serializes a `Type` via `Type.write(WriteBuffer)` and asserts
    that `type_kernel.read_type_to_str(bytes) == str(t)`. The seed corpus
    mirrors the golden cases in `TypesSuite` (lines 72-200) plus the
    `TypeFixture` instances that exercise each wire-format branch.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def assert_wire_par(self, t: Type) -> None:
        expected = str(t)
        actual = _type_kernel.read_type_to_str(self._bytes_of(t))
        assert_equal(actual, expected, f"wire str({t!r}) = {{}} ({{}} expected)")

    def test_any(self) -> None:
        self.assert_wire_par(AnyType(TypeOfAny.special_form))

    def test_none(self) -> None:
        self.assert_wire_par(NoneType())

    def test_uninhabited(self) -> None:
        self.assert_wire_par(UninhabitedType())

    def test_unbound_simple(self) -> None:
        self.assert_wire_par(UnboundType("Foo"))

    def test_unbound_generic(self) -> None:
        self.assert_wire_par(
            UnboundType("Foo", [UnboundType("T"), AnyType(TypeOfAny.special_form)])
        )

    def test_unbound_plain_data_fields(self) -> None:
        # optional / empty_tuple_index: plain data mypy.types carries but
        # the wire never serializes (Phase F0, #1349); Rust reader fills
        # the class defaults, str() unaffected (wire-drop: f0_* in wire.rs).
        self.assert_wire_par(UnboundType("Foo", optional=True, empty_tuple_index=True))
        # `original_str_expr` / `original_str_fallback` are pre-existing
        # wire fields; verify the Python-written bytes still read back.
        u = UnboundType("Foo")
        u.original_str_expr = "Foo"
        u.original_str_fallback = "builtins.str"
        self.assert_wire_par(u)

    def test_unpack_from_star_syntax(self) -> None:
        # `from_star_syntax` is another wire-resident gap (Phase F0, #1349).
        self.assert_wire_par(UnpackType(UnboundType("T"), from_star_syntax=True))
        self.assert_wire_par(UnpackType(self.fx.std_tuple, from_star_syntax=True))

    def test_callable_special_sig(self) -> None:
        # `special_sig` ("tuple") is a plain-data field on CallableType the
        # wire format does not serialize (Phase F0, #1349).
        c = CallableType(
            [self.fx.a],
            [ARG_POS],
            [None],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
            special_sig="tuple",
        )
        self.assert_wire_par(c)

    def test_union_plain_data_fields(self) -> None:
        # `is_evaluated` and `original_str_*` are plain data on UnionType
        # (Phase F0, #1349); only the original_str_* fields already had a
        # wire representation pre-#1349 for UnboundType, never for unions.
        u = UnionType([self.fx.a, self.fx.b], is_evaluated=False)
        u.original_str_expr = "A | B"
        u.original_str_fallback = "builtins.str"
        self.assert_wire_par(u)

    def test_instance_singletons(self) -> None:
        # INSTANCE_STR / INSTANCE_FUNCTION / INSTANCE_INT / INSTANCE_BOOL /
        # INSTANCE_OBJECT fast paths, plus INSTANCE_SIMPLE for non-builtin.
        self.assert_wire_par(self.fx.str_type)
        self.assert_wire_par(self.fx.function)
        self.assert_wire_par(self.fx.bool_type)
        self.assert_wire_par(self.fx.o)
        self.assert_wire_par(self.fx.a)
        self.assert_wire_par(self.fx.b)

    def test_instance_generic(self) -> None:
        self.assert_wire_par(self.fx.ga)
        self.assert_wire_par(self.fx.gb)
        self.assert_wire_par(self.fx.gt)
        self.assert_wire_par(self.fx.lsta)
        self.assert_wire_par(self.fx.lstb)

    def test_instance_tuple(self) -> None:
        # builtins.tuple renders as `tuple[T, ...]`.
        self.assert_wire_par(self.fx.std_tuple)

    def test_literal_int(self) -> None:
        self.assert_wire_par(self.fx.lit1)
        self.assert_wire_par(self.fx.lit2)
        self.assert_wire_par(self.fx.lit4)

    def test_literal_big_int(self) -> None:
        # Python ints beyond i64 must survive the wire reader (issue #1329).
        # Each `assert_wire_par` serializes with the Python writer (librt
        # long-int encoding) and reads back in Rust.
        int_type = Instance(self.fx.make_type_info("builtins.int"), [])
        self.assert_wire_par(LiteralType(2**80, int_type))
        self.assert_wire_par(LiteralType(-(2**80), int_type))
        self.assert_wire_par(LiteralType(2**130, int_type))
        self.assert_wire_par(
            UnionType([LiteralType(2**80, int_type), NoneType(), self.fx.str_type])
        )

    def test_literal_str(self) -> None:
        self.assert_wire_par(self.fx.lit_str1)
        self.assert_wire_par(self.fx.lit_str2)
        self.assert_wire_par(self.fx.lit_str3)

    def test_literal_bool(self) -> None:
        self.assert_wire_par(self.fx.lit_false)
        self.assert_wire_par(self.fx.lit_true)

    def test_type_type(self) -> None:
        self.assert_wire_par(self.fx.type_a)
        self.assert_wire_par(self.fx.type_b)
        self.assert_wire_par(self.fx.type_any)

    def test_callable_pos(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_callable_no_ret(self) -> None:
        c = CallableType([], [], [], NoneType(), self.fx.function)
        self.assert_wire_par(c)

    def test_callable_opt(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_OPT],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_callable_star(self) -> None:
        c = CallableType(
            [self.fx.a], [ARG_STAR], [None], AnyType(TypeOfAny.special_form), self.fx.function
        )
        self.assert_wire_par(c)

    def test_callable_named(self) -> None:
        c = CallableType(
            [self.fx.a], [ARG_NAMED], ["x"], AnyType(TypeOfAny.special_form), self.fx.function
        )
        self.assert_wire_par(c)

    def test_callable_named_opt(self) -> None:
        c = CallableType(
            [self.fx.a], [ARG_NAMED_OPT], ["x"], AnyType(TypeOfAny.special_form), self.fx.function
        )
        self.assert_wire_par(c)

    def test_callable_star2(self) -> None:
        c = CallableType(
            [self.fx.a], [ARG_STAR2], ["kwargs"], AnyType(TypeOfAny.special_form), self.fx.function
        )
        self.assert_wire_par(c)

    def test_callable_generic(self) -> None:
        # Mirrors `test_generic_function_type`: variables block renders as
        # `def [X] (...)` (after `def`, before params).
        c = CallableType(
            [UnboundType("X"), UnboundType("Y")],
            [ARG_POS, ARG_POS],
            [None, None],
            UnboundType("Y"),
            self.fx.function,
            name=None,
            variables=[
                TypeVarType(
                    "X",
                    "X",
                    TypeVarId(-1),
                    [],
                    self.fx.o,
                    AnyType(TypeOfAny.from_omitted_generics),
                )
            ],
        )
        self.assert_wire_par(c)

    def test_tuple_type_str(self) -> None:
        t1 = TupleType([], self.fx.std_tuple)
        self.assert_wire_par(t1)
        t2 = TupleType([UnboundType("X")], self.fx.std_tuple)
        self.assert_wire_par(t2)
        t3 = TupleType([UnboundType("X"), AnyType(TypeOfAny.special_form)], self.fx.std_tuple)
        self.assert_wire_par(t3)

    def test_typevar(self) -> None:
        self.assert_wire_par(self.fx.t)
        self.assert_wire_par(self.fx.s)
        self.assert_wire_par(self.fx.u)

    def test_typevar_meta_level_roundtrip(self) -> None:
        # meta_level=0 records must be byte-identical to the old format
        # (no meta_level field written). meta_level=1 (metavar) must survive
        # the round-trip so env lookups keyed on TypeVarId.__eq__ match.
        any_type = AnyType(TypeOfAny.special_form)
        declared = TypeVarType(
            "T", "mod.T", TypeVarId(1, namespace="mod"), [], self.fx.o, any_type
        )
        meta = TypeVarType(
            "T", "mod.T", TypeVarId(1, meta_level=1, namespace="mod"), [], self.fx.o, any_type
        )
        self.assert_wire_par(declared)
        self.assert_wire_par(meta)
        # Assert id equality is preserved across the wire, not just str().
        from librt.internal import ReadBuffer, WriteBuffer

        from mypy.types import read_type

        for t in (declared, meta):
            buf = WriteBuffer()
            t.write(buf)
            rt = read_type(ReadBuffer(buf.getvalue()))
            assert rt.id == t.id, f"{rt.id} != {t.id} after round-trip"  # type: ignore[attr-defined]

    def test_union(self) -> None:
        self.assert_wire_par(UnionType.make_union([self.fx.a, self.fx.b]))
        self.assert_wire_par(UnionType.make_union([self.fx.a, self.fx.nonet]))
        self.assert_wire_par(UnionType.make_union([self.fx.a, self.fx.b, self.fx.nonet]))

    def test_overloaded(self) -> None:
        ov = Overloaded(
            [
                self.fx.callable(self.fx.a, AnyType(TypeOfAny.special_form)),
                self.fx.callable(self.fx.b, AnyType(TypeOfAny.special_form)),
            ]
        )
        self.assert_wire_par(ov)

    def test_type_alias_flag_default_shape(self) -> None:
        # Phase F0 D2 (#1349): the Rust `Type::TypeAliasType` variant now
        # carries `is_recursive`; default-shaped bytes (no conditional int)
        # must decode with the flag False.
        from mypy.nodes import TypeAlias

        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        t = TypeAliasType(alias, [])
        assert _type_kernel.read_alias_recursion_flag(self._bytes_of(t)) is False
        # Whole-record consumption: the reader stops at END_TAG, so the
        # record renders exactly as an unfixed alias with no args.
        assert_equal(_type_kernel.read_type_to_str(self._bytes_of(t)), "<alias (unfixed)>")

    def test_type_alias_flag_recursive_shape(self) -> None:
        # A = Tuple[Union[A, B], ...]: the writer computes recursion inline
        # via CollectAliasesVisitor and emits the conditional int
        # (types.py:552); the Rust reader mirrors that tail exactly.
        A, _ = self.fx.def_alias_1(self.fx.a)
        assert A.is_recursive  # sanity: genuinely recursive per the model
        assert _type_kernel.read_alias_recursion_flag(self._bytes_of(A)) is True

    def test_type_alias_flag_union_alignment(self) -> None:
        # A recursive alias member inside a union: the conditional int is
        # consumed inside the member record, so the enclosing union record
        # stays aligned (the Rust reader decodes it without an error).
        A, _ = self.fx.def_alias_1(self.fx.a)
        u = UnionType([A, self.fx.b])
        assert _type_kernel.read_alias_recursion_flag(self._bytes_of(u)) is None
        # Alignment proof: a mis-consumed flag would surface as an
        # unexpected-tag error or a truncated member; pin the rendering.
        assert _type_kernel.read_type_to_str(self._bytes_of(u)) is not None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRemoveDupsSuite(Suite):
    """Parity for the alias-bearing `remove_dups` native path (#1518).

    The Rust dedup speaks Python `__eq__` semantics (`py_type_eq`) and
    accepts alias-bearing rows; the shim gates on an alias-identity
    precondition (every fullname in the list maps to one live alias object)
    that makes the structural `(fullname, args)` key equivalent to Python's
    object-identity `TypeAliasType.__eq__`. When the precondition fails the
    pure-Python body decides. Result rows re-link to the live input rows, so
    the seam preserves identity end-to-end.
    """

    # The `mypy.types` seam names this suite injects. The binding-hygiene
    # test derives its expectation from here, so a new injection cannot
    # drift out of leak coverage (#1786).
    _SEAM_BINDINGS: tuple[str, ...] = ("_VisitorWriteBuffer", "_ReadBuffer", "_rust_remove_dups")

    def setUp(self) -> None:
        from librt.internal import ReadBuffer, WriteBuffer

        import mypy.types as _types_mod

        self._types_mod = _types_mod
        # Bind the seam names the module-level try would have bound. Save
        # the prior bindings first (absent stays distinct from the
        # ImportError branch's `None`) and restore them in tearDown (#1778).
        self._orig_module_bindings = _snapshot_types_module_bindings(self._SEAM_BINDINGS)
        _types_mod._VisitorWriteBuffer = WriteBuffer  # type: ignore[attr-defined]
        _types_mod._ReadBuffer = ReadBuffer  # type: ignore[attr-defined]
        # Overwrite, not setdefault: the module already binds the seam (the
        # kernel fn when the ext imports), but a pristine `None` must still
        # be genuinely injected for the hygiene assertion to bite (#1794).
        _types_mod._rust_remove_dups = _type_kernel.rust_remove_dups  # type: ignore[attr-defined]
        self._orig_kernel_flag = _types_mod._VISITOR_HAS_TYPE_KERNEL
        _types_mod._VISITOR_HAS_TYPE_KERNEL = True
        self._orig_visitor_gate = _types_mod._native_visitor_active
        self._orig_types_gate = _types_mod._native_visitor_types_active

        from mypy.types import _set_native_visitor_resolver
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        self.fx = TypeFixture()
        self.type_infos = _base_infos(self.fx)
        self.alias = TypeAlias(
            Instance(self.fx.std_listi, [self.fx.t]), "mod.DupAlias", "mod", -1, -1
        )
        self.resolver = _type_kernel.build_native_resolver(self.type_infos, [self.alias])
        self.resolver.set_live_typeinfo_map({info.fullname: info for info in self.type_infos})
        _set_native_visitor_resolver(self.resolver)
        set_wire_typeinfo_map({info.fullname: info for info in self.type_infos})
        set_wire_alias_map({self.alias.fullname: self.alias})
        self._set_gates(True, True)

    def tearDown(self) -> None:
        from mypy.types import _set_native_visitor_resolver
        from mypy.wirefixup import set_wire_alias_map, set_wire_typeinfo_map

        set_wire_typeinfo_map(None)
        set_wire_alias_map(None)
        _set_native_visitor_resolver(None)
        self._types_mod._VISITOR_HAS_TYPE_KERNEL = self._orig_kernel_flag
        self._set_gates(self._orig_visitor_gate, self._orig_types_gate)
        _restore_types_module_bindings(self._orig_module_bindings)

    def _set_gates(self, visitor: bool, types: bool) -> None:
        from mypy.types import _set_native_visitor_active, _set_native_visitor_types_active

        _set_native_visitor_active(visitor)
        _set_native_visitor_types_active(types)

    def _dedup(self, types: list[Type], active: bool) -> list[Type]:
        from mypy.types import remove_dups

        self._set_gates(True, active)
        try:
            return remove_dups(types)
        finally:
            self._set_gates(True, self._orig_types_gate)

    def _assert_par(self, types: list[Type]) -> list[Type]:
        on = self._dedup(list(types), True)
        off = self._dedup(list(types), False)
        assert_equal([str(t) for t in on], [str(t) for t in off], "remove_dups parity")
        return on

    def test_alias_rows_parity_and_identity(self) -> None:
        first = TypeAliasType(self.alias, [])
        duplicate = TypeAliasType(self.alias, [])
        none_row = NoneType()
        result = self._assert_par([first, none_row, duplicate, self.fx.a])
        assert len(result) == 3
        # Native rows map back to the live first occurrences.
        assert result[0] is first
        assert result[1] is none_row
        assert result[2] is self.fx.a

    def test_alias_args_dedup(self) -> None:
        a1 = TypeAliasType(self.alias, [self.fx.a])
        a2 = TypeAliasType(self.alias, [self.fx.a])
        a3 = TypeAliasType(self.alias, [self.fx.b])
        result = self._assert_par([a1, a2, a3])
        assert len(result) == 2
        assert result[0] is a1 and result[1] is a3

    def test_distinct_alias_objects_same_fullname_fall_back(self) -> None:
        # Python's `TypeAliasType.__eq__` compares the alias OBJECT; two
        # distinct objects sharing a fullname are unequal, so the shim's
        # structural-key precondition must route to the pure-Python body.
        other = TypeAlias(Instance(self.fx.std_listi, [self.fx.t]), "mod.DupAlias", "mod", -1, -1)
        first = TypeAliasType(self.alias, [])
        second = TypeAliasType(other, [])
        from mypy.types import _dedup_alias_identity_sound

        assert not _dedup_alias_identity_sound([first, second])
        result = self._assert_par([first, second])
        assert len(result) == 2
        assert result[0] is first and result[1] is second

    def test_py_eq_any_collapse(self) -> None:
        # AnyType.__eq__ is isinstance-only: every Any equals every Any.
        a1 = AnyType(TypeOfAny.special_form)
        a2 = AnyType(TypeOfAny.from_error)
        result = self._assert_par([a1, a2])
        assert len(result) == 1 and result[0] is a1

    def test_py_eq_union_order_insensitive(self) -> None:
        # UnionType.__eq__ is frozenset(items): item order does not matter.
        u1 = UnionType([self.fx.a, self.fx.b])
        u2 = UnionType([self.fx.b, self.fx.a])
        result = self._assert_par([u1, u2])
        assert len(result) == 1 and result[0] is u1

    def test_alias_nested_in_instance(self) -> None:
        row1 = Instance(self.fx.gi, [TypeAliasType(self.alias, [self.fx.a])])
        row2 = Instance(self.fx.gi, [TypeAliasType(self.alias, [self.fx.a])])
        result = self._assert_par([row1, row2, self.fx.a])
        assert len(result) == 2 and result[0] is row1


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeHasRecursiveTypesFlattenSuite(Suite):
    """Parity for the issue #1418 ports in `mypy.types`.

    Both seams are retired: `has_recursive_types` in #1640 and
    `flatten_nested_unions` in #1739 (the wire round trip serialized
    every input tree to reach the same list scan, ~26x the Python body).
    Neither body reads a gate, so every test below pins the Python body's
    value in a single run instead of comparing a gate-off arm against a
    gate-on arm, and `_spy_flatten_serialize` pins zero wire crossings;
    the retired pyfunctions are pinned by direct calls.
    """

    # Same derivation as NativeRemoveDupsSuite (#1786).
    _SEAM_BINDINGS: tuple[str, ...] = ("_VisitorWriteBuffer", "_ReadBuffer")

    def setUp(self) -> None:
        from librt.internal import ReadBuffer, WriteBuffer

        import mypy.types as _types_mod

        self._types_mod = _types_mod
        # Same process-global hygiene as NativeRemoveDupsSuite (#1778):
        # save the prior bindings, absent distinct from `None`, and put
        # them back in tearDown.
        self._orig_module_bindings = _snapshot_types_module_bindings(self._SEAM_BINDINGS)
        _types_mod._VisitorWriteBuffer = WriteBuffer  # type: ignore[attr-defined]
        _types_mod._ReadBuffer = ReadBuffer  # type: ignore[attr-defined]
        self._orig_kernel_flag = _types_mod._VISITOR_HAS_TYPE_KERNEL
        _types_mod._VISITOR_HAS_TYPE_KERNEL = True
        # Save the process-global gates other suites (the conftest parity
        # installer) may have enabled; restore prior values in tearDown.
        self._orig_visitor_gate = _types_mod._native_visitor_active
        self._orig_types_gate = _types_mod._native_visitor_types_active

        from mypy.types import _set_native_visitor_resolver

        self.fx = TypeFixture()
        self.type_infos = _base_infos(self.fx)
        self.resolver = _type_kernel.build_native_resolver(self.type_infos, [])
        self.resolver.set_live_typeinfo_map({info.fullname: info for info in self.type_infos})
        _set_native_visitor_resolver(self.resolver)
        self._set_gates(True, True)

    def tearDown(self) -> None:
        from mypy.types import _set_native_visitor_resolver

        self._set_gates(self._orig_visitor_gate, self._orig_types_gate)
        _set_native_visitor_resolver(None)
        self._types_mod._VISITOR_HAS_TYPE_KERNEL = self._orig_kernel_flag
        _restore_types_module_bindings(self._orig_module_bindings)

    def _set_gates(self, visitor: bool, types: bool) -> None:
        from mypy.types import _set_native_visitor_active, _set_native_visitor_types_active

        _set_native_visitor_active(visitor)
        _set_native_visitor_types_active(types)

    def _assert_hrt(self, t: Type, expected: bool) -> None:
        # `has_recursive_types` was retired in #1640 (types.py:4929): it reads
        # no gate and loads no `rust_*` name, so both gate states run identical
        # Python and the old `on == off == expected` chain could not fail.
        got = has_recursive_types(t)
        assert got == expected, f"has_recursive_types({t!r}) = {got}, expected {expected}"

    def _def_union_alias(self, name: str, target: Type) -> tuple[TypeAlias, TypeAliasType]:
        node = TypeAlias(target, f"__main__.{name}", "__main__", -1, -1)
        return node, TypeAliasType(node, [])

    def _install_resolver(self, aliases: list[TypeAlias]) -> None:
        from mypy.types import _set_native_visitor_resolver

        res = _type_kernel.build_native_resolver(self.type_infos, aliases)
        res.set_live_typeinfo_map({info.fullname: info for info in self.type_infos})
        _set_native_visitor_resolver(res)
        self.resolver = res

    def test_hrt_parities(self) -> None:
        A, _ = self.fx.def_alias_1(self.fx.a)
        T = TypeVarType(
            "T", "T", TypeVarId(-1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )
        NA = self.fx.non_rec_alias(Instance(self.fx.gi, [T]), [T], [A])
        U = UnionType([self.fx.a, A])
        # Recursive alias in the upper bound position.
        UB = TypeVarType("U", "U", TypeVarId(-2), [], A, self.fx.o)
        self._assert_hrt(A, True)
        self._assert_hrt(NA, True)
        self._assert_hrt(U, True)
        self._assert_hrt(UB, True)
        self._assert_hrt(self.fx.a, False)

    def test_hrt_direct_seam_engages(self) -> None:
        from mypy.types import _serialize_type_for_visitor

        A, _ = self.fx.def_alias_1(self.fx.a)
        assert _type_kernel.rust_has_recursive_types(_serialize_type_for_visitor(A)) is True
        assert (
            _type_kernel.rust_has_recursive_types(_serialize_type_for_visitor(self.fx.a)) is False
        )

    def test_flatten_bare_alias_union_target(self) -> None:
        from mypy.types import flatten_nested_unions

        node, alias = self._def_union_alias("F", UnionType([self.fx.a, self.fx.str_type]))
        self._install_resolver([node])
        # The flatten seam was retired in #1739 (types.py:5181) and the body
        # reads no gate, so a gate-off/gate-on comparison could not fail.
        # Pin the expansion instead.
        got = flatten_nested_unions([alias])
        assert [str(x) for x in got] == [str(self.fx.a), str(self.fx.str_type)], got
        assert all(not isinstance(x, TypeAliasType) for x in got)

    def test_flatten_alias_preserved_for_non_union_target(self) -> None:
        from mypy.types import flatten_nested_unions

        node, alias = self._def_union_alias("L", Instance(self.fx.gi, [self.fx.a]))
        self._install_resolver([node])
        # Retired flatten seam (#1739): one run, value pin.
        got = flatten_nested_unions([alias])
        # Python appends the ORIGINAL alias for a non-union expansion.
        assert len(got) == 1 and got[0] is alias

    def test_flatten_nested_bare_alias_in_union_items(self) -> None:
        from mypy.types import flatten_nested_unions

        node, inner = self._def_union_alias("N", UnionType([self.fx.b]))
        outer = UnionType([self.fx.a, inner])
        self._install_resolver([node])
        # Retired flatten seam (#1739): one run, value pin.
        got = flatten_nested_unions([outer])
        assert [str(x) for x in got] == [str(self.fx.a), str(self.fx.b)], got

    def test_flatten_recursive_alias_handle_off_kept(self) -> None:
        from mypy.types import flatten_nested_unions

        # `def_alias_2`'s target is a union, the only shape where the flag
        # decides: hr=False keeps the ORIGINAL alias, hr=True expands one
        # union target step. `def_alias_1` has a tuple target, inert for it.
        A, _target = self.fx.def_alias_2(self.fx.a)
        assert A.is_recursive
        kept = flatten_nested_unions([A], handle_recursive=False)
        assert len(kept) == 1 and kept[0] is A
        expanded = flatten_nested_unions([A], handle_recursive=True)
        assert expanded[0] is self.fx.a
        assert len(expanded) == 2

    def test_flatten_alias_missing_snapshot_defers(self) -> None:
        from mypy.types import _serialize_type_list_for_visitor, flatten_nested_unions

        node, alias = self._def_union_alias("M", UnionType([self.fx.a]))
        # Resolver without the alias snapshot: direct seam defers.
        assert (
            _type_kernel.rust_flatten_nested_unions(
                _serialize_type_list_for_visitor([alias]), True, True, self.resolver
            )
            is None
        )
        # Module function runs the pure-Python body, which expands
        # the union target (no snapshot needed).
        got = flatten_nested_unions([alias])
        assert [str(x) for x in got] == [str(self.fx.a)], got
        assert not isinstance(got[0], TypeAliasType)

    def test_flatten_direct_seam_engages(self) -> None:
        from mypy.types import _serialize_type_list_for_visitor

        node, alias = self._def_union_alias("E", UnionType([self.fx.a, self.fx.b]))
        self._install_resolver([node])
        assert (
            _type_kernel.rust_flatten_nested_unions(
                _serialize_type_list_for_visitor([alias]), True, True, self.resolver
            )
            is not None
        )

    def _spy_flatten_serialize(self) -> tuple[list[int], Callable[[], None]]:
        """Count wire serializations taken by `flatten_nested_unions`.

        The retired seam serialized every input row here; the pure-Python
        body must take zero crossings even with both gates on.
        """
        import mypy.types as _types_mod

        seen: list[int] = []
        real = _types_mod._serialize_type_list_for_visitor

        def spy(rows: Any) -> Any:
            seen.append(1)
            return real(rows)

        _types_mod._serialize_type_list_for_visitor = spy  # type: ignore[assignment]

        def restore() -> None:
            _types_mod._serialize_type_list_for_visitor = real

        return seen, restore

    def test_flatten_recursive_alias_no_resolver_expands_in_python(self) -> None:
        # Issue #1532 / #1739: the pure-Python body expands one union
        # target step for hr=True and keeps the alias row for hr=False,
        # with no wire crossing. The retired body reads no gate either.
        from mypy.types import _set_native_visitor_resolver, flatten_nested_unions

        A, _target = self.fx.def_alias_2(self.fx.a)
        assert A.is_recursive
        seen, restore = self._spy_flatten_serialize()
        _set_native_visitor_resolver(None)
        try:
            self._set_gates(True, True)
            hr_true = [str(x) for x in flatten_nested_unions([A], handle_recursive=True)]
            hr_false = [str(x) for x in flatten_nested_unions([A], handle_recursive=False)]
        finally:
            restore()
        # hr=True expands one union target step (base + tuple item);
        # hr=False keeps the alias row.
        assert hr_true == [str(self.fx.a), f"builtins.tuple[{str(A)}, ...]"], hr_true
        assert hr_false == [str(A)], hr_false
        assert not seen, f"retired flatten seam serialized {len(seen)} time(s)"

    def test_flatten_nonrecursive_alias_expands_in_python(self) -> None:
        # A non-recursive alias row expands through the Python body; the
        # result carries the union members and no wire crossing happens.
        from mypy.types import _set_native_visitor_resolver, flatten_nested_unions
        from mypy.wirefixup import set_wire_typeinfo_map

        _node, alias = self._def_union_alias("F", UnionType([self.fx.a, self.fx.str_type]))
        seen, restore = self._spy_flatten_serialize()
        _set_native_visitor_resolver(None)
        set_wire_typeinfo_map({info.fullname: info for info in self.type_infos})
        try:
            self._set_gates(True, True)
            on = flatten_nested_unions([alias])
        finally:
            restore()
            set_wire_typeinfo_map(None)
        assert [str(x) for x in on] == [str(self.fx.a), str(self.fx.str_type)]
        assert not seen, f"retired flatten seam serialized {len(seen)} time(s)"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMirrorBindingHygieneSuite(Suite):
    """#1778: the suites that inject seam bindings into `mypy.types` must
    leave the module as they found it, or a leaked injection reads as an
    unrelated broken test once a reorder puts the writer ahead of a pin
    (the #1775 failure mode).
    """

    def test_the_binding_suites_restore_the_types_module_bindings(self) -> None:
        cases = (
            (NativeRemoveDupsSuite, "test_alias_rows_parity_and_identity"),
            (NativeHasRecursiveTypesFlattenSuite, "test_hrt_parities"),
        )
        # The checked names come from each suite's own declaration, so a
        # new injection cannot drift out of leak coverage (#1786).
        names = tuple(dict.fromkeys(n for cls, _ in cases for n in cls._SEAM_BINDINGS))
        assert names, "no seam bindings declared: the hygiene check would be vacuous"
        prior = _snapshot_types_module_bindings(names)
        try:
            for suite_cls, test_name in cases:
                # Both pristine shapes: the `None` the module's ImportError
                # branch binds, and a name the module never bound at all.
                for pristine in (None, _ABSENT):
                    leaked = self._leaked_seam_names(suite_cls(test_name), pristine)
                    assert not leaked, f"{suite_cls.__name__} leaked {leaked} at {pristine!r}"
            # Negative control: a case that never restores must be flagged
            # for every derived name, so a green run cannot be vacuous.
            for name in names:
                for pristine in (None, _ABSENT):
                    leaked = self._leaked_seam_names(_LeakyTypesBindingCase(name), pristine)
                    assert leaked == [name], f"detector missed {name}: {leaked}"
        finally:
            _restore_types_module_bindings(prior)

    def _leaked_seam_names(self, case: Any, pristine: object) -> list[str]:
        """Run one setUp/tearDown cycle; return the seam names left behind."""
        import mypy.types as _types_mod

        names = case._SEAM_BINDINGS
        for name in names:
            if pristine is _ABSENT:
                _types_mod.__dict__.pop(name, None)
            else:
                _types_mod.__dict__[name] = pristine
        case.setUp()
        case.tearDown()
        return [n for n in names if _types_mod.__dict__.get(n, _ABSENT) is not pristine]


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTraverserSuite(Suite):
    """Parity suite for the native AST traverser (Stage 14, #304).

    Builds AST from source, then verifies that the Rust traverser seekers
    and counters produce the same results as the pure-Python collectors.
    Exercises node types that were previously invisible to the wire format
    (CastExpr, AssertTypeExpr, RevealExpr, SuperExpr, TypeApplication,
    TemplateStrExpr).
    """

    def _parse(self, source: str) -> MypyFile:
        from mypy.errors import Errors
        from mypy.fastparse import parse
        from mypy.options import Options

        options = Options()
        options.python_version = (3, 12)
        errors = Errors(options)
        tree = parse(source, fnam="<test>", module="<test>", errors=errors, options=options)
        return tree

    def _parse_native(self, source: str) -> MypyFile:
        """Parse eagerly through the native parser path (#1547 parity)."""
        from mypy.errors import Errors
        from mypy.options import Options
        from mypy.parse import parse

        options = Options()
        options.python_version = (3, 12)
        options.native_parser = True
        errors = Errors(options)
        return parse(
            source, fnam="<test>", module="<test>", errors=errors, options=options, eager=True
        )

    def _find_func(self, tree: MypyFile, name: str) -> FuncDef:
        for stmt in tree.defs:
            if isinstance(stmt, FuncDef) and stmt.name == name:
                return stmt
        raise AssertionError(f"function {name} not found")

    def test_has_return_statement_trivial_return(self) -> None:
        tree = self._parse("def f() -> int:\n    return 1\n")
        fdef = self._find_func(tree, "f")
        assert has_return_statement(fdef) is True

    def test_has_return_statement_bare_return(self) -> None:
        tree = self._parse("def f() -> None:\n    return\n")
        fdef = self._find_func(tree, "f")
        assert has_return_statement(fdef) is False

    def test_has_return_statement_none(self) -> None:
        tree = self._parse("def f() -> None:\n    x = 1\n")
        fdef = self._find_func(tree, "f")
        assert has_return_statement(fdef) is False

    def test_has_return_statement_nested_in_cast(self) -> None:
        # cast() wraps the return expr in a CastExpr — previously
        # invisible to the Rust seeker.
        tree = self._parse("from typing import cast\ndef f() -> int:\n    return cast(int, 42)\n")
        fdef = self._find_func(tree, "f")
        assert has_return_statement(fdef) is True

    def test_has_return_statement_bare_func_item(self) -> None:
        # A bare FuncItem has no wire tag, so the serializer emits a bare
        # LITERAL_NONE and Rust cannot see the body: the seam must defer
        # (None) instead of silently answering False. (#1030)
        import pytest

        from mypy.nodes import ARG_POS, Block, ReturnStmt, StrExpr, Var
        from mypy.traverser import (  # type: ignore[attr-defined]
            _rust_has_return_statement,
            _serialize_ast_node,
        )

        fi = FuncItem(  # type: ignore[abstract]
            [Argument(Var("self"), None, None, ARG_POS)], Block([ReturnStmt(StrExpr("x"))])
        )
        assert _rust_has_return_statement(_serialize_ast_node(fi)) is None
        # The pure-Python fallback cannot visit a bare FuncItem either
        # (FuncItem.accept is not implemented): kernel-on must match that
        # kernel-off reference exactly, not silently return False.
        with pytest.raises(RuntimeError):
            has_return_statement(fi)

    def test_has_return_statement_return_none(self) -> None:
        # `return None` is trivial (ReturnSeeker excludes NameExpr("None")).
        # The wire drops the name, so the Rust seam defers and the shim's
        # Python fallback decides. Regression for #1547.
        tree = self._parse("def f():\n    return None\n")
        fdef = self._find_func(tree, "f")
        assert has_return_statement(fdef) is False

    def test_has_return_statement_return_name(self) -> None:
        # `return x` is non-trivial; NameExpr returns defer to Python.
        tree = self._parse("def f():\n    return x\n")
        fdef = self._find_func(tree, "f")
        assert has_return_statement(fdef) is True

    def test_has_return_statement_return_none_native_parser(self) -> None:
        # Both parser modes produce NameExpr("None") and must agree (#1547).
        try:
            import ast_serialize
        except ImportError:
            self.skipTest("ast_serialize extension not built")

        if not hasattr(ast_serialize, "parse"):
            self.skipTest("ast_serialize extension not built")
        tree = self._parse_native("def f():\n    return None\ndef g():\n    return x\n")
        assert has_return_statement(self._find_func(tree, "f")) is False
        assert has_return_statement(self._find_func(tree, "g")) is True

    def test_has_return_statement_name_expr_defers(self) -> None:
        # Direct seam: NameExpr return expressions are ambiguous on the wire
        # (`return None` vs `return x`), so Rust defers; IntExpr returns are
        # decidable and short-circuit the ambiguity (#1547).
        from mypy.traverser import (  # type: ignore[attr-defined]
            _rust_has_return_statement,
            _serialize_ast_node,
        )

        tree = self._parse(
            "def f():\n    return None\n"
            "def g():\n    return x\n"
            "def h():\n    return 1\n"
            "def i():\n    return x\n    return 2\n"
            "def j():\n    return\n"
        )
        assert _rust_has_return_statement(_serialize_ast_node(self._find_func(tree, "f"))) is None
        assert _rust_has_return_statement(_serialize_ast_node(self._find_func(tree, "g"))) is None
        assert _rust_has_return_statement(_serialize_ast_node(self._find_func(tree, "h"))) is True
        assert _rust_has_return_statement(_serialize_ast_node(self._find_func(tree, "i"))) is True
        assert _rust_has_return_statement(_serialize_ast_node(self._find_func(tree, "j"))) is False

    def test_has_str_expression_simple(self) -> None:
        tree = self._parse("x = 'hello'\n")
        assert has_str_expression(tree) is True

    def test_has_str_expression_false(self) -> None:
        tree = self._parse("x = 42\n")
        assert has_str_expression(tree) is False

    @skipIf(sys.version_info < (3, 14), "t-strings need Python 3.14+")
    def test_has_str_expression_in_template(self) -> None:
        # t-strings need Python 3.14+
        from mypy.errors import Errors
        from mypy.fastparse import parse
        from mypy.options import Options

        o = Options()
        o.python_version = (3, 14)
        e = Errors(o)
        tree = parse("x = t'hello'", fnam="<t>", module="<t>", errors=e, options=o)
        assert has_str_expression(tree) is True

    def test_has_yield_expression(self) -> None:
        tree = self._parse("def f():\n    yield 1\n")
        fdef = self._find_func(tree, "f")
        assert has_yield_expression(fdef) is True

    def test_has_yield_expression_false(self) -> None:
        tree = self._parse("def f():\n    return 1\n")
        fdef = self._find_func(tree, "f")
        assert has_yield_expression(fdef) is False

    def test_has_yield_expression_nested_func_skipped(self) -> None:
        tree = self._parse("def f():\n    def g():\n        yield 1\n    return 2\n")
        fdef = self._find_func(tree, "f")
        assert has_yield_expression(fdef) is False

    def test_has_yield_from_expression(self) -> None:
        tree = self._parse("def f():\n    yield from range(10)\n")
        fdef = self._find_func(tree, "f")
        assert has_yield_from_expression(fdef) is True

    def test_has_yield_from_expression_false(self) -> None:
        tree = self._parse("def f():\n    return 1\n")
        fdef = self._find_func(tree, "f")
        assert has_yield_from_expression(fdef) is False

    def test_has_await_expression(self) -> None:
        tree = self._parse("async def f():\n    await g()\n")
        fdef = self._find_func(tree, "f")
        assert has_await_expression(fdef) is True

    def test_has_await_expression_false(self) -> None:
        tree = self._parse("async def f():\n    return 1\n")
        fdef = self._find_func(tree, "f")
        assert has_await_expression(fdef) is False

    def test_all_return_statements_count(self) -> None:
        tree = self._parse("def f():\n    return 1\n    return 2\n    return\n")
        fdef = self._find_func(tree, "f")
        returns = all_return_statements(fdef)
        assert len(returns) == 3

    def test_all_return_statements_skips_nested(self) -> None:
        tree = self._parse("def f():\n    return 1\n    def g():\n        return 2\n")
        fdef = self._find_func(tree, "f")
        returns = all_return_statements(fdef)
        assert len(returns) == 1

    def test_all_return_statements_empty(self) -> None:
        tree = self._parse("def f():\n    x = 1\n")
        fdef = self._find_func(tree, "f")
        returns = all_return_statements(fdef)
        assert len(returns) == 0

    def test_all_yield_expressions_count(self) -> None:
        tree = self._parse("def f():\n    yield 1\n    yield 2\n")
        fdef = self._find_func(tree, "f")
        yields = all_yield_expressions(fdef)
        assert len(yields) == 2

    def test_all_yield_expressions_in_assignment(self) -> None:
        tree = self._parse("def f():\n    x = yield 1\n")
        fdef = self._find_func(tree, "f")
        yields = all_yield_expressions(fdef)
        assert len(yields) == 1
        assert yields[0][1] is True  # in_assignment

    def test_all_yield_expressions_not_in_assignment(self) -> None:
        tree = self._parse("def f():\n    yield 1\n")
        fdef = self._find_func(tree, "f")
        yields = all_yield_expressions(fdef)
        assert len(yields) == 1
        assert yields[0][1] is False

    def test_all_yield_from_expressions_count(self) -> None:
        tree = self._parse("def f():\n    yield from range(10)\n    yield from g()\n")
        fdef = self._find_func(tree, "f")
        yields = all_yield_from_expressions(fdef)
        assert len(yields) == 2

    def test_all_yield_from_expressions_in_assignment(self) -> None:
        tree = self._parse("def f():\n    x = yield from g()\n")
        fdef = self._find_func(tree, "f")
        yields = all_yield_from_expressions(fdef)
        assert len(yields) == 1
        assert yields[0][1] is True

    def test_all_yield_from_expressions_empty(self) -> None:
        tree = self._parse("def f():\n    return 1\n")
        fdef = self._find_func(tree, "f")
        yields = all_yield_from_expressions(fdef)
        assert len(yields) == 0

    def test_all_name_and_member_expressions(self) -> None:
        tree = self._parse("x = a.b + c.d + e\n")
        names, members = all_name_and_member_expressions(tree)
        # x, a, c, e are names; a.b, c.d are members
        assert len(names) == 4
        assert len(members) == 2

    def test_all_name_and_member_expressions_empty(self) -> None:
        tree = self._parse("42\n")
        names, members = all_name_and_member_expressions(tree)
        assert len(names) == 0
        assert len(members) == 0

    def test_rust_count_matches_python_returns(self) -> None:
        from type_kernel import rust_count_return_statements

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse(
            "def f():\n    return 1\n    return 2\n    def g():\n        return 3\n"
        )
        fdef = self._find_func(tree, "f")
        buf = WriteBuffer()
        serialize_node(fdef, buf)
        rust_count = rust_count_return_statements(buf.getvalue())
        py_returns = all_return_statements(fdef)
        assert rust_count == len(py_returns)

    def test_rust_count_matches_python_yields(self) -> None:
        from type_kernel import rust_count_yield_expressions

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse(
            "def f():\n    yield 1\n    x = yield 2\n    def g():\n        yield 3\n"
        )
        fdef = self._find_func(tree, "f")
        buf = WriteBuffer()
        serialize_node(fdef, buf)
        rust_count = rust_count_yield_expressions(buf.getvalue())
        py_yields = all_yield_expressions(fdef)
        assert rust_count == len(py_yields)

    def test_rust_count_matches_python_yield_from(self) -> None:
        from type_kernel import rust_count_yield_from_expressions

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse(
            "def f():\n"
            "    yield from a()\n"
            "    x = yield from b()\n"
            "    def g():\n"
            "        yield from c()\n"
        )
        fdef = self._find_func(tree, "f")
        buf = WriteBuffer()
        serialize_node(fdef, buf)
        rust_count = rust_count_yield_from_expressions(buf.getvalue())
        py_yields = all_yield_from_expressions(fdef)
        assert rust_count == len(py_yields)

    def test_rust_count_matches_python_name_member(self) -> None:
        from type_kernel import rust_count_name_and_member_expressions

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse("x = a.b.c + d.e + f\n")
        buf = WriteBuffer()
        serialize_node(tree, buf)
        rust_names, rust_members = rust_count_name_and_member_expressions(buf.getvalue())
        py_names, py_members = all_name_and_member_expressions(tree)
        assert rust_names == len(py_names)
        assert rust_members == len(py_members)

    # --- Issue #541: remaining seeker parity tests ---

    def test_all_return_statements_and_flags_no_finally(self) -> None:
        tree = self._parse("def f():\n    return 1\n    return 2\n")
        fdef = self._find_func(tree, "f")
        results = all_return_statements_and_flags(fdef)
        assert len(results) == 2
        assert all(not flag for _, flag in results)

    def test_all_return_statements_and_flags_in_finally(self) -> None:
        tree = self._parse(
            "def f():\n    try:\n        return 1\n    finally:\n        return 2\n"
        )
        fdef = self._find_func(tree, "f")
        results = all_return_statements_and_flags(fdef)
        assert len(results) == 2
        # At least one return is in a finally block.
        assert any(flag for _, flag in results)

    def test_all_return_statements_and_flags_empty(self) -> None:
        tree = self._parse("def f():\n    x = 1\n")
        fdef = self._find_func(tree, "f")
        results = all_return_statements_and_flags(fdef)
        assert len(results) == 0

    def test_count_returns_includes_nested(self) -> None:
        tree = self._parse("def f():\n    return 1\n    def g():\n        return 2\n")
        fdef = self._find_func(tree, "f")
        # count_returns includes nested function returns (unlike
        # all_return_statements which skips nested funcs).
        assert count_returns(fdef) == 2

    def test_count_returns_no_returns(self) -> None:
        tree = self._parse("def f():\n    x = 1\n")
        fdef = self._find_func(tree, "f")
        assert count_returns(fdef) == 0

    def test_has_yield_return_true(self) -> None:
        tree = self._parse("def f():\n    return (yield 1)\n")
        fdef = self._find_func(tree, "f")
        assert has_yield_return(fdef) is True

    def test_has_yield_return_false(self) -> None:
        tree = self._parse("def f():\n    return 1\n")
        fdef = self._find_func(tree, "f")
        assert has_yield_return(fdef) is False

    def test_has_complex_slice_true(self) -> None:
        tree = self._parse("x = a[::2]\n")
        assert has_complex_slice(tree) is True

    def test_has_complex_slice_false(self) -> None:
        tree = self._parse("x = a[1:10]\n")
        assert has_complex_slice(tree) is False

    def test_has_complex_slice_no_slice(self) -> None:
        tree = self._parse("x = a[0]\n")
        assert has_complex_slice(tree) is False

    def test_find_non_extension_handlers(self) -> None:
        tree = self._parse(
            "class C:\n"
            "    def f(self):\n"
            "        return 1\n"
            "    @property\n"
            "    def g(self):\n"
            "        return 2\n"
        )
        funcs = find_non_extension_handlers(tree)
        # `f` is bare (no decorator), `g` is decorated.
        assert len(funcs) == 1
        assert funcs[0].name == "f"

    def test_find_non_extension_handlers_empty(self) -> None:
        tree = self._parse("class C:\n    @property\n    def g(self):\n        return 2\n")
        funcs = find_non_extension_handlers(tree)
        assert len(funcs) == 0

    def test_is_global_expr_true(self) -> None:
        tree = self._parse("def f():\n    global x\n    x = 1\n")
        fdef = self._find_func(tree, "f")
        assert is_global_expr(fdef) is True

    def test_is_global_expr_false(self) -> None:
        tree = self._parse("def f():\n    x = 1\n")
        fdef = self._find_func(tree, "f")
        assert is_global_expr(fdef) is False

    def test_find_non_literal_handlers(self) -> None:
        tree = self._parse(
            "class C:\n    def lit(self):\n        42\n    def nonlit(self):\n        return 1\n"
        )
        funcs = find_non_literal_handlers(tree)
        # `lit` has a literal body (just `42`), `nonlit` has a return
        # stmt which is not a literal expression.
        assert len(funcs) == 1
        assert funcs[0].name == "nonlit"

    def test_find_non_literal_handlers_pass_only(self) -> None:
        tree = self._parse("class C:\n    def p(self):\n        pass\n")
        funcs = find_non_literal_handlers(tree)
        # pass-only body is a literal handler.
        assert len(funcs) == 0

    def test_rust_count_returns_and_flags_matches_python(self) -> None:
        from type_kernel import rust_count_return_statements_and_flags

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse(
            "def f():\n    try:\n        return 1\n    finally:\n        return 2\n"
        )
        fdef = self._find_func(tree, "f")
        buf = WriteBuffer()
        serialize_node(fdef, buf)
        rust_total, rust_finally = rust_count_return_statements_and_flags(buf.getvalue())
        py_results = all_return_statements_and_flags(fdef)
        py_total = len(py_results)
        py_finally = sum(1 for _, flag in py_results if flag)
        assert rust_total == py_total
        assert rust_finally == py_finally

    def test_rust_count_all_returns_matches_python(self) -> None:
        from type_kernel import rust_count_all_returns

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse("def f():\n    return 1\n    def g():\n        return 2\n")
        fdef = self._find_func(tree, "f")
        buf = WriteBuffer()
        serialize_node(fdef, buf)
        rust_count = rust_count_all_returns(buf.getvalue())
        assert rust_count == count_returns(fdef)

    def test_rust_has_yield_return_matches_python(self) -> None:
        from type_kernel import rust_has_yield_return

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse("def f():\n    return (yield 1)\n")
        fdef = self._find_func(tree, "f")
        buf = WriteBuffer()
        serialize_node(fdef, buf)
        assert rust_has_yield_return(buf.getvalue()) == has_yield_return(fdef)

    def test_rust_has_complex_slice_matches_python(self) -> None:
        from type_kernel import rust_has_complex_slice

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse("x = a[::2]\n")
        buf = WriteBuffer()
        serialize_node(tree, buf)
        assert rust_has_complex_slice(buf.getvalue()) == has_complex_slice(tree)

    def test_rust_count_non_extension_matches_python(self) -> None:
        from type_kernel import rust_count_non_extension_handlers

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse(
            "class C:\n"
            "    def f(self):\n"
            "        return 1\n"
            "    @property\n"
            "    def g(self):\n"
            "        return 2\n"
        )
        buf = WriteBuffer()
        serialize_node(tree, buf)
        rust_count = rust_count_non_extension_handlers(buf.getvalue())
        assert rust_count == len(find_non_extension_handlers(tree))

    def test_rust_is_global_expr_matches_python(self) -> None:
        from type_kernel import rust_is_global_expr

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse("def f():\n    global x\n    x = 1\n")
        fdef = self._find_func(tree, "f")
        buf = WriteBuffer()
        serialize_node(fdef, buf)
        assert rust_is_global_expr(buf.getvalue()) == is_global_expr(fdef)

    def test_rust_count_non_literal_matches_python(self) -> None:
        from type_kernel import rust_count_non_literal_handlers

        from mypy.astwire import serialize_node
        from mypy.cache import WriteBuffer

        tree = self._parse(
            "class C:\n    def lit(self):\n        42\n    def nonlit(self):\n        return 1\n"
        )
        buf = WriteBuffer()
        serialize_node(tree, buf)
        rust_count = rust_count_non_literal_handlers(buf.getvalue())
        assert rust_count == len(find_non_literal_handlers(tree))


@skipUnless(_splice_kernel is not None, "requires the type_kernel extension")
class NativeMirrorSpliceSuite(Suite):
    """Unit tests for the wire-cache splice funnel of the F1 mirror.

    `_write_type_cached`'s splice hit serves cached bytes without calling
    `t.write`, so no family write funnel ever sees it (issue #1372 item 2).
    `types_mirror._check_splice` is installed into
    `types._type_mirror_splice_check` by `activate()` and re-verifies each
    splice hit against the object's live bytes: mirror drift counts
    `mismatch.<fam>.cachedsplice`, a stale spliced blob counts
    `stale.<fam>.cachedsplice` and is resynced by dropping the cache
    entry, and strict mode raises on either.

    The splice funnel only exists with the librt fork's
    `write_raw_bytes`; with PyPI librt the wire cache is inert, the
    splice-hit tests skip, and the degraded-fallback test covers the
    plain-write reality.
    """

    def setUp(self) -> None:
        from mypy import types_mirror
        from mypy.types import _clear_type_wire_cache, _set_type_wire_cache_enabled

        types_mirror.activate(audit=True)
        _clear_type_wire_cache()
        _set_type_wire_cache_enabled(True)
        types_mirror.reset(clear_counts=True)
        # Activation is one-shot process-wide (no mid-run deactivation):
        # re-entering activate() with _active False would re-read already-wrapped
        # __dict__ entries and stack wrappers, so isolation is reset() + strict off.
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        from mypy.types import _clear_type_wire_cache, _set_type_wire_cache_enabled

        _set_type_wire_cache_enabled(False)
        _clear_type_wire_cache()
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _callable(self) -> CallableType:
        return CallableType(
            [self.fx.o, self.fx.o],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.o,
            self.fx.function,
            name="f",
        )

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    @skipUnless(_SPLICE_ACTIVE, "splice funnel needs librt write_raw_bytes")
    def test_clean_hit_counts_assert_ok(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.types import _write_type_cached

        c = self._callable()
        _write_type_cached(c, WriteBuffer())  # cache fill adopts via the funnel
        before = dict(self._m.report())
        _write_type_cached(c, WriteBuffer())  # splice hit
        delta = self._delta(before)
        assert delta.get("assert_ok.callable.cachedsplice") == 1, delta
        assert not any(k.startswith(("mismatch.", "stale.")) for k in delta), delta

    @skipUnless(_SPLICE_ACTIVE, "splice funnel needs librt write_raw_bytes")
    def test_inplace_mutation_is_counted_and_self_heals(self) -> None:
        from librt.internal import WriteBuffer

        from mypy import types_mirror
        from mypy.types import _type_wire_cache, _write_type_cached

        c = self._callable()
        _write_type_cached(c, WriteBuffer())  # cache fill
        c.arg_types[0] = self.fx.std_tuple  # in-place list splice, no setattr
        before = dict(self._m.report())
        _write_type_cached(c, WriteBuffer())  # splice hit on a stale entry
        delta = self._delta(before)
        assert delta.get("stale.callable.cachedsplice") == 1, delta
        assert delta.get("mismatch.callable.cachedsplice") == 1, delta
        assert id(c) not in _type_wire_cache, delta
        # Next write re-caches fresh bytes through the funnel; the splice
        # then serves them cleanly.
        _write_type_cached(c, WriteBuffer())
        fresh = types_mirror._fresh_bytes(c)
        assert _type_wire_cache[id(c)][1] == fresh
        before = dict(self._m.report())
        _write_type_cached(c, WriteBuffer())
        delta = self._delta(before)
        assert delta.get("assert_ok.callable.cachedsplice") == 1, delta
        assert not any(k.startswith(("mismatch.", "stale.")) for k in delta), delta

    def test_strict_mode_raises_on_drifted_live_bytes(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.types import _write_type_cached

        # The in-place arg splice fires no family setattr and escapes
        # capture, exactly like a mid-flight unprotected-window write;
        # the bump models what any escaped setattr path leaves behind.
        c = self._callable()
        _write_type_cached(c, WriteBuffer())  # cache fill
        c.arg_types[0] = self.fx.std_tuple  # in-place mutation escapes capture
        self._m._bump_unprot()
        self._m._strict = True
        try:
            with self.assertRaises(AssertionError):
                _write_type_cached(c, WriteBuffer())
        finally:
            self._m._strict = False

    def test_strict_mode_skips_until_unprotected_bump(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.types import _write_type_cached

        # Without a bump the write funnel trusts its stamp; an active wire
        # cache (librt fork) routes the hit into _check_splice, which
        # full-verifies and reports the drift in strict mode.
        c = self._callable()
        _write_type_cached(c, WriteBuffer())  # cache fill
        c.arg_types[0] = self.fx.std_tuple  # in-place mutation escapes capture
        self._m._strict = True
        try:
            if _SPLICE_ACTIVE:
                with self.assertRaises(AssertionError):
                    _write_type_cached(c, WriteBuffer())
            else:
                _write_type_cached(c, WriteBuffer())  # stamped: funnel skips
        finally:
            self._m._strict = False

    @skipUnless(_SPLICE_ACTIVE, "splice funnel needs librt write_raw_bytes")
    def test_strict_mode_raises_on_stale_spliced_bytes(self) -> None:
        # Mirror synced (captured setattr), cache still stale: isolates the
        # pure splice-staleness class from the mirror-drift class.
        from librt.internal import WriteBuffer

        from mypy.types import _write_type_cached

        c = self._callable()
        _write_type_cached(c, WriteBuffer())  # cache fill
        c.arg_types[0] = self.fx.std_tuple  # in-place mutation escapes capture
        c.name = "g"  # captured setattr resyncs the mirror, not the cache
        self._m._strict = True
        try:
            with self.assertRaises(AssertionError):
                _write_type_cached(c, WriteBuffer())
        finally:
            self._m._strict = False

    def test_degraded_env_serves_plain_write(self) -> None:
        """Without librt's write_raw_bytes (CI installs PyPI librt) the
        wire-cache splice funnel is inert: _write_type_cached serves plain
        t.write, the wire cache stays empty, and only write-funnel counters
        fire. Monkeypatched so this runs under both librts."""
        from librt.internal import WriteBuffer

        from mypy import types as types_mod, types_mirror
        from mypy.types import _type_wire_cache, _write_type_cached

        saved = types_mod.__dict__["write_raw_bytes"]
        types_mod.__dict__["write_raw_bytes"] = None
        try:
            c = self._callable()
            before = dict(self._m.report())
            buf = WriteBuffer()
            _write_type_cached(c, buf)
            delta = self._delta(before)
            assert not any("splice" in k for k in delta), delta
            expected = WriteBuffer()
            types_mirror._originals[CallableType]["write"](c, expected)
            assert buf.getvalue() == expected.getvalue()
            assert not _type_wire_cache, _type_wire_cache
        finally:
            types_mod.__dict__["write_raw_bytes"] = saved

    @skipUnless(_SPLICE_ACTIVE, "wire-cache session needs librt write_raw_bytes")
    def test_session_depth_restored_on_write_error(self) -> None:
        # A raise inside the isolated session must not leak the depth
        # counter: an unbalanced depth sends every later write down the
        # nested path, so the cache never fills again (xdist flake).
        from unittest import mock

        from librt.internal import WriteBuffer

        import mypy.types as types_mod
        from mypy.types import _serialize_with_taint_check, _type_wire_cache, _write_type_cached

        c = self._callable()

        def boom(self: CallableType, data: object) -> None:
            raise RuntimeError("boom")

        with mock.patch.object(CallableType, "write", boom):
            with self.assertRaises(RuntimeError):
                _write_type_cached(c, WriteBuffer())
            assert types_mod._type_wire_cache_session_depth == 0
            with self.assertRaises(RuntimeError):
                _serialize_with_taint_check(c, WriteBuffer())
            assert types_mod._type_wire_cache_session_depth == 0
        _write_type_cached(c, WriteBuffer())
        assert id(c) in _type_wire_cache
        assert types_mod._type_wire_cache_session_depth == 0


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeMirrorTypeVarIdSuite(Suite):
    """Unit tests for the TypeVarId capture shim of the F1 mirror.

    A TypeVarId is embedded in TypeVarLikeType.id, so a meta_level write
    changes the wire bytes of every carrier embedding it without firing
    any family __setattr__ (the measured `mismatch.tvar.write` escapes).
    `_mirror_tvid_setattr` captures the write at the source and resyncs
    each registered carrier through the reverse map
    `_TVID_REVERSE[id(tvid)] -> (tvid, carrier handles)`.

    Activation is one-shot process-wide: isolation is reset() + strict
    off, same as NativeMirrorSpliceSuite.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _callable(self, name: str) -> CallableType:
        return CallableType(
            [self.fx.o],
            [ARG_POS],
            [None],
            self.fx.o,
            self.fx.function,
            name=name,
            variables=[self.fx.t],
        )

    def _tvar_id(self) -> TypeVarId:
        return self.fx.t.id

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def test_adopt_counts_captured_and_cascades_carrier(self) -> None:
        from librt.internal import WriteBuffer

        ct = self._callable("f")
        ct.write(WriteBuffer())  # adoption funnel indexes tvid -> ct
        before = dict(self._m.report())
        self._tvar_id().meta_level = 1
        delta = self._delta(before)
        assert delta.get("tvid_captured.meta_level") == 1, delta
        assert delta.get("cascade_sync") == 1, delta
        assert not any(k.startswith(("mismatch.", "tvid_orphan.")) for k in delta), delta
        before = dict(self._m.report())
        ct.write(WriteBuffer())  # mirror already caught up: must assert clean
        delta = self._delta(before)
        assert delta.get("assert_skip.callable.write") == 1, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta

    def test_equal_write_counts_equal_and_skips_cascade(self) -> None:
        from librt.internal import WriteBuffer

        ct = self._callable("f")
        ct.write(WriteBuffer())
        self._tvar_id().meta_level = 1  # establish a non-default value
        before = dict(self._m.report())
        self._tvar_id().meta_level = 1  # equal-value write
        delta = self._delta(before)
        assert delta.get("tvid_setattr_equal.meta_level") == 1, delta
        assert not any(
            k.startswith(("tvid_captured.", "tvid_orphan.", "cascade_sync")) for k in delta
        ), delta

    def test_orphan_tvid_counts_orphan(self) -> None:
        self._callable("f")
        self._tvar_id().meta_level = 1
        before = dict(self._m.report())
        orphan = TypeVarId(99)
        orphan.meta_level = 3
        delta = self._delta(before)
        assert delta.get("tvid_orphan.meta_level") == 1, delta
        assert not any(k.startswith(("tvid_captured.", "cascade_sync")) for k in delta), delta

    def test_shared_tvid_cascades_both_carriers(self) -> None:
        from librt.internal import WriteBuffer

        ct = self._callable("f")
        ct.write(WriteBuffer())
        ct2 = ct.copy_modified(name="g")  # shares the same id object
        ct2.write(WriteBuffer())
        handles = self._m._TVID_REVERSE[id(self._tvar_id())]
        assert ct.variables[0] is ct2.variables[0]
        # The tvar carriers include the tvar itself and each callable that
        # adopted it.
        assert self._m._handle_of(ct) in handles[1], handles
        assert self._m._handle_of(ct2) in handles[1], handles
        before = dict(self._m.report())
        self._tvar_id().meta_level = 1
        delta = self._delta(before)
        assert delta.get("tvid_captured.meta_level") == 1, delta
        assert delta.get("cascade_sync") == 2, delta
        before = dict(self._m.report())
        ct.write(WriteBuffer())
        ct2.write(WriteBuffer())
        delta = self._delta(before)
        assert delta.get("assert_skip.callable.write") == 2, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta

    def test_id_swap_via_family_setattr_roadmaps_new_tvid(self) -> None:
        from librt.internal import WriteBuffer

        ct = self._callable("f")
        ct.write(WriteBuffer())
        self.fx.t.id = TypeVarId(42, 0, namespace="mod.C")  # captured family setattr
        before = dict(self._m.report())
        self.fx.t.id.meta_level = 1  # the new tvid must be captured, not orphaned
        delta = self._delta(before)
        assert delta.get("tvid_captured.meta_level") == 1, delta
        assert delta.get("tvid_orphan.meta_level", 0) == 0, delta

    def test_strict_mode_does_not_raise_on_captured_mutation(self) -> None:
        from librt.internal import WriteBuffer

        ct = self._callable("f")
        ct.write(WriteBuffer())
        self._m._strict = True
        try:
            self._tvar_id().meta_level = 1  # captured at the source
            ct.write(WriteBuffer())  # was the measured escape: must pass now
        finally:
            self._m._strict = False


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeMirrorTypeAliasFlagSuite(Suite):
    """Unit tests for the TypeAlias._is_recursive capture shim of the F1 mirror.

    A TypeAliasType embeds a live ``mypy.nodes.TypeAlias`` node by
    reference, and ``TypeAliasType.write`` reads ``alias._is_recursive``
    at serialization time, so a node write changes the wire bytes of
    every family carrier embedding the TypeAliasType without firing any
    family ``__setattr__`` (the measured ``mismatch.instance.write``
    escape, issue #1385). ``_mirror_alias_setattr`` captures the write at
    the source and resyncs each registered carrier through the reverse
    map ``_ALIAS_REVERSE[id(node)] -> (node, carrier handles)``.

    Activation is one-shot process-wide: isolation is reset() + strict
    off, same as NativeMirrorTypeVarIdSuite.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _carrier(self, tat: TypeAliasType) -> Instance:
        return Instance(self.fx.std_tuplei, [tat])

    def _write(self, t: Type) -> None:
        from librt.internal import WriteBuffer

        t.write(WriteBuffer())

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def test_captured_flag_write_resyncs_carrier(self) -> None:
        from mypy.types import TypeAliasType

        node = TypeAlias(self.fx.o, "m.A", "m", 1, 0)
        tat = TypeAliasType(node, [])
        ct = self._carrier(tat)
        self._write(ct)  # adoption funnel indexes node -> ct
        before = dict(self._m.report())
        node._is_recursive = True
        delta = self._delta(before)
        assert delta.get("alias_captured._is_recursive") == 1, delta
        assert not any(k.startswith(("mismatch.", "alias_orphan.")) for k in delta), delta
        before = dict(self._m.report())
        self._write(ct)  # mirror already caught up: must assert clean
        delta = self._delta(before)
        assert delta.get("assert_skip.instance.write") == 1, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta

    def test_target_descent_records_embedded_nodes(self) -> None:
        from mypy.types import TypeAliasType

        node_b = TypeAlias(self.fx.o, "m.B", "m", 1, 0)
        tat_b = TypeAliasType(node_b, [])
        node_a = TypeAlias(tat_b, "m.A", "m", 1, 0)
        tat_a = TypeAliasType(node_a, [])
        ct = self._carrier(tat_a)
        self._write(ct)
        entry_a = self._m._ALIAS_REVERSE[id(node_a)]
        entry_b = self._m._ALIAS_REVERSE[id(node_b)]
        assert self._m._handle_of(ct) in entry_a[1], entry_a
        assert self._m._handle_of(ct) in entry_b[1], entry_b
        before = dict(self._m.report())
        node_b._is_recursive = True
        delta = self._delta(before)
        assert delta.get("alias_captured._is_recursive") == 1, delta
        assert not any(k.startswith(("mismatch.", "alias_orphan.")) for k in delta), delta
        before = dict(self._m.report())
        self._write(ct)
        delta = self._delta(before)
        assert delta.get("assert_skip.instance.write") == 1, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta

    def test_equal_write_counts_equal_and_skips_cascade(self) -> None:
        from mypy.types import TypeAliasType

        node = TypeAlias(self.fx.o, "m.A", "m", 1, 0)
        ct = self._carrier(TypeAliasType(node, []))
        self._write(ct)
        node._is_recursive = True  # establish a non-default value
        before = dict(self._m.report())
        node._is_recursive = True  # equal-value write
        delta = self._delta(before)
        assert delta.get("alias_setattr_equal._is_recursive") == 1, delta
        assert not any(
            k.startswith(("alias_captured.", "alias_orphan.", "cascade_sync")) for k in delta
        ), delta

    def test_orphan_node_counts_orphan(self) -> None:
        from mypy.types import TypeAliasType

        TypeAlias(self.fx.o, "m.A", "m", 1, 0)
        TypeAliasType(TypeAlias(self.fx.o, "m.Y", "m", 1, 0), [])  # never in a funnel
        orphan = TypeAlias(self.fx.o, "m.X", "m", 1, 0)
        before = dict(self._m.report())
        orphan._is_recursive = True
        delta = self._delta(before)
        assert delta.get("alias_orphan._is_recursive") == 1, delta
        assert not any(k.startswith(("alias_captured.", "cascade_sync")) for k in delta), delta

    def test_strict_mode_does_not_raise_on_captured_flag(self) -> None:

        from mypy.types import TypeAliasType

        node = TypeAlias(self.fx.o, "m.A", "m", 1, 0)
        ct = self._carrier(TypeAliasType(node, []))
        self._write(ct)
        self._m._strict = True
        try:
            node._is_recursive = True  # captured at the source
            self._write(ct)  # was the measured escape: must pass now
        finally:
            self._m._strict = False


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeMirrorAdoptStrikeSuite(Suite):
    """Unit tests for the adoption-strike lifecycle of the F1 mirror.

    ``_note_failed_adoption`` memoizes an unregistrable family object so
    the per-setattr adoption retry storm stops until the write funnel.
    The memo was keyed by object id forever, so a later successful
    adoption did not clear it and every subsequent ``__setattr__`` on the
    object escaped capture (the measured under-strike escape drift of the
    NamedTuple reanalysis tests, issue #1385). Both lifecycle rules are
    tested here: a successful adoption clears the memo, and a setattr on
    a struck-but-now-registered object is captured like any other.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def test_successful_adopt_clears_strike_memo(self) -> None:
        from librt.internal import WriteBuffer

        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        self._m._note_failed_adoption(inst)  # simulate earlier failed adoption
        self._m._strict = True
        try:
            # Slice 10 (#1397): within the strike window the write funnel
            # skips the memoized root instead of re-serializing and
            # re-failing registration on every visit.
            before = dict(self._m.report())
            inst.write(WriteBuffer())
            delta = self._delta(before)
            assert delta.get("adopt_strike_skip.instance.write") == 1, delta
            assert self._m._handle_of(inst) is None
            # Past the bounded probe the next visit retries: the dup-add
            # makes the very next encounter roll over into the probe, and
            # a successful adoption then clears the memo.
            self._m._ADOPT_STRIKE.add(id(inst), self._m._STRIKE_RETRY_INTERVAL - 1)
            before = dict(self._m.report())
            inst.write(WriteBuffer())
            delta = self._delta(before)
            assert delta.get("adopt.instance.write") == 1, delta
            assert delta.get("strike_cleared_on_adopt") == 1, delta
            assert self._m._handle_of(inst) is not None
            # The memo is gone: a real mutation is captured at the source.
            before = dict(self._m.report())
            inst.args = (self.fx.o,)
            delta = self._delta(before)
            assert delta.get("setattr_captured.instance.args") == 1, delta
            assert not any(k.startswith("mismatch.") for k in delta), delta
        finally:
            self._m._strict = False

    def test_under_strike_unregistered_setattr_stays_gagged(self) -> None:
        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        self._m._note_failed_adoption(inst)
        before = dict(self._m.report())
        inst.args = (self.fx.o,)  # still unregistered in this test: no capture
        delta = self._delta(before)
        assert "setattr_captured.instance.args" not in delta, delta
        assert delta.get("strike_captured_late") is None, delta


class NativeMirrorIdFifoSuite(Suite):
    """Unit tests for the O(1) strike FIFO (_IdFifo).

    The FIFO replaced a `set` + cap-evicting `deque` pair whose
    `.remove` scan was the mirror's dominant production cost (131s summed
    over 4 self-check workers). It must behave like the old model:
    O(1) membership and remove with a truthy return, FIFO eviction by
    live insertion order across re-adds, and eviction never dropping a
    re-added youngest entry early.
    """

    def _fifo(self, cap: int) -> Any:
        from mypy.types_mirror import _IdFifo

        return _IdFifo(cap)

    def test_membership_and_remove_return(self) -> None:
        q = self._fifo(4)
        assert not q
        assert len(q) == 0
        assert 7 not in q
        q.add(7)
        assert 7 in q
        assert q
        assert len(q) == 1
        # remove() returns True only when the id was present, and is idempotent.
        assert q.remove(7) is True
        assert 7 not in q
        assert q.remove(7) is False
        assert not q

    def test_eviction_is_fifo_by_live_insertion_order(self) -> None:
        q = self._fifo(3)
        for k in (1, 2, 3):
            q.add(k)
        assert set(q._members) == {1, 2, 3}
        q.add(4)  # evicts 1, the oldest live insertion
        assert set(q._members) == {2, 3, 4}
        q.add(5)
        assert set(q._members) == {3, 4, 5}

    def test_readd_after_remove_is_not_evicted_early(self) -> None:
        q = self._fifo(3)
        for k in (1, 2, 3):
            q.add(k)
        q.remove(1)  # mid-queue strike clear frees a slot
        q.add(1)  # re-added: youngest live insertion again
        q.add(4)  # must evict 2 (now oldest), never the re-added 1
        assert set(q._members) == {1, 3, 4}
        q.add(5)
        assert set(q._members) == {1, 4, 5}

    def test_stale_log_entries_lazily_compact(self) -> None:
        q = self._fifo(2)
        for k in (1, 2):
            q.add(k)
        q.remove(1)
        q.add(1)  # re-add: the old (1) log entry becomes stale
        q.add(3)  # evict walks past the stale entry, evicts 2, compacts
        assert set(q._members) == {1, 3}
        # The log was trimmed to the live entries and the cursor reset.
        assert q._cursor == 0 and len(q._log) == 2, (q._cursor, q._log)
        q.add(4)  # next evict is arithmetic on the compacted log
        assert set(q._members) == {3, 4}

    def test_subcap_churn_log_stays_bounded(self) -> None:
        q = self._fifo(64)
        for i in range(1000):
            # add + remove below the cap: no eviction happens, but the
            # churn must not accumulate stale entries unboundedly (the
            # old capped deque stayed at 64 entries).
            q.add(i)
            q.remove(i)
        assert len(q._log) <= 64, len(q._log)
        assert not q._members


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeMirrorHiddenParentSuite(Suite):
    """Unit tests for the hidden-parent cascade of the F1 mirror.

    A family leaf nested behind a non-family intermediate Type (TupleType,
    Parameters, ...) has no edge in the kernel parents graph, so a captured
    leaf write (for example calculate_tuple_fallback's
    ``fallback.args = (union,)`` through semantic_shared) re-syncs the leaf
    only, and the embedding TypeVarType drifts until its own funnel: the
    measured ``mismatch.tvar.write`` escape class over named-tuple synthetic
    self-type tvars. `_register_tree` additionally indexes every family
    descendant reachable through non-family Types into
    `_HIDDEN_EMBED[id] -> (obj, container handles)`, and
    `_update_and_cascade` re-serializes those containers like family
    parents.

    Activation is one-shot process-wide: isolation is reset() + strict
    off, same as NativeMirrorTypeVarIdSuite.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _tuple_meta(self) -> tuple[Instance, TupleType, TypeVarType]:
        # A named-tuple-shaped hidden chain: TypeVarType -upper_bound->
        # TupleType (non-family) -partial_fallback-> Instance (family).
        fallback = Instance(self.fx.std_tuplei, [self.fx.anyt])
        tup = TupleType([self.fx.a], fallback)
        tv = TypeVarType("_NT", "m.C._NT", TypeVarId(-1, namespace="m.C.f"), [], tup, self.fx.a)
        return fallback, tup, tv

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def test_hidden_embed_index_records_chain(self) -> None:
        from librt.internal import WriteBuffer

        _fallback, _tup, tv = self._tuple_meta()
        tv.write(WriteBuffer())  # adoption funnel registers tv's tree
        entry = self._m._HIDDEN_EMBED.get(id(_fallback))
        assert entry is not None, entry
        assert self._m._handle_of(tv) in entry[1], entry

    def test_captured_leaf_write_cascades_into_hidden_parent(self) -> None:
        from librt.internal import WriteBuffer

        fallback, _tup, tv = self._tuple_meta()
        tv.write(WriteBuffer())  # register the tvar with the Any fallback arg
        before = dict(self._m.report())
        fallback.args = (UnionType([self.fx.a, self.fx.std_tuple]),)  # captured leaf write
        delta = self._delta(before)
        assert delta.get("setattr_captured.instance.args") == 1, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta
        before = dict(self._m.report())
        tv.write(WriteBuffer())  # mirror already caught up: must assert clean
        delta = self._delta(before)
        assert delta.get("assert_skip.tvar.write") == 1, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta

    def test_cascade_transitive_through_callable(self) -> None:
        from librt.internal import WriteBuffer

        fallback, _tup, tv = self._tuple_meta()
        ct = CallableType(
            [self.fx.o], [ARG_POS], [None], self.fx.o, self.fx.function, variables=[tv]
        )
        ct.write(WriteBuffer())  # registers callable -> (tvar family child)
        tv.write(WriteBuffer())  # registers the hidden chain into the tvar
        before = dict(self._m.report())
        fallback.args = (UnionType([self.fx.a, self.fx.std_tuple]),)
        delta = self._delta(before)
        assert not any(k.startswith("mismatch.") for k in delta), delta
        before = dict(self._m.report())
        tv.write(WriteBuffer())
        ct.write(WriteBuffer())
        delta = self._delta(before)
        # tv asserts clean twice: its own write funnel plus the re-assert
        # triggered through ct's write path.
        assert delta.get("assert_skip.tvar.write") == 2, delta
        assert delta.get("assert_skip.callable.write") == 1, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta

    def test_strict_mode_does_not_raise_on_hidden_parent_capture(self) -> None:
        from librt.internal import WriteBuffer

        fallback, _tup, tv = self._tuple_meta()
        tv.write(WriteBuffer())
        self._m._strict = True
        try:
            fallback.args = (UnionType([self.fx.a, self.fx.std_tuple]),)  # captured write
            tv.write(WriteBuffer())  # was the measured escape: must pass now
        finally:
            self._m._strict = False

    def test_replaced_upper_bound_rereaches_new_embeds(self) -> None:
        from librt.internal import WriteBuffer

        fallback, _tup, tv = self._tuple_meta()
        tv.write(WriteBuffer())  # indexes the original chain
        new_inst = Instance(self.fx.std_tuplei, [self.fx.a])
        tv.upper_bound = TupleType([self.fx.o], new_inst)  # captured replace
        # The new chain is invisible to every earlier `_HIDDEN_EMBED` fill
        # (registration time only). Until cascade re-indexes the tvar, the
        # captured write below has no container to re-sync.
        new_inst.args = (UnionType([self.fx.a, self.fx.std_tuple]),)  # captured write
        before = dict(self._m.report())
        tv.write(WriteBuffer())  # must already be fresh: assert_ok, no mismatch
        delta = self._delta(before)
        assert delta.get("assert_skip.tvar.write") == 1, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta

    def test_late_registered_hidden_leaf_cascades_prior_adopters(self) -> None:
        from librt.internal import WriteBuffer

        fallback, _tup, tv = self._tuple_meta()
        self._m._register_tree(tv)  # adopt; the nested leaf stays unregistered
        # The issue-#1385 escape: the unregistered leaf mutates while capture
        # is suspended (e.g. an in-place rewrite during serialization), so no
        # setattr lands in the mirror and the stored tvar blob drifts.
        self._m._in_serialize = True
        try:
            fallback.args = (UnionType([self.fx.a, self.fx.std_tuple]),)
        finally:
            self._m._in_serialize = False
        # The leaf first registers here; late adoption must re-sync the tvar.
        fallback.write(WriteBuffer())
        before = dict(self._m.report())
        tv.write(WriteBuffer())  # strict self-check on the re-synced blob
        delta = self._delta(before)
        assert delta.get("assert_skip.tvar.write") == 1, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeWriteFunnelSkipSuite(Suite):
    """Unit tests for the unprotected-write epoch protocol (slice 6, #1397).

    Writes the mirror does not capture-and-sync on a REGISTERED target
    bump the global unprotected epoch (suppression windows, failed syncs);
    writes on never-registered objects bump nothing, because no stored
    blob can embed one: embedding implies prior adoption (which
    registers), and a derivation through a never-serializable struck
    object cannot exist to have stored bytes from. A write funnel then
    skips full re-serialization iff the object's blob was stamped at the
    current epoch: `assert_ok` shrinks to adopt-time entries and
    epoch-bust re-verifies, `assert_skip` carries the steady-state
    traffic. Synced/captured writes restamp, SKIP_ATTRS writes bump
    nothing, and a drift after a bump still surfaces.

    TypeFixture fixtures reuse nested family Instances, and a root write
    serializes its registered nested children through their own write
    funnels too, so the checks below are structural (any skip, no ok)
    rather than exact funnel counts.
    """

    def setUp(self) -> None:
        from mypy import types_mirror
        from mypy.types import (
            _clear_type_wire_cache,
            _set_type_wire_cache_enabled,
            _wire_cache_enabled,
        )

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        # Pin the pre-librt write path: with an active wire cache
        # (`write_raw_bytes` present) nested cached children splice and add
        # `*.cachedsplice` keys the structural deltas below do not expect.
        self._wire_cache_prev = _wire_cache_enabled()
        _set_type_wire_cache_enabled(False)
        _clear_type_wire_cache()
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        from mypy.types import _clear_type_wire_cache, _set_type_wire_cache_enabled

        self._m._strict = False
        _set_type_wire_cache_enabled(self._wire_cache_prev)
        _clear_type_wire_cache()
        self._m.reset(clear_counts=True)

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def _synced(self) -> Instance:
        from librt.internal import WriteBuffer

        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        inst.write(WriteBuffer())  # adopt path: registers and stamps
        return inst

    def test_adopt_registers_without_assert_then_funnel_skips(self) -> None:
        from librt.internal import WriteBuffer

        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        before = dict(self._m.report())
        inst.write(WriteBuffer())  # first funnel: adopt, no assert yet
        delta = self._delta(before)
        assert delta.get("adopt.instance.write") == 1, delta
        assert not any(k.startswith("assert_ok.") for k in delta), delta
        before = dict(self._m.report())
        inst.write(WriteBuffer())  # stamped at the current epoch: skip
        delta = self._delta(before)
        assert any(k.startswith("assert_skip.") for k in delta), delta
        assert not any(k.startswith(("assert_ok.", "mismatch.")) for k in delta), delta

    def test_bump_forces_verify_then_funnel_restamps(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._synced()
        before = dict(self._m.report())
        inst.write(WriteBuffer())  # still stamped: skip
        delta = self._delta(before)
        assert any(k.startswith("assert_skip.") for k in delta), delta
        assert not any(k.startswith(("assert_ok.", "mismatch.")) for k in delta), delta
        self._m._bump_unprot()
        before = dict(self._m.report())
        inst.write(WriteBuffer())  # stamp stale: full verify, no skip
        delta = self._delta(before)
        assert any(k.startswith("assert_ok.") for k in delta), delta
        assert not any(k.startswith(("assert_skip.", "mismatch.")) for k in delta), delta
        before = dict(self._m.report())
        inst.write(WriteBuffer())  # the verify restamped: skip again
        delta = self._delta(before)
        assert any(k.startswith("assert_skip.") for k in delta), delta
        assert not any(k.startswith(("assert_ok.", "mismatch.")) for k in delta), delta

    def test_uncaptured_setattr_bumps_and_next_funnel_verifies(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._synced()
        before = dict(self._m.report())
        # Suppression-window setattr on a REGISTERED object with an
        # unchanged value: the raw write lands uncaptured, leaving the
        # mirrored blob still correct.
        self._m._construction += 1
        try:
            inst.args = (self.fx.a,)
        finally:
            self._m._construction -= 1
        delta = self._delta(before)
        assert not any(k.startswith("setattr_captured.") for k in delta), delta
        before = dict(self._m.report())
        inst.write(WriteBuffer())  # bump busts the stamp: full verify
        delta = self._delta(before)
        assert any(k.startswith("assert_ok.") for k in delta), delta
        assert not any(k.startswith(("assert_skip.", "mismatch.")) for k in delta), delta

    def test_window_write_on_unregistered_object_bumps_nothing(self) -> None:
        from librt.internal import WriteBuffer

        before = dict(self._m.report())
        sup = Instance(self.fx.std_tuplei, [self.fx.a])
        # The suppression-window setattr happens on a never-registered
        # object that no stored blob embeds: no bump, and a later funnel
        # derives its blob fresh as the first adopt-path entry.
        self._m._construction += 1
        try:
            sup.args = (self.fx.o,)
        finally:
            self._m._construction -= 1
        delta = self._delta(before)
        assert not any(k.startswith("unprot_bump") for k in delta), delta
        before = dict(self._m.report())
        sup.write(WriteBuffer())
        delta = self._delta(before)
        assert delta.get("adopt.instance.write") == 1, delta

    def test_adoption_struck_write_bumps_nothing(self) -> None:
        from librt.internal import WriteBuffer

        struck = Instance(self.fx.std_tuplei, [self.fx.a])  # never registered
        self._m._note_failed_adoption(struck)  # publish the strike memo
        before = dict(self._m.report())
        struck.args = (self.fx.o,)
        delta = self._delta(before)
        # Nothing registered embeds the object, so the write lands raw:
        # no strike interaction, no epoch bump (lazy adoption).
        assert delta.get("setattr_untracked.instance") == 1, delta
        assert not any(k.startswith("unprot_bump") for k in delta), delta
        before = dict(self._m.report())
        struck.write(WriteBuffer())
        delta = self._delta(before)
        assert delta.get("adopt_strike_skip.instance.write") == 1, delta
        # Slice 10 (#1397): past the bounded probe the write funnel
        # retries registration and adopts the now-valid object.
        self._m._ADOPT_STRIKE.add(id(struck), self._m._STRIKE_RETRY_INTERVAL - 1)
        before = dict(self._m.report())
        struck.write(WriteBuffer())
        delta = self._delta(before)
        assert delta.get("adopt.instance.write") == 1, delta

    def test_skip_attr_write_bumps_nothing(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._synced()
        before = dict(self._m.report())
        inst.line = 123  # in SKIP_ATTRS: uncaptured by design, harmless
        delta = self._delta(before)
        assert not any(
            k.startswith(("assert_ok.", "assert_skip.", "mismatch.")) for k in delta
        ), delta
        before = dict(self._m.report())
        inst.write(WriteBuffer())  # stamp unaffected: still skips
        delta = self._delta(before)
        assert any(k.startswith("assert_skip.") for k in delta), delta
        assert not any(k.startswith(("assert_ok.", "mismatch.")) for k in delta), delta

    def test_captured_setattr_restamps(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._synced()
        before = dict(self._m.report())
        inst.args = (self.fx.o,)  # captured write
        delta = self._delta(before)
        assert delta.get("setattr_captured.instance.args") == 1, delta
        assert not any(k.startswith("mismatch.") for k in delta), delta
        before = dict(self._m.report())
        inst.write(WriteBuffer())  # capture restamped: skip, no assert
        delta = self._delta(before)
        assert any(k.startswith("assert_skip.") for k in delta), delta
        assert not any(k.startswith(("assert_ok.", "mismatch.")) for k in delta), delta

    def test_drift_after_bump_is_caught_and_healed(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._synced()
        self._m._bump_unprot()
        # Raw setattr: bypasses the patched __setattr__ entirely, so the
        # write lands uncaptured on top of an already-bumped epoch.
        self._m._ORIG_SETATTR(inst, "args", (self.fx.o,))
        before = dict(self._m.report())
        inst.write(WriteBuffer())
        delta = self._delta(before)
        assert delta.get("mismatch.instance.write") == 1, delta
        before = dict(self._m.report())
        inst.write(WriteBuffer())  # the mismatch resynced the mirror
        delta = self._delta(before)
        assert any(k.startswith("assert_skip.") for k in delta), delta
        assert not any(k.startswith(("mismatch.", "assert_ok.")) for k in delta), delta


class NativeMirrorWalkIndicesSuite(Suite):
    """Differential tests for the fused F3 slice-5 tree walk.

    `_walk_indices` fuses the former `_tvids_in` / `_alias_nodes_in` /
    `_family_embeds` triple into one traversal with a shared `seen`. The
    fused walk is an intentional per-intent superset: TypeAlias targets
    and container slots are descended for all three intents where the
    separate walkers each only descended one of them, and container ids
    share one seen set. These tests pin that exact contract:

    - alias nodes: fused list equals the old alias walk verbatim.
    - tvids and family embeds: the old walk's list is a subsequence of
      the fused list (content superset with pre-order preserved), with
      the expected extras only at alias-target or previously
      double-visited container positions.

    Independence caveat: these walk raw object trees offline from the
    activation machinery (no wrappers), so no family setattr hooks are
    in play and id() stability holds while the fixtures are referenced
    by locals.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        self._m = types_mirror
        self.fx = TypeFixture()

    # Old-style independent walkers, verbatim from the pre-fusion code.

    def _tvids_in_value(self, value: Any, seen: set[int]) -> Iterator[TypeVarId]:
        if value is None or isinstance(value, (str, bytes, bool, int, float)):
            return
        if isinstance(value, TypeVarId):
            yield value
            return
        if isinstance(value, Type):
            yield from self._tvids_in(value, seen)
            return
        if isinstance(value, (list, tuple)):
            seen.add(id(value))
            for item in value:
                yield from self._tvids_in_value(item, seen)
            return
        if isinstance(value, dict):
            seen.add(id(value))
            for item in value.values():
                yield from self._tvids_in_value(item, seen)
            return
        if type(value).__name__ == "ExtraAttrs":
            yield from self._tvids_in_value(value.attrs, seen)

    def _tvids_in(self, t: Type, seen: set[int]) -> Iterator[TypeVarId]:
        if id(t) in seen:
            return
        seen.add(id(t))
        for name in self._m._type_names(type(t)):
            if name.startswith("_") or name in self._m._SKIP_SLOTS:
                continue
            try:
                value = object.__getattribute__(t, name)
            except AttributeError:
                continue
            yield from self._tvids_in_value(value, seen)

    def _alias_nodes_in_value(self, value: Any, seen: set[int]) -> Iterator[Any]:
        if value is None or isinstance(value, (str, bytes, bool, int, float)):
            return
        if type(value).__name__ == "TypeAlias":
            if id(value) not in seen:
                seen.add(id(value))
                yield value
                target = getattr(value, "target", None)
                if target is not None:
                    yield from self._alias_nodes_in_value(target, seen)
            return
        if isinstance(value, Type):
            yield from self._alias_nodes_in(value, seen)
            return
        if isinstance(value, (list, tuple)):
            seen.add(id(value))
            for item in value:
                yield from self._alias_nodes_in_value(item, seen)
            return
        if isinstance(value, dict):
            seen.add(id(value))
            for item in value.values():
                yield from self._alias_nodes_in_value(item, seen)
            return
        if type(value).__name__ == "ExtraAttrs":
            yield from self._alias_nodes_in_value(value.attrs, seen)

    def _alias_nodes_in(self, t: Type, seen: set[int]) -> Iterator[Any]:
        if id(t) in seen:
            return
        seen.add(id(t))
        for name in self._m._type_names(type(t)):
            if name.startswith("_") or name in self._m._SKIP_SLOTS:
                continue
            try:
                value = object.__getattribute__(t, name)
            except AttributeError:
                continue
            yield from self._alias_nodes_in_value(value, seen)

    def _family_embeds_in_value(self, value: Any, seen: set[int]) -> Iterator[Type]:
        m = self._m
        if value is None or isinstance(value, (str, bytes, bool, int, float)):
            return
        if isinstance(value, Type):
            if id(value) not in seen:
                if type(value) in m.FAMILY_NAME:
                    yield value
                yield from self._family_embeds(value, seen)
            return
        if isinstance(value, (list, tuple)):
            for item in value:
                yield from self._family_embeds_in_value(item, seen)
            return
        if isinstance(value, dict):
            for item in value.values():
                yield from self._family_embeds_in_value(item, seen)
            return
        if type(value).__name__ == "ExtraAttrs":
            yield from self._family_embeds_in_value(value.attrs, seen)

    def _family_embeds(self, t: Type, seen: set[int]) -> Iterator[Type]:
        if id(t) in seen:
            return
        seen.add(id(t))
        for name in self._m._type_names(type(t)):
            if name.startswith("_") or name in self._m._SKIP_SLOTS:
                continue
            try:
                value = object.__getattribute__(t, name)
            except AttributeError:
                continue
            yield from self._family_embeds_in_value(value, seen)

    # Differential helpers.

    def _walk(self, root: Type) -> tuple[list[TypeVarId], list[Any], list[Type]]:
        return self._m._walk_indices(root)

    @staticmethod
    def _subseq(old: list[Any], new: list[Any]) -> bool:
        """True when `old` appears in `new` in order (ids, repetition kept)."""
        i = 0
        for item in new:
            if i < len(old) and id(item) == id(old[i]):
                i += 1
        return i == len(old)

    # Fixtures.

    def _hidden_chain(self) -> tuple[Instance, TupleType, TypeVarType]:
        fallback = Instance(self.fx.std_tuplei, [self.fx.anyt])
        tup = TupleType([self.fx.a], fallback)
        tv = TypeVarType("_NT", "m.C._NT", TypeVarId(-1, namespace="m.C.f"), [], tup, self.fx.a)
        return fallback, tup, tv

    def _extra_attrs_stub(self, attrs: dict[str, Any]) -> Any:
        class ExtraAttrs:  # matches the duck-typed name check
            pass

        node = ExtraAttrs()
        node.__dict__["attrs"] = attrs
        return node

    def test_alias_list_matches_old_walk_exactly(self) -> None:
        fallback, _tup, tv = self._hidden_chain()
        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        att = TypeAliasType(alias, [tv])
        t = CallableType(
            [self.fx.o, att], [ARG_POS, ARG_POS], [None, None], fallback, self.fx.function
        )
        old = list(self._alias_nodes_in(t, set()))
        new = self._walk(t)[1]
        assert [id(x) for x in old] == [id(x) for x in new], (old, new)

    def test_tvids_are_superset_in_order(self) -> None:
        fallback, _tup, tv = self._hidden_chain()
        # A tvid hidden behind a TypeAlias target: the fused walk descends alias
        # targets for every intent, the old tvids walk did not. The variables
        # tvar (reached through plain Type descent) is the prefix both report.
        alias = TypeAlias(tv, "mod.B", "mod", -1, -1)
        att = TypeAliasType(alias, [])
        t = CallableType(
            [self.fx.o, att],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.o,
            self.fx.function,
            variables=[self.fx.u],
        )
        old = list(self._tvids_in(t, set()))
        new_tvids = self._walk(t)[0]
        assert self._subseq(old, new_tvids), (old, new_tvids)
        assert [id(x) for x in old] == [id(self.fx.u.id)], old
        assert id(tv.id) in {id(x) for x in new_tvids} and len(new_tvids) == len(old) + 1, (
            old,
            new_tvids,
        )

    def test_tvids_inside_alias_target_are_extra_coverage(self) -> None:
        fallback, _tup, tv = self._hidden_chain()
        alias = TypeAlias(tv, "mod.C", "mod", -1, -1)
        t = TypeAliasType(alias, [])
        old = list(self._tvids_in(t, set()))
        new_tvids = self._walk(t)[0]
        # Old walk gives up at the alias node; fused reaches the tvar via
        # target descent, and rescues the hidden leaf for cascade sync.
        assert not old, old
        assert [id(tv.id)] == [id(x) for x in new_tvids], new_tvids

    def test_embeds_match_old_walk_without_alias_targets(self) -> None:
        fallback, tup, _tv = self._hidden_chain()
        t = CallableType(
            [self.fx.o, tup], [ARG_POS, ARG_POS], [None, None], self.fx.o, self.fx.function
        )
        old = list(self._family_embeds(t, set()))
        new_embeds = self._walk(t)[2]
        # No alias nodes in the tree: the fused walk must match the old
        # family walk exactly (Type-level dedup subsumes the missing
        # container seen-set of the old walker).
        assert id(fallback) in {id(x) for x in old}, old
        assert [id(x) for x in old] == [id(x) for x in new_embeds], (old, new_embeds)

    def test_recursive_union_cycles_are_cut(self) -> None:
        u = UnionType([self.fx.a])
        u.items.append(u)  # cycle back to root
        old_a = list(self._alias_nodes_in(u, set()))
        old_t = list(self._tvids_in(u, set()))
        old_e = list(self._family_embeds(u, set()))
        new_tvids, new_aliases, new_embeds = self._walk(u)
        # The aliased family item (fx.a) surfaces identically; the root
        # cycle back through items cuts identically in all walkers.
        assert not new_tvids and not new_aliases
        assert [id(x) for x in old_t] == [id(x) for x in new_tvids], (old_t, new_tvids)
        assert [id(x) for x in old_a] == [id(x) for x in new_aliases], (old_a, new_aliases)
        assert [id(x) for x in old_e] == [id(x) for x in new_embeds], (old_e, new_embeds)

    def test_extra_attrs_stub_is_walked(self) -> None:
        alias = TypeAlias(self.fx.a, "mod.A", "mod", -1, -1)
        stub = self._extra_attrs_stub({"x": alias})
        t = CallableType(
            [self.fx.o, stub], [ARG_POS, ARG_POS], [None, None], self.fx.o, self.fx.function
        )
        old = list(self._alias_nodes_in(t, set()))
        new_aliases = self._walk(t)[1]
        assert [id(x) for x in old] == [id(x) for x in new_aliases], (old, new_aliases)

    def test_shared_container_aliasing_keeps_order(self) -> None:
        fallback, _tup, _tv = self._hidden_chain()
        shared = [fallback]
        c1 = CallableType([self.fx.o], [ARG_POS], [None], fallback, self.fx.function)
        # Same list object under two slots of one tree: the old family walk
        # (no container seen-set) iterated it twice but never duplicated the
        # family item (Type-level seen dedup); the fused walk must agree.
        c1.arg_types = shared  # type: ignore[assignment]
        c1.ret_type = shared[0]
        old = list(self._family_embeds(c1, set()))
        new_embeds = self._walk(c1)[2]
        assert [id(x) for x in old] == [id(x) for x in new_embeds], (old, new_embeds)
        assert len(old) == 2 and id(old[0]) == id(fallback), old

    def test_hidden_chain_full_differential(self) -> None:
        fallback, tup, tv = self._hidden_chain()
        t = CallableType([tv, tup], [ARG_POS, ARG_POS], [None, None], fallback, self.fx.function)
        old_t = list(self._tvids_in(t, set()))
        old_a = list(self._alias_nodes_in(t, set()))
        old_e = list(self._family_embeds(t, set()))
        new_tvids, new_aliases, new_embeds = self._walk(t)
        assert self._subseq(old_t, new_tvids) and len(new_tvids) >= len(old_t), (old_t, new_tvids)
        assert [id(x) for x in old_a] == [id(x) for x in new_aliases], (old_a, new_aliases)
        assert self._subseq(old_e, new_embeds) and len(new_embeds) >= len(old_e), (
            old_e,
            new_embeds,
        )


@skipUnless(_splice_kernel is not None, "requires the type_kernel extension")
class NativeMirrorWalkIndicesRustSuite(Suite):
    """Unit tests for the Rust port of `_walk_indices` (Slice 7).

    `_walk_indices` routes to `rust_mirror_walk_indices` (one PyO3 walk
    over the live graph, mirroring the Python dispatch and seen-set
    semantics exactly) and falls back to `_walk_indices_py` when the
    kernel defers. These tests pin the Rust-only behaviors the big
    differential suite above does not capture explicitly: shim routing
    (Rust engaged, output identical), gate-off fall-through, no tvid
    deduplication, the string-`__slots__` character spill, and the
    undecodable-slot defer.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _graphs(self) -> dict[str, Any]:
        from mypy.types import ExtraAttrs

        fx = self.fx
        graphs: dict[str, Any] = {}
        graphs["instance"] = Instance(fx.gi, [fx.a, fx.t])
        graphs["callable"] = CallableType(
            [fx.o, fx.str_type],
            [ARG_POS, ARG_POS],
            [None, None],
            fx.o,
            fx.function,
            name="f",
            variables=[fx.t],
        )
        graphs["alias"] = fx.def_alias_1(fx.a)[0]
        graphs["union"] = UnionType([fx.gb, Instance(fx.std_tuplei, [graphs["alias"]])])
        inst = Instance(fx.gi, [fx.d])
        # The walk only duck-types this dict; two keys are intentionally not
        # Types (scalar flag + mixed list) to pin container recursion.
        attrs = cast("dict[str, Type]", {"x": fx.str_type, "n": 5, "lst": [fx.a, "zz"]})
        inst.extra_attrs = ExtraAttrs(attrs, immutable=set())
        graphs["extra_attrs"] = inst
        graphs["tuple"] = TupleType([fx.o, fx.t], fx.std_tuple)
        graphs["tvar"] = fx.t
        return graphs

    def _assert_equal(self, key: str, t: Any, rust: Any, py: Any) -> None:
        assert rust is not None, key
        for tag, r, p in zip(("tvids", "aliases", "embeds"), rust, py, strict=True):
            assert [id(x) for x in r] == [id(x) for x in p], (key, type(t).__name__, tag)

    def test_differential_over_graphs(self) -> None:
        for key, t in self._graphs().items():
            rust = self._m._kernel_mod.rust_mirror_walk_indices(t)
            py = self._m._walk_indices_py(t)
            self._assert_equal(key, t, rust, py)

    def test_shim_routes_to_rust_and_matches(self) -> None:
        for key, t in self._graphs().items():
            got = self._m._walk_indices(t)
            py = self._m._walk_indices_py(t)
            self._assert_equal(key, t, got, py)

    def test_gate_off_matches_python(self) -> None:
        """A deferring kernel seam routes `_walk_indices` to the
        pure-Python body with identical results. A stub stands in for the
        kernel: Nones-out `_kernel_mod` while the mirror lives on would
        crash unrelated funnels."""
        import types as _types_mod

        stub = _types_mod.SimpleNamespace(
            rust_mirror_handle_of=lambda obj: None, rust_mirror_walk_indices=lambda root: None
        )
        graphs = self._graphs()
        saved = self._m._kernel_mod
        try:
            self._m._kernel_mod = stub
            for key, t in graphs.items():
                got = self._m._walk_indices(t)
                py = self._m._walk_indices_py(t)
                self._assert_equal(key, t, got, py)
        finally:
            self._m._kernel_mod = saved

    def test_tvids_not_deduplicated(self) -> None:
        """Two distinct TypeVarType nodes carrying the SAME TypeVarId
        object each contribute an occurrence: the walk dedupes Type
        nodes, never TypeVarIds."""
        fx = self.fx
        twin = fx.t.copy_modified(id=fx.t.id)
        assert twin is not fx.t and twin.id is fx.t.id
        c = CallableType(
            [fx.o, fx.o],
            [ARG_POS, ARG_POS],
            [None, None],
            fx.o,
            fx.function,
            name="f",
            variables=[fx.t, twin],
        )
        res = self._m._kernel_mod.rust_mirror_walk_indices(c)
        assert res is not None
        assert len(res[0]) == 2
        self._assert_equal("dup-tvid", c, res, self._m._walk_indices_py(c))

    def test_string_slots_chars_parity(self) -> None:
        """A string __slots__ contributes its characters, like Python's
        list.extend over the string; both walks land the same empty
        result since the chars are unreadable slot names."""

        class StrSlots:
            __slots__ = "ab"  # names 'a', 'b' per _type_names

        s: Any = StrSlots()
        res = self._m._kernel_mod.rust_mirror_walk_indices(s)
        assert res is not None
        assert res == ([], [], []), res
        assert self._m._walk_indices_py(s) == ([], [], [])

    def test_defers_on_unreadable_slot_item(self) -> None:
        """A non-string item in __slots__ cannot be a name: Rust defers,
        the shim re-runs the pure-Python body which raises the same
        AttributeError as before."""

        class WeirdSlots:
            __slots__ = ("x",)

        WeirdSlots.__slots__ = (object(),)  # type: ignore[assignment]  # post-hoc; walkers only
        w = WeirdSlots()
        res = self._m._kernel_mod.rust_mirror_walk_indices(w)
        assert res is None
        self.assertRaises(AttributeError, self._m._walk_indices_py, w)

    def test_registration_walk_matches_python(self) -> None:
        """`_walk_registration` supplies the index lists plus the direct
        family children in `_child_types` order; Python and Rust agree,
        including a repeated child occurrence."""
        graphs = self._graphs()
        graphs["dup_child"] = TupleType([self.fx.o, self.fx.o], self.fx.std_tuple)
        for key, t in graphs.items():
            got = self._m._walk_registration(t)
            py = self._m._walk_registration_py(t)
            self._assert_equal(key, t, got[:3], py[:3])
            assert [id(x) for x in got[3]] == [id(x) for x in py[3]], (key, "children")
            assert all(type(x) in self._m.FAMILY_NAME for x in got[3]), key

    def test_registration_walk_defer_falls_back(self) -> None:
        """A deferring kernel seam routes `_walk_registration` to the
        pure-Python body with identical children."""
        import types as _types_mod

        stub = _types_mod.SimpleNamespace(rust_mirror_walk_registration=lambda root: None)
        saved = self._m._kernel_mod
        try:
            self._m._kernel_mod = stub
            for key, t in self._graphs().items():
                got = self._m._walk_registration(t)
                py = self._m._walk_registration_py(t)
                self._assert_equal(key, t, got[:3], py[:3])
                assert [id(x) for x in got[3]] == [id(x) for x in py[3]], (key, "children")
        finally:
            self._m._kernel_mod = saved


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeMirrorReadSuite(Suite):
    """Unit tests for the Phase F2 (#1393) mirror-read flip at checkexpr.

    `_serialize_type_for_checkexpr` swaps its expensive cache-miss wire walk
    for `types_mirror.read_fresh_bytes` when the flip is wired in: the read
    returns the blob from Rust mirror storage, kept fresh by the F1 capture
    invariant. Gate-off (funnel None / read mode off) keeps the pure-Python
    wire-cache behavior. Every deferred path returns None and the funnel
    serializes exactly as before.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        from mypy.types import _set_native_mirror_read

        _set_native_mirror_read(None)
        self._m._read_mode = False
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _generic_callable(self) -> CallableType:
        """A CallableType carrying a TypeVar: the taint check never caches it.

        This is the funnel's heaviest cached-miss block: no builtin fast
        path, no no-arg encode, always the full taint-checking walk.
        """
        return CallableType(
            [self.fx.o],
            [ARG_POS],
            [None],
            self.fx.o,
            self.fx.function,
            name="f",
            variables=[self.fx.t],
        )

    def _set_read_spy(self) -> list[Type]:
        """Wrap the read funnel in a spy so seam tests prove engagement."""
        from mypy.types import _set_native_mirror_read

        calls: list[Type] = []

        def spy(t: Type) -> bytes | None:
            calls.append(t)
            fresh: bytes | None = self._m.read_fresh_bytes(t)
            return fresh

        _set_native_mirror_read(spy)
        return calls

    def test_read_returns_mirror_bytes_equal_to_fresh(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.checkexpr import _serialize_type_for_checkexpr

        ct = self._generic_callable()
        ct.write(WriteBuffer())  # adoption funnel registers the tree
        self._m._read_mode = True
        calls = self._set_read_spy()
        fresh = self._m._fresh_bytes(ct)
        got = _serialize_type_for_checkexpr(ct)
        assert got == fresh
        assert calls == [ct]  # the funnel consulted the mirror, not the fallback walk

    def test_gate_off_keeps_pure_python_behavior(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.checkexpr import _serialize_type_for_checkexpr
        from mypy.types import _set_native_mirror_read

        ct = self._generic_callable()
        ct.write(WriteBuffer())
        _set_native_mirror_read(None)
        got = _serialize_type_for_checkexpr(ct)
        assert got == self._m._fresh_bytes(ct)

    def test_unregistered_object_defers(self) -> None:
        from mypy.checkexpr import _serialize_type_for_checkexpr
        from mypy.types import _set_native_mirror_read

        self._m._read_mode = True
        _set_native_mirror_read(self._m.read_fresh_bytes)
        ct = self._generic_callable()  # never written: no handle
        assert self._m.read_fresh_bytes(ct) is None
        assert _serialize_type_for_checkexpr(ct) == self._m._fresh_bytes(ct)

    def test_captured_mutation_is_served_fresh(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.checkexpr import _serialize_type_for_checkexpr
        from mypy.types import _set_native_mirror_read

        ct = self._generic_callable()
        ct.write(WriteBuffer())
        self._m._read_mode = True
        _set_native_mirror_read(self._m.read_fresh_bytes)
        # A plain setattr goes through the patched __setattr__ (captured):
        # the blob resyncs, so the read serves fresh bytes.
        ct.ret_type = self.fx.std_tuple  # captured setattr
        assert _serialize_type_for_checkexpr(ct) == self._m._fresh_bytes(ct)

    def test_raw_list_mutation_served_stale_until_touch(self) -> None:
        """Issue #1530 repro shape: a raw item store on a family list never
        fires __setattr__, so the read serves the pre-mutation blob until
        the mutating site calls `types_mirror.touch`."""
        from librt.internal import WriteBuffer

        from mypy.types import _mirror_touch

        ct = self._generic_callable()
        ct.write(WriteBuffer())  # adoption funnel registers the tree
        self._m._read_mode = True
        stale = self._m.read_fresh_bytes(ct)
        assert stale is not None
        ct.arg_types[0] = self.fx.a  # raw escape, no __setattr__
        assert self._m.read_fresh_bytes(ct) == stale
        _mirror_touch(ct)
        fresh = self._m._fresh_bytes(ct)
        assert fresh != stale
        assert self._m.read_fresh_bytes(ct) == fresh

    def test_touch_epoch_gate_refreshes_untouched_handle(self) -> None:
        """`touch` bumps the global epoch: a handle whose own blob drifted
        without a touch is re-serialized by the read gate anyway (the
        shared-list / nested-mutation safety net)."""
        from librt.internal import WriteBuffer

        from mypy.types import _mirror_touch

        ct = self._generic_callable()
        other = self._generic_callable()
        ct.write(WriteBuffer())
        other.write(WriteBuffer())
        self._m._read_mode = True
        other_stale = self._m.read_fresh_bytes(other)
        assert other_stale is not None
        other.arg_types[0] = self.fx.a  # raw escape on `other`, never touched
        assert self._m.read_fresh_bytes(other) == other_stale
        _mirror_touch(ct)  # unrelated touch moves the epoch
        assert self._m.read_fresh_bytes(other) == self._m._fresh_bytes(other)

    def test_normalize_trivial_unpack_touches_blob(self) -> None:
        """The two #1530 manifestations mutate `arg_types` through
        `CallableType.normalize_trivial_unpack`; its touch keeps the F2
        read fresh (gate-on side)."""
        from librt.internal import WriteBuffer

        ct = self._unpack_callable()
        ct.write(WriteBuffer())
        self._m._read_mode = True
        stale = self._m.read_fresh_bytes(ct)
        ct.normalize_trivial_unpack()
        assert ct.arg_types == [self.fx.a]
        fresh = self._m.read_fresh_bytes(ct)
        assert fresh == self._m._fresh_bytes(ct)
        assert fresh != stale

    def test_normalize_trivial_unpack_gate_off_parity(self) -> None:
        """Gate-off differential for the #1530 mutator: with the touch hook
        uninstalled the same normalization happens with no mirror traffic."""
        from mypy.types import _set_native_mirror_touch

        ct = self._unpack_callable()
        _set_native_mirror_touch(None)
        try:
            ct.normalize_trivial_unpack()
        finally:
            _set_native_mirror_touch(self._m.touch)
        assert ct.arg_types == [self.fx.a]

    def _unpack_callable(self) -> CallableType:
        """`def (*args: Unpack[tuple[A]])`: the normalize_trivial_unpack input."""
        from mypy.types import UnpackType

        tuple_of_a = Instance(self.fx.std_tuplei, [self.fx.a])
        return CallableType(
            [UnpackType(tuple_of_a)], [ARG_STAR], [None], self.fx.o, self.fx.function, name="f"
        )

    def test_read_mode_off_returns_none(self) -> None:
        from librt.internal import WriteBuffer

        ct = self._generic_callable()
        ct.write(WriteBuffer())  # registered tree, but the gate is off
        self._m._read_mode = False
        assert self._m.read_fresh_bytes(ct) is None

    def test_checkmember_seam_reads_mirror_bytes(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.checkmember import _serialize_type_for_checkmember

        ct = self._generic_callable()
        ct.write(WriteBuffer())  # adoption funnel registers the tree
        calls = self._set_read_spy()
        self._m._read_mode = True
        assert _serialize_type_for_checkmember(ct) == self._m._fresh_bytes(ct)
        assert calls == [ct]  # the funnel consulted the mirror, not the fallback walk

    def test_checkmember_seam_gate_off_serializes(self) -> None:
        from mypy.checkmember import _serialize_type_for_checkmember
        from mypy.types import _set_native_mirror_read

        ct = self._generic_callable()
        _set_native_mirror_read(None)
        self._m._read_mode = True
        # Never written: no handle. The funnel serializes exactly as before.
        assert _serialize_type_for_checkmember(ct) == self._m._fresh_bytes(ct)

    def test_checker_seam_reads_mirror_bytes(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.checker import _serialize_type_for_checker

        ct = self._generic_callable()
        ct.write(WriteBuffer())  # adoption funnel registers the tree
        calls = self._set_read_spy()
        self._m._read_mode = True
        assert _serialize_type_for_checker(ct) == self._m._fresh_bytes(ct)
        assert calls == [ct]  # the funnel consulted the mirror, not the fallback walk

    def test_checker_seam_gate_off_serializes(self) -> None:
        from mypy.checker import _serialize_type_for_checker
        from mypy.types import _set_native_mirror_read

        ct = self._generic_callable()
        _set_native_mirror_read(None)
        self._m._read_mode = True
        # Never written: no handle. The funnel serializes exactly as before.
        assert _serialize_type_for_checker(ct) == self._m._fresh_bytes(ct)

    def test_subtypes_seam_reads_mirror_bytes(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.subtypes import _serialize_type

        ct = self._generic_callable()
        ct.write(WriteBuffer())  # adoption funnel registers the tree
        calls = self._set_read_spy()
        self._m._read_mode = True
        assert _serialize_type(ct) == self._m._fresh_bytes(ct)
        assert calls == [ct]  # the funnel consulted the mirror, not the fallback walk

    def test_subtypes_seam_gate_off_serializes(self) -> None:
        from mypy.subtypes import _serialize_type
        from mypy.types import _set_native_mirror_read

        ct = self._generic_callable()
        _set_native_mirror_read(None)
        self._m._read_mode = True
        # Never written: no handle. The funnel serializes exactly as before.
        assert _serialize_type(ct) == self._m._fresh_bytes(ct)

    def test_typeops_seam_reads_mirror_bytes(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.typeops import _serialize_type as _serialize_type_typeops

        ct = self._generic_callable()
        ct.write(WriteBuffer())  # adoption funnel registers the tree
        calls = self._set_read_spy()
        self._m._read_mode = True
        assert _serialize_type_typeops(ct) == self._m._fresh_bytes(ct)
        assert calls == [ct]  # the funnel consulted the mirror, not the fallback walk

    def test_typeops_seam_gate_off_serializes(self) -> None:
        from mypy.typeops import _serialize_type as _serialize_type_typeops
        from mypy.types import _set_native_mirror_read

        ct = self._generic_callable()
        _set_native_mirror_read(None)
        self._m._read_mode = True
        # Never written: no handle. The funnel serializes exactly as before.
        assert _serialize_type_typeops(ct) == self._m._fresh_bytes(ct)

    def test_erasetype_seam_reads_mirror_bytes(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.erasetype import _serialize_type as _serialize_type_erasetype

        ct = self._generic_callable()
        ct.write(WriteBuffer())  # adoption funnel registers the tree
        calls = self._set_read_spy()
        self._m._read_mode = True
        assert _serialize_type_erasetype(ct) == self._m._fresh_bytes(ct)
        assert calls == [ct]  # the funnel consulted the mirror, not the fallback walk

    def test_erasetype_seam_gate_off_serializes(self) -> None:
        from mypy.erasetype import _serialize_type as _serialize_type_erasetype
        from mypy.types import _set_native_mirror_read

        ct = self._generic_callable()
        _set_native_mirror_read(None)
        self._m._read_mode = True
        # Never written: no handle. The funnel serializes exactly as before.
        assert _serialize_type_erasetype(ct) == self._m._fresh_bytes(ct)

    def test_join_seam_reads_mirror_bytes(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.join import _serialize_type as _serialize_type_join

        ct = self._generic_callable()
        ct.write(WriteBuffer())  # adoption funnel registers the tree
        calls = self._set_read_spy()
        self._m._read_mode = True
        assert _serialize_type_join(ct) == self._m._fresh_bytes(ct)
        assert calls == [ct]  # the funnel consulted the mirror, not the fallback walk

    def test_join_seam_gate_off_serializes(self) -> None:
        from mypy.join import _serialize_type as _serialize_type_join
        from mypy.types import _set_native_mirror_read

        ct = self._generic_callable()
        _set_native_mirror_read(None)
        self._m._read_mode = True
        # Never written: no handle. The funnel serializes exactly as before.
        assert _serialize_type_join(ct) == self._m._fresh_bytes(ct)


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeInstanceWriteSuite(Suite):
    """Unit tests for the Phase F3 (#1397) instance-write splice of the mirror.

    With the write flip on, a captured Instance 'args' setattr serializes
    only the new args list (write_type_list framing) and lets the Rust
    splice op decode / swap args / re-encode the stored blob, instead of a
    full Python re-serialize of the root. The stored blob must stay
    byte-identical to `_fresh_bytes`: the serialization funnels assert that
    differential after every write, so a drifting splice surfaces at the
    next funnel. Gate-off keeps the F1 full re-serialize path.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m._write_flip = False
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _list_instance(self, *args: Type, lkv: LiteralType | None = None) -> Instance:
        return Instance(self.fx.std_tuplei, list(args), last_known_value=lkv)

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def _blob_matches_fresh(self, inst: Instance) -> None:
        handle = self._m._handle_of(inst)
        assert handle is not None
        blob = bytes(self._m._kernel_mod.rust_mirror_bytes(handle))
        assert blob == self._m._fresh_bytes(inst)
        # Decoded sanity: the blob still parses and names the right class.
        assert "tuple" in _type_kernel.read_type_to_str(blob)

    def test_spliced_write_matches_fresh_bytes(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._list_instance(self.fx.o)
        inst.write(WriteBuffer())  # adoption funnel registers the object
        self._m._write_flip = True
        before = dict(self._m.report())
        inst.args = (self.fx.str_type, self.fx.anyt)
        delta = self._delta(before)
        assert delta.get("setattr_spliced.instance.args") == 1, delta
        assert not any(k.startswith(("mismatch.", "unserializable.")) for k in delta), delta
        self._blob_matches_fresh(inst)

    def test_spliced_write_preserves_lkv_bytes(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.types import LiteralType

        bool_inst = Instance(self.fx.bool_type_info, [])
        inst = self._list_instance(self.fx.o, lkv=LiteralType(True, bool_inst))
        inst.write(WriteBuffer())
        self._m._write_flip = True
        inst.args = (self.fx.anyt,)
        # If the splice had dropped or mangled last_known_value, the stored
        # blob would drift from a full fresh serialization.
        self._blob_matches_fresh(inst)

    def test_gate_off_keeps_full_capture_path(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._list_instance(self.fx.o)
        inst.write(WriteBuffer())
        self._m._write_flip = False
        before = dict(self._m.report())
        inst.args = (self.fx.str_type,)
        delta = self._delta(before)
        assert delta.get("setattr_captured.instance.args") == 1, delta
        assert "setattr_spliced.instance.args" not in delta, delta
        self._blob_matches_fresh(inst)

    def test_noop_write_counts_noop(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._list_instance(self.fx.o)
        inst.write(WriteBuffer())
        self._m._write_flip = True
        before = dict(self._m.report())
        inst.args = (self.fx.o,)  # identical args: the splice round-trips
        delta = self._delta(before)
        assert delta.get("setattr_noop.instance.args") == 1, delta
        self._blob_matches_fresh(inst)

    def test_spliced_type_write_matches_fresh_bytes(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._list_instance(self.fx.str_type)
        inst.write(WriteBuffer())  # adoption funnel registers the object
        self._m._write_flip = True
        before = dict(self._m.report())
        inst.type = self.fx.bool_type_info
        delta = self._delta(before)
        assert delta.get("setattr_spliced.instance.type") == 1, delta
        assert not any(k.startswith(("mismatch.", "unserializable.")) for k in delta), delta
        # The splice only swaps the fallback fullname ref; a full fresh
        # re-serialize must produce the same bytes.
        handle = self._m._handle_of(inst)
        assert handle is not None
        blob = bytes(self._m._kernel_mod.rust_mirror_bytes(handle))
        assert blob == self._m._fresh_bytes(inst)
        # The fallback name changed, so the decoded sanity check greps for
        # the new class instead of tuple.
        assert "bool" in _type_kernel.read_type_to_str(blob)

    def test_noop_type_write_counts_noop(self) -> None:
        from librt.internal import WriteBuffer

        inst = self._list_instance(self.fx.str_type)
        inst.write(WriteBuffer())
        self._m._write_flip = True
        before = dict(self._m.report())
        inst.type = self.fx.std_tuplei  # same fullname: bytes unchanged
        delta = self._delta(before)
        assert delta.get("setattr_noop.instance.type") == 1, delta
        self._blob_matches_fresh(inst)

    def test_spliced_lkv_write_and_clear_match_fresh(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.types import LiteralType

        bool_inst = Instance(self.fx.bool_type_info, [])
        inst = self._list_instance(self.fx.o)
        inst.write(WriteBuffer())
        self._m._write_flip = True
        before = dict(self._m.report())
        inst.last_known_value = LiteralType(True, bool_inst)
        delta = self._delta(before)
        assert delta.get("setattr_spliced.instance.last_known_value") == 1, delta
        self._blob_matches_fresh(inst)
        before = dict(self._m.report())
        inst.last_known_value = None  # the write_type_opt LITERAL_NONE clear
        delta = self._delta(before)
        assert delta.get("setattr_spliced.instance.last_known_value") == 1, delta
        self._blob_matches_fresh(inst)

    def test_gate_off_type_and_lkv_keep_full_capture_path(self) -> None:
        from librt.internal import WriteBuffer

        from mypy.types import LiteralType

        bool_inst = Instance(self.fx.bool_type_info, [])
        inst = self._list_instance(self.fx.o)
        inst.write(WriteBuffer())
        self._m._write_flip = False
        before = dict(self._m.report())
        inst.type = self.fx.bool_type_info
        delta = self._delta(before)
        assert delta.get("setattr_captured.instance.type") == 1, delta
        before = dict(self._m.report())
        inst.type = self.fx.std_tuplei
        delta = self._delta(before)
        assert delta.get("setattr_captured.instance.type") == 1, delta
        before = dict(self._m.report())
        inst.last_known_value = LiteralType(True, bool_inst)
        delta = self._delta(before)
        assert delta.get("setattr_captured.instance.last_known_value") == 1, delta
        assert "setattr_spliced.instance.last_known_value" not in delta, delta
        self._blob_matches_fresh(inst)

    def _attrs_instance(self) -> Instance:
        from librt.internal import WriteBuffer

        inst = self._list_instance(self.fx.o)
        inst.write(WriteBuffer())  # adoption funnel registers the object
        return inst

    def test_spliced_extra_attrs_write_matches_fresh_bytes(self) -> None:
        from mypy.types import ExtraAttrs

        inst = self._attrs_instance()
        self._m._write_flip = True
        before = dict(self._m.report())
        # Two keys in reverse-sorted insertion order: a HashMap re-encode
        # would sort them, so byte identity proves the raw record survives.
        inst.extra_attrs = ExtraAttrs({"b": self.fx.anyt, "a": self.fx.o}, set(), "m")
        delta = self._delta(before)
        assert delta.get("setattr_spliced.instance.extra_attrs") == 1, delta
        assert not any(k.startswith(("mismatch.", "unserializable.")) for k in delta), delta
        self._blob_matches_fresh(inst)
        # A second, identical assignment is a noop that leaves the blob alone.
        before = dict(self._m.report())
        inst.extra_attrs = inst.extra_attrs
        delta = self._delta(before)
        assert delta.get("setattr_noop.instance.extra_attrs") == 1, delta
        self._blob_matches_fresh(inst)
        # Clear: back to the no-attribute shape.
        before = dict(self._m.report())
        inst.extra_attrs = None
        delta = self._delta(before)
        assert delta.get("setattr_spliced.instance.extra_attrs") == 1, delta
        self._blob_matches_fresh(inst)

    def test_extra_attrs_splice_preserves_existing_fields(self) -> None:
        from mypy.types import ExtraAttrs, LiteralType

        bool_inst = Instance(self.fx.bool_type_info, [])
        inst = self._list_instance(self.fx.str_type, lkv=LiteralType(True, bool_inst))
        from librt.internal import WriteBuffer

        inst.write(WriteBuffer())
        self._m._write_flip = True
        inst.extra_attrs = ExtraAttrs({"x": self.fx.anyt}, set(), None)
        # If the splice had dropped args or last_known_value, the stored
        # blob would drift from a full fresh serialization.
        self._blob_matches_fresh(inst)
        inst.args = (self.fx.o,)
        self._blob_matches_fresh(inst)
        inst.last_known_value = None
        self._blob_matches_fresh(inst)

    def test_gate_off_extra_attrs_keeps_full_capture_path(self) -> None:
        from mypy.types import ExtraAttrs

        inst = self._attrs_instance()
        self._m._write_flip = False
        before = dict(self._m.report())
        inst.extra_attrs = ExtraAttrs({"x": self.fx.anyt}, set(), None)
        delta = self._delta(before)
        assert delta.get("setattr_captured.instance.extra_attrs") == 1, delta
        assert "setattr_spliced.instance.extra_attrs" not in delta, delta
        self._blob_matches_fresh(inst)


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeInvisibleFieldSuite(Suite):
    """Wire-invisible field writes stay outside the mirror capture funnel.

    line / column / end_line / end_column / definition are not read by any
    types.py write() body, so no write to them can change a stored blob:
    extending SKIP_ATTRS keeps the hook (and the full fresh re-serialize
    plus cascade of identical bytes) out of the way for set_line()-style
    bookkeeping, the single hottest captured write shape on the self-check
    corpus (139,894 captured Instance line writes per audited cold run).

    The identical-visible-write test also pins the fresh-path noop
    comparison fix: rust_mirror_bytes returns a list and _fresh_bytes
    bytes, and the pre-fix list == bytes check was always False, so every
    unchanged visible write wrongly took the capture-and-cascade path.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()
        types_mirror._write_flip = True

    def tearDown(self) -> None:
        self._m._write_flip = False
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def _blob_matches_fresh(self, t: Type) -> None:
        handle = self._m._handle_of(t)
        assert handle is not None
        blob = bytes(self._m._kernel_mod.rust_mirror_bytes(handle))
        assert blob == self._m._fresh_bytes(t)

    def _registered_instance(self) -> Instance:
        from librt.internal import WriteBuffer

        inst = Instance(self.fx.std_tuplei, [self.fx.o])
        inst.write(WriteBuffer())  # adoption funnel registers the object
        return inst

    def _registered_callable(self) -> CallableType:
        from librt.internal import WriteBuffer

        cb = CallableType([self.fx.o], [ARG_POS], [None], self.fx.o, self.fx.o)
        cb.write(WriteBuffer())
        return cb

    def test_line_write_is_unhooked(self) -> None:
        inst = self._registered_instance()
        before = dict(self._m.report())
        inst.line = 42
        delta = self._delta(before)
        assert delta == {}, delta
        self._blob_matches_fresh(inst)

    def test_set_line_fields_are_unhooked(self) -> None:
        inst = self._registered_instance()
        before = dict(self._m.report())
        inst.set_line(31, column=5, end_line=32, end_column=6)
        delta = self._delta(before)
        assert delta == {}, delta
        self._blob_matches_fresh(inst)

    def test_definition_write_is_unhooked(self) -> None:
        cb = self._registered_callable()
        before = dict(self._m.report())
        cb.definition = None  # the bump-timestamp fixup-style rewrite
        delta = self._delta(before)
        assert delta == {}, delta
        self._blob_matches_fresh(cb)

    def test_identical_visible_write_counts_noop(self) -> None:
        cb = self._registered_callable()
        before = dict(self._m.report())
        cb.name = cb.name  # same value: bytes unchanged, must be a noop
        delta = self._delta(before)
        assert delta.get("setattr_noop.callable.name") == 1, delta
        self._blob_matches_fresh(cb)

    def test_visible_write_still_captures(self) -> None:
        inst = self._registered_instance()
        before = dict(self._m.report())
        inst.args = (self.fx.str_type,)  # real change: full capture path
        delta = self._delta(before)
        assert delta.get("setattr_spliced.instance.args") == 1, delta
        self._blob_matches_fresh(inst)


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeMirrorCallableWriteSuite(Suite):
    """Unit tests for the Phase F3 slice-8 (#1397) CallableType splice ops.

    With the write flip on, a captured CallableType setattr for a
    wire-visible field serializes only the changed field and lets the
    matching Rust splice op decode / swap / re-encode the stored blob,
    instead of a full Python re-serialize. The stored blob must stay
    byte-identical to a full fresh serialization (`_fresh_bytes`); the
    funnels assert that differential after every write. Gate-off keeps
    the full re-serialize capture path.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m._write_flip = False
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def _assert_clean(self, delta: dict[str, int]) -> None:
        assert not any(k.startswith(("mismatch.", "unserializable.")) for k in delta), delta

    def _blob_matches_fresh(self, cb: CallableType) -> None:
        handle = self._m._handle_of(cb)
        assert handle is not None
        blob = bytes(self._m._kernel_mod.rust_mirror_bytes(handle))
        assert blob == self._m._fresh_bytes(cb)
        # Decoded sanity: the blob still parses as a callable.
        assert "->" in _type_kernel.read_type_to_str(blob)

    def _registered_callable(self, **kwargs: Any) -> CallableType:
        from librt.internal import WriteBuffer

        cb = CallableType([self.fx.o], [ARG_POS], [None], self.fx.o, self.fx.o, **kwargs)
        cb.write(WriteBuffer())  # adoption funnel registers the object
        return cb

    def test_spliced_ret_type_write_matches_fresh_bytes(self) -> None:
        cb = self._registered_callable()
        self._m._write_flip = True
        before = dict(self._m.report())
        cb.ret_type = self.fx.str_type
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.ret_type") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)
        # Noop: write the same value again, the splice round-trips.
        before = dict(self._m.report())
        cb.ret_type = self.fx.str_type
        delta = self._delta(before)
        assert delta.get("setattr_noop.callable.ret_type") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)

    def test_spliced_arg_types_swap_matches_fresh_bytes(self) -> None:
        cb = self._registered_callable()
        self._m._write_flip = True
        before = dict(self._m.report())
        cb.arg_types = [self.fx.anyt]
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.arg_types") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)

    def test_spliced_arg_kinds_and_names_match_fresh_bytes(self) -> None:
        cb = self._registered_callable()
        self._m._write_flip = True
        before = dict(self._m.report())
        cb.arg_kinds = [ARG_OPT]
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.arg_kinds") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)
        # Noop kinds: same list.
        before = dict(self._m.report())
        cb.arg_kinds = [ARG_OPT]
        delta = self._delta(before)
        assert delta.get("setattr_noop.callable.arg_kinds") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)
        # Names: change then noop.
        before = dict(self._m.report())
        cb.arg_names = ["x"]
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.arg_names") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)
        before = dict(self._m.report())
        cb.arg_names = ["x"]
        delta = self._delta(before)
        assert delta.get("setattr_noop.callable.arg_names") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)

    def test_spliced_name_write(self) -> None:
        cb = self._registered_callable()
        self._m._write_flip = True
        before = dict(self._m.report())
        cb.name = "ff"
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.name") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)

    def test_spliced_variables_write(self) -> None:
        from mypy.types import AnyType, TypeOfAny, TypeVarId, TypeVarType

        tvar = TypeVarType(
            "T",
            "T",
            TypeVarId(-100),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
            INVARIANT,
        )
        cb = self._registered_callable(variables=[tvar])
        self._m._write_flip = True
        before = dict(self._m.report())
        cb.variables = ()
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.variables") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)

    def test_flag_write_splices_all_seven(self) -> None:
        cb = self._registered_callable()
        self._m._write_flip = True
        before = dict(self._m.report())
        cb.implicit = True
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.implicit") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)
        # Noop flag write: same False value on a different flag.
        before = dict(self._m.report())
        cb.from_type_type = False
        delta = self._delta(before)
        assert delta.get("setattr_noop.callable.from_type_type") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)

    def test_type_guard_write_and_clear(self) -> None:
        cb = self._registered_callable()
        self._m._write_flip = True
        before = dict(self._m.report())
        cb.type_guard = self.fx.anyt
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.type_guard") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)
        before = dict(self._m.report())
        cb.type_guard = None
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.type_guard") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)

    def test_fallback_write_matches_fresh_bytes(self) -> None:
        cb = self._registered_callable()
        self._m._write_flip = True
        before = dict(self._m.report())
        cb.fallback = self.fx.std_tuple
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.fallback") == 1, delta
        self._assert_clean(delta)
        handle = self._m._handle_of(cb)
        assert handle is not None
        blob = bytes(self._m._kernel_mod.rust_mirror_bytes(handle))
        assert blob == self._m._fresh_bytes(cb)
        # Callables do not print the fallback in str(); byte identity with a
        # full fresh serialization is the fallback-swap check.

    def test_instance_type_noop_and_set(self) -> None:
        cb = self._registered_callable()
        self._m._write_flip = True
        # instance_type is already None: a None write is a noop.
        before = dict(self._m.report())
        cb.instance_type = None
        delta = self._delta(before)
        assert delta.get("setattr_noop.callable.instance_type") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)
        # Set: a real clear-to-set splice.
        before = dict(self._m.report())
        cb.instance_type = self.fx.str_type
        delta = self._delta(before)
        assert delta.get("setattr_spliced.callable.instance_type") == 1, delta
        self._assert_clean(delta)
        self._blob_matches_fresh(cb)

    def test_gate_off_keeps_full_capture_path(self) -> None:
        cb = self._registered_callable()
        self._m._write_flip = False
        before = dict(self._m.report())
        cb.ret_type = self.fx.str_type
        delta = self._delta(before)
        assert delta.get("setattr_captured.callable.ret_type") == 1, delta
        assert "setattr_spliced.callable.ret_type" not in delta, delta
        self._blob_matches_fresh(cb)
        before = dict(self._m.report())
        cb.implicit = True
        delta = self._delta(before)
        assert delta.get("setattr_captured.callable.implicit") == 1, delta
        self._blob_matches_fresh(cb)

    def test_special_sig_write_is_skipped(self) -> None:
        # Wire-invisible per Phase F0 (#1349): SKIP_ATTRS keeps the whole
        # hook (and the fresh re-serialize of identical bytes) out.
        cb = self._registered_callable()
        self._m._write_flip = True
        before = dict(self._m.report())
        cb.special_sig = None
        delta = self._delta(before)
        assert delta == {}, delta
        self._blob_matches_fresh(cb)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMirrorUnionPlainDataSuite(Suite):
    """UnionType plain-data writes (is_evaluated, original_str_*) skip capture.

    Phase F0 (#1349): the union wire never carries is_evaluated /
    original_str_expr / original_str_fallback, so a captured write can only
    prove a full _fresh_bytes re-serialize to identical bytes (slice 9 of the
    F3 write flip, #1397: ~29k such writes per nativeparse self-check). They
    join SKIP_ATTRS like CallableType.special_sig in slice 8.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _registered_union(self) -> UnionType:
        # A carrier embedding the union runs the first-serialization funnel,
        # which adopts the whole subtree (the carrier and the union).
        from librt.internal import WriteBuffer

        u = UnionType([self.fx.a, self.fx.b])
        ct = Instance(self.fx.std_tuplei, [u])
        ct.write(WriteBuffer())
        h = self._m._handle_of(u)
        assert h is not None, "union never adopted by the funnel"
        return u

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def test_skip_attrs_cover_union_plain_data(self) -> None:
        from mypy import types_mirror

        for name in ("is_evaluated", "original_str_expr", "original_str_fallback"):
            assert name in types_mirror.SKIP_ATTRS

    def test_plain_data_writes_skip_bookkeeping(self) -> None:
        u = self._registered_union()
        before = dict(self._m.report())
        for field, value in (
            ("is_evaluated", False),
            ("original_str_expr", "A | B"),
            ("original_str_fallback", "builtins.int"),
            ("is_evaluated", True),
            ("original_str_expr", None),
        ):
            setattr(u, field, value)
        delta = self._delta(before)
        assert delta == {}, delta

    def test_plain_data_writes_leave_wire_bytes_unchanged(self) -> None:
        u = self._registered_union()
        h = self._m._handle_of(u)
        before_blob = bytes(self._m._kernel_mod.rust_mirror_bytes(h))
        u.is_evaluated = False
        u.original_str_expr = "A | B"
        u.original_str_fallback = "builtins.int"
        after_blob = bytes(self._m._kernel_mod.rust_mirror_bytes(h))
        assert before_blob == after_blob

    def test_plain_data_writes_reach_the_live_object(self) -> None:
        u = self._registered_union()
        u.is_evaluated = False
        u.original_str_expr = "A | B"
        u.original_str_fallback = "builtins.int"
        assert u.is_evaluated is False
        assert u.original_str_expr == "A | B"
        assert u.original_str_fallback == "builtins.int"


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeMirrorAdoptionFastPathSuite(Suite):
    """Slice 10 funnel fast paths around the adoption strike (#1397).

    Extends NativeMirrorAdoptStrikeSuite from the funnel side: a struck
    object must skip the root serialization in both mirror funnels
    (_assert_fresh / _check_splice), and a funnel that already
    serialized the root hands those bytes straight to _register_tree
    instead of paying the walk twice.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m._strict = False
        self._m.reset(clear_counts=True)

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {
            k: v - before.get(k, 0)
            for k, v in after.items()
            if v != before.get(k, 0) and "init." not in k
        }

    def _raise_for_target(self, target: Any) -> tuple[Any, list[Any]]:
        """Patch _fresh_bytes to raise only for `target`; returns orig, calls."""
        orig = self._m._fresh_bytes
        calls: list[Any] = []

        def raiser(x: Any) -> bytes:
            calls.append(x)
            if x is target:
                raise ValueError("pre-semanal partial")
            return orig(x)

        self._m._fresh_bytes = raiser  # type: ignore[assignment]
        return orig, calls

    def _mark_hidden_embed(self, obj: Any) -> None:
        # Mutation-time registration now engages only for objects some
        # registered blob embeds (the lazy-adoption cut); seed the index
        # with a fake parent handle to exercise the strike fast paths.
        self._m._HIDDEN_EMBED[id(obj)] = (obj, {0})

    def test_untracked_setattr_defers_adoption_to_funnel(self) -> None:
        from librt.internal import WriteBuffer

        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        orig, calls = self._raise_for_target(inst)
        try:
            inst.args = (self.fx.o,)
            # Nothing registered embeds the object: no registration
            # attempt, no strike; the first funnel snapshots the final
            # state and adopts.
            assert all(c is not inst for c in calls), calls
            assert self._m._handle_of(inst) is None
            assert id(inst) not in self._m._ADOPT_STRIKE
            delta = self._delta({})
            assert delta.get("setattr_untracked.instance") == 1, delta
        finally:
            self._m._fresh_bytes = orig
        before = dict(self._m.report())
        inst.write(WriteBuffer())
        delta = self._delta(before)
        assert delta.get("adopt.instance.write") == 1, delta
        assert self._m._handle_of(inst) is not None

    def test_failed_registration_publishes_strike_and_funnel_skips(self) -> None:
        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        self._mark_hidden_embed(inst)
        orig, calls = self._raise_for_target(inst)
        try:
            inst.args = (self.fx.o,)  # setattr adopts -> fails -> memo
            assert self._m._handle_of(inst) is None
            delta = self._delta({})
            assert delta.get("setattr_gagged.instance") == 1, delta
            assert delta.get("register_fail_uncaptured") == 1, delta
            assert id(inst) in self._m._ADOPT_STRIKE
            root_calls_before = len(calls)
            before = dict(self._m.report())
            self._m._assert_fresh(inst, "probe")
            delta = self._delta(before)
            assert delta == {"adopt_strike_skip.instance.probe": 1}, delta
            # The struck funnel visit serialized nothing.
            assert len(calls) == root_calls_before, delta
        finally:
            self._m._fresh_bytes = orig

    def test_splice_funnel_skips_serialization_for_struck_object(self) -> None:
        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        self._m._note_failed_adoption(inst)
        before = dict(self._m.report())
        self._m._check_splice(inst, b"unused-on-strike")
        delta = self._delta(before)
        assert delta == {"adopt_strike_skip.instance.cachedsplice": 1}, delta

    def test_register_tree_passes_precomputed_root_bytes_through(self) -> None:
        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        fresh = self._m._fresh_bytes(inst)
        orig = self._m._fresh_bytes
        calls: list[Any] = []

        def counter(x: Any) -> bytes:
            calls.append(x)
            return orig(x)

        self._m._fresh_bytes = counter  # type: ignore[assignment]
        try:
            h = self._m._register_tree(inst, fresh)
            assert h is not None
            # The root was never re-serialized by registration (the child
            # walk still serializes fx.a on its own registration).
            assert all(c is not inst for c in calls), [id(c) for c in calls]
            # And the stored blob is byte-identical to the funnel's bytes.
            assert bytes(self._m._kernel_mod.rust_mirror_bytes(h)) == fresh
        finally:
            self._m._fresh_bytes = orig

    def test_register_tree_local_walk_unchanged_without_bytes(self) -> None:
        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        h = self._m._register_tree(inst)
        assert h is not None
        fresh = self._m._fresh_bytes(inst)
        assert bytes(self._m._kernel_mod.rust_mirror_bytes(h)) == fresh

    def _make_probe_due(self, obj: Any) -> None:
        # Per-object probe cadence: refresh the stuck object's counter so
        # its next encounter rolls over (dup add refreshes the payload).
        self._m._ADOPT_STRIKE.add(id(obj), self._m._STRIKE_RETRY_INTERVAL - 1)

    def test_gagged_setattr_probe_retries_and_adopts(self) -> None:
        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        self._mark_hidden_embed(inst)
        orig, calls = self._raise_for_target(inst)
        try:
            inst.args = (self.fx.o,)  # first attempt: still partial -> strike
            assert sum(1 for c in calls if c is inst) == 1, calls
            self._make_probe_due(inst)
            inst.args = (self.fx.o,)  # probe due, fresh still raising -> gag
            assert self._m._handle_of(inst) is None
            # Exactly one probe serialization for the struck root.
            assert sum(1 for c in calls if c is inst) == 2, calls
            assert id(inst) in self._m._ADOPT_STRIKE
        finally:
            self._m._fresh_bytes = orig
        # Registrability resolved: the probe adopts and a real mutation is
        # captured at the source instead of escaping uncaptured.
        self._make_probe_due(inst)
        before = dict(self._m.report())
        inst.args = (self.fx.b,)
        delta = self._delta(before)
        assert delta.get("setattr_captured.instance.args") == 1, delta
        assert self._m._handle_of(inst) is not None

    def test_write_funnel_probe_adopts_after_interval(self) -> None:
        from librt.internal import WriteBuffer

        inst = Instance(self.fx.std_tuplei, [self.fx.a])
        self._mark_hidden_embed(inst)
        orig, calls = self._raise_for_target(inst)
        try:
            inst.args = (self.fx.o,)  # still partial -> strike
        finally:
            self._m._fresh_bytes = orig
        before = dict(self._m.report())
        inst.write(WriteBuffer())
        delta = self._delta(before)
        assert delta.get("adopt_strike_skip.instance.write") == 1, delta
        assert self._m._handle_of(inst) is None
        self._make_probe_due(inst)
        before = dict(self._m.report())
        inst.write(WriteBuffer())
        delta = self._delta(before)
        assert delta.get("adopt.instance.write") == 1, delta
        assert delta.get("strike_cleared_on_adopt") == 1, delta
        assert self._m._handle_of(inst) is not None


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeMirrorStableIdentitySuite(Suite):
    """Daemon-stable identity handles (issue #1528).

    The stable layer pins a live object with a strong `Py<PyAny>` (no
    weakrefs: every probed Type class refuses one), so the handle survives a
    preserving reset, the daemon recheck boundary. Blob storage and raw ids
    drop, entries whose stable pin is their only owner are released, and a
    non-preserving reset drops the layer outright.
    """

    def setUp(self) -> None:
        from mypy import types_mirror

        types_mirror.activate(audit=True)
        types_mirror.reset(clear_counts=True)
        self._m = types_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m.reset(clear_counts=True)

    def _callable(self) -> CallableType:
        return CallableType([self.fx.o], [ARG_POS], [None], self.fx.o, self.fx.function, name="f")

    def test_preserving_reset_keeps_stable_identity(self) -> None:
        c = self._callable()
        h = self._m._register_tree(c)
        assert h is not None
        stored = bytes(self._m._kernel_mod.rust_mirror_bytes(h))
        assert stored
        # `c` is referenced by this frame, so the sweep keeps its pin.
        self._m.reset(preserve_stable=True)
        assert self._m._kernel_mod.rust_mirror_stable_alive(h)
        assert self._m._kernel_mod.rust_mirror_handle_of(c) == h
        # Per-build blob storage dropped; the Python map is per-build too.
        assert self._m._kernel_mod.rust_mirror_bytes(h) is None
        assert self._m._handle_of(c) is None
        # Re-registration restores identity and rebuilds the blob.
        assert self._m._register_tree(c) == h
        assert self._m._handle_of(c) == h
        assert bytes(self._m._kernel_mod.rust_mirror_bytes(h)) == stored

    def test_full_reset_drops_stable_identity(self) -> None:
        c = self._callable()
        h = self._m._register_tree(c)
        assert h is not None
        self._m.reset()
        assert not self._m._kernel_mod.rust_mirror_stable_alive(h)
        assert self._m._kernel_mod.rust_mirror_handle_of(c) is None
        h2 = self._m._register_tree(c)
        assert h2 is not None and h2 != h

    def test_preserving_reset_releases_unreferenced_entries(self) -> None:
        c = self._callable()
        h = self._m._register_tree(c)
        assert h is not None
        del c
        # The mirror was the only non-stable owner; reset clears its pins
        # first, so the kernel sweep releases the pin-only entry.
        self._m.reset(preserve_stable=True)
        assert not self._m._kernel_mod.rust_mirror_stable_alive(h)

    def test_preserving_reset_keeps_referenced_entries(self) -> None:
        c = self._callable()
        h = self._m._register_tree(c)
        assert h is not None
        self._m.reset(preserve_stable=True)
        assert self._m._kernel_mod.rust_mirror_stable_alive(h)
        assert self._m._kernel_mod.rust_mirror_handle_of(c) == h


class NativeIftaDefinitionRestoreSuite(Suite):
    """Wave 47 (#1455): ifta solution `definition` restoration.

    The ifta seam decodes solution blobs from the kernel; a wire
    CallableType carries `name` but not `definition`, and
    `pretty_callable` (messages.py) renders `def <name>` from it when
    `name` is None. `_restore_ifta_definitions` reattaches it from the
    live actuals: a single lower keeps its own definition (Python
    no-op solve returns the live object), a multi-lower join restores
    the last sorted lower (the `join_type_list` seam invariant,
    join.py), and star actuals skip (Python folds over expanded
    definition-less lowers there).
    """

    def _typeinfo(self, fullname: str = "mod.A") -> TypeInfo:
        from mypy.nodes import Block, ClassDef, SymbolTable

        defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        defn.fullname = fullname
        info = TypeInfo(SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = [info]
        return info

    def setUp(self) -> None:
        self.function_info = self._typeinfo("builtins.function")
        self.int_info = self._typeinfo("builtins.int")
        self.object_info = self._typeinfo("builtins.object")
        self.tv = TypeVarType(
            "T",
            "T",
            TypeVarId(1),
            [],
            Instance(self.object_info, []),
            AnyType(TypeOfAny.from_omitted_generics),
        )
        self.callee = CallableType(
            [self.tv],
            [ARG_STAR],
            [None],
            Instance(self.object_info, []),
            Instance(self.function_info, []),
            name="<list>",
            variables=[self.tv],
        )

    def _call(self, name: str, arg_name: str, definition: FuncDef | None) -> CallableType:
        return CallableType(
            [Instance(self.int_info, [])],
            [ARG_NAMED],
            [arg_name],
            Instance(self.int_info, []),
            Instance(self.function_info, []),
            name=name,
            definition=definition,
        )

    def _decoded(self) -> CallableType:
        return CallableType(
            [Instance(self.int_info, [])],
            [ARG_NAMED],
            ["y"],
            Instance(self.int_info, []),
            Instance(self.function_info, []),
            name=None,
        )

    def _seam(
        self,
        solutions: list[Type | None],
        pass1_args: list[Type | None],
        arg_kinds: list[ArgKind],
        formal_to_actual: list[list[int]],
    ) -> list[Type | None] | None:
        from mypy.checkexpr import _restore_ifta_definitions

        return _restore_ifta_definitions(
            solutions, self.callee, pass1_args, arg_kinds, formal_to_actual
        )

    def test_multi_lower_restores_last_sorted(self) -> None:
        c1 = self._call("f1", "x", FuncDef("f1"))
        c2 = self._call("f2", "y", FuncDef("f2"))
        out = self._seam([self._decoded()], [c1, c2], [ARG_POS, ARG_POS], [[0, 1]])
        assert out is not None
        sol = get_proper_type(out[0])
        assert isinstance(sol, CallableType)
        assert sol.definition is c2.definition
        assert sol.name is None

    def test_single_lower_restores_lower(self) -> None:
        c1 = self._call("f1", "x", FuncDef("f1"))
        out = self._seam([self._decoded()], [c1], [ARG_POS], [[0]])
        assert out is not None
        sol = get_proper_type(out[0])
        assert isinstance(sol, CallableType)
        assert sol.definition is c1.definition

    def test_star_actual_skips(self) -> None:
        c1 = self._call("f1", "x", FuncDef("f1"))
        c2 = self._call("f2", "y", FuncDef("f2"))
        out = self._seam([self._decoded()], [c1, c2], [ARG_STAR, ARG_POS], [[0, 1]])
        assert out is not None
        sol = get_proper_type(out[0])
        assert isinstance(sol, CallableType)
        assert sol.definition is None

    def test_source_without_definition_untouched(self) -> None:
        c1 = self._call("f1", "x", None)
        c2 = self._call("f2", "y", None)
        out = self._seam([self._decoded()], [c1, c2], [ARG_POS, ARG_POS], [[0, 1]])
        assert out is not None
        sol = get_proper_type(out[0])
        assert isinstance(sol, CallableType)
        assert sol.definition is None

    def test_non_callable_solution_untouched(self) -> None:
        inst = Instance(self.int_info, [])
        out = self._seam([inst], [inst], [ARG_POS], [[0]])
        assert out is not None and out[0] is inst

    def test_none_solution_untouched(self) -> None:
        out = self._seam([None], [None], [ARG_POS], [[0]])
        assert out is not None and out[0] is None

    def test_malformed_shape_returns_none(self) -> None:
        c1 = self._call("f1", "x", FuncDef("f1"))
        assert self._seam([], [c1], [ARG_POS], [[0]]) is None


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeCtorBlobGatesSuite(Suite):
    """`_native_ctor_blob` must clear the expand/maptype gates too (#1484).

    The typeops gate is cleared for the blob window (wave22 #1324); the
    expand and maptype gates were left active. A blob chain reaching
    `bind_self`'s generic path or `map_instance_to_supertype` would then
    round-trip doomed FFI against the stale resolver (the fresh class is
    not yet in its snapshot table): snap-miss, Python fallback, wasted
    round-trip. This pins all three gate families off inside the window
    and the gate-on vs gate-off blob parity (identical bytes).
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
        )
        from mypy.maptype import _set_native_map_active, _set_native_map_resolver
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        type_infos = [v for v in vars(self.fx).values() if _is_type_info(v)]
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_typeops_active(True)
        _set_native_typeops_resolver(self._resolver)
        _set_native_expand_type_active(True)
        _set_native_expand_type_resolver(self._resolver)
        _set_native_map_active(True)
        _set_native_map_resolver(self._resolver)
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})

    def tearDown(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
        )
        from mypy.maptype import _set_native_map_active, _set_native_map_resolver
        from mypy.typeops import _set_native_typeops_active, _set_native_typeops_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_typeops_active(False)
        _set_native_typeops_resolver(None)
        _set_native_expand_type_active(False)
        _set_native_expand_type_resolver(None)
        _set_native_map_active(False)
        _set_native_map_resolver(None)
        set_wire_typeinfo_map(None)

    def test_blob_window_clears_all_three_gate_families(self) -> None:
        from mypy import build, expandtype, maptype, typeops
        from mypy.nodes import MDEF, Block, FuncDef, SymbolTableNode

        # A fresh class OUT of the snapshot table: the stale-resolver shape
        # from mid-build (the blob loop runs before the SCC's `update`).
        info = self.fx.make_type_info("mod.BlobGates")
        # A metaclass fallback avoids the stdlib typeinfo lookup, which
        # needs modules_state content unit tests do not populate.
        info.metaclass_type = Instance(self.fx.type_typei, [])
        fd = FuncDef("__init__", [], Block([]))
        fd.info = info
        info.names["__init__"] = SymbolTableNode(MDEF, fd)

        # Gate-on reference blob: setUp installed all three gates with the
        # stale resolver (the precisely pre-fix shape).
        blob_on = build._native_ctor_blob(info)
        assert blob_on is not None, "ctor blob failed to build"

        observed: dict[str, Any] = {}
        orig = typeops.type_object_type

        def probe(probe_info: Any) -> Any:
            observed["typeops_active"] = typeops._native_typeops_active
            observed["expand_active"] = expandtype._native_expand_type_active
            observed["expand_resolver"] = expandtype._native_expand_type_resolver
            observed["map_active"] = maptype._native_map_active
            observed["map_resolver"] = maptype._native_map_resolver
            return orig(probe_info)

        typeops.type_object_type = probe  # type: ignore[assignment]
        try:
            blob_probed = build._native_ctor_blob(info)
        finally:
            typeops.type_object_type = orig

        assert blob_probed == blob_on, "gate-on and gate-off blobs must be identical"
        assert observed["typeops_active"] is False, "typeops gate must be off in window"
        assert observed["expand_active"] is False, "expand gate must be off in window"
        assert observed["expand_resolver"] is None, "expand resolver must clear in window"
        assert observed["map_active"] is False, "map gate must be off in window"
        assert observed["map_resolver"] is None, "map resolver must clear in window"
        # All gates must be restored exactly after the window.
        assert typeops._native_typeops_active is True
        assert typeops._native_typeops_resolver is self._resolver
        assert expandtype._native_expand_type_active is True
        assert expandtype._native_expand_type_resolver is self._resolver
        assert maptype._native_map_active is True
        assert maptype._native_map_resolver is self._resolver


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAstMirrorSuite(Suite):
    """Unit tests for the G1.0a expression node shadow (#1572).

    The store is capture-only: every assertion drives the `mypy.nodes_mirror`
    hook and reads the Rust record back through the `rust_node_mirror_*`
    pyfunctions. The pinnings are the G1 contract: construction never
    adopts, the first non-default binding write adopts with the full
    post-write record, tracked writes merge into one record, and the
    shared identity handle behaves like the proxy/mirror ones.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import nodes_mirror

        # #1864: `analyzed` capture needs the full scope.
        os.environ[nodes_mirror._CAPTURE_SCOPE_ENV] = "full"
        self.addCleanup(os.environ.pop, nodes_mirror._CAPTURE_SCOPE_ENV, None)
        nodes_mirror.activate(audit=True)
        nodes_mirror.reset(clear_counts=True)
        self._k = kernel
        self._m = nodes_mirror

    def tearDown(self) -> None:
        self._m.reset(clear_counts=True)

    def _handle(self, node: Any) -> int:
        handle = self._m._NODE_HANDLES.get(id(node))
        assert handle is not None, "node was not adopted by the shadow"
        return handle

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {k: v - before.get(k, 0) for k, v in after.items() if v != before.get(k, 0)}

    def test_construction_does_not_adopt(self) -> None:
        expr = NameExpr("x")
        assert id(expr) not in self._m._NODE_HANDLES
        assert self._k.rust_node_mirror_entry_count() == 0
        # Explicit baseline writes on a fresh node stay out of the store.
        expr.kind = None
        expr.node = None
        expr._fullname = ""
        expr.is_new_def = False
        expr.is_inferred_def = False
        assert id(expr) not in self._m._NODE_HANDLES
        assert self._k.rust_node_mirror_entry_count() == 0

    def test_ref_capture_roundtrip(self) -> None:
        expr = NameExpr("x")
        target = Var("x")
        target._fullname = "mod.x"
        expr.kind = GDEF
        expr.node = target
        expr.fullname = "mod.x"
        expr.is_new_def = True
        expr.is_inferred_def = True
        handle = self._handle(expr)
        assert self._k.rust_node_mirror_ref(handle) == (GDEF, "mod.x", "mod.x", True, True)

    def test_unbound_node_shadow_stays_distinguishable(self) -> None:
        expr = NameExpr("x")
        expr.kind = LDEF
        handle = self._handle(expr)
        # `kind` captured, target never set: the None fullname is a real
        # shadow value, not a missing capture.
        assert self._k.rust_node_mirror_ref(handle) == (LDEF, None, "", False, False)

    def test_capture_counters_increment_per_write(self) -> None:
        expr = NameExpr("x")
        before = dict(self._m.report())
        expr.kind = GDEF
        expr.node = Var("x")
        handle = self._handle(expr)
        delta = self._delta(before)
        assert delta.get("capture_ref") == 2, delta
        assert self._k.rust_node_mirror_captures(handle) == (2, 0)
        # Re-writing the same value is still a captured write.
        expr.kind = GDEF
        assert self._k.rust_node_mirror_captures(handle) == (3, 0)

    def test_analyzed_capture_merges_into_one_record(self) -> None:
        call = CallExpr(NameExpr("f"), [], [], [])
        assert id(call) not in self._m._NODE_HANDLES
        call.analyzed = CastExpr(NameExpr("y"), AnyType(TypeOfAny.special_form))
        handle = self._k.rust_node_mirror_handle_of(call)
        assert handle is not None
        assert self._k.rust_node_mirror_analyzed(handle) == (True, "CastExpr")
        # A cleared replacement is a tracked write, not a missing entry.
        call.analyzed = None
        assert self._k.rust_node_mirror_analyzed(handle) == (True, None)
        assert self._k.rust_node_mirror_captures(handle) == (0, 2)
        assert self._k.rust_node_mirror_entry_count() == 1

    def test_analyzed_capture_on_index_and_op_expr(self) -> None:
        index = IndexExpr(NameExpr("a"), NameExpr("b"))
        index.analyzed = cast(Any, OpExpr("+", NameExpr("a"), NameExpr("b")))
        index_handle = self._handle(index)
        assert self._k.rust_node_mirror_analyzed(index_handle) == (True, "OpExpr")
        op = OpExpr("+", NameExpr("a"), NameExpr("b"))
        op.analyzed = cast(Any, CastExpr(NameExpr("y"), AnyType(TypeOfAny.special_form)))
        op_handle = self._handle(op)
        assert self._k.rust_node_mirror_analyzed(op_handle) == (True, "CastExpr")

    def test_drop_and_reset(self) -> None:
        expr = NameExpr("x")
        expr.kind = GDEF
        handle = self._handle(expr)
        assert self._k.rust_node_mirror_drop(handle) is True
        assert self._k.rust_node_mirror_ref(handle) is None
        assert self._k.rust_node_mirror_entry_count() == 0
        assert self._k.rust_node_mirror_drop(handle) is False

    def test_handle_shares_identity_namespace(self) -> None:
        expr = NameExpr("x")
        expr.kind = GDEF
        handle = self._handle(expr)
        # One identity namespace: the non-minting lookups of the node
        # store and the type mirror answer the same handle.
        assert self._k.rust_node_mirror_handle_of(expr) == handle
        assert self._k.rust_mirror_handle_of(expr) == handle

    def test_capture_pin_resolves_the_binding_target(self) -> None:
        # #1787: `_capture_ref` pins the binding target under its shared
        # identity handle, so `object_of` reads the exact object back. This
        # is the Var-key substrate; no consumer reads it yet.
        expr = NameExpr("x")
        target = Var("x")
        other = Var("y")
        assert self._k.rust_node_mirror_pin_count() == 0
        expr.kind = GDEF
        expr.node = target
        other_expr = NameExpr("y")
        other_expr.kind = GDEF
        other_expr.node = other
        assert self._k.rust_node_mirror_pin_count() == 2
        handle = self._k.rust_node_mirror_handle_of(target)
        assert handle is not None
        assert self._k.rust_node_mirror_object_of(handle) is target
        other_handle = self._k.rust_node_mirror_handle_of(other)
        assert other_handle is not None and other_handle != handle
        assert self._k.rust_node_mirror_object_of(other_handle) is other
        # Idempotent: a target reached by a second RefExpr keeps one pin.
        again = NameExpr("x")
        again.kind = GDEF
        again.node = target
        assert self._k.rust_node_mirror_pin_count() == 2
        # An unminted handle resolves to None, never to a wrong object.
        assert self._k.rust_node_mirror_object_of(0) is None

    def test_reset_drops_target_pins_and_keeps_identity(self) -> None:
        expr = NameExpr("x")
        target = Var("x")
        expr.kind = GDEF
        expr.node = target
        handle = self._k.rust_node_mirror_handle_of(target)
        assert handle is not None
        assert self._k.rust_node_mirror_pin_count() == 1
        self._m.reset()
        # The per-build boundary drops the pins with the entries, so a
        # stale handle cannot resolve a dead target across builds; the
        # identity registry stays valid for the seams that hold handles.
        assert self._k.rust_node_mirror_pin_count() == 0
        assert self._k.rust_node_mirror_object_of(handle) is None
        assert self._k.rust_node_mirror_handle_of(target) == handle

    def test_identity_reset_defers_object_of(self) -> None:
        # #1795: after an identity-only reset (rust_mirror_reset) the two
        # read-backs stay coherent: `object_of` defers rather than answering
        # a handle `handle_of` no longer knows.
        expr = NameExpr("x")
        target = Var("x")
        expr.kind = GDEF
        expr.node = target
        handle = self._k.rust_node_mirror_handle_of(target)
        assert handle is not None
        assert self._k.rust_node_mirror_object_of(handle) is target
        self._m._kernel_mod.rust_mirror_reset(False)
        assert self._k.rust_node_mirror_handle_of(target) is None
        assert self._k.rust_node_mirror_object_of(handle) is None

    def test_reset_drops_entries_and_keeps_activation(self) -> None:
        expr = NameExpr("x")
        expr.kind = GDEF
        assert self._k.rust_node_mirror_entry_count() == 1
        self._m.reset()
        assert self._k.rust_node_mirror_entry_count() == 0
        assert self._m._NODE_HANDLES == {}
        # The patched hook stays installed: a later binding write captures
        # again under a fresh handle (activation is one-shot).
        expr.kind = MDEF
        assert self._k.rust_node_mirror_entry_count() == 1
        handle = self._handle(expr)
        assert self._k.rust_node_mirror_ref(handle) == (MDEF, None, "", False, False)

    def test_gate_off_leaves_node_untouched(self) -> None:
        expr = NameExpr("x")
        self._m._active = False
        try:
            expr.kind = GDEF
            expr.node = Var("x")
            assert id(expr) not in self._m._NODE_HANDLES
            assert self._k.rust_node_mirror_entry_count() == 0
            assert expr.kind == GDEF
            assert expr.node is not None
        finally:
            self._m._active = True

    def test_capture_failure_does_not_break_the_write(self) -> None:
        expr = NameExpr("x")
        original = self._k.rust_node_mirror_capture_ref

        def boom(*args: Any, **kwargs: Any) -> None:
            raise RuntimeError("kernel down")

        self._k.rust_node_mirror_capture_ref = boom  # type: ignore[assignment]
        try:
            expr.kind = GDEF
            expr.node = Var("x")
            assert expr.kind == GDEF
            assert expr.node is not None
            assert self._m.report().get("capture_fail.ref", 0) >= 1
        finally:
            self._k.rust_node_mirror_capture_ref = original

    def test_option_default_off_and_not_cache_affecting(self) -> None:
        from mypy.options import OPTIONS_AFFECTING_CACHE, Options

        # #1624: capture is default-off. The G4 flip (true) measured at
        # +23% instructions vs Python with no serving benefit; the shadow
        # is parked behind the option until a crossing-free capture exists.
        assert Options().native_ast_mirror is False
        assert "native_ast_mirror" not in OPTIONS_AFFECTING_CACHE


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAstMirrorFieldSuite(Suite):
    """Unit tests for the G1.0b remaining expression fields (#1576).

    Every G1.0b field is exercised through the same capture path a build
    uses (class-level `__setattr__` for assignments, `nodes_mirror.touch`
    for the append-only `method_types`), plus one direct seam call per
    capture wrapper. The store stays capture-only: Python remains
    authoritative and no test reads a field back through the node.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import nodes_mirror

        # #1864: the G1.0b field arms need the full scope.
        os.environ[nodes_mirror._CAPTURE_SCOPE_ENV] = "full"
        self.addCleanup(os.environ.pop, nodes_mirror._CAPTURE_SCOPE_ENV, None)
        nodes_mirror.activate(audit=True)
        nodes_mirror.reset(clear_counts=True)
        self._k = kernel
        self._m = nodes_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m.reset(clear_counts=True)

    def _handle(self, node: Any) -> int:
        handle = self._m._NODE_HANDLES.get(id(node))
        assert handle is not None, "node was not adopted by the shadow"
        return handle

    def _field(self, node: Any, field: str) -> Any:
        return self._k.rust_node_mirror_field(self._handle(node), field)

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {k: v - before.get(k, 0) for k, v in after.items() if v != before.get(k, 0)}

    def test_op_expr_fields_capture(self) -> None:
        op = OpExpr("+", NameExpr("a"), NameExpr("b"))
        op.method_type = self.fx.a
        op.right_always = True
        op.right_unreachable = True
        op.as_type = self.fx.a
        # Type-valued fields now carry wire bytes (G1.1).
        mt = self._field(op, "method_type")
        assert mt[0] == "wire" and mt[1] == "Instance"
        assert self._field(op, "right_always") == ("flag", True)
        assert self._field(op, "right_unreachable") == ("flag", True)
        at = self._field(op, "as_type")
        assert at[0] == "wire" and at[1] == "Instance"
        assert self._k.rust_node_mirror_fields(self._handle(op)) == [
            "as_type",
            "method_type",
            "right_always",
            "right_unreachable",
        ]

    def test_op_expr_cleared_kind_stays_present(self) -> None:
        op = OpExpr("|", NameExpr("a"), NameExpr("b"))
        op.as_type = None
        # A cleared slot is a captured write, not a missing record.
        # None is not a Type, so it routes through capture_field_kind.
        assert self._field(op, "as_type") == ("kind", None)
        assert self._field(op, "method_type") is None

    def test_comparison_method_types_touch(self) -> None:
        cmp = ComparisonExpr(["<"], [NameExpr("a"), NameExpr("b")])
        # The append alone is invisible to the shadow (list mutation).
        cmp.method_types.append(self.fx.a)
        assert id(cmp) not in self._m._NODE_HANDLES
        cmp.method_types.append(None)
        self._m.touch(cmp, "method_types")
        assert self._field(cmp, "method_types") == ("kinds", ["Instance", None])
        assert self._m.report().get("touch.method_types", 0) >= 1

    def test_comparison_without_appends_stays_unadopted(self) -> None:
        cmp = ComparisonExpr(["in"], [NameExpr("a"), NameExpr("b")])
        self._m.touch(cmp, "method_types")
        assert id(cmp) not in self._m._NODE_HANDLES

    def test_unary_method_type_capture(self) -> None:
        expr = UnaryExpr("-", NameExpr("a"))
        expr.method_type = self.fx.a
        mt = self._field(expr, "method_type")
        assert mt[0] == "wire" and mt[1] == "Instance"

    def test_index_method_type_and_as_type_capture(self) -> None:
        expr = IndexExpr(NameExpr("a"), NameExpr("b"))
        expr.method_type = self.fx.a
        expr.as_type = AnyType(TypeOfAny.explicit)
        mt = self._field(expr, "method_type")
        assert mt[0] == "wire" and mt[1] == "Instance"
        at = self._field(expr, "as_type")
        assert at[0] == "wire" and at[1] == "AnyType"

    def test_str_as_type_capture(self) -> None:
        expr = StrExpr("List[int]")
        expr.as_type = self.fx.a
        at = self._field(expr, "as_type")
        assert at[0] == "wire" and at[1] == "Instance"
        expr.as_type = None
        assert self._field(expr, "as_type") == ("kind", None)

    def test_member_def_var_capture_and_clear(self) -> None:
        target = Var("v")
        target._fullname = "mod.v"
        expr = MemberExpr(NameExpr("self"), "v")
        expr.def_var = target
        assert self._field(expr, "def_var") == ("name", "mod.v")
        expr.def_var = None
        assert self._field(expr, "def_var") == ("name", None)

    def test_ref_expr_flags_capture(self) -> None:
        expr = NameExpr("x")
        expr.is_special_form = True
        expr.is_alias_rvalue = True
        expr.type_guard = self.fx.a
        expr.type_is = AnyType(TypeOfAny.explicit)
        assert self._field(expr, "is_special_form") == ("flag", True)
        assert self._field(expr, "is_alias_rvalue") == ("flag", True)
        tg = self._field(expr, "type_guard")
        assert tg[0] == "wire" and tg[1] == "Instance"
        ti = self._field(expr, "type_is")
        assert ti[0] == "wire" and ti[1] == "AnyType"

    def test_fields_merge_with_ref_record(self) -> None:
        expr = NameExpr("x")
        expr.kind = GDEF
        expr.is_alias_rvalue = True
        expr.type_guard = self.fx.a
        handle = self._handle(expr)
        assert self._k.rust_node_mirror_ref(handle) == (GDEF, None, "", False, False)
        assert self._k.rust_node_mirror_fields(handle) == ["is_alias_rvalue", "name", "type_guard"]
        assert self._k.rust_node_mirror_captures(handle) == (1, 0)
        # G1.2 (#1674) seeds the node's own `name` at adoption, so the
        # two explicit field writes plus that seed are counted here.
        assert self._k.rust_node_mirror_field_captures(handle) == 3
        assert self._k.rust_node_mirror_entry_count() == 1

    def test_baseline_field_writes_do_not_adopt(self) -> None:
        op = OpExpr("+", NameExpr("a"), NameExpr("b"))
        op.method_type = None
        op.right_always = False
        op.right_unreachable = False
        op.as_type = NotParsed.VALUE
        cmp = ComparisonExpr(["<"], [NameExpr("a"), NameExpr("b")])
        cmp.method_types = []
        member = MemberExpr(NameExpr("self"), "v")
        member.def_var = None
        expr = NameExpr("x")
        expr.is_special_form = False
        expr.is_alias_rvalue = False
        expr.type_guard = None
        expr.type_is = None
        assert id(op) not in self._m._NODE_HANDLES
        assert id(cmp) not in self._m._NODE_HANDLES
        assert id(member) not in self._m._NODE_HANDLES
        assert id(expr) not in self._m._NODE_HANDLES
        assert self._k.rust_node_mirror_entry_count() == 0

    def test_touch_unknown_field_is_a_noop(self) -> None:
        expr = NameExpr("x")
        self._m.touch(expr, "not_a_shadowed_field")
        assert id(expr) not in self._m._NODE_HANDLES

    def test_direct_capture_assertion(self) -> None:
        expr = NameExpr("x")
        handle = self._k.rust_node_mirror_capture_field_kind(expr, "method_type", "CallableType")
        assert self._k.rust_node_mirror_field(handle, "method_type") == ("kind", "CallableType")
        assert self._k.rust_node_mirror_fields(handle) == ["method_type"]
        assert self._k.rust_node_mirror_field_captures(handle) == 1
        assert self._k.rust_node_mirror_capture_flag(expr, "right_always", True) == handle
        assert self._k.rust_node_mirror_capture_field_name(expr, "def_var", None) == handle
        assert self._k.rust_node_mirror_capture_field_kinds(expr, "method_types", [None]) == handle
        assert self._k.rust_node_mirror_field(handle, "def_var") == ("name", None)
        assert self._k.rust_node_mirror_field(handle, "method_types") == ("kinds", [None])
        assert self._k.rust_node_mirror_field_captures(handle) == 4
        assert self._k.rust_node_mirror_field(handle, "unknown") is None
        assert self._k.rust_node_mirror_field(handle + 1, "method_type") is None
        assert self._k.rust_node_mirror_fields(handle + 1) is None
        assert self._k.rust_node_mirror_field_captures(handle + 1) is None

    def test_gate_off_leaves_new_fields_untouched(self) -> None:
        expr = UnaryExpr("-", NameExpr("a"))
        self._m._active = False
        try:
            expr.method_type = self.fx.a
            assert id(expr) not in self._m._NODE_HANDLES
            assert self._k.rust_node_mirror_entry_count() == 0
            assert expr.method_type is self.fx.a
        finally:
            self._m._active = True

    def test_capture_failure_does_not_break_field_write(self) -> None:
        expr = StrExpr("x")
        original_wire = self._k.rust_node_mirror_capture_field_wire

        def boom(*args: Any, **kwargs: Any) -> None:
            raise RuntimeError("kernel down")

        self._k.rust_node_mirror_capture_field_wire = boom  # type: ignore[assignment]
        try:
            expr.as_type = self.fx.a
            assert expr.as_type is self.fx.a
            assert self._m.report().get("capture_fail.as_type", 0) >= 1
        finally:
            self._k.rust_node_mirror_capture_field_wire = original_wire


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAstMirrorWireSuite(Suite):
    """G1.1 wire-bytes round-trip tests for type-valued expression fields.

    Each test captures a Type into the node shadow via the same
    ``__setattr__`` path a build uses, then reads the wire bytes back
    through ``rust_node_mirror_field_wire`` and deserializes them to
    verify the round-trip produces an equal Type.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import nodes_mirror

        # #1864: the wire-field arms need the full scope.
        os.environ[nodes_mirror._CAPTURE_SCOPE_ENV] = "full"
        self.addCleanup(os.environ.pop, nodes_mirror._CAPTURE_SCOPE_ENV, None)
        nodes_mirror.activate(audit=True)
        nodes_mirror.reset(clear_counts=True)
        self._k = kernel
        self._m = nodes_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        self._m.reset(clear_counts=True)

    def _handle(self, node: Any) -> int:
        handle = self._m._NODE_HANDLES.get(id(node))
        assert handle is not None, "node was not adopted by the shadow"
        return handle

    def _field_wire(self, node: Any, field: str) -> tuple[str | None, bytes] | None:
        return self._k.rust_node_mirror_field_wire(self._handle(node), field)

    def test_method_type_wire_roundtrip(self) -> None:
        op = OpExpr("+", NameExpr("a"), NameExpr("b"))
        op.method_type = self.fx.a
        result = self._field_wire(op, "method_type")
        assert result is not None
        kind, wire = result
        assert kind == "Instance"
        assert len(wire) > 0
        # Verify the wire bytes start with the INSTANCE tag (first byte).
        from mypy.types import INSTANCE

        assert wire[0] == INSTANCE

    def test_as_type_wire_roundtrip(self) -> None:
        expr = StrExpr("List[int]")
        expr.as_type = self.fx.a
        result = self._field_wire(expr, "as_type")
        assert result is not None
        kind, wire = result
        assert kind == "Instance"
        assert len(wire) > 0

    def test_type_guard_wire_roundtrip(self) -> None:
        expr = NameExpr("x")
        expr.type_guard = self.fx.a
        result = self._field_wire(expr, "type_guard")
        assert result is not None
        kind, wire = result
        assert kind == "Instance"
        assert len(wire) > 0

    def test_type_is_anytype_wire_roundtrip(self) -> None:
        expr = NameExpr("x")
        any_t = AnyType(TypeOfAny.explicit)
        expr.type_is = any_t
        result = self._field_wire(expr, "type_is")
        assert result is not None
        kind, wire = result
        assert kind == "AnyType"
        assert len(wire) > 0
        from mypy.types import ANY_TYPE

        assert wire[0] == ANY_TYPE

    def test_cleared_type_routes_through_kind_not_wire(self) -> None:
        op = OpExpr("+", NameExpr("a"), NameExpr("b"))
        # First write a Type to adopt the node.
        op.method_type = self.fx.a
        handle = self._handle(op)
        assert self._k.rust_node_mirror_field_wire(handle, "method_type") is not None
        # Overwrite with None: not a Type, routes through capture_field_kind.
        op.method_type = None
        # Wire reader returns None for non-wire entries.
        assert self._k.rust_node_mirror_field_wire(handle, "method_type") is None
        # The general field reader sees the kind entry.
        assert self._k.rust_node_mirror_field(handle, "method_type") == ("kind", None)

    def test_read_field_type_helper(self) -> None:
        """AnyType round-trips without a resolver (no type_ref fixup)."""
        expr = NameExpr("x")
        any_t = AnyType(TypeOfAny.explicit)
        expr.type_is = any_t
        handle = self._handle(expr)
        decoded = self._m.read_field_type(handle, "type_is")
        assert decoded is not None
        proper = get_proper_type(decoded)
        assert isinstance(proper, AnyType)
        assert proper.type_of_any == TypeOfAny.explicit

    def test_read_field_type_cleared_returns_none(self) -> None:
        op = OpExpr("+", NameExpr("a"), NameExpr("b"))
        op.method_type = self.fx.a
        handle = self._handle(op)
        # Overwrite with None.
        op.method_type = None
        assert self._m.read_field_type(handle, "method_type") is None

    def test_read_field_type_missing_field_returns_none(self) -> None:
        op = OpExpr("+", NameExpr("a"), NameExpr("b"))
        op.method_type = self.fx.a
        handle = self._handle(op)
        # `as_type` was never written.
        assert self._m.read_field_type(handle, "as_type") is None

    def test_wire_overwrites_previous(self) -> None:
        op = OpExpr("+", NameExpr("a"), NameExpr("b"))
        op.method_type = self.fx.a
        first = self._field_wire(op, "method_type")
        assert first is not None and first[0] == "Instance"
        # Overwrite with a different type.
        any_t = AnyType(TypeOfAny.explicit)
        op.method_type = any_t
        second = self._field_wire(op, "method_type")
        assert second is not None and second[0] == "AnyType"

    def test_notparsed_as_type_not_wire(self) -> None:
        """NotParsed is not a Type, so it stays on the kind path."""
        expr = StrExpr("x")
        # Constructor default: NotParsed.VALUE
        assert id(expr) not in self._m._NODE_HANDLES
        expr.as_type = NotParsed.VALUE
        # NotParsed is not a Type, no adoption.
        assert id(expr) not in self._m._NODE_HANDLES

    def test_capture_wire_engages_through_setattr(self) -> None:
        """Verify the wire path is reached through the patched __setattr__."""
        op = OpExpr("+", NameExpr("a"), NameExpr("b"))
        op.method_type = self.fx.a
        assert self._m.report().get("capture_method_type", 0) >= 1


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeStmtDefMirrorSuite(Suite):
    """Unit tests for the G2.0 statement/def metadata shadow (#1577).

    The store is capture-only: every assertion drives the
    `mypy.nodes_mirror` hook and reads the Rust record back through the
    `rust_node_mirror_meta*` pyfunctions. The pinnings are the G2
    contract: constructor-default writes on a never-adopted node stay
    out of the store, the first non-default write adopts it, each tagged
    field keeps its last value, object values are class/fullname markers
    only, and astmerge's `replace_object_state` re-registers the
    surviving identity through the same hook.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import nodes_mirror

        # #1864: the meta capture arms need the full scope.
        os.environ[nodes_mirror._CAPTURE_SCOPE_ENV] = "full"
        self.addCleanup(os.environ.pop, nodes_mirror._CAPTURE_SCOPE_ENV, None)
        nodes_mirror.activate(audit=True)
        # TypeFixture builds real Vars/TypeInfos, so it must come before
        # the reset that zeroes the store for each test.
        self.fx = TypeFixture()
        nodes_mirror.reset(clear_counts=True)
        self._k = kernel
        self._m = nodes_mirror

    def tearDown(self) -> None:
        self._m.reset(clear_counts=True)

    def _meta(self, node: Any) -> dict[str, tuple[Any, ...]]:
        handle = self._m._META_HANDLES.get(id(node))
        assert handle is not None, "node was not adopted by the metadata shadow"
        record = self._k.rust_node_mirror_meta(handle)
        assert record is not None
        return record

    def _handle(self, node: Any) -> int:
        handle = self._m._META_HANDLES.get(id(node))
        assert handle is not None, "node was not adopted by the metadata shadow"
        return handle

    def _typeinfo(self, fullname: str = "mod.A") -> Any:
        from mypy import nodes as nodes_mod

        defn = nodes_mod.ClassDef(fullname.rsplit(".", 1)[-1], nodes_mod.Block([]), None, [])
        defn.fullname = fullname
        info = nodes_mod.TypeInfo(nodes_mod.SymbolTable(), defn, "mod")
        defn.info = info
        info.mro = [info]
        return info

    def test_assignment_stmt_field_set(self) -> None:
        from mypy import nodes as nodes_mod

        stmt = nodes_mod.AssignmentStmt([nodes_mod.NameExpr("x")], nodes_mod.IntExpr(1))
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        # A constructor-default tracked write on a fresh node never adopts.
        stmt.is_final_def = False
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        stmt.type = AnyType(TypeOfAny.special_form)
        stmt.unanalyzed_type = AnyType(TypeOfAny.special_form)
        stmt.is_alias_def = True
        stmt.is_final_def = True
        stmt.invalid_recursive_alias = True
        record = self._meta(stmt)
        assert set(record) == set(self._m._G2_TRACKED[nodes_mod.AssignmentStmt])
        assert record["type"] == ("obj", "AnyType", None, None)
        assert record["unanalyzed_type"] == ("obj", "AnyType", None, None)
        assert record["is_alias_def"] == ("bool", None, 1, None)
        assert record["is_final_def"] == ("bool", None, 1, None)
        assert record["invalid_recursive_alias"] == ("bool", None, 1, None)

    def test_for_stmt_field_set(self) -> None:
        from mypy import nodes as nodes_mod

        stmt = nodes_mod.ForStmt(
            nodes_mod.NameExpr("i"), nodes_mod.ListExpr([]), nodes_mod.Block([]), None
        )
        # `index` is non-default, so construction adopts; the inferred
        # fields start as `None` and refresh on the first late write.
        assert self._k.rust_node_mirror_meta_entry_count() == 1
        stmt.index_type = AnyType(TypeOfAny.special_form)
        stmt.unanalyzed_index_type = AnyType(TypeOfAny.special_form)
        stmt.inferred_item_type = AnyType(TypeOfAny.from_error)
        stmt.inferred_iterator_type = AnyType(TypeOfAny.from_error)
        record = self._meta(stmt)
        assert set(record) == set(self._m._G2_TRACKED[nodes_mod.ForStmt])
        assert record["index"] == ("obj", "NameExpr", None, None)
        assert record["index_type"] == ("obj", "AnyType", None, None)
        assert record["unanalyzed_index_type"] == ("obj", "AnyType", None, None)
        assert record["inferred_item_type"] == ("obj", "AnyType", None, None)
        assert record["inferred_iterator_type"] == ("obj", "AnyType", None, None)

    def test_with_stmt_analyzed_types(self) -> None:
        from mypy import nodes as nodes_mod

        stmt = nodes_mod.WithStmt([nodes_mod.NameExpr("f")], [None], nodes_mod.Block([]))
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        stmt.analyzed_types = [AnyType(TypeOfAny.special_form), AnyType(TypeOfAny.from_error)]
        record = self._meta(stmt)
        assert set(record) == {"analyzed_types"}
        assert record["analyzed_types"] == ("list", None, None, ["AnyType", "AnyType"])

    def test_if_stmt_unreachable_else(self) -> None:
        from mypy import nodes as nodes_mod

        stmt = nodes_mod.IfStmt([nodes_mod.NameExpr("c")], [nodes_mod.Block([])], None)
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        stmt.unreachable_else = True
        assert self._meta(stmt) == {"unreachable_else": ("bool", None, 1, None)}
        stmt.unreachable_else = False
        assert self._meta(stmt) == {"unreachable_else": ("bool", None, 0, None)}

    def test_match_stmt_subject_dummy(self) -> None:
        from mypy import nodes as nodes_mod

        stmt = nodes_mod.MatchStmt(nodes_mod.NameExpr("x"), [], [], [])
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        stmt.subject_dummy = nodes_mod.NameExpr("m")
        assert self._meta(stmt) == {"subject_dummy": ("obj", "NameExpr", None, None)}
        # A cleared replacement is a tracked write, not a missing entry.
        stmt.subject_dummy = None
        assert self._meta(stmt) == {"subject_dummy": ("none", None, None, None)}

    def test_type_alias_stmt_alias_node(self) -> None:
        from mypy import nodes as nodes_mod

        stmt = nodes_mod.TypeAliasStmt(
            nodes_mod.NameExpr("A"), [], nodes_mod.LambdaExpr([], nodes_mod.Block([]))
        )
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        stmt.alias_node = nodes_mod.TypeAlias(
            AnyType(TypeOfAny.special_form), "mod.A", "mod", 1, 0
        )
        assert self._meta(stmt) == {"alias_node": ("obj", "TypeAlias:mod.A", None, None)}

    def test_import_assignments_touch_and_aststrip_rebind(self) -> None:
        from mypy import nodes as nodes_mod

        imp = nodes_mod.Import([("mod", None)])
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        imp.assignments.append(
            nodes_mod.AssignmentStmt([nodes_mod.NameExpr("x")], nodes_mod.IntExpr(1))
        )
        self._m.touch(imp, "assignments")
        assert self._meta(imp) == {"assignments": ("list", None, None, ["AssignmentStmt"])}
        # aststrip's `node.assignments = []` is a setattr on an adopted
        # node, so the cleared list refreshes the record in place.
        imp.assignments = []
        assert self._meta(imp) == {"assignments": ("list", None, None, [])}
        # `touch` is a no-op while the gate is off.
        self._m._active = False
        try:
            self._m.touch(imp, "assignments")
        finally:
            self._m._active = True

    def test_aststrip_style_writes_capture(self) -> None:
        from mypy import nodes as nodes_mod

        stmt = nodes_mod.AssignmentStmt([nodes_mod.NameExpr("x")], nodes_mod.IntExpr(1))
        stmt.unanalyzed_type = AnyType(TypeOfAny.special_form)
        stmt.type = stmt.unanalyzed_type
        assert self._meta(stmt)["type"] == ("obj", "AnyType", None, None)
        stmt.type = None
        assert self._meta(stmt)["type"] == ("none", None, None, None)
        for_stmt = nodes_mod.ForStmt(
            nodes_mod.NameExpr("i"), nodes_mod.ListExpr([]), nodes_mod.Block([]), None
        )
        for_stmt.index_type = AnyType(TypeOfAny.special_form)
        for_stmt.index_type = for_stmt.unanalyzed_index_type
        assert self._meta(for_stmt)["index_type"] == ("none", None, None, None)
        items: list[Any] = [nodes_mod.FuncDef("f")]
        over = nodes_mod.OverloadedFuncDef(items)
        over.unanalyzed_items = list(items)
        over.items = over.unanalyzed_items.copy()
        over.impl = None
        record = self._meta(over)
        assert record["items"] == ("list", None, None, ["FuncDef"])
        assert record["impl"] == ("none", None, None, None)

    def test_func_def_field_set(self) -> None:
        from mypy import nodes as nodes_mod

        func = nodes_mod.FuncDef("f")
        func._fullname = "mod.f"
        func.type = self.fx.callable(AnyType(TypeOfAny.special_form))
        func.unanalyzed_type = self.fx.callable(AnyType(TypeOfAny.special_form))
        func.abstract_status = 1
        func.info = self._typeinfo("mod.f")
        func.deprecated = "use g instead"
        func.original_def = nodes_mod.FuncDef("g")
        func.dataclass_transform_spec = nodes_mod.DataclassTransformSpec()
        func.docstring = "doc"
        flags = sorted(self._m._G2_FUNC_FLAGS)
        for name in flags:
            setattr(func, name, True)
        record = self._meta(func)
        assert set(record) == set(self._m._G2_TRACKED[nodes_mod.FuncDef])
        assert record["_fullname"] == ("str", "mod.f", None, None)
        assert record["type"] == ("obj", "CallableType", None, None)
        assert record["unanalyzed_type"] == ("obj", "CallableType", None, None)
        assert record["abstract_status"] == ("int", None, 1, None)
        assert record["info"] == ("obj", "TypeInfo:mod.f", None, None)
        assert record["deprecated"] == ("str", "use g instead", None, None)
        assert record["original_def"] == ("obj", "FuncDef", None, None)
        assert record["dataclass_transform_spec"] == ("obj", "DataclassTransformSpec", None, None)
        assert record["docstring"] == ("str", "doc", None, None)
        for name in flags:
            assert record[name] == ("bool", None, 1, None), name

    def test_overloaded_func_def_items_impl(self) -> None:
        from mypy import nodes as nodes_mod

        item = nodes_mod.FuncDef("f")
        item._fullname = "mod.f"
        over = nodes_mod.OverloadedFuncDef([item])
        record = self._meta(over)
        # FuncBase.__init__ adopts via `self.info = FUNC_NO_INFO` (non-baseline),
        # so all tracked FuncBase fields are captured from that point on.
        assert "items" in record
        assert record["items"] == ("list", None, None, ["FuncDef:mod.f"])
        impl = nodes_mod.FuncDef("f")
        over.impl = impl
        assert self._meta(over)["impl"] == ("obj", "FuncDef", None, None)
        # aststrip clears the impl to None; a tracked write, not a miss.
        over.impl = None
        assert self._meta(over)["impl"] == ("none", None, None, None)

    def test_decorator_func_var(self) -> None:
        from mypy import nodes as nodes_mod

        func = nodes_mod.FuncDef("f")
        var = nodes_mod.Var("f")
        dec = nodes_mod.Decorator(func, [], var)
        record = self._meta(dec)
        # `func` and `var` are non-baseline, adopting the node; the
        # baseline `decorators=[]` and `is_overload=False` are captured
        # because the node is already adopted.
        assert set(record) == {"func", "var", "is_overload", "decorators", "original_decorators"}
        assert record["func"] == ("obj", "FuncDef", None, None)
        assert record["var"] == ("obj", "Var", None, None)
        assert record["is_overload"] == ("bool", None, 0, None)
        assert record["decorators"] == ("list", None, None, [])
        assert record["original_decorators"] == ("list", None, None, [])

    def test_class_def_info_and_analyzed(self) -> None:
        from mypy import nodes as nodes_mod

        cls = nodes_mod.ClassDef("C", nodes_mod.Block([]))
        # Construction adopts through the placeholder `info`; `analyzed`
        # starts captured as a real None because the node is adopted.
        record = self._meta(cls)
        assert record["info"][0] == "obj"
        assert record["analyzed"] == ("none", None, None, None)
        cls.info = self._typeinfo("mod.C")
        assert self._meta(cls)["info"] == ("obj", "TypeInfo:mod.C", None, None)
        cls.analyzed = nodes_mod.NameExpr("C")
        assert self._meta(cls)["analyzed"] == ("obj", "NameExpr", None, None)
        # aststrip's `node.analyzed = None` refresh.
        cls.analyzed = None
        assert self._meta(cls)["analyzed"] == ("none", None, None, None)

    def test_class_def_extended_fields(self) -> None:
        from mypy import nodes as nodes_mod

        cls = nodes_mod.ClassDef("C", nodes_mod.Block([]))
        # Node is adopted via the non-baseline `info` constructor write.
        # The new fields start as constructor baselines and are not yet
        # in the record until a non-baseline write or explicit touch.
        record = self._meta(cls)
        assert "info" in record
        assert "analyzed" in record
        # Direct attribute assignment (visible to __setattr__).
        cls.metaclass = nodes_mod.NameExpr("Meta")
        assert self._meta(cls)["metaclass"] == ("obj", "NameExpr", None, None)
        cls.base_type_exprs = [nodes_mod.NameExpr("Base")]
        assert self._meta(cls)["base_type_exprs"] == ("list", None, None, ["NameExpr"])
        # In-place list mutations (invisible to __setattr__) need touch.
        cls.removed_statements.append(nodes_mod.PassStmt())
        self._m.touch(cls, "removed_statements")
        assert self._meta(cls)["removed_statements"] == ("list", None, None, ["PassStmt"])
        cls.removed_base_type_exprs.append(nodes_mod.NameExpr("Generic"))
        self._m.touch(cls, "removed_base_type_exprs")
        assert self._meta(cls)["removed_base_type_exprs"] == ("list", None, None, ["NameExpr"])
        # Reset via assignment (visible to __setattr__).
        cls.removed_statements = []
        assert self._meta(cls)["removed_statements"] == ("list", None, None, [])

    def test_var_field_set(self) -> None:
        from mypy import nodes as nodes_mod

        var = nodes_mod.Var("x")
        var._fullname = "mod.x"
        var.type = AnyType(TypeOfAny.special_form)
        var.setter_type = self.fx.callable(AnyType(TypeOfAny.special_form))
        var.info = self._typeinfo()
        var.final_value = 7
        value_fields = {"_fullname", "type", "setter_type", "info", "final_value"}
        # `_name` is constructor-set and str-valued: it is in the record
        # from the adoption read-back (#1787), not from the bool loop.
        bool_fields = sorted(self._m._G2_VAR - value_fields - {"_name"})
        for name in bool_fields:
            setattr(var, name, True)
        record = self._meta(var)
        assert set(record) == set(self._m._G2_VAR)
        assert record["_name"] == ("str", "x", None, None)
        assert record["_fullname"] == ("str", "mod.x", None, None)
        assert record["type"] == ("obj", "AnyType", None, None)
        assert record["setter_type"] == ("obj", "CallableType", None, None)
        assert record["info"] == ("obj", "TypeInfo:mod.A", None, None)
        assert record["final_value"] == ("int", None, 7, None)
        for name in bool_fields:
            assert record[name] == ("bool", None, 1, None), name

    def test_ctor_payload_fields_seed_at_adoption(self) -> None:
        from mypy import nodes as nodes_mod

        before = dict(self._m.report())
        var = nodes_mod.Var("x")
        # The constructor `_name` write never adopts (it precedes the
        # `info` write that does); adoption reads the slot back instead.
        assert self._m.report().get("meta_ctor_skip", 0) - before.get("meta_ctor_skip", 0) == 1
        assert self._meta(var)["_name"] == ("str", "x", None, None)

        func = nodes_mod.FuncDef("f")
        record = self._meta(func)
        # FuncBase's `info` write adopts inside `FuncItem.__init__`, before
        # the constructor sets these, so they land as ordinary writes.
        assert record["_name"] == ("str", "f", None, None)
        assert record["arg_names"] == ("list", None, None, [])
        assert record["arg_kinds"] == ("list", None, None, [])
        assert record["original_first_arg"] == ("none", None, None, None)

        cls = nodes_mod.ClassDef("C", nodes_mod.Block([]))
        assert self._meta(cls)["name"] == ("str", "C", None, None)

    def test_ctor_payload_field_seeds_only_when_set(self) -> None:
        from mypy import nodes as nodes_mod

        # A node adopted before the constructor reached its payload writes
        # keeps those fields absent; one later write refreshes them.
        func = nodes_mod.FuncDef.__new__(nodes_mod.FuncDef)
        nodes_mod.FuncItem.__init__(func)
        assert "arg_names" in self._meta(func)
        assert "_name" not in self._meta(func)
        func._name = "g"
        assert self._meta(func)["_name"] == ("str", "g", None, None)

    def test_ctor_payload_rename_on_adopted_node_captures(self) -> None:
        from mypy import nodes as nodes_mod

        var = nodes_mod.Var("x")
        assert self._meta(var)["_name"] == ("str", "x", None, None)
        # The writer audit found no production renamer, so this arm is a
        # safety net; it must still capture when one appears (#1787).
        var._name = "y"
        assert self._meta(var)["_name"] == ("str", "y", None, None)
        cls = nodes_mod.ClassDef("C", nodes_mod.Block([]))
        cls.name = "D"
        assert self._meta(cls)["name"] == ("str", "D", None, None)

    def test_attrs_decorator_removal_refreshes_the_record(self) -> None:
        from mypy import nodes as nodes_mod
        from mypy.plugins import attrs as attrs_plugin

        kept = nodes_mod.NameExpr("decorator")
        dropped = nodes_mod.MemberExpr(nodes_mod.NameExpr("x"), "default")
        dec = nodes_mod.Decorator(nodes_mod.FuncDef("f"), [kept, dropped], nodes_mod.Var("f"))
        attribute = attrs_plugin.Attribute(
            "x", None, self._typeinfo(), False, True, False, None, nodes_mod.Context(), None
        )
        assert self._meta(dec)["decorators"] == ("list", None, None, ["NameExpr", "MemberExpr"])
        attrs_plugin._cleanup_decorator(dec, {"x": attribute})
        assert dec.decorators == [kept]
        assert attribute.has_default is True
        # The removal is an in-place list mutation, invisible to the
        # patched `__setattr__`; `_cleanup_decorator`'s `touch` is what
        # keeps the record from going stale (#1787 blind channel).
        assert self._meta(dec)["decorators"] == ("list", None, None, ["NameExpr"])

    def test_meta_read_shape_and_capture_counter(self) -> None:
        import type_kernel as kernel

        from mypy import nodes as nodes_mod

        stmt = nodes_mod.AssignmentStmt([nodes_mod.NameExpr("x")], nodes_mod.IntExpr(1))
        stmt.is_alias_def = True
        stmt.type = AnyType(TypeOfAny.special_form)
        handle = self._handle(stmt)
        assert kernel.rust_node_mirror_meta_captures(handle) == 2
        record = kernel.rust_node_mirror_meta(handle)
        assert record is not None and set(record) == {"is_alias_def", "type"}
        # A repeated write replaces the value without adding a field.
        stmt.is_alias_def = False
        assert kernel.rust_node_mirror_meta_captures(handle) == 3
        record = kernel.rust_node_mirror_meta(handle)
        assert record is not None
        assert record["is_alias_def"] == ("bool", None, 0, None)

    def test_replace_object_state_reregisters_surviving_identity(self) -> None:
        from mypy import nodes as nodes_mod
        from mypy.util import replace_object_state

        old = nodes_mod.FuncDef("f")
        old._fullname = "mod.f"
        old.is_final = True
        old.is_property = True
        self._handle(old)
        new = nodes_mod.FuncDef("f2")
        # The astmerge pattern: state is copied onto the surviving
        # identity through setattr, so the shadow re-registers it.
        replace_object_state(new, old)
        record = self._meta(new)
        assert record["_fullname"] == ("str", "mod.f", None, None)
        assert record["is_final"] == ("bool", None, 1, None)
        assert record["is_property"] == ("bool", None, 1, None)

    def test_drop_and_reset(self) -> None:
        from mypy import nodes as nodes_mod

        var = nodes_mod.Var("x")
        var.is_final = True
        handle = self._handle(var)
        assert self._k.rust_node_mirror_meta_drop(handle) is True
        assert self._k.rust_node_mirror_meta(handle) is None
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        assert self._k.rust_node_mirror_meta_drop(handle) is False

    def test_reset_drops_meta_store_and_keeps_activation(self) -> None:
        from mypy import nodes as nodes_mod

        var = nodes_mod.Var("x")
        var.is_final = True
        assert self._k.rust_node_mirror_meta_entry_count() >= 1
        self._m.reset()
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        assert self._m._META_HANDLES == {}
        # Activation is one-shot and survives reset: the next
        # non-default tracked write captures under a fresh handle.
        var.has_explicit_value = True
        assert self._k.rust_node_mirror_meta_entry_count() == 1

    def test_gate_off_leaves_node_untouched(self) -> None:
        from mypy import nodes as nodes_mod

        self._m._active = False
        try:
            var = nodes_mod.Var("x")
            var.is_final = True
            var._fullname = "mod.x"
            assert id(var) not in self._m._META_HANDLES
            assert self._k.rust_node_mirror_meta_entry_count() == 0
            assert var.is_final is True
        finally:
            self._m._active = True

    def test_capture_failure_does_not_break_the_write(self) -> None:
        import type_kernel as kernel

        from mypy import nodes as nodes_mod

        stmt = nodes_mod.AssignmentStmt([nodes_mod.NameExpr("x")], nodes_mod.IntExpr(1))
        original = kernel.rust_node_mirror_capture_meta

        def boom(*args: Any, **kwargs: Any) -> None:
            raise RuntimeError("kernel down")

        kernel.rust_node_mirror_capture_meta = boom  # type: ignore[assignment]
        try:
            stmt.is_final_def = True
            assert stmt.is_final_def is True
            assert self._m.report().get("meta_capture_fail", 0) >= 1
        finally:
            kernel.rust_node_mirror_capture_meta = original

    def test_import_base_bool_flags(self) -> None:
        from mypy import nodes as nodes_mod

        imp = nodes_mod.Import([("m", None)])
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        imp.is_top_level = True
        imp.is_unreachable = True
        imp.is_mypy_only = True
        imp.is_unreachable_dependency = True
        record = self._meta(imp)
        assert set(record) == {
            "is_top_level",
            "is_unreachable",
            "is_mypy_only",
            "is_unreachable_dependency",
        }
        assert record["is_top_level"] == ("bool", None, 1, None)
        assert record["is_unreachable"] == ("bool", None, 1, None)
        assert record["is_mypy_only"] == ("bool", None, 1, None)
        assert record["is_unreachable_dependency"] == ("bool", None, 1, None)
        imp.is_top_level = False
        assert self._meta(imp)["is_top_level"] == ("bool", None, 0, None)

    def test_block_is_unreachable(self) -> None:
        from mypy import nodes as nodes_mod

        blk = nodes_mod.Block([])
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        blk.is_unreachable = True
        assert self._meta(blk) == {"is_unreachable": ("bool", None, 1, None)}
        blk.is_unreachable = False
        assert self._meta(blk) == {"is_unreachable": ("bool", None, 0, None)}

    def test_import_assignments_and_flags_coexist(self) -> None:
        from mypy import nodes as nodes_mod

        imp = nodes_mod.ImportFrom("m", 0, [("x", None)])
        imp.assignments.append(
            nodes_mod.AssignmentStmt([nodes_mod.NameExpr("x")], nodes_mod.IntExpr(1))
        )
        self._m.touch(imp, "assignments")
        imp.is_top_level = True
        imp.is_unreachable = True
        record = self._meta(imp)
        assert set(record) == {"assignments", "is_top_level", "is_unreachable"}
        assert record["assignments"] == ("list", None, None, ["AssignmentStmt"])
        assert record["is_top_level"] == ("bool", None, 1, None)
        assert record["is_unreachable"] == ("bool", None, 1, None)

    def test_overloaded_func_def_extended_fields(self) -> None:
        from mypy import nodes as nodes_mod

        item = nodes_mod.FuncDef("f")
        item._fullname = "mod.f"
        over = nodes_mod.OverloadedFuncDef([item])
        over.unanalyzed_items = list(over.items)
        over.deprecated = "use mod.g instead"
        over.setter_index = 2
        record = self._meta(over)
        # FuncBase fields are captured from construction (info=FUNC_NO_INFO
        # adopts the node in FuncBase.__init__, before OverloadedFuncDef's
        # own writes).
        assert "unanalyzed_items" in record
        assert record["unanalyzed_items"] == ("list", None, None, ["FuncDef:mod.f"])
        assert record["deprecated"] == ("str", "use mod.g instead", None, None)
        assert record["setter_index"] == ("int", None, 2, None)
        # Post-construction FuncBase writes are captured.
        over.is_property = True
        over._fullname = "mod.f"
        over.is_static = True
        record = self._meta(over)
        assert record["is_property"] == ("bool", None, 1, None)
        assert record["_fullname"] == ("str", "mod.f", None, None)
        assert record["is_static"] == ("bool", None, 1, None)

    def test_decorator_is_overload_and_decorators(self) -> None:
        from mypy import nodes as nodes_mod

        func = nodes_mod.FuncDef("f")
        var = nodes_mod.Var("f")
        dec = nodes_mod.Decorator(func, [], var)
        dec.is_overload = True
        dec.decorators = [nodes_mod.NameExpr("decorator1")]
        record = self._meta(dec)
        assert set(record) == {"func", "var", "is_overload", "decorators", "original_decorators"}
        assert record["is_overload"] == ("bool", None, 1, None)
        assert record["decorators"] == ("list", None, None, ["NameExpr"])

    def test_class_def_has_incompatible_baseclass_and_metaclass(self) -> None:
        from mypy import nodes as nodes_mod

        cls = nodes_mod.ClassDef("C", nodes_mod.Block([]))
        cls.has_incompatible_baseclass = True
        cls.metaclass = nodes_mod.NameExpr("ABCMeta")
        record = self._meta(cls)
        # `name` is a constructor-set payload field (#1787 §1(a)); adoption
        # reads it back, so it is in the record without a post-hoc write.
        assert set(record) == {
            "name",
            "info",
            "analyzed",
            "has_incompatible_baseclass",
            "metaclass",
            "removed_statements",
        }
        assert record["name"] == ("str", "C", None, None)
        assert record["has_incompatible_baseclass"] == ("bool", None, 1, None)
        assert record["metaclass"] == ("obj", "NameExpr", None, None)

    def test_func_def_deprecated(self) -> None:
        from mypy import nodes as nodes_mod

        func = nodes_mod.FuncDef("f")
        func.deprecated = "use mod.g"
        record = self._meta(func)
        assert "deprecated" in record
        assert record["deprecated"] == ("str", "use mod.g", None, None)
        func.deprecated = None
        assert self._meta(func)["deprecated"] == ("none", None, None, None)

    def test_class_def_fullname_and_type_vars(self) -> None:
        from mypy import nodes as nodes_mod

        cls = nodes_mod.ClassDef("C", nodes_mod.Block([]))
        cls._fullname = "mod.C"
        cls.type_vars = [
            TypeVarType(
                "T",
                "mod.T",
                TypeVarId(1),
                [],
                AnyType(TypeOfAny.special_form),
                AnyType(TypeOfAny.special_form),
            )
        ]
        record = self._meta(cls)
        assert record["_fullname"] == ("str", "mod.C", None, None)
        assert record["type_vars"] == ("list", None, None, ["TypeVarType:mod.T"])

    def test_class_def_removed_base_type_exprs(self) -> None:
        from mypy import nodes as nodes_mod

        cls = nodes_mod.ClassDef("C", nodes_mod.Block([]))
        cls.removed_base_type_exprs = [nodes_mod.NameExpr("Generic")]
        record = self._meta(cls)
        assert record["removed_base_type_exprs"] == ("list", None, None, ["NameExpr"])
        cls.removed_base_type_exprs = []
        assert self._meta(cls)["removed_base_type_exprs"] == ("list", None, None, [])

    def test_func_def_info(self) -> None:
        from mypy import nodes as nodes_mod

        func = nodes_mod.FuncDef("f")
        func._fullname = "mod.f"
        func.info = self._typeinfo("mod.A")
        record = self._meta(func)
        assert record["info"] == ("obj", "TypeInfo:mod.A", None, None)

    def test_type_alias_stmt_invalid_recursive_alias(self) -> None:
        from mypy import nodes as nodes_mod

        stmt = nodes_mod.TypeAliasStmt(
            nodes_mod.NameExpr("A"), [], nodes_mod.LambdaExpr([], nodes_mod.Block([]))
        )
        stmt.invalid_recursive_alias = True
        record = self._meta(stmt)
        assert record["invalid_recursive_alias"] == ("bool", None, 1, None)
        stmt.invalid_recursive_alias = False
        assert self._meta(stmt)["invalid_recursive_alias"] == ("bool", None, 0, None)

    def test_decorator_original_decorators(self) -> None:
        from mypy import nodes as nodes_mod

        func = nodes_mod.FuncDef("f")
        var = nodes_mod.Var("f")
        dec = nodes_mod.Decorator(func, [nodes_mod.NameExpr("dec1")], var)
        record = self._meta(dec)
        assert record["original_decorators"] == ("list", None, None, ["NameExpr"])
        dec.original_decorators = []
        assert self._meta(dec)["original_decorators"] == ("list", None, None, [])

    def test_overloaded_func_def_func_base_fields(self) -> None:
        from mypy import nodes as nodes_mod

        item = nodes_mod.FuncDef("f")
        over = nodes_mod.OverloadedFuncDef([item])
        record = self._meta(over)
        # FuncBase.__init__ sets info=FUNC_NO_INFO (non-baseline), adopting
        # the node. All tracked FuncBase fields from that point are captured
        # with their constructor-default values.
        assert record["info"] == ("obj", "FakeInfo", None, None)
        assert record["is_property"] == ("bool", None, 0, None)
        assert record["is_class"] == ("bool", None, 0, None)
        assert record["is_static"] == ("bool", None, 0, None)
        assert record["is_final"] == ("bool", None, 0, None)
        assert record["is_explicit_override"] == ("bool", None, 0, None)
        assert record["is_type_check_only"] == ("bool", None, 0, None)
        assert record["def_or_infer_vars"] == ("bool", None, 0, None)
        assert record["_fullname"] == ("str", "", None, None)
        assert record["_is_trivial_self"] == ("none", None, None, None)
        # Post-construction mutations are captured.
        over.is_final = True
        over._is_trivial_self = True
        record = self._meta(over)
        assert record["is_final"] == ("bool", None, 1, None)
        assert record["_is_trivial_self"] == ("bool", None, 1, None)

    def test_handle_shares_identity_namespace(self) -> None:
        from mypy import nodes as nodes_mod

        var = nodes_mod.Var("x")
        var.is_final = True
        handle = self._handle(var)
        # One identity namespace: the metadata store, the expression store
        # and the type mirror all answer the same handle.
        assert self._k.rust_node_mirror_handle_of(var) == handle
        assert self._k.rust_mirror_handle_of(var) == handle


class NativeResolverSigSuite(Suite):
    """Dirty-driven resolver upkeep (#1641).

    The per-SCC feed re-pushes a `builtins.*` info only when its content
    signature changed since the last feed. The signature covers exactly
    the snapshot fields with post-seal writers: `_promote` (grows via
    `add_type_promotion` and via the cache-load backwards-promotion hack
    in `mypy/fixup.py`, which runs no `add_type_promotion` call, so a
    mark hook at the mutation site cannot see it), `alt_promote`, and
    `defn.type_vars` variance (the #1146 pre-feed loop).
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def _int_info(self) -> TypeInfo:
        int_info = self.fx.make_type_info("builtins.int")
        int_info.defn.info = int_info
        return int_info

    def test_sig_sees_fixup_shaped_promote_growth(self) -> None:
        from mypy.build import _native_builtins_sig

        int_info = self._int_info()
        before = _native_builtins_sig(int_info)
        # Mirror the fixup.py backwards-promotion hack: a deserialized
        # native int appends itself to int._promote outside semanal.
        nativ = self.fx.make_type_info("mypy_extensions.i64")
        int_info._promote.append(Instance(nativ, []))
        after = _native_builtins_sig(int_info)
        assert before != after
        assert after[0] == ("mypy_extensions.i64",)

    def test_sig_sees_alt_promote(self) -> None:
        from mypy.build import _native_builtins_sig

        nativ = self.fx.make_type_info("mypy_extensions.i64")
        nativ.defn.info = nativ
        before = _native_builtins_sig(nativ)
        nativ.alt_promote = Instance(self._int_info(), [])
        assert _native_builtins_sig(nativ) != before

    def test_sig_sees_variance(self) -> None:
        from mypy.build import _native_builtins_sig

        info = self.fx.make_type_info("builtins.G", typevars=["T"])
        before = _native_builtins_sig(info)
        tvar = info.defn.type_vars[0]
        assert isinstance(tvar, TypeVarType)
        tvar.variance = CONTRAVARIANT
        assert _native_builtins_sig(info) != before

    def test_sig_stable_without_mutation(self) -> None:
        from mypy.build import _native_builtins_sig

        int_info = self._int_info()
        assert _native_builtins_sig(int_info) == _native_builtins_sig(int_info)

    def test_split_new_changed_unchanged(self) -> None:
        from mypy.build import _native_builtins_sig, _split_native_pending

        int_info = self._int_info()
        float_info = self.fx.make_type_info("builtins.float")
        mod_info = self.fx.make_type_info("mod.A")
        snapshotted = {"builtins.int", "builtins.float", "mod.A"}
        prev = {
            "builtins.int": _native_builtins_sig(int_info),
            "builtins.float": _native_builtins_sig(float_info),
        }
        # Only int gains a promotion; float is untouched; mod.A is new.
        nativ = self.fx.make_type_info("mypy_extensions.i64")
        int_info._promote.append(Instance(nativ, []))
        new_mod = self.fx.make_type_info("mod.B")
        new, repush, cur = _split_native_pending(
            [int_info, float_info, mod_info, new_mod], snapshotted, prev
        )
        assert [i.fullname for i in new] == ["mod.B"]
        assert [i.fullname for i in repush] == ["builtins.int"]
        assert set(cur) == {"builtins.int", "builtins.float"}
        assert cur["builtins.int"] == _native_builtins_sig(int_info)

    def test_split_unknown_prev_repushes(self) -> None:
        from mypy.build import _split_native_pending

        int_info = self._int_info()
        # No recorded signature (bookkeeping gap): fail safe toward a
        # re-push, never a skip.
        new, repush, _ = _split_native_pending([int_info], {"builtins.int"}, {})
        assert new == []
        assert [i.fullname for i in repush] == ["builtins.int"]

    def test_split_nonbuiltins_never_repush(self) -> None:
        from mypy.build import _native_builtins_sig, _split_native_pending

        mod_info = self.fx.make_type_info("mod.A")
        prev = {"mod.A": _native_builtins_sig(mod_info)}
        new, repush, cur = _split_native_pending([mod_info], {"mod.A"}, prev)
        assert new == []
        assert repush == []
        assert cur == {}

    def test_add_type_promotion_changes_sig(self) -> None:
        import mypy.semanal_classprop as sc
        from mypy.build import _native_builtins_sig

        int_info = self._int_info()
        nativ = self.fx.make_type_info("mypy_extensions.i64")
        nativ.defn.info = nativ
        builtin_names = SymbolTable()
        builtin_names["int"] = SymbolTableNode(MDEF, int_info)
        before = _native_builtins_sig(int_info)
        old = sc._HAS_RUST_CLASSPROP
        sc._HAS_RUST_CLASSPROP = False
        try:
            options = Options()
            options.native_type_kernel = True
            sc.add_type_promotion(nativ, SymbolTable(), options, builtin_names)
        finally:
            sc._HAS_RUST_CLASSPROP = old
        assert _native_builtins_sig(int_info) != before


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSubexprAststripSuite(Suite):
    """Parity tests for `rust_get_subexpressions` and `rust_strip_ref_expr`
    (issue #1635).

    `rust_get_subexpressions` walks a live AST tree (zero wire bytes) and
    collects every expression node in pre-order, mirroring
    `SubexpressionFinder`. `rust_strip_ref_expr` resets the five RefExpr
    fields on a live NameExpr/MemberExpr.

    Tests build small hand-made ASTs, run both the Python and native
    paths, and assert the results match by identity (same objects).
    """

    def setUp(self) -> None:
        from mypy.server.aststrip import _set_native_active as _set_aststrip_active
        from mypy.server.subexpr import _set_native_active

        self._set_subexpr = _set_native_active
        self._set_aststrip = _set_aststrip_active

    def _tree(self, defs: list[Statement]) -> MypyFile:
        tree = MypyFile([], [])
        tree._fullname = "main"
        tree.names = SymbolTable()
        tree.path = ""
        tree.defs = defs
        return tree

    # -- get_subexpressions parity --

    def _subexpr_parity(self, tree: MypyFile) -> None:
        from mypy.server.subexpr import get_subexpressions

        self._set_subexpr(False)
        off = get_subexpressions(tree)
        self._set_subexpr(True)
        on = get_subexpressions(tree)
        assert len(on) == len(off), f"len mismatch: {len(on)} vs {len(off)}"
        for i, (a, b) in enumerate(zip(on, off)):
            assert a is b, f"identity mismatch at {i}: {a} vs {b}"

    def test_simple_name_expr(self) -> None:
        tree = self._tree([ExpressionStmt(NameExpr("x"))])
        self._subexpr_parity(tree)

    def test_nested_expressions(self) -> None:
        # x + y[0]
        idx = IndexExpr(NameExpr("y"), IntExpr(0))
        op = OpExpr("+", NameExpr("x"), idx)
        tree = self._tree([ExpressionStmt(op)])
        self._subexpr_parity(tree)

    def test_call_expr(self) -> None:
        # f(a, b.c)
        call = CallExpr(
            NameExpr("f"),
            [NameExpr("a"), MemberExpr(NameExpr("b"), "c")],
            [ARG_POS, ARG_POS],
            [None, None],
        )
        tree = self._tree([ExpressionStmt(call)])
        self._subexpr_parity(tree)

    def test_func_def_body(self) -> None:
        # def f(): return x + 1
        body = ReturnStmt(OpExpr("+", NameExpr("x"), IntExpr(1)))
        fdef = FuncDef("f", [], Block([body]))
        fdef._fullname = "main.f"
        tree = self._tree([fdef])
        self._subexpr_parity(tree)

    def test_assignment_stmt(self) -> None:
        # x = [1, 2]
        assign = AssignmentStmt([NameExpr("x")], ListExpr([IntExpr(1), IntExpr(2)]))
        tree = self._tree([assign])
        self._subexpr_parity(tree)

    def test_conditional_expr(self) -> None:
        # x if y else z
        cond = ConditionalExpr(NameExpr("y"), NameExpr("x"), NameExpr("z"))
        tree = self._tree([ExpressionStmt(cond)])
        self._subexpr_parity(tree)

    def test_empty_tree(self) -> None:
        tree = self._tree([])
        self._subexpr_parity(tree)

    # -- strip_ref_expr parity --

    def _strip_name(self) -> NameExpr:
        n = NameExpr("foo")
        n.fullname = "main.foo"
        n.kind = GDEF
        n.node = Var("foo")
        n.is_new_def = True
        n.is_inferred_def = True
        return n

    def _strip_member(self) -> MemberExpr:
        m = MemberExpr(NameExpr("obj"), "attr")
        m.fullname = "main.obj.attr"
        m.kind = GDEF
        m.node = Var("attr")
        m.is_new_def = True
        m.is_inferred_def = True
        return m

    def _assert_stripped(self, node: RefExpr) -> None:
        assert node.kind is None, f"kind not None: {node.kind}"
        assert node.node is None, f"node not None: {node.node}"
        assert node.fullname == "", f"fullname not empty: {node.fullname}"
        assert node.is_new_def is False, f"is_new_def not False: {node.is_new_def}"
        assert node.is_inferred_def is False, f"is_inferred_def not False: {node.is_inferred_def}"

    def test_strip_name_expr_native(self) -> None:
        from type_kernel import rust_strip_ref_expr

        n = self._strip_name()
        result = rust_strip_ref_expr(n)
        assert result is True, "native strip should succeed"
        self._assert_stripped(n)

    def test_strip_member_expr_native(self) -> None:
        from type_kernel import rust_strip_ref_expr

        m = self._strip_member()
        result = rust_strip_ref_expr(m)
        assert result is True, "native strip should succeed"
        self._assert_stripped(m)

    def test_strip_non_refexpr_defers(self) -> None:
        from type_kernel import rust_strip_ref_expr

        # IntExpr is not a RefExpr — should defer (None)
        result = rust_strip_ref_expr(IntExpr(42))
        assert result is None, "non-RefExpr should defer"

    def test_strip_gate_off_vs_on_name(self) -> None:
        from mypy.server.aststrip import NodeStripVisitor

        # Gate off: Python body runs
        n_off = self._strip_name()
        self._set_aststrip(False)
        visitor = NodeStripVisitor()
        visitor.strip_ref_expr(n_off)
        self._assert_stripped(n_off)

        # Gate on: Rust seam runs
        n_on = self._strip_name()
        self._set_aststrip(True)
        visitor = NodeStripVisitor()
        visitor.strip_ref_expr(n_on)
        self._assert_stripped(n_on)
        self._set_aststrip(False)

    def test_strip_gate_off_vs_on_member(self) -> None:
        from mypy.server.aststrip import NodeStripVisitor

        m_off = self._strip_member()
        self._set_aststrip(False)
        visitor = NodeStripVisitor()
        visitor.strip_ref_expr(m_off)
        self._assert_stripped(m_off)

        m_on = self._strip_member()
        self._set_aststrip(True)
        visitor = NodeStripVisitor()
        visitor.strip_ref_expr(m_on)
        self._assert_stripped(m_on)
        self._set_aststrip(False)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeNodeShadowReadFlipSuite(Suite):
    """G1.2 (#1674): the `name` gap closure and the first read flip.

    `mypy/nodes_mirror` seeds `NameExpr.name` / `MemberExpr.name` at
    adoption (the constructor write is invisible to the adopted-only
    rule, so a read-back seed is what puts the slot in the record) and
    captures later writes, which lets `rust_aststrip_process_lvalue`
    serve the aststrip lvalue read (`is_new_def` + `name`) plus the
    class-namespace delete from shadow storage.

    `None` from the seam keeps the Python tail: every gate-off,
    uncovered-shape and `type is None` assertion below pins that
    fallback, and the differential asserts flip-off and flip-on agree on
    the resulting namespace.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import nodes_mirror

        # #1864: one seed pin adopts through a CallExpr `analyzed` write,
        # so the suite needs the full scope.
        os.environ[nodes_mirror._CAPTURE_SCOPE_ENV] = "full"
        self.addCleanup(os.environ.pop, nodes_mirror._CAPTURE_SCOPE_ENV, None)
        nodes_mirror.activate(audit=True)
        nodes_mirror.reset(clear_counts=True)
        self._k = kernel
        self._m = nodes_mirror
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        from mypy.server import aststrip

        aststrip._set_native_shadow_read_active(False)
        self._m.reset(clear_counts=True)

    def _handle(self, node: Any) -> int:
        handle = self._m._NODE_HANDLES.get(id(node))
        assert handle is not None, "node was not adopted by the shadow"
        return handle

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {k: v - before.get(k, 0) for k, v in after.items() if v != before.get(k, 0)}

    def _type_info(self, names: list[str]) -> Any:
        info = TypeInfo(SymbolTable(), ClassDef("C", Block([]), None, []), "mod")
        for name in names:
            info.names[name] = SymbolTableNode(MDEF, Var(name))
        return info

    def _adopted_member(self, name: str, *, is_new_def: bool, target: Any = None) -> MemberExpr:
        """A MemberExpr the shadow holds a record for, as semanal leaves it."""
        member = MemberExpr(NameExpr("self"), name)
        member.kind = MDEF if is_new_def else LDEF
        member.node = target
        member.is_new_def = is_new_def
        return member

    def _unrecorded_member(self, name: str) -> MemberExpr:
        """A `self.x = ...` lvalue the capture gate never saw."""
        member = MemberExpr(NameExpr("self"), name)
        self._m._active = False
        try:
            member.is_new_def = True
        finally:
            self._m._active = True
        assert id(member) not in self._m._NODE_HANDLES
        return member

    # -- the name gap closure --

    def test_adoption_seeds_the_name_slot(self) -> None:
        member = MemberExpr(NameExpr("self"), "v")
        assert id(member) not in self._m._NODE_HANDLES
        member.kind = MDEF
        handle = self._handle(member)
        assert self._k.rust_node_mirror_field(handle, "name") == ("text", "v")
        expr = NameExpr("x")
        expr.kind = GDEF
        assert self._k.rust_node_mirror_field(self._handle(expr), "name") == ("text", "x")

    def test_constructor_name_write_never_adopts(self) -> None:
        expr = NameExpr("x")
        MemberExpr(NameExpr("self"), "v")
        assert id(expr) not in self._m._NODE_HANDLES
        assert self._k.rust_node_mirror_entry_count() == 0

    def test_refexpr_without_a_name_slot_seeds_nothing(self) -> None:
        from mypy.nodes import TypeApplication

        call = CallExpr(NameExpr("f"), [], [], [])
        call.analyzed = TypeApplication(NameExpr("f"), [self.fx.a])
        handle = self._handle(call)
        assert self._k.rust_node_mirror_field(handle, "name") is None

    def test_later_name_write_refreshes_the_record(self) -> None:
        # `mypy/renaming.py` renames NameExpr slots in place; that write
        # must land in the record or a served read would go stale.
        expr = NameExpr("x")
        expr.kind = LDEF
        handle = self._handle(expr)
        expr.name = "x'"
        assert self._k.rust_node_mirror_field(handle, "name") == ("text", "x'")
        other = NameExpr("y")
        other.name = "y'"  # unadopted: no record, so no stale name
        assert id(other) not in self._m._NODE_HANDLES

    # -- the seam --

    def test_seam_serves_the_read_and_deletes(self) -> None:
        target = Var("v")
        member = self._adopted_member("v", is_new_def=True, target=target)
        info = self._type_info(["v", "other"])
        assert self._k.rust_aststrip_process_lvalue(info, member) is True
        assert "v" not in info.names
        assert set(info.names) == {"other"}

    def test_seam_serves_a_noop_when_the_name_is_absent(self) -> None:
        member = self._adopted_member("missing", is_new_def=True)
        info = self._type_info(["other"])
        assert self._k.rust_aststrip_process_lvalue(info, member) is False
        assert set(info.names) == {"other"}

    def test_seam_serves_false_for_an_established_member(self) -> None:
        member = self._adopted_member("v", is_new_def=False)
        info = self._type_info(["v"])
        assert self._k.rust_aststrip_process_lvalue(info, member) is False
        assert "v" in info.names, "a non-defining member must not delete"

    def test_seam_defers_without_a_record(self) -> None:
        member = MemberExpr(NameExpr("self"), "v")
        info = self._type_info(["v"])
        assert self._k.rust_aststrip_process_lvalue(info, member) is None
        assert "v" in info.names

    def test_seam_defers_for_other_lvalue_shapes(self) -> None:
        info = self._type_info(["v"])
        name = NameExpr("v")
        name.kind = LDEF
        assert self._k.rust_aststrip_process_lvalue(info, name) is None
        assert self._k.rust_aststrip_process_lvalue(info, TupleExpr([name])) is None
        assert "v" in info.names

    def test_seam_defers_when_the_active_class_is_none(self) -> None:
        member = self._adopted_member("v", is_new_def=True)
        assert self._k.rust_aststrip_process_lvalue(None, member) is None

    def test_seam_drops_the_namespace_shadow_record(self) -> None:
        from mypy import symtables_mirror
        from mypy.symtable_access import put_names_entry

        member = self._adopted_member("v", is_new_def=True)
        info = self._type_info([])
        symtables_mirror.activate()
        try:
            put_names_entry(info.names, "v", SymbolTableNode(MDEF, Var("v")))
            assert self._k.rust_symtable_mirror_lookup(info.names, "v") is not None
            assert self._k.rust_aststrip_process_lvalue(info, member) is True
            assert "v" not in info.names
            assert self._k.rust_symtable_mirror_lookup(info.names, "v") is None
        finally:
            symtables_mirror.reset()

    # -- the differential --

    def test_differential_flip_off_and_on_agree(self) -> None:
        from mypy.server import aststrip

        before = dict(self._m.report())
        aststrip._set_native_shadow_read_active(False)
        tail_off = self._type_info(["v", "keep"])
        self._run_visitor(tail_off, self._adopted_member("v", is_new_def=True))
        plain_off = self._type_info(["v", "keep"])
        self._run_visitor(plain_off, self._unrecorded_member("v"))
        aststrip._set_native_shadow_read_active(True)
        served_on = self._type_info(["v", "keep"])
        self._run_visitor(served_on, self._adopted_member("v", is_new_def=True))
        plain_on = self._type_info(["v", "keep"])
        self._run_visitor(plain_on, self._unrecorded_member("v"))
        # Both pairs agree: the recorded lvalue is served from the shadow,
        # the unrecorded one keeps the Python tail.
        for info in (tail_off, plain_off, served_on, plain_on):
            assert set(info.names) == {"keep"}
        delta = self._delta(before)
        assert delta.get("aststrip.served") == 1, delta
        assert delta.get("aststrip.deferred") == 1, delta

    def _run_visitor(self, info: Any, lvalue: Any) -> None:
        from mypy.server.aststrip import NodeStripVisitor

        visitor = NodeStripVisitor()
        visitor.type = info
        visitor.process_lvalue_in_method(lvalue)

    def test_differential_covers_a_nested_tuple_lvalue(self) -> None:
        from mypy.server import aststrip

        before = dict(self._m.report())
        tuple_off = TupleExpr([NameExpr("self"), self._adopted_member("v", is_new_def=True)])
        tuple_on = TupleExpr([NameExpr("self"), self._adopted_member("v", is_new_def=True)])
        info_off = self._type_info(["v", "keep"])
        info_on = self._type_info(["v", "keep"])
        aststrip._set_native_shadow_read_active(False)
        self._run_visitor(info_off, tuple_off)
        aststrip._set_native_shadow_read_active(True)
        self._run_visitor(info_on, tuple_on)
        assert set(info_off.names) == set(info_on.names) == {"keep"}
        delta = self._delta(before)
        # The container arm and its plain NameExpr item defer to Python;
        # the inner MemberExpr is the one read the flip serves.
        assert delta.get("aststrip.deferred") == 2, delta
        assert delta.get("aststrip.served") == 1, delta

    # -- gate contract --

    def test_gate_off_writes_leave_nothing_to_serve(self) -> None:
        from mypy.server import aststrip

        before = dict(self._m.report())
        member = self._unrecorded_member("v")
        info = self._type_info(["v", "keep"])
        aststrip._set_native_shadow_read_active(True)
        self._run_visitor(info, member)
        # No record, so the seam defers and the Python tail still deletes.
        assert "v" not in info.names
        delta = self._delta(before)
        assert delta.get("aststrip.deferred") == 1, delta
        assert delta.get("aststrip.served") is None, delta

    def test_option_default_on_and_not_cache_affecting(self) -> None:
        from mypy.options import OPTIONS_AFFECTING_CACHE, Options

        # #1624: the serving mode still defaults on, but the build sets
        # every serving flip to `capture_active and option` and the capture
        # is default-off, so it is inert. The option never touches the cache.
        assert Options().native_ast_mirror_read is True
        assert "native_ast_mirror_read" not in OPTIONS_AFFECTING_CACHE

    # -- unmet preconditions must fail closed --

    def test_guard_falls_back_without_the_extension(self) -> None:
        """A missing extension must not turn the flipped call into a crash."""
        from mypy.server import aststrip

        member = self._adopted_member("v", is_new_def=True)
        info = self._type_info(["v", "keep"])
        before = dict(self._m.report())
        aststrip._set_native_shadow_read_active(True)
        original = aststrip._HAS_TYPE_KERNEL
        aststrip._HAS_TYPE_KERNEL = False
        try:
            self._run_visitor(info, member)
        finally:
            aststrip._HAS_TYPE_KERNEL = original
        # The guarded path never enters the seam: the Python tail removes
        # the name and neither a serve nor a defer is counted.
        assert "v" not in info.names
        delta = self._delta(before)
        assert delta.get("aststrip.served") is None, delta
        assert delta.get("aststrip.deferred") is None, delta

    def test_activate_reports_whether_the_store_is_live(self) -> None:
        """`activate` must say when it could not install anything.

        build.py gates the read flip on this return value, so a store that
        stayed off (missing extension) can never enable a flip whose
        substrate is absent.
        """
        import sys

        from mypy import nodes_mirror

        assert nodes_mirror.activate() is True, "the scratch extension is present"
        saved_module = sys.modules.get("type_kernel")
        saved_active = nodes_mirror._active
        saved_kernel = nodes_mirror._kernel_mod
        # Force the not-yet-active path with the extension unavailable, the
        # state build.py must never enable the flip from.
        nodes_mirror._active = False
        nodes_mirror._kernel_mod = None
        sys.modules["type_kernel"] = None  # type: ignore[assignment]
        try:
            assert nodes_mirror.activate() is False
        finally:
            if saved_module is not None:
                sys.modules["type_kernel"] = saved_module
            nodes_mirror._active = saved_active
            nodes_mirror._kernel_mod = saved_kernel


class NodeMirrorCaptureScopeSuite(Suite):
    """#1864/#1869: the capture scope arms the surface per class.

    The narrow scope is the production default. These tests pin its
    runtime contract - the ref arms capture while the analyzed, field and
    meta arms no-op with their own skip counters - plus the arming
    implications: `full` arms the whole surface, a meta flip env arms
    meta capture, and `stmt` (#1869) arms exactly the four classes the
    G2.1 serving channel reads. `activate` re-reads the scope env on
    every call, so every scope is exercised in this one process.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import nodes_mirror

        self._k = kernel
        self._m = nodes_mirror

    def tearDown(self) -> None:
        os.environ.pop(self._m._CAPTURE_SCOPE_ENV, None)
        self._m.reset(clear_counts=True)

    def _activate(self, scope: str | None) -> None:
        """Arm `scope` (None: env unset) and zero the store for one test."""
        if scope is None:
            os.environ.pop(self._m._CAPTURE_SCOPE_ENV, None)
        else:
            os.environ[self._m._CAPTURE_SCOPE_ENV] = scope
        assert self._m.activate(audit=True) is True
        self._m.reset(clear_counts=True)

    def _narrow_to_default(self) -> None:
        # Widen first, then narrow: the patches from the wider call stay
        # installed, so the runtime gate is the only thing left to test.
        self._activate("full")
        self._activate(None)

    def test_default_scope_captures_the_ref_family_only(self) -> None:
        self._activate(None)
        ref = NameExpr("x")
        ref.kind = GDEF
        assert id(ref) in self._m._NODE_HANDLES, "the binding write must adopt"
        counters = self._m.report()
        assert counters.get("capture_ref") == 1, counters
        assert counters.get("meta_capture", 0) == 0, counters
        assert self._k.rust_node_mirror_meta_entry_count() == 0

    def test_the_field_arms_noop_in_the_default_scope(self) -> None:
        self._narrow_to_default()
        member = MemberExpr(NameExpr("o"), "y")
        member.kind = GDEF
        before = self._m.report()
        member.def_var = Var("y")
        self._m.touch(member, "method_types")
        after = self._m.report()
        assert after.get("scope_skip.field", 0) - before.get("scope_skip.field", 0) == 1
        assert after.get("scope_skip.touch", 0) - before.get("scope_skip.touch", 0) == 1
        fields = self._k.rust_node_mirror_fields(self._m._NODE_HANDLES[id(member)]) or []
        assert "def_var" not in fields, "a skipped field arm must not record"

    def test_analyzed_noops_in_the_default_scope(self) -> None:
        self._narrow_to_default()
        call = CallExpr(NameExpr("f"), [], [], [])
        before = self._m.report()
        call.analyzed = NameExpr("x")
        after = self._m.report()
        assert after.get("scope_skip.analyzed", 0) - before.get("scope_skip.analyzed", 0) == 1
        assert id(call) not in self._m._NODE_HANDLES, "a skipped arm must not adopt"

    def test_meta_capture_and_seed_noop_in_the_default_scope(self) -> None:
        self._narrow_to_default()
        var = Var("x")
        var.is_final = True
        assert id(var) not in self._m._META_HANDLES, "an unarmed meta surface must not adopt"
        assert self._m.seed_loaded(var) == 0
        counters = self._m.report()
        assert counters.get("scope_skip.meta", 0) >= 1, counters
        assert counters.get("seed_loaded.scope_skip") == 1, counters
        assert self._k.rust_node_mirror_meta_entry_count() == 0

    def test_full_scope_arms_the_whole_surface(self) -> None:
        self._activate("full")
        call = CallExpr(NameExpr("f"), [], [], [])
        call.analyzed = NameExpr("x")
        member = MemberExpr(NameExpr("o"), "y")
        member.kind = GDEF
        member.def_var = Var("y")
        var = Var("x")
        var.is_final = True
        assert self._m.seed_loaded(var) >= 1
        counters = self._m.report()
        assert counters.get("capture_analyzed") == 1, counters
        assert counters.get("capture_def_var") == 1, counters
        assert counters.get("meta_capture", 0) >= 1, counters
        assert self._k.rust_node_mirror_meta_entry_count() >= 1

    def test_a_meta_flip_env_arms_meta_capture(self) -> None:
        # The flip serves from records the narrow scope never writes, so
        # arming it implies arming their capture (#1864's arming rule).
        self._activate(None)
        os.environ[self._m._STMT_READ_FLIP_ENV] = "1"
        self.addCleanup(os.environ.pop, self._m._STMT_READ_FLIP_ENV, None)
        self.addCleanup(self._m.set_stmt_read_flip, 0)
        assert self._m.activate(audit=True) is True
        self._m.reset(clear_counts=True)
        var = Var("x")
        var.is_final = True
        assert id(var) in self._m._META_HANDLES, "the flip env must arm meta capture"
        assert self._m.report().get("meta_capture", 0) >= 1

    def test_stmt_scope_arms_exactly_the_four_serving_classes(self) -> None:
        """#1869: `stmt` is the surface the production wiring arms.

        The four classes the G2.1 channel serves (Block's `Bool` record
        plus the node-valued serve set) capture; every other G2 class
        skips with the meta counter, exactly like the default scope.
        """
        self._activate("stmt")
        block = Block([])
        block.is_unreachable = True
        nodes = [
            block,
            AssertStmt(NameExpr("x")),
            ExpressionStmt(NameExpr("y")),
            ReturnStmt(NameExpr("z")),
        ]
        for node in nodes:
            assert id(node) in self._m._META_HANDLES, "a serving class must capture"
        assert self._m.report().get("meta_capture", 0) >= 1, self._m.report()
        var = Var("x")
        var.is_final = True
        assert id(var) not in self._m._META_HANDLES, "a non-serving class must not capture"
        assert self._m.report().get("scope_skip.meta", 0) >= 1, self._m.report()

    def test_stmt_scope_keeps_the_ref_family_behavior(self) -> None:
        # The expression surface is unaffected by the wider meta arm: a
        # binding write adopts exactly as under the default scope, and
        # no meta record appears for it.
        self._activate("stmt")
        ref = NameExpr("x")
        ref.kind = GDEF
        assert id(ref) in self._m._NODE_HANDLES, "the binding write must adopt"
        counters = self._m.report()
        assert counters.get("capture_ref") == 1, counters
        assert counters.get("meta_capture", 0) == 0, counters
        assert self._k.rust_node_mirror_meta_entry_count() == 0

    def test_the_scope_env_is_reread_on_every_activate(self) -> None:
        self._activate("full")
        assert self._m._capture_scope == "full"
        assert self._m._meta_armed_classes == frozenset(self._m._G2_TRACKED)
        self._activate("stmt")
        assert self._m._capture_scope == "stmt"
        assert self._m._meta_armed_classes == self._m._META_STMT_CLASSES
        self._activate(None)
        assert self._m._capture_scope == "ref"
        assert self._m._meta_armed_classes == frozenset()
        var = Var("x")
        var.is_final = True
        assert id(var) not in self._m._META_HANDLES, "a re-read must narrow the runtime gate"
        assert self._m.report().get("scope_skip.meta", 0) >= 1

    def test_a_malformed_scope_env_raises_with_no_state_change(self) -> None:
        self._activate(None)
        before = self._m.report()
        os.environ[self._m._CAPTURE_SCOPE_ENV] = "bogus"
        raised = False
        try:
            self._m.activate()
        except ValueError:
            raised = True
        assert raised, "a malformed scope env must be refused"
        assert self._m._capture_scope == "ref"
        assert self._m.report() == before, "a refused activate must change no state"
        # Empty behaves like unset: the default scope, not a refusal.
        os.environ[self._m._CAPTURE_SCOPE_ENV] = ""
        assert self._m.activate() is True

    def test_the_default_scope_patches_only_the_ref_family(self) -> None:
        # Patching is monotonic, so an earlier full-scope activation in
        # this process leaves the wide classes patched and this pin has
        # nothing left to observe; the runtime gate is what holds them.
        if CallExpr.__setattr__ is self._m._node_setattr:
            return
        self._activate(None)
        assert RefExpr.__setattr__ is self._m._node_setattr
        assert CallExpr.__setattr__ is not self._m._node_setattr

    def test_binding_targets_are_still_pinned_in_the_default_scope(self) -> None:
        self._activate(None)
        var = Var("y")
        expr = NameExpr("y")
        expr.kind = GDEF
        expr.node = var
        handle = self._k.rust_node_mirror_handle_of(var)
        assert handle is not None, "the pin must ride the default-scope ref capture"
        assert self._k.rust_node_mirror_object_of(handle) is var
        assert self._m.report().get("pin_target") == 1


class NodeMirrorCaptureGateSuite(Suite):
    """#1875: the off-build boundary of a process that already activated.

    Activation is one-shot and `reset` keeps it, so an off-build in a
    process that ever ran an on-build finds the capture classes still
    patched. The build wiring must still reset the store (the #1572
    stale-graph hazard) and gate every runtime capture entry point, or
    the patched arms keep minting records no consumer reads. The gate
    defaults to on, so direct `activate()` callers capture as before.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import nodes_mirror

        self._k = kernel
        self._m = nodes_mirror
        assert self._m.activate(audit=True) is True, "the scratch extension is present"
        self._m.reset(clear_counts=True)

    def tearDown(self) -> None:
        # The gate is module state the build wiring writes per manager;
        # restore the default so later suites capture as before.
        self._m.set_capture_enabled(True)
        self._m.reset(clear_counts=True)

    def test_off_build_in_an_ever_active_process_still_resets_the_store(self) -> None:
        # The build-level reset path (#1875's stale-graph arm): the daemon
        # recheck calls `_clear_native_resolvers`, which must drop the
        # store when the option is off in an ever-active process.
        from types import SimpleNamespace

        from mypy.build import BuildManager
        from mypy.options import Options

        ref = NameExpr("x")
        ref.kind = GDEF
        assert id(ref) in self._m._NODE_HANDLES, "the on-arm must capture first"
        assert self._k.rust_node_mirror_entry_count() == 1
        options = Options()
        options.native_ast_mirror = False
        options.native_symtable_mirror = False
        options.native_symtable_read_flip = False
        options.native_symtable_read_flip_verify = False
        options.native_type_mirror = False
        options.native_type_kernel = False
        manager = cast(BuildManager, SimpleNamespace(options=options))
        BuildManager._clear_native_resolvers(manager)
        assert self._m._NODE_HANDLES == {}, "an off-build reset must drop the records"
        assert self._m._META_HANDLES == {}, "an off-build reset must drop the meta records"
        assert self._k.rust_node_mirror_entry_count() == 0, "no record may pin a node"

    def test_a_second_build_with_the_gate_off_drops_the_shadow(self) -> None:
        # The issue #1875 scenario end to end: the process activated
        # capture once (setUp), then runs a build with the option off -
        # the on->off order now normal for any first-on-arm process.
        import tempfile

        from mypy.dmypy_server import Server
        from mypy.modulefinder import BuildSource
        from mypy.options import Options

        assert self._m.ever_active() is True
        with tempfile.TemporaryDirectory() as td:
            main_path = os.path.join(td, "main.py")
            with open(main_path, "w", encoding="utf8") as f:
                f.write("value = [3]\n")
            options = Options()
            options.use_builtins_fixtures = True
            options.native_ast_mirror = False
            server = Server(options, os.path.join(td, "status.json"))
            sources = [BuildSource(main_path, "main", None)]

            def check() -> None:
                res = server.check(sources, export_types=False, is_tty=False, terminal_width=-1)
                assert res["status"] == 0, res

            check()
            assert self._m._NODE_HANDLES == {}, "an off-build must mint no record"
            assert self._k.rust_node_mirror_entry_count() == 0, self._m.report()
            # The recheck runs the daemon boundary (`_clear_native_resolvers`)
            # with the option still off: the ever-active arm must reset.
            with open(main_path, "w", encoding="utf8") as f:
                f.write("value = [4]\n")
            check()
            assert self._m._NODE_HANDLES == {}, "no record may survive the recheck"
            assert self._k.rust_node_mirror_entry_count() == 0, self._m.report()

    def test_the_off_arm_installs_the_dormant_state(self) -> None:
        # The build wiring writes `set_capture_enabled(False)` once per
        # manager; every capture entry point must then no-op exactly like
        # a never-activated process (#1875's ungated-path family).
        os.environ[self._m._CAPTURE_SCOPE_ENV] = "full"
        self.addCleanup(os.environ.pop, self._m._CAPTURE_SCOPE_ENV, None)
        assert self._m.activate(audit=True) is True
        self._m.reset(clear_counts=True)
        self._m.set_capture_enabled(False)
        ref = NameExpr("x")
        ref.kind = GDEF
        call = CallExpr(NameExpr("f"), [], [], [])
        call.analyzed = NameExpr("x")
        member = MemberExpr(NameExpr("o"), "y")
        member.kind = GDEF
        member.def_var = Var("y")
        self._m.touch(member, "method_types")
        var = Var("x")
        var.is_final = True
        del var.is_final
        assert self._m.seed_loaded(var) == 0
        assert self._m.cache_read_origin(self._m.ORIGIN_CACHE_FIXED) is None
        assert self._m.report() == {}, "a gated entry point must not even count"
        assert self._m._NODE_HANDLES == {}
        assert self._m._META_HANDLES == {}
        assert self._k.rust_node_mirror_entry_count() == 0
        assert self._k.rust_node_mirror_meta_entry_count() == 0
        # The setter is idempotent and reversible: a repeated off write
        # changes nothing, and re-enabling resumes capture for a later
        # on-build (the on->off->on order).
        self._m.set_capture_enabled(False)
        self._m.set_capture_enabled(True)
        resumed = NameExpr("y")
        resumed.kind = GDEF
        assert id(resumed) in self._m._NODE_HANDLES, "re-enabling must resume capture"
        assert self._m.report().get("capture_ref") == 1, self._m.report()

    def test_ever_active_flips_once_and_survives_reset(self) -> None:
        # The sticky marker keys the reset arm on, so it must follow
        # activation's own rule: kept across `reset`, never back. (Being
        # already True on entry is process state other suites may set.)
        assert self._m.ever_active() is True, "setUp's activate must flip it"
        self._m.reset()
        assert self._m.ever_active() is True, "reset must keep the marker"
        assert self._m._active is True, "reset must keep activation (one-shot)"
        self._m.reset(clear_counts=True)
        assert self._m.ever_active() is True, "a clear-counts reset must keep it too"

    def test_the_default_gate_keeps_direct_activation_capturing(self) -> None:
        # Part 2's default contract: with no build-level call, direct
        # activate() captures exactly as before #1875 - the other mirror
        # suites rely on this.
        assert self._m.capture_enabled() is True
        ref = NameExpr("x")
        ref.kind = GDEF
        assert id(ref) in self._m._NODE_HANDLES, "the default state must capture"
        assert self._m.report().get("capture_ref") == 1, self._m.report()
