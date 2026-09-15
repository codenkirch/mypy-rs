# Wave 3 — Mass Migration

**Date:** 2026-09-15
**Starting position:** `main = 45c94f541`
**Objective:** Empty the queue. After this wave, every item in the #1625
blocker table and the Phase-2 slice list is either landed or explicitly
floored with evidence. What remains after that is genuinely new work
(the ownership flip), not backlog.

**Explicit non-goal:** this wave does not start the ownership flip (Rust
owning the nodes/types graphs). It finishes the strangler-head inventory,
the wire/solver gaps, and perf close-out — the precondition for that flip,
not the flip itself.

## Open and queued

- #1642, #1637 (perf)
- #1634, #1635 (module slices)
- #1628, #1629 (solver)
- #1620 (wire remainder)

Standing: #1624, #1626–#1629.

## Phase 1 — five workers, disjoint files

| Worker | Issue  | Files                                       | Gate suites                                                                 |
|--------|--------|---------------------------------------------|-----------------------------------------------------------------------------|
| W1     | #1634  | complex-statement drivers (try/for/with/match) `mypy/checker.py`, `checker_functions.rs` | full testcheck `-k "not slow"` (checker.py is the hottest file — no subsetting), self-check |
| W2     | #1635  | subexpr/aststrip surgery `mypy/server/subexpr.py`, `aststrip.py`, new Rust module | fine-grained family (the long pole — starts first) |
| W3     | #1642  | check_call cluster `mypy/checkexpr.py`, `checkcall.rs` | testcheck call/overload/inference subsets, testinfer |
| W4     | #1628  | expandtype substitution arms `mypy/expandtype.py`, `expandtype.rs` | testtypes expand suites, testcheck inference subset |
| W5     | #1629  | spike only — pass-1-only solve split viable? | measurements + go/no-go light suites; no PR unless go |

## Phase 2 — after Phase 1 merges, serialized deliberately

- **W6:** #1620 wire remainder (definition slot, ParamSpec/TVT
  `meta_level`, `PartialType`, `ErasedType`) + `CACHE_VERSION` bump.
  Runs alone: a format bump touches everyone, so it lands on a quiet
  tree. Must pass `testfinegrainedcache`, `testmerge`, cold + warm
  self-check.
- **W7:** #1637 sweep pass over the post-Phase-1 tree — delete/gate
  leftover scalar round-trips the module ports expose. Runs after W6
  (both touch `types.py` edges).

## Wave rules

Two new rules (R1, R4); the rest are carried from prior waves.

- **R1 — direction is advisory, evidence rules.** Two consecutive
  corrections (#1640's shape, #1641's mechanism) prove the issue text
  goes stale. Every worker re-probes before implementing and may change
  shape with numbers.
- **R2 — resource discipline unchanged:** 46 GB backpressure check,
  `-n2`, `CARGO_BUILD_JOBS=4`, own scratch `.so`, `.venv/bin/python`
  only.
- **R3 — landing protocol unchanged:** strip probes, explicit staging,
  CI parity green, OCR blockers-only, rebase keep-both-sides, `--admin
  squash merge`.
- **R4 — test-collision protocol:** `testtypes.py` EOF append +
  keep-both on rebase; `lib.rs`/`type_kernel.pyi` alphabetical
  placement. This tax appeared in all five Wave-2 PRs and cost minutes,
  not hours — accepted.
- **R5 — coordinator owns:** issue closing for non-auto-close, ledger,
  cleanup, handoff, final combined verification.

## Risks

- **W1** has the highest regression surface (`checker.py`); mitigated by
  requiring the full non-slow testcheck locally, not a subset.
- **W6**'s cache bump is the only persistent-format change in the wave;
  any drift blocks it, everything else is in-process FFI.
- **W5** may return no-go — that closes the spike successfully, it
  doesn't block the wave.
- Memory supported five workers last wave; Phase 2 drops to one or two.

## Success criteria

- Six issues landed-or-floored with before/after measurements.
- Zero slice issues left open except deliberate residuals.
- Ledger + handoff updated.
- Combined head self-check clean.
