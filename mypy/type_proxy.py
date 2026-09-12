"""ADR-0004 proxy gate (P1, issue #1553).

Blob-backed read shadow of the four family `Type` classes, keyed by the
`identity::handle_for` handles minted behind the type kernel's
`rust_proxy_*` pyfunctions. P1 ships the store scaffold and this Python
gate only: nothing calls `read_scope_bytes` yet, so behavior is
unchanged. P2 wires the lazy `Instance` read shadow through the F2
read funnel.

Design notes (see docs/plans/2026-09-11-adr0004-proxy-brief.md):
- Shadow, never replacement (ADR-0004 Decision 1): Python stays
  canonical, the store mirrors wire bytes; `isinstance`, `__slots__`,
  `copy_modified`, visitors, and plugin hooks see live objects.
- Handles are tracked FFI-free: `_PROXY_HANDLES` maps `id(obj)` to the
  native handle, exactly like `types_mirror._HANDLE_BY_ID`, so the hot
  path never pays a `rust_proxy_handle_of` crossing. `_PROXY_PINS` is
  the Python-side strong pin (the store also pins Rust-side).
- The epoch counter is the coherence gate. A stored entry is served
  only while the epoch it was stamped at still matches the current one;
  `touch` bumps the epoch after an uncaptured in-place mutation, so
  every stale entry misses and the next read re-serializes. No proxy
  state may enter `mypy.cache`: `native_type_proxy` is deliberately not
  in `OPTIONS_AFFECTING_CACHE`.
- Serialization is side-effect-free: `_fresh_bytes` disables the wire
  cache and suppresses the `is_recursive` lazy cache write, so a proxy
  read never flips a live node. A serialization failure returns None
  and never stores, so the caller's own serialization raises
  identically.
"""

from __future__ import annotations

from typing import Any

from librt.internal import WriteBuffer

import mypy.types as _types_mod
from mypy.types import Instance, Type

_kernel_mod: Any = None
_active = False
_audit_mode = False
# id(obj) -> native handle for every object the store holds bytes for.
# The shim is the only minting path (via `rust_proxy_put`), so no FFI
# handle lookup is needed on reads.
_PROXY_HANDLES: dict[int, int] = {}
# id(obj) -> live object: the Python-side strong pin. The Rust store
# pins too; both are dropped together in `reset`/`touch`.
_PROXY_PINS: dict[int, Any] = {}
# Monotonic coherence epoch: entries are served only at their stamp.
_PROXY_EPOCH: int = 0
_audit: dict[str, int] = {}


def _count(key: str, n: int = 1) -> None:
    if not _audit_mode:
        return
    _audit[key] = _audit.get(key, 0) + n


def activate(*, audit: bool = False) -> None:
    """Enable the proxy gate.

    Unlike the F1 mirror this patches nothing: P1 has no funnel wiring,
    and P2 hooks a provider slot instead of a class wrapper. A missing
    extension leaves the gate off (no-op), so imports degrade safely.
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
    _active = True
    _count("activate")


def _fresh_bytes(t: Type) -> bytes | None:
    """Serialize `t` with the wire cache off and the rec-cache suppressed.

    Mirrors `types_mirror._fresh_bytes` but returns None on failure: the
    caller keeps its own serialization as the authoritative path, so the
    original exception surfaces there instead of here.
    """
    prev = _types_mod._type_wire_cache_enabled
    _types_mod._set_type_wire_cache_enabled(False)
    prev_cache = _types_mod._REC_CACHE_SUPPRESSED
    _types_mod._REC_CACHE_SUPPRESSED = True
    try:
        buf = WriteBuffer()
        t.write(buf)
        return buf.getvalue()
    except Exception:
        _count("serialize_fail")
        return None
    finally:
        _types_mod._REC_CACHE_SUPPRESSED = prev_cache
        _types_mod._set_type_wire_cache_enabled(prev)


def read_scope_bytes(t: Type) -> bytes | None:
    """Proxy-cached wire bytes for `t`, or None to serialize as before.

    P1 scope: no funnel calls this yet. P2 wires it into the F2 provider
    ordering (`proxy first, mirror second, None last`) for `Instance`
    roots; every other kind and every failure mode returns None, and the
    caller's normal serialization runs untouched.
    """
    if not _active or _kernel_mod is None:
        _count("scope_defer.inactive")
        return None
    # Exact class, not isinstance: the proper-type plugin rejects
    # isinstance on unexpanded types, and the P2 scope is Instance roots.
    if type(t) is not Instance:
        _count("scope_defer.not_instance")
        return None
    handle = _PROXY_HANDLES.get(id(t))
    if handle is not None:
        blob = _kernel_mod.rust_proxy_read(handle, _PROXY_EPOCH)
        if blob is not None:
            _count("hit")
            return bytes(blob)
        # The epoch moved since this entry was stored (or a native reset
        # dropped it): re-serialize below and re-put.
        _count("stale")
    else:
        _count("miss")
    fresh = _fresh_bytes(t)
    if fresh is None:
        return None
    handle = _kernel_mod.rust_proxy_put(t, fresh, _PROXY_EPOCH)
    _PROXY_HANDLES[id(t)] = handle
    _PROXY_PINS[id(t)] = t
    _count("put")
    return fresh


def touch(t: Type) -> None:
    """Invalidate proxy state after an uncaptured in-place mutation.

    P2 composes this onto the existing `types._mirror_touch` hook. The
    epoch bump makes every entry stored before the mutation a miss
    (shared list items, nested containers); the touched object's entry is
    also dropped so its next read re-serializes immediately.
    """
    if not _active or _kernel_mod is None:
        return
    global _PROXY_EPOCH
    _PROXY_EPOCH += 1
    _count("touch")
    key = id(t)
    handle = _PROXY_HANDLES.pop(key, None)
    if handle is not None:
        _kernel_mod.rust_proxy_drop(handle)
        _PROXY_PINS.pop(key, None)
        _count("touch_drop")


def reset(*, clear_counts: bool = False) -> None:
    """Drop proxy storage and pins (per-build boundary).

    Deliberately does not touch `identity`: the raw handle registry is
    reset by `rust_mirror_reset` alone (`types_mirror.reset`), so other
    seams' handles survive a proxy reset. Activation is one-shot and
    kept across reset, like the mirror.
    """
    if _kernel_mod is not None:
        _kernel_mod.rust_proxy_reset()
    _PROXY_HANDLES.clear()
    _PROXY_PINS.clear()
    global _PROXY_EPOCH
    _PROXY_EPOCH += 1
    _count("reset")
    if clear_counts:
        _audit.clear()


def report() -> dict[str, int]:
    """Return a copy of the audit counters."""
    return dict(_audit)
