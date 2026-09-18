"""Retired-seam pins: `...RetiredSuite` classes moved out of the per-area native test files.

A retired seam's pin asserts the shim is gone, no wire crossing comes back, and the
values still match the Python body. Those pins are self-contained `Suite` subclasses
that never touch the area suites, so they live here instead of being appended to the
engagement files, where every retirement lane had to patch the same file and serialize
against the others (#1746, #1757).

Keep new pins here. Engagement suites (a `Native...Suite` without `Retired` in the name)
live in `testtypes_native_engagement_<area>.py`. If this file becomes a hotspot, split it
by area into `testtypes_native_retired_<area>.py`.
"""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from types import SimpleNamespace
from typing import Any, cast
from unittest import skipUnless

from mypy.argmap import _set_native_argmap_active
from mypy.checkexpr import ExpressionChecker
from mypy.nodes import ARG_NAMED, ARG_POS, ARG_STAR, NameExpr
from mypy.test.helpers import Suite, assert_equal
from mypy.test.testinfer import _NATIVE_ARGMAP_ENABLED
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED
from mypy.test.typefixture import TypeFixture
from mypy.types import (
    AnyType,
    CallableType,
    Instance,
    NoneType,
    ParamSpecFlavor,
    ParamSpecType,
    TupleType,
    Type,
    TypedDictType,
    TypeOfAny,
    TypeType,
    TypeVarId,
    TypeVarTupleType,
    TypeVarType,
    UnpackType,
    get_proper_type,
    has_recursive_types,
)

# Moved from mypy/test/testtypes_native_checker.py.


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckArgCountRetiredSuite(Suite):
    """Pin the #1739 retirement of `check_argument_count`'s Rust shim.

    `rust_check_argument_count` measured 1.80x (too-many actuals), 1.97x
    (too-few) and 2.03x (no-error) slower than the Python body, min-of-7
    ns/call with the gate flag the only variable. The shim classified every
    actual's proper type to a shape tag before crossing, and the crossing
    returned records that Python then translated back into messages: the
    classification alone cost more than the whole Python body.

    Structural evidence: the shim name is gone from the module and the
    method body loads no `rust_*` global, which is zero crossings with the
    gate ON. Value evidence: `NativeCheckArgCountSuite.test_on_*` pins the
    message records. The pyfunction stays registered for the `test_seam_*`
    direct-seam cases.
    """

    def test_shim_name_gone(self) -> None:
        import inspect

        from mypy import checkexpr

        assert not hasattr(checkexpr, "_rust_check_argument_count")
        src = inspect.getsource(checkexpr.ExpressionChecker.check_argument_count)
        assert "rust_" not in src, "check_argument_count should be pure Python"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.checkexpr import ExpressionChecker, _set_native_checkexpr_active

        _set_native_checkexpr_active(True)
        code = ExpressionChecker.check_argument_count.__code__
        loaded = [n for n in code.co_names if "rust_" in n]
        assert loaded == [], f"check_argument_count still loads {loaded}"

    def test_values_match_python_with_gate_on(self) -> None:
        from mypy.checkexpr import _set_native_checkexpr_active
        from mypy.nodes import TempNode

        fx = TypeFixture()
        captured: list[tuple[str, ...]] = []
        msg = SimpleNamespace(
            too_many_arguments=lambda c, ctx: captured.append(("too_many",)),
            unexpected_keyword_argument=lambda c, n, t, ctx: captured.append(("unexpected_kw", n)),
            too_many_arguments_from_typed_dict=lambda c, t, ctx: captured.append(("too_many_td",)),
            too_few_arguments=lambda c, ctx, ns: captured.append(("too_few",)),
            missing_named_argument=lambda c, ctx, n: captured.append(("missing_named", n)),
            duplicate_argument_value=lambda c, i, ctx: captured.append(("dup", str(i))),
            too_many_positional_arguments=lambda c, ctx: captured.append(("too_many_pos",)),
            fail=lambda m, ctx: captured.append(("fail", str(m))),
            note=lambda m, ctx: captured.append(("note", str(m))),
        )
        chk = SimpleNamespace(in_checked_function=lambda: True)
        ec = ExpressionChecker.__new__(ExpressionChecker)
        ec.chk = chk  # type: ignore[assignment]
        ec.msg = msg  # type: ignore[assignment]
        _set_native_checkexpr_active(True)
        context = TempNode(AnyType(TypeOfAny.special_form))
        callee = fx.callable(fx.a, fx.a, fx.anyt)  # 2 formals -> Any
        # One positional actual for two formals: too_few fires, return False.
        ok = ec.check_argument_count(callee, [fx.a], [ARG_POS], [None], [[0], []], context)
        assert ok is False
        assert ("too_few",) in captured, captured

    def test_pyfunction_stays_registered(self) -> None:
        import type_kernel

        result = type_kernel.rust_check_argument_count(
            [int(ARG_POS.value)],
            False,
            None,
            [int(ARG_POS.value)],
            [None],
            [0],
            [0],
            [[0]],
            False,
            None,
            True,
        )
        assert result == (True, [], False), result


class NativeHotSeamsRetiredSuite(Suite):
    """Pin the #1640 retirement of hot short-call seams in mypy/types.py.

    The O(1) property reads (is_generic, is_var_arg, is_kw_arg,
    min_args, max_possible, tuple/union length, can_be_true/false
    defaults, has_recursive_types) serialized whole type trees so
    Rust could read one scalar. They now run pure Python with zero
    wire crossings; this suite fails if any native call returns.
    """

    _RETIRED_HELPERS = (
        "_native_callable_is_generic",
        "_native_callable_is_kw_arg",
        "_native_callable_is_var_arg",
        "_native_callable_max_possible_positional_args",
        "_native_callable_min_args",
        "_native_can_be_false_default",
        "_native_can_be_true_default",
        "_native_tuple_length",
        "_native_union_length",
    )

    def setUp(self) -> None:
        from mypy.types import _set_native_visitor_active

        self.fx = TypeFixture()
        import mypy.types as _tmod

        self._tmod = _tmod
        self._orig_gate = _tmod._native_visitor_active
        _set_native_visitor_active(True)

    def tearDown(self) -> None:
        from mypy.types import _set_native_visitor_active

        _set_native_visitor_active(self._orig_gate or False)

    def test_helpers_removed(self) -> None:
        for name in self._RETIRED_HELPERS:
            assert not hasattr(self._tmod, name), f"{name} should be retired"

    def test_hot_reads_serialize_nothing(self) -> None:
        from mypy.types import TupleType, UnionType

        calls: list[str] = []
        orig = self._tmod._serialize_type_for_visitor

        def spy(t: Any) -> bytes:
            calls.append("serialize")
            return orig(t)

        self._tmod._serialize_type_for_visitor = spy
        try:
            c = self.fx.callable(self.fx.a, self.fx.b)
            assert c.is_generic() is False
            assert c.min_args == 1
            assert c.is_var_arg is False
            assert c.is_kw_arg is False
            assert c.max_possible_positional_args() == 1
            tt = TupleType([self.fx.a], self.fx.std_tuple)
            assert tt.length() == 1
            u = UnionType([self.fx.a, self.fx.b])
            assert u.length() == 2
            assert u.can_be_true is True
            assert has_recursive_types(self.fx.a) is False
        finally:
            self._tmod._serialize_type_for_visitor = orig
        assert calls == [], f"hot reads serialized: {len(calls)} calls"

    def test_values_match_python(self) -> None:
        from mypy.nodes import ARG_POS, ARG_STAR, ARG_STAR2
        from mypy.types import CallableType, TupleType, UnionType

        c = CallableType(
            [self.fx.a, self.fx.b], [ARG_POS, ARG_STAR], [None, None], self.fx.b, self.fx.function
        )
        assert c.min_args == 1
        assert c.is_var_arg is True
        assert c.is_kw_arg is False
        assert c.max_possible_positional_args() == 2**63 - 1
        assert c.is_generic() is False
        t0 = TupleType([], self.fx.std_tuple)
        assert t0.length() == 0
        assert t0.can_be_true is False
        assert t0.can_be_false is True
        u = UnionType([self.fx.a, self.fx.nonet])
        assert u.can_be_true is True
        assert u.can_be_false is True
        kw = CallableType([self.fx.a], [ARG_STAR2], [None], self.fx.b, self.fx.function)
        assert kw.is_kw_arg is True
        assert kw.max_possible_positional_args() == 2**63 - 1


class NativeScalarCheckmemberSeamsRetiredSuite(Suite):
    """Pin the #1668 retirement of the scalar-only checkmember wire seams.

    `descriptor_has_get_set` (audit rank 1, 22.8k calls on the cold
    self-check) and `analyze_none_member_access` (audit rank 5) both
    serialized the type tree to read a member-presence bool / tag. The
    Python bodies are a `has_readable_member("__get__")` MRO walk and a
    `name == "__bool__"` split; the native shims are retired and the Rust
    pyfunctions stay registered for direct-seam tests.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def _make_mx(self, is_lvalue: bool = False) -> Any:
        from mypy.checkmember import MemberContext
        from mypy.options import Options

        def named_type(name: str) -> Instance:
            return self.fx.bool_type if name == "builtins.bool" else self.fx.function

        chk = SimpleNamespace(
            msg=SimpleNamespace(fail=lambda *a, **kw: None, options=Options()),
            named_type=named_type,
        )
        return MemberContext(
            is_lvalue=is_lvalue,
            is_super=False,
            is_operator=False,
            original_type=self.fx.o,
            context=NameExpr("x"),
            chk=cast(Any, chk),
        )

    def _plain_instance(self) -> Instance:
        return Instance(self.fx.make_type_info("mod.Plain", mro=[self.fx.oi]), [])

    def test_native_shims_removed(self) -> None:
        import inspect

        from mypy import checkmember

        none_src = inspect.getsource(checkmember.analyze_none_member_access)
        assert "rust_" not in none_src, "analyze_none_member_access should be pure Python"
        desc_src = inspect.getsource(checkmember.analyze_descriptor_access)
        assert "rust_descriptor_has_get_set" not in desc_src
        assert not hasattr(checkmember, "_rust_analyze_none_member_access")
        assert not hasattr(checkmember, "_rust_descriptor_has_get_set")

    def test_no_wire_serialization(self) -> None:
        from mypy import checkmember
        from mypy.checkmember import analyze_descriptor_access, analyze_none_member_access

        calls: list[str] = []
        orig = checkmember._serialize_type_for_checkmember

        def spy(t: Any) -> bytes:
            calls.append("serialize")
            return orig(t)

        checkmember._serialize_type_for_checkmember = spy
        try:
            mx = self._make_mx()
            bool_member = analyze_none_member_access("__bool__", NoneType(), mx)
            assert str(bool_member) == "def () -> Literal[False]"
            plain = self._plain_instance()
            assert analyze_descriptor_access(plain, mx) is plain
        finally:
            checkmember._serialize_type_for_checkmember = orig
        assert calls == [], f"retired checkmember seams serialized: {len(calls)} calls"

    def test_values_match_python(self) -> None:
        from mypy.checkmember import analyze_descriptor_access, analyze_none_member_access

        mx = self._make_mx()
        bool_member = get_proper_type(analyze_none_member_access("__bool__", NoneType(), mx))
        assert isinstance(bool_member, CallableType)
        assert str(bool_member.ret_type) == "Literal[False]"
        plain = self._plain_instance()
        assert analyze_descriptor_access(plain, mx) is plain
        assert analyze_descriptor_access(plain, self._make_mx(is_lvalue=True)) is plain


class NativeResidualScalarCheckmemberSeamsRetiredSuite(Suite):
    """Pin the #1668 residual sweep of the scalar-only checkmember seams.

    Four seams serialized the type tree to read a tag or a single field:
    `analyze_typeddict_access` (audit rank 10, `__delitem__` returns a fixed
    CallableType), `bind_self_fast` (rank 9, its shim rebuilt the live method
    anyway), `instance_fallback` and `meta_has_operator` (both zero
    engagement on the self-check). The Python bodies read live objects; the
    native shims are retired and the Rust pyfunctions stay registered for
    direct-seam tests.
    """

    _SEAMS = {
        "bind_self_fast": "rust_bind_self_fast",
        "instance_fallback": "rust_instance_fallback",
        "meta_has_operator": "rust_meta_has_operator",
    }

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def _make_mx(self) -> Any:
        from mypy.checkmember import MemberContext
        from mypy.options import Options

        def named_type(name: str) -> Instance:
            if name == "builtins.str":
                return self.fx.str_type
            return self.fx.function

        chk = SimpleNamespace(
            msg=SimpleNamespace(fail=lambda *a, **kw: None, options=Options()),
            named_type=named_type,
        )
        return MemberContext(
            is_lvalue=False,
            is_super=False,
            is_operator=False,
            original_type=self.fx.o,
            context=NameExpr("x"),
            chk=cast(Any, chk),
        )

    def _typeddict(self) -> TypedDictType:
        return TypedDictType({}, set(), set(), self.fx.o)

    def _method(self) -> CallableType:
        return CallableType(
            [self.fx.a, self.fx.b], [ARG_POS, ARG_POS], [None, None], self.fx.o, self.fx.function
        )

    def _meta_instance(self) -> Instance:
        meta_info = self.fx.make_type_info("mod.Meta", mro=[self.fx.oi])
        base_info = self.fx.make_type_info("mod.Base", mro=[self.fx.oi])
        inst = Instance(base_info, [])
        inst.type.metaclass_type = Instance(meta_info, [])
        return inst

    def test_native_shims_removed(self) -> None:
        import inspect

        from mypy import checkmember

        for func_name, seam_name in self._SEAMS.items():
            src = inspect.getsource(getattr(checkmember, func_name))
            assert seam_name not in src, f"{func_name} should not call {seam_name}"
        td_src = inspect.getsource(checkmember.analyze_typeddict_access)
        assert "rust_" not in td_src, "analyze_typeddict_access should be pure Python"
        for shim in (
            "_rust_analyze_typeddict_access",
            "_rust_bind_self_fast",
            "_rust_instance_fallback",
            "_rust_meta_has_operator",
        ):
            assert not hasattr(checkmember, shim), f"{shim} should be gone"

    def test_no_wire_serialization(self) -> None:
        from mypy import checkmember
        from mypy.checkmember import (
            analyze_typeddict_access,
            bind_self_fast,
            instance_fallback,
            meta_has_operator,
        )

        calls: list[str] = []
        orig = checkmember._serialize_type_for_checkmember

        def spy(t: Any) -> bytes:
            calls.append("serialize")
            return orig(t)

        checkmember._serialize_type_for_checkmember = spy
        try:
            deleted = get_proper_type(
                analyze_typeddict_access("__delitem__", self._typeddict(), self._make_mx(), None)
            )
            assert isinstance(deleted, CallableType)
            assert deleted.name == "__delitem__"
            assert bind_self_fast(self._method()).is_bound is True
            assert instance_fallback(self.fx.lit_str1) is self.fx.str_type
            assert meta_has_operator(AnyType(TypeOfAny.special_form), "__add__") is True
        finally:
            checkmember._serialize_type_for_checkmember = orig
        assert calls == [], f"retired checkmember seams serialized: {len(calls)} calls"

    def test_values_match_python(self) -> None:
        from mypy.checkmember import (
            analyze_typeddict_access,
            bind_self_fast,
            instance_fallback,
            meta_has_operator,
        )

        deleted = get_proper_type(
            analyze_typeddict_access("__delitem__", self._typeddict(), self._make_mx(), None)
        )
        assert isinstance(deleted, CallableType)
        assert [str(a) for a in deleted.arg_types] == ["builtins.str"]
        assert str(deleted.ret_type) == "None"
        assert str(deleted.fallback) == "builtins.function"
        bound = bind_self_fast(self._method())
        assert isinstance(bound, CallableType)
        assert bound.arg_types == [self.fx.b]
        assert bound.is_bound is True
        assert instance_fallback(self.fx.a) is self.fx.a
        assert instance_fallback(self._typeddict()) is self.fx.o
        assert meta_has_operator(AnyType(TypeOfAny.special_form), "__add__") is True
        assert meta_has_operator(self._meta_instance(), "__add__") is False


# Moved from mypy/test/testtypes_native_types.py.


class NativeIsLiteralTypeLikeRetiredSuite(Suite):
    """Pin the #1661 retirement of the is_literal_type_like wire seam.

    `is_literal_type_like` (typeops.py) serialized the whole type tree
    so Rust could read one scalar bool (LiteralType/Union/TypeVar walk).
    At 248k calls / 100% native share on the cold self-check the wire
    round-trip cost more than the decision was worth (shape d, #1624
    audit rank #10, 0.32s proxy). The native shim is now retired; the
    Rust pyfunction stays registered for direct-seam tests. This suite
    fails if any native call returns when is_literal_type_like runs.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_native_shim_removed(self) -> None:
        import inspect

        from mypy import typeops

        src = inspect.getsource(typeops.is_literal_type_like)
        assert (
            "rust_is_literal_type_like" not in src
        ), "is_literal_type_like should not call the native seam"

    def test_no_wire_serialization(self) -> None:
        from mypy import typeops
        from mypy.typeops import is_literal_type_like

        calls: list[str] = []
        orig = typeops._serialize_type

        def spy(t: Any) -> bytes:
            calls.append("serialize")
            return orig(t)

        typeops._serialize_type = spy
        try:
            assert is_literal_type_like(self.fx.lit_str1) is True
            from mypy.types import UnionType

            u = UnionType([self.fx.lit_str1, self.fx.a])
            assert is_literal_type_like(u) is True
            assert is_literal_type_like(self.fx.a) is False
            assert is_literal_type_like(None) is False
        finally:
            typeops._serialize_type = orig
        assert calls == [], f"is_literal_type_like serialized: {len(calls)} calls"

    def test_values_match(self) -> None:
        from mypy.typeops import is_literal_type_like
        from mypy.types import UnionType

        assert is_literal_type_like(self.fx.lit_str1) is True
        assert is_literal_type_like(self.fx.a) is False
        assert is_literal_type_like(None) is False
        u = UnionType([self.fx.lit_str1, self.fx.a])
        assert is_literal_type_like(u) is True
        u2 = UnionType([self.fx.a, self.fx.b])
        assert is_literal_type_like(u2) is False


class NativeScalarTypeopsSeamsRetiredSuite(Suite):
    """Pin the #1668 retirement of the scalar-only typeops wire seams.

    `is_recursive_pair`, `is_singleton_identity_type` and
    `is_singleton_equality_type` serialized the whole type tree so Rust
    could read a couple of scalar fields (audit ranks 2-4, #1637). The
    Python bodies are shallow isinstance chains, so the wire round-trip
    cost more than the decision. The native shims are retired; the Rust
    pyfunctions stay registered for direct-seam tests. This suite fails
    if any native call returns on these paths.
    """

    _SEAMS = {
        "is_recursive_pair": "rust_is_recursive_pair",
        "is_singleton_identity_type": "rust_is_singleton_identity_type",
        "is_singleton_equality_type": "rust_is_singleton_equality_type",
    }

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_native_shims_removed(self) -> None:
        import inspect

        from mypy import typeops

        for func_name, seam_name in self._SEAMS.items():
            src = inspect.getsource(getattr(typeops, func_name))
            assert seam_name not in src, f"{func_name} should not call {seam_name}"

    def test_no_wire_serialization(self) -> None:
        from mypy import typeops
        from mypy.typeops import (
            is_recursive_pair,
            is_singleton_equality_type,
            is_singleton_identity_type,
        )

        calls: list[str] = []
        orig = typeops._serialize_type

        def spy(t: Any) -> bytes:
            calls.append("serialize")
            return orig(t)

        typeops._serialize_type = spy
        try:
            recursive_alias, _ = self.fx.def_alias_1(self.fx.a)
            assert is_recursive_pair(recursive_alias, self.fx.b) is True
            assert is_recursive_pair(self.fx.a, self.fx.b) is False
            assert is_singleton_identity_type(NoneType()) is True
            assert is_singleton_identity_type(self.fx.a) is False
            assert is_singleton_equality_type(self.fx.lit1) is True
            assert is_singleton_equality_type(self.fx.a) is False
        finally:
            typeops._serialize_type = orig
        assert calls == [], f"retired typeops seams serialized: {len(calls)} calls"

    def test_values_match_python(self) -> None:
        from mypy.typeops import (
            is_recursive_pair,
            is_singleton_equality_type,
            is_singleton_identity_type,
        )

        recursive_alias, _ = self.fx.def_alias_1(self.fx.a)
        assert is_recursive_pair(recursive_alias, self.fx.b) is True
        assert is_recursive_pair(self.fx.a, self.fx.b) is False
        assert is_singleton_identity_type(NoneType()) is True
        assert is_singleton_identity_type(self.fx.a) is False
        assert is_singleton_equality_type(self.fx.lit1) is True
        assert is_singleton_equality_type(self.fx.a) is False


class NativeResidualScalarTypeopsSeamsRetiredSuite(Suite):
    """Pin the #1668 residual sweep of the scalar-only typeops seams.

    `simple_literal_type` (audit rank 6), `erase_to_bound` (rank 11) and
    `is_simple_literal` (zero engagement on the self-check) each serialized
    the type tree so Rust could read one field. The Python bodies are
    isinstance chains over live objects, so the wire round-trip cost more
    than the decision. The native shims are retired; the Rust pyfunctions
    stay registered for direct-seam tests. This suite fails if any of these
    paths serializes again.
    """

    _SEAMS = {
        "erase_to_bound": "rust_erase_to_bound",
        "simple_literal_type": "rust_simple_literal_type",
        "is_simple_literal": "rust_is_simple_literal",
    }

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_native_shims_removed(self) -> None:
        import inspect

        from mypy import typeops

        for func_name, seam_name in self._SEAMS.items():
            src = inspect.getsource(getattr(typeops, func_name))
            assert seam_name not in src, f"{func_name} should not call {seam_name}"

    def test_no_wire_serialization(self) -> None:
        from mypy import typeops
        from mypy.typeops import erase_to_bound, is_simple_literal, simple_literal_type

        calls: list[str] = []
        orig = typeops._serialize_type

        def spy(t: Any) -> bytes:
            calls.append("serialize")
            return orig(t)

        typeops._serialize_type = spy
        try:
            assert erase_to_bound(self.fx.t) is self.fx.o
            assert simple_literal_type(self.fx.lit_str1) is self.fx.str_type
            assert is_simple_literal(self.fx.lit_str1) is True
        finally:
            typeops._serialize_type = orig
        assert calls == [], f"retired typeops seams serialized: {len(calls)} calls"

    def test_values_match_python(self) -> None:
        from mypy.typeops import erase_to_bound, is_simple_literal, simple_literal_type

        assert erase_to_bound(self.fx.a) is self.fx.a
        assert erase_to_bound(self.fx.t) is self.fx.o
        assert erase_to_bound(TypeType(self.fx.t)) == TypeType.make_normalized(self.fx.o)
        assert simple_literal_type(self.fx.lit_str1) is self.fx.str_type
        assert simple_literal_type(self.fx.lit_str1_inst) is self.fx.str_type
        assert simple_literal_type(self.fx.lit1) is self.fx.a
        assert simple_literal_type(self.fx.a) is None
        assert simple_literal_type(None) is None
        assert is_simple_literal(self.fx.lit_str1) is True
        assert is_simple_literal(self.fx.lit_str1_inst) is True
        assert is_simple_literal(self.fx.lit1) is False
        assert is_simple_literal(self.fx.a) is False


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeWireRoundTripSeamsRetiredSuite(Suite):
    """Pin the #1739 retirement of the copy_modified / flatten seams.

    `copy_modified` and `flatten_nested_unions` serialized a whole type
    tree to reach an O(1)/O(n) Python body. Measured min-of-7 ns/call on
    the live seam, with both arms in one process: the wire path cost
    17x the Python body for an Instance arg swap, 72x for a TupleType
    item swap, 16x for a callable return-type swap, and 26x/14x for
    flatten with nothing / one union row to flatten. Both now run pure
    Python; the Rust pyfunctions stay registered for direct-seam tests.
    `rust_expand_type` has since been retired too (#1624): its microbenchmark
    verdict (2.3x faster) did not survive the end-to-end instruction-count
    A/B on the cold self-check, where deferring the seam saved 1.66% of
    retired instructions.

    This suite fails if a wire crossing returns on either path.
    """

    _RETIRED = (
        "_native_copy_modified",
        "_serialize_copy_modified_value",
        "_collect_wire_meta_types",
        "_restore_wire_meta",
        "_restore_wire_var_identity",
        "_restore_wire_lines",
        "_restore_wire_type_lines",
        "_restore_flat_row_flags",
        "_WireMetaCollector",
        "_WireMetaFixer",
        "_WireVarCanonCollector",
        "_WireVarCanonizer",
    )

    def setUp(self) -> None:
        import mypy.types as _types_mod
        from mypy.types import _set_native_visitor_active, _set_native_visitor_types_active

        self._types_mod = _types_mod
        self._orig_visitor_gate = _types_mod._native_visitor_active
        self._orig_types_gate = _types_mod._native_visitor_types_active
        _set_native_visitor_active(True)
        _set_native_visitor_types_active(True)
        self.fx = TypeFixture()

    def tearDown(self) -> None:
        from mypy.types import _set_native_visitor_active, _set_native_visitor_types_active

        _set_native_visitor_active(self._orig_visitor_gate)
        _set_native_visitor_types_active(self._orig_types_gate)

    def test_helpers_removed(self) -> None:
        for name in self._RETIRED:
            assert not hasattr(self._types_mod, name), f"{name} should be retired"

    def test_native_shims_removed(self) -> None:
        import inspect

        from mypy.types import (
            CallableType,
            Instance,
            TupleType,
            TypedDictType,
            flatten_nested_unions,
        )

        seams = ("_rust_copy_modified", "_native_copy_modified", "_rust_flatten_nested_unions")
        funcs = (
            flatten_nested_unions,
            Instance.copy_modified,
            TupleType.copy_modified,
            CallableType.copy_modified,
            TypedDictType.copy_modified,
        )
        for func in funcs:
            src = inspect.getsource(func)
            for seam in seams:
                assert seam not in src, f"{func.__qualname__} must not consult {seam}"

    def test_pyfunctions_still_registered(self) -> None:
        for name in ("rust_copy_modified", "rust_flatten_nested_unions"):
            assert hasattr(_type_kernel, name), f"{name} must stay callable"
        assert hasattr(_type_kernel, "rust_expand_type"), "expand_type port must stay"

    def test_no_wire_serialization(self) -> None:
        from mypy.types import Instance, TupleType, TypedDictType, flatten_nested_unions

        calls: list[str] = []
        orig_one = self._types_mod._serialize_type_for_visitor
        orig_many = self._types_mod._serialize_type_list_for_visitor

        def spy_one(t: Any) -> bytes:
            calls.append("type")
            return orig_one(t)

        def spy_many(rows: Any) -> list[bytes]:
            calls.append("list")
            return orig_many(rows)

        self._types_mod._serialize_type_for_visitor = spy_one
        self._types_mod._serialize_type_list_for_visitor = spy_many  # type: ignore[assignment]
        try:
            inst = Instance(self.fx.ai, [self.fx.a])
            assert [str(x) for x in inst.copy_modified(args=[self.fx.b]).args] == [str(self.fx.b)]
            tup = TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)
            assert [str(x) for x in tup.copy_modified(items=[self.fx.c]).items] == [str(self.fx.c)]
            cb = self.fx.callable(self.fx.a, self.fx.b)
            assert str(cb.copy_modified(ret_type=self.fx.c).ret_type) == str(self.fx.c)
            td = TypedDictType({"x": self.fx.a}, {"x"}, set(), self.fx.a)
            assert [str(x) for x in td.copy_modified(item_types=[self.fx.b]).items.values()] == [
                str(self.fx.b)
            ]
            assert [str(x) for x in flatten_nested_unions([self.fx.a, self.fx.b])] == [
                str(self.fx.a),
                str(self.fx.b),
            ]
        finally:
            self._types_mod._serialize_type_for_visitor = orig_one
            self._types_mod._serialize_type_list_for_visitor = orig_many
        assert calls == [], f"retired copy_modified/flatten seam serialized: {calls}"

    def test_values_match_python(self) -> None:
        from mypy.types import (
            CallableType,
            Instance,
            TupleType,
            TypedDictType,
            UnionType,
            flatten_nested_unions,
        )

        inst = Instance(self.fx.ai, [self.fx.a], last_known_value=self.fx.lit1)
        inst.can_be_true = False
        inst.can_be_false = True
        inst_copy = inst.copy_modified(args=[self.fx.b])
        assert [str(x) for x in inst_copy.args] == [str(self.fx.b)]
        assert inst_copy.last_known_value is self.fx.lit1
        assert inst_copy.can_be_true is False and inst_copy.can_be_false is True

        tup = TupleType([self.fx.a, self.fx.b], self.fx.std_tuple, implicit=True)
        assert tup.implicit is True
        new_tup = tup.copy_modified(items=[self.fx.c])
        assert [str(x) for x in new_tup.items] == [str(self.fx.c)]
        # Python's constructor resets implicit; the wire path had to patch
        # this back to match.
        assert new_tup.implicit is False

        cb: CallableType = self.fx.callable(self.fx.a, self.fx.b)
        modified_cb = cb.copy_modified(ret_type=self.fx.c)
        assert isinstance(modified_cb, CallableType)
        assert str(modified_cb.ret_type) == str(self.fx.c)
        assert modified_cb.arg_types == cb.arg_types
        assert modified_cb.fallback is cb.fallback

        td = TypedDictType({"x": self.fx.a, "y": self.fx.b}, {"x", "y"}, set(), self.fx.a)
        assert [
            str(x) for x in td.copy_modified(item_types=[self.fx.c, self.fx.c]).items.values()
        ] == [str(self.fx.c), str(self.fx.c)]
        # item_names filtering exists only on the Python path.
        filtered = td.copy_modified(item_types=[self.fx.c, self.fx.c], item_names=["x"])
        assert set(filtered.items) == {"x"}

        rows = flatten_nested_unions([self.fx.a, self.fx.b])
        assert rows[0] is self.fx.a and rows[1] is self.fx.b
        flat = flatten_nested_unions([UnionType([self.fx.a, self.fx.b])])
        assert [str(x) for x in flat] == [str(self.fx.a), str(self.fx.b)]
        assert flat[0] is self.fx.a and flat[1] is self.fx.b


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeFillTypevarsRetiredSuite(Suite):
    """Pin the #1739 retirement of the fill_typevars wire seam.

    `fill_typevars` had Rust re-serialize every class type parameter and
    wrap it in an encoded Instance, then re-decoded that Instance in
    Python to read `root.args`. Measured min-of-7 ns/call, both arms in
    one process, gate flag the only variable, decode cache warm, against
    type_kernel built 2026-09-16 16:42 (/private/tmp/mypy-rs-audit-tk):

      shape                      python   native   ratio
      non-generic (0 tvars)       250.3    736.1   2.94x
      G[T] (1 tvar)               909.1   2970.2   3.27x
      H[S,T] (2 tvars)           1486.1   5231.6   3.52x
      W8 (8 tvars)               4773.4  16547.6   3.47x
      named tuple                 519.9   3547.2   6.82x
      V[T, *Ts]                  1629.5   6917.5   4.25x
      P[**P] (ParamSpec)         1356.9   3492.9   2.57x

    With the decode cache cold the loss widens to 12.5-22x. `fill_typevars`
    now runs pure Python; `rust_fill_typevars` stays registered for
    direct-seam tests (`NativeFillTypevarsSuite`). This suite fails if a
    wire crossing returns on this path.
    """

    def setUp(self) -> None:
        from mypy.typevars import _set_native_typevars_active

        self.fx = TypeFixture()
        self._set_gate = _set_native_typevars_active
        self._set_gate(True)

    def tearDown(self) -> None:
        self._set_gate(False)

    def test_native_shim_removed(self) -> None:
        import inspect

        from mypy.typevars import fill_typevars

        src = inspect.getsource(fill_typevars)
        assert "rust_fill_typevars" not in src, "fill_typevars must not call the seam"
        assert "_native_decode_well_formed" not in src, "decode helper is unreachable now"

    def test_pyfunction_still_registered(self) -> None:
        assert hasattr(_type_kernel, "rust_fill_typevars"), "seam must stay callable"

    def test_no_wire_crossing(self) -> None:
        from mypy import typevars as _typevars_mod
        from mypy.typevars import fill_typevars

        real_mod = _typevars_mod._type_kernel  # type: ignore[attr-defined]
        seen: list[str] = []

        class _Counter:
            def __getattr__(self, name: str) -> Any:
                return getattr(real_mod, name)

            def rust_fill_typevars(self, *args: Any, **kw: Any) -> Any:
                seen.append("rust_fill_typevars")
                return real_mod.rust_fill_typevars(*args, **kw)

            def rust_fill_typevars_with_any(self, *args: Any, **kw: Any) -> Any:
                seen.append("rust_fill_typevars_with_any")
                return real_mod.rust_fill_typevars_with_any(*args, **kw)

        _typevars_mod._type_kernel = _Counter()  # type: ignore[attr-defined, assignment]
        try:
            assert _typevars_mod._native_typevars_active, "pin requires the gate on"
            for info in (self.fx.ai, self.fx.gi, self.fx.hi):
                fill_typevars(info)
        finally:
            _typevars_mod._type_kernel = real_mod  # type: ignore[attr-defined]
        assert seen == [], f"fill_typevars crossed the wire: {seen}"

    def test_values_match_python(self) -> None:
        from mypy.typevars import fill_typevars

        plain = fill_typevars(self.fx.ai)
        assert isinstance(plain, Instance)
        assert plain.type is self.fx.ai and list(plain.args) == []

        one = fill_typevars(self.fx.gi)
        assert isinstance(one, Instance) and one.type is self.fx.gi
        source_tv = cast(TypeVarType, self.fx.gi.defn.type_vars[0])
        assert list(one.args) == [source_tv.copy_modified(line=-1, column=-1)]
        assert one.args[0].line == -1 and one.args[0].column == -1

        two = fill_typevars(self.fx.hi)
        assert isinstance(two, Instance) and two.type is self.fx.hi
        assert [a.line for a in two.args] == [-1, -1]
        assert [a.column for a in two.args] == [-1, -1]

        variadic = self.fx.make_type_info(
            "V", mro=[self.fx.oi], typevars=["T", "Ts"], typevar_tuple_index=1
        )
        unpacked = fill_typevars(variadic)
        assert isinstance(unpacked, Instance) and unpacked.type is variadic
        assert isinstance(unpacked.args[1], UnpackType)
        assert unpacked.args[1].from_star_syntax is False
        assert isinstance(unpacked.args[1].type, TypeVarTupleType)
        assert unpacked.args[1].type.name == "Ts"
        assert unpacked.args[1].type.line == -1 and unpacked.args[1].type.column == -1

        paramspec = self.fx.make_type_info("P", mro=[self.fx.oi])
        paramspec.defn.type_vars = [
            ParamSpecType(
                "P",
                "P",
                TypeVarId(1),
                ParamSpecFlavor.BARE,
                Instance(self.fx.oi, [], -1),
                NoneType(),
            )
        ]
        ps = fill_typevars(paramspec)
        assert isinstance(ps, Instance) and ps.type is paramspec
        assert isinstance(ps.args[0], ParamSpecType) and ps.args[0].line == -1

        named = self.fx.make_type_info(
            "NT",
            mro=[self.fx.oi, self.fx.std_tuplei],
            bases=[Instance(self.fx.std_tuplei, [self.fx.a])],
        )
        named.tuple_type = TupleType(
            [self.fx.a, self.fx.b], Instance(self.fx.std_tuplei, [self.fx.o])
        )
        nt = fill_typevars(named)
        assert isinstance(nt, TupleType)
        assert nt.partial_fallback.type is named
        assert [str(i) for i in nt.items] == [str(self.fx.a), str(self.fx.b)]
        # The source tuple_type keeps its own fallback: copy_modified, not mutation.
        source = named.tuple_type
        assert isinstance(source, TupleType)
        assert source.partial_fallback.type is self.fx.std_tuplei


# Moved from mypy/test/testtypes_native_misc.py.


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeRetiredPredicateDirectSuite(Suite):
    """Direct pins for retired predicate pyfunctions (#1739).

    The shims lost to their Python bodies and were retired; the
    pyfunctions stay registered, and these tests keep them exercised.
    """

    def setUp(self) -> None:
        import type_kernel as _tk

        self._tk = _tk
        self.fx = TypeFixture()

    def test_func_has_self_or_cls_argument(self) -> None:
        from mypy.nodes import Block, FuncDef

        plain = FuncDef("f", [], Block([]))
        assert self._tk.rust_func_has_self_or_cls_argument(plain) is True
        static = FuncDef("f", [], Block([]))
        static.is_static = True
        assert self._tk.rust_func_has_self_or_cls_argument(static) is False
        dunder_new = FuncDef("__new__", [], Block([]))
        dunder_new.is_static = True
        assert self._tk.rust_func_has_self_or_cls_argument(dunder_new) is True

    def test_is_true_false_literal(self) -> None:
        from mypy.nodes import IntExpr, NameExpr

        t = NameExpr("True")
        t.fullname = "builtins.True"
        assert self._tk.rust_is_true_literal(t) is True
        assert self._tk.rust_is_false_literal(t) is False
        z = IntExpr(0)
        assert self._tk.rust_is_true_literal(z) is False
        assert self._tk.rust_is_false_literal(z) is True

    def test_has_placeholder(self) -> None:
        from mypy.types import PlaceholderType

        assert self._tk.rust_has_placeholder(self.fx.o) is False
        ph = PlaceholderType("mod.x", [], 1)
        assert self._tk.rust_has_placeholder(ph) is True


# Moved from mypy/test/testtypes_native_semanal.py.


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeReplaceImplicitFirstTypeRetiredSuite(Suite):
    """Pin the #1663 retirement of the replace_implicit_first_type seam.

    Production calls always serialized a method `CallableType` whose
    fallback is still a `FakeInfo` ("fallback can't be filled out until
    semanal"), so the wire encode raised on every call and the Python body
    ran anyway: 21,974 Python calls / 0 Rust engagements on the cold
    self-check. The Rust pyfunction stays registered for direct-seam tests.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_native_shim_removed(self) -> None:
        import inspect

        from mypy import semanal

        src = inspect.getsource(semanal.replace_implicit_first_type)
        assert (
            "rust_replace_implicit_first_type" not in src
        ), "replace_implicit_first_type should not call the native seam"

    def test_no_wire_serialization(self) -> None:
        from mypy import semanal

        sig = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.anyt,
            self.fx.function,
        )
        calls: list[Type] = []
        orig = semanal._serialize_semanal_type

        def spy(t: Type) -> bytes:
            calls.append(t)
            return orig(t)

        semanal._serialize_semanal_type = spy
        semanal._set_native_semanal_active(True)
        try:
            out = semanal.replace_implicit_first_type(sig, self.fx.o)
        finally:
            semanal._serialize_semanal_type = orig
            semanal._set_native_semanal_active(False)
        assert calls == []
        assert isinstance(out, CallableType)
        assert_equal(out.arg_types, [self.fx.o, self.fx.b])

    def test_rust_pyfunction_still_registered(self) -> None:
        import type_kernel as _type_kernel
        from librt.internal import ReadBuffer

        from mypy.semanal import _serialize_semanal_type
        from mypy.types import read_type as _read_type

        sig = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fx.anyt,
            self.fx.function,
        )
        result = _type_kernel.rust_replace_implicit_first_type(
            _serialize_semanal_type(sig), _serialize_semanal_type(self.fx.o)
        )
        assert result is not None
        decoded = get_proper_type(_read_type(ReadBuffer(bytes(result))))
        assert isinstance(decoded, CallableType)
        assert len(decoded.arg_types) == 2
        # The first slot is the replacement; the round-trip keeps the
        # unresolved type_ref because fixup_wire_type is not applied here.
        first = get_proper_type(decoded.arg_types[0])
        assert isinstance(first, Instance)
        assert first.type_ref == "builtins.object"


# Moved from mypy/test/testinfer.py.


@skipUnless(_NATIVE_ARGMAP_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeArgMapSeamsRetiredSuite(Suite):
    """Pin the #1739 retirement of the argmap mapping seams.

    Retired: `rust_map_actuals_to_formals` (3.13x-4.15x slower), the
    star-actual `rust_map_actuals_to_formals_with_types` (4.25x-6.35x),
    `rust_map_formals_to_actuals` (2.25x), and `_serialize_actual_type`,
    the helper reachable only from the star path. Shadows like these paid
    list conversions plus a whole-tree type serialization to answer a
    pure-Python list walk over live objects.

    Kept: `rust_expand_actual_type`, whose Python body is a recursive
    visitor and which measured 0.43-0.61x (up to 2.3x faster).

    Values are pinned by `MapActualsToFormalsSuite`,
    `MapActualsToFormalsStarSuite` and `MapFormalsToActualsSuite`, which run
    with the gate ON (module-level env flip). This suite pins the structure:
    the shim names are gone from `mypy.argmap` and the retired bodies load
    no `rust_*` global, which is zero crossings with the gate on. The Rust
    pyfunctions stay registered for direct-seam calls.
    """

    _GONE = (
        "_rust_map_actuals_to_formals",
        "_rust_map_actuals_to_formals_with_types",
        "_rust_map_formals_to_actuals",
        "_serialize_actual_type",
    )

    def setUp(self) -> None:
        # Zero-crossing claims below are made with the production gate ON.
        _set_native_argmap_active(True)

    def test_retired_shim_names_gone(self) -> None:
        from mypy import argmap

        for name in self._GONE:
            assert not hasattr(argmap, name), f"{name} should be gone"

    def test_surviving_seam_still_registered(self) -> None:
        from mypy import argmap

        assert argmap._HAS_TYPE_KERNEL, "expand_actual_type seam must stay live"
        # The alias is private inside `mypy.argmap` (not a re-export), so it
        # is read by string. Direct access trips the self-check's
        # implicit_reexport=False; ruff's B009 fix would reintroduce it.
        assert getattr(argmap, "_rust_expand_actual_type") is not None  # noqa: B009
        assert argmap._HAS_LIBRT

    def test_retired_bodies_load_no_rust_name(self) -> None:
        from mypy.argmap import map_actuals_to_formals, map_formals_to_actuals

        for fn in (map_actuals_to_formals, map_formals_to_actuals):
            loaded = [n for n in fn.__code__.co_names if "rust_" in n]
            assert loaded == [], f"{fn.__name__} still loads {loaded}"

    def test_pyfunctions_stay_callable(self) -> None:
        # Rule 5 of the retirement method: the Rust entry points remain
        # registered even though production no longer calls them.
        import type_kernel

        for name in (
            "rust_map_actuals_to_formals",
            "rust_map_actuals_to_formals_with_types",
            "rust_map_formals_to_actuals",
        ):
            assert callable(getattr(type_kernel, name)), f"{name} should stay registered"
        assert type_kernel.rust_map_actuals_to_formals(
            [int(ARG_POS.value)], [None], [int(ARG_POS.value)], [None]
        ) == [[0]]
        assert (
            type_kernel.rust_map_actuals_to_formals(
                [int(ARG_STAR.value)], [None], [int(ARG_STAR.value)], [None]
            )
            is None
        ), "the plain ticket must still defer on star actuals"

    def test_values_match_python_with_gate_on(self) -> None:
        from mypy.argmap import map_actuals_to_formals

        fixture = TypeFixture()
        # The production gate is ON here; the values must be the Python ones.
        result = map_actuals_to_formals(
            [ARG_POS, ARG_NAMED],
            [None, "y"],
            [ARG_POS, ARG_POS],
            ["x", "y"],
            lambda i: fixture.anyt,
        )
        assert_equal(result, [[0], [1]])
        star = map_actuals_to_formals(
            [ARG_STAR], [None], [ARG_POS, ARG_POS], [None, None], lambda i: fixture.std_tuple
        )
        assert_equal(star, [[0], [0]])
