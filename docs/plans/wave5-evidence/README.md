# Wave evidence artifacts

Convention (issue #1679, deliverable 3): each lane writes a small evidence file
for its wave so wave verification reads artifacts instead of re-running suites.
The verifier then runs **one** merged-head battery, not one per lane.

Path: `docs/plans/wave<N>-evidence/<lane>.json`, where `<lane>` is the lane slug
(e.g. `w5-a2`, `coordinator`). Shape:

```json
{
  "lane": "w5-a2",
  "issue": 1670,
  "branch": "feature/g31-symtable-read-flip",
  "tier": "T2",
  "commit": "<merged head sha the numbers were taken on>",
  "gates": [{"cmd": "cargo test -p mypy-type-kernel", "result": "2838 passed; 0 failed"}],
  "counters": {"seam_calls": 0, "defers": 0},
  "provisional_wall_clock": false,
  "uptime": "load averages: 31,51 38,74"
}
```

Rules: counters are load-invariant and are the primary evidence. Any wall-clock
number sets `provisional_wall_clock: true` and carries the `uptime` line from the
same moment. Numbers are as observed, never re-derived.

## Coordinator measurements, 2026-09-15 (wave 5)

Host: 18-core arm64 macOS, load averaged 31 to 106 across the wave (external:
Norton, Spotlight/`fseventsd` at 13.7 GB RSS, another project's mutation run),
uid-501 RSS 37-39 GB against the 51 GB cap.

| Measurement | Value | Conditions |
| --- | --- | --- |
| Release kernel build | 74s | load ~86, per-lane `target/` |
| Release kernel build, cold `target/` | 1m19s, **25 compiled units** | pool slot, load ~31 |
| Release kernel build, after pool claim+reset | 59.4s, **1 compiled unit** | same slot, load ~31 |
| `cargo test -p mypy-type-kernel` | 2838 passed, 0 failed, 11 ignored | reported independently by lanes A8 and A4 |
| Merged-head cold self-check | `Success: no issues found in 353 source files` | lane A7, on `271175175` |
| CI `pr-gate`, docs-only diff | 266s (#1676), 177s (#1683) | GitHub-hosted, parity jobs path-filtered out |
| CI `native-kernel-parity`, 7 checks | 568s | #1681 pre-fix run |
| Local `ocr review`, 3-file diff | 9m37s, ~761k tokens, 1 `[documentation · low]` finding | #1669, of which ~578k input tokens were cache reads |
| Local `ocr review`, workflow-only diff | 1 `[bug · medium]` finding | #1681; caught a filter that would have skipped the job it edited |
| Local `ocr review`, 1-script diff | 2 `[low]` findings | #1683 |
| Tool command cap | 600s | exceeded by a build at load 86; exit 143, and memwatch logged no kill |

### Lane-reported counters (attributed, not re-derived)

Lane A4 (H1d, #1672) engagement probe on a 6-file sample, `gate_active=False`:

| Seam | Calls | Deferred | Wire bytes |
| --- | --- | --- | --- |
| `rust_should_report_unreachable_issues` | 5662 | 0 | 0 |
| `rust_flatten_lvalues` | 20844 | 0 | 0 |
| `rust_literal_int_expr` | 8957 | 0 | 24 |
| `rust_refers_to_different_scope` | 0 | 0 | 0 |

**Correction (2026-09-15, later the same day). The last row was wrong, and the
error was mine, not the lane's.** The zero was a *sample-coverage artifact* of a
6-file probe, not a dead seam. On lane A4's re-run against its own worktree
source, suite-level engagement reads:

| Seam | Calls | Decided | Deferred |
| --- | --- | --- | --- |
| `rust_should_report_unreachable_issues` | 13 | 12 | 1 |
| `rust_refers_to_different_scope` | 16 | 15 | 1 |
| `rust_flatten_lvalues` | 22 | 21 | 1 |
| `rust_literal_int_expr` | 41 | 29 | 12 |

So **do not retire `rust_refers_to_different_scope` on the earlier zero.** The
general lesson, which is why this correction is kept rather than deleted: a
zero-call reading from a sampled probe is a statement about the sample's
coverage, not about the seam. Only a whole-corpus count licenses "retire".

Two related hazards found the same hour, both repaired by the same guard:
- The zero reading above was first taken while the run resolved `import mypy` to
  the **main** checkout instead of the worktree (symptom:
  `_pytest.pathlib.ImportPathMismatchError` on `mypy.test.conftest`). Any count
  is attributable only after
  `PYTHONPATH=<worktree>:<scratch .so dir>:$PYTHONPATH .venv/bin/python -c "import mypy; print(mypy.__file__)"`
  prints the worktree path.
- The probe's own harness exited 0 while raising, so a crashed probe read as a
  green run. A probe whose failure exits 0 is worse than no probe.

## Why this file exists

Two of the wave's four largest costs were invisible before this wave: the local
full-corpus run duplicated the CI `parity` job on the same commit (8,144 vs
8,198 passed, the 54-test spread is platform skips), and one `ocr review` pass
costs 9m37s at ~761k tokens. Recording them here is what makes the tiering in
`AGENTS.md` falsifiable instead of folklore, and it is what tells the next wave
whether it is re-inflating the loop.