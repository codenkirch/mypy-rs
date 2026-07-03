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
- Make native parser default in a limited mode first.
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

## CI Coverage Observed

The GitHub Actions test matrix runs:

- multiple Python versions
- Windows, Linux, and macOS jobs
- interpreted and mypyc-compiled mypy jobs
- parallel checking jobs with `--mypy-num-workers`
- mypyc runtime tests
- type-checking jobs
- lint jobs
- a separate `mypy_primer` workflow over real projects

This is strong coverage for ordinary mypy development. For a Rust migration, it
needs to be supplemented with adapter parity tests, native-parser mode tests,
cache-format compatibility tests, daemon identity tests, and performance
regression tracking.

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
- The fallback path remains available until CI coverage proves stability.

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
  metadata for supported argument expressions.
- Serializes import side-channel metadata in the format expected by
  `mypy.nativeparse.deserialize_imports`, including top-level flags,
  function-local import flags, mypy-only flags for `TYPE_CHECKING`/`MYPY`
  blocks, and basic reachability for `PY2`/`PY3`, boolean operators, and
  `sys.version_info` comparisons.
- Serializes type-ignore side-channel metadata for `# type: ignore` comments,
  including bracketed error-code lists.
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
`82 failed, 8062 passed, 69 skipped, 7 xfailed`. This is no longer just
serializer fixture parity; remaining failures are concentrated in semantic
integration details such as whole-module ignores, inline config comments,
unreachable/import handling, selected type-comment diagnostics and locations,
variadic tuple edge cases, and incremental/redefinition behavior.

This implementation is not ready to become the default parser solely from this
file-level parity result. The next correctness step is broader integration
validation around daemon, cache, incremental-mode, import discovery, and
realistic mypy self-check paths before `mypy.nativeparse` is switched to prefer
the in-tree extension.
