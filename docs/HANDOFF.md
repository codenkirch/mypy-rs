# Handoff: strangler-fig Rust migration loop (seam-deferral reduction)

*Written 2026-08-28, refreshed 2026-09-05 (post-wave34, #1419 merged;
wave35 agent in flight). Goal: "migrate all python code to rust, really
all", pursued as the established measure -> file -> dispatch-agents ->
process-PRs -> gate loop. This file is the resume point.*

## Where main stands (2026-09-05)

- `main` = `5a708c714` (`perf(type_kernel): total has_recursive_types +
  alias-aware flatten (#1418) (#1419)`), local ff'd to origin.
- Phase state: F0 audit + F1 dual-write mirror + F2 read flip (slices
  1-10, #1393) all landed. F3 (#1397) write flip has Instance +
  CallableType splice ops; the planned tvar/union splice slice was
  profiled with `misc/f3s9_tvar_union.py` and came back EMPTY (zero
  per-field writes on a self-check) - dropped, do not build it.
- Gates on clean main: shared-`.so` parity `testtypes.py` +
  `testcheck.py` -n4 ~11,312 passed (wave-34 agent run 12,707 across
  the extended battery); `cargo fmt` + `cargo clippy -D warnings`
  clean for `type_kernel`; self-check clean on both configs
  ("Success: no issues found in 347 source files" in the survey run;
  the earlier `UpdateDataSuite` flake #1300 no longer reproduces).
- Shared `.so` rebuilt + codesigned at `5a708c714` content
  (`/private/tmp/mypy-rs-local-typekernel|resolver|ast`).
- Survey (post-wave34): **7,759,901 seam calls, 99.99% native**.
  Top deferrals: `rust_is_literal_type_like` 646,
  `rust_flatten_nested_unions` 165, `rust_check_argument_types_plan`
  86. Full ranking in issue #1420.
- Survey caveats (do not chase): `rust_is_subtype_batch` reports 219%
  and the total fallback line went NEGATIVE (-4,950) - per-decision
  counting artifacts; `\`rust_type_analyze\`` share line is a
  kernel-boundary artifact. Discount when ranking.
- Runner note: the repo's ephemeral runner cannot re-register (403,
  admin-blocked; #1249 open). GH `ocr-review` jobs stay `queued`
  forever; the operative review gate is the local
  `ocr review --from origin/main --to <branch> --audience agent`,
  then `gh pr merge --squash --admin` after pr-gate + parity green.
- GH `pr-gate` runs `cargo fmt --check` + `cargo clippy
  -D warnings` - push is cheap, run BOTH locally before every push.

## Waves 33-34 (since the 2026-08-31 refresh)

| PR | Issue | What | Numbers |
|----|-------|------|---------|
| #1416 | #1393 | F2 read flip slices 6-7 (erasetype, join seams) | read flip complete |
| #1417 | #1412 | fix self-return == semantics + line/meta parity; `_VISITOR_HAS_TYPE_KERNEL` never engaged | visitor kernel engages now |
| #1419 | #1418 | wave34: `has_recursive_types` total (wire already carries `is_recursive`); `flatten_nested_unions` alias-aware via 4th resolver arg | hrt 45,565 -> 0 (875,855 calls, 100%); flatten 23,928 -> 165 (99.95%); survey 99.2% -> 99.99% |

Closed alongside: #1412, #1393 (F2 complete), #1397 (F3 partial,
Instance/CallableType only), #1300. #1418 stays open for the flatten
residual + general wave tracking; wave35 runs under #1420.

## Open backlog (next waves; dispatch max ~2 port agents)

1. **#1420 (in flight)**: wave35 defer reduction - close the last ~6k
   defers (agent-130, worktree `mypy-rs-wave35`, branch
   `perf/wave35-defer-reduction`). Ranked rows: `is_subtype` ~1,070,
   `find_unpack_in_list` ~960, `flatten_nested_tuples` ~920,
   `remove_redundant_union_items` ~810, `is_literal_type_like` 646,
   `check_overload_call` ~550, then `infer_constraints_full` ~220,
   `solve_generic_call` ~170, tail ~400.
2. **#624**: meta Phase E1 - the `visit_*` decision-head program that
   unlocks the 50%-Rust milestone. The kernel branch/defer surface is
   nearly exhausted (99.99%); this is the next structural front.
3. **#1249**: runner 403 - needs admin, skip until credentials change.
4. After wave35 merges: re-run the survey; if all top rows are 100%,
   the defer-reduction program is DONE - fold #1418/#1420 and shift the
   loop to #624 E1 ports (one seam at a time, same
   audit-first/Gate/PR discipline).

## Older session record

See `git log` and the closed issue stream (#896..#1417) for the earlier
waves (17..33: F2 mirrors, type-mirror splice ops, subtype/protocol
ports, overload-call fronts); the loop protocol below is unchanged.

## The loop (how to continue)

1. Rebuild + codesign shared `.so` from current main:
   `cargo rustc -p mypy-type-kernel --features extension-module --lib
   --crate-type cdylib --release -- -C link-arg=-undefined -C
   link-arg=dynamic_lookup`, cp to
   `/private/tmp/mypy-rs-local-typekernel/type_kernel.cpython-313-darwin.so`,
   `codesign -f -s - /private/tmp/mypy-rs-local-typekernel/*.so`.
2. Survey: `PYTHONPATH=/private/tmp/mypy-rs-local-typekernel:/private/tmp/mypy-rs-local-resolver:/private/tmp/mypy-rs-local-ast
   uv run --no-sync python scripts/measure_native_share.py > /tmp/survey.txt
   2>&1`; the per-seam table lands on stderr, rank non-100% lines by
   absolute fallbacks (`calls * (1 - native%)`).
3. Dup-check (`gh issue list --state open --search ...`), file a
   conventional issue with the numbers + audit-first method.
4. Dispatch max ~2 coder agents per wave with the full workflow
   briefing (own worktree, private scratch dir, gates, PR flow,
   cross-file exclusions, cleanup duty). Branch from origin/main AFTER
   the previous wave's PRs merge; rebases onto main with sibling
   testtypes.py changes shift line numbers ~40 lines (CI self-check
   runs the MERGE).
5. Agents usually self-merge end-to-end; if one ends right after
   opening its PR, you own: fix the fmt/clippy deltas `gh pr checks`
   reports, `ocr review` locally, then
   `gh pr merge <N> -R codenkirch/mypy-rs --squash --admin`.
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
- Worktree venvs MUST be py3.13 (`uv sync --python 3.13`); a default
  `uv run` pulls 3.14 and the cpython-313 `.so`s fail (ValueError:
  invalid bool value).
- Self-check: same PYTHONPATH + `TEST_NATIVE_TYPE_KERNEL=1 .venv/bin/python
  -m mypy --config-file mypy_self_check.ini -p mypy -p mypyc`.
- `mypy_self_check.ini` has `num_workers = 4`; a bare single-file run
  (`--no-incremental mypy/test/testtypes.py`) reports identical errors
  at DIFFERENT line numbers than CI; match by error text, not line.
- Known CI flake:
  `NativeCompatibilityClassvarSuperSuite::test_parity_every_branch`: one
  rerun = green.
- Rebase protocol when sibling PRs conflict: testtypes.py -> origin/main's
  file + only my suite appended; AGENTS.md -> keep both bullets; lib.rs ->
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
  share (#1109/#1115 trap).
- OCR: GH `ocr-review` is stuck `queued` forever (runner 403, #1249);
  the operative review gate is local `ocr review`, then pr-gate +
  parity green locally, then `--squash --admin`.
