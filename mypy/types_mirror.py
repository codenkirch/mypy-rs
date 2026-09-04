"""Phase F1 dual-write shadow mirror (issue #1370).

Python stays canonical. With the mirror gate on, every construction and
attribute mutation of the four family classes (Instance, CallableType,
TypeVarType, UnionType) is mirrored into Rust storage behind the type
kernel's mirror pyfunctions, and each wire serialization of a family
object asserts that a fresh ``Type.write`` equals the stored blob. No
consumer reads the mirror in F1: it exists to prove Python's serialized
graph is byte-identical to what a Rust owner would have held.

Design notes (see crates/type_kernel/doc/f1_mirror.md):
- Capture is via class-level monkeypatching of ``__init__``/``__setattr__``
  /``write`` (never swapping class identity, so ``type(t) is Instance``
  keeps working). When the gate is off nothing is patched.
- Bytes are recomputed fresh on every assert; the mirror never trusts the
  ``_type_wire_cache`` (which can legally be stale for in-place-mutated
  types), and the funnel serialization runs with the wire cache disabled.
- Registration is lazy: objects enter the mirror at their first
  serialization funnel (``__init__`` capture only counts), because
  semanal constructs partial objects whose wire bytes cannot exist yet.
  Mutations before first registration define the per-object adoption
  baseline and are invisible in F1, exactly like mutations of
  non-family objects.
- Mirrored objects are pinned strongly until ``reset`` so a recycled
  ``id()`` cannot adopt a stale handle; escaped mutations (raw list ops
  on family fields) are detected at the next serialization funnel
  instead of at ``__setattr__``. TypeVarId writes are captured through
  their own shim plus a reverse map from tvid to carrier handles, and
  family leaves behind a non-family Type (e.g. a tuple fallback
  Instance under a TypeVarType's TupleType upper_bound) are closed
  over by `_HIDDEN_EMBED`, so a captured leaf write re-serializes the
  hidden family container too (issue #1372).
- A TypeAliasType embeds a live ``mypy.nodes.TypeAlias`` node by
  reference, whose ``_is_recursive`` flag is a wire input no family
  ``__setattr__`` sees. The flag gets its own capture shim plus a
  reverse map (alias node -> carriers), mirroring the TypeVarId shim.
- Mirror serialization is side-effect-free: ``_fresh_bytes`` suppresses
  the ``is_recursive`` None->bool lazy cache write via
  ``_REC_CACHE_SUPPRESSED`` and saves/restores its own re-entrancy flag,
  so a mirror read never flips a live node, and a later live flip still
  reaches the capture shim exactly once.
- A captured write may hit an only-later-serializable subtree (the
  semantics of pre-semanal partial objects), so the failing setattr
  defers its capture (``_PENDING_CAPTURE``): the next funnel that
  serializes the object cleanly resumes the capture and refreshes every
  stale parent blob via the normal cascade (``_flush_pending_capture``).
"""

from __future__ import annotations

import atexit
import json
import os
import sys
from collections import deque
from collections.abc import Iterator, Sequence
from typing import Any, Final, cast

from mypy.types import CallableType, Instance, Type, TypeVarId, TypeVarType, UnionType

FAMILY_CLASSES: Final = (Instance, CallableType, TypeVarType, UnionType)
FAMILY_NAME: Final[dict[type, str]] = {
    Instance: "instance",
    CallableType: "callable",
    TypeVarType: "tvar",
    UnionType: "union",
}
# TypeVarId is not a Type: embedded in TypeVarLikeType.id, invisible to the
# family parents graph, and a meta_level write fires no family __setattr__.
# It gets its own capture shim plus a reverse map (TypeVarId -> carriers).
TVID_CLASSES: Final = (TypeVarId,)
# Attributes written by hashing, lazy truthiness init, or Context.set_line
# never affect the wire bytes of any family class (no types.py write() body
# reads line / column / end_line / end_column / definition); skip them.
SKIP_ATTRS: Final = frozenset(
    {
        "_hash",
        "_can_be_true",
        "_can_be_false",
        # Derived truthiness facets; the wire recomputes them on read.
        "can_be_true",
        "can_be_false",
        # Context bookkeeping and definition: never part of the wire bytes.
        "line",
        "column",
        "end_line",
        "end_column",
        "definition",
    }
)


_kernel_mod: Any = None
# Guard against recursion: while our own fresh serialization runs, all
# mirror work is a no-op.
_in_serialize = False
# Depths of family __init__ wrappers currently in flight: __setattr__
# calls made during construction are init-capture work, not mutations,
# and the object is still a partial blob that cannot serialize.
_construction: int = 0
_active = False
_strict = False
_audit_mode = False
# Phase F2 (#1393): when True (and the mirror is active), kernel wire-seam
# funnels read a registered family object's blob from Rust mirror storage
# instead of re-serializing the live object.
_read_mode = False
# Phase F3 (#1397): an Instance write for one of these field names captured
# by _mirror_setattr is pushed into the stored blob by the per-field Rust
# splice op; a splice that drifts surfaces at the next serial funnel.
_write_flip = False
_FLIP_FIELDS: Final[frozenset[str]] = frozenset({"args", "type", "last_known_value"})
_ORIG_SETATTR: Any = None
# family class -> saved originals (for a future multi-run protocol).
_originals: dict[type, dict[str, Any]] = {}
# handle -> live object (strong pin until reset).
_BY_HANDLE: dict[int, Any] = {}
# Registered objects whose captured write could not be serialized at
# mutation time; the capture is deferred to a later funnel that drains
# them (see `_drain_pending_captures`). Bounded FIFO like the strike memo.
_PENDING_CAPTURE: dict[int, Any] = {}
_PENDING_CAPTURE_Q: deque[int] = deque()
_audit: dict[str, int] = {}
_mismatch_examples: dict[str, str] = {}
# Cross-reset accumulators: mirror-suite teardowns call reset(clear_counts=True)
# mid-process, which must not erase counters not yet dumped at atexit.
_audit_total: dict[str, int] = {}
_mismatch_total: dict[str, str] = {}
# TypeVarId capture state (see TVID_CLASSES). `_TVID_CONSTRUCTION` counts
# TypeVarId.__init__ calls in flight so construction writes are never captured;
# `_TVID_REVERSE` pins the tvid (id() stays valid until reset() per build).
_TVID_CONSTRUCTION: int = 0
_TVID_REVERSE: dict[int, tuple[TypeVarId, set[int]]] = {}
# TypeAlias capture state (see the module docstring). `_ALIAS_REVERSE`
# pins the alias node (id() stays valid until reset() per build).
_ALIAS_REVERSE: dict[int, tuple[Any, set[int]]] = {}
# Hidden-parent closure: a family leaf behind a non-family Type (e.g. TupleType
# upper_bound) has no parents-graph edge, so `_HIDDEN_EMBED[id(leaf)]` -> (leaf,
# container handles) pins the leaf and supplies ancestors hidden from cascades.
_HIDDEN_EMBED: dict[int, tuple[Any, set[int]]] = {}
# Slots whose captured replacement can introduce or orphan hidden embeds: these
# setattrs re-index the root (parents are not: a same-slot subtree swap cannot
# hide a visible ancestor chain); other setattrs skip it, funnels keep full walk.
_REINDEX_SLOTS: Final[frozenset[str]] = frozenset(
    {
        "args",
        "items",
        "variables",
        "arg_types",
        "ret_type",
        "upper_bound",
        "values",
        "default",
        "fallback",
        "last_known_value",
        "original_str_expr",
        "original_str_fallback",
        "extra_attrs",
        "instance_type",
        "type_guard",
        "type_is",
    }
)

# Objects and TypeVarId carriers: bounded FIFO with the same cap. The
# pending-capture dict + deque pair stays cold in the battery, so it keeps
# the simple structures (the drain walk needs the stored objects).
_PENDING_CAPTURE_CAP: Final = 65536


class _IdFifo:
    """Bounded FIFO set of unique ids with O(1) membership and removal.

    Replaces a ``set`` guarded by a cap-evicting ``deque``: clearing a
    struck id dequeues with ``.remove`` which scans O(cap) on a full
    queue, and at a 65,536-entry cap with ~450k clears per self-check
    that scan was the mirror's dominant production cost. Membership is
    the dict (O(1) ``in`` / remove); eviction order rides an append-only
    (token, key) log plus a cursor that lazily skips stale entries. A
    re-added id is inserted with a fresh token, so the evict never drops
    a live youngest entry early, matching ``deque.remove`` + ``append``.
    """

    __slots__ = ("_cap", "_members", "_log", "_cursor", "_seq")

    def __init__(self, cap: int) -> None:
        self._cap = cap
        # key -> token of the newest live insertion (tokens are >= 1).
        self._members: dict[int, int] = {}
        self._log: list[tuple[int, int]] = []
        self._cursor = 0
        self._seq = 0

    def __contains__(self, key: object) -> bool:
        return key in self._members

    def __len__(self) -> int:
        return len(self._members)

    def __bool__(self) -> bool:
        return bool(self._members)

    def add(self, key: int) -> None:
        if key in self._members:
            # Duplicate insertions are guarded by the callers; free here.
            return
        if len(self._members) >= self._cap:
            self._evict_oldest()
        self._seq += 1
        self._members[key] = self._seq
        self._log.append((self._seq, key))
        self._maybe_compact()

    def remove(self, key: int) -> bool:
        return self._members.pop(key, None) is not None

    def _evict_oldest(self) -> None:
        # Live members are always present in the log, so the cursor
        # walks at most the stale entries that accumulated since.
        while True:
            token, key = self._log[self._cursor]
            self._cursor += 1
            if self._members.get(key) == token:
                del self._members[key]
                return

    def _maybe_compact(self) -> None:
        # Live members have exactly one live log entry each, so any surplus
        # is stale. Gating on stale count (not the cursor, which only advances
        # during evicts at full membership) keeps sub-cap churn bounded.
        if len(self._log) - len(self._members) >= self._cap:
            self._log = [
                entry
                for entry in self._log[self._cursor :]
                if self._members.get(entry[1]) == entry[0]
            ]
            self._cursor = 0

    def clear(self) -> None:
        self._members.clear()
        self._log = []
        self._cursor = 0
        self._seq = 0


# Objects whose adoption already failed (pre-semanal partials the wire
# cannot serialize). Bounded FIFO by id(): a recycled id retries once
# more later; the write funnel stays the authoritative registration point.
_ADOPT_STRIKE: _IdFifo = _IdFifo(65536)


# Unprotected-write epoch (F3 slice 6, #1397) for REGISTERED family objects:
# an uncaptured mutation bumps it and each handle's next funnel re-verifies.
# Never-registered writes bump nothing: no stored blob embeds such an object.
_UNPROT_EPOCH: int = 0


def _bump_unprot(origin: str = "") -> None:
    global _UNPROT_EPOCH
    _UNPROT_EPOCH += 1
    _count("unprot_bump")
    if origin:
        _count("unprot_bump." + origin)


def _note_failed_adoption(obj: Any) -> None:
    _ADOPT_STRIKE.add(id(obj))


def _note_successful_adoption(obj: Any) -> None:
    if _ADOPT_STRIKE.remove(id(obj)):
        _count("strike_cleared_on_adopt")


def _note_failed_capture(obj: Any) -> None:
    key = id(obj)
    if key in _PENDING_CAPTURE:
        _count("pending_already")
        return
    if len(_PENDING_CAPTURE) >= _PENDING_CAPTURE_CAP:
        old = _PENDING_CAPTURE_Q.popleft()
        _PENDING_CAPTURE.pop(old, None)
    _PENDING_CAPTURE[key] = obj
    _PENDING_CAPTURE_Q.append(key)


def _flush_pending_capture(obj: Any, handle: int, fresh: bytes) -> None:
    key = id(obj)
    if key not in _PENDING_CAPTURE:
        return
    _PENDING_CAPTURE.pop(key, None)
    _PENDING_CAPTURE_Q.remove(key)
    _count("pending_capture_flushed")
    _update_and_cascade(handle, fresh, None)


def _flush_pending_embed(embed: Type) -> None:
    """Close the flush-before-adoption hole (issue #1385).

    A pending capture's drain-cascade only reaches the container edges
    that exist when the drain runs. When a later registration or
    re-index adopts a still-pending embed, the flush fired earlier (or
    has not fired yet at an ancestor) and the adopter keeps a blob of
    the embed's pre-drain bytes. Creating that edge therefore flushes
    the pending capture immediately, so the cascade runs with the edge
    already in place. The pending entry is dropped before cascading so
    a re-entrant edge adoption cannot recurse on the same capture.
    """
    key = id(embed)
    if key not in _PENDING_CAPTURE:
        return
    _PENDING_CAPTURE.pop(key, None)
    _PENDING_CAPTURE_Q.remove(key)
    h = _handle_of(embed)
    if h is None:
        # Unregistered (evicted): its next funnel re-registers fresh.
        _count("pending_capture_flushed_unregistered")
        return
    _count("pending_capture_flushed")
    try:
        fresh = _fresh_bytes(embed)
    except Exception:
        _count("pending_capture_still_unserializable")
        _note_failed_capture(embed)
        _bump_unprot("pending_recover")
        return
    _update_and_cascade(h, fresh, None)


def _drain_pending_captures() -> None:
    """Resume captures that once failed serialization (issue #1385).

    A write on an already-registered object may mutate a subtree that is
    still only serializable later (e.g. calculate_tuple_fallback's
    in-place args rewrite on the shared fallback Instance). The failing
    setattr enqueues the object; the first later funnel that serializes
    it cleanly resyncs the stale parent blobs through the normal
    cascade. Called at the top of `_assert_fresh` / `_check_splice`
    while the map is non-empty, so an asserting container also drains
    its pended ancestors before its own bytes are judged.
    """
    for key in list(_PENDING_CAPTURE):
        # A re-entrant flush (`_flush_pending_embed` through a cascade's
        # `_apply_hidden_embeds`) may have drained this entry already.
        obj = _PENDING_CAPTURE.pop(key, None)
        if obj is None:
            continue
        try:
            _PENDING_CAPTURE_Q.remove(key)
        except ValueError:
            pass
        h = _handle_of(obj)
        try:
            fresh = _fresh_bytes(obj)
        except Exception:
            # Still unserializable: keep it queued for a later funnel.
            _count("pending_capture_still_unserializable")
            _note_failed_capture(obj)
            _bump_unprot("pending_drain")
            continue
        if h is None:
            # Evicted or never adopted; its first funnel will register it.
            _count("pending_capture_flushed_unregistered")
            continue
        _count("pending_capture_flushed")
        _update_and_cascade(h, fresh, None)


# ---- _short_stack: mismatch diagnostics (production audit mode) ----


def _short_stack() -> str:
    # Only called on a mismatch, so formatting cost is bounded. Keep both
    # ends: the shallow frames name the mutation origin, the deepest
    # frames name the funnel that detected the drift.
    import traceback as _tb

    frames = _tb.format_stack()[:-2]
    if len(frames) <= 24:
        return "".join(frames)
    return "".join([*frames[:12], "\n    ...\n", *frames[-12:]])


def _count(key: str, n: int = 1) -> None:
    _audit[key] = _audit.get(key, 0) + n


def _note_mismatch(key: str, msg: str) -> None:
    _mismatch_examples.setdefault(key, msg)


def _wire_cache_enabled() -> bool:
    from mypy.types import _type_wire_cache_enabled

    return _type_wire_cache_enabled


# ---- child walking ----

# Slot names that never hold a Type child (positions plus private state).
_SKIP_SLOTS: Final = frozenset({"line", "column", "end_line", "end_column"})

# MRO slot-name tuples are input-invariant; the family classes plus every
# non-family Type walked by the embed/carrier helpers hit this millions
# of times per run, so re-collecting was a measured top cost.
_SLOT_NAMES: dict[type, tuple[str, ...]] = {}


def _type_names(cls: type) -> tuple[str, ...]:
    """Collect all __slots__ names across the MRO, deduplicated in order."""
    names = _SLOT_NAMES.get(cls)
    if names is not None:
        return names
    collected: list[str] = []
    for klass in cls.__mro__:
        slots = getattr(klass, "__slots__", ())
        if slots:
            collected.extend(slots)
    names = tuple(dict.fromkeys(collected))
    _SLOT_NAMES[cls] = names
    return names


def _child_types(t: Type) -> Iterator[tuple[str, Type]]:
    """Yield (site, child) for every Type reachable from `t`'s slots."""
    for name in _type_names(type(t)):
        if name.startswith("_") or name in _SKIP_SLOTS:
            continue
        try:
            value = object.__getattribute__(t, name)
        except AttributeError:
            continue
        yield from _child_types_in_value(value, name)


def _child_types_in_value(value: Any, site: str) -> Iterator[tuple[str, Type]]:
    """Descend a slot value into containers, yielding each Type found."""
    if value is None or isinstance(value, (str, bytes, bool, int, float)):
        return
    if isinstance(value, Type):
        yield (site, value)
        return
    if isinstance(value, (list, tuple)):
        for i, item in enumerate(value):
            yield from _child_types_in_value(item, f"{site}[{i}]")
        return
    if isinstance(value, dict):
        for item in value.values():
            yield from _child_types_in_value(item, site)
        return
    # ExtraAttrs carries a dict of Types but is not a Type itself.
    if type(value).__name__ == "ExtraAttrs":
        yield from _child_types_in_value(value.attrs, f"{site}.attrs")


def _walk_slots(
    t: Type, seen: set[int], tvids: list[TypeVarId], aliases: list[Any], embeds: list[Type]
) -> None:
    """Slot-scan one Type node once for every reverse index."""
    for name in _type_names(type(t)):
        if name.startswith("_") or name in _SKIP_SLOTS:
            continue
        try:
            value = object.__getattribute__(t, name)
        except AttributeError:
            continue
        _walk_value(value, seen, tvids, aliases, embeds)


def _walk_value(
    value: Any, seen: set[int], tvids: list[TypeVarId], aliases: list[Any], embeds: list[Type]
) -> None:
    """Descend a slot value collecting TypeVarIds, TypeAlias nodes, and
    family Types (the walk root is pre-seeded in `seen`, so it is never
    a member).

    Fuses the former `_tvids_in` / `_alias_nodes_in` / `_family_embeds`
    triple: all three descend the same slots of the same Types, so the
    fused walk pays `_type_names` plus `object.__getattribute__` once per
    node instead of three times. Container ids and Type nodes share one
    `seen` set; a node reached by any intent is visited once and
    collects for all three intents, which covers everything each
    separate walk visited (each walker's own `seen` already pruned
    repeat visits within its own pass).
    """
    if value is None or isinstance(value, (str, bytes, bool, int, float)):
        return
    if isinstance(value, TypeVarId):
        tvids.append(value)
        return
    if isinstance(value, Type):
        if id(value) not in seen:
            seen.add(id(value))
            if type(value) in FAMILY_NAME:
                embeds.append(value)
            _walk_slots(value, seen, tvids, aliases, embeds)
        return
    if type(value).__name__ == "TypeAlias":
        if id(value) not in seen:
            seen.add(id(value))
            aliases.append(value)
            target = getattr(value, "target", None)
            if target is not None:
                _walk_value(target, seen, tvids, aliases, embeds)
        return
    if isinstance(value, (list, tuple)):
        seen.add(id(value))
        for item in value:
            _walk_value(item, seen, tvids, aliases, embeds)
        return
    if isinstance(value, dict):
        seen.add(id(value))
        for item in value.values():
            _walk_value(item, seen, tvids, aliases, embeds)
        return
    if type(value).__name__ == "ExtraAttrs":
        _walk_value(value.attrs, seen, tvids, aliases, embeds)


def _walk_indices(root: Type) -> tuple[list[TypeVarId], list[Any], list[Type]]:
    """One traversal yielding every carrier/embed index item from `root`.

    Returns (tvids, alias_nodes, family_embeds_excluding_root), ordered
    like the separate walks each produced.
    """
    tvids: list[TypeVarId] = []
    aliases: list[Any] = []
    embeds: list[Type] = []
    _walk_slots(root, {id(root)}, tvids, aliases, embeds)
    return tvids, aliases, embeds


def _apply_tvid_carriers(tvids: list[TypeVarId], handle: int) -> None:
    """Index every TypeVarId item under its carrier handle."""
    for tvid in tvids:
        entry = _TVID_REVERSE.get(id(tvid))
        if entry is None:
            _TVID_REVERSE[id(tvid)] = (tvid, {handle})
        else:
            entry[1].add(handle)


def _apply_alias_carriers(alias_nodes: list[Any], handle: int) -> None:
    """Index every TypeAlias node item under its carrier handle."""
    for alias_node in alias_nodes:
        entry = _ALIAS_REVERSE.get(id(alias_node))
        if entry is None:
            _ALIAS_REVERSE[id(alias_node)] = (alias_node, {handle})
        else:
            entry[1].add(handle)


def _apply_hidden_embeds(embeds: list[Type], handle: int) -> None:
    """Index family-embed items under `handle`, flushing pending captures.

    Flush (not collection) is the side-effectful part; callers sequence
    it after their own `rust_mirror_update` so the flush's re-entrant
    cascade sees the same edge state the separate-walk code produced.
    """
    for embed in embeds:
        key = id(embed)
        entry = _HIDDEN_EMBED.get(key)
        if entry is None:
            _HIDDEN_EMBED[key] = (embed, {handle})
            _flush_pending_embed(embed)
        else:
            entry[1].add(handle)
            _flush_pending_embed(embed)


# ---- fresh serialization ----


def _fresh_bytes(t: Type) -> bytes:
    """Serialize `t` to fresh bytes with the wire cache disabled."""
    global _in_serialize
    from librt.internal import WriteBuffer

    import mypy.types as _types_mod
    from mypy.types import _set_type_wire_cache_enabled as _set

    prev = _wire_cache_enabled()
    _set(False)
    prev_serialize = _in_serialize
    prev_cache = _types_mod._REC_CACHE_SUPPRESSED
    _in_serialize = True
    # The is_recursive property caches None -> bool on the live node: inside a
    # mirror serialization that write would flip a node the alias capture hook
    # treats as observed (stale-byte drift). Value-unchanged, module-object write.
    _types_mod._REC_CACHE_SUPPRESSED = True
    try:
        buf = WriteBuffer()
        t.write(buf)
        return buf.getvalue()
    finally:
        _in_serialize = prev_serialize
        _types_mod._REC_CACHE_SUPPRESSED = prev_cache
        _set(prev)


def _args_list_bytes(args: Sequence[Type]) -> bytes | None:
    """Serialize only an Instance args tuple as `write_type_list` bytes.

    Same suppression trio as `_fresh_bytes` (wire cache off, re-entrancy and
    recursive-alias-cache guards), so each item's `write` is side-effect-free
    and never re-enters the mirror. Returns None when an item cannot
    serialize yet; the splice then defers to the full fresh path.
    """
    global _in_serialize
    from librt.internal import WriteBuffer

    import mypy.types as _types_mod
    from mypy.types import _set_type_wire_cache_enabled as _set, write_type_list

    prev = _wire_cache_enabled()
    _set(False)
    prev_serialize = _in_serialize
    prev_cache = _types_mod._REC_CACHE_SUPPRESSED
    _in_serialize = True
    _types_mod._REC_CACHE_SUPPRESSED = True
    try:
        buf = WriteBuffer()
        write_type_list(buf, args)
        return buf.getvalue()
    except Exception:
        return None
    finally:
        _in_serialize = prev_serialize
        _types_mod._REC_CACHE_SUPPRESSED = prev_cache
        _set(prev)


def _single_type_bytes(t: Type) -> bytes | None:
    """Serialize one Type subtree (Instance lkv splice input, #1397).

    Same suppression trio as `_args_list_bytes`: wire cache off,
    re-entrancy and recursive-alias-cache guards, so the `write` is
    side-effect-free and never re-enters the mirror. Returns None when
    the value cannot serialize yet; the splice then defers to the full
    fresh path.
    """
    global _in_serialize
    from librt.internal import WriteBuffer

    import mypy.types as _types_mod
    from mypy.types import _set_type_wire_cache_enabled as _set

    prev = _wire_cache_enabled()
    _set(False)
    prev_serialize = _in_serialize
    prev_cache = _types_mod._REC_CACHE_SUPPRESSED
    _in_serialize = True
    _types_mod._REC_CACHE_SUPPRESSED = True
    try:
        buf = WriteBuffer()
        t.write(buf)
        return buf.getvalue()
    except Exception:
        return None
    finally:
        _in_serialize = prev_serialize
        _types_mod._REC_CACHE_SUPPRESSED = prev_cache
        _set(prev)


# ---- registration / adoption / cascade ----


def _handle_of(t: Any) -> int | None:
    return cast("int | None", _kernel_mod.rust_mirror_handle_of(t))


def read_fresh_bytes(t: Type) -> bytes | None:
    """Phase F2 (#1393): mirror-blob bytes for a registered family Type.

    Kernel wire-seam funnels (slice 1: checkexpr) swap their cache-miss
    serialization for this read. Returns None (caller serializes as before)
    when read mode is off, the mirror is not active, or the object was never
    registered. The blob is kept fresh by the F1 capture invariant: strict
    mode raises at the mutation funnel on any drift, so a live object leaf
    other than a captured-mutation lag cannot be served here.
    """
    if not _read_mode or not _active or _in_serialize:
        return None
    h = _handle_of(t)
    if h is None:
        return None
    blob = _kernel_mod.rust_mirror_bytes(h)
    if blob is None:
        return None
    return bytes(blob)


def _register_tree(t: Type) -> int | None:
    """Register `t` (family only) with its family children, recursively.

    Returns the handle or None if the object could not be registered.
    """
    fam = FAMILY_NAME.get(type(t))
    if fam is None:
        return None
    h = _handle_of(t)
    if h is not None:
        return h
    # Prior hidden-embed adopters: containers that already indexed `t` while
    # it was unregistered. Their stored blobs predate any in-place mutation
    # of `t` that happened between adoption and this registration.
    prior_adopters = _HIDDEN_EMBED.get(id(t)) is not None
    child_handles = []
    for _site, child in _child_types(t):
        if type(child) in FAMILY_NAME:
            ch = _register_tree(child)
            if ch is not None:
                child_handles.append(ch)
    try:
        fresh = _fresh_bytes(t)
    except Exception as exc:
        _count("unserializable." + fam)
        _mismatch_examples.setdefault(f"unserializable.{fam}", f"{exc!r}")
        return None
    handle = _kernel_mod.rust_mirror_register(t, fam, fresh, child_handles)
    _BY_HANDLE[handle] = t
    # A later successful adoption clears the failed one: the strike memo is
    # only about "cannot adopt yet", not "never capture writes again".
    _note_successful_adoption(t)
    _kernel_mod.rust_mirror_stamp_sync(handle, _UNPROT_EPOCH)
    tvids, aliases, embeds = _walk_indices(t)
    _apply_tvid_carriers(tvids, handle)
    _apply_alias_carriers(aliases, handle)
    _apply_hidden_embeds(embeds, handle)
    _flush_pending_capture(t, handle, fresh)
    if prior_adopters:
        # Late adoption cascade (issue #1385): a container adopted this
        # object while unregistered and unreachable by any earlier
        # cascade; re-sync its prior adopters now (`""` = no re-index).
        _update_and_cascade(handle, fresh, "")
    return handle  # type: ignore[no-any-return]


def _sync_seed_handles(obj: Type) -> Iterator[int]:
    """Container handles beyond the kernel parents graph.

    Kernel parents cover family-child containment; `_HIDDEN_EMBED` also
    handles family containers that embed `obj`'s bytes through a
    non-family Type, invisible to the kernel graph.
    """
    entry = _HIDDEN_EMBED.get(id(obj))
    if entry is not None and entry[0] is obj:
        yield from entry[1]


def _update_and_cascade(handle: int, fresh: bytes, replaced_slot: str | None = None) -> None:
    """Write `fresh` for `handle`, then re-sync parents containing it.

    `replaced_slot` controls the hidden-embed re-index: `None` means the
    callers' unknown path (funnel or splice mismatch) and re-indexes the
    root and every loop parent, the blind mode; a slot name means a
    captured setattr through `_mirror_setattr`; a same-slot subtree
    cannot hide a previously visible chain in an ancestor, so the root
    gets the re-index only for `_REINDEX_SLOTS` slots and loop parents
    are skipped entirely. `""` (the TypeVarId carrier path) saw no slot
    replacement and re-indexes nothing.
    """
    # A captured family setattr may have swapped an id slot, introducing
    # TypeVarIds no funnel ever adopted; re-index the root when the slot could
    # carry embeds (`_apply_hidden_embeds` keeps replaced subtrees reachable).
    skip_all = replaced_slot == ""
    root_reindex = replaced_slot is None or (not skip_all and replaced_slot in _REINDEX_SLOTS)
    stamp_epoch = _UNPROT_EPOCH
    stack = list(_kernel_mod.rust_mirror_parents(handle))
    obj = _BY_HANDLE.get(handle)
    if obj is not None:
        stack.extend(_sync_seed_handles(obj))
    seen = {handle}
    _kernel_mod.rust_mirror_update(handle, fresh)
    _kernel_mod.rust_mirror_stamp_sync(handle, stamp_epoch)
    if obj is not None:
        tvids, aliases, embeds = _walk_indices(obj)
        _apply_tvid_carriers(tvids, handle)
        _apply_alias_carriers(aliases, handle)
        if root_reindex:
            _apply_hidden_embeds(embeds, handle)
    while stack:
        ph = stack.pop()
        if ph in seen:
            continue
        seen.add(ph)
        parent = _BY_HANDLE.get(ph)
        if parent is None:
            _count("cascade_missing_parent")
            continue
        _count("cascade_sync")
        tvids, aliases, embeds = _walk_indices(parent)
        _apply_tvid_carriers(tvids, ph)
        _apply_alias_carriers(aliases, ph)
        try:
            pfresh = _fresh_bytes(parent)
        except Exception:
            _count("cascade_unserializable")
            _bump_unprot("cascade")
            continue
        _kernel_mod.rust_mirror_update(ph, pfresh)
        _kernel_mod.rust_mirror_stamp_sync(ph, stamp_epoch)
        if replaced_slot is None:
            _apply_hidden_embeds(embeds, ph)
        stack.extend(_kernel_mod.rust_mirror_parents(ph))
        stack.extend(_sync_seed_handles(parent))


def _drift_message(t: Type, fam: str, site: str, handle: int, fresh: bytes) -> str:
    """Full diagnostic for a strict-mode drift: hex blobs and decoded forms.

    Only called on a real mismatch, so its cost is irrelevant to the
    funnels' hot paths.
    """
    import type_kernel as _k

    old_b = _kernel_mod.rust_mirror_bytes(handle)
    try:
        old_s = _k.read_type_to_str(bytes(old_b))[:600]
    except Exception as err:
        old_s = f"<decode failed: {err}>"
    try:
        new_s = _k.read_type_to_str(fresh)[:600]
    except Exception as err:
        new_s = f"<decode failed: {err}>"
    fields = {
        k: repr(getattr(t, k))[:300]
        for k in ("type_ref", "args", "line", "column")
        if hasattr(t, k)
    }
    fields["self_id"] = f"{id(t):x}"
    text = (
        f"native-type-mirror ({fam}, {site}) drifted\n"
        f"type={type(t).__name__!r} fields={fields}\n"
        f"OLDHEX={bytes(old_b).hex()}\nNEWHEX={fresh.hex()}\n"
        f"OLD={old_s}\nNEW={new_s}"
    )
    return text


def _assert_fresh(t: Type, site: str) -> None:
    """Assert the mirror blob matches fresh live bytes for a family type.

    On mismatch: strict mode raises AssertionError; audit mode counts and
    re-syncs plus cascades, so an escaped mutation triggers exactly once
    here before the mirror state catches up.
    """
    if _PENDING_CAPTURE:
        _drain_pending_captures()
    fam = FAMILY_NAME[type(t)]
    h = _handle_of(t)
    if h is not None and _kernel_mod.rust_mirror_write_skip(h, _UNPROT_EPOCH):
        # Nothing unprotected has drifted this object's subtree since its
        # blob last synced at the current epoch: the fresh walk would
        # re-derive identical bytes.
        _count(f"assert_skip.{fam}.{site}")
        return
    try:
        fresh = _fresh_bytes(t)
    except Exception:
        # A partial object the real write would also fail on; let the
        # unpatched serialization surface its own original error.
        _count(f"unserializable.{fam}.funnel:{site}")
        return
    if h is None:
        _count(f"adopt.{fam}.{site}")
        _register_tree(t)
        return
    if _mirror_expect_ok(h, fresh):
        _kernel_mod.rust_mirror_stamp_sync(h, _UNPROT_EPOCH)
        _count(f"assert_ok.{fam}.{site}")
        _flush_pending_capture(t, h, fresh)
        return
    key = f"mismatch.{fam}.{site}"
    _count(key)
    if _strict:
        raise AssertionError(_drift_message(t, fam, site, h, fresh))
    if _audit_mode:
        _note_mismatch(key, _short_stack())
    _update_and_cascade(h, fresh)
    if _audit_mode and not _mirror_expect_ok(h, fresh):
        # The parents graph cannot reach whatever embeds these bytes:
        # an edge is missing, not just a captured mutation that ran late.
        _count(f"cascade_failed.{fam}.{site}")
        _mismatch_examples.setdefault(
            f"cascade_failed.{fam}.{site}", _drift_message(t, fam, f"{site}+after", h, fresh)
        )


def _mirror_expect_ok(handle: int, fresh: bytes) -> bool:
    """Run rust_mirror_expect; returns False on mismatch (and notes it)."""
    try:
        _kernel_mod.rust_mirror_expect(handle, fresh)
        return True
    except ValueError as err:
        _note_mismatch_key(handle, fresh, str(err))
        return False


def _check_splice(t: Type, blob: bytes) -> None:
    """Funnel for wire-cache splice hits: the splice never reaches t.write.

    In-place mutations between caching and a splice would serve stale
    bytes silently. Compare the spliced blob to the family object's live
    bytes; on drift count (and, strict mode, raise), resync the mirror,
    and drop the stale cache entry so the next serialization re-caches
    fresh bytes and each escape fires once.
    """
    if _PENDING_CAPTURE:
        _drain_pending_captures()
    if not _rules_ok(t):
        return
    fam = FAMILY_NAME[type(t)]
    try:
        fresh = _fresh_bytes(t)
    except Exception:
        _count(f"unserializable.{fam}.cachedsplice")
        return
    h = _handle_of(t)
    if h is None:
        _count(f"adopt.{fam}.cachedsplice")
        _register_tree(t)
    elif not _mirror_expect_ok(h, fresh):
        key = f"mismatch.{fam}.cachedsplice"
        _count(key)
        if _audit_mode:
            _note_mismatch(key, _short_stack())
        if _strict:
            raise AssertionError(f"native-type-mirror ({fam}, cachedsplice) drifted")
        _update_and_cascade(h, fresh)
    elif fresh == blob:
        _count(f"assert_ok.{fam}.cachedsplice")
        return
    if fresh == blob:
        return
    key = f"stale.{fam}.cachedsplice"
    _count(key)
    if _audit_mode:
        _note_mismatch(key, _short_stack())
    if _strict:
        raise AssertionError(f"native-type-mirror ({fam}, cachedsplice) stale spliced bytes")
    from mypy.types import _type_wire_cache

    _type_wire_cache.pop(id(t), None)


def _note_mismatch_key(handle: int, fresh: bytes, msg: str) -> None:
    obj = _BY_HANDLE.get(handle)
    fam = FAMILY_NAME[type(obj)] if obj is not None and type(obj) in FAMILY_NAME else "?"
    _mismatch_examples.setdefault(f"expect.{fam}", f"{msg[:400]} (fresh len {len(fresh)})")


# ---- class patching ----


def _rules_ok(t: Type) -> bool:
    # Single cheap gate for every funnel invocation.
    return _active and not _in_serialize and type(t) in FAMILY_NAME


def _make_write_wrapper(orig_write: Any, family: str) -> Any:
    def write(self: Type, data: Any) -> None:
        if _rules_ok(self):
            _assert_fresh(self, "write")
        orig_write(self, data)

    write.__name__ = f"mirror_{family}_write"
    return write


def _make_init_wrapper(orig_init: Any, family: str) -> Any:
    def init(self: Type, *args: Any, **kwargs: Any) -> None:
        global _construction
        _construction += 1
        try:
            orig_init(self, *args, **kwargs)
        finally:
            _construction -= 1
        # Capture only counts here. Registration is lazy: semanal needs
        # partial fallbacks that cannot serialize yet, so every object
        # enters the mirror at its first serialization funnel instead.
        if _rules_ok(self):
            _count(f"init.{family}")

    init.__name__ = f"mirror_{family}_init"
    return init


def _mirror_setattr(self: Type, name: str, value: Any) -> None:
    hook = _rules_ok(self) and _construction == 0 and name not in SKIP_ATTRS
    if not hook and _active and type(self) in FAMILY_NAME and name not in SKIP_ATTRS:
        # Suppression-window setattr on a REGISTERED object: blobs embedding
        # these bytes were not captured, so force their next funnel to
        # verify. Unregistered targets cannot corrupt storage (see below).
        if _handle_of(self) is not None:
            _bump_unprot("setattr_window")
    if hook and id(self) in _ADOPT_STRIKE:
        # The strike memo is per-object id: a write on an object that has
        # since become registrable is a real mirror mutation (capture it);
        # only still-unregistrable objects stay gagged.
        if _handle_of(self) is not None:
            _ADOPT_STRIKE.remove(id(self))
            _count("strike_captured_late")
        else:
            hook = False
            # Still gagged: the raw write lands uncaptured, but no stored
            # blob can embed a struck object (deriving through it fails
            # serialization), so no epoch bump.
            _count("strike_gag_uncaptured")
    h = None
    if hook:
        h = _handle_of(self)
        if h is None:
            h = _register_tree(self)
            if h is None:
                # A partial object fresh serialization cannot handle yet
                # (unfilled semanal fallback). Memo the failure so the
                # per-setattr retry storm stops until the write funnel.
                fam = FAMILY_NAME[type(self)]
                _count(f"setattr_gagged.{fam}")
                _note_failed_adoption(self)
                # Uncaptured write on a never-registered object: safe without
                # an epoch bump (no stored blob derives through it until it
                # serializes; see the `_UNPROT_EPOCH` comment).
                _count("register_fail_uncaptured")
                # The mirror cannot bind this object, but the mutation is
                # real: apply it as the unpatched __setattr__ would, skipping
                # only capture bookkeeping (a dropped write lost .name here).
                _ORIG_SETATTR(self, name, value)
                return
    _ORIG_SETATTR(self, name, value)
    if not hook:
        return
    fam = FAMILY_NAME[type(self)]
    if h is None:
        _count("post_setattr_unregistered." + fam)
        return
    if _write_flip and type(self) is Instance and name in _FLIP_FIELDS:
        # F3 (#1397): push the changed field into the stored blob via the
        # Rust splice op (decode + swap + re-encode) instead of Python
        # re-serialization. A None defers to the full fresh path.
        hidden_stored = _kernel_mod.rust_mirror_bytes(h)
        if name == "args":
            args_blob = _args_list_bytes(self.args)
            if args_blob is not None:
                new_blob = _kernel_mod.rust_mirror_patch_instance_args(h, args_blob)
            else:
                new_blob = None
        elif name == "type":
            # The wire only carries `type.fullname` (types.py:1941).
            try:
                new_ref = self.type.fullname
            except Exception:
                new_ref = None
            if new_ref is not None:
                new_blob = _kernel_mod.rust_mirror_patch_instance_type(h, new_ref)
            else:
                new_blob = None
        else:  # last_known_value
            if value is None:
                # A None write is the write_type_opt LITERAL_NONE clear.
                new_blob = _kernel_mod.rust_mirror_patch_instance_lkv(h, None)
            else:
                lkv_blob = _single_type_bytes(value)
                if lkv_blob is not None:
                    new_blob = _kernel_mod.rust_mirror_patch_instance_lkv(h, lkv_blob)
                else:
                    new_blob = None
        if new_blob is not None:
            new_bytes = bytes(new_blob)
            if new_bytes == bytes(hidden_stored or b""):
                _count(f"setattr_noop.{fam}.{name}")
            else:
                _count(f"setattr_spliced.{fam}.{name}")
                _update_and_cascade(h, new_bytes, name)
            # Either branch leaves the stored blob authoritative.
            _kernel_mod.rust_mirror_stamp_sync(h, _UNPROT_EPOCH)
            return
        _count(f"setattr_splice_defer.{fam}.{name}")
    try:
        fresh = _fresh_bytes(self)
    except Exception:
        # Serialization can fail on a partially-built object; defer the
        # capture to the next successful funnel, which re-syncs the stale
        # parent blobs (tuple fallback args rewrite, issue #1385).
        _count("unserializable." + fam + ".setattr:" + name)
        if _audit_mode:
            _mismatch_examples.setdefault(f"unserializable.{fam}.setattr:{name}", _short_stack())
        _note_failed_capture(self)
        _bump_unprot("capt_fail")
        return
    stored = bytes(_kernel_mod.rust_mirror_bytes(h))
    if stored == fresh:
        _count(f"setattr_noop.{fam}.{name}")
        _kernel_mod.rust_mirror_stamp_sync(h, _UNPROT_EPOCH)
        return
    # A write through __setattr__ is a captured mutation, not an escape:
    # mirror it and re-sync any parent wire blobs that embed these bytes.
    # The slot name gates the blind re-index walk (see _REINDEX_SLOTS).
    _count(f"setattr_captured.{fam}.{name}")
    _update_and_cascade(h, fresh, name)


def _make_tvid_init_wrapper(orig_init: Any) -> Any:
    def init(self: TypeVarId, *args: Any, **kwargs: Any) -> None:
        global _TVID_CONSTRUCTION
        _TVID_CONSTRUCTION += 1
        try:
            orig_init(self, *args, **kwargs)
        finally:
            _TVID_CONSTRUCTION -= 1

    init.__name__ = "mirror_tvid_init"
    return init


def _mirror_tvid_setattr(self: TypeVarId, name: str, value: Any) -> None:
    # A TypeVarId write changes the wire bytes of every family carrier
    # embedding it (nested funnels, e.g. freeze_all_type_vars), so capture at
    # the source and re-sync each registered carrier through the normal cascade.
    hook = _active and not _in_serialize and _construction == 0 and _TVID_CONSTRUCTION == 0
    old = getattr(self, name, None)
    _ORIG_SETATTR(self, name, value)
    if not hook:
        # Only tvids already pinned by adoption have embedded registered
        # carriers whose blobs could go stale; unseen tvids are fresh trees.
        if _active and id(self) in _TVID_REVERSE:
            _bump_unprot("tvid_window")
        return
    if old == value:
        _count(f"tvid_setattr_equal.{name}")
        return
    entry = _TVID_REVERSE.get(id(self))
    if entry is None:
        # The tvid was never reachable from a registered tree at any
        # adoption point; its carriers will surface at their funnels.
        _count(f"tvid_orphan.{name}")
        return
    _count(f"tvid_captured.{name}")
    for h in entry[1]:
        obj = _BY_HANDLE.get(h)
        if obj is None:
            _count("cascade_missing_parent")
            continue
        fam = FAMILY_NAME[type(obj)]
        try:
            fresh = _fresh_bytes(obj)
        except Exception:
            _count(f"unserializable.{fam}.tvid:{name}")
            _bump_unprot("tvid_fail")
            continue
        if not _mirror_expect_ok(h, fresh):
            # No slot replacement on the TypeVarId itself, so neither the
            # root nor any parent needs the hidden-embed re-index.
            _update_and_cascade(h, fresh, "")
        else:
            _kernel_mod.rust_mirror_stamp_sync(h, _UNPROT_EPOCH)
            _count(f"tvid_cascade_ok.{name}")


def _mirror_alias_setattr(self: Any, name: str, value: Any) -> None:
    # A TypeAlias._is_recursive write is a wire input of embedded TypeAliasType
    # bytes, so capture at the source and re-sync each registered carrier
    # through the normal cascade, like the TypeVarId shim above.
    hook = _active and not _in_serialize and _construction == 0 and name == "_is_recursive"
    old = getattr(self, name, None)
    _ORIG_SETATTR(self, name, value)
    if not hook:
        # The shim only captures _is_recursive; other writes (a rebound
        # `.target`, for example) restamp only when this alias node is
        # already pinned by adoption (registered carriers embed it).
        if _active and name != "_is_recursive" and id(self) in _ALIAS_REVERSE:
            _bump_unprot("alias_window")
        return
    if old == value:
        _count(f"alias_setattr_equal.{name}")
        return
    entry = _ALIAS_REVERSE.get(id(self))
    if entry is None or entry[0] is not self:
        # The node was never reachable from a registered tree at any
        # adoption point; its carriers will surface at their funnels.
        _count(f"alias_orphan.{name}")
        return
    _count(f"alias_captured.{name}")
    for h in entry[1]:
        obj = _BY_HANDLE.get(h)
        if obj is None:
            _count("cascade_missing_parent")
            continue
        fam = FAMILY_NAME[type(obj)]
        try:
            fresh = _fresh_bytes(obj)
        except Exception:
            _count(f"unserializable.{fam}.alias:{name}")
            _bump_unprot("alias_fail")
            continue
        if not _mirror_expect_ok(h, fresh):
            # No slot replacement on the node itself, so neither the root
            # nor any parent needs the hidden-embed re-index.
            _update_and_cascade(h, fresh, "")
        else:
            _kernel_mod.rust_mirror_stamp_sync(h, _UNPROT_EPOCH)
            _count(f"alias_cascade_ok.{name}")


def activate(
    *, strict: bool = False, audit: bool = False, read: bool = False, instance_write: bool = False
) -> None:
    """Patch family classes to mirror construction/mutation into Rust."""
    global _active, _strict, _audit_mode, _ORIG_SETATTR, _kernel_mod
    if _active:
        return
    try:
        import type_kernel as _km
    except ImportError:
        _count("activate_failed.no_type_kernel")
        return
    _kernel_mod = _km
    _strict = strict
    _audit_mode = audit
    # read sets the F2 (#1393) seam-read flip; only meaningful while the
    # mirror captures (kept across reset(), like _active).
    global _read_mode
    _read_mode = read
    global _write_flip
    _write_flip = instance_write
    _ORIG_SETATTR = object.__setattr__
    for cls in FAMILY_CLASSES:
        saved: dict[str, Any] = {"init": cls.__dict__["__init__"], "write": cls.__dict__["write"]}
        _originals[cls] = saved
        cls.__init__ = _make_init_wrapper(saved["init"], FAMILY_NAME[cls])  # type: ignore[method-assign]
        cls.write = _make_write_wrapper(saved["write"], FAMILY_NAME[cls])  # type: ignore[method-assign]
        cls.__setattr__ = _mirror_setattr  # type: ignore[method-assign, assignment]
    for cls in TVID_CLASSES:
        saved_tvid: dict[str, Any] = {"init": cls.__dict__["__init__"]}
        _originals[cls] = saved_tvid
        cls.__init__ = _make_tvid_init_wrapper(saved_tvid["init"])  # type: ignore[method-assign]
        cls.__setattr__ = _mirror_tvid_setattr  # type: ignore[method-assign, assignment]
    # The alias shim rides on the same saved __setattr__ (object.__setattr__)
    # as the family/tvid classes; nodes classes do not define __setattr__.
    import mypy.nodes as _nodes_mod

    _nodes_mod.TypeAlias.__setattr__ = _mirror_alias_setattr  # type: ignore[method-assign]
    _active = True
    _count("activate")
    # Wire-cache splice funnel (see _check_splice): the splice path in
    # types._write_type_cached bypasses the patched t.write. reset() leaves
    # it installed on purpose: there is no mid-run deactivation.
    import mypy.types as _types_mod

    _types_mod._type_mirror_splice_check = _check_splice
    if audit:
        atexit.register(_dump_audit)


# Deactivation is deliberately unsupported: un-patching mid-run would
# desync mirror state for live objects and drop the cascade graph.
DEACTIVATE_UNSUPPORTED: Final = True


def _dump_audit() -> None:
    out = os.environ.get("MYPY_TK_MIRROR_AUDIT_OUT")
    if not out:
        return
    out = out.replace("{pid}", str(os.getpid()))
    counters = dict(_audit_total)
    for key, n in _audit.items():
        counters[key] = counters.get(key, 0) + n
    examples = dict(_mismatch_total)
    examples.update(_mismatch_examples)
    try:
        with open(out, "w") as f:
            json.dump({"counters": counters, "examples": examples}, f, indent=1)
    except OSError as err:
        print(f"native-type-mirror: audit dump failed: {err}", file=sys.stderr)


def report() -> dict[str, int]:
    """Return a copy of the audit counters."""
    return dict(_audit)


def reset(*, clear_counts: bool = False) -> None:
    """Drop mirror storage and reset the handle registry (per-build boundary)."""
    if _kernel_mod is not None:
        _kernel_mod.rust_mirror_reset()
    _BY_HANDLE.clear()
    _ADOPT_STRIKE.clear()
    _PENDING_CAPTURE.clear()
    _PENDING_CAPTURE_Q.clear()
    _TVID_REVERSE.clear()
    _ALIAS_REVERSE.clear()
    _HIDDEN_EMBED.clear()
    _SLOT_NAMES.clear()
    if clear_counts:
        for key, n in _audit.items():
            _audit_total[key] = _audit_total.get(key, 0) + n
        _audit.clear()
        _mismatch_total.update(_mismatch_examples)
        _mismatch_examples.clear()
