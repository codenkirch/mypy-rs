# Phase E1 Unlock: visit_* Porting to Reach 50% Rust

Issue: #624
Date: 2026-08-21
Status: Draft for review

## Problem

The strangler seams for pure helpers are exhausted. Every remaining
high-value port is a `visit_*` method on `mypy/checker.py`,
`mypy/checkexpr.py`, `mypy/semanal.py`, or `mypy/build.py` that must
read or mutate `TypeInfo` / `TypeAlias` object-graph references across
the GIL. The current PyO3 wire-format seams serialize a `Type` subgraph
to bytes, decode it, and never hold graph refs. That boundary is the
Phase E1 constraint recorded in `docs/rust-migration-strangler.md`.

## Investigation Findings (2026-08-21)

### What already ships (verified at origin/main 502201f)

- `write_type` encoder (wire.rs:1997) including `INSTANCE_GENERIC`
  with args (line 2048). The "no encoder" comments in older code
  are stale.
- `visit_instance_meet_args` (setops.rs:1153): same-type-with-args
  meet, per-arg recursive meet, `SetOpResult::Encoded`.
- `visit_instance_with_args` (setops.rs:3220): same-type-with-args
  join, per-arg variance dispatch, `SameTypeWithArgs`.
- `make_simplified_union` (setops.rs:3376): full union flatten,
  dedup, literal contraction, `union_make_union`.
- `meet_union` (setops.rs:771): pairwise meet + simplify + encode.
- `visit_callable_fallback` (setops.rs:2705): non-protocol fallback
  join + encode.

### What genuinely defers (E1-blocked, not portable today)

Every remaining `return None` in `setops.rs` (71 sites) falls into one
of these categories, all requiring live Python graph data:

1. **TypeAliasType** (setops.rs:3056, expandtype.rs:65/104/164): wire
   format carries only `type_ref` (a fullname string), not the live
   `TypeAlias` node. Python's `_expand_once` / `is_recursive` dereference
   `alias.target`, which asserts on None. Fixing requires resolving
   `type_ref` to a live `TypeAlias` via the typeinfo_map before the
   walk, then passing it into Rust.

2. **Protocol instances** (setops.rs:2715): `unpack_callback_proxy`
   needs the live `TypeInfo.is_protocol` flag (available on the
   snapshot) AND `unpack_callback_protocol(t)` which walks the live
   `TypeInfo.names` symbol table to find `__call__`. The symbol table
   is not in the wire snapshot.

3. **TypeVarTuple / ParamSpec variadic** (setops.rs:3229,
   expandtype.rs:169): needs `split_with_prefix_and_suffix` which
   requires `type_var_tuple_prefix` / `_suffix` (on the snapshot) but
   also constructs `TupleType` with a `tuple_fallback` that is a live
   `TypeInfo` reference.

4. **Promote lists** (setops.rs:4056, join.py:263-270): Python's
   `join_instances_via_supertype` iterates `t.type._promote` and
   `s.type._promote` (lists of live `Instance` objects). The snapshot
   has no `_promote` field.

5. **fixup_wire_type** (wirefixup.py:55, 13 call sites): the
   `_TypeRefFixer` walks the decoded Python `Type` object graph and
   mutates in-place (`t.type = typeinfo_map[ref]`, `t.type_ref = None`).
   The walk cost is the ~2.2s overhead named in #606. The mutation is
   the E1-hostile surface.

### Files with zero Rust dispatch (all AST NodeVisitor modules)

`treetransform.py` (820 lines), `strconv.py` (707 lines),
`visitor.py` (639 lines), `type_visitor.py` (630 lines): all walk
`mypy.nodes` (the AST), explicitly excluded by the migration plan's
"do not start by porting mypy.nodes" constraint. These are E1-blocked.

## Decision 1: Storage Model

### Recommended: Reflect-into-Python-on-write (borrow + compute + write back)

Rust borrows the live Python `TypeInfo` / `TypeAlias` objects via
PyO3 `PyAny` references, reads their fields through `getattr` (as
the existing `NativeTypeResolver` snapshot builder already does),
computes the result in Rust, and writes the result back through
Python attribute assignment. The Python object graph stays the source
of truth; Rust is a compute-across-the-GIL co-processor.

### Rejected: Arena-of-Rust-objects

Rust owns the `TypeInfo` / `Type` graph as native structs. Python
reads through FFI accessors. This is the "pure" end state but implies
re-architecting how plugins (which iterate `TypeInfo.names` directly),
the daemon (which holds `TypeInfo` across incremental rechecks), and
cache serialization (which serializes `TypeInfo` to `.meta`/`.data`)
all interact with the graph. The compat surface is massive: every
plugin, every daemon path, every cache reader. Not reachable without
a multi-release migration that breaks plugins.

### Why reflect wins

- **Daemon / fine-grained**: `TypeInfo` objects persist across
  rechecks. Rust-arena would need to keep Rust objects alive across
  GIL releases and re-checks, with sync back to Python. Reflect
  keeps Python objects alive as-is; Rust re-reads via `getattr` on
  each call (or caches the snapshot, as the resolver already does).
- **Plugin-visible mutation**: plugins call
  `TypeInfo.names[...].type = ...` directly. Reflect keeps this
  working (Python objects mutate, Rust re-reads). Arena would need
  write-through from Python to Rust, or a copy-on-write fence.
- **Cache serialization**: `fixup.py` / `cache.py` serialize the
  Python `TypeInfo` graph. Reflect is invisible to this path (Rust
  never owns the graph). Arena would need a Rust-to-Python
  serialization bridge.
- **Incremental risk**: reflect is additive (Rust functions return
  results, Python applies them). Arena is a replacement (Rust owns
  the graph, Python borrows). Additive is the strangler-fig contract.

### Cost of reflect

- `getattr` overhead per field access (already paid by the
  `NativeTypeResolver` snapshot builder; amortized by per-SCC
  snapshot caching).
- Rust can't hold graph refs across GIL releases (must snapshot
  per call, like the resolver already does). The snapshot is the
  pattern that works.

## Decision 2: Visitor Dispatch

### Recommended: Match-based dispatch on wire Type enum

The existing `Type` enum in `wire.rs` (line 461) already has one
variant per `mypy.types` subclass. The visitor dispatch is a `match`
on the `Type` variant, same as `is_subtype`, `join_types`,
`meet_types` already do. No categorical-per-kind layer needed; the
match is the dispatch.

### Rejected: Categorical per kind

Group variants into categories (e.g. "FunctionLike" = CallableType +
Overloaded, "Instance-like" = Instance + LiteralType + TypeType).
Adds an indirection layer with no benefit: the existing ports already
match on the variant directly, and the match arms are small enough
that categorization would just move code without reducing complexity.

## Decision 3: Vertical Slice

### Recommended: `fixup_wire_type` native fast-path

Not a `visit_*` method on checker/semanal, but the highest-frequency
graph-touching operation in the kernel. It runs on every `disc=7`
(Encoded) result from join/meet/subtypes/expand (13 call sites). The
~2.2s overhead (#606) is the single largest perf lever available.

### What to port

The `_TypeRefFixer` walk: recursively visit every `Type` node,
collect `Instance.type_ref` and `TypeAliasType.type_ref` strings,
verify each against the `typeinfo_map`. If all resolve, return the
list of `(instance_id, type_ref)` pairs so Python can do the in-place
mutations. If any is missing, signal defer.

### Why fixup, not semanal.visit_class_def

- **Frequency**: fixup runs on every kernel result (thousands per
  SCC x 394 SCCs). `visit_class_def` runs once per class definition
  (~1500 classes in the self-check).
- **Mutation surface**: fixup mutates `Instance.type` and
  `Instance.type_ref` (two fields, always the same two). The graph
  mutation is shallow and predictable.
  `visit_class_def` mutates the symbol table, MRO, type_vars,
  base_classes, decorators, and writes AST nodes. Deep, unpredictable
  mutation surface.
- **Proof of concept**: if Rust can drive the fixup walk (the
  hottest graph-touching path), the same pattern (Rust scans,
  Python mutates) generalizes to any `visit_*` method. If it can't,
  the E1 design needs rethinking before touching the deeper visitors.
- **Measurable**: the ~2.2s overhead is a concrete before/after
  benchmark target. `visit_class_def` has no isolated perf signal.

### Implementation approach

1. Add `rust_collect_type_refs(bytes: &[u8]) -> Option<Vec<(usize,
   String)>>` to `crates/type_kernel/src/wire.rs`: walks the wire
   bytes (no `Type` tree allocation) and returns every
   `Instance.type_ref` / `TypeAliasType.type_ref` found, keyed by
   position index. Returns None on malformed bytes.

2. Add `rust_verify_refs(bytes: &[u8], typeinfo_keys: Vec<String>)
   -> bool` to the kernel: calls `collect_type_refs`, checks every
   ref against the key set, returns `true` if all resolve.

3. In `mypy/wirefixup.py`, add a fast path: if
   `rust_verify_refs(bytes, list(typeinfo_map.keys()))` returns
   `true`, proceed with the in-place mutations (which now only
   touch the verified refs, no missing flag checks needed). If
   `false` or `None`, fall back to the existing Python walk.

4. The in-place mutations stay in Python (E1-safe: Rust never
   touches the live graph). The expensive recursive walk (the
   `TypeTranslator.accept()` dispatch through every node) moves
   to Rust.

### What this proves for E1

- Rust can scan wire bytes and extract graph-relevant metadata
  (type_refs) without allocating a full `Type` tree.
- Python can apply mutations based on Rust-produced instructions
  (the ref list), keeping the graph mutation in Python.
- The pattern (Rust scans + verifies, Python mutates) is the
  building block for deeper visitors: `visit_class_def` would
  follow the same split (Rust computes scope/MRO/typevar setup,
  Python applies the mutations).

### What this does NOT prove

- Whether Rust can hold `TypeInfo` references across the GIL for
  read access (the resolver snapshot already does this via
  `getattr`, but a `visit_*` method needs more fields than the
  snapshot carries). That's a follow-up: extend the snapshot or
  add a per-call `getattr`-based reader.

## Decision 4: Parity Test Surface

### Required suites (must pass unchanged)

1. `mypy/test/testtypes.py` (all `Native*Suite` classes): the
   differential harness that runs both Rust and Python paths under
   `TEST_NATIVE_TYPE_KERNEL=1`.
2. `mypy/test/testcheck.py`: full checker parity (8144+ tests).
3. `mypy/test/testfinegrained.py` + `testfinegrainedcache.py` +
   `testdaemon.py`: daemon/incremental (1333 tests).
4. Self-check: `mypy_self_check.ini --no-incremental -p mypy` with
   0 errors (kernel-on vs kernel-off delta must not widen).

### Differential harness pattern

Follow the existing `TEST_NATIVE_TYPE_KERNEL` gate (helpers.py):
the test harness overrides `Options.native_type_kernel` after option
parsing. The fixup fast path is gated by the same option, so
`=1` exercises the native fixup, `=0` exercises the Python-only path.

### New test: fixup fast-path parity

Add `NativeFixupWireTypeSuite` to `testtypes.py`:
- Encode a type with known `type_ref` strings, call
  `rust_verify_refs`, assert `true`.
- Encode with a bogus `type_ref`, assert `false`.
- Compare: for every `Native*Suite` test case that produces
  `disc=7` (Encoded), the decoded result with the fast path must
  equal the decoded result without it (byte-for-byte type equality).

## Phased Execution

### Phase 0: `rust_collect_type_refs` (this PR)

- Add the wire-bytes scanner to `wire.rs`.
- Add `rust_verify_refs` PyO3 entry point.
- Unit tests in Rust (round-trip known types, verify ref extraction).
- No Python-side wiring yet.

### Phase 1: Wire the fast path (follow-up PR)

- Add the fast path to `wirefixup.py:fixup_wire_type`.
- Add `NativeFixupWireTypeSuite` to `testtypes.py`.
- Run full parity: testtypes, testcheck, fine-grained, self-check.
- Benchmark: measure `type_check_time` before/after.

### Phase 2: Extend the snapshot (follow-up)

- If Phase 1 shows the scan+verify pattern works, extend the
  `TypeInfoSnapshot` with fields needed by deeper visitors
  (`_promote` lists, `names` symbol-table keys, `is_protocol` +
  `__call__` presence).
- This unlocks protocol fallback and promote-list deferral
  reduction in setops.rs (the two largest buckets after
  TypeAliasType).

### Phase 3: First `visit_*` vertical slice (follow-up)

- Pick the shallowest checker or semanal visitor (candidate:
  `checker.visit_try_stmt` or `semanal.visit_global_decl`).
- Apply the same scan+mutate split: Rust computes the type
  decisions, Python applies the AST mutations.
- Validate against the full parity suite.

## Verification Gate (each phase)

1. `cargo test -p mypy-type-kernel --lib` (1333+ passed,
   3 pre-existing treetransform env failures OK).
2. `cargo fmt --check && cargo clippy` clean.
3. `testtypes.py -k Native` ON vs OFF identical.
4. `testcheck.py` full suite (8144+ passed).
5. Self-check 0 errors.
6. Work-share measurement (Phase 1+): `scripts/measure_work_share.py
   --pairs 3`.
