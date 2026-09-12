# ADR-0004 proxy graduation brief (wave 66C / issue #1549)

Date: 2026-09-11. Basis: read-only audit at `bf019a1f1` plus probes.
Persisted by the orchestrator from the wave-66C brief.

## ADR staleness (read first)

- All Python line anchors in ADR-0004 are from `4f499bf1d`; do not use them
  as file:line facts. Use the inventory labels.
- ADR Decision 1 says the identity map is "installed into Rust as a single
  PyObject"; the code deliberately went the other way (pure-Python
  `_HANDLE_BY_ID` avoids 59.8M FFI crossings, `types_mirror.py:172-176`,
  `:810-814`). The proxy must follow the code.
- ADR Decision 3 names `mypy/nativeproxy.touch`; the real hook is
  `mypy.types._mirror_touch` (`types.py:213-234`) via
  `types_mirror.touch` (`types_mirror.py:817-847`).
- Post-ADR work to fold into any O/R/S table: `_native_copy_modified`
  (#475), `py_type_eq` (#1412), `ExtraAttrs.raw` (#1527), the 7-site
  raw-mutator touch registry (#1535), #1528's re-scope to strong pins.
- Model conflict: the plan text promises proxy views replacing storage;
  ADR-0004 Decision 1 says "shadow, never replacement". ADR-0004 wins
  (plugins, `isinstance`, `__slots__`, astmerge all assume live nodes);
  record the supersession in a successor ADR.
- #1528's body claim is wrong at HEAD: a probe shows ALL eight `Type`
  classes (including `CallableType`) fail `weakref.ref`; only the
  strong-pin retire protocol remains.

## Proxy surface (first slices)

- Families: the four `FAMILY_CLASSES` (`types_mirror.py:76`) are roots;
  non-family leaves are children; Set-2 wrappers never get entries and
  defer.
- Storage model for P1/P2: blob-backed shadow in a new `proxy.rs` keyed by
  the existing `identity::handle_for` handle (no second handle namespace),
  reusing the bytes-in seam interface; field-level storage is P4.
- Instance first (largest construction volume, all four splice ops
  already exist, no NOT_READY hazard). Field routing: `type` via fullname
  snapshot, `args` as child handles, `type_ref`/`last_known_value`/
  `extra_attrs` live-read + touch-on-write.
- Invariants under the proxy: `isinstance`/`type(x)` unchanged;
  `__slots__` untouched; `copy_modified`/visitors/`__eq__`/`__hash__`
  unchanged; positions never trusted from blobs.

## Identity bridge and purge points

- Reuse `identity::handle_for` raw handles now; move to stable handles in
  the same PR that lets entries survive a recheck (#1528).
- Purge: `_clear_native_resolvers` (`build.py:1813`, called from
  `server/update.py:710`, `:1123`) must reset the proxy in the existing
  `native_type_mirror` branch; keep `rust_mirror_reset` as the single owner
  of `identity::reset()`; `type_proxy.reset()` clears entries/pins only.

## First slice (P2): lazy Instance read shadow

Compute blobs lazily at the F2 read funnel instead of eager capture: first
read serializes once (store blob+epoch), later reads hit if the epoch is
unchanged. No class patching, no eager adoption, no cascade. Exact changes
are enumerated in the brief: new `crates/type_kernel/src/proxy.rs` +
pyfunctions, new `mypy/type_proxy.py`, `types.py` provider slot +
`_read_mirror_blob` ordering, `Options.native_type_proxy` (default off,
`TEST_NATIVE_TYPE_PROXY`), touch composition onto the existing hook,
`NativeProxyStoreSuite` + `NativeInstanceProxyReadSuite` + gate-off/on
funnel differentials + plugin/DefaultPlugin identity checks +
fine-grained/daemon runs, and `misc/wf4_proxy_selfcheck.py` measurement.

Decision rule from ADR-0004 Consequences and the mirror audit: if
proxy-on does not beat baseline wall, DROP the slice; do not defend it.
The within-10% gate is a full-proxy graduation target, not a P2 target.

## Risk deltas

- Coherence is the top correctness risk: the touch list is an audit
  artifact; a missed checking-phase in-place writer serves stale blobs.
  P3 strict mode asserts `blob == fresh` at every funnel and must be green
  on testcheck + the plugin corpus before any default flip.
- Plugins see live objects (ADR Decision 5); do not implement the stale
  plan text that hands hooks Rust types.
- mypyc: the proxy patches nothing (safer); `type_proxy.py` must degrade to
  a no-op under a compiled build.
- Cache: no proxy state may enter `mypy.cache`; keep the flag out of
  `OPTIONS_AFFECTING_CACHE`; run `testfinegrainedcache` + a warm A/B.
- Daemon: per-recheck clearing keeps raw handles safe; stable-handle reuse
  is out of P1-P4 and requires #1528 first.
- Cold path: one FFI crossing on hit, serialize+store on miss; emit
  hit/miss and per-size counters, plus an entry cap in P3.

## Ordered PR sequence

1. **P1** `feat(type_proxy): scope store scaffold + gate` - proxy.rs
   store/pyfunctions, `type_proxy.py` shim, option + env gate, reset
   branch, units, successor-ADR draft. No funnel wiring; zero behavior
   change.
2. **P2** `feat(type_proxy): lazy Instance read shadow` - funnel provider
   ordering, serialize-on-miss + pins, touch composition, suites,
   `wf4_proxy_selfcheck.py` A/B with the drop-if-no-win rule.
3. **P3** `fix(type_proxy): coherence hardening` - touch calls at the
   Decision-3 writers, strict-mode funnel assert, entry cap/eviction,
   plugin differential, daemon recheck test.
4. **P4** `feat(type_proxy): field-level Instance shadow + first
   handle-based read` - per-node storage, one kernel consumer pilot.
