# Handoff: strangler-fig Rust migration loop (seam-deferral reduction)

*Written 2026-08-28, refreshed 2026-08-31 (post-survey13, #1284 + #1286
merged). Goal:
"migrate all python code to rust, really all" — pursued as the
established measure -> file -> dispatch-2-agents -> process-PRs -> gate
loop. This file is the resume point.*

## Where main stands

- `main` = `6d5b0c473` (`perf(type_kernel): narrow IAMA freeze
  survivor gate (17% -> 92% native) (#1286)`), local ff'd to origin.
- Gates on clean main: self-check clean (survey13 sanity run: 200
  files 0 issues; testtypes 2955 passed / 3 skipped); cargo test
  green.
- Shared `.so` set rebuilt + codesigned at `6d5b0c473` content
  (`/private/tmp/mypy-rs-local-typekernel|resolver|ast`). Rebuild
  before the next survey (procedure under "The loop").
- Last survey (survey13, main checkout at `6d5b0c473`): 5,134,951 seam
  calls, 1,299 fallbacks (0.03% defer) — survey12 was 5,131,412 / 3,349
  (0.07%), so the wave cut fallbacks ~61%. Residual <98% pools by
  absolute fallbacks (calls x (1 - native%)):
  is_subtype 29,502 @ 95% (~1,475; #1276 filed),
  analyze_member_access 12,030 @ 89% (1,323; #1288 filed),
  is_valid_inferred_type 17,913 @ 96% (716),
  check_overload_call 13,132 @ 95% (656),
  replace_meta_vars 21,410 @ 97% (642),
  narrow_declared_type 29,127 @ 98% (582),
  expand_type_by_instance 995 @ 55% (448; #1289 filed),
  conditional_types 9,617 @ 96% (385),
  find_self_type 35,426 @ 99% (354),
  and_conditional_maps 8,143 @ 96% (326).
  is_subtype_batch's 211% line is a kernel-boundary counting artifact;
  discount it (see survey caveat below).
- Wave wins (survey12 -> survey13): callables_compatible 503 @ 1%
  (498 defers) -> 489 calls @ 91% (shakeout of the SubtypeVisitor
  normalization bootstrap, #1283); analyze_unbound_without_info 574
  @ 19% -> 570 @ 100% (0 defers, #1284); re-survey
  analyze_instance_member_access: freeze-survivor gate narrowed,
  dispatch seam 93,270 @ 100%, residual access 335 calls @ 34% (#1286;
  pristine-share widening rejected post-merge as an incorrectness
  risk, see #1286 PR body). analyze_unbound_type_without_type_info
  reached 100%.
- In flight: wave13 agents dispatched on #1288 (member-access residual
  audit-port) and #1289 (expand_type_by_instance residual
  audit-port); resume points in the session log.
- Runner/OCR note: the repo's ephemeral runner cannot re-register (403,
  admin-blocked; #1249 open). OCR gates stay `queued` forever; use the
  local fallback (`ocr review --from <merge-base> --to <head> --format
  json --audience agent`, resume an interrupted session with
  `--resume <session_id>` and the exact full-SHA range) and merge
  `--squash --admin` after pr-gate + parity are green. Note `ocr
  review` needs ~6-8 min for a 7-file range and dies silently when its
  parent shell is reaped; resume recovers in-place.

## Recent merges (since survey12 refresh)

| PR | Issue | What | Numbers |
|----|-------|------|---------|
| #1283 | #1279 | callables_compatible via is_callable_compat kernel | 503 @ 1% -> 489 calls @ 91% |
| #1284 | #1278 | analyze_unbound_without_info tail table decided natively | 574 @ 19% -> 570 @ 100% (0 defers) |
| #1286 | #1277 | IAMA freeze-survivor gate narrowed to meta_level == 0 | dispatch seam 93,270 @ 100%; access residual 335 @ 34% |

Discovered during the wave, not yet fixed:

- #1287 (filed): TEST_NATIVE_* parity-off env vars don't disable all
  seams while a type-kernel `.so` is importable; gate-off differential
  runs silently stay native. Sibling env trap #1285 is open too.
- #1280 (filed): disc-4 AnyType-reconstruction helper duplicated
  across setops/condmaps/checker_helpers/checkexpr (was the #1274
  acknowledged cleanup; now tracked).
- join_type_list round 3 walls remain at 74 @ 28% (#1281 filed).

## Closed: #1266 (join_type_list round 2) history and residual walls

STATUS 2026-08-31, CLOSED. Round 1 (route mid-fold pair defers through
the #824 `join_instances_core`) and design part 1 (the n<=1 shim early
path) BOTH LANDED in #1270; the "design part 1 remains" text below was
written pre-#1270 and is stale. Routing n<=1 lists THROUGH the Rust
single-item passthrough would regress, not improve: a single item
would come back as a wire-decoded copy instead of the live
`types[0]`, and the non-round-trippable singleton shapes (`tv|T` 333,
`union|n2` 158, `params` 93, dict tvs 74) defer inside the kernel
anyway. Post-merge residual (survey12): 74 whole-call events @ 28%
native, ~53 defers (lkv 39, join_instances_core-declined pairs 9,
join_one_pair arg_disc-4 4); all three walls are Rust-side, tracked in
#1281 (round 3). Regression lock added on top of #1270:
`NativeJoinTypeListSuite.test_nle1_lists_never_cross_ffi` forbids FFI
crossings for length <= 1 lists (uses RuntimeError because the shim
catches AssertionError and would silently fall back).

Audit history (pre-#1270 numbers, written 2026-08-30 21:35 during the
audit phase; design settled. Issue #1266 assigned to
@Jonathangadeaharder):

- **Setup**: worktree `/Users/jonathangadeaharder/projects/coding-utils/
  mypy-rs-i1266`, branch `perf/1266` at `f76d216bd`, two modified files
  (audit instrumentation only, no port code yet). The worktree venv MUST
  be py3.13: `uv sync --python 3.13`. A bare `uv run` with the default
  interpreter pulls 3.14, the cpython-313 `.so`s fail to load in build
  workers, and the run dies with `ValueError: invalid bool value` /
  `Worker 0 disconnected` (cost ~15 min to diagnose).
- **Instrumented build**: `/private/tmp/mypy-rs-local-tk-i1266/`
  (type_kernel `.so` with `MYPY_TK_JTL_AUDIT` logging, built 21:16 from
  the worktree). Audit invocation:
  `PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver:/private/tmp/mypy-rs-local-tk-i1266 MYPY_TK_JTL_AUDIT=1 uv run python -m mypy --config-file mypy_self_check.ini mypy mypyc misc script` (stderr → log).
- **VERIFIED audit numbers** (cold self-check 357 files / 0 issues,
  2026-08-30 21:30): 2,715 seam entries; 1,759 native (64.8%); 955
  defers (35.2%): `single-unsafe` (n==1 non-round-trippable singleton)
  889, `lkv` (any item carries `last_known_value`) 46 calls, `pair`
  (`join_one_pair` defer mid-fold) 20, `fta` 0, `pyexc` 0. Top
  single-unsafe tags: `tv|T` 333, `union|n2` 158, `params` 93, dict
  tvs `_KT`/`_VT` 74, `erased` 25, arged Instances ~70. Top pair
  shapes: `type`-fb callables 6+4=10, Instance pairs 10. The earlier
  pre-compaction audit read 2,605 calls / 942 defers — same
  composition; use the fresh 2,715/955 numbers going forward.
  Survey11 recorded 904 fallbacks @ 64% (same pool, older basis).
- **Settled design** (both parts are behavior-preserving vs the
  pure-Python body, which for n==0 returns `UninhabitedType()` and for
  n==1 returns `types[0]` verbatim without calling `join_types`):
  1. Shim early path for n<=1 at the top of `join_type_list` in
     `mypy/join.py`, BEFORE the native gate and serialization: n==0 →
     `UninhabitedType()`, n==1 → `return types[0]` (live object, no
     wire round-trip). Kills all 889 n==1 defers and skips 2,629
     serializations. The Rust kernel's identity-safe passthrough stays
     as parity for the kernel API but stops being exercised from
     production.
  2. `join_type_list_inner`: when `join_one_pair` defers mid-fold,
     route the pair through `join_instances_core` (the #824
     `join_instances` engine: handles lkv fresh-Instance joins and
     nominal Instance joins) before returning `None`. Also expected to
     decide the 46 lkv calls (its same-type args-less arm handles LKV).
     Callable/fb-type pairs (10 of the 20 pair defers) remain deferred
     — record as residual. Post-port expectation: ~10-20 defers on
     2,715 entries (>=99% native); measure with the same audit env and
     update this section's numbers in the PR body.
- **TEMP AUDIT instrumentation** was stripped before the #1270 commit
  (it had lived in `mypy/join.py` `join_type_list` and
  `checker_helpers.rs` `join_type_list_inner`); the audit env was
  `MYPY_TK_JTL_AUDIT` with the instrumented build at
  `/private/tmp/mypy-rs-local-tk-i1266/`.
- Archive note (implementation consumed by #1270): strip
  instrumentation → implement the two design parts → cargo test →
  testtypes + testcheck parity (-n4) → self-check clean → measure
  post-port numbers → commit → PR → pr-gate + parity green →
  fallback OCR → merge `--squash --admin` → refresh shared `.so` set.
- Shared-`.so` hygiene: `/private/tmp/mypy-rs-local-typekernel/` was
  accidentally overwritten with the instrumented build on 2026-08-30
  ~21:22 and restored to the `f76d216bd` baseline (5645616-byte dylib
  from the main checkout's `target/release`) + re-codesigned. If another
  agent's parity run fails an END_TAG/bool assert, suspect a clobbered
  shared `.so` first.

## Older session record

See `git log` and the closed issue stream (#896..#1242) for the earlier
waves; the loop protocol below is unchanged.

| #1110 | — | survey script: protocol-test-callee → CLASSIFIER_NEGATIVE_SEAMS | chore |
| #1116 | #1108 | descriptor-head guards decided in Rust | biggest bucket (29.9k @ 2%) |
| #1118 | #1111 | protocol-right Instance arm natively (assuming guard, dep record, member-flag arbitration) | is_subtype defers 22,419 → 20,680; fixed mro_has miss pre-check + dropped IS_CLASSVAR |
| #1119 | #1112 | IAMA dispatch: freshen TypeAliasType arm + builtins.tuple map case; TupleType arm now recurses on tuple_fallback | 12,126 → 7,919 fallbacks (88% → 92%) |
| #1120 | #1117 | M20 gate skips type-obj callables and PartialTypes | member_access 16,079 @ 68% → 12,295 @ 89%; ~1,230 defers left (documented: CallableType/Overloaded tail belongs to #342 mega-port, TupleType = IAMA handoffs) |

Closed not-planned with evidence (negative results, the loop working):

- **#1109** (type_analyze defers): the 78% share was kernel-boundary;
  end-to-end the seam wins 0 calls — the wirefixup map only gains an SCC's
  TypeInfos after that SCC's semanal completes, and parallel workers never
  build native resolvers. Follow-up filed as **#1115** (decode lifecycle in
  `process_stale_scc`, plus worker-side resolver wiring).
- **#1113** (expand_type defers): buckets structural — leftover-typevar
  3,020 (solver identity contract), input/result-alias 2,597 (wire
  `TypeAliasType` has `alias=None`), encode-fail 28, callable-unpack 3. The
  92% share is real end-to-end. Side finding: `rust_expand_type_by_instance`
  wins only 465 of 108,797 Python-side calls (0.4%).

All session worktrees/branches cleaned up by the agents; remaining
worktrees (`swarm-checkexpr`, `swarm-seam1`, `mypy-rs-callable-name-1100`)
and stashes are pre-existing — leave them.

## Open backlog (next waves; dispatch max ~2 port agents)

1. **#1276 (filed)** — is_subtype protoR residual (~1,475 @ 95%),
   get_protocol_member_inner deferrals dominate. Largest bucket.
2. **#1288 (filed, IN FLIGHT with wave13)** — analyze_member_access
   residual audit-port (1,323 @ 89%). The CallableType/Overloaded tail
   still belongs to #342 mega-port long-term; this round is audit +
   decidable-leaf ports only.
3. **#1289 (filed, IN FLIGHT with wave13)** — expand_type_by_instance
   residual audit-port (448 @ 55%); check #1113 precedent before
   chasing structural walls.
4. **#1115** — build-side decode lifecycle; bigger slice crossing
   semanal/worker build paths; needs careful daemon/cache parity assessment.
5. **#342** — analyze_class_attribute_access mega-port (owns the
   member_access CallableType/Overloaded tail; audit-first).
6. Next-wave queue from survey13 (file issues with these numbers,
   then dispatch ~2):
   - is_valid_inferred_type 17,913 @ 96% (716).
   - check_overload_call 13,132 @ 95% (656).
   - Then: replace_meta_vars 21,410 @ 97% (642),
     narrow_declared_type 29,127 @ 98% (582),
     conditional_types 9,617 @ 96% (385),
     are_parameters_compatible 459 @ 44% (survey12; re-measure),
     type_object_type_from_function 1,798 @ 84% (287),
     is_overlapping_types 2,835 @ 90% (283),
     infer_function_type_arguments 519 @ 55% (233).
   - Also: #1280 disc-4 helper convergence (cleanup PR, not a port).

## The loop (how to continue)

1. Rebuild + codesign shared `.so` from current main:
   `cargo rustc -p mypy-type-kernel --features extension-module --lib
   --crate-type cdylib --release -- -C link-arg=-undefined -C
   link-arg=dynamic_lookup`, cp to
   `/private/tmp/mypy-rs-local-typekernel/type_kernel.cpython-31{3,4}-darwin.so`,
   `codesign -f -s - /private/tmp/mypy-rs-local-typekernel/*.so`.
2. Survey: `PYTHONPATH=/private/tmp/mypy-rs-local-typekernel:/private/tmp/mypy-rs-local-resolver:/private/tmp/mypy-rs-local-ast
   uv run --no-sync python scripts/measure_native_share.py > /tmp/survey.txt
   2>&1`; rank non-100% lines by absolute fallbacks
   (`calls * (1 - native%)`).
3. Dup-check (`gh issue list --state open --search ...`), file a
   conventional issue with the numbers + audit-first method + the
   #1091/#1109/#1113 precedent.
4. Dispatch max ~2 coder agents per wave with the full workflow briefing
   (own worktree, private scratch dir, gates, PR flow, cross-file
   exclusions, cleanup duty).
5. Agents usually self-merge end-to-end; if one ends right after opening its
   PR, you own: `agent-wait until github.pr <N> -R codenkirch/mypy-rs -t
   900`, then `gh pr merge <N> -R codenkirch/mypy-rs --squash --admin`.
6. After merges: `git checkout main && git pull --ff-only`, rebuild +
   codesign the shared `.so`, re-run both gates, next survey.

## Hard rules (each learned at real cost — do not rediscover)

- Every Bash call starts `cd <dir> && ` (no persistent cwd; the Bash cwd
  parameter is rejected for worktree paths).
- pytest `-n 4` max, never `-n auto` (64GB machine OOMs the full suite).
- Codesign `-f -s -` any copied `.so` or the interpreter SIGKILLs.
- Rebuild the `.so` after any Rust edit; use PRIVATE scratch dirs
  (`/private/tmp/mypy-rs-local-tk-<issue>/`) when agents run in parallel.
- Worktree venvs lack pytest — use the main checkout's `.venv/bin/python`
  with PYTHONPATH pointing at the private scratch dir plus the shared
  resolver/ast dirs; put the WORKTREE root first on PYTHONPATH when
  surveying from a worktree (venv import otherwise shadows it — #1120's
  agent hit this).
- Self-check: same PYTHONPATH + `TEST_NATIVE_TYPE_KERNEL=1 .venv/bin/python
  -m mypy --config-file mypy_self_check.ini -p mypy -p mypyc`.
- Known CI flake:
  `NativeCompatibilityClassvarSuperSuite::test_parity_every_branch` — one
  rerun = green.
- Rebase protocol when sibling PRs conflict: testtypes.py → origin/main's
  file + only my suite appended; AGENTS.md → keep both bullets; lib.rs →
  keep both registration lines; `grep -rn "<<<<<<< HEAD" crates/ mypy/
  AGENTS.md` before `push --force-with-lease`.
- Comment blocks: max 3 consecutive lines, ≤88 chars (pre-commit hook
  enforces).
- Never maturin develop for these crates (repo-root pyproject shadowing).
- Audit instrumentation is env-gated and REMOVED before commit; a negative
  audit closes the issue not-planned with the bucket table (precedent
  #1091/#1109/#1113). Verify end-to-end wins, not just kernel-boundary
  share (#1109/#1115 trap). Survey caveat: `rust_type_analyze`'s share line
  is a kernel-boundary artifact; discount it when ranking.
- OCR is disabled on this repo; CI-green is the operative merge gate.
