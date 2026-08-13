# Remaining Rust Migration Plan

Date: 2026-08-13

## Current State

### Metrics (local tree, August 2026)

| Metric | Value |
|--------|-------|
| Rust bytes (local, .rs) | ~3.1M |
| Python bytes (mypy + mypyc, ex test/) | ~6.6M |
| Rust % (local) | ~32% |
| Rust % (GitHub languages API) | ~25.6% |
| Rust LOC (crates/) | ~86K |
| Rust source files | 81 (type_kernel) + ast_serialize + module_resolver + fs_probe |

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

**D3: Performance regression tracking**

The M17 benchmark (52.4% build-time reduction) is the current baseline.
After Phase B (deferral reduction), re-run the self-check benchmark to
measure the improvement. Target: 60%+ total reduction.

**D4: Rust % measurement**

After Phase B + C, re-measure the GitHub languages API Rust %. The
local tree is already ~32%; the GitHub API lags because it counts
generated/stub files differently. Target: 30%+ on the GitHub API.

### Phase E: Long-term architecture decisions

**E1: nodes.py / types.py (explicitly out of scope)**

`mypy/nodes.py` (5675 lines) and `mypy/types.py` (5221 lines) are the
"widely shared mutable object graphs" the migration plan says NOT to
port. They are plugin-visible, cache-serialized, and identity-sensitive.

A future port would require:
- A Rust-owned `Type` enum with Python proxy objects
- Plugin API changes (hooks receive Rust types, not Python objects)
- Cache format changes (Rust-owned serialization)
- Daemon mode changes (identity preservation across incremental updates)

This is a multi-quarter effort and should only start after Phases A-D
are complete and the migration has proven stable in production.

**E2: Daemon VFS port (deferred)**

The daemon (`fine_grained_incremental`) uses native resolution (the
`_native_gate_active` exclusion was dropped in Phase 2). But the daemon
VFS (virtual filesystem for in-memory edits) is still Python-owned.
Porting the VFS to Rust would eliminate the last Python-owned FS path.

**E3: mypyc integration**

mypyc compiles mypy to C extensions. The Rust extensions coexist with
mypyc-compiled Python. A future milestone could compile the Python
fallbacks with mypyc AND use the Rust extensions, getting both speeds.
Current status: coexisting, no conflicts observed.

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

Phase E (long-term, after A-D complete):
  E1: nodes.py / types.py evaluation
  E2: Daemon VFS port
  E3: mypyc integration
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
