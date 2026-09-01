# Handoff: strangler-fig Rust migration loop (seam-deferral reduction)

*Written 2026-08-28, refreshed 2026-08-31 (post-wave16, #1306 + #1307
merged; wave15 #1303 merged, #1276 negative-closed). Goal:
"migrate all python code to rust, really all", pursued as the
established measure -> file -> dispatch-agents -> process-PRs -> gate
loop. This file is the resume point.*

## Where main stands

- `main` = `9d181f2bf` (`perf(type_kernel): port alias defers in
  ewb/cma/meet-overlap seams (#1304)`), local ff'd to origin.
- Gates on clean main: shared-`.so` testtypes parity 2986 passed /
  3 skipped; `cargo fmt` clean for `type_kernel` (NOTE: `ast_serialize`
  has 2 pre-existing fmt diffs on main; the gate only checks
  `type_kernel`, do not reformat ast_serialize opportunistically).
- Self-check baseline: `UpdateDataSuite::test_update_data` FAILS on
  clean main too; tracked as #1300 (noticed during #1302, not fixed).
- Shared `.so` set rebuilt + codesigned at `9d181f2bf` content
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

## Merged since the survey15 refresh (waves 15 + 16)

| PR | Issue | What | Numbers |
|----|-------|------|---------|
| #1303 | #1298 | alias defers in rmv/ivit/use-meet seams + erasetype flag-rebind bug fix | rmv 662 -> 0, ivit 790 -> 0, use-meet 297 -> 0 |
| #1306 | #1298 | ct alias fronts (cur-alias, tgt0-alias, prop-alias, restrict-subtype-away) + ndt ri-decl/msu-narrow2 | ~440 events closed; ct 96% -> 98%, ndt 98% -> 98% (residuals re-tabled on #1298) |
| #1307 | #1304 | ewb alias-args + alias-Vars; cma alias ret-type; iot Callable-vs-Callable overlap arm | ewb ~270 closed, cma 313-defer wall closed, iot ~120 closed |

Closed in the same window: #1276 not-planned (audit comment with bucket
table; no portable bucket >= 15%), #1304 closed with residual note,
#1288/#1289 closed as superseded (survey17 numbers show their targets
collapsed across later waves).

Discovered during the waves, not yet fixed:

- #1300 (filed): `UpdateDataSuite::test_update_data` self-check failure,
  pre-existing on main.
- #1280 (filed): disc-4 AnyType-reconstruction helper duplicated across
  setops/condmaps/checker_helpers/checkexpr (cleanup PR, not a port).
- #1298 (open): remaining setops residuals (ct sub-concrete/sub-structural
  need a subtype-callback channel, same wall as #1260; ndt
  ovl-disjoint/ovl-pair need Python overload context; oc buckets all
  SKIP by prior decision).
- #1281 (closed 2026-09-01): join_type_list round 3. No-action verdict
  (see the #1266 entry below); first blocker for any revisit is #1335
  (filed from the audit, below).
- #1335 (filed): rru lkv survivor rule diverges from Python when the
  dedup item has no last_known_value (remove_redundant.rs
  find_subtype_index vs typeops.py:1432-1442; setops.rs has the correct
  formulation). Differential probe reproduces it; fix first.

## Residual ranking (survey17, post-wave16, `/tmp/survey17.txt`)

Total 5,155,526 seam calls. By absolute fallbacks (calls x (1 - native%)):

- is_subtype 29,363 @ 95% (~1,468; #1276 closed not-planned, blocked on
  #1184 unify_generic_callable; do not reopen)
- check_overload_call 13,198 @ 95% (~659; oc buckets all SKIP by prior
  decision on #1298)
- narrow_declared_type 28,949 @ 98% (~578; residual walls documented on
  #1298: overload context + live objects)
- find_self_type 35,462 @ 99% (~354; was 100% after #1114, possible
  regression -> #1308 wave17 audit)
- make_simplified_union 29,000 @ 99% (~290; intentional decode/fixup
  defers, closed with #1304)
- is_overlapping_types 2,263 @ 88% (~271; residual ~120 small shapes
  after #1307)
- solve_generic_call 8,204 @ 97% (~246; #826 residual)
- analyze_member_access 12,040 @ 98% (~240; #1288 closed as superseded)
- infer_function_type_arguments 522 @ 55% (~234; #1308 wave17)
- expand_and_bind_callable 5,562 @ 96% (~222; ParamSpec/Unpack walls)
- analyze_instance_member_access 335 @ 34% (~221; #1309 wave17)
- map_instance_to_supertype 5,521 @ 96% (~221)
- infer_constraints_full 21,459 @ 99% (~214)
- join_instances 316 @ 36% (~202; #1281 closed, no active issue owns it)
- conditional_types 8,959 @ 98% (~179; subtype-callback wall on #1298)
- remove_redundant_union_items 5,831 @ 97% (~174)
- get_type_vars 1,894 @ 91% (~170)
- append_invariance_notes 201 @ 15% (~170; #1308 wave17 territory)
- has_any_from_unimported_type 7,838 @ 98% (~156)
- map_type_from_supertype 129 @ 6% (~121; #1309 wave17)
- is_typeddict_type_context 257 @ 56% (~113; #1309 wave17)
- get_protocol_member 487 @ 81% (~93); add_class_tvars 1,265 @ 93%
  (~89); join_types 539 @ 79% (~113).
- Survey caveats: `is_subtype_batch` 213% and `rust_type_analyze` lines
  are kernel-boundary counting artifacts; discount when ranking;
  `analyze_instance_member_dispatch` "0% defer" negatives are not
  fallbacks.

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
- **#1281** (join_type_list round 3): CLOSED 2026-09-01, not-planned
  (wave27 Lane B, branch `audit/join-r3-w27b` at e5a369c8d). The three
  residual walls (lkv 39, join_instances_core-declined pairs 9,
  arg_disc-4 4; 0.001% of the survey12 corpus) all fail the
  composition bar:
  - lkv 39: guard-drop requires the union dedup and callable-combine
    arms to be lkv-complete. The audit found the already-shipped lkv
    survivor rule in `find_subtype_index` (remove_redundant.rs:236-252)
    diverges from typeops.py:1432-1442 when the dedup item has no lkv:
    Python collapses `[I(A,lkv=1), I(A)]` to `[A]` via pass 2; the
    production `rust_remove_redundant_union_items` seam keeps
    `[Literal[1]?]` (gate-off vs gate-on differential, fresh .so from
    HEAD). setops.rs:3565-3585 implements the rule correctly, so the
    two Rust copies disagree with each other. Filed as **#1335**; a
    port would need that fix, a dedup-implementation unification, an
    audit of the can_be_true/can_be_false survivor restore
    (typeops.py:1443-1455; wire Instance carries no flags, see
    `_has_mutated_truthiness` typeops.py:164-186), and a full
    differential, all for 39 defers.
  - declined pairs 9: both-generic combine_similar_callables shapes;
    `TypeVarId.new` is a global counter Rust cannot replicate without
    breaking wire-equal `CallableType.__eq__` parity (mypy/join.py:188,
    joinfns.rs:150). Structural wall.
  - arg_disc-4 4: shim pick (mypy/join.py:615-629) has no plain
    AnyType operand; fixing means a new wire-emitting path for the
    SameTypeWithArgs arm. Adjacent cleanup on #1280.

## Open backlog (next waves; dispatch max ~2 port agents)

1. **#1308 (in flight, wave17 A)**: find_self_type regression audit +
   infer_function_type_arguments residual (agent-86, worktree
   `wave17-1308`).
2. **#1309 (in flight, wave17 B)**: low-share bundle ama /
   is_typeddict_type_context / map_type_from_supertype (agent-87,
   worktree `wave17-1309`).
3. **#1115**: build-side decode lifecycle; bigger slice crossing
   semanal/worker build paths; needs careful daemon/cache parity
   assessment.
4. **#342**: analyze_class_attribute_access mega-port (owns the
   member_access CallableType/Overloaded tail; audit-first).
5. **#1280**: disc-4 helper convergence (cleanup PR, not a port).
6. Next-wave queue (file issues with fresh survey17 numbers, then
   dispatch ~2): solve_generic_call ~246, expand_and_bind_callable
   ~222, map_instance_to_supertype ~221, remove_redundant_union_items
   ~174, get_type_vars ~170, has_any_from_unimported_type ~156,
   get_protocol_member ~93, add_class_tvars ~89.

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
