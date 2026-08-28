"""Unit tests for the per-module semanal wirefixup map install (issue #1115).

Covers `BuildManager._install_semal_wirefixup`: the hook that publishes a
module's TypeInfos/aliases into the wirefixup decode maps as soon as its
top-level semanal pass completes, so wire decodes during the remaining
semanal of the SCC can resolve cross-module references.
"""

from __future__ import annotations

import sys
import types as py_types
import unittest

from mypy import wirefixup
from mypy.build import BuildManager
from mypy.nodes import (
    GDEF,
    Block,
    ClassDef,
    MypyFile,
    SymbolTable,
    SymbolTableNode,
    TypeAlias,
    TypeInfo,
)
from mypy.test.helpers import Suite
from mypy.types import AnyType, TypeOfAny

_FAKE_KERNEL = py_types.ModuleType("type_kernel")


def _make_class(fullname: str, module_name: str) -> TypeInfo:
    defn = ClassDef(name=fullname.rsplit(".", 1)[-1], defs=Block([]))
    defn._fullname = fullname
    return TypeInfo(SymbolTable(), defn, module_name)


def _make_module(prefix: str) -> MypyFile:
    info = _make_class(f"{prefix}.C", prefix)
    inner = _make_class(f"{prefix}.C.Inner", prefix)
    info.names["Inner"] = SymbolTableNode(GDEF, inner)
    alias = TypeAlias(AnyType(TypeOfAny.implementation_artifact), f"{prefix}.A", prefix, 1, 0)
    mod = MypyFile(defs=[], imports=[])
    mod._fullname = prefix
    mod.names = SymbolTable({"C": SymbolTableNode(GDEF, info), "A": SymbolTableNode(GDEF, alias)})
    return mod


def _add_reachable_only_aliases(prefix: str, mod: MypyFile) -> None:
    """Attach the info-reachable aliases of issue #1133 to `mod`.

    A TypedDict/NamedTuple special alias on `C`, a class-level alias in
    `C.names`, and an alias on the nested class `C.Inner`: none of these
    is reachable from the module top-level `names` walk.
    """
    target = AnyType(TypeOfAny.implementation_artifact)
    cls = mod.names["C"].node
    assert isinstance(cls, TypeInfo)
    cls.special_alias = TypeAlias(
        AnyType(TypeOfAny.implementation_artifact), f"{prefix}.C.Named", prefix, 1, 0
    )
    cls.names["Alias"] = SymbolTableNode(
        GDEF, TypeAlias(target, f"{prefix}.C.Alias", prefix, 1, 0)
    )
    inner = cls.names["Inner"].node
    assert isinstance(inner, TypeInfo)
    inner.names["Alias"] = SymbolTableNode(
        GDEF,
        TypeAlias(AnyType(TypeOfAny.implementation_artifact), f"{prefix}.C.Inner.A", prefix, 1, 0),
    )


class WirefixupInstallSuite(Suite):
    def setUp(self) -> None:
        self._saved_info_map = wirefixup._wire_typeinfo_map
        self._saved_alias_map = wirefixup._wire_alias_map
        self._saved_kernel_mod = sys.modules.get("type_kernel")
        # Make the optional-accelerator availability check pass even when
        # the Rust .so is not installed in the test environment.
        sys.modules["type_kernel"] = _FAKE_KERNEL
        self.mgr = BuildManager.__new__(BuildManager)
        self.mgr.options = py_types.SimpleNamespace(  # type: ignore[assignment]
            native_type_kernel=True
        )
        self.mgr.modules = {}
        self.mgr._native_typeinfo_map = {}
        self.mgr._native_alias_map = {}
        self.mgr._native_snapshotted = set()

    def tearDown(self) -> None:
        wirefixup.set_wire_typeinfo_map(self._saved_info_map)
        wirefixup.set_wire_alias_map(self._saved_alias_map)
        if self._saved_kernel_mod is None:
            sys.modules.pop("type_kernel", None)
        else:
            sys.modules["type_kernel"] = self._saved_kernel_mod

    def test_populates_maps_and_installs_wirefixup(self) -> None:
        self.mgr.modules["mod"] = _make_module("mod")
        self.mgr._install_semal_wirefixup("mod")
        assert set(self.mgr._native_typeinfo_map) == {"mod.C", "mod.C.Inner"}
        assert set(self.mgr._native_alias_map) == {"mod.A"}
        assert wirefixup._wire_typeinfo_map is self.mgr._native_typeinfo_map
        assert wirefixup._wire_alias_map is self.mgr._native_alias_map

    def test_gate_off_leaves_maps_untouched(self) -> None:
        self.mgr.options.native_type_kernel = False
        self.mgr.modules["mod"] = _make_module("mod")
        self.mgr._install_semal_wirefixup("mod")
        assert self.mgr._native_typeinfo_map == {}
        assert self.mgr._native_alias_map == {}
        assert wirefixup._wire_typeinfo_map is not self.mgr._native_typeinfo_map

    def test_missing_module_is_a_noop(self) -> None:
        self.mgr._install_semal_wirefixup("nonexistent")
        assert self.mgr._native_typeinfo_map == {}
        assert self.mgr._native_alias_map == {}

    def test_second_call_grows_maps_in_place(self) -> None:
        # The map object identity must stay stable across installs so
        # wire-decode caches keyed on the map remain valid.
        self.mgr.modules["mod1"] = _make_module("mod1")
        self.mgr._install_semal_wirefixup("mod1")
        map_id = id(self.mgr._native_typeinfo_map)
        self.mgr.modules["mod2"] = _make_module("mod2")
        self.mgr._install_semal_wirefixup("mod2")
        assert id(self.mgr._native_typeinfo_map) == map_id
        assert "mod2.C" in self.mgr._native_typeinfo_map
        assert "mod1.C" in self.mgr._native_typeinfo_map

    def test_info_reachable_aliases_are_collected(self) -> None:
        # issue #1133: the alias snapshot must cover `info.special_alias`
        # and class-level aliases, not only module top-level ones.
        self.mgr.modules["mod"] = _make_module("mod")
        _add_reachable_only_aliases("mod", self.mgr.modules["mod"])
        self.mgr._install_semal_wirefixup("mod")
        assert set(self.mgr._native_alias_map) == {
            "mod.A",
            "mod.C.Named",
            "mod.C.Alias",
            "mod.C.Inner.A",
        }

    def test_collect_incremental_covers_info_reachable_aliases(self) -> None:
        # issue #1133: the per-SCC collection feeding the Rust resolver
        # must see the same info-reachable aliases.
        self.mgr._native_walked_modules = set()
        self.mgr.modules["mod"] = _make_module("mod")
        _add_reachable_only_aliases("mod", self.mgr.modules["mod"])
        infos, aliases = self.mgr._collect_incremental(["mod"])
        assert {info.fullname for info in infos} == {"mod.C", "mod.C.Inner"}
        assert {alias.fullname for alias in aliases} == {
            "mod.A",
            "mod.C.Named",
            "mod.C.Alias",
            "mod.C.Inner.A",
        }


if __name__ == "__main__":
    unittest.main()
