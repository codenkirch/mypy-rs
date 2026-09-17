# G1.2 node-shadow fidelity audit + first expression-node read flip

Issue: #1674. Basis: `506aa7e4c` (`origin/main` at branch point), worktree
`worktrees/mypy-rs-g12`, branch `feature/g12-node-read-flip`. Every number
below was measured against **this worktree's** `mypy/` tree
(`.venv/bin/python -c "import mypy; print(mypy.__file__)"` →
`…/worktrees/mypy-rs-g12/mypy/__init__.py`), not the shared checkout's.
Rebased for the final merge; the last base is `58d32ab24` (#1694). The
generated tables were regenerated there (`--check` clean) and the gate
battery re-run on that tree: cargo `2856 passed / 0 failed / 11 ignored`,
testtypes `4013 passed / 7 skipped` with the flip gates on, self-check
clean (354 files). The measurement sections below were taken pre-rebase.

## Why this audit exists

The G0 brief (`docs/plans/2026-09-11-phase-g0-next-steps.md`) requires a
full-fidelity node enum before any read flip: the Rust side must cover
every field the Python-only paths mutate, not just the subset
`ast_serialize` writes. No table of Python field set vs Rust shadow field
set existed, so a read flip could not be shown correct.

The tables below are **generated** from source by
`misc/g12_node_shadow_audit.py` (Python slots by `ast` over
`mypy/nodes.py`, shadow fields by `ast` over `mypy/nodes_mirror.py` /
`mypy/symtables_mirror.py`, Rust records by a text scan of
`crates/type_kernel/src/node_mirror.rs`). Re-run:

```bash
.venv/bin/python misc/g12_node_shadow_audit.py > docs/plans/2026-09-15-g12-node-shadow-fidelity-audit.md
.venv/bin/python misc/g12_node_shadow_audit.py --check docs/plans/2026-09-15-g12-node-shadow-fidelity-audit.md
```

Verdict vocabulary, per slot:

- **served** — the shadow holds a value a reader can return verbatim.
- **served (kind + G1.1 wire bytes)** — type-valued G1 field; the record
  carries the class name plus the serialized `Type`, so a reader
  reconstructs an equal type.
- **partial (…)**, **partial (marker only)** — the shadow holds a record
  but only a class/fullname marker (or, for `analyzed`, the replacement
  class name), not the value. A read flip over that slot needs the real
  object or its wire form.
- **AST wire (structural)** — no write site outside `mypy/nodes.py`, so
  the field is set once (by the parser or the class's own `__init__`) and
  the AST wire writer owns it, not the analysis-shadow.
- **gap** — some Python path writes the slot outside `mypy/nodes.py` and
  the shadow has no record for it.

Totals over the three shadowed families (213 slot rows):
`served` 76, `served (wire)` 8, `partial (marker only)` 42,
`partial (element/class-name only)` 6, `AST wire (structural)` 41,
`gap` 40.

## Bottom line

1. **G1 expression family.** The five `RefExpr` binding scalars
   (`kind`, `node`, `_fullname`, `is_new_def`, `is_inferred_def`) are
   served exactly. Every G1 *field-map* slot the patch tracks is now
   served, **including `name` (`NameExpr`/`MemberExpr`), which this issue
   closes** (`mypy/nodes_mirror.py:136`, `crates/type_kernel/src/node_mirror.rs:275`).
   Before the closure a served read of `MemberExpr.name` was impossible:
   `NameExpr.__init__` / `MemberExpr.__init__` write the slot before the
   node is adopted, and the lazy-adoption rule (`_is_field_baseline`)
   could not distinguish a constructor write from an analysis write.
   The closure seeds it at adoption (`nodes_mirror.py:_seed_name`) and
   captures later writes for already-adopted nodes, which is what keeps
   `mypy/renaming.py:585` (`expr.name = new_name`) from staling a record.
2. **Residual G1 gaps** (unnamed-and-closed in this PR, see the ledger):
   `MemberExpr.expr`, `IndexExpr.base`/`index`, `CallExpr.args`,
   `OpExpr.right`, `UnaryExpr.expr`, `StrExpr.value` are **child-node
   slots**: serving them needs the child graph, which is G4's AST
   ownership, not a shadow record. `MemberExpr.def_var` and
   `ComparisonExpr.method_types` are served *partially* (target
   fullname / element class names only); closing them needs object or
   multi-blob storage, which is a store extension, not a wire-format
   change.
3. **G2/G3 object-valued fields are markers.** 42 tracked slots return
   `partial (marker only)` — including `Var.type`, `FuncDef.type`,
   `AssignmentStmt.type`, `ClassDef.info`, `TypeInfo.bases`/`mro`/
   `names`/`typeddict_type`, `SymbolTableNode._node`. This is the
   decisive fact for consumer choice below.
4. **Consumer audit.** Of the two candidates in the issue:
   - `rust_snapshot_definition` (`crates/type_kernel/src/astdiff_symbols.rs`)
     is **floored**: its smallest arm (`Var`, `mypy/server/astdiff.py:313-314`)
     reads `node.type` (a live `Type`) and `node.is_final`; `is_final` is
     served but `type` is `partial (marker only)` (`Var.type`,
     `nodes.py:1538`). The `Func` arm adds `node.items`, `node.impl`,
     `first_item.var.setter_type`, `dataclass_transform_spec`; the
     `TypeInfo` arm adds `mro`, `bases`, `_promote`, `defn.type_vars`,
     `metaclass_type`, `tuple_type`, `typeddict_type`. Serving those needs
     object or wire storage for the whole F-phase type graph, which this
     issue's scope forbids (no wire-format, plugin or `CACHE_VERSION`
     change).
   - the **#1635 aststrip surgery** (`mypy/server/aststrip.py` +
     `crates/type_kernel/src/subexpr_strip.rs`) is **covered**: its
     `process_lvalue_in_method` MemberExpr arm reads exactly
     `lvalue.is_new_def` (G1.0a record, served) and `lvalue.name` (the
     gap closed here), then does a class-namespace delete
     (`mypy/symtable_access.py:delete_names_entry`, whose shadow half is
     the G3 store). **This is the consumer this PR flips.**
   - Post-audit note: the symbol-table *enumeration* half of the first
     candidate landed separately as G3.1 (#1670, `rust_snapshot_symbol_table_shadow`,
     `crates/type_kernel/src/symtable_mirror.rs`), flipping namespace
     storage and order while the walk still reads the live node graph.
     That does not change this table: the `snapshot_definition` arms it
     feeds still read `Var.type` / `TypeInfo.bases` and stay floored.

   Idiom note: this flip's deferral follows `subexpr_strip.rs`'s existing
   contract (`Option`, `None` = the Python body stays), not the
   store-level `ShadowGap` reason enum in `symtable_mirror.rs`, whose
   purpose is to report *why* a namespace cannot be served. Reasons here
   are counted Python-side (`nodes_mirror.count` → `aststrip.served` /
   `aststrip.deferred`).

## What landed

- **Gap closure, expression family:** `FieldValue::Text` +
  `rust_node_mirror_capture_field_text`
  (`crates/type_kernel/src/node_mirror.rs:275`, `:464`), the
  `_TEXT_FIELDS` table plus adoption seed and adopted-only write capture
  (`mypy/nodes_mirror.py:136`, `:229`, `:352`).
- **Read flip:** `rust_aststrip_process_lvalue(type_info, lvalue)`
  (`crates/type_kernel/src/subexpr_strip.rs:612`) serves
  `is_new_def` + `name` from the shadow and performs the delete through
  `crate::symtable_mirror::delete` + `dict.__delitem__`, mirroring
  `delete_names_entry`. It returns `None` for every shape it does not
  cover (non-`MemberExpr` lvalue, no record, `type is None`), so
  `mypy/server/aststrip.py:257` keeps the Python tail as the identical
  fallback; `assert self.type is not None` stays Python-side.
- **Gate:** `Options.native_ast_mirror_read` (default off, not in
  `OPTIONS_AFFECTING_CACHE`, `mypy/options.py:432`),
  `TEST_NATIVE_AST_MIRROR_READ` in the test helpers, wired in
  `mypy/build.py:1393` **inside** the `native_ast_mirror` branch, so the
  flip can never serve from an empty store.

## Evidence

Corpus (one targeted file, `mypy/test/testfinegrained.py`, `-n2`); the
flip's branch can only run here, because `strip_target` is called from
`mypy/server/update.py:1121`, which runs *before*
`_clear_native_resolvers` resets the shadow in the same update:

| state | result line |
|---|---|
| gates off (control) | `747 passed, 27 skipped` |
| capture on, read flip off | `747 passed, 27 skipped` |
| capture on, read flip on | `747 passed, 27 skipped` |

Engagement (`MYPY_TK_AST_MIRROR_AUDIT=1`, counters from
`nodes_mirror.report()`, summed over the two xdist workers):

| state | `aststrip.served` | `aststrip.deferred` | `aststrip.*` bytes |
|---|---|---|---|
| read flip on | 26 | 128 | — |
| read flip off | 0 (branch not entered) | 0 | — |

Wire traffic over the same corpus (`MYPY_SERIALIZE_STATS=1`, per-worker
totals summed; the xdist split differs between runs, so sums are the
comparable quantity): flip off `calls 246,042 / writes 92,991 / bytes
2,832,522`; flip on `calls 246,040 / writes 92,990 / bytes 2,832,490`;
`mirror` (F2 blob-serve count) `0` in both states. The aststrip read flip
is a live-object walk with no type wire, so it moves no wire traffic; the
≤2-event spread is the xdist split.

Rust: `cargo test -p mypy-type-kernel` → `2839 passed; 0 failed; 11
ignored`; `cargo clippy -p mypy-type-kernel --lib -- -D warnings` and
`cargo fmt -p mypy-type-kernel -- --check` clean.

Wall clock is **provisional**: this host ran at load 12-82 during these
runs and other lanes held all three semaphore slots at times (one paired
run never started inside its 600s budget while every slot was held, which
is queueing, not the flip). Read-flip off measured 26.82s (load 28.85)
and 59.31s; read-flip on 72.63s (load 58.03), 201.25s (load 12.32) and
53.40s (load 81.69). The spread tracks host contention, not the flip: its
branch executed 154 times in the whole 774-case corpus, so it cannot
account for minutes of difference.

## Residual gaps (named, not closed here)

| gap | why it blocks a read flip | what closes it |
|---|---|---|
| `MemberExpr.expr`, `IndexExpr.base`/`index`, `CallExpr.args`, `OpExpr.right`, `UnaryExpr.expr`, `StrExpr.value`, `Block.body`, `ForStmt.body`/`else_body` | child-node slots; a served read needs the child graph | G4 AST ownership (Rust-owned nodes), not a shadow field |
| `RefExpr.node`, `MemberExpr.def_var`, `SymbolTableNode._node`, `ClassDef.info`, `Var.info` | identity only (fullname marker); the target object is not stored | object-pin records or stable handles (#1528-class) |
| `ComparisonExpr.method_types`, `Var.type`, `FuncDef.type`, `AssignmentStmt.type`, `TypeInfo.bases`/`mro`/`metaclass_type`/`typeddict_type`/`_promote` | object-valued; markers only | wire (or pinned-object) storage for G2/G3 type-valued fields, plus the F-phase TypeInfo graph |
| `TypeInfo.is_abstract`, `abstract_attributes`, `tuple_type`, `is_named_tuple`, `is_newtype`, `slots`, `deletable_attributes`, `defn`, `alt_promote`, `type_object_type`, `typeddict_data`, `SymbolTableNode.unfixed`, `stored_info` | untracked in the G3 meta tables | G3.1 symbol-table read flip (the next slice) |

## Tables (generated)

<!-- BEGIN GENERATED -->
<!-- generated by misc/g12_node_shadow_audit.py; do not hand-edit -->

### G1 expression shadow (record + field map)

| Python class | slot (`mypy/nodes.py`) | Rust home | served as | verdict |
|---|---|---|---|---|
| `RefExpr` | `kind` (nodes.py:2491) | `node_mirror.rs:43` | NodeShadow.kind | served |
| `RefExpr` | `node` (nodes.py:2492) | `node_mirror.rs:44` | NodeShadow.node_fullname | served |
| `RefExpr` | `_fullname` (nodes.py:2493) | `node_mirror.rs:45` | NodeShadow.fullname | served |
| `RefExpr` | `is_new_def` (nodes.py:2494) | `node_mirror.rs:46` | NodeShadow.is_new_def | served |
| `RefExpr` | `is_inferred_def` (nodes.py:2495) | `node_mirror.rs:47` | NodeShadow.is_inferred_def | served |
| `RefExpr` | `is_alias_rvalue` (nodes.py:2496) | `node_mirror.rs:345` | `FieldValue::Flag` (`nodes_mirror.py:165`) | served |
| `RefExpr` | `type_guard` (nodes.py:2497) | `node_mirror.rs:355` | `FieldValue::Wire` (`nodes_mirror.py:162`) | served (kind + G1.1 wire bytes) |
| `RefExpr` | `type_is` (nodes.py:2498) | `node_mirror.rs:355` | `FieldValue::Wire` (`nodes_mirror.py:162`) | served (kind + G1.1 wire bytes) |
| `NameExpr` | `name` (nodes.py:2539) | `node_mirror.rs:350` | `FieldValue::Text` (`nodes_mirror.py:172`) | served |
| `NameExpr` | `is_special_form` (nodes.py:2539) | `node_mirror.rs:345` | `FieldValue::Flag` (`nodes_mirror.py:165`) | served |
| `MemberExpr` | `expr` (nodes.py:2559) | - | - | gap |
| `MemberExpr` | `name` (nodes.py:2559) | `node_mirror.rs:350` | `FieldValue::Text` (`nodes_mirror.py:172`) | served |
| `MemberExpr` | `def_var` (nodes.py:2559) | `node_mirror.rs:347` | `FieldValue::Name` (`nodes_mirror.py:167`) | partial (element fullname/class only) |
| `CallExpr` | `callee` (nodes.py:2624) | - | - | AST wire (structural) |
| `CallExpr` | `args` (nodes.py:2624) | - | - | gap |
| `CallExpr` | `arg_kinds` (nodes.py:2624) | - | - | AST wire (structural) |
| `CallExpr` | `arg_names` (nodes.py:2624) | - | - | AST wire (structural) |
| `CallExpr` | `analyzed` (nodes.py:2624) | `node_mirror.rs:49` | `NodeShadow.analyzed_kind` (`nodes_mirror.py:157`) | partial (replacement class name only) |
| `IndexExpr` | `base` (nodes.py:2689) | - | - | gap |
| `IndexExpr` | `index` (nodes.py:2689) | - | - | gap |
| `IndexExpr` | `method_type` (nodes.py:2689) | `node_mirror.rs:355` | `FieldValue::Wire` (`nodes_mirror.py:162`) | served (kind + G1.1 wire bytes) |
| `IndexExpr` | `analyzed` (nodes.py:2689) | `node_mirror.rs:49` | `NodeShadow.analyzed_kind` (`nodes_mirror.py:157`) | partial (replacement class name only) |
| `IndexExpr` | `as_type` (nodes.py:2689) | `node_mirror.rs:355` | `FieldValue::Wire` (`nodes_mirror.py:162`) | served (kind + G1.1 wire bytes) |
| `OpExpr` | `op` (nodes.py:2762) | - | - | AST wire (structural) |
| `OpExpr` | `left` (nodes.py:2763) | - | - | AST wire (structural) |
| `OpExpr` | `right` (nodes.py:2764) | - | - | AST wire (structural) |
| `OpExpr` | `method_type` (nodes.py:2765) | `node_mirror.rs:355` | `FieldValue::Wire` (`nodes_mirror.py:162`) | served (kind + G1.1 wire bytes) |
| `OpExpr` | `right_always` (nodes.py:2766) | `node_mirror.rs:345` | `FieldValue::Flag` (`nodes_mirror.py:165`) | served |
| `OpExpr` | `right_unreachable` (nodes.py:2767) | `node_mirror.rs:345` | `FieldValue::Flag` (`nodes_mirror.py:165`) | served |
| `OpExpr` | `analyzed` (nodes.py:2768) | `node_mirror.rs:49` | `NodeShadow.analyzed_kind` (`nodes_mirror.py:157`) | partial (replacement class name only) |
| `OpExpr` | `as_type` (nodes.py:2769) | `node_mirror.rs:355` | `FieldValue::Wire` (`nodes_mirror.py:162`) | served (kind + G1.1 wire bytes) |
| `UnaryExpr` | `op` (nodes.py:2720) | - | - | AST wire (structural) |
| `UnaryExpr` | `expr` (nodes.py:2720) | - | - | AST wire (structural) |
| `UnaryExpr` | `method_type` (nodes.py:2720) | `node_mirror.rs:355` | `FieldValue::Wire` (`nodes_mirror.py:162`) | served (kind + G1.1 wire bytes) |
| `ComparisonExpr` | `operators` (nodes.py:2818) | - | - | AST wire (structural) |
| `ComparisonExpr` | `operands` (nodes.py:2818) | - | - | AST wire (structural) |
| `ComparisonExpr` | `method_types` (nodes.py:2818) | `node_mirror.rs:352` | `FieldValue::Kinds` (`nodes_mirror.py:168`) | partial (element fullname/class only) |
| `StrExpr` | `value` (nodes.py:2372) | - | - | AST wire (structural) |
| `StrExpr` | `as_type` (nodes.py:2372) | `node_mirror.rs:355` | `FieldValue::Wire` (`nodes_mirror.py:162`) | served (kind + G1.1 wire bytes) |

### G2 statement/def shadow (field-name keyed meta record)

| Python class | slot (`mypy/nodes.py`) | home | verdict |
|---|---|---|---|
| `ImportBase` | `is_unreachable` (nodes.py:693) | `nodes_mirror.py:619` via `_G2_TRACKED[ImportBase]` | served |
| `ImportBase` | `is_unreachable_dependency` (nodes.py:694) | `nodes_mirror.py:620` via `_G2_TRACKED[ImportBase]` | served |
| `ImportBase` | `is_top_level` (nodes.py:695) | `nodes_mirror.py:621` via `_G2_TRACKED[ImportBase]` | served |
| `ImportBase` | `is_mypy_only` (nodes.py:696) | `nodes_mirror.py:622` via `_G2_TRACKED[ImportBase]` | served |
| `ImportBase` | `assignments` (nodes.py:697) | `nodes_mirror.py:618` via `_G2_TRACKED[ImportBase]` | partial (marker only) |
| `Import` | `ids` (nodes.py:726) | - | AST wire (structural) |
| `ImportFrom` | `id` (nodes.py:743) | - | gap |
| `ImportFrom` | `names` (nodes.py:743) | - | gap |
| `ImportFrom` | `relative` (nodes.py:743) | - | AST wire (structural) |
| `ImportAll` | `id` (nodes.py:764) | - | gap |
| `ImportAll` | `relative` (nodes.py:764) | - | AST wire (structural) |
| `Block` | `body` (nodes.py:1905) | - | gap |
| `Block` | `is_unreachable` (nodes.py:1905) | `nodes_mirror.py:625` via `_G2_TRACKED[Block]` | served |
| `AssignmentStmt` | `lvalues` (nodes.py:1956) | - | AST wire (structural) |
| `AssignmentStmt` | `rvalue` (nodes.py:1957) | - | AST wire (structural) |
| `AssignmentStmt` | `type` (nodes.py:1958) | `nodes_mirror.py:627` via `_G2_TRACKED[AssignmentStmt]` | partial (marker only) |
| `AssignmentStmt` | `unanalyzed_type` (nodes.py:1959) | `nodes_mirror.py:627` via `_G2_TRACKED[AssignmentStmt]` | partial (marker only) |
| `AssignmentStmt` | `new_syntax` (nodes.py:1960) | - | AST wire (structural) |
| `AssignmentStmt` | `is_alias_def` (nodes.py:1961) | `nodes_mirror.py:627` via `_G2_TRACKED[AssignmentStmt]` | served |
| `AssignmentStmt` | `is_final_def` (nodes.py:1962) | `nodes_mirror.py:627` via `_G2_TRACKED[AssignmentStmt]` | served |
| `AssignmentStmt` | `invalid_recursive_alias` (nodes.py:1963) | `nodes_mirror.py:627` via `_G2_TRACKED[AssignmentStmt]` | served |
| `ForStmt` | `index` (nodes.py:2054) | `nodes_mirror.py:631` via `_G2_TRACKED[ForStmt]` | partial (marker only) |
| `ForStmt` | `index_type` (nodes.py:2055) | `nodes_mirror.py:632` via `_G2_TRACKED[ForStmt]` | partial (marker only) |
| `ForStmt` | `unanalyzed_index_type` (nodes.py:2056) | `nodes_mirror.py:633` via `_G2_TRACKED[ForStmt]` | partial (marker only) |
| `ForStmt` | `inferred_item_type` (nodes.py:2057) | `nodes_mirror.py:634` via `_G2_TRACKED[ForStmt]` | partial (marker only) |
| `ForStmt` | `inferred_iterator_type` (nodes.py:2058) | `nodes_mirror.py:635` via `_G2_TRACKED[ForStmt]` | partial (marker only) |
| `ForStmt` | `expr` (nodes.py:2059) | - | AST wire (structural) |
| `ForStmt` | `body` (nodes.py:2060) | - | gap |
| `ForStmt` | `else_body` (nodes.py:2061) | - | AST wire (structural) |
| `ForStmt` | `is_async` (nodes.py:2062) | - | gap |
| `WithStmt` | `expr` (nodes.py:2257) | - | AST wire (structural) |
| `WithStmt` | `target` (nodes.py:2257) | - | gap |
| `WithStmt` | `unanalyzed_type` (nodes.py:2257) | - | gap |
| `WithStmt` | `analyzed_types` (nodes.py:2257) | `nodes_mirror.py:638` via `_G2_TRACKED[WithStmt]` | partial (marker only) |
| `WithStmt` | `body` (nodes.py:2257) | - | gap |
| `WithStmt` | `is_async` (nodes.py:2257) | - | gap |
| `IfStmt` | `expr` (nodes.py:2175) | - | AST wire (structural) |
| `IfStmt` | `body` (nodes.py:2175) | - | gap |
| `IfStmt` | `else_body` (nodes.py:2175) | - | gap |
| `IfStmt` | `unreachable_else` (nodes.py:2175) | `nodes_mirror.py:639` via `_G2_TRACKED[IfStmt]` | partial (marker only) |
| `MatchStmt` | `subject` (nodes.py:2290) | - | AST wire (structural) |
| `MatchStmt` | `subject_dummy` (nodes.py:2290) | `nodes_mirror.py:640` via `_G2_TRACKED[MatchStmt]` | partial (marker only) |
| `MatchStmt` | `patterns` (nodes.py:2290) | - | AST wire (structural) |
| `MatchStmt` | `guards` (nodes.py:2290) | - | AST wire (structural) |
| `MatchStmt` | `bodies` (nodes.py:2290) | - | AST wire (structural) |
| `TypeAliasStmt` | `name` (nodes.py:2320) | - | gap |
| `TypeAliasStmt` | `type_args` (nodes.py:2320) | - | gap |
| `TypeAliasStmt` | `value` (nodes.py:2320) | - | AST wire (structural) |
| `TypeAliasStmt` | `invalid_recursive_alias` (nodes.py:2320) | `nodes_mirror.py:641` via `_G2_TRACKED[TypeAliasStmt]` | served |
| `TypeAliasStmt` | `alias_node` (nodes.py:2320) | `nodes_mirror.py:641` via `_G2_TRACKED[TypeAliasStmt]` | partial (marker only) |
| `FuncDef` | `_name` (nodes.py:1192) | `nodes_mirror.py:570` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `is_decorated` (nodes.py:1193) | `nodes_mirror.py:548` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `is_conditional` (nodes.py:1194) | `nodes_mirror.py:549` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `abstract_status` (nodes.py:1195) | `nodes_mirror.py:563` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `original_def` (nodes.py:1196) | `nodes_mirror.py:565` via `_G2_TRACKED[FuncDef]` | partial (marker only) |
| `FuncDef` | `is_trivial_body` (nodes.py:1197) | `nodes_mirror.py:550` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `is_trivial_self` (nodes.py:1198) | `nodes_mirror.py:551` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `is_invalid_redefinition` (nodes.py:1199) | `nodes_mirror.py:553` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `is_mypy_only` (nodes.py:1200) | `nodes_mirror.py:552` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `dataclass_transform_spec` (nodes.py:1202) | `nodes_mirror.py:566` via `_G2_TRACKED[FuncDef]` | partial (marker only) |
| `FuncDef` | `docstring` (nodes.py:1203) | `nodes_mirror.py:567` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `deprecated` (nodes.py:1204) | `nodes_mirror.py:564` via `_G2_TRACKED[FuncDef]` | served |
| `FuncDef` | `original_first_arg` (nodes.py:1205) | `nodes_mirror.py:573` via `_G2_TRACKED[FuncDef]` | served |
| `OverloadedFuncDef` | `items` (nodes.py:880) | `nodes_mirror.py:644` via `_G2_TRACKED[OverloadedFuncDef]` | partial (marker only) |
| `OverloadedFuncDef` | `unanalyzed_items` (nodes.py:881) | `nodes_mirror.py:644` via `_G2_TRACKED[OverloadedFuncDef]` | partial (marker only) |
| `OverloadedFuncDef` | `impl` (nodes.py:882) | `nodes_mirror.py:644` via `_G2_TRACKED[OverloadedFuncDef]` | partial (marker only) |
| `OverloadedFuncDef` | `deprecated` (nodes.py:883) | `nodes_mirror.py:644` via `_G2_TRACKED[OverloadedFuncDef]` | served |
| `OverloadedFuncDef` | `setter_index` (nodes.py:884) | `nodes_mirror.py:644` via `_G2_TRACKED[OverloadedFuncDef]` | served |
| `OverloadedFuncDef` | `_is_trivial_self` (nodes.py:885) | `nodes_mirror.py:644` via `_G2_TRACKED[OverloadedFuncDef]` | served |
| `Decorator` | `func` (nodes.py:1408) | `nodes_mirror.py:647` via `_G2_TRACKED[Decorator]` | partial (marker only) |
| `Decorator` | `decorators` (nodes.py:1408) | `nodes_mirror.py:647` via `_G2_TRACKED[Decorator]` | partial (marker only) |
| `Decorator` | `original_decorators` (nodes.py:1408) | `nodes_mirror.py:647` via `_G2_TRACKED[Decorator]` | partial (marker only) |
| `Decorator` | `var` (nodes.py:1408) | `nodes_mirror.py:647` via `_G2_TRACKED[Decorator]` | partial (marker only) |
| `Decorator` | `is_overload` (nodes.py:1408) | `nodes_mirror.py:647` via `_G2_TRACKED[Decorator]` | served |
| `ClassDef` | `name` (nodes.py:1750) | `nodes_mirror.py:650` via `_G2_TRACKED[ClassDef]` | served |
| `ClassDef` | `_fullname` (nodes.py:1751) | `nodes_mirror.py:655` via `_G2_TRACKED[ClassDef]` | served |
| `ClassDef` | `defs` (nodes.py:1752) | - | gap |
| `ClassDef` | `type_args` (nodes.py:1753) | - | gap |
| `ClassDef` | `type_vars` (nodes.py:1754) | `nodes_mirror.py:657` via `_G2_TRACKED[ClassDef]` | partial (marker only) |
| `ClassDef` | `base_type_exprs` (nodes.py:1755) | `nodes_mirror.py:658` via `_G2_TRACKED[ClassDef]` | partial (marker only) |
| `ClassDef` | `removed_base_type_exprs` (nodes.py:1756) | `nodes_mirror.py:656` via `_G2_TRACKED[ClassDef]` | partial (marker only) |
| `ClassDef` | `info` (nodes.py:1757) | `nodes_mirror.py:651` via `_G2_TRACKED[ClassDef]` | partial (marker only) |
| `ClassDef` | `metaclass` (nodes.py:1758) | `nodes_mirror.py:654` via `_G2_TRACKED[ClassDef]` | partial (marker only) |
| `ClassDef` | `decorators` (nodes.py:1759) | - | gap |
| `ClassDef` | `keywords` (nodes.py:1760) | - | AST wire (structural) |
| `ClassDef` | `analyzed` (nodes.py:1761) | `nodes_mirror.py:652` via `_G2_TRACKED[ClassDef]` | partial (replacement class name only) |
| `ClassDef` | `has_incompatible_baseclass` (nodes.py:1762) | `nodes_mirror.py:653` via `_G2_TRACKED[ClassDef]` | served |
| `ClassDef` | `docstring` (nodes.py:1763) | - | gap |
| `ClassDef` | `removed_statements` (nodes.py:1764) | `nodes_mirror.py:659` via `_G2_TRACKED[ClassDef]` | partial (marker only) |
| `Var` | `_name` (nodes.py:1525) | `nodes_mirror.py:582` via `_G2_TRACKED[Var]` | served |
| `Var` | `_fullname` (nodes.py:1526) | `nodes_mirror.py:583` via `_G2_TRACKED[Var]` | served |
| `Var` | `info` (nodes.py:1527) | `nodes_mirror.py:586` via `_G2_TRACKED[Var]` | partial (marker only) |
| `Var` | `type` (nodes.py:1528) | `nodes_mirror.py:584` via `_G2_TRACKED[Var]` | partial (marker only) |
| `Var` | `setter_type` (nodes.py:1529) | `nodes_mirror.py:585` via `_G2_TRACKED[Var]` | partial (marker only) |
| `Var` | `final_value` (nodes.py:1530) | `nodes_mirror.py:587` via `_G2_TRACKED[Var]` | partial (marker only) |
| `Var` | `is_self` (nodes.py:1531) | `nodes_mirror.py:588` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_cls` (nodes.py:1532) | `nodes_mirror.py:589` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_ready` (nodes.py:1533) | `nodes_mirror.py:590` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_inferred` (nodes.py:1534) | `nodes_mirror.py:591` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_initialized_in_class` (nodes.py:1535) | `nodes_mirror.py:592` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_staticmethod` (nodes.py:1536) | `nodes_mirror.py:593` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_classmethod` (nodes.py:1537) | `nodes_mirror.py:594` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_property` (nodes.py:1538) | `nodes_mirror.py:595` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_settable_property` (nodes.py:1539) | `nodes_mirror.py:596` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_classvar` (nodes.py:1540) | `nodes_mirror.py:597` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_abstract_var` (nodes.py:1541) | `nodes_mirror.py:598` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_final` (nodes.py:1542) | `nodes_mirror.py:599` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_index_var` (nodes.py:1543) | `nodes_mirror.py:600` via `_G2_TRACKED[Var]` | served |
| `Var` | `final_unset_in_class` (nodes.py:1544) | `nodes_mirror.py:601` via `_G2_TRACKED[Var]` | served |
| `Var` | `final_set_in_init` (nodes.py:1545) | `nodes_mirror.py:602` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_suppressed_import` (nodes.py:1546) | `nodes_mirror.py:603` via `_G2_TRACKED[Var]` | served |
| `Var` | `explicit_self_type` (nodes.py:1547) | `nodes_mirror.py:604` via `_G2_TRACKED[Var]` | served |
| `Var` | `from_module_getattr` (nodes.py:1548) | `nodes_mirror.py:605` via `_G2_TRACKED[Var]` | served |
| `Var` | `has_explicit_value` (nodes.py:1549) | `nodes_mirror.py:606` via `_G2_TRACKED[Var]` | served |
| `Var` | `allow_incompatible_override` (nodes.py:1550) | `nodes_mirror.py:607` via `_G2_TRACKED[Var]` | served |
| `Var` | `invalid_partial_type` (nodes.py:1551) | `nodes_mirror.py:608` via `_G2_TRACKED[Var]` | served |
| `Var` | `is_argument` (nodes.py:1552) | `nodes_mirror.py:609` via `_G2_TRACKED[Var]` | served |

### G3 symbol-table shadow (`symtables_mirror.py`, keyed by table+name)

| Python class | slot (`mypy/nodes.py`) | home | verdict |
|---|---|---|---|
| `SymbolTableNode` | `kind` (nodes.py:5065) | `symtables_mirror.py:84` via `_FLAG_FIELDS` | served |
| `SymbolTableNode` | `_node` (nodes.py:5066) | `symtables_mirror.py:91` via `_FLAG_FIELDS` | served |
| `SymbolTableNode` | `_node_bytes` (nodes.py:5067) | - | AST wire (structural) |
| `SymbolTableNode` | `_node_tag` (nodes.py:5068) | - | AST wire (structural) |
| `SymbolTableNode` | `module_public` (nodes.py:5069) | `symtables_mirror.py:85` via `_FLAG_FIELDS` | served |
| `SymbolTableNode` | `module_hidden` (nodes.py:5070) | `symtables_mirror.py:86` via `_FLAG_FIELDS` | served |
| `SymbolTableNode` | `cross_ref` (nodes.py:5071) | `symtables_mirror.py:90` via `_FLAG_FIELDS` | served |
| `SymbolTableNode` | `implicit` (nodes.py:5072) | `symtables_mirror.py:87` via `_FLAG_FIELDS` | served |
| `SymbolTableNode` | `plugin_generated` (nodes.py:5073) | `symtables_mirror.py:88` via `_FLAG_FIELDS` | served |
| `SymbolTableNode` | `no_serialize` (nodes.py:5074) | `symtables_mirror.py:89` via `_FLAG_FIELDS` | served |
| `SymbolTableNode` | `unfixed` (nodes.py:5075) | - | gap |
| `SymbolTableNode` | `stored_info` (nodes.py:5076) | - | gap |
| `TypeInfo` | `_fullname` (nodes.py:3742) | `symtables_mirror.py:103` via `_META_FIELDS` | served |
| `TypeInfo` | `module_name` (nodes.py:3743) | - | gap |
| `TypeInfo` | `defn` (nodes.py:3744) | - | gap |
| `TypeInfo` | `mro` (nodes.py:3745) | `symtables_mirror.py:101` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `_mro_refs` (nodes.py:3746) | - | gap |
| `TypeInfo` | `bad_mro` (nodes.py:3747) | `symtables_mirror.py:106` via `_META_FIELDS` | served |
| `TypeInfo` | `is_final` (nodes.py:3748) | `symtables_mirror.py:107` via `_META_FIELDS` | served |
| `TypeInfo` | `is_disjoint_base` (nodes.py:3749) | `symtables_mirror.py:108` via `_META_FIELDS` | served |
| `TypeInfo` | `declared_metaclass` (nodes.py:3750) | `symtables_mirror.py:116` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `metaclass_type` (nodes.py:3751) | `symtables_mirror.py:102` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `names` (nodes.py:3752) | `symtables_mirror.py:104` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `is_abstract` (nodes.py:3753) | - | gap |
| `TypeInfo` | `is_protocol` (nodes.py:3754) | `symtables_mirror.py:110` via `_META_FIELDS` | served |
| `TypeInfo` | `runtime_protocol` (nodes.py:3755) | `symtables_mirror.py:115` via `_META_FIELDS` | served |
| `TypeInfo` | `abstract_attributes` (nodes.py:3756) | - | gap |
| `TypeInfo` | `deletable_attributes` (nodes.py:3757) | - | gap |
| `TypeInfo` | `slots` (nodes.py:3758) | - | gap |
| `TypeInfo` | `assuming` (nodes.py:3759) | - | AST wire (structural) |
| `TypeInfo` | `assuming_proper` (nodes.py:3760) | - | AST wire (structural) |
| `TypeInfo` | `inferring` (nodes.py:3761) | - | gap |
| `TypeInfo` | `is_enum` (nodes.py:3762) | `symtables_mirror.py:109` via `_META_FIELDS` | served |
| `TypeInfo` | `fallback_to_any` (nodes.py:3763) | `symtables_mirror.py:113` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `meta_fallback_to_any` (nodes.py:3764) | `symtables_mirror.py:114` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `type_vars` (nodes.py:3765) | `symtables_mirror.py:117` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `has_param_spec_type` (nodes.py:3766) | - | AST wire (structural) |
| `TypeInfo` | `bases` (nodes.py:3767) | `symtables_mirror.py:100` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `_promote` (nodes.py:3768) | - | AST wire (structural) |
| `TypeInfo` | `tuple_type` (nodes.py:3769) | - | gap |
| `TypeInfo` | `special_alias` (nodes.py:3770) | - | gap |
| `TypeInfo` | `is_named_tuple` (nodes.py:3771) | - | gap |
| `TypeInfo` | `typeddict_type` (nodes.py:3772) | - | AST wire (structural) |
| `TypeInfo` | `is_newtype` (nodes.py:3773) | - | gap |
| `TypeInfo` | `is_intersection` (nodes.py:3774) | `symtables_mirror.py:112` via `_META_FIELDS` | served |
| `TypeInfo` | `metadata` (nodes.py:3775) | - | AST wire (structural) |
| `TypeInfo` | `alt_promote` (nodes.py:3776) | - | gap |
| `TypeInfo` | `has_type_var_tuple_type` (nodes.py:3777) | - | AST wire (structural) |
| `TypeInfo` | `type_var_tuple_prefix` (nodes.py:3778) | - | AST wire (structural) |
| `TypeInfo` | `type_var_tuple_suffix` (nodes.py:3779) | - | AST wire (structural) |
| `TypeInfo` | `self_type` (nodes.py:3780) | `symtables_mirror.py:118` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `dataclass_transform_spec` (nodes.py:3781) | `symtables_mirror.py:119` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `is_type_check_only` (nodes.py:3782) | `symtables_mirror.py:111` via `_META_FIELDS` | served |
| `TypeInfo` | `deprecated` (nodes.py:3783) | `symtables_mirror.py:120` via `_META_FIELDS` | served |
| `TypeInfo` | `type_object_type` (nodes.py:3784) | - | gap |
| `TypeInfo` | `default_depends` (nodes.py:3785) | `symtables_mirror.py:121` via `_META_FIELDS` | partial (marker only) |
| `TypeInfo` | `typeddict_data` (nodes.py:3786) | - | gap |

### Gap ledger (Python write sites outside `mypy/nodes.py`)

| family | class | slot | writes outside `nodes.py` |
|---|---|---|---|
| G1 | `MemberExpr` | `expr` | `mypy/checkstrformat.py:865` |
| G1 | `CallExpr` | `args` | `mypy/exprtotype.py:132`, `mypy/semanal.py:2894`, `mypy/semanal_shared.py:329` |
| G1 | `IndexExpr` | `base` | `mypy/checkstrformat.py:863` |
| G1 | `IndexExpr` | `index` | `mypy/checkstrformat.py:858` |
| G2 | `ImportFrom` | `id` | `mypy/build.py:232` |
| G2 | `ImportFrom` | `names` | `mypy/build.py:4675`, `mypy/treetransform.py:156` |
| G2 | `ImportAll` | `id` | `mypy/build.py:232` |
| G2 | `Block` | `body` | `mypy/fastparse.py:852`, `mypy/nativeparse.py:1936`, `mypy/semanal_namedtuple.py:145` |
| G2 | `ForStmt` | `body` | `mypy/fastparse.py:852`, `mypy/nativeparse.py:1936`, `mypy/server/aststrip.py:147` |
| G2 | `ForStmt` | `is_async` | `mypy/fastparse.py:1329`, `mypy/nativeparse.py:471`, `mypy/treetransform.py:355` |
| G2 | `WithStmt` | `target` | `mypy/semanal.py:5522` |
| G2 | `WithStmt` | `unanalyzed_type` | `mypy/fastparse.py:1050`, `mypy/nativeparse.py:789`, `mypy/semanal.py:10081` |
| G2 | `WithStmt` | `body` | `mypy/fastparse.py:852`, `mypy/nativeparse.py:1936` |
| G2 | `WithStmt` | `is_async` | `mypy/fastparse.py:1329`, `mypy/nativeparse.py:471`, `mypy/treetransform.py:355` |
| G2 | `IfStmt` | `body` | `mypy/fastparse.py:852`, `mypy/nativeparse.py:1936` |
| G2 | `IfStmt` | `else_body` | `mypy/reachability.py:101` |
| G2 | `TypeAliasStmt` | `name` | `mypy/stubutil.py:335` |
| G2 | `TypeAliasStmt` | `type_args` | `mypy/semanal.py:10076` |
| G2 | `ClassDef` | `defs` | `mypy/server/astmerge.py:212` |
| G2 | `ClassDef` | `type_args` | `mypy/semanal.py:10076` |
| G2 | `ClassDef` | `decorators` | `mypy/fastparse.py:1192`, `mypy/nativeparse.py:830`, `mypy/semanal.py:2364` |
| G2 | `ClassDef` | `docstring` | `mypy/fastparse.py:1073`, `mypy/nativeparse.py:775`, `mypy/stubutil.py:337` |
| G3 | `SymbolTableNode` | `unfixed` | `mypy/fixup.py:157` |
| G3 | `SymbolTableNode` | `stored_info` | `mypy/fixup.py:166` |
| G3 | `TypeInfo` | `module_name` | `mypy/stubgen.py:590` |
| G3 | `TypeInfo` | `defn` | `mypy/semanal.py:3443` |
| G3 | `TypeInfo` | `_mro_refs` | `mypy/fixup.py:138` |
| G3 | `TypeInfo` | `is_abstract` | `mypy/semanal_classprop.py:73` |
| G3 | `TypeInfo` | `abstract_attributes` | `mypy/semanal_classprop.py:74` |
| G3 | `TypeInfo` | `deletable_attributes` | `mypy/semanal.py:6759` |
| G3 | `TypeInfo` | `slots` | `mypy/plugins/attrs.py:1259`, `mypy/plugins/dataclasses.py:714`, `mypy/semanal.py:6818` |
| G3 | `TypeInfo` | `inferring` | `mypy/typestate.py:130` |
| G3 | `TypeInfo` | `tuple_type` | `mypy/checker.py:8421` |
| G3 | `TypeInfo` | `special_alias` | `mypy/fixup.py:473` |
| G3 | `TypeInfo` | `is_named_tuple` | `mypy/semanal_namedtuple.py:497` |
| G3 | `TypeInfo` | `is_newtype` | `mypy/semanal_newtype.py:235` |
| G3 | `TypeInfo` | `alt_promote` | `mypy/semanal_classprop.py:229` |
| G3 | `TypeInfo` | `type_object_type` | `mypy/typeops.py:476` |
| G3 | `TypeInfo` | `typeddict_data` | `mypy/semanal_typeddict.py:852` |

<!-- END GENERATED -->
