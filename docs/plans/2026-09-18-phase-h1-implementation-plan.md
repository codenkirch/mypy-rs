# Phase H1 implementation plan: Rust checker driver + binder join (#1861)

Date: 2026-09-18. Base: `main` @ `9606f9fba` (G2.2 def-family Var-key flip, #1870).
Worktree `feat/1861-h1`. Design of record: the #1770 brief and the
`docs/plans/2026-09-13-phase-h-readiness-brief.md` corrections.

> **Amendment 2026-09-18 (read before §0):** slices 2–3 below were superseded
> by measurement during slice-1 review. Sections 0 and 5 are retained as the
> original reasoning; see §6 for the recon, the PyO3 crossing microbenchmark,
> the cProfile run, and the resulting decision. The evidence lives on #1861.

## 0. What this slice is, and the honest economics

H1 replaces the Python checker's **control-flow loops** with a Rust driver that
calls back into Python per statement for the `visit_*` bodies, and extends the
existing metadata-only Rust binder to own `Frame.types` through the #1811 `Var`
handle scheme.

The #1770 brief's arithmetic stands: statement dispatch is ~30 ms over ~91k
statements (~1.8% of an 8.4 s run) at the measured 0.33 us PyO3 crossing, and the
saved Python vtable dispatch nets it to roughly zero. **The real ownership mass is
the binder** (eliminating the Python `Frame.types` dict operations), and the
driver is the structural seam that makes a Rust-owned binder reachable. So the
slice is one unit (driver + binder join) per #1861, not a driver-only probe.

This plan splits that unit into ordered, independently-verifiable commits so each
lands behind a default-off gate with the Python fallback intact.

## 1. Why the known asymmetry matters for the differential

The checker is **not idempotent**: it writes `var.type`, `var.is_inferred`,
`defn.type`, `lvalue_node.type` and ~42 node fields, pushes/pops binder frames
and drains `deferred_nodes` / `_type_maps`. There is therefore **no in-run
verify** (unlike G3.1's mode 2). The correctness net is a **cross-run
differential**: byte-identical error output, byte-identical `exportjson` AST +
symbol-table dump, the `_type_maps` invariant, and both deferral budgets.

## 2. Ordered slices

### Slice 1 — gate + Rust-owned counter surface (T2, this commit series)

The measurement-code rule applies: counters are evidence-critical, and a
structural zero or an accounting gap that survives the abnormal path is a defect.
So the instrument ships first, before the driver can use it.

- `mypy/options.py`: `self.native_checker_traversal = False`, next to
  `native_binder` (`options.py:459`), with the phase comment ending "Not in
  OPTIONS_AFFECTING_CACHE." Do **not** add it to the cache tuple.
- `crates/type_kernel/src/checker_driver.rs` (new): a `DriverCounters` struct
  with the §4.2 counter set — `driver_entered`, `statements_dispatched`,
  `callbacks_emitted`, `callbacks_raised`, `bailouts_to_python_loop`,
  `deferred_nodes_deferred`, `unreachable_marked`, `breaks_taken`,
  `visit_block_iterations` — plus pyfunctions
  `rust_checker_driver_reset()`, `rust_checker_driver_counters()`,
  `set_/driver gate mode` mirroring the existing 3-mode `FlipState`. Registered
  in `crates/type_kernel/src/lib.rs`.
- `mypy/test/helpers.py`: add
  `"TEST_NATIVE_CHECKER_TRAVERSAL": ("type_kernel", "rust_checker_driver_mode")`
  to `_NATIVE_ENV_MODULE_PROBES`, and in `parse_options`
  `options.native_checker_traversal = _env_gate("TEST_NATIVE_CHECKER_TRAVERSAL")`.
- `mypy/checker.py`: a Python-side counter increment on the **gate-off** path so
  gate-off and gate-on report the **same** `statements_dispatched` /
  `visit_block_iterations` totals (`MYPY_TK_H1_STATS` env), the §4.2
  "count the same event on both paths" requirement.
- Session-lifecycle dump: `sessionfinish`-style writer wired in
  `mypy/build.py` (mirror `MYPY_TK_SYMTABLE_SESSIONFINISH_OUT`,
  `build.py:470-487`) under `MYPY_TK_H1_SESSIONFINISH_OUT`, `{pid}` expansion,
  never raises.

**Verification (T2):** crate build + `cargo test -p mypy-type-kernel`; a
targeted `testtypes_native_var_key`-style harness that proves counters reset,
increment, and read back; the base-commit / gate-off run must report
`driver_entered == 0`; and the classifier is exercised with a negative control
that proves the counter bites. No wall-clock claim.

### Slice 2 — the Rust driver (T3)

`crates/type_kernel/src/checker_driver.rs` gains the loop bodies. The driver owns:

- `check_first_pass` (`checker.py:1449-1495`): the reachability protocol
  (`binder.is_unreachable()` -> `mark_unreachable` -> `should_report_unreachable_issues`
  -> `is_noop_for_reachability` -> `msg.unreachable_statement` -> **break**),
  the `__all__` tail, calling back to Python for each `visit_*` body via
  `self.accept(d)`.
- `visit_block` (`checker.py:4662-4683`): the inner loop incl. the
  `expr_checker.expr_cache.clear()` per-statement callback.
- `check_second_pass` (`checker.py:1497-1537`): `pass_num += 1`, the
  `deferred_nodes` swap, the `done` identity-set dedup, the `active_typeinfo`
  scope push, `check_partial`.
- `check_partial` / `check_partial_impl` / `check_top_level`.
- `accept_loop` (`checker.py:1630-1690`) if in scope: the fixpoint, the
  `iter == 20 -> RuntimeError` cap, `binder.last_pop_changed`.
- The deferral producer: `handle_cannot_determine_type` (`pass_num < last_pass`),
  `defer_node`, `current_node_deferred` lifecycle (every set/restore/assert site).

The Python `visit_*` bodies are the callbacks. `TypeCheckerSharedApi`
(`mypy/checker_shared.py`) is the enumerable callback contract to model on; the
dispatch direction (Rust owns the loop, calls **out**) has no precedent in the
`H1a`-`H1s` leaf-seam family, which is why this is the first control-flow
inversion in the checker family.

- `Options.native_checker_traversal` gates the driver; gate-off keeps the Python
  loops byte-for-byte. The Python loops call the same native decision heads
  (`_rust_should_report_unreachable_issues`, `_rust_is_noop_for_reachability`)
  unchanged.
- The `daemon` path (`mypy/server/update.py:1141-1156`, `last_pass = 3`) **bails
  out** to the Python loop, counted in `bailouts_to_python_loop`. The daemon
  driver is a follow-up issue; the sort-by-line / strip-merge-resolver ordering
  contract is recorded, not improvised.

**Verification (T3):** full local battery both gate states — `testcheck` +
cold self-check + the fine-grained family (`testfinegrained`,
`testfinegrainedcache`, `testdeps`) — plus the cross-run differential
(error output + `exportjson` byte-diff on a pinned module set) and the
`driver_entered == 0` base-commit run.

### Slice 3 — the binder join (T3, the lever)

Extend `crates/type_kernel/src/binder.rs` so `Frame.types` is Rust-owned:

- Keys become `Var` handles via the #1811 scheme (`identity::handle_of`,
  `TARGET_PINS`, `object_of`), with the live `Var` pinned at capture
  (`_capture_ref`, `node_mirror.rs:92-120`).
- `update_from_options` needs `extract_var_from_literal_hash(key)` and
  `var.is_inferred` / `var.is_argument` per key (`binder.py:393-395`); the Rust
  join resolves handles back through `object_of`, so the Var flags are available.
- Precondition, already landed: #1811 (`Var` binder-key handle translation) and
  #1870 (G2.2 def-family production `Var`-key flip, `VAR_KEY` counters
  `734,324/734,324/0/0` on the 378-file self-check).

**Verification (T3):** the same cross-run differential, `_type_maps` invariant
per module in both states, and the `binder.rs` `pop_frame` refusal-to-pop-last
invariant preserved.

### Slice 4 — CI job + docs

- New `parity-checker-driver` job in `.github/workflows/native-kernel-parity.yml`
  on the `parity` path filter, both gate states: `testcheck`,
  `testfinegrained`, `testfinegrainedcache`, `testdeps`, `testexportjson`,
  `testtypes*.py`, the `exportjson` byte-diff on a pinned module set, and the
  `MYPY_TK_H1_STATS` counter-equality assertion. Model on `parity-symtable-flip`
  (`native-kernel-parity.yml:336-446`).
- Update `docs/plans/type-kernel-seam-ledger.md` and
  `docs/remaining-migration-plan.md` with the H1 rung.

## 3. Negative controls (the differential must be proven to bite)

1. **Reverse the driver's statement order.** The differential must fail the
   `exportjson` byte-diff (`is_inferred` promotion is order-dependent,
   `checker.py:6336-6337`) while the positive path stays green.
2. **Drop the unreachable-marking callback.** The reachability suites and
   `--warn-unreachable` fixtures must fail (the `break` is load-bearing:
   `checker.py:1666`, `:4670`).
3. **Base-commit run** (`9606f9fba`, no driver) must report `driver_entered == 0`.
4. **Tautology check** on every new assertion: each added test needs a control
   that makes it fail.

## 4. Risks (from #1770 §6) and the cheap falsifier

| risk | cheap falsifier |
| --- | --- |
| No fallback: a divergence is a wrong answer, not a defer | reversed-order mutation + `exportjson` byte-diff on a module with conditional assignment |
| Deferral semantics (two budgets, identity dedup, 24 read sites) | diff the `pass_num` histogram + `deferred_nodes` sequence between gate states; a defer-before-annotation fixture |
| 42 node-field writes invisible to error output | `exportjson` byte-diff restricted to `_G2_FUNC_BASE`/`_G2_FUNC_FLAGS` + `Var.is_inferred`/`is_property`; if a field is uncovered, that gap is the finding |
| Same-line deferred-target ordering (set iteration order) | two deferred targets on one line, run twice |
| Plugin hooks (9 sites, checkmember/checkexpr) | `testcheck.py -k plugin` in both states |
| Daemon / incremental interaction | the fine-grained family in both states |

## 5. Open questions this lane must answer with measurements

1. The true corpus size from a real run's counters (the #1770 source-only count
   is a lower bound).
2. Whether `exportjson` covers all 42 mutated node fields
   (`misc/g12_node_shadow_audit.py` is the instrument).
3. The per-call PyO3 callback cost on this host (the 0.33 us figure is
   F-phase-derived and load-affected).
4. Whether the H1b binder rung has any measured ratio (the slice table's ratio
   column is `-`).

## 6. Amendment 2026-09-18: measurement revises slices 2–3

Slice 1 landed the instrument; the instant it existed, the lane used it (and a
profiler) to check the plan's economics before building either heavier slice.
Three results, all recorded on #1861:

1. **The driver is not the seam that makes the binder reachable (recon).**
   Every `Frame.types` operation lives in `mypy/binder.py` at method
   granularity, triggered from `FrameContext.__enter__/__exit__`, `assign_type`,
   `put`, `get`, `cleanse`, `allow_jump`. None comes from the statement/block
   loops the driver would own, so the join is reachable from Python-driven loops
   exactly as the H1b flag store already is. The driver also cannot amortize the
   join: the binder crossings happen inside the Python `visit_*` bodies, which
   run under the driver too.
2. **The 0.33 µs crossing figure is ~10× too high (microbenchmark).** Measured
   on this host: a no-arg PyO3 crossing is ~28 ns, one-arg ~48 ns, versus a
   ~24 ns Python call and an ~18 ns Python dict store. The driver is one
   crossing *plus* one Python callback per statement, so it is a small net loss,
   not the wash §0 assumed.
3. **The binder is ~1.4% of the run, not "the real ownership mass" (cProfile).**
   Summed `tottime` of every `mypy/binder.py` seam is 1.42% of a 6-file corpus
   run; the hot mass is `semanal.py` 11.4%, `types.py` 9.9%,
   `nodes_mirror.py` 9.5%, `nodes.py` 6.1%. Slice 3's ceiling is ~1.4%, under
   after crossing and callback costs.

**Decision.** Slice 2 is parked (measured basis, not cancelled). Slice 3's
premise is falsified: "the real ownership mass is the binder" is true as *state
owned* and false as *time spent*, so it is not built as a perf rung. The next
lane target comes from the measured mass (the existing kernel-mirror direction
over `types.py`/`nodes.py`/`semanal.py`), not a new checker-driver/binder rung.
The parked rung's notes and these numbers are the record if a later endgame
phase makes a Rust-owned checker loop structurally necessary.

