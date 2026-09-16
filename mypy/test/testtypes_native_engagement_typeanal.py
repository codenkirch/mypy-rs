"""Native engagement suites for the typeanal area (`mypy/typeanal.py`).

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

from collections.abc import Callable, Sequence
from types import SimpleNamespace
from typing import Any
from unittest import skipUnless

from mypy.errorcodes import ErrorCode
from mypy.nodes import (
    ARG_POS,
    ARG_STAR,
    INVARIANT,
    MDEF,
    Context,
    ImportFrom,
    PlaceholderNode,
    SymbolTableNode,
    TypeAlias,
    TypeInfo,
    TypeVarExpr,
    Var,
)
from mypy.options import Options
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED, T, _is_type_info
from mypy.test.typefixture import TypeFixture
from mypy.typeanal import _set_native_typeanal_active, native_analyze_type
from mypy.types import (
    AnyType,
    CallableType,
    DeletedType,
    EllipsisType,
    Instance,
    LiteralType,
    NoneType,
    Overloaded,
    Parameters,
    ParamSpecFlavor,
    ParamSpecType,
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
)


# Moved from mypy/test/testtypes_native_checker.py.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckUnpacksInListSuite(Suite):
    """Parity for the Rust `check_unpacks_in_list` filter.

    `TypeAnalyser.check_unpacks_in_list` (typeanal.py:2991) counts the
    non-tuple `Unpack` items in a type-arg list: the first passes through,
    later ones are dropped, and more than one emits "More than one
    variadic Unpack in a type is not allowed" with the final unpack's
    inner type as context. Rust returns the kept indices plus the final
    unpack index; Python applies the fail and rebuilds the list.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        self._set_active = _set_native_typeanal_active
        self._set_active(True)

    def tearDown(self) -> None:
        self._set_active(False)

    def _analyser(self) -> tuple[Any, Any]:
        from mypy.typeanal import TypeAnalyser

        class FakeApi:
            def __init__(self) -> None:
                self.errors: list[str] = []

            def fail(self, msg: str, ctx: Context, code: Any = None) -> None:
                self.errors.append(msg)

            def note(self, msg: str, ctx: Context, code: Any = None) -> None:
                self.errors.append(f"note: {msg}")

        api = FakeApi()
        ta = TypeAnalyser.__new__(TypeAnalyser)
        ta.fail_func = api.fail  # type: ignore[assignment]
        ta.note_func = api.note
        ta.options = Options()
        return ta, api

    def _variadic_unpack(self) -> UnpackType:
        # Unpack[Any]: the TODO-documented forward-reference shape; the
        # proper type is not a TupleType, so it counts as variadic.
        return UnpackType(AnyType(TypeOfAny.from_error))

    def _tuple_unpack(self) -> UnpackType:
        # Unpack[tuple[X, ...]] with a proper TupleType inner: an ordinary
        # item here, not a variadic unpack.
        return UnpackType(TupleType([self.fx.a], fallback=Instance(self.fx.std_tuplei, [])))

    def _tuple_instance_unpack(self) -> UnpackType:
        # Unpack[Instance(tuple, [X])]: the proper type is an Instance, not
        # a TupleType, so Python still counts it as a variadic unpack.
        return UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))

    def _assert_parity(self, items: list[Type]) -> None:
        off_ta, off_api = self._analyser()
        self._set_active(False)
        off = off_ta.check_unpacks_in_list(list(items))
        off_msgs = list(off_api.errors)
        self._set_active(True)
        on_ta, on_api = self._analyser()
        on = on_ta.check_unpacks_in_list(list(items))
        on_msgs = list(on_api.errors)
        assert_equal([str(t) for t in on], [str(t) for t in off], f"result {items!r}")
        assert_equal(on_msgs, off_msgs, f"messages {items!r}")

    def test_no_unpacks(self) -> None:
        self._assert_parity([self.fx.a, self.fx.str_type])

    def test_one_tuple_unpack_is_ordinary(self) -> None:
        # Unpack[tuple[X, ...]] passes through; no error either way.
        self._assert_parity([self.fx.a, self._tuple_unpack()])

    def test_one_variadic_unpack(self) -> None:
        self._assert_parity([self._variadic_unpack(), self.fx.a])

    def test_two_variadic_unpacks_fail(self) -> None:
        self._assert_parity([self._variadic_unpack(), self._variadic_unpack()])

    def test_three_variadic_unpacks_keep_first_only(self) -> None:
        self._assert_parity(
            [self._variadic_unpack(), self._variadic_unpack(), self._variadic_unpack()]
        )

    def test_tuple_unpack_between_variadic_ones(self) -> None:
        # A tuple unpack in the middle is an ordinary item; the variadic
        # count only sees the non-tuple unpacks.
        self._assert_parity(
            [self._variadic_unpack(), self._tuple_unpack(), self._variadic_unpack()]
        )

    def test_alias_unpack_expands_to_tuple(self) -> None:
        # Unpack[Tup] where Tup is a TypeAliasType wrapping a tuple
        # Instance: get_proper_type resolves it, so it is an ordinary
        # item in both paths (Rust resolves via the live Python fn).
        from mypy.nodes import TypeAlias
        from mypy.types import TypeAliasType

        alias = TypeAlias(Instance(self.fx.std_tuplei, [self.fx.a]), "mod.Tup", "mod", -1, -1)
        item = UnpackType(TypeAliasType(alias, []))
        self._assert_parity([item, self.fx.a])

    def test_direct_seam(self) -> None:
        from mypy.typeanal import _rust_check_unpacks_in_list  # type: ignore[attr-defined]

        va, tb = self._variadic_unpack(), self._tuple_unpack()
        assert _rust_check_unpacks_in_list([self.fx.a]) == ([0], None)
        assert _rust_check_unpacks_in_list([va]) == ([0], None)
        assert _rust_check_unpacks_in_list([va, self.fx.a, va]) == ([0, 1], 2)
        assert _rust_check_unpacks_in_list([self.fx.a, tb, va, va]) == ([0, 1, 2], 3)

    def test_tuple_instance_unpack_counts_variadic(self) -> None:
        # Unpack[Instance(tuple, [X])] is NOT a TupleType proper, so both
        # paths count it as a variadic unpack (not an ordinary item).
        self._assert_parity([self._tuple_instance_unpack(), self._tuple_instance_unpack()])

    def test_engagement(self) -> None:
        # The gate-on differential must exercise the Rust path, not just
        # agree with Python: the seam returns kept indices directly.
        from mypy.typeanal import _rust_check_unpacks_in_list  # type: ignore[attr-defined]

        result = _rust_check_unpacks_in_list([self._variadic_unpack(), self._variadic_unpack()])
        assert result is not None
        keep, final_unpack_idx = result
        assert keep == [0]
        assert final_unpack_idx == 1


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckWarnDeprecatedSuite(Suite):
    """Parity for the Rust `check_and_warn_deprecated` arbitration head.

    The deprecation-warning gate chain (deprecated string present,
    typeshed-stub gate, same-class exemption, prefix-exclusion list,
    ImportFrom-presence scan) is decided in Rust from scalar facts; the
    Python shim applies the `self.note` / `self.fail` side effect for the
    tag Rust returns (note when `report_deprecated_as_note`, else fail).
    Every fact is a scalar or string, so the seam never defers.

    Toggling the typeanal gate off (pure Python) and on (Rust seam) must
    produce identical captured (kind, message) pairs, and direct seam calls
    prove the silent/note/fail outcomes on every gate branch.
    """

    def setUp(self) -> None:
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

    def _analyser(
        self,
        *,
        exclude: Sequence[str] = (),
        report_note: bool = True,
        api_type: object = None,
        imports: Sequence[object] = (),
        stub: bool = False,
    ) -> tuple[object, list[tuple[str, str]]]:
        from mypy.typeanal import TypeAnalyser

        ta = TypeAnalyser.__new__(TypeAnalyser)
        captured: list[tuple[str, str]] = []
        ta.fail_func = lambda msg, ctx, code=None: captured.append(("fail", msg))  # type: ignore[assignment, misc]
        ta.note_func = lambda msg, ctx, code=None: captured.append(("note", msg))  # type: ignore[misc]

        class FakeApi:
            type = api_type

        ta.api = FakeApi()  # type: ignore[assignment]
        ta.is_typeshed_stub = stub
        ta.options = SimpleNamespace(  # type: ignore[assignment]
            deprecated_calls_exclude=list(exclude), report_deprecated_as_note=report_note
        )
        ta.cur_mod_node = SimpleNamespace(imports=list(imports))  # type: ignore[assignment]
        ta._captured = captured  # type: ignore[attr-defined]
        return ta, captured

    def _deprecated_info(
        self, fullname: str = "mod.Dep", deprecated: str | None = "use mod.B instead"
    ) -> TypeInfo:
        info = self.fx.make_type_info(fullname.rsplit(".", 1)[-1], module_name="mod")
        info._fullname = fullname
        info.deprecated = deprecated
        return info

    def _assert_par(self, **kwargs: Any) -> list[tuple[str, str]]:
        exclude = kwargs.pop("exclude", ())
        report_note = kwargs.pop("report_note", True)
        api_type = kwargs.pop("api_type", None)
        imports = kwargs.pop("imports", ())
        stub = kwargs.pop("stub", False)
        info = self._deprecated_info(**kwargs)
        off_ta, _ = self._analyser(
            exclude=exclude, report_note=report_note, api_type=api_type, imports=imports, stub=stub
        )
        off = self._with_gate(False, lambda: self._call(off_ta, info))
        on_ta, _ = self._analyser(
            exclude=exclude, report_note=report_note, api_type=api_type, imports=imports, stub=stub
        )
        on = self._with_gate(True, lambda: self._call(on_ta, info))
        assert_equal(on, off, f"deprecated parity {info.fullname}")
        return on

    def _call(self, ta: Any, info: TypeInfo) -> list[tuple[str, str]]:
        ta.check_and_warn_deprecated(info, Context())
        return list(ta._captured)

    def test_warns_as_note_by_default(self) -> None:
        captured = self._assert_par()
        assert_equal(captured, [("note", "use mod.B instead")])

    def test_warns_as_fail_when_notes_off(self) -> None:
        captured = self._assert_par(report_note=False)
        assert_equal(captured, [("fail", "use mod.B instead")])

    def test_no_deprecated_string_is_silent(self) -> None:
        captured = self._assert_par(deprecated=None)
        assert_equal(captured, [])

    def test_typeshed_stub_is_silent(self) -> None:
        captured = self._assert_par(stub=True)
        assert_equal(captured, [])

    def test_same_class_is_exempt(self) -> None:
        info = self._deprecated_info()
        captured = self._assert_par(api_type=info)
        assert_equal(captured, [])

    def test_exclude_exact_and_prefix(self) -> None:
        captured = self._assert_par(exclude=["mod.Dep"])
        assert_equal(captured, [])
        captured = self._assert_par(exclude=["mod"], fullname="mod.Dep.sub")
        assert_equal(captured, [])

    def test_exclude_respects_dot_boundary(self) -> None:
        # "mod.DepX" must not match an exclusion of "mod.Dep".
        captured = self._assert_par(exclude=["mod.Dep"], fullname="mod.DepX")
        assert_equal(captured, [("note", "use mod.B instead")])

    def test_import_from_suppresses_warning(self) -> None:

        captured = self._assert_par(imports=[ImportFrom("mod", 0, [("Dep", None)])])
        assert_equal(captured, [])

    def test_import_from_other_name_warns(self) -> None:

        captured = self._assert_par(imports=[ImportFrom("other", 0, [("X", None)])])
        assert_equal(captured, [("note", "use mod.B instead")])

    def _assert_direct(self, tag: int, **kwargs: Any) -> None:
        from mypy.typeanal import (  # type: ignore[attr-defined]
            _rust_classify_check_warn_deprecated,
        )

        result = _rust_classify_check_warn_deprecated(
            kwargs.get("deprecated", "x"),
            kwargs.get("stub", False),
            kwargs.get("api_fullname"),
            kwargs.get("fullname", "mod.Dep"),
            kwargs.get("name", "Dep"),
            kwargs.get("exclude", []),
            kwargs.get("report_note", True),
            kwargs.get("imports", []),
        )
        assert_equal(result, tag, f"direct seam {kwargs}")

    def test_direct_silent_paths(self) -> None:
        from mypy.typeanal import _DEPRECATED_TAG_SILENT

        self._assert_direct(_DEPRECATED_TAG_SILENT, deprecated=None)
        self._assert_direct(_DEPRECATED_TAG_SILENT, deprecated="")
        self._assert_direct(_DEPRECATED_TAG_SILENT, stub=True)
        self._assert_direct(_DEPRECATED_TAG_SILENT, api_fullname="mod.Dep")
        self._assert_direct(_DEPRECATED_TAG_SILENT, exclude=["mod.Dep"])
        self._assert_direct(_DEPRECATED_TAG_SILENT, imports=["Dep"])

    def test_direct_note_and_fail(self) -> None:
        from mypy.typeanal import _DEPRECATED_TAG_FAIL, _DEPRECATED_TAG_NOTE

        self._assert_direct(_DEPRECATED_TAG_NOTE, report_note=True)
        self._assert_direct(_DEPRECATED_TAG_FAIL, report_note=False)


# Moved from mypy/test/testtypes_native_types.py.
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
