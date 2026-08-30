# Handoff: strangler-fig Rust migration loop (seam-deferral reduction)

*Written 2026-08-28, refreshed 2026-08-30 (post-survey10-wave). Goal:
"migrate all python code to rust, really all" — pursued as the
established measure -> file -> dispatch-2-agents -> process-PRs -> gate
loop. This file is the resume point.*

## Where main stands

- `main` = `f76d216bd` (`perf(type_kernel): port type[T]-vs-callable and
  protocol-arm constraint inference (#1263)`), local ff'd to origin.
- Gates on clean main: survey11 self-check run by the orchestrator on the
  fresh shared `.so` = 345 files 0 issues; cargo test green (~2,396-2,399
  passed per the two merged PRs' pre-merge runs).
- Shared `.so` set rebuilt + codesigned at `f76d216bd`
  (`/private/tmp/mypy-rs-local-typekernel|resolver|ast`; type_kernel
  refreshed by the orchestrator after the merges); rebuild before the
  next survey (procedure under "The loop").
- Last survey (survey11, `f76d216bd`): 5,142,259 seam calls, 5,131
  fallbacks (0.1% defer) — survey10 was 8,970 (0.2%), so the wave cut
  fallbacks 43%. Residual <99% pools by absolute fallbacks:
  analyze_member_access 1,322 @ 89%, classify_unbound_front 1,095 @ 89%,
  join_type_list 904 @ 64%, analyze_instance_member_access 572 @ 17%,
  callables_compatible 498 @ 1%, analyze_unbound_without_info 466 @ 72%,
  expand_type_by_instance 445 @ 56%, type_object_type_from_function 288
  @ 84%, is_overlapping_types 284 @ 90%, are_parameters_compatible 258
  @ 44%, infer_function_type_arguments 234 @ 55% (was 828 @ 40%).
  is_subtype flat at 95% by design (#1256 precedent: kernel-boundary
  share flat, wins land in testdata parity suites).
- Wave wins (survey10 -> survey11): check_overload_call 1,459 @ 89% ->
  656 @ 95%; infer_constraints_full 1,368 @ 94% -> 213 @ 99%;
  solve_generic_call 1,140 @ 87% -> 245 @ 97%.
- Runner/OCR note: the repo's ephemeral runner cannot re-register (403,
  admin-blocked; #1249 open). OCR gates stay `queued` forever; use the
  local fallback `runners/_shared/ocr-review-pr.sh` (or direct `ocr
  review --from <merge-base> --to <head> --format json --audience
  agent`, resume errored files with `--resume <session_id>` and the
  exact full-SHA range) and merge `--squash --admin` after pr-gate +
  parity are green.

## Recent merges (since 2026-08-30 refresh)

| PR | Issue | What | Numbers |
|----|-------|------|---------|
| #1261 | #1259 | per-PR decides type-object/callable + tuple-fallback constraint arms in constraints/solve | infer_constraints_full 22,792 calls 94% -> 99% (1,368 -> 213 fallbacks); OCR: 2 low nits acknowledged |
| #1263 | #1260 | rusty port type[T]-vs-callable + protocol-arm + inst-protocol-template constraint inference; rebased over #1261 | solve_generic_call 8,768 calls 87% -> 97% (1,140 -> 245 fallbacks); RESULT_OK 10,840 -> 10,992 / 11,420 (96.2%); OCR 3 findings (2 med dead-code, 1 low) all fixed |
| #1250 | #1247 | docs: bytes-literal escape leniency matches CPython | docs only; runner-blocked OCR bypassed via fallback script |
| #1252 | #1251 | ci: squash-gate treats `skipped` + "No supported files changed" + no comments as clean | unblocks doc-only PRs; ocr-review CI jobs stay queued (runner 403) |
| #1253 | #1248 | fix(type_kernel): str.format char-vs-byte offsets + multibyte parity | 9 unit tests; testtypes 2904, testcheck 8198, self-check clean, mypyc run-strings 25 passed |
| #1257 | #1254 | perf: expand alias operands in rust_check_overload_call | 13,260 calls 87% -> 89% (1,724 -> 1,459 fallbacks); ol.alias*/union_item_none defers gone |
| #1256 | #1255 | perf: Instance>callable and callable>protocol __call__ arms in rust_is_subtype | seam-level 95% (flat on self-check corpus; wins land in testdata parity suites) |

Discovered during the wave, not yet fixed:

- **#1262** — `erase_typevars.rs make_any()` emits `type_of_any: 12`,
  not a `TypeOfAny` member (`special_form` is 6); comment lies. Raw
  value round-trips through the wire; latent parity hazard at
  `typeanal.py:2853`, `checkexpr.py:8886`, `stats.py:489`. Fix is ~5
  literals + a round-trip test.

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

1. **is_subtype tail (~20.7k defers)** — the largest remaining bucket.
   Remaining protoR defers (~13.1k) are dominated by
   `get_protocol_member_inner` deferrals (extra_attrs, base-class members
   behind the same-class guard, descriptors). Unfiled — file an issue with
   these numbers before dispatching.
2. **#1114** — rust_find_self_type 35,210 @ 93% (~2.5k defers).
3. **#1115** — build-side decode lifecycle; bigger slice crossing
   semanal/worker build paths; needs careful daemon/cache parity assessment.
4. **#342** — analyze_class_attribute_access mega-port (now owns the
   member_access CallableType/Overloaded tail; also the natural home for
   the analyze_member_access 1,322 @ 89% residual — audit-first).
5. Next-wave queue from survey11 (file issues with these numbers, then
   dispatch ~2):
   - **classify_unbound_front 1,095 @ 89%** (round 1).
   - **join_type_list 904 @ 64%** (round 2; defers on LKV items,
     fallback_to_any items, undecided pairs — audit-first).
   - Then: analyze_unbound_without_info 466 @ 72%,
     expand_type_by_instance 445 @ 56% (#1113 buckets structural —
     check the precedent first), callables_compatible 498 @ 1% (low
     volume), type_object_type_from_function 288 @ 84%,
     is_overlapping_types 284 @ 90%, are_parameters_compatible 258 @ 44%.
   - **#1262** is a quick standalone fix — slot it into any wave.
6. After the above, re-survey.

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
