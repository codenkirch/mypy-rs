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
  ``id()`` cannot adopt a stale handle; escaped mutations (list ops,
  ``TypeVarId.meta_level``) are detected at the next serialization funnel
  instead of at ``__setattr__``.
"""

from __future__ import annotations

import atexit
import json
import os
import sys
from collections import deque
from collections.abc import Iterator
from typing import Any, Final, cast

from mypy.types import (
    CallableType,
    Instance,
    Type,
    TypeVarType,
    UnionType,
)

FAMILY_CLASSES: Final = (Instance, CallableType, TypeVarType, UnionType)
FAMILY_NAME: Final[dict[type, str]] = {
    Instance: "instance",
    CallableType: "callable",
    TypeVarType: "tvar",
    UnionType: "union",
}
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
    # Only called on a mismatch, so formatting cost is bounded; keep the
    # last frames up to (and including) the caller of the funnel.
    import traceback as _tb

    return "".join(_tb.format_stack()[:-2])[-1600:]


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
    return handle  # type: ignore[no-any-return]


def _update_and_cascade(handle: int, fresh: bytes) -> None:
    """Write `fresh` for `handle`, then re-sync parents containing it."""
    stack = list(_kernel_mod.rust_mirror_parents(handle))
    seen = {handle}
    _kernel_mod.rust_mirror_update(handle, fresh)
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
        try:
            pfresh = _fresh_bytes(parent)
        except Exception:
            _count("cascade_unserializable")
            continue
        _kernel_mod.rust_mirror_update(ph, pfresh)
        stack.extend(_kernel_mod.rust_mirror_parents(ph))


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
    _count(f"setattr_captured.{fam}.{name}")
    _update_and_cascade(h, fresh)


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
        saved: dict[str, Any] = {
            "init": cls.__dict__["__init__"],
            "write": cls.__dict__["write"],
        }
        _originals[cls] = saved
        cls.__init__ = _make_init_wrapper(saved["init"], FAMILY_NAME[cls])  # type: ignore[method-assign]
        cls.write = _make_write_wrapper(saved["write"], FAMILY_NAME[cls])  # type: ignore[method-assign]
        cls.__setattr__ = _mirror_setattr  # type: ignore[method-assign, assignment]
    _active = True
    _count("activate")
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
    if clear_counts:
        _audit.clear()
        _mismatch_examples.clear()
