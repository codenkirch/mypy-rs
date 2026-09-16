"""Retired-seam pins for the checkmember area (#1739, lane R-C).

`...RetiredSuite` pins live in one file per area so that retirement lanes never
collide on a shared suite file. A retired seam has no Python shim (the module
calls the pure-Python body directly) while the Rust pyfunction stays registered
for the direct-seam tests in `testtypes_native_checker.py`. See the retirement
log at the top of `docs/plans/type-kernel-seam-ledger.md`.
"""

from __future__ import annotations

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from types import SimpleNamespace
from typing import Any
from unittest import skipUnless

from mypy.nodes import MDEF, Block, ClassDef, NameExpr, SymbolTable, SymbolTableNode, TypeInfo, Var
from mypy.test.helpers import Suite
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED
from mypy.types import AnyType, CallableType, Instance, PartialType, TypeOfAny


def _make_typeinfo(fullname: str) -> TypeInfo:
    defn = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
    defn.fullname = fullname
    info = TypeInfo(SymbolTable(), defn, "mod")
    defn.info = info
    info.mro = [info]
    return info


def _make_var(
    name: str,
    info: TypeInfo,
    typ: Any = None,
    *,
    is_classvar: bool = False,
    is_inferred: bool = False,
    is_ready: bool = True,
    setter_type: CallableType | None = None,
    is_settable_property: bool = False,
    register: bool = True,
) -> Var:
    var = Var(name)
    if typ is not None:
        var.type = typ
    var.info = info
    var.is_classvar = is_classvar
    var.is_inferred = is_inferred
    var.is_ready = is_ready
    var.setter_type = setter_type
    var.is_settable_property = is_settable_property
    if register:
        info.names[name] = SymbolTableNode(MDEF, var)
    return var


def _make_mx(itype: Instance, is_lvalue: bool = False) -> Any:
    from mypy.checkmember import MemberContext

    msg = SimpleNamespace(
        read_only_property=lambda *a: None,
        cant_assign_to_classvar=lambda *a: None,
        cant_assign_to_method=lambda *a: None,
    )
    chk = SimpleNamespace(
        msg=msg,
        plugin=SimpleNamespace(get_attribute_hook=lambda fullname: None),
        handle_cannot_determine_type=lambda name, ctx: None,
        handle_partial_var_type=lambda typ, lv, var, ctx: AnyType(TypeOfAny.special_form),
    )
    return MemberContext(
        is_lvalue=is_lvalue,
        is_super=False,
        is_operator=False,
        original_type=itype,
        context=NameExpr("A"),
        chk=chk,  # type: ignore[arg-type]
    )


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeCheckmemberRetiredSeamsSuite(Suite):
    """Pin the #1739 retirements of two losing checkmember seams.

    Both were unmeasured at 99,852 (`rust_classify_analyze_var`) and 44,309
    (`rust_is_instance_var`) calls per corpus. Measured min-of-7 ns/call, both
    arms in one process and the FFI ticket spied (200/200 decided on every
    shape, so neither arm silently deferred), the native side lost on every
    shape: the serializer plus FFI ticket cost 1.6x-4.5x the inline
    `analyze_var` head (2.2-12.1 us/call, wire-cache hit included) and the
    four-scalar PyO3 read cost 3.4x-5.5x the `is_instance_var` conjunction
    (380-530 vs 70-140 ns/call). Same class as #1640/#1741/#1753.
    """

    def setUp(self) -> None:
        from mypy.checkmember import (
            _set_native_checkmember_active,
            _set_native_checkmember_resolver,
        )
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_active = _set_native_checkmember_active
        self._set_resolver = _set_native_checkmember_resolver
        self.info = _make_typeinfo("mod.A")
        self.enum_info = _make_typeinfo("mod.E")
        self.enum_info.is_enum = True
        self._live_map = {"mod.A": self.info, "mod.E": self.enum_info}
        resolver = _type_kernel.build_native_resolver(list(self._live_map.values()), [])
        resolver.set_live_typeinfo_map(dict(self._live_map))
        set_wire_typeinfo_map(dict(self._live_map))
        self._set_resolver(resolver)
        self._set_active(True)

    def tearDown(self) -> None:
        from mypy.wirefixup import set_wire_typeinfo_map

        self._set_resolver(None)
        self._set_active(False)
        set_wire_typeinfo_map(None)

    def test_shims_helpers_and_tags_removed(self) -> None:
        from mypy import checkmember

        for gone in (
            "_rust_classify_analyze_var",
            "_rust_is_instance_var",
            "_apply_analyze_var_tag",
            "NATIVE_AV_SETTER",
            "NATIVE_AV_GETTER",
            "NATIVE_AV_PARTIAL",
            "NATIVE_AV_NOT_READY",
            "NATIVE_AV_ENUM_LITERAL",
            "NATIVE_AV_UNBOUND_ANY",
        ):
            assert not hasattr(checkmember, gone), f"{gone} should be gone"

    def test_no_rust_name_loaded_with_gate_on(self) -> None:
        from mypy.checkmember import analyze_var, is_instance_var

        for fn in (analyze_var, is_instance_var):
            loaded = [n for n in fn.__code__.co_names if "rust_" in n]
            assert loaded == [], f"{fn.__name__} still loads {loaded}"

    def test_is_instance_var_values_match_python(self) -> None:
        from mypy.checkmember import is_instance_var

        itype = Instance(self.info, [])
        plain = _make_var("plain", self.info, itype)
        classvar = _make_var("cv", self.info, itype, is_classvar=True)
        inferred = _make_var("inf", self.info, itype, is_inferred=True)
        shadowed = _make_var("shadow", self.info, itype)
        self.info.names["shadow"] = SymbolTableNode(MDEF, Var("shadow"))
        unregistered = _make_var("absent", self.info, itype, register=False)

        cases: list[tuple[Var, bool]] = [
            (plain, True),
            (classvar, False),
            (inferred, False),
            (shadowed, False),
            (unregistered, False),
        ]
        got = [is_instance_var(var) for var, _ in cases]
        assert got == [expected for _, expected in cases], f"unexpected values: {got}"

    def test_analyze_var_values_match_python(self) -> None:
        from mypy.checkmember import analyze_var

        def run(name: str, var: Var, info: TypeInfo, is_lvalue: bool = False) -> str:
            itype = Instance(info, [])
            return str(analyze_var(name, var, itype, _make_mx(itype, is_lvalue)))

        inner = Var("inner")
        getter = _make_var("g", self.info, Instance(self.info, []))
        partial = _make_var("p", self.info, PartialType(None, inner))
        unready = _make_var("u", self.info, None, is_ready=False)
        setter = CallableType([], [], [], AnyType(TypeOfAny.special_form), Instance(self.info, []))
        settable = _make_var(
            "s", self.info, Instance(self.info, []), setter_type=setter, is_settable_property=True
        )
        enum_member = Var("RED", Instance(self.enum_info, []))
        enum_member.info = self.enum_info
        enum_member.is_ready = True
        enum_member.has_explicit_value = True
        self.enum_info.names["RED"] = SymbolTableNode(MDEF, enum_member)

        self._set_active(False)
        expected = [
            run("g", getter, self.info),
            run("p", partial, self.info),
            run("u", unready, self.info),
            run("s", settable, self.info, True),
            run("RED", enum_member, self.enum_info),
        ]
        self._set_active(True)
        got = [
            run("g", getter, self.info),
            run("p", partial, self.info),
            run("u", unready, self.info),
            run("s", settable, self.info, True),
            run("RED", enum_member, self.enum_info),
        ]
        assert got == expected, f"gate-on values diverged: {got} != {expected}"
        assert got == ["mod.A", "Any", "Any", "def () -> Any", "Literal[mod.E.RED]?"], got

    def test_pyfunctions_stay_registered(self) -> None:
        assert _type_kernel is not None
        assert hasattr(_type_kernel, "rust_classify_analyze_var")
        assert hasattr(_type_kernel, "rust_is_instance_var")
