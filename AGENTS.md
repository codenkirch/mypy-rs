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

The type kernel (`Options.native_type_kernel`, default-on since #58) is
backed by the `type_kernel` Rust extension. It implements Rust ports of
the kernel's hot-path functions that walk live Python `Type` objects,
including:

- `erase_type` (Stage 1) — mirrors `mypy.erasetype.EraseTypeVisitor`.
- `remove_instance_last_known_values` (Stage 2) — mirrors
  `mypy.erasetype.LastKnownValueEraser` (a `TypeTranslator`).
- `read_type_to_str` (Stage 3a) — parity-only: reads a serialized
  `mypy.types.Type` from its binary wire format and returns
  `str(t)`. Not wired into any production path; used by
  `NativeTypeWireSuite` to prove the Rust `Type` enum + reader
  reconstructs the same type. Foundation for Stage 3c (`is_subtype`).
- `get_type_triggers` (M28) — mirrors
  `mypy.server.deps.TypeTriggersVisitor` in `crates/type_kernel/src/
  serverdeps.rs`; the `DependencyVisitor` AST walk stays in Python.
  Gated via `_set_native_server_deps_active` (wired from
  `mypy/server/deps.py` by `mypy/build.py`) and covered by
  `NativeServerDepsSuite` in `mypy/test/testtypes.py`.
- `rust_classify_decorators` (issue #348) — mirrors the decorator
  classification dispatch in `mypy.semanal.SemanticAnalyzer.visit_decorator`
  (semanal.py:1831-1897). Rust assigns each decorator a tag by the same
  `refers_to_fullname` / `get_deprecated` name-sets and branch order; Python
  applies the side effects (AST mutation, error reporting, scope checks).
  Parity-gated behind the semanal_visitor gate and unit-tested by
  `NativeDecoratorClassifySuite` in `mypy/test/testtypes.py`.
- `rust_classify_with_metaclass` (issue #914) — mirrors the
  `six.with_metaclass` base-side classifier head of
  `SemanticAnalyzer.infer_metaclass_and_bases_from_compat_helpers`
  (semanal.py:3327-3336). Rust decides from three scalar facts (callee
  fullname, args_len, all-positional) whether the base is a
  `six.with_metaclass` / `future.utils.with_metaclass` /
  `past.utils.with_metaclass` call; Python runs `analyze_type_expr`
  first and applies the two side effects (`with_meta_expr = args[0]`,
  `defn.base_type_exprs = args[1:]`). Gated by the semanal_visitor gate
  and covered by `NativeCompatMetaclassHelperSuite` in
  `mypy/test/testtypes.py`.
- `rust_classify_add_metaclass` (issue #917) — mirrors the
  `@six.add_metaclass(M)` decorator-side classifier head of
  `SemanticAnalyzer.infer_metaclass_and_bases_from_compat_helpers`
  (semanal.py:3373-3377). Rust decides from three scalar facts
  (callee fullname `== "six.add_metaclass"`, args_len `== 1`,
  first arg positional) whether the decorator is an add-metaclass call;
  Python runs `dec_expr.callee.accept(self)` first and applies the
  side effect (`add_meta_expr = args[0]`, break). Gated by the
  semanal_visitor gate and covered by `NativeCompatMetaclassHelperSuite`
  in `mypy/test/testtypes.py`.
- `rust_classify_lvalue_validity` (issue #934) — mirrors the 2-way
  dispatch head of `SemanticAnalyzer.check_lvalue_validity`
  (semanal.py:5445-5449): Rust reads the live `node` via PyO3
  `is_instance` against `mypy.nodes.TypeVarExpr` and
  `mypy.nodes.TypeInfo` and returns a branch tag (PASS / TYPEVAR /
  TYPEINFO); the Python shim applies the `self.fail("Invalid assignment
  target", ctx)` and `self.fail(message_registry.CANNOT_ASSIGN_TO_TYPE,
  ctx)` side effects. Never defers: every reachable branch is classified.
  Gated by the semanal_visitor gate (`_native_semanal_visitor_active`,
  wired from `mypy/build.py`) and covered by
  `NativeLvalueValiditySuite` in `mypy/test/testtypes.py` (direct seam
  tag tests + gate-off vs gate-on differential on the fail message
  list), plus pure decision unit tests in `semanal_bases.rs`.
- `rust_classify_configure_bases` + `rust_classify_configure_mro`
  (issue #1035) — mirror the per-base dispatch and MRO tail of
  `SemanticAnalyzer.configure_base_classes` (semanal.py:3395-3436).
  Rust classifies every base from wire bytes plus the `is_newtype`
  scalars (tuple / instance / newtype-fail / Any ok / Any fail /
  TypedDict-fallback / invalid) and folds the
  `disallow_any_unimported` walk and `check_for_explicit_any` flag
  into per-base emit flags; the MRO call folds `verify_base_classes`
  (identity `is_base_class` walk, PyO3 `.is()`) and
  `verify_duplicate_base_classes` (`rich_compare` Eq, mirroring
  `find_duplicate`) into a tail tag with cyclic indices and the
  duplicate name. Python applies every fail, `unimported_type_becomes_any`
  / `explicit_any`, `fallback_to_any`, `info.bases`, the implicit-object
  append, `configure_tuple_base_class`, and the `set_dummy_mro` /
  `set_any_mro` / `calculate_class_mro` writes; a `None` tail or unreadable
  attribute defers to the pure body. Gated by the semanal_visitor gate
  and covered by `NativeConfigureBasesSuite` in `mypy/test/testtypes.py`
  (direct seam tag tests + gate-off vs gate-on differential), plus pure
  decision unit tests in `semanal_bases.rs`.
- `rust_classify_declared_metaclass` (issue #1037) — mirrors the gate
  chain of `SemanticAnalyzer.get_declared_metaclass` (semanal.py:3767):
  Rust classifies the declared-metaclass expression from the name, the
  looked-up symbol node, the wire-serialized Var type, and the resolved
  symbol (tags OK / DYNAMIC / NAME_ERROR / ANY / DEFER / INVALID /
  NOT_METACLASS). Python performs the `lookup_qualified` and the pure
  alias unwrap feeding the classifier, then applies the four fails and
  the `fill_typevars` construction. Rust short-circuits like Python:
  `tuple_type`/`is_metaclass` are only read when the symbol is a
  `TypeInfo`, so non-TypeInfo nodes (Var/Placeholder) never trigger a
  getattr defer. Gated by the semanal_visitor gate and covered by
  `NativeDeclaredMetaclassSuite` in `mypy/test/testtypes.py` (direct
  seam tag tests + gate-off vs gate-on differential), plus pure decision
  unit tests in `semanal_metaclass.rs`.
- `rust_classify_recalculate_metaclass` (issue #1037) — mirrors the
  branch selection of `SemanticAnalyzer.recalculate_metaclass`
  (semanal.py:3837): Rust reads the live `defn.info` via PyO3 (the MRO
  scan for a protocol base, `metaclass_type` presence + `builtins.type`
  fullname + `enum.EnumMeta` base, and the non-empty `defn.type_vars`)
  and returns a 4-way tag (OK / ABCMETA / IS_ENUM / ENUM_GENERIC_FAIL);
  the Python shim applies the two idempotent prelude writes
  (`declared_metaclass`, `metaclass_type` via the live
  `calculate_metaclass_type`), the `named_type_or_none("abc.ABCMeta")`
  install, the `is_enum = True` write, and the "Enum class cannot be
  generic" fail, keeping the pure-Python body as the fallback. Gated by
  the semanal_visitor gate and covered by `NativeDeclaredMetaclassSuite`
  in `mypy/test/testtypes.py`, plus pure decision unit tests in
  `semanal_metaclass.rs`.
- `rust_bind_self` (issue #492) — mirrors `mypy.typeops.bind_self`'s
  non-generic fast path (typeops.py:540-641): strips the first parameter
  and sets `is_bound=True` for non-variable-carrying `CallableType`s. Rust
  defers (`None`) for generic signatures (needs `infer_type_arguments`),
  so the typevar path stays in Python. The Python shim uses the Rust
  result as a "handled" signal and builds the final object through
  `copy_modified` on the live object so non-wire fields survive. Covered
  by `NativeBindSelfSuite` in `mypy/test/testtypes.py`. Its
  `class_callable` follow-up is `rust_class_callable` (below).
- `rust_fill_typevars` (issue #492) — mirrors
  `mypy.typevars.fill_typevars` (typevars.py:43-85) on a live `TypeInfo`:
  Rust reads `fullname`, `defn.type_vars`, and `tuple_type`, serializes
  each type parameter to the wire format (line/column drop to -1 in the
  round-trip), and wraps `TypeVarTupleType` in `UnpackType`. The Python
  shim uses the decoded tvar-arg list only; the root `Instance` and the
  named-tuple wrapper are rebuilt on the live `typ` so a stale wire-map
  entry cannot substitute a different `TypeInfo` (fine-grained
  regression guard). Gated by `_native_typevars_active` (wired from
  `mypy/build.py`) and covered by `NativeFillTypevarsSuite` in
  `mypy/test/testtypes.py`.
- `rust_class_callable` (issue #492 follow-up) — mirrors the `ret_type`
  decision + type-variable combination in `mypy.typeops.class_callable`
  (typeops.py:428-486). Rust picks `ret_type` (the explicit `__new__` /
  `__init__` return vs. `fill_typevars(info)`) and combines
  `info.defn.type_vars + init.variables`, reading the live `info`
  (`defn.type_vars`, `is_protocol`) via PyO3. The two resolver-backed
  subtype checks (`is_equivalent` / `is_subtype`, both
  `ignore_type_params=True`) run on the Python side (already native via
  the subtype resolver) and are passed in as booleans, avoiding a resolver
  seam. The Python shim rebuilds the live `CallableType` via
  `copy_modified` so non-wire fields survive, and `instance_type` MUST
  stay the live `fill_typevars(info)` result. Gated by
  `_native_typeops_active` (wired from `mypy/build.py`) and covered by
  `NativeClassCallableSuite` in `mypy/test/testtypes.py`, plus 10 pure
  decision unit tests in `typeops.rs`.
- `try_getting_literal` (mypy.checkexpr) — mirrors
  `mypy.checkexpr.try_getting_literal` (checkexpr.py:8339): unwraps an
  `Instance`'s `last_known_value` to the precise LiteralType, or returns
  `get_proper_type(typ)` unchanged. Pure wire call (no resolver); the
  round-trip carries the `last_known_value` field and fixup resolves
  type_refs to live TypeInfo. Gated by `_native_checkexpr_active`
  (wired from `mypy/build.py`) and covered by
  `NativeTryGettingLiteralSuite` in `mypy/test/testtypes.py`.
- `has_erased_component` (mypy.checkexpr) — mirrors
  `mypy.checkexpr.has_erased_component` (checkexpr.py:8060): a
  `BoolTypeQuery(ANY_STRATEGY)` whose only true leaf is `ErasedType`.
  Requires the `ErasedType` wire tag (`ERASED_TYPE = 122` in
  `mypy/types.py` + the Rust `Type::ErasedType` variant); `ErasedType`
  was previously un-serializable, which also kept `replace_meta_vars`
  absent an ErasedType replacement. That kernel path now defers on an
  `ErasedType` target (`rust_replace_meta_vars` guard) so inference
  semantics stay identical. **Invariant:** the Python `read_type` in
  `mypy/types.py` deliberately does NOT decode tag 122 (there is no
  `ERASED_TYPE` branch in `read_type`), so any Rust wire seam that
  emits `ErasedType`-carrying bytes fails to round-trip through
  `read_type` and its `AssertionError`/`NotImplementedError` guard
  defers to the pure-Python fallback. `has_erased_component` itself is
  exempt: its Python seam passes bytes into the kernel and reads back a
  bare `bool`, never decoding `ErasedType` into Python. Removing this
  invariant causes a deep `is_protocol_implementation` ↔
  `is_callable_compatible` ↔ `is_subtype` recursion (mypy #21445
  fragility) on `ziplike`/`f0-overload`. Covered by
  `NativeHasErasedComponentSuite` in `mypy/test/testtypes.py`.
- Wire-cache placeholder guard (mypy.checker) — a wire seam's
  `read_type` can populate `instance_cache` with a `NOT_READY`
  placeholder (`type_ref` set, `.type` = `FakeInfo`) whose fixup is
  re-raced away by a concurrent clear+re-create (`_fix_wire_type`,
  `_native_decode_well_formed`). `TypeChecker.named_type` now routes
  all five cache primitives through `_validated_named_type`, which
  rebuilds from the live `TypeInfo` when the cached entry is absent or
  still carries a `FakeInfo`. Without this, a poisoned `str_type`
  leaked into `infer_literal_expr_type` → `copy_modified` → Python
  fallback → `AssertionError: De-serialization failure: TypeInfo not
  fixed` inside `_is_subtype` (`testSpecialSignatureForSubclassOfDict2`).
- `check_overlapping_overloads` screening loop (mypy.checker): the Rust
  driver in `overload_override.rs` runs the pairwise screening part of
  `TypeChecker.check_overlapping_overloads` (checker.py:1559-1603) in one
  call on wire callables (each signature serialized once instead of once per
  predicate): the argument-count gate, the never-match predicate, the
  unsafe-overlap predicate under `strict_optional=True`, and the flip note.
  The impl-vs-items tail and the message emission stay in Python. Engages
  only when every item's `var.type` is already a plain `CallableType`;
  defers (`None`) on any pair a predicate cannot decide and the Python shim
  runs the original pure-Python loop. Gated by `_native_checker_active`
  (wired from `mypy/build.py`) and covered by
  `NativeOverloadingOverloadsSuite` in `mypy/test/testtypes.py` (gate-off
  vs gate-on differential on the decision lists).
- `rust_classify_final_super` (mypy.checker): the Rust classifier in
  `checker_functions.rs` ports the pure decision of
  `TypeChecker.check_compatibility_final_super` (checker.py:4608-4636):
  the base-node kind gate (`Var`/`FuncBase`/`Decorator` via PyO3
  `is_instance`), the `is_private(name)` pass, the
  `base_node.is_final and (node.is_final or not Var)` cant-override arm,
  the enum-base / enum-special-prop pass, the writability arm, and the
  trailing pass. Rust returns a branch tag; the Python shim applies the
  `cant_override_final` message and `check_if_final_var_override_writable`
  side effects and keeps the pure-Python body as the fallback. Defers
  (`None`) only on an unreadable `base_node.is_final`. Gated by
  `_native_checker_active` and covered by `NativeFinalSuperSuite` in
  `mypy/test/testtypes.py` (direct seam tag tests + gate-off vs gate-on
  differential).
- `rust_classify_classvar_super` (mypy.checker, issue #938): the Rust
  classifier in `checker_functions.rs` ports the pure 2x2 predicate of
  `TypeChecker.check_compatibility_classvar_super` (checker.py:4796-4807):
  the `not isinstance(base_node, Var)` pass, then the
  `node.is_classvar and not base_node.is_classvar` instance-var violation,
  then the `not node.is_classvar and base_node.is_classvar` class-var
  violation, then the trailing pass. Rust reads `isinstance(base_node,
  Var)` and `base_node.is_classvar` via PyO3 and returns a branch tag;
  the Python shim applies the `CANNOT_OVERRIDE_INSTANCE_VAR` /
  `CANNOT_OVERRIDE_CLASS_VAR` `self.fail` side effects and keeps the
  pure-Python body as the fallback. Defers (`None`) only on an
  unreadable `base_node.is_classvar`. Gated by `_native_checker_active`
  and covered by `NativeCompatibilityClassvarSuperSuite` in
  `mypy/test/testtypes.py` (direct seam tag tests + gate-off vs gate-on
  differential), plus 5 pure decision unit tests in
  `checker_functions.rs`.
- `rust_classify_new_signature` (mypy.checker, issue #920) — ports the
  3-way `__new__` return-type decision head of
  `TypeChecker.check___new___signature` (checker.py:2630-2664): Rust
  classifies metaclass / non-instance / instance from two scalar facts
  (`fdef.info.is_metaclass()` and whether
  `get_proper_type(bound_type.ret_type)` is one of {AnyType, Instance,
  TupleType, UninhabitedType, LiteralType}) and returns a branch tag. The
  two `check_subtype` calls and the `INVALID_NEW_TYPE` /
  `NON_INSTANCE_NEW_TYPE` emission (via `format_type`) stay in Python.
  Every branch is classified; `None` is the exception-only deferral.
  Gated by `_native_checker_active` and covered by
  `NativeNewSignatureSuite` in `mypy/test/testtypes.py` (direct seam tag
  tests + gate-off vs gate-on differential), plus 3 pure decision unit
  tests in `checker_functions.rs`.
- `rust_classify_func_def_override` (issue #921, mypy.checker): the Rust
  classifier in `checker_functions.rs` ports the 5-way dispatch head of
  `TypeChecker.check_func_def_override` (checker.py:2106-2162): the
  function-overrides-function arm (`isinstance(original_def, FuncDef)`),
  the `orig_type is None` return, the `PartialType` fill arm, the
  `PartialType` invalid-redefinition arm, the binder-assign +
  `check_subtype` arm, and the implicit no-op tail for an already-invalid
  redefinition. The Python shim extracts five scalar bools
  (`is_funcdef`, `orig_type_is_none`, `is_partial`,
  `partial_type_is_none`, `is_invalid_redefinition`) and applies the
  branch body in Python (`function_type`/`is_same_type`,
  `find_partial_types`, `binder.assign_type`, `check_subtype`, error
  emission). Never defers (`None` only on arg-decoding failure). Gated
  by `_native_checker_active` and covered by `NativeFuncDefOverrideSuite`
  in `mypy/test/testtypes.py` (direct seam tag tests + gate-off vs
  gate-on differential).
- `rust_classify_getattr_method` (issue #985, mypy.checker): the Rust
  classifier in `checker_functions.rs` ports the 4-way dispatch head of
  `TypeChecker.check_getattr_method` (checker.py:3066-3093): module scope
  + `__getattribute__` -> fail; module scope -> 1-arg expected signature;
  class scope -> 2-arg; else pass. Rust reads the live `Scope` via PyO3
  (`len(scope.stack) == 1`, `scope.active_class()`) plus the `name`
  string and returns a branch tag; the Python shim builds the fixed
  `CallableType` via `named_type`, runs `is_subtype` (already native),
  and emits MODULE_LEVEL_GETATTRIBUTE /
  invalid_signature_for_special_method. Defers (`None`) on an unreadable
  `scope.stack` or `active_class()` result. Gated by
  `_native_checker_active` and covered by `NativeGetattrMethodSuite` in
  `mypy/test/testtypes.py` (direct seam tag tests + gate-off vs gate-on
  differential), plus 4 pure decision unit tests in
  `checker_functions.rs`.
- `rust_classify_metaclass_compat` (issue #922) — mirrors the pure bool
  predicate head of `TypeChecker.check_metaclass_compatibility`
  (checker.py:3918-3941): Rust reads the exempt flags off the live
  `TypeInfo` via PyO3 (`is_metaclass` computed via
  `rust_typeinfo_is_metaclass`, plus `is_protocol`/`is_named_tuple`/
  `is_enum`/`typeddict_type`/`metaclass_type`) and walks `info.bases` to
  test whether any base carries a metaclass. Returns a branch tag:
  0 = exempt/no-conflict, 1 = conflict-needs-fail. The Python shim applies
  the `self.fail` (METACLASS code) and `explain_metaclass_conflict()` +
  `self.note` side effects and keeps the pure-Python body as the fallback.
  Defers (`None`) only on an unreadable attribute. Gated by
  `_native_checker_active` (wired from `mypy/build.py`) and covered by
  `NativeMetaclassCompatibilitySuite` in `mypy/test/testtypes.py` (direct
  seam tag tests + gate-off vs gate-on differential on fail/note pairs),
  plus pure decision unit tests in `checker_functions.rs`.
- `rust_classify_enum_new` (issue #923): the Rust fold in
  `checker_functions.rs` mirrors `TypeChecker.check_enum_new`
  (checker.py:3739-3766): an enum base scans `mro[1:-1]` for a non-enum
  mixin exposing `__new__`; a non-enum base tests `__new__` directly; a
  second mixin returns the CONFLICT tag. Rust reads the live
  `defn.info.bases` and returns one SKIP/ADVANCE/CONFLICT tag per base;
  the Python shim applies `self.fail` and tracks `has_new`, keeping the
  pure-Python body as the fallback. Gated by `_native_checker_active` and
  covered by `NativeEnumNewSuite` in `mypy/test/testtypes.py`.
- `rust_classify_enum_bases` (issue #937): the Rust fold in
  `checker_functions.rs` mirrors `TypeChecker.check_enum_bases`
  (checker.py:3850-3876): once an enum base is seen, a later non-enum
  mixin base is an error. Rust reads each `base.type.is_enum` bool via
  PyO3 and returns `(enum_base_idx, violating_idx)` where
  `violating_idx` is the index of the first non-enum base after an enum
  base (-1 if none); the Python shim applies `self.fail` with the
  offending enum base's `str_with_options`, keeping the pure-Python body
  as the fallback. Gated by `_native_checker_active` and covered by
  `NativeEnumBasesSuite` in `mypy/test/testtypes.py`.
- `rust_classify_enum` (issue #971): the Rust classifier in
  `checker_functions.rs` mirrors the three arms of
  `TypeChecker.check_enum` (checker.py:3843-3870): (a) `__members__`
  override fail, (c) the final-enum base loop over `mro[1:-1]`, and
  (b) the stub-empty-enum fail+note. Rust reads the live `defn.info`
  (`names` as a dict, `fullname`, `mro` as a list, `enum_members`) via
  PyO3 plus scalar facts (`is_stub`, `tree_fullname`, and the
  `ENUM_BASES` allowlist) and returns `(tag, base_names)`: tag is a
  bit flag (1 = members-override, 2 = stub-empty) and base_names are
  the arm-(c) offending base fullnames. The Python shim applies
  `self.fail` / `self.note` / `check_final_enum` and then calls
  `check_enum_bases` / `check_enum_new`, keeping the pure-Python body
  as the fallback. Defers (`None`) on a non-dict `names` or non-list
  `mro`, or an unreadable `Var.has_explicit_value` / `enum_members`.
  Gated by `_native_checker_active` and covered by
  `NativeEnumCheckSuite` in `mypy/test/testtypes.py` (direct seam tag
  tests + gate-off vs gate-on differential), plus pure decision unit
  tests in `checker_functions.rs`.
- `rust_is_final_enum_value` (issue #936) — mirrors
  `TypeChecker.is_final_enum_value` (checker.py:3825-3848): a pure bool
  predicate over a `SymbolTableNode`. FuncBase/Decorator -> False (a
  method is fine); non-Var -> True (class or anything else); for a Var,
  a private/dunder/sunder name or a `FunctionLike` proper type -> False,
  else `is_stub or has_explicit_value`. Rust reads the live node via PyO3
  (isinstance against FuncBase/Decorator/Var, the `name` string,
  `get_proper_type(node.type)` is `FunctionLike`, `has_explicit_value`)
  and returns the bool directly, mirroring `rust_is_magic_base` (never
  defers). Gated by `_native_checker_active` (wired from `mypy/build.py`)
  and covered by `NativeIsFinalEnumValueSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus direct
  seam calls), plus Rust unit tests for the name predicates in
  `checker_functions.rs`.
- `rust_check_for_untyped_decorator` (issue #942) — mirrors
  `TypeChecker.check_for_untyped_decorator` (checker.py:6955-6964): the
  bool gate `disallow_untyped_decorators and is_typed_callable(func.type)
  and is_untyped_decorator(dec_type) and not current_node_deferred`. Rust
  folds the two wire-format type sub-predicates (reusing the existing
  `is_typed_callable` / `is_untyped_decorator` ports in
  `checkexpr_functions.rs`) with the two scalar flags, short-circuiting in
  Python order; the Python shim emits `typed_function_untyped_decorator`
  when the result is True and keeps the pure-Python body as the fallback.
  Defers (`None`) on an undecodable blob or a deferred sub-predicate (an
  Instance decorator whose `__call__` needs live TypeInfo). Gated by
  `_native_checker_active` (wired from `mypy/build.py`; the Python shim
  mirrors `rust_classify_final_super` gating) and covered by
  `NativeUntypedDecoratorSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential plus direct seam calls), plus pure decision unit
  tests in `checker_functions.rs`.
- `rust_check_explicit_override_decorator` (issue #939) — mirrors the
  5-flag bool conjunction head of
  `TypeChecker.check_explicit_override_decorator` (checker.py:3139-3160):
  `not plugin_generated and found_method_base_classes and not
  defn.is_explicit_override and defn.name not in ("__init__", "__new__")
  and not is_private(defn.name)`. Rust reads the 5 scalar flags via PyO3
  (`plugin_generated` from `defn.info.get(defn.name).plugin_generated`,
  `found_method_base_classes` truthiness, `defn.is_explicit_override`,
  `defn.name` dunder membership, and `is_private(name)` via the local
  helper) and returns a bool; the Python shim emits
  `self.msg.explicit_override_decorator_missing(name, base_fullname,
  context)` when true and keeps the pure-Python body as the fallback.
  Returns `false` (defer) when `defn.info` is None, the symbol lookup is
  None, or any flag is unreadable, mirroring the Python default for
  `plugin_generated`. Gated by `_native_checker_active` (wired from
  `mypy/build.py`) and covered by `NativeExplicitOverrideDecoratorSuite`
  in `mypy/test/testtypes.py` (gate-off vs gate-on differential on the
  captured message records plus direct seam calls proving engagement),
  and pure decision unit tests in `checker_functions.rs`.
- `rust_classify_check_lvalue` (issue #955) — mirrors the dispatch head of
  `TypeChecker.check_lvalue` (checker.py:5568-5632): computes
  `skip_definition` (the `allow_redefinition` + `NameExpr`-node-`Var` +
  `is_inferred` + `type is not None` + not `PartialType` + not
  `is_index_var` conjunction) then a 6-way dispatch on lvalue node kind
  (NameExpr-definition, MemberExpr-definition, IndexExpr, MemberExpr,
  NameExpr, TupleExpr/ListExpr, StarExpr, else). Rust reads the live
  lvalue node-kind tags (isinstance via PyO3) and the `Var` node facts
  needed for `skip_definition` and returns a branch tag; the Python shim
  runs each branch body (`accept` / `analyze_ordinary_member_access` /
  `analyze_ref_expr` / `store_type` / recursion) and returns
  `(lvalue_type, index_lvalue, inferred)`. Defers (`None`) only on an
  unreadable node fact. Gated by `_native_checker_active` (wired from
  `mypy/build.py`) and covered by `NativeCheckLvalueSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential across all 8
  branches plus direct seam calls), plus pure decision unit tests in
  `checker_functions.rs`.
- `rust_classify_unbound_front` (issue #714) — mirrors the decision
  front of `mypy.typeanal.TypeAnalyser.visit_unbound_type_nonoptional`
  (typeanal.py:310-549): Rust classifies the resolved-symbol dispatch hub
  (unresolved symbol, `PlaceholderNode`, `node is None`, `ParamSpecExpr`,
  `TypeVarExpr`, `TypeVarTupleExpr`) from raw node facts (ints, bools, the
  `alias_type_params_names` string list, the type name) and returns a
  branch tag; the Python shim applies the side effects (defer /
  `record_incomplete_ref` / `fail`) and rebuilds the result object. The
  plugin hook path, non-front node kinds (Var, TypeAlias, TypeInfo, ...),
  and unbound non-alias `TypeVarExpr` defer (`None`) to the pure-Python
  body; `tvar_scope.get_binding` and the "a typevar param is a
  `PlaceholderType` → `api.defer()`" pre-check stay Python-side (the
  shim re-flags the pre-check as a fact and re-applies the deferral on a
  decided tag). Gated by `_set_native_typeanal_active` (wired from
  `mypy/build.py`) and covered by `NativeUnboundBranchFrontSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential on the
  result string, error messages, and defer / record counts), plus pure
  decision unit tests in `typeanal_unbound2.rs`.
- `rust_classify_special_unbound` (issue #720) — mirrors the special-form
  dispatch classifier of
  `mypy.typeanal.TypeAnalyser.try_analyze_special_unbound_type`
  (typeanal.py:987-1199, the `builtins.None` / `Any` / `Final` / `Tuple` /
  `Union` / `Optional` / `Callable` / `Type` / `TypeForm` / `ClassVar` /
  `Never` / `Annotated` / `Required` / `NotRequired` / `ReadOnly`
  elif-chain): Rust decides the branch from scalar facts (fullname + arity
  + `empty_tuple_index` + `allow_typed_dict_special_forms` + the
  `not_in_*` flags + the Tuple lookup / ellipsis-form flags), returning a
  branch tag; the Python shim applies the side effects (fail / note /
  `record_incomplete_ref`) and rebuilds the result objects. Branches the
  classifier cannot decide purely (`Literal`, `TypeGuard`, `Unpack`,
  `Self`, the non-special tail, and every gold path that recurses) defer
  (`None`) to the pure-Python body. Order of checks matches the original:
  `Union` has no arity check (collapses via `make_union`), bare
  `typing.Type` builds `TypeType(Any)` while bare `builtins.type` returns
  `None` (#9476), `ClassVar` runs its nesting / TypedDict-prohibit / alias
  checks before the arg-count dispatch, and the Required/NotRequired/
  ReadOnly bad-context check runs before their arity check. Gated by
  `_set_native_typeanal_active` (wired from `mypy/build.py`) and covered by
  `NativeTryAnalyzeSpecialUnboundSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential on the result string and captured
  fail/note messages, plus a direct seam call proving engagement from the
  scalar facts), and pure decision unit tests in `typeanal_special.rs`.
- `rust_classify_literal_param` (issue #919) — mirrors the 9-way
  dispatch head of `mypy.typeanal.TypeAnalyser.analyze_literal_param`
  (typeanal.py:2474-2557): string-Literal from `original_str_expr`
  (branch a, checked pre-`get_proper_type` on the original arg), Any
  fail/silent (branch c, splits on `type_of_any` ∈ {from_error,
  special_form}), `RawExpressionType` float/complex/arbitrary/
  with-value (branch d, splits on `literal_value is None` +
  `simple_name`), `NoneType`/`LiteralType` pass-through (branch e),
  Instance `last_known_value` extraction (branch f), `UnionType`
  recursion (branch g), and the invalid tail (branch h). Rust returns
  a branch tag (i64, 1-10) from scalar isinstance facts; the Python
  shim (`_native_analyze_literal_param`) applies all side effects
  (LiteralType construction via `named_type`, error emission,
  `visit_unbound_type` recursion, union merge). The shim is two-phase:
  phase 1 checks branch (a) on the original arg (pre-ProperType);
  phase 2 runs the unbound recursion + `get_proper_type` in Python,
  extracts post-chain facts, and classifies branches (c)-(h). No
  `None` deferral: every path maps to a tag (unlike the special-unbound
  and unbound-front classifiers which defer on recursion / plugin
  hooks). Gated by `_set_native_typeanal_active` (wired from
  `mypy/build.py`) and covered by `NativeLiteralParamSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential on
  `str(result)` and captured fail messages across all 9 branches,
  plus a direct seam call proving engagement and a str-beats-Any
  ordering test), and 18 pure decision unit tests in
  `typeanal_literal.rs`.
- `rust_join_instances` — mirrors
  `InstanceJoiner.join_instances` (join.py:208-303): same-type
  args-less (fresh Instance when LKV present), same-type with args
  (via `visit_instance_with_args`; covariant/invariant per-arg join +
  upper-bound check), variadic single-arg (`tuple[X, ...]` rewrap),
  and different-type args-less nominal join (promote-aware
  `join_instances_via_supertype`), with the Python `seen_instances`
  guard mirrored by a Rust-side `(type_ref, encoded-args)` seen Vec
  and `object_from_instance` fallback on a hit. Defers (None) for
  ParamSpec type vars, TypeVarTuple multi-arg / prefix/suffix splits,
  `type_var.values` non-empty, `fallback_to_any`, different-type with
  args, `VARIANCE_NOT_READY` (PEP695 snapshot froze before
  `infer_class_variances` ran; mirrors `subtypes.rs:1980`), and any pair
  already on the Python `seen_instances` stack
  (checked before the shim call so the Python guard still wins).
  Gated by `_native_join_active` (per-call inline gate in
  `InstanceJoiner.join_instances`) and covered by
  `NativeJoinInstancesSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential across the handled paths, plus a direct seam
  call and a pre-seeded `seen_instances` recursion-guard test).
- `rust_function_type` / `rust_callable_type` (issue #747) — mirrors
  `mypy.typeops.function_type` (typeops.py:1422-1482) and
  `mypy.typeops.callable_type` (typeops.py:1485-1509). The
  `function_type` seam classifies the node (typed passthrough,
  broken-overload dummy, or FuncItem self-binding) and the
  `callable_type` seam builds the `CallableType` from a live `FuncItem`
  (`arg_names`/`arg_kinds`/`info`/`has_self_or_cls_argument`/`is_class`,
  plus an optional serialized `ret_type`). For the self-binding arm Rust
  rebuilds the args via `fill_typevars_inner` and wraps the first in
  `TypeType` for classmethods/`__new__`. FuncDef and LambdaExpr callers
  both restore the non-wire line/column/name/definition via
  `copy_modified`. The checkexpr lambda callback
  (`checkexpr.py:visit_lambda_expr`) gates on `_native_checkexpr_active`
  and defers on wire decode or fixup failure. Covered by
  `NativeFunctionTypeSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential on `str()`/line/column/definition plus direct seam
  calls proving engagement, including a lambda-with-`ret_type` case).
- `rust_join_type_list` (issue #824, re-enabled for #816) — Rust drives
  `mypy/join.join_type_list` (join.py:1508-1529): the empty-list
  `UninhabitedType` base case, the identity-safe single-item
  passthrough, and the pairwise fold through the setops join kernel
  (`join_one_pair`: Instance nominal join, union flattening,
  CallableType similarity + combine, TypeType, TypeVar default to
  `object`, Any / None-right absorption). Defers (`None`) — the whole
  call falls back to the pure-Python fold — on any item carrying a
  `last_known_value`, any `fallback_to_any` class item, or any pair the
  setops kernel cannot decide (missing snapshot entries, undecidable
  subtype directions, variadic tuple splits). Gated by
  `_native_join_active` and exercised by the gate-on/off parity
  differential of the full join suites in `mypy/test/testtypes.py`
  (`NativeJoinTypesSuite`, `NativeJoinTypeListSuite`), plus Rust unit
  tests (`test_join_type_list_*` in `checker_helpers.rs`).
- `rust_solve_generic_call` (issue #826) — ported the generic-call solve
  entry (`solve_constraints` + `infer_constraints_full_inner` +
  `apply_generic_arguments`) behind the `_native_checkexpr_active` gate
  in `mypy/checkexpr.py`. Rust solves from wire `Type` bytes and returns
  the fully-resolved `CallableType`; defers (`None`) on ArgTypeExpander
  star-expansion, Multi-Lower constraint joins, and ParamSpec forms. The
  Python shim feeds `strict_optional` + `infer_unions` from
  `state`/`type_state` and falls back to the pure-Python body on any
  deferral. Exercised by the gate-on/off parity differential of the
  checkexpr suites in `mypy/test/testtypes.py` plus 40 pure unit tests
  in `checkcall.rs`.
- `rust_analyze_descriptor_access` — extends the checkmember
  `analyze_descriptor_access` transform head (checkmember.py:1120-1162).
  Rust short-circuits three pure-type branches on the wire Type: a
  `UnionType` mapped item-wise and joined via make_simplified_union, and
  a non-lvalue `TupleType`/`Instance` whose class/partial-fallback has
  no readable `__get__` (the descriptor passes through unchanged). A
  `__get__`-bearing Instance defers (`None`) so the heavy
  `__get__`-analysis path (checker state, transform_callee_type,
  check_call) stays in Python. The shim gate now covers `UnionType` and
  non-`Instance` descriptor types, passing `mx.is_lvalue`. Exercised by
  the native checkmember suites in `mypy/test/testtypes.py` plus Rust
  unit tests (`test_descriptor_access_*` in `checkmember.rs`).
- `rust_analyze_instance_member_access` method path (checkmember.py:415-453)
  — ported for static and trivial-self methods (issue #631). The trivial-self
  path now maps *subclass* receivers natively too: `map_instance_to_supertype`
  already returns None for a non-base receiver (the identical deferral the
  old exact-class guard produced), so the exact-class guard that deferred
  subclass receivers was removed; `bind_self_fast` is receiver-independent.
  Measured: IAMA 85% → 95% native, global share 97.3% → 97.4%. Overloaded
  signatures, missing resolver snapshots / unresolvable derivation paths,
  empty-args or TVT-class mapped instances, ParamSpec/Unpack signatures, and
  un-frozen TypeVar-carrying results still defer to Python. Exercised by the
  native checkmember suites and Rust unit tests in `checkmember.rs`.
- `expand_type_inner` Callable arm `is_bound` (issue #833) — removed the
  `is_bound` defer in the wire Callable expansion: Python's
  `visit_callable_type` never branches on the flag (it survives
  `copy_modified` unchanged and expansion only touches arg_types/ret_type/
  type_guard/type_is/instance_type), so the Rust defer was over-conservative.
  Wall trace showed `callable-bound` dominated the expand defers; after the
  removal, `rust_expand_and_bind_callable` went 54% → 98% native,
  `rust_expand_type_by_instance` 6% → 67%, `rust_expand_type` 85% → 91%.
  ParamSpec/Unpack walls stay deferred.
- `expand_type_by_instance` recursive arms (subtypes.rs, behind the
  `rust_is_subtype` seam) — mirrors `expandtype.py`'s ExpandTypeVisitor:
  five new arms (CallableType, Overloaded, TupleType, TypeType,
  UnpackType) walk the pure tree where decidable. CallableType defers on
  a declared ParamSpec (Parameters args-splice path) and on a var-arg
  `UnpackType` (interpolation splice); TupleType defers on single-item
  normalization and non-builtins fallback; TypeType defers on a Union
  item. 8 Rust unit tests in `subtypes.rs` (`test_expand_by_instance_*`).
- `analyze_type_inner` Callable star-args (issue-kind: Callable in the
  `rust_type_analyze` wire seam) — mirrors `visit_callable_type`
  (typeanal.py:1873-2027) + `anal_star_arg_type` (:1935). A bound
  `ARG_STAR`/`ARG_STAR2` arg typed `ParamSpecType` (`P.args`/`P.kwargs`)
  passes through unchanged (Python returns the pre-built ParamSpec
  directly), while other star args analyze with `allow_unpack=True`
  matching `anal_star_arg_type`'s fallback `anal_type(t, nested,
  allow_unpack=True)` instead of deferring. Covered by
  `NativeTypeAnalSuite` in `mypy/test/testtypes.py`.
- `rust_is_protocol_implementation` (subtypes.py:1766-1895) — ported
  behind the `_is_subtype` fallback: when `rust_is_subtype` defers on a
  protocol-right Instance pair, the seam drives the full member-compat
  loop natively (member lookup via `get_protocol_member_inner`,
  per-member `is_subtype` with a fresh default context, trivial flag
  gate). Decorator nodes on the protocol (right) side unwrap to `.var`
  and route through `member_method_inner` (bind_self + expand),
  matching `find_node_type`'s callable path for decorated protocol
  members. Defers on: protocol-left (recursion-prone `assuming` guard),
  generic Callable-Callable member pairs (needs type inference),
  non-trivial flag combinations (settable/classvar), MRO misses.
  Measured (self-check): 18623 calls, 13049 decided (70% native).
  Covered by `NativeProtocolImplementationSuite` in
  `mypy/test/testtypes.py`.
- `rust_has_abstract_type` (mypy.checkexpr) — mirrors
  `TypeChecker.has_abstract_type` (checkexpr.py:8134-8143): a pure
  boolean conjunction over live types. The seam reads live Python
  objects via PyO3 (isinstance against `FunctionLike`/`TypeType`/
  `Instance`, `is_type_obj`/`type_object` method calls, `is_abstract`/
  `is_protocol` bool attrs) and short-circuits on `allow_abstract_call`,
  so it never defers and always returns a plain bool. Gated by
  `_native_checkexpr_active` (wired from `mypy/build.py`) and covered by
  `NativeHasAbstractTypeSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential plus direct seam calls).
- `rust_classify_reveal_imported` (mypy.checkexpr) — mirrors the dispatch
  head of `TypeChecker.check_reveal_imported` (checkexpr.py:6483-6497):
  returns `None` when `UNIMPORTED_REVEAL` is not an enabled error code
  (Python early-returns), `Some("reveal_locals")` when
  `kind == REVEAL_LOCALS`, `Some("reveal_type")` when
  `kind == REVEAL_TYPE and not is_imported`, and `None` for the else-arm
  early return. `REVEAL_LOCALS`/`REVEAL_TYPE` are read from
  `mypy.semanal` via PyO3 (same pattern as `rust_visit_reveal_expr`).
  Python applies the `chk.fail` + note side effects with the returned
  name. Gated by `_native_checkexpr_active` (wired from `mypy/build.py`)
  and covered by `NativeRevealImportedSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus direct
  seam calls).
- `rust_classify_super_arg_types` (issue #956) — mirrors the stage-1
  arity + scope gate chain of `ExpressionChecker._super_arg_types`
  (checkexpr.py:7440-7483): `not in_checked_function` -> early
  Any(unannotated), zero-arg with no info -> Any(from_error), zero-arg
  outside a method -> fail + Any, zero-arg OK -> fall-through with
  `fill_typevars`, varargs -> fail, non-positional -> fail, single
  arg -> fail, two-arg -> fall-through with `accept`, too many ->
  fail. Rust reads live `chk`/`super_expr` facts (`in_checked_function`,
  `call.args`, `arg_kinds[].value`, `info.is_none`, `scope.active_class`)
  via PyO3 and returns a branch tag; Python applies the `self.fail` /
  `fill_typevars` / `accept` side effects and stage 2 (proper-type
  dispatch). Defers (`None`) on any unreadable fact. Gated by
  `_native_checkexpr_active` (wired from `mypy/build.py`) and covered
  by `NativeSuperArgTypesSuite` in `mypy/test/testtypes.py` (direct seam
  calls for all 9 tags plus gate-off vs gate-on differential on the 7
  early-exit branches), plus 9 pure decision unit tests in
  `checkexpr_functions.rs`.
- `rust_classify_raw_expression_type` (issue #924) — mirrors the 3-way
  message-selection head of
  `TypeAnalyser.visit_raw_expression_type` (typeanal.py:2135-2150):
  `builtins.int`/`builtins.bool` -> "try using Literal[...]",
  `builtins.float`/`builtins.complex` -> "literals cannot be used as a
  type", else -> "Invalid type comment or annotation". Rust owns only
  the set-membership branch and returns a message tag; the Python shim
  formats the message (needs the live `t` for `literal_value` /
  `simple_name()`) and applies `self.fail` / `self.note` when
  `t.note is not None`. Defers (`None`) when `report_invalid_types` is
  false (the whole head is skipped). Gated by
  `_set_native_typeanal_active` (wired from `mypy/build.py`) and
  covered by `NativeRawExpressionTypeSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), and pure
  decision unit tests in `typeanal_rawexpr.rs`.
- `rust_classify_analyze_callable_type` (issue #958) — mirrors the
  two-level dispatch head of `TypeAnalyser.analyze_callable_type`
  (typeanal.py:2330-2394): arity 0 = bare `Callable[..., Any]`, arity 2
  with `arg0` a `TypeList` = `Callable[[ARG, ...], RET]`, arity 2 with
  `arg0` an `EllipsisType` = `Callable[..., RET]`, arity 2 otherwise =
  the ParamSpec `Callable[P, RET]` form, any other arity = the
  invalid-arity message (branching on `options.disallow_any_generics`).
  Rust owns the whole decision table from four scalar facts
  (`arg_count`, `arg0_is_type_list`, `arg0_is_ellipsis`,
  `disallow_any_generics`) and returns a branch tag; the Python shim
  (`_native_callable_type_tag` + `_apply_callable_type_tag`) builds the
  live `CallableType`, enters `tvar_scope`, and emits `fail`/`note` for
  the tag Rust returns. Every branch is decided; `None` is the
  exception-only deferral. Gated by `_set_native_typeanal_active` (wired
  from `mypy/build.py`) and covered by
  `NativeAnalyzeCallableTypeSuite` in `mypy/test/testtypes.py` (gate-off
  vs gate-on differential plus direct seam calls), and pure decision unit
  tests in `typeanal_callable.rs`.
- `rust_classify_function_signature` (issue #940) — mirrors the count
  arbitration of `SemanticAnalyzer.check_function_signature`
  (semanal.py:2072): compares `len(sig.arg_types)` against
  `len(fdef.arguments)` and returns a branch tag (0 ok / 1 too-few /
  2 too-many). The Python shim applies the side effects (too-few extends
  `sig.arg_types` with dummy `AnyType(TypeOfAny.from_error)` arguments +
  `self.fail`; too-many `self.fail(blocker=True)`) and keeps the
  pure-Python body as the fallback. Always decidable; never defers
  (`None` only on a Python-side exception). Gated by the
  `semanal_visitor` gate (`_SEMANAL_VISITOR_HAS_KERNEL` +
  `_native_semanal_visitor_active`, wired from `mypy/build.py`) and
  covered by `NativeFunctionSignatureSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential on fail records and sig length,
  plus direct seam calls), and 3 pure decision unit tests in
  `semanal_checks.rs`.
- `rust_check_decorated_function_is_method` (issue #941) — mirrors the
  single bool conjunction of
  `SemanticAnalyzer.check_decorated_function_is_method`
  (semanal.py:2256-2258): `not self.type or self.is_func_scope()`. Rust
  reads live analyzer state via PyO3 (`self.type` attribute for the
  None-check, `is_func_scope()` bound method) and returns the negation:
  `Some(true)` = method (no-op), `Some(false)` = non-method context
  (Python emits `self.fail`), `None` = defer on an unreadable attribute
  or method call. The Python shim keeps the pure-Python body as the
  fallback. Gated by `_native_semanal_active` (wired from
  `mypy/build.py`) and covered by
  `NativeDecoratedFunctionIsMethodSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential on the fail list plus direct seam
  calls), and 4 pure decision unit tests in `semanal_checks.rs`.
- `rust_should_wait_rhs` (issue #1008) — mirrors the rvalue-wait predicate
  `SemanticAnalyzer.should_wait_rhs` (semanal.py:4179-4206): Rust reads
  `final_iteration` and the rvalue node-kind isinstance tags
  (NameExpr / MemberExpr / IndexExpr / CallExpr / RefExpr /
  PlaceholderNode) via PyO3 and dispatches with a bounded descent through
  `IndexExpr.base` and `CallExpr.callee` (defers `None` past the bound).
  The ported `get_member_expr_fullname` chain walk and the
  placeholder-not-typeinfo lookup-result classification are pure Rust;
  the symbol lookups ride the real `lookup` / `lookup_qualified` methods
  called via PyO3 (resolver-seam pattern), so error emission and
  module_refs recording stay Python-side. The Python shim keeps the
  pure-Python body as the fallback on `None`. Gated by
  `_native_semanal_active` (wired from `mypy/build.py`) and covered by
  `NativeShouldWaitRhsSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential on results and lookup traffic plus direct seam
  calls), plus pure decision unit tests in `semanal_checks.rs`.
- `rust_are_args_compatible` (issue #954) — mirrors the dispatch head of
  `mypy.subtypes.are_args_compatible` (subtypes.py:2627-2681): the
  name-mismatch gate (`is_different(left.name, right.name, ...)`,
  modulated by `ignore_pos_arg_names` / `right.pos`), the position gate
  (`is_different(left.pos, right.pos, allow_overlap=False)` gated by
  `allow_imprecise_kinds`), the required-arity gate (`not
  allow_partial_overlap and not right.required and left.required`), and
  the partial-overlap shortcut (`allow_partial_overlap and not
  left.required and not right.required`), with the "both required ->
  allow_partial_overlap=False" pre-adjustment applied. Rust reads the
  `left`/`right` `FormalArgument` scalar fields (`name`, `pos`,
  `required`) via PyO3 plus the three bool flag args and returns a tag:
  FALSE (0) / TRUE (1) / CALL_IS_COMPAT (2). The Python shim keeps the
  trailing `is_compat(right.typ, left.typ)` tail (already native via the
  subtype resolver) and the pure-Python body as the fallback. `None`
  defers only on an unreadable attribute or comparison failure. Gated by
  `_native_are_args_compatible_active` (`_native_subtype_active`, wired
  from `mypy/build.py`) and covered by `NativeAreArgsCompatibleSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential on
  (return, is_compat call count) plus direct seam calls proving
  engagement), and 12 pure decision unit tests in `subtypes.rs`.
- `rust_is_descriptor` (issue #968) — mirrors `mypy.subtypes.is_descriptor`
  (subtypes.py:2177-2183), a recursive bool predicate. Rust walks the wire
  `Type`: an `Instance` is a descriptor when its class (via MRO) has a
  `__get__` member (reusing `has_readable_member_by_ref` from checkmember);
  a `UnionType` is a descriptor when all relevant items are descriptors
  (`NoneType` items filtered when `strict_optional` is off, matching
  `UnionType.relevant_items`). All other types return `Some(false)`. Defers
  (`None`) on `TypeAliasType` (no alias target on the wire) and on missing
  resolver snapshots for any MRO class consulted. Gated by
  `_native_subtype_active` + `_native_subtype_resolver` (wired from
  `mypy/build.py`) and covered by `NativeIsDescriptorSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus direct
  seam calls for Instance/Union/None/Any/Callable paths).
- `rust_classify_fixed_args` (issue #935) — mirrors the two gap checks of
  `SemanticAnalyzer.check_fixed_args` (semanal.py:6962-6976):
  `len(expr.args) != numargs` (wrong count) and
  `expr.arg_kinds != [ARG_POS]*numargs` (wrong kinds). Rust classifies
  the two gaps into a 3-way tag (OK / wrong-count / wrong-kinds) from
  `args_len`, the integer `arg_kinds` list, and `numargs`; the Python
  shim applies the `self.fail` message per the tag and keeps the
  pure-Python body as the fallback. Never defers (`Some(tag)` always).
  Gated by `_native_semanal_visitor_active` (wired from `mypy/build.py`)
  and covered by `NativeFixedArgsSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), and pure
  decision unit tests in `semanal_checks.rs`.
- `rust_classify_method_signature` (issue #1036) — mirrors the dispatch
  head of `SemanticAnalyzer.prepare_method_signature`
  (semanal.py:1543-1582): the `__new__` is_static write (NEW_STATIC),
  the `__init_subclass__`/`__class_getitem__` is_class write
  (CLASS_SPECIAL), the Any-self trivial/replace arms, the
  redundant-Self / explicit-self-conflict fails, the
  static-method-with-Self fail, and the OK tail. Rust returns
  `(set_is_static, set_is_class, tag)` from live FuncDef facts read via
  PyO3 (name, has_self_or_cls_argument, arguments non-empty,
  functype-is-CallableType) plus the analyzed arg_types[0] proper type
  serialized once to wire (the AnyType check), the unanalyzed-arg kind,
  and the shim-precomputed `is_expected_self_type` bool (needs
  lookup_qualified; the rust_class_callable pattern). The `func.is_class`
  read at 1561 is decidable without the write because the shim applies
  the is_class write before its tag handler re-reads it. Python applies
  the is_static/is_class/is_trivial_self writes, the
  replace_implicit_first_type + func.type assignment, and the three
  self.fail emissions. Defers (`None`) on an unreadable fact, an
  undecodable self-type blob, or an uncomputable
  `is_expected_self_type`. Gated by `_native_semanal_visitor_active`
  (wired from `mypy/build.py`) and covered by
  `NativePrepareMethodSignatureSuite` in `mypy/test/testtypes.py`
  (direct seam tag tests, gate-off vs gate-on differential, deferral
  audit), plus pure decision unit tests in `semanal_checks.rs`.
- `rust_classify_visit_op_expr` (issue #959) — mirrors the 5-way dispatch
  head of `ExpressionChecker.visit_op_expr` (checkexpr.py:5014-5044):
  `e.analyzed` passthrough (tag 0), `and`/`or` boolean op (tag 1),
  `*` with `ListExpr` list multiply (tag 2), `%` with `BytesExpr`/
  `StrExpr` str interpolation (tag 3), else `check_op` (tag 4). Rust reads
  `e.analyzed` (truthiness), `e.op` (string), and `e.left` isinstance tags
  via PyO3; the Python shim delegates each tag to the original branch
  body (`accept`/`check_boolean_op`/`check_list_multiply`/
  `check_str_interpolation`/check-op fall-through). `None` defers only on
  an unreadable attribute or isinstance error. Gated by
  `_native_checkexpr_active` (wired from `mypy/build.py`) and covered by
  `NativeVisitOpExprSuite` in `mypy/test/testtypes.py` (direct seam calls
  for all 5 branches plus edge cases), and 11 pure decision unit tests in
  `checkexpr_functions.rs`.
- `rust_classify_check_boolean_op` (issue #1049) — mirrors the
  map-arrangement + result-tail decision head of
  `ExpressionChecker.check_boolean_op` (checkexpr.py:6062-6145): the
  4-way map-tag dispatch (`right_always` / `right_unreachable` /
  `and` / `or`), the two reachability gates (left/right map values
  scanned for `UninhabitedType`), and the result arbitration
  (return left / return right / `UninhabitedType` restricted type /
  union). Rust classifies from the wire map values, one wire
  serialization of the expanded-left operand, its live
  `can_be_true`/`can_be_false` flags, and `strict_optional`; the tail
  reuses the `false_only`/`true_only` truthiness kernels for the
  restricted type. Python keeps `find_isinstance_check`,
  `analyze_cond_branch`, the two `self.msg.*_operand` emissions, and
  `make_simplified_union`. Defers (`None`) on `TypeAliasType` map
  values, a `Union` expanded-left (the union recursion needs live
  per-item flags), and `true_only` dunder lookups the resolver
  snapshot cannot decide (e.g. an int Instance under `or`). Gated by
  `_native_checkexpr_active` (wired from `mypy/build.py`) and covered
  by `NativeCheckBooleanOpSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), and 13
  pure decision unit tests in `checkexpr_functions.rs`.
- `rust_classify_type_type_member_access` (issue #957) — mirrors the
  9-way dispatch head of
  `mypy.checkmember.analyze_type_type_member_access`
  (checkmember.py:965-1018) plus a nested 4-way sub-dispatch on
  `get_proper_type(typ.item.upper_bound)` for the TypeVarType arm.
  Rust reads the live `TypeType` via PyO3 (isinstance tags against
  `mypy.types` classes: Instance / AnyType / TypeVarType / TupleType /
  FunctionLike / TypeType, plus `is_type_obj()` bool and
  `isinstance(typ.item.item, Instance)` for the TypeType arm) and
  returns a branch tag (0-12); the Python shim applies the terminal
  branches (`_analyze_member_access`, `filter_errors`,
  `tuple_fallback`, `TypeType.make_normalized`, `metaclass_type`).
  Tags NONE / TV_UB_OTHER / FUNC_NOT_TYPEOBJ / TYPE_TYPE_OTHER leave
  `item` as None and fall through to the shared tail; `None` is the
  exception-only deferral (unreadable PyO3 facts). Gated by
  `_native_checkmember_active` (wired from `mypy/build.py`) and covered
  by `NativeTypeTypeMemberAccessSuite` in `mypy/test/testtypes.py`
  (direct seam tag tests for all 13 branches plus gate-off vs gate-on
  differential on the result / call-log through a mock
  MemberContext), and 13 pure decision unit tests in `checkmember.rs`.
- `rust_classify_match_args` (issue #970) — mirrors the predicate head of
  `TypeChecker.check_match_args` (checker.py:3128-3141): `not
  self.scope.active_class()` -> skip (tag 0); `get_proper_type(typ)` not a
  `TupleType` or any non-string-literal item -> fail (tag 2, emit the
  `LITERAL_REQ` note); all items string literals -> ok (tag 1). Rust
  decodes the wire `typ`, resolves the proper type (defers on an
  unresolved `TypeAliasType`), checks the `TupleType` kind, and reuses
  `is_string_literal_inner` per item. Defers (`None`) on decode failure
  or an item the string-literal kernel cannot decide; the Python shim
  emits the note and keeps the pure-Python body as the fallback. Gated
  by `_native_checker_active` (wired from `mypy/build.py`) and covered by
  `NativeMatchArgsSuite` in `mypy/test/testtypes.py` (gate-off vs gate-on
  differential plus direct seam calls), and 5 pure decision unit tests in
  `checker_functions.rs`.
- `rust_is_valid_constructor` (issue #967) — mirrors
  `mypy.typeops.is_valid_constructor` (typeops.py:445-455): a pure bool
  predicate, True for `OverloadedFuncDef`/`FuncDef`
  (`SYMBOL_FUNCBASE_TYPES`) or for a `Decorator` whose
  `get_proper_type(var.type)` is a `FunctionLike`. Rust reads the live
  node via PyO3 isinstance (mirrors `rust_is_magic_base`); the Decorator
  arm calls `mypy.types.get_proper_type(n.type)` then serializes the
  proper type to the wire format and checks the tag is `CallableType` or
  `Overloaded` (the wire form of `FunctionLike`), with a PyO3
  `isinstance(..., FunctionLike)` fallback if serialization unexpectedly
  fails. A `None` type (unanalyzed decorator) yields `False`. Always
  returns a bool, never defers: no resolver / inference / checker
  callbacks. Gated by `_native_typeops_active` (wired from
  `mypy/build.py`) and covered by `NativeIsValidConstructorSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus direct
  seam calls across all branches: FuncDef, OverloadedFuncDef, Decorator
  with CallableType/Overloaded/Instance/None type, Var, None node).
- `rust_classify_type_object_type` (issue #1059) — mirrors the
  init-vs-new arbitration head of `mypy.typeops.type_object_type`
  (typeops.py:495-546): Rust walks the live `TypeInfo`'s MRO via PyO3
  (same `is_valid_constructor_inner` classification as #967), picks the
  first MRO entry defining `__init__` or `__new__`, resolves the
  init-new tie in favor of the entry defining both or, when both come
  from `object` with a bogus base, the TIE_ANY universal-callable arm,
  and reads `special_sig` (tuple subclass), `is_new`, and the
  method-is-uncached bit off the winner. Returns
  `(tag, is_new, special_sig, uncached, method)`; the Python shim
  (`_type_object_type_rust_head`) applies all side effects: the
  invalid-class-definition Any, metaclass/`builtins.type` fallback
  construction, the universal-callable tie arm, the already-native
  `type_object_type_from_function` tail, the `special_sig="tuple"`
  fixup, and the `strict_optional`-gated cache write. Defers (`None`)
  when `type_object_type_from_function`'s pure decision is not
  reachable (unreadable MRO/method facts). Gated by
  `_native_typeops_active` (wired from `mypy/build.py`) and covered by
  `NativeTypeObjectArbitrationSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls for all 5
  tags), plus 10 pure decision unit tests in `typeops.rs`.
- `rust_is_instance_var` (issue #965) — mirrors the pure bool predicate
  `is_instance_var` (checkmember.py:1502-1511): the PEP 526
  instance-variable conjunction `var.name in var.info.names and
  var.info.names[var.name].node is var and not var.is_classvar and
  not var.is_inferred`. Rust reads `var.name`/`var.info.names`/
  `var.is_classvar`/`var.is_inferred` via PyO3 and short-circuits each
  clause in order, returning a plain bool; defers (`None`) only when an
  attribute is unreadable (e.g. `info` is the `VAR_NO_INFO` FakeInfo
  placeholder, whose `__getattribute__` raises `AssertionError`), so the
  Python caller falls back to the pure-Python predicate. Gated by
  `_native_checkmember_active` (wired from `mypy/build.py`) and covered
  by `NativeIsInstanceVarSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential plus direct seam calls), plus a Rust unit test
  proving the deferral path in `checkmember.rs`.
- `rust_classify_analyze_var` (issue #1056) — mirrors the decision head of
  `mypy.checkmember.analyze_var` (checkmember.py:1771-1824 plus the
  enum-literal tail arm at 1835-1838), reduced to a single outcome tag:
  SETTER (settable property read as lvalue) / GETTER / PARTIAL /
  NOT_READY / ENUM_LITERAL / UNBOUND_ANY. Rust reads the live Var
  scalars via PyO3 (is_settable_property, setter_type/type None-ness +
  PartialType kind, is_ready, is_initialized_in_class, is_instance_var,
  info.fullname, info.is_enum, info.enum_members via `__contains__`),
  decodes the wire receiver instance, and gates on the resolver handling
  the receiver's `map_instance_to_supertype` (snapshot miss → defer, so
  Python's total mapping handles the access). Python applies the tagged
  branch's side effects in `_apply_analyze_var_tag` (handle_partial_var_type,
  the not-ready callback, the msg gates, expand/bind tail, enum-literal
  wrap); a None tag (undecodable wire, unreadable attr, FakeInfo,
  snapshot miss) falls back to the pure-Python body. PARTIAL and
  NOT_READY beat ENUM_LITERAL (the partial return and the callback are
  head-body side effects); ENUM_LITERAL engages only when the head body
  is side-effect free under a non-lvalue access, so `name`/`value` and
  the method-alias bind tail stay GETTER. Gated by
  `_native_checkmember_active` (wired from `mypy/build.py`) and covered
  by `NativeAnalyzeVarSuite` in `mypy/test/testtypes.py` (direct seam
  tag tests per branch plus gate-off vs gate-on differentials through
  real `analyze_var`), plus pure decision unit tests in
  `classify_analyze_var_tests` in `checkmember.rs`.
- `rust_is_disjoint_base` (issue #969) — mirrors the pure bool predicate
  `_is_disjoint_base` (typeops.py:2110-2124): returns `True` when
  `info.is_disjoint_base` is set, or when `info.slots` is non-empty and at
  least one slot is "own" (not declared by any direct base's `slots`).
  Rust reads `info.is_disjoint_base`, `info.slots`, and
  `info.bases[*].type.slots` via PyO3 and computes the own-vs-base slot
  set difference, mirroring `rust_is_magic_base` (live-object, no wire
  decode). Never defers: every well-formed `TypeInfo` yields a plain
  bool. The shared `is_disjoint_base_inner` in `typeops.rs` replaces the
  duplicate in `checker_visitor.rs`, so `rust_can_have_shared_disjoint_base`
  uses the same code path. Gated by `_native_typeops_active` (wired from
  `mypy/build.py`) and covered by `NativeIsDisjointBaseSuite` in
  `mypy/test/testtypes.py` (direct seam calls, gate-off vs gate-on
  differential across decorator, no-slots, empty-slots, own-slots,
  all-inherited, mixed, base-slots-None, and multiple-bases cases).
- `rust_is_recursive_pair` (issue #966) — mirrors
  `mypy.typeops.is_recursive_pair` (typeops.py:249-274), the pure bool
  predicate gating `join_types` / `meet_types` / `is_subtype` against
  infinite recursion. Rust classifies two wire Type bytes plus the live
  `is_recursive` flags (the wire `TypeAliasType` has no `is_recursive`
  field; it needs the live `TypeAlias` node). The alias-chain expansion
  (`get_proper_type`) runs through the snapshot alias resolver
  (`expand_alias_shape`); a missing snapshot or an alias cycle defers
  (`None`) and the Python caller falls back. `or`-chain short-circuit is
  preserved by checking the resolver-free branch (`t_rec`/`s_rec`) first;
  a later resolver-dependent branch defers only when no earlier branch
  already returned `True`. Gated by `_native_typeops_active` (wired from
  `mypy/build.py`) and covered by `NativeIsRecursivePairSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus direct
  seam calls), plus 6 pure decision unit tests in `typeops.rs`.
- `rust_is_valid_var_arg` / `rust_is_valid_keyword_var_arg` (issue #981) —
  mirrors the two bool predicates `ExpressionChecker.is_valid_var_arg` /
  `is_valid_keyword_var_arg` (checkexpr.py:8010-8042), called on every call
  with star args (check_var_args_kwargs, visit_comparison_expr). Rust
  reads the proper type's wire bytes and decides the isinstance
  disjunctions (Tuple/Any/ParamSpec/Unpack tags; `builtins.dict` fullname
  for the kwargs dict arm). The four `is_subtype` acceptance calls
  (Iterable[Any], dict args[0] vs str, SKAG[str, Any], SKAG[Never, Never])
  are resolver-backed and already native; the shims pass their results in
  as booleans (`rust_class_callable` pattern). Python's or-chain
  short-circuit is value-preserving under eager boolean evaluation because
  the booleans are pure. Defers (`None`) on undecodable wire bytes, a
  `TypeAliasType` (no resolved alias target on the wire), and a dict
  Instance with no args (Python indexes `typ.args[0]`; defer preserves the
  fallback behavior). Python keeps the `invalid_var_arg` /
  `invalid_keyword_var_arg` error emission at the call sites. Gated by the
  existing `_native_checkexpr_active` (wired from `mypy/build.py`) and
  covered by `NativeValidVarArgSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), plus pure
  decision unit tests in `checkexpr_functions.rs`.

- `rust_refers_to_typeddict` (issue #980, mypy.checkexpr) — mirrors the
  pure bool predicate `ExpressionChecker.refers_to_typeddict`
  (checkexpr.py:1385-1393), which runs for every call expression. Rust
  reads the live callee via PyO3 `is_instance` (mirroring
  `rust_classify_lvalue_validity`): `RefExpr` gate, then `node` as
  `TypeInfo` with `typeddict_type is not None` (direct reference), then
  `node` as `TypeAlias` whose target proper-type — serialized to wire
  bytes by the Python shim — decodes to `Type::TypedDictType`. Returns
  a plain bool; the only raise is a TypeAlias node without decodable
  target bytes (unreachable through the shim), which the Python shim
  treats as a fallback to the pure-Python body. Python keeps the
  consumer branch (`accept` + `check_typeddict_call`) unchanged.
  Gated by `_native_checkexpr_active` (existing wiring, no build.py
  change) and covered by `NativeRefersToTypedDictSuite` in
  `mypy/test/testtypes.py` (direct seam calls plus gate-off vs gate-on
  differential over all branches), plus wire round-trip unit tests in
  `checkexpr_functions.rs`.
- `rust_classify_tuple_type_implicit` (issue #983) — mirrors the
  implicit-tuple message-arbitration head of
  `TypeAnalyser.visit_tuple_type` (typeanal.py:2038-2058): Rust reads
  three scalars (`t.implicit`, `allow_tuple_literal`, `len(t.items)`)
  and returns a tag OK (0, normal named_type + anal_array
  reconstruction), EMPTY (1, `Tuple[()]` suggestion), SINGLE (2,
  spurious-trailing-comma suggestion), or MULTI (3, `Tuple[T1, ..., Tn]`
  suggestion). The Python shim applies the
  "Syntax error in type annotation" fail + one-of-three note and, on OK,
  the reconstruction; the pure-Python arbitration is the fallback when
  the gate is off. Never defers: all three facts are scalars, so every
  triple maps to exactly one tag. Lives in `typeanal_special.rs`,
  gated by `_set_native_typeanal_active` (wired from `mypy/build.py`)
  and covered by `NativeTupleTypeImplicitSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), plus pure
  decision unit tests in `typeanal_special.rs`.
- `rust_check_match_args` (issue #986, rework of the #970 tag-classifier
  seam into the `rust_is_final_enum_value` pure-bool shape) — mirrors the
  type predicate of `TypeChecker.check_match_args`
  (checker.py:3128-3141): Rust reads one wire `typ`, resolves the proper
  type (defers on an unresolved `TypeAliasType`), and returns
  `isinstance(TupleType) and all(is_string_literal(item))` as a bool,
  reusing `is_string_literal_inner` per item. The
  `scope.active_class()` gate and the `LITERAL_REQ` note emission stay in
  Python; the shim returns early on a decided bool and falls through to
  the pure-Python body on `None`. Defers (`None`) on decode failure or an
  item the string-literal kernel cannot decide (Python's
  `try_getting_str_literals_from_type` fallback may still answer). Gated
  differential plus direct seam and alias-deferral calls), and 6 pure
  unit tests in `checker_functions.rs`.
- `rust_classify_check_final` (issue #1011) — mirrors the decision head of
  `TypeChecker.check_final` (checker.py:5095-5196): after the shim computes
  `flatten_lvalues` and `is_final_decl`, everything left is a pure sequence
  of message decisions. Rust reads the live lvalues via PyO3 (RefExpr ->
  Var isinstance gate), the `final_without_value` scalar facts
  (`final_unset_in_class`, `final_set_in_init`, `is_stub`, `s.type is not
  None`, `active_class.is_named_tuple`), and the per-lvalue arbitration:
  the MRO walk over `cls.mro[1:]` looking up `base.names[name]` for a
  final base Var (emit-once + break) and the own `lv.node.is_final` check
  (both messages can fire for one lvalue). Rust returns
  `(without_value, [(name, info_is_none), ...])`; the Python shim applies
  the `final_without_value` / `cant_assign_to_final` emissions and keeps
  the pure-Python body as the fallback. Defers (`None`) on any unreadable
  fact and when the `is_final_decl` pre-check would hit a Python `assert`
  (non-RefExpr first lvalue / non-Var node) so the original body re-runs
  and surfaces the same error. The fast no-final path exits after one
  lookup. Gated by `_native_checker_active` (wired from `mypy/build.py`)
  and covered by `NativeCheckFinalSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), plus pure
  decision unit tests in `checker_functions.rs`.
- `rust_classify_class_pattern_ranges` (issue #987) — mirrors the dispatch
  of `PatternChecker.get_class_pattern_type_ranges`
  (checkpattern.py:794-832): Rust decodes the wire `typ` and recurses over
  `UnionType` items Rust-side, returning one branch tag per leaf in union
  pre-order (FAIL / TYPE_OBJ / CALLABLE_VAR / TYPE_TYPE / ANY). The three
  class-ref scalars (`isinstance(o.class_ref.node, Var)`, `node.type is
  not None`, `node.fullname == "typing.Callable"`) are read via PyO3.
  Python keeps all TypeRange construction from live nodes
  (`fill_typevars_with_any` / `callable_with_ellipsis` / `named_type`) and
  the `self.msg.fail` with `typ.str_with_options`. Defers (`None`) on any
  `TypeAliasType` in the union (Python's `get_proper_type` would expand it
  from live symbols), an undecodable wire blob, an unreadable class-ref
  attribute, and any `CallableType`/`Overloaded` whose fallback is not
  provably `builtins.type` (`is_type_obj` needs the live
  `fallback.type.is_metaclass()`); an alias ret_type also defers. An
  `UninhabitedType` ret_type decides `is_type_obj == False` so the scalar
  class-ref arm still engages. Gated by `_native_checkpattern_active`
  (already wired from `mypy/build.py`) and covered by
  `NativeClassPatternRangesSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential plus direct seam calls), plus pure decision unit
  tests in `checkpattern.rs`.
- `rust_classify_simple_literal_type` (issue #984) — mirrors the 5-way
  dispatch head of `SemanticAnalyzer.analyze_simple_literal_type`
  (semanal.py:4720-4749): function_stack truthiness (skip inside a
  function) and the folded constant kind (None / complex / bool / int /
  str / float) decide the type-name tag (builtins.bool/int/str/float or
  None). The Python shim folds the rvalue via the already-native
  `constant_fold_expr`, applies `named_type_or_none(type_name)`, and
  when `is_final` wraps the result via
  `copy_modified(last_known_value=LiteralType(...))`. `cur_mod_id` and
  `is_final` are carried for signature fidelity but do not affect the
  decision. Never defers in production: the shim only produces the six
  known value kinds; an unknown kind (direct seam calls only) defers
  (`None`) to the pure-Python body. Gated by
  `_native_semanal_visitor_active` and covered by
  `NativeSimpleLiteralTypeSuite` in `mypy/test/testtypes.py` (direct
  seam tag tests + gate-off vs gate-on differential over int/str/float/
  bool/complex/fold-failure/final-var-ref/inside-function), plus pure
  decision unit tests in `semanal_visitor.rs`.

- `rust_classify_class_decorator` (issue #897, Phase E1 slice of #624) —
  mirrors the name-set dispatch of
  `SemanticAnalyzer.analyze_class_decorator_common` (semanal.py:2889):
  Rust checks the decorator against the final / disjoint_base /
  type_check_only name sets (via `refers_to_fullname`, short-circuit in
  branch order) and, when none matched, extracts the
  `@warnings.deprecated("msg")` message from the `CallExpr`'s first
  `StrExpr` arg. Returns a `(tag, deprecated_msg)` pair; `None` defers on
  a name-set arity mismatch. The Python shim applies the flag writes
  (`is_final` / `is_disjoint_base` / `is_type_check_only` / `deprecated`)
  and the two `@disjoint_base` `fail`s (protocol / TypedDict). Gated by
  `_native_semanal_visitor_active` and covered by
  `NativeClassDecoratorCommonSuite` in `mypy/test/testtypes.py`
  (direct seam tag tests + gate-off vs gate-on method differential +
  deferral audit), plus pure decision unit tests in `semanal_visitor.rs`.

- `rust_refers_to_typeddict` (issue #980, mypy.checkexpr) — mirrors the
  pure bool predicate `ExpressionChecker.refers_to_typeddict`
  (checkexpr.py:1385-1393), which runs for every call expression. Rust
  reads the live callee via PyO3 `is_instance` (mirroring
  `rust_classify_lvalue_validity`): `RefExpr` gate, then `node` as
  `TypeInfo` with `typeddict_type is not None` (direct reference), then
  `node` as `TypeAlias` whose target proper-type — serialized to wire
  bytes by the Python shim — decodes to `Type::TypedDictType`. Returns
  a plain bool; the only raise is a TypeAlias node without decodable
  target bytes (unreachable through the shim), which the Python shim
  treats as a fallback to the pure-Python body. Python keeps the
  consumer branch (`accept` + `check_typeddict_call`) unchanged.
  Gated by `_native_checkexpr_active` (existing wiring, no build.py
  change) and covered by `NativeRefersToTypedDictSuite` in
  `mypy/test/testtypes.py` (direct seam calls plus gate-off vs gate-on
  differential over all branches), plus wire round-trip unit tests in
  `checkexpr_functions.rs`.
- `rust_classify_tuple_type_implicit` (issue #983) — mirrors the
  implicit-tuple message-arbitration head of
  `TypeAnalyser.visit_tuple_type` (typeanal.py:2038-2058): Rust reads
  three scalars (`t.implicit`, `allow_tuple_literal`, `len(t.items)`)
  and returns a tag OK (0, normal named_type + anal_array
  reconstruction), EMPTY (1, `Tuple[()]` suggestion), SINGLE (2,
  spurious-trailing-comma suggestion), or MULTI (3, `Tuple[T1, ..., Tn]`
  suggestion). The Python shim applies the
  "Syntax error in type annotation" fail + one-of-three note and, on OK,
  the reconstruction; the pure-Python arbitration is the fallback when
  the gate is off. Never defers: all three facts are scalars, so every
  triple maps to exactly one tag. Lives in `typeanal_special.rs`,
  gated by `_set_native_typeanal_active` (wired from `mypy/build.py`)
  and covered by `NativeTupleTypeImplicitSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), plus pure
  decision unit tests in `typeanal_special.rs`.
- `rust_check_match_args` (issue #986, rework of the #970 tag-classifier
  seam into the `rust_is_final_enum_value` pure-bool shape) — mirrors the
  type predicate of `TypeChecker.check_match_args`
  (checker.py:3128-3141): Rust reads one wire `typ`, resolves the proper
  type (defers on an unresolved `TypeAliasType`), and returns
  `isinstance(TupleType) and all(is_string_literal(item))` as a bool,
  reusing `is_string_literal_inner` per item. The
  `scope.active_class()` gate and the `LITERAL_REQ` note emission stay in
  Python; the shim returns early on a decided bool and falls through to
  the pure-Python body on `None`. Defers (`None`) on decode failure or an
  item the string-literal kernel cannot decide (Python's
  `try_getting_str_literals_from_type` fallback may still answer). Gated
  differential plus direct seam and alias-deferral calls), and 6 pure
  unit tests in `checker_functions.rs`.
- `rust_classify_check_final` (issue #1011) — mirrors the decision head of
  `TypeChecker.check_final` (checker.py:5095-5196): after the shim computes
  `flatten_lvalues` and `is_final_decl`, everything left is a pure sequence
  of message decisions. Rust reads the live lvalues via PyO3 (RefExpr ->
  Var isinstance gate), the `final_without_value` scalar facts
  (`final_unset_in_class`, `final_set_in_init`, `is_stub`, `s.type is not
  None`, `active_class.is_named_tuple`), and the per-lvalue arbitration:
  the MRO walk over `cls.mro[1:]` looking up `base.names[name]` for a
  final base Var (emit-once + break) and the own `lv.node.is_final` check
  (both messages can fire for one lvalue). Rust returns
  `(without_value, [(name, info_is_none), ...])`; the Python shim applies
  the `final_without_value` / `cant_assign_to_final` emissions and keeps
  the pure-Python body as the fallback. Defers (`None`) on any unreadable
  fact and when the `is_final_decl` pre-check would hit a Python `assert`
  (non-RefExpr first lvalue / non-Var node) so the original body re-runs
  and surfaces the same error. The fast no-final path exits after one
  lookup. Gated by `_native_checker_active` (wired from `mypy/build.py`)
  and covered by `NativeCheckFinalSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), plus pure
  decision unit tests in `checker_functions.rs`.
- `rust_classify_class_pattern_ranges` (issue #987) — mirrors the dispatch
  of `PatternChecker.get_class_pattern_type_ranges`
  (checkpattern.py:794-832): Rust decodes the wire `typ` and recurses over
  `UnionType` items Rust-side, returning one branch tag per leaf in union
  pre-order (FAIL / TYPE_OBJ / CALLABLE_VAR / TYPE_TYPE / ANY). The three
  class-ref scalars (`isinstance(o.class_ref.node, Var)`, `node.type is
  not None`, `node.fullname == "typing.Callable"`) are read via PyO3.
  Python keeps all TypeRange construction from live nodes
  (`fill_typevars_with_any` / `callable_with_ellipsis` / `named_type`) and
  the `self.msg.fail` with `typ.str_with_options`. Defers (`None`) on any
  `TypeAliasType` in the union (Python's `get_proper_type` would expand it
  from live symbols), an undecodable wire blob, an unreadable class-ref
  attribute, and any `CallableType`/`Overloaded` whose fallback is not
  provably `builtins.type` (`is_type_obj` needs the live
  `fallback.type.is_metaclass()`); an alias ret_type also defers. An
  `UninhabitedType` ret_type decides `is_type_obj == False` so the scalar
  class-ref arm still engages. Gated by `_native_checkpattern_active`
  (already wired from `mypy/build.py`) and covered by
  `NativeClassPatternRangesSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential plus direct seam calls), plus pure decision unit
  tests in `checkpattern.rs`.
- `rust_classify_simple_literal_type` (issue #984) — mirrors the 5-way
  dispatch head of `SemanticAnalyzer.analyze_simple_literal_type`
  (semanal.py:4720-4749): function_stack truthiness (skip inside a
  function) and the folded constant kind (None / complex / bool / int /
  str / float) decide the type-name tag (builtins.bool/int/str/float or
  None). The Python shim folds the rvalue via the already-native
  `constant_fold_expr`, applies `named_type_or_none(type_name)`, and
  when `is_final` wraps the result via
  `copy_modified(last_known_value=LiteralType(...))`. `cur_mod_id` and
  `is_final` are carried for signature fidelity but do not affect the
  decision. Never defers in production: the shim only produces the six
  known value kinds; an unknown kind (direct seam calls only) defers
  (`None`) to the pure-Python body. Gated by
  `_native_semanal_visitor_active` and covered by
  `NativeSimpleLiteralTypeSuite` in `mypy/test/testtypes.py` (direct
  seam tag tests + gate-off vs gate-on differential over int/str/float/
  bool/complex/fold-failure/final-var-ref/inside-function), plus pure
  decision unit tests in `semanal_visitor.rs`.
- `rust_get_arg_infer_passes` (issue #1000, `checkcall.rs`) — mirrors
  `ExpressionChecker.get_arg_infer_passes` (checkexpr.py:3563-3633)
  wholesale: a pure two-pass argument-inference classifier with zero
  side effects. For each formal Rust decides pass 1 vs pass 2: the
  ParamSpec arm (a `CallableType.param_spec()`-shaped formal whose
  actuals include a non-generic non-lambda CallableType suppresses the
  second pass, with Instance actuals resolved via
  `find_member_call_is_plain_callable`, a restricted
  `find_member("__call__", ..., is_operator=True)` fed through the
  existing resolver seam and `member_method_inner`), plus the
  `ArgInferSecondPassQuery` fold (`BoolTypeQuery(ANY_STRATEGY)` with the
  `visit_callable_type` override and an exact `HasTypeVars` mirror that,
  unlike the visitor kernel, never walks callable `variables` or
  Instance `last_known_value`). Python keeps the result application;
  the function is pure so nothing else stays Python-side. Defers
  (`None`) on undecodable blobs, alias-expansion failures (missing
  snapshot / cycle), out-of-range indices, extra_attrs, non-plain
  `__call__` members (property/Decorator/Var), and any fact the kernel
  cannot read. Hot path: once per generic-call inference from
  `infer_function_type_arguments`. Gated by `_native_checkexpr_active`
  (existing wiring, no build.py change) and covered by
  `NativeArgInferPassesSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential plus direct seam calls), and 14 pure decision
  unit tests in `checkcall.rs`.

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

The type-kernel gate is default-on in production (`native_type_kernel =
True`). The `TEST_NATIVE_TYPE_KERNEL=1` env var is the parity
differential: the test harnesses override the option *after* option
parsing (`mypy/test/helpers.py`), so the kernel runs in direct
comparison with the pure-Python visitors and parity is verified both
ways. Without the env var the default-on option governs. The build
manager propagates `Options.native_type_kernel` to module-level flags in
each kernel module at the start of each build
(`_set_native_erase_active` etc. in `mypy/build.py`), so the hot paths
avoid an options lookup per call.

## Pull Requests

The default branch on this fork is `main` (not `master`). Always target
`main` as the PR base. Branch from `main` before committing — do not commit
directly to `main`.
  all-inherited, mixed, base-slots-None, and multiple-bases cases).
- `rust_is_recursive_pair` (issue #966) — mirrors
  `mypy.typeops.is_recursive_pair` (typeops.py:249-274), the pure bool
  predicate gating `join_types` / `meet_types` / `is_subtype` against
  infinite recursion. Rust classifies two wire Type bytes plus the live
  `is_recursive` flags (the wire `TypeAliasType` has no `is_recursive`
  field; it needs the live `TypeAlias` node). The alias-chain expansion
  (`get_proper_type`) runs through the snapshot alias resolver
  (`expand_alias_shape`); a missing snapshot or an alias cycle defers
  (`None`) and the Python caller falls back. `or`-chain short-circuit is
  preserved by checking the resolver-free branch (`t_rec`/`s_rec`) first;
  a later resolver-dependent branch defers only when no earlier branch
  already returned `True`. Gated by `_native_typeops_active` (wired from
  `mypy/build.py`) and covered by `NativeIsRecursivePairSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus direct
  seam calls), plus 6 pure decision unit tests in `typeops.rs`.
- `rust_make_inferred_type_note` (issue #982) — mirrors the pure bool
  decision of `Messages.make_inferred_type_note` (messages.py:3770-3800):
  Rust decodes the serialized subtype/supertype pair and runs the
  `inferred_note_wire_decision` check plus the `inferred_note_context_fires`
  context classifier (ReturnStmt + NameExpr), returning True when the
  inferred-return-annotation note fires; the Python shim formats the
  "Perhaps you need a type annotation" message. Defers (`Ok(false)`) on
  undecodable wire bytes or a non-firing context. Gated by
  `_native_messages_active` (wired from `mypy/build.py`) and covered by
  `NativeInferredTypeNoteSuite` in `mypy/test/testtypes.py`.
- `rust_classify_has_no_attr` (issue #1006) — mirrors the dispatch of
  `Messages.has_no_attr` (messages.py:364-601): the 11-arm special-case
  front (not-assignable member, `in`, binary-op methods via `op_methods`,
  unary ops, getitem/setitem/call with the type-obj and
  `builtins.function` special cases) plus the non-special tail. The tail
  hangs off `are_type_names_disabled()`: with type names enabled
  everything lands in the Instance suggestion sub-block (module-private
  export, did-you-mean via `COMMON_MISTAKES` + `best_matches`, or plain
  ATTR_DEFINED); the union-item / typevar-upper-bound / silent tags only
  fire when names are disabled. Rust reads 14 scalar facts (isinstance
  tags, name lists, the module symbol table's public/private split) and
  returns a 17-tag arbitration plus the op id and did-you-mean matches
  (via the difflib `best_matches` port); Python applies all fail/note
  side effects and every format call (format_type, format_type_distinctly,
  pretty_seq). Never defers: the scalar facts cover every reachable
  branch. Gated by `_native_messages_active` (wired from `mypy/build.py`)
  and covered by `NativeHasNoAttrSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), plus pure
  decision unit tests in `messages.rs`.
- `rust_classify_truthy_type` (issue #1010, mypy.checker) — mirrors the
  strict-optional truthiness arbitration of
  `TypeChecker.check_for_truthy_type` (checker.py:7898-7956) and its
  `_is_truthy_type` helper (checker.py:7882-7896). Rust walks the live
  proper type via PyO3 (isinstance against `Instance`/`FunctionLike`/
  `UnionType`, `bool(t.type)`, `has_readable_member("__bool__"/"__len__")`,
  `type.fullname`, and per-item `get_proper_type` for union items) and
  returns a branch tag (SKIP / FUNCTION / UNION / ITERABLE / OTHER);
  Python keeps the `state.strict_optional` gate, all `format_type`
  message formatting, `make_fake_typeinfo`, and the `self.fail` emission,
  with the pure-Python `_is_truthy_type` body as the fallback. Defers
  (`None`) only on an unreadable fact (the fallback then raises
  identically). Gated by `_native_checker_active` (wired from
  `mypy/build.py`) and covered by `NativeTruthyTypeSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential on the
  captured fail messages plus direct seam calls), plus 9 pure decision
- `rust_classify_missing_annotations` (issue #1009, mypy.checker) — mirrors
  the decision head of `TypeChecker.check_for_missing_annotations`
  (checker.py:2722-2771): the `show_untyped` gate, the
  `has_explicit_annotation` scan (any non-`is_unannotated_any` site among
  arg_types + ret_type) feeding `check_incomplete_defs`, the self/cls-only
  special case for an untyped def, and the per-site return/param Any-ness
  including generator/coroutine ret unwrapping (reusing the existing
  `get_generator_return_type_inner` / `get_coroutine_return_type_inner`
  ports). Rust reads the option bools, the shim's `fdef.type` isinstance
  tag, `len(fdef.arguments)` / `arg_names`, the generator/coroutine flags,
  and the raw ret/arg types as wire bytes, and returns `(tag, param_fail)`
  (KIND_MISSING_ANN_NONE / RETURN_UNTYPED / FUNC_TYPE_EXPECTED /
  RETURN_EXPECTED). The Python shim applies the fail/note side effects
  (the RETURN_UNTYPED note decision routes through the existing
  `rust_has_return_statement` seam) and keeps the pure-Python body as the
  fallback. Defers (`None`) on an undecodable wire blob, a
  `TypeAliasType` ret type (Python's `get_proper_type` expands it from
  the live alias node), or an undecided generator unwrap. Gated by
  `_native_checker_active` (wired from `mypy/build.py`) and covered by
  `NativeMissingAnnotationsSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differential plus direct seam calls), plus 12 pure decision
  unit tests in `checker_functions.rs`.
- `rust_classify_simple_assignment` (issue #1055, mypy.checker) — mirrors
  the decision head of `TypeChecker.check_simple_assignment`
  (checker.py:6334): the stub `...` early return, the direct-accept path,
  and the try_fallback gate (`inferred is not None or` union lvalue) with
  the `simple_rvalue` short-circuit and the preferred/fallback context
  selector. Rust reads the proper lvalue type as wire bytes plus five
  scalar flags (`is_stub`, `rvalue_is_ellipsis`, `has_inferred`,
  `inferred_is_argument`, `simple_rvalue`) and returns a tag (STUB /
  DIRECT / FALLBACK_NO_PREFERRED / FALLBACK_LVALUE_PREFERRED). The Python
  shim applies the branch bodies (`expr_checker.accept` /
  `infer_rvalue_with_fallback_context`); the shared assignment tail and
  the NEED-ANNOTATION bit stay Python-side (the bit depends on the
  post-accept rvalue_type; both of its predicates are already native via
  #445 and the subtype resolver). The shim keeps its pure-Python stub
  early-return ahead of the seam, so the STUB arm is defensive-only.
  Defers (`None`) on an undecodable wire blob or a `TypeAliasType`
  lvalue. Gated by `_native_checker_active` (wired from `mypy/build.py`)
  and covered by `NativeSimpleAssignmentSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), plus 9 pure
  decision unit tests in `checker_functions.rs`.
- `rust_classify_return_stmt` (issue #1004) — two-phase port of
  `TypeChecker.check_return_stmt` (checker.py:6546). Rust owns the pure
  decisions: `rust_classify_return_stmt_variant` picks the variant tag
  (generator / coroutine / plain, never defers), `rust_classify_return_stmt_pre`
  fires NO_RETURN_EXPECTED (non-ambiguous `UninhabitedType`, suppressed for
  lambdas), and `rust_classify_return_stmt_post` classifies the post-accept
  arms (async-generator fail, warn_return_any gate, declared-None exemptions,
  the `check_subtype` call, and the empty-return arms). Python keeps
  `get_proper_type`, the `accept()` call and its binder side effects, the
  `check_subtype` body, and all fail/note emission: the shim applies the four
  distinct fail messages plus the `incorrectly_returning_any` note from the
  Rust tags and falls back to the verbatim pure-Python tail on a deferral
  (undecodable wire bytes, a `TypeAliasType`, or an unreadable warn-gate
  shape); `accept()` is never re-run on the fallback path. The warn gate's
  `is_proper_subtype(AnyType(special_form), ret)` check is decided
  structurally by `any_is_proper_subtype_of`: bare `AnyType` or a union with
  an `AnyType` item (verified against `subtypes.py` `visit_any` with
  `proper_subtype=True` plus the union-item decomposition), and the object
  clause is an Instance with type_ref `builtins.object`. Gated by
  `_native_checker_active` (wired from `mypy/build.py`) and covered by
  `NativeReturnStmtSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential on the captured
  message lists plus direct seam calls for the variant / pre / post tags and
  the None deferrals), plus pure decision unit tests in `checker_functions.rs`.
- `rust_has_return_statement` (mypy.traverser, `traverser.rs`) — mirrors
  `ReturnSeeker` for `has_return_statement` (traverser.py:946-963). The
  Python shim serializes the `FuncBase` via `mypy/astwire.py` and defers
  to the pure-Python `ReturnSeeker` when Rust returns `None`: the
  serializer emits a bare `LITERAL_NONE` for any node kind without a wire
  tag (e.g. a bare `FuncItem`, issue #1030), and the undecodable root
  defers instead of silently answering `False`. Covered by
  `NativeTraverserSuite` in `mypy/test/testtypes.py`.
- `rust_classify_type_guard_arg` (issue #1043) — mirrors the decision head of
  `TypeAnalyser.anal_type_guard_arg` / `anal_type_is_arg` (typeanal.py:2009-2033)
  plus their outer wrappers `anal_type_guard`/`anal_type_is` (:2001-2025). Rust
  classifies from three scalars (the shim-precomputed `fullname`, `args_len`,
  and an `is_typeis` family flag): NOT_GUARD (fullname outside the
  {"typing.TypeGuard","typing_extensions.TypeGuard"} or
  {"typing.TypeIs","typing_extensions.TypeIs"} name-set -> wrapper returns
  None), FAIL (arity != 1 -> Python emits the existing VALID_TYPE fail +
  AnyType(TypeOfAny.from_error)), or RECURSE (Python runs
  `anal_type(t.args[0])`). The `isinstance(t, UnboundType)` wrapper check and
  `lookup_qualified` stay Python-side; all Rust facts are scalars, never
  defers. Serves all three call sites: the `or`-chain at :1214-1215 (bool
  alias), the native special-unbound tag-applier at :1443-1445, and the
  `visit_callable_type` ret_type wrappers at :1928-1929. Gated by
  `_set_native_typeanal_active` (wired from `mypy/build.py`) and covered by
  `NativeTypeGuardArgSuite` in `mypy/test/testtypes.py` (gate-off vs gate-on
  differential plus direct seam calls), plus pure decision unit tests in
  `typeanal_special.rs`.
- `rust_classify_remove_unpack_kwargs` (issue #1044) — mirrors the guard
  chain + overlap-set arbitration head of
  `SemanticAnalyzer.remove_unpack_kwargs` (semanal.py:1586-1620): Rust
  reads the live `CallableType` `arg_kinds`/`arg_names` via PyO3 plus one
  wire serialization of the last arg type (UnpackType tag, then the
  target proper-type tag == TypedDictType) and returns a 4-way tag
  (PASSTHROUGH / NOT_TD_FAIL / OVERLAP_FAIL with the sorted overlap list
  minus the trailing kwargs name / OK). Python applies both `self.fail`
  emissions, the AnyType(from_error) rewrites, and the OK-path
  `arg_types[:-1] + [p_last_type]` + `unpack_kwargs=True` rewrite. Defers
  (None) on a failed/undecodable last-arg wire serialization or an alias
  target (`get_proper_type` needs the live alias). Gated by
  `_native_semanal_visitor_active` and covered by
  `NativeRemoveUnpackKwargsSuite` in `mypy/test/testtypes.py`.
- `rust_classify_check_arg` (issue #1048) — mirrors the 4-way elif-chain
  head of `ExpressionChecker.check_arg` (checkexpr.py:4161-4204). Rust
  reads the wire caller type (DeletedType tag) plus two Python-computed
  booleans (`is_subtype` via the subtype resolver, `has_abstract_type_part`
  via `rust_has_abstract_type`, Tuple-x-Tuple fold Python-side) and
  returns a branch tag (DELETED / ABSTRACT_ONLY / INCOMPATIBLE / PASS).
  Python applies `deleted_as_rvalue` / `concrete_only_call` /
  `incompatible_argument` + the `is_star()` note gate +
  `check_possible_missing_await`. Defers (None) on undecodable wire
  bytes. Gated by `_native_checkexpr_active` and covered by
  `NativeCheckArgSuite` in `mypy/test/testtypes.py` (gate-off vs gate-on
  differential plus direct seam calls), plus pure decision unit tests in
  `checkexpr_functions.rs`.
- `rust_classify_type_check_raise` (issue #1050) — mirrors the decision
  head of `TypeChecker.type_check_raise` (checker.py:6979-7010): Rust
  decodes the wire proper type of the raised expression and returns a
  3-way tag (DELETED / PLAIN / NOT_IMPLEMENTED). The DeletedType arm
  short-circuits ahead of the not-implemented guard, mirroring the
  Python order; NOT_IMPLEMENTED comes from the wire `Instance.type_ref`
  membership in `NOT_IMPLEMENTED_TYPE_NAMES` or the shim-supplied
  callee fullname fact (`CallExpr` + `RefExpr` with fullname
  `builtins.NotImplemented`). Python applies `deleted_as_rvalue`, the
  `check_subtype` against the BaseException union (already native, with
  the `NoneType` item when `optional`), the zero-arg FunctionLike
  `check_call`, and the "did you mean NotImplementedError" fail. Defers
  (None) on undecodable wire bytes. Gated by `_native_checker_active`
  and covered by `NativeTypeCheckRaiseSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), plus pure
  decision unit tests in `checker_functions.rs`.
