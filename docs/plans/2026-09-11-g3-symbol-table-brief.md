# G3.0 symbol-table accessor brief (wave 71C / issue #1578)

Date: 2026-09-11. Basis: read-only audit after the F close-out. Persisted
by the orchestrator from the wave-71C brief.

## Corrections to the G0 brief's G3 assumptions

1. The "queued `class_type` closures (semanal.py:9572)" risk is wrong:
   that line is `schedule_patch`; the only queued closures are the two
   `calculate_tuple_fallback` lambdas (semanal.py:3708,
   semanal_namedtuple.py:508-510) which mutate *types*, not symbol
   tables, and already execute through Rust. G3 has no patch-queue work.
2. The G0 brief's "daemon-stable handles blocked on #1528" delta is
   stale: #1528 landed (strong pins, shared namespace, preserving reset),
   so G3 can key records by `identity::handle_for` today.
3. The placeholder *replacement* funnel is the ordinary dict put in
   `add_symbol_table_node` (semanal.py:8744-8817, put at 8813), not the
   `mark_incomplete`/`process_placeholder` range (9025-9083).
4. The plan's "views" wording still contradicts ADR-0004; accessors are
   capture/consistency seams over live Python dicts under the shadow
   model.

## Accessor surface (capture face now, Rust-facing face later)

- `SymbolTableAccess.put/delete/lookup/entries`, plus
  `set_ref_flags` and `set_cross_ref` and `rebind_names_table`
  (namespace generation changes on `owner.names = SymbolTable()`).
- `TypeInfoAccess.set_names_entry/delete_names_entry`, `set_info`,
  `set_bases_mro`, `set_meta`, and the mandatory
  `reset_subtype_caches` side effect.
- Capture strategy: class-level patches (`SymbolTable.__setitem__` /
  `__delitem__` / `pop`, `SymbolTableNode.__setattr__`) mirroring the
  G1.0a node hook; accessor calls stay the normative write path so
  coverage is falsifiable.

## Two bypass facts

1. `rust_remove_imported_names_from_symtable` deletes via
   `PyDict::del_item` (semanal_visitor.rs:458) and bypasses a Python
   class patch: reroute through the accessor or record a known-bypass
   counter.
2. Dict-subclass C paths (`SymbolTable.read`/`copy` constructors,
   `dict.update/clear`) bypass overrides; patch `pop` explicitly and
   treat constructor-built tables as an explicit "adopt namespace".

## Lifecycle constraints (record committed writes only)

- Placeholder creation/replacement validations stay in Python; refusals
  leave no shadow trace by construction.
- The put-gate and `progress` signal (semanal.py:8810-8815,
  `final_iteration = not any_progress`) are convergence semantics; do
  not re-derive them in the shadow.
- Namespace rebinds mint a new table generation; per-name records must
  not merge across generations.

## astmerge contract

`replace_object_state` copies slots via `setattr`, so a surviving
identity re-registers through the same capture hook. Skipped fields
(`special_alias`, `setter`) need explicit refresh; abandoned new objects
become stale pins until reset. Re-registration rides the existing
`_refresh_native_wirefixup_maps` boundary (build.py:1991-2036,
update.py:937/1138), not a new one. Never perturb: the new->old map
direction, `merge_asts`'s `node is old` assertion, type/fullname/kind
admission, the `AttributeError` swallow, or the `type_state.reset_*`
calls.

## First slice (G3.0a)

New `crates/type_kernel/src/symtable_mirror.rs` keyed by
`(owner handle, name)` with generation + seq; new
`mypy/symtable_access.py` (`put_names_entry`) and
`mypy/symtables_mirror.py` (class patches); gate
`Options.native_symtable_mirror` default off + `TEST_NATIVE_SYMTABLE_MIRROR`;
reset branch in `_clear_native_resolvers`. Route the semanal adding
funnel (semanal.py:8683-8849, prepare_file writes 1072/1127/1149, the
implicit-attr put 5975) through `put_names_entry`. No read flip.

Acceptance: `TEST_NATIVE_SYMTABLE_MIRROR=1` shows `bypass.put == 0` on
the funnel cluster over testcheck and the cold self-check, and
`shadow.entry_count(owner) == len(owner.names)` after semanal on the
driven corpus; the 11-pin `NativeSymtableMirrorSuite` (placeholders,
refusals, generations, ref flags, deletes, identity, failure-safety,
gate-off).

## Ordered G3 sequence

1. G3.0a entry funnel + capture scaffold.
2. G3.0b direct-write and delete sweep (plugins, synthetic tables,
   checker fake infos, aststrip/update deletes).
3. G3.0c TypeInfoAccess meta fields (set_info/bases-mro/meta,
   fixup + Rust fixup writes).
4. G3.0d astmerge re-registration; only after this gate may a G3.1 read
   flip be designed (first consumer candidate:
   `rust_snapshot_symbol_table`).
