"""Phase G1 expression dual-write node shadow (issues #1572, #1576).

Python stays canonical. With the AST-mirror gate on, writes to the first
shadowed node family are recorded into Rust storage behind the type
kernel's `rust_node_mirror_*` pyfunctions: the `RefExpr` binding scalars
(`kind`, target `node` fullname, `_fullname`, `is_new_def`,
`is_inferred_def`) and the last `analyzed` replacement class name
(record-only, CallExpr-like expression classes). G1.0b (#1576) extends
the store to the remaining G1 expression fields: `method_type`
(OpExpr/IndexExpr/UnaryExpr), `method_types` (ComparisonExpr), `as_type`
(OpExpr/IndexExpr/StrExpr), `right_always`/`right_unreachable` (OpExpr),
`def_var` (MemberExpr) and the RefExpr/NameExpr `is_special_form`,
`is_alias_rvalue`, `type_guard` and `type_is` flags. G1.2 (#1674) adds
the node's own `name` slot (`NameExpr`/`MemberExpr`) so the aststrip
lvalue read flip has a shadow record to serve. Each field keeps a
presence marker plus a shape record (class name, bool, fullname, or
kind list); the payload type graph is G1.1, which extends these records
with wire bytes. G1.2 (#1674) gives the store its first consumer: the
aststrip lvalue read (`mypy/server/aststrip.py`, gated by
`Options.native_ast_mirror_read`) is served from here for the
`is_new_def` and `name` slots, and falls back to the live slots for
everything the records do not cover.

G1.1 adds the store's first *mode-gated* serving read channel:
`set_read_flip` selects a mode (0 off, 1 serve, 2 serve + differential
compare) that the native dependency walker obeys when it reads the
`RefExpr` binding scalars, so those reads are answered from the record
instead of crossing to the live slots. Mode 1 is the production default
(#1860, G4 expression-family graduation): `build` calls
`set_production_read_flip` with the capture and `native_ast_mirror_read`
gates, so a default production run serves, while an explicit
`MYPY_TK_NODE_READ_FLIP` still wins for the measurement arms and mode 2
is never a production mode. `read_counters` reports the channel's
provenance (served, deferred, compared, mismatched), which is what makes
a run's evidence checkable rather than assumed.

G2 store extension (#1787 / PR A, record-only, no consumer): the
statement/def metadata store registers the constructor-set payload
fields (`Var._name`, `FuncDef._name` / `arg_names` / `arg_kinds` /
`original_first_arg`, `ClassDef.name`) in `_META_CTOR_FIELDS`, reads
them back once at first adoption (`_seed_meta_ctor_fields`, the G1.2
`_seed_name` pattern generalized), and pins every captured `RefExpr`
binding target under its identity handle
(`rust_node_mirror_capture_pin`), which `rust_node_mirror_object_of`
resolves back to that exact live object - the substrate the Var key
scheme needs.

G2.1 (#1787 PR B) gives the statement family its own mode-gated serving
read: `set_stmt_read_flip` selects a mode (0 off, 1 serve, 2 serve +
differential compare) that the native consumers obey when they read the
one registered statement field they read live, `Block.is_unreachable`
(the dependency walker and the native semanal visitor). A record is
served only in the exact shape the capture wrote (`Bool`); a
shape-crossed record defers to the live slot, so record-shape drift can
never answer a read. `stmt_read_counters` reports the same provenance
seven-tuple as the G1.1 channel, which is what makes a run's evidence
checkable rather than assumed.

G2.3 (#1787, the node-valued serve set) extends the same channel to the
node-valued structural field native code reads live,
`AssertStmt`/`ReturnStmt`/`ExpressionStmt` `expr`. Those records carry the
captured object's identity handle (`_META_NODE_FIELDS`), and a read is
answered with the live object `rust_node_mirror_object_of` resolves that
handle to, so the served child is the pinned object itself rather than a
copy. A record without a resolvable handle defers, which is what keeps a
stale or marker-only record from answering with a wrong child. These three
classes have no post-construction writer for their serve field, so the
constructor write is their adoption point (`_meta_ctor_adopts`) and the
adoption-time seed holds the set from there on.

G2.2 (#1787, the Var handle scheme) gives the binder key its own
mode-gated translation: `set_var_key_flip` selects a mode (0 keeps the
live-object key, 1 emits the store handle, 2 emit + per-key differential
compare) that `mypy/literals.py` obeys for `("Var", ...)` narrowing keys and
that `extract_var_from_literal_hash` reverses through
`rust_node_mirror_object_of`. A handle only translates when it resolves back
to the exact pinned `Var`, so an unresolvable key defers to the live object
instead of keying a lookup on the wrong node. The mode lives in Rust storage
(`FlipState`), the gate is opt-in (`MYPY_TK_VAR_KEY_FLIP`, an unset
variable wires no hook into `literal_hash`), and the counters report
`deferred == 0` in mode 2 to prove the key space stayed homogeneous.

G2.4 (#1825) makes the store's cache-loaded coverage a contract rather
than a by-product of the reader. The fixed-format and JSON cache readers
materialize a def-family node through plain attribute writes, so the
patched ``__setattr__`` sees them - but a write at its constructor default
is skipped, so which slots a cached node's record held followed the
reader's write order and values. `seed_loaded` now runs once per finished
node (from `read_symbol`, `read_overload_part` and the JSON
`SymbolNode.deserialize`) and records every tracked slot the live object
still has, in one kernel crossing. A field record is attributed on two
axes: the capture origin (`meta_written.parse` /
`meta_written.cache_fixed` / `meta_written.cache_json`, set by
`cache_read_origin` around the readers) and the mechanism
(`meta_written.*` for the capture funnel, `meta_seeded.*` for the seed).
Absence still means "not recorded": a slot the live object lacks is skipped
and counted (`meta_seed_loaded_missing`), and a class the store tracks no
slot for is counted as a refusal (`meta_seed_rejected_untracked`) rather
than passing silently, so "the seed declined" and "the seed never ran" are
never the same reading.

Blind channels, audited against #1787 §1(b):
- `replace_object_state` (`mypy/util.py`): its `setattr` leg re-registers
  the surviving identity through the same hook, pinned by
  `test_replace_object_state_reregisters_surviving_identity`. The
  `delattr(new, attr)` leg fires only where `old` lacks a slot, and every
  tracked slot of the G2 families is constructor-set on `old`, so it cannot
  drop a captured value; should it ever fire on a recorded one, the
  `__delattr__` patch retracts it (#1841). The `new.__dict__` leg is inert
  for these `__slots__`-only classes.
- An in-place list mutation is invisible to the patch, so its writer must
  call `touch` afterwards. `mypy/plugins/attrs.py` does, after its
  `decorators.remove` loop - the second explicit touch site after
  `ImportBase.assignments`.
- A `del` on a tracked *G1* slot is out of contract (#1856): the G1
  classes carry only the `__setattr__` patch, so the record keeps the
  deleted slot. The G1 record is not per-field where it matters - the five
  binding scalars the serving channel reads are one snapshot gated by a
  single presence marker (`ref_captures`) - so the #1841-style per-field
  retire cannot cover them without new record state. #1860 sent the G1
  serving flip default-on with this gap still open, by owner contract:
  no production `del` on a tracked G1 slot exists, the pins stay in force
  (`NodeSlotDeletionOutOfContractSuite`), and the write-flip work must
  flip them knowingly.

Design notes:
- Capture is via class-level monkeypatching of ``__setattr__`` on
  ``RefExpr``, ``CallExpr``, ``IndexExpr`` and ``OpExpr``. These classes
  use ``__slots__`` but define no ``__setattr__`` of their own, so the
  patch composes cleanly and ``type(x) is NameExpr`` keeps working. When
  the gate is off nothing is patched, so the cost in normal runs is one
  ``if`` at build activation and reset.
- Registration is lazy: an unregistered node's writes only mint an entry
  when a tracked field leaves its constructor default, so building a
  ``NameExpr`` never pins anything. The first binding write (semanal's
  ``lvalue.kind = ...`` etc.) adopts the node and stores the full
  post-write record; later tracked writes refresh it. Writes to
  non-shadowed fields (``name``, ``line``, ``value``, ...) pass
  straight through.
- Strong pins: the Rust store pins each captured node until reset, so
  the ``id()``-keyed ``_NODE_HANDLES`` map cannot go stale; no Python
  pin dict is needed. ``reset`` drops entries and pins but does NOT
  touch ``identity``: ``rust_mirror_reset`` remains the single owner of
  ``identity::reset`` (the proxy contract).
- Capture failures never propagate into the write path: the original
  write has already been applied, and a failure only increments an audit
  counter so a partial object never breaks a build.
- ``analyzed`` capture covers the expression classes that carry the
  field (``CallExpr``, ``IndexExpr``, ``OpExpr``). ``ClassDef.analyzed``
  is a G2 statement-family input and deliberately out of scope here.

Shadowed write sites (the G0 brief's G1 table, all captured through the
patched ``__setattr__``; semanal.py anchors from the #1572 base):
- semanal.py:4347-4351 import-dummy lvalue/rvalue binding,
- semanal.py:4572-4573 module-import fallback binding,
- semanal.py:4789 special-form fullname write,
- semanal.py:4994 inferred-def reset,
- semanal.py:5501 alias-rvalue ``analyzed`` write,
- semanal.py:5773-5776 new-def binding,
- semanal.py:5964-5965 implicit-attribute new-def binding,
- semanal.py:6020 declared-types inferred reset,
- semanal.py:7247-7249 visit_name_expr binding,
- checker.py:2372/2416/2525 NameExpr binder construction bindings,
- checker.py:4088 ``__init_subclass__`` base binding,
- checker.py:6726 redefinition binder binding,
- checker.py:10716-10717 forward-reference binding,
- server/astmerge.py:308-311 CallExpr ``analyzed`` fixup.

G1.0b write sites (anchors from the #1574 base, all through the patched
``__setattr__`` except the ``touch`` sites):
- checkexpr.py:5620 OpExpr ``method_type`` (``visit_op_expr``),
- checkexpr.py:6555 OpExpr ``method_type`` (``check_list_multiply``),
- checkexpr.py:5714/5718/5747/5785 ComparisonExpr ``method_types``
  appends (in place; covered by ``touch`` at the end of
  ``visit_comparison_expr``),
- checkexpr.py:6582 UnaryExpr ``method_type``,
- checkexpr.py:6716/6788 IndexExpr ``method_type`` (``visit_index_with_type``
  native and Python tails),
- checkexpr.py:2721/2723 RefExpr ``type_guard``/``type_is`` (callee),
- checker.py:6701 IndexExpr ``method_type`` (``check_indexed_assignment``),
- semanal.py:4355/5501 RefExpr ``is_alias_rvalue``,
- semanal.py:4789 NameExpr ``is_special_form``,
- semanal.py:5972 MemberExpr ``def_var`` (``analyze_member_lvalue``),
- semanal.py:7676/7681 OpExpr ``right_unreachable``/``right_always``,
- semanal.py:9779-9893 ``as_type`` writes (StrExpr/IndexExpr/OpExpr),
- server/astmerge.py:274-275 MemberExpr ``def_var`` fixup,
- treetransform.py:485/493 ``is_special_form``/``def_var`` copies.
"""

from __future__ import annotations

import json
import os
import sys
from typing import Any, Final

from mypy.nodes import (
    AssertStmt,
    AssignmentStmt,
    Block,
    CallExpr,
    ClassDef,
    ComparisonExpr,
    Decorator,
    ExpressionStmt,
    ForStmt,
    FuncDef,
    IfStmt,
    ImportBase,
    IndexExpr,
    MatchStmt,
    NotParsed,
    OpExpr,
    OverloadedFuncDef,
    RefExpr,
    ReturnStmt,
    StrExpr,
    TypeAliasStmt,
    UnaryExpr,
    Var,
    WithStmt,
)
from mypy.types import Type as MypyType

# The five RefExpr binding scalars. `fullname` is a property writing
# `_fullname`, so the slot name is the tracked key.
_REF_FIELDS: Final[frozenset[str]] = frozenset(
    {"kind", "node", "_fullname", "is_new_def", "is_inferred_def"}
)
# The expression classes that carry `analyzed`.
_ANALYZED_CLASSES: Final[tuple[type, ...]] = (CallExpr, IndexExpr, OpExpr)
# G1.0b (#1576) field sets: kind = class name (None when cleared),
# flag = bool, name = `def_var` fullname, kinds = `method_types` list.
# Only `method_types` is append-only and needs `touch`.
_KIND_FIELDS: Final[frozenset[str]] = frozenset(
    {"method_type", "as_type", "type_guard", "type_is"}
)
_FLAG_FIELDS: Final[frozenset[str]] = frozenset(
    {"right_always", "right_unreachable", "is_special_form", "is_alias_rvalue"}
)
_NAME_FIELDS: Final[frozenset[str]] = frozenset({"def_var"})
_KINDS_FIELDS: Final[frozenset[str]] = frozenset({"method_types"})
# G1.2 (#1674): the node's own name slot (NameExpr/MemberExpr). Parse-time
# shaped, so it is captured only for an already-adopted node (see
# `_node_setattr`) plus a read-back seed at adoption (`_seed_name`).
_TEXT_FIELDS: Final[frozenset[str]] = frozenset({"name"})
_FIELD_NAMES: Final[frozenset[str]] = (
    _KIND_FIELDS | _FLAG_FIELDS | _NAME_FIELDS | _KINDS_FIELDS | _TEXT_FIELDS
)

_kernel_mod: Any = None
_active = False
_audit_mode = False
# Re-entrancy guard: a capture reads live fields, never writes them, but
# the guard keeps a future field-property hook from recursing.
_in_capture = False
_ORIG_SETATTR: Any = object.__setattr__
_ORIG_DELATTR: Any = object.__delattr__
# id(node) -> native handle for every node the store holds a record for.
# The Rust store pins the node, so the id() keys cannot be recycled while
# an entry exists; the patched __setattr__ is the only minting path.
_NODE_HANDLES: dict[int, int] = {}
_audit: dict[str, int] = {}

# G1.1: the serving-read gate. The mode lives in Rust storage, so
# `set_read_flip` is a delegate and never a second source of truth.
# `activate` reads this env var, so a run can enable it without an option.
_READ_FLIP_ENV: Final = "MYPY_TK_NODE_READ_FLIP"
_READ_MODES: Final[frozenset[int]] = frozenset({0, 1, 2})
# G2.1 (#1787 PR B): the statement-family serving gate, same convention
# as the G1.1 gate above.
_STMT_READ_FLIP_ENV: Final = "MYPY_TK_STMT_READ_FLIP"
# G2.2 (#1787): the `Var` binder-key translation gate. Opt-in even among the
# other flips: an unset variable leaves the key path untouched rather than
# wiring a PyO3 crossing into every `literal_hash`.
_VAR_KEY_FLIP_ENV: Final = "MYPY_TK_VAR_KEY_FLIP"

# G2.4 (#1825): capture origins. A field record is attributed to the path
# that materialized the object; the marker is module state, not a per-call
# argument, because the reader's own writes are captures too.
ORIGIN_PARSE: Final = "parse"
ORIGIN_CACHE_FIXED: Final = "cache_fixed"
ORIGIN_CACHE_JSON: Final = "cache_json"
_capture_origin: str = ORIGIN_PARSE


def _count(key: str, n: int = 1) -> None:
    if not _audit_mode:
        return
    _audit[key] = _audit.get(key, 0) + n


def _count_origin(prefix: str, n: int = 1) -> None:
    """Count one capture under the origin now in force (#1825).

    The key is built only under audit, so the off-audit path pays one call
    and no string allocation: these sites are on mypy's write and cache
    paths, where an f-string per record would be a real cost.
    """
    if not _audit_mode:
        return
    _count(f"{prefix}.{_capture_origin}", n)


def _kernel() -> Any | None:
    """The kernel module, imported and cached on first need.

    Every read-flip entry point resolves the extension through here, so
    a mode set before `activate` registered the module stays readable
    afterwards and the three functions can never disagree (#1779).
    """
    global _kernel_mod
    if _kernel_mod is None:
        try:
            import type_kernel as _km
        except ImportError:
            return None
        _kernel_mod = _km
    return _kernel_mod


def set_read_flip(mode: int) -> int:
    """Set the node-shadow serving mode and return the mode now in force.

    Modes: 0 off (default), 1 serve, 2 serve + differential compare. A
    missing extension leaves the mode at 0, which is the same state as an
    inert channel, so no caller can be misled into thinking it served.
    """
    if mode not in _READ_MODES:
        raise ValueError(f"node read flip mode must be 0, 1 or 2, got {mode!r}")
    kernel = _kernel()
    if kernel is None:
        _count("read_flip.no_type_kernel")
        return 0
    return int(kernel.rust_node_mirror_set_read_mode(mode))


def read_flip() -> int:
    """The serving mode now in force (0 when the extension is absent)."""
    kernel = _kernel()
    if kernel is None:
        return 0
    return int(kernel.rust_node_mirror_read_mode())


def set_production_read_flip(serve: bool) -> int:
    """Set the G1.1 mode a production build runs (#1860).

    `serve` is `capture_active and Options.native_ast_mirror_read`: with
    both on, the expression family's walker reads are served (mode 1).
    An explicit `MYPY_TK_NODE_READ_FLIP` always wins, so the differential
    arms keep control of the channel, and the compare mode (2) stays a
    measurement mode this entry point never selects. Called on every
    manager, so a later build cannot inherit a mode from an earlier one
    in the process (the aststrip flip's rule).
    """
    if os.environ.get(_READ_FLIP_ENV) is not None:
        return read_flip()
    return set_read_flip(1 if serve else 0)


def read_counters() -> dict[str, int]:
    """Provenance counters for the serving channel.

    `served` counts reads the record answered, `deferred_off` the mode-0
    deferrals, `deferred_unrecorded` the reads no exact record covered,
    and `compared`/`mismatched` the differential. A run whose `compared`
    is 0 proves nothing about the served values.
    """
    kernel = _kernel()
    if kernel is None:
        return {}
    (
        consulted,
        served,
        deferred_off,
        deferred_unrecorded,
        compared,
        mismatched,
        compare_errors,
    ) = kernel.rust_node_mirror_read_counters()
    return {
        "consulted": int(consulted),
        "served": int(served),
        "deferred_off": int(deferred_off),
        "deferred_unrecorded": int(deferred_unrecorded),
        "compared": int(compared),
        "mismatched": int(mismatched),
        "compare_errors": int(compare_errors),
    }


def set_stmt_read_flip(mode: int) -> int:
    """Set the statement-family serving mode; returns the mode in force.

    Modes: 0 off (default), 1 serve, 2 serve + differential compare. A
    missing extension leaves the mode at 0, the same state as an inert
    channel, so no caller can be misled into thinking it served.
    """
    if mode not in _READ_MODES:
        raise ValueError(f"stmt read flip mode must be 0, 1 or 2, got {mode!r}")
    kernel = _kernel()
    if kernel is None:
        _count("stmt_read_flip.no_type_kernel")
        return 0
    return int(kernel.rust_node_mirror_set_stmt_read_mode(mode))


def stmt_read_flip() -> int:
    """The statement-family serving mode (0 when the extension is absent)."""
    kernel = _kernel()
    if kernel is None:
        return 0
    return int(kernel.rust_node_mirror_stmt_read_mode())


def stmt_read_counters() -> dict[str, int]:
    """Provenance counters for the statement-family serving channel.

    One seven-tuple for both served record shapes: the `Bool` records
    (`Block.is_unreachable`) and the node-valued ones (`expr`). `served`
    counts a read the record answered, `deferred_off` the mode-0
    deferrals, `deferred_unrecorded` the reads no exact record covered (or
    whose handle no longer resolves), and `compared`/`mismatched`/
    `compare_errors` the differential. A run whose `compared` is 0 proves
    nothing about the served values.
    """
    kernel = _kernel()
    if kernel is None:
        return {}
    (
        consulted,
        served,
        deferred_off,
        deferred_unrecorded,
        compared,
        mismatched,
        compare_errors,
    ) = kernel.rust_node_mirror_stmt_read_counters()
    return {
        "consulted": int(consulted),
        "served": int(served),
        "deferred_off": int(deferred_off),
        "deferred_unrecorded": int(deferred_unrecorded),
        "compared": int(compared),
        "mismatched": int(mismatched),
        "compare_errors": int(compare_errors),
    }


def set_var_key_flip(mode: int) -> int:
    """Set the `Var`-key translation mode; returns the mode now in force.

    Modes: 0 off (keys keep the live object, deferrals counted), 1 serve the
    store handle, 2 serve + a per-key differential compare. A missing
    extension leaves the mode at 0 and installs no hook, so the key path stays
    untouched rather than half-wired.
    """
    if mode not in _READ_MODES:
        raise ValueError(f"var key flip mode must be 0, 1 or 2, got {mode!r}")
    kernel = _kernel()
    if kernel is None:
        _count("var_key_flip.no_type_kernel")
        return 0
    mode_in_force = int(kernel.rust_node_mirror_set_var_key_mode(mode))
    _install_var_key_hooks()
    return mode_in_force


def var_key_flip() -> int:
    """The `Var`-key translation mode (0 when the extension is absent)."""
    kernel = _kernel()
    if kernel is None:
        return 0
    return int(kernel.rust_node_mirror_var_key_mode())


def var_key_counters() -> dict[str, int]:
    """Provenance counters for the `Var`-key translation.

    The same seven-tuple as `read_counters`, plus the derived `deferred`
    (`deferred_off` + `deferred_unrecorded`): mode 2 requires it to be 0,
    because a key space mixing handle keys and live-object keys is what the
    translation exists to rule out.
    """
    kernel = _kernel()
    if kernel is None:
        return {}
    (
        consulted,
        served,
        deferred_off,
        deferred_unrecorded,
        compared,
        mismatched,
        compare_errors,
    ) = kernel.rust_node_mirror_var_key_counters()
    return {
        "consulted": int(consulted),
        "served": int(served),
        "deferred_off": int(deferred_off),
        "deferred_unrecorded": int(deferred_unrecorded),
        "deferred": int(deferred_off) + int(deferred_unrecorded),
        "compared": int(compared),
        "mismatched": int(mismatched),
        "compare_errors": int(compare_errors),
    }


def _translate_var_key(var: Var) -> Any:
    """The handle for a captured `Var`'s key element, or the live object.

    A `None` from the seam is the defer: mode 0, or a `Var` the capture never
    pinned, so the key must stay the live object.
    """
    kernel = _kernel()
    if kernel is None:
        return var
    handle = kernel.rust_node_mirror_serve_var_key(var)
    return var if handle is None else handle


def _resolve_var_key(element: Any) -> Var | None:
    """Resolve a translated key element back to the live `Var` (#1787)."""
    kernel = _kernel()
    if kernel is None or not isinstance(element, int):
        return None
    obj = kernel.rust_node_mirror_object_of(element)
    return obj if isinstance(obj, Var) else None


def _install_var_key_hooks() -> None:
    """Wire the translation hooks into `mypy.literals` (idempotent)."""
    from mypy.literals import set_var_key_hooks

    set_var_key_hooks(_translate_var_key, _resolve_var_key)


def _uninstall_var_key_hooks() -> None:
    """Clear the translation hooks so `literal_hash` keeps live keys."""
    from mypy.literals import set_var_key_hooks

    set_var_key_hooks(None, None)


def count(key: str, n: int = 1) -> None:
    """Record one engagement counter; a no-op without the audit flag.

    Read-flip seams (`mypy/server/aststrip.py`) report served reads and
    fallbacks through this so the flip's engagement is provable per path.
    """
    _count(key, n)


def _serialize_type_wire(t: MypyType) -> bytes:
    """Serialize a Type to wire bytes for the node mirror store.

    Uses the mirror's fresh-bytes path so the wire cache does not
    interfere.  On failure returns empty bytes (the capture site treats
    an empty blob as a cleared field).
    """
    try:
        from mypy.types_mirror import _fresh_bytes

        return _fresh_bytes(t)
    except Exception:
        _count("wire_serialize_fail")
        return b""


def read_field_type(handle: int, field: str) -> MypyType | None:
    """Read a type-valued field from the shadow store and deserialize it.

    Returns the Type, or None when the field is absent, was cleared, or
    cannot be deserialized.
    """
    if _kernel_mod is None:
        return None
    result = _kernel_mod.rust_node_mirror_field_wire(handle, field)
    if result is None:
        return None
    kind, wire = result
    if kind is None or not wire:
        return None
    try:
        from mypy.cache import ReadBuffer
        from mypy.types import read_type

        data = ReadBuffer(wire)
        return read_type(data)
    except Exception:
        _count("wire_deserialize_fail")
        return None


def _is_baseline(name: str, value: Any) -> bool:
    """True for a constructor-default write of a tracked field.

    Skipping these keeps unbound nodes (the overwhelming majority at
    construction time) out of the store; the first non-default write
    adopts the node with the full post-write record.
    """
    if name == "kind" or name == "node":
        return value is None
    if name == "_fullname":
        return isinstance(value, str) and value == ""
    return value is False  # is_new_def / is_inferred_def


def _seed_name(node: Any) -> None:
    """Read back the node's own `name` slot at adoption (G1.2, #1674).

    `name` is written by the constructor, before any adopted write can
    see it, so adoption reads it back instead; every later write to the
    slot is captured by the adopted-only rule in `_node_setattr` (the
    `mypy/renaming.py` rename is the one production writer).
    """
    try:
        name = node.name
    except AttributeError:
        return
    if isinstance(name, str):
        _kernel_mod.rust_node_mirror_capture_field_text(node, "name", name)


def _capture_ref(node: RefExpr) -> None:
    global _in_capture
    _in_capture = True
    try:
        target = node.node
        node_fullname: str | None = None
        if target is not None:
            try:
                fullname = target.fullname
            except Exception:
                fullname = None
            if isinstance(fullname, str):
                node_fullname = fullname
            # G2 (#1787): pin the binding target under its identity handle
            # so `rust_node_mirror_object_of` can resolve the handle back
            # to this exact object later (the Var key substrate).
            _kernel_mod.rust_node_mirror_capture_pin(target)
            _count("pin_target")
        handle = _kernel_mod.rust_node_mirror_capture_ref(
            node, node.kind, node_fullname, node._fullname, node.is_new_def, node.is_inferred_def
        )
        # Seed the parse-time `name` slot only on first adoption: later
        # writes to an adopted node re-issue the identical record (#1714),
        # while adopted-only `_TEXT_FIELDS` capture covers real renames.
        newly_adopted = id(node) not in _NODE_HANDLES
        _NODE_HANDLES[id(node)] = handle
        _count("capture_ref")
        if newly_adopted:
            _seed_name(node)
    except Exception:
        _count("capture_fail.ref")
    finally:
        _in_capture = False


def _capture_analyzed(node: Any) -> None:
    global _in_capture
    _in_capture = True
    try:
        value = node.analyzed
        analyzed_kind = None if value is None else type(value).__name__
        handle = _kernel_mod.rust_node_mirror_capture_analyzed(node, analyzed_kind)
        _NODE_HANDLES[id(node)] = handle
        _count("capture_analyzed")
    except Exception:
        _count("capture_fail.analyzed")
    finally:
        _in_capture = False


def _is_field_baseline(name: str, value: Any) -> bool:
    """True for a constructor-default write of a G1.0b field.

    `as_type` defaults to the `NotParsed.VALUE` sentinel while an
    analysis write of None is meaningful; the other type-valued fields
    default to None; the flags default to False and `method_types` to
    an empty list.
    """
    if name == "as_type":
        return isinstance(value, NotParsed)
    if name in ("method_type", "type_guard", "type_is", "def_var"):
        return value is None
    if name == "method_types":
        return not value
    return value is False


def _capture_field(node: Any, name: str) -> None:
    global _in_capture
    _in_capture = True
    try:
        value = getattr(node, name)
        handle: int
        if name in _KIND_FIELDS:
            kind = None if value is None else type(value).__name__
            if isinstance(value, MypyType):
                wire = _serialize_type_wire(value)
                handle = _kernel_mod.rust_node_mirror_capture_field_wire(node, name, kind, wire)
            else:
                handle = _kernel_mod.rust_node_mirror_capture_field_kind(node, name, kind)
        elif name in _FLAG_FIELDS:
            handle = _kernel_mod.rust_node_mirror_capture_flag(node, name, bool(value))
        elif name in _TEXT_FIELDS:
            handle = _kernel_mod.rust_node_mirror_capture_field_text(node, name, str(value))
        elif name in _NAME_FIELDS:
            target = value
            fullname: str | None = None
            if target is not None:
                try:
                    raw = target.fullname
                except Exception:
                    raw = None
                fullname = raw if isinstance(raw, str) else None
            handle = _kernel_mod.rust_node_mirror_capture_field_name(node, name, fullname)
        else:
            # method_types is the only list-valued shadowed field.
            kinds = [None if item is None else type(item).__name__ for item in value]
            handle = _kernel_mod.rust_node_mirror_capture_field_kinds(node, name, kinds)
        _NODE_HANDLES[id(node)] = handle
        _count("capture_" + name)
    except Exception:
        _count("capture_fail." + name)
    finally:
        _in_capture = False


def _node_setattr(self: Any, name: str, value: Any) -> None:
    # Apply the write first: the store records the post-write state, and
    # any AttributeError from an unknown slot propagates exactly as the
    # unpatched object.__setattr__ would.
    _ORIG_SETATTR(self, name, value)
    if not _active or _in_capture:
        return
    if name in _REF_FIELDS:
        if _NODE_HANDLES.get(id(self)) is None and _is_baseline(name, value):
            # Constructor-default write on a never-adopted node: nothing
            # to shadow yet (lazy adoption baseline).
            _count("baseline_skip.ref")
            return
        _capture_ref(self)
    elif name == "analyzed" and value is None and id(self) not in _NODE_HANDLES:
        _count("baseline_skip.analyzed")
    elif name == "analyzed":
        _capture_analyzed(self)
    elif name in _TEXT_FIELDS:
        # G1.2 (#1674): a parse-time slot, so the constructor write is not
        # a capture trigger; only an adopted node can hold a stale record
        # for it (`mypy/renaming.py` renames NameExpr slots in place).
        if _NODE_HANDLES.get(id(self)) is None:
            _count("baseline_skip." + name)
            return
        _capture_field(self, name)
    elif name in _FIELD_NAMES:
        if _NODE_HANDLES.get(id(self)) is None and _is_field_baseline(name, value):
            _count("baseline_skip." + name)
            return
        _capture_field(self, name)


# ===========================================================================
# G2.0 statement/def metadata shadow (issue #1577) + G2.1 ImportBase flags.
# ===========================================================================

# Record-only metadata capture for the statement and def families, in its
# own section because the parallel G1.0b agent extends the G1 classes
# above; G2.1 adds the ImportBase bool flags and Block.is_unreachable.

# Same gate (`Options.native_ast_mirror`) and identity base as G1.0a;
# no consumer reads a record. The tracked field table is per patch
# class: records store one tagged value per written field.

# Scalar flags, strings, type objects and node collections share one
# store; object values become class/fullname markers (`InstanceType`,
# `NameExpr:mod.x`) and are never serialized.

# Constructor-set payload fields (#1787 §1(a)): `_META_CTOR_FIELDS` below
# names them, `_seed_meta_ctor_fields` reads them back at adoption, and
# the rename-safety-net contract is documented on that function.

# Fidelity gap (recorded, not fixed): list payload fields (`arg_names`,
# `arg_kinds`) hold per-item class markers, not values, like every other
# list field; a criterion-2 cache-writer slice needs a value-encoding.

_G2_FUNC_BASE: Final[frozenset[str]] = frozenset(
    {
        "type",
        "unanalyzed_type",
        "info",
        "is_property",
        "is_class",
        "is_static",
        "is_final",
        "is_explicit_override",
        "is_type_check_only",
        "def_or_infer_vars",
        "_fullname",
    }
)

_G2_FUNC_FLAGS: Final[frozenset[str]] = frozenset(
    {
        "is_property",
        "is_class",
        "is_static",
        "is_final",
        "is_explicit_override",
        "is_type_check_only",
        "def_or_infer_vars",
        "is_overload",
        "is_generator",
        "is_coroutine",
        "is_async_generator",
        "is_awaitable_coroutine",
        "is_decorated",
        "is_conditional",
        "is_trivial_body",
        "is_trivial_self",
        "is_mypy_only",
        "is_invalid_redefinition",
    }
)

_G2_FUNC_DEF: Final[frozenset[str]] = (
    frozenset(
        {
            "type",
            "unanalyzed_type",
            "_fullname",
            "abstract_status",
            "deprecated",
            "original_def",
            "dataclass_transform_spec",
            "docstring",
            # Constructor-set payload fields (#1787 §1(a)); seeded at
            # adoption through `_META_CTOR_FIELDS` below.
            "_name",
            "arg_names",
            "arg_kinds",
            "original_first_arg",
        }
    )
    | _G2_FUNC_BASE
    | _G2_FUNC_FLAGS
)

_G2_VAR: Final[frozenset[str]] = frozenset(
    {
        "_name",
        "_fullname",
        "type",
        "setter_type",
        "info",
        "final_value",
        "is_self",
        "is_cls",
        "is_ready",
        "is_inferred",
        "is_initialized_in_class",
        "is_staticmethod",
        "is_classmethod",
        "is_property",
        "is_settable_property",
        "is_classvar",
        "is_abstract_var",
        "is_final",
        "is_index_var",
        "final_unset_in_class",
        "final_set_in_init",
        "is_suppressed_import",
        "explicit_self_type",
        "from_module_getattr",
        "has_explicit_value",
        "allow_incompatible_override",
        "invalid_partial_type",
        "is_argument",
    }
)

# The node-valued serve set (#1787 §3): the fields whose record carries the
# captured object's handle, so a served read resolves the live node through
# `rust_node_mirror_object_of`. See `_meta_node_fields`/`_meta_ctor_adopts`.
_META_NODE_FIELDS: Final[dict[type, frozenset[str]]] = {
    AssertStmt: frozenset({"expr"}),
    ReturnStmt: frozenset({"expr"}),
    ExpressionStmt: frozenset({"expr"}),
}

# Class -> tracked slots. `ImportBase.assignments` is a list; the append
# sites call `touch()` because the patched `__setattr__` cannot see it.
_G2_TRACKED: Final[dict[type, frozenset[str]]] = {
    ImportBase: frozenset(
        {
            "assignments",
            "is_unreachable",
            "is_unreachable_dependency",
            "is_top_level",
            "is_mypy_only",
        }
    ),
    Block: frozenset({"is_unreachable"}),
    # The node-valued serve set (#1787 §3): `depswalk.rs` reads `expr`
    # live here, and since each tracked set is constructor-set the
    # constructor write is the adoption point (`_meta_ctor_adopts`).
    AssertStmt: _META_NODE_FIELDS[AssertStmt],
    ReturnStmt: _META_NODE_FIELDS[ReturnStmt],
    ExpressionStmt: _META_NODE_FIELDS[ExpressionStmt],
    AssignmentStmt: frozenset(
        {"type", "unanalyzed_type", "is_alias_def", "is_final_def", "invalid_recursive_alias"}
    ),
    ForStmt: frozenset(
        {
            "index",
            "index_type",
            "unanalyzed_index_type",
            "inferred_item_type",
            "inferred_iterator_type",
        }
    ),
    WithStmt: frozenset({"analyzed_types"}),
    IfStmt: frozenset({"unreachable_else"}),
    MatchStmt: frozenset({"subject_dummy"}),
    TypeAliasStmt: frozenset({"alias_node", "invalid_recursive_alias"}),
    FuncDef: _G2_FUNC_DEF,
    OverloadedFuncDef: frozenset(
        {"items", "unanalyzed_items", "impl", "deprecated", "setter_index", "_is_trivial_self"}
    )
    | _G2_FUNC_BASE,
    Decorator: frozenset({"func", "var", "is_overload", "decorators", "original_decorators"}),
    ClassDef: frozenset(
        {
            "name",
            "info",
            "analyzed",
            "has_incompatible_baseclass",
            "metaclass",
            "_fullname",
            "removed_base_type_exprs",
            "type_vars",
            "base_type_exprs",
            "removed_statements",
        }
    ),
    Var: _G2_VAR,
}

# Constructor-set payload fields (#1787 §1(a)): written by `__init__`
# before any adopted tracked write, so a tracked-set entry alone would
# adopt every constructed node at parse time (see the seed below).
_META_CTOR_FIELDS: Final[dict[type, frozenset[str]]] = {
    Var: frozenset({"_name"}),
    FuncDef: frozenset({"_name", "arg_names", "arg_kinds", "original_first_arg"}),
    ClassDef: frozenset({"name"}),
    # The node-valued serve set is constructor-set too, and read back by
    # the same seed; one table, so the two can never drift apart.
    **_META_NODE_FIELDS,
}

_META_FIELDS: dict[type, frozenset[str]] = {}
_META_CTOR_CACHE: dict[type, frozenset[str]] = {}
# id(node) -> native handle for every node the metadata store holds.
_META_HANDLES: dict[int, int] = {}


def _meta_tracked(cls: type) -> frozenset[str]:
    """Tracked slots for a patched class or any of its subclasses."""
    tracked = _META_FIELDS.get(cls)
    if tracked is None:
        tracked = frozenset()
        for base in cls.__mro__:
            found = _G2_TRACKED.get(base)
            if found is not None:
                tracked = found
                break
        _META_FIELDS[cls] = tracked
    return tracked


def _meta_ctor_fields(cls: type) -> frozenset[str]:
    """Constructor-set slots for a patched class or any of its subclasses."""
    fields = _META_CTOR_CACHE.get(cls)
    if fields is None:
        fields = frozenset()
        for base in cls.__mro__:
            found = _META_CTOR_FIELDS.get(base)
            if found is not None:
                fields = found
                break
        _META_CTOR_CACHE[cls] = fields
    return fields


def _meta_node_fields(cls: type) -> frozenset[str]:
    """Node-valued serve slots for a patched class or its subclasses."""
    fields: frozenset[str] = frozenset()
    for base in cls.__mro__:
        found = _META_NODE_FIELDS.get(base)
        if found is not None:
            fields = found
            break
    return fields


def _meta_ctor_adopts(cls: type) -> bool:
    """Whether a constructor write is also the adoption point for `cls`.

    True when every tracked slot of the class is constructor-set. Such a
    class has no post-construction writer, so the constructor write is the
    only adoption point it has; without this the record would be empty and
    serving the field would defer on every read. Every other class keeps
    the lazy-adoption baseline: its constructor stays out of the store and
    the first post-construction write adopts it, with the seed reading the
    constructor-set slots back.
    """
    tracked = _meta_tracked(cls)
    return bool(tracked) and tracked <= _meta_ctor_fields(cls)


def _meta_marker(value: Any) -> str:
    """Record-only marker for an object value: `Class` or `Class:fullname`."""
    name = type(value).__name__
    try:
        fullname = value.fullname
    except Exception:
        return name
    if isinstance(fullname, str) and fullname:
        return f"{name}:{fullname}"
    return name


def _meta_node_handle(value: Any) -> int | None:
    """Pin `value` under its identity handle for a served read-back.

    Only a node-valued serve field pays for this: the pin retains the
    object for the life of the build, which every other object-valued
    field deliberately avoids. A failed pin leaves the record marker-only,
    which can never serve, so the failure direction is the defer.
    """
    try:
        return int(_kernel_mod.rust_node_mirror_capture_pin(value))
    except Exception:
        _count("meta_pin_fail")
        return None


def _meta_encode(
    value: Any, *, node_field: bool = False
) -> tuple[str, str | None, int | None, list[str] | None]:
    """Encode one field value for the Rust store (kind, text, num, items).

    A node-valued serve field (`node_field`) carries its identity handle in
    `num`, so the store can answer the read with the exact live object;
    every other object-valued field stays marker-only.
    """
    if value is None:
        return "none", None, None, None
    if value is True or value is False:
        return "bool", None, 1 if value else 0, None
    if isinstance(value, int):
        return "int", None, value, None
    if isinstance(value, str):
        return "str", value, None, None
    if isinstance(value, (list, tuple)):
        return "list", None, None, [_meta_marker(item) for item in value]
    handle = _meta_node_handle(value) if node_field else None
    return "obj", _meta_marker(value), handle, None


def _meta_is_baseline(value: Any) -> bool:
    """True for a constructor-default value on a never-adopted node."""
    if value is None or value is False:
        return True
    if isinstance(value, (list, tuple)):
        return len(value) == 0
    if isinstance(value, str):
        return value == ""
    return False


def _seed_meta_ctor_fields(node: Any) -> None:
    """Read back the constructor-set slots at first adoption (#1787).

    Generalizes the G1.2 `_seed_name` pattern: the constructor wrote these
    before the node was adopted, so absence keeps meaning "not recorded"
    only if adoption reads them back once. A field the constructor left
    unset (mid-construction adoption) stays absent until a later captured
    write lands, so the record never holds a value the live object lacks.

    The constructor write itself does not adopt for a class with a later
    writer (`_META_CTOR_FIELDS` arm in `_meta_setattr`); a write to an
    already-adopted node is captured, which is the rename safety net. For a
    class whose whole tracked set is constructor-set, the constructor write
    IS the adoption point (`_meta_ctor_adopts`) and the seed holds the
    serve set from there on. The #1787 open-question-1 writer audit found
    no production writer that renames these slots: `_name` only in
    `FuncDef.__init__` / `Var.__init__`, `arg_names` / `arg_kinds` only in
    `FuncItem.__init__` plus the `read` classmethods (fresh objects),
    `original_first_arg` in `FuncDef.__init__` (both branches) plus `read`,
    `ClassDef.name` only in its `__init__`, and the node-valued `expr`
    slots only in the constructors of the three statement classes.
    """
    for field in _meta_ctor_fields(type(node)):
        try:
            getattr(node, field)
        except AttributeError:
            _count("meta_seed_skip")
            continue
        _capture_meta(node, field)
        _count("meta_seed")


def _capture_meta(node: Any, field: str) -> None:
    global _in_capture
    _in_capture = True
    try:
        node_field = field in _meta_node_fields(type(node))
        kind, text, num, items = _meta_encode(getattr(node, field), node_field=node_field)
        handle = _kernel_mod.rust_node_mirror_capture_meta(node, field, kind, text, num, items)
        newly_adopted = id(node) not in _META_HANDLES
        _META_HANDLES[id(node)] = handle
        _count("meta_capture")
        _count_origin("meta_written")
        if newly_adopted:
            _seed_meta_ctor_fields(node)
    except Exception:
        _count("meta_capture_fail")
    finally:
        _in_capture = False


def cache_read_origin(origin: str) -> str | None:
    """Mark the capture origin while a cache reader materializes nodes.

    Returns the origin to hand back to `restore_origin`, or None when the
    store is inactive: an off-gate reader pays one call and changes no
    state. The pair is deliberately not a context manager, because the
    readers are on mypy's hot cache path and this must cost one call.
    """
    global _capture_origin
    if not _active:
        return None
    previous = _capture_origin
    _capture_origin = origin
    return previous


def restore_origin(previous: str | None) -> None:
    """Restore the origin `cache_read_origin` returned (None: no-op)."""
    global _capture_origin
    if previous is not None:
        _capture_origin = previous


def seed_loaded(node: Any) -> int:
    """Seed one finished cache-loaded node's shadow record (#1825).

    `mypy.nodes.read_symbol`, `read_overload_part` and the JSON
    `SymbolNode.deserialize` materialize a def-family node through plain
    attribute writes. The patched `__setattr__` sees every one of them, but
    a write at its constructor default is skipped, so which slots a cached
    node's record held depended on the reader's write order and on the
    values it wrote. This hands the finished node to the kernel once, which
    records every tracked slot the live object still has.

    Returns the number of field records the seed took (0 when the gate is
    off, the class is untracked, or the call failed). Absence keeps meaning
    "not recorded": a slot the live object does not have is skipped and
    counted, never fabricated, and a class with no tracked slot is counted
    as a refusal rather than passing silently. Never raises into the
    reader's deserialization path.
    """
    if not _active:
        return 0
    tracked = _meta_tracked(type(node))
    if not tracked:
        # The store tracks nothing for this class, so the seed declined.
        # Counting the decline is what separates it from "never ran": the
        # two look identical in every other counter (#1810's rule).
        _count("meta_seed_rejected_untracked")
        return 0
    node_fields = _meta_node_fields(type(node))
    records: list[tuple[str, str, str | None, int | None, list[str] | None]] = []
    missing = 0
    for field in tracked:
        try:
            value = getattr(node, field)
        except AttributeError:
            missing += 1
            continue
        kind, text, num, items = _meta_encode(value, node_field=field in node_fields)
        records.append((field, kind, text, num, items))
    if not records:
        # Every tracked slot is unreadable on the live object: a refusal
        # for the same reason as an untracked class, split from it so the
        # two shapes stay distinguishable.
        _count("meta_seed_rejected_unreadable")
        return 0
    try:
        handle, preexisting, minted, replaced = _kernel_mod.rust_node_mirror_seed_loaded(
            node, records
        )
    except Exception:
        # Same convention as the capture failures: account, never raise
        # into the reader's deserialization path.
        _count("meta_seed_failed")
        return 0
    # Register the handle like any other adoption: `_meta_setattr` keys its
    # skip arms on this map, so a seeded node missing from it would drop a
    # later default write and keep the pre-load value the live object lost.
    _META_HANDLES[id(node)] = int(handle)
    taken = int(minted) + int(replaced)
    _count("meta_seed_loaded")
    # A zero-valued counter would break the "presence means it happened"
    # convention: a key with no event must not materialize (#1849).
    if preexisting:
        _count("meta_seed_loaded_preexisting")
    _count("meta_seed_loaded_minted", int(minted))
    _count("meta_seed_loaded_replaced", int(replaced))
    if missing:
        _count("meta_seed_loaded_missing", missing)
    _count_origin("meta_seeded", taken)
    return taken


def _meta_setattr(self: Any, name: str, value: Any) -> None:
    # Apply the write first; the store records the post-write state and
    # an unknown-slot AttributeError propagates unchanged.
    _ORIG_SETATTR(self, name, value)
    if not _active or _in_capture:
        return
    if name not in _meta_tracked(type(self)):
        return
    if id(self) not in _META_HANDLES and name in _meta_ctor_fields(type(self)):
        if _meta_ctor_adopts(type(self)) and not _meta_is_baseline(value):
            # This class has no later adoption point, so the constructor
            # write adopts it and the seed holds the serve set from there;
            # a default value still adopts nothing.
            _capture_meta(self, name)
            _count("meta_ctor_adopt")
        else:
            # A constructor-set field never adopts here: parse-time
            # construction stays out of the store, and adoption reads the
            # value back.
            _count("meta_ctor_skip")
        return
    if _meta_is_baseline(value) and id(self) not in _META_HANDLES:
        _count("meta_baseline_skip")
        return
    _capture_meta(self, name)


def touch(node: Any, field: str) -> None:
    """Capture one in-place field mutation (G1.0b list fields, G2 metadata).

    `__setattr__` cannot observe list mutations, so mutating sites call
    this after the write; G2's `ImportBase.assignments.append` and
    G1.0b's `ComparisonExpr.method_types` are the current callers.
    Off-gate calls return immediately; unknown fields are ignored.
    """
    if not _active or _in_capture:
        return
    if field in _FIELD_NAMES:
        if _is_field_baseline(field, getattr(node, field)):
            return
        _capture_field(node, field)
        _count("touch." + field)
        return
    _capture_meta(node, field)


def _meta_delattr(self: Any, name: str) -> None:
    # Apply the deletion first (an unknown-slot AttributeError propagates
    # as the unpatched object.__delattr__ would), then retract the slot's
    # record so absence keeps meaning "not recorded" (#1841).
    _ORIG_DELATTR(self, name)
    if not _active or _in_capture:
        return
    if name not in _meta_tracked(type(self)):
        return
    _retract_meta(self, name)


def _retract_meta(node: Any, field: str) -> None:
    """Retract one tracked slot's record after a `del` (#1841).

    The whole entry must not be dropped: the other tracked slots are still
    live. Never raises into the deleting site; a failed retraction is
    counted, and the stale record then answers mode-2 catches as an error.
    """
    try:
        handle = _META_HANDLES.get(id(node))
        if handle is None:
            # Never adopted: there is no record to retract, and minting a
            # handle here would adopt a node by deleting a slot from it.
            _count("meta_del_unadopted")
            return
        _count("meta_del." + field)
        if not _kernel_mod.rust_node_mirror_meta_retire_field(int(handle), field):
            _count("meta_del_unrecorded")
    except Exception:
        _count("meta_del_failed")


def _activate_meta() -> None:
    for cls in _G2_TRACKED:
        try:
            # `_G2_TRACKED` keys are plain `type`s, so the class-level
            # assignment reports as an incompatible assignment (unlike the
            # G1 tuple of concrete classes).
            cls.__setattr__ = _meta_setattr  # type: ignore[assignment]
            cls.__delattr__ = _meta_delattr  # type: ignore[assignment]
        except Exception:
            # A compiled (mypyc) class refuses class-level patching; a
            # partial install only skips that class's capture.
            _count("meta_activate_failed.patch")


def _reset_meta() -> None:
    if _kernel_mod is not None:
        _kernel_mod.rust_node_mirror_meta_reset()
    _META_HANDLES.clear()


def activate(*, audit: bool = False) -> bool:
    """Enable node-shadow capture; a missing extension leaves it off.

    Activation is one-shot (un-patching mid-run would desync live
    records), matching the type mirror. A later activate call may still
    turn counters on. A malformed serving-mode env var raises before any
    state change, so the failure cannot half-activate and a retry is not
    swallowed by the one-shot guard.

    Returns whether the store is active afterwards, so a caller that
    gates a consumer on the shadow (the G1.2 read flip) can refuse to
    enable it against a missing extension instead of trusting the option.
    """
    global _active, _audit_mode
    if _active:
        if audit:
            _audit_mode = True
        return True
    # G1.1: parse and validate the env gate before anything is patched, so
    # a bad value fails loudly with the module untouched.
    raw_mode = os.environ.get(_READ_FLIP_ENV, "0")
    try:
        env_mode = int(raw_mode)
    except ValueError:
        raise ValueError(f"{_READ_FLIP_ENV} must be 0, 1 or 2, got {raw_mode!r}") from None
    if env_mode not in _READ_MODES:
        raise ValueError(f"{_READ_FLIP_ENV} must be 0, 1 or 2, got {raw_mode!r}")
    # G2.1 (#1787 PR B): same contract for the statement-family gate,
    # parsed before anything is patched so a bad value cannot half-activate.
    raw_stmt_mode = os.environ.get(_STMT_READ_FLIP_ENV, "0")
    try:
        env_stmt_mode = int(raw_stmt_mode)
    except ValueError:
        raise ValueError(
            f"{_STMT_READ_FLIP_ENV} must be 0, 1 or 2, got {raw_stmt_mode!r}"
        ) from None
    if env_stmt_mode not in _READ_MODES:
        raise ValueError(f"{_STMT_READ_FLIP_ENV} must be 0, 1 or 2, got {raw_stmt_mode!r}")
    # G2.2 (#1787): the Var-key gate is opt-in even among the flips, so an
    # unset variable leaves the key path untouched (no hook, no PyO3 crossing
    # per `literal_hash`); a malformed value still fails before any patching.
    raw_var_key = os.environ.get(_VAR_KEY_FLIP_ENV)
    env_var_key_mode = 0
    if raw_var_key is not None:
        try:
            env_var_key_mode = int(raw_var_key)
        except ValueError:
            raise ValueError(
                f"{_VAR_KEY_FLIP_ENV} must be 0, 1 or 2, got {raw_var_key!r}"
            ) from None
        if env_var_key_mode not in _READ_MODES:
            raise ValueError(f"{_VAR_KEY_FLIP_ENV} must be 0, 1 or 2, got {raw_var_key!r}")
    kernel = _kernel()
    if kernel is None:
        _count("activate_failed.no_type_kernel")
        return False
    _audit_mode = audit
    # G1.0b adds ComparisonExpr / StrExpr / UnaryExpr; NameExpr and
    # MemberExpr already route through the RefExpr patch.
    for cls in (RefExpr, CallExpr, IndexExpr, OpExpr, ComparisonExpr, StrExpr, UnaryExpr):
        try:
            cls.__setattr__ = _node_setattr  # type: ignore[method-assign]
        except Exception:
            # A compiled (mypyc) class refuses class-level patching; a
            # partial install stays inert because `_active` never flips.
            _count("activate_failed.patch")
            return False
    _active = True
    _count("activate")
    # G2.0 (#1577): patch the statement/def family (separate section).
    _activate_meta()
    # G1.1: the serving mode is set only once capture is live, so the
    # channel can never answer from an empty store. The env var is the
    # measurement gate (see `_READ_FLIP_ENV`).
    set_read_flip(env_mode)
    _count(f"read_flip.mode{read_flip()}")
    # G2.1 (#1787 PR B): set only once the meta capture is live, so the
    # statement channel can never answer from an empty store.
    set_stmt_read_flip(env_stmt_mode)
    _count(f"stmt_read_flip.mode{stmt_read_flip()}")
    # G2.2 (#1787): engage the Var-key translation only when the env asked
    # for it, so the default path never crosses into Rust per `literal_hash`.
    if raw_var_key is not None:
        set_var_key_flip(env_var_key_mode)
        _count(f"var_key_flip.mode{var_key_flip()}")
    return True


def reset(*, clear_counts: bool = False) -> None:
    """Drop node-shadow storage and pins (per-build boundary).

    Deliberately does not touch ``identity``: ``rust_mirror_reset`` alone
    owns the raw handle registry, so other seams' handles survive a node
    reset. Activation is one-shot and kept across reset, like the mirror,
    and so is the serving mode: a reset must not silently stop serving.

    The serving counters follow the audit counters, not the store: a
    plain reset keeps them accumulating across builds, and only
    ``clear_counts`` zeroes them. Clearing them per build would make a
    whole-run reading come from the last build alone, which reads as "the
    channel never engaged" whenever the last build is a small one.

    It also restores the capture origin to `parse`: the readers restore it
    themselves in a `finally`, and the boundary makes a marker that somehow
    outlived its reader incapable of mis-attributing a later build.
    """
    global _capture_origin
    if _kernel_mod is not None:
        _kernel_mod.rust_node_mirror_reset()
        if clear_counts:
            _kernel_mod.rust_node_mirror_read_reset()
            _kernel_mod.rust_node_mirror_stmt_read_reset()
            _kernel_mod.rust_node_mirror_var_key_reset()
    _NODE_HANDLES.clear()
    # G2.4 (#1825): the per-build boundary restores the capture origin, so a
    # marker that outlived its reader cannot silently mis-attribute a later
    # build's records to a cache path.
    _capture_origin = ORIGIN_PARSE
    # G2.0 (#1577): drop the statement/def metadata store too.
    _reset_meta()
    _count("reset")
    if clear_counts:
        _audit.clear()


def report() -> dict[str, int]:
    """Return a copy of the audit counters."""
    return dict(_audit)


_STMT_SESSIONFINISH_ENV: Final = "MYPY_TK_STMT_SESSIONFINISH_OUT"


def stmt_sessionfinish() -> dict[str, object]:
    """The statement serving evidence of one finished session.

    Three sections: `stmt_read` (the Rust read counters of the statement
    serving channel), `capture` (the Python audit counters, non-empty only
    in audit mode) and `meta_entries` (the live metadata entry count of
    the session's last build, so the store's population cost is readable
    from the same dump rather than inferred).
    """
    entries = None
    if _kernel_mod is not None:
        entries = int(_kernel_mod.rust_node_mirror_meta_entry_count())
    return {"stmt_read": stmt_read_counters(), "capture": report(), "meta_entries": entries}


def stmt_sessionfinish_dump() -> None:
    """Write `stmt_sessionfinish` where the environment asks for it.

    Called from `mypy.build.build`'s finally, like the symtable mirror's
    `sessionfinish_dump`. No-op without `MYPY_TK_STMT_SESSIONFINISH_OUT`
    (`{pid}` expands to the process id) and without an activated shadow.
    Never raises: the call site sits in a `finally` whose job is to
    preserve the in-flight build exception.
    """
    out = os.environ.get(_STMT_SESSIONFINISH_ENV)
    if not out or _kernel_mod is None:
        return
    out = out.replace("{pid}", str(os.getpid()))
    try:
        with open(out, "w") as f:
            json.dump(stmt_sessionfinish(), f, indent=1, sort_keys=True)
    except Exception as err:
        # Same convention as the capture failures: account, never raise.
        print(f"native-nodes-mirror: stmt sessionfinish dump failed: {err}", file=sys.stderr)


_VAR_KEY_SESSIONFINISH_ENV: Final = "MYPY_TK_VAR_KEY_SESSIONFINISH_OUT"


def var_key_sessionfinish() -> dict[str, object]:
    """The `Var`-key translation evidence of one finished session.

    Two sections: `key_translation` (the Rust translation counters) and
    `capture` (the Python audit counters, non-empty only in audit mode).
    """
    return {"key_translation": var_key_counters(), "capture": report()}


def var_key_sessionfinish_dump() -> None:
    """Write `var_key_sessionfinish` where the environment asks for it.

    Same contract as `stmt_sessionfinish_dump`: called from `mypy.build.build`'s
    finally, a no-op without `MYPY_TK_VAR_KEY_SESSIONFINISH_OUT` (`{pid}`
    expands to the process id) and without an activated shadow, and never
    raises into the `finally`.
    """
    out = os.environ.get(_VAR_KEY_SESSIONFINISH_ENV)
    if not out or _kernel_mod is None:
        return
    out = out.replace("{pid}", str(os.getpid()))
    try:
        with open(out, "w") as f:
            json.dump(var_key_sessionfinish(), f, indent=1, sort_keys=True)
    except Exception as err:
        # Same convention as the capture failures: account, never raise.
        print(f"native-nodes-mirror: var key sessionfinish dump failed: {err}", file=sys.stderr)
