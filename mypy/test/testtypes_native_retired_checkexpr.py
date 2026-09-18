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

from mypy.nodes import (
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    MDEF,
    ArgKind,
    Context,
    MypyFile,
    SymbolTable,
    SymbolTableNode,
    TypeInfo,
)
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

    Its constituent typeobj gate was **kept by this PR** on a per-shape
    rule ("wins on the abstract arm, so keep"). #1833 measured that gate
    against production shape weights and retired it, so the
    `NativeClassifyTypeobjGateRetiredSuite` below now owns its pins and the
    enum arm returning before the gate is moot: nothing is left to call.
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

    def test_typeobj_gate_call_is_gone_too(self) -> None:
        # #1833 retired the gate this PR kept; the head must carry no trace of
        # it. The seam's own pins live in the #1833 suite below.
        import inspect

        from mypy import checkexpr

        assert not hasattr(checkexpr, "_rust_classify_typeobj_gate")
        src = inspect.getsource(checkexpr.ExpressionChecker.check_callable_call)
        assert "rust_classify_typeobj_gate" not in src

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


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeClassifyTypeobjGateRetiredSuite(Suite):
    """Pin the #1833 retirement of the `classify_typeobj_gate` shim.

    #1830 kept this seam on a per-shape rule: it beat the Python if/elif on
    the abstract arm (0.68x-0.70x) and lost on the protocol arm, so "wins on
    some shape" read as keep. The frequency evidence shows the two shapes that
    decided that rule barely occur: over a cold self-check (183,318 crossings,
    tag/shape agreement checked per crossing) the split is 88.26%
    non-type-object, 11.74% type-object with no arm, **0 protocol**, 1
    abstract; a second corpus (73,018 crossings, `-p sphinx -p _pytest`) is
    89.79% / 10.21% / 0 / 0.

    Weighted by those frequencies the crossing costs 1.02x-1.07x the Python
    body (`sum(f*native)/sum(f*python)`, two timing runs x two corpora), so the
    retirement is a small net win: 12% of crossings win 0.92x-0.94x, but 88%
    pay 1.07x-1.14x, and the arm that justified the keep is a single call.

    The tag if/elif/else is now the only source. The Rust pyfunction stays
    registered for the direct-seam tests in `NativeTypeobjGateSuite`.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_shim_and_alias_removed(self) -> None:
        import inspect

        from mypy import checkexpr

        assert not hasattr(checkexpr, "_rust_classify_typeobj_gate")
        src = inspect.getsource(checkexpr.ExpressionChecker.check_callable_call)
        assert "rust_classify_typeobj_gate" not in src, "check_callable_call should be pure Python"

    def test_no_rust_name_in_co_names(self) -> None:
        # Not "no rust_ name at all": the head legitimately still loads
        # `_rust_solve_generic_call` and `_rust_calibrate_type_obj_return`.
        # `co_names` is static, so this pins the source, not the toggle (#1847).
        from mypy.checkexpr import ExpressionChecker

        dead = ("_rust_classify_typeobj_gate",)
        names = ExpressionChecker.check_callable_call.__code__.co_names
        loaded = [n for n in names if n in dead]
        assert loaded == [], f"check_callable_call still loads {loaded}"

    def test_tag_default_is_gone_and_all_arms_assign(self) -> None:
        # The `tag: int | None = None` defer default was the thing a failing
        # seam fell back through; with it gone the chain must be total.
        import inspect

        from mypy.checkexpr import ExpressionChecker

        src = inspect.getsource(ExpressionChecker.check_callable_call)
        assert "tag: int | None = None" not in src
        for arm in (
            "NATIVE_TYPEOBJ_GATE_PROTOCOL",
            "NATIVE_TYPEOBJ_GATE_ABSTRACT",
            "NATIVE_TYPEOBJ_GATE_NONE",
        ):
            assert arm in src, f"check_callable_call lost the {arm} arm"
        assert "and not callee.from_type_type" in src
        assert "and not callee.type_object().fallback_to_any" in src

    def test_constants_match_the_rust_arms(self) -> None:
        # The tag values the direct-seam tests assert as raw ints.
        from mypy import checkexpr

        assert checkexpr.NATIVE_TYPEOBJ_GATE_NONE == 0
        assert checkexpr.NATIVE_TYPEOBJ_GATE_PROTOCOL == 1
        assert checkexpr.NATIVE_TYPEOBJ_GATE_ABSTRACT == 2

    def test_pyfunction_stays_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_classify_typeobj_gate")

    # --- the surviving Python gate still emits both fails with the gate on ---

    def _fails(self, callee: CallableType, gate: bool) -> list[str]:
        from mypy.checker import TypeChecker
        from mypy.checkexpr import ExpressionChecker, _set_native_checkexpr_active
        from mypy.errors import Errors
        from mypy.messages import MessageBuilder
        from mypy.options import Options
        from mypy.plugin import Plugin

        options = Options()
        errors = Errors(options)
        tree = MypyFile([], [])
        tree.is_stub = True
        tree.names = SymbolTable()
        # A bare checker must resolve `typing`/`builtins`: the call proceeds
        # past the gate into `check_argument_types`, and the KeyError it hit
        # there used to be swallowed, so a crash read as "no fail fired" (#1847).
        modules = {
            "typing": self._module_tree(("Mapping", "Iterable", "Sequence"), self.fx),
            "builtins": self._module_tree(
                (
                    "dict",
                    "list",
                    "tuple",
                    "set",
                    "frozenset",
                    "type",
                    "object",
                    "int",
                    "str",
                    "bytes",
                    "bool",
                    "float",
                ),
                self.fx,
            ),
        }
        chk = TypeChecker(errors, modules, options, tree, "", Plugin(options), {})
        ec = ExpressionChecker(chk, MessageBuilder(errors, {}), Plugin(options), {})
        captured: list[str] = []
        ec.chk.fail = lambda m, ctx, code=None: captured.append("protocol")  # type: ignore[method-assign, misc, assignment]
        ec.msg.cannot_instantiate_abstract_class = lambda name, attrs, ctx: captured.append(  # type: ignore[method-assign, assignment]
            "abstract"
        )
        _set_native_checkexpr_active(gate)
        try:
            ec.check_callable_call(callee, [], [], Context(), None, None, None, None)
        finally:
            _set_native_checkexpr_active(False)
        return captured

    @staticmethod
    def _module_tree(names: tuple[str, ...], fx: TypeFixture) -> MypyFile:
        """A MypyFile instantiating each bare-name lookup the harness needs."""

        tree = MypyFile([], [])
        tree.names = SymbolTable()
        for name in names:
            info = fx.make_type_info(f"typing.{name}" if name[0].isupper() else name)
            tree.names[name] = SymbolTableNode(MDEF, info)
        return tree

    def _type_object_callable(self, info: TypeInfo, from_type_type: bool = False) -> CallableType:
        from mypy.types import Instance

        callee = self.fx.callable_type(self.fx.a, Instance(info, []))
        callee.from_type_type = from_type_type
        return callee

    def _protocol_info(self) -> TypeInfo:
        info = self.fx.make_type_info("ProtoKlass")
        assert info is not None
        info.is_protocol = True
        return info

    def _abstract_info(self) -> TypeInfo:
        info = self.fx.make_type_info("AbsKlass")
        assert info is not None
        info.is_abstract = True
        return info

    def test_protocol_fail_still_fires_gate_on(self) -> None:
        assert "protocol" in self._fails(self._type_object_callable(self._protocol_info()), True)

    def test_abstract_fail_still_fires_gate_on(self) -> None:
        assert "abstract" in self._fails(self._type_object_callable(self._abstract_info()), True)

    def test_plain_typeobj_fires_no_gate_fail(self) -> None:
        # `_fails` no longer swallows the KeyError a bare checker raised, so a
        # crash under the call fails this test rather than satisfying it (#1847).
        got = self._fails(self.fx.callable_type(self.fx.a, self.fx.b), True)
        assert "protocol" not in got
        assert "abstract" not in got

    def test_gate_toggle_is_inert_for_this_body(self) -> None:
        # With no Rust arm left the toggle cannot change the captured fails;
        # a re-wired call would show up here as a differential. A raise cannot
        # masquerade as a differential: it propagates out of `_fails` (#1847).
        for info in (self._protocol_info(), self._abstract_info()):
            callee = self._type_object_callable(info)
            assert self._fails(callee, False) == self._fails(callee, True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckArgClassifyRetiredSuite(Suite):
    """Pin the #1654 unwiring of the `classify_check_arg` wire seam (#1834).

    fcc1f8e2c removed the call from `ExpressionChecker.check_arg` after
    measuring 233k calls / 10.3MB wire / 0.60s proxy on a cold self-check:
    the classifier read one DeletedType wire tag plus two Python-computed
    booleans, which the pure-Python if/elif chain answers without serializing
    `caller_type` on every call. Its message ends "The Rust pyfunction stays
    registered for direct-seam tests", and the pyfunction is registered. What
    it did not add is this pin, so until now nothing distinguished "retired
    deliberately" from "wired off by accident", and `NativeCheckArgSuite`'s
    gate-off/gate-on comparisons could not fail.

    The detector is the `co_names` scan, not `hasattr`: unlike the #1739
    retirements this lane keeps the import (checkexpr.py:261) and the
    whole-`None` fallback at :341 is the extension-missing path, so
    `checkexpr._rust_classify_check_arg` is a live callable on a kernel host.
    Nothing here wraps a scan in a gate toggle: `co_names` is a static
    code-object attribute, so the toggle could not change the answer (#1847).
    The `NATIVE_CHECK_ARG_*` tag constants stay in lockstep with `CHECK_ARG_*`
    in `crates/type_kernel/src/checkexpr_functions.rs`; the direct-seam tests
    in `NativeCheckArgSuite` exercise them.
    """

    def test_alias_kept_and_pyfunction_registered(self) -> None:
        # `getattr`, not `checkexpr._rust_classify_check_arg` and not a
        # `from mypy.checkexpr import _rust_classify_check_arg`: the host binds
        # it as `... as _rust_classify_check_arg`, not a re-export (strict).
        from mypy import checkexpr

        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_classify_check_arg")
        alias = getattr(checkexpr, "_rust_classify_check_arg", None)
        assert callable(alias), f"alias not callable: {alias!r}"

    def test_no_rust_name_loaded_by_check_arg(self) -> None:
        from mypy.checkexpr import ExpressionChecker

        loaded = [n for n in ExpressionChecker.check_arg.__code__.co_names if "rust_" in n]
        assert loaded == [], f"check_arg still loads {loaded}"

    def test_the_co_names_scan_bites(self) -> None:
        # Negative control: the scan above must flag a code object that loads
        # a live rust_* alias, or a green `loaded == []` proves nothing. This
        # probe is a wired neighbour; retarget it if that seam ever retires.
        from mypy import checkexpr
        from mypy.checkexpr import ExpressionChecker

        found = [
            n for n in ExpressionChecker.check_argument_types.__code__.co_names if "rust_" in n
        ]
        live = [n for n in found if callable(getattr(checkexpr, n, None))]
        assert live, f"no live rust_* alias in check_argument_types: {found}"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckOverloadCallRetiredSuite(Suite):
    """Pin the #1624 retirement of the `rust_check_overload_call` shim.

    The seam pre-picked the first-match overload item across the FFI
    (generic items via the constraint-solve kernel, plus a Python-side
    per-target type-object gate-fact list) and only trusted the pick when
    re-running `check_call` added no new errors. A load-robust
    instruction-count A/B on the cold self-check (`/usr/bin/time -l`
    instructions retired, single-process, 3+2 interleaved runs) measured
    the crossing a net loss: wrapping the binding in a no-op left the
    pure-Python resolution plus the Python-side gate and wire prep, and
    still saved 2.3e9 of the 487.6e9 baseline instructions (-0.47%). The
    unconditional Python trial resolution below the gate was already the
    authoritative body, so the seam is now uncalled; the Rust pyfunction
    stays registered for the direct-seam tests in `NativeOverloadCallSuite`.
    """

    def test_shim_helpers_and_source_removed(self) -> None:
        import inspect

        from mypy import checkexpr

        assert not hasattr(checkexpr, "_rust_check_overload_call")
        assert not hasattr(checkexpr, "_typeobj_gate_flag_for_roc")
        src = inspect.getsource(checkexpr.ExpressionChecker.check_overload_call)
        for dead in ("rust_", "native_idx"):
            assert dead not in src, f"check_overload_call still carries {dead}"

    def test_no_rust_name_loaded(self) -> None:
        from mypy.checkexpr import ExpressionChecker

        loaded = [
            n for n in ExpressionChecker.check_overload_call.__code__.co_names if "rust_" in n
        ]
        assert loaded == [], f"check_overload_call still loads {loaded}"

    def test_pyfunction_stays_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_check_overload_call")
