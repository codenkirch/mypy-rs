"""Native seam suites for the server area (split from testtypes.py, #1677)."""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
    from librt.internal import WriteBuffer as _WriteBuffer
except ImportError:
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _type_kernel = None  # type: ignore[assignment]

from types import SimpleNamespace
from typing import Any, cast
from unittest import skipUnless

from mypy.nodes import (
    ARG_POS,
    ARG_STAR,
    COVARIANT,
    GDEF,
    INVARIANT,
    MDEF,
    Argument,
    AssignmentStmt,
    Block,
    CallExpr,
    ClassDef,
    Decorator,
    DelStmt,
    Expression,
    ExpressionStmt,
    ForStmt,
    FuncDef,
    Import,
    ImportAll,
    ImportFrom,
    IndexExpr,
    MemberExpr,
    MypyFile,
    NameExpr,
    OpExpr,
    OverloadedFuncDef,
    PlaceholderNode,
    Statement,
    SymbolTable,
    SymbolTableNode,
    TypeAlias,
    TypeInfo,
    TypeVarExpr,
    Var,
    WithStmt,
)
from mypy.options import Options
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import _HAS_TYPE_KERNEL, _NATIVE_WIRE_ENABLED
from mypy.test.typefixture import TypeFixture
from mypy.types import (
    AnyType,
    CallableType,
    DeletedType,
    ErasedType,
    Instance,
    NoneType,
    Overloaded,
    Parameters,
    ParamSpecFlavor,
    ParamSpecType,
    PartialType,
    TupleType,
    Type,
    TypeAliasType,
    TypedDictType,
    TypeOfAny,
    TypeType,
    TypeVarId,
    TypeVarType,
    UnboundType,
    UninhabitedType,
    UnionType,
    UnpackType,
)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAttributeTriggersSuite(Suite):
    """Parity tests for the Rust `attribute_triggers` port (#1007).

    Mirrors the structure of `NativeServerDepsSuite`: each test calls
    `DependencyVisitor.attribute_triggers` with the Rust gate on and off and
    asserts identical trigger lists. The Rust path reads live `Type` objects
    via PyO3 and defers (`None`) on unreadable facts, in which case the
    Python method runs instead, so parity holds by construction; these tests
    verify the Rust path actually handles each case and produces the same
    result.
    """

    def setUp(self) -> None:
        from mypy.server.deps import _set_native_server_deps_active
        from mypy.server.trigger import make_trigger

        self._set_active = _set_native_server_deps_active
        self.make_trigger = make_trigger
        self.fx = TypeFixture()
        self.v = self._make_visitor()

    def _make_visitor(self) -> Any:
        from mypy.server.deps import DependencyVisitor

        # attribute_triggers only reads types; no AST walk state is needed.
        return DependencyVisitor.__new__(DependencyVisitor)

    def _triggers(self, typ: Any, name: str = "x") -> list[str]:
        self._set_active(False)
        py = self.v.attribute_triggers(typ, name)
        self._set_active(True)
        rs = self.v.attribute_triggers(typ, name)
        assert_equal(rs, py, f"Rust/Python mismatch for {typ!r} (name={name!r})")
        return cast("list[str]", rs)

    def test_instance(self) -> None:
        assert_equal(self._triggers(self.fx.a), [self.make_trigger("A.x")])

    def test_type_var_unwraps_upper_bound(self) -> None:
        # fx.t's upper bound is object.
        assert_equal(self._triggers(self.fx.t), [self.make_trigger("builtins.object.x")])

    def test_tuple_type_uses_partial_fallback(self) -> None:
        assert_equal(self._triggers(self.fx.std_tuple), [self.make_trigger("builtins.tuple.x")])

    def test_none_type_is_empty(self) -> None:
        assert_equal(self._triggers(self.fx.nonet), [])

    def test_any_type_is_empty(self) -> None:
        assert_equal(self._triggers(self.fx.anyt), [])

    def test_type_type_of_instance(self) -> None:
        # No metaclass: only the item trigger.
        assert_equal(self._triggers(self.fx.type_a), [self.make_trigger("A.x")])

    def test_type_type_with_metaclass(self) -> None:
        cls = self.fx.make_type_info("Cls")
        meta = self.fx.make_type_info("Meta")
        cls.metaclass_type = Instance(meta, [])
        assert_equal(
            self._triggers(TypeType.make_normalized(Instance(cls, [])), "m"),
            [self.make_trigger("Cls.m"), self.make_trigger("Meta.m")],
        )

    def test_type_object_callable(self) -> None:
        # A callable whose ret_type is an Instance is a type object: member
        # trigger from the ret_type's type, then recursion on the fallback.
        c = CallableType([], [], [], self.fx.a, self.fx.type_type)
        assert_equal(
            self._triggers(c), [self.make_trigger("A.x"), self.make_trigger("builtins.type.x")]
        )

    def test_union_flattens(self) -> None:
        u = UnionType([self.fx.a, self.fx.b])
        assert_equal(self._triggers(u), [self.make_trigger("A.x"), self.make_trigger("B.x")])

    def test_union_with_empty_arm(self) -> None:
        u = UnionType([self.fx.a, self.fx.nonet])
        assert_equal(self._triggers(u), [self.make_trigger("A.x")])

    def test_direct_seam(self) -> None:
        import type_kernel as tk

        assert_equal(tk.rust_attribute_triggers(self.fx.a, "x"), [self.make_trigger("A.x")])
        assert_equal(tk.rust_attribute_triggers(self.fx.nonet, "x"), [])
        assert_equal(
            tk.rust_attribute_triggers(self.fx.t, "y"), [self.make_trigger("builtins.object.y")]
        )
        # Metaclass and type-object branches must be natively decided too
        # (not silently deferred to the Python fallback).
        cls = self.fx.make_type_info("Cls")
        meta = self.fx.make_type_info("Meta")
        cls.metaclass_type = Instance(meta, [])
        assert_equal(
            tk.rust_attribute_triggers(TypeType.make_normalized(Instance(cls, [])), "m"),
            [self.make_trigger("Cls.m"), self.make_trigger("Meta.m")],
        )
        c = CallableType([], [], [], self.fx.a, self.fx.type_type)
        assert_equal(
            tk.rust_attribute_triggers(c, "x"),
            [self.make_trigger("A.x"), self.make_trigger("builtins.type.x")],
        )

    def test_gate_off_matches_python(self) -> None:
        from mypy.server.deps import _HAS_TYPE_KERNEL

        assert _HAS_TYPE_KERNEL
        self._set_active(False)
        triggers = self.v.attribute_triggers(self.fx.a, "x")
        self._set_active(True)
        self.assertEqual(triggers, [self.make_trigger("A.x")])


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeAstdiffSnapshotSuite(Suite):
    """Issue #1497 (B7 slice 1): native `astdiff.snapshot_type` port.

    The Rust seam mirrors `SnapshotTypeVisitor` for every non-generic type
    arm; the direct seam calls prove engagement, the defer arms (generic
    `CallableType`, `PartialType`, unhandled shapes), and the gate-off vs
    gate-on differential proves the public `snapshot_type` answers
    identically in equality and hash across a spread of TypeFixture shapes.
    """

    def setUp(self) -> None:
        from mypy.server import astdiff

        self.astdiff = astdiff
        self.fx = TypeFixture()
        self._prev_active = astdiff._native_astdiff_active
        astdiff._set_native_astdiff_active(True)

    def tearDown(self) -> None:
        self.astdiff._set_native_astdiff_active(self._prev_active)

    def _snapshot(self, typ: Any, active: bool) -> Any:
        self.astdiff._set_native_astdiff_active(active)
        try:
            return self.astdiff.snapshot_type(typ)
        finally:
            self.astdiff._set_native_astdiff_active(True)

    def _assert_parity(self, label: str, typ: Any) -> Any:
        off = self._snapshot(typ, False)
        on = self._snapshot(typ, True)
        assert_equal(on, off, f"{label}: gate-on snapshot differs ({typ!r})")
        assert hash(on) == hash(off), f"{label}: gate-on snapshot hash differs"
        return on

    def _alias(self) -> TypeAlias:
        return TypeAlias(Instance(self.fx.std_listi, [self.fx.t]), "mod.SnapAlias", "mod", -1, -1)

    def _callable(self, *, generic: bool = False) -> CallableType:
        return CallableType(
            [self.fx.a],
            [ARG_POS],
            ["x"],
            self.fx.b,
            self.fx.function,
            is_ellipsis_args=False,
            is_bound=True,
            type_guard=self.fx.b,
            type_is=self.fx.a,
            instance_type=self.fx.a,
            variables=[self.fx.t] if generic else None,
        )

    def _cases(self) -> list[tuple[str, Any]]:
        from mypy.types import ExtraAttrs

        fx = self.fx
        return [
            ("AnyType", AnyType(TypeOfAny.special_form)),
            ("NoneType", NoneType()),
            ("UninhabitedType", UninhabitedType()),
            ("ErasedType", ErasedType()),
            ("DeletedType", DeletedType("x")),
            ("UnboundType", UnboundType("Foo")),
            (
                "UnboundType-full",
                UnboundType("Foo", [fx.a], optional=True, empty_tuple_index=True),
            ),
            ("Instance", fx.a),
            ("Instance-generic", fx.ga),
            ("Instance-lkv", fx.lit1_inst),
            (
                "Instance-extra-attrs",
                Instance(fx.ai, [], extra_attrs=ExtraAttrs({"y": fx.b, "x": fx.a}, {"x"})),
            ),
            ("TypeVarType", fx.t),
            (
                "TypeVarType-values",
                TypeVarType("V", "V", TypeVarId(7), [fx.a, fx.b], fx.o, fx.anyt, COVARIANT),
            ),
            (
                "ParamSpecType",
                ParamSpecType("P", "P", TypeVarId(-1), ParamSpecFlavor.BARE, fx.o, fx.anyt),
            ),
            ("TypeVarTupleType", fx.ts),
            ("UnpackType", UnpackType(fx.ts)),
            ("Parameters", Parameters([fx.a, fx.b], [ARG_POS, ARG_STAR], ["x", None])),
            ("CallableType", self._callable()),
            ("TupleType", TupleType([fx.a, fx.b], fx.std_tuple)),
            (
                "TypedDictType",
                TypedDictType({"b": fx.b, "a": fx.a}, {"b"}, {"a"}, fx.o, is_closed=True),
            ),
            ("LiteralType-int", fx.lit1),
            ("LiteralType-str", fx.lit_str1),
            ("UnionType", UnionType([fx.b, fx.a, fx.b, NoneType()])),
            ("Overloaded", Overloaded([self._callable(), self._callable()])),
            ("TypeType", TypeType.make_normalized(fx.a)),
            ("TypeType-form", TypeType(fx.a, is_type_form=True)),
            ("TypeAliasType", TypeAliasType(self._alias(), [fx.a])),
        ]

    def test_gate_off_on_parity(self) -> None:
        for label, typ in self._cases():
            self._assert_parity(label, typ)

    def test_engagement_plain_instance(self) -> None:
        raw = _type_kernel.rust_snapshot_type(self.fx.a)
        assert raw is not None, "Rust snapshot_type did not engage for a plain Instance"
        assert isinstance(raw, tuple)
        assert_equal(raw, self._snapshot(self.fx.a, False))

    def test_optional_and_sequence_helpers_route_through_seam(self) -> None:
        assert_equal(self.astdiff.snapshot_optional_type(None), ("<not set>",))
        assert_equal(
            self.astdiff.snapshot_optional_type(self.fx.a), self._snapshot(self.fx.a, True)
        )
        assert_equal(
            self.astdiff.snapshot_types([self.fx.a, self.fx.b]),
            (self._snapshot(self.fx.a, True), self._snapshot(self.fx.b, True)),
        )

    def test_union_sorted_and_deduped(self) -> None:
        s1 = self._assert_parity("union1", UnionType([self.fx.b, self.fx.a, self.fx.b]))
        s2 = self._assert_parity("union2", UnionType([self.fx.a, self.fx.b]))
        assert_equal(s1, s2, "union snapshots must be order/dedup insensitive")
        items = s1[1]
        assert items == tuple(sorted(items)), "union items must be sorted"
        assert len(items) == 2, "duplicate union items must be removed"

    def test_typed_dict_item_order_and_sorted_keys(self) -> None:
        td = TypedDictType({"b": self.fx.b, "a": self.fx.a}, {"b", "a"}, {"b"}, self.fx.o)
        snap = self._assert_parity("typeddict", td)
        assert [key for key, _ in snap[1]] == ["b", "a"], "items preserve dict order"
        assert snap[2] == ("a", "b"), "required keys are sorted"
        assert snap[3] == ("b",), "readonly keys are sorted"
        assert snap[4] is False

    def test_extra_attrs_sorted_pairs(self) -> None:
        from mypy.types import ExtraAttrs

        inst = Instance(
            self.fx.ai, [], extra_attrs=ExtraAttrs({"y": self.fx.b, "x": self.fx.a}, {"x"})
        )
        snap = self._assert_parity("extra-attrs", inst)
        pairs, immutable = snap[4]
        assert [key for key, _ in pairs] == ["x", "y"], "extra attrs sorted by key"
        assert set(immutable) == {"x"}

    def test_generic_callable_defers(self) -> None:
        generic = self._callable(generic=True)
        assert _type_kernel.rust_snapshot_type(generic) is None
        snap = self._assert_parity("generic-callable", generic)
        assert snap[0] == "CallableType"
        # The Python fallback normalizes tvar ids to -1 - i.
        assert snap[7][0][3] == -1

    def test_partial_type_defers(self) -> None:

        partial = PartialType(None, Var("x"))
        assert _type_kernel.rust_snapshot_type(partial) is None
        with self.assertRaises(RuntimeError):
            self._snapshot(partial, True)
        with self.assertRaises(RuntimeError):
            self._snapshot(partial, False)

    def test_unhandled_shape_defers(self) -> None:
        # A non-Type object: the seam defers so the Python path raises the
        # same AttributeError.
        assert _type_kernel.rust_snapshot_type(42) is None
        with self.assertRaises(AttributeError):
            self._snapshot(42, True)


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeAstdiffSymbolSnapshotSuite(Suite):
    """Issue #1500 (B7 slice 2): native `astdiff.snapshot_symbol_table` port.

    The direct seam call proves engagement; the gate-off vs gate-on
    differential proves recursive dict/tuple equality (including the
    nested class table and `(abstract)` entry) and `table.items()` order;
    the generic-CallableType case proves the per-node Python type-snapshot
    fallback still completes the symbol snapshot natively; the unknown /
    ``None`` node cases prove the deferral contract (the pure-Python
    fallback raises the identical AssertionError).
    """

    def setUp(self) -> None:
        from mypy.server import astdiff

        self.astdiff = astdiff
        self.fx = TypeFixture()
        self._prev_active = astdiff._native_astdiff_active
        astdiff._set_native_astdiff_active(True)

    def tearDown(self) -> None:
        self.astdiff._set_native_astdiff_active(self._prev_active)

    def _snapshot(self, prefix: str, table: Any, active: bool) -> Any:
        self.astdiff._set_native_astdiff_active(active)
        try:
            return self.astdiff.snapshot_symbol_table(prefix, table)
        finally:
            self.astdiff._set_native_astdiff_active(True)

    def _assert_parity(self, prefix: str, table: Any) -> Any:
        off = self._snapshot(prefix, table, False)
        on = self._snapshot(prefix, table, True)
        assert_equal(on, off, "gate-on symbol snapshot differs")
        assert list(on) == list(table), "snapshot must preserve table.items() order"
        return on

    def _func(
        self,
        name: str,
        fullname: str,
        typ: Any = None,
        *,
        is_property: bool = False,
        is_trivial_body: bool = False,
        deprecated: str | None = None,
    ) -> FuncDef:
        from mypy.nodes import Block

        func = FuncDef(name, [Argument(Var("x"), self.fx.a, None, ARG_POS)], Block([]), typ)
        func._fullname = fullname
        func.is_property = is_property
        func.is_trivial_body = is_trivial_body
        func.deprecated = deprecated
        return func

    def _var(self, name: str, fullname: str, typ: Any = None) -> Var:
        var = Var(name)
        var._fullname = fullname
        var.type = typ
        return var

    def _generic_callable(self) -> Any:
        from mypy.types import CallableType

        return CallableType(
            [self.fx.a], [ARG_POS], ["x"], self.fx.b, self.fx.function, variables=[self.fx.t]
        )

    def _type_info(self) -> TypeInfo:
        from mypy.mro import calculate_mro
        from mypy.nodes import Block, ClassDef, DataclassTransformSpec

        nested = SymbolTable()
        nested["m"] = SymbolTableNode(MDEF, self._func("m", "mod.C.m"))
        # Property overload whose first item's func carries a spec, so the
        # recursive Python find_dataclass_transform_spec fallback and the
        # setter_type arm both run.
        getter = self._func("prop", "mod.C.prop", is_property=True)
        getter.dataclass_transform_spec = DataclassTransformSpec(field_specifiers=("x",))
        getter_dec = Decorator(getter, [], self._var("prop", "mod.C.prop"))
        getter_dec.var.setter_type = self.fx.callable(self.fx.a)
        setter = self._func("prop", "mod.C.prop")
        overloaded = OverloadedFuncDef(
            [getter_dec, Decorator(setter, [], self._var("prop", "mod.C.prop"))]
        )
        overloaded._fullname = "mod.C.prop"
        overloaded.deprecated = "use q"
        nested["prop"] = SymbolTableNode(MDEF, overloaded)
        # Generic signature: the slice-1 type walk defers and the shim
        # must fall back to Python's snapshot_type for this node only.
        nested["g"] = SymbolTableNode(MDEF, self._func("g", "mod.C.g", self._generic_callable()))
        decorated = Decorator(self._func("d", "mod.C.d"), [], self._var("d", "mod.C.d", self.fx.a))
        nested["d"] = SymbolTableNode(MDEF, decorated)

        cdef = ClassDef("C", Block([]))
        cdef.fullname = "mod.C"
        info = TypeInfo(nested, cdef, "mod")
        cdef.info = info
        info.bases = [self.fx.a]
        calculate_mro(info)
        info.metaclass_type = info.calculate_metaclass_type()
        info.abstract_attributes = [("b", 2), ("a", 1)]
        info.dataclass_transform_spec = DataclassTransformSpec(eq_default=True)
        return info

    def _table(self) -> SymbolTable:
        from mypy.nodes import DataclassTransformSpec, ParamSpecExpr, TypeAlias, TypeVarTupleExpr
        from mypy.types import Instance

        table = SymbolTable()
        table["f"] = SymbolTableNode(GDEF, self._func("f", "mod.f", self._generic_callable()))
        untyped = self._func("uf", "mod.uf", is_trivial_body=True, deprecated="d")
        untyped.dataclass_transform_spec = DataclassTransformSpec(order_default=True)
        table["uf"] = SymbolTableNode(GDEF, untyped)
        table["v"] = SymbolTableNode(MDEF, self._var("v", "mod.v", self.fx.a))
        table["T"] = SymbolTableNode(
            GDEF, TypeVarExpr("T", "mod.T", [self.fx.a], self.fx.o, self.fx.anyt, INVARIANT)
        )
        table["P"] = SymbolTableNode(
            GDEF, ParamSpecExpr("P", "mod.P", self.fx.o, self.fx.anyt, INVARIANT)
        )
        table["Ts"] = SymbolTableNode(
            GDEF,
            TypeVarTupleExpr(
                "Ts", "mod.Ts", self.fx.o, self.fx.std_tuple, self.fx.anyt, INVARIANT
            ),
        )
        table["A"] = SymbolTableNode(
            GDEF, TypeAlias(Instance(self.fx.std_listi, [self.fx.t]), "mod.A", "mod", -1, -1)
        )
        table["cross"] = SymbolTableNode(GDEF, self._func("c", "other.c"))
        mod_file = MypyFile([], [])
        mod_file._fullname = "other.mod"
        table["modref"] = SymbolTableNode(GDEF, mod_file)
        table["C"] = SymbolTableNode(GDEF, self._type_info())
        return table

    def test_gate_off_on_parity_full_fixture(self) -> None:
        table = self._table()
        snap = self._assert_parity("mod", table)
        assert snap["cross"][0] == "CrossRef"
        assert snap["cross"][2] == "FuncDef"
        assert snap["modref"][0] == "Moduleref"
        assert snap["T"][0] == "TypeVar"
        assert snap["P"][0] == "ParamSpec"
        assert snap["Ts"][0] == "TypeVarTuple"
        assert snap["A"][0] == "TypeAlias"
        assert snap["uf"][0] == "Func"
        assert snap["uf"][7] is True, "is_trivial_body"
        assert snap["uf"][9] == "d", "FuncDef.deprecated"
        assert snap["v"][0] == "Var"

    def test_type_info_nested_table_and_abstract(self) -> None:
        snap = self._assert_parity("mod", self._table())
        info_snap = snap["C"]
        assert info_snap[0] == "TypeInfo"
        nested = info_snap[3]
        assert list(nested) == ["m", "prop", "g", "d", "(abstract)"], "nested order preserved"
        assert nested["(abstract)"] == ("Abstract", (("a", 1), ("b", 2))), "abstract attrs sorted"
        assert nested["m"][0] == "Func"
        assert nested["d"][0] == "Decorator"
        assert nested["d"][2][0] == "Instance", "Decorator var type snapshotted"
        # dataclass-transform specs (overloaded first item; TypeInfo attr)
        assert nested["prop"][8]["field_specifiers"] == ["x"]
        assert info_snap[2][14]["eq_default"] is True
        overloaded = nested["prop"]
        assert overloaded[0] == "Func"
        assert overloaded[9] == ["use q", None, None], "decorator deprecations collected"
        # multi-part property setter type captured
        assert overloaded[10][0] == "CallableType"

    def test_engagement_direct_seam(self) -> None:
        table = self._table()
        raw = _type_kernel.rust_snapshot_symbol_table("mod", table)
        assert raw is not None, "Rust snapshot_symbol_table did not engage"
        assert isinstance(raw, dict)
        assert_equal(raw, self._snapshot("mod", table, False))

    def test_generic_signature_falls_back_per_node(self) -> None:
        table = self._table()
        generic = cast(Any, table["f"].node).type
        assert _type_kernel.rust_snapshot_type(generic) is None
        snap = self._assert_parity("mod", table)
        # The symbol table is native even though the type leaf deferred.
        assert _type_kernel.rust_snapshot_symbol_table("mod", table) is not None
        assert snap["f"][6][0] == "CallableType"
        assert snap["C"][3]["g"][6][0] == "CallableType"

    def test_empty_table_is_native(self) -> None:
        table = SymbolTable()
        assert _type_kernel.rust_snapshot_symbol_table("mod", table) == {}
        assert self._snapshot("mod", table, True) == {}

    def test_unknown_node_defers(self) -> None:

        table = SymbolTable()
        table["p"] = SymbolTableNode(GDEF, PlaceholderNode("mod.p", Var("dummy"), -1))
        assert _type_kernel.rust_snapshot_symbol_table("mod", table) is None
        with self.assertRaises(AssertionError):
            self._snapshot("mod", table, True)
        with self.assertRaises(AssertionError):
            self._snapshot("mod", table, False)

    def test_none_node_defers(self) -> None:
        table = SymbolTable()
        table["x"] = SymbolTableNode(GDEF, None)
        assert _type_kernel.rust_snapshot_symbol_table("mod", table) is None
        with self.assertRaises(AssertionError):
            self._snapshot("mod", table, True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCacheMetaWriterSuite(Suite):
    """Issue #1503 (B1/B3 slice 1): native fixed-format cache meta writer.

    Byte parity Rust vs Python `CacheMeta.write` / `CacheMetaEx.write` over
    a fixture battery (all fields, empty collections, nested options JSON,
    plugin_data dict/tuple, error tuples with None path/code, i64 and
    arbitrary-precision ints, multiple `imports_ignored` entries), a
    round-trip through the existing native readers, the gate-off vs gate-on
    shim differential, and the defer contract for unsupported
    `plugin_data`.
    """

    def setUp(self) -> None:
        from mypy import cache

        self.cache = cache
        self._prev_active = cache._native_cache_active
        cache._set_native_cache_active(True)

    def tearDown(self) -> None:
        self.cache._set_native_cache_active(self._prev_active)

    def _ref(self, meta: Any, *, ex: bool = False) -> bytes:
        buf = _WriteBuffer()
        meta.write(buf)
        return buf.getvalue()

    def _rust(self, meta: Any, *, ex: bool = False) -> bytes | None:
        if ex:
            return _type_kernel.rust_write_cache_meta_ex(meta)
        return _type_kernel.rust_write_cache_meta(meta)

    def _assert_parity(self, meta: Any, *, ex: bool = False) -> bytes:
        ref = self._ref(meta, ex=ex)
        rust = self._rust(meta, ex=ex)
        assert rust is not None, "Rust writer must engage for the fixture"
        assert_equal(rust, ref, "Rust cache meta bytes differ from Python")
        # The cache.py shim must return the same bytes with the gate on and
        # None with it off (so build.py runs the pure-Python writer).
        if ex:
            assert self.cache._try_native_write_cache_meta_ex(meta) == ref
        else:
            assert self.cache._try_native_write_cache_meta(meta) == ref
        self.cache._set_native_cache_active(False)
        try:
            if ex:
                assert self.cache._try_native_write_cache_meta_ex(meta) is None
            else:
                assert self.cache._try_native_write_cache_meta(meta) is None
        finally:
            self.cache._set_native_cache_active(True)
        return rust

    def _meta(self, **overrides: Any) -> Any:
        from mypy.cache import CacheMeta

        fields: dict[str, Any] = {
            "id": "mod",
            "path": "/tmp/mod.py",
            "mtime": 1234567890,
            "size": 42,
            "hash": "deadbeef",
            "dependencies": ["a", "b"],
            "data_mtime": 1234567891,
            "data_file": "mod.data.ff",
            "suppressed": ["c"],
            "imports_ignored": {1: ["ignore"], 10: ["misc"]},
            "options": {"platform": "linux", "other_options": "hash"},
            "suppressed_deps_opts": b"\x01\x02",
            "dep_prios": [0, 1, 2],
            "dep_lines": [1, 2, 3],
            "dep_hashes": [b"\xaa" * 8, b"\xbb" * 16],
            "interface_hash": b"\xcc" * 16,
            "trans_dep_hash": b"\xdd" * 24,
            "version_id": "1.2.3",
            "ignore_all": False,
            "plugin_data": {"mypyc": True},
        }
        fields.update(overrides)
        return CacheMeta(**fields)

    def _meta_ex(self, **overrides: Any) -> Any:
        from mypy.cache import CacheMetaEx

        fields: dict[str, Any] = {
            "dependencies": ["a", "b"],
            "suppressed": ["c"],
            "dep_hashes": [b"\x01", b"\x02\x03"],
            "error_lines": [
                ("/tmp/mod.py", 1, 2, 3, 4, "error", "msg", "code"),
                (None, 10, 0, 10, 5, "note", "no path/code", None),
            ],
        }
        fields.update(overrides)
        return CacheMetaEx(**fields)

    def test_meta_full_fixture_parity_and_roundtrip(self) -> None:
        meta = self._meta(
            ignore_all=True, plugin_data={"mypyc": True, "n": 5, "f": 1.5, "none": None}
        )
        rust = self._assert_parity(meta)
        decoded = _type_kernel.rust_read_cache_meta(rust)
        assert decoded is not None, "native reader must decode the native bytes"
        assert_equal(decoded["id"], meta.id)
        assert_equal(decoded["path"], meta.path)
        assert_equal(decoded["mtime"], meta.mtime)
        assert_equal(decoded["hash"], meta.hash)
        assert_equal(decoded["dependencies"], meta.dependencies)
        assert_equal(decoded["suppressed"], meta.suppressed)
        assert_equal(decoded["imports_ignored"], meta.imports_ignored)
        assert_equal(decoded["options"], meta.options)
        assert_equal(decoded["suppressed_deps_opts"], meta.suppressed_deps_opts)
        assert_equal(decoded["dep_prios"], meta.dep_prios)
        assert_equal(decoded["dep_lines"], meta.dep_lines)
        assert_equal(decoded["dep_hashes"], meta.dep_hashes)
        assert_equal(decoded["interface_hash"], meta.interface_hash)
        assert_equal(decoded["trans_dep_hash"], meta.trans_dep_hash)
        assert_equal(decoded["version_id"], meta.version_id)
        assert_equal(decoded["ignore_all"], meta.ignore_all)
        assert_equal(decoded["plugin_data"], meta.plugin_data)

    def test_meta_empty_collections(self) -> None:
        meta = self._meta(
            dependencies=[],
            suppressed=[],
            imports_ignored={},
            dep_prios=[],
            dep_lines=[],
            dep_hashes=[],
            plugin_data=None,
        )
        rust = self._assert_parity(meta)
        decoded = _type_kernel.rust_read_cache_meta(rust)
        assert decoded is not None
        assert_equal(decoded["dependencies"], [])
        assert_equal(decoded["suppressed"], [])
        assert_equal(decoded["imports_ignored"], {})
        assert_equal(decoded["dep_hashes"], [])
        assert_equal(decoded["plugin_data"], None)

    def test_meta_options_nested_json(self) -> None:
        options: dict[str, Any] = {
            "platform": "darwin",
            "other_options": "h",
            "flag": True,
            "off": False,
            "nothing": None,
            "ratio": 0.5,
            "level": 3,
            "names": ["x", "y"],
            "pair": (1, "two"),
            "nested": {"z": None, "deep": [3, (4, False), {"k": 1.5}]},
        }
        meta = self._meta(options=options)
        rust = self._assert_parity(meta)
        decoded = _type_kernel.rust_read_cache_meta(rust)
        assert decoded is not None
        assert_equal(decoded["options"], options)

    def test_meta_plugin_data_dict_and_tuple(self) -> None:
        for plugin_data in (
            {"a": 1, "b": [1, 2], "c": (3, None)},
            (1, ("nested", True), None),
            "plain",
            7,
            2.5,
            True,
            None,
        ):
            meta = self._meta(plugin_data=plugin_data)
            rust = self._assert_parity(meta)
            decoded = _type_kernel.rust_read_cache_meta(rust)
            assert decoded is not None
            assert_equal(decoded["plugin_data"], plugin_data)

    def test_meta_large_ints(self) -> None:
        meta = self._meta(
            mtime=2**62,
            size=2**63 - 1,
            data_mtime=-(2**62),
            imports_ignored={2**40: ["big"]},
            dep_lines=[2**40, 1],
        )
        rust = self._assert_parity(meta)
        decoded = _type_kernel.rust_read_cache_meta(rust)
        assert decoded is not None
        assert_equal(decoded["mtime"], 2**62)
        assert_equal(decoded["size"], 2**63 - 1)
        assert_equal(decoded["data_mtime"], -(2**62))
        assert_equal(decoded["dep_lines"], [2**40, 1])

    def test_meta_arbitrary_precision_ints(self) -> None:
        # Beyond i64: writer byte parity only. The native reader is
        # i64-bounded by design, so the read seam defers to Python here.
        meta = self._meta(mtime=2**70, size=-(2**80), data_mtime=2**200)
        self._assert_parity(meta)

    def test_meta_multiple_imports_ignored_entries(self) -> None:
        imports_ignored = {5: ["b"], 1: ["a"], 100: ["c", "d"], 3: []}
        meta = self._meta(imports_ignored=imports_ignored)
        rust = self._assert_parity(meta)
        decoded = _type_kernel.rust_read_cache_meta(rust)
        assert decoded is not None
        assert_equal(decoded["imports_ignored"], imports_ignored)
        assert_equal(list(decoded["imports_ignored"]), list(imports_ignored))

    def test_meta_ex_error_tuples_and_roundtrip(self) -> None:
        meta_ex = self._meta_ex(
            error_lines=[
                (None, 1, 2, 3, 4, "error", "msg", None),
                ("/tmp/x.py", 2**40, 0, 2**40, 8, "note", "m2", "code"),
            ]
        )
        rust = self._assert_parity(meta_ex, ex=True)
        decoded = _type_kernel.rust_read_cache_meta_ex(rust)
        assert decoded is not None
        assert_equal(decoded["dependencies"], meta_ex.dependencies)
        assert_equal(decoded["suppressed"], meta_ex.suppressed)
        assert_equal(decoded["dep_hashes"], meta_ex.dep_hashes)
        assert_equal(decoded["error_lines"], meta_ex.error_lines)

    def test_meta_ex_empty_collections(self) -> None:
        meta_ex = self._meta_ex(dependencies=[], suppressed=[], dep_hashes=[], error_lines=[])
        rust = self._assert_parity(meta_ex, ex=True)
        decoded = _type_kernel.rust_read_cache_meta_ex(rust)
        assert decoded is not None
        assert_equal(decoded["error_lines"], [])

    def test_unsupported_plugin_data_defers(self) -> None:
        meta = self._meta(plugin_data={1, 2})
        assert _type_kernel.rust_write_cache_meta(meta) is None
        assert self.cache._try_native_write_cache_meta(meta) is None
        with self.assertRaises(AssertionError):
            self._ref(meta)

    def test_build_shim_prefix_and_metastore(self) -> None:
        from librt.internal import cache_version

        from mypy import build
        from mypy.cache import CACHE_VERSION

        class _MetaStore:
            def __init__(self) -> None:
                self.writes: list[tuple[str, bytes]] = []

            def write(self, path: str, data: bytes) -> bool:
                self.writes.append((path, data))
                return True

        manager = cast(
            Any,
            SimpleNamespace(
                options=SimpleNamespace(fixed_format_cache=True, debug_cache=False),
                metastore=_MetaStore(),
                log=lambda msg: None,
            ),
        )
        meta = self._meta()
        meta_ex = self._meta_ex()
        prefix = bytes([cache_version(), CACHE_VERSION])

        self.cache._set_native_cache_active(False)
        try:
            build.write_cache_meta(meta, manager, "/tmp/mod.meta.ff")
            build.write_cache_meta_ex("/tmp/mod.meta.ff", meta_ex, manager)
        finally:
            self.cache._set_native_cache_active(True)
        python_writes = list(manager.metastore.writes)

        manager.metastore.writes.clear()
        build.write_cache_meta(meta, manager, "/tmp/mod.meta.ff")
        build.write_cache_meta_ex("/tmp/mod.meta.ff", meta_ex, manager)
        rust_writes = list(manager.metastore.writes)
        assert len(rust_writes) == 2
        assert rust_writes[0][1] == prefix + self._ref(meta)
        assert rust_writes[1][1] == self._ref(meta_ex, ex=True)
        # Gate off vs gate on produce identical files.
        assert_equal([(p, d) for p, d in rust_writes], [(p, d) for p, d in python_writes])


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeServerDepsWalkSuite(Suite):
    """Parity tests for the native DependencyVisitor walk (#1632).

    Each test builds a small hand-made AST, runs `get_dependencies` with the
    server-deps gate off and on, and asserts the maps match. The gate-on run
    goes through `rust_walk_dependency_visitor`; tests also call the seam
    directly and assert it engages (does not defer) except for the deferral
    pin, which asserts it returns None on an unknown node kind.
    """

    def setUp(self) -> None:
        from mypy.server.deps import _set_native_server_deps_active

        self.fx = TypeFixture()
        self._set_active = _set_native_server_deps_active

    def _tree(self, defs: list[Statement]) -> MypyFile:
        tree = MypyFile([], [])
        tree._fullname = "main"
        tree.names = SymbolTable()
        tree.path = ""
        tree.defs = defs
        return tree

    def _maps(
        self, tree: MypyFile, type_map: dict[Expression, Type] | None = None, logical: bool = False
    ) -> dict[str, set[str]]:
        from type_kernel import rust_walk_dependency_visitor

        from mypy.server.deps import get_dependencies

        if type_map is None:
            type_map = {}
        options = Options()
        options.logical_deps = logical
        self._set_active(False)
        off = get_dependencies(tree, type_map, (3, 13), options)
        native = rust_walk_dependency_visitor(tree, type_map, tree.alias_deps, logical)
        assert native is not None, "native walk deferred unexpectedly"
        native_map = {k: set(v) for k, v in native.items()}
        assert_equal(native_map, off, "native seam/Python mismatch")
        self._set_active(True)
        on = get_dependencies(tree, type_map, (3, 13), options)
        assert_equal(on, off, "gate-off/gate-on mismatch")
        return on

    def _ref(self, name: str, fullname: str, kind: int = GDEF) -> NameExpr:
        ref = NameExpr(name)
        ref.fullname = fullname
        ref.kind = kind
        return ref

    def test_import_shapes(self) -> None:
        tree = self._tree(
            [
                Import([("os", None)]),
                ImportFrom("collections", 0, [("defaultdict", None)]),
                ImportAll("typing", 0),
            ]
        )
        assert_equal(
            self._maps(tree),
            {
                "<os>": {"main"},
                "<collections>": {"main"},
                "<collections.defaultdict>": {"main"},
                "<typing[wildcard]>": {"main"},
            },
        )

    def test_func_def_plain(self) -> None:
        body_ref = self._ref("x", "main.x")
        fdef = FuncDef("f", [], Block([ExpressionStmt(body_ref)]))
        fdef._fullname = "main.f"
        tree = self._tree([fdef])
        assert_equal(self._maps(tree), {"<main.x>": {"main.f"}})

    def test_func_def_typed(self) -> None:
        from mypy.types import CallableType

        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.anyt,
            self.fx.function,
        )
        fdef = FuncDef("f", [], Block([]), typ=c)
        fdef._fullname = "main.f"
        tree = self._tree([fdef])
        assert_equal(
            self._maps(tree), {"<A>": {"main.f", "<main.f>"}, "<B>": {"main.f", "<main.f>"}}
        )

    def test_class_def(self) -> None:
        info = self.fx.make_type_info("C", mro=[self.fx.ai, self.fx.oi])
        var = Var("y")
        var.is_initialized_in_class = True
        var.info = info
        info.names["y"] = SymbolTableNode(GDEF, var)
        cdef = ClassDef("C", Block([]))
        cdef.fullname = "C"
        cdef.info = info
        tree = self._tree([cdef])
        assert_equal(
            self._maps(tree),
            {
                "<C>": {"C"},
                "<A>": {"C"},
                "<C.y>": {"main"},
                "<A.y>": {"<C.y>"},
                "<A.__bool__>": {"<C.__bool__>"},
                "<A.__init__>": {"<C.__init__>"},
                "<A.__new__>": {"<C.__new__>"},
                "<A.(abstract)>": {"<C.__init__>", "main"},
            },
        )

    def test_decorator(self) -> None:
        func = FuncDef("h", [], Block([]))
        func._fullname = "main.h"
        tree = self._tree([Decorator(func, [self._ref("dec", "main.dec")], Var("h"))])
        assert_equal(self._maps(tree), {"<main.h>": {"main"}, "<main.dec>": {"main"}})

    def test_decorator_logical(self) -> None:
        func = FuncDef("h", [], Block([]))
        func._fullname = "main.h"
        tree = self._tree([Decorator(func, [self._ref("dec", "main.dec")], Var("h"))])
        assert_equal(self._maps(tree, logical=True), {"<main.dec>": {"main", "<main.h>"}})

    def test_logical_assignment_tail(self) -> None:
        callee = self._ref("f", "main.f")
        call = CallExpr(callee, [], [], [])
        lv = self._ref("x", "main.x")
        lv.is_new_def = True
        tree = self._tree([AssignmentStmt([lv], call)])
        assert_equal(
            self._maps(tree, logical=True),
            {"<main.f>": {"main", "<main.x>"}, "<main.x>": {"main"}},
        )

    def test_operator_expr(self) -> None:
        left = NameExpr("a")
        right = NameExpr("b")
        tree = self._tree([ExpressionStmt(OpExpr("+", left, right))])
        assert_equal(
            self._maps(tree, {left: self.fx.a, right: self.fx.a}),
            {"<A.__add__>": {"main"}, "<A.__radd__>": {"main"}},
        )

    def test_member_expr(self) -> None:
        base = NameExpr("o")
        member = MemberExpr(base, "y")
        tree = self._tree([ExpressionStmt(member)])
        assert_equal(self._maps(tree, {base: self.fx.a}), {"<A.y>": {"main"}})

    def test_for_stmt(self) -> None:
        index = NameExpr("i")
        expr = NameExpr("rng")
        tree = self._tree([ForStmt(index, expr, Block([]), Block([]))])
        assert_equal(
            self._maps(tree, {expr: self.fx.a}),
            {"<A.__iter__>": {"main"}, "<A.__getitem__>": {"main"}},
        )

    def test_with_and_del_stmt(self) -> None:
        mgr = NameExpr("m")
        tree = self._tree([WithStmt([mgr], [None], Block([]))])
        assert_equal(
            self._maps(tree, {mgr: self.fx.a}),
            {"<A.__enter__>": {"main"}, "<A.__exit__>": {"main"}},
        )
        base = NameExpr("d")
        tree2 = self._tree([DelStmt(IndexExpr(base, NameExpr("k")))])
        assert_equal(
            self._maps(tree2, {base: self.fx.a}),
            {"<A.__getitem__>": {"main"}, "<A.__delitem__>": {"main"}},
        )

    def test_if_stmt_traversal(self) -> None:
        from mypy.nodes import IfStmt

        cond = self._ref("c", "main.c")
        tree = self._tree(
            [
                IfStmt(
                    [cond],
                    [Block([ExpressionStmt(self._ref("t", "main.t"))])],
                    Block([ExpressionStmt(self._ref("e", "main.e"))]),
                )
            ]
        )
        assert_equal(
            self._maps(tree), {"<main.c>": {"main"}, "<main.t>": {"main"}, "<main.e>": {"main"}}
        )

    def test_target_driver(self) -> None:
        from type_kernel import rust_walk_dependency_target

        from mypy.server.deps import get_dependencies_of_target

        fdef = FuncDef("f", [], Block([ExpressionStmt(self._ref("x", "main.x"))]))
        fdef._fullname = "main.f"
        tree = self._tree([fdef])
        for target in (tree, fdef):
            self._set_active(False)
            off = get_dependencies_of_target("main", tree, target, {}, (3, 13))
            native = rust_walk_dependency_target("main", tree, target, {})
            assert native is not None, "native target walk deferred"
            assert_equal({k: set(v) for k, v in native.items()}, off)
            self._set_active(True)
            assert_equal(get_dependencies_of_target("main", tree, target, {}, (3, 13)), off)

    def test_defer_unknown_node(self) -> None:
        from type_kernel import rust_walk_dependency_visitor

        class Mystery(Statement):
            pass

        tree = self._tree([Mystery()])
        native = rust_walk_dependency_visitor(tree, {}, tree.alias_deps, False)
        assert native is None, "unknown node kind must defer"
