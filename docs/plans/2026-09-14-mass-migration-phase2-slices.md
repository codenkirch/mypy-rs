# Mass migration Phase 2: ranked vertical slices

Date: 2026-09-14. Read-only decomposition for #1625 (master plan). Deliverable:
independently-grabbable slices a swarm can pick up now, plus the blocked and
floor lists so the same ground is not re-derived. No production code changed.

## 1. Method and evidence base

**Survey method: `rg` seam inventory, not a live survey.** The seam survey
(`docs/HANDOFF.md` section "The loop", step 2, `scripts/measure_native_share.py`)
was skipped on the resource rule: the pre-flight combined-RSS check
(`ps -axo rss | awk '{s+=$1} END {printf "%.1f GB\n", s/1048576}'`) read
**44.8 GB**, then **42.8 GB**, then **43.6 GB** across the session, all above the
40 GB cut. The top RSS holders were a persistent macOS VM (3.48 GB),
`mds_stores` (1.40 GB) and a `lean` process (1.15 GB), none transient, so the
reading was not going to fall on its own. Every number below is therefore either
(a) a measured count reproduced with `rg`/`wc` on this head, or (b) a count
already recorded in `AGENTS.md`/`docs/HANDOFF.md` and cited with its wave.

Everything in section 2 was measured on this head. Everything in section 3 is
measured-or-cited. The ranking criterion the brief asked for (defer count retired
per unit risk) needs defer counts, and those exist only inside the survey's
per-seam table or the wave audits; where I have neither I rank on a stated
structural proxy and say so. **Whoever picks up any ticket below should run the
survey first as step 0 and re-rank if the numbers disagree with this document.**

## 2. The load-bearing finding: a large production-inert surface

This is the single most important input to the ranking, and it is not in #1625.

`Options.native_type_kernel`, `native_parser` and `native_resolver` default
`True` (`mypy/options.py:394, 401, 412`). Everything else native defaults `False`
(`mypy/options.py:428-444`):

| Option | Default | Feature | Python side | Rust side |
|---|---|---|---|---|
| `native_type_kernel` | **True** | the whole kernel | - | - |
| `native_parser` | **True** | `ast_serialize` | `mypy/nativeparse.py` (2293) | `crates/ast_serialize` |
| `native_resolver` | **True** | `module_resolver` | `mypy/modulefinder.py` (1193) | `crates/module_resolver` |
| `native_type_proxy` | False | proxy P1 scaffold | `mypy/type_proxy.py` (186) | `proxy.rs` (275) |
| `native_ast_mirror` | False | G1/G2 node shadow | `mypy/nodes_mirror.py` (675) | `node_mirror.rs` (1171) |
| `native_symtable_mirror` | False | G3 symtable shadow | `mypy/symtables_mirror.py` (507) + `mypy/symtable_access.py` (124) | `symtable_mirror.rs` (831) |
| `native_cache_data` | False | G0.5 cache writer | `mypy/cache_data.py` (94) | `ast_serialize/src/sym_node.rs` (1222) |
| `native_binder` | False | H1b binder store | - | `binder.rs` (244) |

Plus an env-gated pair, set only when `MYPY_ENABLE_NATIVE_SEMANAL` is present:

- `mypy/build.py:1088-1092` -> `_set_native_semanal_active`
- `mypy/build.py:1206-1209` -> `_set_native_semanal_visitor_active`

The consequence, measured:

- `mypy/semanal.py` carries **111** `and _native_semanal_visitor_active` guard
  sites and **10** `and _native_semanal_active` guard sites (**121** total),
  behind **392** `rust_*` references. All 121 are dark in a default production
  run. The whole `semanal_*.rs` family (**11,687** lines) is dark with them,
  except `semanal_shared.rs`, whose `_native_semanal_shared_active` gate is
  default-ON (`mypy/build.py:1213`).
- CI runs them ON: `native-kernel-parity.yml:137-141` sets
  `MYPY_NATIVE_PARITY_INSTALL_RESOLVERS=1`,
  `MYPY_NATIVE_PARITY_INSTALL_SEMANAL_VISITOR=1`, `MYPY_ENABLE_NATIVE_SEMANAL=1`
  in the `parity` job. The test suite turns them on too
  (`mypy/test/conftest.py:207-209`, and per-suite `_set_active(True)` calls).
- Therefore a port whose only gate is `_native_semanal_active` /
  `_native_semanal_visitor_active` / `Options.native_binder` / the three
  `native_*_mirror` flags is **parity-covered but production-dead**. The in-flight
  H1p/H1q/H1r family is in this class: issue #1616 (is_self_member_ref) says
  "Gated by `_native_semanal_active`", and `mypy/semanal.py:6026-6031` reads that
  flag, which `mypy/build.py:1090` sets only under the env var.

`docs/remaining-migration-plan.md` (08-26 late-2 entry) records why: the semanal
seams measured as a **net loss** (+8.9pp total work share, semanal phase share cut
-68.5% -> -32.5%), because that phase has many short calls where the fixed
wire-encode cost dominates. The gate was deliberately flipped to opt-IN. #1624
carries the same diagnosis.

**Ranking rule that follows:** production-live gate first, parity-only gate last,
regardless of how much Python a slice moves. A slice that grows the dark surface
is negative-value work.

## 3. Measured inventory on this head

Structural counts (`rg`/`wc`, this head, `656cddbe3` + this branch):

| Metric | Value |
|---|---|
| `add_function`/`add_class` registrations in `crates/type_kernel/src/lib.rs` | 948 |
| `return None` sites in `crates/type_kernel/src/*.rs` | 1,115 |
| Python LOC under `mypy/` excluding tests | 143,763 |
| Python LOC in the 16 modules named by #1625 | 77,750 |
| Python LOC in the 27 module files inspected as slice candidates | 86,986 |

Top `return None` files: `setops.rs` 121, `checkmember.rs` 116, `subtypes.rs` 95,
`constraints.rs` 78, `checkexpr_functions.rs` 63, `typeops.rs` 62, `meet.rs` 42,
`expandtype.rs` 39, `checkcall.rs` 32, `checker_helpers.rs` 31,
`callable_compat.rs` 26, `messages.rs` 24, `solve.rs` 23, `checkpattern.rs` 23.

**Caution: `return None` count is a surface metric, not a progress metric.** It
rose from the 514 recorded in `docs/remaining-migration-plan.md` ("Phase B", 2026
figures) to 1,115 because every port adds defer branches at its leaves. Do not
treat it as work remaining.

Per-call wire-cost surface (`rg -c "_serialize_type|_serialize_type_for_|_serialize_type_list"`),
which is what #1624 lever (b) has to attack:

| Module | serialize entry call sites |
|---|---|
| `mypy/checkexpr.py` | 69 |
| `mypy/checker.py` | 62 |
| `mypy/types.py` | 37 |
| `mypy/typeops.py` | 35 |
| `mypy/subtypes.py` | 33 |
| `mypy/checkmember.py` | 30 |
| `mypy/join.py` | 16 |
| `mypy/checkpattern.py` | 14 |
| `mypy/messages.py` | 13 |
| `mypy/typeanal.py` | 12 |
| `mypy/meet.py` | 12 |
| `mypy/solve.py` | 11 |

Active counters that make these measurable: `MYPY_SERIALIZE_STATS` is live
(`mypy/types.py:170-174`); the survey proxy counts per-seam fallbacks.

Unported / near-unported modules with a clean oracle (0 or few `rust_` refs):

| Module | LOC | `rust_` refs | Oracle |
|---|---|---|---|
| `mypy/server/update.py` | 1515 | 27 | fine-grained family |
| `mypy/server/deps.py` | 1290 | 23 | fine-grained family |
| `mypy/stubgen.py` | 2099 | 6 | `teststubgen` (373) |
| `mypy/checkpattern.py` | 1124 | 10 | `testcheck` exact |
| `mypy/subexpr.py` (`server/`) | 208 | 0 | `testfinegrained` |
| `mypy/server/aststrip.py` | 259 | 0 | `testfinegrained` |
| `mypy/server/astmerge.py` | 589 | 0 | `testfinegrained` |
| `mypy/server/mergecheck.py` | 84 | 0 | `testfinegrained` |
| `mypy/server/objgraph.py` | 101 | 0 | debug tool |
| `mypy/treetransform.py` | 824 | 0 | `testcheck` |
| `mypy/suggestions.py` | 1081 | 0 | `testsuggestions` |
| `mypy/report.py` | 934 | 0 | `testreports` |
| `mypy/traverser.py` | 1485 | 50 | subclass surface, see floors |

Contention map for dispatch (who is editing what right now): the H1 family
(`656cddbe3`, `f3d1b1dcb`, `b19b9733f` and siblings, plus open PR #1617) touches
`crates/type_kernel/src/{checker_functions,semanal_checks,semanal_visitor,
checkmember,checkexpr_functions,typeops}.rs`, `mypy/{checker,semanal,checkmember,
checkexpr}.py` and `mypy/test/testtypes.py`. The blockers #1618-#1623 are the
`wire.rs` / `mypy/types.py` / `subtypes.rs` / `setops.rs` / `solve.rs` /
`constraints.rs` / `expandtype.rs` / `visitor.rs` / `unify.rs` / `mypy/build.py`
owners. Quiet files: `checkpattern.rs`, `messages.rs`, `meet.rs`,
`serverdeps.rs`, `stubgen.rs`, `traverser.rs`, `binder.rs` and the `mypy/server/`
package.

## 4. (a) Unblocked now -- dispatch today

Ranked by production effect x oracle strength / risk. None of these touches
`wire.rs`, `mypy/types.py`, `subtypes.rs`, `setops.rs`, `solve.rs`,
`constraints.rs`, `expandtype.rs`, `visitor.rs`, `unify.rs` or `mypy/build.py`.

### A1. Native fine-grained dependency walk (`DependencyVisitor`) -- issue #1632

- Anchors: `mypy/server/deps.py:323` `class DependencyVisitor(TraverserVisitor)`,
  **52** `visit_*` methods from `:354` to `:976` plus `:1098 visit_instance`;
  `add_dependency`/`make_trigger`/`Scope` usage throughout.
- Already ported, and the interface to extend: `mypy/server/deps.py:91-97`
  imports `rust_attribute_triggers`, `rust_compute_target_modules`,
  `rust_compute_wildcard_triggers`, `rust_get_type_triggers`,
  `rust_has_user_bases`, `rust_merge_dependencies`, `rust_non_trivial_bases`
  (`crates/type_kernel/src/serverdeps.rs`, 42 pyfunctions). Note
  `deps.py:1079-1080` calls `_rust_get_type_triggers(typ, use_logical_deps)` with
  a **live object, not wire bytes** -- this family is already on the cheap
  interface, so extending it does not add serialize cost.
- Gate: `mypy/server/deps.py:113-119` + `mypy/build.py:1268`
  (`_set_native_server_deps_active(self.options.native_type_kernel)`) --
  **production-live**.
- Moves: the 52 `visit_*` bodies, `Scope`/target-string construction, the
  `deps` map writes.
- Stays Python: the public `Deps`/`get_dependencies` API and its callers in
  `mypy/server/update.py`; `MypyFile.plugin_deps` reads (plugin bridge, #1622
  territory); the `alias_deps` defaultdict produced by semanal and assigned at
  `mypy/server/deps.py:348` (`__init__` signature at `:324`); `TraverserVisitor`
  itself (a subclass surface, see floors). Reason for each: live-object identity
  and plugin-closure ownership, not algorithm.
- Parity plan: new `NativeServerDepsWalkSuite` (direct seam + gate-off/on
  differential over the `deps` map), plus the existing oracles which assert
  dependency maps directly -- `testfinegrained` (747/27), `testfinegrainedcache`
  (549/229), `testdaemon` (37), `testmerge` (41/1), `testdiff` (79) -- plus the
  `testcheck` exact-count gate (local 8,144/69/7; CI 8,198/15/7).
- Measurable target: an env-gated audit counter for Python-body `visit_*` hits
  reaches **0** on the fine-grained corpora, at which point the Python walk is
  deleted in the same PR. Step 0 for the owner: run the survey and record
  `rust_get_type_triggers` / `rust_attribute_triggers` share before and after.
- Dependencies: none. Order the work as (1) expression visitors `:769-976`,
  (2) top-level/def/class visitors `:354-543`, (3) assignment `:547-701`,
  (4) statement visitors `:701-769`, (5) `visit_instance` `:1098`.

### A2. Native pattern-check driver (`PatternChecker`) -- issue #1633

- Anchors: `mypy/checkpattern.py:139` `class PatternChecker(PatternVisitor[PatternType])`;
  drivers `:205 visit_or_pattern`, `:255 visit_value_pattern`,
  `:269 visit_singleton_pattern`, `:284 visit_sequence_pattern` (~170 lines),
  `:573 visit_mapping_pattern`, `:652 visit_class_pattern` (~150 lines);
  helpers `:621 get_mapping_item_type`, `:644 get_simple_mapping_item_type`,
  `:854 _class_pattern_leaves`, `:865 _class_pattern_ranges_from_tags`,
  `:967 generate_types_from_names`, `:978 update_type_map`,
  `:993 construct_sequence_child`.
- Already ported seams to extend: `:467`, `:527`, `:805`
  (`rust_classify_class_pattern_ranges`), `:916` (`rust_should_self_match`),
  `:940` (`rust_can_match_sequence`), `:1006`, `:1054`, `:1088`, `:1113`
  (`crates/type_kernel/src/checkpattern.rs`, 9 pyfunctions, 23 `return None`
  sites).
- Gate: `mypy/build.py:1203` (`_native_checkpattern_active`,
  `options.native_type_kernel`) -- **production-live**.
- Moves: the `visit_*` bodies, the `PatternType` construction, `early_non_match`,
  `visit_tuple`/starred expansion (`:455`, `:519` tails), `should_self_match` /
  `can_match_sequence` bodies (defer branches of already-ported seams).
- Stays Python: `self.update_type_map`/binder writes, `self.msg` emission
  (`PatternChecker.fail` -> `mypy/messages.py`), and `TypeRange` construction
  from live `TypeInfo`s. Reason: binder and error state are live checker
  objects, and message text is the `testcheck` exact-match contract.
- Parity plan: `NativePatternCheckSuite` (gate-off/on differential over
  `PatternType.type`/`rest_type`/`is_sequence_pattern` triples from
  `PatternType` at `:133`), plus the match-statement corpus inside `testcheck`
  exact-count gate and the self-check.
- Measurable target: Python-body hits for the ported arms -> **0** (audit
  counter), and `checkpattern.rs` `return None` sites attributable to the ported
  arms deleted. Order: sequence -> mapping -> class -> or.
- Dependencies: none.

### A3. Native complex-statement checker drivers (try / for / with / match) -- issue #1634

- Anchors: `mypy/checker.py:7345 visit_try_stmt` (~230 lines),
  `:7573 visit_for_stmt` (~300), `:7874 visit_with_stmt` (~110),
  `:7984 visit_match_stmt`, `:7191 visit_if_stmt` (~150). `checker.py` has 46
  `visit_*` methods and **25** `_native_checker_stmts_active` gate references
  (`mypy/checker.py:697-715` definition, gates from `:772`).
- Already ported helpers to compose: `crates/type_kernel/src/checker_stmts.rs`
  `is_unreachable_map_inner` (`:170`), `with_exit_suppresses_inner` (`:250`),
  `try_handler_union_inner` (`:317`), `type_requires_usage_inner` (`:82`),
  `is_valid_inferred_type_inner` (`:453`). `mypy/checkpattern.py` already
  consumes the kernel for `match`.
- Gate: `mypy/build.py:1195` -- **production-live**.
- Moves: the dispatch/narrowing *head* of each driver (reachability gate, frame
  bookkeeping calls, handler/else shaping), mirroring the already-established
  `rust_classify_*` tag protocol (`rust_classify_check_assignment` #1090,
  `rust_classify_return_stmt_*` #1004 are the precedent).
- Stays Python: `self.binder` frame pushes/pops and `analyze_cond_branch`
  (binder is Python-owned until the `native_binder` flip and its type-map port);
  `self.msg` emission; `accept()` recursion into `expr_checker`; plugin hook
  dispatch (#1622). Reason: live mutable checker state plus the emission
  contract.
- Parity plan: `NativeStmtDriverSuite` (per-arm tag tests + gate-off/on
  differential), `testcheck` exact-count gate, self-check 0 errors. The
  narrowing suites already in `testtypes.py` cover the `if`/`try` narrowing.
- Measurable target: per-driver Python-head hits -> 0 for the ported arms; the
  audit bucket names to move are the wave-48b-style per-driver counters. Step 0:
  survey + name the four current call counts.
- Dependencies: none, but **medium contention**: the H1 family is editing
  `mypy/checker.py` (different methods, same file) and `mypy/test/testtypes.py`
  (append-only, see section 6). Branch late in a wave or accept rebases.
- Order: with -> if -> try -> for -> match (ascending size, shared head pattern).

### A4. Native fine-grained server node surgery and subexpression collection -- issue #1635

- Anchors, all with **0** `rust_` references today: `mypy/server/subexpr.py`
  (208 lines, pure expression/type collection with a clean output),
  `mypy/server/aststrip.py` (259, node reset for re-checking),
  `mypy/server/astmerge.py` (589, symbol-graph merge),
  `mypy/server/mergecheck.py` (84), `mypy/server/objgraph.py` (101).
- Gate: none yet -- this is greenfield, so the owner introduces
  `Options.native_server_node_ops` (or reuses `_native_update_active`,
  `mypy/build.py:1273`) and it must be **default-ON** to be worth doing (section 2).
- Moves: `subexpr.py` wholesale first (it is a read-only walk producing
  `(expr -> type)` maps, no identity surgery); then `aststrip.py`'s reset logic;
  `mergecheck.py`/`objgraph.py` are debug tooling and are best left until last.
- Stays Python: `astmerge.py`'s identity-preserving `replace_object_state` /
  cross-reference fixups while the node graph is Python-canonical (Phase G
  constraint, E1 -> F/G decision at `docs/remaining-migration-plan.md`); file I/O
  and the daemon command surface (`mypy/dmypy_server.py`).
- Parity plan: `testfinegrained` (747/27), `testfinegrainedcache` (549/229),
  `testdaemon` (37), `testmerge` (41/1), `testdiff` (79); plus a
  `NativeSubexprSuite` gate-off/on differential on the collected maps.
- Measurable target: audit counter for Python `subexpr`/`aststrip` calls -> 0 on
  the fine-grained corpus; then the Python bodies are deleted.
- Dependencies: none. Mild risk: `astmerge`/`aststrip` are identity-sensitive, so
  keep them out of the first sub-step and say so in the ticket.

### A5. Native stubgen printer and collector family -- issue #1636

- Anchors: `mypy/stubgen.py` (2099 lines; only 2 seams imported at `:173-174`,
  used at `:457` and `:503`), `mypy/stubgenc.py` (1073),
  `mypy/stubutil.py` (900). Rust side: `crates/type_kernel/src/stubgen.rs`
  `rust_stubgen_render` (`:392`), `rust_stubgen_render_type_args` (`:399`),
  `rust_get_assigned_names` (`:420`), `rust_is_none_expr` (`:453`),
  `rust_is_pybind11_overloaded_function_docstring` (`:469`),
  `rust_method_name_sort_key` (`:482`).
- Gate: importability only (`mypy/stubgen.py:177 _HAS_NATIVE_STUBGEN`) --
  **production-live whenever the extension is present**, no opt-in flag.
- Moves: the `StubGenerator` `visit_*` family, `get_str_type_of_node`-style type
  rendering, `AnnotationPrinter`/`AliasPrinter` string emission -- all pure
  text construction over live nodes.
- Stays Python: `main()`/argparse (`mypy/stubgen.py` CLI surface), file I/O,
  `Options`, and the `mypy.parse` invocation. Reason: host/CLI surface, per the
  master plan's "Python remains host, config/CLI surface".
- Parity plan: `teststubgen` is a text-exact oracle (373 passed/1 skipped/2
  xfailed on this head; the 2 xfails are pre-#1547 items unrelated to stubgen:
  `test_infer_sig_from_docstring_args_kwargs_errors` and the data-driven
  `testNestedClassInNamedTuple_semanal-xfail`), plus a `NativeStubgenSuite`
  gate-off/on differential and the `parity-ast` CI job.
- Measurable target: audit counter for Python rendering hits -> 0 on the
  `teststubgen` corpus; then the Python `visit_*` bodies are deleted.
- Dependencies: none. Flag in the ticket that stubgen is **off the type-checker
  critical path** (#1625's endgame is the checker pipeline), so rank it below A1-A3
  if capacity is short.

### A6. Cut per-call wire serialization on scalar-only seams -- issue #1637

- Anchors: the serialize call-site table in section 3 (`mypy/checkpattern.py`
  14, `mypy/messages.py` 13, `mypy/meet.py` 12, `mypy/typeops.py` 35,
  `mypy/checkmember.py` 30). The live-object read path already exists:
  `crates/type_kernel/src/mirror.rs:1053 read_slot` plus the 28 `mirror.rs`
  pyfunctions; `mypy/server/deps.py:1079` is an in-tree example of a seam that
  takes a live object instead of bytes.
- Why this is a slice and not a cleanup: #1624 fix direction 2 names exactly this
  ("Non-wire interface for hot seams ... eliminates serialize/deserialize
  entirely for scalar-fact seams"), and its evidence is a measured diagnose, not a
  guess. This is the one unblocked slice that can improve production **speed**
  while the defer-count work is blocked.
- Moves: for a seam whose Rust body reads only scalar/class facts, replace the
  serialize call at the Python call site with a live-object argument and read it
  through the existing `read_slot` reader.
- Stays Python: the wire format itself (cache and snapshot consumers need it, and
  `mypy/types.py` is excluded from parallel dispatch) -- only the in-process
  seam arguments change.
- Parity plan: `NativeMessagesDeferralSuite` / the affected seam's existing suite
  gate-off/on differential + `testcheck` exact-count gate + self-check parity.
- Measurable target: `MYPY_SERIALIZE_STATS=1` round-trip count on the cold
  self-check falls by a stated absolute number (the owner must record the
  before/after, not a percentage). Secondary: cold self-check type_check share
  from `scripts/measure_work_share.py`, which needs a **quiet host** -- do not
  report it off a loaded machine.
- Dependencies: none for the `checkpattern`/`messages`/`meet` scopes. Scope away
  from `subtypes.py`/`checkexpr.py`/`checker.py` in the first pass (those are the
  swarm's and the blockers' files).

## 5. (b) Blocked by a named open blocker

Mapping the standing audited floors (`docs/HANDOFF.md` "Standing audited floors")
onto the blockers. Do not dispatch these until the blocker moves; the reason
column is the blocker's own, not a re-derivation.

| Slice | Floor count (source) | Blocked by | Note |
|---|---|---|---|
| Flip the semanal gate back to default-ON with a non-wire interface | 121 dark guard sites, 11,687 dark Rust lines | #1624 + `mypy/build.py` contention | two lines at `build.py:1090`/`:1208`; measured net loss until the serialize cost is cut. Highest-value blocked item. |
| `extra_tvars` channel in constraints/solve/unify | 51 `ffi-extra-tvars` + 217 generic-right `cc|cc` | #1618 | wire-format change, `wire.rs` owner |
| Resolver snapshot miss (synthesized TypeInfo) | 74 `inst:left_snap_missing`; site `crates/type_kernel/src/subtypes.rs:2763-2765` (`left_snap.is_none() -> return None`) | #1619 | live-PyO3 `has_base` fallback is the smaller path |
| `definition` / `PartialType` / `meta_level` wire gaps | 37 `format_type_distinctly` -- **stale, see section 7** | #1620 | the residual `PartialType` (astdiff) and `remove_trivial` `meta_gate` (`mypy/expandtype.py:1392`) parts remain real |
| ParamSpec / TypeVarTuple solve path | 463 `lam_typevar` + 90 `need_refresh` (wave-48b audit tags; the tags are instrumentation-only and absent from the tree, see section 7) | #1621 | gates live at `mypy/checkexpr.py:3080-3099` (`need_refresh`, `has_rec_ctx`, `py_dict_kwargs`, `lam_typevar_ctx`) |
| Plugin hook bodies | hook dispatch optimized, bodies Python | #1622 | blocks the `check_callable_call` tail (`mypy/checkexpr.py:2927-3496`) |
| `remove_trivial` partial-list fresh-var identity | `meta_gate=True` at `mypy/expandtype.py:1392` | #1623 | whole-tree seams already work |
| Error-emission coupling (messages.py `report_*` builders) | n/a | #1624, then the 8th blocker | #1625 is explicit: "No issue needed until the performance blocker (#1624) is resolved and error emission becomes the bottleneck". Do not file it. |
| `mypy/join.py` LKV wall | 39 | setops.rs owner + wave-33 recursive-alias guardrail | the `msu(handle_recursive=False)` revert is deliberate |

## 6. (c) Floors -- do not port

| Surface | Reason |
|---|---|
| `TraverserVisitor` base class (`mypy/traverser.py:1485 lines, 185 visit_*`) | it is a **subclass surface**; plugins and mypy internals override `visit_*`. Moving it needs the callback channel (#1622 class) first. Concrete traversals (`has_return_statement`, done #1547) can move; the base cannot. |
| `mypy/treetransform.py` (824, 0 seams) | node **construction**, so it needs a G-phase write path. Python node graph is canonical (E1 -> F/G), not a Phase-2 slice. |
| `mypy/stubtest.py` (2634, 0 seams) | runtime introspection of live Python objects (`inspect`/`importlib`); reimplementing CPython introspection in Rust is not a port, it is a rewrite with no parity oracle. |
| `mypy/report.py` (934), `mypy/suggestions.py` (1081) | opt-in report/annotation-suggestion tooling, off the checker critical path; `suggestions.py` also depends on `treetransform` above. |
| `mypy/errors.py` `remove_duplicates` / `parent_error` identity | `ErrorInfo.write` asserts `parent_error is None`; dedupe semantics break across the wire, and the B6 audit measured **0** traffic on the gate corpus (`docs/HANDOFF.md`, wave 52 / #1479). |
| `mypy/build.py` (6724) | host: build manager, gate wiring, CLI/config surface. Stays Python by the master plan. |
| `mypy/plugins/*.py` hook bodies (`attrs` 1476, `dataclasses` 1375) | plugin closures over `self.chk`; #1622's core, not a Phase-2 slice. |
| `mypy/types.py` / `mypy/nodes.py` class bodies | Phase F/G graph ownership; excluded from parallel dispatch by this brief anyway. |
| `mypy/server/astmerge.py` identity surgery | Phase G (E1) constraint, per A4's "stays Python". |
| message text construction (`mypy/messages.py` `report_*`) | #1625 defers it explicitly until #1624 lands and emission becomes the bottleneck. |
| `scripts/`, `misc/`, `mypy/test/` | tooling and test surface, not the product. |

## 7. Inherited claims verified, corrected, or disproven

The brief asked for each inherited premise to be checked before it reaches a new
issue. Results:

1. **#1621 premise 1 is stale.** "Add a `variables` wire field to `CallableType`"
   -- it is already there: `crates/type_kernel/src/wire.rs:639` `pub variables:
   Vec<Type>,`, written by `write_type_var_likes` (`wire.rs:2395`,
   `:2515`) and read at `wire.rs:1085`/`:1210`. The remaining work is solve-path
   semantics, not wire plumbing.
2. **#1621 premise 2 is stale.** "Port `new_unification_variable` semantics to
   Rust" -- `crates/type_kernel/src/freshen.rs` already handles ParamSpec and
   TypeVarTuple freshening (53 references to `variables`; `Type::ParamSpecType` /
   `Type::TypeVarTupleType` arms at `:424`, `:712-773`, `:843-853`), and
   `rust_freshen_all_functions_type_vars` was measured 33 -> 0 defers, 779 calls
   @ 100% (wave-62C, #1518). Restate #1621 as "solve-path ParamSpec/TVT
   inference", not "variables channel".
3. **#1620 gap 1 is stale.** "Blocks 37 `format_type_distinctly` defers" -- those
   were retired in wave-62D (#1519): 37 -> 0, via `_pretty_hint`
   (`mypy/messages.py:3393`) + `apply_pretty_hint`
   (`crates/type_kernel/src/messages.rs:1899`). Only 1 residual
   `rust_format_type_bare` defer remains and it is a different cause (a
   `_pytest.raises.AbstractRaises[Any]` snapshot miss, per wave-62D).
4. **#1623 item 1 is stale.** "26 defers remain when the gate is unsound" --
   `rust_remove_dups` was measured 26 -> 0 defers, 41 calls @ 100% (wave-62C,
   #1518). `_dedup_alias_identity_sound` still exists
   (`mypy/types.py:5859`, called at `:6201`) but it is now a *precondition guard*
   whose false branch falls back to Python, not a counted FFI defer. An accurate
   restatement needs a new measurement of the false-branch rate.
5. **The brief's own note is wrong**: `_dedup_alias_identity_sound` **does** exist
   in the tree (`mypy/types.py:5859`), tested at `mypy/test/testtypes.py:11200`.
   Do not file "the gate was removed".
6. **#1623 items 2 and 3 are correctly self-flagged** as a permanent floor and as
   already fixed (#1493/#1496 correction); keep them out of new work.
7. **#1624's numbers are the last trustworthy measurement** (2026-08-27:
   total -52.6%, type_check -87.9%) and could not be re-measured here: the host is
   at 42-45 GB combined RSS and `scripts/measure_work_share.py` needs a quiet
   machine. Cite the existing numbers, do not invent new ones.
8. **The "514 `return None`" figure** in `docs/remaining-migration-plan.md` is now
   1,115 on this head. It grew because ports add defer leaves. Do not use it as a
   progress metric (section 3).
9. **`docs/HANDOFF.md` is one wave behind**: its "Open backlog" item 1 still lists
   #1581 (G3.0a) as next, but G3.0a-d and the G2.1-G2.6 shadow families have
   landed (`c89a7cc78`, `25b06dd60`..`b28d89398`) and its head pointer
   (`2a3d36427`) is 20 commits behind `656cddbe3`. Whoever owns the next docs
   refresh should move the head pointer and the backlog.

## 8. Dispatch notes for the swarm

- Run the survey as step 0 of every ticket (section 1 gives the invocation and
  the `$PWD`-first `PYTHONPATH` hazard from `docs/HANDOFF.md` step 2) and paste the
  per-seam numbers into the PR body. This document's ranking is a structural
  proxy; the survey is the oracle.
- `mypy/test/testtypes.py` is append-only and shared by every H1 PR. Rebase by
  taking `origin/main`'s file plus only your own appended suite
  (`docs/HANDOFF.md` "Rebase protocol").
- Keep the probe env-gated and strip it before landing; a negative result closes
  the issue not-planned with the bucket table (precedent #1091/#1109/#1113).
- Comment blocks: max 3 consecutive lines, <=88 chars (pre-commit hook).
- README-level gate before merge: `cargo test`, `cargo fmt --check`,
  `cargo clippy -D warnings`, `testtypes` and `testcheck` with
  `TEST_NATIVE_TYPE_KERNEL=1`, the fine-grained family, and a cold self-check.
  Report `testcheck` in both the local (8,144/69/7) and CI (8,198/15/7) forms.
- Do not grow the section-2 dark surface. If a ticket's only available gate is
  `_native_semanal_*`, `native_binder`, or one of the `native_*_mirror` options,
  stop and re-scope: the port would be parity-covered and production-dead.

## 9. Filed issues

| Issue | Slice | Rank reason |
|---|---|---|
| #1632 | A1 native fine-grained dependency walk | production-live gate, clean map oracle, 52 methods |
| #1633 | A2 native pattern-check driver | production-live gate, type-level oracle, ~500 lines |
| #1634 | A3 native complex-statement checker drivers | production-live gate, ~800 lines, established tag protocol |
| #1635 | A4 native fine-grained server node surgery | greenfield (0 seams), fine-grained oracle |
| #1636 | A5 native stubgen printer family | production-live gate, text-exact oracle, off critical path |
| #1637 | A6 cut per-call wire serialization | only unblocked slice that improves production speed (#1624-aligned) |