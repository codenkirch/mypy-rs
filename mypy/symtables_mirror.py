"""Phase G3.0a namespace dual-write capture scaffold (issue #1581).

Python stays canonical. With the symtable-mirror gate on, committed
symbol-table writes are recorded into Rust storage behind the type
kernel's `rust_symtable_mirror_*` pyfunctions: one record per
``(owner table handle, name)`` carrying the table generation, a monotonic
capture seq, the referenced node's fullname and the symbol ref flags.

Design notes (see docs/plans/2026-09-11-g3-symbol-table-brief.md):
- `mypy.symtable_access.put_names_entry` is the normative committed-put
  path and writes through `dict.__setitem__` directly, so it never
  reaches the patched `SymbolTable.__setitem__`. Every write the patch
  sees is therefore not routed through the accessor: it is captured (the
  shadow stays complete) and counted as ``bypass.put``, so the accessor
  coverage of a corpus is falsifiable.
- The patches mirror the G1.0a node hook: `__setitem__` (put capture plus
  bypass count), `__delitem__`/`pop` (delete capture) and
  `SymbolTableNode.__setattr__` (ref-flag refresh for adopted nodes).
  C-level dict paths (`SymbolTable.read`/`copy` constructors,
  `dict.update/clear`, the Rust `PyDict::del_item` in
  `rust_remove_imported_names_from_symtable`) bypass Python overrides;
  the last one is pinned as a documented known bypass until G3.0b
  reroutes it.
- Lazy adoption: `SymbolTableNode` constructor writes happen before any
  put, so the node patch skips nodes with no record and the first put
  captures the post-construction flag snapshot. A later flag write
  refreshes every record referencing that node.
- Namespace rebinds (`owner.names = SymbolTable()`) mint a fresh owner
  identity, hence a fresh table generation: per-name records cannot
  merge across generations.
- Strong pins: the Rust store pins each captured table and node until
  reset, so the `id()`-keyed handle maps cannot go stale and no Python
  pin dict is needed. `reset` drops entries and pins but does NOT touch
  `identity`: `rust_mirror_reset` remains the single owner of
  `identity::reset` (the proxy contract).
- Capture failures never propagate into the write path: the write has
  already been applied, and a failure only increments an audit counter.
"""

from __future__ import annotations

import os
import sys
from typing import Any, Final

from mypy.nodes import SymbolTable, SymbolTableNode, TypeInfo

_kernel_mod: Any = None
_active = False
_audit_mode = False
# Re-entrancy guard: a capture reads live fields, never writes them, but
# the guard keeps a future field-property hook from recursing.
_in_capture = False
_ORIG_SETITEM: Any = dict.__setitem__
_ORIG_DELITEM: Any = dict.__delitem__
_ORIG_POP: Any = dict.pop
_ORIG_NODE_SETATTR: Any = object.__setattr__
# id(table) -> owner handle for every adopted namespace.
_TABLE_HANDLES: dict[int, int] = {}
# id(node) -> node handle; presence marks adoption by a recorded put. The
# Rust store pins the node, so the id() keys cannot be recycled.
_NODE_HANDLES: dict[int, int] = {}
_audit: dict[str, int] = {}
# Audit-mode per-callsite counters: "<prefix>@<file>:<line>" -> count.
_site_counts: dict[str, int] = {}

# Ref-flag slots that refresh an adopted node's records. `_node` is the
# raw slot behind the `node` property (never read through the property:
# a capture must not trigger cross-ref fixup).
_FLAG_FIELDS: Final[frozenset[str]] = frozenset(
    {
        "kind",
        "module_public",
        "module_hidden",
        "implicit",
        "plugin_generated",
        "no_serialize",
        "cross_ref",
        "_node",
    }
)

# G3.0c: TypeInfo meta fields captured by `TypeInfo.__setattr__`.
# `names` is a namespace rebind; others carry list length or fullname.
# All other fields pass through untouched.
_META_FIELDS: Final[frozenset[str]] = frozenset(
    {"bases", "mro", "metaclass_type", "_fullname", "names"}
)


def _count(key: str, n: int = 1) -> None:
    if not _audit_mode:
        return
    _audit[key] = _audit.get(key, 0) + n


def _count_site(prefix: str) -> None:
    """Attribute one capture to its external callsite (audit mode only)."""
    if not _audit_mode:
        return
    frame = sys._getframe(2)
    while frame is not None:
        filename = frame.f_code.co_filename
        if "symtables_mirror" not in filename and "symtable_access" not in filename:
            site = f"{os.path.basename(filename)}:{frame.f_lineno}"
            key = f"{prefix}@{site}"
            _site_counts[key] = _site_counts.get(key, 0) + 1
            return
        frame = frame.f_back


def _node_fullname(symbol: Any) -> str | None:
    """Fullname of the raw `_node` slot, or None when unreadable/absent."""
    try:
        target = symbol._node
    except Exception:
        return None
    if target is None:
        return None
    try:
        fullname = target.fullname
    except Exception:
        return None
    return fullname if isinstance(fullname, str) else None


def _cross_ref(symbol: Any) -> str | None:
    try:
        value = symbol.cross_ref
    except Exception:
        return None
    return value if isinstance(value, str) else None


def _capture(table: Any, name: str, symbol: Any, prefix: str) -> tuple[int, int, int, int] | None:
    """Store one post-write record; None when the capture failed."""
    global _in_capture
    _in_capture = True
    try:
        result = _kernel_mod.rust_symtable_mirror_put(
            table,
            name,
            symbol,
            int(symbol.kind),
            _node_fullname(symbol),
            bool(symbol.module_public),
            bool(symbol.module_hidden),
            bool(symbol.implicit),
            bool(symbol.plugin_generated),
            bool(symbol.no_serialize),
            _cross_ref(symbol),
        )
        owner_handle, node_handle, seq, generation = result
        _TABLE_HANDLES[id(table)] = owner_handle
        _NODE_HANDLES[id(symbol)] = node_handle
        _count(prefix)
        _count_site(prefix)
        return (owner_handle, node_handle, seq, generation)
    except Exception:
        _count("capture_fail.put")
        return None
    finally:
        _in_capture = False


def put(table: Any, name: str, symbol: Any) -> tuple[int, int, int, int] | None:
    """Normative write path: raw dict put + capture.

    Called only by `mypy.symtable_access.put_names_entry`; when the gate
    is off the raw write still happens and None is returned. The raw
    `dict.__setitem__` deliberately does not reach the class patch, so
    every patched `__setitem__` is a bypassing write by construction.
    """
    if not _active:
        _ORIG_SETITEM(table, name, symbol)
        return None
    _ORIG_SETITEM(table, name, symbol)
    if not isinstance(symbol, SymbolTableNode):
        _count("capture_fail.not_symbol")
        return None
    return _capture(table, name, symbol, "routed.put")


def _delete(table: Any, name: Any) -> None:
    try:
        if _kernel_mod.rust_symtable_mirror_delete(table, name):
            _count("delete.recorded")
    except Exception:
        _count("capture_fail.delete")


def _refresh_flags(symbol: Any, field: str) -> None:
    global _in_capture
    _in_capture = True
    try:
        _kernel_mod.rust_symtable_mirror_refresh_flags(
            symbol,
            int(symbol.kind),
            _node_fullname(symbol),
            bool(symbol.module_public),
            bool(symbol.module_hidden),
            bool(symbol.implicit),
            bool(symbol.plugin_generated),
            bool(symbol.no_serialize),
            _cross_ref(symbol),
        )
        _count("refresh_flags." + field)
    except Exception:
        _count("capture_fail.flags")
    finally:
        _in_capture = False


def _symtable_setitem(self: Any, key: Any, value: Any) -> None:
    # Apply the write first: the capture records the post-write state and
    # any exception from the dict write propagates unchanged.
    _ORIG_SETITEM(self, key, value)
    if not _active or _in_capture:
        return
    _count("bypass.put")
    if isinstance(value, SymbolTableNode):
        _capture(self, key, value, "bypass.put")
    else:
        _count("capture_fail.not_symbol")


def _symtable_delitem(self: Any, key: Any) -> None:
    _ORIG_DELITEM(self, key)
    if not _active or _in_capture:
        return
    _delete(self, key)
    _count("delete.delitem")


def _symtable_pop(self: Any, key: Any, *args: Any) -> Any:
    result = _ORIG_POP(self, key, *args)
    if _active and not _in_capture:
        _delete(self, key)
        _count("delete.pop")
    return result


def _symtable_node_setattr(self: Any, name: str, value: Any) -> None:
    _ORIG_NODE_SETATTR(self, name, value)
    if not _active or _in_capture:
        return
    if name not in _FLAG_FIELDS:
        return
    if id(self) not in _NODE_HANDLES:
        # Constructor-default write on a node no put adopted yet.
        _count("flags_skip.unadopted")
        return
    _refresh_flags(self, name)


def _instance_fullname(typ: Any) -> str | None:
    """Fullname of a metaclass_type Instance or None."""
    try:
        fullname: str | None = typ.type.fullname
        return fullname
    except Exception:
        return None


def _capture_meta(info: Any, field: str) -> None:
    """Record the post-write meta fields of one TypeInfo."""
    global _in_capture
    _in_capture = True
    try:
        try:
            bases_count = len(info.bases) if info.bases is not None else 0
            mro_count = len(info.mro) if info.mro is not None else 0
            mc = info.metaclass_type
            mc_fullname = _instance_fullname(mc) if mc is not None else None
            try:
                fullname = info._fullname
            except Exception:
                fullname = None
            names_table = info.names
        except Exception:
            _count("capture_fail.meta_read")
            return
        try:
            _kernel_mod.rust_symtable_mirror_meta_put(
                info,
                bases_count,
                mro_count,
                mc_fullname,
                fullname if isinstance(fullname, str) else None,
                names_table,
            )
            _count("meta.put." + field)
        except Exception:
            _count("capture_fail.meta_put")
    finally:
        _in_capture = False


def _typeinfo_setattr(self: Any, name: str, value: Any) -> None:
    _ORIG_NODE_SETATTR(self, name, value)
    if not _active or _in_capture:
        return
    if name not in _META_FIELDS:
        return
    _capture_meta(self, name)


def activate(*, audit: bool = False) -> None:
    """Enable namespace-shadow capture; a missing extension leaves it off.

    Activation is one-shot (un-patching mid-run would desync live
    records), matching the type and node mirrors. A later activate call
    may still turn audit counters on.
    """
    global _active, _audit_mode, _kernel_mod
    if _active:
        if audit:
            _audit_mode = True
        return
    try:
        import type_kernel as _km
    except ImportError:
        _count("activate_failed.no_type_kernel")
        return
    _kernel_mod = _km
    _audit_mode = audit
    patches = (
        (SymbolTable, "__setitem__", _symtable_setitem),
        (SymbolTable, "__delitem__", _symtable_delitem),
        (SymbolTable, "pop", _symtable_pop),
        (SymbolTableNode, "__setattr__", _symtable_node_setattr),
        (TypeInfo, "__setattr__", _typeinfo_setattr),
    )
    for cls, attr, hook in patches:
        try:
            setattr(cls, attr, hook)
        except Exception:
            # A compiled (mypyc) class refuses class-level patching; a
            # partial install stays inert because `_active` never flips.
            _count("activate_failed.patch")
            return
    _active = True
    _count("activate")


def reset(*, clear_counts: bool = False) -> None:
    """Drop namespace-shadow storage and pins (per-build boundary).

    Deliberately does not touch ``identity``: ``rust_mirror_reset`` alone
    owns the raw handle registry, so other seams' handles survive a reset.
    Activation is one-shot and kept across reset, like the mirrors.
    """
    if _kernel_mod is not None:
        _kernel_mod.rust_symtable_mirror_reset()
    _TABLE_HANDLES.clear()
    _NODE_HANDLES.clear()
    _count("reset")
    if clear_counts:
        _audit.clear()
        _site_counts.clear()


def entry_count(owner: Any) -> int:
    """Live shadow entry count for one owner table."""
    if _kernel_mod is None:
        return 0
    return int(_kernel_mod.rust_symtable_mirror_entry_count(owner))


def total_entry_count() -> int:
    if _kernel_mod is None:
        return 0
    return int(_kernel_mod.rust_symtable_mirror_total_entry_count())


def names(owner: Any) -> list[str]:
    if _kernel_mod is None:
        return []
    return list(_kernel_mod.rust_symtable_mirror_names(owner))


def lookup(owner: Any, name: str) -> dict[str, Any] | None:
    if _kernel_mod is None:
        return None
    record = _kernel_mod.rust_symtable_mirror_lookup(owner, name)
    return dict(record) if record is not None else None


def generation(owner: Any) -> int | None:
    if _kernel_mod is None:
        return None
    result = _kernel_mod.rust_symtable_mirror_generation(owner)
    return None if result is None else int(result)


def handle_of(obj: Any) -> int | None:
    if _kernel_mod is None:
        return None
    result = _kernel_mod.rust_symtable_mirror_handle_of(obj)
    return None if result is None else int(result)


def report() -> dict[str, int]:
    """Return a copy of the audit counters (including per-site entries)."""
    merged = dict(_audit)
    merged.update(_site_counts)
    return merged
