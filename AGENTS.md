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

The migration plan is recorded in `docs/rust-migration-strangler.md`; the
endgame and phase list in `docs/remaining-migration-plan.md`.

Follow a strangler-fig approach:

- Keep Python-facing behavior stable while adding Rust behind narrow interfaces.
- Prefer Rust adapters that exchange plain records, bytes, or stable IDs with
  Python.
- The endgame is a full Rust port (Phases F-H): Rust takes ownership of the
  `mypy.nodes` / `mypy.types` graphs. Until those flips graduate, both graphs
  stay Python-canonical and the per-call defer gates remain the mechanism:
  do not port them ad hoc.
- Native parser, native resolver, and the type kernel are default-on production
  seams behind `Options.native_*`; the type kernel is the active target.
- Measured and dropped; do not re-attempt: the import-graph prepass and the
  cache indexing/validation seam (see "Phase 4 measurement" in
  `docs/rust-migration-strangler.md`).
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

The native build and parity rules below are the always-on part; the full
rationale and history live in `docs/native-build-reference.md`.

### Feedback-loop tiers (blast radius decides the gates)

Fan-out work is gated by blast radius, not by habit. CI runs the full corpus
(`pr-gate` + `parity*`); do not duplicate it locally.

| Tier | Change class | Local gates |
| --- | --- | --- |
| T1 | docs, plans, comments, test-only | none |
| T2 | new Rust seam behind a gate with the Python fallback intact; default-off shadow or read flip; Python-only refactor | build + `cargo test` for the touched crate; the lane's own suite file (it already holds the direct seam tests and the gate-off/on differentials); ONE targeted corpus file; engagement counters (`MYPY_SERIALIZE_STATS=1` call/defer totals, seam hit counts) |
| T3 | default-ON gate flip, an `Options` default, wire format or `CACHE_VERSION`, resolver/per-SCC identity, astmerge, daemon, plugin contract, deleting a seam that answered in production | full local battery, both gate states: `testcheck` + cold self-check + fine-grained family |
| T4 | wave level | one full battery on the merged head per wave, plus one `ocr review` pass over the wave diff |

Rules that do not move: a lane may shrink the corpus, never the differential and
never the engagement proof. A change that trips an unexpected behavior change
escalates itself to T3. Cap T3 lanes at one or two per wave, since this machine
holds about two heavy ops under its memory cap.

**Measurement code is treated as evidence-critical regardless of its size.** A
probe, counter or audit script fails by printing *silently wrong numbers*, which
no test catches because the tool runs and emits values, so give any probe or
measurement change the full `ocr review` pass even when it looks like T1. Treat
its findings as defects rather than advisories in these two families, both
observed in one wave: **structural zeros** (an unreachable branch, a stats key
initialized and never incremented, a counter incremented and never read) and
**accounting bypassed on the abnormal path** (bookkeeping skipped when a wrapped
call raises; a report emitted only on `SystemExit`, so any other exception
discards the output the script exists to produce). A probe that can undercount is
worse than no probe, because its numbers are treated as primary evidence.

Pre-flight for T2/T3/T4: prove which source tree you are testing. A worktree
`.venv` is a symlink to the main checkout's venv, and that venv is a PEP 660
editable install. Two channels can resolve `import mypy` to a foreign tree
(#1789): the editable finder on `sys.meta_path` maps `mypy` to whatever tree
last installed into the venv (possibly a deleted worktree), and the interpreter
puts the current directory at `sys.path[0]`, shadowing every PYTHONPATH entry.
The old `PYTHONPATH=... python -c "import mypy"` incantation therefore
reports the tree you are standing in, not the tree you asked about. The symptom
is `_pytest.pathlib.ImportPathMismatchError` naming the main checkout's
`mypy/test/conftest.py` against yours, with 0 engagement on every seam while a
probe reports thousands.

`conftest.py` makes a hollow green impossible: before any suite code runs
(in every xdist worker, since each worker imports the conftest fresh), it
asserts that the imported `mypy` lives in the tree whose tests are collecting.
Do not remove or weaken that assertion. Before trusting any count, suite
result or engagement number, run the standalone check with the same venv
interpreter and PYTHONPATH the real run will use:

```bash
.venv/bin/python scripts/assert_worktree_import.py <tree under test>
```

Exit 0 plus the resolved path is the only acceptable result; the helper
strips the editable finder and the cwd entry, so neither channel can fool it.
A probe harness that exits 0 while raising is worse than no probe: read its
exit status before reading its numbers.

Worktree lanes: run pytest from inside the worktree, so the cwd entry is the
tree under test and the conftest assertion holds. `pyproject.toml` sets
`addopts = "-nauto"`; a fresh xdist worker re-creates the editable finder,
and the conftest assertion is what holds workers honest. For quick
single-suite runs override the fan-out with `-n0` on the command line or
`PYTEST_ADDOPTS="-n0"` (both beat `addopts`).

Heavy ops (cargo build/test, pytest, self-check) run through the weighted pool:
`/private/tmp/mypy-rs-sem.sh run 1 <build>` for a build, `run 2 <corpus>` for a
corpus run. Three slots, build = 1, corpus = 2, so a corpus and a build overlap
while two corpora never do. Class by cost, not by whether the command is pytest:
a single suite file is build-class (`run 1`, one slot, never queues behind a
corpus), while `testcheck`, a self-check or a multi-file run is `run 2`. The
flat `/private/tmp/mypy-rs-heavy.lock` mutex is retired: a build used to wait
out an entire corpus run behind it. The pool still honours the legacy lock for a
transition period, and acquiring it is a race rather than a queue, so a
mis-classed command starves (a 9-second suite was once queued 24 minutes behind
fourteen `run 2` waiters).

Review: T1/T2 use `ocr delegate preview|rule` plus an independent reader (never
the author alone, since self-review is not review); T3 and the wave diff use
`ocr review`. Cost is a wave cost, and the two measured passes span a range:
3 files at 9m37s / ~760k tokens, and 5 files at 6m56s / ~2.4M tokens, so a
single early sample understates a batched pass: roughly 3x the tokens for
under 2x the files. Treat OCR findings as leads to verify against the code,
not as authoritative statements: the 5-file pass above logged three internal
`file_read failed: invalid line range` errors before emitting confident
findings. Prefer `ocr delegate preview|rule` when the rule set is what is
wanted rather than a verdict.

### Native resolver / dependency-records parity

The native resolver and dependency-records extraction are behind
`Options.native_resolver`, default `True`. The daemon and parallel mode force
it on; only Bazel stays on the Python resolver (`_native_gate_active`).

Build the extension before parity, using the `cargo rustc` + scratch-dir
approach documented below, never `maturin develop`:

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

`TEST_NATIVE_RESOLVER` is a harness differential, not a production switch: the
suites set `options.native_resolver` from it *after* option parsing, so unset
exercises the default-on path and `=0` forces the Python fallback.

### Native parser build order

The native parser (`Options.native_parser`, defaulted on and force-on under
parallel mode) is backed by the `ast_serialize` extension. The serialized AST
format is fixed by `crates/ast_serialize/src/lib.rs` and read by
`mypy/nativeparse.py`; the two must stay in lockstep.

**Rebuild the extensions after any change to `crates/ast_serialize/src/lib.rs`
or `crates/module_resolver/src/`.** A stale binary produces silent
deserialization mismatches (e.g. `AssertionError: 255`, END_TAG read where a
LOCATION tag was expected) that crash parallel workers during self-check.

Do **not** use `maturin develop` for these crates: `crates/ast_serialize` has no
`pyproject.toml`, so maturin picks up the repo-root one and installs a bogus
`mypy-0.1.0` that shadows the real mypy. A second hazard: `pyproject.toml`
declares the PyPI `ast-serialize` stub (type stubs only, no `parse`
implementation), so without the Rust `.so` on `PYTHONPATH`, `import
ast_serialize` resolves to the stub and fails with `no attribute 'parse'`. A
third: a real but stale wheel (e.g. 0.6.0) is importable yet rejects the current
`include_docstrings` keyword, surfacing as `INTERNAL ERROR` with
`TypeError: parse() got an unexpected keyword argument 'include_docstrings'`
instead of the stub message. `parse_to_binary_ast` rewrites both failure shapes
into a pointed `RuntimeError` naming the remedy. Build to a scratch dir and put
it on `PYTHONPATH`:

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

`mypy_self_check.ini` sets `num_workers = 4`, which forces both gates on, so
the self-check is the cheapest end-to-end gate after a rebuild.

### librt build order (F3 wire-cache splice)

The F3 wire cache (`_write_type_cached`, `mypy/types.py`) splices cached wire
bytes through `librt.internal.write_raw_bytes`. The PyPI `librt` wheel does not
export that symbol, so the import degrades to `None`, `_write_type_cached` falls
back to plain `t.write`, and the `NativeMirrorSpliceSuite` splice tests
self-skip. Never install or upgrade `librt` in the shared `.venv`: every
worktree symlinks it, and a mutation breaks parallel agents mid-run. Build the
in-repo lib-rt into a private scratch prefix instead:

```bash
# from the repo root (the C sources are relative to mypyc/lib-rt)
(cd mypyc/lib-rt && ../../.venv/bin/python setup.py build_ext \
  --build-lib /private/tmp/mypy-rs-<issue>-librt \
  --build-temp /private/tmp/mypy-rs-<issue>-librt-build)
PYTHONPATH=/private/tmp/mypy-rs-<issue>-librt .venv/bin/python \
  -c "from librt.internal import write_raw_bytes; print('splice active')"
```

`librt` stays a namespace package, so modules not shadowed resolve from the PyPI
wheel. CI builds the same prefix and prepends `LIBRT_SCRATCH` in the
`native-kernel-parity` `parity` / `parity-mirror` jobs.

### Type kernel build order

Build and install the `type_kernel` extension the same way as the other
extensions:

```bash
cargo rustc -p mypy-type-kernel --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/release/libtype_kernel.dylib \
  /private/tmp/mypy-rs-local-typekernel/type_kernel.cpython-313-darwin.so
codesign -f -s - /private/tmp/mypy-rs-local-typekernel/*.so
```

The seam-by-seam inventory (wave ledger, per-seam defer audits, gate
names, wire invariants) is archived at
`docs/plans/type-kernel-seam-ledger.md`. Load it when working on a
kernel seam; it does not belong in always-on context.

## Pull Requests

The default branch on this fork is `main` (not `master`). Always target
`main` as the PR base. Branch from `main` before committing — do not commit
directly to `main`.
