# F4.0 mirror graduation audit (wave 63A, issue #1527)

Date: 2026-09-11. Basis: read-only audit at `82e75bb21` (post-wave62)
plus cold self-check A/B runs, a cProfile capture run, and a temporary
env-gated defer-audit sweep. All instrumentation was stripped before
landing; only the intentional CI env gate and audit counters remain.

## 1. Harness

Cold self-check, single process, cache off:

```bash
W63_MIRROR=1 PYTHONPATH=<ast>:<resolver>:<type_kernel> TEST_NATIVE_TYPE_KERNEL=1 \
  .venv/bin/python misc/wf1_selfcheck_mirror.py -- \
  --config-file mypy_self_check.ini -n0 --no-incremental -p mypy -p mypyc
```

The driver patches `Options` from `W63_*` env vars and uses
`main(clean_exit=True)` so the audit `atexit` dump survives. Machine
note: a sibling port agent ran in parallel on the same host, so absolute
walls carry some noise; the A/B pairs below were taken back to back.

## 2. Measured wall clock

| config | audit | wall | vs off |
|---|---|---|---|
| mirror off | n/a | 91.9s | baseline |
| capture pre-cut, same harness | on | 290.1s | +198.2s (+216%) |
| capture post-cut, same harness | on | 226.0s | +134.1s (+146%) |
| capture pre-cut (brief #1520) | off | 263.3s / 248.5s | +163.2s (+163%) |
| capture + F2 read flip (brief) | off | 266.4s | +166.3s |
| capture post-cut, clean | off | 217.4s | +125.5s (+137%) |
| capture pre-cut, cProfile | on | 584.0s (profiler on) | n/a |

Same-harness A/B (audit on): 290.1s -> 226.0s, a 64.1s cut (-22.1% of
the capture wall), i.e. the capture overhead over baseline fell from
+198.2s to +134.1s. Audit-free post-cut capture is 217.4s; the brief's
263.3s baseline is cross-harness, so read that pair as -17.4% with the
machine-load caveat. The plan's cold-path gate is within 10% of
baseline (`docs/remaining-migration-plan.md:707`, risk register
"Cold-path wrapper overhead"). At +137% the capture path is ~14x over
that gate, so the gate is NOT reachable by hot-path tuning; the
remaining cost is architectural (see section 5).

## 3. Ranked cost table (pre-cut capture, cProfile, 584.0s wall)

| function | tottime | cumtime | calls |
|---|---|---|---|
| `_mirror_setattr` | 50.8s | 124.4s | 100.4M |
| `type_kernel.rust_mirror_walk_indices` | 47.1s | 47.1s | 1.7M |
| `_handle_of` | 15.4s | 27.9s | 59.8M |
| `_rules_ok` | 14.1s | 14.1s | 128.1M |
| `_child_types` | 12.5s | 26.4s | 4.1M |
| `isinstance` | 12.2s | 13.0s | 137.5M |
| `dict.get` | 11.0s | 11.1s | 53.9M |
| `Instance.write` | 10.8s | 77.1s | 16.5M |
| `_register_tree` | 9.4s | 135.5s | 2.8M |
| `type_kernel.rust_mirror_handle_of` | 8.9s | 8.9s | 59.8M |
| family `write` wrapper | 8.8s | 178.5s | 21.7M |
| `_assert_fresh` | 7.5s | 140.8s | 7.7M |
| `_fresh_bytes` | 5.7s | 52.9s | 2.5M |
| `_count` | 5.3s | 8.1s | 21.7M |

Event volumes (audit counters, same config): 4.29M Instance, 0.96M
CallableType, 0.54M TypeVarType and 0.27M UnionType constructions;
7.7M funnel asserts (4.9M epoch skips, 0.77M adoptions); 1.9M strike
skips; 5.6M gagged setattrs; 0.61M failed registrations; 1.7M
`_walk_indices` crossings; 0.14M cascade syncs.

The shape: every family object pays one extra full serialization
(blob) plus a Rust index walk at adoption, and every funnel pays a
Python wrapper plus 1 to 2 FFI crossings. The 100M setattr calls are
mostly constructor slot writes (6.1M constructions x ~15 slots).

## 4. What was cut (all landed, opt-in path only)

1. `_handle_of` is now a Python `_HANDLE_BY_ID` map instead of
   `rust_mirror_handle_of` on every call. `_register_tree` is the only
   minting path, so the maps stay in lockstep; `reset()` clears both
   (the per-build reset contract is unchanged). Removes 59.8M FFI
   crossings per self-check.
2. `_mirror_setattr` restructured: suppression window
   (`_construction` / `_in_serialize`) and non-family writes take a
   single `_ORIG_SETATTR` fast path; `_rules_ok` is inlined into the
   `write` wrapper and deleted.
3. `_count` is a guard-only no-op unless audit mode is on (21.7M dict
   updates removed from default runs). `activate(audit=True)` now
   enables counters retroactively so one-shot activation order cannot
   starve test assertions.
4. Mutation-time registration is skipped for unregistered objects that
   no stored blob embeds (`id(self) not in _HIDDEN_EMBED`). Nothing
   stored derives through them, so their first funnel snapshots the
   mutated state. This removed all 5.58M `strike_gag_uncaptured`
   interactions, the 1.92M write-funnel strike skips, 0.61M
   `register_fail_uncaptured` and 0.77M failed setattr serializations;
   the strikes remain for hidden embeds, which is where they are
   load-bearing.

Measured audit deltas (pre -> post): `strike_gag_uncaptured`
5,581,661 -> 0; `register_fail_uncaptured` 606,631 -> 0;
`unserializable.*` at setattr 769,086 -> 0; `setattr_untracked.*`
0 -> 6,249,772 (fast path); adoptions at write funnels 774,051 ->
1,482,626 (registrations moved from mutation time to first funnel).

Rejected and reverted in the same wave: lazy recursive child
registration. It removes the eager child blobs, but the child's own
`write` (called by `write_type_list` inside the parent's funnel)
re-adopts it immediately, so the child is serialized twice per parent
funnel instead of once. Net wall regressed and three adoption tests
failed; the experiment was reverted.

## 5. Decision record: phase-scoped capture vs ADR-0004 proxy vs opt-in tool

- **Phase-scoped capture (deferred).** Gate capture on the existing
  checking-phase signal (`mypy.types._type_wire_cache_enabled`) and
  invalidate reads at phase switches. This inherits a proven boundary,
  but it needs hooks in `mypy/types.py` plus a read-invalidation path,
  and the F2 read flip is already incorrect for in-place list mutations
  (#1530), so widening its reach now is the wrong order. The semanal
  share of the capture cost was not separately measured; the cProfile
  run is phase-agnostic. Revisit after #1530.
- **ADR-0004 Rust-owned proxy (the graduation path).** Per-scope
  shadows remove the per-call re-serialization and the per-object FFI,
  which is exactly where the remaining +134s live. It is design-only
  with an open mutator registry and identity/memory risks; multi-wave.
  This is the only option that can plausibly reach the within-10% gate.
- **Opt-in audit tool (chosen for now).** The read flip alone is within
  noise (brief #1520); the value of capture today is the F1 byte-identity
  proof, not production speed. Keep capture/re flip opt-in, keep the new
  CI capture gates, and do not default-on (P4) until #1530 is fixed and
  a proxy slice (or a measured phase-scoped capture) lands.

Follow-up: #1530 (read flip stale blobs after in-place list mutations);
the P4 default-on flip stays blocked on it by design.

## 6. F3 close-out: `Instance.extra_attrs` splice

`_FLIP_FIELDS` gains `extra_attrs`, and
`rust_mirror_patch_instance_extra_attrs` decodes one `ExtraAttrs.write`
record (`EXTRA_ATTRS` ... `END_TAG`) or `None` and replaces the stored
slot. The decoded record's exact bytes are preserved (`ExtraAttrs.raw`,
new in `wire.rs`) because Python's `write_type_map` iterates dict
insertion order while a `HashMap` cannot; without it any splice on a
multi-key record would reorder keys and drift from `_fresh_bytes` (a
latent bug the existing Instance ops had too). `NativeInstanceWriteSuite`
gains three cases (multi-key round trip, noop, gate-off full path) and
a field-preservation case that interleaves `args` / `last_known_value`.

## 7. CI coverage

`native-kernel-parity.yml` gains a `parity-mirror` job:
capture-only `testtypes`, capture+read `testcheck` (the known #1530
cases deselected: `testVariadicStarArgsCallNoCrash` on macOS,
`testRevealBoundParamSpecArgs` on linux), and capture+read fine-grained.
`mypy/types_mirror.py` and `mypy/types.py` join the path filters. This is
the first CI the mirror env has ever had; it immediately caught #1530,
which was invisible on main.

## 8. Wave 65A: capture overhead cuts (issue #1539)

Re-run of section 3's method at `7a2b2f850` (post-wave64), then four
opt-in-path cuts, all pinned by the mirror suites.

### 8.1 Profile ranking (before cuts; cProfile, audit-on capture, 428.3s)

| function | tottime | cumtime | calls |
|---|---|---|---|
| `rust_mirror_walk_indices` | 39.9s | 39.9s | 2.75M |
| `_mirror_setattr` | 26.8s | 32.5s | 100.5M |
| `_child_types` (+`_child_types_in_value`) | 11.1s (6.4s) | 23.2s (9.0s) | 4.94M (24.5M) |
| `_register_tree` | 9.2s | 105.7s | 3.5M |
| `_assert_fresh` | 6.2s | 141.9s | 7.7M |
| write wrapper (`types_mirror.py:1154`) | 5.6s | 166.8s | 18.0M |
| `_fresh_bytes` | 4.9s | 36.3s | 2.9M |
| `rust_mirror_register` | n/a | 3.4s | 2.29M |

The shape is unchanged from section 3: adoption (`_register_tree`: one
Rust index walk plus one Python `_child_types` scan per adopted object)
and the funnel (`_assert_fresh` per `Type.write`) dominate; the setattr
wrapper is the third block.

### 8.2 Cuts landed

1. **Walk fixed cost.** `mypy.types` class context and per-class
   `__slots__` names are cached thread-locally in Rust (strong type pin;
   `rust_mirror_reset` clears both), slot reads borrow instead of
   increfing, and container dispatch runs before the tvid/Type/name
   probes. Micro: AnyType 2.38 -> 0.38us, 2-arg Instance 4.63 -> 3.75us,
   Callable 13.3 -> 5.4us per walk.
2. **Registration walk fusion.** New `rust_mirror_walk_registration`
   returns the direct family children (in `_child_types` order) with the
   index lists, so `_register_tree` does one Rust descent instead of a
   Python child scan plus a Rust index walk. The pure-Python
   `_walk_registration_py` body keeps the deferral/differential contract.
3. **Hot-path guards (clean runs).** `_audit_mode` gates the funnel
   f-string counters (`assert_skip`/`adopt`/`untracked`), `_handle_of` is
   inlined in `_assert_fresh`, and the empty-strike / empty-pending calls
   are skipped. Audit-mode counters are unchanged (delta < 0.03% pre/post,
   all run-to-run volume drift; zero mismatches).
4. **Import hoisting.** `WriteBuffer`, `write_type_list`, and the
   `mypy.types` module handle moved out of the four per-serialization
   helpers (2.9M calls).

### 8.3 Wall clock A/B (cold self-check, `-n0 --no-incremental`, 347 files)

Interleaved same-harness runs; audit off.

| config | runs (s) | median | vs off |
|---|---|---|---|
| mirror off | 97.1 / 92.6 / 82.9 | 92.6 | baseline |
| capture (wave 63A, section 2) | 217.4 | 217.4 | +125.5 |
| capture (wave 65A) | 174.4 / 158.2 / 172.6 / 170.0 | 171.3 | +78.7 |
| capture audit-on (wave 63A) | 226.0 | 226.0 | +134.1 |
| capture audit-on (wave 65A, under load) | 198.0 | 198.0 | +105.4 |

Capture overhead fell +125.5s -> +78.7s (-37%), capture wall -21%
(217.4 -> 171.3), ratio 2.37x -> 1.85x. The within-10% cold-path gate
(`docs/remaining-migration-plan.md:707`) would need capture <= ~102s; the
~69s residual is fixed per-object adoption/serialization cost (2.3M
registrations and 2.9M fresh serializations per self-check, both
semantics-bound by the F1 proof), not seam overhead. The section 5
decision stands: capture/read stay opt-in until an ADR-0004 proxy slice
(or a measured phase-scoped capture) removes the per-object work; no
further hot-path micro-optimization is queued.
