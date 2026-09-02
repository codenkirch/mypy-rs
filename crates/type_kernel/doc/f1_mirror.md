# F1 dual-write shadow mirror (issue #1370)

Python stays canonical. With `Options.native_type_mirror` on, every
construction and attribute mutation of the four family classes
(`Instance`, `CallableType`, `TypeVarType`, `UnionType`) is mirrored into
Rust storage inside the type kernel, and every wire serialization of a
family object asserts that the stored blob equals a freshly computed
`Type.write`. F1 has no reader: the mirror exists to prove that the
Python-serialized graph is byte-identical to what a Rust owner (Phase F2+
owns the graphs) would have held, on the exact bytes every consumer would
read.

## Design

- Capture is class-level monkeypatching of `__init__`, `__setattr__`, and
  `write` on the four family classes (`mypy/types_mirror.py`). Class
  identity is never swapped, so `type(t) is Instance` keeps working.
  When the gate is off nothing is patched: off-path cost is zero.
- Registration is lazy. `__init__` capture only counts; an object enters
  the mirror at its first serialization funnel (the wrapped `write`),
  because semanal constructs partial objects whose wire bytes cannot
  exist yet ("fallback can't be filled out until semanal",
  `PlaceholderType`/`EllipsisType` are unserializable mid-phase).
  Mutations that happen before an object's first funnel write define its
  adoption baseline and are invisible in F1; this is the same visibility
  any non-family object already has.
- The wire bytes asserted at every funnel are recomputed fresh by the
  mirror (`_fresh_bytes`) with `_type_wire_cache_enabled` forced off and
  an in-serialize guard set. The mirror never trusts the
  `_type_wire_cache`, which can legally be stale after an in-place
  mutation; conversely the mirror never poisons the cache (its bytes are
  computed with the cache disabled, and no write path sees them).
- `__setattr__` capture is assert-POST: the object is serialized after
  the attribute lands; if the bytes did not change the write is counted
  `setattr_noop`, otherwise it is a captured mutation and the mirror
  updates the object's blob and re-serializes every registered parent
  whose blob embeds the child bytes (the cascade; `UnionType.write`
  derives `can_be_true`/`can_be_false` and byte-lengths from items, so
  child updates must re-emit parent blobs). Objects whose fresh
  serialization still fails are put on a bounded failed-adoption memo
  (`_ADOPT_STRIKE`, 64k ids, FIFO) so per-setattr retries stop; the
  write funnel remains the authoritative registration point either way.
- Live objects are pinned strongly in `_BY_HANDLE` until `reset()`
  (types carry no `__weakref__`), so a recycled `id()` cannot adopt a
  stale handle. Handles come from the type kernel's raw identity layer
  (`handle_for` mints, `handle_of` never mints); `reset()` clears the
  Rust registry and the identity generation together.
- Cascade parents are stored Rust-side (`parents_of` child -> parents);
  re-registration of a parent replaces its child list (no dangling
  parent entries).
- Modes: strict (`MYPY_TK_MIRROR=1`) raises on the first divergence;
  audit (`MYPY_TK_MIRROR_AUDIT=1`) counts, captures one example plus a
  short traceback per mismatch class, and resyncs so each escape fires
  once. JSON dumps to `$MYPY_TK_MIRROR_AUDIT_OUT` at atexit; use `{pid}`
  in the path for per-pytest-worker files (spliced-serve mismatch and
  stale-splice records land in those same `{pid}` files).

## Gate plumbing

- `Options.native_type_mirror: bool = False` (options.py), test-gated via
  `TEST_NATIVE_TYPE_MIRROR` in `mypy/test/helpers.py` (`_env_gate`).
- For cold CLI runs there is no CLI flag; the self-check gate uses
  `misc/wf1_selfcheck_mirror.py`, a wrapper that forces
  `Options.native_type_mirror = True` pre-`main` (the option round-trips
  to workers through `options.to_bytes`, so `num_workers` subprocesses
  inherit it).
- Activation is independent of `native_type_kernel`: `mypy/build.py`
  calls `types_mirror.activate(...)` at build start when the option is
  on, with modes from `MYPY_TK_MIRROR` / `MYPY_TK_MIRROR_AUDIT`.
- `_clear_native_resolvers` calls `types_mirror.reset()` before the
  kernel gate's early return, so a mirror-only run (kernel off) still
  drops its per-build state.
- There is no CLI flag and no deactivation: un-patching mid-run would
  desync live objects and drop the cascade graph.

## Rust side

`crates/type_kernel/src/mirror.rs` holds the thread-local registry:

- `by_handle: handle -> (family, bytes)` blob store;
- `parents_of` / `children_of` cascade graph (re-register unlinks);
- `register` / `update` / `expect` (ValueError with lens + first-diff
  byte offset + 8-byte hex context) / `entry_bytes` / `entry_family` /
  `parents` / `reset` / `entry_count` pyfunctions, plus the non-minting
  `rust_mirror_handle_of`.

## Audit table (cold self-check + parity suites, audit mode)

`TEST_NATIVE_TYPE_MIRROR=1 MYPY_TK_MIRROR_AUDIT=1` runs (see the state
report): testtypes (unit suites), testcheck (8198 passed), counters
aggregated across 4 pytest workers. A cold (`--cache-dir` fresh from
empty) self-check through `misc/wf1_selfcheck_mirror.py` completes with
0 errors in 347 files and non-zero counters (2.53M instance / 886k
callable / 422k tvar / 280k union inits), with funnel escapes in the
same in-place-drift class as the table below (tvar 588, callable 135,
instance 16, union 0); the higher rates over testcheck reflect the
self-check corpus' heavier in-place checker/semantic-analyzer mutation
of mypy's own source, reached through `names.write`/`fill_typevars`
serialization funnels. Post-splice-funnel runs (fork librt wheel on
PYTHONPATH, wire cache active) shift those write-path counts to tvar
590 / callable 87 / instance 12 / union 0 and add splice counters:
`adopt.instance.cachedsplice` 343,
`assert_ok.instance/union/callable.cachedsplice`
341,694/33,197/5,987, plus zero `stale.<fam>.cachedsplice` and zero
`mismatch.<fam>.cachedsplice`: no spliced serve carried drifted bytes.

| counter class | meaning | result |
| ------------- | ------- | ------ |
| `init.<fam>` | family constructions seen | 3.57M instances, 1.68M tvars, 2.05M callables, 186k unions per full testcheck |
| `adopt.<fam>.write` | lazy registrations at first funnel | ~21k instances, ~15k callables, ~700 tvars/unions |
| `adopt.<fam>.cachedsplice` | mirror registrations through a wire-cache splice hit (object unregistered at splice time; kernel-on runs only) | scc: 343 instances; testcheck: 436 instances |
| `assert_ok.<fam>.<site>` | funnel asserts that matched stored bytes | the overwhelming majority (`cachedsplice` site: scc 342k instances, 33k unions, 6k callables) |
| `setattr_captured.<fam>.<attr>` | mutations through `__setattr__`, mirrored + cascaded | e.g. `instance.args` 706k, `instance.end_line` 671k, `tvar.default` 305k, `callable.definition` 18k |
| `setattr_noop.*` | attribute writes that changed no wire byte | cache/tuple identity writes |
| `setattr_gagged.<fam>` | setattr on an object the wire cannot yet bind (bounded retry memo) | 241k callables ("fallback can't be filled out until semanal"), ~7k instances, ~100 tvars |
| `mismatch.<fam>.write` / `mismatch.<fam>.cachedsplice` | funnel-detected escapes (write serve / splice serve), counted then resynced | see below; `cachedsplice` 0 in every audited corpus |
| `stale.<fam>.cachedsplice` | splice served bytes that drifted from the live type (strict raise; audit pops the cache entry so the next write re-caches) | 0 in every audited corpus |
| `unserializable.*` | partial objects the wire cannot serialize (counted, deferred to funnel) | semanal-phase objects only |

Escapes are audit counts, resynced after each, full testcheck at
kernel-off:

| family | count | cause |
| ------ | ----- | ----- |
| `tvar` | 175 | in-place fallback/meta mutations reached through `TypeVarType.write` (`.tuple` fallback rebuilds, `TypeVarId.meta_level`); not a family-class setattr |
| `instance` | 2 | in-place `union items` list splices surfaced through a child instance's write funnel |
| `callable` | 0 | (pre-lazy-registration runs showed 191 `normalize_trivial_unpack` splices; they were artifacts of a dropped-write bug in the gagged-setattr path, fixed: a setattr on an object the mirror cannot bind now applies the write and only skips capture) |
| `union` | 0 | - |

With the fork librt wheel on PYTHONPATH the wire cache becomes active
(the venv's stock librt lacks `write_raw_bytes`, so the cache was inert
in the numbers above). That alone moves the testcheck write-path counts
to tvar 188 / instance 2 / callable 0 / union 0 (a cache-inert rerun
reproduces 176/2/0/0, i.e. baseline within one count), so the delta is
cache activation rather than the splice hook; the exact per-event
mechanism was not traced. Splice asserts
(`assert_ok.<fam>.cachedsplice`) fire in both kernel-off and kernel-on
fork-librt runs; the `adopt.<fam>.cachedsplice` row appears only in
kernel-on runs (436 instances at full testcheck).

## Explicitly not captured in F1

- Mutations before an object's first funnel write (adoption baseline).
- (Was not captured before #1372.) A family write served fully from the
  wire cache via `_write_type_cached`'s `write_raw_bytes` splice never
  called `t.write`, so no funnel fired. This is now funneled:
  `_write_type_cached`'s cache-hit branch calls
  `_type_mirror_splice_check(t, blob)` (mirroring in
  `mypy/types.py`, implementation `_check_splice` in
  `mypy/types_mirror.py`) right before `write_raw_bytes`.
  `_check_splice` fresh-serializes `t` with the cache disabled and
  compares: an unregistered object is adopted
  (`adopt.<fam>.cachedsplice`); a registered object whose bytes drifted
  is a `mismatch.<fam>.cachedsplice` escape with resync and cascade
  (strict mode raises); a match counts `assert_ok.<fam>.cachedsplice`;
  and when the *spliced* blob itself differs from the live bytes the
  serve is stale: `stale.<fam>.cachedsplice` counts, strict mode
  raises, and
  audit mode pops the cache entry so the next write re-caches instead
  of serving drifted bytes again. The hook is only installed by
  `types_mirror.activate`, so mirror-off runs pay nothing. Perf note:
  an active mirror re-serializes on every splice hit regardless of
  mode (audit mode additionally records examples and stacks), so a
  cached serialization under `native_type_mirror` costs one extra
  fresh `Type.write`.
- Mutations of non-family objects (`Overloaded.items`,
  `TypeVarId.meta_level` fields other than through the funnel,
  `ExtraAttrs.attrs` rewrites in place) are visible only through the
  funnel of a family object that embeds them.
- Objects that never reach a `write` funnel before process exit.
