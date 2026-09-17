"""Retired-seam pins for the checker area (#1739).

`...RetiredSuite` pins live in one file per area so that retirement lanes never
collide on a shared suite file. A retired seam has no Python shim (the module
calls the pure-Python body directly) while the Rust pyfunction stays registered
for the direct-seam tests in `testtypes_native_engagement_checker.py`. See the
retirement log at the top of `docs/plans/type-kernel-seam-ledger.md`.
"""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from types import SimpleNamespace
from typing import Any, cast
from unittest import skipUnless

from mypy.nodes import (
    Block,
    ExpressionStmt,
    FuncDef,
    IndexExpr,
    IntExpr,
    MemberExpr,
    NameExpr,
    ReturnStmt,
    StarExpr,
    TupleExpr,
    Var,
    YieldExpr,
)
from mypy.test.helpers import Suite
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED
from mypy.test.typefixture import TypeFixture
from mypy.types import PartialType, Type


def _loaded_rust_names(code: Any) -> list[str]:
    """Names a code object loads at call time; a retired body loads none."""
    return [n for n in code.co_names if "rust_" in n]


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsDefinitionRetiredSuite(Suite):
    """Pin the #1739 retirement of the `is_definition` Rust shim.

    `TypeChecker.is_definition` carried 73,298 calls per cold self-check.
    Measured min-of-7 ns/call with both arms in one process and the FFI ticket
    spied (200/200 decided on every shape), the PyO3 walk cost 6.3x-10.9x the
    Python body across four shapes: the body is a pair of isinstance checks
    over live attributes (`is_inferred_def`, `node`, `node.type`) with no
    allocation, while the shim resolved two `mypy.nodes` classes and re-read
    the same attributes through the FFI.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_shim_and_alias_removed(self) -> None:
        import inspect

        from mypy import checker

        assert not hasattr(checker, "_rust_is_definition")
        src = inspect.getsource(checker.TypeChecker.is_definition)
        assert "rust_" not in src, "is_definition should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.checker import TypeChecker, _set_native_checker_active

        _set_native_checker_active(True)
        try:
            loaded = _loaded_rust_names(TypeChecker.is_definition.__code__)
        finally:
            _set_native_checker_active(False)
        assert loaded == [], f"is_definition still loads {loaded}"

    def test_the_co_names_scan_bites(self) -> None:
        # Negative control: the scan above must flag a code object that loads
        # a live rust_* alias, or a green `loaded == []` proves nothing. This
        # probe is a wired neighbour; retarget it if that seam ever retires.
        from mypy import checker
        from mypy.checker import TypeChecker

        found = [
            n for n in TypeChecker.find_isinstance_check_helper.__code__.co_names if "rust_" in n
        ]
        live = [n for n in found if callable(getattr(checker, n, None))]
        assert live, f"no live rust_* alias in find_isinstance_check_helper: {found}"

    def _cases(self) -> list[tuple[Any, bool]]:
        fx = self.fx
        name_inferred = NameExpr("x")
        name_inferred.is_inferred_def = True
        name_var_typed = NameExpr("x")
        name_var_typed.node = Var("x", fx.a)
        name_var_untyped = NameExpr("x")
        name_var_untyped.node = Var("x")
        name_other_node = NameExpr("x")
        name_other_node.node = SimpleNamespace()  # type: ignore[assignment]
        member_inferred = MemberExpr(NameExpr("o"), "attr")
        member_inferred.is_inferred_def = True
        return [
            (name_inferred, True),
            (name_var_untyped, True),
            (name_var_typed, False),
            (name_other_node, False),
            (NameExpr("x"), False),
            (member_inferred, True),
            (MemberExpr(NameExpr("o"), "attr"), False),
            (IndexExpr(NameExpr("a"), NameExpr("b")), False),
        ]

    def test_values_match_python_with_gate_on(self) -> None:
        from mypy.checker import TypeChecker, _set_native_checker_active

        cases = self._cases()
        chk = TypeChecker.__new__(TypeChecker)

        def run() -> list[bool]:
            return [chk.is_definition(node) for node, _ in cases]

        _set_native_checker_active(False)
        try:
            expected = run()
        finally:
            _set_native_checker_active(True)
        try:
            got = run()
        finally:
            _set_native_checker_active(False)
        assert expected == [want for _, want in cases], f"unexpected values: {expected}"
        assert got == expected, f"gate-on values diverged: {got} != {expected}"

    def test_pyfunction_stays_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_is_definition")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassifyCheckLvalueRetiredSuite(Suite):
    """Pin the #1739 retirement of the `classify_check_lvalue` Rust shim.

    73,298 calls per cold self-check. The shim resolved seven `mypy.nodes`
    classes and imported `mypy.types` for a `PartialType` lookup on *every*
    call, to decide a tag whose Python twin is a short isinstance chain over
    the node the caller already holds. Measured min-of-7 ns/call with both arms
    in one process and the FFI ticket spied (200/200 decided on every shape),
    the shim cost 5.0x-6.2x the Python classification across four shapes.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_shim_and_alias_removed(self) -> None:
        import inspect

        from mypy import checker

        assert not hasattr(checker, "_rust_classify_check_lvalue")
        src = inspect.getsource(checker.TypeChecker.check_lvalue)
        assert "rust_" not in src, "check_lvalue should be pure Python"

    def test_tag_constants_removed(self) -> None:
        from mypy import checker

        stale = [n for n in dir(checker) if n.startswith("NATIVE_LVALUE_")]
        assert stale == [], f"orphaned lvalue tags: {stale}"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.checker import TypeChecker, _set_native_checker_active

        _set_native_checker_active(True)
        try:
            loaded = _loaded_rust_names(TypeChecker.check_lvalue.__code__)
        finally:
            _set_native_checker_active(False)
        assert loaded == [], f"check_lvalue still loads {loaded}"

    def _run(self, lvalue: Any, *, allow_redefinition: bool = False) -> tuple[Any, Any, Any]:
        from mypy.checker import TypeChecker

        chk = TypeChecker.__new__(TypeChecker)
        chk.options = SimpleNamespace(allow_redefinition=allow_redefinition)  # type: ignore[assignment]
        chk._expr_checker = SimpleNamespace(  # type: ignore[assignment]
            accept=lambda expr: "accept",
            analyze_ordinary_member_access=lambda lv, is_lvalue, rvalue: "member_access",
            analyze_ref_expr=lambda lv, lvalue: "ref_expr",
        )
        chk.store_type = lambda lv, t: None  # type: ignore[assignment]
        chk.named_type = lambda name: self.fx.std_tuple  # type: ignore[method-assign]
        return chk.check_lvalue(lvalue)

    def _star(self) -> StarExpr:
        star = StarExpr(NameExpr("xs"))
        star.expr.node = Var("xs", self.fx.a)  # type: ignore[attr-defined]
        return star

    def _shapes(self) -> list[Any]:
        return [self._name(), self._member(), self._index(), self._member_def(), self._star()]

    def _name(self) -> NameExpr:
        name = NameExpr("x")
        name.node = Var("x", self.fx.a)
        return name

    def _member(self) -> MemberExpr:
        return MemberExpr(NameExpr("base"), "attr")

    def _index(self) -> IndexExpr:
        return IndexExpr(NameExpr("a"), NameExpr("b"))

    def _member_def(self) -> MemberExpr:
        member = MemberExpr(NameExpr("base"), "attr")
        member.is_inferred_def = True
        member.def_var = Var("attr")
        return member

    def _project(self, lvalue: Any) -> tuple[Any, bool, bool]:
        """Value-comparable projection; the raw tuple holds node identities."""
        lvalue_type, index_lvalue, inferred = self._run(lvalue)
        if not isinstance(lvalue_type, str) and lvalue_type is not None:
            lvalue_type = type(lvalue_type).__name__
        return (lvalue_type, index_lvalue is not None, inferred is not None)

    def test_values_match_python_with_gate_on(self) -> None:
        from mypy.checker import _set_native_checker_active

        def run() -> list[tuple[Any, bool, bool]]:
            return [self._project(shape) for shape in self._shapes()]

        _set_native_checker_active(False)
        try:
            off = run()
        finally:
            _set_native_checker_active(True)
        try:
            on = run()
        finally:
            _set_native_checker_active(False)
        expected = [
            ("ref_expr", False, False),
            ("member_access", False, False),
            (None, True, False),
            (None, False, True),
            ("ref_expr", False, False),
        ]
        assert off == expected, f"unexpected values: {off}"
        assert on == off, f"gate-on values diverged: {on} != {off}"

        index = self._index()
        assert self._run(index) == (None, index, None)
        member_def = self._member_def()
        assert self._run(member_def) == (None, None, member_def.def_var)
        lvalue_type, index_lvalue, inferred = self._run(TupleExpr([NameExpr("a")]))
        assert index_lvalue is None and inferred is None
        assert type(lvalue_type).__name__ == "TupleType", f"tuple lvalue: {lvalue_type!r}"

    def test_allow_redefinition_promotes_an_inferred_name(self) -> None:
        name = self._name()
        assert isinstance(name.node, Var)
        name.node.is_inferred = True
        lvalue_type, index_lvalue, inferred = self._run(name, allow_redefinition=True)
        assert (lvalue_type, index_lvalue, inferred) == ("ref_expr", None, name.node)
        plain = self._run(name)
        assert plain == ("ref_expr", None, None), f"gate off should not infer: {plain}"

    def test_pyfunction_stays_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_classify_check_lvalue")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassifyCheckAssignmentRetiredSuite(Suite):
    """Pin the #1739 retirement of the `classify_check_assignment` Rust shim.

    67,098 calls per cold self-check. The shim re-derived the special-name
    front and the `lvalue_type` branch from live objects: two `mypy.nodes`
    class lookups, a `mypy.types` import for a `PartialType` lookup whenever an
    lvalue type was present, and a string extract for every name. Measured
    min-of-7 ns/call with both arms in one process and the FFI ticket spied
    (200/200 decided on every shape), the classification cost 6.2x-10.5x the
    Python isinstance chain on its own, and the production `check_assignment`
    arm was slower with the gate on.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_shim_and_alias_removed(self) -> None:
        import inspect

        from mypy import checker

        assert not hasattr(checker, "_rust_classify_check_assignment")
        src = inspect.getsource(checker.TypeChecker.check_assignment)
        assert "rust_" not in src, "check_assignment should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.checker import TypeChecker, _set_native_checker_active

        _set_native_checker_active(True)
        try:
            loaded = _loaded_rust_names(TypeChecker.check_assignment.__code__)
        finally:
            _set_native_checker_active(False)
        assert loaded == [], f"check_assignment still loads {loaded}"

    def test_the_co_names_scan_bites(self) -> None:
        # Negative control: the scan above must flag a code object that loads
        # a live rust_* alias, or a green `loaded == []` proves nothing. This
        # probe is a wired neighbour; retarget it if that seam ever retires.
        from mypy import checker
        from mypy.checker import TypeChecker

        found = [n for n in TypeChecker.check_match_args.__code__.co_names if "rust_" in n]
        live = [n for n in found if callable(getattr(checker, n, None))]
        assert live, f"no live rust_* alias in check_match_args: {found}"

    def _run(
        self, lvalue: Any, lvalue_type: Type | None, inferred: Any, active_class: Any = None
    ) -> tuple[str, ...]:
        from mypy.checker import TypeChecker
        from mypy.types import Instance

        rvalue = IntExpr(0)
        rt: Type = Instance(self.fx.ai, [])

        obs: list[str] = []
        chk = cast(Any, TypeChecker.__new__(TypeChecker))
        chk.options = SimpleNamespace(allow_redefinition=False)
        chk.is_stub = False
        chk.current_node_deferred = False
        chk.can_skip_diagnostics = True
        chk.var_decl_frames = {}
        chk._expr_checker = SimpleNamespace(
            accept=lambda expr, type_context=None, always_allow_any=False: rt
        )
        chk.scope = SimpleNamespace(active_class=lambda: active_class)
        chk.binder = SimpleNamespace(
            frames=[],
            put=lambda lv, t: obs.append("put"),
            assign_type=lambda lv, rvt, lvt: obs.append("assign"),
        )
        chk.msg = SimpleNamespace(concrete_only_assign=lambda lt, rv: obs.append("c_only"))
        chk.try_infer_partial_generic_type_from_assignment = lambda lv, rv, op: obs.append("pg")
        chk.check_lvalue = lambda lv, rv=None: (lvalue_type, None, inferred)
        chk.fail = lambda msg, ctx: obs.append("fail")
        chk.check_setattr_method = lambda sig, lv: obs.append("setattr")
        chk.check_getattr_method = lambda sig, lv, name: obs.append("getattr")
        chk.check_slots_definition = lambda typ, lv: obs.append("slots_def")
        chk.check_match_args = lambda typ, t, lv: obs.append("match_args")

        def _member_assignment(
            lv: Any, it: Any, lt: Any, rv: Any, context: Any = None
        ) -> tuple[Any, Any, bool]:
            obs.append("member")
            return rt, lt, True

        def _simple_assignment(
            lt: Any, rv: Any, context: Any = None, inferred: Any = None, lvalue: Any = None
        ) -> tuple[Any, Any]:
            obs.append("simple")
            return rt, lt

        chk.check_member_assignment = _member_assignment
        chk.check_simple_assignment = _simple_assignment
        chk.check_indexed_assignment = lambda il, rv, lv: obs.append("indexed")
        chk.get_variable_type_context = lambda inf, rv: None
        chk.infer_variable_type = lambda inf, lv, rvt, rv: obs.append("infer_var")
        chk.check_assignment_to_slots = lambda lv: obs.append("slots")
        chk.find_partial_types = lambda var: {var: None}
        chk.set_inferred_type = lambda var, lv, t: obs.append("set_type")
        chk.infer_partial_type = lambda var, lv, rvt: False
        chk.inference_error_fallback_type = lambda rvt: rvt
        chk.check_assignment_to_multiple_lvalues = lambda i, rv, c, ilt: obs.append("multi")
        chk.check_assignment(lvalue, rvalue)
        return tuple(obs)

    def _named(self, name: str) -> NameExpr:
        node = NameExpr(name)
        node.node = Var(name, self.fx.a) if name == "x" else Var(name)
        return node

    def _cases(self) -> list[tuple[Any, Any, Any, Any, tuple[str, ...], tuple[str, ...]]]:
        partial_lv = NameExpr("x")
        partial_lv.node = Var("x")
        partial_lv.node.type = PartialType(None, partial_lv.node)
        member = MemberExpr(NameExpr("base"), "attr")
        return [
            (
                self._named("x"),
                self.fx.a,
                None,
                None,
                ("simple", "assign"),
                ("setattr", "slots_def", "member"),
            ),
            (
                self._named("__slots__"),
                None,
                None,
                SimpleNamespace(),
                ("slots_def",),
                ("simple", "setattr"),
            ),
            (self._named("__setattr__"), None, None, None, ("setattr",), ("simple", "slots_def")),
            (
                self._named("__match_args__"),
                None,
                Var("x"),
                None,
                ("match_args",),
                ("simple", "setattr"),
            ),
            (
                self._named("__post_init__"),
                None,
                None,
                SimpleNamespace(metadata={"dataclass"}),
                ("fail",),
                ("simple", "setattr"),
            ),
            (
                MemberExpr(NameExpr("b"), "__match_args__"),
                None,
                None,
                None,
                ("fail",),
                ("simple", "member"),
            ),
            (member, self.fx.a, None, None, ("member",), ("simple", "slots_def")),
            (
                partial_lv,
                PartialType(None, Var("x")),
                None,
                None,
                ("set_type",),
                ("simple", "member"),
            ),
        ]

    def test_values_match_python_with_gate_on(self) -> None:
        from mypy.checker import _set_native_checker_active

        cases = self._cases()

        def run() -> list[tuple[str, ...]]:
            return [self._run(lv, lt, inf, ac) for lv, lt, inf, ac, _, _ in cases]

        _set_native_checker_active(False)
        try:
            expected = run()
        finally:
            _set_native_checker_active(True)
        try:
            got = run()
        finally:
            _set_native_checker_active(False)
        assert got == expected, f"gate-on values diverged: {got} != {expected}"
        for obs, (_, _, _, _, present, absent) in zip(expected, cases):
            missing = [m for m in present if m not in obs]
            wrong = [m for m in absent if m in obs]
            assert (
                not missing and not wrong
            ), f"branch evidence {obs}: missing={missing} unexpected={wrong}"

    def test_pyfunction_stays_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_classify_check_assignment")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsEmptyGeneratorFunctionRetiredSuite(Suite):
    """Pin the #1739 retirement of the `_is_empty_generator_function` shim.

    31,630 calls per cold self-check. The body is a `len(body) == 2` guard plus
    three isinstance checks; the shim crossed the FFI to read `func.body.body`
    and resolve three `mypy.nodes` classes. Measured min-of-7 ns/call with both
    arms in one process, the shim cost 2.9x-8.8x the Python body across three
    shapes, the corpus-dominant short-body shape included. The pyfunction
    returns a plain `bool`, so it can never defer and the engagement spy
    recorded an answer on every one of the 200 calls per shape.
    """

    def test_shim_and_alias_removed(self) -> None:
        import inspect

        from mypy import checker

        assert not hasattr(checker, "_rust_is_empty_generator_function")
        src = inspect.getsource(checker._is_empty_generator_function)
        assert "rust_" not in src, "_is_empty_generator_function should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.checker import _is_empty_generator_function, _set_native_checker_stmts_active

        _set_native_checker_stmts_active(True)
        try:
            loaded = _loaded_rust_names(_is_empty_generator_function.__code__)
        finally:
            _set_native_checker_stmts_active(False)
        assert loaded == [], f"_is_empty_generator_function still loads {loaded}"

    def _cases(self) -> list[tuple[FuncDef, bool]]:
        none_name = NameExpr("None")
        none_name.fullname = "builtins.None"

        def func(stmts: list[Any]) -> FuncDef:
            return FuncDef("f", [], Block(stmts))

        return [
            (func([ReturnStmt(None), ExpressionStmt(YieldExpr(None))]), True),
            (func([ReturnStmt(None), ExpressionStmt(YieldExpr(none_name))]), True),
            (func([ExpressionStmt(IntExpr(i)) for i in range(6)]), False),
            (func([ReturnStmt(None), ExpressionStmt(IntExpr(1))]), False),
            (func([ReturnStmt(IntExpr(1)), ExpressionStmt(YieldExpr(None))]), False),
            (func([ExpressionStmt(YieldExpr(None)), ReturnStmt(None)]), False),
            (func([ReturnStmt(None)]), False),
        ]

    def test_values_match_python_with_gate_on(self) -> None:
        from mypy.checker import _is_empty_generator_function, _set_native_checker_stmts_active

        cases = self._cases()

        def run() -> list[bool]:
            return [_is_empty_generator_function(f) for f, _ in cases]

        _set_native_checker_stmts_active(False)
        try:
            expected = run()
        finally:
            _set_native_checker_stmts_active(True)
        try:
            got = run()
        finally:
            _set_native_checker_stmts_active(False)
        assert expected == [want for _, want in cases], f"unexpected values: {expected}"
        assert got == expected, f"gate-on values diverged: {got} != {expected}"

    def test_pyfunction_stays_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_is_empty_generator_function")
