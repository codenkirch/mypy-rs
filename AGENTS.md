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
  args, and any pair already on the Python `seen_instances` stack
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
