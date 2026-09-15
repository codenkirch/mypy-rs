# Native extension build and parity reference

Narrative form of the AGENTS.md build and parity sections, archived
2026-09-15. AGENTS.md keeps the rules and commands; this file keeps the
rationale, history, and hazard background behind them. Load it when a
build or parity failure needs the long explanation.

## Rust Migration Direction

The migration plan is recorded in `docs/rust-migration-strangler.md`.

Follow a strangler-fig approach:

- Keep Python-facing behavior stable while adding Rust behind narrow interfaces.
- Prefer Rust adapters that exchange plain records, bytes, or stable IDs with
  Python.
- Goal revised 2026-09-01 (#1347): the endgame is a full Rust port. Rust
  becomes the owner of the `mypy.nodes` and `mypy.types` graphs themselves
  (Phases F-H in `docs/remaining-migration-plan.md`), with Python remaining
  as host, config/CLI surface, and plugin compatibility bridge. Until the
  Phase F/G flips graduate, those graphs stay Python-canonical and the
  per-call defer gates remain the mechanism: do not port them ad hoc.
- Treat the native parser path as the first production migration seam.
- The native module resolver (`FindModuleCache._find_module`) and the
  dependency-records extraction (`BuildManager.all_imported_modules_in_file`)
  are the second and third seams; both are ported behind the
  `native_resolver` dispatch gate and now default-on. The import-graph
  prepass and cache indexing/validation were both measured and dropped
  (see "Phase 4 measurement" in `docs/rust-migration-strangler.md`). The
  type kernel graduated to default-on (`Options.native_type_kernel = True`,
  #58) and is the active migration target, starting with `erase_type`
  (Stage 1). Stage 2 ports `remove_instance_last_known_values`
  (`LastKnownValueEraser`) on the same PyO3 seam. Stage 3a adds a Rust
  `Type` enum + binary wire-format reader (`wire::read_type_to_str`),
  parity-tested but not wired into production; foundation for a possible
  `is_subtype` port (see `docs/remaining-migration-plan.md` Phase E1).
- Preserve daemon, cache, plugin, and incremental-mode semantics unless a change
  is explicitly called out and tested.


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


### librt build order (F3 wire-cache splice)

The F3 wire cache (`_write_type_cached`, `mypy/types.py`) splices cached
wire bytes through `librt.internal.write_raw_bytes`. The PyPI `librt`
wheel (>=0.12.0) does **not** export that symbol even though the in-repo
`mypyc/lib-rt/internal/librt_internal.c` implements it, so the import in
`mypy/types.py:29-34` degrades to `None`, `_write_type_cached` falls back
to plain `t.write`, and the `NativeMirrorSpliceSuite` splice tests
self-skip. Build the in-repo lib-rt into a private scratch prefix and
prepend it to `PYTHONPATH` to exercise the splice. Never install or
upgrade `librt` in the shared `.venv`: every worktree symlinks it, and a
mutation breaks parallel agents mid-run.

```bash
# from the repo root (the C sources are relative to mypyc/lib-rt)
(cd mypyc/lib-rt && ../../.venv/bin/python setup.py build_ext \
  --build-lib /private/tmp/mypy-rs-<issue>-librt \
  --build-temp /private/tmp/mypy-rs-<issue>-librt-build)
PYTHONPATH=/private/tmp/mypy-rs-<issue>-librt .venv/bin/python \
  -c "from librt.internal import write_raw_bytes; print('splice active')"
```

The build covers all six in-repo modules (`internal`, `strings`,
`base64`, `vecs`, `time`, `random`). `librt` stays a namespace package,
so any module not shadowed resolves from the PyPI wheel (the fallback);
a scratch prefix containing only `librt/internal*.so` also works. CI
builds the same prefix in the `native-kernel-parity` `parity` and
`parity-mirror` jobs and prepends `LIBRT_SCRATCH` to `PYTHONPATH`, so
the splice suite runs instead of skipping.

