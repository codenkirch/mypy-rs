# Handoff: strangler-fig Rust migration loop (seam-deferral reduction)

## RESUME POINT — 2026-09-18, morning (wave 12 closed: 17 PRs merged; #1860 G4 flip NO-GO, unmerged)

`main` = `47209c257` (#1859). Seventeen PRs merged this wave; the wave's one
T3 lane (#1860) finished with a decisive NO-GO and closed unmerged.

### Merged this wave

| merge SHA | PR | what |
|---|---|---|
| `cb09a1dc7` | #1826 | feat(type_kernel): statement-family serve seed + handle encoding for stmt.expr (#1787) |
| `a0b8a5350` | #1829 | fix(audit): name the remedy when type_kernel is not importable (#1821) |
| `9601d2b9c` | #1830 | perf(type_kernel): retire the checkexpr call head and arg-context shims (#1739) |
| `3546293d1` | #1832 | test(checkmember): pin values where a gate differential cannot fail |
| `1b20d5d65` | #1839 | feat(parity): cross-run differential runner for gate-state comparison (#1770) |
| `e0085dcf5` | #1840 | fix(audit): crashed-run refusal, extension identity, unconsumed cause split |
| `655017555` | #1843 | perf(type_kernel): retire rust_classify_typeobj_gate (#1833) |
| `19093d51d` | #1844 | feat(type_kernel): explicit load-time seed + provenance split for the def-family store (#1825) |
| `40c64f541` | #1848 | test: pin values where the remaining gate comparisons cannot fail (#1834) |
| `d4e59684c` | #1850 | fix(audit): shared-blob attribution, operator abort, type-misc green |
| `dfa12ad20` | #1851 | test(type_kernel): repair two vacuous pins and a stale retirement claim |
| `0a97f7f35` | #1852 | fix(type_kernel): def-family seed hygiene - dedupe, dead constants, zero counters |
| `da333de00` | #1854 | test: pin values where the 22 gate comparisons cannot fail (#1845) |
| `9393db907` | #1855 | fix(type_kernel): retract a deleted tracked slot's record |
| `8fe6ef805` | #1857 | test: pin values where the 8 gate comparisons cannot fail (#1853) |
| `42187f067` | #1858 | docs: ratify the G4 forks - read-serving, landed order, per-family claims (#1836) |
| `47209c257` | #1859 | test(type_kernel): pin retraction counters and the G1 del gap (#1856) |

### The #1860 G4 flip: NO-GO, closed unmerged (the wave's one T3 lane)

The flip (`Options.native_ast_mirror` + `Options.native_ast_mirror_read`
default `True`, expression-family serving) was fully implemented on
`feat/1860-g4-expression-serve-flip` (`7bece0d20`, PR #1862) and every
correctness battery was green in both gate states: `testcheck` 8144/69/7
each state, fine-grained family 1296/256 each state, reversed-order
isolation 12370/76/7, cold self-check 378 files byte-identical, the #1839
cross-run differential 5/5 legs, tree-pinned probe 4/4 served gate-on vs
0 on an unpatched-main control.

The quiet-gate wall-clock measurement (quiet window 03:05:37, load1 8.44):
3 interleaved cold self-check rounds; the two clean pairs both show
**~+55% overhead** (on 20.8/20.5 s vs off 13.2/13.2 s) against the 10%
gate; work-share total −22.1% (native slower). Diagnosis:
**capture-dominated** — serving is armed but nearly inert in batch mode
(the fine-grained deps walk does not run), while ~112k mirror refs are
dual-written every run. #1860 closed as NO-GO (precedent #1663, #1698),
PR #1862 closed unmerged, **branch preserved for the re-attempt**.

### Next work (in order)

1. **#1864** (self-assigned): reduce mirror capture overhead below the 10%
   flip gate — lazy/selective capture, cheaper per-write encoding, or
   batched capture. Acceptance: quiet-gate ≤10% on two clean interleaved
   pairs, batteries green both states. Then re-open the G4 flip from the
   preserved branch `feat/1860-g4-expression-serve-flip` (rebase first).
2. **G2.1/G2.2 flip issues** (drafts on disk at
   `/private/tmp/mypy-rs-1860-probe/issue-g21-stmt-flip-draft.md`,
   `issue-g22-def-flip-draft.md`): file them only after #1864 lands and
   the G4 re-flip passes its gate.
3. **#1861 H1 slice** starts after the G4 flip lands.
4. **#1745**: branch protection — blocked on the owner's ADMIN credential;
   the decision and the ready command are posted on the issue.

### Rules that bit this session

- **Pin `cwd` on every Bash pool/pytest call.** Post-compaction, tool calls
  lost the worktree `cwd` and the recorded command showed no `cd` prefix: the
  run would have executed from the main checkout and silently tested
  unpatched `main` (the #1789 wrong-tree hazard, conftest assertion and all).
- **Do not pre-queue a corpus behind another corpus.** Two `run 2` waiters
  were killed with their process group 25-45 s into the pool wait (no output,
  exit -1; cause outside the pool script - it only polls). Launch a corpus
  when the status shows free slots, or accept losing the run.
- **The engagement probe must pin the tree** (strip the editable finder and
  the cwd entry, then refuse unless `mypy.__file__` is under the tree): the
  first probe draft resolved mypy from the main checkout through the venv's
  editable finder and happily reported `mirror=False` for the branch.
- **A helper script is not `-m`**: `python /private/tmp/.../script.py` puts the
  script's own directory at `sys.path[0]`, so `import mypy` goes through the
  venv's editable finder (the main checkout) while `-m`-spawned build workers
  import the worktree — the parent/worker wire formats then disagree and the
  self-check dies with `Cannot connect to build worker(s)`. Every helper that
  imports mypy must pin the tree the way the probe does, or be run via `-m`
  from the worktree.
- **A loaded wall-clock number is never a gate verdict.** The 10% gate waited
  out 60–90 load1 from foreign nightlies before the 03:05 quiet window; the
  watcher interleave (alternating start order, pool `run 2` per leg, on-leg
  plain `-m mypy` defaults, off-leg tree-pinned `Options` patch) is the
  reusable recipe (`/private/tmp/mypy-rs-1860-probe/quiet_gate.sh`).
- **r3-style contamination is detectable in-band**: log load1 at leg end and
  discard pairs where the trend reverses (load rose 9.6 → 11.1 mid-r3); the
  verdict rests on clean pairs only.
- **`read_flip.modeN` in the audit is `activate`'s record, not the final
  mode**: with the env unset it always reads `mode0` because the production
  wiring patches the mode after `activate` ran. The effective mode is
  `read_mode` / `rust_node_mirror_read_mode()`.

## RESUME POINT — 2026-09-17, night (wave 11: 4 PRs merged; #1624's caching lever measured dead)

`main` = `476accf40` (#1819). Four PRs merged this wave; four lanes were still
in flight as this was written — **verify real state before trusting the table**.

### Merged this wave

| merge SHA | PR | what |
|---|---|---|
| `22e25aa0a` | #1816 | docs: wave-10 resume point in the handoff |
| `b878fbedc` | #1817 | refactor(type_kernel): collapse `ReadState`/`StmtReadState`/`VarKeyState` into one `FlipState` (#1815, closed) |
| `dcf02b804` | #1818 | perf: retire `rust_classify_protocol_test_callee` (#1739) |
| `476accf40` | #1819 | perf: retire `rust_check_unpacks_in_list` (#1739) |

Review of record: `ocr review` returned **0 findings** on #1818 (2 files) and on
#1819 (3 files, 3m23s). #1817 is a pure refactor and was reviewed by an
independent read of the diff, not by OCR: the three per-channel mode-error
strings preserved verbatim and distinct (`node read` / `stmt read` / `var key`),
the seven-counter tuples and the three public type aliases unchanged, `reset`
still preserving `mode`, thread-locals still per-channel, Python diff
docstring-only, `grep -c '#\[test\]'` 59 on both sides. #1819's engagement
numbers were re-verified from the lane's own artifacts rather than the PR body:
before total 125,545 with `rust_check_unpacks_in_list: 17584 calls, 0 defers,
0B`; after 107,961 with the seam in the zero-call list — the delta is exactly
that seam's traffic and no other seam moved.

### #1624's handle caching: mechanism real, lever dead — do not re-attempt

The lane completed with **no PR** (kill criterion honoured, tree byte-identical
to `main`). Cached class handles do beat uncached per call (−0.45 µs static,
−0.47 µs trivial-self, −1.62 µs generic), but the kernel-wide mechanism is
bounded by the per-call import cost it removes: **1,692,196 counted imports**
unarmed and **2,477,218** with `MYPY_ENABLE_NATIVE_SEMANAL=1` (`mypy.nodes`
1,017,979 / 1,593,914; `mypy.types` 616,262 / 717,167; `mypy.semanal` 0 /
99,927; `mypy.state` 47,831 / 47,831) at ~162 ns each ≈ **0.40 s of imports
(~0.5 s with the getattr term) against a ~72 s cold self-check, ~0.6 %**. It
cannot flip a measured keep, and the shim ratio moved ≤4 % with inconsistent
sign.

Also from that lane: the **1.10x keep of `rust_analyze_instance_member_dispatch`
is not reproducible** in any reader-visible shape (measured 0.40-0.62x loss on
static / trivial-self / generic). That is a method/shape gap, not a
falsification — the ledger never records which shape produced 1.10x. Do **not**
retire the seam on the different-shape number; the originating harness needs
recording or the keep re-deriving. Full reasoning on #1624, including the pyo3
finding that `PyModule::import`'s wrapper is not the cost (162 ns vs 156 ns for
`ffi::PyImport_ImportModule` + static `CStr`), so nobody should chase that swap.

### Measurement corrections this wave (cite these before ranking anything)

- **#1739's round-5 table is stale in its top rows**: five checked rows have
  zero production call sites — `rust_flatten_nested_unions` (278,082),
  `rust_copy_modified` (265,437), `rust_has_abstract_type` (246,897),
  `rust_refers_to_typeddict` (111,940), `rust_has_placeholder` (~79k). "Slice 1
  (predicate sweep)" and half of slice 2 as written target dead seams.
- **Registered ≠ live**: 826 registered, 153 called, **673 zero-call** on one
  probe run. Rank the live set, never the registered surface.
- **Liveness method caveat**: a `\b`-anchored `rg` **misses calls made through
  the `_rust_*` alias** (`_` is a word character, so `\brust_x\b` cannot match
  `_rust_x(`). Corrected census on `476accf40` — live: `rust_expand_type`
  (`expandtype.py:355`), `rust_analyze_member_access` (`checkmember.py:605`,
  retirement in flight), `rust_analyze_instance_member_dispatch`
  (`checkmember.py:756`, keep disputed), `rust_check_call_head`
  (`checkexpr.py:2947`), `rust_compute_arg_context_indices`
  (`checkexpr.py:3515`), `rust_classify_special_unbound` (`typeanal.py:993`).
  Everything else named in the round-5 table is uncalled.
- **#1820 filed**: `misc/audit_wire_traffic.py` prints a full report of zeros
  and exits 0 when the audited check fans out (`mypy_self_check.ini:31`
  `num_workers = 4`); there is no fan-out guard.
- **#1754 addendum**: the pinned census command never arms
  `MYPY_ENABLE_NATIVE_SEMANAL`, which nearly halves the counted imports, so
  census totals are gate-dependent.

### In flight as this was written (NOT merged — verify)

| slot | branch | scope |
|---|---|---|
| w3 | `perf/1739-retire-analyze-member-access` | retire `rust_analyze_member_access` (T3) |
| w4 | `feat/1787-stmt-serve-seed-wire` | #1787 §3 minimum extension: adoption-time seed + a wire slot for one type-valued consumer (T2) |
| w5 | `fix/1820-audit-fail-on-fanout` | the #1820 fan-out and hollow-zero guards |
| w1 | `recon/1787-next-ladder-slice` | read-only: pins the next Phase-G slice after the §3 extension as a new issue |

### Pool and host

The pool is now **five** slots (`w1`-`w5`), but the throttle remains the
weighted semaphore (3 units; build = 1, corpus = 2, never two corpora), not the
slot count. Host load ran 24-30 on 18 cores with three heavy lanes live, and
CI's parity jobs run on **this same host**: #1819's parity run took **546 s**
where #1817's took ~91 s. Budget before adding a fourth heavy lane.

### Rules that bit this session

- **Fast-forward the local checkout after every GitHub merge.** A stale local
  `main` made a liveness check read the pre-merge file and report
  `rust_classify_protocol_test_callee` as still called after its retirement had
  merged.
- Never `agent-wait until github.pr` (the fork's AI Code Review is skipped); use
  `agent-wait until github.ci <run-id>`. Background bash caps at 600 s.
- Gate every seam selection on a production-call-site liveness check.
- Do not let a lane commit its probe into the crate: `crates/type_kernel/src/lib.rs`
  declares every module, so a probe module ships. Probes belong in `misc/`,
  `scripts/`, or `/private/tmp`.

## RESUME POINT — 2026-09-17, evening (wave 10: #1811 Var-key landed; two lanes in flight)

`main` = `a4b86848c` (#1811, the Var binder-key handle translation — §2 option A
of #1787). This section is current state; the wave-9 tables below remain valid
history.

### Merged this session

| merge SHA | PR | what |
|---|---|---|
| `a4b86848c` | #1811 | feat(type_kernel): #1787 Var binder-key handle translation |

**#1787 stays OPEN** — its §3 store-shape work is not delivered; the lane
deliberately did not close it.

#### #1811 detail

Gate `MYPY_TK_VAR_KEY_FLIP` (0 off default / 1 serve / 2 serve+differential).
Files: `crates/type_kernel/src/node_mirror.rs`, `mypy/nodes_mirror.py`,
`mypy/literals.py`, `mypy/test/testtypes_native_var_key.py`,
`stubs/type_kernel_mirror.pyi`, `mypy/build.py`. Merged head `460ecd7fc`.

- CI green on `460ecd7fc`: pr-gate `35227955522` (lint-changed + pr-gate);
  parity `35227955614` (parity, parity-mirror, parity-symtable-flip,
  parity-typeops; parity-ast skipped). AI Code Review skipped (fork — never
  `agent-wait until github.pr`).
- Independent review (separate read-only agent, meta-pro): no blocking findings,
  seven advisories. Fixes 1-5 landed in `460ecd7fc`; advisory 6 tracked as
  #1815; advisory 7 recorded, no code change. Dispositions posted as PR comment
  (issuecomment-5715248049).
- Local evidence on `460ecd7fc`: `cargo test -p mypy-type-kernel` 2899 passed /
  0 failed / 8 ignored; `testtypes_native_var_key.py` 17 passed; under
  `MYPY_TK_VAR_KEY_CONTROL=share` 8 failed / 9 passed (identity test reddens as
  designed); fmt/ruff/black clean.
- Cleanup done: w1 released, local + remote `feat/1787-varkey-handle-scheme`
  deleted, local `main` fast-forwarded.

### In flight at handoff time (NOT merged)

Two lanes were dispatched in parallel, both based on `a4b86848c`, both on
`meta-speed` (the standing model order for #1787-family work). At this writing
neither had committed or opened a PR — **verify real state before trusting it**
(`gh pr list -R codenkirch/mypy-rs --state open`; `git -C <slot> log -1`).

| slot | branch | issue | agent id / task id |
|---|---|---|---|
| w2 | `refactor/1815-flipstate-collapse` | #1815 | agent-26 / agent-uwf0k3f2 |
| w3 | `perf/1739-retire-classify-protocol-test-callee` | #1739 | agent-27 / agent-q4f4wjwn |

- **#1815**: collapse `ReadState` / `StmtReadState` / `VarKeyState` (three
  identical `mode`+7-counter structs in `node_mirror.rs`) into one `FlipState`
  with per-seam thread-locals; keep the per-channel pyfunctions as thin
  delegates; preserve the three mode-error strings and the three counter type
  aliases. Pure refactor, T2.
- **#1739**: retire `rust_classify_protocol_test_callee` (measured 1.3-3.7x
  loss, 0/200 decided on the common shapes). Call `checkexpr.py:1770`, import
  `:265`, fallback `:348`; pins into `mypy/test/testtypes_native_retired.py`.
  T3 (both gate states).

Recovery: `Agent(resume="agent-26", ...)` / `Agent(resume="agent-27", ...)`, or
read their `agents/<id>/wire.jsonl` under the session dir.

### Next-perf-lane recon (agent-25, read-only, complete)

`#1739`'s round-5 table is ~consumed (9 of 16 seams already retired). Ready
slices, in order:
1. retire `rust_classify_protocol_test_callee` — **in flight** (w3, above).
2. retire `rust_check_unpacks_in_list` — `typeanal.py:3212` (5.5-10.2x loss,
   110,966 calls). T3.
3. retire `rust_analyze_member_access` — `checkmember.py:605` (6.2-9.1x loss,
   13,060 calls). T3.
   AGENTS caps a wave at 1-2 T3 lanes — pick one of 2/3 next, queue the other.
4. **#1624's live lever: handle caching** in `checker_functions.rs` — cache
   `nodes_class` results (fn `:42-46`, 42 sites) and hoist the 9
   `py.import("mypy.types")` lookups (`:1329 :1684 :3549 :5175 :6094 :6109
   :6202 :6571 :7363`). T2.
5. re-measure `rust_analyze_instance_member_dispatch` (`checkmember.py:756`)
   after 4 — marginal 1.10x keep. T2 measurement, evidence-critical.

Stale anchors flagged: `#1624`'s `checker_functions.rs:5117-5120` is now
`:5175`; wave-6 census drift (`get_declaration` `binder.py:735`,
`flatten_lvalues` `checker.py:5919`, `infer_condition_value`
`reachability.py:149`, `unmangle` `util.py:588`). `rust_get_declaration` is a
*semantic* collision with #1787's binder-key scheme — defer it.

### Host / environment

- Wall-clock leg of `scripts/measure_work_share.py` stays retired (no quiet
  window; bar is 1-min load < 5). All post-retirement evidence is load-invariant
  counters (`scripts/measure_native_share.py`, `misc/audit_wire_traffic.py`).
- A single-core oracle process from another portfolio ran days at ~98% CPU;
  flagged, not killed (owner decision).

### Pool state at handoff

w1 free; w2 claimed (`refactor/1815-flipstate-collapse`); w3 claimed
(`perf/1739-retire-classify-protocol-test-callee`). Release with
`sh scripts/worktree_pool.sh release <slot>` (refuses on a dirty tree; deletes
the local branch).

### Standing rules that bit this session

- Never `agent-wait until github.pr` (hangs on the fork's skipped AI Code
  Review check) — use `agent-wait until github.ci <run-id>`. Background bash
  tasks are capped at 600s by default; pass a longer `timeout` for long waits
  (the parity wait was killed at 600s mid-run and had to be re-checked).
- Pre-flight before any suite run:
  `.venv/bin/python scripts/assert_worktree_import.py <tree>` — exit 0 + the
  tree path is the only acceptable result (a worktree `.venv` symlinks the main
  venv, so `import mypy` can resolve to the wrong tree).
- Heavy ops via `/private/tmp/mypy-rs-sem.sh run 1 <build>` / `run 2 <corpus>`;
  never bare, never `-n auto`.
- Kernel build: `cargo rustc -p mypy-type-kernel --features extension-module
  --lib --crate-type cdylib --release -- -C link-arg=-undefined
  -C link-arg=dynamic_lookup`, copy the `.so` to a private scratch dir,
  `codesign -f -s -`; NEVER `maturin develop`.
- Merge style is `--squash --admin`; branch from `main`; never commit to `main`.

## RESUME POINT — 2026-09-17, afternoon (wave 9 close: 12 PRs landed, #1787 PR A+B in, Var-key lane in flight)

`main` = `0aa7844a6` (#1804, #1787 PR B — the wave's last merge). Twelve PRs
landed this wave, eleven issues closed (#1795 closed by merge).

### Wave-9 lane outcomes

| PR | issue | outcome |
|---|---|---|
| #1788 | #1761 | perf: retired the `rust_unknown_unpack` wire seam |
| #1790 | #1787 PR A | store extension + Var handle substrate |
| #1792 | #1763 | chore: retired the write-only visitor resolver + dead test registration |
| #1793 | #1786 | test: derive binding-hygiene names from the suites |
| #1797 | #1791 | docs: regenerated the stale G1.2 node-shadow audit tables |
| #1798 | #1754 | fix(audit): report zero-call registered seams and semanal gate state |
| #1799 | #1789 | fix(harness): make cross-tree suite runs fail loudly |
| #1801 | #1796 | ci: gate the G1.2 audit tables on verdict-only drift |
| #1803 | #1802 | ci: drop the audit gate's runtime ripgrep install |
| #1805 | #1794 | test(mirror): inject `_rust_remove_dups` with plain assignment |
| #1806 | #1773 | feat: seed the symtable shadow from the fixed-format cache reader |
| #1804 | #1787 PR B, closes #1795 | feat: statement-family serving read flip (`MYPY_TK_STMT_READ_FLIP`) |

### #1787 progression (the wave's spine)

PR A (#1790) extended the meta store and added the Var handle substrate; PR B
(#1804) delivered the statement-family serving read flip over the single
registered statement field native code reads live, `Block.is_unreachable`
(gate `MYPY_TK_STMT_READ_FLIP`, modes 0 off / 1 serve / 2 serve+differential),
plus `rust_node_mirror_object_of`. The default stays mode 0: no `Options`
default moved, so both PRs are T2. **#1787 stays open for the Var-key flip**
(§2 option A, gate `MYPY_TK_VAR_KEY_FLIP`) — that lane is in flight (below).

### #1795 resolved: validate inside `object_of`

`capture_pin` keys pins by identity handle and `object_of` resolves a handle
back to the pinned object; before the fix `object_of` answered from
`TARGET_PINS` alone while `handle_of` delegated to the identity layer, so after
an identity-only reset a stale pin could still resolve and the two read-backs
disagreed. Verdict: **`object_of` now validates** (`identity::handle_of(pin) ==
handle`), not a documented independence. Rationale recorded in the ledger:
divergence is unreachable in the production reset order, the validation is one
cold thread-local lookup (not the PyO3 round-trip class #1785 rejected for
compare-before-answer), and the fail direction is the defer (`None`), never a
wrong object. The Var-key lane inherits the invariant `object_of(h) = Some(obj)
=> handle_of(obj) = Some(h)`.

### Audit gate and harness guard are now load-bearing

- **Audit gate** (#1798, #1801, #1803): the census now prints `N registered, M
  called, K zero-call` plus a `registered seams with ZERO calls` section, and
  labels the ALL-seams total a *lower bound* (the pinned command leaves
  `MYPY_ENABLE_NATIVE_SEMANAL` unset, so the semanal families are dark by
  design). #1801 gates the tables on **verdict-only** drift, so line-anchor
  shifts no longer trip it, and #1803 dropped the runtime ripgrep install — the
  gate is anchor-insensitive and no longer depends on `rg`.
- **Harness guard** (#1799): `mypy/test/conftest.py` asserts, before any suite
  code runs and in **every** xdist worker, that the imported `mypy` lives in
  the tree under test; `scripts/assert_worktree_import.py` is the documented
  pre-flight (it strips the editable finder and the cwd `sys.path[0]` entry,
  both of which defeat the old `PYTHONPATH=... python -c "import mypy"`
  incantation). Cross-tree hollow green now fails loudly (rc=4) instead of
  reporting engagement against a foreign tree.

### #1754 correction (mechanism refuted)

The originally-recorded "0-defer filter" mechanism was **wrong**: the script was
never a filter. The real blind spot is **registered seams that are never
called** — seams shadowed by another answering seam (`rust_is_duplicate_mapping`
sat dark behind `rust_check_argument_count` until #1747 retired it, then became
the #1 seam) leave no row for a call-counter census to rank, and the semanal
family (~110 seams) is dark in every census run. Fixed by the zero-call section
above; corrigenda posted on #1739 and #1754.

### In flight (dispatch 2026-09-17 afternoon)

- **`fix/1807-symtable-seed-fail-closed`** (pool w2, meta-speed): the two
  defensive-path consistency gaps OCR left on PR #1806 (post-`prepare_seed`
  re-check tests only `by_owner`; `len().unwrap_or(0)` maps a failed read to
  "empty" against the fail-closed doc). Advisory severity, no observable
  defect today.
- **`feat/1787-varkey-handle-scheme`** (pool w1, meta-speed): #1787 §2 option A
  — mint a store handle for the `Var` at `_capture_ref`, emit `("Var", handle)`
  in `literal_hash` under `MYPY_TK_VAR_KEY_FLIP` (mode 0 keeps the live object
  key), resolve handle→Var through `rust_node_mirror_object_of`, with identity /
  injectivity / mint-order / negative-control tests and a mode-2
  `key_translation.deferred == 0` differential.

### Advisory dispositions (PR #1804 review of record)

Two `low` / advisory findings, verified real, **not applied** (an advisory-only
commit would force a full CI cycle for no correctness gain):
`mypy/test/testtypes_native_mirror.py:4513` should read `self._k` not
`self._m._kernel_mod`; `mypy/test/testtypes_native_stmt_read.py:71` has a dead
`self.fx = TypeFixture()` and an unused `TypeFixture` import. Both ride along
with the next lane touching those files.

### Next queue

1. Land the two in-flight lanes above (both must stop at CI-green for the
   orchestrator to review and merge).
2. **#1785** — served `RefView` answers can drift from live slots for
   out-of-contract writes (`depswalk.rs` `RefView::kind_is_none` /
   `fullname_opt`). Needs the narrow-vs-document contract decision before any
   gate that serves those fields is promoted.
3. **#1800** — the shared venv holds a real but stale `ast_serialize` 0.6.0
   wheel; fix touches the shared venv, so run it only when no lane is mid-run.
4. **#1770** (H1 Rust traversal driver) and **#1624** (handle caching at
   `checker_functions.rs:42-47`) remain the larger open direction; **#1745**
   (branch protection on `main` — every gate is advisory-only) is an owner
   decision.

## RESUME POINT — 2026-09-17, morning (wave 8 close: lanes landed, counters published, timing leg retired)

All five wave-8 lanes are resolved. `main` = `f191bb71a`+ (G1.1 `#1777`, suite
split `#1775`, flip-gate arming `#1774`).

### Wave-8 lane outcomes

| lane | outcome |
|---|---|
| N1 | **landed** `#1777` — G1.1 mode-gated serving read channel. Its `ocr` pass filed four leftovers: `#1778` (mirror test never restores `mypy.types` bindings), `#1779` (serving gate self-contradiction: `set_read_flip`/`read_flip` disagree, half-activation, unvalidated mode), `#1780` (mode-2 differential counts an unreadable fullname as a mismatch, unlike the capture), `#1781` (chore lows). |
| N2 | **landed** `#1774` (`#1765`). The deeper finding is **`#1773`**: the fixed-format cache reader never seeds the symtable shadow, so the read flip cannot serve cache-loaded namespaces. |
| T1 | **landed** `#1775` (`#1757`). |
| G0V | **done** — G0 partial, G4 criterion 2 needs retargeting (below, `#1767`; criteria pinned `#1768`). |
| W1 | **counters published** on `#1723`/`#1624`: 311 seams with calls, 3,138,600 calls, 99.8% native, 5,271 fallbacks; the retirement wave `809b53b90 -> e2d648080` removed 2,621,274 crossings. |

### The timing leg is retired (owner decision)

The quiet-host wall-clock A/B (`#1723` item 3, `#1624` wall clock) is **retired
as unreachable on this host; do not re-attempt as a tracked work item**. Full
evidence trail on `#1723` (closed). Summary: the admission bar (1-min load
< 5.00) is met only in minute-scale dips — 3.92 was observed at 2026-09-17
08:09, gone by 08:12 (Vidiom CI on the shared runner, Spotlight, Norton) —
while a full 6-run interleaved set needs ~15-30 quiet minutes. Two integrity
facts from the final attempt: the main-arm scratch kernel had gone missing and
`w1-quiet-ab.sh`'s guard checks `mypy.__file__` but not `type_kernel`, so the
provisioned script would have silently measured a **kernel-less main arm**; any
future attempt on a genuinely quiet machine must rebuild both kernels and add a
`type_kernel` import assertion to the guard. The A/B worktrees are removed; the
load-invariant counters remain the operative evidence standard. `#1624` stays
open for **handle caching** (`checker_functions.rs:42-47`, `:5117-5120`), the
one mechanism fix that can flip a measured keep.

### In flight

- **`fix/g11-review-leftovers`** (pool slot w1, dispatched 2026-09-17 morning):
  one lane, one PR fixing `#1778`+`#1779`+`#1780`+`#1781`. Rationale for its
  priority over the queue's statement-family item: that lane builds directly on
  the G1.1 channel, and `#1779`/`#1780` sit exactly in the gate state handling
  and the differential it will consume — do not build the statement family on a
  self-contradicting gate or a false-mismatch differential.

### Queue after it lands

1. **Statement family + `Var` handles** (H1's precondition), then H1 as ONE
   T3 slice (`#1770` holds the corrections a designer must read first).
2. Owner decisions pending: ADR-0006 disposition (delete prototype or successor;
   `testtypeview.py` runs in no CI job), G4 write-flip-vs-read-serving fork,
   `#1745` (branch protection).
3. `#1773` (cache-reader shadow seeding), `#1761` (`rust_unknown_unpack` 5.81x),
   `#1763`, `#1754`.

## RESUME POINT — 2026-09-17, early (wave 8: the objective is the full migration, parallelised)

The active objective changed to *"do the full migration to rust parallelize as
much work as possible"*, so this wave attacks the **ownership ladder** — F, G, H,
J — rather than more seam retirement. Reconnaissance settled three things with
evidence; they are the wave's real output so far.

### Where `main` stands

`main` = `57d99198f`+ (wave-7 close #1766, G4 criteria #1768, the F NO-GO record
#1771). Session total on 2026-09-16: 19 PRs merged, ~19 net-loss seams retired,
G3.2 landed, plus the G4 criteria and the F record. Tree clean, no lane
worktrees, no stashes.

### Settled: Phase F is closed by measurement, not contract

ADR-0006 (`58d32ab24`, PR #1694) **is** the experiment for #1671 and it measured
**NO-GO by ~4.7x on arithmetic**: the `Instance` funnel is 2.119% of the cold
self-check's total work, so a replacement view removing all of it at zero cost
beats the native default by 2.12% against the 10% bar. All four ADR-0004
contract surfaces (plugins, `isinstance`/identity, `__slots__`, astmerge)
measured **satisfiable**, so the earlier "declined on contract" framing is stale.
The 2026-09-16 retirements made the gap **worse**: funnel `serialize_calls`
2,839,692 -> 1,287,230 (-55%) while total work fell at most ~6.7%, ceiling now
~0.9-1.1%, gap ~9-11x. Recorded in the plan and the close-out (#1771); assessment
in #1769. Do not re-open without a new mechanism.

### Settled: G4's criteria are pinned; H1 is not next

- G4 criteria, each with a receipt: `docs/plans/2026-09-16-g4-graduation-criteria.md`
  (#1767/#1768). Forks ratified 2026-09-17 (#1836): read-serving suffices for
  G4 (write flip lands beside H), order expression -> statement -> def,
  claims are per family.
- **H1 now: no** (#1770). The binder is H1's only mass and `mypy/literals.py:204`
  keys narrowing on the live `Var` object (`extract_var_from_literal_hash` reads
  `key[1]` back as a `Var`), so Rust cannot own `Frame.types` without a Var handle
  scheme — which is what the node read channel is building. Sequence: **node
  serving channel -> G-family read flips (Var handles) -> H1 as ONE slice
  (driver + binder join)**. H1's perf case is also weak: statement dispatch ~1.8%
  gross and expression dispatch ~5% at a 3x undercount, both under F's 10% bar —
  it is a ladder step, not a speed win.
- H1's differential must be **cross-run, not in-run** (the checker is not
  idempotent): byte-identical `exportjson` AST+symbol-table dump is the cheap
  detector, plus the `_type_maps` invariant and both deferral budgets. **There is
  no per-call fallback**, so a divergence is a wrong answer rather than a defer.
  Tier T3, with a new `parity-checker-driver` job.

### In flight (wave 8)

| lane | scope |
|---|---|
| N1 | node serving read channel (G1.1) — the critical path |
| N2 | #1765: the flip gate's cache-data step cannot engage the flip |
| T1 | #1757: split the engagement suites per area |
| W1 | end-to-end counters + the wall-clock leg never yet run |
| G0V | **done** — G0 is partial; see the verdict below |

F1 and H1 completed; their deliverables are issues #1769 and #1770.

### Settled: G0 is *partial*, and G4's criterion 2 needs retargeting (G0V, #1767)

G0V audited the writer against the fields Python-only paths mutate and the answer
is **no**, on three independent layers:

1. **The store is a per-write delta, not a snapshot.** `capture_meta`
   (`node_mirror.rs:392-406`) stores one field per write and lazy adoption
   (`nodes_mirror.py:607-619`) skips baseline-only writes, so no mirror registers
   `line/column/end_line/end_column` at all while `Context.set_line`
   (`nodes.py:180-191`) has **120** call sites outside tests. Structural statement
   fields (`expr`, `body`, `rvalue`, `target`, `patterns`, `arguments`, ...) are
   absent.
2. **No store consumer exists.** `rg rust_node_mirror_meta` outside tests finds
   only `_reset_meta` (`nodes_mirror.py:655`): G0.5 moved the cache-payload
   *format* to Rust, not its *source* — the cache writer still reads the live
   Python tree (`sym_node.rs:515-537`, 65 `getattr`).
3. **Def-family cache-payload fields are unregistered**: `FuncDef._name`,
   `arg_names`, `arg_kinds`, `original_first_arg`; `Var._name`; `ClassDef.name`.
   Plus post-construction mutations on unregistered fields
   (`IfStmt.else_body` `reachability.py:101`, `FuncDef.type_args`
   `semanal.py:10076`, `Block.body` `astmerge.py:216`, `ClassDef.decorators`
   `treetransform.py:278`, ...) and two blind channels
   (`replace_object_state`'s dynamic descriptor copy with no `__delattr__` hook,
   and `Decorator.decorators.remove` at `plugins/attrs.py:893` with no `touch`).

**Detection limits, stated by the lane:** attribution is by name + annotation and
is not type-checked, so **2,589 of 8,687 sites (30%) are unattributed** and the
missing-field list is a **lower bound**.

**Consequence for G4:** criterion 2 is **blocked for both sub-families** until the
issue says whether the parse-wire blob stays parser-produced or the store owns
parse-time fields; criterion 3 is adjacent, since aststrip's reset and astmerge's
dynamic copy act on unregistered fields — a store-sourced def family would be
stale exactly where G3.2's aststrip hazard lived. The lane's proposed rewrite:
make criterion 2 target the **cache-payload field set derived from the Python
writers**, not the unbounded "fields Python-only paths mutate". G1 is clean by
comparison: no post-construction mutation field is missing from its registration.

### Queue

1. **After N1**: the statement family and `Var` handles (H1's precondition), then
   H1 as one T3 slice.
2. **Owner decisions pending**: ADR-0006's disposition (delete the prototype or
   file a successor — still `Draft`, and `mypy/test/testtypeview.py`'s 33 tests run
   in **no** CI job); G4's write-flip-versus-read-serving fork; #1745 (no branch
   protection, so every gate is advisory).
3. #1761 (`rust_unknown_unpack`, 5.81x), #1763 (retirement leftovers), #1754
   (census under-reporting).

## RESUME POINT — 2026-09-16, late (wave 7 close: 16 PRs today, G3.2 read-flip coverage landed)

Wave 7 ran five lanes against the wave-6 inventory: four retirement lanes
(disjoint modules, per-area pin files) plus the G3.2 ownership step. All landed.
`main` = `33c7a5b79`. No open PRs, no lane worktrees, no stashes.

### Landed in wave 7

| PR | what |
|---|---|
| `33c7a5b79` (#1764) | **G3.2**: write-time seed for pre-populated symtable owners (#1755) — the read flip now covers owners whose dict was already non-empty at first recorded write |
| `a3919bbec` (#1762) | typeanal: `validate_instance` (3.48x-4.76x), `classify_type_with_info` (1.92x-2.77x) |
| `28e59f392` (#1759) | checker: `is_definition` (10.94x), `classify_check_lvalue` (5.60x), `classify_check_assignment` (7.62x), `is_empty_generator_function` (2.95x) |
| `cbab29443` (#1758) | checkmember: `is_instance_var` (3.73x-5.30x), `classify_analyze_var` (1.72x-4.32x) |
| `2d80926ca` (#1760) | types/nodes: `flatten_nested_tuples` (24.6x-**59.3x**), `func_item_is_dynamic` (2.7x-7.0x) |

Ten previously-unmeasured seams retired, every one with engagement proven per
shape, a post-retirement zero-crossing probe, and no edit to the shared
engagement suites (the per-area pin convention from #1746/#1751 held). Today's
total: **16 PRs merged**.

### G3.2 in detail (the ownership step, not a perf step)

Mechanism: at the first recorded write of an owner whose dict is already
non-empty, one `owner.items()` read outside the store borrow, ordinals minted in
live order, entries pinned and indexed like write-log entries; **fail-closed** —
any live value without the capture's flag slots leaves the owner `Inherited`.
Default **not** flipped; no reset/pin/`CACHE_VERSION`/`OPTIONS_AFFECTING_CACHE`
change.

What its differential now catches that it did not: `astdiff.py:271/:277`'s
mode-2 `owned != baseline` / `list(owned) != list(baseline)` comparisons
**previously never ran for these tables** (the flip returned `None` before
reaching them). Plus non-vacuity counters (`defer_inherited == 0`,
`tables_mirrored`/`entries_mirrored` advancing, `seed_rejects == 0`), the `@`-key
pinned explicitly (`func_scoped_name("D", 5)` survivor, order `["D@5", "a"]`),
and provenance counters separating write-log (`put_entries`) from seeded entries
— the attempt receipt that `seeded_owners == 0` cannot express.

Negative control, falsified by mutation: on a kernel built with permissive
`read_flags`, `test_unreadable_namespace_does_not_seed` fails at
`assert 1 == 0` while the positive test still passes. On the real fine-grained
slice (36 cases): `deferred 140 -> 124`, `defer_inherited 16 -> 0`,
`tables_mirrored 213 -> 248`, `seeded_owners 0 -> 11`, `seeded_entries 0 -> 62`.

**The number #1755 asked for:** `defer_no_handle` (124) is now the *whole*
remaining defer volume, and it is untouched by this change — that is the figure
that gates whether the optional `NoHandle` read-time seed is worth building.

**Honest caveat, unfalsified and stated in the PR:** the change *widens* the
documented mode-1 assumption — any owner with one captured write now serves, so
a length-preserving uncaptured mutation after the seed would be served where it
used to defer. No production path emitting one could be constructed (the
C-level deletes are paired with captured re-adds -> `LenLong`), so it is a
widened accepted assumption, not a demonstrated wrong serve. Also: the counters
count mint events, not which absorbed key was uncaptured; and only mode 2's
verify catches post-seed order/value drift.

### The lesson that repeated three times today

CI was green while a new assertion **could not fail** — in three separate lanes:
a missing-vs-superfluous `type: ignore` pair, a tautology (`api.errors == []`
that could never grow), and a negative control that never triggered the code path
it named (`table_len == 1` making the `table_len > 1` gate false). Each was
caught by the operative local `ocr review`, never by CI. Treat the review pass as
the gate and CI as necessary but insufficient; track the third as the
`parity-symtable-flip` vacancy in **#1765**.

### Other findings filed this wave

- **#1761** — `rust_unknown_unpack` (measured 5.81x loss) became load-bearing when
  #1762's retirement shifted calls onto it; net ratio recomputed 4.76x -> 4.3x.
- **#1763** — two retirement leftovers: `_native_visitor_resolver` is now
  write-only (its docstring still describes the retired flatten seam), and a dead
  `info.names` registration in a pin.
- **#1765** — `parity-symtable-flip` runs `testcachedata.py`, which builds
  `Options()` directly and therefore cannot engage the flip at all (measured: no
  dump written, 0 mirrored), so the one gate step covering cache-loaded tables
  cannot detect a regression there.
- **#1624** comment — the per-call import mechanism behind the live-object seam
  losses: `checker_functions.rs:42-47` calls `nodes_class` per class per seam
  call and `:5117-5120` does `py.import("mypy.types")` on every call. Caching
  those handles is the first lever here that *fixes* rather than deletes, and
  `rust_analyze_instance_member_dispatch` (kept at 1.10x) should be re-measured
  after it.

### Queue for the next wave

1. **`#1624`'s handle caching** — the one mechanism fix that could flip a
   measured *keep* into a win, and it applies to every live-object seam.
2. **The rest of the 86** (M1's table on #1739): `check_call_head` (180k, needs
   the corpus differential), `get_declaration` (binder, 91k),
   `special_function_elide_names` (sharedparse, 31k), then the 1k-10k band
   (76 seams, 319k calls). R-D flagged two sub-10k lookalikes of the seam it
   retired (`nodes.py:1028`, `:1487`).
3. **#1757** — split the engagement suites per area; that is what makes a
   retirement lane's test footprint entirely its own and removes the last
   serialization point.
4. **The wall-clock leg** (#1723 3-pair, #1624 wall clock) — still unmeasured.
   Host load fell to ~15 late in the session (its lowest), still above the
   documented `< 5` bar, so nothing was published from it. Everything shipped
   today is counters and per-call ns, not elapsed time.
5. **#1745** — `main` still has no branch protection or ruleset; every gate
   remains advisory. Owner decision.

## RESUME POINT — 2026-09-16, night (wave 6 close: retirements, lint gate, and a 73.5%-unmeasured inventory)

Wave 6 finished #1739's slices 1-3, added a per-file CI lint gate, cleared the
repo-wide lint debt, and — most consequentially — produced a full seam inventory
that **falsified the previous "the perf axis is nearly exhausted" framing**. Five
lane-6.5 lanes are in flight as this is written; see the queue.

### Where `main` stands

`main` = `4c1ed279c` (`is_duplicate_mapping` retired — the highest-call seam on
`main`, 342,101 calls, PR #1753), on top of `a52f8a166` (repo-wide ruff/black
debt cleared, PR #1752, closes #1742),
on top of `e52dde80c` (retired-seam pins moved into
`testtypes_native_retired.py`, PR #1751, closes #1746), `bc5482bf5` (this
file's previous resume point, PR #1750), `fc7e39f51` (seam retirement log in
the ledger, PR #1749, closes #1748), `ce615df84` (argmap mapping seams +
`check_argument_count` retired, PR #1747), `cb9430c5a` (`rust_fill_typevars`
retired, PR #1744), `b3fc0e526` (`lint-changed` CI gate, PR #1743),
`8c1b0c6c3` (`rust_copy_modified` + `rust_flatten_nested_unions` retired, PR
#1741), `10762e5a5` (six predicate seams retired, PR #1740), then the wave-5
head `5b012830e`.

### The inventory changes the plan (issue #1739, lane M1's comment)

804 registered seams, 4,204,642 calls on a cold self-check. Disposition:

| disposition | seams | calls | share |
|---|---|---|---|
| measured keep | 5 | 526,129 | 12.5% |
| retire queued | 3 | 235,735 | 5.6% |
| **unmeasured, >=10k calls** | **86** | **3,089,332** | **73.5%** |
| unmeasured, 1k-10k | 76 | 319,225 | 7.6% |
| already retired (0 calls) | 18 | 0 | — |

Class split at >=10k: O(1) read 42 seams / 1,572,105 calls; O(n) scan 12 /
345,886; recursive visitor 31 / 1,140,678; unclassified 1 / 30,663. The
retirement-favourable O(1)/O(n) subset is 54 seams / 1,917,991 calls, bounded at
**<=4.17s** of exposure (whole set <=6.75s) — both are bounds from the audit's
own constants, not wall clocks.

Two measurement defects that hid this are filed as **#1754**: #1739's table
header filters "0 defers", which **excluded the single highest-call seam on
`main`** (`rust_is_duplicate_mapping`, 342,101 calls, 24% defers — measured and
retired today in #1753), and `MYPY_ENABLE_NATIVE_SEMANAL` gates a family the
pinned census command never arms (114 call sites + 67 `rust_visit_*` seams), so
4.2M is a lower bound.

### Landed this wave (the retirements, with their measured ratios)

Slice 1 (#1740, six predicates 2.4x-35.7x), slice 2 (#1741, `copy_modified`
15.8x-71.8x + `flatten_nested_unions` 14.4x/25.7x), slice 3 (#1747, the argmap
mapping family + `check_argument_count` 1.71x-6.40x), #1744
(`rust_fill_typevars` 2.57x-6.82x warm, 12.5x-22x cold), #1753
(`is_duplicate_mapping` 2.9x-8.6x, 342k calls), plus #1743's gate, #1749's
retirement log (#1748) and #1752's lint clearance (#1742). Corpus effect so far:
**~1.31M fewer FFI crossings and 19.6MB less wire** per cold self-check from
slices 1-2 alone, plus the later retirements.

### Decision rule, extended

The wire interface **loses** when the Python body is an O(1)/O(n) rebuild, list
scan or scalar read, and **wins** when the body is a recursive visitor —
`rust_expand_type` and `rust_expand_actual_type` (0.43x-0.61x) are the standing
keeps. Measured keeps not to re-open: `rust_analyze_instance_member_dispatch`
(1.10x), `rust_classify_special_unbound` (0.78x on its common shape),
`rust_compute_arg_context_indices` (1.38x but 98ns/call).

**Operational corollary discovered today (this is what unlocks 3-wide lane
fan-out):** a retirement usually needs **no edit to the area engagement suite**,
because those suites assert gate-off == gate-on parity plus direct-seam
engagement, and both survive a retirement. So a retirement lane puts its pin in
its own `testtypes_native_retired_<area>.py` and touches no shared file. Two
lanes proving it today: #1753 and the R-A/R-B/R-C/R-D set.

### G3 ownership track (issue #1755)

The wave-5 framing of the read flip's residual was **wrong** and the correction
shrinks it: a cache-loaded table (`SymbolTable.deserialize`) *is* captured from
empty and served, and the aststrip-survivor defer is minted in Rust `put()` on
`first_write && table_len > 1`, so it was never a build-boundary problem. The
chosen mechanism is a **write-time seed** (T2, default-off, fallback intact):
read `owner.items()` once at the first recorded write of a non-empty owner, mint
ordinals in live order. Mandatory differential additions, because the existing
values/order assertions **pass vacuously today**: non-vacuity
(`defer_inherited == 0`, `tables_mirrored` advanced), the `@`-key value pinned,
and **provenance counters** separating write-log from seeded entries — the only
falsifier for a bulk `dict.update` flip, which no value assertion can catch.

### In flight as this was written (five lanes, disjoint modules)

| lane | scope |
|---|---|
| G3.2 | `#1755` write-time seed (`mypy/build.py`, `crates/type_kernel/src/symtable_mirror.rs`, a `sessionfinish` counter report) |
| R-A | `mypy/checker.py`: `is_definition` 73k, `classify_check_lvalue` 73k, `classify_check_assignment` 67k, `is_empty_generator_function` 32k |
| R-B | `mypy/typeanal.py`: `validate_instance` 86k, `classify_type_with_info` 82k |
| R-C | `mypy/checkmember.py`: `classify_analyze_var` 100k, `is_instance_var` 44k |
| R-D | `mypy/types.py` + `mypy/nodes.py`: `flatten_nested_tuples` 95k, `func_item_is_dynamic` 101k |

### Queue for the next wave

1. **The rest of the 86** (M1's comment has the full ranked table): `check_call_head`
   (180k, needs the corpus differential because its fallback crosses other
   seams), `get_declaration` (`mypy/binder.py`, 91k), `special_function_elide_names`
   (`mypy/sharedparse.py`, 31k), then the 1k-10k band (76 seams, 319k calls).
   Retire only measured losers; the visitor-class 31 seams are the likely keeps.
2. **The quiet-host leg** — #1723's 3-pair protocol and #1624's wall clock, the
   only check that these retirements moved the end-to-end number. A bounded
   background watcher exits when 1-minute load drops below 5; load ran 50-103
   all evening, so it has not fired.
3. **#1745** — `main` has no branch protection or ruleset, so every gate is
   advisory-only; needs an owner decision on the context set (do not paste
   QWEN.md's snippet: it names a check this repo does not have, and
   `required_approving_review_count: 1` would make every lane PR unmergeable).
4. **#1754** — fix the census so its table cannot hide deferring seams again, and
   state which gates the run arms.

### Method traps added today (all cost a cycle or nearly did)

- A scratch `.so` is **stale** if `main` moved past crates commits: it still
  imports and still passes parity, so only a marginal ratio reveals it. Compare
  its mtime against `git log --since=<build time> -- crates/`; verdicts >=2x are
  version-immune, <=1.5x must be re-measured.
- `agent-wait` can print a **stale run's failure**; a CONFLICTING PR runs no CI,
  so a wait returns "0 checks passed" and reads like a pass. Bind any tally to
  `gh pr view N --json headRefOid`.
- Ruff B009/B010's autofix produces the form the self-check rejects
  (`implicit_reexport=False`); keep dynamic access with a per-site `noqa`.
- A measurement harness whose gate is OFF prints a plausible 1.00x; assert the
  FFI ticket was reached (`calls == 200`) in the harness itself.
- A retirement comment longer than 3 consecutive lines is rejected by the
  pre-commit hook.

## RESUME POINT — 2026-09-15, night (wave 5: tiered feedback, ledger archived out of AGENTS.md)

Eight lanes ran wave 5 under a new feedback protocol: gates are chosen by blast
radius (tiers T1-T4), the full local corpus is a wave-level step rather than a
per-lane step, and heavy ops go through a weighted pool instead of a flat
mutex. The seam ledger itself moved out of `AGENTS.md` into
`docs/plans/type-kernel-seam-ledger.md` in the same wave.

### Where `main` stands

`main` = `5b012830e` (wave-5 ledger entries and the final-head T4 battery, PR
`#1707`), on top of `a5bb83244` (G1.2 node-shadow audit and first
expression-node read flip, PR `#1695`), `58d32ab24` (F reopening one-family
view, PR `#1694`), `cd54ca293` (semanal live-object seams, PR `#1699`),
`0046062a1`
(residual scalar-seam retirement, PR `#1685`), `24ddff920` (pool
checkout and venv fix, PR `#1705`), `9b0f426ca` (`check_callable_call` tail
seam, PR `#1700`), `d6647b8bc` (this read-flip correction, PR `#1704`),
`ac68f0eaf` (the G3.1 read flip, PR `#1687`), `e28cbca16` (measurement-code
rule, PR `#1703`), `c73260211` (this resume point, PR `#1697`), `ba762f12a`
(tier protocol, PR `#1682`), `a7329c5d4` (H1d live-object decision heads, PR
`#1690`), `08ef56f57` (POSIX glob for the `parity-ast` gate, PR `#1691`),
`3d5ace300` (seam-split planner, PR `#1692`), `e531197d3` (zero-call correction,
PR `#1686`), `a0f6150a7` (evidence convention, PR `#1684`), `e9017bf46`
(worktree pool, PR `#1683`), `3329850ac` (`parity-ast` path gate, PR `#1681`),
`271175175` (the ledger archive, PR `#1676`) and `618c2b196` (#1669). Wave 5
branched from `506aa7e4c`.

Landed this wave, in merge order:

- `618c2b196` **retire residual scalar-only wire seams (#1668, PR #1669)** —
  five gates deleted (`rust_descriptor_has_get_set`, the singleton
  identity/equality pair, `rust_is_recursive_pair`,
  `rust_analyze_none_member_access`); 43,909 calls / 953,594 bytes -> 0 as
  reported by that lane. Tier T3.
- `271175175` **archive the seam ledger out of `AGENTS.md` (PR #1676)** —
  `AGENTS.md` 4,937 -> 232 lines, lossless: of 4,708 nonblank removed lines,
  4,628 byte-identical in the archive, 80 in `docs/native-build-reference.md`,
  0 unaccounted. Tier T1.
- `3329850ac` **gate the AST parity job on AST-path changes (#1678, PR
  #1681)** — CI tier; the redundancy it records (parity and parity-typeops are
  the same corpus; parity runs testcheck twice inside itself) was deliberately
  not fixed.
- `e9017bf46` **reusable worktree pool (#1680, PR #1683)** — compiled-unit
  counts, not wall clock, are the load-invariant evidence.
- `a0f6150a7` **wave evidence-artifact convention plus wave-5 measurements
  (#1679, PR #1684)** — docs tier.
- `e531197d3` **correct the zero-call seam claim (#1679, PR #1686)** — the
  falsified claim is left visible with its lesson. Docs tier.
- `3d5ace300` **verified planner for the seam-registration split (#1677, PR
  #1692)** — 961 `wrap_pyfunction!` sites across 120 defining modules. Tier T1.
- `08ef56f57` **POSIX glob for the `parity-ast` path gate (#1681, PR #1691)** —
  the previous regex had been verified with BSD `grep` while CI runs GNU
  `grep`; the skip is now verified on a real kernel PR (#1690). CI tier.
- `a7329c5d4` **H1d cluster: four live-object decision heads (#1672, PR
  #1690)** — `rust_should_report_unreachable_issues`, `rust_flatten_lvalues`,
  `rust_refers_to_different_scope`, `rust_literal_int_expr`, each keeping the
  pure-Python body as the fallback; 72 suite tests with gate-off/on
  differentials, `wire_delta=0` on all four, and the corpus gate delegated to
  CI's `parity` job. Tier T2. It landed **with `parity-ast: skipped`** under the
  new gate, `changes: success`, and its rollup went green 5 pass / 2 skipped
  (`parity` 12m5s, `parity-mirror` 6m27s, `parity-typeops` 6m0s, `pr-gate`
  4m11s, `changes` 12s; the skips are `parity-ast` and `ocr-review`), which is
  the AST-gate saving observed on a real kernel PR.
- `ba762f12a` **the tier protocol, the wave-5 ledger section and this handoff
  (PR #1682)** — tier table plus the weighted pool and the source-tree pre-flight
  in `AGENTS.md`, seven wave-5 entries in the ledger, this resume point. Tier T1.
- `c73260211` **head refresh and the H1d ledger entry (PR #1697)** — this resume
  point brought up to date with the merges that landed within minutes of `#1682`;
  the `#1672`/`#1690` entry added to the ledger. Tier T1.
- `e28cbca16` **measurement code is evidence-critical (PR #1703)** —
  `AGENTS.md` rule naming the two failure families (structural zeros; accounting
  bypassed on the abnormal path) and why a probe needs the full review pass. Tier
  T1.
- `ac68f0eaf` **the G3.1 read flip (PR #1687, issue #1670)** — default-off
  behind `Options.native_symtable_read_flip`; `CACHE_VERSION` and
  `OPTIONS_AFFECTING_CACHE` untouched; new gate `parity-symtable-flip`, which
  caught a real defect on its first run. Tier T2 (default-off read flip).
- `d6647b8bc` **read-flip correction in this handoff (PR #1704)** — the defect
  was ordering, not a leak; the earlier "leaked `X@N` keys" line was an inference
  from one side of a diff dump and is retracted. Retracted in the ledger too.
- `9b0f426ca` **`check_callable_call` tail seam (PR #1700, issue #1673)** —
  calls 177,899 -> 378, defers 177,524 -> 2, aggregate `serialize_calls` -15.3%;
  refreshed ranking published; the lane also fixed its own audit probe's
  id-recycling undercount and lost-report path in the same PR. Tier T2/T3.
- `24ddff920` **pool checkout and venv fix (PR #1705)** — the pool resolves the
  main checkout via `git rev-parse --git-common-dir` and re-points `.venv` on
  every `claim`; lock order and release fixed. Tier T1.
- `0046062a1` **residual scalar-seam retirement (PR #1685, issue #1668)** — seven
  gates, per-seam 204 calls / 190 answered / 10,942 bytes -> 0; aggregate
  explicitly not claimed because the trees differ by six or more foreign commits.
  Tier T3.
- `cd54ca293` **semanal live-object seams (PR #1699, issue #1663)** — landed the
  non-wire conversion and flipped no default; the issue's mechanism was disproven
  (semanal wire is 0.54% of corpus writes) and the net loss is 3,177,447 gated
  non-wire calls against a 48ns raw-FFI floor. Tier T3 attempted, closed NO-GO.
- `58d32ab24` **F reopening one-family view (PR #1694, issue #1671)** — the
  `Instance` replacement view, env-gated default off, measured against the
  close-out's reopening bar and closed **NO-GO by ~4.7x**: the family's wire
  funnel is 2.12% of total work (1.347s of 63.572s) against a 10% bar, capture
  removes 32.4% of the walk's encodes (3.21M registrations), and routing reads
  adds no wire saving for 17.43M pyO3 round-trips (+5.8s). All four ADR-0004
  contract surfaces measured satisfiable, so the binding constraint is
  arithmetic, not contract; ADR-0006 is the draft successor. Tier T2.
- `a5bb83244` **G1.2 node-shadow audit and first expression-node read flip (PR
  #1695, issue #1674)** — the generated fidelity audit (213 slot rows: 76 served,
  8 wire, 42 marker-only, 6 class-name-only, 41 structural, 40 gap) floors the
  `snapshot_definition` candidate and picks the #1635 aststrip lvalue surgery;
  `rust_aststrip_process_lvalue` serves `is_new_def` + `name` from the shadow
  behind `Options.native_ast_mirror_read` (default off, not in
  `OPTIONS_AFFECTING_CACHE`) with the Python tail as the identical fallback.
  `testfinegrained` 747/27 in all three gate states; `aststrip.served` 26 /
  `deferred` 128; wire delta <=2 events. Tier T2. Merge prep fixed the stale
  `-> None` annotation that failed CI's self-check (8 errors) and filed #1708
  for an inherited identity-layer flake.
- `5b012830e` **wave-5 ledger entries and the final-head T4 battery (PR #1707)**
  — the last two entries (`#1671`/`58d32ab24`, `#1674`/`a5bb83244`), the head
  line and queue brought to the merged state, and the "T4 on the merged wave
  head" section. Tier T1.

### T4 baseline (lane A8), measured at `271175175`

Kernel rebuilt from source into a private scratch dir; the worktree's
`crates/type_kernel/src` hashed identical to the main checkout's (126 files),
and the source-tree guard printed the worktree path for `mypy`.

- `cargo test -p mypy-type-kernel` -> `test result: ok. 2838 passed; 0 failed;
  11 ignored; 0 measured; 0 filtered out; finished in 0.16s`
- `testtypes -n0` -> `3905 passed, 7 skipped in 7.28s`. The suite's native
  classes are gated on `_NATIVE_WIRE_ENABLED = _env_gate("TEST_NATIVE_TYPE_KERNEL")
  and _HAS_TYPE_KERNEL_WIRE`, so the same file without that env var reports
  `354 passed, 3558 skipped` and proves nothing about the native seams.
- `testcheck -n0` -> `8198 passed, 15 skipped, 7 xfailed in 684.41s (0:11:24)`,
  identical to the CI `parity` line; the ledger's older 8,144/69/7 figure came
  from the `TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1` differential form
- cold self-check `-n0 --no-incremental -p mypy -p mypyc` -> `Success: no
  issues found in 353 source files`

**`main` moved eighteen times after that measurement** (`271175175` ->
`e531197d3` -> ... -> `a5bb83244`; the full chain is at the top of this
section), and seven of those are production changes (`a7329c5d4` H1d heads,
`ac68f0eaf` the read flip, `9b0f426ca`, `0046062a1`, `cd54ca293`, `58d32ab24`,
`a5bb83244`). That battery therefore describes the code at `271175175` only; the
battery below re-runs it on the merged head. It was nevertheless clean at the
pinned head. Its wall-clock figure is provisional: `uptime` was recorded
alongside the run, but the host was under external load (load average 34-48),
not quiet.

### T4 on the merged wave head (`a5bb83244`)

Full battery re-run by the coordinator on the final wave head; kernel rebuilt
from source into `/private/tmp/mypy-rs-wave5-tk` (the source-tree guard printed
the main checkout for `mypy`):

- `cargo test -p mypy-type-kernel` -> `2856 passed; 0 failed; 11 ignored`
- `testtypes -n0` (`TEST_NATIVE_TYPE_KERNEL=1`) -> `4013 passed, 7 skipped`
- `testcheck -n0` -> `8198 passed, 15 skipped, 7 xfailed in 281.62s` — the
  counts are line-identical to the `271175175` battery above, which is the
  load-invariant claim; the wall clock is not comparable (host conditions)
- fine-grained family (`testfinegrained`, `testfinegrainedcache`, `testdaemon`,
  `testmerge`, `testdiff`, `-n0`) -> `1454 passed, 257 skipped in 411.58s`
- cold self-check `-n0 --no-incremental` -> `Success: no issues found in 354
  source files` (354 = the baseline's 353 plus `mypy/typeview.py`)
- audit `--check` (`misc/g12_node_shadow_audit.py`) -> tables match the derived
  state on this base

Counts are the evidence; no `uptime` line was taken for this run, so its wall
clocks carry no claim.

### Process change: tiers, the weighted pool, the source-tree guard

- The tiers are recorded in `AGENTS.md` under `Verification Expectations`. The
  local full corpus is now the T4 wave step, not a per-lane step: CI runs
  `pr-gate` plus the `parity*` jobs on every production PR, so a lane restores
  only its own suite file, the gate-off/on differential and engagement
  counters. T3 lanes are capped at one or two per wave.
- Heavy ops run through `/private/tmp/mypy-rs-sem.sh` (3 slots; build = 1,
  corpus = 2). The flat `/private/tmp/mypy-rs-heavy.lock` mutex is retired: a
  release kernel build used to wait out an entire corpus run behind it.
- Review: `ocr review` for T3 and the wave diff; `ocr delegate preview|rule`
  plus an independent reader for T1/T2.

### Invariants learned this wave

- **Source-tree guard.** A worktree `.venv` symlinks to the main checkout's
  venv and its editable install points at the main checkout, so `import mypy`
  can resolve there and invalidate every count. The symptom is
  `ImportPathMismatchError` naming the main checkout's `mypy/test/conftest.py`
  against the worktree's, with 0 engagement on every seam while a probe reports
  thousands. The guard must print the worktree path before any number is
  trusted.
- **A sampled-probe zero is not a dead seam.** A zero-call reading from a
  sampled probe says something about the sample's coverage, not about the seam;
  only a whole-corpus count licenses "retire" (PR #1686 kept the falsified row
  visible for this reason).
- **GNU versus BSD tooling produces confident wrong numbers, not errors.** The
  `parity-ast` regex was verified with BSD `grep` while CI runs GNU `grep` (PR
  #1691), and a BSD `sed` that ignores `\s*` produced a wrong seam count (PR
  #1692). When a measurement feeds a decision, do it in Python.
- **OCR findings are leads, not verdicts.** A pass emits confident findings
  while its own reads fail (three internal `file_read failed: invalid line
  range` errors in one 5-file pass). Cost is a range, not a figure: 3 files at
  9m37s / ~760k tokens to 5 files at 6m56s / ~2.4M tokens.
- **Numeric claims need their head and their load.** Wall-clock numbers carry
  `uptime` and a provisional mark; load-invariant counters (compiled units,
  call and defer totals) are the primary evidence on this host.
- **Class heavy ops by cost, and expect the pool to be a race.** Pool
  acquisition is not FIFO: a 9-second single-file suite classed as `run 2` sat
  behind fourteen waiters and the legacy lock for 24 minutes, while the same
  command as `run 1` acquired a slot and finished in 51 seconds. A single suite
  file is build-class; only a real corpus is `run 2`.

### Queue for the next wave

1. **Landed: the first read flip** — `ac68f0eaf` (#1687, issue #1670),
   default-off: `rust_snapshot_symbol_table_shadow` serves the namespace from the
   G3.0a shadow store instead of `table.items()`, behind
   `Options.native_symtable_read_flip` (plus `..._verify`, which implies it),
   with `CACHE_VERSION` and `OPTIONS_AFFECTING_CACHE` untouched. Its own new gate
   `parity-symtable-flip` failed on its first run with `RuntimeError: … changed
   namespace order for '__main__'`, and the cause was **ordering only, not a
   leak**: `mypy/server/aststrip.py:113-118` keeps `@`-named keys across a strip,
   so `D@5` kept its original dict position while the store re-minted its ordinal
   after a per-build reset — same key set, same values, only the position moved
   (the dump carries the same 10 keys on both paths). Fixed by treating an owner
   whose first recorded write already finds keys as `ShadowGap::Inherited`
   (`defer_inherited`) and deferring to the live walk, with both differential
   assertions left strict (values/keys and `list(owned)` order). **Residual for
   G3.2:** only namespaces the store saw from empty in this build are served;
   aststrip survivors and cache-loaded tables defer, so closing that needs the
   store to survive the build boundary.
2. **The seam-registration split.** `scripts/plan_seam_split.py --verify`
   supplies the grouping (961 sites, 120 defining modules, 5 declared modules
   registering nothing); the transform itself (per-module
   `register_registry(m)`) is the next commit. Confirm the six doubly
   registered names before moving them: `rust_classify_simple_literal_type`,
   `rust_classify_tuple_type_implicit`, `rust_count_stats`,
   `rust_object_from_instance`, `rust_pretty_seq`,
   `rust_refers_to_typeddict`.
3. **Wave-5 lane landings: closed.** Every production lane merged and its entry
   is appended to `docs/plans/type-kernel-seam-ledger.md` in this PR (the last
   two are `#1671`/`58d32ab24` and `#1674`/`a5bb83244`).
4. **`#1698` — the semanal net-loss seams, assigned.** The `#1668`-style hot
   net-loss set from `#1699`'s NO-GO (`refers_to_fullname` +556ns,
   `rust_lookup` +273ns, `refers_to_class_or_function` +497ns) plus the
   quiet-host re-measurement. The re-measurement must precede any future semanal
   gate flip: the five-window `semanal_time` deltas run against the load trend in
   the stable windows, so nothing there is a clean verdict.
5. **`#1706` — filed, open.** `hard_exit` flushes stdout before
   `atexit._run_exitfuncs()` and then `os._exit`s without flushing, dropping
   redirected-stdout writes, which is the thing the `#1061` comment claims to
   prevent. Relevant to every probe that writes its report to a file.
6. **Open PRs at handoff time: only this docs PR (`#1707`).** `#1694` merged as
   `58d32ab24` and `#1695` as `a5bb83244`, both with all checks green and no
   unreplied review comments; their lane worktrees and branches were removed
   after merge.
7. **`#1708` — filed, open (noticed, not fixed).**
   `NativeSymtableReadFlipSuite::test_never_adopted_table_defers` flakes because
   the shared raw identity layer answers `handle_of` for a never-registered
   `SymbolTable` whose address was recycled: the raw map is unpinned, survives
   the mirror resets, and is only cleared by `rust_mirror_reset`, which does not
   run with the type mirror off. Reproduced on `main` without wave-5 gates
   (1/10 full-file runs), so it is inherited; fix direction and evidence on the
   issue.

## RESUME POINT — 2026-09-14, night (second parallel wave landed)

Five parallel workers ran the top of the queue (perf #1640/#1641 +
Phase-2 slices #1632/#1633/#1636). All five landed as merged PRs the
same day; the two issues PR bodies failed to auto-close (#1640, #1636)
were closed manually with landed evidence. `AGENTS.md` ledger entries
for the five landings are in the same docs PR as this section.

### Where `main` stands

`main` = `c28d98f82` (feat native dependency walk, PR `#1651`), on top
of `a41f4a94a` (`#1652` resolver upkeep), `82964b3c9` (`#1650` pattern
driver), `bdacb0910` (`#1649` stubgen collectors), `ad8783e97` (`#1648`
hot-seam retirement), `fb325d068` (`#1647` prior ledger+handoff).

Landed this wave, in merge order:
- `#1648` **retire hot short-call seams (#1640)**: deleted the
  serialize-round-trip fast paths for `is_generic`,
  `has_recursive_types`, `can_be_true/false_default`, `is_var_arg`,
  `is_kw_arg`, `min_args`, `max_possible_positional_args`,
  `TupleType.length`, `UnionType.length` (types.py 112+/198-). Proxy
  saving ~6.97s across ~1.58M seam calls (-> 0). testtypes 3774/7,
  testcheck exact, self-check clean.
- `#1649` **stubgen printer collectors (#1636)**: three seams in new
  `stubgen.rs` behind `_HAS_NATIVE_STUBGEN`; caught a real unwrap-loop
  bug pre-merge (math vs `not` phases). 1,710 Python hits -> 0.
  teststubgen 373/1/2 text-exact, self-check clean.
- `#1650` **pattern-check driver heads (#1633)**: six classifiers in
  `checkpattern.rs` with verbatim-Python fallbacks. 1,345 Python-body
  hits -> 0 on the match corpus (self-check has zero match stmts, so
  engagement is testcheck-measured). testcheck 8198/15/7 exact.
- `#1652` **dirty-driven per-SCC resolver upkeep (#1641)**: content-sig
  diff over post-seal fields in `build.py`; the issue's
  mark-at-promotion direction failed parallel self-check (155 errors)
  via the fixup backwards-promotion hack and was dropped. Builtins
  re-pushes 53,563 -> 1 (-n0) / 4 (workers). fg 747/27, fgc 549/229,
  daemon 38, warm self-check cache-consuming.
- `#1651` **native dependency walk (#1632)**: full `DependencyVisitor`
  over live PyO3 objects in new `depswalk.rs` (~2,100 lines), Python
  walk kept as fallback. 12,410 Python-body hits -> 0. testdeps 230,
  full fine-grained family green, testcheck exact.

Closed: #1640, #1641, #1632, #1633, #1636. Still open: #1624 (standing
perf blocker), #1620 (wire gaps), #1621 + slices #1628/#1629, #1626-#1629,
#1634/#1635/#1637 (unclaimed Phase-2 slices), #1642 (check_call cluster).

### Post-merge verification on `c28d98f82` (fresh kernel, this session)

- Kernel rebuilt from the merged head into
  `/private/tmp/mypy-rs-local-tk-final`.
- Cold self-check `-n0 --no-incremental`: clean, 353 files.
- Targeted suites for all five landings: 70 passed.
- Shared `.venv` editable re-pointed at main (`uv sync`).

### Queue for the next wave

1. #1642 (check_call cluster) + #1637 (scalar-only seams) — last two
   unowned perf slices.
2. #1634 (complex-statement drivers) + #1635 (subexpr/aststrip) — last
   two unclaimed Phase-2 module slices.
3. #1621 slices #1628/#1629 (ParamSpec/TVT solve) — `solve.rs`
   released by #1618; the 95-call origin-rebuild boundary is the wall.
4. #1620 remaining (definition slot, ParamSpec/TVT meta_level,
   PartialType, ErasedType).
5. All five wave worktrees removed; branches deleted locally and on
   origin; per-issue scratch `.so` dirs deleted. `git worktree list`
   shows only the main checkout.

---

## RESUME POINT — 2026-09-14, evening (stopped swarm finished, all work landed)

The mid-flight swarm from the previous resume point was resumed with four
parallel agents and finished the same day. All four workstreams landed as
merged PRs; no work remains uncommitted anywhere. `AGENTS.md` ledger entries
for the four landings are in the same docs PR as this section.

### Where `main` stands

`main` = `3f2b30293` (fix(expandtype) `remove_trivial` relink, PR `#1644`),
on top of `8408236c1` (`#1643` perf-audit docs), `3189ff903` (`#1646`
extras channel), `34204931f` (`#1645` live nominal), `501663e5c` (`#1638`
Phase-2 ranking), `aa9a3e00a` (`#1639` prior handoff).

Landed this session, in merge order:
- `#1645` **live nominal fallback for snapshot-missing classes (#1619)**:
  `LiveNominal` + `visit_instance_nominal_live` + the maptype
  no-type-vars fast path over the live `TypeInfo` (subtypes.rs). Probe
  45 -> 19 snapshot-miss sites; the four PEP 695 variance tests green
  (the on-demand-sealing regression did not recur). Gates: cargo
  2822/11, testtypes 3754/7, testcheck 8198/15/7 exact, fg 747/27,
  daemon 38, self-check clean.
- `#1646` **constraint `extra_tvars` channel (#1618)**: `extra_tvars`
  ride the Rust->Python FFI blob (`origin | op | target | extras-count
  | extras`); dead `callable_with_vars_reachable` deleted. 22,184 calls
  / 56 defers -> 22,131 / 5; 42 extras delivered natively. **No
  `CACHE_VERSION` bump** (in-process FFI only). Gates: cargo 2812/11,
  testtypes 3748/7, full parity green in CI.
- `#1643` **perf wire-traffic audit docs (#1624)**: persists
  `docs/plans/2026-09-14-perf-wire-traffic-audit.md`. Fix direction 4
  exhausted (0.028% serialized-then-deferred); top-15 hot-seam ranking
  with fix shapes. Follow-ups filed: #1640, #1641, #1642. #1624 stays
  OPEN as the standing perf blocker.
- `#1644` **`remove_trivial` fresh-var relink (#1623)**: strict
  `resync_var_identities_list`; `_needs_python` drops `meta_gate`;
  issue items 1-3 audited stale (remove_dups landed, parent_error a
  floor, Name@line alias-caused). Gate defers 459 -> 17, strict-relink
  defers 0. Local OCR 0 findings. Gates: cargo 2822/11, testtypes
  3761/3, full parity green in CI.

Closed with evidence, no code: `#1249` (self-hosted runner). No owner
PAT exists in this environment, so re-registration was impossible; the
label path was retired by setting repo variable
`ENABLE_AI_CODE_REVIEW` `true` -> `false` (2026-09-14, verified
read-back). The `ocr-review` job now skips instead of queueing forever.
OCR stays the reviewer of record, run manually per PR. Restore path
(re-register with an owner PAT, flip the var back) is on the closed
issue. The local ephemeral-manager will keep logging its 60s 403
backoff until then; harmless.

Filed and open: #1632-#1637 (Phase-2 slices, from #1638), #1640-#1642
(perf follow-ups, from the #1624 audit), #1624 (standing blocker),
#1626-#1629 (plugin/paramspec follow-ups, prior session).

### Post-merge verification on `3f2b30293` (this session, fresh kernel)

- Kernel rebuilt from the merged head into
  `/private/tmp/mypy-rs-local-tk-final` (never shared across checkouts).
- Cold self-check `mypy_self_check.ini -n0 --no-incremental -p mypy
  -p mypyc`: `Success: no issues found in 353 source files`.
- Targeted testtypes (constraints + snapshot-gap + remove_trivial +
  freeze-identity + unify suites): 76 passed.
- Shared `.venv` editable re-pointed at the main checkout (`uv sync`;
  concurrent agents had re-pointed it at their worktrees mid-session —
  always force-prefix the worktree on `PYTHONPATH` when agents run
  concurrently, or avoid `uv run` entirely in favor of
  `.venv/bin/python`).

### Queue for the next wave

1. #1640 (types.py short-call seams: `has_recursive_types`,
   `callable_is_generic`, `copy_modified`) — the top of the hot-seam
   ranking; live-object or batched interfaces.
2. #1641 (dirty-driven per-SCC resolver update) — root cause (a),
   ~1.9s resolver upkeep.
3. #1618 follow-through: the 95-call origin-rebuild uniqueness
   boundary is #1621's wall (free ParamSpecs solve to `Never` if
   relaxed); #1621 slices #1628/#1629 are unblocked file-wise now
   (`solve.rs` released).
4. #1620 remaining: `definition` slot, `meta_level` on ParamSpec/TVT
   ids, `PartialType`, `ErasedType`.
5. Worktrees below are removed after their merges; `git worktree list`
   should show only the main checkout. Per-issue scratch `.so` dirs
   under `/private/tmp/mypy-rs-local-tk-*` may be deleted freely.

### Unmerged branches — none. Worktrees removed this session

`feature/mv-wire-extra-tvars`, `feature/mv-identity-contracts`,
`perf/mv-wire-traffic`, `docs/handoff-2026-09-14` (branches deleted
locally and on origin after their squash-merges); `worktrees/mypy-rs-
1618`, `mypy-rs-wave87-identity`, `worktrees/mypy-rs-1624`,
`worktrees/mypy-rs-handoff` removed. The two foreign stashes
(`swarm/seam3-join-type-list`, `swarm/checkexpr-fastpaths`) belong to
other workspaces and were left alone.

---

## RESUME POINT — 2026-09-14 (mass-migration swarm, stopped mid-flight)

Work was deliberately stopped on 2026-09-14. Five of six dispatched agents
were cancelled with their worktrees intact, so the sections below are the
authoritative resume state. Nothing was deleted; no branch was discarded.

### Where `main` stands

`main` = `98d844aee` (docs: backfill AGENTS.md ledger, PR `#1630`), on top of
`fe173320e` (`#1631` design briefs), `ae0590ece` (`#1617` H1s), `656cddbe3`
(`#1616` H1r), `f3d1b1dcb` (`#1615` H1q).

Landed today:
- `#1617` **H1s `rust_is_type_like`** merged (rebased over H1r, 4-file
  conflict resolved; `IsTypeLike`/`IsSelfMemberRef`/`IsOverloadedItem` 31
  passed locally; `pr-gate` + all four `parity*` jobs green; local `ocr` gate
  0 blocking / 2 advisory, both recorded on the PR).
- `#1597` **closed with evidence** (H1c `rust_check_exit_return_type` was
  already in `main`; 14 tests re-verified on the current head).
- `#1630` **ledger backfill merged**: `AGENTS.md` had no records for
  `G1.1`, `G2.1`-`G2.6`, `H1c`-`H1s` (17 PRs); entries reconstructed from the
  landed commits with re-verified `file:line` anchors, no invented metrics.
- `#1631` **design briefs merged**: `docs/plans/2026-09-14-plugin-callback-brief.md`
  and `docs/plans/2026-09-14-paramspec-variables-brief.md`. `#1622` closed
  `NOT_PLANNED` with a cost-argued verdict; follow-ups filed: `#1626`
  (enumerable plugin hook surface), `#1627` (pure `DefaultPlugin` hook
  bodies, low priority), `#1628` / `#1629` (the two `#1621` slices).
- `#1249` triaged: the runners API is still `403` for a collaborator token,
  but PR gates now run **GitHub-hosted `ubuntu-latest`** and are green, so it
  no longer blocks. Owner action required (re-register with an owner PAT, or
  retire the self-hosted label in `code-review.yml`).
- PR `#1638` **open, not merged**: `docs: rank Phase-2 mass-migration vertical
  slices (#1625)` — the Phase-2 ranking document (454 lines). Its issues were
  NOT yet filed; the survey was skipped on the resource rule (combined RSS
  42.8-44.8 GB against the 40 GB cut) so it ranks on an `rg` seam inventory
  plus cited wave numbers, and says so. Merge or re-run its step 0 first.

### Stopped mid-flight — work preserved on disk

| Workstream | Worktree | Branch | Uncommitted state | Next step |
|---|---|---|---|---|
| `#1618` wire `extra_tvars` (+`#1620`) | `worktrees/mypy-rs-1618` | `feature/mv-wire-extra-tvars` | `crates/type_kernel/src/constraints.rs`, `mypy/constraints.py` (+85/-16) | early: the wire record itself is untouched. Extend `wire.rs` + `mypy/types.py` `CallableType`, bump `CACHE_VERSION` in `mypy/cache.py`, then port `callable_with_vars_reachable` (`visitor.rs` ~320-345). |
| `#1619` snapshot gap | `worktrees/mypy-rs-1619` | `feature/mv-snapshot-live-nominal` | `crates/type_kernel/src/subtypes.rs`, `mypy/build.py` (+374/-3) | substantive. Build, then verify the PEP 695 variance tests stay green (on-demand sealing regressed them before) plus `inst:left_snap_missing` (74) via an env-gated probe. |
| `#1623` identity contracts | `mypy-rs-wave87-identity` | `feature/mv-identity-contracts` | `mypy/expandtype.py`, `mypy/wirefixup.py`, `mypy/test/testtypes.py` (+293/-60) | substantive. Finish, then run fine-grained + daemon suites (identity regressions live there). Part of the issue's premise is stale — audit before porting. |
| `#1624` performance | `worktrees/mypy-rs-1624` | `perf/mv-wire-traffic` | no code; audit evidence in `/private/tmp/mypy-rs-1624-audit*.out` | finish the ranked table and post it on `#1624`. Build its own `/private/tmp/mypy-rs-tk-1624` from current source before trusting any kernel-on count. |
| Phase-2 planning | (none; ran in the main checkout) | `docs/mv-phase2-slices` | committed + pushed; PR `#1638` open | merge `#1638`, then file the ranked slices as issues. |

Cancelled agent transcripts are retained under
`~/.qwen/projects/-Users-jonathangadeaharder-projects-coding-utils-mypy-rs/subagents/`
and can be revived with `send_message` against their task ids if the original
thread is still resident.

**Measurement from the perf audit (load-insensitive counters, one cold
self-check in the `#1624` worktree at `656cddbe3`):**
- `mypy.checkexpr.plugin_call_hook_known_absent` 323,014 and
  `_try_native_plugin_hook` 323,014, plus `plugin_hook_known_absent` 111,346 —
  an inert hook-probe surface (see invariant 1 below).
- `_collect_incremental`: 521 calls, 424,094 module visits, 53,563
  `re_pushed_builtins`, `resolver_build_s = 1.864` — the per-SCC snapshot
  upkeep cost the perf issue names as root cause (a).

### Queue for the next wave

1. Merge `#1638` (Phase-2 ranking), then file its unblocked slices as issues.
2. Finish `#1618` first: it is the largest wall (268 defers) and it holds
   `solve.rs`, which blocks `#1621`.
3. `#1621` slices `#1628` (expandtype substitution arms) and `#1629`
   (pass-1-only solve seam split; spike-gated) — both need `#1618` / `#1623`
   to release their files. `#1621`'s premise was corrected: the wire already
   carries `variables` (`wire.rs:639`) and `new_unification_variable` is
   already ported for all three kinds (`freshen.rs:722-800`) — it is a
   substitution-engine problem, not a wire-format one.
4. `#1620` items beyond `extra_tvars`: `definition` slot (37
   `format_type_distinctly` defers), `meta_level` on ParamSpec/TVT ids,
   `PartialType`, `ErasedType`.
5. Ledger: append the mass-migration wave entries once their PRs merge. The
   backfill facts for the older series are staged at
   `/private/tmp/wave-record-backfill-notes.md`.

### Invariants and gotchas learned 2026-09-14

1. **A user plugin makes every native hook seam inert.** `mypy/build.py:2095`
   computes `has_user_plugins = len(plugins) > 1` and
   `mypy/checkexpr.py:504-507` states the consequence: the registry is ignored
   and all lookups defer to Python, because user plugins may match arbitrary
   fullnames. `mypy_self_check.ini:22` loads `mypy.plugins.proper_plugin`, so
   **the gate corpus is a user-plugin corpus**: measured 328,408 inert resolve
   attempts with 0 usable results. Consequence for planning: hook-path defer
   numbers measured on the cold self-check are best-case, not what a
   plugin-using user sees. Tracked as `#1626`.
2. **The kernel import is all-or-nothing.** `try: from type_kernel import
   (...)` in `mypy/semanal.py` et al. — a stale `.so` missing one newly added
   function makes the whole block raise `ImportError` and nulls that entire
   battery of seams silently. Example: H1s added `rust_is_type_like` to
   `mypy/semanal.py`'s import list, so any H1r-vintage `.so` now disables
   every semanal seam. Always rebuild per checkout, into a per-issue scratch
   dir, and never share one `.so` across worktrees at different commits.
3. **`ocr-review` never completes** (`#1249`); the operative review gate is
   `ocr review --from origin/main --to <branch> --audience agent` run from the
   worktree, and it posts nothing to GitHub. Merge with
   `gh pr merge --squash --admin` once `pr-gate` + `parity*` are green.
4. **PR gates run on GitHub-hosted `ubuntu-latest`** now, not the self-hosted
   label; docs-only diffs skip the `parity*` jobs via path filters and run
   only `pr-gate`.
5. **Memory**: with six agents live, combined uid-501 RSS sat at 38-44 GB
   against the 51 GB cap. Cap `pytest` at `-n2` when more than two agents run
   heavy suites, and never `-n auto`.
6. **Worktree hygiene**: per-issue kernel scratch dirs live at
   `/private/tmp/mypy-rs-local-tk-{1618,1619,1623,1624,h1s,h1r}`; the older
   shared `local-ast` / `local-resolver` dirs are read-only inputs. Remove a
   worktree only after its PR merges; `git worktree list` should be clean
   before finishing.

### Unmerged branches to harvest or discard

```
feature/mv-wire-extra-tvars      # #1618, uncommitted changes in the worktree
feature/mv-snapshot-live-nominal # #1619, uncommitted changes in the worktree
feature/mv-identity-contracts    # #1623, uncommitted changes in the worktree
perf/mv-wire-traffic             # #1624, no commits (audit artifacts in /tmp)
docs/mv-phase2-slices            # PR #1638 open
```

The three `#1618`/`#1619`/`#1623` worktrees hold real, uncommitted work; do not
`worktree remove --force` them without reading the diff first.

---

*Written 2026-08-28, refreshed 2026-09-11 (post-wave71: waves 52-71
landed the st find_member/unpack/apply-report ports (#1492, embedded
112 -> 61), the icf SUBTYPE_OF protocol-actual arm (#1487), the
plugin-synthesized TypeInfo registrar (#1489), the ctor-blob gate
clearing (#1488), the alias-aware typeobj decode (#1496, decode_None
60 -> 0), the astdiff snapshot builders (#1498/#1501), the
fixed-format cache meta writer (#1504, byte-parity), the wave-60/61
audit + retire waves (#1508/#1509, #1513/#1514), the wave-62
max-parallel wave (D #1521 alias instantiation 68 -> 0 + pretty 37 ->
0; C #1522 fresh 31 -> 0, remove_dups 26 -> 0, typeobj 18 -> 0; B
#1523 protocol-member 7 -> 3; A #1524 join/meet 58 -> 22 net, with the
wave-33 msu defer restored after a combined-tree fine-grained
segfault), wave 63 (A #1531 mirror graduation audit: capture overhead
-22.1%, extra_attrs splice, parity-mirror CI job), and wave 64 (A
#1535 fixed the F2 stale-blob bug (#1530 closed; 7 raw-list mutators
get a `touch`/epoch bump; the two CI deselections re-enabled); B #1536
bounded the recursive-alias pretty walks (#1532 closed; deterministic
tests, layout-perturbation repro 20s -> 0.07s); C #1534 added the
in-repo librt build enabling `write_raw_bytes` (#1526 closed; splice
suite 3 passed/3 skipped -> 7/0; two latent bugs fixed) and the
`mypyc/lib-rt/**` CI trigger), and wave 65 (A #1544 cut
mirror capture overhead 37.3% (+125.5s -> +78.7s; 10% gate unreachable
without the ADR-0004 proxy per the revised decision); C #1542 retired
five icf protocol-member sub-buckets, raw-FFI fallbacks 149 -> 56
(-62%); B #1540 produced the Phase G0 brief persisted at
docs/plans/2026-09-11-phase-g0-next-steps.md with scoped follow-ups
#1545 (AST wire v5: docstring/custom-typing-module/version-pin bugs),
#1546 (dead scaffolding + AST CI), #1547 (teststubgen pre-existing
failures)), and wave 66 (A #1552 fixed the three native-parser parity bugs
via AST wire v5 - docstrings, --custom-typing-module, reader-side
pos_only; B #1550 deleted 2,040 dead AST-scaffolding lines and added the
`parity-ast` CI gate (#1547's yield trio xfailed there); C #1549 produced
the ADR-0004 proxy brief at docs/plans/2026-09-11-adr0004-proxy-brief.md
with P1/P2 filed as #1553/#1554; #1551 filed for the multi-alias import
split), waves 67-68 (G0.3/G0.4 expression + statement enums with
byte-parity A/B; the #1547 stubgen trio root-caused to a traverser
tri-state bug; proxy P1 scaffold; proxy P2 measured and DROPPED; strong
pins landed), and wave 69 (G0.5 symbol-node writer + cache payload,
default-off; multi-alias import grouping with AST wire v6; proxy P2b
second negative), and wave 70 (G1.0a node dual-write shadow scaffold;
the F-program close-out: F kernel-complete, F4 rung retired unclaimed), and wave 71 (G1.0b
expression fields, G2.0 statement/def metadata shadow, the G3
symbol-table brief + G3.0a filed).
Two
docs-only negative closes also landed (B6 render bundle #1481; maptype
timing-gap #1494 - the #1493 audit that followed disproved its own
hypothesis and landed the alias-decode fix #1496 instead). Goal:
"migrate all python code to rust, really all", pursued as the established measure -> file -> dispatch-agents -> process-PRs ->
gate loop. This file is the resume point.*

## Where main stands (2026-09-11, post-wave71)

- `main` = `2a3d36427` (G2 shadow, `#1580`, #1577) on top of
  `239037192` (`#1582`, #1578), `6f0b4ef9d` (`#1579`, #1576),
  `efb524ee7` (`#1574`, #1572), `db7f32450` (`#1571`),
  `7a3410dda` (`#1568`, #1551),
  `be99e1664` (`#1569`, #1566), `f001381c4` (`#1570`, #1567),
  `770fe596c` (`#1565`, #1537), `8fd1ee5b5` (`#1563`, #1554),
  `04f725791` (`#1562`, #1528), `93aa18ed4` (`#1561`, #1560),
  `b63f903e1` (`#1558`, #1553), `95546e537` (`#1559`, #1547),
  `85fadf488` (`#1550`, #1546), `37633b024` (`#1552`, #1545),
  `bf019a1f1` (`#1548`, #1540), `aa11047e1` (`#1542`, #1541),
  `ddd4ff44a` (`#1544`, #1539), `7a2b2f850` (`#1538`),
  `beeab0fef` (`#1534`, #1526), `92f27e06d` (`#1536`, #1532),
  `c08025b32` (`#1535`, #1530), `57ccaa38b` (`#1533`), `490a06c3e`
  (`#1531`, #1527), `82e75bb21` (`#1529`),
  `3d9b3b8fb` (`#1525`, #1520), `13e9e9f7b`
  (`#1524`, #1516), `19d68949c` (`#1523`, #1517), `81a8149dd` (`#1522`,
  #1518), `0bb6827cb` (`#1521`, #1519), `66e709794` (`#1515`),
  `6f8a031e1` (`#1513`, #1512), `530919b65` (`#1514`, #1511),
  `65bb5d49e` (`#1510`), `6f74795ce` (`#1509`, #1507), `dacf5f2d2`
  (`#1508`, #1506), `e37d7a7b6` (`#1505`), `2a4f638fd` (`#1504`,
  #1503), `fc7a61dba` (`#1502`), `19ab01862` (`#1501`, #1500),
  `f112dca00` (`#1499`), `e7205b87e` (`#1498`, #1497), `89f161b91`
  (`#1496`, #1493), `fc9892ab4` (`#1495`) and `5a5038ad2` (`#1494`,
  #1490); local ff'd to origin.
- Phase state: F3 complete as implemented; the F2 read flip is now
  correct on the identified raw-list mutators (#1530 closed) and the
  wire-cache splice is exercisable (#1526 closed). #1528 (daemon-stable
  handles) is re-scoped to the strong-pin protocol.
- Gates on the merged head `2a3d36427`: cargo type_kernel 2,811/11,
  ast_serialize 26/0; testtypes 3,471/7; testcheck 8,198/15/7 exact in
  kernel, node-shadow-on, and G2-shadow-on modes; fine-grained family
  905/28 (fg+daemon+merge+diff); cold self-check clean (351 files).
- F program CLOSED (#1573, `docs/plans/2026-09-11-f-program-close-out.md`):
  F0-F3 landed opt-in; F4 retired unclaimed; the claim-ladder rung is
  removed in `docs/remaining-migration-plan.md` and reopening requires a
  one-family replacement-view prototype clearing >=10% relative total
  work share with full parity green. G is now the active ownership
  track (G0 complete, G1.0a landed).
- Shared `.so` rebuilt 2026-09-11 at the merged head content (wave-63
  Rust, codesigned); `/private/tmp/mypy-rs-local-typekernel`.
- Wave-56: the alias-aware typeobj decode retry
  (`_deserialize_type_with_aliases`, the #1224/#1309 contract) retired
  all 60 cold-self-check `decode_None` events; the wave-55 "missing
  TypeInfo" attribution was a module-global probe artifact - the real
  cause is alias-bearing composites; 32 `kernel_none` unchanged.
- Wave-57: astdiff `snapshot_type` ported to `rust_snapshot_type`
  (`crates/type_kernel/src/astdiff_snapshot.rs`); testfinegrained
  75,713 calls @ 98.3% (1,263 defers), testfinegrainedcache 31,120 @
  98.05% (607), testdaemon 631 @ 100%. All defers are generic
  `CallableType` (`normalize_callable_variables` needs
  `expand_type`/`strict_optional_set`) - the designed wall.
- Wave-58: astdiff `snapshot_symbol_table` / `snapshot_definition` /
  `snapshot_untyped_signature` ported to `rust_snapshot_symbol_table`
  (`crates/type_kernel/src/astdiff_symbols.rs`); testfinegrained
  7,580 calls @ 100%, finegrainedcache 3,384 @ 100%, daemon 97 @ 100%,
  zero whole-table defers. The only non-native leaf is the slice-1
  generic-CallableType callback. B7 is now complete for both halves.
- Wave-59: fixed-format cache meta writer ported
  (`rust_write_cache_meta` / `_ex`, cache.rs; new wire primitives
  `write_bytes*`, `write_str_list`, `write_big_int`, tagged JSON writer).
  Byte-parity battery + nativeread round-trip; cold self-check 808+808
  meta writes @ 100% native; warm run consumes the Rust-written cache.
  Cache read+write are now native; the DATA payload (`tree.write` /
  node deserialization) remains Phase G. OCR healthy again (1 low
  advisory: unused `ex` param in `NativeCacheMetaWriterSuite._ref` -
  trivial future cleanup).
- Wave-60A (audit, #1506): engagement audit of the 0-40% native-share
  seams - ZERO ports, all 151 fallbacks across the 12 targets
  classified; no engagement/gate regression of the #1455 ifta class.
  Bucket table in the AGENTS.md wave-60A entry (not-delitem scope
  floor, linearize snap-miss, covers/restrict floors, remove_dups
  alias identity, bare-recv-tvar, icf protocol/union engine, join
  lkv/args walls, icf expand floors). Do not re-audit these blind.
- Wave-60B (ports, #1507): `rust_is_singleton_identity_type` 48 -> 0
  fallbacks (FunctionLike singleton arm + type-object `is_final`
  cascade), `rust_container_type` 36 -> 17 (decided-none sentinel skips
  the duplicate Python join), `rust_format_type_distinctly` 44 -> 37 +
  `rust_format_type_bare` 9 -> 1 (definition-free pretty delegation
  gated by `_pretty_wire_safe`); `rust_instantiate_type_alias` 68 stays
  a documented floor (bare-generic `set_any_tvars` with defaulted alias
  tvars). OCR rounds fixed the singleton cascade and the
  `_pretty_wire_safe` TypedDict traversal gap.
- Wave-61A (#1511): decidable leftovers retired - `restrict_subtype_away`
  check2 2 -> 0 (`erase_instances=True` proper-subtype check),
  `map_type_from_supertype` expand 10 -> 0 (FlatAliasGuard alias-union,
  ParamSpec splice, unpack interpolation/TVT default, itemgetter),
  `covers_at_runtime` Overloaded erase 1 -> 0; also a latent
  `with_normalized_var_args` TVT divergence fixed. Floors untouched:
  12 snap-miss (#1490/#1486), 1 tuple-super, 2 covers subtype-none.
- Wave-61B (#1512): five un-audited seams, net 72 -> 4 fallbacks -
  `arg_approximate_similarity` 20 -> 0, `builtin_item_type` 17 -> 0,
  `add_class_tvars` 8 -> 0, `narrow_with_len` 11 -> 0,
  `is_overlapping_types` 16 -> 4. Floor: 4 overlap step-6 st
  TypeType-left pairs (`Type[...]` vs `Callable(Extension)` x3,
  `Union[Type[...]]` vs `Overloaded` x1). Rebased over #1514; all
  gates re-run on the combined tree (a shared-file overlap in
  `argapprox.rs`/`checker_helpers.rs` auto-merged; cargo + testcheck +
  self-check verified).
- Wave-62 (max-parallel, 4 agents + 1 brief): D (#1521)
  `instantiate_type_alias` 68 -> 0 (defaults-only `set_any_tvars` tag
  with native `expand_type` per default) and `format_type_distinctly`
  37 -> 0 (per-callable `(func_name, first_arg)` hints via
  `_pretty_hint`); C (#1522) `freshen_all_functions_type_vars` 31 -> 0,
  `remove_dups` 26 -> 0 (`py_type_eq` + `_dedup_alias_identity_sound`
  precondition), `type_object_type_from_function` kernel_none 18 -> 0;
  B (#1523) `get_protocol_member` 7 -> 3 (class-Self key gate, inverted
  `is_instance_var` polarity fixed, live attribute-hook probe) with
  union/none/member-method floors re-affirmed; A (#1524) join/meet
  58 -> 22 net (meet tuple arm 6 -> 0, cross-arm joins, nested alias
  materialization) - its `msu(handle_recursive=False)` alias-expansion
  sub-feature was REVERTED during merge: the combined tree segfaulted
  in `testfinegrained` via exactly the wave-33
  `join -> tuple_fallback -> msu -> is_subtype` loop, and the
  `None => defer` guardrail is restored with its regression test.
- Wave-62E (#1520/#1525): Phase F brief persisted at
  `docs/plans/2026-09-11-phase-f-next-steps.md`; follow-ups #1526
  (librt `write_raw_bytes` missing, wire cache inert), #1527 (mirror
  graduation audit: capture 2.5-2.6x vs 10% gate, no CI coverage, dead
  profiler hook), #1528 (daemon-stable handles blocked on weakrefs).
- Wave-63A (#1527/#1531): mirror capture overhead cut 22.1% (290.1s ->
  226.0s audit-on; +198.2s -> +134.1s over the 91.9s off baseline);
  `Instance.extra_attrs` splice op; new `parity-mirror` CI job
  (capture + read over testtypes/testcheck/fine-grained); dead
  `misc/f3s9_tvar_union.py` hook retired; audit/decision doc at
  `docs/plans/2026-09-11-mirror-capture-audit.md`. Found pre-existing
  F2 read-flip stale-blob bug #1530 (two read-step tests deselected in
  CI with the issue link).
- Wave-63B (#1528) DEFERRED: the `__weakref__`-on-`Type.__slots__`
  route hangs `testNoCrashOnRecursiveTupleFallback` (native sample:
  unbounded `rust_flatten_nested_unions` re-entry; layout-sensitive,
  not fixed by alias-deferring guards). WIP diff preserved at
  `/private/tmp/w1528_evidence.patch`; re-scope to the strong-pin
  retire protocol; hardening filed as #1532
  (`flatten_nested_unions` no-resolver recursive-alias loop).
- Wave-64A (#1530/#1535): F2 raw-list-mutation stale-blob fix - a
  registry of 7 mutator sites (typeanal.py:2070, types.py:2870
  `normalize_trivial_unpack`, semanal.py:2331/2342 extends,
  semanal_typeargs.py:101/117/128, checker.py:1725) now calls
  `types_mirror.touch` (epoch bump + re-serialize/cascade); reads gate
  on `rust_mirror_write_skip(h, _UNPROT_EPOCH)` and re-serialize when
  stale. The two parity-mirror read-step deselections are re-enabled.
- Wave-64B (#1532/#1536): recursive-alias pretty-walk hardening - the
  driver was `_pretty_wire_safe` / `_unsafe_pretty_callables` (fresh
  proper trees defeat the id-keyed seen set), not `flatten_nested_unions`;
  `_repeat_alias_expansion` cuts on alias node + args in both walkers,
  and the no-resolver flatten branch defers every recursive row.
  Deterministic regression tests + layout-perturbation repro 20s ->
  0.07s. Noticed, not fixed: #1537 (2 pre-existing ruff C408).
- Wave-64C (#1526/#1534): in-repo `mypyc/lib-rt` build provides the
  Python-level `write_raw_bytes` missing from the PyPI wheel; CI builds
  it and prepends it in pr-gate + parity/parity-mirror; splice suite
  3 passed/3 skipped -> 7/0. Fixed a raise-path leak of
  `_type_wire_cache_session_depth` that permanently disabled the wire
  cache (try/finally, 10/10 stress green) and the
  `NativeWriteFunnelSkipSuite` cache coupling. OCR high finding fixed:
  `mypyc/lib-rt/**` added to the parity paths trigger.
- Wave-65A (#1539/#1544): mirror capture overhead cut 37.3% (capture
  217.4s -> 171.3s median; +125.5s -> +78.7s over the 92.6s baseline;
  ratio 2.37x -> 1.85x) via a cached walk context, registration fusion
  (`rust_mirror_walk_registration`), clean-run guards, and import
  hoisting. Audit counters unchanged. Revised decision: the 10% gate is
  unreachable by hot-path tuning (residual is the F1 proof's fixed
  per-object work); ADR-0004 proxy stays the graduation path.
- Wave-65B (#1540): Phase G0 brief persisted
  (`docs/plans/2026-09-11-phase-g0-next-steps.md`): no AST node enum in
  production; three confirmed native-parser parity bugs (docstrings,
  `--custom-typing-module`, `pos_only_special_methods`) plus a latent
  `transform_source` gap; dead scaffolding; AST-path CI blindness; the
  family order/risk ranking; and the G0.1-G0.5 PR sequence.
- Wave-65C (#1541/#1542): icf protocol-member actual-shape arms -
  both-protocol SUP widened, callback/typeobj callable arms, Overloaded
  matcher + Instance `__call__`, tuple-protocol tail; raw-FFI fallbacks
  149 -> 56 (-62%). Residual 51 `ffi-extra-tvars` (#1171/#1427
  multi-wave channel) + 5 small defers. Line-loss path deferred and
  tracked as #1543.
- Wave-66A (#1545/#1552): AST wire v5 - FuncDef/ClassDef docstrings
  (byte-exact vs ast.get_docstring(clean=False), surrogate-safe),
  `--custom-typing-module` translation with implicit asname,
  reader-side `pos_only_special_methods`, `AST_WIRE_VERSION = 5`
  enforced at the parse entry, `transform_source` routed on the native
  branch. Writer-side `argument_elide_name` is unconditional in
  fastparse too, so gating it would diverge - documented deviation.
  testIncludeDocstrings green; #1551 filed (multi-alias import split).
- Wave-66B (#1546/#1550): deleted 2,040 dead AST-scaffolding lines
  (nodes_full/nodes_codec/full_ast_codec/visitor_engine); added
  ast_serialize cargo test/fmt/clippy and a `parity-ast` CI job
  (test_nativeparse + testparse + teststubgen, paths trigger extended);
  moved the astwire-only tags out of the 150-152 cache range to 230-232
  (traverser-local, no stored format change); #1547's yield trio
  reproduces on main and is xfailed with the issue link.
- Wave-66C (#1549): ADR-0004 proxy graduation brief persisted
  (`docs/plans/2026-09-11-adr0004-proxy-brief.md`): ADR staleness
  corrections, the blob-backed shadow first slice, purge points,
  coherence/plugin/cache/daemon risks, and the P1-P4 sequence (P1/P2
  filed as #1553/#1554). #1528's weakref claim corrected (all eight
  Type classes fail weakref).
- Wave-67A (#1556/#1557): G0.3 expression enum - `ExprNode` +
  `ast_writer.rs::write_expr` + frozen `expr_legacy.rs` A/B reference;
  byte parity over the 250-case corpus (241 parsable) and goldens; the
  A/B caught a missing STR_EXPR END_TAG pre-landing. No wire change.
- Wave-67B (#1553/#1558): proxy P1 scaffold - `crates/type_kernel/
  src/proxy.rs` blob store keyed by identity handles with strong pins,
  six `rust_proxy_*` pyfunctions, `mypy/type_proxy.py` FFI-free maps,
  `Options.native_type_proxy` (default off), reset branch, ADR-0005
  draft, 11-test suite. Zero behavior change. OCR high findings
  (reentrancy: pins dropped under the RefCell borrow) fixed before
  merge by moving pin drops outside the guard and renaming `drop`.
- Wave-67C (#1547/#1559): the stubgen yield trio root-caused to the
  type kernel, not stubgen: `rust_has_return_statement` answered true
  for `return None` because astwire drops NameExpr.name. Seeker is now
  tri-state (non-NameExpr return -> true; NameExpr-only -> defer to
  the Python ReturnSeeker; no expression -> false). Xfail removed;
  teststubgen 373/1/2/0.
- Wave-68A (#1560/#1561): G0.4 statement/pattern enums - `StmtNode` +
  `PatternNode` + `stmt_legacy.rs` frozen reference; cross-binary
  sha256-identical for all 250 corpus cases and all 343 mypy/mypyc
  sources; parser-options matrix and golden blobs unchanged.
- Wave-68B (#1554/#1563): proxy P2 lazy Instance read shadow -
  implemented, all correctness gates green (13 tests, proxy-on
  self-check clean), then DROPPED on the drop-if-no-win rule: 5
  interleaved pairs, median wall 174.7s off vs 173.8s on (-0.5%),
  user CPU +5.6% median; ~349.5k hits / ~192.5k misses / ~198.8k
  puts. Implementation preserved at
  /private/tmp/mypy-rs-1554-p2.patch; re-scope options recorded in
  #1554 (size/engagement threshold, wire-cache-off corpus).
- Wave-68C (#1528/#1562): strong-pin stable handles - identity.rs
  stable layer holds strong `Py<PyAny>` pins (weakrefs are dead for
  every Type class), shared stable-first handle namespace,
  retire/reset_stable/preserve_stable, refcount-1 sweep; mirror
  prefers stable handles; `_clear_native_resolvers` calls proxy reset
  first then the preserving mirror reset; dmypy recheck identity test
  (daemon 38). Residual: a dropped graph inside a mypy-side reference
  cycle keeps its pin (documented, conservative).
- Wave-69A (#1566/#1569): G0.5 symbol-node writer - `sym_node.rs`
  ports the nodes.py fixed-format write field sets (MypyFile, symbol
  tables, TypeInfo, Var/FuncDef/Decorator/OverloadedFuncDef, the three
  TypeVar-like exprs, TypeAlias, DataclassTransformSpec) with opaque
  captured type/JSON payloads; `mypy/cache_data.py` + build dispatch;
  byte-identical output (CACHE_VERSION stays 13); 811 production writes
  audited 0 defer / 0 mismatch; testcachedata suite. The hybrid write
  phase measured ~1.5x the Python writer (36.3ms vs 55.1ms per 57-tree
  round), so the bridge ships behind a new default-off
  `Options.native_cache_data` (`TEST_NATIVE_CACHE_DATA`).
- Wave-69B (#1551/#1568): multi-alias import grouping - `import a, b`
  now yields one `Import` node; `IMPORT_METADATA` carries the statement
  alias list; AST wire v5 -> v6 with the caller/version pin + golden
  blob; parser differential + 5 data cases; deps/stubgen unaffected.
  #1551 had been auto-closed by #1552 in error and was reopened first.
- Wave-69C (#1567/#1570): proxy P2b (thresholded shadow) second
  negative - corpus A +8.97% wall / +13.99% CPU (threshold defers);
  corpus B (wire cache off) -11.02% wall but inside noise (ceiling
  <=0.5%, 22% wire-cache hit rate, 91% of puts <=64B); DROPPED; patch
  at /private/tmp/mypy-rs-1567-p2b.patch. The F program stays
  kernel-opt-in.
- Wave-70A (#1572/#1574): G1.0a node dual-write scaffold - new
  `crates/type_kernel/src/node_mirror.rs` (identity-handle store,
  strong pins, merged per-node record, 9 pyfunctions) + new
  `mypy/nodes_mirror.py` (patched `__setattr__` on RefExpr/CallExpr/
  IndexExpr/OpExpr, lazy adoption, gated by default-off
  `Options.native_ast_mirror`); RefExpr binding fields
  (kind/target/_fullname/is_new_def/is_inferred_def) + analyzed class
  record captured at all 20 known sites. Parity: testcheck and
  fine-grained identical with the shadow on; 12-test suite. Residuals:
  ClassDef.analyzed/G2 metadata; cold shadow-on pins adopted nodes;
  target stored by fullname (G1.1 read-flip channel). One medium OCR
  advisory (partial class patch on compiled builds leaves inert hooks).
- Wave-70B (#1573): F-program close-out brief persisted
  (`docs/plans/2026-09-11-f-program-close-out.md`): status table, why
  the claim cannot be earned by the shadow, the falsifiable reopening
  bar, and the three load-bearing facts. Claim ladder updated in
  `docs/remaining-migration-plan.md` (F4 rung removed; G4 text now
  "the AST executes in Rust").
- Wave-71A (#1576/#1579): G1.0b remaining expression fields - the node
  shadow gains a per-field record (kind/flag/name/kinds) for
  method_type(s), as_type, right_always/right_unreachable, def_var, and
  the RefExpr special flags; `nodes_mirror.touch` covers the append-only
  `method_types` list. 15-test suite; engagement method_type 5 sites,
  touch.method_types 715, as_type 1363.
- Wave-71B (#1577/#1580): G2.0 statement/def metadata shadow - tagged
  field-map store (interned names, strong pins), `_meta_setattr` on 12
  classes, all claimed fields captured (AssignmentStmt/For/With/If/
  Match/TypeAliasStmt/ImportBase, FuncDef+flags, OverloadedFuncDef,
  Decorator, ClassDef.info+analyzed, Var). 20-test suite; astmerge
  re-registration pinned via `replace_object_state`.
- Wave-71C (#1578/#1582): G3 symbol-table brief persisted
  (`docs/plans/2026-09-11-g3-symbol-table-brief.md`) with three G0-brief
  corrections, the accessor surface, the two C-API bypass facts, the
  astmerge contract, and the G3.0a-d sequence; G3.0a filed as #1581.
- Wave-71 merge lesson: the two shadow agents edited the same files and
  independently defined `nodes_mirror.touch` and same-named test
  helpers. Concatenating conflict sides is NOT sufficient: the repair
  rebuilt testtypes.py as A's file + B's inserted suite, unified `touch`
  (G1 list fields vs G2 metadata), and fixed a truncated Rust function
  and a cut lib.rs registration macro. Always compile plus run the
  Python suites AND the self-check after a same-file rebase; `cargo test`
  alone missed the Python-side damage.
- Runner note unchanged: the repo runner cannot re-register (403,
  admin-blocked; #1249 open); GH `ocr-review` jobs stay `queued`
  forever. The operative review gate is the local
  `ocr review --from origin/main --to <branch> --audience agent`,
  then `gh pr merge --squash --admin` after pr-gate + parity green.
  The 2026-09-11 provider-level OCR outage was transient; waves 59-61
  reviewed normally. After parallel agents, CHECK the main checkout:
  wave 60 left stray formatter churn in 5 files (wrong line length +
  isort reorder); discarded with `git checkout --` before pulling.
  Draft PRs must be `gh pr ready` before `gh pr merge` (waves 61A/62A
  hit the "still a draft" error once each). After a multi-PR rebase
  chain, run the FINE-GRAINED suite on the combined tree, not just
  cargo/testcheck/self-check: wave 62A's
  `msu(handle_recursive=False)` alias expansion merged cleanly but
  segfaulted the combined tree via the wave-33
  join/tuple_fallback/msu loop, caught only by `testfinegrained`. The
  local pre-commit comment-block hook (max 3 consecutive comment
  lines) also blocks on otherwise-fine Rust comments; keep added
  comment runs at 3 lines.

## Waves 33-63 (since the 2026-08-31 refresh)

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
| #1469 | #1462 | wave49: sgc/ct embedded defer audits - re-pin exact (sgc 253 = icf 173 / apply_generic 69 / solve_defer 7 / multi_lower_fnlike 4; ct 29 = concrete_sub_undecided 14 / overlap 13 / restrict_away_2 2), zero ports, negative close with bucket tables + AGENTS.md wave-48b entry (precedent #1458); audit dump via os.write(2,...) (hard_exit swallows atexit output) | sgc/ct floors documented; gates exact (cargo 2,721/11, testtypes 3,243/6, testcheck 8,198/15/7) |
| #1468 | #1466 | wave49: wave-48 classifier seams deferral-contract gap - `?` PyErr propagation first mapped to a blanket Ok(None) (agent), then narrowed to `is_instance_of::<PyAttributeError>` ONLY after an ocr [bug·high] blocker on the PR (orchestrator fix round); other PyErrs re-propagate so kernel bugs stay visible (mirror.rs read_slot precedent) + error-class boundary pins (`_BrokenAttrInfo` exc param, RuntimeError re-propagates on both seams) | both seams defer on AttributeError only; C1 9,268 / C2 167,532 @ 100%; testtypes 3,251/6 (+8), testcheck exact, self-check clean 347 |
| #1472 | #1456 | wave50: register runtime-synthesized fake TypeInfos into the NativeTypeResolver snapshot - `TypeChecker.make_fake_typeinfo` single funnel for all 4 fabrication sites fires a registrar (BuildManager map, `update` first-seal-wins, rollback on failure, cleared at all 3 resolver-clear sites); zero Rust changes; daemon discipline per #1137/#1146 (no variance inference => empty-scc hazard N/A) | st fake-info defers 78 -> 0 (embedded probe; astmerge `<subclass of ...>` 68 + maptype/checkexpr 10); testtypes 3,258/6 (+7 NativeFakeInfoRegistrationSuite); testcheck exact; fine-grained 747/27, daemon 37, merge 41/1, diff 79 green |
| #1473 | #1457 | wave50: retire the rust_is_subtype nested-alias defer leaf - leaf-reason audit (env-gated) pinned `expand_top_aliases` else-branch (raw-target unroll mirroring get_proper_type) + a flag-only `contains_recursive_alias` walk (is_recursive, depth cap, defers union-join paths) with the wave-33 segfault guardrails (recursive-alias set + fine-grained regressions green) | st 632 -> 260 embedded (-372, -59%); testtypes 3,251/6 (recursive-alias gate pin now asserts True); cargo 2,727/11 (+6 units); testcheck exact; testRecursiveTupleFallback1-5 + TypeAliasUpdate* Coarse/Fine green |
| #1474 | #1459 | wave51: delete 5 zero-caller seams in errors_helpers.rs (dead kernel surface, YAGNI + Phase E1 design-only directive; B6 decision recorded: the 4 B6-relevant ones get re-created against a live contract when the errors render bundle ports) + 6 self-pinning unit tests + .pyi stub lines + lib.rs registrations | gates EXACT post-deletion: cargo 2,721/11 (-6 removed tests), testtypes 3,258/6, testcheck 8,198/15/7, self-check clean |
| #1475 | #1470 | wave51: `rust_classify_final_super` getattr swallow narrowed to `is_instance_of::<PyAttributeError>` only (wave-49 pattern; inner `v.is_true()` swallow left per issue, tracked #1477); `_BrokenFinalVar` + 4 pins (seam defers on AttributeError, re-propagates RuntimeError, gate-off/on parity both classes) | testtypes 3,262/6 (+4); cargo 2,729/11; testcheck exact; self-check clean |
| #1476 | #1461 | wave51: wire CallableType Display (and shared `write_parameters_inner`) bound `arg_kinds`/`arg_names` via `.get(i)` -> ARG_POS default / unnamed, enumerate rewrite (clippy needless_range_loop), 2 Rust pins for the fallback render | no panic on shape-mismatched blobs; cargo 2,729/11 (+2); gates exact otherwise |
| #1480 | #1477 | wave52: `rust_classify_final_super` INNER `v.is_true()` blanket swallow narrowed to `is_instance_of::<PyAttributeError>` only (wave-49 pattern) + `_BrokenFinalVar`/RuntimeError pins | contract-class arm closed; testtypes +4; gates exact |
| #1481 | #1479 | wave52: B6 errors render-bundle audit - documented NEGATIVE, zero legacy-path traffic on the gate corpus | docs-only; the 4 deleted #1459 seams stay dead |
| #1486 | #1483 | wave53: `rust_expand_type_by_instance` residual audit - 49 snap-miss + 1 call-unpack; issue hypotheses disproven; floors routed to #1484/#1485/#1490 | docs-only; 0 ports |
| #1487 | #1482 | wave53b: icf SUBTYPE_OF protocol-actual structural arm (native protocol-member constraints for the Instance-vs-protocol-template dispatch) | icf SUBTYPE_OF arm native; gates exact |
| #1488 | #1484 | wave54: clear the expand/maptype gates inside `_native_ctor_blob` (extend wave-22 #1324), zero doomed FFI round-trips in the blob window | no seam entries in blob windows (probe) |
| #1489 | #1485 | wave54: register plugin-synthesized TypeInfos (`make_fake_register_class_instance`) via the #1456 registrar | `functools._SingleDispatchRegisterCallable` class closed; testtypes +7 |
| #1492 | #1491 | wave55: st find_member fetch semantics (`find_member_semantics=true`), `visit_unpack_type`, `callable_corresponding_argument` meet subset, `APPLY_REPORTED` channel; local OCR advisory (test tearDown wire-map leak) fixed in `1daf86a84` | st embedded 112 -> 61 (-45.5%); cargo 2,736/11 (+13); testtypes 3,279/6; testcheck 8,198/15/7 exact; self-check clean 347 |
| #1494 | #1490 | wave55: maptype timing-gap residual audit - documented floor (5 events; on-demand sealing rejected: mid-SCC PEP 695 variance finality, 4-test regression) | docs-only; function-local wire-ref gap filed #1493 |
| #1496 | #1493 | wave56: alias-aware typeobj decode retry (`_deserialize_type_with_aliases`, #1224/#1309 contract) - the #1493 hypothesis (missing/local TypeInfos) was disproven by a per-fixer probe; the real cause was alias-bearing composites | `decode_None` 60 -> 0 (32 `kernel_none` unchanged); testtypes 3,282/6 (+3 NativeTypeObjectAliasDecodeSuite); testcheck 8,198/15/7 exact; self-check clean |
| #1498 | #1497 | wave57: astdiff type-snapshot builder port (`rust_snapshot_type`, new `astdiff_snapshot.rs` ~630 lines; order-sensitive arms call Python `set`/`sorted`; generic-Callable/Partial defer) | testfinegrained 75,713 @ 98.3% (1,263 defers, 100% generic CallableType), finegrainedcache 31,120 @ 98.05%, daemon 631 @ 100%; cargo 2,736/11; testtypes 3,291/6 (+9 NativeAstdiffSnapshotSuite); testcheck exact; self-check clean |
| #1501 | #1500 | wave58: astdiff symbol/definition snapshot builder port (`rust_snapshot_symbol_table`, new `astdiff_symbols.rs` ~570 lines; slice-1 walk factored `pub(crate)`; per-node Python callback only for the generic-CallableType leaf) | testfinegrained 7,580 @ 100%, finegrainedcache 3,384 @ 100%, daemon 97 @ 100%, 0 whole-table defers; cargo 2,736/11; testtypes 3,298/6 (+7); testcheck exact; fine-grained 747/27, daemon 37, merge 41/1, diff 79; self-check clean. OCR provider-level failure -> manual review |
| #1504 | #1503 | wave59: fixed-format cache meta writer port (`rust_write_cache_meta`/`_ex` live-object walkers + `write_errors`/`write_json*`; cache.rs; wire.rs gains `write_bytes*`/`write_str_list`/`write_big_int`/tagged JSON writer; build.py native-first with Python `WriteBuffer` fallback; version prefix stays Python) | byte-parity battery + native-read round-trip; cold self-check 808+808 meta writes @ 100% native, 0 defers; warm run consumes Rust-written cache; cargo 2,739/11 (+3); testtypes 3,309/6 (+11); testcheck 8,198/15/7 exact; fine-grained 747/27, daemon 37, finegrainedcache 549/229; self-check clean |
| #1508 | #1506 | wave60A: engagement audit of the 0-40% share seams (`MYPY_TK_W60A_AUDIT` probes, stripped) - 12 targets / 151 fallbacks classified; no ifta-class gate bug | docs-only; bucket table in AGENTS.md; gates exact (cargo 2,739/11, testtypes 3,309/6, testcheck 8,198/15/7, self-check clean) |
| #1509 | #1507 | wave60B: singleton identity (`is_singleton_identity_type` FunctionLike arm + type-object `is_final` cascade), container decided-none sentinel, definition-free pretty delegation via `_pretty_wire_safe`; OCR fixes (singleton cascade + TypedDict traversal) | singleton 48 -> 0, container 36 -> 17, distinctly 44 -> 37, bare 9 -> 1, instantiate 68 floor; cargo 2,741/11 (+2); testtypes 3,321/6 (+12); testcheck 8,198/15/7 exact; fine-grained 747/27, daemon 37, finegrainedcache 549/229; self-check clean |
| #1514 | #1511 | wave61A: restrict check2 (`erase_instances=True`), maptype expand arms (FlatAliasGuard alias-union, ParamSpec splice, unpack interpolation/TVT default), covers Overloaded erase; latent `with_normalized_var_args` TVT fix | restrict 2 -> 0, maptype expand 10 -> 0, covers 1 -> 0; floors untouched (12 snap-miss, 1 tuple-super, 2 covers); cargo 2,746/11 (+5); testtypes 3,323/6; testcheck 8,198/15/7 exact; fine-grained family green; self-check clean; OCR 2 low advisories |
| #1513 | #1512 | wave61B: five un-audited seams - alias expansion (arg similarity, builtin item), `expand_type_by_instance_free` for class tvars, alias/union len-narrow with proper-first special method, live `__call__` fetch for overlap | net 72 -> 4 fallbacks (arg 20->0, builtin 17->0, class tvars 8->0, narrow 11->0, overlap 16->4); cargo 2,746/11; testtypes 3,343/6 (+20); testcheck exact; rebased over #1514 with all gates re-run |
| #1521 | #1519 | wave62D: defaulted alias-tvar fill tag (native `expand_type` per default) + per-callable pretty hints (`_pretty_hint` -> `apply_pretty_hint`) | instantiate_type_alias 68 -> 0, format_type_distinctly 37 -> 0; cargo 2,808/11; testtypes 3,350/6; testcheck exact; fine-grained 747/27, daemon 37 |
| #1522 | #1518 | wave62C: freshen Overloaded/ParamSpec/TVT + resolver/alias decode retry; remove_dups `py_type_eq` + alias-identity precondition; typeobj `alias_ok` + FlatAliasGuard + `collect_query_tvars` alias chain | freshen 31 -> 0, remove_dups 26 -> 0, typeobj 18 -> 0; cargo 2,750/11; testtypes 3,355/6; testcheck exact; fine-grained 747/27, daemon 37 |
| #1523 | #1517 | wave62B: protocol-member class-Self key gate + inverted `is_instance_var` fix + live attribute-hook probe; join structural-protocol parity guard | get_protocol_member 7 -> 3; union/none/AMM floors re-affirmed; cargo 2,752/11; testtypes 3,353/6; testcheck exact; fine-grained 747/27, daemon 37 |
| #1524 | #1516 | wave62A: meet tuple arm, cross-arm instance joins with remap, nested alias materialization, TypeType default-arm parity fix; `msu(handle_recursive=False)` alias expansion REVERTED (wave-33 guardrail restored after a combined-tree fine-grained segfault) | join_types 21 -> 7, join_instances 17 -> 7, join_type_list 10 -> 7, meet_types 6 -> 0, join_tuples 4 -> 1; cargo 2,769/11; testtypes 3,352/6; testcheck exact; fine-grained 784/27 (fg+daemon) after the revert |
| #1525 | #1520 | wave62E: Phase F next-steps scoping brief persisted (docs/plans/2026-09-11-phase-f-next-steps.md); follow-ups #1526/#1527/#1528 | docs-only |
| #1531 | #1527 | wave63A: mirror graduation audit - capture cuts (`_HANDLE_BY_ID`, setattr restructure, untracked skip), `Instance.extra_attrs` splice, `parity-mirror` CI job, audit doc; found #1530 | capture 290.1s -> 226.0s audit-on (-22.1%); cargo 2,772/11; testtypes 3,385/6 both gate states; testcheck exact both states; fine-grained 747/27 + daemon 37 with capture+read |
| #1535 | #1530 | wave64A: F2 raw-list mutator touch/epoch registry (7 sites) + stale-gate in read_fresh_bytes; CI deselections re-enabled | repro 2 failed -> 2 passed; testcheck 8,198/15/7 exact in kernel/capture/read modes; testtypes 3,389/6; fine-grained 747/27 + daemon 37 with capture+read |
| #1536 | #1532 | wave64B: pretty-walk alias-expansion cut in both walkers + no-resolver recursive-row defer; deterministic regression tests | testcheck 8,198/15/7 exact; testtypes 3,389/6 (+4); fine-grained 747/27 + daemon 37; layout repro 20s -> 0.07s |
| #1534 | #1526 | wave64C: in-repo librt build (write_raw_bytes), CI wiring + `mypyc/lib-rt/**` trigger; cache-depth leak and write-funnel test-coupling fixes | splice suite 3/3 skipped -> 7/0; testtypes 3,397/3 with scratch librt; testcheck 8,198/15/7 exact; self-check clean |
| #1544 | #1539 | wave65A: mirror capture cuts (cached walk ctx, registration fusion, guards, import hoisting); revised 10% decision in the audit doc | capture 217.4s -> 171.3s (-21.2%), overhead -37.3%; cargo 2,773/11; testcheck exact in all three modes; fine-grained 747/27 + daemon 37 capture+read |
| #1542 | #1541 | wave65C: icf protocol-member actual-shape arms (both-protocol SUP, callback/typeobj callable, Overloaded + `__call__`, tuple-protocol tail) | raw-FFI fallbacks 149 -> 56 (-62%); cargo 2,773/11; testtypes 3,403/7; testcheck 8,198/15/7 exact; fine-grained 747/27 + daemon 37; #1543 filed |
| #1552 | #1545 | wave66A: AST wire v5 - docstrings, `--custom-typing-module`, reader-side pos-only, version pin, `transform_source` routing | AST suites 881/75/5 xfail; testcheck 8,198/15/7 exact; self-check clean; #1551 filed |
| #1550 | #1546 | wave66B: 2,040 dead scaffolding lines deleted; ast_serialize cargo gates + `parity-ast` CI job; astwire tag move to 230-232; #1547 xfailed with link | ast_serialize 9/0; type_kernel 2,773/11; AST parity 876/75/3 xfail; testcheck exact; self-check clean |
| #1557 | #1556 | wave67A: G0.3 expression enum + writer, frozen legacy A/B, byte parity | cargo 11/0; AST suites 884/75/2; testcheck exact; self-check clean |
| #1559 | #1547 | wave67C: traverser tri-state fix (`return None` vs `return x`); stubgen xfail removed | teststubgen 373/1/2/0; testcheck 8,198/15/7 exact; cargo 2,775/11 |
| #1558 | #1553 | wave67B: proxy P1 store scaffold + gate (proxy.rs, type_proxy.py, option, reset, 11 tests); OCR reentrancy fixes | cargo 2,783/11; testtypes 3,420/7; testcheck exact; self-check 348 |
| #1561 | #1560 | wave68A: G0.4 statement/pattern enums + writer; cross-binary sha256 parity | cargo 15/0; AST suites 884/75/2; testcheck exact; fine-grained 747/27 + daemon 37 |
| #1562 | #1528 | wave68C: strong-pin stable handles (identity.rs, mirror prefer-stable, preserving reset, daemon recheck test) | cargo 2,791/11; testtypes 3,424/7; daemon 38; testcheck exact; self-check 348 |
| #1563 | #1554 | wave68B: proxy P2 lazy read shadow DROPPED after A/B (-0.5% wall, +5.6% CPU median); docs-only negative close | implementation preserved at /private/tmp/mypy-rs-1554-p2.patch |
| #1569 | #1566 | wave69A: G0.5 symbol-node writer + cache-data bridge (default-off `native_cache_data`); byte-identical format | cargo ast 22/0; 811 writes 0 defer/0 mismatch; AST suites 888/75/2; testcheck exact; self-check 350 |
| #1568 | #1551 | wave69B: multi-alias import grouping; AST wire v5 -> v6 + caller pin, golden blob, differential tests | AST suites 896/75/2; testcheck 8,198/15/7 exact; self-check 350; fine-grained 747/27 + daemon 38 |
| #1570 | #1567 | wave69C: proxy P2b thresholded shadow measured on two corpora, DROPPED (second negative) | docs-only; patch preserved at /private/tmp/mypy-rs-1567-p2b.patch |
| #1574 | #1572 | wave70A: G1.0a expression node dual-write shadow scaffold (node_mirror.rs + nodes_mirror.py, RefExpr bindings, gated default-off) | cargo 2,798/11; testtypes 3,436/7 (+12); testcheck 8,198/15/7 identical shadow-on; fine-grained 747/27 + daemon 38; self-check 351 |
| (docs) | #1573 | wave70B: F-program close-out brief + claim-ladder update (F4 retired unclaimed, reopening bar recorded) | docs-only |
| #1579 | #1576 | wave71A: G1.0b remaining expression shadow fields (per-field records + touch) | cargo 2,806/11; testtypes 3,451/7; testcheck 8,198/15/7 exact; fine-grained 747/27 + daemon 38 |
| #1580 | #1577 | wave71B: G2.0 statement/def metadata shadow (tagged field map, 12 patched classes) + rebase repair (touch union, suite split) | cargo 2,811/11; testtypes 3,471/7; testcheck exact off/on; fine-grained family 905/28; self-check 351 |
| (docs) | #1578 | wave71C: G3 symbol-table accessor brief persisted; G3.0a filed as #1581 | docs-only |
| (docs) | #1549 | wave66C: ADR-0004 proxy graduation brief persisted; #1553/#1554 filed | docs-only |
| (docs) | #1540 | wave65B: Phase G0 scoping brief persisted + follow-ups #1545/#1546/#1547 | docs-only |

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
hand), #1458, #1460, #1464 (#1465 auto-closed it), #1465,
#1466 (#1468 auto-closed it), #1462 (#1469 auto-closed it), #1468,
#1469, #1456 (#1472 auto-closed it), #1457 (#1473 auto-closed it),
#1459 (resolved by #1474, deleted), #1470 (#1475 auto-closed it),
#1461 (#1476 auto-closed it), #1472, #1473, #1474, #1475, #1476,
#1477 (#1480 auto-closed it), #1479 (#1481 auto-closed it), #1482
(#1487 auto-closed it), #1483 (#1486 auto-closed it), #1484 (#1488
auto-closed it), #1485 (#1489 auto-closed it), #1490 (#1494
auto-closed it), #1491 (#1492 auto-closed it), #1432 (unify.rs
PolyModeGuard prev-restore + boolean labels, `dc1da15a4`), #1493
(#1496 auto-closed it), #1497 (#1498 auto-closed it), #1500 (shipped
in #1501, closed by hand), #1503 (#1504 auto-closed it), #1506
(#1508 auto-closed it), #1507 (#1509 + manual close), #1511 (#1514
merged; closed by hand), #1512 (#1513 merged; closed by hand), #1516,
#1517, #1518 (closed by hand after the wave-62 merges), #1519 (#1521
auto-closed it), #1520 (#1525 auto-closed it), #1527 (#1531 merged; closed by hand); #1528 deferred with evidence), #1530 (#1535 auto-closed it), #1532 (closed by hand after #1536), #1526 (closed by hand after #1534), #1539 (#1544 auto-closed it), #1541 (#1542 auto-closed it), #1540 (closed by the G0-brief docs PR), #1545 (#1552 auto-closed it), #1546 (#1550 auto-closed it); #1549 closed by the proxy-brief docs PR; #1547 fixed by #1559 (closed by hand), #1553 (#1558 auto-closed it), #1556 (#1557 auto-closed it), #1554 (negative close after #1563), #1566 (#1569 auto-closed it), #1551 (reopened after a wrong auto-close, fixed by #1568), #1567 (second negative after #1570), #1572 (#1574 auto-closed it), #1573 (close-out docs PR), #1576 (#1579 auto-closed it), #1577 (closed by hand after the #1580 repair), #1578 (docs PR #1582).

## Open backlog (next waves; dispatch max ~2 port agents)

1. **#1581 (G3.0a)** (next): namespace entry funnel + dual-write
   capture scaffold per the G3 brief (semanal adding funnel routed
   through `put_names_entry`, symtable_mirror.rs, 11-pin suite).
2. **G1.1/G2.1 read-channel design**: only with a measured consumer;
   G1.0b/G2.0 are record-only by design.
3. **G3.0b-d**: direct-write/delete sweep, TypeInfoAccess meta fields,
   astmerge re-registration (all after G3.0a).
4. **F reopen bar**: only a one-family replacement-view prototype
   clearing >=10% relative total work share with full parity green (see
   the close-out doc); the proxy scaffold is inert (wire or delete).
3. **F2/F4 graduation**: capture cuts are exhausted (wave 65A revised
   decision); the next step is the ADR-0004 proxy or the P4 default-on
   decision, and #1528 needs the strong-pin protocol first.
4. **#1537**: 2 pre-existing ruff C408 in testtypes - trivial cleanup
   when touching the file.
5. **#624 (seam work)**: residual fallbacks are the documented engine
   floors (st extra_tvars channel, icf residual 51, IAMA contract floor,
   sgc/roc/ifta); a fresh survey is needed before any new seam wave.
6. **#1249**: runner 403 - needs admin, skip until credentials change.

Standing audited floors (do NOT re-audit blind; the mechanism that
would unlock each is noted): st 61 embedded (19 serfail + 42 kernel:
18 `uds-var-unsafe` owned/meta-tvar wire identity - needs the fuller
owned-tvar/extra_tvars channel; 10 `cbd-expand-other`; 9
`ud-infer-args`); icf 266 (protocol-member engine class, multi-wave);
sgc 253 (icf 173 / apply_generic 69 / solve_defer 7 / multi_lower 4);
ct 29; ifta var_pspec_tvt 80 / engine 47 / solve 5; dc-final-overlap
67 (overlap kernel); join lkv wall 39; ama 120 contract floor; maptype
timing-gap 5 (documented #1490 floor); is_overlapping_types 4 (st
TypeType-left); join residual 22 (st protocol-right 19, via-supertype
generic tvar, wave-43 lkv guard, one NOT_READY wire-map entry); TD
access 14 (checker-state `__setitem__`/`get`); astdiff slice-1
generic-Callable callback. Bucket tables in AGENTS.md
(wave-47/48a/48b/49/53/55/56/57/58/59/60A/61A/61B/62A-D entries) - do
not re-derive.

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
  runs 2-3, and twice in a row on the wave-49 diff where the
  testtypes.py hunks sit far past ~line 3200). It is resumable: rerun
  with `--resume <session-id>` (the failure prints the session id),
  or just rerun fresh; the diff is rerolled and the run usually
  succeeds. Do not treat the error as a finding, but do not trust a
  "0 comments" summary that still shows the failure - re-verify the
  previous round's blockers are addressed by the new diff yourself.
- Deferral-contract seams: map ONLY `PyAttributeError` to `Ok(None)`
  (mirror.rs read_slot pattern, `e.is_instance_of::<PyAttributeError>
  (py)`); a blanket `Ok(x.unwrap_or(None))` swallow converts genuine
  kernel bugs into invisible defers (ocr [bug·high] blocker on #1468,
  fixed on the PR by narrowing). `rust_classify_final_super` getattr
  arm narrowed (#1470/#1475); its INNER `v.is_true()` arm is still
  blanket (tracked #1477).
- `agent-wait until github.pr` never converges: it also waits on the
  `ocr-review` check, stuck `queued` forever (#1249). The operative
  wait is `agent-wait until github.ci <run-id>` on the
  native-kernel-parity workflow run (jobs parity + parity-typeops),
  then merge on pr-gate + that run green.
