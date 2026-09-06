# Handoff: strangler-fig Rust migration loop (seam-deferral reduction)

*Written 2026-08-28, refreshed 2026-09-06 (post-wave41, #1439 closed by
#1440: rust_check_overload_call wrapper 95.9 -> 99.1% native, wrapper
defers 563 -> 101, the 386 type-object-target defers retired).
Goal: "migrate
all python code to rust, really all", pursued as the established measure
-> file -> dispatch-agents -> process-PRs -> gate loop. This file is the
resume point.*

## Where main stands (2026-09-06, post-wave41)

- `main` = `007323a11` (`perf(type_kernel): roc typeobj targets decided
  via shim gate facts (#1440)`),
  local ff'd to origin.
- Phase state: F0 audit + F1 dual-write mirror + F2 read flip (slices
  1-10, #1393) all landed. F3 (#1397) write flip has Instance +
  CallableType splice ops; the planned tvar/union splice slice was
  profiled with `misc/f3s9_tvar_union.py` and came back EMPTY (zero
  per-field writes on a self-check) - dropped, do not build it.
- Gates on the wave-41 squash content: `cargo fmt` + clippy -p
  mypy-type-kernel clean on the merged head (re-verified 2026-09-06
  with a forced re-emission run) + 2,675 kernel unit tests;
  testtypes differential 3,182 passed / 6 skipped (+5 gate-fact tests);
  testcheck 8,144 passed / 69 skipped / 7 xfailed (exact parity
  baseline); cold self-check clean (202 files).
- Shared `.so` rebuilt + codesigned at `007323a11` content
  (`/private/tmp/mypy-rs-local-typekernel|resolver|ast`).
- Survey (post-wave41): 7,735,729 seam calls;
  `rust_check_overload_call` 13,750 @ 99% (~101 wrapper defers, was
  563); `rust_is_subtype` 25,666 @ 99%;
  `rust_infer_constraints_full` 21,375 @ 99%;
  `rust_solve_generic_call` 8,506 @ 98%;
  `rust_remove_redundant_union_items` 10,101 @ 99%.
- Survey caveats (do not chase): `rust_is_subtype_batch` reports 228%
  and the total fallback line went NEGATIVE - per-decision counting
  artifacts; discount when ranking.
- Runner note: the repo's ephemeral runner cannot re-register (403,
  admin-blocked; #1249 open). GH `ocr-review` jobs stay `queued`
  forever; the operative review gate is the local
  `ocr review --from origin/main --to <branch> --audience agent`,
  then `gh pr merge --squash --admin` after pr-gate + parity green.

## Waves 33-40 (since the 2026-08-31 refresh)

| PR | Issue | What | Numbers |
|----|-------|------|---------|
| #1416 | #1393 | F2 read flip slices 6-7 (erasetype, join seams) | read flip complete |
| #1417 | #1412 | fix self-return == semantics + line/meta parity; `_VISITOR_HAS_TYPE_KERNEL` never engaged | visitor kernel engages now |
| #1419 | #1418 | wave34: `has_recursive_types` total (wire already carries `is_recursive`); `flatten_nested_unions` alias-aware via 4th resolver arg | hrt 45,565 -> 0 (875,855 calls, 100%); flatten 23,928 -> 165 (99.95%); survey 99.2% -> 99.99% |
| #1422 | #1420 | wave35: audit-first alias-wall closure - `find_unpack_in_list` non-strict decode, `flatten_nested_tuples` alias fold via `expanded_alias_target` + re-entry guard, applied-alias expansion in `flatten_nested_unions` (+ shim `row_expansions` startup path), `is_literal_type_like` snapshot threading; OCR composed fixes (per-level args, level-0 args contract, no_args chain resolution + Cow) | fui 700->0, fnt 770->0, fnu 163->1, lit 544->0; audit 2,176/3,763 defers eliminated |
| #1424 | - | docs: HANDOFF refresh for post-wave35 loop state | - |
| #1425 | #1423 | wave36: alias prepass closes the `is_subtype` engine walls - `expand_top_aliases` resolves top-level alias chains through the resolver snapshot at the entry of `is_subtype`/`is_same_type`/`is_equivalent`/`is_more_precise`/batch seam, `alias_assuming_contains` RAII recursion guard, alias fold in the `remove_redundant_union_items` + `check_argument_types_plan` paths; OCR composed fixes (scope-gated assuming walk, per-level args contract, Cow alias shapes) | #1423 closed; st engine walls shut (2,176+ defers eliminated), post-wave37 residual st ~3%/rru ~8% only |
| #1428 | #1426 | wave37: port `unify_generic_callable` non-generic-right arm (`unify.rs::unify_generic_callable_core`) + thread ambient `infer_unions` through the subtype seams. Generic-right (cc_vars, extra_tvars) shapes + 6 residual `p42619` 1|0 defers stay by design | sgc 8,502 @ 98% post-wave37 |
| #1430 | #1427 | wave38: kernel `extra_tvars` channel (Rust-internal `Vec<Type>` on `Constraint`, wire stays 3-field, `Eq` keeps Python's 3-field semantics) + ambient `infer_polymorphic` mode plumbing through constraints/visitor/solve `unify` shims; testtypes ambient-flake pin commit `a09283a5d`. Honest outcome: the headline sgc share did NOT move (below wrapper granularity); real corpus wins are at constraint-builder level | icf 21,734 -> 21,673 calls; skip_reverse_union_constraints 82 -> 49 (100%); sgc 8,502 / 139 fallbacks unchanged |
| #1434 | #1433 | wave39: audit-first rru wall - kernel now decides the mutated-survivor pairs natively in `remove_redundant.rs` (`scripts/rru_audit_driver.py` audit: 9,292 ok @91.8% -> 10,023 ok @99.0%, 728 widen_mutated defers -> 0); checkoffset plan/plan-server check stays; all 3 OCR files clean (0 comments) | rru 10,124 calls @ 99% native post-squash (from 92%); testtypes 3,177/6, testcheck exact |
| #1437 | #1436 | wave40: u:def solve-chain + dependent-solve bounds + tvar-bearing return-solve decided natively (constraints/solve/subtypes/unify/callable_compat/checkcall); OCR rounds: 2 blocking fixed (`5139dabf0`: protocol-member live_typeinfo None guard + nested-owned-tvar doppelganger deferral), 10 advisory noted unpushed. Wrapper roc unchanged (driver-level defers are the roc residue); embedded engine defers fell 7,337 -> 4,580 (cc:unify 814 -> 110, st |Callable|Callable 348 -> 37, u:def 766 -> 50) | st wrapper 98 -> 99% (25,666 @ 99% post-squash); testtypes 3,177/6; testcheck exact |
| #1440 | #1439 | wave41: roc type-object targets decided through shim gate facts (per-target `_typeobj_gate_flag_for_roc` pre-argument instantiation-gate scalar on the opt-in `typeobj_gate_fails` seam), OCR round-1 fix (missing/unknown gate fact defers, `6060cb253`), round-2 advisory constants (`7c36ccb1f`); gen_solve 112 -> 89, dupcheck 79 -> 27; no_match/star/plain buckets by design; advisory: 1 pushed | roc 95.9 -> 99.1% native (563 -> 101 defers); testtypes 3,182/6; testcheck exact; clippy clean on head |

Closed alongside: #1412, #1393 (F2 complete), #1397 (F3 partial,
Instance/CallableType only), #1300, #1418 (closed 2026-09-05 with the
#1419/#1422 pointers), #1420 (auto-closed by #1422), #1423 (#1425
auto-closed it), #1426 (#1428 auto-closed it), #1427 (#1430 + manual
close, PR body lacked the `Closes` line), #1424, #1425, #1426,
#1428, #1429, #1430, #1431, #1433 (#1434 + manual close), #1434,
#1435, #1436 (#1437 + manual close), #1437, #1439 (#1440
auto-closed it).

## Open backlog (next waves; dispatch max ~2 port agents)

1. **Wave 42 (issue to file, dup-check first)**: the three engine
   residues all ~99% now - st 25,666 @ 99% (~257), icf 21,375 @ 99%
   (~214), sgc 8,506 @ 98% (~170). Audit their remaining defer buckets
   once more (post-wave40/41 rounds changed the upstream shape) and
   retire what is decidable; expect diminishing returns, document an
   honest taxonomy.
2. **#624**: meta Phase E1 - the `visit_*` decision-head program that
   unlocks the 50%-Rust milestone. The kernel branch/defer surface is
   exhausted (top rows 100%); this is the next structural front.
3. **#1249**: runner 403 - needs admin, skip until credentials change.

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
- GH `pr-gate` runs `cargo fmt --check` + `cargo clippy
  -D warnings` - push is cheap, run BOTH locally before every push.
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
