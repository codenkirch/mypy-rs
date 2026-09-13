# F-program close-out decision (wave 70B / issue #1573)

Date: 2026-09-11. Basis: read-only audit at `db7f32450`. Persisted by the
orchestrator from the wave-70B brief.

## Decision

**Declare F kernel-complete; retire the F4 rung unclaimed.** The F0-F3
shadow storage stays as landed (all opt-in), the mirror stays as an
opt-in byte-identity audit tool, and the ownership-transfer claim is not
made. Reopening requires the measurement in "Reopening bar" below.

## Status table

| Component | State | Default | Measured |
|---|---|---|---|
| F0 identity service (`identity.rs`) | landed (raw + strong-pin stable) | internal | correctness layer; cycle-retained pins are the documented residual |
| F1 dual-write capture (`types_mirror.py`) | landed | off | capture +78.7s median (+85%, 1.85x); within-10% gate unreachable |
| F2 read flip | landed | off; CI `parity-mirror` | within noise |
| F3 write splices (Instance/CallableType) | landed | off; not in CI | mutation-only: 18-287 changed writes/run; cannot serve construction |
| librt wire-cache splice | in-repo build, CI scratch only | inert in stock checkout | PyPI wheel lacks `write_raw_bytes` |
| Proxy P1 scaffold | landed | off, zero production callers | inert scaffold (wire or delete) |
| Proxy P2 lazy shadow | dropped | - | wall -0.5%, CPU +5.6% median |
| Proxy P2b thresholded | dropped | - | corpus A +8.97% wall; corpus B -11% inside noise; ceiling <=0.5% |
| Cache-data bridge | landed | off | hybrid ~1.5x the Python writer per round |

## Why the claim cannot be earned by the shipped architecture

- Every F mechanism measured zero or worse; the two proxy patches are
  preserved in `/private/tmp` and must not be re-run as-is.
- The addressable wire traffic was measured in single-digit seconds of a
  13-27s phase, and resolver upkeep is unaffected by type storage.
- The only architecture that removes per-object Python work is
  replacement views, which ADR-0004 Decision 1 declined for plugin /
  `isinstance` / `__slots__` / astmerge contract reasons.
- The remaining seam defers are documented engine floors; deferral
  shaving is exhausted as a lever.
- F is not on the critical path for Phase G (G0.1-G0.5 landed without it).

## Reopening bar (falsifiable)

The one non-noise mechanism is replacement views for one family,
requiring a successor to ADR-0004 Decision 1. Corpus: cold self-check
(`mypy_self_check.ini -n0 --no-incremental -p mypy -p mypyc`, 3+
interleaved pairs, quiet host, `scripts/measure_work_share.py`) plus full
parity (testcheck/testtypes/fine-grained/daemon/plugin suites). Target:
the one-family view prototype beats the current native default by >=10%
relative total work share with capture/read off, all parity green.
Phase-scoped capture is excluded (P2b bounded its ceiling).

## Load-bearing facts (do not re-derive)

1. The proxy read shadow is measured-exhausted, not untested (P2/P2b
   numbers above; 91% of blobs <=64B, 22% wire-cache hit rate).
2. Capture overhead is fixed per-object semantics, not tunable slack
   (2.3M registrations + 2.9M fresh serializations; wave-65A already cut
   37.3%).
3. Identity is strong-pin only (all eight Type classes refuse weakrefs)
   and the wire-cache splice depends on the in-repo librt build; never
   upgrade the shared `.venv` librt.

## Optional cleanups (non-blocking)

- Wire or delete the inert proxy scaffold (`type_proxy.py` has zero
  production callers).
- Refresh the overlapping F briefs rather than keep three snapshots
  (`phase-f-next-steps.md` weakref/CI-coverage claims; the audit doc's
  stale #1530 deselect note -- re-enabled in #1535).
