# Handoff: strangler-fig Rust migration loop (seam-deferral reduction)

*Written 2026-08-28. Goal: "migrate all python code to rust, really all" —
pursued as the established measure → file → dispatch-2-agents → process-PRs
→ gate loop. This file is the resume point.*

## Where main stands

- `main` = `f90ed2a77` (`perf(checkmember): skip type-obj callables,
  partials in M20 gate (#1120)`), local ff'd to origin.
- Gates on clean main: parity `10850 passed, 72 skipped, 7 xfailed`
  (`TEST_NATIVE_TYPE_KERNEL=1 TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1
  .venv/bin/python -m pytest mypy/test/testtypes.py mypy/test/testcheck.py
  -n4 -q`), self-check `Success: no issues found in 344 source files`.
- Shared `.so` in `/private/tmp/mypy-rs-local-typekernel/` was rebuilt and
  codesigned at `43c3e2644`; **rebuild + codesign from `f90ed2a77` before
  the next survey** (procedure under "The loop", step 4).
- Last full survey (`cd9abd4e5`): **6,381,797 seam calls, 97.6% native**.
  Since then: IAMA dispatch 88% → 92% (#1119), member_access 68% → 89%
  (#1120), is_subtype defers 22,419 → 20,680 (#1118).

## This session's merges (in order)

| PR | Issue | What | Numbers |
|----|-------|------|---------|
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
   member_access CallableType/Overloaded tail).
5. After the above, re-survey; remaining >90% seams (coerce_to_literal 97%,
   find_self_type 93%, infer_constraints_full 84% — verify end-to-end) are
   the next candidates.

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
