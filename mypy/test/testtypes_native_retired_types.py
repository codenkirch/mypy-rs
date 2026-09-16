"""Retired-seam pins for the `mypy/types.py` + `mypy/nodes.py` seams (#1739).

Split by area out of `testtypes_native_retired.py`, per that file's own
instruction. Each suite pins four things: the shim name and its private
helpers are gone from the module, the production body loads no `rust_*`
global with the gate ON (zero crossings), the values still match the Python
body across the measured shapes, and the Rust pyfunction stays registered for
the direct-seam suites.
"""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

import inspect
from types import SimpleNamespace
from typing import Any
from unittest import skipUnless

import mypy.nodes as _nodes_mod
import mypy.types as _types_mod
from mypy.nodes import Block, FuncDef
from mypy.test.helpers import Suite
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED
from mypy.test.typefixture import TypeFixture
from mypy.types import (
    CallableType,
    TupleType,
    Type,
    UnpackType,
    _set_native_visitor_types_active,
    flatten_nested_tuples,
)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFlattenNestedTuplesRetiredSuite(Suite):
    """Pin the #1739 retirement of `rust_flatten_nested_tuples`.

    Measured min-of-7 ns/call, both arms in one process with the gate flag the
    only variable, against type_kernel built 2026-09-16 16:42
    (/private/tmp/mypy-rs-audit-tk):

      shape                  python   native    ratio
      nothing to flatten        84.3   5000.3   59.30x
      one nested tuple row     250.9   9205.9   36.69x
      three-level flatten      578.8  14224.2   24.58x

    Engagement was live, not a silent defer: the FFI spy reported
    `ffi=200/200 decided=200/200 used=200/200` on every shape. The shim paid
    the wire cost twice over, serializing each input row and then each decoded
    row for the `_restore_list_identity` identity fixup, to reach a Python body
    that is a plain list scan. The pyfunction stays registered.
    """

    _RETIRED_HELPERS = ("_restore_list_identity",)

    def setUp(self) -> None:
        self._orig_types_gate = _types_mod._native_visitor_types_active
        _set_native_visitor_types_active(True)
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        _set_native_visitor_types_active(self._orig_types_gate)

    def _unpack(self, items: list[Type]) -> UnpackType:
        return UnpackType(TupleType(list(items), self.fx.std_tuple))

    def _shapes(self) -> list[tuple[str, list[Type], bool]]:
        return [
            ("nothing-to-flatten", [self.fx.a, self.fx.b], True),
            ("plain-unpack-row", [self.fx.a, UnpackType(self.fx.std_tuple)], True),
            ("one-nested-row", [self.fx.a, self._unpack([self.fx.b, self.fx.c])], True),
            (
                "three-level",
                [self._unpack([self.fx.a, self._unpack([self.fx.b, self._unpack([self.fx.c])])])],
                True,
            ),
            ("one-nested-row-non-recursive", [self._unpack([self.fx.b, self.fx.c])], False),
        ]

    def test_helpers_removed(self) -> None:
        for name in self._RETIRED_HELPERS:
            assert not hasattr(_types_mod, name), f"{name} should be retired"
        assert not hasattr(_types_mod, "_rust_flatten_nested_tuples")

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        code = flatten_nested_tuples.__code__
        loaded = [n for n in code.co_names if "rust_" in n]
        assert loaded == [], f"flatten_nested_tuples still loads {loaded}"

    def test_no_wire_serialization(self) -> None:
        calls: list[str] = []
        orig_many = _types_mod._serialize_type_list_for_visitor
        orig_decode = _types_mod._deserialize_type_list_from_visitor

        def spy_many(rows: Any) -> list[bytes]:
            calls.append("serialize")
            return orig_many(rows)

        def spy_decode(bs: Any, **kw: Any) -> Any:
            calls.append("deserialize")
            return orig_decode(bs, **kw)

        _types_mod._serialize_type_list_for_visitor = spy_many  # type: ignore[assignment]
        _types_mod._deserialize_type_list_from_visitor = spy_decode
        try:
            for _, row, handle_recursive in self._shapes():
                flatten_nested_tuples(row, handle_recursive)
        finally:
            _types_mod._serialize_type_list_for_visitor = orig_many
            _types_mod._deserialize_type_list_from_visitor = orig_decode
        assert calls == [], f"retired flatten seam crossed the wire: {calls}"

    def test_values_match_python_and_keep_identity(self) -> None:
        for label, row, handle_recursive in self._shapes():
            _set_native_visitor_types_active(False)
            try:
                off = flatten_nested_tuples(row, handle_recursive)
            finally:
                _set_native_visitor_types_active(True)
            on = flatten_nested_tuples(row, handle_recursive)
            assert [str(t) for t in on] == [str(t) for t in off], label
            # The retired shim needed `_restore_list_identity` purely to fake
            # this; the Python body shares input identity for untouched rows.
            assert all(a is b for a, b in zip(off, on)), label
        nested = [self.fx.a, self._unpack([self.fx.b, self.fx.c])]
        flat = flatten_nested_tuples(nested)
        assert [str(t) for t in flat] == [str(self.fx.a), str(self.fx.b), str(self.fx.c)]
        assert flat[0] is self.fx.a
        unpacked = flatten_nested_tuples([self._unpack([self.fx.b, self.fx.c])], False)
        assert [str(t) for t in unpacked] == [str(self.fx.b), str(self.fx.c)]

    def test_pyfunction_stays_registered(self) -> None:
        assert hasattr(_type_kernel, "rust_flatten_nested_tuples")


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFuncItemIsDynamicRetiredSuite(Suite):
    """Pin the #1739 retirement of `rust_func_item_is_dynamic`.

    Measured min-of-7 ns/call, both arms in one process with the gate flag the
    only variable, against the same kernel:

      shape                   python  native   ratio
      type is None              44.8   120.9   2.70x
      explicit CallableType     64.7   452.4   7.00x
      implicit CallableType     65.8   452.5   6.88x
      non-callable Instance     67.3   338.3   5.02x

    Engagement was live: `ffi=200/200 decided=200/200` on every shape. The
    Python body is two attribute reads; the pyfunction re-resolved the
    `CallableType` class out of `mypy.types` on every call. The pyfunction
    stays registered: `rust_decorator_is_dynamic` and `rust_overloaded_is_dynamic`
    fold through it inside Rust.
    """

    def setUp(self) -> None:
        self._orig_nodes_gate = _nodes_mod._native_nodes_active
        _nodes_mod._set_native_nodes_active(True)
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        _nodes_mod._set_native_nodes_active(self._orig_nodes_gate)

    def _func(self, typ: Any) -> FuncDef:
        return FuncDef("f", [], Block([]), typ)

    def _shapes(self) -> list[tuple[str, FuncDef, bool]]:
        implicit_callable: CallableType = self.fx.callable(self.fx.a, self.fx.b)
        implicit_callable.implicit = True
        return [
            ("untyped", self._func(None), True),
            ("explicit-callable", self._func(self.fx.callable(self.fx.a, self.fx.b)), False),
            ("implicit-callable", self._func(implicit_callable), True),
            ("non-callable-instance", self._func(self.fx.a), False),
        ]

    def test_shim_and_helper_gone(self) -> None:
        assert not hasattr(_nodes_mod, "_rust_func_item_is_dynamic")
        src = inspect.getsource(_nodes_mod.FuncItem.is_dynamic)
        assert "_rust_" not in src, "FuncItem.is_dynamic should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        code = _nodes_mod.FuncItem.is_dynamic.__code__
        loaded = [n for n in code.co_names if "rust_" in n]
        assert loaded == [], f"FuncItem.is_dynamic still loads {loaded}"

    def test_values_match_python(self) -> None:
        for label, func, expected in self._shapes():
            _nodes_mod._set_native_nodes_active(False)
            try:
                off = func.is_dynamic()
            finally:
                _nodes_mod._set_native_nodes_active(True)
            on = func.is_dynamic()
            assert on == off == expected, f"{label}: off={off} on={on}"
        # `Decorator.is_dynamic` keeps its own gate and delegates to the
        # Python body now; the verdict must be unchanged.
        func = self._func(None)
        dec = SimpleNamespace(func=func)
        assert _nodes_mod.Decorator.is_dynamic(dec) is True  # type: ignore[arg-type]
        assert func.is_dynamic() is True

    def test_pyfunction_stays_registered(self) -> None:
        assert hasattr(_type_kernel, "rust_func_item_is_dynamic")
        assert hasattr(_type_kernel, "rust_decorator_is_dynamic")
