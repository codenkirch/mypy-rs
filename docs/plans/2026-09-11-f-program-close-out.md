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

## Experiment result: the reopening route was run and measured dead

Recorded 2026-09-16 by the F1 assessment lane (#1769), because ADR-0006's
Consequence 1 asked for this record in the close-out and it was never added —
until now the only places the NO-GO existed were the ADR itself and a wave-5
entry in `docs/HANDOFF.md`, while this file and the Phase F section of
`docs/remaining-migration-plan.md` still read as if the route were open.

- **Verdict: NO-GO by ~4.7x, on arithmetic.** ADR-0006 (`58d32ab24`, PR #1694)
  is the experiment for #1671. Its baseline leg measured `serialize_funnel_s`
  1.347s against parse+semanal+type_check 63.572s = **2.119%** of total work, so
  removing the entire funnel at zero cost still beats the native default by
  2.12% against the >=10% bar. The measured arm served 25.1% of funnel calls,
  cut walk encodes 32.4%, cost **+1.67 GB RSS**, and read routing was strictly
  negative (17.43M pyO3 round-trips for zero additional wire saving).
- **It was declined on measurement, not contract.** All four ADR-0004 contract
  surfaces measured satisfiable.
- **Today's retirements widened the gap to ~9-11x.** The funnel's direct call
  counter fell 55% between 09-15 and 09-16 (2,839,692 -> 1,287,230) while the
  whole retire-favourable exposure is bounded at <=4.17s of a >=62s run
  (<=6.7%), so the ceiling fell to ~0.9-1.1% against the same 10% bar.
- **Two loose ends, both still open.** (1) ADR-0006's Status is still
  "Draft. Measured; the maintainer accepts or rejects", and its Consequence 2
  (delete the prototype or file a successor) is unresolved — an owner decision.
  (2) `mypy/test/testtypeview.py` (33 test definitions) is referenced by no CI
  job and no registry: `rg -i typeview .github/` matches nothing, so the
  prototype's own differential suite has no gate at all. That is today's
  "CI green while an assertion could not fail" one level up — here CI does not
  run the assertions.
- **The prototype is still in the tree, default-off, env-gated** (`MYPY_TYPE_VIEW`
  0/1/2 at `mypy/build.py:1386-1397`, store in
  `crates/type_kernel/src/typeview.rs`), so re-running it costs one corpus slot
  and no new code if the owner wants the current head's figure; the counters
  already bound it below 1.1%.

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
