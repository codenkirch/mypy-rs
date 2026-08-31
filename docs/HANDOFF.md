# Handoff: strangler-fig Rust migration loop (seam-deferral reduction)

*Written 2026-08-28, refreshed 2026-08-31 (post-wave14, #1294 + #1299 +
#1302 + #1301 merged; #1291-#1293 in the same window). Goal:
"migrate all python code to rust, really all", pursued as the
established measure -> file -> dispatch-agents -> process-PRs -> gate
loop. This file is the resume point.*

## Where main stands

- `main` = `46237517c` (`perf(type_kernel): decided-None literal seam +
  live untyped-decorator walk (#1301)`), local ff'd to origin.
- Gates on clean main: shared-`.so` testtypes parity 2970 passed /
  3 skipped; `cargo fmt` clean for `type_kernel` (NOTE: `ast_serialize`
  has 2 pre-existing fmt diffs on main; the gate only checks
  `type_kernel`, do not reformat ast_serialize opportunistically).
- Self-check baseline: `UpdateDataSuite::test_update_data` FAILS on
  clean main too; tracked as #1300 (noticed during #1302, not fixed).
- Shared `.so` set rebuilt + codesigned at `46237517c` content
  (`/private/tmp/mypy-rs-local-typekernel|resolver|ast`). Rebuild
  before the next survey (procedure under "The loop").
- Env-gate semantics settled (#1285 + #1287/#1294): missing extensions
  fail loudly; `TEST_NATIVE_*` `0` now genuinely forces the Python
  fallback, so gate-off differentials are a REAL parity tool.
- Runner/OCR note: the repo's ephemeral runner cannot re-register (403,
  admin-blocked; #1249 open). OCR gates stay `queued` forever; use the
  local fallback (`ocr review --from <merge-base> --to <head> --format
  json --audience agent`) and merge `--squash --admin` after pr-gate +
  parity are green.

## Merged since the survey13 refresh

| PR | Issue | What | Numbers |
|----|-------|------|---------|
| #1291 | #1285 | fail loudly when env gates request a missing extension | correctness guard |
| #1292 | (perf) | alias round-trip in `rust_expand_type_by_instance` | 55% -> 99% native |
| #1293 | (perf) | MA var hook gate consults the live plugin chain | correctness guard |
| #1294 | #1287 | decode `TEST_NATIVE_*` so `0` forces Python fallback | gate-off differential now real |
| #1299 | #1288 | property callable bind + var self-type expansion | `rust_analyze_member_access` 97% -> 98% |
| #1302 | #1297 | generic bind_self + free map expand in typeobj seam | `type_object_type_from_function` 84% -> 97%, `builtin_item_type` 84% -> 99% |
| #1301 | #1295, #1296 | decided-None literal seam + live untyped-decorator walk | `simple_literal_type` 0% -> 100%; `check_for_untyped_decorator` 64% -> 86%; `is_untyped_decorator` 220 @ 15% -> 75 calls (residual defers are the #942 seam's own tail) |

Discovered during the wave, not yet fixed:

- #1300 (filed): `UpdateDataSuite::test_update_data` self-check failure,
  pre-existing on main.
- #1280 (filed): disc-4 AnyType-reconstruction helper duplicated across
  setops/condmaps/checker_helpers/checkexpr (cleanup PR, not a port).
- #1298 (filed): setops hot set ~3,300 fallbacks (is_valid_inferred_type
  717 @ 96%, check_overload_call 656 @ 95%, replace_meta_vars 643 @ 97%,
  narrow_declared_type 582 @ 98%, conditional_types 384 @ 96%,
  and_conditional_maps 325 @ 96%), held for wave15.
- join_type_list round 3 walls remain at 74 @ 28% (#1281 filed).

## Residual ranking (survey15, post-wave14, `/tmp/survey15.txt`)

Total 5,148,842 seam calls. By absolute fallbacks (calls x (1 - native%)):

- is_subtype 29,408 @ 95% (~1,470; #1276 filed, largest bucket)
- expand_without_binding 11,255 @ 97% (~338)
- classify_missing_annotations 15,260 @ 98% (~305)
- make_simplified_union 29,435 @ 99% (~294)
- is_overlapping_types 2,836 @ 90% (~284)
- solve_generic_call 8,197 @ 97% (~246)
- infer_function_type_arguments 522 @ 55% (~235)
- analyze_instance_member_access 335 @ 34% (~221)
- map_instance_to_supertype 5,521 @ 96% (~221)
- append_invariance_notes 201 @ 15% (~171)
- get_type_vars 1,876 @ 91% (~169)
- is_typeddict_type_context 257 @ 56% (~113)
- join_types 539 @ 79% (~113)
- map_type_from_supertype 129 @ 6% (~121; call count collapsed from
  302 post-#1302, re-audit before chasing)
- add_class_tvars 1,265 @ 93% (~89); get_protocol_member 487 @ 81%
  (~93); freshen_all_functions_type_vars 808 @ 92% (~65);
  find_type_overlaps 74 @ 65%; analyze_member_method 48 @ 6%;
  is_equality_ambiguous_for_narrowing 53 @ 15%.
- Setops cluster (#1298, wave15 primary):
  is_valid_inferred_type 718, check_overload_call 657,
  replace_meta_vars 644, narrow_declared_type 583, conditional_types
  385, and_conditional_maps 326 (~3,300 absolute fallbacks).
- find_self_type dropped off the ranking (99%+ of 35,446; wall already
  ported); w14 wins confirmed: simple_literal_type 239 @ 100%,
  type_object_type_from_function 1,800 @ 97%, builtin_item_type
  1,400 @ 99%, check_for_untyped_decorator 522 @ 86%.
- Survey caveats: `is_subtype_batch` 213% and `rust_type_analyze` lines
  are kernel-boundary counting artifacts; discount when ranking;
  `analyze_instance_member_dispatch` 451 "fallbacks" are decided
  negatives (0% defer), skip them.

## Older session record

See `git log` and the closed issue stream (#896..#1242) for the earlier
waves; the loop protocol below is unchanged.

| #1110 | none | survey script: protocol-test-callee → CLASSIFIER_NEGATIVE_SEAMS | chore |
| #1116 | #1108 | descriptor-head guards decided in Rust | biggest bucket (29.9k @ 2%) |
| #1118 | #1111 | protocol-right Instance arm natively (assuming guard, dep record, member-flag arbitration) | is_subtype defers 22,419 → 20,680; fixed mro_has miss pre-check + dropped IS_CLASSVAR |
| #1119 | #1112 | IAMA dispatch: freshen TypeAliasType arm + builtins.tuple map case; TupleType arm now recurses on tuple_fallback | 12,126 → 7,919 fallbacks (88% → 92%) |
| #1120 | #1117 | M20 gate skips type-obj callables and PartialTypes | member_access 16,079 @ 68% → 12,295 @ 89%; ~1,230 defers left (documented: CallableType/Overloaded tail belongs to #342 mega-port, TupleType = IAMA handoffs) |

Closed not-planned with evidence (negative results, the loop working):

- **#1109** (type_analyze defers): the 78% share was kernel-boundary;
  end-to-end the seam wins 0 calls: the wirefixup map only gains an SCC's
  TypeInfos after that SCC's semanal completes, and parallel workers never
  build native resolvers. Follow-up filed as **#1115** (decode lifecycle in
  `process_stale_scc`, plus worker-side resolver wiring).
- **#1113** (expand_type defers): buckets structural: leftover-typevar
  3,020 (solver identity contract), input/result-alias 2,597 (wire
  `TypeAliasType` has `alias=None`), encode-fail 28, callable-unpack 3. The
  92% share is real end-to-end. Side finding: `rust_expand_type_by_instance`
  wins only 465 of 108,797 Python-side calls (0.4%).
- **#1266** (join_type_list round 2): CLOSED 2026-08-31. Round 1
  (mid-fold pair defers through the #824 `join_instances_core`) and the
  n<=1 shim early path landed in #1270; regression lock
  `NativeJoinTypeListSuite.test_nle1_lists_never_cross_ffi`. Post-merge
  residual: 74 whole-call events @ 28% native, ~53 defers (lkv 39,
  join_instances_core-declined pairs 9, arg_disc-4 4); all Rust-side,
  tracked in #1281 (round 3). Environment lessons from that audit:
  worktree venvs MUST be py3.13 (`uv sync --python 3.13`; a default
  `uv run` pulls 3.14 and the cpython-313 `.so`s fail in build workers
  with `ValueError: invalid bool value`); audit instrumentation is
  env-gated and stripped pre-commit; a clobbered shared `.so` is the
  first suspect for END_TAG/bool asserts in parity runs.

## Open backlog (next waves; dispatch max ~2 port agents)

1. **#1298 (filed, wave15 primary)**: setops hot set ~3,300 fallbacks
   (numbers above). Agent briefing: audit-first, one seam at a time,
   the #1091/#1109/#1113 precedent.
2. **#1276 (filed)**: is_subtype protoR residual (~1,475 @ 95%),
   get_protocol_member_inner deferrals dominate. Largest bucket.
3. **#1115**: build-side decode lifecycle; bigger slice crossing
   semanal/worker build paths; needs careful daemon/cache parity
   assessment.
4. **#342**: analyze_class_attribute_access mega-port (owns the
   member_access CallableType/Overloaded tail; audit-first).
5. **#1280**: disc-4 helper convergence (cleanup PR, not a port).
6. **#1281**: join_type_list round 3 (74 @ 28%).
7. Next-wave queue (file issues with fresh survey15 numbers, then
   dispatch ~2): expand_without_binding ~340, classify_missing_annotations
   ~304, make_simplified_union ~294, map_type_from_supertype 302 @ 43%,
   is_overlapping_types ~283, is_typeddict_type_context 257 @ 56%,
   solve_generic_call ~245, infer_function_type_arguments ~234,
   analyze_instance_member_access ~221, append_invariance_notes ~170.

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
   exclusions, cleanup duty). Branch from origin/main AFTER the previous
   wave's PRs merge; rebases onto main with sibling testtypes.py changes
   shift line numbers ~40 lines (CI self-check runs the MERGE).
5. Agents usually self-merge end-to-end; if one ends right after opening
   its PR, you own: `agent-wait until github.pr <N> -R codenkirch/mypy-rs
   -t 900`, then `gh pr merge <N> -R codenkirch/mypy-rs --squash
   --admin`.
6. After merges: `git checkout main && git pull --ff-only`, rebuild +
   codesign the shared `.so`, re-run both gates, next survey.

## Hard rules (each learned at real cost; do not rediscover)

- Every Bash call starts `cd <dir> && ` (no persistent cwd; the Bash cwd
  parameter is rejected for worktree paths).
- pytest `-n 4` max, never `-n auto` (64GB machine OOMs the full suite).
- Codesign `-f -s -` any copied `.so` or the interpreter SIGKILLs.
- Rebuild the `.so` after any Rust edit; use PRIVATE scratch dirs
  (`/private/tmp/mypy-rs-local-tk-<issue>/`) when agents run in parallel.
- Rebuild the scratch `.so` after a REBASE too (main's seam signatures
  move; a stale binary crashes self-check with `TypeError: ... takes 7
  positional arguments but 8 were given`, hit during #1301).
- Worktree venvs lack pytest; use the main checkout's `.venv/bin/python`
  with PYTHONPATH pointing at the private scratch dir plus the shared
  resolver/ast dirs; put the WORKTREE root first on PYTHONPATH when
  surveying from a worktree (venv import otherwise shadows it, #1120's
  agent hit this).
- Self-check: same PYTHONPATH + `TEST_NATIVE_TYPE_KERNEL=1 .venv/bin/python
  -m mypy --config-file mypy_self_check.ini -p mypy -p mypyc`.
- `mypy_self_check.ini` has `num_workers = 4`; a bare single-file run
  (`--no-incremental mypy/test/testtypes.py`) reports identical errors
  at DIFFERENT line numbers than CI; match by error text, not line.
- Known CI flake:
  `NativeCompatibilityClassvarSuperSuite::test_parity_every_branch`: one
  rerun = green.
- Rebase protocol when sibling PRs conflict: testtypes.py → origin/main's
  file + only my suite appended; AGENTS.md → keep both bullets; lib.rs →
  keep both registration lines; `grep -rn "<<<<<<< HEAD" crates/ mypy/
  AGENTS.md` before `push --force-with-lease`.
- Comment blocks: max 3 consecutive lines, ≤88 chars (pre-commit hook
  enforces).
- Never maturin develop for these crates (repo-root pyproject shadowing).
- This repo's ruff config has `fix = true`: a bare `ruff check`
  AUTO-REWRITES files; use `--no-fix`.
- Audit instrumentation is env-gated and REMOVED before commit; a negative
  audit closes the issue not-planned with the bucket table (precedent
  #1091/#1109/#1113). Verify end-to-end wins, not just kernel-boundary
  share (#1109/#1115 trap). Survey caveat: `rust_type_analyze`'s share line
  is a kernel-boundary artifact; discount it when ranking.
- OCR is disabled on this repo; CI-green is the operative merge gate.
