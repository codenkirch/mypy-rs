# ADR-0002: Architecture for the remaining Rust migration (wire walls, seam protocol, E1 backplane)

- Status: Proposed
- Date: 2026-08-28
- Issue: follow-on to #624 / #1121 / #1115 (no single tracking issue yet)
- Follows: ADR-0001 (TypeInfo storage model, visitor dispatch, slice, parity)

## Context

The strangler-fig loop (measure a seam, file the bucket, port the decidable
head, gate on/off parity) is at the end of its curve. Three facts are now
documented and load-bearing:

- The seam-call share (97.9% native on the 2026-08-28 survey) is a *coverage*
  proxy. It has decoupled from the objective. `docs/remaining-migration-plan.md`
  records the last clean wall-clock measure: total work share is negative
  (native slower than pure Python), and its own verdict is that deferral
  shaving "is exhausted as a lever; this is now a perf-fix, not a coverage,
  goal."
- The named near-term levers in that document are (a) incremental
  `_build_native_resolvers` (shipped, #893) and (b) residual per-call wire
  serialize/fixup traffic in Python-side callers. Lever (b) is unshipped.
- Phase E1 (Rust-owned Type/Node) is recorded as multi-quarter and "do not
  start while the strangler per-call gates are still carrying the load."

The user has directed all three remaining effort classes to start now. This
ADR sequences them so they are not pursued as three competing rewrites. They
are a dependency chain, not a menu.

## Decision 1: wire walls are snapshot completeness, not a new wire format

**Decision: do not add the resolved alias target to the wire `TypeAliasType`.
Fix the alias-snapshot completeness and the decode lifecycle instead.**

**Evidence:**

- `TypeAliasType.__slots__ == ("alias", "args", "type_ref")` and its
  `serialize()` asserts `alias is not None` then writes the fullname as
  `type_ref` plus serialized `args` (`mypy/types.py:416-545`). The `alias`
  node (`nodes.TypeAlias` with `.target`, `.no_args`, `.tvar_tuple_index`,
  `.alias_tvars`, `._is_recursive`) is deliberately not serialized: a
  recursive alias cannot be inlined without infinite recursion, and a
  non-recursive one can be resolved from the snapshot.
- The Rust side already mirrors this: `Type::TypeAliasType { args, type_ref }`
  (`crates/type_kernel/src/types_impl.rs:932-939`), expanded through
  `resolver.alias_resolver()` / `expanded_alias_target` (`:332-350`) and the
  wirefixup `set_wire_alias_map` (`mypy/build.py:1554-1555`).
- The actual wall is two gaps, both fixable without touching the wire codec:
  1. Snapshot completeness: `_collect_incremental(scc)` returns the growing
     alias set, but `rust_type_analyze` runs during `semantic_analysis_for_scc`,
     *before* `_build_native_resolvers(scc)` installs that SCC's aliases
     (`mypy/build.py:5681`). Result: the current SCC's aliases are never in the
     map when the seam consumes them (issue #1115).
  2. Chain depth: `resolve_fallback` (`types_impl.rs:332-350`) defers when the
     first expansion is still a `TypeAliasType`, so `A -> B -> int` aliases
     defer even when both snapshots are present.

**Rejected alternative:** inline `.target` on the wire. Rejected because a
recursive alias (`A -> list[A]`) has no finite wire form, and the Python
serializer is shared with the incremental cache format, which must not grow a
lazy, identity-sensitive target field.

## Decision 2: decode lifecycle split (issue #1115)

**Decision: split the resolver side effects so the decode-only map (aliases)
is installed before semanal for the current SCC, and install it in parallel
workers.**

**Evidence:**

- `_build_native_resolvers` conflates two different-time things. The TypeInfo
  resolver snapshot must run *after* semanal (TypeInfos are final only when
  the SCC is sealed). The decode/alias map only needs the alias fullname to
  target-type mapping, which is available once the alias is analyzed. The fix
  is a pre-semanal `set_wire_alias_map` hook driven from `process_stale_scc`,
  keeping the after-semanal `_build_native_resolvers` unchanged.
- Parallel workers (`MYPY_NUM_WORKERS > 0`) never build resolvers
  (`build.py` installer runs in the main process); the shim falls through to
  Python. Worker-side resolver wiring is the second half of #1115.
- Caveat carried forward: the semanal seams are net-negative and now opt-in
  off. This fix raises *coverage*, not wall-clock. Its value is that the alias
  map is the shared prerequisite for the on-by-default alias-bearing seams
  (subtype/join/expand family), which is where the coverage mattered.

## Decision 3: seam protocol redesign (lever b)

**Decision: generalize the existing classifier pattern to remove wire traffic
from the hot seams. For seams whose decision needs only scalars, pass scalars;
for list folds, pass batches. Gate the metric back to `measure_work_share.py`.**

**Evidence:**

- The `rust_classify_*` seams already read scalar/live facts over PyO3 and
  return a tag, no wire blob (`semanal_visitor.rs`, `checker_functions.rs`,
  ADR-0001 Decision 2). These do not pay the per-call serialize cost.
- The net-negative diagnosis is specifically the per-call `encode`/`fixup` of
  a whole type graph on the wire seams. The lever is to extend the scalar pattern
  to the remaining wire seams and to batch the folds (union/join/tuple) so N
  items ride one call.
- Success is wall-clock (`scripts/measure_work_share.py`), not seam-call share.
  Any protocol change must keep the `None`-defers-to-Python contract intact.

## Decision 4: E1 backplane, design-only and gated

**Decision: E1 proceeds as a design line, not a production flip. Two stages,
both behind the existing `native_type_kernel` gate; no removal of the Python
fallback until the wire Type enum round-trips every `mypy.types.Type` variant
and gate-off/gate-on parity is byte-identical for 100% of seams.**

**Evidence:**

- Stage A (wire totality): the Python `read_type` deliberately does not decode
  tag 122 (`ERASED_TYPE`), see the AGENTS.md invariant. A Rust-owned Type that
  must round-trip ErasedType first needs that invariant resolved, plus a
  decision on `TypeAliasType.alias` (stays a resolver key, Decision 1).
- Stage B (proxy model): ADR-0001 already rejects a Rust arena of `TypeInfo`;
  a Rust `Type` proxy must follow the same reflect-into-Python-on-write model,
  holding a `TypeInfo`/`TypeAlias` by identity, not by value.
- The plan's "do not start while strangler gates load" is honored by keeping
  the fallback: this is the terminal constraint, and the seam-protocol work
  (Decision 3) is what removes the per-call overhead that E1-prime would
  replace wholesale. Sequencing E1 behind Decisions 1-3 avoids building the
  proxy twice.

## Consequences

- Execution order: Decision 2 (decode lifecycle, #1115) first, because it is
  the prerequisite for Decision 1's completeness and is already filed; driven
  as a bounded build.py change with daemon/cache parity.
- Then Decision 1's chain-depth and snapshot-completeness close the alias
  deferral category, consumed by the on-by-default subtype/join/expand seams.
- Decision 3 runs in parallel as the wall-clock lever, measured by
  `measure_work_share.py` only.
- Decision 4 Stages A/B remain behind gates and are the terminal line; no
  Python-fallback removal is proposed in any of Decisions 1-3.
