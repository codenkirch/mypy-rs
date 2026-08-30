"""Test cases for type inference helper functions."""

from __future__ import annotations

import os
from unittest import skipUnless

from librt.internal import ReadBuffer, WriteBuffer

from mypy.argmap import _set_native_argmap_active, map_actuals_to_formals
from mypy.cache import CacheMeta, CacheMetaEx
from mypy.checker import DisjointDict, group_comparison_operands

# Stage 4 dispatch kinds, mirrored from checkcall.rs via checkexpr.
from mypy.checkexpr import (
    CALL_ANY,
    CALL_INSTANCE,
    CALL_OTHER,
    CALL_OVERLOADED,
    CALL_PLAIN,
    CALL_TYPE_TYPE,
    CALL_UNION,
    CALL_WITH_VARS,
    _try_native_classify_call,
    _try_native_normalize_callable,
)
from mypy.constraints import SUBTYPE_OF, SUPERTYPE_OF
from mypy.literals import Key
from mypy.nodes import ARG_NAMED, ARG_OPT, ARG_POS, ARG_STAR, ARG_STAR2, ArgKind, NameExpr
from mypy.solve import solve_one
from mypy.test.helpers import Suite, assert_equal
from mypy.test.typefixture import TypeFixture
from mypy.types import (
    AnyType,
    CallableType,
    Overloaded,
    TupleType,
    Type,
    TypedDictType,
    TypeOfAny,
    TypeVarId,
    TypeVarType,
    UnionType,
    UnpackType,
    get_proper_type,
)

# Stage 4 parity: flip the argmap gate from the env var so the unit tests
# exercise the Rust path when TEST_NATIVE_TYPE_KERNEL is set. Mirrors the
# testtypes.py gate for the Stage 3a/3b/3c parity suites.
_set_native_argmap_active(bool(os.environ.get("TEST_NATIVE_TYPE_KERNEL")))

_NATIVE_ARGMAP_ENABLED = bool(os.environ.get("TEST_NATIVE_TYPE_KERNEL"))


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class SolveOneParitySuite(Suite):
    """Parity: solve_one native vs pure-Python.

    Asserts the Rust solve path returns the same candidate as the
    pure-Python solve_one for the join/meet/selection corner cases.
    Only runs when the kernel is importable (env-gated like the other
    native suites in this file).
    """

    def test_join_lowers_single(self) -> None:
        fixture = TypeFixture()
        self.assert_equal_solve([fixture.a], [], fixture)

    def test_join_lowers_two_classes(self) -> None:
        # join(B, C) with B, C sharing base A -> A
        fixture = TypeFixture()
        self.assert_equal_solve([fixture.b, fixture.c], [], fixture)

    def test_join_lowers_union(self) -> None:
        # join of a union and a class -> union preserves
        fixture = TypeFixture()
        self.assert_equal_solve([fixture.b, fixture.c], [], fixture)

    def test_upper_single(self) -> None:
        # no lowers, one upper -> the upper itself
        fixture = TypeFixture()
        self.assert_equal_solve([], [fixture.a], fixture)

    def test_lower_upper_subtype(self) -> None:
        # B <: A, so candidate = B
        fixture = TypeFixture()
        self.assert_equal_solve([fixture.b], [fixture.a], fixture)

    def test_lower_upper_not_subtype(self) -> None:
        # D not <: A -> None (no solution)
        fixture = TypeFixture()
        self.assert_equal_solve([fixture.d], [fixture.a], fixture)

    def test_no_bounds(self) -> None:
        fixture = TypeFixture()
        self.assert_equal_solve([], [], fixture)

    def assert_equal_solve(
        self, lowers: list[Type], uppers: list[Type], fixture: TypeFixture
    ) -> None:
        # Pure-Python path (gate off).
        from mypy.solve import (
            _native_solve_active,
            _native_solve_resolver,
            _set_native_solve_active,
            _set_native_solve_resolver,
        )

        saved_active = _native_solve_active
        saved_resolver = _native_solve_resolver
        try:
            _set_native_solve_active(False)
            expected = solve_one(lowers, uppers)
            # Native path (gate on, fixture resolver installed).
            import type_kernel

            from mypy.wirefixup import set_wire_typeinfo_map

            type_infos = [v for v in vars(fixture).values() if hasattr(v, "fullname")]
            native = type_kernel.build_native_resolver(type_infos, [])
            set_wire_typeinfo_map({info.fullname: info for info in type_infos})
            _set_native_solve_resolver(native)
            _set_native_solve_active(True)
            actual = solve_one(lowers, uppers)
            assert_equal(actual, expected)
        finally:
            _set_native_solve_active(saved_active)
            _set_native_solve_resolver(saved_resolver)
            from mypy.wirefixup import set_wire_typeinfo_map

            set_wire_typeinfo_map(None)


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class CacheMetaParitySuite(Suite):
    """Parity: rust_read_cache_meta native vs pure-Python decode.

    The Rust kernel's fixed-format decode of `CacheMeta.write` bytes must
    match Python's `CacheMeta.read` field-for-field, including the error
    tuples and JSON values in `CacheMetaEx`.
    """

    def test_cache_meta_roundtrip(self) -> None:
        self.assert_equal_decode(self.sample_meta())

    def test_cache_meta_json_value(self) -> None:
        meta = self.sample_meta()
        meta.options["list_value"] = [1, "two", None]
        meta.options["tuple_value"] = (1, "two", None)
        self.assert_equal_decode(meta)

    def test_cache_meta_ex_errors(self) -> None:
        meta = CacheMetaEx(
            dependencies=["mod.a", "mod.b"],
            suppressed=["mod.c"],
            dep_hashes=[b"\x01\x02", b"\x03"],
            error_lines=[
                (None, 1, 2, 3, 4, "error", "msg", "code"),
                ("file.py", 10, 20, 30, 40, "note", "other", None),
            ],
        )
        self.assert_equal_decode_ex(meta)

    def sample_meta(self) -> CacheMeta:
        return CacheMeta(
            id="mod",
            path="/src/mod.py",
            mtime=1000,
            size=100,
            hash="abc123",
            dependencies=["dep.a", "dep.b"],
            data_mtime=2000,
            data_file="mod.data.ff",
            suppressed=["s1"],
            imports_ignored={1: ["unused-ignore"], 2: []},
            options={"platform": "darwin", "n": 42, "b": True, "f": 1.5},
            suppressed_deps_opts=b"\x00\x01",
            dep_prios=[1, 2],
            dep_lines=[3, 4],
            dep_hashes=[b"\xaa", b"\xbb"],
            interface_hash=b"\xcc",
            trans_dep_hash=b"\xdd",
            version_id="1.0",
            ignore_all=False,
            plugin_data={"k": "v"},
        )

    def assert_equal_decode(self, meta: CacheMeta) -> None:
        buffer = WriteBuffer()
        meta.write(buffer)
        blob = buffer.getvalue()
        import type_kernel

        native = type_kernel.rust_read_cache_meta(blob)
        assert native is not None
        expected = CacheMeta.read(ReadBuffer(blob), meta.data_file)
        assert expected is not None
        expected_dict = {
            "id": expected.id,
            "path": expected.path,
            "mtime": expected.mtime,
            "size": expected.size,
            "hash": expected.hash,
            "dependencies": expected.dependencies,
            "data_mtime": expected.data_mtime,
            "suppressed": expected.suppressed,
            "imports_ignored": expected.imports_ignored,
            "options": expected.options,
            "suppressed_deps_opts": expected.suppressed_deps_opts,
            "dep_prios": expected.dep_prios,
            "dep_lines": expected.dep_lines,
            "dep_hashes": expected.dep_hashes,
            "interface_hash": expected.interface_hash,
            "trans_dep_hash": expected.trans_dep_hash,
            "version_id": expected.version_id,
            "ignore_all": expected.ignore_all,
            "plugin_data": expected.plugin_data,
        }
        assert_equal(native, expected_dict)

    def assert_equal_decode_ex(self, meta: CacheMetaEx) -> None:
        buffer = WriteBuffer()
        meta.write(buffer)
        blob = buffer.getvalue()
        import type_kernel

        native = type_kernel.rust_read_cache_meta_ex(blob)
        assert native is not None
        expected = CacheMetaEx.read(ReadBuffer(blob))
        assert expected is not None
        expected_dict = {
            "dependencies": expected.dependencies,
            "suppressed": expected.suppressed,
            "dep_hashes": expected.dep_hashes,
            "error_lines": expected.error_lines,
        }
        assert_equal(native, expected_dict)


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class ConstraintInferParitySuite(Suite):
    """Parity: top-level TypeVarType infer_constraints native vs Python.

    Asserts the Rust constraint-inference path returns the same
    Constraint set (origin TypeVarType identity, op, target) as the
    pure-Python branch for the top-level TypeVarType case.
    """

    def test_simple_subtype(self) -> None:
        fixture = TypeFixture()
        self.assert_equal_infer(fixture.t, fixture.a)

    def test_supertype(self) -> None:
        fixture = TypeFixture()
        self.assert_equal_infer(fixture.t, fixture.a, SUPERTYPE_OF)

    def test_meta_var(self) -> None:
        # Template with a meta type variable (meta_level=1) round-trips
        # the id.meta_level through the wire.
        fixture = TypeFixture()
        fixture.t.id = TypeVarId.new(meta_level=1)
        self.assert_equal_infer(fixture.t, fixture.a)

    def assert_equal_infer(
        self, template: TypeVarType, actual: Type, direction: int = SUBTYPE_OF
    ) -> None:
        from mypy.constraints import (
            _native_constraints_active,
            _set_native_constraints_active,
            infer_constraints,
        )

        saved_active = _native_constraints_active
        try:
            _set_native_constraints_active(False)
            expected = infer_constraints(template, actual, direction)
            _set_native_constraints_active(True)
            native_result = infer_constraints(template, actual, direction)
            assert_equal(native_result, expected)
        finally:
            _set_native_constraints_active(saved_active)


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class ClassifyCallParitySuite(Suite):
    """Parity: check_call dispatch classification native vs Python.

    Asserts the Rust classifier returns the same CALL_* kind as the
    Python isinstance chain in check_call would choose, for each callee
    shape. This is the verification surface for the Stage 4 dispatch
    classifier; the check_call call sites remain pure Python until a
    later slice gates a branch behind it.
    """

    def test_plain_callable(self) -> None:
        fixture = TypeFixture()
        self.assert_classify(fixture.callable(fixture.a), CALL_PLAIN)

    def test_callable_with_vars(self) -> None:
        fixture = TypeFixture()
        callable_t = fixture.callable(fixture.a)
        callable_t.variables = (fixture.t,)
        self.assert_classify(callable_t, CALL_WITH_VARS)

    def test_overloaded(self) -> None:
        fixture = TypeFixture()
        self.assert_classify(
            Overloaded([fixture.callable(fixture.a), fixture.callable(fixture.b)]), CALL_OVERLOADED
        )

    def test_any(self) -> None:
        fixture = TypeFixture()
        self.assert_classify(fixture.anyt, CALL_ANY)

    def test_union(self) -> None:
        fixture = TypeFixture()
        self.assert_classify(UnionType.make_union([fixture.a, fixture.b]), CALL_UNION)

    def test_instance(self) -> None:
        fixture = TypeFixture()
        self.assert_classify(fixture.a, CALL_INSTANCE)

    def test_type_type(self) -> None:
        fixture = TypeFixture()
        self.assert_classify(fixture.type_any, CALL_TYPE_TYPE)

    def test_other_shapes(self) -> None:
        # TypeVarType, TupleType, UninhabitedType fall through to
        # CALL_OTHER in the classifier (Python recurses or errors).
        fixture = TypeFixture()
        tuple_t = TupleType([fixture.a], fixture.std_tuple)
        self.assert_classify(fixture.t, CALL_OTHER)
        self.assert_classify(tuple_t, CALL_OTHER)
        self.assert_classify(fixture.uninhabited, CALL_OTHER)

    def test_typevar_as_callable(self) -> None:
        # TypeVarType with an upper bound that is a callable still falls
        # through to CALL_OTHER (the isinstance chain recurses).
        fixture = TypeFixture()
        self.assert_classify(fixture.t, CALL_OTHER)

    def assert_classify(self, callee: Type, expected: int) -> None:
        from mypy.checkexpr import _native_checkexpr_active, _set_native_checkexpr_active

        saved_active = _native_checkexpr_active
        try:
            _set_native_checkexpr_active(True)
            kind = _try_native_classify_call(get_proper_type(callee))
            assert_equal(kind, expected)
        finally:
            _set_native_checkexpr_active(saved_active)


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class CheckCallDispatchParitySuite(Suite):
    """Parity: check_call dispatch guard native vs Python.

    Asserts the structural `_dispatch_kind` used by the dispatch guard
    agrees with the Rust classifier for the shapes the isinstance chain
    routes. The guard in check_call raises on disagreement; these tests
    prove the guard is silent on the canonical shapes before the corpus
    runs.
    """

    def assert_dispatch_agrees(self, callee: Type) -> None:
        from mypy.checkexpr import (
            ExpressionChecker,
            _native_checkexpr_active,
            _set_native_checkexpr_active,
            _try_native_classify_call,
        )

        saved_active = _native_checkexpr_active
        try:
            _set_native_checkexpr_active(True)
            native_kind = _try_native_classify_call(get_proper_type(callee))
            py_kind = ExpressionChecker._dispatch_kind(None, callee)  # type: ignore[arg-type]
            if native_kind is not None and native_kind != CALL_OTHER:
                assert_equal(native_kind, py_kind)
        finally:
            _set_native_checkexpr_active(saved_active)

    def test_plain_callable(self) -> None:
        fixture = TypeFixture()
        self.assert_dispatch_agrees(fixture.callable(fixture.a, fixture.b))

    def test_generic_callable(self) -> None:
        fixture = TypeFixture()
        tvar = TypeVarType(
            "T", "T", TypeVarId(-1), [], fixture.o, AnyType(TypeOfAny.from_omitted_generics)
        )
        callee = CallableType(
            [tvar], [ARG_POS], [None], fixture.b, fixture.function, variables=[tvar]
        )
        self.assert_dispatch_agrees(callee)

    def test_overloaded(self) -> None:
        fixture = TypeFixture()
        c1 = fixture.callable(fixture.a, fixture.b)
        c2 = fixture.callable(fixture.b, fixture.a)
        self.assert_dispatch_agrees(Overloaded([c1, c2]))

    def test_any(self) -> None:
        fixture = TypeFixture()
        self.assert_dispatch_agrees(fixture.anyt)

    def test_union(self) -> None:
        fixture = TypeFixture()
        self.assert_dispatch_agrees(UnionType([fixture.a, fixture.b]))

    def test_instance(self) -> None:
        fixture = TypeFixture()
        self.assert_dispatch_agrees(fixture.a)

    def test_type_type(self) -> None:
        fixture = TypeFixture()
        self.assert_dispatch_agrees(fixture.type_any)

    def test_other_shapes_defer(self) -> None:
        # TypeVar/TupleType/UninhabitedType classify CALL_OTHER in Rust;
        # the guard skips verification and the chain recurses.
        fixture = TypeFixture()
        self.assert_dispatch_agrees(fixture.t)
        self.assert_dispatch_agrees(TupleType([fixture.a], fixture.std_tuple))
        self.assert_dispatch_agrees(fixture.uninhabited)


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class NormalizeCallableParitySuite(Suite):
    """Parity: check_callable_call callee normalization native vs Python.

    Asserts the Rust-normalized callee equals the Python
    `with_unpacked_kwargs().with_normalized_var_args()` result for the
    shapes the head of check_callable_call normalizes: unpacked
    **kwargs (TypedDict tail), plain *args, non-tuple *args.
    """

    def test_plain_callable_unchanged(self) -> None:
        fixture = TypeFixture()
        callee = fixture.callable(fixture.a, fixture.b)
        self.assert_normalize(callee)

    def test_unpacked_kwargs_typeddict(self) -> None:
        fixture = TypeFixture()
        td = TypedDictType(
            {"x": fixture.a, "y": fixture.b}, {"x"}, set(), fixture.a, is_closed=True
        )
        callee = CallableType(
            [fixture.a, td],
            [ARG_POS, ARG_STAR2],
            [None, "kwargs"],
            fixture.b,
            fixture.function,
            unpack_kwargs=True,
        )
        self.assert_normalize(callee)
        assert callee.unpack_kwargs

    def test_var_args_plain_tuple(self) -> None:
        fixture = TypeFixture()
        callee = CallableType(
            [UnpackType(fixture.std_tuple)], [ARG_STAR], [None], fixture.b, fixture.function
        )
        self.assert_normalize(callee)

    def test_var_args_non_tuple_unpack_unchanged(self) -> None:
        fixture = TypeFixture()
        # *args: *tuple[X, ...] is a nested UnpackType of Instance, which
        # with_normalized_var_args leaves unchanged.
        nested = UnpackType(fixture.lsta)
        callee = CallableType(
            [UnpackType(nested)], [ARG_STAR], [None], fixture.b, fixture.function
        )
        self.assert_normalize(callee)

    def assert_normalize(self, callee: CallableType) -> None:
        from mypy.checkexpr import _native_checkexpr_active, _set_native_checkexpr_active

        saved_active = _native_checkexpr_active
        try:
            _set_native_checkexpr_active(True)
            agreed = _try_native_normalize_callable(callee)
            assert_equal(agreed, True)
        finally:
            _set_native_checkexpr_active(saved_active)


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class MapFormalsToActualsParitySuite(Suite):
    """Parity: map_formals_to_actuals reverse mapping native vs Python.

    Runs the mapping with the argmap gate off (pure Python) and on (Rust
    kernel) and asserts identical results for the non-star shapes the Rust
    path handles. Star actuals defer to Python in both regimes.
    """

    def assert_reverse_parity(
        self,
        actual_kinds: list[ArgKind],
        actual_names: list[str | None] | None,
        formal_kinds: list[ArgKind],
        formal_names: list[str | None],
    ) -> None:
        from mypy.argmap import (
            _native_argmap_active,
            _set_native_argmap_active,
            map_formals_to_actuals,
        )

        def run() -> list[list[int]]:
            return map_formals_to_actuals(
                actual_kinds,
                actual_names,
                formal_kinds,
                formal_names,
                lambda i: fixture.anyt,  # only used for star actuals (deferred)
            )

        fixture = TypeFixture()
        saved_active = _native_argmap_active
        try:
            _set_native_argmap_active(False)
            expected = run()
            _set_native_argmap_active(True)
            actual = run()
        finally:
            _set_native_argmap_active(saved_active)
        assert_equal(actual, expected)

    def test_pos_to_pos_reverse(self) -> None:
        self.assert_reverse_parity([ARG_POS], [None], [ARG_POS], ["x"])

    def test_pos_to_star_formal_reverse(self) -> None:
        self.assert_reverse_parity([ARG_POS, ARG_POS], [None, None], [ARG_STAR], [None])

    def test_pos_overflow_reverse(self) -> None:
        self.assert_reverse_parity([ARG_POS, ARG_POS], [None, None], [ARG_POS], ["x"])

    def test_pos_into_star2_reverse(self) -> None:
        self.assert_reverse_parity([ARG_POS], [None], [ARG_STAR2], [None])

    def test_named_to_named_reverse(self) -> None:
        self.assert_reverse_parity([ARG_NAMED], ["x"], [ARG_POS], ["x"])

    def test_named_to_star2_reverse(self) -> None:
        self.assert_reverse_parity([ARG_NAMED], ["z"], [ARG_POS, ARG_STAR2], ["x", None])

    def test_named_not_found_reverse(self) -> None:
        self.assert_reverse_parity([ARG_NAMED], ["z"], [ARG_POS], ["x"])

    def test_multiple_named_reverse(self) -> None:
        self.assert_reverse_parity(
            [ARG_NAMED, ARG_NAMED], ["x", "y"], [ARG_POS, ARG_POS], ["x", "y"]
        )

    def test_pos_then_named_reverse(self) -> None:
        self.assert_reverse_parity(
            [ARG_POS, ARG_NAMED], [None, "y"], [ARG_POS, ARG_POS], ["x", "y"]
        )

    def test_empty_caller_reverse(self) -> None:
        self.assert_reverse_parity([], [], [ARG_POS], ["x"])

    def test_empty_callee_reverse(self) -> None:
        self.assert_reverse_parity([ARG_POS, ARG_NAMED], [None, "y"], [], [])


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class MapActualsToFormalsStarParitySuite(Suite):
    """Parity: map_actuals_to_formals with star actuals native vs Python.

    Runs the mapping with the argmap gate off (pure Python, callback used)
    and on (Rust kernel, wire-serialized actual types) and asserts identical
    results for tuple *args, iterable *args, TypedDict **kwargs, and
    ambiguous non-TypedDict **kwargs.
    """

    def assert_star_parity(
        self,
        actual_kinds: list[ArgKind],
        actual_names: list[str | None] | None,
        formal_kinds: list[ArgKind],
        formal_names: list[str | None],
        actual_types: list[Type],
    ) -> None:
        from mypy.argmap import (
            _native_argmap_active,
            _set_native_argmap_active,
            map_actuals_to_formals,
        )

        def run() -> list[list[int]]:
            return map_actuals_to_formals(
                actual_kinds, actual_names, formal_kinds, formal_names, lambda i: actual_types[i]
            )

        saved_active = _native_argmap_active
        try:
            _set_native_argmap_active(False)
            expected = run()
            _set_native_argmap_active(True)
            actual = run()
        finally:
            _set_native_argmap_active(saved_active)
        assert_equal(actual, expected)

    def test_star_tuple_fixed_formals(self) -> None:
        fixture = TypeFixture()
        tup = TupleType([fixture.a, fixture.b], fixture.std_tuple, line=1, column=1)
        self.assert_star_parity(
            [ARG_STAR], [None], [ARG_POS, ARG_POS, ARG_POS], [None, None, None], [tup]
        )

    def test_star_iterable_while(self) -> None:
        fixture = TypeFixture()
        lst = fixture.lsta  # list[A]
        self.assert_star_parity([ARG_STAR], [None], [ARG_POS, ARG_STAR], ["x", None], [lst])

    def test_star2_typeddict_routes(self) -> None:
        fixture = TypeFixture()
        td = TypedDictType(
            {"x": fixture.a, "y": fixture.b}, {"x"}, set(), fixture.a, is_closed=True
        )
        self.assert_star_parity([ARG_STAR2], [None], [ARG_POS, ARG_STAR2], ["x", None], [td])

    def test_star2_non_typeddict_ambiguous(self) -> None:
        fixture = TypeFixture()
        lst = fixture.lsta
        self.assert_star_parity([ARG_STAR2], [None], [ARG_POS, ARG_POS], ["x", "y"], [lst])


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class ExpandActualTypeParitySuite(Suite):
    """Parity: ArgTypeExpander.expand_actual_type native vs Python.

    Exercises the deterministic structural branches (tuple *args item
    indexing with wrap-around, named-key TypedDict **kwargs lookup) that the
    Rust kernel resolves via wire-serialized actual types. The graph-
    dependent and hash-order-dependent branches run Python-side in both
    regimes.
    """

    def assert_expand_parity(
        self, calls: list[tuple[Type, ArgKind, str | None, ArgKind, bool]]
    ) -> None:
        from mypy.argmap import ArgTypeExpander, _native_argmap_active
        from mypy.infer import ArgumentInferContext

        fixture = TypeFixture()
        # Structural branches never touch the context; provide a valid one so
        # Iterable/Mapping branches (if reached) behave identically in both
        # regimes.
        context = ArgumentInferContext(fixture.std_tuple, fixture.std_tuple)

        def run() -> tuple[list[Type], int, set[str] | None]:
            mapper = ArgTypeExpander(context)
            out = []
            for actual_type, actual_kind, formal_name, formal_kind, allow_unpack in calls:
                out.append(
                    mapper.expand_actual_type(
                        actual_type, actual_kind, formal_name, formal_kind, allow_unpack
                    )
                )
            return out, mapper.tuple_index, mapper.kwargs_used

        saved_active = _native_argmap_active
        try:
            _set_native_argmap_active(False)
            expected = run()
            _set_native_argmap_active(True)
            actual = run()
        finally:
            _set_native_argmap_active(saved_active)
        assert_equal(actual, expected)

    def test_tuple_star_first_item(self) -> None:
        fixture = TypeFixture()
        tup = TupleType([fixture.a, fixture.b], fixture.std_tuple, line=1, column=1)
        self.assert_expand_parity([(tup, ARG_STAR, None, ARG_POS, False)])

    def test_tuple_star_sequence(self) -> None:
        fixture = TypeFixture()
        tup = TupleType([fixture.a, fixture.b], fixture.std_tuple, line=1, column=1)
        self.assert_expand_parity(
            [(tup, ARG_STAR, None, ARG_POS, False), (tup, ARG_STAR, None, ARG_POS, False)]
        )

    def test_tuple_star_wrap_after_exhaustion(self) -> None:
        fixture = TypeFixture()
        tup = TupleType([fixture.a, fixture.b], fixture.std_tuple, line=1, column=1)
        self.assert_expand_parity(
            [
                (tup, ARG_STAR, None, ARG_POS, False),
                (tup, ARG_STAR, None, ARG_POS, False),
                (tup, ARG_STAR, None, ARG_POS, False),
            ]
        )

    def test_tuple_star_unpack_allow(self) -> None:
        fixture = TypeFixture()
        item = UnpackType(fixture.a)
        tup = TupleType([item, fixture.b], fixture.std_tuple, line=1, column=1)
        self.assert_expand_parity([(tup, ARG_STAR, None, ARG_POS, True)])

    def test_star2_typeddict_named_key(self) -> None:
        fixture = TypeFixture()
        td = TypedDictType(
            {"x": fixture.a, "y": fixture.b}, {"x"}, set(), fixture.a, is_closed=True
        )
        self.assert_expand_parity([(td, ARG_STAR2, "x", ARG_POS, False)])

    def test_star2_typeddict_two_keys(self) -> None:
        fixture = TypeFixture()
        td = TypedDictType(
            {"x": fixture.a, "y": fixture.b}, {"x"}, set(), fixture.a, is_closed=True
        )
        self.assert_expand_parity(
            [(td, ARG_STAR2, "x", ARG_POS, False), (td, ARG_STAR2, "y", ARG_POS, False)]
        )

    def test_star2_typeddict_star2_formal(self) -> None:
        # formal_kind == ARG_STAR2: Python's arbitrary pop, both regimes
        # run Python (Rust defers), so parity must hold.
        fixture = TypeFixture()
        td = TypedDictType(
            {"x": fixture.a, "y": fixture.b}, {"x"}, set(), fixture.a, is_closed=True
        )
        self.assert_expand_parity([(td, ARG_STAR2, None, ARG_STAR2, False)])

    def test_star2_typeddict_unmatched_name(self) -> None:
        # Name not among keys: arbitrary pop path, Python runs both.
        fixture = TypeFixture()
        td = TypedDictType(
            {"x": fixture.a, "y": fixture.b}, {"x"}, set(), fixture.a, is_closed=True
        )
        self.assert_expand_parity([(td, ARG_STAR2, "z", ARG_POS, False)])


class MapActualsToFormalsSuite(Suite):
    """Test cases for argmap.map_actuals_to_formals."""

    def test_basic(self) -> None:
        self.assert_map([], [], [])

    def test_positional_only(self) -> None:
        self.assert_map([ARG_POS], [ARG_POS], [[0]])
        self.assert_map([ARG_POS, ARG_POS], [ARG_POS, ARG_POS], [[0], [1]])

    def test_optional(self) -> None:
        self.assert_map([], [ARG_OPT], [[]])
        self.assert_map([ARG_POS], [ARG_OPT], [[0]])
        self.assert_map([ARG_POS], [ARG_OPT, ARG_OPT], [[0], []])

    def test_callee_star(self) -> None:
        self.assert_map([], [ARG_STAR], [[]])
        self.assert_map([ARG_POS], [ARG_STAR], [[0]])
        self.assert_map([ARG_POS, ARG_POS], [ARG_STAR], [[0, 1]])

    def test_caller_star(self) -> None:
        self.assert_map([ARG_STAR], [ARG_STAR], [[0]])
        self.assert_map([ARG_POS, ARG_STAR], [ARG_STAR], [[0, 1]])
        self.assert_map([ARG_STAR], [ARG_POS, ARG_STAR], [[0], [0]])
        self.assert_map([ARG_STAR], [ARG_OPT, ARG_STAR], [[0], [0]])

    def test_too_many_caller_args(self) -> None:
        self.assert_map([ARG_POS], [], [])
        self.assert_map([ARG_STAR], [], [])
        self.assert_map([ARG_STAR], [ARG_POS], [[0]])

    def test_tuple_star(self) -> None:
        any_type = AnyType(TypeOfAny.special_form)
        self.assert_vararg_map([ARG_STAR], [ARG_POS], [[0]], self.make_tuple(any_type))
        self.assert_vararg_map(
            [ARG_STAR], [ARG_POS, ARG_POS], [[0], [0]], self.make_tuple(any_type, any_type)
        )
        self.assert_vararg_map(
            [ARG_STAR],
            [ARG_POS, ARG_OPT, ARG_OPT],
            [[0], [0], []],
            self.make_tuple(any_type, any_type),
        )

    def make_tuple(self, *args: Type) -> TupleType:
        return TupleType(list(args), TypeFixture().std_tuple)

    def test_named_args(self) -> None:
        self.assert_map(["x"], [(ARG_POS, "x")], [[0]])
        self.assert_map(["y", "x"], [(ARG_POS, "x"), (ARG_POS, "y")], [[1], [0]])

    def test_some_named_args(self) -> None:
        self.assert_map(["y"], [(ARG_OPT, "x"), (ARG_OPT, "y"), (ARG_OPT, "z")], [[], [0], []])

    def test_missing_named_arg(self) -> None:
        self.assert_map(["y"], [(ARG_OPT, "x")], [[]])

    def test_duplicate_named_arg(self) -> None:
        self.assert_map(["x", "x"], [(ARG_OPT, "x")], [[0, 1]])

    def test_varargs_and_bare_asterisk(self) -> None:
        self.assert_map([ARG_STAR], [ARG_STAR, (ARG_NAMED, "x")], [[0], []])
        self.assert_map([ARG_STAR, "x"], [ARG_STAR, (ARG_NAMED, "x")], [[0], [1]])

    def test_keyword_varargs(self) -> None:
        self.assert_map(["x"], [ARG_STAR2], [[0]])
        self.assert_map(["x", ARG_STAR2], [ARG_STAR2], [[0, 1]])
        self.assert_map(["x", ARG_STAR2], [(ARG_POS, "x"), ARG_STAR2], [[0], [1]])
        self.assert_map([ARG_POS, ARG_STAR2], [(ARG_POS, "x"), ARG_STAR2], [[0], [1]])

    def test_both_kinds_of_varargs(self) -> None:
        self.assert_map([ARG_STAR, ARG_STAR2], [(ARG_POS, "x"), (ARG_POS, "y")], [[0, 1], [0, 1]])

    def test_special_cases(self) -> None:
        self.assert_map([ARG_STAR], [ARG_STAR, ARG_STAR2], [[0], []])
        self.assert_map([ARG_STAR, ARG_STAR2], [ARG_STAR, ARG_STAR2], [[0], [1]])
        self.assert_map([ARG_STAR2], [(ARG_POS, "x"), ARG_STAR2], [[0], [0]])
        self.assert_map([ARG_STAR2], [ARG_STAR2], [[0]])

    def assert_map(
        self,
        caller_kinds_: list[ArgKind | str],
        callee_kinds_: list[ArgKind | tuple[ArgKind, str]],
        expected: list[list[int]],
    ) -> None:
        caller_kinds, caller_names = expand_caller_kinds(caller_kinds_)
        callee_kinds, callee_names = expand_callee_kinds(callee_kinds_)
        result = map_actuals_to_formals(
            caller_kinds,
            caller_names,
            callee_kinds,
            callee_names,
            lambda i: AnyType(TypeOfAny.special_form),
        )
        assert_equal(result, expected)

    def assert_vararg_map(
        self,
        caller_kinds: list[ArgKind],
        callee_kinds: list[ArgKind],
        expected: list[list[int]],
        vararg_type: Type,
    ) -> None:
        result = map_actuals_to_formals(caller_kinds, [], callee_kinds, [], lambda i: vararg_type)
        assert_equal(result, expected)


@skipUnless(_NATIVE_ARGMAP_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeArgMapSuite(Suite):
    """Parity tests for `argmap::rust_map_actuals_to_formals` (Stage 4).

    Each test runs the same non-star-actual cases as `MapActualsToFormalsSuite`
    through the public `map_actuals_to_formals` entry point with the Rust gate
    active, asserting identical results. Star-actual cases are covered by the
    `return None -> fall through to Python` contract (the Rust path declines,
    Python handles them; parity holds trivially).
    """

    def test_basic_and_positional(self) -> None:
        self.assert_map([], [], [])
        self.assert_map([ARG_POS], [ARG_POS], [[0]])
        self.assert_map([ARG_POS, ARG_POS], [ARG_POS, ARG_POS], [[0], [1]])

    def test_optional_formals(self) -> None:
        self.assert_map([], [ARG_OPT], [[]])
        self.assert_map([ARG_POS], [ARG_OPT], [[0]])
        self.assert_map([ARG_POS], [ARG_OPT, ARG_OPT], [[0], []])

    def test_callee_star_formal(self) -> None:
        self.assert_map([], [ARG_STAR], [[]])
        self.assert_map([ARG_POS], [ARG_STAR], [[0]])
        self.assert_map([ARG_POS, ARG_POS], [ARG_STAR], [[0, 1]])

    def test_too_many_positional(self) -> None:
        self.assert_map([ARG_POS], [], [])
        self.assert_map([ARG_POS, ARG_POS], [ARG_POS], [[0]])

    def test_named_args(self) -> None:
        self.assert_map(["x"], [(ARG_POS, "x")], [[0]])
        self.assert_map(["y", "x"], [(ARG_POS, "x"), (ARG_POS, "y")], [[1], [0]])

    def test_some_and_missing_named(self) -> None:
        self.assert_map(["y"], [(ARG_OPT, "x"), (ARG_OPT, "y"), (ARG_OPT, "z")], [[], [0], []])
        self.assert_map(["y"], [(ARG_OPT, "x")], [[]])

    def test_duplicate_named_arg(self) -> None:
        self.assert_map(["x", "x"], [(ARG_OPT, "x")], [[0, 1]])

    def test_named_into_star2(self) -> None:
        self.assert_map(["x"], [ARG_STAR2], [[0]])
        self.assert_map(["x", ARG_STAR2], [(ARG_POS, "x"), ARG_STAR2], [[0], [1]])
        # Named actual matching a positional formal, with an ARG_STAR2 slot
        # also present: the named actual binds by name (not to the varkwargs
        # slot), so slot 0 gets [0, 1] and the varkwargs slot is empty.
        self.assert_map([ARG_POS, "x"], [(ARG_POS, "x"), ARG_STAR2], [[0, 1], []])

    def test_named_routes_to_star2_when_formal_is_star(self) -> None:
        # Name matches an ARG_STAR formal: routes to ARG_STAR2 if present,
        # dropped otherwise (mirrors argmap.py:81-84).
        self.assert_map(["x"], [(ARG_STAR, "x"), ARG_STAR2], [[], [0]])
        self.assert_map(["x"], [(ARG_STAR, "x")], [[]])

    def test_pos_then_named_mixed(self) -> None:
        self.assert_map([ARG_POS, "y"], [(ARG_POS, "x"), (ARG_POS, "y")], [[0], [1]])

    def test_empty_caller(self) -> None:
        self.assert_map([], [(ARG_POS, "x")], [[]])

    def test_star_actuals_fall_through(self) -> None:
        # Star actuals must produce the same result the pure-Python path would;
        # the Rust path declines (returns None) and Python handles them.
        self.assert_map([ARG_STAR], [ARG_STAR], [[0]])
        self.assert_map([ARG_POS, ARG_STAR], [ARG_STAR], [[0, 1]])
        self.assert_map([ARG_STAR], [ARG_POS, ARG_STAR], [[0], [0]])
        self.assert_map([ARG_STAR], [ARG_OPT, ARG_STAR], [[0], [0]])
        self.assert_map([ARG_STAR], [ARG_STAR, (ARG_NAMED, "x")], [[0], []])
        self.assert_map([ARG_STAR, "x"], [ARG_STAR, (ARG_NAMED, "x")], [[0], [1]])
        self.assert_map(["x", ARG_STAR2], [ARG_STAR2], [[0, 1]])
        self.assert_map([ARG_POS, ARG_STAR2], [(ARG_POS, "x"), ARG_STAR2], [[0], [1]])
        self.assert_map([ARG_STAR2], [(ARG_POS, "x"), ARG_STAR2], [[0], [0]])
        self.assert_map([ARG_STAR2], [ARG_STAR2], [[0]])
        self.assert_map([ARG_STAR, ARG_STAR2], [(ARG_POS, "x"), (ARG_POS, "y")], [[0, 1], [0, 1]])
        self.assert_map([ARG_STAR], [ARG_STAR, ARG_STAR2], [[0], []])
        self.assert_map([ARG_STAR, ARG_STAR2], [ARG_STAR, ARG_STAR2], [[0], [1]])

    def assert_map(
        self,
        caller_kinds_: list[ArgKind | str],
        callee_kinds_: list[ArgKind | tuple[ArgKind, str]],
        expected: list[list[int]],
    ) -> None:
        caller_kinds, caller_names = expand_caller_kinds(caller_kinds_)
        callee_kinds, callee_names = expand_callee_kinds(callee_kinds_)
        result = map_actuals_to_formals(
            caller_kinds,
            caller_names,
            callee_kinds,
            callee_names,
            lambda i: AnyType(TypeOfAny.special_form),
        )
        assert_equal(result, expected)


def expand_caller_kinds(
    kinds_or_names: list[ArgKind | str],
) -> tuple[list[ArgKind], list[str | None]]:
    kinds = []
    names: list[str | None] = []
    for k in kinds_or_names:
        if isinstance(k, str):
            kinds.append(ARG_NAMED)
            names.append(k)
        else:
            kinds.append(k)
            names.append(None)
    return kinds, names


def expand_callee_kinds(
    kinds_and_names: list[ArgKind | tuple[ArgKind, str]],
) -> tuple[list[ArgKind], list[str | None]]:
    kinds = []
    names: list[str | None] = []
    for v in kinds_and_names:
        if isinstance(v, tuple):
            kinds.append(v[0])
            names.append(v[1])
        else:
            kinds.append(v)
            names.append(None)
    return kinds, names


class OperandDisjointDictSuite(Suite):
    """Test cases for checker.DisjointDict, which is used for type inference with operands."""

    def new(self) -> DisjointDict[int, str]:
        return DisjointDict()

    def test_independent_maps(self) -> None:
        d = self.new()
        d.add_mapping({0, 1}, {"group1"})
        d.add_mapping({2, 3, 4}, {"group2"})
        d.add_mapping({5, 6, 7}, {"group3"})

        self.assertEqual(
            d.items(), [({0, 1}, {"group1"}), ({2, 3, 4}, {"group2"}), ({5, 6, 7}, {"group3"})]
        )

    def test_partial_merging(self) -> None:
        d = self.new()
        d.add_mapping({0, 1}, {"group1"})
        d.add_mapping({1, 2}, {"group2"})
        d.add_mapping({3, 4}, {"group3"})
        d.add_mapping({5, 0}, {"group4"})
        d.add_mapping({5, 6}, {"group5"})
        d.add_mapping({4, 7}, {"group6"})

        self.assertEqual(
            d.items(),
            [
                ({0, 1, 2, 5, 6}, {"group1", "group2", "group4", "group5"}),
                ({3, 4, 7}, {"group3", "group6"}),
            ],
        )

    def test_full_merging(self) -> None:
        d = self.new()
        d.add_mapping({0, 1, 2}, {"a"})
        d.add_mapping({3, 4, 2}, {"b"})
        d.add_mapping({10, 11, 12}, {"c"})
        d.add_mapping({13, 14, 15}, {"d"})
        d.add_mapping({14, 10, 16}, {"e"})
        d.add_mapping({0, 10}, {"f"})

        self.assertEqual(
            d.items(),
            [({0, 1, 2, 3, 4, 10, 11, 12, 13, 14, 15, 16}, {"a", "b", "c", "d", "e", "f"})],
        )

    def test_merge_with_multiple_overlaps(self) -> None:
        d = self.new()
        d.add_mapping({0, 1, 2}, {"a"})
        d.add_mapping({3, 4, 5}, {"b"})
        d.add_mapping({1, 2, 4, 5}, {"c"})
        d.add_mapping({6, 1, 2, 4, 5}, {"d"})
        d.add_mapping({6, 1, 2, 4, 5}, {"e"})

        self.assertEqual(d.items(), [({0, 1, 2, 3, 4, 5, 6}, {"a", "b", "c", "d", "e"})])


class OperandComparisonGroupingSuite(Suite):
    """Test cases for checker.group_comparison_operands."""

    def literal_keymap(self, assignable_operands: dict[int, NameExpr]) -> dict[int, Key]:
        output: dict[int, Key] = {}
        for index, expr in assignable_operands.items():
            output[index] = ("FakeExpr", expr.name)
        return output

    def test_basic_cases(self) -> None:
        # Note: the grouping function doesn't actually inspect the input exprs, so we
        # just default to using NameExprs for simplicity.
        x0 = NameExpr("x0")
        x1 = NameExpr("x1")
        x2 = NameExpr("x2")
        x3 = NameExpr("x3")
        x4 = NameExpr("x4")

        basic_input = [("==", x0, x1), ("==", x1, x2), ("<", x2, x3), ("==", x3, x4)]

        none_assignable = self.literal_keymap({})
        all_assignable = self.literal_keymap({0: x0, 1: x1, 2: x2, 3: x3, 4: x4})

        for assignable in [none_assignable, all_assignable]:
            self.assertEqual(
                group_comparison_operands(basic_input, assignable, set()),
                [("==", [0, 1]), ("==", [1, 2]), ("<", [2, 3]), ("==", [3, 4])],
            )
            self.assertEqual(
                group_comparison_operands(basic_input, assignable, {"=="}),
                [("==", [0, 1, 2]), ("<", [2, 3]), ("==", [3, 4])],
            )
            self.assertEqual(
                group_comparison_operands(basic_input, assignable, {"<"}),
                [("==", [0, 1]), ("==", [1, 2]), ("<", [2, 3]), ("==", [3, 4])],
            )
            self.assertEqual(
                group_comparison_operands(basic_input, assignable, {"==", "<"}),
                [("==", [0, 1, 2]), ("<", [2, 3]), ("==", [3, 4])],
            )

    def test_multiple_groups(self) -> None:
        x0 = NameExpr("x0")
        x1 = NameExpr("x1")
        x2 = NameExpr("x2")
        x3 = NameExpr("x3")
        x4 = NameExpr("x4")
        x5 = NameExpr("x5")

        self.assertEqual(
            group_comparison_operands(
                [("==", x0, x1), ("==", x1, x2), ("is", x2, x3), ("is", x3, x4)],
                self.literal_keymap({}),
                {"==", "is"},
            ),
            [("==", [0, 1, 2]), ("is", [2, 3, 4])],
        )
        self.assertEqual(
            group_comparison_operands(
                [("==", x0, x1), ("==", x1, x2), ("==", x2, x3), ("==", x3, x4)],
                self.literal_keymap({}),
                {"==", "is"},
            ),
            [("==", [0, 1, 2, 3, 4])],
        )
        self.assertEqual(
            group_comparison_operands(
                [("is", x0, x1), ("==", x1, x2), ("==", x2, x3), ("==", x3, x4)],
                self.literal_keymap({}),
                {"==", "is"},
            ),
            [("is", [0, 1]), ("==", [1, 2, 3, 4])],
        )
        self.assertEqual(
            group_comparison_operands(
                [("is", x0, x1), ("is", x1, x2), ("<", x2, x3), ("==", x3, x4), ("==", x4, x5)],
                self.literal_keymap({}),
                {"==", "is"},
            ),
            [("is", [0, 1, 2]), ("<", [2, 3]), ("==", [3, 4, 5])],
        )

    def test_multiple_groups_coalescing(self) -> None:
        x0 = NameExpr("x0")
        x1 = NameExpr("x1")
        x2 = NameExpr("x2")
        x3 = NameExpr("x3")
        x4 = NameExpr("x4")

        nothing_combined = [("==", [0, 1, 2]), ("<", [2, 3]), ("==", [3, 4, 5])]
        everything_combined = [("==", [0, 1, 2, 3, 4, 5]), ("<", [2, 3])]

        # Note: We do 'x4 == x0' at the very end!
        two_groups = [
            ("==", x0, x1),
            ("==", x1, x2),
            ("<", x2, x3),
            ("==", x3, x4),
            ("==", x4, x0),
        ]
        self.assertEqual(
            group_comparison_operands(
                two_groups, self.literal_keymap({0: x0, 1: x1, 2: x2, 3: x3, 4: x4, 5: x0}), {"=="}
            ),
            everything_combined,
            "All vars are assignable, everything is combined",
        )
        self.assertEqual(
            group_comparison_operands(
                two_groups, self.literal_keymap({1: x1, 2: x2, 3: x3, 4: x4}), {"=="}
            ),
            nothing_combined,
            "x0 is unassignable, so no combining",
        )
        self.assertEqual(
            group_comparison_operands(
                two_groups, self.literal_keymap({0: x0, 1: x1, 3: x3, 5: x0}), {"=="}
            ),
            everything_combined,
            "Some vars are unassignable but x0 is, so we combine",
        )
        self.assertEqual(
            group_comparison_operands(two_groups, self.literal_keymap({0: x0, 5: x0}), {"=="}),
            everything_combined,
            "All vars are unassignable but x0 is, so we combine",
        )

    def test_multiple_groups_different_operators(self) -> None:
        x0 = NameExpr("x0")
        x1 = NameExpr("x1")
        x2 = NameExpr("x2")
        x3 = NameExpr("x3")

        groups = [("==", x0, x1), ("==", x1, x2), ("is", x2, x3), ("is", x3, x0)]
        keymap = self.literal_keymap({0: x0, 1: x1, 2: x2, 3: x3, 4: x0})
        self.assertEqual(
            group_comparison_operands(groups, keymap, {"==", "is"}),
            [("==", [0, 1, 2]), ("is", [2, 3, 4])],
            "Different operators can never be combined",
        )

    def test_single_pair(self) -> None:
        x0 = NameExpr("x0")
        x1 = NameExpr("x1")

        single_comparison = [("==", x0, x1)]
        expected_output = [("==", [0, 1])]

        assignable_combinations: list[dict[int, NameExpr]] = [{}, {0: x0}, {1: x1}, {0: x0, 1: x1}]
        to_group_by: list[set[str]] = [set(), {"=="}, {"is"}]

        for combo in assignable_combinations:
            for operators in to_group_by:
                keymap = self.literal_keymap(combo)
                self.assertEqual(
                    group_comparison_operands(single_comparison, keymap, operators),
                    expected_output,
                )

    def test_empty_pair_list(self) -> None:
        # This case should never occur in practice -- ComparisonExprs
        # always contain at least one comparison. But in case it does...

        self.assertEqual(group_comparison_operands([], {}, set()), [])
        self.assertEqual(group_comparison_operands([], {}, {"=="}), [])


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class InferFunctionTypeArgumentsParitySuite(Suite):
    """Parity: infer_function_type_arguments native vs Python.

    Asserts the whole inference loop (constraints + solve leaves) returns
    the same inferred argument list with the native gates on as with them
    off, for the generic-callee shapes the production path serves.
    """

    def _generic_callee(self, fixture: TypeFixture, nvars: int) -> CallableType:
        tvars = [
            TypeVarType(
                f"T{i}",
                f"T{i}",
                TypeVarId(-1 - i),
                [],
                fixture.o,
                AnyType(TypeOfAny.from_omitted_generics),
            )
            for i in range(nvars)
        ]
        return CallableType(
            [t for t in tvars],
            [ARG_POS] * nvars,
            [None] * nvars,
            tvars[-1],
            fixture.function,
            variables=tvars,
        )

    def setUp(self) -> None:
        # Install a native resolver over the fixture type graph so the
        # expand_type and solve_one leaves route through the Rust kernel.
        import type_kernel

        from mypy.wirefixup import set_wire_typeinfo_map

        fixture = TypeFixture()
        self.fixture = fixture
        type_infos = [v for v in vars(fixture).values() if hasattr(v, "fullname")]
        native = type_kernel.build_native_resolver(type_infos, [])
        set_wire_typeinfo_map({info.fullname: info for info in type_infos})
        from mypy.expandtype import _set_native_expand_type_resolver
        from mypy.solve import _set_native_solve_resolver

        _set_native_expand_type_resolver(native)
        _set_native_solve_resolver(native)

    def tearDown(self) -> None:
        from mypy.expandtype import _set_native_expand_type_resolver
        from mypy.solve import _set_native_solve_resolver
        from mypy.wirefixup import set_wire_typeinfo_map

        _set_native_expand_type_resolver(None)
        _set_native_solve_resolver(None)
        set_wire_typeinfo_map(None)

    def test_single_var_identity(self) -> None:
        fixture = TypeFixture()
        callee = self._generic_callee(fixture, 1)
        self.assert_equal_infer(fixture, callee, [fixture.a])

    def test_two_var_mapping(self) -> None:
        fixture = TypeFixture()
        callee = self._generic_callee(fixture, 2)
        self.assert_equal_infer(fixture, callee, [fixture.a, fixture.b])

    def test_join_two_classes(self) -> None:
        # T inferred from two lowers (B, C) sharing base A -> A.
        fixture = TypeFixture()
        callee = self._generic_callee(fixture, 1)
        self.assert_equal_infer(fixture, callee, [fixture.b, fixture.c], two_lowers=True)

    def test_freshen_expands_to_fresh_vars(self) -> None:
        # freshen_function_type_vars substitutes fresh unification vars for
        # the callee's generic variables; with the expand resolver
        # installed the substitution runs through the Rust expand_type leaf.
        fixture = TypeFixture()
        callee = self._generic_callee(fixture, 1)
        from mypy.expandtype import freshen_function_type_vars

        fresh = freshen_function_type_vars(callee)
        assert fresh.is_generic()
        assert fresh.variables[0].id != callee.variables[0].id
        assert_equal(fresh.arg_types[0], fresh.variables[0])

    def assert_equal_infer(
        self,
        fixture: TypeFixture,
        callee: CallableType,
        arg_types: list[Type],
        two_lowers: bool = False,
    ) -> None:
        from mypy.constraints import _native_constraints_active, _set_native_constraints_active
        from mypy.infer import ArgumentInferContext, infer_function_type_arguments
        from mypy.solve import _native_solve_active, _set_native_solve_active

        if two_lowers:
            formal_to_actual: list[list[int]] = [[0, 1]]
            arg_kinds = [ARG_POS, ARG_POS]
        else:
            formal_to_actual = [[i] for i in range(len(callee.arg_types))]
            arg_kinds = [ARG_POS] * len(callee.arg_types)
        context = ArgumentInferContext(fixture.std_tuple, fixture.std_tuple)

        def run() -> list[Type | None]:
            return infer_function_type_arguments(
                callee, arg_types, arg_kinds, None, formal_to_actual, context, strict=True
            )[0]

        # Resolver is installed in setUp; toggle the leaf gates here.
        saved_solve_active = _native_solve_active
        saved_constraints_active = _native_constraints_active
        try:
            _set_native_constraints_active(False)
            _set_native_solve_active(False)
            expected = run()
            _set_native_constraints_active(True)
            _set_native_solve_active(True)
            actual = run()
            # Compare semantically: the native path rebuilds type nodes
            # through the wire, so results are string-identical but not
            # necessarily the same Python objects as the fixture's.
            assert_equal([str(t) for t in actual], [str(t) for t in expected])
        finally:
            _set_native_constraints_active(saved_constraints_active)
            _set_native_solve_active(saved_solve_active)


@skipUnless(
    os.environ.get("TEST_NATIVE_TYPE_KERNEL"),
    "requires TEST_NATIVE_TYPE_KERNEL (Rust type-kernel build)",
)
class CheckExprScalarQueryParitySuite(Suite):
    """Parity: scalar checkexpr query helpers native vs Python.

    Toggles _native_checkexpr_active and asserts the Rust wire kernels
    agree with the pure-Python visitors for has_ambiguous_uninhabited_
    component and allow_fast_container_literal on fixture types. The
    ambiguous flag round-trips through the wire (UninhabitedType.write),
    so the ambiguous-narrowing branches are exercised on both sides.
    """

    def assert_agrees(self, t: Type | None) -> None:
        from mypy.checkexpr import (
            _native_checkexpr_active,
            _set_native_checkexpr_active,
            allow_fast_container_literal,
            has_ambiguous_uninhabited_component,
        )

        saved_active = _native_checkexpr_active
        try:
            _set_native_checkexpr_active(False)
            py_amb = has_ambiguous_uninhabited_component(t)
            py_fast = allow_fast_container_literal(t) if t is not None else None
            _set_native_checkexpr_active(True)
            rust_amb = has_ambiguous_uninhabited_component(t)
            rust_fast = allow_fast_container_literal(t) if t is not None else None
            assert_equal(rust_amb, py_amb, f"has_ambiguous mismatch for {t}")
            assert_equal(rust_fast, py_fast, f"allow_fast mismatch for {t}")
        finally:
            _set_native_checkexpr_active(saved_active)

    def test_uninhabited_plain(self) -> None:
        # TypeFixture().uninhabited is UninhabitedType() with
        # ambiguous left at the default False.
        self.assert_agrees(TypeFixture().uninhabited)

    def test_uninhabited_ambiguous(self) -> None:
        # TypeFixture().a_uninhabited has ambiguous=True, which the
        # ambiguous query must surface as a hit.
        self.assert_agrees(TypeFixture().a_uninhabited)

    def test_uninhabited_in_union(self) -> None:
        fixture = TypeFixture()
        self.assert_agrees(UnionType.make_union([fixture.a, fixture.a_uninhabited]))

    def test_bare_never_in_union(self) -> None:
        fixture = TypeFixture()
        self.assert_agrees(UnionType.make_union([fixture.a, fixture.uninhabited]))

    def test_instance_no_ambiguous(self) -> None:
        self.assert_agrees(TypeFixture().a)

    def test_tuple_of_instances(self) -> None:
        fixture = TypeFixture()
        self.assert_agrees(TupleType([fixture.a, fixture.b], fixture.std_tuple))

    def test_allow_fast_empty_tuple(self) -> None:
        fixture = TypeFixture()
        self.assert_agrees(TupleType([], fixture.std_tuple))

    def test_allow_fast_tuple_with_union_item(self) -> None:
        # A union item inside the tuple is not an Instance, so
        # allow_fast_container_literal must be False.
        fixture = TypeFixture()
        self.assert_agrees(
            TupleType([UnionType.make_union([fixture.a, fixture.b])], fixture.std_tuple)
        )
