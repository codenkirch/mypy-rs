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
from mypy.types import Type, TypedDictType


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
