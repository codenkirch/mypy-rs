# ADR-0005: proxy store scaffold and shadow model (draft, successor to ADR-0004)

Status: Draft (P1 landed, P2-P4 pending).
Date: 2026-09-12.
Basis: ADR-0004, the wave-66C proxy brief
(`docs/plans/2026-09-11-adr0004-proxy-brief.md`), and the P1 implementation
(`crates/type_kernel/src/proxy.rs`, `mypy/type_proxy.py`, issues #1553/#1554).

## What this ADR supersedes

1. **Storage shape.** ADR-0004 Decision 1 describes the identity map as a
   single `PyObject` installed into Rust. The implementation went the
   other way: `_HANDLE_BY_ID` stays pure-Python to avoid tens of millions
   of FFI crossings (`mypy/types_mirror.py:172-176`, `:810-814`). The
   proxy follows the code: handles come from the existing
   `identity::handle_for` raw layer (no second handle namespace), Python
   keeps its own `id()`-keyed `_PROXY_HANDLES`/`_PROXY_PINS` maps, and the
   Rust store holds one `ProxyEntry { bytes, stamp }` per handle plus a
   strong `Py<PyAny>` pin.
2. **Plan text vs ADR text.** The migration plan promises proxy *views
   replacing storage*; ADR-0004 Decision 1 already says "shadow, never
   replacement". The ADR text wins: plugins, `isinstance`, `__slots__`,
   and astmerge all assume live nodes. The proxy never patches a class
   (P1) and, in P2, hooks a provider slot for reads instead of wrapping
   `write`.

## Decision: blob-backed shadow, lazy at the read funnel

- P1 (#1553) ships the inert scaffold: the Rust store, the six
  `rust_proxy_*` pyfunctions, `mypy/type_proxy.py` (activation, FFI-free
  handle maps, epoch counter, audit counters), `Options.native_type_proxy`
  (default off, not in `OPTIONS_AFFECTING_CACHE`), and the
  `_clear_native_resolvers` reset branch. No funnel reads an entry, so
  behavior is unchanged.
- P2 (#1554) computes blobs lazily at the F2 read funnel: first read
  serializes the `Instance` once (store blob + epoch), later reads hit
  while the epoch is unchanged. No eager adoption and no `__setattr__`
  capture.
- The epoch counter is the coherence gate: `touch` bumps it after any
  uncaptured in-place mutation, so every entry stored before the mutation
  misses. A serialization failure never stores; it returns `None` and the
  caller's own serialization raises identically.
- `reset` clears entries and pins only. `identity::reset` stays owned by
  `rust_mirror_reset`; a proxy reset must not invalidate handles other
  seams still hold.
- Decision rule from ADR-0004 Consequences and the mirror audit: if
  proxy-on does not beat the baseline wall, drop the P2 slice. The
  within-10% gate remains a full-proxy graduation target, not a P2 one.

## Consequences

- Cache safety: no proxy state may enter `mypy.cache`; the option stays
  out of `OPTIONS_AFFECTING_CACHE`.
- Daemon safety: raw handles are safe because `_clear_native_resolvers`
  resets the store per recheck; stable-handle reuse still requires #1528.
- P3 adds the touch calls at every checking-phase in-place writer, a
  strict funnel assert (`blob == fresh`), an entry cap, and the plugin
  differential before any default flip.
- P4 moves to field-level `Instance` storage and one kernel consumer.
