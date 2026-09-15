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

Lane A4 (H1d, #1672) engagement probe on its corpus, `gate_active=False`:

| Seam | Calls | Deferred | Wire bytes |
| --- | --- | --- | --- |
| `rust_should_report_unreachable_issues` | 5662 | 0 | 0 |
| `rust_flatten_lvalues` | 20844 | 0 | 0 |
| `rust_literal_int_expr` | 8957 | 0 | 24 |
| `rust_refers_to_different_scope` | 0 | 0 | 0 |

The zero-call seam is recorded as-is: it is a candidate for retirement rather
than a port, and the lane's own report supersedes this table.

## Why this file exists

Two of the wave's four largest costs were invisible before this wave: the local
full-corpus run duplicated the CI `parity` job on the same commit (8,144 vs
8,198 passed, the 54-test spread is platform skips), and one `ocr review` pass
costs 9m37s at ~761k tokens. Recording them here is what makes the tiering in
`AGENTS.md` falsifiable instead of folklore, and it is what tells the next wave
whether it is re-inflating the loop.