"""Retired-seam pins for the checkexpr area (#1739, #1746).

`...RetiredSuite` pins live in one file per area so that retirement lanes never
collide on a shared suite file. A retired seam has no Python shim (the module
calls the pure-Python body directly) while the Rust pyfunction stays registered
for direct-seam tests. See the retirement log at the top of
`docs/plans/type-kernel-seam-ledger.md`.
"""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from unittest import skipUnless

from mypy.nodes import ARG_POS, ARG_STAR, ARG_STAR2, ArgKind
from mypy.test.helpers import Suite
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED
from mypy.test.typefixture import TypeFixture
from mypy.types import CallableType, Type, TypedDictType


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeIsDuplicateMappingRetiredSuite(Suite):
    """Pin the #1739 retirement of the `is_duplicate_mapping` Rust shim.

    It was the highest-call seam on `main` (342,101 calls per cold self-check)
    and the one #1739's census omitted, because that table filters "0 defers"
    and this seam deferred 24%. Measured min-of-7 ns/call with both arms in one
    process and the FFI ticket spied (200/200 decided on every shape), the wire
    path cost 2.9x-8.6x the Python body: the body is a `len(mapping) > 1` guard
    plus two short exemptions, while the shim serialized every mapped actual
    type and crossed the FFI with a resolver. Same class as #1640/#1741.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        self.td = TypedDictType({"x": self.fx.a}, {"x"}, set(), self.fx.a)

    def test_shim_and_alias_removed(self) -> None:
        import inspect

        from mypy import checkexpr

        assert not hasattr(checkexpr, "_rust_is_duplicate_mapping")
        src = inspect.getsource(checkexpr.is_duplicate_mapping)
        assert "rust_" not in src, "is_duplicate_mapping should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, is_duplicate_mapping

        _set_native_checkexpr_active(True)
        try:
            loaded = [n for n in is_duplicate_mapping.__code__.co_names if "rust_" in n]
        finally:
            _set_native_checkexpr_active(False)
        assert loaded == [], f"is_duplicate_mapping still loads {loaded}"

    def test_values_match_python_with_gate_on(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active, is_duplicate_mapping

        a, b, td = self.fx.a, self.fx.b, self.td
        cases: list[tuple[list[int], list[Type], list[ArgKind], bool]] = [
            ([0], [a], [ARG_POS], False),
            ([0, 1], [a, b], [ARG_POS, ARG_POS], True),
            ([0, 1], [a, b], [ARG_STAR, ARG_STAR2], False),
            ([0, 1], [a, b], [ARG_STAR2, ARG_STAR2], False),
            ([0, 1], [a, td], [ARG_STAR2, ARG_STAR2], True),
        ]
        _set_native_checkexpr_active(False)
        try:
            expected = [is_duplicate_mapping(m, t, k) for m, t, k, _ in cases]
        finally:
            _set_native_checkexpr_active(True)
        got = [is_duplicate_mapping(m, t, k) for m, t, k, _ in cases]
        assert got == expected, f"gate-on values diverged: {got} != {expected}"
        assert got == [exp for *_, exp in cases], f"unexpected values: {got}"
        _set_native_checkexpr_active(False)

    def test_pyfunction_stays_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_is_duplicate_mapping")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassifyProtocolTestCalleeRetiredSuite(Suite):
    """Pin the #1739 retirement of the `classify_protocol_test_callee` shim.

    The seam re-derived across the FFI a tag `visit_call_expr_inner` already
    held on live AST nodes: whether the callee is a `RefExpr` with an
    isinstance/issubclass fullname and exactly two args. It measured
    1.3x-3.7x the Python predicate and decided on 0/200 calls on the two
    common shapes, so the crossing was doomed work. This path is now pure
    Python; the Rust pyfunction stays registered for the direct-seam tests in
    `NativeEnumProtocolClassifierSuite`.
    """

    def test_shim_and_alias_removed(self) -> None:
        import inspect

        from mypy import checkexpr

        assert not hasattr(checkexpr, "_rust_classify_protocol_test_callee")
        src = inspect.getsource(checkexpr.ExpressionChecker.visit_call_expr_inner)
        assert "rust_" not in src, "visit_call_expr_inner should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy import checkexpr
        from mypy.checkexpr import ExpressionChecker, _set_native_checkexpr_active

        _orig_gate = checkexpr._native_checkexpr_active
        _set_native_checkexpr_active(True)
        try:
            loaded = [
                n
                for n in ExpressionChecker.visit_call_expr_inner.__code__.co_names
                if "rust_" in n
            ]
        finally:
            _set_native_checkexpr_active(_orig_gate)
        assert loaded == [], f"visit_call_expr_inner still loads {loaded}"

    def test_protocol_branches_remain(self) -> None:
        # The retired seam only gated two downstream calls; both must survive
        # so the isinstance/issubclass work still runs on the Python path.
        import inspect

        from mypy.checkexpr import ExpressionChecker

        src = inspect.getsource(ExpressionChecker.visit_call_expr_inner)
        assert "self.check_runtime_protocol_test(e)" in src
        assert "self.check_protocol_issubclass(e)" in src


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckCallHeadRetiredSuite(Suite):
    """Pin the #1739 retirement of the `check_call_head` batch shim.

    The batch merged `is_enum_callable_base` + `classify_typeobj_gate` into one
    FFI crossing (#1642, "saves ~183k crossings on cold self-check"). Measured
    min-of-7 ns/call with every arm in one process and the FFI ticket spied
    (200/200 decided on all five head shapes), it cost 1.24x-8.38x the Python
    body on every shape (3.04x plain non-type-object callee, 8.38x the enum
    arm), while being a wash against the sibling chain it shadowed
    (0.95x-1.08x): the enum half rebuilt a 5-string `HashSet` from `ENUM_BASES`
    on every call to answer a predicate Python does in ~90 ns.

    Its enum half is retired with it -- measured on its own shapes it cost
    6.80x-12.37x the `isinstance(...) and fullname in ENUM_BASES` predicate, so
    landing the batch on that chain would have made the head slower.

    The typeobj gate is **kept**: it measured 0.70x-0.91x the Python if/elif on
    the type-object arms. It stays registered and wired, so this file asserts
    it too. The enum arm returns before the gate, which is a pure decision
    function, so the gate no longer runs on a path that discards its tag.
    """

    def test_shim_and_alias_removed(self) -> None:
        import inspect

        from mypy import checkexpr

        assert not hasattr(checkexpr, "_rust_check_call_head")
        assert not hasattr(checkexpr, "_rust_is_enum_callable_base")
        src = inspect.getsource(checkexpr.ExpressionChecker.check_callable_call)
        for dead in ("_rust_check_call_head", "_rust_is_enum_callable_base", "enum_hit"):
            assert dead not in src, f"check_callable_call still carries {dead}"
        arg_src = inspect.getsource(checkexpr.ExpressionChecker.infer_arg_types_in_context)
        assert "rust_" not in arg_src, "infer_arg_types_in_context should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.checkexpr import ExpressionChecker, _set_native_checkexpr_active

        dead = ("_rust_check_call_head", "_rust_is_enum_callable_base")
        names = ExpressionChecker.check_callable_call.__code__.co_names
        _set_native_checkexpr_active(True)
        try:
            loaded = [n for n in names if n in dead]
        finally:
            _set_native_checkexpr_active(False)
        assert loaded == [], f"check_callable_call still loads {loaded}"

    def test_enum_predicate_is_the_only_guard(self) -> None:
        # The enum arm must stay a plain Python predicate on the live node.
        import inspect

        from mypy.checkexpr import ExpressionChecker

        src = inspect.getsource(ExpressionChecker.check_callable_call)
        assert "isinstance(callable_node, RefExpr) and callable_node.fullname in ENUM_BASES" in src
        assert "check_enum_call()" in src

    def test_typeobj_gate_stays_wired(self) -> None:
        # The kept seam: registered, aliased, and still the first tag source.
        import inspect

        from mypy import checkexpr

        assert hasattr(checkexpr, "_rust_classify_typeobj_gate")
        src = inspect.getsource(checkexpr.ExpressionChecker.check_callable_call)
        assert "_rust_classify_typeobj_gate(callee)" in src

    def test_pyfunctions_stay_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_check_call_head")
        assert hasattr(_type_kernel, "rust_is_enum_callable_base")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeComputeArgContextIndicesRetiredSuite(Suite):
    """Pin the #1739 retirement of the `compute_arg_context_indices` shim.

    The seam took scalars (no wire serializer) but rebuilt, across the FFI, an
    O(n) index map the caller then re-materialised: `[ak.value for ak in
    arg_kinds]` on the way in, a `list[int]` on the way out, plus the crossing.
    Measured min-of-7 ns/call with both arms in one process and the FFI ticket
    spied (200/200 decided on all five shapes), it cost **1.37x-1.59x** the
    Python double loop on every shape. Same class as #1640/#1741: the Python
    body is an O(n) rebuild.
    """

    def test_shim_and_alias_removed(self) -> None:
        import inspect

        from mypy import checkexpr

        assert not hasattr(checkexpr, "_rust_compute_arg_context_indices")
        src = inspect.getsource(checkexpr.ExpressionChecker.infer_arg_types_in_context)
        assert "rust_" not in src, "infer_arg_types_in_context should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.checkexpr import ExpressionChecker, _set_native_checkexpr_active

        _set_native_checkexpr_active(True)
        try:
            loaded = [
                n
                for n in ExpressionChecker.infer_arg_types_in_context.__code__.co_names
                if "rust_" in n
            ]
        finally:
            _set_native_checkexpr_active(False)
        assert loaded == [], f"infer_arg_types_in_context still loads {loaded}"

    def test_pyfunction_stays_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_compute_arg_context_indices")

    def _contexts(
        self, arg_kinds: list[ArgKind], formal_to_actual: list[list[int]], n_args: int
    ) -> list[Type]:
        """The surviving Python path's output: one context per actual."""
        from mypy.checkexpr import ExpressionChecker
        from mypy.nodes import Expression, TempNode

        fx = TypeFixture()
        callee: CallableType = CallableType(
            [fx.a, fx.b, fx.c], [ARG_POS, ARG_POS, ARG_POS], [None, None, None], fx.o, fx.function
        )
        ec = ExpressionChecker.__new__(ExpressionChecker)

        def accept(arg: Expression, ctx: Type | None = None) -> Type:
            return ctx  # type: ignore[return-value]

        ec.accept = accept  # type: ignore[assignment]
        args: list[Expression] = [TempNode(fx.o) for _ in range(n_args)]
        return list(ec.infer_arg_types_in_context(callee, args, arg_kinds, formal_to_actual))

    def test_values_1to1(self) -> None:
        out = self._contexts([ARG_POS, ARG_POS], [[0], [1]], 2)
        assert [t is not None for t in out] == [True, True]

    def test_values_star_actual_skipped(self) -> None:
        # `arg_kinds[ai].is_star()` keeps the star actual's context None.
        out = self._contexts([ARG_POS, ARG_STAR, ARG_STAR2], [[0, 1, 2]], 3)
        assert [t is not None for t in out] == [True, False, False]

    def test_values_unmapped_actual(self) -> None:
        out = self._contexts([ARG_POS, ARG_POS, ARG_POS], [[0], [], [2]], 3)
        assert [t is not None for t in out] == [True, False, True]
