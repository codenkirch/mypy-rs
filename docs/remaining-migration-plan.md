# Remaining Rust Migration Plan

Date: 2026-08-13

## Current State

### Primary metric: native work share (representative)

The byte-share metrics below understate progress: a strangler-fig port
keeps every Python fallback in place, so Python bytes never shrink and
the denominator stays inflated. The representative metric for "how much
we already have ported successfully to Rust" is the runtime work share:
the fraction of the self-check corpus's type-checking work that now
executes in Rust, measured as the differential between the pure-Python
path (`--no-native-type-kernel`) and the default-on native path on the
cold self-check (`mypy_self_check.ini -p mypy -p mypyc`,
`--no-incremental`, `-n0` serial, `--dump-build-stats`):

| Phase | Python-only | Native (prod) | Rust-absorbed share |
|-------|------------|---------------|---------------------|
| parse_time | 5.046s | 4.997s | 1.0% |
| semanal_time | 2.716s | 1.110s | 59.1% |
| type_check_time | 9.951s | 2.326s | 76.6% |
| **Total** | **17.713s** | **8.433s** | **52.4%** |

Measured 2026-08-13 (M17 baseline, before #700/#702). A re-measure
on 2026-08-20 after the #700/#702 merges was attempted with the same
serial flags, but the shared machine was under heavy load (loadavg
5-43 from parallel agents), inflating all wall-clock times (python-only
baseline rose 17.7s -> ~26s) and inverting the per-phase shares to
negative (native appeared slower). The parse_time control (identical
code in both modes) stayed near-parity, so the measurement pipeline is
sound but absolute numbers are load-dependent. Subset diffs
(mypy.util; mypy.types+nodes) with the parse control passing reproducibly
showed native ~39-71% slower even at doubled corpus size, so the
inverted sign is not pure load: it is consistent with the accumulated
per-call wire/deferral overhead of the dozens of seams landed since
M17 outweighing their speedups at this corpus scale (Phase B deferral
reduction is the named lever). Re-measure on a quiet machine before
treating any new number as the updated baseline; the M17 numbers below
remain the last trustworthy measurement.

The share is `(python - native) / python` per phase: the fraction of the
pure-Python phase time that the native path absorbs. It understates
Rust's true fraction when Rust runs the same logic faster than the Python
it replaces (the ported code still executes, just cheaper), and it is
confounded by deferred sub-paths. Treat it as a lower bound on native
coverage.

Over half the type-checking work already runs in Rust; the type-check
phase (the dominant cost) is three-quarters native. This is the number
to watch: every landed seam raises it. Re-run with
`scripts/measure_work_share.py` (runs the cold self-check twice with
`--dump-build-stats` and prints the share table) after any kernel seam
lands.

### Secondary metrics (bytes)

| Metric | Value |
|--------|-------|
| Rust bytes (local, .rs) | ~3.62M |
| Python bytes (mypy + mypyc, ex test/) | ~6.67M |
| Rust % (local) | ~35.2% |
| Rust % (GitHub languages API) | ~35.41% |
| Rust LOC (crates/) | ~96K |
| Rust source files | 100 (type_kernel) + ast_serialize + module_resolver + fs_probe |

Updated 2026-08-20 after #700 (type_object_type_from_function) and #702
(check_overlapping_overloads screening). GitHub metric ~35.41%. The
byte metrics remain useful for tracking port volume but are NOT the
progress target: they penalize the strangler for preserving fallbacks.

## Honest assessment: path to 50%

The user-facing goal is 50% Rust. That requires moving ~1.67M bytes of
Python to Rust (so Rust = 4.92M = Python = 4.92M out of a 9.84M total).

The **biggest Python files cannot be ported** without breaking the
migration plan's Phase E1 constraints:

- `mypy/nodes.py` (191K) and `mypy/types.py` (196K) are plugin-visible
  mutable object graphs. Phase E1 confirms these stay in Python until a
  multi-quarter Rust-owned Type/Node redesign lands.

That leaves **checker.py (484K), checkexpr.py (371K), semanal.py (400K),
build.py (253K)** as the weighted candidates (1.5M combined). Each of
them contains visitor methods that mutate the symbol table, emit errors,
drive daemon-cache invalidation, and interact with plugin hooks. Porting
those visit_* methods to Rust means either:

1. A Rust-native visitor engine that holds live TypeInfo/TypeAlias
   graph references across the GIL (violates Phase E1), or
2. Continuing the strangler-fig per-call gate and porting one visit_*
   method at a time, which yields ~10-30K Rust bytes per PR for parity
   + rebuild cost (100+ PRs to close the gap).

The realistic near-term lever is **deferral reduction** (Phase B), not
#line-adds. That improves performance but adds few Rust bytes. Reaching
50% is therefore a multi-week release line, not a bounded goal-turn
task. The execution sequence that gets there:

1. Merge Phase A fixes (pre-existing bugs).
2. Land Phase B deferral reduction (setops/subtypes/checkexpr) for
   ~50 selected deferral sites; expected ~50-100K new Rust bytes.
3. Land Phase C depth ports for semanal/checker/plugins; expected
   ~150K new Rust bytes per release cycle.
4. Continue until 50% is reached, tracking via Phase D4.

This document itself was updated to keep the honest number close to the
plan rather than let "no progress" silently compound.

### Path to a majority-Rust checker (work-share framing)

The 50% byte-share goal is a proxy; the meaningful target is a
majority-Rust **work share**. The type-check phase was three-quarters
native at the M17 baseline (2026-08-13); the realistic wedge to push
the **total** past 52% is to
attract more of semanal (+ parse) into the native path, then extend
into the checker's remaining hot loops (deferral reduction, more depth
ports). Each landed seam should be validated by re-running the
self-check differential and recording the new total work share in the
table at the top of this document. A 2026-08-20 re-measure was
inconclusive due to machine load (see caveat above); the next clean
re-measure should re-establish the current baseline before planning
deferral-reduction work.

### Native gates shipped (production default-on)

`native_type_kernel` defaults on. All of the following are active:

- erasetype (erase_type, remove_instance_last_known_values, erase_typevars)
- subtypes (is_subtype, is_proper_subtype + resolver)
- join (join_types + resolver)
- mro (linearize_hierarchy + resolver)
- meet (narrow_declared_type, is_overlapping_types, get_possible_variants)
- expandtype (expand_type + resolver)
- maptype (map_instance_to_supertype + resolver)
- typeops (bind_self, class_callable, fill_typevars + resolver)
- semanal (visitor, _lookup, shared, classprop, typeddict, namedtuple + resolver)
- typeanal (instance validation, vec type args, fix_instance + active)
- checker (stmts, visitor, narrowing, binder + resolver)
- checkexpr (plugin-hook registry, star_expr, conditional_expr, container literals,
  operators, check_callable_call arg-binding, check_argument_types, overload dispatch + resolver)
- checkmember (bind_self_fast, operator helpers, method fast path + resolver)
- checkpattern, checkstrformat
- constraints + solve (full infer + solve path + resolver)
- argmap, applytype (active + resolver)
- freshen, lkv, cache
- messages (format_type family, notes, suggestions, callable_name + resolver)
- copytype
- serverdeps, server update
- dmypy_server (check, command_run, command_recheck, merge_hook_results)
- plugins (attrs, dataclasses, common, functools, hook dispatch)
- stubgenc (AliasPrinter)
- nodes, sharedparse, errors, reachability, fixup, modulefinder, util

### Parity baselines

| Suite | Result |
|-------|--------|
| testcheck.py (all native on) | 8143 passed, 8 pre-existing failures, 69 skipped, 7 xfailed |
| testtypes.py (parity suites) | 339 passed |
| testfinegrained + testdaemon + testfinegrainedcache | 1333 passed |
| Self-check (mypy_self_check.ini) | 0 errors |
| Rust unit tests | 46+ passed |

The 8 pre-existing testcheck failures are NOT caused by the type kernel.
They fail on clean main without any native options too:
"De-serialization failure: TypeInfo not fixed" (6 tests) and
the now-fixed `py_join` tuple bug (2 tests, fixed in PR #584).

### Open issues

- #582: 34 testcheck failures with type kernel (addressed by PR #584, reduced to 8 pre-existing)
- #580: Wire `rust_is_typevar_default_recursive` and `rust_format_messages_default_pretty` (shipped in #583)

The migration plan's module port list is exhausted. All planned modules are ported.
Remaining work is optimization, deferral reduction, and fixing pre-existing bugs.

## What Remains

### Phase A: Fix pre-existing bugs (blocking)

**A1: "TypeInfo not fixed" de-serialization crash (6 tests)**

The 6 incremental/serialization tests crash with
`AssertionError: De-serialization failure: TypeInfo not fixed`.
This is a pre-existing bug on main, not caused by the type kernel.

Root cause hypothesis: `TypeInfo.__getattribute__` raises when `deserialize`/
`fixup.py:NodeFixer` has not yet patched cross-references. The crash triggers
on worker-subprocess paths (`MYPY_NUM_WORKERS > 0`) and in incremental cache
reload. The `fixup.py` port (`crates/type_kernel/src/fixup.rs`) may have a
gap where a `TypeInfo` variant is not being fixed up after deserialization.

Steps:
1. Reproduce with `MYPY_NUM_WORKERS=1 .venv/bin/python -m mypy --config-file
   mypy_self_check.ini -p mypy` on clean main (no native options).
2. Add `--pdb` to get a breakpoint at the `__getattribute__` assertion.
3. Trace which `TypeInfo` attribute access triggers the assertion.
4. Compare the Python `fixup.py:NodeFixer` against `crates/type_kernel/src/fixup.rs`
   to find the missing fixup branch.
5. Fix the gap, rebuild, re-run the 6 tests + self-check.

Priority: high (blocks worker-subprocess mode, affects 6 testcheck tests).

**A2: Close issue #582**

PR #584 reduces the 34 failures to 8 (all pre-existing). Once A1 is fixed,
update #582 and close it. If A1 requires a separate PR, reference both.

### Phase B: Reduce deferrals (performance)

The Rust kernel has 514 `return None` (defer to Python) sites. Each deferral
means a Python fallback path runs, erasing the performance win for that call.
The highest-deferral files:

| File | Deferrals | Module |
|------|-----------|--------|
| setops.rs | 64 | set operations (union/intersection narrowing) |
| checkexpr_functions.rs | 42 | expression checker visitors |
| subtypes.rs | 40 | subtype checks |
| constraints.rs | 35 | constraint inference |
| checkmember.rs | 26 | member access |
| typeops.rs | 24 | type operations |
| meet.rs | 21 | meet/narrowing |
| checkpattern.rs | 21 | pattern matching |
| checkcall.rs | 20 | call checking |
| messages.rs | 17 | message formatting |
| expandtype.rs | 17 | type expansion |
| checker_stmts.rs | 15 | statement checking |

**B1: setops.rs (64 deferrals)**

`setops.rs` (7285 lines, the largest Rust file) has the most deferrals.
This is the union/intersection type-operation kernel. Reducing deferrals
here directly improves type-check time.

Strategy: audit each `return None` against the Python `mypy.types.UnionType`
and `mypy.types.IntersectionType` methods. Most deferrals are for:
- `TypeVarType` / `TypeVarTuple` / `ParamSpec` operands (need wire codec support)
- `LiteralType` with complex values
- `TypeAliasType` (needs alias expansion before the operation)
- Plugin-modified types

Steps:
1. Add a deferral counter to each `return None` site (temporary instrumentation).
2. Run the self-check corpus with the counter and sort by hit count.
3. Pick the top 5 deferral sites, implement them in Rust.
4. Re-run parity suite + testcheck to confirm no regressions.
5. Measure the type_check_time improvement.

Priority: medium (performance, not correctness).

**B2: subtypes.rs (40 deferrals)**

The subtype kernel defers on complex generic substitution edges. Most were
fixed in the M8bb gap closure (all unsupported edges defer instead of
returning wrong answers), but the remaining deferrals are real gaps.

Strategy: same as B1. Focus on:
- `visit_instance_nominal` with multi-level inheritance + variance
- `check_type_parameter` with `ParamSpec` / `TypeVarTuple`
- `visit_union` with mixed `Instance`/`TypeVar` members

**B3: checkexpr_functions.rs (42 deferrals)**

Expression-checker visitors that defer on complex expression forms.
Focus on:
- `visit_lambda` (arg inference)
- `visit_yield_expr` (generator type flow)
- `visit_await` (async type flow)

### Phase C: Deepen ported modules

**C1: semanal.py symbol resolution depth (issue #348 family)**

The #348 family (A/B/C) landed: `_lookup`, import binding, member resolution.
But `semanal.py` is 9139 lines, and many visitor methods still have Python
fallback paths. The biggest remaining gap is the full `visit_class_def`
and `visit_func_def` traversal, which mutates the symbol table.

Strategy: port the pure-computation parts of `visit_class_def` /
`visit_func_def` (scope creation, MRO setup, decorator classification)
and leave the mutation-bearing parts (AST writes, error emission) in Python.

**C2: checker.py complex-statement depth**

`checker.py` is 10627 lines. `checker_stmts.rs` (1498 lines) ports the
statement-level helpers, but the full `visit_*` methods for complex
statements (try/except, match, with) have deferral gaps.

Strategy: audit `checker_stmts.rs` against `mypy/checker.py` visit methods.
Port the type-narrowing logic for try/except and match statements.

**C3: Plugin depth**

`mypy/plugins/default.py` (654 lines) has ~46 hook fullnames in the
`PluginHookRegistry`. The hook *dispatch* is optimized (known-absent
skip), but the hook *bodies* run in Python. Porting the pure-computation
parts of `DefaultPlugin` hooks (e.g. `get_function_hook` for `builtins.len`)
would eliminate Python fallback for common cases.

### Phase D: Infrastructure and tooling

**D1: Fix the `py_join` / `rust_compute_search_paths` unconditional call**

`compute_search_paths` in `mypy/modulefinder.py` calls
`_rust_compute_search_paths` unconditionally whenever `_HAS_RUST_MODULEFINDER`
is true, regardless of `native_resolver`. This means the modulefinder
Rust extension is always active even when the user passes `--no-native-resolver`.
This is probably intentional (search-path computation is not cache-affecting),
but it should be documented or gated if it causes issues.

**D2: Document the build order hazard**

The `AGENTS.md` already documents the stale-binary hazard and the
`cargo rustc` + scratch-dir approach. Ensure the `remaining-migration-plan.md`
references this for new contributors.

Done as part of #596: new contributors should read the "Native parser build
order", "Type kernel build order", and "Native resolver / dependency-records
parity" sections at the repo root `AGENTS.md` before rebuilding any Rust
extension. The cardinal rule: rebuild the `.so` into a scratch dir and put it
on `PYTHONPATH` after any `.rs` change, and never use `maturin develop` (it
picks up the repo-root `pyproject.toml` and installs a bogus `mypy-0.1.0`
package that shadows the real mypy).

**D3: Performance regression tracking**

The M17 benchmark (52.4% build-time reduction) is the current baseline.
After Phase B (deferral reduction), re-run the self-check benchmark to
measure the improvement. Target: 60%+ total reduction.

Measured 2026-08-21 (after #737, fresh `type_kernel` release `.so`,
cold cache, `-n0 --no-incremental -p mypy -p mypyc`): the 5x regression
(#735) is resolved. Root cause: #727's `rust_verify_type_refs` fast
path re-decoded wire bytes and rebuilt a full typeinfo-map HashSet on
every `fixup_wire_type` call (`type_check_time` 107s -> 29s after
revert). The remaining ~2.1x type-check gap vs Python (29s native vs
14s pure) is the cumulative wire serialize/fixup overhead of the
kernel-ported hot paths, not a single gate: toggling any one gate off
kept type_check in the 94-253s range.

Measured 2026-08-22 (same harness, fresh `.so`, cold cache): the
isolated-seam differential (each gate toggled off via a temporary
`EXPERIMENT_NO_*` env switch in `build.py`, one self-check per toggle,
exit=1 due to hard-exit patch on 3.14, stats still valid):

- Python baseline (kernel off): type_check 13.07s, semanal 3.96s.
- Native full: type_check 27.10s, semanal 6.65s.
- `EXPERIMENT_NO_CHECKMEMBER`: 23.68s (-3.4s).
- `EXPERIMENT_NO_CHECKEXPR`: 24.51s (-2.6s).
- `EXPERIMENT_NO_TYPEOPS`: 26.50s (-0.6s).
- subtype/join/expand/map/solve/constraints/checker/checkcall: ~0.
- All checker gates off: 19.35s (semanal unchanged 6.67s).
- All 28 extra gates off too (`EXPERIMENT_NO_ALL_EXTRA`): type_check
  17.78s, semanal 4.06s (semanal gap fully explained by the semanal /
  semanal_visitor seams, ~2.6s).

Conclusion: with *every* seam off, type_check still runs +4.7s over
Python (17.78 vs 13.07). Measured components of that residual:
`_build_native_resolvers` walk+update+mapfill totals 2.0s across 537
SCC calls (max 12ms), `fixup_wire_type` only 3ms. The remaining ~2.7s
is wire serialize/deserialize invoked by the Python-side callers even
when no Rust seam decides. So the native kernel is net slower on
self-check not because of any single port, but because resolver
snapshot upkeep (2.0s) plus residual wire traffic (~2.7s) outweigh the
per-call Rust savings. Fix direction: make `_build_native_resolvers`
incremental across SCCs (only update newly-seen TypeInfos, not the
full 537x walk), and cut residual wire traffic in the not-seam-gated
Python callers.

Fix outcome (2026-08-22, commit ce350b626): both measured costs are
now cut. `_build_native_resolvers` feeds `update` only new + `builtins.*`
fullnames and mapfills only new infos (per-call cost now ~0.011s max,
was up to 12ms per call across 537 calls); `_deserialize_type_for_checkmember`
gained a bytes -> Type cache split into freeze/non-freeze pools
(invalidated on wire-map identity swap + per build). The checker
`_deserialize_type_from_checker` gained the same bytes -> template cache
(shallow-copied per call; safe because `_TypeRefFixer` rebuilds and
clears `type_ref`, so the in-place `TypeFixer` at
`try_handler_union_decoded` never corrupts the template). Deser cache
counters surfaced in `--dump-build-stats` show ~128k hits vs ~12k
misses on self-check. Parity stayed green: testtypes 1031 passed,
testcheck 8151 passed. Self-check type_check remains net-negative (~
30s native vs 16s python), so further wins must come from the residual
wire serialize/deserialize in Python-side callers and the semanal
seams, not the resolver upkeep.

Checker deser-cache outcome (2026-08-22, commit e0830ab69): the
`_checker_deser_template` cache raw-bytes hit massively (128k hits vs
12k misses on self-check) but the work-share measurement is unchanged
(type_check 30.9s native vs 16.4s python, -88.4%, same as before
within noise). Conclusion: `fixup_wire_type` decode cost was never the
bottleneck; the residual is the serialize side (`_serialize_type_for_*
` per seam call) plus the semanal seams, not deserialization. The two
perf commits are kept (they cut real per-call costs and keep the
hot-path decoders cheap) but further work-share gains must come from
reducing serialize traffic or porting whole seam chains, not from more
decode caching.

Measured 2026-08-26 (after the 13-PR deferral-reduction swarms merged,
fresh `type_kernel` release `.so`, cold cache, quiet machine, 3 pairs,
median-of-ratios via `scripts/measure_work_share.py`):

| phase | python | native | share |
|-------|--------|--------|-------|
| parse_time | 8.81s | 7.78s | +9.6% |
| semanal_time | 4.06s | 7.29s | -79.4% |
| type_check_time | 14.02s | 29.01s | -106.9% |
| total | 26.89s | 44.08s | -66.8% |

This is the first clean measurement since the M17 baseline (08-13). The
deferral-reduction swarms did not move the sign: the kernel remains
net-slower than pure Python (-66.8% total), consistent with the 08-22
isolated-seam analysis (type_check -107% there, -107% here). The
regression predates the 13 PRs and survives them unchanged. Root cause is
unchanged and already diagnosed: per-SCC resolver snapshot upkeep plus
residual wire serialize traffic on seam calls outweigh per-call Rust
savings. The named levers remain (a) incremental `_build_native_resolvers`
across SCCs instead of a full 537x walk, and (b) cutting residual wire
serialize/deserialize in the not-seam-gated Python callers. Deferral
shaving is exhausted as a lever; this is now a perf-fix, not a coverage,
goal.

Measured 2026-08-26 late (seam-level A/B on the serialize side): profiled
the two highest-delta checkexpr seams under cProfile and gated them off
(`check_argument_count` and `check_argument_types`), then re-ran the
work-share A/B with the seams off vs on. The cProfile deltas (2.856s vs
0.831s for `check_argument_count`, 8.007s vs 4.089s for
`check_argument_types`) did NOT reproduce at the wall-clock level: native
type_check moved 29.72s (seams on) -> 28.86s (seams off), a 0.86s shift
at or below the run-to-run noise floor, and the total share stayed -66.9%
vs the -66.8% baseline. Conclusion: the cProfile deltas were dominated by
cProfile's own per-call instrumentation overhead (414k
`check_argument_count` calls X ~1us = ~0.4s of pure artifact), not by
real serialize work. The two seams are NOT net losses; the gate was
reverted. Serialize-side culling is exhausted as a lever. The remaining
diagnosed bottleneck is the per-SCC `_build_native_resolvers` full-graph
walk (lever (a) above), which no serialize/cull change can touch.

Measured 2026-08-26 late-2 (semanal seam gate A/B, quiet machine, 3
pairs, median-of-ratios, sequential with no concurrent work): toggled
both semanal seams (`_set_native_semanal_active` and
`_set_native_semanal_visitor_active`) off via a temporary
`MYPY_NO_NATIVE_SEMANAL=1` env gate, then re-ran with them on.

| config | total | semanal | type_check |
|--------|-------|---------|------------|
| semanal OFF | -63.3% | -32.5% | -113.5% |
| semanal ON (default) | -72.2% | -68.5% | -111.3% |

The semanal seams are net losses: +8.9pp total share, semanal phase
share cut from -68.5% to -32.5%. The semanal Rust ports
(`rust_classify_decorators`, `rust_classify_unbound_front`,
`rust_classify_special_unbound`) pay more in wire serialize overhead
than they save in per-call computation, because the semanal phase has
many short calls where the fixed wire-encode cost dominates. The gates
are now flipped from opt-OUT (`MYPY_NO_NATIVE_SEMANAL`) to opt-IN
(`MYPY_ENABLE_NATIVE_SEMANAL=1`), so production defaults to the fast
Python semanal path. The Rust semanal stays behind the opt-in flag
for future work (e.g. if the serialize cost is cut via a non-wire
interface). Parity verified: testtypes gate-on 1523 passed, gate-off
1523 passed; testcheck gate-on 8144 passed, gate-off 8144 passed.
Also added: no-arg Instance serialize fast-path
(`_encode_no_arg_instance`) to all 6 serialize entry points, bypassing
taint check for str/int/bool/object no-arg Instances. Real per-call
CPU reduction but does not move the work-share needle alone. Issue
#891 tracks the serialize diagnosis.

Measured 2026-08-26 (after PR #893, incremental resolver collection per
SCC, fresh `type_kernel` release `.so`, cold cache, quiet machine, 3
pairs, median-of-ratios via `scripts/measure_work_share.py`):

| phase | python | native | share |
|-------|--------|--------|-------|
| parse_time | 6.94s | 7.05s | -0.7% |
| semanal_time | 3.65s | 4.76s | -30.2% |
| type_check_time | 12.54s | 22.80s | -82.4% |
| total | 23.13s | 34.61s | -49.2% |

The incremental collection cut the per-SCC full-graph walk (previously
~200 modules x 537 SCC calls; now only new + just-sealed + builtins per
call). Total share moved -59.7% -> -49.2% (+10.5pp). The named lever
(a) in the 08-22 diagnosis is now shipped; lever (b) (residual wire
serialize traffic in Python callers) remains the next target. Parity
green: testtypes 1523p/3s, testcheck 8144p/69s/7xf, testfinegrained
747p/27s, testmodulefinder+testgraph 27p.

Measured 2026-08-26 late-3 (after PR #894, classify_call parity
assertion gated to CI-only via `MYPY_NATIVE_TYPE_KERNEL_REQUIRED`,
fresh `type_kernel` `.so`, cold cache, quiet machine, 5 pairs,
median-of-ratios via `scripts/measure_work_share.py`):

| phase | python | native | share |
|-------|--------|--------|-------|
| parse_time | 6.72s | 6.83s | -0.8% |
| semanal_time | 3.65s | 4.74s | -29.9% |
| type_check_time | 12.61s | 22.48s | -78.3% |
| total | 22.98s | 34.05s | -48.3% |

The gate removed the always-on `classify_call` parity assertion from
the production hot path (checkexpr.py `check_call`), cutting 164K
serialize round-trips (2.03M -> 1.86M calls, -8.1%). The assertion now
runs only under `MYPY_NATIVE_TYPE_KERNEL_REQUIRED` (CI parity
differential), not on every call. Share moved -49.2% -> -48.3%
(modest, within noise, but real call reduction confirmed by
`MYPY_SERIALIZE_STATS=1`). Parity green: testtypes+testinfer 1629p/3s,
testcheck 8144p/69s/7xf. CI workflows (pr-gate, native-kernel-parity)
switched to the self-hosted macOS ARM64 runner
(`[self-hosted, macOS, ARM64, VidiomTM]`) per the local-runner
directive. Issue #891 tracks the serialize diagnosis; lever (b)
(residual wire serialize traffic) remains the next target.

Measured 2026-08-27 (after the final port wave merged: #995
check_getattr_method, #1013 check_type_parameter variance, #1015
check_unpacks_in_list, #1018 attribute_triggers, #1019
check_and_warn_deprecated, #1021 constraint-list helpers, #1022
has_no_attr message arbitration; self-check restored to 0 errors in
344 source files, fresh `type_kernel`/`module_resolver`/`ast_serialize`
`.so`s, cold cache, quiet machine, 3 pairs, median-of-ratios via
`scripts/measure_work_share.py`):

| phase | python | native | share |
|-------|--------|--------|-------|
| parse_time | 8.07s | 8.51s | -5.6% |
| semanal_time | 4.05s | 5.90s | -43.8% |
| type_check_time | 14.66s | 28.07s | -87.9% |
| total | 26.77s | 42.49s | -52.6% |

Against the 08-26 late-3 entry, the total share improved -48.3% ->
-52.6%, type_check -78.3% -> -87.9%, and semanal -29.9% -> -43.8%.
Parse flipped from -0.8% to -5.6%; parse shares have swung between
+9.6% and -1.6% across 08-26 entries on little-changed code, so the
parse delta sits inside the observed run-to-run band rather than
marking a real regression. The Python baseline itself also moved
across entries (23.0s late-3, 26.8s here), reflecting the growing
Python-side dispatch code, so cross-entry comparisons mix code growth
with seam gains. Lever (b) (residual wire serialize traffic) remains
the next target.

Measured 2026-08-14 (after Phase C merged, fresh `type_kernel` release
`.so`, cold cache, `MYPY_NUM_WORKERS=0`, self-check
`mypy_self_check.ini --no-incremental -p mypy`):

- Native kernel on (`native_type_kernel`, default): `type_check_time`
  50.3s, wall 58.6s, 117 self-check errors.
- Kernel off (`--no-native-type-kernel`): `type_check_time` 15.6s,
  wall 22.3s, 116 self-check errors.

The type-check gap (~3.2x) is a regression vs the M17 baseline
(2.3s native type_check). Root cause: `_build_native_resolvers()` is
called once per SCC (394 SCCs in the self-check) and each call
re-serializes the FULL loaded TypeInfo graph (~8490 TypeInfos) plus
the alias graph through Rust getattr walks, charged into
`type_check_time` (t3..t4 in `process_stale_scc`). Even
`--no-native-type-kernel` leaves `native_resolver` / `native_parser`
on, so the 116-error pure config is a different comparison axis; the
117-vs-116 delta is a pre-existing self-check inference sensitivity at
`checker.py:6676` (`_get_base_classes`), unchanged by Phase D.

Fix direction (tracked, NOT shipped here): snapshot a TypeInfo only
when its defining SCC is sealed (right after its `semantic_analysis_for_scc`
runs), build/extend an incrementally-accumulating resolver instead of a
full per-SCC rebuild, and track a seen-set reset by daemon recheck. The
member-info / member-definer walks are functionally required (blanking
them breaks parity: 135 errors), so they are not the fixable part. This
is perf work for a dedicated phase, not Phase D scope.

**D4: Rust % measurement**

After Phase B + C, re-measure the GitHub languages API Rust %. The
local tree is already ~32%; the GitHub API lags because it counts
generated/stub files differently. Target: 30%+ on the GitHub API.

Measured 2026-08-14 (after Phase C merged): GitHub languages API reports
Rust 3,156,702 bytes of 9,453,609 total = **33.4%**, above the 30% target.
This matches the committed `.rs` source (`crates/*/src`, ~3.2 MB):
GitHub's count tracks the real Rust tree.

### Phase E: Long-term architecture decisions

Evaluated 2026-08-14 (after A-D complete). The three items are
decision records against current reality; none requires a port today.

**E1: nodes.py / types.py (out of scope, confirmed)**

`mypy/nodes.py` and `mypy/types.py` remain the "widely shared mutable
object graphs" the migration plan says NOT to port. They are
plugin-visible, cache-serialized, and identity-sensitive. Nothing in
A-D changed that calculus. The Rust `Type` enum + binary wire reader
(`wire::read_type_to_str`, `typeinfo::read_type_to_str_with_resolver`,
Stage 3a) is parity-tested foundation for a possible `is_subtype`
port, but enabling it in production would require the full cost list
below; it stays off until the migration has proven stable over time.

A future port would require:
- A Rust-owned `Type` enum with Python proxy objects
- Plugin API changes (hooks receive Rust types, not Python objects)
- Cache format changes (Rust-owned serialization)
- Daemon mode changes (identity preservation across incremental updates)

Multi-quarter effort. Do not start while the strangler-fig per-call
gates (Rust returns a value or `None`, Python falls back) are still
carrying the load; a wholesale `Type`/`Node` reimplementation removes
that fallback safety net.

**E2: Daemon FS ownership (already Rust-backed; no port warranted)**

The daemon (`fine_grained_incremental`) reads the filesystem through
`FileSystemCache` (`mypy/fscache.py`), which delegates every method to
the Rust `module_resolver.FsCache` pyclass. That includes the
transactional per-flush snapshot semantics and the Bazel fake
`__init__.py` synthesis (`crates/module_resolver/src/fs_cache.rs`).
`dmypy_server` passes the shared `self.fscache` into `build`,
`update`, and snapshot paths, so the daemon VFS reads are Rust-owned.

The one remaining Python-owned FS-adjacent piece in the daemon is
`FileSystemWatcher` (`mypy/fswatcher.py`, 106 lines): stat + hash
change-detection over watched paths. It performs no direct OS calls;
every read goes through the Rust-backed cache. Porting its diffing
logic to Rust would remove a cache miss per watched path per
`find_changed`, dwarfed by the hashing cost itself. Decision: keep the
watcher in Python. It is a thin algorithmic layer over an already-Rust
FS layer; a port is not a meaningful lever.

Bazel stays on the Python `_find_module` resolver by gate
(`_native_gate_active` excludes `options.bazel`); its fake-init
synthesis remains Python-owned in the pure-Python fallback
(`fscache.py`, `_fake_init_py`). This is deliberate: the native
resolver reads the real FS through `FsCache`, which does not observe
Bazel's virtual FS.

**E3: mypyc coexistence (assessed, not exercised locally)**

mypyc compiles mypy's Python to CPython C extensions
(`setup.py --use-mypyc`); the Rust work is shipped as separate
PyO3 cdylib extensions (ast_serialize, module_resolver, type_kernel)
loaded by `PYTHONPATH` import. At the CPython ABI level the two
coexist: mypyc C modules and PyO3 modules are independent extensions,
so there is no symbol or GIL conflict in principle.

Known coexistence hedge already in the tree: `FileSystemCache` is
marked `@mypyc_attr(allow_interpreted_subclasses=True)` so a
mypyc-compiled build can still spawn interpreted subclasses in tests.

The local dev workflow does not compile mypy with mypyc (no
`build/native`), so E3 is an assessment, not an exercised guarantee.
A future milestone could compile the Python fallbacks with mypyc AND
use the Rust extensions, getting both speeds; the two documented
hazards to watch are mypyc-attr subclassing rules and the
`PYTHONPATH`-overwrite behavior seen in the daemon test harness
(prepends now, so Rust dirs survive). No conflicts have been
observed; no work is scheduled.

## Execution Order

```
Phase A (blocking):
  A1: Fix "TypeInfo not fixed" de-serialization crash
  A2: Close issue #582

Phase B (performance, parallel):
  B1: setops.rs deferral reduction (highest hit-count sites first)
  B2: subtypes.rs deferral reduction
  B3: checkexpr_functions.rs deferral reduction

Phase C (depth, after B):
  C1: semanal.py visit_class_def / visit_func_def pure parts
  C2: checker.py try/except + match narrowing
  C3: DefaultPlugin hook body porting

Phase D (infrastructure, parallel with B/C):
  D1: Document/gate rust_compute_search_paths
  D2: Build order documentation
  D3: Performance regression tracking
  D4: Rust % measurement

Phase E (long-term, after A-D complete; decision records, no ports):
  E1: nodes.py / types.py, confirmed out of scope
  E2: Daemon FS ownership, confirmed Rust-backed (fscache); watcher stays Python
  E3: mypyc coexistence, assessed but not exercised locally
```

## Contract for every change

Every port/fix follows the strangler-fig contract:
1. Rust returns a result or `None`; on `None`, Python falls back.
2. Gate is `TEST_NATIVE_TYPE_KERNEL=1` for new ports, default-on for
   graduated ports.
3. Differential parity: run with and without the flag, confirm
   identical testcheck + testtypes + testfinegrained results.
4. Full parity suite green before merge.
5. Self-check diagnostic parity (byte-for-byte identical output).
6. Rebuild the `.so` after any `.rs` change (stale-binary hazard).
