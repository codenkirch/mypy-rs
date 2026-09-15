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
exactly on the target seam (177,899 calls / 177,524 defers in both) and within
low-double-digit noise elsewhere (the documented `id(bytes)` reuse effect).

## Headline buckets

| bucket | before | after (this PR) | delta |
|---|---:|---:|---:|
| serialization events | 1,297,835 | 1,223,049 | -74,786 |
| useful (consumed, decided) | 854,968 (39,454,415 B) | 894,289 (41,339,294 B) | +39,321 |
| deferred (serialized, seam said `None`) | 137,443 (11,032,608 B) | 659 (149,011 B) | -136,784 |
| unconsumed (never reached a seam) | 305,424 (6,101,565 B) | 328,101 (7,015,042 B) | +22,677 |

The `useful` / `unconsumed` movement is a **re-attribution, not new traffic**.
The harness marks a blob consumed once and then treats every later
serialization returning that same object as a wire-cache hit, not a new event.
Removing a consumer therefore promotes cache-hit re-serializations at unrelated
call sites into counted events (`types.py:5827` 158,647 -> 172,893,
`subtypes.py:880` 14,904 -> 21,109). Per-site bucket rows are not comparable
across this change; the seam-level counters and the global funnel counters below
are.

Global `MYPY_SERIALIZE_STATS` totals (the issue's named counter):

| counter | before | after | delta |
|---|---:|---:|---:|
| `serialize_calls` | 2,860,627 | 2,443,622 | -417,005 (-14.6%) |
| `serialize_writes` | 1,207,503 | 1,050,916 | -156,587 (-13.0%) |
| `serialize_bytes` | 50,410,933 | 36,344,093 | -14,066,840 (-27.9%) |
| `serialize_hits` | 1,174,621 | 948,900 | -225,721 (-19.2%) |
| `serialize_builtin` | 153,520 | 119,002 | -34,518 |

These come from mypy's own funnel instrumentation, not from the harness's
identity keying, so the re-attribution above does not touch them. One confound:
`mypy/test/testtypes.py` is inside the self-check corpus and this PR adds ~150
lines to it, so the after column carries that extra workload; the reduction is a
**lower bound**. The seam's decided count is identical in both runs (375), which
is the direct evidence that the corpus delta does not perturb this seam.

## Refreshed top-15 (before this PR)

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
- The deferred bucket after this retirement is 659 events (-99.5%), i.e. the
  class is exhausted: the largest remaining deferred call site is 157 events
  (`checkexpr.py:1069` container type), then 77 and 73
  (`checkexpr.py:3184/3168`). No deferred row is above the noise floor of a
  single seam worth gating.
- The unconsumed bucket (305,424 -> 328,101 events / 6.10 -> 7.02 MB) is
  unchanged in kind and is 89% one site, `types.py:5596`
  `_restore_list_identity`, the identity-restoration keys the 09-14 audit
  already assigned to `#1623`. Its absolute movement is the re-attribution
  effect described above, not new traffic.
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
| `rust_check_callable_call` calls | 177,899 | 377 | -177,522 (-99.79%) |
| `rust_check_callable_call` defers | 177,524 | 2 | -177,522 |
| `rust_check_callable_call` decided | 375 | 375 | 0 |
| `rust_check_callable_call` call MB | 29.73 | 0.00 (242 B) | -29.73 |
| `rust_check_callable_call` encoded MB | 10.91 | 0.000242 | -10.91 |
| proxy (fixed + bytes) | 1.74s | 0.0002s | -1.74s |
| deferred bucket events / bytes | 137,443 / 11,032,608 | 659 / 149,011 | -136,784 / -10,883,597 |
| `serialize_calls` / `writes` / `bytes` | 2,860,627 / 1,207,503 / 50,410,933 | 2,443,622 / 1,050,916 / 36,344,093 | -14.6% / -13.0% / -27.9% |

The decided count is the parity check that matters: it stays at **375**. A
lower number would mean the gate rejected a shape Rust decides. The 377
remaining crossings are 375 decided calls plus 2 Rust-side defers (alias or
force-fallback uncertainty inside the shape the gate admits).

`rust_resolve_plugin_hook` is **not** a counterexample: 30,591 -> 30,621 calls
(unchanged). Its crossings come from the Python `_try_native_plugin_hook` tail
that runs for every call with a `callable_name`, not from the gated seam, whose
internal probe those 377 calls no longer reach at scale.

Post-retirement top 5 (same run, same corpus):

| # | seam | calls | call MB | enc MB | proxy | defers |
|--:|---|--:|--:|--:|--:|---:|
| 1 | `rust_copy_modified` | 263,049 | 19.17 | 6.15 | 1.18s | 0 |
| 2 | `rust_expand_type` | 117,441 | 10.80 | 6.37 | 0.76s | 0 |
| 3 | `rust_flatten_nested_unions` | 276,105 | 9.74 | 4.43 | 0.74s | 1 |
| 4 | `rust_check_overload_call` | 14,482 | 5.92 | 2.41 | 0.34s | 128 |
| 5 | `rust_analyze_instance_member_dispatch` | 100,760 | 3.44 | 0.71 | 0.23s | 35 |

Every one of them is floored: ranks 1-3 on the live-object return interface
(`#1623`), rank 4 in the reserved `#1642` cluster.

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

The committed harness no longer prints two **structural zeros**: a probe that
reports an unreachable branch as a measured `0` breaks the load-invariant
counter rule this wave runs on. Neither zero is cited anywhere in this document.

- `unsourced_seam_bytes` (the `entry is None` branch in `consume()` is
  unreachable: the only caller passes a blob that `_lookup_blob` returned from
  `pending`): branch and stat line removed.
- `re_pushed_already_snapshotted` (initialized, never incremented; the
  classification loop bumps `re_pushed_builtins` / `other_infos`): key removed.
  The `re_pushed_builtins` figure this document does cite is unaffected.

Also fixed: `report()` now runs in the `finally` block, so an exception from the
audited run can no longer discard the counters, and the unused `useful_pair`
counter was dropped.

**The fixes are output-neutral for every cited number.** Two runs of the fixed
harness on a one-file corpus (`mypy/util.py`) differ from each other by 389 diff
lines; old against fixed differs by 408, i.e. the fix adds no more than run-to-run
`id(bytes)` noise. `re_pushed_builtins` is byte-identical across all three runs
(4,739) and the seam call/defer counts are identical.

The before/after dumps cited above were produced by the pre-fix harness; the
removed lines were always `0` and are not part of any claim.

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
- The harness keys serialization events by `id(bytes)` and does not hold
  consumed ids alive, so id reuse adds low-double-digit noise to the
  per-call-site tables, and removing a consumer shifts events between buckets
  (see the headline section). The seam-level counts and the funnel counters
  are stable across the three runs taken here.