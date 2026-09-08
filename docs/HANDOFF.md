# Handoff: strangler-fig Rust migration loop (seam-deferral reduction)

*Written 2026-08-28, refreshed 2026-09-08 (post-wave48: wave 48
(#1464) closed via #1465 (C1 `get_type_range_of_type` leaf dispatch +
C2 checkexpr typeobj-fail gate); wave 44 #1444 remains a documented
negative result). Goal: "migrate all python code to rust, really
all", pursued as the established measure -> file -> dispatch-agents
-> process-PRs -> gate loop. This file is the resume point.*

## Where main stands (2026-09-08, post-wave48)

- `main` = `4a7f5634a` (`feat(type_kernel): wave 48 - C1
  get_type_range + C2 typeobj gate ports (#1465)`), local ff'd to
  origin.
- Phase state: F0 audit + F1 dual-write mirror + F2 read flip (slices
  1-10, #1393) all landed. F3 (#1397) write flip has Instance +
  CallableType splice ops; the planned tvar/union splice slice was
  profiled with `misc/f3s9_tvar_union.py` and came back EMPTY (zero
  per-field writes on a self-check) - dropped, do not build it.
- Gates on the wave-48 head `4a7f5634a`: cargo fmt + clippy -p
  mypy-type-kernel clean, 2,721 kernel unit tests / 11 ignored;
  testtypes 3,243/6 (kernel ON); testcheck 8,198/15/7 (exact
  wave-47 baseline); cold self-check clean (347 files); pr-gate +
  parity + parity-typeops green on #1465.
- Shared `.so` rebuilt + codesigned at the wave-48 head content
  2026-09-08 (`/private/tmp/mypy-rs-local-typekernel`; resolver/ast
  unchanged, still valid).
- Survey (post-wave48, 2026-09-08, total 7,928,970 seam calls):
  C1 `rust_classify_type_range` 9,264 @ 100%; C2
  `rust_classify_typeobj_gate` 167,443 @ 100%; st 25,496 @ 99%
  (~256, floors documented in AGENTS.md), icf
  (`rust_infer_constraints_full`) 21,425 @ 99% (~214, floors), roc
  13,818 @ 99% (~138), ama 12,334 @ 99% (contract floor), ct
  (`rust_conditional_types`) 8,658 @ 100% (embedded residual 29
  documented), sgc 8,533 @ 98% (~170 wrapper; #1460 embedded
  counts 251), ifta 189 @ 30% wrapper-unmoved (embedded: 56 native
  of 188, floors var_pspec_tvt 80 / engine 47 / solve 5; the
  50%->30% wrapper share was an audit artifact, never real).
  Wave-45/38 lesson stands: rank waves by embedded audits, not
  wrapper movement (ct was the one wrapper-visible mover this wave).
- Survey caveats (do not chase): `rust_is_subtype_batch` reports 228%
  and the total fallback line went NEGATIVE - per-decision counting
  artifacts; discount when ranking.
- Runner note: the repo's ephemeral runner cannot re-register (403,
  admin-blocked; #1249 open). GH `ocr-review` jobs stay `queued`
  forever; the operative review gate is the local
  `ocr review --from origin/main --to <branch> --audience agent`,
  then `gh pr merge --squash --admin` after pr-gate + parity green.

## Waves 33-48 (since the 2026-08-31 refresh)

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
| #1446 | #1442 | wave42: unify-engine residue - `FlatAliasGuard` restores the previous `FLAT_ALIASES` value on Drop (nesting-aware install per engine call); the union-flatten arm expands alias items through the live map; `flatten_union_expanding_aliases` walks with an active identity stack (re-entered (type_ref, args) defers the whole flatten, mirroring Python's lazy unroll, types.py:3855, pins testRecursiveAliasesJoins); `expand_top_aliases` takes the shared Arc alias snapshot directly, is_subtype entry checks `resolver.aliases()` explicitly; recursive-union alias targets keep the deliberate no-install behaviour | alias-expansion defer residue inside the st/icf/sgc engines retired; wrapper counts stayed flat (wave-38 lesson: below-wrapper granularity; post-wave43 survey confirms st ~256 / icf ~214 / sgc ~170 unchanged at wrapper level); gates exact (2675/11, 3182/6, 8144/69/7) |
| #1445 | #1443 | wave43: retire decision-seam defers - by_name/by_position 4-tuple wire result now carries the formal's arg index (shim builds FormalArgument from the right slot instead of re-deriving; star-arg formals report index -1 and defer); rust_join_type_list three LKV retirements (same-ref args-less LKV of a plain class via the prejoin fresh Instance, LKV+extra_attrs dropped, join.py:392; LKV on distinct args-bearing instances + LKV nested in the signature stay deferred) | by_name 284 (was 82%), by_position 143 (was 68%) -> both 100% native; jtl 55 calls 45% -> 71% native; standing audited defers: ovl ~60, xbe ~50, jt 21, aa 20, fto 24, pc 8 |
| - | #1444 | wave44: format/resolve-family seams (~260 defers) - DOCUMENTED NEGATIVE (see wave-45 row for the follow-up) | negative result; follow-up #1447 (wave 45) |
| #1451 | #1447 | wave45: fmt:alias_top in format-type seams - non-recursive TypeAliasType expands through the alias snapshot (`expand_alias_for_format`: chain resolve + `no_args` instance-swap + tvar-arg substitution) and formats the expanded target byte-identically; recursive aliases / missing snapshot / cycle / variadic shapes defer (`<alias (unfixed)>` shape, wave-33 segfault guardrail: cycles via the `is_recursive` flag, never recursion). OCR: 1 blocking [bug] fixed by the orchestrator (zip truncation on snapshot arity mismatch -> arity guard defers; 2 pin unit tests) + advisory (redundant pre-lookup) applied in the same fix commit (#1440 precedent) | fmt alias defers 42 -> 0; testtypes 3,191/6; testcheck 8,144/69/7 exact |
| #1452 | #1450 | wave46b: ct residual - `rust_conditional_types` structural-branch `Some(false)` now falls through like Python's `if is_subtype(...)`; only undecided engine shapes defer | 67 -> 29 defers (embedded); wrapper 8,650 calls @ 99.76%; cargo test 2,682/11; testtypes 3,184/6 (+2); testcheck exact |
| #1460 | #1455 | wave47-B: ifta wire-framing fix + definition restore (agent B) | ifta true-native 0 (decode_mismatch all 56) -> 56 (~30%); floors var_pspec_tvt 80 / engine 47 / solve 5; cargo 2700/11, testtypes 3208/6, testcheck 8198/15/7, self-check clean |
| #1458 | #1455 | wave47-A: st/icf residual audits -> floor decisions, docs-only (agent A) | 0 ports; buckets -> #1456 (fake-TypeInfo snapshot), #1457 (nested-alias wall); icf 266 = protocol-member engine class, multi-wave floor; testtypes 8198-equivalent gates green |
| #1453 | #1449 | wave46a: ama residual - is_self blanket defer retired, enum head gate retired, latent non-callable `call_type` defer retired. OCR: 1 blocking [bug high] fixed by the agent (stale snapshot `enum_members` -> live read) | embedded 181 -> 120 events = contract floor (taxonomy in project memory `ama-residual-contract-floor.md`); cargo 2,689/11; testtypes 3,201/6 (+10 NativeAmaResidualSuite) |
| #1465 | #1464 | wave48: C1 `rust_classify_type_range` (`TypeChecker.get_type_range_of_type` leaf dispatch, zero-wire PyO3 classifier; union/typevar folds stay Python) + C2 `rust_classify_typeobj_gate` (`check_callable_call` protocol/abstract typeobj gate, double-eval collapsed to one `is_type_obj`). Orchestrator merged (parity green, ocr-review runner-stuck per #1249); OCR round 2: 2 substantive advisories -> #1466 (open bug, below) | C1 9,264 non-union leaves @ 100% native (9,286 calls; 20-item union fold stays Python); C2 167,443 gate calls @ 100% (88.9% short-circuit before `type_object()`); cargo 2,721/11 (+21); testtypes 3,243/6 (+35 NativeTypeRangeSuite + NativeTypeobjGateSuite); testcheck 8,198/15/7 exact; self-check clean 347 |

Closed alongside: #1412, #1393 (F2 complete), #1397 (F3 partial,
Instance/CallableType only), #1300, #1418 (closed 2026-09-05 with the
#1419/#1422 pointers), #1420 (auto-closed by #1422), #1423 (#1425
auto-closed it), #1426 (#1428 auto-closed it), #1427 (#1430 + manual
close, PR body lacked the `Closes` line), #1424, #1425, #1426,
#1428, #1429, #1430, #1431, #1433 (#1434 + manual close), #1434,
#1435, #1436 (#1437 + manual close), #1437, #1439 (#1440
auto-closed it), #1442 (#1446 auto-closed it), #1443 (#1445
auto-closed it), #1444 (wave 44, closed by hand with the
documented negative result), #1445, #1446, #1447 (#1451
auto-closed it), #1449 (#1453 auto-closed it), #1450 (#1452
auto-closed it), #1451, #1452, #1453, #1455 (commented + closed by
hand), #1459, #1458, #1460, #1464 (#1465 auto-closed it), #1465.
Open as follow-up: #1466 (bug: wave-48 classifier seams propagate
PyErr via `?` instead of deferring to `Ok(None)` on an unreadable
attribute; the Python shims catch only
AssertionError/NotImplementedError/ValueError/TypeError, so a
malformed live type object could crash the checker instead of
falling back - type_range.rs:111-158, checkcall_typeobj.rs:17-73).

## Open backlog (next waves; dispatch max ~2 port agents)

1. **#1466 (open bug, small)**: wave-48 seams' AttributeError
   deferral-contract gap - Rust `?` propagation vs Python shim's
   4-tuple catch. Fix = map attribute-read failures to `Ok(None)`
   (the documented "None defers" contract) + pin tests; low-risk.
2. **#1462**: sgc (~251 embedded) + ct (~29) embedded defer audits,
   unfinished from wave 47.
3. **#1457**: st nested-alias wall port (biggest single wall).
4. **#1456**: fake-TypeInfo snapshot registration.
5. **#624**: meta Phase E1 after the wave-48 C1/C2 slice proved the
   classifier shape; B1/B3 (build-cache front) and B7 (astdiff) are
   the bytes-heavy follow-ups; semanal candidates are opt-in only.
6. **#1432**: chore - unify.rs polish. Low priority.
7. **#1249**: runner 403 - needs admin, skip until credentials change.

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
2. Survey: `PYTHONPATH=$PWD:/private/tmp/mypy-rs-local-typekernel:/private/tmp/mypy-rs-local-resolver:/private/tmp/mypy-rs-local-ast
   uv run --no-sync python scripts/measure_native_share.py > /tmp/survey.txt
   2>&1`; the per-seam table lands on stderr, rank non-100% lines by
   absolute fallbacks (`calls * (1 - native%)`). The leading `$PWD`
   is REQUIRED: `uv run --no-sync` does not install the project, and
   script mode puts `scripts/` (not the repo root) on sys.path, so
   `import mypy` fails with ModuleNotFoundError without it (hit
   2026-09-07).
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
- Do not idle-wait: run `agent-wait` in the background and stop calling
  tools until the notification.
- testtypes gate counts are only meaningful WITH
  `TEST_NATIVE_TYPE_KERNEL=1`; without it ~3k native-gated cases
  report as skipped and the count masquerades as pass.
- OCR tool bug: on large diffs the local ocr can fail with
  `file_read failed: invalid line range: start_line N is greater than
  end_line M` (hit 2026-09-07 on wave-45 first attempt and wave-46a
  runs 2-3). It is resumable: rerun with `--resume <session-id>` (the
  failure prints the session id), or just rerun fresh; the diff is
  rerolled and the run usually succeeds. Do not treat the error as a
  finding.
