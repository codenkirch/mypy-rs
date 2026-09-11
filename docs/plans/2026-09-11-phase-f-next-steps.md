# Phase F next steps (scoping brief, wave 62-E / issue #1520)

Date: 2026-09-11. Basis: read-only audit at `66e709794` (pre-wave-62 main)
plus runtime probes (cold self-check profiles, mirror A/B, weakref probes,
wheel inspection). The orchestrator's persisted summary of the brief.

## 1. F3 status: complete as implemented; no splice-op continuation

- Instance splice covers `args`, `type` ref, `last_known_value`
  (`mypy/types_mirror.py:122`, `crates/type_kernel/src/mirror.rs:447`);
  CallableType covers 11 wire fields + 7 flags (`types_mirror.py:136`).
  UnionType moved its plain data to `SKIP_ATTRS` (#1407).
- The tvar/union splice slice stays EMPTY. Mirror-report evidence
  (cold self-check, mirror + read on): `tvar.default` 18 changed writes,
  `instance.extra_attrs` 287, `instance.args` 263, `callable.variables` 18,
  `union.*` 0. The 200k-level `copy_modified` traffic is construction,
  which post-setattr splicing cannot serve (needs view construction).
- The only wire-visible gap is `Instance.extra_attrs` (~287 changed
  writes/run, no splice op). Treat as an F3 close-out item.
- Caveat: `misc/f3s9_tvar_union.py`'s `__setattr__` hook is dead after
  `types_mirror.activate()` overwrites the classes' `__setattr__`
  (`types_mirror.py:1430`); its per-field table is misleading. Read the
  mirror report instead.

## 2. F4: the cache writer is the wrong first slice

- Measured cost split (cold `-p mypy`, instrumented): module data write
  `MypyFile.write` total 0.480s for 582 modules; type writes during
  kernel seams 9,291,453 calls / 15.44s. The cache payload is ~0.5% of
  the work F2/F4 target.
- The node data payload (`tree.write` / `MypyFile.read` + fixup) is
  Phase G by the HANDOFF's own rule; `remaining-migration-plan.md:694`
  ("cache writes serialize from Rust") contradicts that and should be
  corrected.
- Blocker: the installed PyPI `librt` wheel has no `write_raw_bytes`, so
  the wire cache splice path is inert and its suite self-skips
  (`mypy/types.py:29-34`, `testtypes.py:50224`). A Rust cache writer
  needs the fork wheel or manual raw-append.
- Recommended first F4 slice instead: graduate the already-landed F2
  read flip and cut capture overhead (see P1-P4 below). Mirror capture
  measured 2.5-2.6x cold wall (100.1s off vs 263.3/248.5s capture) versus
  the plan's within-10% gate; the read flip itself is within noise, so
  the cost is capture/cascade machinery.

## 3. Identity service

- `identity.rs:90 handle_for` (raw) is used by the mirror only;
  `handle_for_stable` / `handle_of_stable` (`identity.rs:176/202`) have
  zero production callers.
- Blocker: `Instance`, `TypeVarType`, `UnionType` (also `TupleType`,
  `LiteralType`, `TypeAliasType`) cannot create weakrefs
  (`TypeError: cannot create weak reference`); only `CallableType` is
  weakref-able. Daemon-stable handles need `__weakref__` in those slots
  or a strong-pin retire protocol.
- Highest-risk identity-keyed sites to replace: `_type_wire_cache`
  (`mypy/types.py:131` + 7 per-module copies), the mirror's `id()`-keyed
  graph, `_metaclass_memo` (`nodes.py:4285`), `_serialize_ast_cache`
  (`traverser.py:171`), `join.py` `seen_instances`, `subtypes.py`
  `assuming`/`assuming_proper` stacks.

## 4. Risk-register deltas

- No CI coverage for the mirror: `TEST_NATIVE_TYPE_MIRROR` appears in no
  workflow; `mypy/test/helpers.py:472-475` is the only gate.
- mypyc: `types_mirror.activate` patches `cls.__init__/write/__setattr__`
  unconditionally (`types_mirror.py:1425-1430`); a mypyc-compiled mypy
  would raise, not degrade.
- Daemon: fine-grained skips the module-data cache
  (`build.py:4723-4729`), so an F4 cache writer has zero daemon effect.
- ADR-0004 (design-only) and the plan text disagree with the code on the
  F3 model (shadow, never replacement); use the code as truth.

## 5. Recommended PR sequence

1. **F4.0 mirror graduation audit** (P1): profile mirror-on cost with
   cProfile; A/B read-flip on/off; decide phase-scoped capture vs
   ADR-0004 proxy vs opt-in tool. No behavior change.
2. **F4.1 CI mirror coverage + F3 close-out** (P2): add
   `TEST_NATIVE_TYPE_MIRROR=1` (+ `_READ`) to `native-kernel-parity.yml`;
   add the `Instance.extra_attrs` splice op; extend the suite.
3. **F4.2 cut capture overhead** (P3): skip hook work for unregisterable
   objects and scope capture to phases where reads happen; target <=10%
   wall on the P1 harness.
4. **F4.3 default-on F2 read flip** (P4): flip `native_type_mirror` /
   `native_type_mirror_read` defaults with fallback intact; update the
   plan's claim ladder; re-run work share.
5. **F4.4 daemon-stable handles** (P5): wire `handle_for_stable`, add
   `__weakref__` or the strong-pin retire protocol, split reset, add a
   dmypy recheck identity test.

The cache type-blob writer stays deferred pending the `librt` decision.
