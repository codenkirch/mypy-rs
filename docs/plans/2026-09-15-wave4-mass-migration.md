# Wave 4 plan — big-swarm mass migration (continuation)

## Objective

Land or floor every remaining open item in the mass-migration blocker inventory
with before/after measurements, close the Wave-3 loose ends, and advance the
G-phase read-channel scaffold only where it is parity-covered and
production-live. After this wave the backlog table is empty — what remains is
genuinely new work (the ownership flip), not backlog.

## Starting position

`main` = `c16936dd6`. Open and queued:

| Issue | Status | What |
|---|---|---|
| #1626 | open | enumerable plugin hook surface (native hook registry live with user plugins) |
| #1628 | open | variadic-middle + Unpack-ParamSpec substitution arms (closes #1621 rank 1) |
| #1621 | open | ParamSpec/TypeVarTuple variables channel — closes into #1628 |
| #1627 | open | pure DefaultPlugin hook bodies — **floor** (0.02–0.05% wall; gated on a full check_callable_call port that does not exist) |
| #1635 | open | landed as PR #1655 (a2b6d604f) — auto-close missed, close now |
| #1624 | standing | perf regression blocker — this wave's slices recorded on it |

Standing: #1624, #1626–#1629. RSS 24 GB, memwatch alive → four parallel
workers supported.

## Phase 1 — big swarm, four workers, disjoint files

| Worker | Issue | Scope files | Measure / gates |
|---|---|---|---|
| W-A1 | #1626 plugin hook surface | `mypy/plugin.py`, `mypy/plugins/default.py`, `mypy/plugins/proper_plugin.py`, `mypy/build.py` (`_build_plugin_hook_registry` ~2058–2099), `mypy/checkexpr.py` bit-sites (~500–637) | extend `NativePluginHookSuite` (testtypes EOF); self-check 0 err with fast path live; testcheck exact; record probe-answer counts |
| W-A2 | #1628 variadic-middle + Unpack-ParamSpec splice + guard lifts | `crates/type_kernel/src/expandtype.rs`, `solve.rs:1948-1963`, `checkcall.rs:854-864`, `mypy/checkexpr.py:3085-3087`, `mypy/expandtype.py` (reference) | Rust units; `NativeSolveGenericCallSuite` / `NativeConstraintsDeferralSuite`; gate-off/on byte parity; cold self-check; report need_refresh/var_pspec_tvt re-measure. HIGH RISK (segfault class) |
| W-B1 | new perf issue — typeops scalar cut | `mypy/typeops.py`, `crates/type_kernel/src/typeops.rs` (is_literal_type_like ~:651 + leftovers per re-probe) | `NativeTypeopsDeferralSuite` pins; testcheck exact; self-check; `MYPY_SERIALIZE_STATS` round-trip drop (absolute) |
| W-B1b | new perf issue — checkmember scalar/live cut | `mypy/checkmember.py`, `crates/type_kernel/src/checkmember.rs` (analyze_instance_member_dispatch ~:2295) | `NativeMemberAccessDispatchSuite` + IAMA suites must hold ~92–95% native; testcheck exact; self-check; before/after share |

Disjoint-file contract: checkexpr.py split A1 (~500–637) vs A2 (~3085) far
apart; build.py only A1 this wave; no worker touches `ast_serialize` or
`module_resolver`. testtypes.py / lib.rs / type_kernel.pyi use R4.

## Phase 1.5 — queued (wave 1.5, after the first Phase-1 merge on a clean tree)

- **W-B3**: semanal gate default-ON flip (issue to create). Files:
  `mypy/build.py:1088-1092,1206-1209` + the seam files the re-probe names.
  Re-measure net-loss on the current tree; convert residual net-loss seams to
  a non-wire interface; flip `_native_semanal_active` /
  `_native_semanal_visitor_active` default-ON; measure work-share on a quiet
  host (measure_work_share.py); testcheck exact in BOTH gate states (parallel
  mode forces native anyway); fine-grained + self-check. Highest-value
  remaining item, also the riskiest — queued behind a merge so the 51 GB rule
  holds and the tree is clean.

## Wave rules (carried from Wave 3)

- **R1 — direction is advisory, evidence rules.** Two consecutive corrections
  proved issue text goes stale. Every worker re-probes before implementing and
  may change shape with numbers.
- **R2 — resource discipline.** memwatch check + combined RSS < 44 GB before
  heavy builds, `-n2` pytest max, `CARGO_BUILD_JOBS=4`, own scratch `.so`
  (never `/private/tmp/mypy-rs-local-*`), `.venv/bin/python` only, NEVER
  `maturin develop`, cargo build SEPARATELY from pytest.
- **R3 — landing protocol.** Strip probes, explicit staging, CI parity green
  (local testcheck 8,144/69/7; CI 8,198/15/7), OCR blockers-only, rebase
  keep-both-sides, `--admin` squash merge. Each worker fully self-lands its
  own PR and closes its own issue; the coordinator verifies and files the
  ledger.
- **R4 — test-collision protocol.** testtypes.py EOF append + keep-both on
  rebase; lib.rs/type_kernel.pyi alphabetical placement.
- **R5 — coordinator.** Issue closing for non-auto-close, ledger in AGENTS.md
  (docs: commits pushed directly to main), cleanup, handoff, final combined
  verification.

## Risks

- W-A2 is the highest-risk worker (wave-33 segfault class: variadic middle →
  `make_simplified_union` / `is_subtype` / `tuple_fallback` recursion). Land
  gated, flip one guard at a time; if parity cannot be shown, close #1628/#1621
  as floored with the variance.
- B3's flip is the riskiest structural change (121 dark guard sites, 11,687
  dark Rust lines) — that is why it is serialized to wave 1.5, not in the
  swarm.
- No wire-format change this wave (the paramspec brief forbids a
  `CACHE_VERSION` bump; `variables` is already in the wire). W6's bump landed
  in Wave 3; nothing here touches the persistent format.
- Memory is comfortable (24 GB) for four workers; B3 waits for a merge to free
  headroom.

## Success criteria

Six issues landed-or-floored with before/after measurements; zero slice issues
left open except deliberate residuals; `#1635` and `#1627` closed with
evidence; `#1624` recorded on with this wave's slices; ledger + handoff
updated; combined head self-check clean (353 files / 0 errors).

## Explicit non-goal

This wave does not start the ownership flip (Rust owning the nodes/types
graphs) and does not grow the dark G-phase surface. It finishes the remaining
blocker inventory, the solve-path gaps, and the perf close-out — the
precondition for the flip, not the flip itself.
