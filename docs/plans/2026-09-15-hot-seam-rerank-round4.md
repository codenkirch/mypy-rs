# Hot-seam re-rank, round 4 (#1673)

Status: measurement + one retirement. Hard counts only; no wall-clock A/B is
claimed. Supersedes the top slots of
`docs/plans/2026-09-14-perf-wire-traffic-audit.md`, which the
#1640/#1648, #1652, #1661, #1662 and #1668 retirements have moved.

- Head: `origin/main` = `506aa7e4c` (2026-09-15).
- Measured tree: `worktrees/mypy-rs-perf4`
  (`/Users/jonathangadeaharder/projects/coding-utils/worktrees/mypy-rs-perf4`);
  `mypy.__file__` was verified against the worktree for every run, since the
  shared `.venv`'s editable install otherwise resolves `mypy` to the main
  checkout.
- Kernel: `cargo rustc -p mypy-type-kernel --features extension-module --lib
  --crate-type cdylib --release` into `/private/tmp/mypy-rs-perf4-tk`
  (1m18s at load 43; the extension was rebuilt from the measured tree).
  `ast_serialize` / `module_resolver` scratch dirs are the standard ones
  (their crates have no commits since 2026-09-13).
- Corpus: cold self-check, `mypy_self_check.ini -n0 --no-incremental
  -p mypy -p mypyc`, 353 files, "Success: no issues found".
- Host load during the runs: 15-46 (`uptime`). Every wall-clock figure below
  is provisional.

## Method (pinned)

Unchanged from the 09-14 audit: wrap every `rust_*` attribute of `type_kernel`
and every module-level `_serialize*` function before `mypy.main` imports them,
key each serializer invocation by the identity of the bytes it returned, and
let a seam call "consume" the blob identity it receives. Each event lands in
exactly one bucket (useful / deferred / unconsumed). Cost proxy:

```
proxy_us = calls * 0.63            # fixed PyO3 crossing
         + call_bytes * 0.04       # payload re-decoded on EVERY wire call
         + encoded_bytes * 0.04    # distinct blob encoded once (Python side)
```

The harness is now committed at `misc/audit_wire_traffic.py` so the ranking is
reproducible (the 09-14 run used an untracked copy in `/private/tmp`). Pinned
command, run from the repo root:

```bash
MYPY_AUDIT_ROOT=$PWD MYPY_SERIALIZE_STATS=1 \
MYPY_AUDIT_ARGS="-n0 --no-incremental --dump-build-stats -p mypy -p mypyc" \
PYTHONPATH=$PWD:/private/tmp/mypy-rs-perf4-tk:/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  .venv/bin/python misc/audit_wire_traffic.py
```

Two additive deviations from the 09-14 harness, neither touching the method:
`--dump-build-stats` + `MYPY_SERIALIZE_STATS=1` now also emit the global
serialize totals, and `MYPY_AUDIT_ROOT` drops the `.venv` editable finder when
it resolves `mypy` outside the requested root.

Run-to-run stability: two independent runs on the unmodified tree agree
exactly on the target seam's direct counters (177,899 calls / 177,524 defers in
both). The pre-fix id-keyed tables were less stable than the 09-14 audit's
"low-double-digit noise" claim suggested; see the harness-fix section.

## Headline buckets

**These are the pre-fix-keying numbers and they undercount.** The harness
keyed its buckets by bare `id(bytes)`; a consumed blob whose reference was
dropped could hand its number to a fresh allocation and be read as a
wire-cache hit. The one-file corpus count moves 106,087 -> 198,963 events once
the keying is fixed, so treat every id-keyed row below as a lower bound. The
seam call/defer counters and the `MYPY_SERIALIZE_STATS` funnel counters are
direct counters, not id-keyed, and are unaffected.

| bucket | before (pre-fix keying) | after (pre-fix keying) | fixed keying, post-retirement |
|---|---:|---:|---:|
| serialization events | 1,297,835 | 1,223,049 | 2,177,749 |
| useful (consumed, decided) | 854,968 | 894,289 | 1,612,939 (67,334,928 B) |
| deferred (serialized, seam said `None`) | 137,443 | 659 | 1,220 (295,687 B) |
| unconsumed (never reached a seam) | 305,424 | 328,101 | 564,590 (11,109,651 B) |

The before/after pair above also moves in the wrong direction for the wrong
reason: with a consumer removed, fewer blobs get marked consumed, so unrelated
cache-hit re-serializations counted as new events. No bucket differential is
claimed from that pair; the retire/keep evidence is the seam-level counters
below.

Global `MYPY_SERIALIZE_STATS` totals (the issue's named counter; own
instrumentation, immune to the keying):

| counter | before (506aa7e4c) | after (rebased head) | delta |
|---|---:|---:|---:|
| `serialize_calls` | 2,860,627 | 2,424,137 | -436,490 (-15.3%) |
| `serialize_writes` | 1,207,503 | 1,047,142 | -160,361 (-13.3%) |
| `serialize_bytes` | 50,410,933 | 36,294,640 | -14,116,293 (-28.0%) |
| `serialize_hits` | 1,174,621 | 938,517 | -236,104 (-20.1%) |
| `serialize_builtin` | 153,520 | 113,373 | -40,147 |

The after column is the rebased head, so it carries 12 commits of other lanes'
work as well as this gate; the reduction is therefore a rough figure, not a
controlled A/B. The controlled A/B is the seam pair below.

## Refreshed top-15, pre-retirement (measured at `506aa7e4c`, pre-fix keying)

Calls, defers and the proxy's fixed-cost term are direct counters. The MB
columns are id-keyed and therefore undercounts (see the headline section); they
are shown because the target choice used the ranking's order, which the calls
column drives.

| # | seam | calls | call MB | enc MB | B/call | proxy | wire? | defers | defer % |
|--:|---|--:|--:|--:|--:|--:|---|---:|--:|
| 1 | `rust_check_callable_call` | 177,899 | 29.73 | 10.91 | 167.1 | 1.74s | wire | 177,524 | 99.79% |
| 2 | `rust_copy_modified` | 262,931 | 19.16 | 5.80 | 72.9 | 1.16s | wire | 0 | 0.00% |
| 3 | `rust_expand_type` | 117,347 | 10.80 | 6.32 | 92.0 | 0.76s | wire | 0 | 0.00% |
| 4 | `rust_flatten_nested_unions` | 276,056 | 9.74 | 3.77 | 35.3 | 0.71s | wire | 1 | 0.00% |
| 5 | `rust_check_overload_call` | 14,477 | 5.92 | 2.37 | 408.6 | 0.34s | wire | 128 | 0.88% |
| 6 | `rust_analyze_instance_member_dispatch` | 100,707 | 3.43 | 0.70 | 34.1 | 0.23s | wire | 35 | 0.03% |
| 7 | `rust_freshen_function_type_vars` | 22,866 | 3.59 | 1.66 | 156.9 | 0.22s | wire | 0 | 0.00% |
| 8 | `rust_classify_analyze_var` | 98,990 | 3.55 | 0.44 | 35.9 | 0.22s | wire | 0 | 0.00% |
| 9 | `rust_is_subtype` | 25,618 | 3.77 | 1.18 | 147.1 | 0.21s | wire | 32 | 0.12% |
| 10 | `rust_solve_generic_call` | 9,127 | 3.01 | 1.47 | 329.6 | 0.18s | wire | 171 | 1.87% |
| 11 | `rust_is_subtype_batch` | 11,202 | 3.77 | 0.01 | 336.4 | 0.16s | wire | 24 | 0.21% |
| 12 | `rust_has_abstract_type` | 243,936 | 0.00 | 0.00 | 0.0 | 0.15s | live | 0 | 0.00% |
| 13 | `rust_classify_simple_assignment` | 30,610 | 1.87 | 1.33 | 61.3 | 0.15s | wire | 42 | 0.14% |
| 14 | `rust_map_actuals_to_formals` | 218,049 | 0.00 | 0.00 | 0.0 | 0.14s | live | 0 | 0.00% |
| 15 | `rust_check_argument_count` | 217,902 | 0.00 | 0.00 | 0.0 | 0.14s | live | 0 | 0.00% |

334 seams were called; 45 rows are above the emitter's table cut. The 09-14
rows `rust_has_recursive_types`, `rust_callable_is_generic`,
`rust_can_be_true/false_default`, `rust_callable_is_var_arg`,
`rust_callable_min_args`, `rust_classify_check_arg` and
`rust_is_literal_type_like` are gone from the table entirely (#1648, #1664).

## Reading the refresh

The byte-weighted proxy and the load-invariant counters disagree about the top
slot, and the counters win:

- **`rust_check_callable_call`** is #1 on both. 177,899 crossings, 29.73 MB of
  per-call payload, 10.91 MB of distinct blobs, for **375 decided calls**
  (177,524 defers, 99.79%). The Rust tail (`check_callable_call_tail`) can only
  calibrate a type-object call with exactly one argument whose instance type is
  `builtins.type`; every other call shape serialized the whole callee plus
  every argument type, crossed the FFI, probed the plugin-hook registry, and
  then deferred. Retired here.
- Ranks 2-4 are the documented floors. `rust_copy_modified` (262,931 calls /
  0 defers), `rust_expand_type` (117,347 / 0) and `rust_flatten_nested_unions`
  (276,056 / 1) all decide, and all need a live-object return interface rather
  than a wire round trip; `mypy/copytype.py` is owned by another lane this
  wave. Re-opening them here would be the `#1623` identity work, not a gate.
- Rank 5 `rust_check_overload_call` and ranks 12/14/15
  (`rust_has_abstract_type`, `rust_map_actuals_to_formals`,
  `rust_check_argument_count`) sit inside the `#1642`-floored `check_call`
  cluster: the three live rows are the sequential
  `map_actuals_to_formals -> compute_arg_context -> check_argument_count`
  chain and the per-arg `has_abstract_type` granularity. Counting them here is
  the requested alternative to re-attempting them.
- `rust_is_subtype` / `rust_is_subtype_batch` (ranks 9/11) are `#1618`-#1623
  residual-defer territory, not closeable gates.
- With the fixed keying the deferred bucket is 1,220 events (largest remaining
  call site 225, `checkexpr.py:1069` container type; then 172 and 118). The
  class is exhausted in the sense that no deferred row is a seam worth gating,
  not that the bucket is empty.
- The unconsumed bucket is 564,590 events / 11.11 MB under the fixed keying and
  is still dominated by `types.py:5596` `_restore_list_identity`, the
  identity-restoration keys the 09-14 audit already assigned to `#1623`.
- `mypy/messages.py` is reserved by the coordinator this wave; its rows
  (`rust_format_type_distinctly` 985 calls, `rust_format_type` 716) are
  low-call/high-byte and are not closeable under the fixed-FFI term.

## Retirement: gate `rust_check_callable_call` on its wire shape (#1673)

Fix shape (d): the decision is available on the live objects for free, so the
wire crossing is gated before it is paid.

`_try_native_check_callable_call` now returns early unless

```python
callee.is_type_obj() and len(arg_types) == 1
and is_named_instance(callee.get_instance_type(), "builtins.type")
```

which is exactly the conjunct set the pure-Python calibration two branches
below (`checkexpr.py:3331-3338` / `:3362-3368`) already computes on the live
objects. The gate is a superset of the Rust accept condition, so no decision
moves: the Rust tail additionally defers on `from_concatenate`, non-`builtins.type`
fallbacks, `TypeAliasType` components and "uncertain" force-fallback walks, and
each of those cases fails a gate conjunct.

| counter | before | after | delta |
|---|---:|---:|---:|
| `rust_check_callable_call` calls | 177,899 | 378 | -177,521 (-99.79%) |
| `rust_check_callable_call` defers | 177,524 | 2 | -177,522 |
| `rust_check_callable_call` decided | 375 | 376 | +1 (corpus delta) |
| proxy (fixed term only, both halves direct counters) | 0.11s | 0.0002s | -0.11s |
| deferred bucket events (id-keyed, undercounts) | 137,443 | 659 | not claimed |
| `serialize_calls` / `writes` / `bytes` | 2,860,627 / 1,207,503 / 50,410,933 | 2,424,137 / 1,047,142 / 36,294,640 | -15.3% / -13.3% / -28.0% |

The decided count is the parity check that matters: 375 before, 376 after on the
rebased corpus, i.e. the gate moved no decision (the +1 is the 12 commits of
corpus that landed under the rebase, which add calls of their own). A lower
count would mean the gate rejected a shape Rust decides. The 378 remaining
crossings are those 375-376 decided calls plus 2 Rust-side defers (alias or
force-fallback uncertainty inside the shape the gate admits).

Only the seam counters and the funnel counters are used for the verdict: the
call/defer counts are direct per-crossing counters, and the funnel counters come
from mypy's own instrumentation. Both are immune to the id-keying defect; the
id-keyed bucket pair is not and is not claimed.

`rust_resolve_plugin_hook` is **not** a counterexample: 30,591 -> 30,623 calls
(unchanged). Its crossings come from the Python `_try_native_plugin_hook` tail
that runs for every call with a `callable_name`, not from the gated seam, whose
internal probe the remaining 378 calls no longer reach at scale.

Current ranking, post-retirement, fixed keying, rebased head:

| # | seam | calls | call MB | enc MB | proxy | defers |
|--:|---|--:|--:|--:|--:|---:|
| 1 | `rust_copy_modified` | 263,463 | 19.20 | 12.92 | 1.45s | 0 |
| 2 | `rust_expand_type` | 117,749 | 10.85 | 9.28 | 0.88s | 0 |
| 3 | `rust_flatten_nested_unions` | 276,228 | 9.75 | 6.55 | 0.83s | 1 |
| 4 | `rust_check_overload_call` | 14,490 | 5.94 | 3.32 | 0.38s | 128 |
| 5 | `rust_freshen_function_type_vars` | 23,030 | 3.61 | 3.61 | 0.30s | 0 |

`rust_check_callable_call` is out of the table entirely: 378 calls, 2 defers.
Every remaining top row is floored: ranks 1-3 on the live-object return
interface (`#1623`), rank 4 in the reserved `#1642` cluster.

Engagement and non-regression proof: `NativeCheckCallableCallWireGateSuite`
(`mypy/test/testtypes.py`) asserts that six rejected shapes (non-type-object
callee, two positional arguments, no positional arguments, non-`builtins.type`
`instance_type`, tuple return type, alias return type) cross **zero** times
through the live gate, that the **ungated** Rust seam returns `None` for every
one of those shapes, and that the one accepted shape still crosses and is
decided by Rust.

## Provenance of the counter pair

The before/after pair is `/private/tmp/mypy-rs-perf4-before.out` (23:04) and
`/private/tmp/mypy-rs-perf4-after.out` (23:50); `perf4-audit1.out` (22:58) is a
third run of the before half used as the stability check.

- **Both halves are worktree-sourced.** Every run exported
  `MYPY_AUDIT_ROOT=<worktree>`, and the harness refuses to start unless
  `mypy.__file__` resolves under that root (the shared `.venv`'s editable
  install otherwise maps `mypy` to the main checkout). Independently verified
  outside the harness:
  `PYTHONPATH=<worktree>:<scratch .so dir> .venv/bin/python -c "import mypy;
  print(mypy.__file__)"` prints the worktree path. The dumps themselves do not
  carry the root; the harness now prints it (added after these runs, no effect
  on the measured path).
- **The two halves measured different trees**, which is the point: the before
  half was taken at 23:04 on the un-gated tree (gate commit `f9f2b6a95` landed
  23:34), and the differential is the evidence, `rust_check_callable_call`
  177,899 calls / 177,524 defers before against 377 / 2 after.
- One asymmetry, disclosed above: `mypy/test/testtypes.py` is in the corpus and
  gained this PR's suite between the halves, so the global funnel deltas are a
  lower bound.

### Harness fixes after the review

The review of `misc/audit_wire_traffic.py` found two defect classes, both of the
"silently wrong numbers" kind that this wave's evidence rule cannot tolerate.
Neither is a style nit, and all of them are fixed by construction rather than by
a guard clause:

1. **Bookkeeping that assumed the happy path.** `report()` ran only after a
   `SystemExit`, so any other exception from the audited run discarded the whole
   output; a wrapped seam that raised never incremented `seam_calls`. `report()`
   now runs in the `finally` block and `seam_calls[name] += 1` precedes the call,
   so no accounting path is reachable only on success.
2. **Bookkeeping that assumed `id()` is a stable identity.** The buckets were
   keyed by bare `id(bytes)`; once a consumed blob was freed, a fresh allocation
   could reuse its number and be read as a wire-cache hit, silently dropping
   events. Every registered blob is now held by a strong reference for the
   process lifetime, so a tracked id cannot be recycled.

Also removed: two structural zeros printed as measured quantities
(`unsourced_seam_bytes`, whose branch was unreachable, and
`re_pushed_already_snapshotted`, which was never incremented), the unused
`useful_pair` counter, and the dead `_bytes_len` helper.

**Behavioural evidence** (fixtures in `/private/tmp/mypy-rs-perf4-check-*.py`,
run against the old harness copy and the fixed one):

| check | old | fixed |
|---|---|---|
| register+consume+drop over 4,000 equal-length blobs, events recorded | 12 | 4,000 |
| wrapped seam that raises, `seam_calls` | 0 | 1 |
| injected `RuntimeError` in the audited run, report emitted | no | yes |
| one-file corpus (`mypy/util.py`) serialization events | 106,087 | 198,963 |

The last row is why the bucket tables above are labelled undercounts: the old
keying was dropping roughly half of all events. The seam `calls`/`defers`
columns and the `MYPY_SERIALIZE_STATS` funnel counters are direct counters and
are byte-identical across the old and fixed harness on the same corpus
(`rust_flatten_nested_unions` 25,559 calls / 1 defer, `rust_copy_modified`
16,557 / 0, `re_pushed_builtins` 4,739 in all runs).

## Caveats

- Load-invariant counters are the verdict here. The `--dump-build-stats` wall
  figures in the same dumps (`resolver_build_s` 0.81-1.13s, total run wall
  102.1s before / 111.8s after at host load 15-69) are **provisional and
  unused**: load moved 69 -> 51 across the runs, and the proxy is an
  attribution model calibrated on `#1624`'s own figures, not a measurement.
- The `_collect_incremental` probe reports `re_pushed_builtins` 53,563 with
  `resolver_build_s` 0.81-1.13s. The probe counts infos **fed** to
  `resolver.update`, before the `#1652` dirty-signature filter, so it does not
  measure post-#1652 re-snapshots; no claim is made about #1652 from these
  numbers.
- The harness keyed its buckets by `id(bytes)` until this PR's last commit; the
  id-keyed tables here are pre-fix and undercount (demonstrated 106,087 ->
  198,963 events on a one-file corpus). Seam `calls`/`defers` and the funnel
  counters are direct counters and are unaffected. The **fixed** keying holds
  every registered blob alive for the process lifetime, so a full-corpus run
  carries extra resident memory that the pre-fix runs did not; the measured
  files are ~50 MB of distinct encoded bytes, and the corrected full-corpus run
  completed cleanly at 353 files.