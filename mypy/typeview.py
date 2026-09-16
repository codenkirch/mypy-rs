"""Replacement-view gate for the `Instance` family (issue #1671).

The F-reopening experiment: instead of the wire seam walking the live
`Instance` in Python on every call, the `Instance` field set lives in Rust
(`crates/type_kernel/src/typeview.rs`) and the encode is emitted from the
store. This module is the Python half: activation, handle maps, field
write-through, and the counters.

Gating and cost
---------------

Default off. Activation installs a `__setattr__` interceptor on `Instance`
and, in the read arm, an `args` descriptor. A build that never activates
the gate runs the exact class shape and the exact attribute bytecode it ran
before, and the one funnel hook in `mypy/types.py` is a `None`-checked
module global, so the inactive cost is a single `is not None` test per seam
call.

Arms
----

`MYPY_TYPE_VIEW=1` stores the field set and serves the encode from Rust;
`Instance` attribute *reads* stay plain slot reads, so this arm measures
the payoff alone. `MYPY_TYPE_VIEW=2` additionally routes `Instance.args`
reads through `rust_view_args`, the literal "attribute API reads through
the view" shape, which measures what that routing costs.

Coherence
---------

An entry is registered only when it can be proven current, and served only
while it still is:

- `__setattr__` intercepts every `Instance` attribute write, so a write to
  `args`, `type_ref`, `last_known_value` or `extra_attrs` re-registers the
  object from its new field set.
- `Instance.type` is a plain slot and is *not* intercepted (it is read on
  nearly every seam, and a descriptor there would tax the whole build), so
  the Rust side verifies the stored fullname against the live
  `TypeInfo.fullname` on every serve and refuses on a mismatch.
- An instance carrying a `last_known_value` or `extra_attrs`, an instance
  with an argument that has no view of its own, and any tvar-tainted
  argument set are never registered. A pre-fixup instance
  (`type_ref is not None`) is registered with `fixed_up` unset and is
  refused at serve time instead.

Every one of those is a miss, not a guess: a miss returns `None` and the
caller runs its own encode, so an unproven entry can only cost a lookup.

`CACHE_VERSION` is untouched. No view state reaches `mypy.cache`.
"""

from __future__ import annotations

import os as _os
import threading as _threading
from typing import Any

import mypy.types as _types_mod
from mypy.types import Instance, ParamSpecType, TypeVarTupleType, TypeVarType

# Fields the store observes. `type` is deliberately absent (see the module
# docstring): it has no writer hook and the Rust side verifies it instead.
_VIEW_FIELDS = frozenset(("args", "type_ref", "last_known_value", "extra_attrs"))

_TVAR_KINDS = (TypeVarType, ParamSpecType, TypeVarTupleType)

# The `Instance` slot descriptors, captured before the read arm replaces
# `args`. They stay the storage of record, and a build with the gate off has
# no descriptor installed at all.
_MEMBERS: dict[str, Any] = {}

_km: Any = None
_active = False
_read_route = False
# Activating thread (#1712): kernel handles are thread-local while these
# maps are process-global, so entry points degrade to a miss off-thread.
_OWNER_THREAD: int | None = None
# Monotonic coherence stamp, bumped by `reset`. An entry recorded at an
# older stamp is never served.
_STAMP: int = 1

# id(obj) -> view handle. FFI-free on the hot path, exactly like
# `types_mirror._HANDLE_BY_ID`.
_HANDLES: dict[int, int] = {}
# id(obj) -> live object. The Rust store pins too; both drop together, so a
# handle key (derived from `id()`) can never be adopted by a new object.
_PINS: dict[int, Any] = {}
# id(obj) -> (obj, the stamp a late registration last failed at). Holding the
# object stops a freed instance's `id()` from suppressing a new object's retry;
# the stamp expires the memo with the store.
_MISSES: dict[int, tuple[Any, int]] = {}

_audit: dict[str, int] = {}
_audit_mode = False

# `Instance.__setattr__` at activation, or None when the class had none
# ("restore by deletion"). Chained through, never replaced: the F1 mirror
# installs its own hook on the same class (mypy/types_mirror.py:1576-1581).
_ORIG_SETATTR: Any = None


def _count(key: str, n: int = 1) -> None:
    if not _audit_mode:
        return
    _audit[key] = _audit.get(key, 0) + n


def gate_from_env() -> int:
    """`MYPY_TYPE_VIEW` as an arm selector: 0 off, 1 store, 2 store + reads."""
    raw = _os.environ.get("MYPY_TYPE_VIEW", "")
    if not raw.isdigit():
        return 0
    return int(raw)


def active() -> bool:
    return _active


def _on_owner_thread() -> bool:
    return _OWNER_THREAD is not None and _threading.get_ident() == _OWNER_THREAD


def report() -> dict[str, int]:
    """Index-keyed audit counters (empty unless activation asked for them)."""
    return {f"a_{k}": v for k, v in _audit.items()}


def _slot_get(inst: Instance, name: str) -> Any:
    """Read one `Instance` field from its slot, bypassing any descriptor."""
    return _MEMBERS[name].__get__(inst, Instance)


def _drop(inst: Any) -> None:
    key = id(inst)
    handle = _HANDLES.pop(key, None)
    _PINS.pop(key, None)
    if handle is not None and _km is not None:
        _km.rust_view_touch(handle)


def register(inst: Any) -> None:
    """Push the field set of `inst` into the store, or drop the entry.

    Called from the `Instance.__setattr__` interceptor, so it runs on every
    field write and must stay total: anything it cannot prove registrable
    leaves the object absent from the store, which is a serve miss and
    nothing worse.
    """
    if not _active or _km is None or not _on_owner_thread():
        return
    try:
        args = _slot_get(inst, "args")
        type_ref = _slot_get(inst, "type_ref")
        lkv = _slot_get(inst, "last_known_value")
        extra = _slot_get(inst, "extra_attrs")
    except (AttributeError, TypeError, KeyError):
        # A partially constructed instance (or an object with none of these
        # slots): the last `__init__` assignment re-registers the full set.
        _count("register.partial")
        return
    if lkv is not None or extra is not None:
        # The Rust encoder emits the two `LITERAL_NONE` clears
        # unconditionally; an instance carrying a side field is unservable.
        _drop(inst)
        _count("register.side_field")
        return
    handles = []
    for arg in args:
        h = _HANDLES.get(id(arg))
        if h is None:
            # An argument with no view of its own: Rust cannot emit it, so
            # the instance is unservable.
            _drop(inst)
            _count("register.unregistered_arg")
            return
        handles.append(h)
    try:
        handle = _km.rust_view_put(
            inst,
            inst.type.fullname,
            list(args),
            handles,
            type_ref is None,
            not any(type(arg) in _TVAR_KINDS for arg in args),
            _STAMP,
        )
    except Exception:
        # A `NOT_READY` TypeInfo or a missing identity handle. The caller's
        # own encode is unaffected.
        _drop(inst)
        _count("register.failed")
        return
    _HANDLES[id(inst)] = handle
    _PINS[id(inst)] = inst
    _MISSES.pop(id(inst), None)
    _count("register")


def encode(t: Any) -> bytes | None:
    """Wire bytes for `t` from the store, or `None` to encode it here.

    The funnel calls this before its own wire-cache probe; a served encode
    is byte-for-byte what the walk would have produced.
    """
    if not _active or _km is None or not _on_owner_thread():
        return None
    if type(t) is not Instance:
        _count("encode.not_instance")
        return None
    key = id(t)
    handle = _HANDLES.get(key)
    if handle is None:
        # A construction-order gap: an argument may have become registrable
        # after this instance was built. Retried once per object per stamp, so
        # a permanently unservable instance pays the scan once, not per call.
        memo = _MISSES.get(key)
        if memo is not None and memo[0] is t and memo[1] == _STAMP:
            _count("encode.miss")
            return None
        register(t)
        handle = _HANDLES.get(key)
        if handle is None:
            _MISSES[key] = (t, _STAMP)
            _count("encode.miss")
            return None
        _count("encode.late_register")
    try:
        blob = _km.rust_view_encode(handle, _STAMP, t.type.fullname)
    except Exception:
        _count("encode.failed")
        return None
    if blob is None:
        _count("encode.defer")
        return None
    _count("encode.serve")
    return bytes(blob)


def _inst_setattr(inst: Instance, name: str, value: Any) -> None:
    """`Instance` attribute write, routed through the view store.

    Chains into whatever hook was installed before the gate activated, so a
    co-active `types_mirror` capture keeps working.
    """
    (_ORIG_SETATTR or object.__setattr__)(inst, name, value)
    if name in _VIEW_FIELDS:
        register(inst)


def _get_args_routed(inst: Instance) -> Any:
    """`Instance.args` read from the store (the read arm)."""
    if not _on_owner_thread():
        return _slot_get(inst, "args")
    handle = _HANDLES.get(id(inst))
    if handle is not None:
        routed = _km.rust_view_args(handle, _STAMP)
        if routed is not None:
            _count("args.routed")
            return routed
        _count("args.route_miss")
    _count("args.slot")
    return _slot_get(inst, "args")


def _install_hooks(read_route: bool) -> None:
    """Route `Instance` field writes, and optionally `args` reads, to Rust.

    The `__slots__` contract is preserved: the member descriptors stay the
    storage of record, the class keeps its name, `isinstance` behaviour and
    attribute API, and only the access path changes.

    Safe to call again for an arm switch. The slot descriptors are captured
    once and never re-captured, so a routed `args` property can never become
    the storage of record (which would make `_slot_get` recurse through its
    own getter), and the read arm is un-installed on the way out of arm 2.
    """
    global _ORIG_SETATTR
    for name in _VIEW_FIELDS:
        if name in _MEMBERS:
            continue
        member = Instance.__dict__.get(name)
        if member is None or isinstance(member, property) or not hasattr(member, "__set__"):
            _count("install.skipped")
            continue
        _MEMBERS[name] = member
    if read_route and "args" in _MEMBERS:
        # The gate replaces the slot descriptor deliberately. mypy's own
        # checker rejects that shape statically (`args` in `__slots__`
        # conflicts with a class variable), recorded as an ADR finding.
        Instance.args = property(  # type: ignore[assignment,misc]
            _get_args_routed, _MEMBERS["args"].__set__
        )
    elif "args" in _MEMBERS and isinstance(Instance.__dict__.get("args"), property):
        setattr(Instance, "args", _MEMBERS["args"])
    if Instance.__dict__.get("__setattr__") is not _inst_setattr:
        # Captured once. `None` records "the class had no hook of its own",
        # which `deactivate` restores by deletion; re-capturing here on an arm
        # switch would record our own hook and loop `_inst_setattr`.
        _ORIG_SETATTR = Instance.__dict__.get("__setattr__")
    Instance.__setattr__ = _inst_setattr  # type: ignore[method-assign,assignment]
    _types_mod._set_native_type_view_encode(encode)
    _count("install")


def activate(*, read_route: bool = False, audit: bool = False) -> bool:
    """Turn the gate on. Returns whether a store was reachable.

    A missing extension leaves the gate off, so an environment without the
    rebuilt kernel behaves exactly as it did before.
    """
    global _km, _active, _read_route, _audit_mode, _OWNER_THREAD
    if not _active:
        try:
            import type_kernel as _kernel
        except ImportError:
            _count("activate.no_type_kernel")
            return False
        if not hasattr(_kernel, "rust_view_put"):
            # A stale extension without the view store: leave the gate off.
            _count("activate.no_view_store")
            return False
        _km = _kernel
        _active = True
        _OWNER_THREAD = _threading.get_ident()
    if audit:
        _audit_mode = True
    if read_route != _read_route:
        _read_route = read_route
        _install_hooks(read_route)
    elif not _MEMBERS or Instance.__dict__.get("__setattr__") is not _inst_setattr:
        # A co-resident hook (types_mirror.activate is one-shot and does not
        # chain) may have taken the slot while the gate stayed active.
        # Re-assert, and count the steal.
        _count("activate.reassert")
        _install_hooks(read_route)
    if Instance.__dict__.get("__setattr__") is not _inst_setattr:
        # Observable guard: `install` succeeding while the slot is not ours
        # means a third mechanism is fighting for it, and the gate would be
        # believed active while inert.
        _count("activate.hook_not_held")
    return True


def reset(*, clear_counts: bool = False) -> None:
    """Drop view storage, pins, and bump the stamp (per-build boundary).

    Deliberately does not touch `identity`: the handle registry is owned by
    `rust_mirror_reset`, so view state cannot invalidate handles other seams
    hold.
    """
    global _STAMP
    if _km is not None:
        _km.rust_view_reset()
    _HANDLES.clear()
    _PINS.clear()
    _MISSES.clear()
    _STAMP += 1
    _count("reset")
    if clear_counts:
        _audit.clear()


def deactivate() -> None:
    """Remove the hooks and restore the plain slot class shape.

    The gate is one-shot for a build, so this exists for the gate-off/on
    differential tests and for any embedding that needs the pre-activation
    class back. The captured member descriptors are reinstalled, so
    `__slots__`, `isinstance` and the attribute API return to exactly the
    pre-activation shape.
    """
    global _active, _read_route, _ORIG_SETATTR, _OWNER_THREAD
    if not _active:
        return
    _OWNER_THREAD = None
    for name, member in _MEMBERS.items():
        setattr(Instance, name, member)
    if Instance.__dict__.get("__setattr__") is _inst_setattr:
        if _ORIG_SETATTR is None:
            del Instance.__setattr__
        else:
            Instance.__setattr__ = _ORIG_SETATTR  # type: ignore[method-assign]
    _ORIG_SETATTR = None
    _types_mod._set_native_type_view_encode(None)
    reset()
    _active = False
    _read_route = False
    _MEMBERS.clear()


def stats() -> dict[str, int]:
    """Store-side counters: serves, defers, routed reads, live entries."""
    if _km is None:
        return {}
    encodes, defers, routes = _km.rust_view_stats()
    return {
        "encodes": encodes,
        "defers": defers,
        "read_routes": routes,
        "entries": _km.rust_view_count(),
    }
