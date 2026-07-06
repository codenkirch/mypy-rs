# Agent Instructions

This repository is a working branch for migrating mypy toward Rust while keeping
the existing Python behavior stable.

## Commit Style

Use Conventional Commits for all commits:

- `feat: ...` for new user-visible functionality
- `fix: ...` for bug fixes
- `docs: ...` for documentation-only changes
- `test: ...` for tests
- `refactor: ...` for behavior-preserving internal changes
- `perf: ...` for performance changes
- `build: ...` for packaging, dependency, and build-system changes
- `ci: ...` for CI workflow changes
- `chore: ...` for maintenance that does not fit the above

Prefer a single clear subject line under 72 characters. Add a body when the
change has non-obvious motivation, migration notes, or test coverage details.

## Development Workflow

Use `uv` for local development. Do not add tox-based workflows.

Common commands:

```bash
uv sync
uv run all
uv run test
uv run pytest -n0 -k test_name
uv run lint
uv run format
uv run typecheck
uv run docs
```

`uv run test` delegates to `runtests.py` so the existing grouped test behavior
is preserved. Use `uv run pytest ...` when you need direct pytest arguments.

## Rust Migration Direction

The migration plan is recorded in `docs/rust-migration-strangler.md`.

Follow a strangler-fig approach:

- Keep Python-facing behavior stable while adding Rust behind narrow interfaces.
- Prefer Rust adapters that exchange plain records, bytes, or stable IDs with
  Python.
- Do not start by porting `mypy.nodes` or `mypy.types`; they are widely shared
  mutable object graphs and plugin-visible.
- Treat the native parser path as the first production migration seam.
- The native module resolver (`FindModuleCache._find_module`) and the
  dependency-records extraction (`BuildManager.all_imported_modules_in_file`)
  are the second and third seams; both are ported behind the
  `native_resolver` dispatch gate and now default-on. The import-graph
  prepass and cache indexing/validation were both measured and dropped
  (see "Phase 4 measurement" in `docs/rust-migration-strangler.md`); the
  type kernel is the active migration target, starting with `erase_type`
  behind the `native_type_kernel` gate (Stage 1, opt-in). Stage 2 ports
  `remove_instance_last_known_values` (`LastKnownValueEraser`) on the same
  PyO3 seam, also opt-in. Stage 3a adds a Rust `Type` enum + binary
  wire-format reader (`wire::read_type`), parity-tested but not yet wired
  into production — foundation for Stage 3c (`is_subtype`).
- Preserve daemon, cache, plugin, and incremental-mode semantics unless a change
  is explicitly called out and tested.

## Search Tools

Use `rg` (ripgrep) and `fd` instead of `grep` and `find` for any
codebase search. They are faster, respect `.gitignore` by default, and
produce cleaner output. Reach for them when locating symbols, files,
or patterns rather than the POSIX equivalents. Examples:

```bash
rg "native_resolver" mypy/
fd -e py -p "testfinegrained"
```

Only fall back to `grep`/`find` when a pipeline or environment strictly
requires POSIX semantics.

## Design Principles

Use the following design principles when changing the codebase:

- Prefer deep modules: small, stable interfaces hiding meaningful complexity.
- Avoid shallow pass-through modules that merely split code without reducing the
  caller's burden.
- Optimize for locality: keep related decisions, invariants, and error handling
  close to the code that owns them.
- Design interfaces around what callers need to know, including invariants,
  ordering constraints, error modes, and performance expectations.
- Make complexity explicit where it is essential, and hide accidental complexity
  behind well-named modules.
- Do not leak implementation details across seams. If callers must understand
  the implementation to use the module correctly, improve the interface.
- Prefer consistency and boring structure over cleverness.
- Add comments for non-obvious reasoning and invariants, not for restating what
  the code already says.
- When changing shared behavior, test through the public interface rather than
  testing internal incidental structure.

## Verification Expectations

For workflow or infrastructure changes, run the smallest relevant uv commands
first, then the broader suite when practical:

```bash
uv lock --check
uv run all
```

For Rust migration work, add targeted parity tests and include native-parser,
daemon, cache, and incremental-mode checks when affected.

### Native resolver / dependency-records parity

The native resolver and dependency-records extraction are gated behind
`Options.native_resolver`, which now defaults to `True` (Phase 3). The
daemon (`dmypy_server`) and parallel mode (`main.py`) force it on
regardless of the default; the only path that previously fell back to
the Python `FindModuleCache._find_module` was a normal cold-run `mypy`
invocation, which now also uses the native resolver. Bazel remains on
the Python resolver by the dispatch gate in `_native_gate_active`.

Build the extension before running parity — use the `cargo rustc` + scratch-dir
approach documented under "Native parser build order" below, not
`maturin develop`:

```bash
cargo rustc -p mypy-module-resolver --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/release/libmodule_resolver.dylib \
  /private/tmp/mypy-rs-local-resolver/module_resolver.cpython-313-darwin.so

# Parity suites — both run against the in-tree Rust extension.
PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  TEST_NATIVE_RESOLVER=1 uv run python -m pytest -n0 \
  mypy/test/testmodulefinder.py mypy/test/testgraph.py
PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run python -m pytest -n0 mypy/test/testcheck.py
```

`TEST_NATIVE_RESOLVER=1` is now redundant for production parity (the
default is on), but still serves as a parity differential in the test
harness: `testcheck.py` and `testmodulefinder.py` set
`options.native_resolver = bool(os.environ.get("TEST_NATIVE_RESOLVER"))`
*after* option parsing, overriding the default. Unset, they exercise the
default-on path; `=0` forces the Python fallback. `=1` is kept so the
differential stays explicit. The daemon (`fine_grained_incremental`)
path uses the native resolver (it reads through the shared `FsCache`);
only Bazel stays on the Python resolver by the dispatch gate, so the
Bazel path needs no special env var.

### Native parser build order

The native parser (`Options.native_parser`, defaulted on and force-on under
parallel mode) is backed by the `ast_serialize` Rust extension. The
serialized AST format is fixed by `crates/ast_serialize/src/lib.rs` and read
by `mypy/nativeparse.py`; the two must stay in lockstep.

**Rebuild the extensions after any change to `crates/ast_serialize/src/lib.rs`
or `crates/module_resolver/src/`.** A stale binary produces silent
deserialization mismatches — e.g. an `AssertionError: 255` (END_TAG read
where a LOCATION tag was expected) that crashes parallel workers during
self-check. The on-disk source can look correct while the installed binary
is stale, so always rebuild.

Do **not** use `maturin develop` for these crates: `crates/ast_serialize`
has no `pyproject.toml`, so maturin picks up the repo-root `pyproject.toml`
(mypy's) and installs a bogus `mypy-0.1.0` package that shadows the real
mypy. Build the `.so`s to a scratch dir via `cargo rustc` and put them on
`PYTHONPATH` instead — this is the verified approach the migration doc uses:

```bash
cargo rustc -p mypy-ast-serialize --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cargo rustc -p mypy-module-resolver --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/release/libast_serialize.dylib \
  /private/tmp/mypy-rs-local-ast/ast_serialize.cpython-313-darwin.so
cp target/release/libmodule_resolver.dylib \
  /private/tmp/mypy-rs-local-resolver/module_resolver.cpython-313-darwin.so
```

Run parity with those dirs prepended to `PYTHONPATH`:

```bash
PYTHONPATH=/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
  TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run --group test python -m pytest mypy/test/testcheck.py -q
```

A second hazard: `pyproject.toml` declares the PyPI `ast-serialize>=0.6.0`
stub package (type stubs only — no `parse` implementation). When the Rust
`.so` is not on `PYTHONPATH`, `import ast_serialize` resolves to this stub
and crashes with `AttributeError: module 'ast_serialize' has no attribute
'parse'`. The daemon test harness historically overwrote `PYTHONPATH`
(see `testdaemon.py:run_cmd`), which dropped the Rust dirs and triggered
this; that harness now prepends instead of overwriting.

`mypy_self_check.ini` runs with `num_workers = 4`, which forces both
`native_parser` and `native_resolver` on, so the self-check exercises both
extensions end-to-end and is the cheapest correctness gate after a rebuild.

### Native-parser parity

`Options.native_parser` defaults to `True` (Phase 1). The native parser
(ruff-based) matches the Python parser (CPython-based) on all parity suites:
testcheck (8144 passed), fine-grained / daemon / cache (1333 passed), and
self-check (0 errors). Three parity fixes were applied:

1. **Type-comment handling on `for` and `with` statements**: the Rust
   serializer now extracts `# type:` comments on `for`-loop and `with`
   statements and writes them into the binary AST (cache_version bumped
   to 4). The Python deserializer reads them back as `index_type` /
   `target_type`.
2. **Syntax-error message + location parity**: when ruff reports a syntax
   error, the Rust extension re-parses with CPython's `ast.parse` to get
   CPython's exact `SyntaxError.msg`, `lineno`, and `offset`. This
   guarantees byte-identical error output. Syntax errors are rare in
   production, so the double-parse cost is negligible.
3. **PEP 263 encoding handling**: when the native parser reads a file
   directly (source is `None`), it now decodes via Python's
   `decode_python_encoding` so `# coding:` declarations are respected
   and decode errors surface as `CompileError("Cannot decode file: ...")`
   — matching the Python path in `build.py:get_source()`.

### Type kernel build order

The type kernel (`Options.native_type_kernel`, default off — opt-in) is
backed by the `type_kernel` Rust extension. It implements two PyO3
functions that walk live Python `Type` objects:

- `erase_type` (Stage 1) — mirrors `mypy.erasetype.EraseTypeVisitor`.
- `remove_instance_last_known_values` (Stage 2) — mirrors
  `mypy.erasetype.LastKnownValueEraser` (a `TypeTranslator`).
- `read_type_to_str` (Stage 3a) — parity-only: reads a serialized
  `mypy.types.Type` from its binary wire format and returns
  `str(t)`. Not wired into any production path; used by
  `NativeTypeWireSuite` to prove the Rust `Type` enum + reader
  reconstructs the same type. Foundation for Stage 3c (`is_subtype`).

Stages 1/2 return `None` for any type class Rust does not handle, and
the Python caller falls back to the pure-Python visitor. This is the
strangler-fig per-call gate. See "Milestone 3/4/5 (Phase 4)" in
`docs/rust-migration-strangler.md` for the staging roadmap.

**Rebuild the extension after any change to
`crates/type_kernel/src/lib.rs`.** The same stale-binary hazard as the
native parser applies: the on-disk source can look correct while the
installed `.so` is stale. Build via `cargo rustc` to a scratch dir (not
`maturin develop`, for the same reason as the other crates):

```bash
cargo rustc -p mypy-type-kernel --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/release/libtype_kernel.dylib \
  /private/tmp/mypy-rs-local-typekernel/type_kernel.cpython-313-darwin.so
```

Run parity with all three extension dirs prepended to `PYTHONPATH`:

```bash
PYTHONPATH=/private/tmp/mypy-rs-local-typekernel:/private/tmp/mypy-rs-local-resolver:/private/tmp/mypy-rs-local-ast \
  TEST_NATIVE_TYPE_KERNEL=1 TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 \
  uv run python -m pytest mypy/test/testtypes.py mypy/test/testcheck.py -q
```

The type-kernel gate is opt-in: without `TEST_NATIVE_TYPE_KERNEL=1`,
both `erase_type` and `remove_instance_last_known_values` use the
pure-Python visitors unchanged. The build manager propagates
`Options.native_type_kernel` to a module-level flag in
`mypy/erasetype.py` at the start of each build (`_set_native_erase_active`),
so the hot path avoids an options lookup per call.

## Pull Requests

The default branch on this fork is `main` (not `master`). Always target
`main` as the PR base. Branch from `main` before committing — do not commit
directly to `main`.
