"""Native seam suites for the symtable area (split from testtypes.py, #1677)."""

from __future__ import annotations

from typing import Any
from unittest import skipUnless

from mypy.nodes import (
    GDEF,
    MDEF,
    Block,
    ClassDef,
    FuncDef,
    PlaceholderNode,
    SymbolTable,
    SymbolTableNode,
    TypeInfo,
    Var,
)
from mypy.test.helpers import Suite
from mypy.test.testtypes import _HAS_TYPE_KERNEL


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeSymtableMirrorSuite(Suite):
    """Unit tests for the G3.0a namespace entry funnel + capture scaffold (#1581).

    The store is capture-only: every assertion drives the
    `mypy.symtable_access.put_names_entry` funnel or the patched
    `SymbolTable` hooks and reads the Rust record back through the
    `rust_symtable_mirror_*` pyfunctions. The pinnings are the G3.0a
    contract: committed puts record exactly once, refusals leave no
    trace, namespace rebinds mint a fresh generation, ref-flag writes
    refresh adopted records, deletes drop records, identity shares the
    proxy/mirror namespace, capture failures never break the write, the
    gate-off path leaves the table untouched, and the Rust
    `remove_imported_names` deleter is a documented known bypass until
    the G3.0b reroute.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import symtables_mirror

        symtables_mirror.activate(audit=True)
        symtables_mirror.reset(clear_counts=True)
        self._k = kernel
        self._m = symtables_mirror

    def tearDown(self) -> None:
        self._m.reset(clear_counts=True)

    def _var(self, name: str, fullname: str) -> Any:
        var = Var(name)
        var._fullname = fullname
        return var

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {k: v - before.get(k, 0) for k, v in after.items() if v != before.get(k, 0)}

    def test_routed_put_records_entry(self) -> None:
        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        before = self._m.report()
        result = put_names_entry(table, "x", SymbolTableNode(GDEF, self._var("x", "mod.x")))
        assert result.committed is True
        assert result.generation > 0
        assert result.seq > 0
        assert table["x"].node is not None
        assert self._m.entry_count(table) == 1
        assert self._k.rust_symtable_mirror_entry_count(table) == 1
        record = self._m.lookup(table, "x")
        assert record is not None
        assert record["kind"] == GDEF
        assert record["node_fullname"] == "mod.x"
        assert record["generation"] == result.generation
        assert record["seq"] == result.seq
        delta = self._delta(before)
        assert delta.get("routed.put", 0) == 1
        assert delta.get("bypass.put", 0) == 0

    def test_placeholder_put_then_replacement_relinks(self) -> None:
        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        placeholder = PlaceholderNode("mod.x", Var("dummy"), 1)
        first_sym = SymbolTableNode(GDEF, placeholder)
        put_names_entry(table, "x", first_sym)
        first = self._m.lookup(table, "x")
        assert first is not None
        assert first["node_fullname"] == "mod.x"
        replacement = self._var("x", "mod.x")
        replacement_sym = SymbolTableNode(GDEF, replacement)
        put_names_entry(table, "x", replacement_sym)
        second = self._m.lookup(table, "x")
        assert second is not None
        assert second["seq"] > first["seq"]
        assert second["generation"] == first["generation"]
        # The replaced placeholder node is unlinked: refreshing it no
        # longer touches the record, the new node does.
        assert (
            self._k.rust_symtable_mirror_refresh_flags(
                first_sym, GDEF, "mod.x", True, False, False, False, False, None
            )
            is False
        )
        assert (
            self._k.rust_symtable_mirror_refresh_flags(
                replacement_sym, GDEF, "mod.x", True, False, False, False, False, None
            )
            is True
        )
        assert self._m.entry_count(table) == 1

    def test_refused_placeholder_new_leaves_no_trace(self) -> None:
        from mypy.semanal import is_valid_replacement
        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        existing = SymbolTableNode(GDEF, self._var("x", "mod.x"))
        put_names_entry(table, "x", existing)
        before_count = self._m.entry_count(table)
        before_record = self._m.lookup(table, "x")
        # A placeholder arriving over a real definition is not a valid
        # replacement: add_symbol_table_node returns False without
        # putting, so the shadow must keep the old record untouched.
        refused = SymbolTableNode(GDEF, PlaceholderNode("mod.x", Var("dummy"), 1))
        assert is_valid_replacement(existing, refused) is False
        assert self._m.entry_count(table) == before_count
        assert self._m.lookup(table, "x") == before_record
        assert self._m.lookup(table, "y") is None

    def test_construction_without_put_leaves_no_trace(self) -> None:
        table: SymbolTable = SymbolTable()
        # Merely constructing a node (the refusal path builds one but
        # never puts it) must not adopt anything.
        SymbolTableNode(GDEF, self._var("y", "mod.y"))
        assert self._m.entry_count(table) == 0
        assert self._m.lookup(table, "y") is None
        assert self._k.rust_symtable_mirror_total_entry_count() == 0
        # Unknown shapes defer safely: a non-SymbolTableNode value still
        # writes through the raw dict but records nothing.
        from mypy.symtable_access import put_names_entry

        result = put_names_entry(table, "junk", object())
        assert result.committed is True
        assert result.generation == 0
        assert "junk" in table
        assert self._m.lookup(table, "junk") is None
        assert self._m.entry_count(table) == 0

    def test_namespace_rebind_mints_fresh_generation(self) -> None:
        from mypy.symtable_access import put_names_entry

        old: SymbolTable = SymbolTable()
        first = put_names_entry(old, "x", SymbolTableNode(GDEF, self._var("x", "mod.x")))
        new: SymbolTable = SymbolTable()
        second = put_names_entry(new, "x", SymbolTableNode(GDEF, self._var("x", "mod.x")))
        assert second.generation != first.generation
        assert self._m.generation(old) == first.generation
        assert self._m.generation(new) == second.generation
        # Per-name records never merge across generations.
        assert self._m.entry_count(old) == 1
        assert self._m.entry_count(new) == 1
        assert self._k.rust_symtable_mirror_total_entry_count() == 2

    def test_ref_flags_refresh_on_setattr(self) -> None:
        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        sym = SymbolTableNode(GDEF, self._var("x", "mod.x"))
        put_names_entry(table, "x", sym)
        # Constructor-default writes on a never-adopted node stay out of
        # the store; the first put adopts with the post-write snapshot.
        fresh = SymbolTableNode(GDEF, self._var("u", "mod.u"))
        fresh.implicit = True
        assert self._k.rust_symtable_mirror_total_entry_count() == 1
        sym.implicit = True
        sym.module_public = False
        sym.no_serialize = True
        record = self._m.lookup(table, "x")
        assert record is not None
        assert record["implicit"] is True
        assert record["module_public"] is False
        assert record["no_serialize"] is True
        sym.kind = MDEF
        refreshed = self._m.lookup(table, "x")
        assert refreshed is not None
        assert refreshed["kind"] == MDEF

    def test_cross_ref_plugin_and_hidden_flags_captured(self) -> None:
        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        sym = SymbolTableNode(
            GDEF, self._var("x", "other.x"), plugin_generated=True, no_serialize=True
        )
        sym.module_hidden = True
        sym.cross_ref = "other.x"
        put_names_entry(table, "x", sym)
        record = self._m.lookup(table, "x")
        assert record is not None
        assert record["plugin_generated"] is True
        assert record["no_serialize"] is True
        assert record["module_hidden"] is True
        assert record["cross_ref"] == "other.x"
        assert record["node_fullname"] == "other.x"

    def test_delete_via_delitem_and_pop(self) -> None:
        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        put_names_entry(table, "a", SymbolTableNode(GDEF, self._var("a", "mod.a")))
        put_names_entry(table, "b", SymbolTableNode(GDEF, self._var("b", "mod.b")))
        assert self._m.entry_count(table) == 2
        del table["a"]
        assert "a" not in table
        assert self._m.lookup(table, "a") is None
        assert self._m.entry_count(table) == 1
        table.pop("b")
        assert "b" not in table
        assert self._m.lookup(table, "b") is None
        assert self._m.entry_count(table) == 0

    def test_identity_shares_namespace(self) -> None:
        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        sym = SymbolTableNode(GDEF, self._var("x", "mod.x"))
        put_names_entry(table, "x", sym)
        handle = self._m.handle_of(table)
        assert handle is not None
        # One identity namespace: the symtable store and the type mirror
        # answer the same handle for the same object.
        assert self._k.rust_symtable_mirror_handle_of(table) == handle
        assert self._k.rust_mirror_handle_of(table) == handle
        node_handle = self._m._NODE_HANDLES.get(id(sym))
        assert node_handle is not None
        assert self._k.rust_symtable_mirror_handle_of(sym) == node_handle

    def test_failure_safety_write_survives_kernel_failure(self) -> None:
        import type_kernel as kernel

        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        original = kernel.rust_symtable_mirror_put

        def boom(*args: Any, **kwargs: Any) -> Any:
            raise RuntimeError("kernel down")

        kernel.rust_symtable_mirror_put = boom
        try:
            before = self._m.report()
            sym = SymbolTableNode(GDEF, self._var("x", "mod.x"))
            result = put_names_entry(table, "x", sym)
            # The dict write is normative and never fails with the
            # capture; the receipt degrades to the plain one.
            assert table["x"] is sym
            assert result.generation == 0
            assert self._m.lookup(table, "x") is None
            assert self._delta(before).get("capture_fail.put", 0) >= 1
        finally:
            kernel.rust_symtable_mirror_put = original

    def test_gate_off_leaves_table_untouched(self) -> None:
        from mypy.symtable_access import put_names_entry

        self._m._active = False
        try:
            table: SymbolTable = SymbolTable()
            sym = SymbolTableNode(GDEF, self._var("x", "mod.x"))
            result = put_names_entry(table, "x", sym)
            assert table["x"] is sym
            assert result.generation == 0
            assert id(sym) not in self._m._NODE_HANDLES
            assert self._k.rust_symtable_mirror_total_entry_count() == 0
        finally:
            self._m._active = True

    def test_direct_write_counts_bypass_and_rust_remove_is_known_bypass(self) -> None:
        # A direct `table[name] = ...` write bypasses put_names_entry: the
        # class patch still captures it (the shadow stays complete) but
        # counts it as `bypass.put`, so funnel coverage stays falsifiable.
        table: SymbolTable = SymbolTable()
        before = self._m.report()
        table["direct"] = SymbolTableNode(GDEF, self._var("direct", "mod.direct"))
        assert self._m.lookup(table, "direct") is not None
        delta = self._delta(before)
        assert delta.get("bypass.put", 0) >= 1
        # G3.0b: the Rust remove-imported path now calls
        # symtable_mirror::delete before PyDict::del_item, so the
        # shadow record is cleaned up alongside the dict entry.
        import type_kernel as kernel

        from mypy.symtable_access import put_names_entry

        table2: SymbolTable = SymbolTable()
        put_names_entry(table2, "local", SymbolTableNode(GDEF, self._var("local", "mod.local")))
        put_names_entry(
            table2, "imported", SymbolTableNode(GDEF, self._var("imported", "other.imported"))
        )
        assert self._m.entry_count(table2) == 2
        kernel.rust_remove_imported_names_from_symtable(table2, "mod")
        assert "imported" not in table2
        assert "local" in table2
        # G3.0b: shadow record is now cleaned up by the Rust delete.
        assert self._m.lookup(table2, "imported") is None
        assert self._m.entry_count(table2) == 1

    # ---- G3.0c: TypeInfo meta-field capture ----

    def _make_info(self, fullname: str = "mod.Cls") -> Any:
        from mypy.nodes import Block, ClassDef, TypeInfo

        info = TypeInfo(SymbolTable(), ClassDef(fullname, Block([])), "")
        info._fullname = fullname
        return info

    def test_meta_bases_write_captured(self) -> None:
        info = self._make_info()
        info.bases = []
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["bases_count"] == 0
        info.bases = [info]  # self-referencing for the test
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["bases_count"] == 1

    def test_meta_mro_write_captured(self) -> None:
        info = self._make_info()
        info.mro = [info]
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["mro_count"] == 1

    def test_meta_metaclass_type_captured(self) -> None:
        info = self._make_info()
        # _make_info already wrote _fullname (a meta field), so a record
        # exists; writing metaclass_type=None updates it with None.
        info.metaclass_type = None
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["metaclass_fullname"] is None

    def test_meta_fullname_captured(self) -> None:
        info = self._make_info("mod.Old")
        info._fullname = "mod.New"
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["fullname"] == "mod.New"

    def test_meta_names_rebind_captures_fresh_handle(self) -> None:
        info = self._make_info()
        old_names = info.names
        info.names = old_names  # same object, still captures
        record1 = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record1 is not None
        old_names_handle = record1["names_handle"]
        new_names: SymbolTable = SymbolTable()
        info.names = new_names
        record2 = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record2 is not None
        assert record2["names_handle"] != old_names_handle

    def test_meta_non_meta_field_passes_through(self) -> None:
        info = self._make_info()
        before = self._k.rust_symtable_mirror_meta_entry_count()
        info.is_abstract = True  # not in _META_FIELDS
        assert self._k.rust_symtable_mirror_meta_entry_count() == before

    def test_meta_accessor_set_bases_mro(self) -> None:
        from mypy.symtable_access import set_bases_mro

        info = self._make_info()
        set_bases_mro(info, [info], [info, info])
        assert info.bases == [info]
        assert info.mro == [info, info]
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["bases_count"] == 1
        assert record["mro_count"] == 2

    def test_meta_accessor_set_meta(self) -> None:
        from mypy.symtable_access import set_meta

        info = self._make_info()
        set_meta(info, None)
        assert info.metaclass_type is None
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["metaclass_fullname"] is None

    def test_meta_accessor_set_info_fullname(self) -> None:
        from mypy.symtable_access import set_info_fullname

        info = self._make_info("mod.Old")
        set_info_fullname(info, "mod.New")
        assert info._fullname == "mod.New"
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["fullname"] == "mod.New"

    def test_meta_accessor_rebind_names_table(self) -> None:
        from mypy.symtable_access import rebind_names_table

        info = self._make_info()
        new_names: SymbolTable = SymbolTable()
        rebind_names_table(info, new_names)
        assert info.names is new_names
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["names_handle"] == self._k.rust_symtable_mirror_handle_of(new_names)

    def test_meta_delete_removes_record(self) -> None:
        info = self._make_info()
        info.bases = []
        assert self._k.rust_symtable_mirror_meta_lookup(info) is not None
        assert self._k.rust_symtable_mirror_meta_delete(info) is True
        assert self._k.rust_symtable_mirror_meta_lookup(info) is None
        assert self._k.rust_symtable_mirror_meta_delete(info) is False

    def test_meta_gate_off_leaves_no_trace(self) -> None:
        self._m._active = False
        try:
            info = self._make_info()
            info.bases = []
            assert self._k.rust_symtable_mirror_meta_entry_count() == 0
        finally:
            self._m._active = True

    def test_astmerge_inplace_mro_mutation_recaptured(self) -> None:
        # G3.0d: info.mro[i] = ... is a list item assignment that bypasses
        # the TypeInfo.__setattr__ patch. The astmerge process_type_info
        # path re-captures via _capture_meta after the in-place mutations.

        info = self._make_info("mod.Cls")
        base = self._make_info("mod.Base")
        info.mro = [info, base]
        record_before = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record_before is not None
        assert record_before["mro_count"] == 2
        # Simulate astmerge in-place mro mutation (replace one entry)
        replacement = self._make_info("mod.Base2")
        info.mro[1] = replacement
        # The shadow is stale until _capture_meta re-runs
        from mypy import symtables_mirror

        symtables_mirror._capture_meta(info, "astmerge_fixup")
        record_after = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record_after is not None
        assert record_after["mro_count"] == 2
        assert record_after["seq"] > record_before["seq"]

    def test_astmerge_replace_object_state_typeinfo_recaptured(self) -> None:
        # G3.0d: replace_object_state(new, old) copies state via setattr,
        # which triggers _typeinfo_setattr -> _capture_meta on the surviving
        # `new` identity. The shadow record must appear on `new`, not `old`.
        from mypy.nodes import Block, ClassDef, TypeInfo
        from mypy.util import replace_object_state

        old = self._make_info("mod.Old")
        old.bases = [old]
        old.is_final = True
        old_record = self._k.rust_symtable_mirror_meta_lookup(old)
        assert old_record is not None
        assert old_record["bases_count"] == 1
        # Build a fresh TypeInfo with same class (required by
        # replace_object_state).
        new = TypeInfo(SymbolTable(), ClassDef("mod.New", Block([])), "")
        # replace_object_state copies old's state onto new via setattr.
        replace_object_state(new, old, skip_slots=("special_alias",))
        new_record = self._k.rust_symtable_mirror_meta_lookup(new)
        assert new_record is not None
        assert new_record["bases_count"] == 1
        assert new_record["fullname"] == "mod.Old"
        assert new_record["seq"] > old_record["seq"]

    def test_astmerge_replace_nodes_in_symbol_table_refreshes_flags(self) -> None:
        # G3.0d: node._node = new in replace_nodes_in_symbol_table writes to
        # a _FLAG_FIELDS slot, triggering _symtable_node_setattr ->
        # _refresh_flags on the SymbolTableNode.
        from mypy.nodes import GDEF, SymbolTable, SymbolTableNode, Var

        table = SymbolTable()
        old_node = Var("x")
        old_node._fullname = "mod.x"
        entry = SymbolTableNode(GDEF, old_node)
        table["x"] = entry
        record_before = self._m.lookup(table, "x")
        assert record_before is not None
        assert record_before["node_fullname"] == "mod.x"
        # Simulate the astmerge replacement: swap the node identity.
        new_node = Var("y")
        new_node._fullname = "mod.y"
        entry._node = new_node
        record_after = self._m.lookup(table, "x")
        assert record_after is not None
        assert record_after["node_fullname"] == "mod.y"


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeSymtableMetaExtraSuite(Suite):
    """G3.0c: extended TypeInfo meta fields (bool flags, type_vars
    count, self_type/declared_metaclass fullnames) captured through
    the ``meta_put_field`` FFI."""

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import symtables_mirror

        symtables_mirror.activate(audit=True)
        symtables_mirror.reset(clear_counts=True)
        self._k = kernel
        self._m = symtables_mirror

    def tearDown(self) -> None:
        self._m.reset(clear_counts=True)

    def _make_info(self, fullname: str = "mod.Cls") -> Any:
        from mypy.nodes import Block, ClassDef, TypeInfo

        info = TypeInfo(SymbolTable(), ClassDef(fullname, Block([])), "")
        info._fullname = fullname
        return info

    def test_is_final_captured(self) -> None:
        info = self._make_info()
        info.is_final = True
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["extra"].get("is_final") == "true"
        info.is_final = False
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["extra"].get("is_final") == "false"

    def test_is_protocol_captured(self) -> None:
        info = self._make_info()
        info.is_protocol = True
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["extra"].get("is_protocol") == "true"

    def test_is_enum_captured(self) -> None:
        info = self._make_info()
        info.is_enum = True
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["extra"].get("is_enum") == "true"

    def test_type_vars_count_captured(self) -> None:
        info = self._make_info()
        info.type_vars = 3
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["extra"].get("type_vars") == "3"

    def test_fallback_to_any_captured(self) -> None:
        info = self._make_info()
        info.fallback_to_any = True
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["extra"].get("fallback_to_any") == "true"

    def test_core_write_refreshes_extras(self) -> None:
        info = self._make_info()
        info.is_final = True
        # A core field write should also refresh the extended snapshot.
        info.bases = []
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["extra"].get("is_final") == "true"

    def test_baseline_write_on_adopted_does_not_skip(self) -> None:
        # TypeInfo.__init__ writes _fullname and names (core meta fields),
        # so the TypeInfo is adopted at construction. A baseline-value
        # write to an extra field refreshes rather than being skipped.
        info = self._make_info()
        before = self._m.report()
        info.is_final = False
        delta = self._delta(before)
        # Should NOT be skipped — the TypeInfo is already adopted.
        assert delta.get("meta_baseline_skip.is_final", 0) == 0
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        assert record["extra"].get("is_final") == "false"

    def _delta(self, before: dict[str, int]) -> dict[str, int]:
        after = self._m.report()
        return {k: v - before.get(k, 0) for k, v in after.items() if v != before.get(k, 0)}

    def test_reset_clears_meta_adopted(self) -> None:
        info = self._make_info()
        info.is_final = True
        assert id(info) in self._m._META_ADOPTED
        self._m.reset()
        assert id(info) not in self._m._META_ADOPTED

    def test_multiple_extras_captured(self) -> None:
        info = self._make_info()
        info.is_final = True
        info.is_protocol = True
        info.is_enum = True
        record = self._k.rust_symtable_mirror_meta_lookup(info)
        assert record is not None
        extra = record["extra"]
        assert extra.get("is_final") == "true"
        assert extra.get("is_protocol") == "true"
        assert extra.get("is_enum") == "true"

    def test_gate_off_no_extra_captured(self) -> None:
        self._m._active = False
        try:
            info = self._make_info()
            before = self._k.rust_symtable_mirror_meta_entry_count()
            info.is_final = True
            assert self._k.rust_symtable_mirror_meta_entry_count() == before
        finally:
            self._m._active = True


@skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeSymtableReadFlipSuite(Suite):
    """Issue #1670 (G3.1): symbol-table read flip for the astdiff snapshot.

    The G3.0a shadow is capture-only; G3.1 makes the first *read* come from
    it. `astdiff.snapshot_symbol_table` serves the namespace from Rust
    storage when the store mirrors the table exactly, and every other table
    falls back to the live-table walk. These pins hold the two halves of
    that contract: the store is served (and the served snapshot is
    byte-identical to the flip-off one, order included), and a table the
    capture could not see (a C-level `dict` write, a stale record, a never
    adopted namespace) defers instead of answering. The verify mode is the
    differential that would catch an aliased record the length gate cannot
    see, and it raises rather than serving the wrong snapshot.
    """

    def setUp(self) -> None:
        import type_kernel as kernel

        from mypy import symtables_mirror
        from mypy.server import astdiff

        self._k = kernel
        self._m = symtables_mirror
        self._astdiff = astdiff
        self._prev_active = astdiff._native_astdiff_active
        self._prev_mode = symtables_mirror.read_flip_mode()
        astdiff._set_native_astdiff_active(True)
        symtables_mirror.activate(audit=True)
        symtables_mirror.set_read_flip(0)
        symtables_mirror.reset(clear_counts=True)
        self._astdiff._read_flip_stats.clear()

    def tearDown(self) -> None:
        self._m.set_read_flip(self._prev_mode)
        self._m.reset(clear_counts=True)
        self._astdiff._read_flip_stats.clear()
        self._astdiff._set_native_astdiff_active(self._prev_active)

    # ---- helpers ----

    def _var(self, name: str, fullname: str) -> Var:
        var = Var(name)
        var._fullname = fullname
        return var

    def _sym(self, name: str, fullname: str) -> SymbolTableNode:
        return SymbolTableNode(GDEF, self._var(name, fullname))

    def _table(self, fullnames: dict[str, str]) -> SymbolTable:
        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        for name, fullname in fullnames.items():
            put_names_entry(table, name, self._sym(name, fullname))
        return table

    def _snapshot(self, table: SymbolTable, mode: int) -> Any:
        self._m.set_read_flip(mode)
        try:
            return self._astdiff.snapshot_symbol_table("mod", table)
        finally:
            self._m.set_read_flip(0)

    def _stats(self) -> dict[str, int]:
        return self._astdiff.read_flip_stats()

    def _record(self, table: SymbolTable, name: str, fullname: str) -> None:
        """Write one store record without touching the live table.

        The FFI is the only way to model a record the `dict` does not back
        (the accessor writes both); it is how the drift pins below
        construct a shadow the live namespace disagrees with.
        """
        self._k.rust_symtable_mirror_put(
            table,
            name,
            self._sym(name, fullname),
            GDEF,
            fullname,
            True,
            False,
            False,
            False,
            False,
            None,
        )

    def _suppressed_puts(self) -> int:
        """Un-routed namespace writes the class patch saw (audit counter)."""
        return self._m.report().get("bypass.put", 0)

    # ---- served: the store answers the read ----

    def test_flip_serves_shadow_and_matches_live(self) -> None:
        table = self._table({"a": "mod.a", "b": "mod.b", "c": "mod.c"})
        live = self._snapshot(table, 0)
        assert live == {
            "a": ("Var", ("mod.a", GDEF, True), ("<not set>",), False),
            "b": ("Var", ("mod.b", GDEF, True), ("<not set>",), False),
            "c": ("Var", ("mod.c", GDEF, True), ("<not set>",), False),
        }
        # The flip-off snapshot never consults the store.
        assert self._stats() == {}
        flipped = self._snapshot(table, 1)
        assert flipped == live
        assert list(flipped) == list(table)
        assert self._stats() == {"calls": 1, "served": 1}
        # The shadow-consistency invariants the G3 brief pins, flip on.
        assert self._m.entry_count(table) == len(table)
        assert self._suppressed_puts() == 0
        counters = self._m.flip_report()
        assert counters["tables_looked"] == 1
        assert counters["tables_mirrored"] == 1
        assert counters["entries_mirrored"] == 3

    def test_flip_off_never_touches_the_store(self) -> None:
        table = self._table({"a": "mod.a"})
        live = self._snapshot(table, 0)
        assert list(live) == ["a"]
        assert self._stats() == {}
        assert self._m.flip_report()["tables_looked"] == 0

    def test_bypass_captured_write_is_served(self) -> None:
        # `table[name] = ...` is not routed through `put_names_entry`, but
        # the class patch still captures it: the shadow stays complete, so
        # the entry is `bypass.put` *and* servable.
        table: SymbolTable = SymbolTable()
        table["a"] = self._sym("a", "mod.a")
        assert self._suppressed_puts() == 1
        assert self._m.entry_count(table) == len(table) == 1
        flipped = self._snapshot(table, 1)
        assert flipped == self._snapshot(table, 0)
        assert list(flipped) == ["a"]
        assert self._stats() == {"calls": 1, "served": 1}

    def test_replace_keeps_position_and_reinsert_moves_last(self) -> None:
        from mypy.symtable_access import delete_names_entry

        table = self._table({"a": "mod.a", "b": "mod.b", "c": "mod.c"})
        # A replace keeps the namespace position in both models.
        from mypy.symtable_access import put_names_entry

        put_names_entry(table, "b", self._sym("b", "mod.b2"))
        flipped = self._snapshot(table, 1)
        assert list(flipped) == ["a", "b", "c"] == list(table)
        assert flipped == self._snapshot(table, 0)
        # A delete releases the ordinal; the re-insert lands last.
        delete_names_entry(table, "a")
        put_names_entry(table, "a", self._sym("a", "mod.a"))
        flipped = self._snapshot(table, 1)
        assert list(flipped) == ["b", "c", "a"] == list(table)
        assert flipped == self._snapshot(table, 0)
        assert self._stats() == {"calls": 2, "served": 2}

    def test_nested_class_namespace_is_served_too(self) -> None:
        from mypy.symtable_access import put_names_entry

        names: SymbolTable = SymbolTable()
        body = FuncDef("m", [], Block([]), None)
        body._fullname = "mod.C.m"
        put_names_entry(names, "m", SymbolTableNode(MDEF, body))
        cdef = ClassDef("C", Block([]))
        cdef.fullname = "mod.C"
        info = TypeInfo(names, cdef, "mod")
        cdef.info = info
        outer: SymbolTable = SymbolTable()
        put_names_entry(outer, "C", SymbolTableNode(GDEF, info))
        flipped = self._snapshot(outer, 1)
        assert flipped == self._snapshot(outer, 0)
        # Both the module and the class namespace came from the store.
        counters = self._m.flip_report()
        assert counters["tables_mirrored"] == 2
        assert counters["entries_mirrored"] == 2
        assert self._stats() == {"calls": 1, "served": 1}

    # ---- deferred: the store cannot answer, the live walk does ----

    def test_unmirrored_c_level_write_defers(self) -> None:
        table = self._table({"a": "mod.a"})
        # `dict.__setitem__` is invisible to the class patch: the live
        # table gains an entry the store never saw.
        dict.__setitem__(table, "b", self._sym("b", "mod.b"))
        flipped = self._snapshot(table, 1)
        assert flipped == self._snapshot(table, 0)
        assert list(flipped) == ["a", "b"]
        assert self._stats() == {"calls": 1, "deferred": 1}
        assert self._m.flip_report()["defer_len_short"] == 1

    def test_stale_record_defers(self) -> None:
        table = self._table({"a": "mod.a"})
        # A store record with no live entry of that name: length drift the
        # other way round (an uncaptured delete would do this in the wild).
        self._record(table, "ghost", "mod.ghost")
        flipped = self._snapshot(table, 1)
        assert flipped == self._snapshot(table, 0)
        assert list(flipped) == ["a"]
        assert self._stats() == {"calls": 1, "deferred": 1}
        assert self._m.flip_report()["defer_len_long"] == 1

    def test_never_adopted_table_defers(self) -> None:
        table: SymbolTable = SymbolTable()
        dict.__setitem__(table, "a", self._sym("a", "mod.a"))
        assert self._k.rust_symtable_mirror_handle_of(table) is None
        assert len(table) == 1
        flipped = self._snapshot(table, 1)
        assert flipped == self._snapshot(table, 0)
        assert list(flipped) == ["a"]
        assert self._stats() == {"calls": 1, "deferred": 1}
        assert self._m.flip_report()["defer_no_handle"] == 1

    def test_inherited_namespace_defers(self) -> None:
        # A namespace holding a key when the store first saw it cannot be
        # served: that position predates the store's ordinals (aststrip
        # survivors, loaded cache). Counts match; only the rule catches it.
        from mypy.symtable_access import put_names_entry

        table: SymbolTable = SymbolTable()
        dict.__setitem__(table, "a", self._sym("a", "mod.a"))
        put_names_entry(table, "b", self._sym("b", "mod.b"))
        put_names_entry(table, "a", self._sym("a", "mod.a"))
        assert self._m.entry_count(table) == len(table) == 2
        flipped = self._snapshot(table, 1)
        assert flipped == self._snapshot(table, 0)
        assert list(flipped) == list(table) == ["a", "b"]
        assert self._stats() == {"calls": 1, "deferred": 1}
        assert self._m.flip_report()["defer_inherited"] == 1

    # ---- verify mode: the differential ----

    def test_verify_mode_matches_and_counts(self) -> None:
        table = self._table({"a": "mod.a", "b": "mod.b"})
        flipped = self._snapshot(table, 2)
        assert flipped == self._snapshot(table, 0)
        assert self._stats() == {"calls": 1, "served": 1, "verify.ok": 1}

    def test_verify_mode_raises_on_aliased_record(self) -> None:
        table = self._table({"a": "mod.a", "b": "mod.b"})
        # Same length, same names, different bound node: only the
        # differential can see this record, and it must never be served.
        self._record(table, "b", "mod.OTHER")
        with self.assertRaises(RuntimeError) as ctx:
            self._snapshot(table, 2)
        assert "diverged" in str(ctx.exception)
        assert self._stats() == {"calls": 1, "served": 1, "verify.mismatch": 1}

    def test_verify_mode_raises_on_order_divergence(self) -> None:
        table = self._table({"a": "mod.a", "b": "mod.b"})
        symbol = table["a"]
        # An uncaptured delete + re-insert moves the name to the end of the
        # live namespace while the store keeps its original ordinal: same
        # values, same length, different order.
        dict.__delitem__(table, "a")
        dict.__setitem__(table, "a", symbol)
        assert list(table) == ["b", "a"]
        with self.assertRaises(RuntimeError) as ctx:
            self._snapshot(table, 2)
        assert "order" in str(ctx.exception)
        assert self._stats() == {"calls": 1, "served": 1, "verify.order_mismatch": 1}

    def test_read_flip_mode_validation(self) -> None:
        with self.assertRaises(ValueError):
            self._m.set_read_flip(3)
