# Remaining Rust Migration Plan

Date: 2026-08-13

## Current State

### Metrics (local tree, August 2026)

| Metric | Value |
|--------|-------|
| Rust bytes (local, .rs) | ~3.2M |
| Python bytes (mypy + mypyc, ex test/) | ~7.7M |
| Rust % (local) | ~29.4% |
| Rust % (GitHub languages API) | ~25.6% |
| Rust LOC (crates/) | ~86K |
| Rust source files | 81 (type_kernel) + ast_serialize + module_resolver + fs_probe |
| Gap to 50% | ~4.5M bytes Python->Rust |

## Honest assessment: path to 50%

The user-facing goal is 50% Rust. That requires moving ~2.24M bytes of
Python to Rust (so Rust = 4.48M = Python = 4.48M out of a 8.96M total).

The **biggest Python files cannot be ported** without breaking the
migration plan's Phase E1 constraints:

- `mypy/nodes.py` (191K) and `mypy/types.py` (196K) are plugin-visible
  mutable object graphs. Phase E1 confirms these stay in Python until a
  multi-quarter Rust-owned Type/Node redesign lands.

That leaves **checker.py (484K), checkexpr.py (371K), semanal.py (400K),
build.py (253K)** as the weighted candidates (1.5M combined). Each of
them contains visitor methods that mutate the symbol table, emit errors,
drive daemon-cache invalidation, and interact with plugin hooks. Porting
those visit_* methods to Rust means either:

1. A Rust-native visitor engine that holds live TypeInfo/TypeAlias
   graph references across the GIL (violates Phase E1), or
2. Continuing the strangler-fig per-call gate and porting one visit_*
   method at a time, which yields ~10-30K Rust bytes per PR for parity
   + rebuild cost (100+ PRs to close the gap).

The realistic near-term lever is **deferral reduction** (Phase B), not
#line-adds. That improves performance but adds few Rust bytes. Reaching
50% is therefore a multi-week release line, not a bounded goal-turn
task. The execution sequence that gets there:

1. Merge Phase A fixes (pre-existing bugs).
2. Land Phase B deferral reduction (setops/subtypes/checkexpr) for
   ~50 selected deferral sites; expected ~50-100K new Rust bytes.
3. Land Phase C depth ports for semanal/checker/plugins; expected
   ~150K new Rust bytes per release cycle.
4. Continue until 50% is reached, tracking via Phase D4.

This document itself was updated to keep the honest number close to the
plan rather than let "no progress" silently compound.

### Performance (M17 graduation, cold self-check)

| Phase | Python baseline | Native (prod) | Reduction |
|-------|----------------|---------------|-----------|
| parse_time | 5.046s | 4.997s | 1.0% |
| semanal_time | 2.716s | 1.110s | 59.1% |
| type_check_time | 9.951s | 2.326s | 76.6% |
| **Total** | **17.713s** | **8.433s** | **52.4%** |

### Native gates shipped (production default-on)

`native_type_kernel` defaults on. All of the following are active:

- erasetype (erase_type, remove_instance_last_known_values, erase_typevars)
- subtypes (is_subtype, is_proper_subtype + resolver)
- join (join_types + resolver)
- mro (linearize_hierarchy + resolver)
- meet (narrow_declared_type, is_overlapping_types, get_possible_variants)
- expandtype (expand_type + resolver)
- maptype (map_instance_to_supertype + resolver)
- typeops (bind_self, class_callable, fill_typevars + resolver)
- semanal (visitor, _lookup, shared, classprop, typeddict, namedtuple + resolver)
- typeanal (instance validation, vec type args, fix_instance + active)
- checker (stmts, visitor, narrowing, binder + resolver)
- checkexpr (plugin-hook registry, star_expr, conditional_expr, container literals,
  operators, check_callable_call arg-binding, check_argument_types, overload dispatch + resolver)
- checkmember (bind_self_fast, operator helpers, method fast path + resolver)
- checkpattern, checkstrformat
- constraints + solve (full infer + solve path + resolver)
- argmap, applytype (active + resolver)
- freshen, lkv, cache
- messages (format_type family, notes, suggestions, callable_name + resolver)
- copytype
- serverdeps, server update
- dmypy_server (check, command_run, command_recheck, merge_hook_results)
- plugins (attrs, dataclasses, common, functools, hook dispatch)
- stubgenc (AliasPrinter)
- nodes, sharedparse, errors, reachability, fixup, modulefinder, util

### Parity baselines

| Suite | Result |
|-------|--------|
| testcheck.py (all native on) | 8143 passed, 8 pre-existing failures, 69 skipped, 7 xfailed |
| testtypes.py (parity suites) | 339 passed |
| testfinegrained + testdaemon + testfinegrainedcache | 1333 passed |
| Self-check (mypy_self_check.ini) | 0 errors |
| Rust unit tests | 46+ passed |

The 8 pre-existing testcheck failures are NOT caused by the type kernel.
They fail on clean main without any native options too:
"De-serialization failure: TypeInfo not fixed" (6 tests) and
the now-fixed `py_join` tuple bug (2 tests, fixed in PR #584).

### Open issues

- #582: 34 testcheck failures with type kernel (addressed by PR #584, reduced to 8 pre-existing)
- #580: Wire `rust_is_typevar_default_recursive` and `rust_format_messages_default_pretty` (shipped in #583)

The migration plan's module port list is exhausted. All planned modules are ported.
Remaining work is optimization, deferral reduction, and fixing pre-existing bugs.

## What Remains

### Phase A: Fix pre-existing bugs (blocking)

**A1: "TypeInfo not fixed" de-serialization crash (6 tests)**

The 6 incremental/serialization tests crash with
`AssertionError: De-serialization failure: TypeInfo not fixed`.
This is a pre-existing bug on main, not caused by the type kernel.

Root cause hypothesis: `TypeInfo.__getattribute__` raises when `deserialize`/
`fixup.py:NodeFixer` has not yet patched cross-references. The crash triggers
on worker-subprocess paths (`MYPY_NUM_WORKERS > 0`) and in incremental cache
reload. The `fixup.py` port (`crates/type_kernel/src/fixup.rs`) may have a
gap where a `TypeInfo` variant is not being fixed up after deserialization.

Steps:
1. Reproduce with `MYPY_NUM_WORKERS=1 .venv/bin/python -m mypy --config-file
   mypy_self_check.ini -p mypy` on clean main (no native options).
2. Add `--pdb` to get a breakpoint at the `__getattribute__` assertion.
3. Trace which `TypeInfo` attribute access triggers the assertion.
4. Compare the Python `fixup.py:NodeFixer` against `crates/type_kernel/src/fixup.rs`
   to find the missing fixup branch.
5. Fix the gap, rebuild, re-run the 6 tests + self-check.

Priority: high (blocks worker-subprocess mode, affects 6 testcheck tests).

**A2: Close issue #582**

PR #584 reduces the 34 failures to 8 (all pre-existing). Once A1 is fixed,
update #582 and close it. If A1 requires a separate PR, reference both.

### Phase B: Reduce deferrals (performance)

The Rust kernel has 514 `return None` (defer to Python) sites. Each deferral
means a Python fallback path runs, erasing the performance win for that call.
The highest-deferral files:

| File | Deferrals | Module |
|------|-----------|--------|
| setops.rs | 64 | set operations (union/intersection narrowing) |
| checkexpr_functions.rs | 42 | expression checker visitors |
| subtypes.rs | 40 | subtype checks |
| constraints.rs | 35 | constraint inference |
| checkmember.rs | 26 | member access |
| typeops.rs | 24 | type operations |
| meet.rs | 21 | meet/narrowing |
| checkpattern.rs | 21 | pattern matching |
| checkcall.rs | 20 | call checking |
| messages.rs | 17 | message formatting |
| expandtype.rs | 17 | type expansion |
| checker_stmts.rs | 15 | statement checking |

**B1: setops.rs (64 deferrals)**

`setops.rs` (7285 lines, the largest Rust file) has the most deferrals.
This is the union/intersection type-operation kernel. Reducing deferrals
here directly improves type-check time.

Strategy: audit each `return None` against the Python `mypy.types.UnionType`
and `mypy.types.IntersectionType` methods. Most deferrals are for:
- `TypeVarType` / `TypeVarTuple` / `ParamSpec` operands (need wire codec support)
- `LiteralType` with complex values
- `TypeAliasType` (needs alias expansion before the operation)
- Plugin-modified types

Steps:
1. Add a deferral counter to each `return None` site (temporary instrumentation).
2. Run the self-check corpus with the counter and sort by hit count.
3. Pick the top 5 deferral sites, implement them in Rust.
4. Re-run parity suite + testcheck to confirm no regressions.
5. Measure the type_check_time improvement.

Priority: medium (performance, not correctness).

**B2: subtypes.rs (40 deferrals)**

The subtype kernel defers on complex generic substitution edges. Most were
fixed in the M8bb gap closure (all unsupported edges defer instead of
returning wrong answers), but the remaining deferrals are real gaps.

Strategy: same as B1. Focus on:
- `visit_instance_nominal` with multi-level inheritance + variance
- `check_type_parameter` with `ParamSpec` / `TypeVarTuple`
- `visit_union` with mixed `Instance`/`TypeVar` members

**B3: checkexpr_functions.rs (42 deferrals)**

Expression-checker visitors that defer on complex expression forms.
Focus on:
- `visit_lambda` (arg inference)
- `visit_yield_expr` (generator type flow)
- `visit_await` (async type flow)

### Phase C: Deepen ported modules

**C1: semanal.py symbol resolution depth (issue #348 family)**

The #348 family (A/B/C) landed: `_lookup`, import binding, member resolution.
But `semanal.py` is 9139 lines, and many visitor methods still have Python
fallback paths. The biggest remaining gap is the full `visit_class_def`
and `visit_func_def` traversal, which mutates the symbol table.

Strategy: port the pure-computation parts of `visit_class_def` /
`visit_func_def` (scope creation, MRO setup, decorator classification)
and leave the mutation-bearing parts (AST writes, error emission) in Python.

**C2: checker.py complex-statement depth**

`checker.py` is 10627 lines. `checker_stmts.rs` (1498 lines) ports the
statement-level helpers, but the full `visit_*` methods for complex
statements (try/except, match, with) have deferral gaps.

Strategy: audit `checker_stmts.rs` against `mypy/checker.py` visit methods.
Port the type-narrowing logic for try/except and match statements.

**C3: Plugin depth**

`mypy/plugins/default.py` (654 lines) has ~46 hook fullnames in the
`PluginHookRegistry`. The hook *dispatch* is optimized (known-absent
skip), but the hook *bodies* run in Python. Porting the pure-computation
parts of `DefaultPlugin` hooks (e.g. `get_function_hook` for `builtins.len`)
would eliminate Python fallback for common cases.

### Phase D: Infrastructure and tooling

**D1: Fix the `py_join` / `rust_compute_search_paths` unconditional call**

`compute_search_paths` in `mypy/modulefinder.py` calls
`_rust_compute_search_paths` unconditionally whenever `_HAS_RUST_MODULEFINDER`
is true, regardless of `native_resolver`. This means the modulefinder
Rust extension is always active even when the user passes `--no-native-resolver`.
This is probably intentional (search-path computation is not cache-affecting),
but it should be documented or gated if it causes issues.

**D2: Document the build order hazard**

The `AGENTS.md` already documents the stale-binary hazard and the
`cargo rustc` + scratch-dir approach. Ensure the `remaining-migration-plan.md`
references this for new contributors.

Done as part of #596: new contributors should read the "Native parser build
order", "Type kernel build order", and "Native resolver / dependency-records
parity" sections at the repo root `AGENTS.md` before rebuilding any Rust
extension. The cardinal rule: rebuild the `.so` into a scratch dir and put it
on `PYTHONPATH` after any `.rs` change, and never use `maturin develop` (it
picks up the repo-root `pyproject.toml` and installs a bogus `mypy-0.1.0`
package that shadows the real mypy).

**D3: Performance regression tracking**

The M17 benchmark (52.4% build-time reduction) is the current baseline.
After Phase B (deferral reduction), re-run the self-check benchmark to
measure the improvement. Target: 60%+ total reduction.

Measured 2026-08-14 (after Phase C merged, fresh `type_kernel` release
`.so`, cold cache, `MYPY_NUM_WORKERS=0`, self-check
`mypy_self_check.ini --no-incremental -p mypy`):

- Native kernel on (`native_type_kernel`, default): `type_check_time`
  50.3s, wall 58.6s, 117 self-check errors.
- Kernel off (`--no-native-type-kernel`): `type_check_time` 15.6s,
  wall 22.3s, 116 self-check errors.

The type-check gap (~3.2x) is a regression vs the M17 baseline
(2.3s native type_check). Root cause: `_build_native_resolvers()` is
called once per SCC (394 SCCs in the self-check) and each call
re-serializes the FULL loaded TypeInfo graph (~8490 TypeInfos) plus
the alias graph through Rust getattr walks, charged into
`type_check_time` (t3..t4 in `process_stale_scc`). Even
`--no-native-type-kernel` leaves `native_resolver` / `native_parser`
on, so the 116-error pure config is a different comparison axis; the
117-vs-116 delta is a pre-existing self-check inference sensitivity at
`checker.py:6676` (`_get_base_classes`), unchanged by Phase D.

Fix direction (tracked, NOT shipped here): snapshot a TypeInfo only
when its defining SCC is sealed (right after its `semantic_analysis_for_scc`
runs), build/extend an incrementally-accumulating resolver instead of a
full per-SCC rebuild, and track a seen-set reset by daemon recheck. The
member-info / member-definer walks are functionally required (blanking
them breaks parity: 135 errors), so they are not the fixable part. This
is perf work for a dedicated phase, not Phase D scope.

**D4: Rust % measurement**

After Phase B + C, re-measure the GitHub languages API Rust %. The
local tree is already ~32%; the GitHub API lags because it counts
generated/stub files differently. Target: 30%+ on the GitHub API.

Measured 2026-08-14 (after Phase C merged): GitHub languages API reports
Rust 3,156,702 bytes of 9,453,609 total = **33.4%**, above the 30% target.
This matches the committed `.rs` source (`crates/*/src`, ~3.2 MB) —
GitHub's count tracks the real Rust tree.

### Phase E: Long-term architecture decisions

Evaluated 2026-08-14 (after A-D complete). The three items are
decision records against current reality; none requires a port today.

**E1: nodes.py / types.py (out of scope, confirmed)**

`mypy/nodes.py` and `mypy/types.py` remain the "widely shared mutable
object graphs" the migration plan says NOT to port. They are
plugin-visible, cache-serialized, and identity-sensitive. Nothing in
A-D changed that calculus. The Rust `Type` enum + binary wire reader
(`wire::read_type_to_str`, `typeinfo::read_type_to_str_with_resolver`,
Stage 3a) is parity-tested foundation for a possible `is_subtype`
port, but enabling it in production would require the full cost list
below; it stays off until the migration has proven stable over time.

A future port would require:
- A Rust-owned `Type` enum with Python proxy objects
- Plugin API changes (hooks receive Rust types, not Python objects)
- Cache format changes (Rust-owned serialization)
- Daemon mode changes (identity preservation across incremental updates)

Multi-quarter effort. Do not start while the strangler-fig per-call
gates (Rust returns a value or `None`, Python falls back) are still
carrying the load; a wholesale `Type`/`Node` reimplementation removes
that fallback safety net.

**E2: Daemon FS ownership (already Rust-backed; no port warranted)**

The daemon (`fine_grained_incremental`) reads the filesystem through
`FileSystemCache` (`mypy/fscache.py`), which delegates every method to
the Rust `module_resolver.FsCache` pyclass. That includes the
transactional per-flush snapshot semantics and the Bazel fake
`__init__.py` synthesis (`crates/module_resolver/src/fs_cache.rs`).
`dmypy_server` passes the shared `self.fscache` into `build`,
`update`, and snapshot paths, so the daemon VFS reads are Rust-owned.

The one remaining Python-owned FS-adjacent piece in the daemon is
`FileSystemWatcher` (`mypy/fswatcher.py`, 106 lines): stat + hash
change-detection over watched paths. It performs no direct OS calls;
every read goes through the Rust-backed cache. Porting its diffing
logic to Rust would remove a cache miss per watched path per
`find_changed`, dwarfed by the hashing cost itself. Decision: keep the
watcher in Python. It is a thin algorithmic layer over an already-Rust
FS layer; a port is not a meaningful lever.

Bazel stays on the Python `_find_module` resolver by gate
(`_native_gate_active` excludes `options.bazel`); its fake-init
synthesis remains Python-owned in the pure-Python fallback
(`fscache.py`, `_fake_init_py`). This is deliberate: the native
resolver reads the real FS through `FsCache`, which does not observe
Bazel's virtual FS.

**E3: mypyc coexistence (assessed, not exercised locally)**

mypyc compiles mypy's Python to CPython C extensions
(`setup.py --use-mypyc`); the Rust work is shipped as separate
PyO3 cdylib extensions (ast_serialize, module_resolver, type_kernel)
loaded by `PYTHONPATH` import. At the CPython ABI level the two
coexist: mypyc C modules and PyO3 modules are independent extensions,
so there is no symbol or GIL conflict in principle.

Known coexistence hedge already in the tree: `FileSystemCache` is
marked `@mypyc_attr(allow_interpreted_subclasses=True)` so a
mypyc-compiled build can still spawn interpreted subclasses in tests.

The local dev workflow does not compile mypy with mypyc (no
`build/native`), so E3 is an assessment, not an exercised guarantee.
A future milestone could compile the Python fallbacks with mypyc AND
use the Rust extensions, getting both speeds; the two documented
hazards to watch are mypyc-attr subclassing rules and the
`PYTHONPATH`-overwrite behavior seen in the daemon test harness
(prepends now, so Rust dirs survive). No conflicts have been
observed; no work is scheduled.

## Execution Order

```
Phase A (blocking):
  A1: Fix "TypeInfo not fixed" de-serialization crash
  A2: Close issue #582

Phase B (performance, parallel):
  B1: setops.rs deferral reduction (highest hit-count sites first)
  B2: subtypes.rs deferral reduction
  B3: checkexpr_functions.rs deferral reduction

Phase C (depth, after B):
  C1: semanal.py visit_class_def / visit_func_def pure parts
  C2: checker.py try/except + match narrowing
  C3: DefaultPlugin hook body porting

Phase D (infrastructure, parallel with B/C):
  D1: Document/gate rust_compute_search_paths
  D2: Build order documentation
  D3: Performance regression tracking
  D4: Rust % measurement

Phase E (long-term, after A-D complete; decision records, no ports):
  E1: nodes.py / types.py, confirmed out of scope
  E2: Daemon FS ownership, confirmed Rust-backed (fscache); watcher stays Python
  E3: mypyc coexistence, assessed but not exercised locally
```

## Contract for every change

Every port/fix follows the strangler-fig contract:
1. Rust returns a result or `None`; on `None`, Python falls back.
2. Gate is `TEST_NATIVE_TYPE_KERNEL=1` for new ports, default-on for
   graduated ports.
3. Differential parity: run with and without the flag, confirm
   identical testcheck + testtypes + testfinegrained results.
4. Full parity suite green before merge.
5. Self-check diagnostic parity (byte-for-byte identical output).
6. Rebuild the `.so` after any `.rs` change (stale-binary hazard).
