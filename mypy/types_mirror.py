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
"""

from __future__ import annotations

import atexit
import json
import os
import sys
from collections import deque
from collections.abc import Iterator
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
# Attributes written by hashing or lazy truthiness init never affect the
# wire bytes of any family class; skipping them keeps the mirror quiet.
SKIP_ATTRS: Final = frozenset(
    {
        "_hash",
        "_can_be_true",
        "_can_be_false",
        # Derived truthiness facets; the wire recomputes them on read.
        "can_be_true",
        "can_be_false",
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
_ORIG_SETATTR: Any = None
# family class -> saved originals (for a future multi-run protocol).
_originals: dict[type, dict[str, Any]] = {}
# handle -> live object (strong pin until reset).
_BY_HANDLE: dict[int, Any] = {}
# Objects whose adoption already failed (pre-semanal partials the wire
# cannot serialize). Bounded FIFO by id(): a recycled id retries once
# more later; the write funnel stays the authoritative registration point.
_ADOPT_STRIKE: set[int] = set()
_ADOPT_STRIKE_Q: deque[int] = deque()
_ADOPT_STRIKE_CAP: Final = 65536
_audit: dict[str, int] = {}
_mismatch_examples: dict[str, str] = {}
# TypeVarId capture state (see TVID_CLASSES). `_TVID_CONSTRUCTION` counts
# TypeVarId.__init__ calls in flight so construction writes are never captured;
# `_TVID_REVERSE` pins the tvid (id() stays valid until reset() per build).
_TVID_CONSTRUCTION: int = 0
_TVID_REVERSE: dict[int, tuple[TypeVarId, set[int]]] = {}
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


def _note_failed_adoption(obj: Any) -> None:
    key = id(obj)
    if key in _ADOPT_STRIKE:
        return
    if len(_ADOPT_STRIKE) >= _ADOPT_STRIKE_CAP:
        old = _ADOPT_STRIKE_Q.popleft()
        _ADOPT_STRIKE.discard(old)
    _ADOPT_STRIKE.add(key)
    _ADOPT_STRIKE_Q.append(key)


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


def _type_names(cls: type) -> tuple[str, ...]:
    """Collect all __slots__ names across the MRO, deduplicated in order."""
    names: list[str] = []
    for klass in cls.__mro__:
        slots = getattr(klass, "__slots__", ())
        if slots:
            names.extend(slots)
    return tuple(dict.fromkeys(names))


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


def _tvids_in_value(value: Any, seen: set[int]) -> Iterator[TypeVarId]:
    """Descend any slot value, yielding every TypeVarId reachable.

    Unlike the child-types walk this descends into every Type (family or
    not): a TypeVarId can hide inside a ParamSpecType, a TypeVarTupleType
    fallback, or a plain list of variables, never touching a family slot.
    `seen` cuts cycles (recursive alias trees) and is per-walk, so id()
    stability is guaranteed while the root keeps the tree alive.
    """
    if value is None or isinstance(value, (str, bytes, bool, int, float)):
        return
    if isinstance(value, TypeVarId):
        yield value
        return
    if isinstance(value, Type):
        yield from _tvids_in(value, seen)
        return
    if isinstance(value, (list, tuple)):
        seen.add(id(value))
        for item in value:
            yield from _tvids_in_value(item, seen)
        return
    if isinstance(value, dict):
        seen.add(id(value))
        for item in value.values():
            yield from _tvids_in_value(item, seen)
        return
    if type(value).__name__ == "ExtraAttrs":
        yield from _tvids_in_value(value.attrs, seen)


def _tvids_in(t: Type, seen: set[int]) -> Iterator[TypeVarId]:
    if id(t) in seen:
        return
    seen.add(id(t))
    for name in _type_names(type(t)):
        if name.startswith("_") or name in _SKIP_SLOTS:
            continue
        try:
            value = object.__getattribute__(t, name)
        except AttributeError:
            continue
        yield from _tvids_in_value(value, seen)


def _record_tvid_carriers(root: Type, handle: int) -> None:
    """Index every TypeVarId reachable from `root` to its carrier handle."""
    for tvid in _tvids_in(root, set()):
        entry = _TVID_REVERSE.get(id(tvid))
        if entry is None:
            _TVID_REVERSE[id(tvid)] = (tvid, {handle})
        else:
            entry[1].add(handle)


def _family_embeds(t: Type, seen: set[int]) -> Iterator[Type]:
    """Yield every family Type reachable from `t` (excluding `t` itself).

    Unlike `_child_types` this does NOT stop at non-family Types: the
    chain tvar -> TupleType -> Instance fallback must stay visible so a
    captured write on the Instance can cascade into the tvar.
    """
    if id(t) in seen:
        return
    seen.add(id(t))
    for name in _type_names(type(t)):
        if name.startswith("_") or name in _SKIP_SLOTS:
            continue
        try:
            value = object.__getattribute__(t, name)
        except AttributeError:
            continue
        yield from _family_embeds_in_value(value, seen)


def _family_embeds_in_value(value: Any, seen: set[int]) -> Iterator[Type]:
    if value is None or isinstance(value, (str, bytes, bool, int, float)):
        return
    if isinstance(value, Type):
        if id(value) not in seen:
            if type(value) in FAMILY_NAME:
                yield value
            yield from _family_embeds(value, seen)
        return
    if isinstance(value, (list, tuple)):
        for item in value:
            yield from _family_embeds_in_value(item, seen)
        return
    if isinstance(value, dict):
        for item in value.values():
            yield from _family_embeds_in_value(item, seen)
        return
    if type(value).__name__ == "ExtraAttrs":
        yield from _family_embeds_in_value(value.attrs, seen)


# ---- fresh serialization ----


def _fresh_bytes(t: Type) -> bytes:
    """Serialize `t` to fresh bytes with the wire cache disabled."""
    global _in_serialize
    from librt.internal import WriteBuffer

    from mypy.types import _set_type_wire_cache_enabled as _set

    prev = _wire_cache_enabled()
    _set(False)
    _in_serialize = True
    try:
        buf = WriteBuffer()
        t.write(buf)
        return buf.getvalue()
    finally:
        _in_serialize = False
        _set(prev)


# ---- registration / adoption / cascade ----


def _handle_of(t: Any) -> int | None:
    return cast("int | None", _kernel_mod.rust_mirror_handle_of(t))


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
    _record_tvid_carriers(t, handle)
    _add_hidden_embeds(t, handle)
    return handle  # type: ignore[no-any-return]


def _add_hidden_embeds(obj: Type, handle: int) -> None:
    """Index every family descendant reachable through non-family Types.

    Registered both at `_register_tree` time and after every cascade sync:
    a captured setattr can replace a subtree (`tv.upper_bound = ...`),
    whose new chain is invisible to any earlier fill, so containers must
    be re-indexed whenever their bytes are re-serialized. Which objects
    get the re-index walk is gated by `replaced_slot` in
    `_update_and_cascade`; this helper itself always walks the full
    `_family_embeds` set of the container it is given.
    """
    for embed in _family_embeds(obj, set()):
        entry = _HIDDEN_EMBED.get(id(embed))
        if entry is None:
            _HIDDEN_EMBED[id(embed)] = (embed, {handle})
        else:
            entry[1].add(handle)


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
    # carry embeds (`_add_hidden_embeds` keeps replaced subtrees reachable too).
    skip_all = replaced_slot == ""
    root_reindex = replaced_slot is None or (not skip_all and replaced_slot in _REINDEX_SLOTS)
    stack = list(_kernel_mod.rust_mirror_parents(handle))
    obj = _BY_HANDLE.get(handle)
    if obj is not None:
        stack.extend(_sync_seed_handles(obj))
    seen = {handle}
    _kernel_mod.rust_mirror_update(handle, fresh)
    if obj is not None:
        _record_tvid_carriers(obj, handle)
        if root_reindex:
            _add_hidden_embeds(obj, handle)
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
        _record_tvid_carriers(parent, ph)
        try:
            pfresh = _fresh_bytes(parent)
        except Exception:
            _count("cascade_unserializable")
            continue
        _kernel_mod.rust_mirror_update(ph, pfresh)
        if replaced_slot is None:
            _add_hidden_embeds(parent, ph)
        stack.extend(_kernel_mod.rust_mirror_parents(ph))
        stack.extend(_sync_seed_handles(parent))


def _assert_fresh(t: Type, site: str) -> None:
    """Assert the mirror blob matches fresh live bytes for a family type.

    On mismatch: strict mode raises AssertionError; audit mode counts and
    re-syncs plus cascades, so an escaped mutation triggers exactly once
    here before the mirror state catches up.
    """
    fam = FAMILY_NAME[type(t)]
    h = _handle_of(t)
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
    assert_passes = _mirror_expect_ok(h, fresh)
    if assert_passes:
        _count(f"assert_ok.{fam}.{site}")
        return
    key = f"mismatch.{fam}.{site}"
    _count(key)
    if _strict:
        raise AssertionError(f"native-type-mirror ({fam}, {site}) pure-Python bytes drifted")
    if _audit_mode:
        _note_mismatch(key, _short_stack())
    _update_and_cascade(h, fresh)


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
    hook = (
        _rules_ok(self)
        and _construction == 0
        and name not in SKIP_ATTRS
        and id(self) not in _ADOPT_STRIKE
    )
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
    try:
        fresh = _fresh_bytes(self)
    except Exception:
        # Serialization can legitimately fail on a partial object; count
        # and move on so the assertion surfaces at the next funnel.
        _count("unserializable." + fam + ".setattr:" + name)
        return
    stored = _kernel_mod.rust_mirror_bytes(h)
    if stored == fresh:
        _count(f"setattr_noop.{fam}.{name}")
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
            continue
        if not _mirror_expect_ok(h, fresh):
            # No slot replacement on the TypeVarId itself, so neither the
            # root nor any parent needs the hidden-embed re-index.
            _update_and_cascade(h, fresh, "")
        else:
            _count(f"tvid_cascade_ok.{name}")


def activate(*, strict: bool = False, audit: bool = False) -> None:
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
    _ORIG_SETATTR = object.__setattr__
    for cls in FAMILY_CLASSES:
        saved: dict[str, Any] = {"init": cls.__dict__["__init__"], "write": cls.__dict__["write"]}
        _originals[cls] = saved
        cls.__init__ = _make_init_wrapper(saved["init"], FAMILY_NAME[cls])  # type: ignore[method-assign]
        cls.write = _make_write_wrapper(saved["write"], FAMILY_NAME[cls])  # type: ignore[method-assign]
        cls.__setattr__ = _mirror_setattr  # type: ignore[method-assign, assignment]
    for cls in TVID_CLASSES:
        saved_tvid: dict[str, Any] = {
            "init": cls.__dict__["__init__"],
        }
        _originals[cls] = saved_tvid
        cls.__init__ = _make_tvid_init_wrapper(saved_tvid["init"])  # type: ignore[method-assign]
        cls.__setattr__ = _mirror_tvid_setattr  # type: ignore[method-assign, assignment]
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
    try:
        with open(out, "w") as f:
            json.dump({"counters": _audit, "examples": _mismatch_examples}, f, indent=1)
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
    _ADOPT_STRIKE_Q.clear()
    _TVID_REVERSE.clear()
    _HIDDEN_EMBED.clear()
    if clear_counts:
        _audit.clear()
        _mismatch_examples.clear()
