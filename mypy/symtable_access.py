"""Namespace accessor seam (Phase G3.0a, issue #1581).

`put_names_entry` is the normative committed-put path for symbol tables:
it writes the entry and records it in the namespace shadow. It mirrors
the committed branch of ``SemanticAnalyzer.add_symbol_table_node``
(semanal.py:8814-8820) and the other committed writes of the semanal
adding funnel; the placeholder / replacement-validity / missing-names
decision logic stays in Python, and a refused put simply never calls this
function (refusals leave no shadow trace by construction). The put gate
and the ``progress`` signal (semanal.py:8810-8815) are convergence
semantics and are not re-derived here.

Callers (the G3.0a routing):
- ``semanal.prepare_file`` / ``prepare_builtins_namespace`` (three writes),
- ``semanal.SemanticAnalyzer.add_symbol_table_node`` (the main put),
- ``semanal.SemanticAnalyzer.add_redefinition``,
- ``semanal.SemanticAnalyzer.add_global_symbol``,
- the implicit-attribute put in ``analyze_member_lvalue``.

Direct ``SymbolTable.__setitem__`` writes outside this seam (plugins,
synthetic tables, checker fake infos) are captured by the same class
patch and counted as ``bypass.put``; the G3.0b sweep reroutes them.

``delete_names_entry`` / ``delete_names_entry_safe`` are the normative
delete paths: they remove the dict entry via ``dict.__delitem__`` /
``dict.pop`` (bypassing the patched ``SymbolTable.__delitem__`` / ``pop``)
and clean up the shadow record when the mirror is active.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from mypy import symtables_mirror


@dataclass(frozen=True)
class PutResult:
    """Receipt for one committed namespace put.

    ``committed`` mirrors the True branch of ``add_symbol_table_node``:
    the entry now exists in the table. ``generation`` and ``seq`` are the
    shadow receipt (0 when the mirror is off or the capture failed, both
    failure-safe no-ops for the write itself).
    """

    committed: bool
    generation: int
    seq: int


_PLAIN = PutResult(committed=True, generation=0, seq=0)


def put_names_entry(table: Any, name: str, symbol: Any) -> PutResult:
    """Write ``symbol`` under ``name`` and record it in the shadow.

    The raw ``dict.__setitem__`` write is the normative path and never
    reaches the patched ``SymbolTable.__setitem__``, so the class patch's
    ``bypass.put`` counter measures un-routed writes only.
    """
    if not symtables_mirror._active:
        dict.__setitem__(table, name, symbol)
        return _PLAIN
    receipt = symtables_mirror.put(table, name, symbol)
    if receipt is None:
        return _PLAIN
    _owner_handle, _node_handle, seq, generation = receipt
    return PutResult(committed=True, generation=generation, seq=seq)


def delete_names_entry(table: Any, name: str) -> None:
    """Delete ``name`` from ``table`` and clean up the shadow record."""
    dict.__delitem__(table, name)
    if symtables_mirror._active:
        symtables_mirror._delete(table, name)


def delete_names_entry_safe(table: Any, name: str) -> None:
    """Delete ``name`` from ``table`` without raising on a missing key."""
    dict.pop(table, name, None)
    if symtables_mirror._active:
        symtables_mirror._delete(table, name)


# ---- G3.0c: TypeInfo meta-field accessors ----


def set_bases_mro(info: Any, bases: Any, mro: Any) -> None:
    """Write ``info.bases`` and ``info.mro``, then invalidate subtype caches.

    Uses ``object.__setattr__`` so the class patch's re-entrant guard is
    not tripped; the shadow capture runs via ``_capture_meta`` directly.
    """
    object.__setattr__(info, "bases", bases)
    object.__setattr__(info, "mro", mro)
    if symtables_mirror._active:
        symtables_mirror._capture_meta(info, "bases_mro")
    from mypy.typestate import type_state

    type_state.reset_subtype_caches_for(info)


def set_meta(info: Any, metaclass_type: Any) -> None:
    """Write ``info.metaclass_type`` and invalidate subtype caches."""
    object.__setattr__(info, "metaclass_type", metaclass_type)
    if symtables_mirror._active:
        symtables_mirror._capture_meta(info, "metaclass_type")
    from mypy.typestate import type_state

    type_state.reset_subtype_caches_for(info)


def set_info_fullname(info: Any, fullname: str) -> None:
    """Write ``info._fullname`` (no subtype-cache invalidation needed)."""
    object.__setattr__(info, "_fullname", fullname)
    if symtables_mirror._active:
        symtables_mirror._capture_meta(info, "_fullname")


def rebind_names_table(info: Any, names: Any) -> None:
    """Replace ``info.names`` with a new ``SymbolTable`` identity."""
    object.__setattr__(info, "names", names)
    if symtables_mirror._active:
        symtables_mirror._capture_meta(info, "names")
