# ADR-0001: Phase E1 unlock for visit_* porting

- Status: Proposed
- Date: 2026-08-26
- Issue: #624 (meta: Phase E1 unlock for visit_* porting to reach 50pct Rust)
- Source: synthesis of five read-only investigation briefs over the current tree

## Context

The path to 50% Rust requires porting `visit_*` bodies on `checker.py`,
`checkexpr.py`, `semanal.py`, and `build.py`. Each of those bodies holds
`TypeInfo` and `TypeAlias` references across the GIL. Phase E1 in
`docs/rust-migration-strangler.md` excluded this path until a Rust-owned
Type/Node design lands. This record settles the four decisions itemized in
issue #624, so body-move work can start.

Two facts already settled, recorded here so they are not reopened:

- `is_subtype` is already ported and production-wired default-on
  (`mypy/subtypes.py` `_native_subtype_active` / `_set_native_subtype_resolver`,
  `mypy/build.py:1521`, `crates/type_kernel/src/subtypes.rs` `rust_is_subtype`).
  The earlier "Phase E1 = is_subtype port on `wire::read_type_to_str`" premise
  was stale.
- The C3 MRO linearization is already the canonical pure seam
  (`mypy/mro.py:100` -> `type_kernel::rust_linearize_hierarchy`).

The four decisions that follow are about the *remaining* visit_* bodies.

## Decision 1: TypeInfo storage model

**Decision: reflect-into-Python-on-write.** Keep `TypeInfo` as the live Python
object. Rust reads and writes fields over the GIL via PyO3, and groups reads
into the existing frozen `TypeInfoSnapshot` keyed by `fullname`, with the
`live_info_map` -> `live_typeinfo` transient-GIL escape hatch for fields that
go stale. Do not build an arena of Rust-owned `TypeInfo` objects.

**Rejected alternative:** arena-of-Rust-objects.

**Evidence:**

- The blocker fields carry live Python object identity and are mutated by
  Python at runtime: `defn` (shared AST, plugin-reachable), `names` (the core
  shared mutable `SymbolTable`, the fine-grained merge target, and the primary
  plugin mutation surface via `add_method_to_class` / `add_attribute_to_class`
  and direct `info.names[...] =`), `mro` and `bases` (cross-reference the whole
  class graph by identity; `has_base` / `get` / `protocol_members` depend on
  it), `declared_metaclass` / `metaclass_type` / `_promote` / `alt_promote` /
  `tuple_type` / `typeddict_type` / `self_type` / `special_alias` (live `Type`
  and `TypeAlias` nodes that recurse into `TypeInfo`), `type_object_type` (lazy
  cache mutated during checking), `assuming` / `assuming_proper` / `inferring`
  (live `Instance` stacks transiently append/popped and used as recursion
  guards), `metadata` (plugin-extensible dict), `default_depends` and
  `typeddict_data` (live `TypeInfo` / `TypeAlias` references). See the field
  inventory in the #624 comment thread.
- The `get_customize_class_mro_hook` and `ClassDefContext` contracts let
  plugins rewrite `info.bases` / `info.mro` / `info.metaclass_type` / generated
  members *in place* (`mypy/semanal.py:3263-3265`, `mypy/plugins/plugin.py:511-514`).
- Only a minority of fields are plain data (`module_name`, `_fullname`,
  `type_vars`, `abstract_attributes`, `deletable_attributes`, `slots`,
  `deprecated`, the bool flags, `bad_mro`, `has_param_spec_type`, the
  TypeVarTuple ints), and they are co-mutated in the same semanal functions as
  the live-object fields (`configure_base_classes` writes `bases` and
  `fallback_to_any` together; `recalculate_metaclass` writes
  `declared_metaclass`, `metaclass_type`, `is_enum` together). Splitting them
  adds a write-through sync seam at every mutation site, including plugin hooks.
- The crate already implements this pattern: `TypeInfoSnapshot`
  (`crates/type_kernel/src/typeinfo.rs:32-136`, populated at `:1135-1213`) is a
  frozen map keyed by `fullname` (`:200-205`), and `live_info_map`
  (`Option<PyObject>` holding `dict[str, TypeInfo]`, `:963`) is read back via
  `live_typeinfo` (`:1323-1331`) for stale enum-member reads. There is no
  raw-pointer or `Py<PyAny>` retention of individual `TypeInfo` objects; "arena"
  appears nowhere in `docs/rust-migration-strangler.md`.

## Decision 2: Visitor dispatch

**Decision: classifier-style categorical tag.** Each visit_* seam is one
`#[pyfunction]` returning `Option<...>` where `None` means "defer to the
pure-Python visitor". Rust computes a categorical tag from scalar/live facts
and returns it; the Python shim applies AST mutation, error reporting, and
plugin side effects. Route among object kinds with a `match` on the live
class-name string or `is_instance(obj, refs.X)` blocks.

**Rejected alternative:** one giant inlined `match` over the live Python object
graph, or a numeric kind/tag the Python shim pre-computes for a `Type` visit.

**Evidence:** the crate already splits dispatch along the seam boundary
(`crates/type_kernel/src/`):

- Wire-serialized pure computation uses a flat `match` on the `Type` enum
  (`wire.rs:1150`, `visitor.rs:86`, `subtypes.rs:1508`). This stays the idiom
  for byte-in/byte-out kernels.
- Live-object classification uses classifier fns returning tags: `semanal_visitor.rs:657`
  (`rust_classify_decorators`, `Option<Vec<String>>`) and `:906`
  (`rust_classify_member_resolution`, `(Option<String>, Option<Py<PyAny>>)`);
  `subtypes.rs:146` (guard-chain `Option<bool>`); `serverdeps.rs:116`
  (`is_instance(obj, refs.X)` chain with `DeferError`): live-object walking
  uses categorical-per-kind, not a giant match. `match` on the live class-name
  string already exists at `treetransform.rs:434`.

## Decision 3: Vertical slice

**Decision: `mypy/semanal.py:analyze_class_decorator_common` (lines 2741-2752),
not `visit_class_def`.** Rust returns one tag from the pure name-set membership
test over `FINAL` / `DISJOINT` / `TYPE_CHECK_ONLY` / `deprecated` decorators;
the Python shim applies the three side effects (`info.is_final`,
`info.is_disjoint_base`, `info.is_type_check_only`, `info.deprecated`) and the
two `fail`s (`:2745` protocol-disjoint, `:2747` TypedDict-disjoint).

**Why not visit_class_def:** `visit_class_def` as a whole is not a viable
slice. Its pure computation and side effects share basic blocks: a single
`isinstance` chain in `configure_base_classes` (`:3163-3186`) both classifies a
base and writes `info.fallback_to_any` / `info.bases` and emits `fail`s; the
same interleaving appears in `clean_up_bases_and_infer_type_variables`
(`:2755-2865`). Porting the function whole would mean either violating the
"Python applies side effects" split or re-implementing the AST and
symbol-table mutation in Rust. The pure core already ported (C3 MRO) shows the
shape the remaining work should take: narrow decision heads, not whole visitors.

`analyze_class_decorator_common` is the cleanest *unported* decision head. It
mirrors the existing seams `_rust_classify_setup_type_vars`
(`semanal.py:2485-2515`) and `_native_base_classification`
(`semanal.py:2906-2930`), which already return a branch tag and let Python
apply effects. `visit_class_def` should be reached by decomposing it into such
heads first, not by moving the function body as a unit.

## Decision 4: Parity test surface (before any body move)

The standard, mirrored from the existing `Native*Suite`s in
`mypy/test/testtypes.py` and `mypy/test/testsubtypes.py`:

- Direct seam call: one test calls `type_kernel.rust_<seam>(...)` directly and
  asserts a non-`None` decided result (`test_seam_engages`).
- Gate-on/off differential: run the public Python function with the module-level
  active flag `False` (pure-Python oracle) and `True` (Rust path) via a
  `_par` / `_with_gate` helper in `setUp`/`tearDown`, asserting the two agree.
- Equality and error-message parity: `assert_equal`/`==` on the object, `str()`
  on the rendered type, and captured fail/note text for error-bearing seams,
  plus a deferral-audit test asserting an undecidable case returns `None` and
  the gated shim falls through to Python.
- Rust unit test on the pure core: a `#[cfg(test)] mod tests { ... }` block in
  the new `.rs` file builds wire `Type` enum literals and `assert_eq!` /
  `assert!(...is_none())` with no PyO3 and no resolver, mirroring
  `typeops.rs:2561`, `typeanal_special.rs`, `subtypes.rs:2940`.

A concrete slice spec for `analyze_class_decorator_common`: a
`NativeClassDecoratorCommonSuite` in `testtypes.py` decorated
`@skipUnless(_NATIVE_WIRE_ENABLED, ...)`, `setUp` installing resolver + wire
map + `_set_native_<module>_active(True)`, a per-tag `assert_par`, a
`test_seam_engages`, one `test_..._defers`, and a Rust `mod tests` exercising
the tag classifier on wire values.

## Consequences

- Model: no Rust-owned `TypeInfo` arena; the existing snapshot + live-map
  pattern is the seam for every future visit_* body.
- Dispatch: classifier-style tags with `Option`/`None` defer; no giant matches
  and no numeric kind for `Type` visits.
- Slice: first body to move is `analyze_class_decorator_common`, followed by a
  decomposition of `visit_class_def` into further decision heads.
- Parity: the four-movement standard applies unchanged; the parity surface must
  be written before the body moves.
- Gap to 50%: Decision 1 removes the arena prerequisite that blocked #624, and
  Decision 3 names a concrete first seam. The remaining blocks are by design:
  plugin hooks, error emission, and live-object state stay in Python.
