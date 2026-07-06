# Rust Migration Feasibility and Strangler Plan

Date: 2026-07-02

This note records a local feasibility assessment for migrating mypy toward Rust
piece by piece. It is an exploratory architecture report, not an accepted
project decision.

## Summary

A gradual Rust migration is feasible, but a direct rewrite of mypy is not a good
first move.

The existing architecture already contains one useful strangler pattern:
`mypy.parse.parse()` can choose the Python parser or the native parser. The
native parser path uses `mypy.nativeparse`, which consumes the `ast-serialize`
Rust extension and returns serialized AST data that is later materialized into
the existing Python `MypyFile`/node objects.

That is the right shape for the rest of the migration: keep Python-facing
interfaces stable, add Rust adapters behind narrow interfaces, and migrate
behavior only when parity is measurable.

## Repository Facts Observed

- `README.md` documents that mypy is already compiled with mypyc and is about
  4x faster than interpreted Python.
- `pyproject.toml` declares `ast-serialize>=0.6.0,<1.0.0` and `librt>=0.12.0`.
- There is no Rust source checked into this repository today.
- The native parser is exposed through `mypy.parse` and implemented by
  `mypy.nativeparse`.
- The current core checker object model is Python:
  - `mypy.nodes.Node`
  - `mypy.nodes.MypyFile`
  - `mypy.nodes.TypeInfo`
  - `mypy.types.Type`
  - `mypy.types.Instance`
  - `mypy.types.CallableType`
- A mechanical import scan found:
  - 97 direct importers of `mypy.nodes`
  - 85 direct importers of `mypy.types`
- Approximate source size scanned:
  - `mypy`: 195 Python files, 128,951 LOC, 572 classes, 6,510 functions
  - `mypyc`: 137 Python files, 46,719 LOC, 218 classes, 2,462 functions

## Main Constraint

`mypy.nodes` and `mypy.types` are not good first migration targets. They are
mutable Python object graphs used throughout semantic analysis, type checking,
incremental mode, cache loading, tests, and plugin hooks.

Migrating those first would force most of the codebase to understand Rust-owned
objects or wrapped proxy objects. That creates high risk around:

- Python plugin compatibility
- daemon object identity
- fine-grained incremental updates
- cache compatibility
- mypyc compilation behavior
- reference ownership across the Python/Rust boundary

For a long time, Rust modules should produce or consume plain records, bytes, or
stable IDs, while Python continues to expose the compatibility object model.

## Recommended Migration Order

### 1. Productionize the Native Parser

Recommendation: strong.

Relevant files:

- `mypy/parse.py`
- `mypy/nativeparse.py`
- `mypy/test/test_nativeparse.py`
- `test-data/unit/native-parser*.test`

Current shape:

- `mypy.parse.parse()` chooses the old parser or native parser based on
  `Options.native_parser`.
- The native parser returns raw serialized AST data plus import metadata.
- Python still materializes `MypyFile` and child nodes.

Plan:

- Expand parser parity tests.
- Run broad checker tests with `TEST_NATIVE_PARSER=1`.
- Make native parser default in a limited mode first. ✓ `Options.native_parser` now defaults to `True`; the Python parser remains available via `--no-native-parser`. The test harness still forces the Python path unless `TEST_NATIVE_PARSER=1` so both paths stay covered.
- Remove or demote the Python parser only after behavior is stable.

Why this is first:

- It already exists.
- Its interface is narrow.
- It avoids Rust ownership of the checker graph.
- It has clear behavior and performance expectations.

### 2. Add a Rust Module Discovery and Import Graph Prepass

Recommendation: strong.

Relevant files:

- `mypy/modulefinder.py`
- `mypy/find_sources.py`
- `mypy/fscache.py`
- `mypy/build.py`

Plan:

- Add a Rust-backed resolver that returns plain records:
  - module id
  - path
  - package status
  - dependency names
  - dependency priorities
  - dependency line numbers
  - suppressed/missing dependency records
- Keep Python responsible for diagnostics, options policy, `State` construction,
  and build orchestration initially.

Why this is a good strangler target:

- It is string/path heavy.
- It can be tested against existing modulefinder and command-line tests.
- It avoids changing `mypy.types` or checker semantics.
- It can improve cold start and graph-load performance without changing plugin
  behavior.

**Status:** the Rust resolver (`crates/module_resolver`), dependency-records
extraction, and shared `FsCache` backing all landed in Milestone 2 and are
now default-on across every path except Bazel (`Options.native_resolver`
defaults to `True` as of Phase 3; the daemon and parallel mode force it on
regardless). The per-module `find_module` lookup is still Python-orchestrated
— the import-graph prepass that replaces it with a single traversal remains
future work (see "Next milestone" below).

### 3. Move Cache Indexing and Validation Below Python Object Materialization

Recommendation: worth exploring.

Relevant files:

- `mypy/cache.py`
- `mypy/build.py`
- `mypy/metastore.py`
- `mypy/nodes.py`
- `mypy/types.py`

Current shape:

- mypy already has a fixed-format binary cache.
- Many Python classes implement `read()` and `write()` methods directly.

Plan:

- Keep Python object materialization in Python.
- Add Rust below it for:
  - validating fixed-format records
  - indexing cache files
  - hashing/cache metadata operations
  - streaming slices of binary data to Python readers

Why this is not first:

- The schema is distributed across many Python classes.
- Cache changes affect incremental correctness.
- Fine-grained cache behavior is subtle.

### 4. Add a Pure Type-Operations Kernel

Recommendation: worth exploring, but only after the earlier seams prove out.

Relevant files:

- `mypy/subtypes.py`
- `mypy/join.py`
- `mypy/meet.py`
- `mypy/typeops.py`
- `mypy/types.py`
- `mypy/test/testtypes.py`

Plan:

- Do not port `mypy.types` first.
- Introduce a Python facade for selected type operations.
- Encode supported pure type subsets as stable IDs or compact records.
- Call a Rust kernel for supported cases.
- Fall back to Python for unsupported, plugin-sensitive, recursive, or
  identity-sensitive cases.

Risks:

- Conversion overhead may erase performance wins.
- The type system has many semantic edge cases.
- Plugins and special cases may require Python fallback for a long time.

### 5. Defer Full Semantic Analyzer and Checker Rewrite

Recommendation: speculative.

Relevant files:

- `mypy/semanal.py`
- `mypy/semanal_main.py`
- `mypy/checker.py`
- `mypy/checkexpr.py`
- `mypy/plugin.py`
- `mypy/server/update.py`

Reason to defer:

- These modules are large, plugin-aware, and mutate shared AST/type state.
- The daemon keeps ASTs and type maps in memory across incremental runs.
- Fine-grained mode preserves object identity and merges ASTs.
- A second checker implementation would create long-term parity risk.

If attempted later, start at target-level checking behind the existing daemon
target model instead of replacing the whole checker at once.

## Test and Coverage Notes

Normal local test setup:

```bash
uv sync
uv run test
```

`uv run test` runs the default local suite. According to
`runtests.py`, that excludes the opt-in `pytest-extra`, `mypyc-fast`, and
`mypyc-extra` groups.

More exhaustive local run:

```bash
uv run all
```

Direct check commands:

```bash
uv run pytest
uv run lint
uv run typecheck
```

Coverage command:

```bash
uv run pytest --cov=mypy --cov-branch --cov-report=term-missing
```

Coverage is configured in `pyproject.toml` with branch coverage over `mypy`,
parallel collection, and `mypy/test/*` omitted from reports.

For Rust migration work, line coverage is not enough. Migration-specific gates
should include:

```bash
TEST_NATIVE_PARSER=1 uv run pytest mypy/test/testcheck.py
uv run pytest mypy/test/test_nativeparse.py
uv run pytest mypy/test/testfinegrained.py mypy/test/testdaemon.py
```

For checker/type-system behavior changes, use `mypy_primer` as a differential
test over real projects.

## CI Coverage

This fork does not run GitHub Actions (no CI credits). The workflow files that
upstream mypy ships under `.github/workflows/` have been removed. Parity is
validated locally instead, via the native-parser test suites, the daemon and
incremental suites, the mypy self-check, and (when needed) a local
`mypy_primer` differential run. Add CI back only when there is a hosted runner
to run it on.

## Suggested First Milestone

Milestone 1 should not introduce a broad Rust workspace rewrite. It should make
the existing native parser seam production-grade.

Definition of done:

- Native parser test suite passes.
- Type checker data-driven suite passes with `TEST_NATIVE_PARSER=1`, except
  explicitly documented unsupported cases.
- Incremental and daemon tests pass in native parser mode where applicable.
- Parser output parity is tracked in test data.
- Performance is measured against the current parser path.
- The fallback path remains available until the local parity baselines above
  prove stable.

After that, start the module discovery/import graph prepass.

## In-Tree Rust Parser Replacement Status

The repository now has the start of an owned Rust replacement for the external
`ast-serialize` wheel:

- `Cargo.toml`
- `crates/ast_serialize/Cargo.toml`
- `crates/ast_serialize/src/lib.rs`

The crate builds a Python extension module named `ast_serialize` and preserves
the existing Python API shape:

```python
parse(...) -> tuple[bytes, list[ParseError], TypeIgnores, bytes, ASTData]
```

Current correctness status:

- Uses Ruff's Rust parser crates for the Rust-side Python parse.
- Owns the mypy-specific translation layer in this repo:
  Ruff AST -> mypy native binary AST bytes.
- Serializes the existing binary AST format for a small first slice:
  expression statements, calls, names, strings,
  member access, binary operators, small integer literals, tuples, lists, sets,
  dictionaries, index and slice expressions, boolean operations, comparisons,
  unary operations, `None`/boolean/ellipsis literals, float/complex/bytes/big
  integer literals, bytes literals with escaped display payloads, and plain
  assignments.
- Serializes simple statement tags for augmented assignment, return, pass,
  raise, assert, delete, break, continue, global, and nonlocal.
- Serializes annotated assignments for the supported type-expression subset,
  including no-RHS assignments through `TempNode`, assignment type comments,
  and nested list/tuple assignment targets.
- Serializes function definitions, including function blocks,
  positional-only parameters, positional parameters, keyword-only parameters,
  `*args`, `**kwargs`, defaults, async functions, parameter annotations,
  return annotations, and decorated functions.
- Serializes a growing type-expression subset for annotations: unbound names,
  dotted names, subscripted types, PEP 604 unions, list type arguments,
  ellipsis type arguments, unpacked type arguments, `Arg(...)` callable
  argument constructor calls, invalid-expression fallbacks, literal
  string/bytes/int/bool values, string forward references, and PEP 695 type
  parameters/type aliases.
- Serializes `if`/`elif`/`else`, `while`/`else`, `for`/`else`, `with`, and
  `try`/`except`/`finally` statements.
- Serializes comprehensions and generators, lambdas, conditional expressions,
  named expressions, yield expressions, and skip-function-body handling for
  `# mypy: ignore-errors=True`.
- Serializes f-strings through the native parser f-string wire format,
  including conversion flags, format specifiers, nested format-spec
  expressions, and debug f-strings.
- Serializes match statements and the pattern forms covered by the native
  parser suite: class, value, singleton, or, sequence/star, mapping, capture,
  wildcard, and guarded cases.
- Serializes class definitions with bodies, base expressions, decorators,
  metaclass and other class keyword arguments.
- Serializes `import`, `from ... import ...`, and `from ... import *`
  statements.
- Serializes call positional, keyword, `*args`, and `**kwargs` argument
  metadata for supported argument expressions, using mypy's canonical
  positional-then-keyword call argument order.
- Serializes import side-channel metadata in the format expected by
  `mypy.nativeparse.deserialize_imports`, including top-level flags,
  function-local import flags, mypy-only flags for `TYPE_CHECKING`/`MYPY`
  blocks, and basic reachability for `PY2`/`PY3`, boolean operators, and
  `sys.version_info`/`sys.platform` comparisons.
- Preserves native import dependency-discovery behavior for unreachable branch
  imports without reintroducing missing-import diagnostics for dead code after
  top-level always-failing asserts.
- Serializes type-ignore side-channel metadata for `# type: ignore` comments,
  including bracketed error-code lists.
- Preserves inline `# mypy: ...` comments and native raw-load behavior for
  whole-module ignores and top-level always-failing asserts.
- Preserves expression-statement source locations, type-comment diagnostics for
  invalid function signatures, invalid call annotations, duplicate signatures,
  and type-ignore parsing edge cases covered by the fastparse tests.
- Preserves Ruff parser recovery errors and recovered ASTs for the syntax
  error cases covered by `mypy/test/test_nativeparse.py`.
- Uses the same short and long integer byte encoding as `librt` for all bare
  integer fields currently emitted by the Rust serializer.
- Matches the current mypy cache/node tag constants for this branch.
- Raises a normal `UnicodeDecodeError` for invalid UTF-8 byte input.
- Passes the Rust unit test for the trivial binary AST contract.
- When built as a local extension and placed ahead of the installed wheel on
  `PYTHONPATH`, passes `TestNativeParserBinaryFormat`.
- The first native parser data cases now pass with the local extension, and
  the full native parser suite currently has a concrete local-extension
  baseline.

Verification run locally:

```bash
cargo test -p mypy-ast-serialize
cargo rustc -p mypy-ast-serialize --features extension-module --lib \
  --crate-type cdylib -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/debug/libast_serialize.dylib \
  /private/tmp/mypy-rs-local-ast/ast_serialize.cpython-313-darwin.so
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m pytest \
  'mypy/test/test_nativeparse.py::TestNativeParserBinaryFormat' -q
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m pytest \
  'mypy/test/test_nativeparse.py::NativeParserSuite::native-parser.test::testHello' \
  'mypy/test/test_nativeparse.py::NativeParserSuite::native-parser.test::testMemberExpr' \
  'mypy/test/test_nativeparse.py::NativeParserSuite::native-parser.test::testTupleExpr' \
  'mypy/test/test_nativeparse.py::NativeParserSuite::native-parser.test::testOpExpr' \
  'mypy/test/test_nativeparse.py::NativeParserSuite::native-parser.test::testAssignmentStmt' \
  -q
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m pytest mypy/test/test_nativeparse.py -q \
  -k 'SimpleFunction or FunctionWithArgs or FunctionWithVarArgs or FunctionWithKwargs or FunctionWithKwOnly or FunctionWithAllArgKinds or AsyncFunction or FunctionWithDefaultArg or FunctionWithMultipleDefaults or FunctionMixedDefaultsAndRegular or FunctionWithKwOnlyDefault'
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m pytest mypy/test/test_nativeparse.py -q \
  -k 'IfStmt or WhileStmt'
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m pytest mypy/test/test_nativeparse.py -q \
  -k 'IntExpr'
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m pytest mypy/test/test_nativeparse.py -q \
  -k 'SimpleClass or ClassWithMethod or ClassWithSingleBase or ClassWithMultipleBases or Metaclass or ClassWithKeywordArgs or ClassDecorator'
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m pytest mypy/test/test_nativeparse.py -q \
  -k 'NativeParserImportsSuite'
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m pytest mypy/test/test_nativeparse.py -q \
  -k 'NativeParserImportsSuite or TestNativeParserBinaryFormat or BytesLiteral or AnnotatedAssignment or LiteralStringType or LiteralStringWithEscapes or ForwardReference or FunctionSignature or UnionTypes or FunctionWithEllipsisCallableType or FunctionWithCallableType or FunctionWithEmptyCallableType or FunctionWithComplexCallableType or DecoratedFuncDef or FunctionOverload or RaiseStatements or AssertStatements or GlobalAndNonlocal or DelStmt or StarExpression or AwaitExpression or ForStatements or WithStatements'
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m pytest mypy/test/test_nativeparse.py -q
```

Current local-extension baseline for the import side-channel suite:
`78 passed`.

Focused native parser slices added in the latest expansion all pass:
`TypeIgnores`, `NestedListAssignment`, `TypeComment`, `ArgConstructor`,
`CallableWithArg`, `InvalidType`, `FString`, `Match`, `PEP695`, and
`SyntaxError`.

Current local-extension baseline for the full native parser suite:
`254 passed`.

Current local-extension baseline for full native-parser `testcheck.py`:
`8144 passed, 69 skipped, 7 xfailed`.

Current local-extension baseline for native-parser daemon and incremental
suites (run with `TEST_NATIVE_PARSER=1`):

- `mypy/test/testfinegrained.py`: `747 passed, 27 skipped`
- `mypy/test/testdaemon.py`: `37 passed`
- `mypy/test/testfinegrainedcache.py`: `549 passed, 229 skipped`

Verification run locally:

```bash
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  TEST_NATIVE_PARSER=1 uv run --group test python -m pytest \
  mypy/test/testfinegrained.py mypy/test/testdaemon.py \
  mypy/test/testfinegrainedcache.py -q
```

Current local-extension baseline for the mypy self-check (run with
`--config-file mypy_self_check.ini -p mypy -p mypyc`, 340 source files):

- Diagnostic parity: byte-for-byte identical output between the default
  Python parser and `--native-parser`. Both report the same 2 pre-existing
  errors in `mypy/parse.py` (unrelated to native parsing).
- Performance (cold, `--no-incremental`, 3 iterations each, best-of):

  | Parser  | real (s) |
  |---------|----------|
  | Python  | 7.40     |
  | Native  | 7.31     |

  No measurable regression. The self-check is type-checker-dominated, so
  parser time is a small fraction of the total and a parser-only speedup is
  not expected to move this number significantly. Parser-focused microbench
  marks (below) are the right place to measure native-parser throughput.

Parser-focused microbenchmark (release build of the Rust extension, mypyc
build of mypy). Calls `mypy.parse.parse(..., eager=True)` directly on the real
source corpus — 1856 files, 12,159 KiB (`mypy` + `mypyc` `.py` plus the bundled
typeshed `.pyi`). Bypasses type checking entirely so only parse + AST
materialization throughput is measured. Best of 3 iterations:

| Parser  | real (s) | throughput (KiB/s) |
|---------|----------|--------------------|
| Python  | 2.472    | 4918               |
| Native  | 1.761    | 6905               |

Native is **28.8% faster** on this corpus. (A debug build of the Rust
extension was 53% *slower* than the mypyc Python parser — only the release
build is a fair comparison, since the Python parser path runs through
mypyc-optimized code.)

Verification run locally:

```bash
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m mypy --config-file mypy_self_check.ini \
  --no-incremental --cache-dir /tmp/perf-py -p mypy -p mypyc
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python -m mypy --native-parser \
  --config-file mypy_self_check.ini --no-incremental \
  --cache-dir /tmp/perf-native -p mypy -p mypyc

# Parser-only microbenchmark against the real source corpus:
cargo rustc -p mypy-ast-serialize --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/release/libast_serialize.dylib \
  /private/tmp/mypy-rs-local-ast/ast_serialize.cpython-313-darwin.so
PYTHONPATH=/private/tmp/mypy-rs-local-ast \
  uv run --group test python scripts/bench_parser.py
```

Local file-level, daemon, cache, incremental, self-check, and performance
parity are all green. (The daemon/cache/incremental suites initially had
33 native-parser regressions when `native_parser` was defaulted on; these
were fixed — see `AGENTS.md` "Native-parser parity".) With CI not
available on this fork (see "CI Coverage" above), the local baselines
recorded here are the production-readiness gate for `mypy.nativeparse`
switching to prefer the in-tree extension.

## Milestone 2 (First Slice): Rust Module-Resolution Core

The repository now has an in-tree Rust module resolver alongside the parser
extension:

- `crates/module_resolver/Cargo.toml`
- `crates/module_resolver/src/lib.rs`
- `mypy/native_resolve.py` (Python adapter)

The crate builds a Python extension module named `module_resolver` and
preserves the existing `FindModuleCache.find_module` contract:

```python
resolve_module(id, ...) -> tuple[str | ModuleNotFoundReason, bool]
```

### Scope

**Ports to Rust** (the pure, fscache-only resolution core):
- `FindModuleCache._find_module` — the heart of the resolver.
- Helpers: `_find_module_non_stub_helper`, `_update_ns_ancestors`,
  `find_lib_path_dirs`, `get_toplevel_possibilities`, `verify_module`,
  `highest_init_level`.
- `stub_distribution_name` — replicated from the stubinfo tables passed in
  as plain data (flat set + namespace map).

**Stays in Python** (policy/diagnostics/side-effects):
- `FindModuleCache.find_module` — result caching, `use_typeshed` decision,
  WRONG_WORKING_DIRECTORY decoration, dispatch to Rust.
- `find_module_via_source_set` — the `fast_module_lookup` optimization.
- `find_modules_recursive` — touches `exclude` regex, gitignore, `sys.exit`.
- The entire `find_module_simple` / `find_module_with_reason` /
  `find_module_and_diagnose` layer in `build.py` — follow_imports policy,
  diagnostics, `ModuleNotFound`/`CompileError` raising.

### Filesystem strategy

Rust reads the real filesystem via `std::fs` directly, with no per-call
Python callbacks. The `NativeResolver` `#[pyclass]` is owned by
`FindModuleCache` for its lifetime and holds:

- FS caches (`listdir`, `isfile_case`, `exists_case`, `stat`) mirroring
  `FileSystemCache`'s cache fields, persisting across all `find_module`
  calls served by Rust.
- Resolution caches (`initial_components`, `ns_ancestors`) mirroring
  `FindModuleCache`'s cross-call memoization.
- Stable resolver config (search paths, stubinfo tables, resolver flags),
  set once at construction so only per-call varying args (`id`,
  `use_typeshed`, `follow_untyped_imports`) cross the PyO3 boundary on each
  resolve.

The dispatch gate in `FindModuleCache._resolve` routes only cold,
real-filesystem runs to Rust. Daemon (`fine_grained_incremental`) and Bazel
runs fall back to Python `_find_module` so the daemon VFS and Bazel
fake-init synthesis remain Python-owned until they are ported or retired:

```python
if (self.options.native_resolver
        and not self.options.fine_grained_incremental
        and not self.options.bazel):
    # Rust owns the FS for cold runs.
```

This is the direction the strangler-fig migration is heading: pure Rust,
no Python runtime. The callback strategy (Rust calling back into Python's
`FileSystemCache` for every `isfile`/`isdir`/`listdir`) was architecturally
backwards for that goal — it made Rust depend on Python's VFS forever. The
`StdFs` direct-read strategy makes Rust own the FS for cold runs, and the
gate is the honest way to say "Rust owns the FS for cold runs; daemon mode
uses Python until the VFS is ported or the daemon is retired."

Case-sensitive matching on macOS/Windows is replicated in Rust via
`read_dir` listing checks, mirroring `FileSystemCache.isfile_case` and
`exists_case`.

### Wiring

- `Options.native_resolver` (default `False`) gates the dispatch in
  `FindModuleCache._resolve`.
- `--native-resolver` CLI flag (invertible).
- `TEST_NATIVE_RESOLVER=1` env var flips it in the testcheck harness.
- Force-on under parallel mode (`main.py`), same as `native_parser`.
- `native_resolver` is in `OPTIONS_AFFECTING_CACHE`.

### Parity baselines

All suites run with both `TEST_NATIVE_PARSER=1` and
`TEST_NATIVE_RESOLVER=1` against the in-tree Rust extensions on
`PYTHONPATH`:

| Suite | Result |
|-------|--------|
| `testmodulefinder.py` (Python path) | 16 passed |
| `testmodulefinder.py` (`TEST_NATIVE_RESOLVER=1`) | 16 passed |
| `testcheck.py` | 8144 passed, 69 skipped, 7 xfailed |
| `testfinegrained.py` | 747 passed, 27 skipped |
| `testdaemon.py` | 37 passed |
| `testfinegrainedcache.py` | 549 passed, 229 skipped |

mypy self-check diagnostic parity (`mypy_self_check.ini -p mypy -p mypyc`,
341 source files): byte-for-byte identical output between the default
Python resolver, `--native-resolver`, and `--native-parser --native-resolver`.
All three report 0 errors. The two pre-existing type errors in
`mypy/parse.py` (a `list[Block]` vs `list[Statement]` mismatch in the
`ignore_whole_module` branch) are fixed by annotating the initial `defs`
binding; the three errors that PR #4 introduced (the untyped
`module_resolver` import) are fixed by an in-tree stub at `stubs/module_resolver.pyi`
found via `mypy_path` in `mypy_self_check.ini`.

### Performance

Resolver-focused microbenchmark (release build of the Rust extension, pure
Python mypy). Calls `FindModuleCache.find_module` directly on 95 unique
top-level imports extracted from the real source corpus. Best of 5
iterations:

| Resolver        | real (s) | per-module (µs) |
|-----------------|----------|------------------|
| Python          | 0.0016   | 16.5             |
| Native (per-call) | 0.0023 | 23.8             |
| Native (batched)  | 0.0003 | 2.7              |

The per-call native path is **~1.4x slower** than Python in isolation. The
entire gap is PyO3 entry/exit overhead paid once per module id, plus the
`options.clone_for_module` call on the Python side — the Rust kernel itself
is faster than mypyc-compiled Python, but the boundary tax per call dwarfs
the work done inside it.

Hoisting the whole import set into one `NativeResolver.resolve_many` call
(one boundary crossing for N ids, mirroring the existing
`compute_dep_records` shape) closes the gap decisively: **batched native is
~6x faster than Python** (2.7 µs vs 16.5 µs per module) and ~8.6x faster
than the per-call native path. This proves the resolver seam has perf legs
once the per-file import set is resolved in one Rust call rather than one
call per id.

The previous callback strategy (Rust calling back into Python's
`FileSystemCache` for every `isfile`/`isdir`/`listdir`) was ~8x slower
(175µs/module); the direct `std::fs` read with persistent caches closed
that gap.

End-to-end, the per-call overhead is invisible because resolution is a tiny
fraction of total mypy time. Self-check timing (`mypy/modulefinder.py
mypy/native_resolve.py mypy/nativeparse.py`, `--no-incremental`): byte-for-byte
identical output between the Python resolver and `--native-resolver`. The
batched `resolve_many` path is not yet wired into `FindModuleCache`; it is
exposed on the adapter (`mypy.native_resolve.resolve_modules`) and proven
on the microbench above. Production wiring (a batched `find_modules` on
`FindModuleCache` consuming the per-file import set in one Rust call) is the
next milestone for this seam.

### Verification

```bash
cargo test -p mypy-module-resolver
cargo rustc -p mypy-module-resolver --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/release/libmodule_resolver.dylib \
  /private/tmp/mypy-rs-local-resolver/module_resolver.cpython-313-darwin.so

# Parity (both extensions on PYTHONPATH):
PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run --group test python -m pytest mypy/test/testmodulefinder.py -q
PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run --group test python -m pytest mypy/test/testcheck.py -q
PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run --group test python -m pytest \
  mypy/test/testfinegrained.py mypy/test/testdaemon.py \
  mypy/test/testfinegrainedcache.py -q

# Self-check diagnostic parity:
PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  uv run --group test python -m mypy --config-file mypy_self_check.ini \
  --no-incremental --cache-dir /tmp/perf-py -p mypy -p mypyc
PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  uv run --group test python -m mypy --native-resolver \
  --config-file mypy_self_check.ini --no-incremental \
  --cache-dir /tmp/perf-native -p mypy -p mypyc

# Resolver-only microbenchmark against the real corpus:
PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  uv run --group test python scripts/bench_resolver.py
```

Local modulefinder, testcheck, fine-grained, daemon, cache, incremental,
and self-check parity are all green. (The fine-grained / daemon / cache
suites initially had 33 native-parser regressions — type-comment handling,
error-message parity, and PEP 263 encoding — which were fixed; see
`AGENTS.md` "Native-parser parity".) The native parser is now the
default (`native_parser = True`); the native resolver remains opt-in
(`native_resolver = False`) until the daemon VFS path is resolved or
dmypy is retired. The direct `std::fs` read strategy with
persistent caches brings the isolated microbench within 1.4x of pure
Python (and faster than mypyc-compiled Python would be once the boundary
overhead is eliminated by hoisting more work into Rust). End-to-end
performance is unaffected.

## Milestone 2 (Second Slice): Rust Dependency-Record Extraction

The second slice of Milestone 2 ports
`BuildManager.all_imported_modules_in_file` (`mypy/build.py:1202-1262`) to
Rust, behind the same `native_resolver` dispatch gate as the resolution
core. This walks a module's import list and emits
`(priority, module_id, line)` tuples — the records `compute_dependencies`
consumes to build the module dependency graph.

### Scope

**Ports to Rust** (the pure dependency-walk core):
- `import_priority` — `is_top_level`/`is_mypy_only` → priority constant.
- `correct_rel_imp` — relative-import resolution (pure string manipulation).
- The import walk itself: `Import` (with ancestor expansion),
  `ImportFrom` (with submodule-vs-name discrimination and the #4498
  cycle-workaround priority), `ImportAll`.
- `is_module` — the build-graph fast path (`known_modules` set) then
  filesystem resolution via the same `NativeResolver` that `_resolve`
  uses, with `use_typeshed` computed in Rust (see below).
- `use_typeshed_for` — mirrors `FindModuleCache.find_module`'s
  `use_typeshed` decision + `_typeshed_has_version`, so a stdlib module
  outside the target Python version range is NOT looked up in typeshed.

**Stays in Python** (side-effects, plugin integration):
- `plugin.get_additional_deps` — concatenated after the Rust call.
- `Errors.report(..., blocker=True)` for the "No parent module"
  relative-import error — Rust returns an `Option<(line, message)>` and
  Python reports it via `errors.set_file` + `errors.report`.
- The dispatch gate in `State.compute_dependencies` (`build.py`).

### Wiring

The dispatch is in `State.compute_dependencies` (`mypy/build.py`):

```python
if manager.find_module_cache._native_gate_active():
    # Rust walks the import list and resolves module ids via the same
    # NativeResolver that _resolve uses, returning (priority, module_id,
    # line) records. Plugin deps and the correct_rel_imp error reporting
    # stay in Python (concatenated / reported after the Rust call).
    dep_entries = _native.compute_dep_records(
        resolver, file=self.tree, known_modules=known,
        errors=manager.errors, options=manager.options,
    ) + manager.plugin.get_additional_deps(self.tree)
else:
    dep_entries = manager.all_imported_modules_in_file(
        self.tree) + manager.plugin.get_additional_deps(self.tree)
```

`_import_to_record` (`mypy/native_resolve.py`) flattens each
`ImportBase` AST node into the plain tuple shape Rust expects (PyO3's
`FromPyObject` for a tuple struct reads positionally). The import records
are already deserialized by `load_from_raw` with
`dependency_discovery=True`, so `is_unreachable` /
`is_unreachable_dependency` flags cross the boundary as plain bools.

### The `use_typeshed` computation

The first slice computed `use_typeshed` Python-side and passed it to Rust
as a bool on each `resolve` call. The dependency walk calls `is_module` on
many ids in a tight loop, so computing `use_typeshed` Python-side per id
would defeat the purpose of running the walk in Rust. Instead, the
`NativeResolver` now carries the stdlib version table
(`stdlib_versions: HashMap<String, ((u8, u8), Option<(u8, u8)>)>`) and
the clamped target Python version (`python_version: (u8, u8)`, clamped to
`(3, 10)` minimum mirroring `typeshed_py_version`). `use_typeshed_for`
in Rust replicates `FindModuleCache.find_module`'s decision exactly:

```rust
fn use_typeshed_for(id, python_version, stdlib_versions) -> bool {
    // Mirrors find_module's id-then-top_level lookup.
    let (min, max) = stdlib_versions[key];
    python_version >= min && max.map_or(true, |m| python_version <= m)
}
```

This is what makes `import tomllib` (added in Python 3.11) resolve as
`NOT_FOUND` when targeting 3.10, so the dependency walk skips it via
`include_only_if_resolvable` — matching the Python path's behavior
exactly. Without this, the self-check would report a phantom
`import-not-found` error for `tomllib` whenever `compute_dep_records`
ran (which it does under `num_workers > 0`, since that forces
`native_resolver = True`).

### Parity baselines

| Suite | Result |
|-------|--------|
| `testmodulefinder.py` (`TEST_NATIVE_RESOLVER=1`) | 16 passed |
| `testgraph.py` (`TEST_NATIVE_RESOLVER=1`) | 11 passed |
| `testcheck.py` (`TEST_NATIVE_RESOLVER=1`) | 8198 passed, 15 skipped, 7 xfailed |
| Rust unit tests | 32 passed (15 resolution + 15 dep-walk + 2 version-gating regression) |

The 2 version-gating regression tests pin the `tomllib` fix: a stdlib
module outside the target version range is skipped (empty records), while
the same module in range is included.


## Milestone 2 (Third Slice): Rust `FsCache` Backing `FileSystemCache`

The third slice ports `mypy.fscache.FileSystemCache` to Rust behind a
`FsCache` `#[pyclass]`. The Python class becomes a thin delegate that
forwards every method to the Rust implementation. This was the foundation
for dropping the daemon exclusion gate (`_native_gate_active` excluded
`fine_grained_incremental`): Phase 2 (below) repointed `NativeResolver`
at the shared `FsCache`, eliminating the dual-cache hazard so the daemon
now uses native resolution.

### Why this slice

`FileSystemCache` is not a virtual filesystem — it is a transactional
memoizing cache over the real filesystem (`os.stat` / `os.listdir` /
`open`), with one synthetic overlay (Bazel fake `__init__.py`). Its two
real jobs are (a) memoize syscalls within a transaction and (b) snapshot
consistency: repeated reads of the same path return the same result even
if the real filesystem changes underneath. `flush()` starts a new
transaction. Both are naturally Rust-implementable.

The dual-cache hazard was the concrete blocker for daemon parity:
`NativeResolver` owned four caches (`listdir_cache`, `isfile_case_cache`,
`exists_case_cache`, `stat_cache`) that mirrored `FileSystemCache`'s caches
one-for-one, and both could disagree within a transaction. The
`_native_gate_active` exclusion of `fine_grained_incremental`
(`mypy/modulefinder.py:403`) was the only thing preventing that hazard in
daemon mode. Phase 2 (below) resolved this by repointing `NativeResolver`
at the shared `FsCache`.

### Scope

**Ports to Rust** (new crate `crates/fs_cache/`):
- `FsCache` `#[pyclass]` with the full `FileSystemCache` surface:
  `stat_or_none`, `listdir`, `isfile`, `isfile_case`, `exists_case`,
  `isdir`, `exists`, `read`, `hash_digest`, `samefile`, `flush`,
  `set_package_root`, `init_under_package_root`.
- The Bazel fake-`__init__.py` synthesis (`init_under_package_root` +
  `fake_init`), which had no Python tests. The Rust port includes the
  first dedicated unit tests for this path.
- Snapshot consistency: all caches are per-transaction; `flush()` clears
  them. `package_root` survives across flushes (matches the Python
  contract).
- The mtime-vs-contents ordering invariant: `read` stats before opening
  the file so the cached mtime is from an instant no earlier than the
  contents.

**Stays in Python**:
- `FileSystemCache` class: thin delegate. Each method forwards to
  `self._rust` (the `FsCache` pyclass). Falls back to the pure-Python
  implementation when `fs_cache` is not importable (e.g. daemon
  subprocesses that override `PYTHONPATH` to the repo root), preserving
  the strangler-fig contract.
- `FakeFSCache` test subclass (`mypy/test/test_find_sources.py`):
  unchanged. MRO gives its overrides precedence; `self._rust` is never
  constructed on those instances.
- `copy_os_error` helper: stays for the Python fallback path.

**New shared crate `crates/fs_probe/`**: the `FsProbe` trait lives in a
tiny no-pyo3 crate so `fs_cache` and `module_resolver` can both depend
on it without one pyo3 cdylib needing to link the other. The trait is the
single seam through which `NativeResolver` reads the filesystem; Phase 2
swapped production's `fs` parameter from `&NativeResolver` (which `impl
FsProbe`) to `&FsCache`.

### Parity baselines

| Suite | Result |
|-------|--------|
| `testfscache.py` | 5 passed |
| `test_find_sources.py` (incl. `FakeFSCache` subclass) | 8 passed |
| `testmodulefinder.py` + `testgraph.py` (`TEST_NATIVE_RESOLVER=1`) | 27 passed |
| `testcheck.py` (`TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1`) | 8144 passed, 69 skipped, 7 xfailed |
| `testfinegrained.py` + `testdaemon.py` + `testfinegrainedcache.py` | 1333 passed, 256 skipped |
| Rust unit tests (`fs_cache`) | 11 passed |
| Rust unit tests (`module_resolver`) | 35 passed |
| Self-check (`mypy_self_check.ini -p mypy -p mypyc`, 341 files) | 0 errors |

The fine-grained / daemon / cache suites initially had 33 native-parser
regressions (type-comment handling, error-message parity, PEP 263
encoding) introduced when `native_parser` was defaulted on (commit
`5041901ca`, Phase 1). These were fixed in a subsequent parity fix
commit — see `AGENTS.md` ("Native-parser parity") for details.

### Verification

> **Note**: Phase 2 merged `crates/fs_cache/` into `crates/module_resolver/`,
> so the `mypy-fs-cache` package no longer exists. The commands below are
> kept for historical reference; use the Phase 2 Verification commands
> (which build only `mypy-module-resolver` and `mypy-ast-serialize`) for
> current work.

```bash
cargo test -p mypy-fs-cache -p mypy-module-resolver -p mypy-fs-probe   # historical
cargo rustc -p mypy-fs-cache --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cargo rustc -p mypy-module-resolver --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/release/libfs_cache.dylib \
  /private/tmp/mypy-rs-local-fscache/fs_cache.cpython-313-darwin.so
cp target/release/libmodule_resolver.dylib \
  /private/tmp/mypy-rs-local-resolver/module_resolver.cpython-313-darwin.so

PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver:/private/tmp/mypy-rs-local-fscache \
  uv run --group test python -m pytest mypy/test/testfscache.py mypy/test/test_find_sources.py -q
PYTHONPATH=... \
  TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run --group test python -m pytest mypy/test/testcheck.py -q
PYTHONPATH=... \
  TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run --group test python -m pytest \
  mypy/test/testfinegrained.py mypy/test/testdaemon.py \
  mypy/test/testfinegrainedcache.py -q

# Self-check diagnostic parity:
PYTHONPATH=... \
  uv run --group test python -m mypy --config-file mypy_self_check.ini \
  --no-incremental --cache-dir /tmp/perf-fscache -p mypy -p mypyc
```


## Milestone 2 (Phase 2): `NativeResolver` Reads Through the Shared `FsCache`

Phase 2 retires the dual-cache hazard by repointing `NativeResolver` at
the shared `FsCache`. The resolver no longer owns private FS caches; it
borrows the `FsCache` pyclass owned by the Python `FileSystemCache`
delegate, so there is exactly one FS cache per transaction. With the
hazard gone, the `_native_gate_active` exclusion of
`fine_grained_incremental` is dropped and daemon mode uses the native
resolution path, landing the 6x batched-resolution win in daemon mode.

### Why this slice

The strangler-fig contract requires that adding Rust behind a narrow
interface never changes Python-facing behavior. Phase 1 left the daemon
exclusion in place because `NativeResolver`'s private caches could
disagree with `FileSystemCache`'s caches within a transaction — a
correctness hazard, not a performance one. Phase 2 removes the hazard at
its root (one cache, not two) rather than patching around it (exclusion
gate), which is the only sound way to extend native resolution to daemon
mode.

### Scope

**Rust** (`crates/module_resolver/`):
- `FsCache` pyclass moved into `module_resolver` (merged from the deleted
  `crates/fs_cache` crate). Two pyo3 cdylibs cannot depend on each other
  (feature unification propagates `extension-module`, breaking the link
  step), so both pyclasses live in one cdylib.
- `NativeResolver` holds `Py<FsCache>` instead of four private cache
  fields. `resolve` / `resolve_many` / `compute_dep_records` borrow it
  per call (`self.fs_cache.borrow(py)` → `&*fs`) and pass `&FsCache` as
  the `fs: F: FsProbe` parameter.
- Removed: `impl FsProbe for NativeResolver`, `stat_cached`,
  `listdir_cached`, `listdir_uncached`, `case_check`,
  `case_check_uncached`, `split_head_tail` (all now dead — the shared
  `FsCache` owns these).
- New `NativeResolver::flush()` pymethod clears the resolver's derived
  resolution caches (`initial_components`, `ns_ancestors`) that must not
  outlive an FS transaction.

**`crates/fs_probe/`**: added a blanket `impl<T: FsProbe + ?Sized>
FsProbe for &T` so `&FsCache` (obtained via `&*PyRef<FsCache>`) satisfies
the `F: FsProbe` bound without `NativeResolver` needing its own impl.

**Python**:
- `mypy/modulefinder.py`: `_native_gate_active` no longer excludes
  `fine_grained_incremental` (only `bazel` remains, for the fake-init
  synthesis path). `FindModuleCache.clear()` now calls
  `self._native_resolver.flush()` so the derived caches don't outlive an
  FS transaction. `_ensure_native_resolver` passes `fscache=self.fscache`
  to `make_resolver`.
- `mypy/native_resolve.py`: `make_resolver` takes a `fscache:
  FileSystemCache` param and passes `fscache._rust` (the `FsCache`
  pyclass) as `NativeResolver`'s first constructor arg.
- `mypy/dmypy_server.py`: when `_HAS_RUST_CACHE and not options.bazel`,
  sets `options.native_resolver = True` before the initial options
  snapshot (in `__init__`) and re-applies it after `process_options` in
  `check()` (before `compare_stable`), so the daemon uses native
  resolution without triggering a "configuration changed" restart.
- `mypy/fscache.py`: imports `FsCache` from `module_resolver` (not the
  deleted `fs_cache` extension).
- `mypy/options.py`: removed `native_resolver` from
  `OPTIONS_AFFECTING_CACHE`. It is a pure performance optimization
  (verified by testcheck parity) that does not change cache semantics;
  leaving it in caused fine-grained cache invalidation because the
  initial `build.build()` run (cmd1) used the default `native_resolver=
  False` while the subsequent `Server.check()` increments used `True`.

**Stays in Python**: `FakeFSCache` test subclass, `copy_os_error` helper,
the `FileSystemCache` delegate surface.

### Parity baselines

| Suite | Result |
|-------|--------|
| `testfscache.py` + `test_find_sources.py` | 13 passed |
| `testmodulefinder.py` + `testgraph.py` (`TEST_NATIVE_RESOLVER=1`) | 27 passed |
| `testcheck.py` (`TEST_NATIVE_RESOLVER=1`) | 8198 passed, 15 skipped, 7 xfailed |
| `testfinegrained.py` + `testdaemon.py` + `testfinegrainedcache.py` | 1333 passed, 256 skipped |
| Rust unit tests (`module_resolver`, incl. `fs_cache` submodule) | 46 passed |
| Self-check (`mypy_self_check.ini -p mypy`, 197 files) | 0 errors |

The fine-grained / daemon / cache suites initially had 33 native-parser
regressions from Phase 1 (commit `5041901ca`, defaulting `native_parser`
on). These were fixed in a subsequent parity fix commit — see
`AGENTS.md` ("Native-parser parity") for details. Phase 2 itself adds
zero new failures: diffing `TEST_NATIVE_PARSER=1` with and without
`TEST_NATIVE_RESOLVER=1` yields an identical result set.

### Verification

```bash
cargo test --release -p mypy-module-resolver -p mypy-fs-probe

# Build the extensions to a scratch dir (NOT maturin develop — see
# AGENTS.md "Native parser build order" for why).
cargo rustc -p mypy-ast-serialize --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cargo rustc -p mypy-module-resolver --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/release/libast_serialize.dylib \
  /private/tmp/mypy-rs-local-ast/ast_serialize.cpython-313-darwin.so
cp target/release/libmodule_resolver.dylib \
  /private/tmp/mypy-rs-local-resolver/module_resolver.cpython-313-darwin.so

export PYEXT=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver

PYTHONPATH=$PYEXT TEST_NATIVE_RESOLVER=1 uv run python -m pytest -n0 \
  mypy/test/testmodulefinder.py mypy/test/testgraph.py
PYTHONPATH=$PYEXT TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run python -m pytest -n0 mypy/test/testcheck.py
PYTHONPATH=$PYEXT uv run python -m pytest -n0 \
  mypy/test/testfscache.py mypy/test/test_find_sources.py
PYTHONPATH=$PYEXT TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run python -m pytest -n0 \
  mypy/test/testfinegrained.py mypy/test/testdaemon.py \
  mypy/test/testfinegrainedcache.py
PYTHONPATH=$PYEXT uv run python -m mypy --config-file mypy_self_check.ini -p mypy
```

## Milestone 2 (Phase 3): Default-on Native Resolver

`Options.native_resolver` now defaults to `True`. The daemon
(`dmypy_server.py`) and parallel mode (`main.py`) already forced it on
when `_HAS_RUST_CACHE`; this closes the gap for normal cold-run `mypy`
invocations, which previously fell back to `FindModuleCache._find_module`.
Bazel remains on the Python resolver by the `_native_gate_active` dispatch
gate.

### Parity baselines

| Suite | Result |
|-------|--------|
| `testmodulefinder.py` + `testgraph.py` + `testfscache.py` + `test_find_sources.py` | 40 passed |
| `testcheck.py` (`TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1`) | 8144 passed, 69 skipped, 7 xfailed |
| `testfinegrained.py` + `testdaemon.py` + `testfinegrainedcache.py` | 1333 passed, 256 skipped |
| Self-check (`mypy_self_check.ini -p mypy`, 197 files) | 0 errors |

The default flip does not change test behavior: `testcheck.py` and
`testmodulefinder.py` override `options.native_resolver` from
`TEST_NATIVE_RESOLVER` *after* option parsing, so unset exercises the
default-on path and `=0` forces the Python fallback — preserving the
two-way parity differential the strangler-fig contract requires.

### Next milestone (Phase 4)

**Status: not pursued.** The prepass rationale — "improve cold start and
graph-load performance" — does not survive measurement. With the native
resolver + shared `FsCache` default-on (Phase 3), `--dump-build-stats`
on the self-check corpus (197 modules, parallel) shows:

| stat | value |
|------|-------|
| `find_module_calls` | 1004 |
| `find_module_time` | 0.010s |
| `total_process_stale_time` | 4.109s |
| `type_check_time_implementation` | 2.938s (71%) |
| `semanal_time` | 0.650s (16%) |

`find_module` is **0.24% of build time**. A prepass would optimize away
~10ms by replacing per-module lookups with a single traversal, while
duplicating logic `module_resolver` already implements per-call. This
violates the strangler-fig principle of solving real problems behind
narrow interfaces — it would be a shallow pass-through with no measurable
gain.

The real bottleneck is the **type checker and semantic analyzer**
(87% of build time combined), which is the "pure type-operations kernel"
the migration doc lists as item #4 but flags as high-risk because
`mypy.types`/`mypy.nodes` are widely-shared mutable object graphs and
plugin-visible. That warning stands; the kernel is high-value but needs
careful scoping.

**Revised next candidate: the type kernel (item #4).** The cache
indexing/validation slice (item #3) was also measured and dropped —
`load_meta_time` is 8ms and `validate_meta_time` is 5ms on a ~4s warm
build (0.17% combined), and validation is dominated by `stat()` syscalls
Rust cannot speed up. See "Stage 1" below for the type-kernel staging
plan, which targets the real bottleneck (type checker + semantic analyzer,
87% of build time combined).

## Performance baseline

Recorded 2026-07-06 on the self-check corpus (`mypy_self_check.ini -p mypy`,
197 modules, 4 parallel workers), native parser + native resolver
default-on, extensions built `--release`:

| stat | value |
|------|-------|
| `find_module_calls` | 1004 |
| `find_module_time` | 0.010s |
| `fm_cache_size` | 394 |
| `parse_time` | 0.149s |
| `semanal_time` | 0.650s |
| `type_check_time_implementation` | 2.938s |
| `type_check_time_interface` | 0.225s |
| `scc_wait_time` | 1.080s |
| `total_process_stale_time` | 4.109s |
| `deserialize_time` | 0.032s |
| `flush_and_cache_time` | 0.054s |

This is the regression baseline for the default-on resolver + parser.
Re-run with `--dump-build-stats` after changes to the parser or resolver
seams and compare; a >10% regression in any row warrants investigation
before merging.

## Milestone 3 (Phase 4): Type Kernel — Stage 1 (`erase_type`)

The type kernel is the highest-risk slice in the migration: `mypy.types`
is a widely-shared mutable object graph, plugin-visible, with 30+ `Type`
subclasses, a `TypeVisitor` dispatch, two serialization formats (dict +
binary), and mutation seams (`type_ref` fixup, `instance_cache`
flyweights, lazy bool caches). AGENTS.md says "do not start by porting
`mypy.nodes` or `mypy.types`" — Stage 1 consciously relaxes that for
the kernel, but keeps every stage behind a parity gate with Python
fallback, so no behavior changes ship unproven.

### The seam challenge

Every kernel operation needs to resolve `TypeInfo` objects (MRO, variances,
protocol members) referenced by name via `type_ref` — these are *not* in
the binary wire format. Two possible seams:

- **(A) PyO3 on live Python `Type` objects** — Rust walks Python objects
  via the C API. Simple, no new wire format, but per-call FFI overhead
  makes it slower than Python for cheap operations, and it touches the
  mutable graph directly.
- **(B) Rust-owned `Type` representation built from the binary wire
  format** — Rust holds its own `Type` enum, built from `Type.write(bytes)`
  and a `TypeInfo` snapshot side-table. Faster at steady state, but
  requires a full Rust `Type` hierarchy + snapshot protocol before any
  operation can run.

**Stage 1 uses (A)** because it proves the seam end-to-end with the
smallest surface area and lets Rust fall back to Python per-call for any
case it doesn't handle yet (true strangler-fig). Stage 3+ (`is_subtype`,
the perf win) will move to (B) once the operation shape is validated.

### Why `erase_type` is the right first operation

- **Pure visitor**: `Type → Type`, no plugin hooks, no mutation of input,
  no `TypeInfo` mutation. The only `TypeInfo` dependency is
  `t.type.defn.type_vars` (count + kinds, for `Instance` erasure) — a
  narrow, stable record read directly from the live object.
- **Well-tested**: 8 dedicated tests in `mypy/test/testtypes.py`
  (`test_trivial_erase`, `test_erase_with_type_variable`,
  `test_erase_with_generic_type`, `_recursive`, `_tuple_type`,
  `_function_type`, `_type_object`, `_type_type`). Parity contract is
  `str(erase_type(orig)) == str(result)` — string equality of
  pretty-printed types, robust and already used by the tests.
- **Called constantly** from the checker and subtypes, so the seam is
  exercised under real load.
- **Cheap enough to not regress visibly** if the PyO3 path is slower
  per-call, but real enough to prove the full dispatch gate.

### Implementation

New crate `crates/type_kernel` exposes one PyO3 function:

```rust
#[pyfunction]
fn erase_type(typ: &PyAny) -> PyResult<PyObject>
```

It walks `typ` as a Python `Type` object via `isinstance` checks against
resolved class objects (so plugin subclasses are handled correctly),
mirroring `EraseTypeVisitor`. For `Instance`, it reads
`t.type.defn.type_vars` directly from the live `TypeInfo` (same as the
Python visitor — no snapshot cache needed in Stage 1). For
`CallableType`, `UnionType`, `TypeType`, `Overloaded`, `TupleType`,
`TypedDictType`, it recurses or constructs new Python objects via the
same Python constructors. For any class it doesn't recognize, it
returns `None` — the Python caller falls back to the pure-Python
visitor. This is the strangler-fig per-call gate.

The Python side gates `erase_type()` in `mypy/erasetype.py` on a
module-level flag set by the build manager from
`Options.native_type_kernel` (default `False` — Stage 1 is opt-in).
Test harnesses flip it from `TEST_NATIVE_TYPE_KERNEL`, mirroring the
`TEST_NATIVE_PARSER`/`TEST_NATIVE_RESOLVER` pattern.

### Parity baselines

| Suite | Result |
|-------|--------|
| `testtypes.py -k test_erase` (`TEST_NATIVE_TYPE_KERNEL=1`) | 7 passed |
| `testtypes.py` full (`TEST_NATIVE_TYPE_KERNEL=1`) | 119 passed, 2 skipped |
| `testcheck.py` (`TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 TEST_NATIVE_TYPE_KERNEL=1`) | 8144 passed, 69 skipped, 7 xfailed |
| Self-check (`mypy_self_check.ini -p mypy`, 197 files) | 0 errors |

The default-off path is unchanged: `testtypes.py -k test_erase` without
the env var passes identically. The gate is opt-in until parity is
proven across the full suite, at which point a future stage can flip the
default.

### Staging roadmap

- **Stage 1 (this milestone)**: `erase_type` via PyO3, gated,
  parity-tested. Proves the seam.
- **Stage 2**: More pure visitors (`LastKnownValueEraser`,
  `TypeStrVisitor`) on the same seam. Cheap wins, broadens Rust coverage
  of the visitor dispatch.
- **Stage 3**: Rust-owned `Type` enum + bytes seam. Port
  `is_subtype`/`is_proper_subtype` (the perf win — 26 unit tests +
  thousands of data-driven cases). `TypeInfo` snapshot protocol
  formalized.
- **Stage 4**: `check_call` / `ExpressionChecker.visit_call_expr_inner`
  — the big one, highest value, needs the plugin-hook snapshot
  protocol.
- **Stage 5**: Semantic analyzer kernel (`semanal_time`, 16% of build).

