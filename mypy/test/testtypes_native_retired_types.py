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
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED, _is_type_info
from mypy.test.typefixture import TypeFixture
from mypy.types import (
    CallableType,
    Instance,
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


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeExpandTypeRetiredSuite(Suite):
    """Pin the #1624 retirement of the `rust_expand_type` shim.

    The seam was the standing microbenchmark keep (the Rust port measured
    2.3x the Python visitor per call), but a load-robust instruction-count
    A/B on the cold self-check (`/usr/bin/time -l` instructions retired,
    single-process, 3+2 interleaved runs) falsified that verdict end to
    end: wrapping the seam's Python binding in a no-op (the pure-Python
    path plus the Python-side gate and wire prep that still run) saved
    8.1e9 of the 487.6e9 baseline instructions (-1.66%). A true retire
    also drops the gate and serialization, so the retire gain is at least
    the measured delta. `expand_type` now runs its pure-Python visitor;
    the Rust pyfunction stays registered for the direct-seam tests in
    `NativeExpandTypeEmptyEnvSuite` / `NativeExpandTypeAliasSuite` /
    `NativeExpandParamSpecSpliceSuite`. The sibling
    `rust_expand_type_by_instance` seam is untouched and stays live.
    """

    def setUp(self) -> None:
        from mypy.expandtype import (
            _set_native_expand_type_active,
            _set_native_expand_type_resolver,
            _set_native_expand_type_typeinfo_map,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self.fx = TypeFixture()
        self._set_active = _set_native_expand_type_active
        self._set_resolver = _set_native_expand_type_resolver
        type_infos = [v for v in vars(self.fx).values() if _is_type_info(v)]
        self._resolver = _type_kernel.build_native_resolver(type_infos, [])
        self._set_resolver(self._resolver)
        self._set_active(True)
        _set_native_expand_type_typeinfo_map({i.fullname: i for i in type_infos})
        set_wire_typeinfo_map({i.fullname: i for i in type_infos})

    def tearDown(self) -> None:
        from mypy.expandtype import _set_native_expand_type_typeinfo_map
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active(False)
        self._set_resolver(None)
        _set_native_expand_type_typeinfo_map(None)
        set_wire_typeinfo_map(None)

    def _gate_shapes(self) -> list[tuple[str, Any, Any]]:
        """(expected render, typ, env) for the shapes the retired gate served."""
        return [
            ("G[A]", self.fx.ga, {}),
            ("G[T]", self.fx.gt, {}),
            ("G[A]", self.fx.gt, {self.fx.t.id: self.fx.a}),
            ("G[A]", Instance(self.fx.gi, [self.fx.a]), {self.fx.t.id: self.fx.b}),
            ("def (B) -> A", self.fx.callable(self.fx.t, self.fx.a), {self.fx.t.id: self.fx.b}),
        ]

    def test_shim_helpers_and_source_removed(self) -> None:
        import mypy.expandtype as expandtype

        for gone in (
            "_env_substitutes_unsafe",
            "_contains_alias_raw",
            "_expand_type_decode_cache",
        ):
            assert not hasattr(expandtype, gone), f"{gone} should be retired"
        src = inspect.getsource(expandtype.expand_type)
        assert "rust_" not in src, "expand_type should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.expandtype import expand_type

        loaded = [n for n in expand_type.__code__.co_names if "rust_" in n]
        assert loaded == [], f"expand_type still loads {loaded}"

    def test_no_wire_serialization_on_the_gate_shapes(self) -> None:
        import mypy.expandtype as expandtype
        from mypy.expandtype import expand_type

        calls: list[str] = []
        orig_type = expandtype._serialize_type
        orig_env = expandtype._serialize_env

        def spy_type(t: Any) -> bytes:
            calls.append("type")
            return orig_type(t)

        def spy_env(env: Any) -> bytes:
            calls.append("env")
            return orig_env(env)

        expandtype._serialize_type = spy_type
        expandtype._serialize_env = spy_env
        try:
            for _expected, typ, env in self._gate_shapes():
                expand_type(typ, env)
        finally:
            expandtype._serialize_type = orig_type
            expandtype._serialize_env = orig_env
        assert calls == [], f"retired expand_type seam serialized: {calls}"

    def test_values_on_the_gate_shapes(self) -> None:
        from mypy.expandtype import expand_type

        for expected, typ, env in self._gate_shapes():
            got = str(expand_type(typ, env))
            assert got == expected, f"expand_type({typ}) diverged: {got!r} != {expected!r}"

    def test_pyfunction_stays_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_expand_type")
