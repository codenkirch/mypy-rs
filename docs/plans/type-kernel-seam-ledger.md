# Type kernel seam ledger (archived from AGENTS.md, 2026-09-15)

Historical, seam-by-seam record of the mypy-rs native type kernel: wire
invariants, gate names, per-seam defer audits, and the wave migration
ledger. Archived out of AGENTS.md because it is reference material, not
always-on context. The active operations manual (build orders, gates,
parity baselines) stays in AGENTS.md; per-wave plans live in
`docs/plans/`.

Append new ledger entries here instead of re-inflating AGENTS.md.

### Retirement log (check this before treating an entry below as live)

A retired seam has no Python shim: `mypy/<module>` calls the pure-Python body
directly. The Rust pyfunction stays registered, so direct-seam tests still call
it. The per-seam entries further down describe the *pre-retirement* design; a
seam listed there as live may since have been retired, so check this table
first. Three sessions in a row (a lane, a measurement brief, and an issue body)
spent effort on seams that were already retired here.

| retired in | commit | seams |
|---|---|---|
| #1741 | `8c1b0c6c3` | `rust_copy_modified`, `rust_flatten_nested_unions` |
| #1740 | `10762e5a5` | `func_has_self_or_cls_argument`, `has_abstract_type`, `is_true_literal`, `is_false_literal`, `refers_to_typeddict`, `has_placeholder` |
| #1738 | `809b53b90` | `classify_member_resolution` |
| #1737 | `b06ad64ef` | `visit_name_expr`, `should_wait_rhs` |
| #1736 | `0a6bfe978` | `rust_lookup` (`_rust_lookup_qualified` is still live) |
| #1685, #1669 | `0046062a1`, `618c2b196` | residual scalar-only wire seams |
| #1664 | `032caceee` | `is_literal_type_like` |
| #1648 (#1640) | `ad8783e97` | hot short-call reads in `mypy/types.py`: `is_generic`, `has_recursive_types`, truthiness defaults, `is_var_arg`/`is_kw_arg`, min/max args, tuple/union length |
| #1514, #1492 | `530919b65`, `83f0b6706` | wave-61A decidable leftovers; symtable find_member/unpack/apply-report defers |

In flight when this log was written: `rust_fill_typevars` (#1744),
`rust_map_actuals_to_formals`, `rust_map_actuals_to_formals_with_types`,
`rust_map_formals_to_actuals` and `rust_check_argument_count` (#1747).

Correction, 2026-09-17: this table was not extended by the retirement waves
that followed, so treat the **per-area pin files** as the authoritative
per-seam list —
`mypy/test/testtypes_native_retired{,_checker,_checkexpr,_checkmember,_typeanal,_types}.py`.
The five seams named as in flight above have all since been retired: verified on
`476accf40` by scanning every `mypy/**/*.py` outside `mypy/test/` for each name
and classifying each hit as a call site, an import item, a `= None` fallback or a
comment — none has a call site left. Beware one method trap when re-verifying:
a `\b`-anchored search (`rg "\b<seam>\b"`) **misses calls made through the
aliased name** `_rust_<seam>`, because `_` is a word character, so the alias call
form is invisible to it.

The decision rule behind these, measured as min-of-7 ns/call with both arms in
one process and the FFI ticket spied for engagement: **the wire interface loses
when the Python body is an O(1)/O(n) rebuild or scan, and wins when the body is
a recursive visitor.** `rust_expand_type` (0.43-0.61x) is the standing example
of a port that pays and keeps its interface. Per-seam numbers and the ranked
remainder are in #1739; `rust_analyze_instance_member_dispatch` (1.10x) and
`rust_classify_special_unbound` (0.78x on its common shape) measured as
keeps, not retirements.

Caveat added 2026-09-17 (#1624 addendum): the 1.10x figure for
`rust_analyze_instance_member_dispatch` is **not reproducible** in the
reader-visible shapes — re-measured 0.40-0.62x on static, trivial-self and
generic, all three a loss. The shape that produced 1.10x is unrecorded, so the
keep is **unresolved rather than established**. Do not retire the seam on the
different-shape number; the originating harness needs recording or the keep
re-deriving. `rust_expand_type`'s keep is unaffected: it is documented here as
a deliberate keep of a port that pays.

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
- Wire-format invariants (keep Python `read_type`/`*.write` and
  `crates/type_kernel/src/wire.rs` in lockstep; a stale `.so` fails the
  END_TAG assert and defers to Python):
  - `Parameters` carries `is_ellipsis_args` (written last before
    END_TAG on both sides, issue #1115). Because `Parameters.write`/
    `read` are shared with the persistent meta-cache format, this
    change bumped `CACHE_VERSION` in `mypy/cache.py`.
  - The wire `Type` carries NO line/column. Wire-decoded nodes come
    back with `line == -1`; seams whose consumers key on positions
    re-stamp from the live input (the typeanal seam does this via
    `_WirePositionStamper` in `mypy/typeanal.py`, issue #1115).
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
  `mypy/types.py` decodes tag 122 only behind the module-level opt-in
  flag `_ALLOW_WIRE_ERASED_TYPE` (default False), flipped per decode
  with a `finally` restore, so any Rust wire seam that emits
  `ErasedType`-carrying bytes fails to round-trip through
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
- `rust_classify_all_supers_gate` (mypy.checker, issue #1060): the Rust
  classifier in `checker_functions.rs` ports the entry gate and per-base
  skip decisions of `TypeChecker.check_compatibility_all_supers`
  (checker.py): the Var/annotated-line/`lvalue.kind`/has-bases gate that
  decides whether the classvar + final super checks run at all, and for
  each `mro[1:]` base the `allow_incompatible_override` + `is_private`
  skip pair. Rust reads the live `lvalue_node` scalars via PyO3
  (`lvalue.line`, `lvalue.kind`, `lvalue_node.name`, `info.bases`,
  `info.mro`, `allow_incompatible_override`, per-base `base.fullname`)
  and returns `(gate_tag, base_skip_tags)`; the Python shim applies the
  early return, drives the per-base loop with the skip list, and keeps
  the check bodies (`check_compatibility_classvar_super` /
  `check_compatibility_final_super` / `check_compatibility_super`),
  `node_type_from_base`, and the inferred-var stash/restore in Python.
  Defers (`None`) on an unreadable fact so the pure-Python body runs
  unchanged. Consumes no wire types. Gated by `_native_checker_active`
  and covered by `NativeAllSupersGateSuite` in `mypy/test/testtypes.py`
  (direct seam tag tests + gate-off vs gate-on differential), plus 10
  pure decision unit tests in `checker_functions.rs`.
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
- `rust_check_for_untyped_decorator` (issue #942, reworked in #1296) —
  mirrors `TypeChecker.check_for_untyped_decorator`
  (checker.py:6955-6964): the bool gate `disallow_untyped_decorators and
  is_typed_callable(func.type) and is_untyped_decorator(dec_type) and not
  current_node_deferred`. Rust folds the wire-format `is_typed_callable`
  port for the func side with the two scalar flags, and walks the
  decorator side as a live PyO3 object (`rust_is_untyped_decorator`,
  below): an Instance decorator no longer defers, the seam runs the real
  `TypeInfo.get_method("__call__")`. Short-circuit order mirrors Python,
  so an untyped func skips the decorator walk. The Python shim emits
  `typed_function_untyped_decorator` when the result is True and keeps
  the pure-Python body as the fallback. Defers (`None`) on an
  undecodable func blob, an alias nested inside the decorator's
  callable, or an unreadable fact in the live walk. Gated by
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
  and the unbound non-alias `TypeVarExpr` fail tail (arg re-analysis +
  messages) defer (`None`) to the pure-Python body; an unbound non-alias
  `TypeVarExpr` under `allow_unbound_tvars` decides natively (new tag,
  issue #1265: the without-info back's Option 2 returns the raw type).
  `tvar_scope.get_binding` and the "a typevar param is a
  `PlaceholderType` → `api.defer()`" pre-check stay Python-side (the
  shim re-flags the pre-check as a fact and re-applies the deferral on a
  decided tag). Gated by `_set_native_typeanal_active` (wired from
  `mypy/build.py`) and covered by `NativeUnboundBranchFrontSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential on the
  result string, error messages, and defer / record counts), plus pure
  decision unit tests in `typeanal_unbound2.rs`.
- `rust_analyze_unbound_without_info` (issue #1278) — mirrors the full
  back seam `TypeAnalyser.analyze_unbound_type_without_type_info`
  (typeanal.py:986-1107), the hook the unbound-front classifier's
  deferred branches route into. The Python shim computes raw node facts
  (isinstance chain on the live `sym.node`, the `sym.fullname` ->
  `node.name` fallback, and skip-to-assert when both are None) and Rust
  decides the ordered table: Var typed Any; under `allow_type_any` the
  `type` special form and `TypeType[Any]`
  (`AnyType(from_another_any, source_any=...)`); the unbound type
  variable kept under `allow_unbound_tvars` or rejected with the
  PEP 695 / classic split; the enum member as a `LiteralType` inside
  Literal or the raw-enum error outside; then the message-tail kinds
  (Var / function symbol / MypyFile / other) from the shim-precomputed
  `tail_kind` plus the resolved `name`, with the
  `builtins.any` / `builtins.callable` / callback-protocol notes
  returned as note tags. Python keeps `tvar_scope.get_binding` (a fact
  input), the `anal_array` arg re-analysis, all result-object
  construction on live objects, and every fail/note emission keyed by
  the returned tag. Never defers on well-formed facts. Gated by
  `_set_native_typeanal_active` (wired from `mypy/build.py`) and
  covered by `NativeUnboundWithoutTypeInfoSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential on
  `str(result)` and captured fail/note messages, plus direct seam calls
  for every table branch), and 10 pure decision unit tests in
  `typeanal_unbound.rs`. Cold self-check: 574 calls @ 19% -> 570 @
  100% native (0 defers; the audit's decidable buckets were
  tail:Var 310, tail:FuncLike 119, tvar_rejected 22, tail:MypyFile 14,
  type_type_any 1).
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
- `join_type_list` defer round 2 (issue #1266) — undecided Instance
  pairs the setops kernel declines (`join_types` -> None) now route
  through `setops::join_instances_core` for a decision instead of
  deferring the whole fold (`join_one_pair` retry, core result
  re-oriented by `map_core_result` to match the shim's `(t, s)`
  naming). `join.py` hoists the n<=1 early returns ahead of the native
  gate; `join_instances_core` / `SeenInstances` widened to
  `pub(crate)`. Cold self-check audit (stripped): whole-call events
  74, native 18 -> 21 (28%), lkv wall 39 unchanged. Remaining walls:
  the lkv bucket (by design), pairs the core itself declines (9),
  concrete-mapping defers in `join_one_pair` (non-plain-Any
  arg_disc-4).
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
- `rust_infer_function_type_arguments` wire framing (issue #1455): fixed
  the ifta solution-list output contract (solve.rs). The per-var
  presence flags were written as raw `push(0|1)` bytes while the
  Python reader (`_deserialize_optional_type_list`, checkexpr.py)
  reads the count AND every flag via `mypy.cache.read_int`
  (LITERAL_INT tag + bare int), so every non-deferred blob failed at
  flag 0 and the seam delivered 0 true-native results despite the
  survey reporting "~30% native" (non-None raw counted as native).
  Flags are now `wire::write_int`. The decoder also runs
  `fixup_wire_type` on each present solution (type_ref -> live
  TypeInfo); without it, wire-decoded Instances crash downstream
  subtype checks with "De-serialization failure: TypeInfo not fixed".
  Wire-decoded solutions also lose the `definition` slot (the wire
  CallableType carries `name` but not `definition`), and
  `pretty_callable` (messages.py) renders `def <name>` from it when
  `name` is None, so a native join of two named function types
  rendered nameless (`testListLiteralWithNameOnlyArgsDoesNotEraseNames`
  regression). `_restore_ifta_definitions` (checkexpr.py) reattaches
  it from the live pass-1 actuals: a single lower keeps its own
  definition (the Python no-op solve returns the live object), a
  multi-lower join takes the last sorted lower (the `join_type_list`
  seam invariant, join.py), and star actuals skip (Python folds over
  expanded definition-less lowers). Cold self-check audit
  (before/after): ifta 188 calls @ 0% true native -> 56 native
  (~30%); residual defers: var_pspec_tvt 80 (ParamSpec/TypeVarTuple,
  by design), engine 47, solve 5. Covered by the framing unit test in
  `solve.rs` (`ifta_solution_list_flag_framing_roundtrips_read_int`),
  `NativeIftaDefinitionRestoreSuite` in `mypy/test/testtypes.py`, and
  the testcheck gate-on/off parity differential.
- `rust_analyze_descriptor_access` (issue #1108) — reworked into a tag
  protocol mirroring the pure guard head of
  `analyze_descriptor_access` (checkmember.py:1376-1432). Rust returns
  `Option<(tag, bytes)>`: tag 0 = ORIG (shim returns the live
  `orig_descriptor_type`), tag 1 = VALUE (a UnionType mapped item-wise
  through the same decision and joined via make_simplified_union; shim
  decodes and restores line/column), `None` = defer. Non-Instance
  proper types (CallableType/NoneType/TupleType — ~85% of measured
  calls) and Instances with no readable `__get__`/`__set__` for the
  access kind decide ORIG; a `__get__`-bearing Instance and the lvalue
  `__set__` assign path defer (checker-state tail:
  transform_callee_type, check_call, warn_deprecated stay Python-side).
  The old TupleType arm wrongly checked the fallback and is fixed.
  Measured: 29,922 calls @ 2% native → 25,516 @ 100% native; the count
  drop is unions no longer re-entering the shim per item (the tail
  never fires in the self-check corpus). Covered by
  `NativeDescriptorHeadSuite` (direct seam tag tests + gate-off vs
  gate-on differential through the real function) plus 12 Rust unit
  tests in `checkmember.rs`.
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
  Issue #1129 ports the tail's `freeze_all_type_vars` (typeops.py:2102): the
  static tail free-expands via `expand_type_by_instance_free`, then
  `collect_freeze_ids` / `apply_freeze` set `meta_level = 0` on every
  typevar listed in a `variables` entry (the wire round-trip broke Python's
  shared-object mutation, so the rewrite is id-keyed over the whole tree).
  Measured (#1129): seam calls 4,983 → 2,763, global python fallbacks
  88,985 → 79,986; remaining seam defers are TypeAliasType in the signature
  (~55%), alias surviving expansion (~20%), and env miss (~19%).
  Issue #1277 narrows the #1129 survivors gate: after a successful
  expansion the old gate rejected legal leftovers Python's freeze also
  leaves untouched (a caller's own tvars riding in through the substituted
  receiver args of class-tvar methods, whose `variables` list is empty) and
  deferred 97.6% of the seam. The #1129 "env miss" rationale was
  over-conservative: the expand already defers on what it cannot key, so a
  gated post-expand tree was by construction post-substitution. The gate
  now defers only on a leftover that is neither a `variables` entry nor
  among the mapped receiver's args (collected via `collect_tvar_keys`;
  ParamSpec/TypeVarTuple match by the -1 meta sentinel) or on any
  `UnpackType` occurrence; the freeze step is parity-safe for the
  receiver-arg class because Python's freeze only rewrites `variables`
  entries. Measured: 689 calls @ 17% native (incl. 561 re-entrant seam
  calls spawned by the defers' Python fallback) → 128 calls @ 92% native;
  the 10 residual defers are 6 `map:failed` and 4 `codec:decode-instance`,
  the same unported small buckets as before.
- `rust_analyze_instance_member_dispatch` defer closures (issue #1112) —
  ports two IAMA dispatch defers into the kernel: (a) the
  `rust_freshen_function_type_vars` `TypeAliasType` arm (freshen walks alias
  args only, TypeVars pass through unchanged per `type_visitor.py:239-240`;
  a `Parameters` arg defers), and (b) the `builtins.tuple` special case of
  `maptype.map_instance_to_supertype` (tuple_map: `Some(Some(mapped))`
  decided tuples ride `tuple_special`, undecided ones defer). Also fixes a
  latent bug in the TupleType arm of `analyze_member_access_inner`: it
  recursed on the wire's `partial_fallback`, which for a plain tuple literal
  is `tuple[Any, ...]`; it now computes `tuple_fallback(typ)` like Python
  (`typeops.py:339-375`) and defers when that does not yield an Instance.
  Measured (self-check): IAMA dispatch 99,535 calls / 12,126 fallbacks
  (88% native) → 96,025 / 7,919 (92% native);
  `rust_freshen_function_type_vars` 100% native. No new Python-side suites:
  exercised by the existing gate-on/off parity differential plus Rust unit
  tests in `checkmember.rs` and `freshen.rs`.
- `expand_type_inner` Callable arm `is_bound` (issue #833) — removed the
  `is_bound` defer in the wire Callable expansion: Python's
  `visit_callable_type` never branches on the flag (it survives
  `copy_modified` unchanged and expansion only touches arg_types/ret_type/
  type_guard/type_is/instance_type), so the Rust defer was over-conservative.
  Wall trace showed `callable-bound` dominated the expand defers; after the
  removal, `rust_expand_and_bind_callable` went 54% → 98% native,
  `rust_expand_type_by_instance` 6% → 67%, `rust_expand_type` 85% → 91%.
  ParamSpec/Unpack walls stay deferred.
- `_needs_python(definition_gate=False)` (issue #1169) — the
  `mypy.expandtype._needs_python` dispatch gate no longer forces a Python
  fallback just because a nested `CallableType` carries a `definition`; the
  four top-level seams (`expand_type`, `expand_type_by_instance`,
  `freshen_function_type_vars`, `freshen_all_functions_type_vars`) pass
  `definition_gate=False` and, on the native path, repair the wire-decoded
  result with the recursive `_resync_definitions(original, decoded)` — it
  pairs Callables/Overloads/Instances/Unions/Tuples/TypeType/Unpack/
  TypeVar upper bounds positionally and copies the live `definition`s over
  (`TypeType` must be rebuilt via `TypeType.make_normalized`, it has no
  `copy_modified`); a pairing mismatch returns `None` and the caller falls
  back to the full Python expansion. Env values keep the default gate.
  Meta-tvar relax round two (issue #1180): the whole-tree seams now take
  fresh (meta) vars across the wire and repair identity after decode via
  `canonicalize_fresh_vars` (optionally `seed=`ed with the variables
  slot), while `remove_trivial` keeps the meta check through the new
  `_needs_python(meta_gate=True)` kwarg: as a partial-list seam it has
  no enclosing variables context to seed, so a decoded fresh var there
  would stay a distinct object and `freeze_all_type_vars` (in-place
  `meta_level` mutation) would miss it. Fresh-var-bearing decoded trees
  stay out of the shared decode caches (an in-place freeze on a cached
  tree would leak across callers).
- maptype decode fresh-var repair (issue #1198) — extends the #1180
  whole-tree seam repair to the map seams: `_native_map_instance_to_supertype`
  and the `_native_map_step_frontier` per-member decode loop now run
  `canonicalize_fresh_vars(_reported)` after `fixup_wire_type`, because
  the wire round-trip splits re-occurrences of one fresh (meta_level > 0)
  id into distinct equal-id objects. Fresh vars reach these seams via
  constraint template mapping (`ConstraintVisitor.visit_instance`, 276 of
  6,003 self-check map-seam calls carry 280 fresh occurrences), so both
  gate walks stay free of a meta branch: gating would convert 276 native
  mappings into Python fallbacks, repair avoids that. The single seam
  keeps fresh-bearing trees out of `_map_supertype_decode_cache`: the
  re-unified tree shares one object per fresh id, so an in-place freeze
  on the cached result would leak into later callers of the identical
  blob (mirroring #1180); the frontier step has no cache and
  canonicalizes too, while its fresh-bearing members still defer
  per-member via the sentinel flag. typeops wire seams
  (`type_object_type_from_function`, `map_type_from_supertype`) carry 0
  fresh occurrences in the cold self-check, so only their `_needs_python`
  whole-tree walk got a docstring recording (#1198 found no new branch).
  Covered by `NativeMapFreshVarRepairSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on parity plus freshness identity, no-cache-insert,
  cache-hit-copy, and frontier per-member checks). No Rust change.
- typeops definition-gate relax (issue #1207): the two typeops wire
  seams (`type_object_type_from_function`, `map_type_from_supertype`)
  rode ~100% gate defers on the cold self-check because `_needs_python`
  bailed on the nested `CallableType.definition` every named
  `__init__`/`__new__` signature carries. Both seams now pass
  `definition_gate=False` (the #1169 pattern) and re-stamp the dropped
  links after the round-trip: `map_type_from_supertype` reuses
  `expandtype._resync_definitions` positionally (a pairing divergence
  defers), and the type-object composite gets a specialized
  `_restamp_composite_definitions`: the pure-Python body preserves
  definitions verbatim through bind_self / map_type_from_supertype /
  class_callable (all `copy_modified`), so the input signature's
  definitions are copied back per overload item, and an item-count
  divergence defers. No Rust change. Cold self-check audit: gate defers
  map 2,396/2,579 -> 0/453, type-object 2,396/2,645 -> 0/2,596 (the map
  total also drops because the composite's internal mapping stops
  crossing the Python seam); residual defers are genuine kernel
  decisions (generic signatures, recursive aliases), not gate defers.
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
- `rust_is_protocol_implementation` (subtypes.py:1766-1895, issue #1111)
  — the protocol-right Instance arm of `rust_is_subtype`. Wired from
  `visit_instance_nominal` via `protocol_right_decision` (subtypes.rs):
  Rust mirrors Python's `assuming` recursion guard with a thread-local
  stack keyed by the proper-subtype dimension, records the fine-grained
  dependency (`record_protocol_subtype_check`) through the live map
  (now on `TypeResolver`, not `NativeTypeResolver`), then drives the full
  member-compat loop natively (member lookup via `get_protocol_member_inner`,
  per-member `is_subtype` with a fresh default context, and the full
  subtypes.py:2025-2055 member-flag arbitration incl. the reversed
  settable check). Decorator nodes on the protocol (right) side unwrap to
  `.var` and route through `member_method_inner` (bind_self + expand),
  matching `find_node_type`'s callable path for decorated protocol
  members. Defers on: protocol-left (recursion-prone `assuming` guard),
  generic Callable-Callable member pairs (needs type inference), explicit
  -setter members (needs is_lvalue re-resolution), module instances and
  other extra_attrs carriers, base-class-defined members behind the
  same-class guard, MRO/resolver misses, and any call without a live
  TypeInfo map. Measured (self-check): protoR defers 14,794 -> 13,149.
  Covered by `NativeProtocolImplementationSuite` in
  `mypy/test/testtypes.py` plus pure unit tests for the assuming guard
  in `subtypes.rs`.
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
- `are_parameters_compatible` standalone shim (issue #1066) — wires the
  standalone `mypy.subtypes.are_parameters_compatible`
  (subtypes.py:2530, the `is_callable_compatible` tail and the
  `constraints.py` / `checker.py` overload paths) to the already-exported
  `rust_are_parameters_compatible` pyfunction (previously reachable only
  via `SubtypeVisitor.visit_parameters` and the meet overlap branch).
  The shim engages only when the caller's nested `is_compat` callback
  provably matches the kernel's fixed nested subtype semantics: the
  module-level `is_subtype` / `is_proper_subtype` (default context, and
  only when `ignore_pos_arg_names` is default so the flag cannot leak
  into nested callable comparisons the Python default context keeps
  off), or `SubtypeVisitor._is_subtype` over a context whose other flags
  (`ignore_type_params`, `ignore_declared_variance`, `always_covariant`,
  `ignore_promotions`, `erase_instances`, `keep_erased_types`) are all
  default. Everything else — `is_more_precise`, `is_same_type`, overlap
  predicates, `flip_compat_check` closures, test stubs — defers (None)
  to the pure-Python body, as do the kernel's own shapes (generic
  callables, unpack/alias types, resolver misses, undecidable nested
  pairs). No new kernel logic. Covered by standalone-shim tests in
  `NativeAreParametersCompatibleSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differentials incl. tvar and foreign-callback
  deferrals, plus direct seam calls) and trivial-arm unit tests in
  `callable_compat.rs`.
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
  restricted type. A `Union` expanded-left would recurse over live
  per-item flags the wire does not carry, so the shim precomputes the
  false_only/true_only(union) `UninhabitedType` verdict and passes it
  in as `restricted_uninhabited` (issue #1161); the Rust union arms
  consume the verdict instead of deferring. Python keeps
  `find_isinstance_check`, `analyze_cond_branch`, the two
  `self.msg.*_operand` emissions, and `make_simplified_union`.
  Defers (`None`) on `TypeAliasType` map values, a missing union
  verdict (`restricted_uninhabited is None`), and dunder lookups the
  resolver snapshot cannot decide (e.g. an int Instance under `or`,
  where `true_only` needs a live `__bool__`).
  Gated by `_native_checkexpr_active` (wired from `mypy/build.py`) and
  covered by `NativeCheckBooleanOpSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), and 19
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
  infinite recursion. Rust classifies two wire Type bytes plus the
  `is_recursive` flags (carried in the wire blob since wave31 #1361 as
  a tagged conditional int, mirrored on the Rust `Type` enum since F0
  #1349). The alias-chain expansion
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
- `rust_can_be_narrowed_with_len` (issue #1065) — exports the
  `can_be_narrowed_with_len` predicate port that shipped with #493
  (`crates/type_kernel/src/lennarrow.rs`): True for fixed `TupleType`
  (or unpack with `builtins.tuple` fallback), `Instance` with
  `builtins.tuple` base, and unions of those; False when a custom
  `__len__` overrides builtin behavior. Python shim at
  `TypeChecker.can_be_narrowed_with_len` (checker.py:9267), the hot
  gate consulted at the leaf of every `find_isinstance_check`
  conditional. Defers (`None`) on undecodable wire bytes, a missing
  resolver snapshot, or an unresolved alias target; the shim falls
  through to the pure-Python body. Gated by `_native_checker_active`
  (existing wiring, no build.py change) and covered by
  `NativeCanBeNarrowedWithLenSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differential plus direct seam calls), plus pure
  decision unit tests in `lennarrow.rs`.
- `rust_is_writable_attribute` (issue #1071) — mirrors
  `TypeChecker.is_writable_attribute` (checker.py:10167): a pure bool
  predicate over a live `Node`. A `Var` is writable unless it is a
  read-only property; a property `OverloadedFuncDef` is writable when
  its first item (kept a `Decorator`, mirroring the Python assert) has
  a settable property var; everything else is not writable. Rust reads
  the live node via PyO3 `is_instance` plus bool attrs, mirroring
  `rust_is_final_enum_value`; defers (`None`) on a non-`Decorator`
  overload head (Python asserts through the fallback) or an unreadable
  attribute. Gated by `_native_checker_active` (existing wiring, no
  build.py change) and covered by `NativeIsWritableAttributeSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus
  direct seam calls), plus pure decision unit tests in
  `checker_functions.rs`.

- `rust_classify_find_member` (issue #1074) — mirrors the
  name-resolution prelude of `mypy.subtypes.find_member`
  (subtypes.py:2025-2047): the `info.get(name)` miss path, the
  `__getattribute__` / `__getattr__` scan (skipping
  `builtins.object`), and the `fallback_to_any` /
  `meta_fallback_to_any` / `extra_attrs` verdicts. Rust reads the live
  `Instance` and `TypeInfo` via PyO3 (zero wire bytes) and returns a
  4-way tag (PROCEED / ANY_SPECIAL_FORM / EXTRA_ATTR / NOT_FOUND); the
  Python shim applies the verdicts (constructing the `AnyType`,
  fetching `itype.extra_attrs.attrs[name]` so the Type never crosses
  the seam, or returning None), and PROCEED falls through to the
  untouched checkmember tail (`MemberContext` +
  `analyze_class_attribute_access` / `analyze_instance_member_access`).
  The `type_checker is None` `find_member_simple` fallback stays
  Python-side and precedes the seam. Defers (`None`) on any unreadable
  attribute so the pure-Python body re-runs. Gated by
  `_native_find_member_prelude_active` (`_native_subtype_active` +
  `_native_subtype_resolver` + `_HAS_TYPE_KERNEL`, wired from
  `mypy/build.py`) and covered by `NativeFindMemberPreludeSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus
  direct seam calls), plus 9 pure decision unit tests in
  `findmember.rs`.

- `rust_check_call_head` (issue #1642, PR #1659) — batches the
  `enum_callable_base` and `typeobj_gate` per-call FFI seams in
  `mypy/checkexpr.py`'s `check_callable_call` into one FFI crossing.
  `rust_check_call_head(callable_node, callee, enum_bases)` returns
  `(enum_hit, typeobj_tag)`: the enum-callable-base check (live PyO3
  RefExpr isinstance + fullname-in-ENUM_BASES, never defers) and the
  typeobj gate (live PyO3 `is_type_obj()` + `type_object()` flags, defers
  None on PyAttributeError) are combined. Saves ~183k FFI crossings on
  the cold self-check (167k `check_callable_call` calls, each
  previously making 2 FFI crossings). Falls back to individual seams
  (`_rust_is_enum_callable_base`, `_rust_classify_typeobj_gate`) on
  exception. The other 4 scalar seams in `check_callable_call` cannot
  be batched: `map_actuals_to_formals` -> `compute_arg_context` ->
  `check_argument_count` form a sequential dependency chain, and
  `has_abstract_type` is per-arg granularity. Covered by the existing
  `NativeTypeobjGateSuite` gate-off/on differential. Gates: cargo
  2836/11, testtypes 325/3554, testcheck 8198/15/7 exact, fine-grained
  747/27, cold self-check clean 353.

- `rust_check_argument_count` (issue #1136) — reworked the wire-fact
  seam into a pure scalar-fact interface: no wire bytes cross the
  boundary. The seam folds `check_for_extra_actual_arguments` and the
  formal loop of `ExpressionChecker.check_argument_count`
  (checkexpr.py:3855) into one decision graph over scalar facts only:
  formal/actual `ArgKind` ints, per-actual shape tags
  (`NATIVE_ARG_SHAPE_PLAIN/TUPLE/TYPEDDICT/PARAM_SPEC/ALIAS` in
  `mypy/checkexpr.py` ~line 404, matching the Rust `ACTUAL_*`
  constants), per-actual item counts (tuple/TypedDict only), the live
  `formal_to_actual` pairs, and three scalars computed shim-side:
  `has_param_spec` (raw `arg_types[-2]` is `ParamSpecType` with the
  last two kind slots `ARG_STAR`/`ARG_STAR2`), `special_sig`, and
  `in_checked_function`. Rust returns
  `(ok, errors, is_unexpected_arg_error)` where each error is an
  `(ERR_*, index, 0)` record translated to a message by the Python
  shim. A `TypeAliasType` actual defers (`None`) via
  `ACTUAL_ALIAS` (proper-expanded aliases classify PLAIN). The
  `is_duplicate_mapping_inner` shape lookup indexes
  `actual_shapes` by the mapped actual index (`mapping[i]`),
  matching Python's `actual_types[m]` facts (#1152). Covered by
  `NativeCheckArgCountSuite` in
  `mypy/test/testtypes.py` (direct seam calls over every `ERR_*` record
  shape plus gate-off vs gate-on parity through the real method), and
  pure decision unit tests in `checkexpr_argcount.rs`.
- `rust_typeinfo_is_metaclass` per-build memo (issue #1137) —
  `TypeInfo.is_metaclass` (nodes.py) memoizes the Rust verdict per
  (MRO list identity, precise) key in `_metaclass_memo`, collapsing the
  #1135 measured 957k cold-check seam calls (~0.63µs fixed FFI cost,
  zero wire bytes) to one crossing per distinct MRO state. Invalidated
  at build boundaries via `_set_native_nodes_active` and per daemon
  recheck via `_clear_native_metaclass_memo` in
  `BuildManager._clear_native_resolvers`. Empty-MRO placeholders are
  never memoized (their MRO can bind later in the same build); entries
  hold strong refs to (info, mro) so a key id cannot be recycled.
  Covered by `NativeTypeinfoMetaclassMemoSuite` in
  `mypy/test/testtypes.py`.
- `try_getting_*_literals_from_type` decided-None protocol (issue #1168) —
  the three pyfunctions behind the typeops literal probes now return
  `(decided, values)` (#1101 protocol): `Some((true, list))` for a
  proven literal list, `Some((true, None))` when Python provably answers
  None (the dominant cold-self-check defer classes: plain Instance
  without `last_known_value` ~43% and non-str `last_known_value` ~34%,
  plus wrong-fallback literals / Instance-with-lkv union items /
  TupleType / AnyType), and shim-level `None` (defer) only for
  `TypeAliasType` candidates (top level or union item), where Python's
  `get_proper_type` needs the live alias target. Note Python checks
  fallback fullname before the value kind, so `Literal[True]` on a bool
  fallback is decided-None under the int target; the fallback-first
  order is mirrored in Rust. Cold self-check audit
  (before/after): str seam 3,310 calls @ 457 native (13.8%) / 2,853
  defers → 1,678 calls @ 1,676 native (99.9%) / 2 defers (both
  `TypeAliasType`); the ~2.9k Python fallback body re-runs are gone from
  the self-check corpus. Covered by gated unit tests in `typeops.rs`
  and `NativeTryGettingStrLiteralsSuite` in `mypy/test/testtypes.py`.
- `rust_any_constraints` original-constraint matching (issue #1171) —
  `_try_native_any_constraints` no longer rebuilds result Constraints
  from wire. The wire round-trip has no representation for
  `extra_tvars` (attached by `visit_callable_type` during
  polymorphic-overload inference), and rebuilding produced fresh
  targets/origins that broke `defaultdict[T, list[T]]` partial-type
  inference (`testPartialDefaultDict*` regressions). The shim now uses
  the kernel's output purely as a decision: each returned
  (origin-id, op, target) blob is matched back against the flattened
  valid options (eager truthiness, ascending order, monotonic cursor)
  and the original live Constraint object is returned; a constraint the
  kernel rewrote (merge_with_any union target) never matches and the
  whole call defers to the pure-Python body. Covered by
  `NativeAnyConstraintsSuite` in `mypy/test/testtypes.py` (gate-off vs
  gate-on differentials plus a direct seam call asserting the original
  object and its extra_tvars survive and value-equal options
  disambiguate).
- `rust_try_getting_instance_fallback` decided-None protocol (issue
  #1183) — the kernel speaks the #1101 `(decided, blob)` protocol:
  `(true, bytes)` for the fallback Instance, `(true, None)` for a
  decided no-fallback, `None` defers. The kernel returns `Fallback` /
  `DecidedNone` / `Defer` (`TgifOut` in `crates/type_kernel/src/
  typeops.rs`): `DecidedNone` covers the `else: return None` dispatch
  tail (TypeType, Union, Uninhabited, Unbound, Unpack, Deleted,
  Erased, ParamSpec, TypeVarTuple — every proper shape outside the
  isinstance chain) and the unreachable empty-`Overloaded` arm; the
  only genuine defer left is a top-level `TypeAliasType` with a
  missing alias snapshot. Python fast paths run before the seam: raw
  `NoneType`/`AnyType` return `None` outright (the largest measured
  cold-audit defer buckets, 1,518 calls) and `TypeGuardedType`
  unwraps to `type_guard` (mirroring `get_proper_type` in
  `types.py:4068`; the narrow fixup defers by design on alias-arg
  Instances — fixup must not freeze aliases without an alias map).
  Defer-audit (cold self-check, instrumentation stripped before
  landing): 18,799 calls @ 86.5% native → 18,567 calls @ 98.3%
  decided natively or by fast path (`native:instance` 87.7%,
  `native:decided-none` 1.2%, fast paths 9.4%; `gate-off` 1.5%,
  residual defers ~500, dominated by fixup-alias-args). Covered by
  `NativeTypeopsDeferralSuite` in `mypy/test/testtypes.py` (rewritten
  seam tests + `test_instance_fallback_decided_none_protocol`) plus
  kernel unit tests in `typeops.rs`.
- `rust_simple_literal_type` decided-None protocol (issue #1295) — the
  typeops `simple_literal_type` seam (shim `mypy/typeops.py:1166`) now
  speaks the #1101 `(decided, blob)` protocol: `(true, bytes)` for the
  fallback Instance of a literal, `(true, None)` for a decided
  not-a-literal (previously the Rust `None` made the Python shim re-run
  its body on every call — 233 calls @ 0% native on the cold
  self-check), `(false, None)` defers (undecodable blob / unencodable
  fallback). The shim returns early on `decided`; a decoded non-Instance
  falls through defensively. Covered by
  `test_simple_literal_decided_none_protocol` /
  `test_simple_literal_instance_with_lkv` in `NativeTypeopsDeferralSuite`
  (`mypy/test/testtypes.py`).
- `rust_is_untyped_decorator` live walker (issue #1296) — the wire
  version (220 calls @ 15% native) deferred on every Instance because
  the `__call__` lookup needs the live `TypeInfo`; it is now a
  live-PyO3-object seam (`rust_is_final_enum_value` shape, zero wire
  bytes for the subject): `get_proper_type` on a top-level alias via the
  real Python function, then the Callable/Overloaded/Instance dispatch,
  with the Instance arm running the real `TypeInfo.get_method("__call__")`
  and recursing through Decorator-head / Overloaded / plain-method
  types. Defers (`None`) on an unreadable attribute, a depth-cap
  overflow (50), or an alias nested inside a callable's arg/ret types.
  Feeds `rust_check_for_untyped_decorator` (issue #942, above), which
  now passes the decorator side live and skips the walk when the func
  side is untyped.
- `rust_analyze_member_method` subclass-receiver guard narrowing (issue
  #1184) — hoists `has_vars` (a CallableType with non-empty
  `variables`, or any Overloaded item carrying variables) to the seam
  entry and applies the classmethod / same-class equality guards only
  when the signature is variable-carrying: for a variable-free
  signature Python's full `bind_self` can only take its non-generic
  strip path, so the seam's filter + map + expand + strip composition
  is exactly the Python tail and the guard is redundant; genuinely
  generic shapes still defer at `mm::has_typevar`. The port exposed a
  real Python bug, fixed in `mypy/checkmember.py`
  `_restore_definition`: the Overloaded-to-CallableType matcher
  accepted only exact arg counts, so a `check_self_arg`-filtered
  overload lost its `definition` link after `bind_self` stripped self
  and `pretty_callable` rendered `def f() -> int` instead of
  `def f(self) -> int` (the full-suite ordering flake in
  `testSelfTypeOverrideCompatibility`). Measured (cold self-check):
  IAMA dispatch 92,585 calls / 4,353 fallbacks → 92,575 / 4,225 (95%
  native); total Python type-kernel fallbacks 31,496 → 29,115. CC
  stays deferred: 450/496 of its seam defers are generic callables
  needing Python's `unify_generic_callable`, not portable under the
  per-call classifier gate. Covered by
  `NativeMemberAccessDispatchSuite` in `mypy/test/testtypes.py`
  (`test_dispatch_completes_subclass_receiver_nongeneric` /
  `test_dispatch_defers_subclass_receiver_generic`) plus Rust unit
  tests in `checkmember.rs` (PR #1193).
- callable-vs-callable native constraint port (`visit_callable_type`
  CallableType-actual arm, constraints.py:1656-1780, landed as
  `callable_vs_callable_native` in `crates/type_kernel/src/
  constraints.rs` with the `param_spec_of` /
  `tuple_fallback_ref_from_unpack` / `repack_callable_args_wire`
  helpers and the shared `infer_callable_arguments_constraints_core`)
  — ports the ret-type constraint (with the type_guard / type_is
  unwraps), the unpack-repack + `build_constraints_for_simple_unpack`
  path, the plain-args `infer_callable_arguments_constraints` fold, and
  the template-ParamSpec prefix + `param_spec_target` arm. Routed from
  `mypy/constraints.py` `infer_callable_arguments_constraints` via
  `_try_native_infer_callable_args` (`rust_infer_callable_arguments_
  constraints`); `infer_directed_arg_constraints` keeps its existing
  seam. Defers (`None`) on normalization failure, a shape the wire
  cannot decide, or an actual carrying type variables: the
  opposite-direction polymorphic inference attaches `extra_tvars` to
  every emitted constraint and the wire format has no representation
  for those (#1171 pattern). The wire drops `meta_level` on ParamSpec /
  TypeVarTuple origin ids, so a decoded origin can never match a fresh
  call-site variable; the Python shims rebuild it via the corrective
  helper `_rebuild_wire_origin` (matches the input type vars by kind +
  raw_id + namespace, copies the full live id, raises to the fallback
  when the match is not unique). `repack_callable_args_wire` keeps the
  star UnpackType untouched when its proper type is not a TupleType,
  mirroring constraints.py:2162-2171 (the shipped Python repack seam
  short-circuits on an Unpack star, so only the internal
  callable-vs-callable path reaches that arm). Cold self-check audit:
  1,409 callable-vs-callable seam calls @ 65.3% native (920 decided /
  489 defers); residual defer buckets: cb-actual-generic 436
  (polymorphic actual), cb-actual-overloaded 44, cb-actual-instance 6,
  cb-actual-other 1, cb-norm-shape 2. Covered by the `test_cb_*` unit
  tests in `constraints.rs` (plain, ellipsis, unpack-template, ParamSpec
  prefix/target, ret-only) and the testcheck parity differential.
- generic overload-target dispatch through the solve kernel (issue
  #1204) — `rust_check_overload_call` no longer whole-call defers on a
  target carrying its own `variables`: the seam classifies each target
  into plain vs generic (via `is_type_obj` + `variables`) and evaluates
  a generic target by calling `rust_solve_generic_call` (first-match
  index semantics: solve, then evaluate the fully-substituted form like
  a plain target), deferring on ParamSpec / TypeVarTuple variables, a
  solve defer, or a residual unsolved callable. Per-target steps moved
  into `evaluate_plain_target` / `evaluate_generic_target` with a
  `MatchDecision` (Yes / No / Undecided) so a generic defer is
  indistinguishable from Python-level uncertainty. Python passes two
  new flags, `chk.in_checked_function()` (`strict`) and
  `type_state.infer_unions`, matching the solve shim thread. Cold
  self-check audit (instrumentation stripped before landing): seam
  defers 13,170 @ 68% native -> whole-call defers 3,917 -> 1,662
  (`shape_generic` 3,573 -> 37), native matches ~8,955 -> 11,501
  (~87%); remaining defers dominated by `gen_solve_defer` (948: the
  solve kernel's own ParamSpec/TVT/bounds defers). Covered by
  `NativeOverloadCallSuite` in `mypy/test/testtypes.py` (first-match
  ordering, reject stepping, star/typeobj/ParamSpec defers, direct
  generic solves) plus the testcheck parity differential.

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
- `rust_compute_arg_context_indices` (issue #1064, mypy.checkexpr) —
  ports the pure index-decision core of
  `ExpressionChecker.infer_arg_types_in_context` (checkexpr.py:3280-3285):
  the `arg_context` precompute loop mapping each actual-arg index to its
  formal index (source of the `callee.arg_types[fi]` context) or `-1`,
  skipping star args via the `ArgKind.is_star()` values (ARG_STAR = 2,
  ARG_STAR2 = 4). Args arrive as plain scalars (`arg_kinds` as integer
  `ArgKind.value`s, `formal_to_actual`, `len(args)`,
  `len(callee.arg_types)`), so no wire serializer is involved; later
  formals overwrite earlier ones, matching the Python loop order.
  Returns `None` (defer) only on malformed input (length mismatch,
  out-of-bounds actual/formal index); the per-arg `self.accept`
  recursion and the `infer_more_unions_for_recursive_type`
  `type_state.infer_unions` toggle stay in Python. Gated by
  `_native_checkexpr_active` and covered by `NativeInferArgContextSuite`
  in `mypy/test/testtypes.py` (direct seam calls plus gate-off vs
  gate-on differential across the 3 call sites), plus 9 pure index
  unit tests in `checkexpr_functions.rs`.
- `rust_always_returns_none` (issue #1070) — mirrors
  `ExpressionChecker.always_returns_none` /
  `defn_returns_none` (checkexpr.py:1714-1779) as a live-PyO3-object
  seam (`rust_is_final_enum_value` shape, zero wire bytes): Rust walks
  the recursive node kinds (FuncDef / OverloadedFuncDef / Var, the
  `OverloadedFuncDef.items` fold, and the `Var.__call__` recursion) and
  reads ret None-ness via the real Python `get_proper_type`, never bare
  attribute reads, so a partially-fixed wire object defers. The
  MemberExpr owner type is checker state (`chk.lookup_type`), so the
  shim pre-resolves it and passes the resulting `TypeInfo`. Any
  unreadable fact defers (`None`) to the untouched pure-Python body.
  Gated by `_native_checkexpr_active` (existing wiring, no build.py
  change) and covered by `NativeAlwaysReturnsNoneSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus
  direct seam calls), plus pure decision unit tests in
  `returns_none.rs`.
- `rust_lookup_definer` (issue #1075) — mirrors
  `ExpressionChecker.lookup_definer` (checkexpr.py:5862-5876), the
  pure MRO walk behind both `check_op_reversible` call sites
  (checkexpr.py:5947-5948): Rust reads the live `Instance`'s
  `typ.type.mro` via PyO3 (zero wire bytes) and returns the first
  `cls` whose `names.get(attr_name)` is present, in MRO order. A found
  verdict is `Some(Some(fullname))`, not found is `Some(None)`; any
  unreadable fact (`typ.type`, an MRO entry, its `names` or
  `fullname`) defers (`None`) to the untouched pure-Python body.
  Gated by `_native_checkexpr_active` (existing wiring, no build.py
  change) and covered by `NativeLookupDefinerSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus
  direct seam calls), plus pure fold unit tests in
  `lookup_definer.rs`.
- `rust_check_operator` code 1 (issue #1131) — the
  `check_op_reversible` STEP 2a ordering seam now returns the
  reverse-first variant code. Previously it deferred (`None`) whenever
  the elif chain would evaluate `covers_at_runtime` (checkexpr.py
  ~6093, mypy #19006): the non-instance path (either operand not an
  `Instance`) and the differing-definers path (behind the
  `alt_promote` gate). Both reduce to
  `covers_at_runtime(right, left)` (item=right, supertype=left), so
  they ride the already parity-tested `covers_at_runtime_inner` port
  in `covers.rs`; a `Some(true)` covers verdict implies exactly the
  reverse-first `variants_raw` Python would build. Rust defers
  (`None`) where the covers port or the definer/snapshot lookups are
  undecided (tuple-shaped operands, alias targets, missing snapshots,
  a `None` same-type result on shortcut ops). Gated by
  `_native_checkexpr_active` + `_native_checkexpr_resolver` (existing
  wiring, no build.py change); parity via the testcheck differential
  and 15 unit tests in `checkoperator.rs` (no Python suite: the seam
  has no direct-construction test shape).
- `rust_infer_operator_assignment_method` (issue #1079) — mirrors
  `infer_operator_assignment_method` + `_find_inplace_method`
  (checker.py:11498-11520), the pure `(True, "__i<rest>")` vs
  `(False, method)` decision for augmented assignments. Rust reads the
  live proper type via PyO3 (isinstance Instance / TypedDictType +
  `typ.fallback`, `typ.type.has_readable_member(...)`) plus the
  `method` string and the `in_ops` membership bool the shim computes
  from `operators.ops_with_inplace_method`, and returns the 2-tuple;
  never defers for well-formed input (`None` only on an unreadable
  attribute). `get_proper_type` stays shim-side. Gated by
  `_native_checker_active` (existing wiring, no build.py change) and
  covered by `NativeInferOperatorAssignmentSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus
  direct seam calls), plus pure decision unit tests in
  `checker_functions.rs`.
- `rust_check_final_member` (issue #1078) — mirrors the MRO fold of
  `mypy.checkmember.check_final_member` (checkmember.py:1360): Rust
  walks the live `info.mro` via PyO3 (zero wire bytes), looks up
  `base.names.get(name)` per entry, classifies the node kind
  (Var / FuncBase / Decorator via `is_instance`, covering the
  `is_final_node` tuple exactly), and reads `is_final`, folding the
  MRO into one bool (True = some entry is final). The Python shim
  keeps the `cant_assign_to_final` emission; a `None` defers on any
  unreadable fact so the pure-Python loop re-runs unchanged. Gated by
  `_native_checkmember_active` (existing wiring, no build.py change)
  and covered by `NativeCheckFinalMemberSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus
  direct seam calls), plus pure fold unit tests in `checkmember.rs`.

- `rust_get_target_type` (issue #1081) — mirrors the branch-selection head
  of `mypy.applytype.get_target_type` (applytype.py:244-296): Rust owns the
  tag arbitration (EXPAND_DEFAULT for an ambiguous UninhabitedType with a
  real tvar default, PASSTHROUGH for ParamSpec/TypeVarTuple/Any/cross-product
  /bound-ok, MATCH with the narrowest-match index over the value list, SKIP,
  REPORT) from wire `tvar` + `type` bytes; the Python shim computes the
  resolver-backed booleans (the is_same_type cross-product conjunction, the
  per-value is_subtype fold, the lazy narrowest-match matrix, and the bound
  check after applying the Self erase_typevars) and passes them in, then
  applies the side effects (`expand_type`, `report_incompatible_typevar_value`)
  and returns live types, so the result Type never crosses the seam. Defers
  (`None`) on undecodable wire bytes, a TypeAliasType argument (the proper
  -type expansion needs the live alias), or a missing fact for the branch
  Rust reaches. Gated by `_native_applytype_active` (existing wiring, no
  build.py change) and covered by `NativeGetTargetTypeSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus direct
  seam calls for every tag), plus pure decision unit tests in
  `applytype.rs`.
- `rust_classify_find_isinstance_head` (issue #1086) — mirrors the
  builtin-callee dispatch head of
  `TypeChecker.find_isinstance_check_helper` (checker.py:8418-8464): Rust
  reads the live callee via PyO3 (RefExpr isinstance, `fullname`, a
  TypeAlias deferral mirroring `refers_to_fullname`) plus the shim-computed
  `literal(expr)` scalar and returns an arm tag per builtin (BAD_ARGS /
  NARROW / TAIL, hasattr keeps the attr gate shim-side) or TYPEGUARD for
  the non-builtin callee. The Python shim applies the arm bodies
  (`conditional_types_to_typemaps`, `infer_issubclass_maps`,
  `conditional_callable_type_map`, `hasattr_type_maps`, and the extracted
  `_typeguard_call_maps` block) and falls back to the pure-Python head on
  `None`; the shared boolean-context tail moved to
  `_boolean_context_type_maps`. Gated by `_native_checker_active` (wired
  from `mypy/build.py`) and covered by `NativeFindIsinstanceHeadSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus direct
  seam calls per arm), plus pure decision unit tests in
  `checker_functions.rs`.

- `rust_classify_comparison_operands` (issue #1087, crates/type_kernel/src/
  comparison_narrowing.rs) — mirrors the Step-1 operand-classification front
  of `TypeChecker.comparison_type_narrowing_helper` (checker.py:8579): the
  `literal(expr) == LITERAL_TYPE` gate, the None / NotImplemented /
  True / False / enum literal suppressions, and the two non-narrowable
  proper-type tests (`FunctionLike.is_type_obj()` via the existing
  `callable_compat::is_type_obj` port, and `TypeType` over a `TypeVarType`).
  Python computes the cheap AST literal facts (`literal` kind + five flags,
  placeholders for non-LITERAL_TYPE operands so the short-circuit order is
  preserved) and serializes each operand type; Rust returns one
  narrowability bool per operand. Issue #1235: alias operands with an alias
  snapshot expand via `get_proper_or_expand` exactly like Python's
  `get_proper_type` (chain cycles and unsupported substitutions still
  defer), and a `TypeAliasType` ret-type of the callable operand expands
  too, deciding `is_type_obj() == False` when the expansion is
  `UninhabitedType` before the fallback-class lookup. `None` defers the
  whole call on a length mismatch, an undecodable wire blob, an alias
  operand/ret whose snapshot is missing or not expandable, or an unresolved
  type-object fallback snapshot (the audit's remaining wall); the shim
  re-runs the original pure-Python loop. The
  literal-hash bookkeeping, the grouping (`rust_group_comparison_operands`
  unchanged), and the narrowing arm bodies stay Python-side. Gated by
  `_native_checker_active` + `_native_checker_resolver` (existing wiring,
  no build.py change) and covered by `NativeComparisonNarrowingSuite` in
  `mypy/test/testtypes.py` (direct seam calls per branch plus gate-off vs
  gate-on differential through the real helper), plus pure decision unit
  tests in `comparison_narrowing.rs`.
- `rust_classify_check_assignment` (issue #1090, crates/type_kernel/src/
  checker_functions.rs) — mirrors the decision front of
  `TypeChecker.check_assignment` (checker.py:4681): the special-name front
  (NameExpr `__setattr__`/`__getattribute__`/`__getattr__` signature check,
  `__slots__` in a class body, `__match_args__` with an inferred Var,
  `__post_init__`, and the MemberExpr `__match_args__` fail) and the
  `lvalue_type` branch (partial-None inference, member assignment when
  `kind is None`, check_simple_assignment tail, no-type fallthrough).
  Rust reads the live lvalue node kind, the node/name scalars, the
  partial-None shape of `lvalue_type`, and the member `kind is None` fact
  via PyO3 and returns `(special_tag, branch_tag)`; the Python shim
  applies every arm body: the signature/slots/match-args/post-init checks,
  the partial-None inference with its binder `put` and
  `set_inferred_type` writes, `check_member_assignment` /
  `check_simple_assignment` / `check_indexed_assignment`, the abstract
  `Type[A]` concrete-only tail, and all binder (`assign_type`) and msg
  side effects. The tuple-vs-single dispatch and
  `try_infer_partial_generic_type_from_assignment` prelude stay Python-side.
  Defers (`None`) on any unreadable attribute so the pure-Python
  classification re-runs. Gated by `_native_checker_active` (existing
  wiring, no build.py change) and covered by
  `NativeCheckAssignmentHeadSuite` in `mypy/test/testtypes.py` (direct
  seam calls per arm tag plus gate-off vs gate-on differential), plus
  pure decision unit tests in `checker_functions.rs`.
- `visit_instance_nominal` per-arg variance walk (issue #1098,
  crates/type_kernel/src/subtypes.rs) — ports the args-differ dispatch of
  `SubtypeContext.visit_instance` (subtypes.py:1195-1203): a non-
  `TypeVarType` tvar gets `effective_variance = COVARIANT` (Python's
  else-branch) instead of deferring, and `check_type_parameter` gains a
  reflexive `left == right` fast path at the top. `VARIANCE_NOT_READY`
  still defers, but `mypy/build.py` (`_build_native_resolvers`, right
  after `_collect_incremental`) now pre-infers snapshot variance via
  `infer_class_variances` for infos carrying a NOT_READY TypeVarType,
  skipped entirely on empty-`scc` daemon mid-propagation calls
  (transitional `self.modules` pins wrong variance; #1146); known
  limitation: classes with unannotated attribute Vars fail
  build-time inference (Var.type is None at semanal) and keep deferring.
  Covered by `NativeArgVarianceWalkSuite` in `mypy/test/testtypes.py`
  (gate-off vs gate-on differentials for covariant/contravariant/
  invariant, ParamSpec same-ref/differing-args, NOT_READY defer proof).

- `rust_get_protocol_member` miss path (issue #1099) — extends
  `get_protocol_member_inner` (checker_helpers.rs) with
  `member_miss_decision`, the find_member missing-attribute front of
  `mypy.subtypes.find_member` / `find_member_simple`
  (subtypes.py:2072-2089): the `__getattribute__` / `__getattr__`
  accessor scan (`get_method_definer` mirrors `TypeInfo.get_method`,
  including the `{name}-redefinition` keys), the `fallback_to_any` ->
  `AnyType(TypeOfAny.special_form)` arm, and the plain miss ->
  `NoneVal`; a non-object accessor defers. `mro_get` /
  `mro_has` look up with `names.get(name)` semantics (a dict subscript
  raises `KeyError` on the first MRO base lacking the name and would
  truncate the walk, flipping base-defined members into wrong misses).
  Consumed both by the Python shim (`get_protocol_member`) and the
  Rust `is_protocol_implementation` member loop (`protocols.rs`).
  Gated by `_native_subtype_active` + `_native_subtype_resolver`
  (existing wiring) and covered by `NativeProtocolMemberMissSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on differential plus
  direct seam calls for the miss / accessor / fallback arms and the
  loop-level pre-check regression).

- `rust_get_declaration` / `rust_constant_fold_expr` decided-None protocol
  (issue #1101) — both seams now return a `(decided, value)` tuple instead
  of a bare value or None. Rust owns the whole walk, so every call is
  decided: a foldable/declared answer yields `(true, scalar)`, a genuine
  no-result (un-foldable expression; non-`RefExpr`, Var without type,
  `PartialType`, non-Var/TypeInfo node) yields `(true, None)`, and the
  Python shims return early on `decided` — previously a decided-None
  re-ran the full pure-Python walk on every call. `(false, None)` is
  reserved for a future deferral and currently unreachable; exceptions
  still propagate (binder keeps its `except Exception` fall-through,
  constant_fold keeps propagating). Covered by `NativeDecidedNoneSuite`
  in `mypy/test/testtypes.py` (direct seam decided-None calls plus
  gate-off vs gate-on parity).

- `rust_set_callable_name` (issue #1100) — mirrors
  `mypy.semanal_shared.set_callable_name` (semanal_shared.py:290-310). The
  class-context test mirrors Python's `if fdef.info:` truthiness via
  PyO3 `is_true` (`TypeInfo.__bool__` returns False for the FakeInfo
  placeholder `FUNC_NO_INFO` that non-method `FuncDef`s carry), not an
  `is None` check; the old check deferred every non-method call (~6,525
  measured on the cold self-check, 100% of the seam's defers) to the
  pure-Python body. Gated by `_native_semanal_shared_active` (existing
  wiring) and covered by `NativeSetCallableNameSuite` in
  `mypy/test/testtypes.py` (direct seam calls for method / TypedDict /
  FakeInfo / None-info / passthrough shapes plus gate-off vs gate-on
  parity through `set_callable_name`).

- `rust_find_self_type` (issue #1114) — mirrors
  `mypy.typeanal.find_self_type` (typeanal.py:4231, the `HasSelfType`
  BoolTypeQuery over a live type tree with the `lookup` Python callback).
  The audit (env-gated buckets over the cold self-check) found every
  defer in three decidable leaf shapes, now ported: `TypeList` items
  (query, matching `visit_type_list`), bare `EllipsisType` (`strategy([])`
  -> False), and `RawExpressionType` (`strategy([])` -> False). The port
  removed all 2,353 measured defers (35,204 calls @ 93% -> 35,210 calls
  @ 100% native). Gated by `_native_typeanal_active` (existing wiring)
  and covered by `NativeFindSelfTypeSuite` in `mypy/test/testtypes.py`
  (direct seam calls plus gate-off vs gate-on differential through
  `find_self_type`). Issue #1157: the walk formerly answered only from a
  `TypeAliasType`'s written args and never looked at the alias target, so
  `find_self_type(TypeAliasType(alias=X, ...))` missed `X = list[Self]`.
  The resolver-less `rust_find_self_type` now defers (`None`) on any
  `TypeAliasType`, and the live seam `rust_find_self_type_live(resolver,
  typ, lookup)` (exported from `type_kernel`; resolver is the
  `_native_typeanal_resolver` installed per build) expands the target
  through the alias snapshot (seen-alias cycle guard; defers on a
  missing snapshot, `no_args` with args, or non-`no_args` with alias
  tvars), then walks the bare target for non-PEP-695 aliases and also
  the written args under `python_3_12_type_alias`, mirroring
  `BoolTypeQuery.visit_type_alias_type`. Covered by the alias cases in
  `NativeFindSelfTypeSuite` (parity through a resolver-backed
  `setUp`, defer proofs for the resolver-less and missing-snapshot
  paths).
- `expand_aliases_depth` recursive-alias cut (issue #1149) — the alias
  expansion inside the subtype kernel now carries an active-path stack
  (`type ActiveAlias = (String, Vec<Type>)`, keyed by alias type_ref
  plus args identity). Re-entering an alias already on the active path
  returns the node unexpanded (the cut), matching what Python's lazy
  `get_proper_type` keeps at the same position; the stack pops on every
  decided path, so sibling occurrences with identical or differing args
  still expand. Defers on cut nodes so engine-level comparisons defer
  to Python, preserving parity. Cold self-check audit:
  present-but-deferred 6320 -> 5811 (-8.0%); `builtins._ClassInfo`
  414 -> 0. Covered by four Rust unit tests in `subtypes.rs`
  (self-recursive union cut, re-entry consistency, sibling
  same-alias-different-args expansion, pop-then-expand) plus
  `test_recursive_alias_gate_parity_no_wrong_verdict` in
  `NativeSubtypesDeferralSuite` (mypy/test/testtypes.py).
- `py_type_eq` (issue #1412, `crate::wire`) — mirrors Python `==` on two
  wire `Type`s for the kernel's self-return comparisons
  (`subtypes.rs:3488`) and the infer-variance identical-class fast paths
  (`infer_variance.rs:113`). Per-class structural equality following
  Python's `__eq__` overrides where they exist (`Instance`: type + args +
  last_known_value + extra_attrs; `TypeAliasType`: alias + args).
  `TypeVarType.__eq__` (types.py:837) compares id/upper_bound/values/
  default and deliberately ignores variance and name/fullname; the wire
  seam stands in for the `TypeVarId` comparison with
  `raw_id`+`namespace`+`meta_level`, which makes the comparison
  variance-insensitive. `ParamSpecType` compares id + flavor + prefix +
  default (the bound is determined by flavor), `TypeVarTupleType`
  compares id + min_len + default; every other pair falls back to derived
  byte equality (matching Python for tvar-free types). Cross-build facts
  (type_ref fullname for live TypeInfo identity) follow the usual
  wire-seam assumption. The fix un-breaks
  `testPEP695InferVarianceRecursive`: the recomputed self-return
  previously compared with wire-structural eq (variance-sensitive), so a
  recursive generic class whose self-return carries a different variance
  read as "self.type is not s.type" and produced a bogus INFERS/wrong
  variance verdict.
- `rust_expand_type` alias-entry defer removal (issue #1195) — the
  `expand_type` seam no longer defers alias-bearing inputs: the
  `alias_entry` guard (any `TypeAliasType` input) and the
  `res_alias` survivor check are gone from the entry path. Alias args
  expand natively (mirroring `visit_type_alias_type`) and the Python
  shim re-links wire-decoded alias nodes to live `TypeAlias` nodes via
  `fixup_wire_type(resolve_aliases=True)` (per-build
  `_wire_alias_map`, cleared per build in `mypy/build.py`); a decoded
  alias missing from the map defers to the pure-Python body. The
  `alias_ok` flag threads through `expand_type_with_env_inner`: only
  the `rust_expand_type` entry passes `true` — `expand_type_with_env`
  (infer_variance / expand_variants callers) keeps the survivor defer,
  since those callers have no re-link path. The env gate
  (`_env_substitutes_unsafe`) gained a raw-tree alias scan
  (`_contains_alias_raw`): a *used* env value carrying a TypeAliasType
  still defers, keeping the substituted node live-identity. Cold
  self-check audit: 65,618 calls @ 94.07% native / 3,901 fallbacks
  -> 64,961 @ 96.72% native / 2,121 fallbacks, zero alias defers
  (alias_entry 1,345 + res_alias 645 -> 0). Covered by
  `NativeExpandTypeAliasSuite` in `mypy/test/testtypes.py`
  (direct-seam engagement + gate-off/on parity + missing-alias-map
  fallback) plus four Rust unit tests in `expandtype.rs`.
- `rust_expand_type` union-flatten alias expansion (issue #1203): the
  second defer audit found `flatten` 437 of the 6,744 fallbacks:
  `expand_type_inner`'s union arm called the kernel's
  `flatten_nested_unions`, which defers on any alias item, while the
  Python tail (`make_union(remove_trivial(flatten_nested_unions(...)))`
  with `get_proper_type`, types.py:4047-4064 + 5057-5100) expands
  top-level alias chains before flattening. The port:
  `TypeAliasResolver` exposes a cheap `shared()` `Arc` view of its
  snapshot map (cached, invalidated per `insert`; inserts are rare,
  first-seal-wins per snapshot pass), a new `AliasLookup` trait makes
  the snapshot helpers (`expand_alias_shape`,
  `expand_alias_target_raw`, `expanded_alias_target`,
  `chain_resolve_alias_target`) take the map-shaped view, and
  `rust_expand_type` installs the map into a `FLAT_ALIASES` TLS slot
  for one call (RAII guard). The union arm then expands each top-level
  alias item's chain via `flatten_union_expanding_aliases`, recursing
  through union positions only and appending the ORIGINAL alias node
  for a non-union expansion (types.py:5098-5099); during chain
  substitution the union arm runs `InstantiateAliasVisitor` semantics
  (types.py:5513-5525, plain rebuild, truthiness flags via
  `union_item_can_be_*`, nested INSTANTIATE restores per guard).
  Defers: no alias map installed (seams reaching
  `expand_type_inner` without the expand entry keep the pre-1203
  contract), missing snapshot, alias cycle, or a `tvar_tuple_index`
  alias in the chain (the kernel zips args where Python splits the
  middle via `split_with_prefix_and_suffix`). The `res_tvar` bucket
  (6,277 -> 6,312 after: +35 flatten calls now surviving substitution
  as fresh trees) is the remaining dominant defer and is an
  object-identity contract (Python `t.visit_type_var` returns the
  original `t`), left for a future revisit. Cold self-check audit:
  113,468 calls / 6,744 fallbacks -> 6,342 fallbacks (-402; flatten
  437 -> 0, 35 of those now defer as `res_tvar`). Covered by eight
  Rust unit tests in `expandtype.rs` (alias-union expansion, original
  node preservation, tvt/missing-snapshot/cycle defers, chain-to-union,
  typed-args substitution with INSTANTIATE-leak check,
  instantiate-vs-flatten differential, no-TLS defer).
- `rust_analyze_member_method` bind-plan port (issue #1214) — closes the
  remaining member-method defers by mirroring `bind_self`'s generic solve
  branch (typeops.py:1062-1108) with per-item bind plans instead of a
  blanket defer on generic methods. Each filtered Callable/Overloaded
  item builds an `ItemBindPlan` (`plan_item_bind`): `self_vars` are the
  method tvars whose ids (TypeVarExtractor walk, ParamSpec/TypeVarTuple
  keyed with a -1 meta-level sentinel) appear in the self param, and
  `bare` is that subset represented by the self param itself when it is
  a plain `TypeVarType` (modern mypy's synthesized `Self`). The Rust
  tail runs the same order as Python: self filter (`check_self_arg`),
  `map_instance_to_supertype` to the defining class, free
  `expand_type_by_instance`, then per item: bare-subset substitution
  with the receiver (`subst_tvar_keys`, ExpandTypeVisitor visiting
  semantics: arg_types/ret_type/type_guard/type_is/instance_type and
  nested callables, not `variables`), removal of the remaining
  `self_vars` from `variables`, the freeze trio
  (`freeze_item`, typeops.py:2102), and the `bind_self_fast_inner`
  strip with `is_bound=True`. Defers: alias-carrying self param
  (Python expands from the live alias node), a tvar-like receiver
  combined with a bare-Self plan (solve-before-strip can diverge from
  strip-before-solve), star-self / empty-args items are planned empty
  (Python returns them unchanged), and any survivor typevar outside
  every `variables` list (env miss: the wire expand keys by receiver
  fullname, Python substitutes via live binders, e.g. a PEP695
  function-local class). `check_self_arg_inner` gained a
  `suppress_self_fail` flag (renamed from the dead
  `_allow_subclass_receiver` param): in the error-suppressed
  `find_member` contexts (protocol member fetches,
  `find_member_call_is_plain_callable`) Python keeps the ORIGINAL
  functype on a zero-match self filter and continues binding, so Rust
  now returns that functype in its three Python-returns-functype arms
  (no-formal-self, bad kind, pass-2 empty) instead of deferring;
  production member access passes `false` and re-runs the Python body
  to emit `incompatible_self_argument`. `get_protocol_member_inner`
  takes `self_type` and threads `original_left` into both member
  fetches (sub and sup; `rust_get_protocol_member` now decodes
  `original_left` instead of discarding it, mirroring
  find_member(member, left, original_left) vs
  find_member(member, right, original_left)); inside
  `is_protocol_implementation_inner` (top of the function
  original_left == left) both fetches pass `left` as `self_type`.
  The `assuming` recursion guard moved from `protocol_right_decision`
  into `is_protocol_implementation_inner` (subtypes.py:1972-1976
  `pop_on_exit(assuming, left, right)`, `assuming_contains` /
  `AssumingPush` now `pub(crate)`) so the direct pyfunction path is
  guarded too: without the stack entry the self-arg pass-2
  `is_subtype(I, P)` re-entry recursed and the protocol suite's
  decided-False tests failed. Cold self-check audit
  (MYPY_DEFER_AUDIT_1214, stripped before landing): 1,103 seam defer
  events @ ~0.3% native -> 79 (4 native, 75 defers; -93%). Covered by
  the gate-on/off parity of `NativeProtocolImplementationSuite` /
  `NativeProtocolMemberMissSuite` / `NativeProtocolMemberDeferSuite`
  plus the reworked `NativeMemberAccessDispatchSuite` tests
  (`test_dispatch_completes_subclass_receiver_generic`,
  `test_dispatch_defers_subclass_receiver_tvar_self`;
  `test_dispatch_defers_non_final_init` keeps deferring by design:
  a non-final `__init__` with a `NoneType` self declaration is decided
  by Python's `check_self_arg` error path, which emission the seam
  must not swallow).
- `rust_expand_type` leftover-tvar relink (issue #1215, defer round 3): the
  third audit showed both pools back to one dominant defer class
  (`expand/res_tvar` 6,332, by-instance `res_tvar` 268), the
  object-identity contract (Python `visit_type_var` returns the original
  `t`) left standing after #1203. The seam now returns the expansion
  instead of deferring when unmatched TypeVars remain:
  `expand_type_with_env_inner` / `expand_type_by_instance_inner` gain a
  `relink_ok` flag threaded from the two FFI entries, and the Python shim
  (`mypy/wirefixup.py:resync_var_identities`) relinks each
  structurally-equal decoded TypeVar onto the live original (unmatched
  vars are matched against `env.values()` / instance args), returning
  `None` (defer to the pure-Python body) when an occurrence cannot be
  matched, including `TypedDictType.items` whose wire shape is a plain
  `dict[str, Type]`. Measured (cold self-check, instrumented run,
  instrumentation stripped before landing): `rust_expand_type` 113,804
  calls / 30 fallbacks (was 6,362; res_tvar 6,332 -> 0; residual: encode
  29 (issue #1209, out of scope) + call_unpack_arg 1);
  `rust_expand_type_by_instance` pool 715 -> 472 (res_tvar 268 -> 0;
  remaining res_alias 442, flatten 29, call_unpack_arg 1). Covered by
  `test_typevar_result_relinks_identity` in `NativeExpandTypeEmptyEnvSuite`
  (seam engagement, gate-off/on parity, `on.args[0] is off.args[0] is
  self.fx.t` identity), plus the existing expand_type gate-on/off
  differentials.
- plugin-synthesized fake registrar (issue #1485) — extends the #1456
  pattern to plugin-built TypeInfos. `mypy/plugins/singledispatch.py:
  make_fake_register_class_instance` constructs the register-hook fake
  directly (fresh TypeInfo for `functools._SingleDispatchRegisterCallable`,
  never entering `self.modules`), bypassing the `make_fake_typeinfo`
  funnel, so `expand_type_by_instance` deferred I(fake) -> Instance args
  on a missing resolver snapshot (10 cold-self-check calls). The creation
  site now fires the same `_register_native_fake_typeinfo` registrar
  through a plugin-module hook (`_native_fake_info_registrar`, installed
  and cleared at the same build boundaries as the checker registrar):
  bases/MRO are final there, so the snapshot is conclusive; first seal
  wins across repeated register calls on the shared fullname. Measured
  (env-gated cold self-check, stripped before landing): those 10 member
  accesses now answer natively at the upstream
  `rust_analyze_instance_member_dispatch` seam (which needs the fake in
  the resolver), `expand_type_by_instance` no longer sees the fake at
  all. Covered by `NativePluginFakeRegistrarSuite` in
  `mypy/test/testtypes.py` (creation-site firing, direct seam
  defer/answer, manager-registrar first-seal-wins, and gate-on/off
  parity through the real `expand_type_by_instance`).
- checkmember/expand alias round-trip (issue #1224) — the IAMA defer
  audit (temporary `MYPY_TK_IAMA_AUDIT` dump, stripped before landing)
  found the two dominant alias defer classes in the member-access
  pools: `exp_alias_input` (alias-bearing signature deferred at
  `expand_type_by_instance_free`'s input gate) and `exp_alias_survives`
  (alias node surviving expansion, which decode leaves `alias=None`).
  The slice lets an alias-bearing method signature round-trip through
  the seam: `expand_type_by_instance_free` threads the existing
  `alias_ok` flag through `expand_type_by_instance_inner` (input and
  survivor checks both relaxed; core/relink keep `false`), matching
  Python's `visit_type_alias_type` (expand alias args, keep the node);
  `_deserialize_type_for_checkmember` passes
  `fixup_wire_type(decoded, resolve_aliases=True)` so decoded alias
  nodes re-link to live `TypeAlias` nodes via `_wire_alias_map`, and
  `set_wire_alias_map` now clears the wire decode caches when a new map
  identity is installed (cached decodes pin re-linked aliases from the
  previous map); `wirefixup._FreshVarCanonicalizer.visit_type_alias_type`
  descends into alias args (was `return t`), restoring the live-tree
  invariant that every occurrence of a type-var id shares one
  `TypeVarType` object — without it the `Pairs[Self]` alias arg stayed
  at meta level 1 after freeze-in-place, `bind_self`'s id-keyed env
  missed it, and `Self` escaped into the final member type
  (`testTypingSelfNestedInAlias`). An alias missing from the map keeps
  deferring via `fixer.missing`. Parity: 11,021 passed
  (`testtypes` + `testcheck`, `-n4`); the cold self-check failure set
  is unchanged against the HEAD baseline (14 pre-existing failures,
  tracked in #1228).
- is_subtype defer round 5 (issue #1233) — two ports from the cold
  self-check defer audit. Port B, the call-left/Instance-right protocol
  arm (`protocol_right_decision` in `subtypes.rs`): Python only runs
  `is_protocol_implementation(class_obj)` when `left.is_type_obj()`
  (subtypes.py:876-883), otherwise the answer is
  `is_subtype(left.fallback, right)`; Rust now decides `is_type_obj`
  via `crate::callable_compat::is_type_obj` and returns that fallback
  answer instead of deferring every Call|protocol-Instance pair
  (measured call-inst-protocol-notypeobj bucket, 144 defers; a true or
  unknown verdict still defers). Port A, `expand_aliases_depth` leaf
  variants: TypeAliasType nodes inside TypeVarType values / upper_bound
  / default, ParamSpecType prefix and bounds, TypeVarTupleType fallback
  and bounds, UnpackType targets, TypedDictType fallback and items,
  LiteralType fallback, and Parameters arg_types/variables are now
  walked like the existing Instance arm (measured alias-operand bucket,
  504 defers); the depth cap (50) and active-stack cut discipline are
  kept. Python-side coverage is the existing gate-off/gate-on
  differentials plus the cold self-check, and one new pair in
  `NativeSubtypesDeferralSuite`
  (`test_callable_protocol_right_no_call_non_typeobj_native` proves
  direct seam engagement via the mod.FbNoCall fallback fixture,
  `test_callable_protocol_right_typeobj_still_defers` proves the
  is_type_obj-True arm still defers). Parity: 11,027 passed
  (testtypes + testcheck, -n4); cold self-check 0 errors after fixing
  an implicit-reexport test import (ARG_POS off `mypy.types`).
- solve-path alias expansion (issue #1241): the three defer walls in
  the type-inference solve paths that cited an alias operand now
  expand `TypeAliasType` nodes through the alias snapshot before
  driving the kernel, mirroring Python's eager `get_proper_type` on
  each operand. `rust_solve_pre_validate` expands the constraint
  lower/upper-bounds before the replacement arc (no Python-side suite
  reaches the replacement arc, so it is exercised by 5 Rust unit
  tests), `expand_actual_arg` gains a 7th `alias_ok` parameter so an
  actual argument that is (or nests) an alias survives to the
  constraint engine, and the sgc formal and
  actual branches of `rust_solve_generic_call` expand their operands
  before serialization. Formal-path parity is verified at the byte
  level: the formal result keeps a raw alias (`fixup_wire_type` still
  refuses any tree containing `TypeAliasType`), so
  `NativeSolveGenericCallSuite._seam_raw` asserts
  `raw == _serialize_type_for_checkexpr(python_reference)` instead of
  object equality. Cold self-check audit: defers
  `rust_infer_function_type_arguments` 1162 -> 828,
  `rust_solve_generic_call` 1207 -> 1127, `rust_is_subtype`
  30422/1631 unchanged (total -414, no regression elsewhere). Covered
  by `NativeSolvePreValidateAliasSuite` and the alias cases in
  `NativeSolveGenericCallSuite` (mypy/test/testtypes.py) plus Rust
  unit tests in `solve.rs` and `checkcall.rs`. Remaining known wall:
  `s_bound_sub` defers can shift into the inst/inst and cc/cc engine
  buckets on later audits.

- constraints defer round 3 (issue #1260): three ports in the
  `infer_constraints` engine plus wire plumbing. (1) Alias upper bounds:
  `applytype.rs`'s bound check now expands a `TypeAliasType` upper bound
  through the alias snapshot (also feeds `gt-alias`), so `class C[X:
  Alias[T]]` infers natively. (2) Protocol template arms: ported
  constraints.py's SUPERTYPE_OF structural-protocol dispatch
  (`visit_instance_protocol_supertype_native` +
  `infer_constraints_from_protocol_members_native` in `constraints.rs`),
  with the `inferring` recursion guard mirrored as a Rust RAII stack
  (`ProtocolInferringPush`), so an Instance-vs-protocol-template call
  decides through the parity-tested `is_protocol_implementation` engine
  plus `get_protocol_member_inner`; protocol-left shapes still defer
  (the `assuming` guard is engine-local). Measured: whichever side is
  the protocol decides natively, `inst-protocol-template` 2491 -> 0.
  (3) `type[T]` template vs a type-object callable: threaded
  `erase_types` through `infer_constraints_full_inner` ->
  `infer_constraints_dispatch` -> `visit_type_type_native` (FFI
  `rust_infer_constraints_full` gains an `erase_types` flag; Python
  `mypy/constraints.py:159` passes it; `suitable_item`-style overload
  callers keep `False`), and `visit_type_type_native` now takes a
  `CallableType` actual with `is_type_obj()` true (wire `instance_type`
  or proper `ret_type`, `erase_typevars_inner` under the flag, recurse).
  Rebasing over #1261 (issue #1259), which independently ported the same
  type-object arms, resolved in favor of #1261's variant: the merged
  `visit_type_type_native` decides Overloaded type-object actuals too
  (mine deferred) and erases with `AnyType(TypeOfAny.special_form)`
  (mine used the unmodeled `make_any()` variant); this PR's `erase_types`
  threading and alias-`ret_type` expansion are what both arms ride.
  The sgc-audit numbers below were measured pre-rebase, on a tree
  without #1261's overlapping ports: `tt-actual-callable` 458 -> 0,
  `infcon-defer` 454 -> 302, RESULT_OK 10,840 -> 10,992 of 11,420
  (+3.6%, ~96% of rust_solve_generic_call engine calls now native).
  Audit instrumentation (`audit_probe.rs`, sgc/defer logging) was
  stripped before landing; post-rebase gates re-verified green (cargo
  tests 2399, parity and self-check counts in the PR description).
  Covered by 5 new Rust
  unit tests in `constraints.rs` (`test_tt_actual_*`) plus the
  reworked `NativeConstraintsDeferralSuite` defer tests in
  `mypy/test/testtypes.py` (now parity-checked under both
  `erase_types` values). Remaining top buckets:
  `inst-protocol-both` 242 (protocol-vs-protocol structural join),
  `union-normalize-fail` 97 (ambiguous), `inst-callable-vs-protocol`
  76, `ap-target` 74 / `apply-defer` 77 (applytypes tail),
  `gt-bound-fail` 59 (needs a subtype-callback channel).

- `rust_dangerous_comparison` alias + container-ref round (issue #1342) —
  the seam mirrors the branch-for-branch decision head of
  `ExpressionChecker.dangerous_comparison` (checkexpr.py) on wire types;
  covered by `NativeDangerousComparisonSuite` in
  `mypy/test/testtypes.py` (gate-off vs gate-on parity plus direct seam
  calls). Two extension ports from the #1342 defer audit: (a) alias
  operands expand through the alias snapshot at entry and again after
  `remove_optional`, which is an alias-aware port of
  `types_utils.remove_optional` (dc-alias 61 -> 0, dc-remove-opt 8 -> 0);
  (b) the Python shim resolves the `typing.AbstractSet` /
  `typing.Mapping` fullnames via `chk.lookup_typeinfo` (KeyError defers)
  and passes them into the seam so the Rust set/Mapping item recursions
  decide without crossing back to Python (dc-setref 63 -> 0, dc-mapref
  19 -> 0). The suite's fixture Infos stamp their `TypeVarId` namespaces
  (`make_type_info` leaves them empty and the native mapping keys on the
  namespace). Remaining buckets: dc-final-overlap 67 (overlap kernel,
  tracked elsewhere), dc-lit 2, dc-map 1.

- `rust_analyze_instance_member_access` survivors-gate widening +
  `narrow_declared_type` TypeType/Callable tails (wave32, issue #1346).
  The static-tail survivors gate now lets receiver-argument tvars ride
  through at any meta level: `collect_tvar_keys` seeds the allowed-key
  set from the mapped receiver's arguments, and the four IAMA decode
  sinks (plain, callsite, generic, union-member paths in
  `mypy/checkmember.py`) re-link decoded rider occurrences onto the
  live mapped-arg variables via the new `resync_receiver_arg_tvars`
  pass (wirefixup), restoring Python's identity semantics across the
  wire round-trip (wave32 equivalent of the #1286 trivial-self
  repair). Variables-entry leftovers still defer. `narrow_declared_
  type` retires the blanket TypeType and CallableType defers:
  both-TypeType pairs meet their items and re-normalize through
  `make_normalized_typetype` (TypeForm flags AND, union items split
  per item), the TypeType-vs-metaclass-Instance arm consults the
  resolver snapshot (`has_base("builtins.type")`, `abc.ABCMeta`,
  `fallback_to_any`), and tvar-carrying callable ret pairs narrow the
  ret through `narrow_rec` while plain-ret pairs hit the meet.py:420
  default. Covered by six new parity + seam-engage pins in
  `NativeMeetDeferralSuite` (TypeType pair, TypeForm-vs-metaclass,
  callable ret G[A] vs G[B]) plus 6 Rust unit tests in `meet.rs`.
  Measured vs baseline probe: IAMA 331 calls / 217 fallbacks (66%
  defer) -> 124 / 6 (5%), `narrow_declared_type` 28934 / 224 (1%) ->
  29050 / 136 (0.5%). Residual audited ndt buckets are meet-kernel
  decisions tracked elsewhere (db-final-overlap 67, dc-lit 2,
  dc-map 1); the fifth IAMA decode sink (top-level ama decode) was
  deliberately not relinked, its recipients are Rust-created fallback
  Instances.

- setops join-kernel alias closure plus the `handle_recursive` gate
  (wave33, issue #1356). The join seam expands a top-level
  `TypeAliasType` operand at `join_types` entry: `proper_top`
  chain-resolves one proper-type step through the alias snapshot
  (snapshot miss, cycle, or bad shape defers the whole call,
  mirroring join.py:505 `get_proper_type`). To feed that walk the
  `TypeResolver` carries a shared alias-snapshot view
  (`install_aliases` / `aliases`), re-installed by
  `NativeTypeResolver` at build time and after each alias insert, so
  engine functions reached with only `&TypeResolver` can expand
  alias nodes without a second resolver argument. The tuple-vs-tuple
  join no longer yields `fbNone` when both fallbacks join
  (vj_fbnone:Ttuple+Ttuple 13 -> 0; rjt_defer 7 -> 1, py_jt_defer
  7 -> 1, jt:fix:2:2 288 -> 296). `setops` gains an alias-aware
  step-1 union flatten (`flatten_alias_union_items`, recursive-alias
  cut in item order; defers on missing snapshot, cycle, or an
  unsubstitutable shape), gated behind the new 6th `expand_aliases`
  parameter of `make_simplified_union_expanded`: the only Python
  caller passing `handle_recursive=False` (`tuple_fallback`,
  typeops.py:392) defers alias-bearing lists to Python at the
  Rust-internal call site instead of expanding a recursive alias
  inside the kernel. That gate is the wave33 parity regression fix:
  the ungated fallback expanded a recursive tuple alias inside
  `join_types -> tuple_fallback -> make_simplified_union ->
  is_subtype`, an msu/is_subtype/tuple_fallback cross-entry loop
  with no active cut, segfaulting the fine-grained
  `testRecursiveTupleFallback*` and
  `testTypeAliasUpdateNonRecursiveToRecursiveCoarse/Fine` cases and
  the cold self-check (lldb repro script kept at `/tmp/w33_lldb.cmd`).
  Regression unit tests in setops.rs:
  `msu_expand_aliases_false_defers_on_alias_item`,
  `msu_expand_aliases_false_still_flattens_plain_unions`,
  `tuple_fallback_recursive_alias_tuple_defers`,
  `msu_alias_flatten_defers_on_union_of_self_regress`. Ground rule:
  the msu seam boundary keeps the wave32 always-expand shape
  (`expand_alias_items`) because Python expands non-recursive
  aliases in both `handle_recursive` modes (types.py:5109 exempts
  only `t.is_recursive`); the three committed
  `NativeAliasExpansionSuite` calls caught the first, over-broad
  draft of the gate. Audit buckets (before -> after3, instrumented
  cold self-check): vj_fbnone:Ttuple+Ttuple 13 -> 0, rjt_defer 7 ->
  1, py_jt_defer 7 -> 1, vj_tupdef 2 -> 2, jt:fix:2:2 288 -> 296;
  the tfn pool re-partitions, tfn_union 107 -> 44 plus new tfn_flat
  81 / tfn_dedup 96 / tfn_dedupk 43, so the tuple-fallback defers
  are now named per step-1/step-3 shape instead of one blob.
  py_jtl_rdefer 35 / py_jtl_nat 22 unchanged. Remaining walls:
  jl_rec and join_one_variadic, semantics-checked as equivalent to
  their Python callers, leftover defers stay in Python. Noted, not
  new and not fixed here: the ast_serialize
  `serializes_trivial_call_like_existing_binary_contract` failure
  is a baseline item tracked in #1273.
- `unify_generic_callable` non-generic-right kernel port (wave37,
  issue #1426, `crates/type_kernel/src/unify.rs`): the solve
  `rust_solve_generic_call` defer bucket for a target callable whose
  right side carries no type variables (`unify_generic_callable`'s
  non-generic-right path, `mypy/solve.py`) now decides natively. Rust
  drives `constraints.py` `infer_constraints_full_inner` on wire
  callables with the `no_extra_tvar_shape` gate (`unify.rs:140`):
  it defers any actual tree with a `variables`-carrying reachable
  callable (`callable_with_vars_reachable`, `visitor.rs:336`),
  because the kernel has no `extra_tvars` channel and Python's
  ambient `type_state.infer_polymorphic` (True in ordinary checking,
  checkexpr.py:1325) attaches extras at any such node. Under
  `old_type_inference=True` the port is parity by construction (the
  ambient flag is exactly the hardcoded False); no fixture exercises
  it. The `infer_unions = false` defaulted FFI param was added to
  `rust_is_subtype` / `rust_is_subtype_batch` /
  `rust_callables_compatible` / `rust_are_parameters_compatible`
  (stub edits in `stubs/type_kernel.pyi`), threading ambient
  `type_state.infer_unions` from the Python seams (`mypy/meet.py`
  via `rust_are_parameters_compatible`, `mypy/subtypes.py` `_is_subtype`
  batch/single and the visitor callable-compat seams) so pairs
  differing only in that flag never share a batched answer. Kernel
  dispatch defers `Parameters` templates entirely, so the
  constraints.py:1339/1343 ambient-branch divergence is unreachable.
  Audit (env-gated, stripped before landing): the biggest-worker
  `cc|cc` bucket went from 236 (split `1|1`:217 / `1|0`:19) to 217
  all-`1|1` with the `1|0` count at 0, so ~13 of the 19 non-generic
  right pairs retired natively; the 217 generic-right pairs stay
  deferred by design pending a kernel `extra_tvars` channel.
  Covered by `NativeCallableUnifyPreludeSuite` in
  `mypy/test/testtypes.py` (seam-engagement, now patching
  `rust_is_subtype` and expecting one call in both the unify-success
  and unify-failure blocks) plus 19 new Rust unit tests
  (11 gate-walker descents in `visitor.rs`, 8 `no_extra_tvar_shape`
  / `strip_ret` in `unify.rs`).
- st/icf residual defer audit, floor decisions (wave 47, issue #1455)
  documentation-only close, zero code change. Env-gated shape audits
  on the `rust_is_subtype` (st) and `rust_infer_constraints_full`
  (icf) seams (`MYPY_TK_ST47_AUDIT` / `MYPY_TK_ICF47_AUDIT`,
  instrumented cold self-check at `-n0 --no-incremental`; the ini's
  4 workers killed buffered audit writes, lesson recorded), stripped
  before landing (tree verified bit-identical to origin/main
  `f245a14d9`; baselines at that rev: cargo 2699/11, testtypes
  3201/6, testcheck 8144/69/7, cold self-check clean). Result: st
  335 / icf 266 residual defers out of ~190k native engine
  decisions, every bucket classified with origin analysis:
  - st nested `TypeAliasType` in tuple/union args (~150): the
    dominant wall; the Rust leaf is not pinned (needs a Rust-side
    leaf-reason audit plus an alias-expansion port, the wave-33
    segfault risk class). Follow-up issue opened. RETIRED in wave 50
    (#1457), see below.
  - st `I(<fake subclass>) -> I(mypy.nodes.Statement/SymbolNode)`
    24x: runtime-synthesized fake `TypeInfo`s from
    `mypy/checker.py:8067` `_make_fake_typeinfo_and_full_name`
    (truthy-type checking area; other call sites 4316 `<dummy>`,
    8139, 8421 typing.Collection) are missing from the resolver
    snapshot, so `visit_instance_nominal` defers
    (`left_snap is None`, subtypes.rs:2706) while Python answers
    via nominal `has_base` (True). Fix path: register synthesized
    `TypeInfo`s into the `NativeTypeResolver` snapshot at creation
    (per-build invalidation per #1137/#1146 discipline) or a
    live-PyO3 nominal classifier seam. Follow-up issue opened.
  - st `I(builtins.type[...]) -> C[...]` 16x: the Python head at
    subtypes.py:1326-1333 (`find_member("__call__", left,
    is_operator=True)` -> `_is_subtype(call, right)` / False) left
    unported: synthesized `type[...]` instances plus operator
    find_member semantics need a deeper read; floor.
  - st floors unchanged: serfail `TypeGuardedType` 9, extra_attrs
    carriers, ParamSpec callables, owned-tvar/extra_tvars generic
    callable pairs (no kernel channel), LKV tuple items.
  - icf 266 (SUBTYPE_OF 136 / SUPERTYPE_OF 130; constants
    constraints.py:617-618): defaultdict-init family ~40,
    Generator/SupportsNext family ~105 (Instance template with
    tvar args vs protocol Instance, Python's `visit_instance`
    protocol-member arm at constraints.py:1405+), Overloaded-actual
    ~20, ParamSpec floors. The protocol-member constraints engine
    plus the ambient `infer_polymorphic`/`extra_tvars` channel is a
    multi-wave effort (same class as the #1426 unify port); floor.
- st nested-alias leaf pin + port (wave 50, issue #1457): the
  dominant wave-47 st wall is retired. Env-gated leaf-reason audit
  on the cold self-check (`MYPY_TK_ST50_AUDIT`, denoised shape dump
  at the `rust_is_subtype` FFI entries) pinned the defer at
  `expand_top_aliases` (`subtypes.rs`): a no-args, no-tvar alias
  occurrence (mypy.cache.JsonValue, astdiff SnapshotItem/Primitive,
  test.data FileOperation) ran the full `expand_type_inner`
  substitution pipeline, whose union arm flattened the target and
  deferred on a top-level alias item (no FlatAliasGuard in that
  context): 372/632 whole-call st defers on the wave-50 tree.
  Fix: mirror Python's `get_proper_type` (types.py:4181-4197) — a
  no-args, no-tvar occurrence returns the RAW target
  (`copy_modified(args=[])`), keeping nested alias refs in place;
  the while-loop still stops at the first non-alias root and the
  depth cap bounds chains. Result: 632 -> 260 st whole-call defers
  (-372), alias bucket 0. Registered side effect (wave-33 risk
  class): with the alias operand now expandable inside the subtype
  engine, the native join's `visit_union_join` / both-union merge
  became reachable for recursive-alias content and re-derived the
  deformed expanded shape (`testRecursiveAliasesJoins`). Fix: a
  flag-only `contains_recursive_alias` walk (`is_recursive` on the
  wire node) defers those two join paths to Python, which keeps
  recursive alias nodes in joined unions (its is_recursive_pair /
  assumption machinery). Guardrail set green: testRecursiveTuple-
  Fallback1-5 + testTypeAliasUpdateNonRecursiveToRecursive (Coarse +
  Fine) + full fine-grained (747) + testcheck exact 8198/15/7 +
  testtypes 3251/6 + cold self-check clean. Python-side pin updated:
  `NativeSubtypesDeferralSuite::test_recursive_alias_gate_parity_no_
  wrong_verdict` now asserts the direct seam answers True (Python
  parity via the ALIAS_ASSUME guard) instead of None. Residual st
  buckets unchanged from the wave-47 table: untagged floors ~178
  (callable-compat generic/owned-tvar/extra_tvars walls, TypeType,
  protocol-member, serfail families), synthesized-TypeInfo
  `inst:left_snap_missing` 74 (sibling #1456 resolver-snapshot
  territory), small union/tuple propagation tails 8.
- sgc/ct embedded defer audits, floor decisions (wave 48b, issue
  #1462) documentation-only close, zero code change. Env-gated
  defer-bucket audits on `solve_generic_call_core` (sgc) and
  `conditional_types_inner` (ct) (`MYPY_TK_SGC1462_AUDIT` /
  `MYPY_TK_CT1462_AUDIT`, instrumented cold self-check at
  `-n0 --no-incremental` with `MYPY_NUM_WORKERS=0`; the atexit
  dump must use `os.write`, `hard_exit` flushes stdout then
  `os._exit`s and drops buffered output), stripped before landing
  (tree verified bit-identical to origin/main `135402e70`). Result:
  sgc 253 / ct 29 embedded defers, every bucket a documented
  engine/wire/emission floor; no decidable port in either seam.
  - sgc 253 (wrapper 9,086 calls @ 92.4% native; the `#1460`
    survey count 251 reproduces as 253): `infer_constraints_defer`
    173 (the icf engine walls ride the wave-47 icf taxonomy
    unchanged: defaultdict-init ~40, Generator/SupportsNext ~105,
    Overloaded-actual ~20, ParamSpec floors; the SUPERTYPE_OF
    formal/actual pairs are already proper-expanded at sgc entry);
    `apply_generic` 69 (`rust_apply_generic_arguments` None:
    sub-audit shows `gtt_bound_report` 90 + `gtt_nomatch_report`
    12 across all `skip_unsatisfied=false` callers, is_subtype
    decided false and Python must emit the "cannot infer type
    parameter" error, an error-emission side effect, not portable,
    plus `ap_expand_args` 7 expand-type-inner walls; the same
    apply path is shared with the wave-37/40 `unify.rs:580`
    caller); `solve_defer` 7 (gen_solve floor, #1430 precedent);
    `multi_lower_fnlike` 4 (wire loses nested FuncDef definitions
    on a multi-lower join, documented in-code). Wrapper-level
    floors: `lam_typevar` 463 (lambda arg whose context type
    carries a callee tvar needs Python's two-pass inference, a
    full algorithm not a decision; the biggest single sgc gate),
    `need_refresh` 90 (ParamSpec/TVT callee, by design), rust-None
    140, has_rec / dict_kwargs / deserialize 0.
  - ct 29 (re-pinned on the current head; the `#1460` "ct (50)"
    number is stale, the wave-46b 29 stands):
    `concrete_sub_undecided` 14 (is_subtype None in the
    concrete-proper-subtype branch; st floor classes. The branch
    must not fall through to the narrowing tail on an undecided
    verdict: Python's pure `is_proper_subtype` could decide true
    and the results diverge);
    `overlap` 13 (meet overlap-kernel floors: alias snapshot /
    cycle, TypeVar shapes, the #1346 wave audit's
    db-final-overlap class); `restrict_away_2` 2 (st/restrict
    multi-step floor). Wrapper 8,659 @ 99.76% native; rust-None
    21, wire-decode AssertionErrors 33 (checker-state dependent).
  Resolution per #1455 precedent: one docs-only PR closes the
  issue with the bucket tables; the next wave must not re-derive
  the sgc/ct strata; any sgc/ct work is icf engine, st engine,
  meet overlap, or emission-channel porting, not seam-local.
- errors render-bundle legacy-path audit, negative close (wave 52,
  issue #1479) documentation-only, zero code change. Env-gated
  call-count audit of the errors.py:1118-1323 render bundle
  (`MYPY_TK_B6_AUDIT`, report rewritten to the env path on every
  bump so `hard_exit` cannot strand it) on the cold self-check at
  `-n0 --no-incremental` plus an error-heavy synthetic corpus
  (1,600 lines, 1,200 errors), stripped before landing (errors.py
  verified bit-identical to origin/main `e594a8bd7`, zero probe
  leftovers). Result: the B6 legacy paths have zero measured
  traffic on the gate corpus; no port earns its parity burden; the
  4 deleted #1459 seams stay dead (YAGNI).
  - Cold self-check (347 files, clean): the pipeline misses by
    construction - `file_messages` 347 calls all early-return
    (`path not in error_info_map`, zero entries), `sort_messages`
    / `sort_within_context` / `remove_duplicates` / `render_messages`
    / `create_errors` / `new_messages` 0 calls; `format_messages`
    347 @ 0 tuples, all served by the already-ported green path
    (`rust_format_messages_default*` 347 FFI crossings @ 0
    entries); `remove_path_prefix` 347 (via `simplify_path`). The
    run's real errors-side work is report-side: 5,081 throwaway
    `Errors(Options())` constructions from
    `semanal.isolated_error_analysis` (type-expression probing)
    with 2,697 records discarded into them - ~4.1us each (~21ms,
    0.26% of the run), measured: noise, not a finding, no issue
    filed.
  - Error-heavy corpus (1,200 rendered): `sort_messages` 1 @ 1,200,
    `sort_within_context` 1,200 @ 1,200 (per-position-group runs),
    `remove_duplicates` 1 @ 1,200 / 0 removed, `render_messages` 1
    @ 1,200 (+800 local-context notes with `--show-error-context`),
    `remove_path_prefix` 2, `format_messages_default` 1 @ 1,200.
    Per-file once, sub-millisecond, orders below the report-side
    message building (`records` 2,060 incl. discarded).
  - Taxonomy / floors: `sort_messages` + `sort_within_context`
    portable-in-principle (wire-safe fields; a permutation-back
    boundary preserves live-object identity) but no corpus value: a
    kernel seam would add FFI cost to 347 empty self-check calls
    (the measured case) and per-entry ErrorInfo serialization beats
    the Python tuple-key sort on error runs; `remove_duplicates`
    floor - `parent_error` live-object identity (`ErrorInfo.write`
    asserts `parent_error is None`), dedupe semantics break across
    the wire; `render_messages` floor - note-string parity (message
    TEXT is the testcheck exact-match risk), `simplify_path` /
    `show_error_context` live-state dependence; `create_errors`
    floor - `--output` formatter path only, 0 calls on every gate
    corpus (the deleted #1459 seam must not be re-created).
  Resolution per #1455/#1458/#1469 precedent: docs-only negative
  close. Next-wave rule: any B6 work is report-side (ErrorInfo
  construction / message-string building in `mypy/messages.py`),
  not the render bundle; render-bundle seams need a corpus that
  exercises them before a port is warranted.
- `rust_classify_type_range` (issue #1464 C1, mypy.checker) — mirrors the
  leaf-decision dispatch of `TypeChecker.get_type_range_of_type`
  (checker.py:10408). The `TypeVarType` upper-bound unroll and the
  `UnionType` item fold are structural recursion and stay Python-side; the
  *leaf* decision (which branch a non-union, non-typevar proper type hits)
  is ported here as a zero-wire classifier. Rust reads the live proper type
  via PyO3 (`FunctionLike` + `is_type_obj`, the `TypeType` item shape
  (`NoneType` / `Instance` `is_final`), `AnyType`, and `Instance`
  fullnames) and returns a branch tag plus the `TypeType` `is_upper_bound`
  arbitration. Tags FN_TYPEOBJ (1) / TYPETYPE (2) / ANY (3) / BUILTINS_TYPE
  (4) / TYPES_UNION (5) / SPECIAL_FORM (6) / REST (7); REST rides the
  Python-side `is_subtype(builtins.type, typ)` gate (already native via the
  subtype resolver) and the can't-conclude `None` tail. The
  `fill_typevars_with_any` / `erase_typevars` tail stays Python-side. Defers
  (`None`) only on an unreadable attribute; every reachable leaf is
  classified. Gated by `_native_checker_active` (existing wiring, no build.py
  change) and covered by `NativeTypeRangeSuite` in `mypy/test/testtypes.py`
  (direct seam tag tests for all eight leaves plus a gate-off vs gate-on
  differential on the captured `(item, is_upper_bound)` pair, REST exercised
  with a `named_type` override), plus 12 pure decision unit tests in
  `type_range.rs`. Cold self-check: 9286 `get_type_range_of_type` calls, 100%
  reaching the head; post-port the seam decides the 9264 non-union leaves at
  100% native (the 20-union item fold stays Python-side recursion).
- `rust_classify_typeobj_gate` (issue #1464 C2, mypy.checkexpr) — mirrors the
  typeobj-fail gate head of `ExpressionChecker.check_callable_call`
  (checkexpr.py:2941): the `if`/`elif` that fires
  `CANNOT_INSTANTIATE_PROTOCOL` for a protocol type-object and
  `cannot_instantiate_abstract_class` for an abstract type-object (not
  exempt by `fallback_to_any`), both suppressed by `from_type_type` (the
  `Type[...]` exemption). The original double-evaluates `is_type_obj()` (once
  in the `if`, once in the `elif`) and re-runs `type_object()` per arm; this
  port collapses that to one `is_type_obj()` (short-circuiting the
  `type_object()` call for the ~89% non-type-object callees) and one
  `type_object()` and returns an arm tag NONE (0) / PROTOCOL (1) / ABSTRACT
  (2). The two `fail`s and the `can_return_none` abstract-attribute fold stay
  Python-side. Defers (`None`) only on an unreadable attribute or a
  `type_object()` assertion. Gated by `_native_checkexpr_active` (existing
  wiring, no build.py change) and covered by `NativeTypeobjGateSuite` in
  `mypy/test/testtypes.py` (direct seam tag tests across every arm plus a
  gate-off vs gate-on differential on the captured protocol/abstract fails
  through the real `check_callable_call`), plus 9 pure decision unit tests in
  `checkcall_typeobj.rs`. Cold self-check: 167343 `check_callable_call` calls
  (88.9% not_typeobj, 11.1% typeobj_none); post-port the seam decides 167437
  gate calls at 100% native.
- `rust_expand_type_by_instance` residual defer audit, floor decisions
  (wave 53, issue #1483): documentation-only close, zero code change.
  Env-gated FFI audit of the by-instance expansion seam plus a Python
  shim probe (`MYPY_TK_XB_AUDIT`, instrumented cold self-check at
  `MYPY_NUM_WORKERS=0 --no-incremental`, stripped before landing; tree
  verified clean: cold self-check 0 errors). Result: 669 FFI calls @
  92.5% native (619 OK / 50 DEFER = 49 `snap-miss` + 1 `call-unpack`).
  - The issue's working hypotheses are DISPROVEN: the #1203 union-arm
    alias flatten is already live on the by-instance path
    (`rust_expand_type_by_instance` installs `FlatAliasGuard` at the
    FFI entry; zero `fa-*`/`union-flat` buckets in 669 calls; pinned by
    `ebi_relink_flattens_alias_union_item` + the flatten unit tests),
    and the #1215 relink contract is threaded (`relink_ok=true` via
    `expand_type_by_instance_relink`; the shim runs
    `resync_var_identities` + `_resync_definitions`). No alias buckets
    exist in this corpus.
  - `snap-miss` 49 (callers: `typeops.type_object_type_from_function`
    26x, `maptype._native_map_step_frontier` 13x,
    `checkmember._analyze_member_access` 10x): the instance class is
    absent from the Rust `TypeResolver` snapshot at expand time. Two
    sub-classes, both resolver-architecture walls, no expandtype.rs
    decision exists (every missed instance is a generic class whose
    env binding requires the snapshot's `defn.type_vars` raw_ids; the
    wire instance carries args but no tvar-keying, so a snapshot-less
    fast path could not keep parity):
    - 39x: the class IS in the build's live `_native_typeinfo_map` at
      defer time but not yet in the Rust snapshot (`in_typeinfo_map=
      True`, `in_rust_dict=False`, snap<map, e.g. snap=908 map=926 for
      the itertools batch; `operator.itemgetter` observed entering the
      snapshot mid-run). Mechanism is the resolver's snapshot-
      population timing gap (#1456 family), not the ctor-blob window:
      an env-gated FFI/seam probe on the wave-53 cold self-check
      (stripped before landing, #1484) counted ZERO expand/map seam
      entries or FFI crossings inside `_native_ctor_blob` (build.py,
      issue #1298) windows even with all gates active, so the 49 were
      whole-run events in normal checking phases. The blob still
      clears the typeops/expand/maptype gates for its duration
      (wave-53 #1484, extending the wave-22 #1324 typeops gate): the
      installed resolver is stale at blob time, so any future blob
      chain reaching the expand/map seams would round-trip doomed FFI
      against the missing fresh class.
    - 10x `functools._SingleDispatchRegisterCallable`: a TypeInfo
      synthesized directly by
      `mypy/plugins/singledispatch.py:make_fake_register_class_instance`
      (never enters `self.modules`, bypasses the #1456
      `make_fake_typeinfo` registrar funnel, absent from the live map
      too). Tracked by #1485 (extend the #1456 registrar pattern to
      plugin-synthesized infos).
  - `call-unpack` 1: a callable signature carrying a bare UnpackType
    var-arg (expandtype.py:482-488 `interpolate_args_for_unpack`, a
    documented not-ported branch); single cold-self-check hit.
- st callable-compat wall: find_member fetch + Unpack arm + arg merge
  (wave 55, issue #1491) — the wave-47 standing floor. Env-gated
  leaf-reason audit (`MYPY_TK_W55RUST_AUDIT`, instrumented cold
  self-check at `-n0 --no-incremental`, stripped before landing; tree
  verified clean) ranked the `rust_is_subtype` embedded defers 112 ->
  **61** events (-51, -45.5%). Landed ports:
  - Instance-left / FunctionLike-right `find_member("__call__", left,
    left, is_operator=True)` now runs plain find_member semantics in
    `get_protocol_member_inner` (`find_member_semantics=true`): the
    precise-metaclass NoneVal and the extra_attrs prelude defer are
    skipped on this path, `member_miss_decision` mirrors the operator
    miss tail (no accessor scan; fallback_to_any Any; extra_attrs hit;
    else None), and the caller maps NoneVal to `Some(false)`. Retired
    the 31 `I(builtins.type|types.FunctionType|types.BuiltinFunctionType|
    functools.partial) -> CallableTypen` fetch defers.
  - `visit_unpack_type` (subtypes.py:1416-1422) ported: Unpack-vs-Unpack
    recurses on targets, builtins.object accepts, else False. Retired
    the `UnpackType -> I(builtins.int)` and tuple-item defers (the
    variadic helper's fixed-length fallthrough now decides).
  - `callable_corresponding_argument` merge subset
    (typeops.py:1190-1201): the optional pos-only/name-only merge gate
    is answered with a resolver-free `meet_types` subset (identical
    proper types via `py_type_eq`, Any absorbs, Uninhabited bottom);
    the non-merge gate answers by_name. Retired the 9
    `CallableTypen -> CallableTypen` `apc-corr-merge` defers and the
    adjacent join var-arg defer (SameS natively).
  - `APPLY_REPORTED` thread-local (applytype.rs) + `unify.rs` mapping:
    `get_target_type`'s `skip_unsatisfied=false` report arms are
    Python's `had_errors -> unify None -> is_callable_compatible False`
    verdict, now answered NoUnify instead of deferring. Retired the 13
    `ud-target-report-{bound,values}` events.
  - Remaining floors (61 = 19 serfail + 42 kernel): 18 `uds-var-unsafe`
    (the named owned/meta-tvar wire-identity wall, solve.rs
    `wire_unsafe_reason` on the solved target — needs the fuller
    owned-tvar/extra_tvars channel); 10 `cbd-expand-other` (ctor-blob
    expansion of `type[X]` callables outside the snapshot/alias
    buckets); 9 `ud-infer-args` (constraints engine
    `infer_constraints_full_inner`); 3 `pip-sub-fetch`/`gpm-method-none`
    (protocol member method binding); 1 `cb-proto-typeobj` (type-object
    `is_protocol_implementation(class_obj)`); 1 `ud-tvar-clash`
    (freshening); 19 serfail (Union operands raising in
    `_serialize_type` before the FFI). Gates: cargo 2736/11, testtypes
    3279/6-skips (+2 NativeFindMemberCallFetchSuite), testcheck
    8198/15/7 exact, cold self-check clean 347; clippy -D warnings +
    fmt clean.

- maptype timing-gap residual, floor decision (wave 55, issue #1490):
  documentation-only close, zero code change. Env-gated probes
  (`MYPY_TK_W55_AUDIT` shims on the maptype/typeops/expand fall-throughs,
  instrumented cold self-check at `-n0 --no-incremental`, stripped
  before landing; cold self-check 0 errors) re-measured the #1483
  snap-miss class at the post-#1484/#1485 head. Result: only 5
  residual events remain, all in `_native_map_instance_to_supertype`,
  all `in_map=True in_snap=False` with snap<map
  (`email.message.EmailMessage -> email.message.Message`,
  `ctypes.wintypes.WPARAM/LPARAM -> _ctypes._SimpleCData`,
  `unittest.runner._TextTestStream -> _typeshed.SupportsWrite` x2).
  So 44/49 of the #1483 audit is closed by #1484 (ctor-blob gate
  clearing) + #1485 (plugin registrar).
  - The residual window is inherent to the #1115 design:
    `_install_semal_wirefixup` publishes a module's TypeInfos to the
    Python wire map at its top-level completion, while the Rust
    snapshot is only fed per SCC post-semanal, so a semanal-time seam
    inside the class's own SCC sees the map grow before the snapshot
    does.
  - Fix candidates evaluated and rejected. (a) On-demand sealing via a
    maptype registrar (the #1456/#1485 pattern: seal the class plus its
    missing MRO chain, retry once) restores all 5 natively but regresses
    4 PEP 695 variance tests (`testPEP695InferVarianceWithInheritedSelf`,
    `testPEP695InheritInvariant`, `testPEP695InheritanceMakesInvariant`,
    `testPEP695InheritCoOrContravariant`): a mid-SCC class can carry
    pre-inference variance (`__main__.Subclass.T` read COVARIANT at
    registration and becomes INVARIANT after `infer_class_variances`),
    and first-seal pins it. Guards that make sealing safe (skip
    current-SCC modules, `Name@line` function-local classes, and
    `VARIANCE_NOT_READY` tvars) drop every registration on the
    self-check (0 `MYPY_TK_W55_REG_DEBUG` hits), so the registrar is
    dead weight. (b) A Rust live-object mapping fallback (walk the live
    TypeInfo's MRO via PyO3 instead of the snapshot) is parity-safe by
    construction but a large kernel change for 5 calls/self-check.
    Decision: document the window as a floor; the Python
    `map_instance_to_supertype` fallback answers correctly.
  - The same audit classified the other 95 fall-through events as NOT
    timing-gap: 32 `typeobj:kernel_none` (Rust composite defer) and 60
    `typeobj:decode_None` where the wire payload references
    function-local class fullnames (`Name@line`, e.g.
    `mypy.traverser.Counter@1350`, `_pytest.pytester.reprec@1195`)
    absent from the wire map because `_collect_incremental` only walks
    module symbol tables. Tracked separately as #1493.

- typeobj wire-seam alias decode (wave 56, issue #1493): the #1490
  audit's `decode_None` diagnosis corrected and fixed. An env-gated
  `_TypeRefFixer` reason probe (instrumented cold self-check at `-n0
  --no-incremental`, stripped before landing; self-check clean 347)
  showed all 60 `typeobj:decode_None` events are ALIAS-caused, not
  missing TypeInfos: the Rust composite mirrors the pure-Python
  bind/map path, which preserves live `TypeAliasType` nodes, while
  the shim's `_deserialize_type` decodes with `resolve_aliases=False`
  and defers on every decoded alias (e.g. `ast._ConstantValue`,
  `logging._FormatStyle`, `_typeshed.AnnotationForm`).
  Zero events had an Instance `type_ref` absent from the wire map, so
  the issue's local-class registrar / seed-map hypothesis is disproven
  (and was dropped: dead weight).
  - Fix: `type_object_type_from_function` retries the decode once
    through `_deserialize_type_with_aliases`, the #1309/#1224 contract
    (re-link decoded aliases through the per-build alias map, re-unify
    fresh vars), then the existing `resync_var_identities` /
    definition-restamp tail runs unchanged. Uncached by design (fresh
    alias-arg vars must not leak across callers, #1180/#1198). No Rust
    changes, no snapshot/registrar work.
  - Audit before/after: `decode_None` 60 -> 0 (32 `kernel_none`
    unchanged); alias-signature parity pinned structurally (live alias
    node, not just `str`) by `NativeTypeObjectAliasDecodeSuite` in
    `mypy/test/testtypes.py`.

- astdiff type-snapshot builder, B7 slice 1 (wave 57, issue #1497):
  new `rust_snapshot_type(typ)` pyfunction
  (`crates/type_kernel/src/astdiff_snapshot.rs`) mirrors
  `SnapshotTypeVisitor` (astdiff.py:389-557) for every non-generic arm:
  simple types, UnboundType, Instance (lkv + extra_attrs), TypeVarType,
  ParamSpecType, TypeVarTupleType, UnpackType, Parameters,
  CallableType, TupleType, TypedDictType, LiteralType, UnionType,
  Overloaded, TypeType, TypeAliasType. Order-sensitive arms call
  Python's own `set`/`sorted` on the constructed tuples (union
  dedup+sort, extra_attrs pairs, TypedDict required/readonly) so the
  snapshot stays byte-for-byte the Python one. `snapshot_type` routes
  through the seam when `_native_astdiff_active`; deferrals (`None`)
  are generic `CallableType` (`normalize_callable_variables` needs
  `expand_type`), `PartialType` (Python raises RuntimeError,
  unchanged), a `TypeAliasType` without alias, unhandled kinds, and
  `PyAttributeError` only (#1466/#1468 contract). Measured on the
  fine-grained corpus (env-gated audit, stripped before landing):
  testfinegrained 75,713 calls @ 98.3% native (1,263 defers, 100%
  generic-callable); testfinegrainedcache 31,120 @ 98.05% (607, all
  generic-callable); testdaemon 631 @ 100%. Gates: cargo 2,736/11,
  testtypes 3,291/6 (+9 `NativeAstdiffSnapshotSuite`), testcheck
  8,198/15/7 exact, fine-grained 747/27, daemon 37, finegrainedcache
  549/229, cold self-check clean 347.
- astdiff symbol/definition snapshot builders, B7 slice 2 (wave 58,
  issue #1500): new `rust_snapshot_symbol_table(name_prefix, table)`
  pyfunction (`crates/type_kernel/src/astdiff_symbols.rs`) mirrors
  `snapshot_symbol_table` / `snapshot_definition` /
  `snapshot_untyped_signature` (astdiff.py:195-583), building the
  Python `dict[str, SymbolSnapshot]` in `table.items()` order. Node
  dispatch: MypyFile (Moduleref), TypeVarExpr, TypeAlias,
  ParamSpecExpr, TypeVarTupleExpr, then CrossRef vs definition by
  `get_prefix(node.fullname) != name_prefix`; the definition arms
  cover SYMBOL_FUNCBASE_TYPES (FuncDef / OverloadedFuncDef: `node.type`
  signature else `snapshot_untyped_signature`, impl / property
  setter_type / deprecated list / dataclass-transform spec), Var,
  Decorator (recursive), and TypeInfo (attrs tuple, recursively
  snapshotted nested table, sorted `(abstract)` entry). Type leaves
  reuse the slice-1 walk; when it defers (generic `CallableType`) the
  node is handed to the Python `astdiff.snapshot_type` callback so the
  enclosing symbol snapshot still completes natively (per-node
  fallback chosen over whole-call deferral; measured 0 whole-table
  defers). `find_dataclass_transform_spec` is called through the live
  Python function, keeping its own gate authoritative. Deferrals
  (`None`): unknown / `None` node kinds (Python asserts), an
  `UNBOUND_IMPORTED` kind, a non-str `fullname`, unreadable
  attributes; `PyAttributeError` only maps to a defer (#1466/#1468).
  Measured (env-gated audit on the fine-grained corpora, stripped
  before landing): testfinegrained 7,580 calls @ 100% native /
  0 defers; testfinegrainedcache 3,384 @ 100% / 0; testdaemon 97 @
  100% / 0. Gates: cargo 2,736/11, fmt + clippy clean, testtypes
  3,298/6 (+7 `NativeAstdiffSymbolSnapshotSuite`), testcheck
  8,198/15/7 exact, fine-grained 747/27, daemon 37, finegrainedcache
  549/229, testdiff 79, testmerge 41/1, cold self-check clean 347.

- fixed-format cache meta writer, B1/B3 slice 1 (wave 59, issue #1503):
  new `rust_write_cache_meta(meta)` / `rust_write_cache_meta_ex(meta_ex)`
  pyfunctions (`crates/type_kernel/src/cache.rs`) mirror
  `CacheMeta.write` / `CacheMetaEx.write` (cache.py:274/366) and their
  helpers on live Python objects: `write_errors`, `write_json`,
  `write_json_value` (tags / bare sizes / sorted str keys, list vs
  tuple preserved), plus the wire primitives `write_bytes`,
  `write_bytes_list`, `write_str_list`, `write_float_bare`, and
  `write_big_int` (arbitrary-precision via `BigInt::from_le_bytes`).
  Shim at `mypy/build.py:write_cache_meta` / `write_cache_meta_ex`:
  `_try_native_write_cache_meta*` (cache.py) engage when
  `fixed_format_cache` and `_HAS_TYPE_KERNEL and _native_cache_active`;
  on `None` the original `WriteBuffer` body runs, and the
  `bytes([cache_version(), CACHE_VERSION])` prefix stays Python.
  Deferrals: unsupported `plugin_data` / non-str dict keys /
  non-list-tuple sequences / bad field types; only `PyAttributeError`
  maps to `None`, other PyErrs propagate (#1466/#1468). Measured
  (env-gated audit, stripped before landing): cold self-check 808 meta
  + 808 meta_ex writes @ 100% native, 0 defers; warm incremental
  re-runs 0 writes (cache consumed). Gates: cargo 2,739/11 (+3 units),
  fmt + clippy clean, testtypes 3,309/6 (+11
  `NativeCacheMetaWriterSuite`), testcheck 8,198/15/7 exact,
  fine-grained 747/27, daemon 37, finegrainedcache 549/229, cold
  self-check clean 347, warm clean and cache-consuming. No cache
  format or CACHE_VERSION change.

- wave-60A engagement audit of the 0-40% native-share seams (wave 60,
  issue #1506): audit-first, zero ports. All 151 fallbacks across the
  12 survey seams were classified with env-gated shim + Rust leaf
  probes (`MYPY_TK_W60A_AUDIT`, stripped before landing; cold
  self-check, survey numbers reproduced exactly). No engagement/gate
  regression of the #1455 ifta class exists; every fallback is a
  documented partial-port wall or floor. Per seam (calls/native ->
  fallback buckets):
  `rust_analyze_typeddict_access` 14/0 -> 14 `not-delitem` (Rust ports
  only `__delitem__`; the corpus is `get` x10 + `__setitem__` x4, and
  `get` recurses into the native IAMA path) - scope floor;
  `rust_linearize_hierarchy` 6/0 -> 6 `snap-miss`, all fake TypeInfos
  whose `calculate_mro` (checker.py:8043) precedes the #1456 registrar
  (checker.py:8048), plus 2,981 semanal gate skips (`build.py:6002`
  clears resolvers per SCC by design) and 4,161 cached -> the MRO port
  has ~0 production coverage, no correctness impact (Python computes);
  `rust_covers_at_runtime` 3/0 -> 1 `erase-item` (Overloaded absent
  from `argapprox::erase_type`) + 2 `subtype-none` (Callable vs
  TypeType); `rust_restrict_subtype_away` 2/0 -> 2 `erase-parity`
  (`consider_runtime_isinstance=False`: the second
  `erase_instances=True` proper-subtype check is decidable but
  unlanded at 2 calls); `rust_filter_satisfiable` 4/2 -> 2
  `subtype-none` (`is_subtype(target, upper_bound)` - st engine
  floor); `rust_remove_dups` 41/15 -> 26 alias-guard
  (`decode_types_for_list_return` defers on any TypeAliasType; the
  wire cannot carry Python's `alias ==` object identity);
  `rust_analyze_member_method` 27/3 -> 24 = 19 `bare-recv-tvar`
  (#1214 bare-Self vs tvar-receiver wall) + 5 `freeze`
  (`EnumMeta.__iter__` env miss); `rust_infer_directed_arg_constraints`
  30/6 -> 24 `infer-none` (18 `SupportsKeysAndGetItem` vs
  `Union[Callable, None]`, 6 `str` vs `Union[SupportsIndex, None]`;
  the wave-47 icf protocol/union engine class); `rust_join_instances`
  28/11 -> 17 = 8 via-supertype nested sub-join + 6
  same-type-with-args (`visit_instance_with_args`) + 3 nominal
  args-less; `rust_join_tuples` 7/3 -> 4 `fixed-item-join` (1 mypyc
  `SlotTable` alias pair, 2 `Mapping[..., Callable]` joins, 1
  `ModuleNotFoundReason ~ str`); `rust_meet_types` 17/11 -> 6
  `inner-none` (2 Tuple x Union, 4 Tuple x variadic Tuple);
  `rust_map_type_from_supertype` 52/29 -> 23 = 10 `expand` (ParamSpec
  `abstractclassmethod`/`staticmethod` wrappers + 4 Overloaded), 12
  `snap-miss` (pathspec Re2/Hyperscan classes absent from the Rust
  snapshot at `dataclasses.expand_typevar_from_subtype` time;
  #1490/#1486 timing-gap family), 1 `tuple-super`
  (`datetime._IsoCalendarDate` -> `builtins.tuple` maptype special
  case). Gates: docs-only (tree == origin/main + this bullet), cargo
  2,739/11, testtypes 3,309/6, testcheck 8,198/15/7 exact, cold
  self-check clean 347.
- wave 60B top un-audited defer buckets (issue #1507): audit-first port
  of five seams (env-gated defer-reason probes, stripped before landing;
  exact before/after measured with the survey proxy on the same
  revision).
  - `rust_is_singleton_identity_type` (typeops.py) 7,103 calls / 48 -> 0
    fallbacks: the `FunctionLike` arm deferred every callable; now a
    non-type-object callable is decided `false` and a type object
    resolves `type_object().is_final` natively (`get_instance_type` with
    `force_fallback=True`: `instance_type` or the proper `ret_type`, with
    the TypeVar upper-bound unwrap cascading into the Tuple / TypedDict /
    Literal fallback checks like Python's `if` chain) plus the live
    TypeInfo map (`is_final` is not snapshotted). Deferral remains only
    when the alias snapshot / live TypeInfo read is missing.
  - `rust_container_type` (checkexpr.py) 1,806 / 36 -> 17: decided-none
    protocol. A multi-item join Rust decides but that fails
    `allow_fast_container_literal` means Python's own
    `_first_or_join_fast_item` would return None after the same join;
    Rust now returns a Python `False` sentinel and the shim skips the
    duplicate pure-Python join (19 calls). Floors: 9 type-object
    callable joins (`join_similar_callables` `from_type_type` handling),
    7 join-engine-undecided pairs, 1 join-decided-but-allow-undecided.
  - `rust_format_type_bare` / `rust_format_type_distinctly`
    (messages.py) 353 / 9 -> 1 and 931 / 44 -> 37: the
    `format_type_inner` pretty path now delegates to the
    definition-free `pretty_callable_inner` when the shim proves every
    callable in the tree renders identically without its wire-dropped
    `definition` (`_pretty_wire_safe`: callable name present, no
    prepended self/cls), threading
    `reveal_verbose_types` / `pretty_wire_safe` through a per-call TLS
    guard (defaults keep unit tests deferring). Snapshot-missing
    Instances read the live TypeInfo `name` instead of deferring.
    Floors: 37 definition-dependent pretty callables, 1 instance absent
    from both the snapshot and the live map; the 25 `PartialType`
    serialization exceptions never reach the seam.
  - `rust_instantiate_type_alias` (typeanal.py) 5,862 / 68 -> 68: floor.
    Every deferred call is the bare-generic `set_any_tvars` fill with at
    least one defaulted alias tvar (`disallow_any=False, tvt=False,
    defaults=True`): gradual `expand_type(arg, env)` over live
    `tv.default`s plus `used_default` / note side effects, exactly the
    deferral the Rust module documented from the start.
  Pins: FunctionLike-arm (incl. the TypeVar-over-tuple cascade) +
  direct-engagement tests in `NativeCoerceLiteralSingletonSuite`,
  `_pretty_wire_safe` / definition-defer tests in
  `NativeMessagesDeferralSuite`, decided-none + type-object-defer tests
  in `NativeCheckexprJoinAndTupleSuite`, plus Rust unit tests for the
  `FastItemOutcome::DecidedNone` / `ContainerOutcome::DecidedNone`
  outcomes.
  Gates: cargo 2,741/11, fmt + clippy clean, cold self-check clean 347,
  testtypes 3,321/6 (+12 tests), testcheck 8,198/15/7 exact,
  fine-grained 747/27, daemon 37, finegrainedcache 549/229.

- wave 61A decidable leftovers (issue #1511): audit-first retirement of
  the wave-60A bucket leftovers (env-gated `MYPY_TK_W61_AUDIT` probes at
  the three seams, stripped before landing; cold self-check
  `-n0 --no-incremental` reproduced every bucket, then re-measured).
  - `rust_restrict_subtype_away` 2 -> 0 fallbacks: the
    `consider_runtime_isinstance=False` branch now runs Python's second
    check natively (`subtypes::is_subtype` with
    `SubtypeContext.erase_instances = true`) after the
    non-generic/non-protocol shortcut. Protocol-right pairs with a
    recorded base relation still defer (join-kernel floor).
  - `rust_map_type_from_supertype` 10 `expand` -> 0: (a) the seam
    installs `FlatAliasGuard` around the by-instance expand (the #1203
    alias-union flatten was live for `rust_expand_type`, missing here),
    retiring the alias-union cases (tempfile / subprocess /
    zip_longest / filterfalse / pathspec); (b) the ParamSpec-to-ParamSpec
    callable splice (`param_spec_callable_arm`) is restored, wire-safe
    because `ParamSpecType.id.meta_level` round-trips since #1417,
    retiring the staticmethod / classmethod / abstract* wrappers; (c)
    `interpolate_args_for_unpack` (plain `Unpack[Ts]`) + the
    unmatched-TVT `expand_unpack` default (`variables.get(id, t.type)`,
    expandtype.py:1097) + the `with_normalized_var_args` handoff retire
    operator.itemgetter. The 12 `snap-miss` + 1 `tuple-super` buckets
    stay the documented #1490/#1486 floors. Also fixes a latent
    `with_normalized_var_args` divergence: the TypeVarTuple branch
    pushed the bare `TypeVarTupleType` instead of Python's enclosing
    `UnpackType` (types.py:2608-2610).
  - `rust_covers_at_runtime` erase-item 1 -> 0: `argapprox::erase_type`
    ports `visit_overloaded` (`items[0].fallback`, erasetype.py:218-219);
    the 2 `subtype-none` calls stay the documented floor.
  - Pins: 5 Rust units (Overloaded erase x2, TVT normalization,
    unmatched-TVT default, interpolation) + 2 Python suite tests
    (`test_alias_union_arg_flattens`,
    `test_overloaded_item_erases_via_first_item_fallback`) and 2
    reworked defer pins (`test_splice_with_paramspec_repl`,
    `test_generic_supertype_check2_decides`).
  - Gates: cargo 2,746/11, fmt + clippy clean, testtypes 3,323/6,
    testcheck 8,198/15/7 exact, fine-grained 747/27, daemon 37,
    finegrainedcache 549/229, merge+diff 120/1, cold self-check clean
    347.
- wave-61B audit + ports, five un-audited seams (wave 61, issue #1512):
  env-gated defer-reason probes (`MYPY_TK_W61_AUDIT`, stripped before
  landing; cold self-check, exact event counts). The audit pinned every
  fallback to a single leaf reason per seam; the ports retired 68 of 72.
  - `rust_arg_approximate_similarity` 144 / 20 -> 116 / 0: all 20 were
    `TypeAliasType` operands at `get_proper_or_defer`; the seam expands
    top-level aliases through the resolver snapshot (the call drop is the
    retired Python fallback re-entries).
  - `rust_builtin_item_type` 1,427 / 17 -> 1,427 / 0: all 17 were alias
    first args / tuple items; the alias expands to its target, which the
    only consumer immediately recovers with `get_proper_type`.
  - `rust_add_class_tvars` 1,309 / 8 -> 1,314 / 0: all 8 were
    `expand_type_by_instance_core_alias` defers where the bound
    classmethod carries its own fresh tvar (`dict.fromkeys` 6,
    `_pytest._code.code.ExceptionInfo.for_later` 2); the CallableType arm
    now uses `expand_type_by_instance_free`, matching Python's
    no-defer `expand_type_by_instance` + the shim's
    `freeze_all_type_vars` (IAMA-tail pattern). The non-callable arm keeps
    the core variant (Python does not freeze there).
  - `rust_narrow_with_len` 273 / 11 -> 273 / 0: 7 entry aliases + 4
    union-item alias recursions. `get_proper_type` expands through the
    resolver snapshot, `can_be_narrowed_with_len` expands before
    `custom_special_method` (Python expands inside it), and the union
    loop passes the expanded item into the recursion. The shared
    `custom_special_method_inner` TupleType arm reads the partial
    fallback's definer directly instead of building `tuple_fallback`'s
    item union (the union build deferred on alias items for no answer
    change).
  - `rust_is_overlapping_types` 968 / 16 -> 898 / 4: the 12
    CallableType-vs-Instance defers now run the live
    `find_member("__call__", instance, instance, is_operator=True)` fetch
    (`get_protocol_member_inner`, find_member semantics) and recurse into
    the fetched FunctionLike, mirroring meet.py:783-792. Floor: 4 step-6
    `is_subtype` engine defers, all TypeType-left (3 `Type[...]` vs
    `Callable(Extension)`, 1 `Union[Type[...]]` vs `Overloaded`) - the st
    TypeType-vs-FunctionLike wall, untouched.
  Pins: `NativeArgApproxAliasSuite`, `NativeBuiltinItemAliasSuite`,
  `NativeNarrowWithLenAliasSuite`, `NativeOverlapCallableInstanceSuite`,
  `NativeAddClassTvarsFreeSuite` (direct seam + gate-off/on parity,
  alias-map-missing defers).
  Gates: cargo 2,741/11, fmt + clippy clean, cold self-check clean 347,
  testtypes 3,341/6 (+20 tests), testcheck 8,198/15/7 exact,
  fine-grained 747/27, daemon 37, finegrainedcache 549/229.
- wave-62D non-engine floors: defaulted alias fill + distinct pretty
  definition hints (wave 62, issue #1519): audit-first env-gated probes
  (`MYPY_TK_W62D_AUDIT`, stripped before landing; cold self-check at
  `-n0 --no-incremental`) pinned every fallback of both seams to a single
  leaf reason before any port.
  - `rust_instantiate_type_alias` 5,867 / 68 -> 5,867 / 0: all 68 were
    the step-5 bare-generic `set_any_tvars` fill with `max_tv=1`, every
    alias tvar defaulted (`no_default=0`), no TypeVarTuple, no default
    cross-references, `disallow_any=False`, `analyzing_tvar_def=False`.
    New tag 3 = defaults-only fill: Rust proves every alias TypeVar has a
    default and none is a TypeVarTuple (deferring on
    `analyzing_tvar_def`), and the Python shim mirrors the `set_any_tvars`
    defaults loop with the existing native `expand_type` per default
    (empty/no-op env in the measured corpus). Floors: Any fills
    (undefaulted tvar), TypeVarTuple, `analyzing_tvar_def`, and the
    error paths.
  - `rust_format_type_distinctly` 931 / 37 -> 931 / 0: all 37 were n=2
    pairs whose sole unsafe pretty callable was the raw top-level node
    (`first_arg_fdef` / `name_none`, depth 0; 0 nested). The seam gains a
    per-type `hints` channel (parallel to the wire blobs) carrying the
    definition-derived `(func_name, first_arg)` scalars (`_pretty_hint`);
    Rust's `pretty_callable` port applies them (`apply_pretty_hint`), and
    `callable_compat::is_type_obj` gates the `self`/`cls` prepend. A
    nested unsafe callable has no live cursor in Rust and keeps the
    pure-Python fallback (`_distinctly_pretty_plan` returns
    `(False, None)`). Side effect: the 37 retired Python fallbacks no
    longer re-enter `format_type_bare` / `find_type_overlaps`
    (330 -> 87 calls; the 1 residual `rust_format_type_bare` fallback is
    the pre-existing `_pytest.raises.AbstractRaises[Any]` snapshot miss
    with `_pretty_wire_safe` true, unchanged).
  - Pins: `NativeInstantiateTypeAliasSuite` +4 (default fill
    single / crossref / mixed-defaults defer / analyzing defer),
    `NativeMessagesDeferralSuite` +3 (name hint, first-arg hint, nested
    defer) plus 5 Rust `apply_pretty_hint` units.
  - Gates: cargo 2,808 passed / 11 ignored (type_kernel 2,752/11),
    fmt + clippy (`-p mypy-type-kernel --lib -D warnings`) clean, cold
    self-check clean 347, testtypes 3,350/6 (+7), testcheck
    8,198/15/7 exact, fine-grained 747/27, daemon 37.

- wave-62C fresh/identity residue audit + ports (issue #1518): env-gated
  leaf-reason probes (`MYPY_TK_W62C_AUDIT`, stripped before landing) on
  the cold self-check ranked the three seams' post-wave-61 defers, then
  all three retired natively.
  - `rust_freshen_all_functions_type_vars` 33 -> 0 defers (779 calls,
    100%): (a) top-level `Overloaded` items map through the visitor
    (type_visitor.py:299-300) instead of deferring the root (7);
    (b) `fresh_type_var`/`var_env_key`/`tvar_default`/`set_typevar_default`
    generalized to ParamSpec/TypeVarTuple (types.py:770-772
    `new_unification_variable` keeps every field and re-ids), retiring the
    non-TypeVar `variables` defer (5); (c) the FFI gained the resolver
    (`FlatAliasGuard` install), so the internal expand union arm expands
    alias items (#1203), and the shim retries the decode once with
    `fixup_wire_type(resolve_aliases=True)` (the #1224 contract) for
    alias-surviving results (21).
  - `rust_remove_dups` 26 -> 0 defers (41 calls, 100%): the FFI dedup now
    uses `py_type_eq` (Python `__eq__` semantics: AnyType isinstance-only,
    UnionType frozenset order-insensitive, CallableType flag exclusions)
    and accepts alias-bearing rows; the shim gates on
    `_dedup_alias_identity_sound` (one live alias object per fullname, so
    the structural `(fullname, args)` key is injective) and retries the
    list decode with `resolve_aliases=True`; `_restore_dedup_identity`
    still returns the live first-occurrence rows. The two in-engine
    `remove_dups` mirrors (`try_expanding_sum_type_to_union`,
    `expand_for_target`) switched to the same py-eq helper.
  - `rust_type_object_type_from_function` 18 -> 0 (1,838 calls, 100%):
    15 `bind-self/expand` defers retired by `alias_ok=true` +
    FlatAliasGuard at the composite entry (the #1496 alias decode already
    re-links survivors), and 7 `alias-in-self` defers retired by alias
    expansion in `collect_query_tvars` (resolver chain + the Python
    `seen_aliases` guard). One `typeobj:map` event (named-tuple
    `datetime._IsoCalendarDate` -> builtins.tuple) stays a Python-side
    floor: the maptype element-preserving tuple fallback (maptype.py:226)
    is not ported.
  - `_resync_definitions_inner` (expandtype.py) tolerates the union-length
    divergence alias-union flattening creates when the original subtree
    carries no definitions, and pairs TypeAliasType args positionally.
  Pins: `NativeFreshenSuite` (Overloaded, ParamSpec, alias union),
  `NativeExpandTypeAliasSuite` (freshen alias union + direct alias),
  `NativeRemoveDupsSuite` (6: identity end-to-end, unsound-alias
  fallback, py-eq differentials), `NativeTypeObjectAliasDecodeSuite`
  generic-self alias; plus 4 Rust `remove_dups_py_eq_inner` unit tests.
  Gates: cargo 2,750/11, fmt + clippy clean, cold self-check clean 347,
  testtypes 3,355/6 (+12), testcheck 8,198/15/7 exact, fine-grained
  747/27, daemon 37, finegrainedcache 549/229.
- wave-62B checkmember/IAMA residue audit + protocol-member ports (wave 62,
  issue #1517): audit-first, `MYPY_TK_W62B_AUDIT` probes (shim counters +
  Rust leaf tags) on the cold self-check at `-n0 --no-incremental`,
  stripped before landing. Surveyed seams: `analyze_union_member_access`
  1,063 / 25 fallbacks, `analyze_none_member_access` 902 / 5,
  `get_protocol_member` 106 / 7, `analyze_member_method` 27 / 24.
  - `rust_get_protocol_member` 106 / 7 -> 92 / 3: three decidable arms
    ported. (a) The class-Self gate is precise: `live_var_plain` no longer
    rejects every member when `info.self_type` is set; the caller reads the
    PEP 673 Self key (`live_self_tvar_key`) and defers only when the member
    type mentions that tvar (`contains_tvar_key`). `expand_self_type`
    (expandtype.py:1345) no-ops for properties, so `_io.FileIO.closed`
    (bool property on a class inheriting `__enter__ -> Self`) and
    `name: Any` decide natively. (b) `live_var_plain`'s `is_instance_var`
    polarity was inverted (`if !is_inferred` required the inferred flag),
    leaving the Var arm dead; fixed to `if is_inferred`. (c) The Var arm's
    attribute-hook gate used `plugin_get_attribute_hook_absent` with a
    dummy fullname plus a user-plugin refusal, deferring every Var lookup
    whenever the build loaded any plugin (the self-check's
    `mypy.plugins.proper_plugin`); it now probes the live
    `chk.plugin.get_attribute_hook(fullname)` chain like the Decorator arm
    (`plugin_get_attribute_hook_hits`), with new guards for callable/alias
    vars (unmodeled `analyze_var` bind) and enum vars (Literal
    last_known_value wrap).
  - Join structural-protocol guard: engaging the Var arm let the subtype
    engine decide protocol pairs that previously deferred, exposing the
    missing `TypeJoinVisitor.visit_instance` structural preference
    (join.py:757-772) in `visit_instance_join` /
    `join_instance_pair_via_core` - `[x: P, y: P2]` joined to `list[object]`
    instead of `list[P]` (testJoinProtocolWithProtocol / WithNormal).
    Protocol operands now defer the nominal kernel so Python applies
    nominal + structural together (3 new setops units).
  - Floors with evidence: union 25 = 100% lvalue unions (message-coupled
    per-item lvalue/super semantics stay in Python); none 5 = `builtins.object`
    missing-attribute paths that must emit `has_no_attr` / match
    `# type: ignore[union-attr]` (`Options`/`HS_FLAG_*`/`lvalue_type.var`);
    AMM 24 = 19 bare-recv-tvar (#1214 wall) + 5 `mmm:freeze`
    (`enum.EnumMeta.__iter__`, tvar identity wall); protocol 3 = 2 fd-mmm
    (EnumMeta `__iter__` freeze/map) + 1 class-obj metaclass path.
  Pins: `NativeProtocolMemberSelfGateSuite` (10 tests: direct seam +
  gate-off/on parity for property/class-Self, var/class-Self, Self-typed
  members, inferred/callable/enum vars, live hook hit/miss).
  Gates: cargo 2,752/11 (+6 units), fmt + clippy clean, testtypes
  3,353/6 (+10 tests), testcheck 8,198/15/7 exact, fine-grained 747/27,
  daemon 37, cold self-check clean 347.
- wave-62A join/meet family residue (wave 62, issue #1516):
  audit-first (`MYPY_TK_W62A_AUDIT` leaf probes, stripped) + four
  ports. Before -> after (cold self-check survey, same-proxy):
  `rust_join_types` 21/208 -> 7/163 (96% native), `rust_join_instances`
  17/28 -> 7/14 (50%), `rust_join_type_list` 10/25 -> 7/20 (65%),
  `rust_meet_types` 6/17 -> 0/11 (100%), `rust_join_tuples` 4/7 ->
  1/4 (75%): 58 -> 22 fallbacks.
  - `visit_meet` tuple arm (meet.py:1355-1361): both-fixed/variadic
    shapes run `meet_tuples_inner`; Python's structural None cases
    (unequal fixed arity, misaligned unpacks, fixed shorter than the
    variadic prefix+suffix) answer `default(self.s)` = Bottom, any
    other inner None defers. `find_unpack_in_list_like_python` mirrors
    Python's None-on-multiple-unpacks. Retires all 6 meet fallbacks.
  - `visit_instance_join` s non-Instance cross arms (join.py:437-454):
    FunctionLike -> `join_types(t, s.fallback)` (defer only on a
    `__call__` protocol t); TypeType/TypedDict/Tuple/Literal ->
    `join_types(t, s)`; TypeVarTuple bound accepted -> SameT; else
    `join_default(s)`. Nested results remap from the recursive frame
    onto the outer `(s, t)` frame (`remap_result_to_outer`, encoding
    concrete non-operand answers).
  - `visit_type_type` case 3 (join.py:864) no longer defers: it encodes
    `default(s)`. Pre-existing parity bug discovered while porting: the
    Object disc is t-derived in the shim, so the TypeType / ParamSpec /
    Parameters default arms diverged gate-on (`join(Instance,
    Parameters)` returned Any vs Python object); all three now encode
    the concrete s-derived result (`join_default_encoded`, 3 Rust-unit
    re-pins + a Python parity pin).
  - Nested join/meet materialization expands top-level alias operands
    first (`expand_top_alias_pair` in `rust_join_types_inner` /
    `rust_meet_types_inner` / `join_one_pair`): SameS/SameT must name
    the EXPANDED operand (Python's `get_proper_type` at join entry), so
    an alias node can no longer leak into a result the shim then fails
    to decode (retired the 4 mypyc SlotTable/SlotGenerator
    `join_tuples_decode` events).
  - `make_simplified_union(handle_recursive=False)` (the
    `tuple_fallback` union) expands NON-recursive alias items and keeps
    recursive alias nodes folded (`is_recursive`), mirroring
    types.py:5109; the wave-33 `contains_recursive_alias` /
    handle_recursive=False guardrails stay (recursive alias
    tuple-fallback still defers).
  - Floors (audited, do not re-audit): st protocol-right/left engine
    (`is_subtype(Context/object, typing.Reversible)` -> None, 19
    embedded events: the via-supertype collection / Callable-ret
    family), `join_instances_via_supertype` generic-ancestor with tvars
    (`Literal['x']?` vs `Literal[b'y']?` -> typing.Sequence), the
    wave-43 whole-list LKV guard in `join_type_list_inner` (SlotTable
    tuple pair), and a NOT_READY wire-map entry (`types.ModuleType`
    join_type_list decode). Every residual fallback classifies here.
  Pins: `NativeJoinMeetWave62Suite` (+9 Python parity/seam tests) + 12
  Rust units (meet tuple arm, cross arms, remap, alias materialization,
  non-recursive expansion).
  Gates: cargo 2,753/11, fmt + clippy clean, cold self-check clean 347,
  testtypes 3,352/6 (+9), testcheck 8,198/15/7 exact, fine-grained
  747/27, daemon 37 (one flake on the first -n4 run, green on rerun),
  finegrainedcache 549/229.

- wave 63A mirror graduation audit (#1527): first CI coverage for the
  F1/F2 mirror (`parity-mirror` job: capture testtypes, capture+read
  testcheck with the #1530 case deselected, capture+read fine-grained;
  path filters add `mypy/types_mirror.py` / `mypy/types.py`), the
  `Instance.extra_attrs` splice (`_FLIP_FIELDS` +
  `rust_mirror_patch_instance_extra_attrs`), and an `ExtraAttrs.raw` wire
  refinement so decoded records re-encode byte-exact (Python's
  `write_type_map` insertion order cannot survive a `HashMap` round
  trip; without it any multi-key splice reorders and drifts, a latent
  bug the existing Instance ops shared). Capture cuts, all on the
  opt-in path only: Python `_HANDLE_BY_ID` replaces the
  `rust_mirror_handle_of` FFI on hot paths (59.8M crossings/self-check),
  `_mirror_setattr` restructure with a construction fast path,
  `_count` audit-gated, and mutation-time registration skipped for
  objects no stored blob embeds (strike interactions 5.58M -> 0,
  failed setattr serializations 0.77M -> 0, write-funnel strike skips
  1.92M -> 0). Same-harness cold capture A/B: 290.1s -> 226.0s (-22.1%,
  audit on); audit-free post-cut 217.4s vs 91.9s mirror-off. The
  within-10% gate is NOT reachable by hot-path tuning; decision record
  at `docs/plans/2026-09-11-mirror-capture-audit.md` (capture stays an
  opt-in audit tool; ADR-0004 proxy is the graduation path; P4
  default-on blocked on #1530). The first CI run found #1530: the F2
  read flip serves pre-mutation blobs after in-place list writes
  (typeanal.py:2069-2070; macOS `testVariadicStarArgsCallNoCrash`,
  linux `testRevealBoundParamSpecArgs`), both deselected in CI until
  fixed.
  `misc/f3s9_tvar_union.py` dead `__setattr__` hook retired (the mirror
  report is the per-field truth). Lazy recursive child registration was
  tried and reverted: nested `write` calls re-adopt the child, so it
  serializes twice per parent funnel instead of once. Gates: cargo
  2,772/11 (+3), fmt + clippy clean, cold self-check 347, testtypes
  3,385/6 both gate states, testcheck 8,198/15/7 exact both states,
  fine-grained 747/27 + daemon 37 mirror-on (capture+read).

- wave 64A F2 stale-blob fix (#1530): the F2 read flip served
  pre-mutation blobs after raw in-place writes (list/dict item stores
  and extend/append) because raw writes never fire the family
  `__setattr__` capture and never bumped `_UNPROT_EPOCH`. Fix per
  ADR-0004 Decision 3, opt-in path only (no default flip):
  `types_mirror.touch(t)` re-serializes/cascades a registered family
  blob and bumps the unprotected epoch; `read_fresh_bytes` now gates on
  `rust_mirror_write_skip(h, _UNPROT_EPOCH)` and re-serializes before
  serving when the epoch moved (covers shared-list siblings and
  nested-mutation carriers). The site hook is
  `mypy.types._mirror_touch` (installed by `types_mirror.activate`,
  one None-check when the mirror is off). Mutator registry, all
  touched: CallableType `arg_kinds[-1] =` via the aliased list
  (typeanal.py:2069, the report's site), CallableType `arg_types[i] =`
  in `normalize_trivial_unpack` (types.py, both repros' mutation),
  CallableType `arg_types.extend` (semanal.py:2330/2340), TupleType
  `items` rebind + item write (semanal_typeargs.py:114/117,
  change-guarded), Overloaded `items[0] =` (checker.py:1724),
  TypeAliasType `args` rebind (semanal_typeargs.py:99). Family-class
  rebinds (Instance.args, CallableType.variables, ...) stay on the
  patched setattr capture, no touch. Instrumentation stripped; the two
  CI-deselected cases re-enabled in `parity-mirror` (the `-k`
  deselection is gone). Repro before -> after: both
  `testVariadicStarArgsCallNoCrash` and `testRevealBoundParamSpecArgs`
  fail (spurious diagnostics) under capture+read -> green. Pins:
  `NativeMirrorReadSuite` +4 (raw-escape served stale until touch,
  epoch gate refreshes an untouched drifted handle,
  `normalize_trivial_unpack` touch + gate-off parity). Gates: cold
  self-check clean 347 (mirror off); testcheck 8,198/15/7/0 both
  mirror states; testtypes 3,389/6 both states (+4); fine-grained
  747/27 + daemon 37 with capture+read; ruff/black clean on the
  changed files (2 pre-existing C408s in testtypes untouched).
- wave 64B recursive-alias pretty-walk loop hardening (#1532): the
  wave-63B `__weakref__` hang of `testNoCrashOnRecursiveTupleFallback`
  is NOT in `flatten_nested_unions`. An instrumented cold self-check
  (env-gated, stripped) pinned the loop driver at
  `messages._pretty_wire_safe` / `_unsafe_pretty_callables`: every
  `get_proper_type` alias expansion mints a fresh proper tree, so the
  id-keyed `seen` set never cuts a recursive alias and the walk stops
  only when the allocator recycles a freed address ("allocation luck").
  Fix: `_repeat_alias_expansion` in `mypy/messages.py` cuts on the alias
  node plus args before expanding, in BOTH walkers (the #1528 WIP
  blanket alias-defer only moved the loop into `_unsafe_pretty_callables`);
  `_pretty_wire_safe` still walks the first unrolling, so recursive
  trees keep the native formatting path. `flatten_nested_unions`
  no-resolver branch: every recursive alias row now appends `None`
  (whole-call defer) regardless of `handle_recursive`; non-recursive
  rows keep the live `get_proper_type` row expansion. Audit: a directly
  self-referential union alias (`X = Union[int, X]`) is rejected by
  semanal ("Invalid recursive alias"), so the row callback only ever
  sees container-nested aliases whose one-step expansion is finite; the
  guard is defense in depth, not a behavior change. Regression pins
  (testtypes): `test_recursive_alias_pretty_walk_terminates` (pins every
  expansion result to block id reuse; bounded call count),
  `test_recursive_alias_format_parity`,
  `test_flatten_recursive_alias_no_resolver_defers_row` and
  `test_flatten_nonrecursive_alias_no_resolver_callback_engages`; the
  perturbed-layout hang (20s+, 62k sequential `flatten_nested_unions`
  calls) now passes in 0.07s. Gates: cargo 2,772/11 ignored, fmt +
  clippy clean, cold self-check clean 347, testtypes 3,389/6 (+4),
  testcheck 8,198/15/7 exact, fine-grained 747/27 + daemon 37.
- wave 64C librt `write_raw_bytes` scratch build (#1526): the PyPI
  `librt>=0.12.0` wheel omits `librt.internal.write_raw_bytes` even
  though `mypyc/lib-rt/internal/librt_internal.c` exports it, so
  `mypy/types.py` degraded the F3 wire-cache splice to `None` and
  `NativeMirrorSpliceSuite` self-skipped 3 tests. The in-repo lib-rt now
  builds into a scratch prefix that is prepended on `PYTHONPATH` (all six
  modules; PyPI stays the namespace fallback; the shared `.venv` is never
  mutated): suite 3 passed/3 skipped -> 7 passed/0 skipped (new
  session-depth regression test; `test_strict_mode_skips_until_unprotected_bump`
  now branches on `_SPLICE_ACTIVE` because the epoch skip is a
  write-funnel behavior unreachable once the splice serves the hit).
  Enabling it exposed a latent cache-inerting bug: a raise inside the
  `_write_type_cached` / `_serialize_with_taint_check` session leaked
  `_type_wire_cache_session_depth`, making the cache inert process-wide
  (a 20% xdist flake in testtypes); both sessions restore the depth in
  `finally`. CI: `pr-gate` + `native-kernel-parity` (`parity`,
  `parity-mirror`) build `mypyc/lib-rt` and prepend `LIBRT_SCRATCH`;
  build order documented under "librt build order". Gates with the
  scratch librt: cold self-check clean 347, testcheck 8,198/15/7 exact,
  testtypes 3,389/3, testinfer 106, F2 mirror capture+read testcheck
  8,196/15/7 (2 deselected, #1530), splice suite 10/10 stress runs green.
- wave 65A capture-overhead cuts (#1539), the F4.2 slice off the wave-63A
  audit. cProfile on the capture cold self-check ranked
  `rust_mirror_walk_indices` first (39.9s tottime / 2.75M calls), then
  `_mirror_setattr` (26.8s / 100.5M) and the Python `_child_types` scan
  (23.2s cum / 4.94M), all inside `_register_tree` (105.7s cum) and
  `_assert_fresh` (141.9s cum) over the `Type.write` funnel (166.8s cum).
  Cuts, opt-in path only: (a) the walk's `mypy.types` class context and
  per-class slot names are cached thread-locally (strong type pin;
  `rust_mirror_reset` clears both), slot reads borrow instead of incref,
  and container dispatch runs before the tvid/Type/name probes (walk
  micro: AnyType 2.38 -> 0.38us, Callable 13.3 -> 5.4us); (b)
  `rust_mirror_walk_registration` returns the direct family children with
  the index lists so `_register_tree` does one Rust descent instead of a
  Python child scan plus a Rust walk (`_walk_registration_py` keeps the
  deferral/differential body); (c) clean-run hot-path guards: `_audit_mode`
  f-string gates on the funnel counters, inlined `_handle_of` in
  `_assert_fresh`, empty-strike/empty-pending call skips; (d) the
  per-serialization `librt`/`mypy.types` function-local imports hoisted.
  Same-harness cold self-check A/B (audit off): off median 92.6s
  (82.9-97.1), capture median 171.3s (158.2-174.4) vs the wave-63A 217.4s
  clean, so the capture overhead fell +125.5s -> +78.7s (-37%; capture
  wall -21%), ratio 2.37x -> 1.85x. Audit-on counter parity pre/post is
  exact within run-to-run volume drift (zero mismatches, `unprot_bump` 80
  both). The within-10% gate stays unreachable (~101s capture needed,
  ~69s residual gap: 2.3M registrations + 2.9M fresh serializations, both
  semantics-bound); the ADR-0004 proxy stays the graduation path and
  capture/read stay opt-in. Gates: testtypes 3,399/3 scratch librt
  (3,395/7 PyPI; +2 registration-walk differential tests), testcheck
  8,198/15/7/0 exact in kernel, capture, and capture+read modes,
  fine-grained 747/27 + daemon 37 capture+read, cold self-check 347
  (off and capture), cargo 2,773/11, fmt + clippy clean. Table in
  `docs/plans/2026-09-11-mirror-capture-audit.md` section 8.
- icf protocol-member actual-shape arms (wave 65C, issue #1541): the
  largest residual seam `rust_infer_constraints_full` (21,888 calls,
  149 raw-FFI fallbacks measured on the wave-65 head; the issue's
  "~219" was the rounded-1% estimate) was audit-first ported. The
  env-gated defer-reason probe (`MYPY_TK_W65C_AUDIT`, stripped before
  landing) ranked the leaves: `vi-proto-both-sup` 40 /
  `vi-callable-proto` 37 / `call-actual-not-any` 36 (Overloaded actual
  27 + Instance actual 9) / `ffi-extra-tvars` 27 /
  `tail-protocol-tuple` 6 / `vi-proto-actual-sup` 3.
  Ported:
  * both-protocol Instance SUPERTYPE_OF: the `template.is_protocol`
    SUP arm is no longer gated on `!actual.is_protocol`
    (constraints.py:1550-1572 fires for any protocol template);
    protocol-actual SUPERTYPE_OF pairs now take the shape tail's []
    (dispatch rework + `merge_arm_res` accumulator mirroring Python's
    shared `res`, including the `if res: return res` precedence before
    the shape tail).
  * Callable actual vs protocol template
    (`visit_instance_callable_protocol_arms`): the generic
    callback-protocol arm (`find_member("__call__", ...)` via
    `get_protocol_member_inner` find_member semantics, erased-member
    subtype gate, recursion) and the class-object arm (`is_type_obj` +
    `get_instance_type(force_fallback=True)` via
    `force_fallback_instance` + the member loop with `class_obj=true`;
    the class-object member fetch remains a documented defer floor),
    then the callable's fallback continues through the instance logic.
  * Tuple actual vs protocol template (`visit_instance_tail_native`):
    the tuple-fallback structural-protocol special case
    (constraints.py:1619-1630) reusing the parity-tested
    `is_protocol_implementation_inner` + member loop.
  * Callable-template actual arms (`visit_callable_native`):
    Overloaded actual via `overload::find_matching_overload_index`
    (new `pub(crate)` core factored out of the unchanged
    `rust_find_matching_overload_items` pyfunction) + normalized-template
    recursion; Instance actual recurses against a found `__call__`
    member (the member-miss case defers: Python's `find_member` owns
    position/side-effect bookkeeping); the remaining shapes take the
    `else: return []` tail.
  Result (clean build, proxy survey): 21,888 -> 21,735 calls,
  149 -> 56 raw-FFI fallbacks (-62%). Residual taxonomy: 51
  `ffi-extra-tvars` (the polymorphic reverse frame attaches
  `extra_tvars`; the 3-field wire has no channel, the #1171/#1427
  floor), 1 protocol-member fetch defer, 4 Instance-actual miss
  defers (the `NoneVal` defer above). The 24 Overloaded-actual calls
  now solve natively but surface extras and defer at the FFI boundary;
  they retire wholesale when the extras channel lands. Pins:
  `NativeInstanceConstraintArmsSuite` (10 gate-off/on parity +
  direct-seam tests; class-object and member-miss defers documented
  and asserted), `NativeIcfProtocolSubtypeArmSuite` supertype-direction
  pin reworked to `..._returns_empty`. Gates: cargo 2,772/11, fmt +
  clippy clean, cold self-check clean 347, testtypes 3,403/7
  (3,393 baseline + 10 new), testcheck 8,144/69/7 exact (same command
  and counts as the pre-change baseline `.so`; the merged-head
  `8,198/15/7` differs only by 54 environment-skips), fine-grained
  747/27 + daemon 37, survey numbers reproduced from the issue.
  Noticed, not fixed: #1543 (a wire-decoded native type used as an
  error context can lose its source line; the shipped code defers the
  member-miss path that exposed it).
- AST wire v5 - native parser docstrings, custom typing module, version pin
  (wave 66A, issue #1545). G0.1 of the Phase G0 brief. `AST_WIRE_VERSION = 5`
  is returned in the parse data dict (`ast_wire_version`) and enforced at the
  `ast_serialize.parse` entry with a RuntimeError on `cache_version` mismatch,
  so a stale `.so` fails at the call instead of mid-deserialization
  (`AssertionError: 255` class). `FUNC_DEF_STMT` / `CLASS_DEF` gain a
  docstring field after the name (present flag, value, corrupted flag,
  optional raw tokens for lone-surrogate repair); readers set
  `FuncDef.docstring` / `ClassDef.docstring`, matching
  `ast.get_docstring(node, clean=False)` including lone surrogates (probe:
  `"\\ud800"` yields `'\ud800'`, not U+FFFD). `--custom-typing-module` is
  threaded into the serializer: `import` / `from` translate the module to
  `typing` with the implicit asname when translation occurs (star imports
  stay untranslated, mirroring fastparse's `visit_ImportFrom`). The writer's
  `argument_elide_name` stays unconditional: fastparse's `make_argument`
  applies it regardless of `pos_only_special_methods` (`def f(__x)` is
  pos-only under both option states), so the fix gates only the reader's
  special-method re-elision; gating the writer would diverge. `mypy.parse`'s
  native branch applies `options.transform_source` (the build path's
  `source=None` reads the file via `nativeparse.read_source` first). New
  in-tree `stubs/ast_serialize.pyi` declares the v5 surface (the pinned PyPI
  wheel still declares v4, which broke the self-check). Tests:
  `TestNativeParserOptionParity` (docstring / custom-typing-module / pos-only /
  wire-version / transform differentials vs fastparse), v5 golden blobs, 8
  Rust unit tests. Gates: cargo 16/0, fmt + clippy clean (crate clippy was 6
  errors on main; the two dead-scaffolding `new_without_default` get module
  allows until #1546 deletes the files, the two type-complexity tails are
  fixed), test_nativeparse + testparse 510 passed/74 skipped, testcheck
  8,144/69/7 exact, teststubgen 4 -> 3 failures (`testIncludeDocstrings`
  green; residual are the #1547 yield failures), cold self-check clean 347.
  Noticed, not fixed: #1551 (multi-alias `import a, b` splits into one
  `Import` node per alias in `tree.imports`).
- wave 66B AST CI + dead-scaffolding deletion (G0.2, issue #1546): the
  four dead AST-scaffolding modules (`nodes_full.rs`, `nodes_codec.rs`,
  `full_ast_codec.rs`, `visitor_engine.rs`, 2,040 lines, zero callers;
  #138/#140 artifacts) deleted with their `lib.rs` declarations; `rg`
  confirms no remaining references. ast_serialize now has cargo
  test/fmt/clippy gates in pr-gate (the pre-existing too-many-arguments
  and type-complexity lints are silenced with repo-convention allows),
  and a new `parity-ast` job in native-kernel-parity runs
  test_nativeparse + testparse + teststubgen (native) plus the testparse
  fastparse differential; the paths trigger gains
  nativeparse/fastparse/parse and the three test modules. AST-only tag
  ranges corrected to 153-159 and 230-253 (150-152 are
  EXTRA_ATTRS/DT_SPEC/LOCATION); the three astwire-only expression tags
  that collided with 150-152 moved to 230-232 in both mirrors
  (`mypy/astwire.py` + `crates/type_kernel/src/astwire.rs`; astwire
  bytes are traverser-local and never cache-persisted). teststubgen is
  green; `testIncludeDocstrings` runs for real once #1552 (G0.1) lands
  in the same wave, so the interim xfail was removed here. #1547's yield
  trio reproduces on main (3/3 with the shared wave-65 extensions) in
  both parser modes and is xfailed in teststubgen for the new AST CI
  gate; remove with the #1547 fix. Gates: cargo type_kernel 2,773/11,
  ast_serialize 3 (the 7 scaffold tests deleted), fmt + clippy clean
  both crates, AST parity 876 passed/75 skipped/3 xfailed, testparse
  fastparse 250/74, testcheck 8,198/15/7 exact, cold self-check 347.
- wave 67C stubgen yield-trio root cause + fix (issue #1547): the three
  pre-existing stubgen failures (`testFunctionYields`,
  `testGeneratorYieldFrom`, `testGeneratorYieldAndYieldFrom`) were not
  parser-mode dependent. The trigger is the native traverser seam
  `rust_has_return_statement` (`crates/type_kernel/src/traverser.rs`):
  astwire drops the `NameExpr.name` scalar, so the Rust seeker could not
  tell `return None` (trivial for `ReturnSeeker`) from `return <name>`
  (non-trivial) and answered `true` for both. Stubgen's generator
  inference (`mypy/stubgen.py:711`) then added the third
  `Generator[..., Incomplete]` parameter for `return None`. Fix: the
  seeker is now tri-state (`Option<bool>`) - `Some(true)` when any
  non-`NameExpr` return expression exists (a decidable non-trivial
  return short-circuits), `None` (defer to Python's `ReturnSeeker`) when
  only `NameExpr` returns are present, `Some(false)` when no
  expression-carrying return exists. No wire-format change: the
  `#1030` defer pattern applies. `mypy/test/teststubgen.py`'s #1547
  xfail block is removed; `NativeTraverserSuite` gains fastparse +
  eager-native-parse parity pins and a direct seam tri-state test
  (`return None`/`return x` defer, `return 1` true, bare return false).
  The CI `parity-ast` job never built `type_kernel`, so its teststubgen
  run was already on the Python fallback; the bug only showed under the
  shared wave-65 extensions. Gates: trio green with the xfail removed;
  full teststubgen 373 passed/1 skipped/2 xfailed (the two remaining
  xfails pre-date #1547: `test_infer_sig_from_docstring_args_kwargs_errors`
  and the data-driven `testNestedClassInNamedTuple_semanal-xfail`);
  testtypes 3,409/7; testcheck 8,198/15/7 exact; cold self-check clean
  347; cargo type_kernel 2,775/11; fmt + clippy clean.

- wave 67A expression enum + writer byte parity (G0.3, issue #1556):
  `crates/ast_serialize/src/ast_node.rs` adds the `ExprNode` enum (one
  variant per `serialize_expr` wire tag with the exact current fields)
  plus nested records (`FStringPartNode` / `FStringItemNode` /
  `FStringFormatSpecNode` / `TStringItemNode` / `ComprehensionNode` /
  `DictItemNode`); `ast_writer.rs` emits the records; the
  `serialize_expr` / `serialize_lvalue` dispatchers now build-then-write,
  so the enum is the single source of truth. Lambda parameter lists stay
  a captured byte payload (def-family sub-record for G0.4); statements,
  patterns and symbol nodes are untouched; wire format byte-identical,
  no `AST_WIRE_VERSION` bump. Byte-parity proof: a test-only frozen copy
  of the old direct writers (`expr_legacy.rs`, `#[cfg(test)]` only) plus
  a Rust A/B over the full native-parser corpus (250 cases: 241 parsable
  = 241 checked, 9 syntax-error skips) and a focused t-string /
  debug-f-string case; the A/B caught a missing `STR_EXPR` END_TAG in the
  first enum-writer draft (fixed before landing). Gates: cargo 11/0, fmt
  + clippy clean (`--lib` and `--all-targets`), AST suites 881/75/5
  xfail exactly, testcheck 8,198/15/7/0 exact, cold self-check clean
  347, fine-grained 747/27, daemon 37.
- wave 67B proxy P1 store scaffold + gate (issue #1553): new
  `crates/type_kernel/src/proxy.rs` - thread-local blob store keyed by
  `identity::handle_for`, `ProxyEntry { bytes, stamp }` plus strong
  `Py<PyAny>` pins; core `read/put/drop/reset/entry_count` and the six
  `rust_proxy_*` pyfunctions registered in `lib.rs`; `reset` deliberately
  does not call `identity::reset` (owned by `rust_mirror_reset`), so other
  seams' handles survive; `rust_proxy_read` returns real `bytes` (a
  `Vec<u8>` return would materialize a list[int] on the P2 hot path).
  New `mypy/type_proxy.py`: activation flag, FFI-free `_PROXY_HANDLES`/
  `_PROXY_PINS` id-keyed maps (wave-65A pattern), epoch counter, audit
  counters, `read_scope_bytes` (Instance-only scope, wire cache off +
  `_REC_CACHE_SUPPRESSED`, serialization failure returns None so callers
  raise identically), `touch`/`reset`/`report`. `Options.native_type_proxy`
  default off, deliberately not in `OPTIONS_AFFECTING_CACHE`;
  `TEST_NATIVE_TYPE_PROXY` env gate in test helpers; `_clear_native_resolvers`
  reset branch next to the mirror reset. No funnel wiring: zero behavior
  change (proxy env leaves testcheck exact). `NativeProxyStoreSuite` (11
  Python tests) + 8 Rust units; successor-ADR draft
  `docs/adrs/0005-proxy-shadow-store-successor.md` records the blob-shadow
  supersession of ADR-0004 Decision 1's single-PyObject identity map and
  of the plan text's "views replacing storage". Gates: cargo type_kernel
  2,781/11 (+8), fmt + clippy clean, cold self-check clean 348 files (347
  + type_proxy.py), testtypes 3,416/7 (= 3,405/7 + 11 new), testcheck
  8,144/69/7/0 local (the documented wave-65C local baseline; CI's
  8,198/15/7 differs by 54 environment-skips), fine-grained 747/27, daemon
  37. P2 (#1554) not attempted in this commit series.
- wave 68C strong-pin stable handles (issue #1528): the wave-63D weakref
  route is dead (every probed Type class refuses `weakref.ref`; no
  `Type.__slots__` change), so `identity.rs` reworks the stable layer to a
  strong `Py<PyAny>` pin per stable handle. Raw and stable now share one
  handle per object (each layer adopts the other's: `handle_for` /
  `handle_of` answer stable-first, `handle_for_stable` migrates a raw entry
  under the pin), so the proxy and the mirror never mint two identities
  (the P1 `NativeProxyStoreSuite` equality pin and the wave-67 stub comment
  encode this). New `retire_stable` / `reset_stable` / `stable_alive` and
  `reset(preserve_stable)`: a preserving reset keeps live-object handles
  and sweeps entries whose pin is their only owner (refcount 1); a
  non-preserving reset drops the layer. Mirror side: `rust_mirror_reset`
  takes `preserve_stable=False`, `rust_mirror_handle_of` is stable-first
  with a raw fallback, new `rust_mirror_stable_alive`, and `register`
  prefers the stable handle (raw fallback) behind the existing gate.
  Python side: `types_mirror.reset(preserve_stable=False)` drops its pins
  first (clears `_BY_HANDLE` / `_HANDLE_BY_ID` / carrier maps) then calls
  the kernel; `_clear_native_resolvers` runs `type_proxy.reset()` first and
  then the preserving mirror reset, so a daemon recheck keeps an
  astmerge-preserved object's handle while blob storage rebuilds under it.
  Residual (documented in `identity.rs`): a dropped graph that stays in a
  mypy-side reference cycle keeps its pin, because a weakref-free pin
  cannot distinguish a cycle root from a live object; a non-preserving
  reset or explicit `retire_stable` releases it. Tests: 15 identity units
  (shared namespace, adoption, retire/reset, mismatch re-check, sweep,
  preserve), 3 mirror units (preserving reuse, full-reset fresh mint,
  pin-only release), `NativeMirrorStableIdentitySuite` (4) and
  `NativeDaemonStableHandleSuite` (1: preserved handle survives a
  fine-grained recheck; a pin-only scratch entry is retired). No Type
  layout, cache format, proxy.py or plugin-visible change. Gates: cargo
  type_kernel 2,791/11, fmt + clippy `-D warnings` clean, cold self-check
  clean 348, testtypes 3,424/7 (3,420/7 + 4), testcheck 8,198/15/7 exact,
  fine-grained 747/27, daemon 38 (37 + 1); the mirror capture / capture+read
  modes re-run exact (testtypes 3,424/7, fine-grained 747/27, testcheck
  8,198/15/7).

- wave 68A statement/pattern enum + writer byte parity (G0.4, issue
  #1560): `crates/ast_serialize/src/ast_node.rs` gains `StmtNode` (one
  variant per statement wire tag with the exact current fields;
  `Assignment` carries both the many-target `StmtAssign` shape and the
  one-target `StmtAnnAssign` shape, split by `new_syntax`) plus
  `PatternNode` / `BlockNode` / `IfClauseNode` / `WithItemNode` /
  `ExceptHandlerNode` / `MatchCaseNode` / `DocstringNode` /
  `SingletonNode`; `ast_writer.rs` gains `write_stmt` / `write_pattern`
  and the two block writers. `serialize_stmt` / `serialize_suite` and
  pattern emission now build-then-write, so the enums are the single
  source of truth. Type annotations, type-param lists and parameter
  records stay captured byte payloads via `capture_bytes` (G0.5 scope),
  re-emitted verbatim; serializer side effects (imports metadata,
  type-comment parse errors, branch modes, function/class depth,
  skip-bodies) run in the build phase in the same order the direct
  writer emitted them. `Block` semantics preserved: an empty block
  writes its fallback loc, optional blocks (for/while else,
  try-else/finally) write none, stripped blocks keep the
  first-statement loc; elif flattening stays the same
  `elif_else_clauses` fold. Wire bytes unchanged, no
  `AST_WIRE_VERSION` bump.
  Byte-parity proof: a frozen verbatim copy of the 21 pre-enum
  statement/pattern emitters (`stmt_legacy.rs`, `#[cfg(test)]` only,
  bodies diff-verified against HEAD) plus the `legacy_stmts` A/B: the
  250-case native-parser corpus in both expression modes (241 parsable
  = 241 checked per mode, 9 syntax-error skips), a parser-options
  matrix (`skip_function_bodies` x `include_docstrings` x
  `custom_typing_module` x expression mode), a language-surface case
  (async def / decorators / with / for-else / try-else-finally / type
  alias / all pattern kinds) and a frozen golden blob for
  import/def/if-elif-else/try/match. Independent cross-binary check:
  `(ast_bytes, errors, import_bytes, data)` hashes identical between
  the pre-change shared `.so` and the new build for all 250 corpus
  cases and all 343 `mypy/` + `mypyc/` sources.
  Gates: cargo ast_serialize 15/0, fmt + clippy clean (`--lib` and
  `--all-targets`); AST suites 884 passed/75 skipped/2 xfailed (the
  brief's 881/75/5 is the pre-#1559 xfail state: #1559 removed the
  stubgen yield-trio xfails, totals 886 either way; the stale shared
  wave-65 typekernel `.so` reproduces the trio as 3 failures);
  testcheck 8,198/15/7/0 exact on the fastparse path and
  8,144/69/7/0 exact with `TEST_NATIVE_PARSER=1
  TEST_NATIVE_RESOLVER=1` (54 `_no_native_parse` skips); cold
  self-check clean 348 (= 347 + `mypy/type_proxy.py` from wave 67B);
  testtypes 3,420/7; fine-grained 747/27; daemon 37; merge+diff 120/1.
  Built in isolated scratch dirs (`mypy-rs-local-ast-1560` plus a
  private `typekernel-1560`) because the shared typekernel predates
  #1559; shared dirs untouched.
- wave 68B proxy P2 lazy Instance read shadow, DROPPED on measurement
  (issue #1554): the full P2 wiring was implemented, gated, and then
  reverted per the ADR-0004 drop-if-no-win rule - `types.py` gained
  `_proxy_read_fn` + `_set_native_proxy_read` with `_read_mirror_blob`
  trying proxy -> mirror -> None; `build.py` activated the proxy and
  installed `type_proxy.read_scope_bytes` only when `native_type_proxy`
  was on and the mirror-read flip off; `type_proxy.activate` composed its
  `touch` onto the `_set_native_mirror_touch` slot; and
  `NativeInstanceProxyReadSuite` (13 tests) plus
  `misc/wf4_proxy_selfcheck.py` pinned and measured the slice. All of it
  is reverted here; the P1 scaffold (`proxy.rs`, `mypy/type_proxy.py`,
  `Options.native_type_proxy`, `TEST_NATIVE_TYPE_PROXY`,
  `_clear_native_resolvers` reset branch) is unchanged from #1553 and
  the final tree is byte-identical to `b63f903e1` plus this bullet.
  Measurement (cold self-check, `mypy_self_check.ini -n0
  --no-incremental -p mypy -p mypyc`, `TEST_NATIVE_TYPE_KERNEL=1`,
  `FORCE_NATIVE_TYPE_PROXY` 0/1, audit on, 5 interleaved pairs):
  wall OFF {180.9, 174.7, 188.5, 149.4, 150.7}s vs ON {173.8, 154.3,
  181.4, 114.2, 207.1}s - median delta -0.9s (-0.5%), mean -2.7s
  (-1.6%); user CPU OFF {137.0, 133.3, 122.6, 121.5, 128.4}s vs ON
  {136.0, 130.6, 135.6, 108.5, 136.7}s - median delta +7.2s (+5.6%,
  mean +0.7%). The proxy engaged deterministically per run (~349.5k
  hits, ~192.5k misses, ~198.8k puts = 12.6MB stored, 6.3k stale
  re-puts, 47 touches, 3 serialize_fail, 206k non-Instance defers;
  size buckets 182k le64 / 15k le256 / 1.5k le1k / 277 gt1k) yet the
  wall win sits inside the load noise (host load avg 60-85 from sibling
  agents; the lone quiet pair showed -23.6% wall / -10.7% CPU while the
  fifth pair flipped +56.4s) and CPU time is a wash: the hit path
  (Python dict + one FFI + pins) costs about what the avoided tiny
  Instance serialization costs, so the slice does not move the
  self-check wall-clock and is dropped rather than defended. Gates on
  the P2 tree before the revert: testtypes 3,433/7 (13 new), proxy-on
  cold self-check clean 348, ruff clean on the touched files. Gates on
  the final tree: cargo type_kernel 2,783/11, fmt + clippy clean,
  testtypes 3,420/7, testcheck 8,144/69/7/0 (exact local baseline; CI's
  8,198/15/7 differs by 54 environment-skips), cold self-check clean
  348. Any future P3/P4 attempt needs a size/engagement threshold (the
  182k le64 puts cannot pay for the store) or a corpus where the F2
  wire cache is off (semanal phases), and must re-run the same A/B on
  a quiet host first.
- wave 69C proxy P2b thresholded read shadow, DROPPED on measurement
  (issue #1567): the preserved P2 patch was re-applied plus an
  engagement/size threshold - no-arg `Instance` roots defer to the
  funnel's `_encode_no_arg_instance` fast path (`scope_defer.trivial`),
  blobs below `_PROXY_MIN_BYTES` (env `MYPY_TK_PROXY_MIN_BYTES`, tuned
  to 64) are served but not shadowed (with the wire cache on the read
  defers so the funnel caches the object; with it off the fresh bytes
  serve the read), and per-size hit counters (`serialize_bytes`,
  `hit_size_*`, `skip_small.*`) plus inlined audit guards tune the cut.
  Measured on two corpora, 4 interleaved pairs each at N=64
  (`FORCE_NATIVE_TYPE_PROXY` 0/1, `MYPY_TK_PROXY_AUDIT=1`): corpus A =
  normal cold self-check, corpus B = `MYPY_NO_WIRE_CACHE=1` (verified
  off: smoke `serialize_hits` 34,420 -> 0).
  - corpus A: wall median +8.97% (pairs +32.2/-21.0/+51.4/-14.3), user
    CPU median +13.99% - the N=64 defer path double-serializes (162,746
    `skip_small.defer` events: one `_fresh_bytes` plus the funnel's own).
  - corpus B: wall median -11.02%, user CPU median -5.63% (pairs
    -13.4/+57.7/-8.8/-2.5); per-pair user ratios span 0.87-1.58 and
    identical OFF runs consumed 61-135s user CPU (load avg 13-67 from
    sibling agents), so the medians cannot separate the expected
    <=0.5% effect from host contention. An earlier block measured during
    a load spike (load avg 60+) was discarded as non-stationary.
  - threshold tuning (corpus B): N=0 stores 194.5k blobs / 12.9MB and
    serves 383.6k hits (-0.8% wall / -2.4% user, 1 pair); N=64 stores
    20.9k / 7.0MB, 26.9k hits, 530k below-N serves; N=256 stores 1.8k /
    5.2MB and is effectively off (376 hits, +4.8% user). The shape gate
    cuts 244k no-arg deferrals. The expected ceiling at N=0 is <=0.5% of
    wall (383.6k hits x ~0.5-1us saved), below the noise floor.
  - the "wire cache off => serialization dominates" premise is
    disproven for Instance roots: corpus B OFF is not slower than
    corpus A OFF, the funnel wire-cache hit rate is 22% on the smoke
    corpus, and 91% of engaged puts serialize to <=64B.
  All P2b wiring is reverted; the P1 scaffold (`proxy.rs`,
  `mypy/type_proxy.py`, `Options.native_type_proxy`,
  `TEST_NATIVE_TYPE_PROXY`, `_clear_native_resolvers` reset branch) is
  unchanged and the tree is `770fe596c` + this bullet. The P2b
  implementation is preserved as
  `/private/tmp/mypy-rs-1567-p2b.patch` for a future slice targeting a
  seam with real serialization volume (non-Instance roots or nested
  writes), not root Instance funnels. Final-tree gates: cargo
  type_kernel 2,791/11, fmt + clippy clean, testtypes 3,424/7,
  testcheck 8,198/15/7/0, fine-grained 747/27, daemon 38, cold
  self-check clean 348.
- wave 69A symbol-node enum + writer + cache payload contract (G0.5,
  issue #1566): new `crates/ast_serialize/src/sym_node.rs` ports the
  `mypy/nodes.py` fixed-format `write()` field sets - `MypyFile`,
  `SymbolTable` / `SymbolTableNode` (sorted bare-key dict, skip
  `__builtins__` / `no_serialize`, cross-ref prefix rule with the
  `from_module_getattr` exception), `TypeInfo` (+ `ClassDef`), `Var`
  (19 flags), `FuncDef` (14 flags; the issue's "FuncData" = the
  FuncItem/FuncDef write fields), `Decorator`, `OverloadedFuncDef`,
  `TypeVarExpr`, `ParamSpecExpr`, `TypeVarTupleExpr`, `TypeAlias` and
  `DataclassTransformSpec`. `SymNode` records are built from live
  PyO3 objects; the writer is pure Rust and mirrors the tag/field/flag
  order byte-for-byte. Type-valued fields, `Var.final_value` and
  `TypeInfo.metadata` stay opaque payloads captured through four
  Python callbacks (the G0.3 `Lambda.parameters` pattern: type capture
  rides `_write_type_cached`, so the F3 wire cache still serves).
  Defer contract: an unknown shape (`PlaceholderNode`, non-MypyFile
  root, unreadable fact) returns `None` and `MypyFile.write` runs;
  `PyAttributeError` / `PyAssertionError` / `PyNotImplementedError`
  map to the same defer, other PyErrs propagate (#1466 pattern).
  Production wiring: `mypy/cache_data.py` shim + `build.py`
  `write_cache` dispatch, activated from the new default-off
  `Options.native_cache_data` (opt-in; `TEST_NATIVE_CACHE_DATA` in test
  helpers): the hybrid write phase measured ~1.5x the Python writer per
  round (36.3ms vs 55.1ms per 57-tree round) with no format change, so
  the bridge stays off until the type payloads move; `stubs/ast_serialize.pyi`
  declares the seam. Cache-format decision: output is byte-identical, so
  `CACHE_VERSION` stays 13, the JSON path is untouched and
  `FileRawData` / the AST wire (`AST_WIRE_VERSION` 5) are unchanged;
  fine-grained never writes cache files (`State.write_cache` guard),
  so it is untouched. Parity evidence: new
  `mypy/test/testcachedata.py` (4 tests: live-tree parity over the
  built package + real typeshed deps, production-path engagement via a
  patched shim counter, cached-tree decode -> re-encode parity + astdiff
  symbol-table structural equality excluding `__builtins__`, defer
  contract) plus a temporary env-gated `MYPY_CACHE_DATA_AUDIT=1` audit
  (stripped before landing) that A/B'd every production write on the
  cold self-check: 811 module writes, 0 defers, 0 mismatches, re-run
  with the in-repo librt scratch (F3 `write_raw_bytes` splice active)
  likewise 811/0/0; warm re-run 811 `Metadata fresh`, 0 writes,
  cache-consuming. Gates: cargo ast_serialize 22/0 (+7 unit tests,
  was 15/0), fmt + clippy `--all-targets -D warnings` clean; AST
  suites 888 passed/75 skipped/2 xfailed (884 baseline + 4 new);
  testcheck 8,198/15/7/0 exact on the fastparse path and
  8,144/69/7/0 exact with `TEST_NATIVE_PARSER=1
  TEST_NATIVE_RESOLVER=1` (54 `_no_native_parse` skips); cold
  self-check clean 350 (= 348 + `mypy/cache_data.py` +
  `mypy/test/testcachedata.py`), warm clean and cache-consuming;
  fine-grained 747/27 + daemon 38; testfinegrainedcache 549/229.
  CI: `parity-ast` gains a cache-data step plus `mypy/cache_data.py`
  / `mypy/test/testcachedata.py` / `stubs/ast_serialize.pyi` path
  triggers. Built in the private `mypy-rs-local-ast-1566` scratch dir;
  shared dirs untouched.
- wave 69B multi-alias import grouping, AST wire v6 (issue #1551):
  `import a, b` split into one `Import` node per alias in
  `tree.imports` because `serialize_import` pushed one
  `IMPORT_METADATA` record per alias (statement nodes were already
  correct; deps unaffected). The record now carries the statement's
  whole alias list (LIST_GEN + count + name/asname pairs, mirroring
  `IMPORTFROM_METADATA`), the writer pushes one record per statement,
  and `nativeparse.deserialize_imports` rebuilds a single
  `Import(names)`. `AST_WIRE_VERSION` 5 -> 6 with the caller constant
  (`mypy/nativeparse.py`) and `stubs/ast_serialize.pyi` updated; the
  dead `asname` field dropped from `ImportMetadata`; the frozen
  `stmt_legacy.rs` reference only loses the field. Single-alias
  imports use the same grouped shape (no dual record layout; the
  version bump covers the bytes). Tests: 5 data-driven
  `native-parser-imports.test` cases (multi-alias, asnames, mixed,
  unreachable, in-function; `format_reachable_imports` now renders one
  line per statement so a split is visible), fastparse differentials
  for tree.imports + statements (`test_multi_alias_import_grouping`)
  and dependency-discovery flags, a v6 golden blob
  (`test_v6_golden_multi_alias_import_metadata`), and 4 Rust record
  unit tests. Gates: cargo ast_serialize 19/0, fmt + clippy clean;
  AST suites 892/75/2 xfailed; testparse fastparse 250/74; testcheck
  8,144/69/7/0 exact local baseline (CI's 8,198/15/7); cold
  self-check clean 348; fine-grained 747/27 + daemon 38.
- wave 70A expression dual-write node shadow scaffold (G1.0a, issue
  #1572): new `crates/type_kernel/src/node_mirror.rs` - a thread-local
  per-node store keyed by `identity::handle_for` with strong pins, one
  merged record per object holding the `RefExpr` binding scalars
  (`kind`, target `node` fullname, `_fullname`, `is_new_def`,
  `is_inferred_def`) plus the last `analyzed` replacement class name
  (record-only) and monotonic capture counters; `reset` drops entries
  and pins but never touches `identity` (`rust_mirror_reset` stays the
  single owner). Nine `rust_node_mirror_*` pyfunctions + lib.rs
  registrations + `stubs/type_kernel.pyi` declarations. New
  `mypy/nodes_mirror.py` activation hook behind default-off
  `Options.native_ast_mirror` (`TEST_NATIVE_AST_MIRROR`, probe in
  helpers): patches `__setattr__` on `RefExpr` / `CallExpr` /
  `IndexExpr` / `OpExpr` (all use `__slots__` but define no
  `__setattr__`, so the patch composes; `ClassDef.analyzed` is G2 and
  out of scope). Lazy adoption: constructor-default writes stay out of
  the store, the first non-default binding write adopts with the full
  post-write record; capture failures increment an audit counter and
  never break the write. Build wiring: activate in
  `BuildManager.__init__` next to the type mirror; reset in
  `_clear_native_resolvers` after the proxy reset and before the
  preserving mirror reset. Site list (all captured by the patch, no
  explicit touch calls needed) is documented in the module docstring:
  semanal.py 4347-4351 / 4572-4573 / 4789 / 4994 / 5501 / 5773-5776 /
  5964-5965 / 6020 / 7247-7249, checker.py 2372 / 2416 / 2525 / 4088 /
  6726 / 10716-10717, server/astmerge.py 308-311. No read flip, no
  cache/plugin/astmerge-identity change. Gates: cargo type_kernel
  2,798/11 (+7 units), fmt + clippy `-D warnings` clean; testtypes
  3,436/7 (+12 `NativeAstMirrorSuite` from 3,424/7) both gate states;
  testcheck 8,198/15/7/0 exact in kernel and kernel+shadow modes
  (shadow-on ~7% slower: 214-234s vs 219-294s load-dependent);
  fine-grained 747/27 + daemon 38 with the shadow on; testgraph 12,
  merge 41/1, diff 79 with the shadow on; cold self-check clean 351
  (350 + `mypy/nodes_mirror.py`).
- wave 71A remaining expression shadow fields + G1.1 read-channel design
  (G1.0b, issue #1576): extends the #1572 node shadow, still record-only.
  New per-field records on the same `NodeShadow` (`HashMap<String,
  FieldValue>`: kind / flag / name / kinds variants) behind
  `rust_node_mirror_capture_field_kind` / `_flag` / `_field_name` /
  `_field_kinds` and the `rust_node_mirror_field` / `_fields` /
  `_field_captures` readers. Captured: `method_type` (Op/Index/Unary),
  `method_types` (Comparison), `as_type` (Op/Index/Str),
  `right_always`/`right_unreachable` (Op), `def_var` (Member), the
  RefExpr/NameExpr `is_special_form`/`is_alias_rvalue`/`type_guard`/
  `type_is` flags; type-valued fields record the value class name, flags
  the bool, `def_var` the target fullname, `method_types` the per-item
  class list. `ComparisonExpr`/`StrExpr`/`UnaryExpr` join the patched
  `__setattr__` class list (`NameExpr`/`MemberExpr` already route through
  the `RefExpr` patch); the append-only `method_types` list is invisible
  to `__setattr__`, so `visit_comparison_expr` records it once after the
  loop via the new `nodes_mirror.touch` hook (empty-list no-op keeps
  lazy adoption). Same strong pins, identity namespace and reset
  contract as G1.0a; no read flip, option stays default-off and absent
  from `OPTIONS_AFFECTING_CACHE`. PR body documents the G1.1 read
  channel (first consumer reads `rust_node_mirror_field(handle, field)`;
  G1.1 extends type-valued records with F2 wire bytes on the same
  handle, Python stays authoritative). Engagement audit on a synthetic
  cold build (env-gated `MYPY_TK_AST_MIRROR_AUDIT`, stripped):
  `capture_method_type` 4 (incl. checker.py:6701), `touch.method_types`
  715, `capture_as_type` 1363, `capture_def_var` 1,
  `capture_is_alias_rvalue` 143, `capture_is_special_form` 1427,
  `capture_right_always` 106, `capture_right_unreachable` 90,
  `capture_type_guard` 1, `capture_type_is` 1, 0 capture failures.
  Tests: `NativeAstMirrorFieldSuite` (+15 Python) and
  `node_field_shadow_tests` (+8 Rust). Gates: cargo type_kernel
  2,806/11, fmt + clippy `-D warnings` clean; testtypes 3,451/7;
  testcheck 8,198/15/7/0 exact shadow off and on; fine-grained 747/27 +
  daemon 38 shadow-on; cold self-check clean 351.
- wave 71B statement/def metadata shadow scaffold (G2.0, issue #1577):
  extends the G1.0a node shadow with a record-only metadata store for the
  statement and def families, same gate (`Options.native_ast_mirror`,
  default off) and identity base, kept in its own Rust store (G2 section
  of `crates/type_kernel/src/node_mirror.rs`) so the parallel G1.0b
  expression work merges independently. One record per object holds
  tagged field values (`none`/`bool`/`int`/`str`/`obj`/`list`); object
  values become class/fullname markers only (`InstanceType`,
  `NameExpr:mod.x`) and are never serialized. Six new pyfunctions
  (`rust_node_mirror_capture_meta` / `_meta` / `_meta_captures` /
  `_meta_drop` / `_meta_reset` / `_meta_entry_count`) + lib.rs
  registrations + stub declarations. Python side (G2 section of
  `mypy/nodes_mirror.py`) patches `__setattr__` on `ImportBase`,
  `AssignmentStmt`, `ForStmt`, `WithStmt`, `IfStmt`, `MatchStmt`,
  `TypeAliasStmt`, `FuncDef`, `OverloadedFuncDef`, `Decorator`,
  `ClassDef` and `Var`; lazy adoption skips constructor defaults on a
  never-adopted node, then each field keeps its last captured value.
  Tracked fields: AssignmentStmt `type`/`unanalyzed_type`/`is_alias_def`/
  `is_final_def`/`invalid_recursive_alias`; ForStmt `index`/`index_type`/
  `unanalyzed_index_type`/`inferred_item_type`/`inferred_iterator_type`;
  WithStmt `analyzed_types`; IfStmt `unreachable_else`; MatchStmt
  `subject_dummy`; TypeAliasStmt `alias_node`; ImportBase `assignments`;
  FuncDef `type`/`unanalyzed_type`/`_fullname`/`abstract_status` plus
  the 18 FuncBase/FuncItem/FuncDef bool flags; OverloadedFuncDef
  `items`/`impl`; Decorator `func`/`var`; ClassDef `info`/`analyzed`
  (`ClassDef.analyzed` was explicitly G2); Var `_fullname`/`type`/
  `setter_type`/`info`/`final_value` plus its 22 metadata flags.
  `ImportBase.assignments` is a list append the patched hook cannot see,
  so `mypy/semanal.py:process_import_over_existing_name` calls
  `nodes_mirror.touch(node, "assignments")` - the only explicit touch
  site; every other G2 site routes through the patch (semanal.py
  1488-1961 def/overload/class metadata, 4494-5660 AssignmentStmt /
  type-alias fields, 6912 / 7021 / 7181 ForStmt / WithStmt /
  TypeAliasStmt; checker.py 7092 / 7467-7468 / 7942; aststrip.py 112-220
  reset sites). Astmerge identity is untouched: `replace_object_state`
  copies slots through `setattr`, so a surviving identity re-registers
  through the same hook (pinned by
  `test_replace_object_state_reregisters_surviving_identity`). No read
  flip, no cache / `OPTIONS_AFFECTING_CACHE` change. Tests:
  `NativeStmtDefMirrorSuite` (20) covers every claimed field, lazy
  adoption, last-value-wins, drop/reset, gate-off, capture-failure
  resilience, the explicit touch, aststrip-style clear writes and the
  shared identity namespace, plus 5 Rust `g2_meta_tests`. Gates: cargo
  type_kernel 2,803/11 (+5 units), fmt + clippy `-D warnings` clean;
  testtypes 3,456/7 (3,436/7 + 20) both gate states; testcheck
  8,198/15/7/0 exact in kernel and kernel+shadow modes; fine-grained
  747/27 + daemon 38 + merge 41/1 + diff 79 shadow-on (905/28
  combined); cold self-check clean 351 in kernel and kernel+shadow
  modes.

- wave 72A namespace entry funnel + dual-write capture scaffold (G3.0a, issue #1581):
  first G3 slice per docs/plans/2026-09-11-g3-symbol-table-brief.md. New
  `crates/type_kernel/src/symtable_mirror.rs` keyed by (owner table handle,
  name) with table generation + monotonic capture seq, strong Py<PyAny> pins,
  and the by-node reverse index for flag refresh; ten
  `rust_symtable_mirror_*` pyfunctions + lib.rs registrations + stub
  declarations; reset drops entries/pins only and never touches
  `identity::reset`. New `mypy/symtable_access.py` (`put_names_entry` ->
  PutResult mirroring the committed branch of
  `add_symbol_table_node`) and `mypy/symtables_mirror.py` (class patches
  on `SymbolTable.__setitem__/__delitem__/pop` and
  `SymbolTableNode.__setattr__`, G1.0a pattern; routed writes use raw
  `dict.__setitem__` so every patched `__setitem__` counts as
  `bypass.put`). Gate `Options.native_symtable_mirror` default off, out
  of OPTIONS_AFFECTING_CACHE, plus `TEST_NATIVE_SYMTABLE_MIRROR` in test
  helpers and the per-build/per-recheck reset branch in
  `_clear_native_resolvers` (before the preserving type-mirror reset).
  Routed the semanal adding funnel through the accessor
  (`prepare_file`/__builtins__, builtins core classes + special vars,
  the implicit-attr put, `add_global_symbol`,
  `add_symbol_table_node` committed put, `add_redefinition`); refusals
  never call it so they leave no shadow trace by construction. Tests:
  `NativeSymtableMirrorSuite` (12: routed put, placeholder replace,
  two refusal pins, unknown-shape defer, generations, ref-flag refresh,
  cross/plugin flags, deletes, shared identity, failure-safety, gate-off)
  plus the documented known-bypass pin for
  `rust_remove_imported_names_from_symtable` (semanal_visitor.rs:458
  `PyDict::del_item`, G3.0b reroute). No read flip, no cache-format
  change, no plugin-visible change. Gates: cargo type_kernel 2,818/11,
  fmt + clippy clean; testtypes 3,483/7 both gate states (3,471 baseline
  + 12); testcheck 8,198/15/7 exact both gate states; fine-grained 747/27
  + daemon 38 + merge 41/1 + diff 79 shadow-on; cold self-check clean 353.

- wave 72B G3.0b direct-write and delete sweep (issue #1581): reroutes
  remaining direct ``SymbolTable`` writes through ``put_names_entry`` and
  all ``del table.names[...]`` / ``table.names.pop(...)`` deletes through
  two new accessor functions in ``mypy/symtable_access``:
  ``delete_names_entry(table, name)`` (hard delete via
  ``dict.__delitem__``) and ``delete_names_entry_safe(table, name)``
  (soft delete via ``dict.pop(..., None)``). Both bypass the patched
  ``SymbolTable.__delitem__`` / ``pop`` (matching ``put_names_entry``'s
  ``dict.__setitem__`` pattern) and clean up the shadow record via
  ``symtables_mirror._delete`` only when the mirror is active. Routed
  puts (9 sites): ``checker.intersect_instances`` /
  ``intersect_instance_callable`` (3), ``plugins.common`` (5: 2 method
  adds, 2 redefinition renames, 1 attribute add), ``plugins.attrs`` (5:
  __hash__ copy, Self tvar, property var, descriptor attr, magic attr),
  ``plugins.dataclasses`` (4: Self tvar, 2 property vars, classvar),
  ``semanal_namedtuple`` (5: field var, method, Self tvar, 2 redef
  renames), ``semanal_newtype`` (1: __init__), ``semanal_enum`` (1: enum
  member). Routed deletes (6 sites): ``semanal.add_typing_extension_aliases``
  (``pop`` -> ``delete_names_entry_safe``), ``semanal.create_alias``
  (``del`` -> ``delete_names_entry``), ``plugins.dataclasses.reset_init_only_vars``
  (``del``), ``server.aststrip`` (2: file names clear, attribute remove),
  ``server.update`` (1: module symbol remove). The Rust bypass in
  ``rust_remove_imported_names_from_symtable`` (semanal_visitor.rs) now
  calls ``crate::symtable_mirror::delete(names, key_str)`` before
  ``PyDict::del_item`` for each removed key, retiring the G3.0a
  known-bypass pin. Gate is ``Options.native_symtable_mirror`` (default
  off); all changes are no-ops when the gate is off. No test files
  touched, no wire/cache/``OPTIONS_AFFECTING_CACHE`` change. Gates: cargo
  type_kernel build + clippy + fmt clean; all 12 edited Python files
  import and parse; comment-block check clean; ``git diff --check``
  clean.

- wave 72C G3.0c extended TypeInfo meta fields (issue #1592):
  extends the symtable shadow's TypeInfo meta tracking from 5 core
  fields to 21 by adding 16 post-construction fields (bool flags:
  ``is_final``, ``is_protocol``, ``is_enum``, ``is_disjoint_base``,
  ``is_type_check_only``, ``is_intersection``, ``fallback_to_any``,
  ``meta_fallback_to_any``, ``runtime_protocol``, ``bad_mro``; scalars:
  ``type_vars`` count, ``declared_metaclass``/``self_type``/
  ``dataclass_transform_spec`` fullnames, ``deprecated`` string,
  ``default_depends`` fullname). A new
  ``rust_symtable_mirror_meta_put_field(info, field, value)`` FFI
  stores string-encoded values in an ``extra: HashMap<String, String>``
  on ``MetaEntry``; core fields keep the existing ``meta_put`` path.
  ``meta_put`` clones the existing ``extra`` before overwriting so
  extended fields survive a core-field refresh; ``meta_lookup`` now
  returns the ``extra`` dict. Python side: ``_META_FIELDS`` split into
  ``_META_CORE`` (5) + ``_META_EXTRA`` (16), ``_encode_extra()``
  serializes bool/int/str/fullname, ``_META_ADOPTED`` set tracks
  TypeInfo adoption for lazy-skip of baseline writes, and ``reset()``
  clears it. A core-field write also refreshes all non-baseline extras.
  Tests: ``NativeSymtableMetaExtraSuite`` (11 tests). Gate is
  ``Options.native_ast_mirror`` (default off, not in
  ``OPTIONS_AFFECTING_CACHE``). No wire/cache format change. Gates:
  cargo type_kernel build + clippy + fmt clean; symtable mirror tests
  35 passed; full testtypes 3519 passed/7 skipped/1 pre-existing
  failure; cold self-check pending.
- wave 73A expression node wire read channel for type-valued fields
  (G1.1, issue #1576): extends the G1.0b expression node shadow to
  store F2 wire bytes for the four type-valued expression fields
  (`method_type`, `as_type`, `type_guard`, `type_is`) so Rust can
  serve reads without crossing back to Python. New `Wire` variant on
  `FieldValue` (`{ kind: Option<String>, bytes: Vec<u8> }`) in
  `node_mirror.rs` with two pyfunctions:
  `rust_node_mirror_capture_field_wire(obj, field, kind, wire)` for
  capture and `rust_node_mirror_field_wire(obj, field)` returning
  `Option<(Option<String>, PyBytes)>` for reads. Python side
  (`mypy/nodes_mirror.py`): `_capture_field` now routes Type-valued
  writes through `rust_node_mirror_capture_field_wire` when
  `isinstance(value, MypyType)`, serializing via
  `types_mirror._fresh_bytes`; non-Type values keep the G1.0b
  `rust_node_mirror_capture_field_kind` path. `read_field_type(handle,
  field)` deserializes wire bytes through `ReadBuffer` + `read_type`
  so a consumer gets a live `Type` object back. Gate is the existing
  `Options.native_ast_mirror` (default off, not in
  ``OPTIONS_AFFECTING_CACHE``). No read flip, no cache-format change, no
  plugin-visible change. Tests: updated 6 `NativeAstMirrorFieldSuite`
  tests to expect `("wire", kind, ...)` for type-valued fields; new
  `NativeAstMirrorWireSuite` (11 tests: wire round-trip for Instance
  and AnyType, cleared-type routing to kind variant, `read_field_type`
  helper, wire overwrites, NotParsed baseline, setattr engagement).
  Gates: cargo type_kernel 2,806/11, fmt + clippy clean; testtypes
  3,521/7 (gate off and on); testcheck 8,144/69/7 exact; fine-grained
  747/27; cold self-check clean.

- wave 73 G3.0d astmerge re-registration for symtable shadow (issue
  #1581 follow-up): the in-place mro/bases mutation re-capture hook in
  ``process_type_info`` (astmerge.py:398-403) is already in place from
  G3.0c. This wave adds two test pins verifying the remaining astmerge
  paths keep the shadow consistent: (a)
  ``test_astmerge_replace_object_state_typeinfo_recaptured``: when
  ``fixup()`` calls ``replace_object_state(new, old,
  skip_slots=("special_alias",))``, the ``setattr`` calls on
  ``_META_FIELDS`` trigger ``_typeinfo_setattr`` -> ``_capture_meta``
  on the surviving ``new`` identity, so the meta record appears on
  ``new`` with the old content and a higher seq; (b)
  ``test_astmerge_replace_nodes_in_symbol_table_refreshes_flags``:
  when ``replace_nodes_in_symbol_table`` writes ``node._node = new``
  (a ``_FLAG_FIELDS`` slot), ``_symtable_node_setattr`` ->
  ``_refresh_flags`` updates the ``node_fullname`` in the existing
  record (seq is not bumped — ``refresh_flags`` updates content, not
  identity). No Rust or production code change; test-only. Gates:
  symtable mirror suite 27 passed; full testtypes 3522 passed/7
  skipped.

- wave 74 H1b native ConditionalTypeBinder frame-stack store (issue
  #1596): new `BinderStore` thread-local in
  `crates/type_kernel/src/binder.rs` mirrors the unreachable and
  `suppress_unreachable_warnings` flags on the Python binder's frame
  stack, with O(1) cached-count queries replacing Python's
  `any(f.unreachable for f in self.frames)` scan called on every
  statement. Ten pyfunctions (`rust_binder_new` / `_reset` /
  `_push_frame` / `_pop_frame` / `_is_unreachable` /
  `_is_unreachable_warning_suppressed` / `_set_unreachable` /
  `_set_top_unreachable` / `_suppress_unreachable_warnings` /
  `_frame_count`) registered in `lib.rs`. The store starts with one
  frame (matching Python's `__init__` which creates `self.frames =
  [Frame(...)]`); `pop_frame` guards against emptying the stack.
  `set_top_unreachable(v)` syncs after `update_from_options` sets
  `self.frames[-1].unreachable = not frames` — can set OR clear the
  flag. The type-merging core (`update_from_options`, `assign_type`,
  `allow_jump`, `_put`/`_get`) stays in Python: `Frame.types` dicts
  use Python tuple keys containing live `Var` objects which cannot be
  hashed in Rust. Break/continue/try frame tracking also stays
  Python-side (`allow_jump` doesn't modify `self.frames`). Gated by
  `Options.native_binder` (default off, NOT in
  `OPTIONS_AFFECTING_CACHE`); shim in `mypy/binder.py` delegates
  `is_unreachable()` / `is_unreachable_warning_suppressed()` to the
  Rust store and falls back to the Python `any()` scan on exception.
  Covered by `NativeBinderFrameSuite` in `mypy/test/testtypes.py` (10
  tests: direct seam push/pop/unreachable/suppress/reset/clear,
  option-default-off, gate-off vs gate-on differential through the
  real `ConditionalTypeBinder`). Gates: cargo type_kernel clean, fmt +
  clippy clean, testtypes 3543 passed/7 skipped, cold self-check clean.

- wave 75 H1c native check__exit__return_type (issue #1597): the Rust
  seam in `checker_functions.rs` mirrors
  `TypeChecker.check__exit__return_type` (checker.py:3951-3983) as a
  live-PyO3-object port (zero wire bytes for the `defn` node). Rust reads
  `defn.type` via PyO3 `is_instance` against `mypy.types.CallableType`,
  calls Python's `get_proper_type` + `has_bool_item` (both already
  native) on the return type, calls `all_return_statements` (already
  native) on the def body, and checks each return's `expr` is a
  `NameExpr` with `fullname == "builtins.False"`. Returns `Option<bool>`:
  `Some(true)` = emit error, `Some(false)` = no error, `None` = defer.
  The `self.msg.incorrect__exit__return(defn)` emission stays
  Python-side. Gated by `_native_checker_active` (existing wiring, no
  build.py change) and covered by `NativeCheckExitReturnTypeSuite` in
  `mypy/test/testtypes.py` (7 direct seam calls + 7 gate-off vs gate-on
  parity differentials), plus pure decision unit tests in
  `checker_functions.rs`. Gates: cargo type_kernel clean, fmt + clippy
  clean, testtypes 3557 passed/7 skipped, cold self-check clean.
- wave 75 H1d native check_final_deletable (issue #1599): the Rust
  seam in `checker_functions.rs` mirrors
  `TypeChecker.check_final_deletable` (checker.py:4053-4059) as a
  live-PyO3-object port (zero wire bytes). Rust walks
  `typ.deletable_attributes` (list of strings), looks up each attr in
  `typ.names` (SymbolTable dict), checks `isinstance(node.node, Var)`
  and `node.node.is_final` via PyO3, and returns the list of offending
  attr names. Returns `Option<Vec<String>>`: `Some(names)` = emit fail
  for each, `None` = defer. The `self.fail(message_registry.CANNOT_MAKE_DELETABLE_FINAL, ...)`
  emission stays Python-side. Gated by `_native_checker_active`
  (existing wiring, no build.py change) and covered by
  `NativeCheckFinalDeletableSuite` in `mypy/test/testtypes.py` (6 direct
  seam calls + 6 gate-off vs gate-on parity differentials), plus pure
  decision unit tests in `checker_functions.rs`. Gates: cargo
  type_kernel clean, fmt + clippy clean, testtypes 3555 passed/7
  skipped, testcheck 8144 passed/69 skipped/7 xfailed, cold self-check
  clean.

- wave 75 H1g native is_valid_defaultdict_partial_value_type (issue
  #1606): the Rust seam in `checker_functions.rs` mirrors
  `TypeChecker.is_valid_defaultdict_partial_value_type`
  (checker.py:6295-6318) as a wire-type bool predicate. Rust decodes
  the proper type, returns `Some(false)` for non-Instance, `Some(true)`
  for 0-arg Instance, and for 1-arg Instance checks the arg proper type
  is `UninhabitedType` or `NoneType` (plus `TypeVarType` when
  `old_type_inference` is True), else `Some(false)`. `Some(false)` for
  2+ args. Defers (`None`) on a `TypeAliasType` arg that cannot be
  resolved. Gated by `_native_checker_active` (existing wiring, no
  build.py change) and covered by
  `NativeIsValidDefaultDictPartialValueTypeSuite` in
  `mypy/test/testtypes.py` (9 direct seam calls + 8 gate-off vs gate-on
  parity differentials).

- `rust_is_defined_in_base_class` (issue #1601, mypy.checker) — mirrors
  `TypeChecker.is_defined_in_base_class` (checker.py:10168-10173): a pure
  bool predicate over a live `Var`. Returns `False` when `var.info` is
  falsy, `True` when `var.info.fallback_to_any`, else walks `info.mro[1:]`
  and returns `True` if any base's `names.get(var.name)` is not None. Rust
  reads the live Var via PyO3 (`info` truthiness via `is_true`,
  `fallback_to_any` bool, `name` string, `mro` list, per-base `names.get`)
  and returns `Option<bool>`, mirroring `rust_is_writable_attribute`
  (live-object, no wire decode). Defers (`None`) only on an unreadable
  attribute so the pure-Python body re-runs unchanged. Gated by
  `_native_checker_active` (existing wiring, no build.py change) and
  covered by `NativeIsDefinedInBaseClassSuite` in
  `mypy/test/testtypes.py` (direct seam calls for all branches plus
  gate-off vs gate-on parity differentials).

- wave 75 H1f native is_definition (issue #1603): the Rust seam in
  `checker_functions.rs` mirrors `TypeChecker.is_definition`
  (checker.py:6173-6188) as a live-PyO3-object port (zero wire bytes).
  Rust reads the live `Lvalue` via PyO3 `is_instance` against
  `mypy.nodes.NameExpr` and `mypy.nodes.MemberExpr`: a `NameExpr` with
  `is_inferred_def=True` returns `Some(true)`, a `NameExpr` whose `node`
  is a `Var` with `type is None` returns `Some(true)`, a `MemberExpr`
  with `is_inferred_def=True` returns `Some(true)`, all other shapes
  return `Some(false)`. Defers (`None`) on an unreadable attribute so
  the pure-Python body re-runs. Gated by `_native_checker_active`
  (existing wiring, no build.py change) and covered by
  `NativeIsDefinitionSuite` in `mypy/test/testtypes.py` (7 direct seam
  calls + 5 gate-off vs gate-on parity differentials). Gates: cargo
  type_kernel clean, fmt + clippy clean, testtypes 3581 passed/7
  skipped, testcheck 8144 passed/69 skipped/7 xfailed, cold self-check
  clean.

- `visit_instance_nominal_live` + `map_supertype_live_non_generic`
  (issue #1619, PR #1645): the resolver snapshot is fed per SCC after
  semanal while the Python wire map publishes at module-top-level
  completion, so semanal-time seams see `in_map=True, in_snap=False`
  and defer. `LiveNominal` (subtypes.rs) reads the nominal prelude
  from the live `TypeInfo` at decision time (nothing stored, so no
  pre-inference variance pinning — the reason on-demand sealing was
  rejected in #1490): `fallback_to_any && !proper` -> true; any MRO
  base with a non-empty `_promote` -> defer; `alt_promote.type is
  right.type` (identity) -> true; the nominal gate (`has_base` /
  `builtins.object` / `TYPED_NAMEDTUPLE_NAMES`) with non-protocol miss
  -> false and protocol right -> defer; a nominal branch with a
  non-generic non-variadic right -> true (empty `type_params` zip).
  The map seam takes Python's second fast path (maptype.py:206-208)
  over the live supertype when it has no type vars. Covered by
  `NativeSnapshotGapLiveNominalSuite` in `mypy/test/testtypes.py`
  (10 tests). Measured (env-gated probe, stripped before landing):
  snapshot-miss sites 45 -> 19 on the cold self-check; the four
  PEP 695 variance tests green. Gates: cargo 2822/11, testtypes
  3754/7, testcheck 8198/15/7 exact, fine-grained 747/27, daemon 38,
  cold self-check clean.
- `write_ffi_constraint` extras channel (issue #1618, PR #1646):
  `Constraint.extra_tvars` (the polymorphic reverse-inference frame's
  payload, constraints.py:1810-1812) now ride the Rust->Python FFI
  blob as a trailing bare-int count plus that many `Type` records
  (`origin | op | target | extras-count | extras`). The whole-call
  extras guard is removed from `rust_infer_constraints_full`; the
  three Rust->Python readers (`_try_native_constraint_builder`,
  the callable-arguments seam, the directed-args filter seam) consume
  the section via `_read_constraint_extras`, and `_restore_extra_tvars`
  relinks decoded extras onto the live donor variables by id (the
  #1171/#1215 identity precedent). The Python->Rust `_write_constraint`
  stays 3-field by design (documented there): solve pre-merges extras
  into `originals`, and any future extras-needing reader must grow the
  section on both sides together. The dead `callable_with_vars_reachable`
  (visitor.rs, consumer gone since wave 37) is deleted with its tests;
  `run_any_constraints` preserves extras through
  `constraint_to_rep`/`rep_to_constraint`; the stale "wire drops
  ParamSpec/TVT meta_level" comments are corrected (carried since
  #1417). **No `CACHE_VERSION` bump**: the section is in-process FFI
  only; the persistent wire format is untouched (the issue's
  extend-`CallableType` direction was stale — `extra_tvars` live on
  `Constraint`, and the wire `CallableType` already carries
  `variables`). Measured: `rust_infer_constraints_full`
  22,184 calls / 56 defers -> 22,131 / 5; extras delivered natively on
  42 constraints across 25 calls; the 95-call origin-rebuild
  uniqueness boundary is kept (relaxing it mis-solves free ParamSpecs
  to `Never`; #1621 owns that wall). Gates: cargo 2812/11, testtypes
  3748/7, testcheck constraint/inference/overload subset 1043/7,
  full parity green in CI, cold self-check clean (353 files).
- `resync_var_identities_list` (issue #1623, PR #1644): `remove_trivial`
  was the last expand-family seam with `_needs_python(meta_gate=True)`.
  The decoded list is now re-linked onto the live input objects via a
  list-shaped counterpart of `resync_var_identities` (#1215) using
  `_VarIdentityCanonicalizer(seed=..., strict=True)`: an occurrence
  with no structurally-equal live original (or same id / different
  content) defers to the pure-Python body, so no doppelganger escapes
  `freeze_all_type_vars`' in-place mutation or id-keyed substitution.
  `_needs_python` drops the dead `meta_gate` kwarg; var-bearing results
  stay out of `_expand_remove_trivial_cache` (only var-free results
  cache); the now-unused `canonicalize_fresh_vars_reported_list` is
  deleted. The issue's other items were audited stale before porting:
  `remove_dups` already landed (wave-62C, 41/41 native, 0 defers),
  `parent_error` is a permanent floor (B6 audit), and the `Name@line`
  misses were alias-caused (wave-56 #1493). Covered by
  `NativeRemoveTrivialFreshVarSuite` in `mypy/test/testtypes.py`
  (engagement-proving spies, plus `instance_type` / `type_guard`
  relink and cache-gate regressions). Measured: gate defers 459
  (442 fresh-var) -> 17, native-ok 3,027 -> 3,467, strict-relink
  defers 0 corpus-wide. Gates: cargo 2822/11, testtypes 3761/3,
  full parity green in CI, cold self-check clean.
- Phase-1 wire-traffic audit (issue #1624, docs PR #1643, no code):
  `docs/plans/2026-09-14-perf-wire-traffic-audit.md` persists the
  counts audit that exhausts the issue's fix direction 4: only 435 of
  1,551,898 serialization events (0.028%) are serialized-then-deferred,
  and the unconsumed bucket is identity-restoration keys (#1623), not
  closeable gates; 8,307,304 seam calls decide 99.966% natively. Ranks
  the top-15 hot seams by a calls x (0.63us fixed FFI + 0.04us/byte)
  cost proxy (`rust_has_recursive_types`, `rust_callable_is_generic`,
  `rust_check_argument_types_plan`, `rust_copy_modified`,
  `rust_expand_type` lead) with fix shapes (live-object interface,
  batching, per-SCC resolver incrementalism, Python-side gating).
  Follow-ups filed: #1640 (types.py short-call seams), #1641
  (dirty-driven per-SCC resolver update), #1642 (check_call cluster).
  #1624 stays open as the standing perf blocker.
- Hot short-call seam retirement (issue #1640, PR #1648): fix shape
  (d, delete) from the #1624 audit — the serialize-round-trip native
  fast paths for `is_generic`, `has_recursive_types`,
  `can_be_true/false_default`, `is_var_arg`, `is_kw_arg`, `min_args`,
  `max_possible_positional_args`, `TupleType.length`, `UnionType.length`
  in `mypy/types.py` cost more in wire traffic than the decision is
  worth, so the 9 `_native_*` helpers + 12 `_rust_*` imports are
  deleted (112+/198-) and the pure-Python bodies answer directly (the
  Rust fns stay registered for direct-seam tests). Covered by
  `NativeHotSeamsRetiredSuite` in `mypy/test/testtypes.py`. Measured
  proxy saving ~6.97s across ~1.58M seam calls (-> 0 on every
  retired seam). Residuals: `rust_is_literal_type_like` (0.32s,
  typeops surface, out of scope) and the copy/flatten/expand floors,
  which need a live-object return interface. Gates: cargo 2812/11,
  testtypes 3774/7, testcheck 8144/69/7 exact, full parity green in
  CI, cold self-check clean (353 files).
- Dirty-driven per-SCC resolver upkeep (issue #1641, PR #1652):
  Python-side content-signature diff over exactly the post-seal fields
  (`_promote`, `alt_promote`, type-var variance) in `mypy/build.py`
  (`_native_builtins_sig` / `_split_native_pending`, sig map on
  `BuildManager`, reset in `_clear_native_resolvers`); unknown shapes
  fail safe to re-push. The issue's mark-at-`add_type_promotion`
  direction was tried and dropped: it passed single-process but failed
  parallel self-check (155 errors) because the fixup
  cache-load backwards-promotion hack (`mypy/fixup.py:124-130`,
  mirrored in `fixup.rs:651-665`) grows `_promote` with no semanal
  hook observing it. Process-local by construction, so no cross-worker
  protocol. Measured: builtins re-pushes 53,563 -> 1 (`-n0`) / 4
  (workers). Covered by `NativeResolverSigSuite` in
  `mypy/test/testtypes.py` (8 tests). Gates: cargo 2812/11, testtypes
  3779/7, testcheck 8144/69/7 exact, fine-grained 747/27,
  finegrainedcache 549/229, daemon 38, cold + warm (cache-consuming)
  self-check clean.
- `rust_walk_dependency_visitor` (issue #1632, PR #1651): the full
  `DependencyVisitor` walk (`mypy/server/deps.py`, ~52 `visit_*`
  methods) over live PyO3 objects (zero wire bytes) in the new
  `crates/type_kernel/src/depswalk.rs` (~2,100 lines), reusing the
  ported `collect_triggers` / `attribute_triggers_walk` kernels; any
  unreadable fact defers (`None`) so Python re-runs (incl. re-raising
  asserts). The Python walk stays as the fallback (deletion would
  remove the strangler safety net). Covered by
  `NativeServerDepsWalkSuite` in `mypy/test/testtypes.py` (14 tests).
  Measured: Python-body walk hits 12,410 -> 0 across the
  fine-grained corpora. Gates: cargo 2812/11, testtypes 3833/7,
  testdeps 230, fine-grained 747/27, finegrainedcache 549/229,
  daemon+merge+diff 158/1, testcheck 8198/15/7 exact, cold self-check
  clean.
- Pattern-check driver heads (issue #1633, PR #1650): six classifiers
  in `crates/type_kernel/src/checkpattern.rs` (seq head/result, map
  rest, or-filter, class alias/kw) with shims in `mypy/checkpattern.py`
  keeping verbatim-Python fallbacks; value/singleton bodies, or-union
  building, and the keyword member-access loop stay Python by design.
  Covered by `NativePatternCheckDriverSuite` in
  `mypy/test/testtypes.py` (33 tests). Measured: 1,345 Python-body
  hits -> 0 on the match corpus. Note: the cold self-check corpus
  contains zero match statements, so self-check is a no-regression
  gate only; engagement is measured on testcheck. Gates: cargo
  2833/11, testtypes 3819/7, testcheck 8198/15/7 exact, cold
  self-check clean.
- Stubgen printer collectors (issue #1636, PR #1649): three seams in
  the new `crates/type_kernel/src/stubgen.rs` behind the existing
  `_HAS_NATIVE_STUBGEN` gate in `mypy/stubgen.py`; the port caught a
  real semantics bug pre-merge (the unified unwrap loop ran both
  math and `not` phases where Python runs exactly one). Covered by
  `NativeStubgenPrinterSuite` in `mypy/test/testtypes.py` (12 tests).
  Measured: 1,710 Python hits -> 0 (1,372 native whole-subtree
  answers). `AnnotationPrinter`/`AliasPrinter` emission, the
  `visit_*` walk, and `stubutil.py` stay Python (deliberate residual).
  Gates: cargo 2815/11, teststubgen 373/1/2 text-exact, cold
  self-check clean.

### Ledger backfill (2026-09-14)

PRs `#1587`-`#1617` merged without appending their ledger records. The
entries below were reconstructed from the landed commits
(`git log` / `git show` on `main` @ `ae0590ece`), so they cite the PR,
commit, seam and suite that exist in the tree and invent no metrics.

#### G2 shadow series (record-only, default-off `Options.native_ast_mirror`)

- G2.1 `#1587` (`25b06dd60`) — `ImportBase` bool flags plus
  `Block.is_unreachable` in the statement/def metadata shadow
  (`mypy/nodes_mirror.py`); 3 new test defs.
- G2.2 `#1588` (`812323405`) — extended shadow fields for
  `OverloadedFuncDef` / `Decorator` / `ClassDef` / `FuncDef`; 4 new tests.
- G2.3 `#1589` (`cef9360a5`) — extended fields for `ClassDef` /
  `FuncDef` / `TypeAliasStmt` / `Decorator`; 6 new tests.
- G2.5 `#1590` (`07b6065ba`) — `ClassDef` extended shadow fields with the
  matching capture sites in `mypy/semanal_namedtuple.py`,
  `mypy/semanal_typeddict.py` and `mypy/server/aststrip.py`; 1 new test.
  No G2.4 exists in the log; none is invented here.
- G2.6 `#1591` (`b28d89398`) — remaining `FuncDef` shadow fields
  (`mypy/nodes_mirror.py`); coverage extended in place, no new test def.
- G1.1 `#1593` (`1e1c997e1`) — wire read channel for type-valued
  expression fields: `rust_node_mirror_capture_field_wire` /
  `rust_node_mirror_field_wire` store F2 wire bytes on the
  `FieldValue::Wire` variant and `read_field_type` decodes them back to a
  live `Type`; new `NativeAstMirrorWireSuite`.

#### H1 series (live-PyO3 decision heads, zero wire bytes)

Every entry below mirrors a pure decision head as a live-object PyO3 read
(the `rust_is_final_enum_value` / `rust_is_writable_attribute` pattern),
gated by `_native_checker_active` or `_native_semanal_active`, covered by a
gate-off vs gate-on differential `Native*Suite` in
`mypy/test/testtypes.py`, and defers (`None`) only on an unreadable
attribute so the pure-Python body re-runs unchanged.

- H1c `#1597` (`b6b7b87d5`) `rust_check_exit_return_type` —
  `TypeChecker.check__exit__return_type` (`mypy/checker.py:3999`, seam at
  :4007); `incorrect__exit__return` stays Python-side;
  `NativeCheckExitReturnTypeSuite`.
- H1d `#1599` (`82af5bb96`) `rust_check_final_deletable` —
  `TypeChecker.check_final_deletable`; `NativeCheckFinalDeletableSuite`.
- H1e `#1601` (`949a91db2`) `rust_is_defined_in_base_class` —
  `TypeChecker.is_defined_in_base_class`; `NativeIsDefinedInBaseClassSuite`.
- H1f `#1604` (`b7f7def3a`) `rust_is_definition` —
  `TypeChecker.is_definition` (`mypy/checker.py:6173`); already recorded
  above; `NativeIsDefinitionSuite`.
- H1h `#1606` (`f9f5e9703`) `rust_is_len_of_tuple` — the AST-shape front of
  `TypeChecker.is_len_of_tuple` (`mypy/checker.py:9607`, seam at :9611);
  `NativeIsLenOfTupleSuite`.
- H1i `#1607` (`828241026`) `rust_is_assignable_slot` —
  `TypeChecker.is_assignable_slot` (`mypy/checker.py:5581`);
  `NativeIsAssignableSlotSuite`.
- H1j `#1608` (`926be1b10`) `rust_is_noop_for_reachability` —
  `TypeChecker.is_noop_for_reachability` (`mypy/checker.py:4698`);
  `NativeIsNoopForReachabilitySuite`.
- H1k `#1609` (`d3d32f583`) `rust_is_literal_enum` —
  `TypeChecker.is_literal_enum` (`mypy/checker.py:10684`);
  `NativeIsLiteralEnumSuite`.
- H1l `#1610` (`e8cdd4402`) `rust_classify_unbound_return_typevar` — the
  unbound-return-TypeVar classification inside
  `check_unbound_return_typevar` (seam at `mypy/checker.py:2845`); Rust
  returns a tag the Python shim applies; `NativeUnboundReturnTypevarSuite`.
- H1m `#1611` (`bf92e6cd3`) `rust_check_incompatible_property_override` —
  `TypeChecker.check_incompatible_property_override`
  (`mypy/checker.py:7846`); `NativeIncompatiblePropertyOverrideSuite`.
- H1n `#1612` (`04671283e`) `rust_can_widen_in_scope` —
  `TypeChecker.can_widen_in_scope` (`mypy/checker.py:6734`);
  `NativeCanWidenInScopeSuite`.
- H1o `#1613` (`41eacc989`) `rust_check_untyped_after_decorator` —
  `TypeChecker.check_untyped_after_decorator` (`mypy/checker.py:7918`);
  `NativeCheckUntypedAfterDecoratorSuite`.
- H1p `#1614` (`b19b9733f`) `rust_is_base_class` —
  `SemanticAnalyzer.is_base_class` (`mypy/semanal.py:3839`);
  `NativeIsBaseClassSuite`. First H1 seam on the semanal gate.
- H1q `#1615` (`f3d1b1dcb`) `rust_is_overloaded_item` —
  `SemanticAnalyzer.is_overloaded_item` (`mypy/semanal.py:8369`);
  `NativeIsOverloadedItemSuite`.
- H1r `#1616` (`656cddbe3`) `rust_is_self_member_ref` —
  `SemanticAnalyzer.is_self_member_ref` (`mypy/semanal.py:6024`);
  `NativeIsSelfMemberRefSuite`.
- H1s `#1617` (`ae0590ece`) `rust_is_type_like` —
  `SemanticAnalyzer.is_type_like` (`mypy/semanal.py:8327`);
  `NativeIsTypeLikeSuite`. Landed 2026-09-14 after a rebase over H1r; CI
  `pr-gate`, `parity`, `parity-mirror`, `parity-typeops`, `parity-ast`
  green; local `ocr` gate 0 blocking / 2 advisory (recorded on the PR).

#### Mass-migration blocker wave (opened 2026-09-14)

`#1625` is the master plan; `#1618`-`#1624` are the seven blockers: wire
`extra_tvars` channel, resolver snapshot timing gap, wire `Type` format
gaps, ParamSpec/TypeVarTuple `variables` channel, plugin callback channel,
live-object identity contracts, and the kernel-net-slower-than-Python
performance regression. Work is staffed in disjoint-write-scope worktrees;
the wire trio (`#1618` / `#1620` / `#1621`) is deliberately serialized
behind one owner because all three extend the same `CallableType` wire
record and share one `CACHE_VERSION` bump. Wave entries land as those PRs
merge.

#### Wave 3 mass migration (started 2026-09-15)

Plan: `docs/plans/2026-09-15-wave3-mass-migration.md`. Starting position
`main = 45c94f541`. Six issues targeted for landed-or-floored.

- W6 `#1620` (`d3924727c`, PR #1658) — wire remainder:
  `CACHE_VERSION` 13 -> 14. `mypy/types.py:3063` writes `definition_ref`
  as `write_str_opt(data, None)` (deliberately None to avoid circular
  imports); `mypy/wirefixup.py:857-874` `_resolve_definition` +
  `_match_definition` resolve it at decode time via a name+arity heuristic
  over the fallback TypeInfo's symbol table. `crates/type_kernel/src/
  wire.rs:1235` reads `definition_ref`. PartialType wire tag added.
  ErasedType guard added. `NativeWireRemainderSuite` (10 tests) in
  `mypy/test/testtypes.py`. Gates: testtypes 3883/3, testcheck 8144/69/7,
  testfinegrainedcache 549/229, testmerge 41/1, cold + warm self-check
  clean 353.
- W1 `#1634` (`ce6e1ae3f`, PR #1657) — complex-statement drivers
  (try/for/with/match) in `checker_functions.rs`; merged.
- W2 `#1635` (`a2b6d604f`, PR #1655) — subexpr walk + aststrip helper;
  merged.
- W3 `#1642` (`504db773e`, PR #1659 + PR #1654) — check_call cluster.
  PR #1654 (`fcc1f8e2c`) deleted the `rust_classify_check_arg` wire
  seam and gated `rust_check_argument_count_plan` on unpack. PR #1659
  batched `enum_callable_base` + `typeobj_gate` into one FFI crossing
  (`rust_check_call_head` in `checkcall_typeobj.rs`). Remaining 4 seams
  floored with evidence: `map_actuals_to_formals` ->
  `compute_arg_context` -> `check_argument_count` form a sequential
  dependency chain (each needs the prior's output);
  `has_abstract_type` is per-arg granularity inside
  `check_argument_count`. Batching them would require a full
  `check_call` plan that is out of scope. Issue closed.
- W4 `#1628` — expandtype substitution arms; floored with evidence.
- W5 `#1629` — pass-1-only solve split spike; CLOSED NO-GO (79.8% of
  generic calls need pass-2, 89.2% do full re-solve).
- W7 `#1637` (`2b555a2e0`, PR #1660) — perf sweep audit. Converted 3
  message seams to live-object PyO3 reads
  (`rust_append_numbers_notes_live`,
  `rust_make_inferred_type_note_live`,
  `rust_append_invariance_notes_live` in `messages.rs`). Serialize
  count barely moved on clean self-check corpus (965188 -> 965419
  writes). Per the issue's stop rule ("stop if serialize count does
  not move"), further module conversions stopped. Gates: testtypes
  3894/3, testcheck 8144/69/7, self-check clean 353.

#### Wave 4 mass migration — big swarm (started 2026-09-15)

Plan: `docs/plans/2026-09-15-wave4-mass-migration.md`. Starting position
`main = c16936dd6`. Four workers in one AgentSwarm (disjoint files),
then a queued phase-1.5 worker. Loose ends closed: `#1635` (merged as
PR #1655), `#1627` (floored not-planned — DefaultPlugin hook bodies
bounded by 7,121 invocations on a 66.8s self-check, ~0.02-0.05% wall).

- W-A1 `#1626` (`1a4c8f130`, PR #1665) — `Plugin.declare_hook_fullnames`
  (`dict[str, frozenset[str]] | None`); `DefaultPlugin` returns
  `DEFAULT_HOOK_FULLNAMES_BY_KIND` (81 fullnames / 7 kinds),
  `ChainedPlugin` per-kind union, `_build_plugin_hook_registry`
  (build.py) installs the union and sets the user-plugin bit only when
  it is None (replacing `len(plugins) > 1`). The native hook registry
  stays live under user plugins (`proper_plugin` declares its 3 names),
  so the 328,408 `_try_native_plugin_hook` probes / 111,649
  `plugin_hook_known_absent(get_attribute_hook)` self-check probes are
  no longer 100% no-FFI. `NativePluginHookDeclareSuite` (testtypes).
- W-A2 `#1628` (`2ce18c9f3`, PR #1667) — ported `normalize_trivial_unpack`
  into the ParamSpec Parameters-splice arm of `expandtype.rs` (mirrors
  types.py:2852-2865) and lifted three guards: the ParamSpec/TVT
  `variables` defer in `solve.rs` `rust_infer_function_type_arguments`
  and in `checkcall.rs` `solve_generic_call_core`, plus the
  `need_refresh` conjunction in `mypy/checkexpr.py:3085-3087`.
  Retired the ~90 need_refresh + ~80 var_pspec_tvt + applytype +
  constraints defer events; `#1621` closed into this (residual solve
  defers = the documented ParamSpec suffix/prefix splits + polymorphic
  `extra_tvars` channel floors). 3 Rust units + 1 rewritten parity test.
  HIGH-RISK variadic class (wave-33 segfault precedent) landed gated
  with gate-off/on parity green. Gates: cargo 2836/11, testtypes
  3890/7, testcheck 8144/69/7, self-check clean 353.
- W-B1 `#1661` (`032caceee`, PR #1664) — retired the `is_literal_type_like`
  wire seam (typeops.py, 248,410 crossings / 3.8MB -> 0, pure Python
  body; the seam was 100% native so the round-trip was pure overhead).
  `NativeIsLiteralTypeLikeRetiredSuite` (3 tests, testtypes). Gates:
  testtypes 3893/7, testcheck 8144/69/7, self-check clean 353.
- W-B1b `#1662` (`5a6810b61`, PR #1666) — IAMA dispatch dedup: pass
  `None` instead of re-serializing the `Instance` when
  `mx.self_type is typ` (98.95% of cold self-check calls), Rust clones
  the decoded `instance` as `self_type` (`Option<&[u8]>` param).
  One wire serialize + one decode eliminated per dispatch call in the
  common case; native share held 100% (100,661 calls, 0 defers).
  Gates: testtypes 3890/7, testcheck 8144/69/7, self-check clean 353.
- (queued phase 1.5) W-B3 `#1663` — semanal gate default-ON with
  non-wire interfaces; dispatched after the four Phase-1 merges;
  entry appended here on landing.

#### Wave 5 (started 2026-09-15) — tiered feedback + ledger archive

Starting position `main = 506aa7e4c`. Eight lanes in parallel; the wave's
process change is the tiered feedback protocol and the weighted heavy-op
pool, both recorded in `AGENTS.md` (issue `#1679`).

The tiers classify work by blast radius (T1 docs/test-only, T2 gated seam
with the Python fallback intact, T3 default-ON flip or wire/identity
change, T4 wave level). Local corpora are now a T4 wave-level step, not a
per-lane step: CI runs the full corpus on every production PR, so lanes
restore only their own suite file, the gate-off/on differential, and
engagement counters. The heavy-op layer is the weighted pool
`/private/tmp/mypy-rs-sem.sh` (3 slots; build = 1, corpus = 2), which
replaced the flat `/private/tmp/mypy-rs-heavy.lock`; under the flat mutex a
release kernel build queued behind an entire corpus run (measured: 1m14s
build at load 86 versus a corpus of many minutes). Per-lane tier and
measurements are appended below as the coordinator reports each landing.

- `#1676` (`271175175`, PR #1676) — docs: archive this ledger out of
  `AGENTS.md`. Tier T1. `AGENTS.md` 4,937 -> 232 lines: 4,764 removed,
  59 inserted. Lossless audit before commit: of 4,708 nonblank removed
  lines, 4,628 are byte-identical in
  `docs/plans/type-kernel-seam-ledger.md`, 80 are preserved in
  `docs/native-build-reference.md` (the rationale/history behind the
  build and parity rules), 0 unaccounted; the two large removed blocks
  (`AGENTS.md` old lines 274-1485 and 1486-4930) are contiguous
  byte-identical substrings of the archive. The same commit lands four
  plan docs that were untracked (`2026-09-15-wave3-mass-migration.md`,
  `2026-09-15-wave3-perf-sweep-audit.md`,
  `2026-09-15-wave4-mass-migration.md`,
  `2026-09-15-solve-pass1-split-spike.md`). Gates: CI `pr-gate` pass
  (run 35022390403, 266s), `ocr-review` skipped; local
  `ocr review --from origin/main --to docs/ledger-archive-wave5` selected
  0 items (docs-only diff), 0 blocking.
- `#1668` (`618c2b196`, PR #1669) — perf: retire residual scalar-only wire
  seams (slice of `#1624`, from the `#1637` sweep audit). Tier T3 by the
  table (seams that answered in production were deleted). Five gates
  deleted: `rust_descriptor_has_get_set` (`checkmember.py`, 22,809 calls /
  512,396 bytes -> 0/0), `rust_is_singleton_identity_type` (7,550 / 41,796
  -> 0/0), `rust_is_singleton_equality_type` (10,577 / 277,110 -> 0/0),
  `rust_is_recursive_pair` (1,972 / 118,288 -> 0/0),
  `rust_analyze_none_member_access` (1,001 / 4,004 -> 0/0); total
  43,909 calls / 953,594 bytes -> 0. Figures are as reported by that lane
  on PR #1669, measured on the cold self-check with an env-gated
  sitecustomize wrapper, and are not re-derived here. Gates reported:
  testtypes 3,905/7 (`TEST_NATIVE_TYPE_KERNEL=1`), testcheck 8,144/69/7
  (`-n2`), cold self-check 0 errors / 353 files,
  `cargo test -p mypy-type-kernel` 2,838/0/11, `cargo fmt --check` and
  `clippy -D warnings` clean.
- `#1678` (`3329850ac`, PR #1681) — CI tier. `native-kernel-parity.yml` gains
  a dependency-free `changes` job that diffs the PR `base..head`;
  `parity-ast` consumes it via `needs` + `if`, so a kernel-only PR no longer
  pays the AST suite (its own x86 `ast_serialize` build plus
  `test_nativeparse` / `testparse` / `teststubgen`). The filter is a
  deliberate superset (`mypy/semanal*.py`, `mypy/nodes.py`, `mypy/errors.py`,
  `mypy/options.py`, `mypy/stubgen.py`, `mypy/stubutil.py`) because
  `teststubgen` runs full semantic analysis. Fail-open: no PR SHAs or an
  unavailable diff publishes `ast=true`, so the worst case is the status quo.
  Redundancy recorded on that PR, deliberately not fixed: `parity` and
  `parity-typeops` are the same testcheck corpus (both install the TYPEOPS and
  CHECKER_STMTS resolvers; `parity-typeops` differs only by
  `MYPY_NATIVE_TYPE_KERNEL_REQUIRED=1`), and `parity` runs testcheck twice
  inside itself (bare, then with `testtypes`), so the corpus gate is roughly
  3x redundant. Merging or deleting a gate is a CI-tier decision about what CI
  verifies, not tidying.
- `#1680` (`e9017bf46`, PR #1683) — dev tier. Reusable worktree pool
  `scripts/worktree_pool.sh` (`init` / `claim` / `release` / `status` /
  `prune`); `claim` does `checkout -B <branch> origin/main` plus `clean -xdf`
  excluding `target` and `.venv`, so a slot resets in seconds with a warm
  `target/`. Compile-unit counts as reported: a fresh worktree with an empty
  `target/` compiles 25 units; after `release` then `claim` with one source
  file touched, 1 unit. So the pool removes 24 of 25 compiled units but only
  about a quarter of the wall clock, because the release compile and link of
  `mypy-type-kernel` itself is ~59s and is not cacheable across a source
  change. Wall numbers there are provisional as quoted here (no `uptime`
  recorded alongside them); the load-invariant evidence is the unit counts.
  Per-slot `target/` is deliberate, not an oversight: a shared
  `CARGO_TARGET_DIR` is last-writer-wins on `target/release/libtype_kernel.dylib`,
  so a lane can copy a kernel it did not build.
- `#1679` (`a0f6150a7`, PR #1684) — docs tier. Evidence-artifact convention
  (`docs/plans/wave<N>-evidence/<lane>.json`) plus the wave-5 measurements it
  seeds: local testcheck 8,144 against CI `parity` 8,198 on the same commit
  (the spread is platform skips), one release kernel build 1m14s at load 86
  (provisional; load recorded, `uptime` not), and one local `ocr review`
  sample at 9m37s / ~761k tokens for 3 files. **That OCR figure is a floor,
  not a bound**, and the range supersedes it: a second pass (5 files, branch
  `feature/h1d-checker-decision-heads`) cost ~2.4M tokens (input ~2,370,798,
  output ~67,778, cache-read ~2,120,448) in 6m56s for 3 `low` findings.
  Roughly 3x the tokens for under 2x the files, so the wave-level batched
  pass must be budgeted from the top of the range. The same pass logged three
  internal `file_read failed: invalid line range` errors (e.g.
  `start_line 1190 is greater than end_line 560`) before reporting its
  findings, so an OCR pass can review with its own reads failing and still
  emit confident findings: treat findings as leads to verify against the
  code, not as authoritative.
  **Part of that file is falsified.** Its `rust_refers_to_different_scope`
  row records 0 calls and calls the seam a retirement candidate. Lane A4's
  re-run against its own worktree source shows the seam live: suite-level
  `calls=16 / decided=15 / deferred=1`, alongside
  `rust_should_report_unreachable_issues` 13/12/1, `rust_flatten_lvalues`
  22/21/1, `rust_literal_int_expr` 41/29/12, 72 tests passed. The zero was a
  sample-coverage artifact of the 6-file probe, not a dead seam. Corrected in
  the file itself by `#1686` (`e531197d3`, PR #1686), tier T1, which leaves
  the wrong row visible because the lesson is the point: a zero-call reading
  from a sampled probe says something about the sample's coverage, not about
  the seam, and only a whole-corpus count licenses "retire". That PR also
  records the two same-hour hazards it was entangled with, the run resolving
  `import mypy` to the main checkout and a probe harness that exits 0 while
  raising. The corrected numbers above are the ones to cite; the zero row is
  not evidence.
- `#1677` (`3d5ace300`, PR #1692) — chore/scripts, tier T1. Adds
  `scripts/plan_seam_split.py`, a verified planner for splitting the 961
  `wrap_pyfunction!` registrations in `crates/type_kernel/src/lib.rs` by
  defining module: 120 defining modules, 49 of them registering exactly one
  function, 125 declared `mod` lines of which 5 register nothing, 955 unique
  names for 961 sites so 6 names are registered twice, all single-level
  `mod::fn` paths with receiver `module`. Largest units: `message_registry`
  154, `semanal_visitor` 97, `checkexpr_functions` 50, `checker_functions`
  47, `messages` 35, `typeops` 32. `--verify` fails non-zero on a site count
  the parse did not see (a silently dropped registration nulls a whole seam
  battery through the single `try: from type_kernel import (...)` block), a
  parsed module that is not declared in `lib.rs`, a function name that is
  really a module name, or a malformed name. Six names registered at two
  sites each are flagged for confirmation before any move:
  `rust_classify_simple_literal_type`, `rust_classify_tuple_type_implicit`,
  `rust_count_stats`, `rust_object_from_instance`, `rust_pretty_seq`,
  `rust_refers_to_typeddict`. Two figures from an earlier inventory comment
  are retracted in that PR and must not be built on ("40 defining modules";
  "199 sites with two colons", the latter an artifact of BSD `sed` ignoring
  `\s*`). The lesson recorded there is a rule for this host: when a
  measurement feeds a decision, do it in Python rather than through a BSD
  shell pipeline, since three of four measurement mistakes that day came from
  GNU-versus-BSD tooling assumptions and each produced a confident wrong
  number rather than an error.
- `#1681` follow-up (`08ef56f57`, PR #1691) — CI tier. The `parity-ast` path
  gate from `#1681` is rewritten to match paths with a POSIX `case` glob
  instead of a grep regex. Reason: the regex had been verified with macOS BSD
  `grep` while CI runs GNU `grep`, so that verification did not validate CI
  behaviour at all. The skip is now verified on a real kernel PR: `#1690`
  (`crates/type_kernel/src/checker_functions.rs`, `mypy/checker.py`,
  `stubs/type_kernel.pyi`, `mypy/test/testtypes.py`) reports
  `parity-ast: skipped` with `changes: success` and the three kernel jobs
  green. A skipped job renders as `SKIPPED` in the rollup, the same as
  `ocr-review`, which settles an earlier anomaly: `#1685` showed
  `parity-ast: SUCCESS`, so the job really did run there, best explained by
  the fail-open branch firing silently rather than by a regex mismatch. Its
  own OCR pass found two defects, both fixed: the fetch retry logged nothing
  about why the retry failed (the exact ambiguity the change exists to
  resolve), and `while IFS= read -r f` dropped an unterminated final line, a
  fail-closed path inside a gate documented to fail open.
- `#1672` (`a7329c5d4`, PR #1690) — feat: the H1d cluster, four live-object
  PyO3 decision heads, each keeping the pure-Python body as the fallback so a
  `None` return (or an unreadable attribute) re-runs the unchanged decision:
  `rust_should_report_unreachable_issues` (`mypy/checker.py:4702`),
  `rust_flatten_lvalues` (`:5950`), `rust_refers_to_different_scope` (`:6761`,
  call site `:6722`) and `rust_literal_int_expr` (`:9760`). The issue's own
  candidate list was audited first and found stale, so the four heads were
  found by scanning every `TypeChecker` method body for a missing `_rust_`
  reference and keeping the pure reads. 72 suite tests across the four
  `Native*Suite`s, each with direct seam assertions and gate-off vs gate-on
  differentials through the real `TypeChecker` method. Suite-level engagement
  as reported by that lane (`MYPY_SERIALIZE_STATS=1`, gate on):
  `13/12/1`, `22/21/1`, `16/15/1`, `41/29/12` calls/decided/deferred, with
  `wire_delta=0` on all four; a 2-module corpus probe gave 52, 1360, 1429 and
  0, the last because its only call site (`check_simple_assignment`'s widening
  block) was not reached by that sample. The lane states plainly that the
  corpus gate for the PR is CI's `parity` job, which is the T2 discipline, so
  tier T2. Gates reported: `cargo test` 2838/0/11 in 0.11s, the four suites
  `72 passed, 3906 deselected in 5.26s`, comment-block hook clean. Two caveats
  were stated rather than hidden: the source-tree hazard (an intermediate run
  resolved `import mypy` to the main checkout, reported all-zero counts and was
  discarded) and the sample-zero above. Its CI landed under the new
  `parity-ast` gate with `parity-ast: skipped` and `changes: success`, and the
  rollup went green 5 pass / 2 skipped (`parity` 12m5s, `parity-mirror` 6m27s,
  `parity-typeops` 6m0s, `pr-gate` 4m11s, `changes` 12s; skips are
  `parity-ast` and `ocr-review`). That is the AST-gate saving observed on a
  real kernel PR. The 0 above is the same sample-coverage artifact recorded for
  this seam in the #1679 entry earlier in this section: the suite counts, not
  the sample, are what prove the head.
- `#1682` (`ba762f12a`, PR #1682) — docs: this wave's protocol record, the
  tier table plus the weighted pool and source-tree pre-flight in `AGENTS.md`,
  the wave-5 section of this ledger, and the handoff resume point. Tier T1.
  Gates: CI `pr-gate` pass (268s), `ocr-review` skipped; local
  `ocr review --from origin/main --to docs/wave5-tiers-ledger` selected 0 items
  (docs-only diff), 0 blocking.
- `#1670` (`ac68f0eaf`, PR #1687) — feat: the G3.1 read flip,
  `rust_snapshot_symbol_table_shadow` serves the namespace from the G3.0a shadow
  store instead of `table.items()`, behind `Options.native_symtable_read_flip`
  (default False, plus `native_symtable_read_flip_verify` which implies it, runs
  the flip-off path alongside and raises on divergence). Verified here: neither
  option is in `OPTIONS_AFFECTING_CACHE`, and the flip's diff touches no cache or
  wire file, so `CACHE_VERSION` is untouched. `mypy/build.py` sets the mode on
  every build so a later build cannot inherit it. A new CI gate,
  `parity-symtable-flip`, was added with it, and on its first run that gate
  failed with `RuntimeError: ... changed namespace order for '__main__'`. The
  cause was **ordering only, not a leak**: `mypy/server/aststrip.py:113-118`
  keeps `@`-named keys across a strip, so `D@5` kept its original dict position
  while the store re-minted its ordinal after a per-build reset, the same key set
  and the same values with only the position moved. Fixed by treating an owner
  whose first recorded write already finds keys as `ShadowGap::Inherited`
  (`defer_inherited`) and deferring to the live walk, with both differential
  assertions left strict. Residual for G3.2: only namespaces the store saw from
  empty in this build are served, so aststrip survivors and cache-loaded tables
  defer, and closing that needs the store to survive the build boundary.
- `e28cbca16` (PR #1703, coordinator) — docs/`AGENTS.md`, tier T1: measurement
  code is evidence-critical regardless of tier, with the two defect families
  observed in one wave (structural zeros: an unreachable branch, a stats key
  initialized and never incremented, a counter incremented and never read;
  accounting bypassed on the abnormal path: bookkeeping skipped when a wrapped
  call raises, a report emitted only on `SystemExit`) and the reason they need
  the full review pass even when they look like T1, namely that a probe fails by
  printing a plausible number and exiting 0, which no test catches. Both families
  were demonstrated in this wave, by the #1700 harness and by the discarded
  all-zero run in #1690.
- `d6647b8bc` (PR #1704, coordinator) — docs/handoff, tier T1: corrects this
  wave's read-flip entry. The flip landed (`ac68f0eaf`) and its defect was
  ordering, not a leak, so the earlier "leaked `X@N` keys" line was an inference
  from one side of a diff dump. It is retracted here and kept only as the lesson.
- `24ddff920` (PR #1705, coordinator) — dev/pool, tier T1: resolves the main
  checkout through `git rev-parse --git-common-dir` instead of the script's own
  directory, and re-points `.venv` on every `claim`, because deleting the
  worktree that launched the pool had left all three slots with dangling venvs.
  It also reorders lock acquisition (slots first, legacy second) and releases the
  legacy lock only when it was acquired, via a `got_legacy` flag, since the naive
  reorder stole an old-protocol holder's lock on `TERM`.
- `#1673` (`9b0f426ca`, PR #1700) — perf: gate the `check_callable_call` tail
  seam and refresh the hot-seam ranking. Tier T2/T3 (a seam that answered in
  production was retired). `rust_check_callable_call` becomes shape d, a
  live-object gate reusing the conjuncts the Python calibration below it already
  computed: calls 177,899 -> 378, defers 177,524 -> 2 (it was 99.79% deferring),
  decided 375 -> 376. Aggregate `serialize_calls` -15.3% (2,860,627 ->
  2,424,137), writes -13.3%, bytes -28.0%, as reported by that lane. Gates:
  testcheck `8198 passed, 15 skipped, 7 xfailed in 159.56s`; cold self-check
  `Success: no issues found in 353 source files`; own suite `3 passed, 3997
  deselected`; CI green. The refreshed ranking is published in
  `docs/plans/2026-09-15-hot-seam-rerank-round4.md`: ranks 2-4 are the documented
  copytype/flatten live-object-return floors (#1623), rank 5 and the live rows
  are the #1642 `check_call` cluster, and `messages.py` is reserved, all counted
  rather than re-opened. **The lane also fixed its own harness in the same PR**:
  id-keyed buckets dropped events when a freed blob's id was recycled (a one-file
  corpus went 106,087 -> 198,963; 4,000 equal-length blobs reported 12 instead of
  4,000) and `report()` was lost on any non-`SystemExit` failure. Both fixed by
  construction (strong-reference registry, count before the call, report in
  `finally`) with behavioural fixtures. The 2026-09-14 audit's id-keyed byte and
  bucket columns are therefore flagged as **undercounts** on #1624 and in that
  document, while seam `calls`/`defers` and `MYPY_SERIALIZE_STATS` were never
  affected, so that audit's verdict stands.
- `#1668` residual sweep (`0046062a1`, PR #1685) — perf: seven scalar-only gates
  retired from the #1637 audit's recommendation-5 batch (items 6-11 plus three
  zero-engagement sites), in the same two files as #1669. Tier T3 (seams that
  answered in production were deleted). Per-seam counters: 204 calls / 190
  answered / 10,942 bytes -> 0 / 0 / 0; `rust_analyze_typeddict_access` was
  called 14 times and answered zero, serializing 1,766 bytes to return `None`.
  The aggregate `MYPY_SERIALIZE_STATS` delta is quoted on the PR but
  **explicitly not claimed**, because the before and after trees differ by six or
  more foreign commits and only per-seam numbers are attributable. That lane also
  re-verified #1669's merged head `618c2b196` independently: testtypes `3905
  passed, 7 skipped`, testcheck `8144 passed, 69 skipped, 7 xfailed` with
  `TEST_NATIVE_PARSER=1 TEST_NATIVE_RESOLVER=1 -n2`, self-check clean. A
  test-quality catch worth carrying: the retirement made two parity pins vacuous
  (`test_instance_fallback_parity` and its `..._rust_tuple_parity` sibling were
  comparing Python against Python), now exercising the Rust seam directly. It
  also filed #1706 (`hard_exit` flushes stdout before
  `atexit._run_exitfuncs()` and then `os._exit`s without flushing, dropping
  redirected-stdout writes, the thing the #1061 comment claims to prevent).
- `#1663` (`cd54ca293`, PR #1699) — perf: live-object semanal seams for
  `remove_unpack_kwargs`, `method_signature` and `declared_metaclass`. Tier T3
  attempted, closed NO-GO: it landed the non-wire conversion and **did not flip
  any gate default** (verified here: the diff touches no `options.py` and no
  `build.py`, and `main` still gates on `native_type_kernel and
  MYPY_ENABLE_NATIVE_SEMANAL`). The issue's stated mechanism was disproven: the
  entire semanal wire footprint is 287,369 of 50,373,200 bytes = 0.54% of corpus
  writes, so removing it cannot recover the loss. Real cause, from load-invariant
  min-of-5 ns/call against a 48ns raw-FFI floor: 3,177,447 gated non-wire calls,
  28% of all 9,635,996 seam calls, of which `refers_to_fullname` costs +556ns
  over 981,323 calls (+0.55s), `rust_lookup` +273ns over 1,132,833 calls
  (+0.31s) and `refers_to_class_or_function` +497ns; the per-call work is
  `normalize_fullnames` allocating a `HashSet<String>`. `semanal_time` off versus
  on over five windows, each with its load line: +6.68/+8.51 at load 64,
  +2.01/-0.20 interleaved, +1.93 at load 13, +5.19, +3.96, so four of five
  adjacent pairs are positive and in the stable or falling-load windows the delta
  runs against the load trend. Residual left deliberately on the wire:
  `configure_bases` (206KB) and `make_any_non_explicit` (81KB). Follow-up #1698
  was filed and assigned for the hot net-loss seams plus the quiet-host
  re-measurement that must precede any future flip.
- `#1671` (`58d32ab24`, PR #1694) — feat: the F reopening experiment, one-family
  `Instance` replacement view. `crates/type_kernel/src/typeview.rs` stores the
  `Instance` field set (fullname, argument handles, `fixed_up`, `args_tvar_clean`)
  per live object, keyed by the shared `identity::handle_for_stable` handle and
  pinned so a handle cannot outlive its referent; `rust_view_encode` emits the
  wire bytes straight from the store and refuses (`None`) on a stale stamp, a
  pre-fixup instance, tvar-tainted arguments, an unregistered argument or a
  stored fullname that no longer matches the live `TypeInfo`. `mypy/typeview.py`
  is the Python half (`MYPY_TYPE_VIEW=1` stores and serves, `=2` also routes
  `Instance.args` reads), default off, no `Options` field, no `CACHE_VERSION`
  participation; the ADR-0005 proxy scaffold is deleted. Tier T2 (env-gated,
  default off). Verdict on the close-out's reopening bar: **NO-GO, missed by a
  measured ~4.7x**. The `Instance` wire funnel is 2.12% of total work on the cold
  self-check (1.347s of 63.572s parse+semanal+type-check), so a view that removed
  all of it at zero cost moves 2.12% against a bar of 10%; the prototype serves
  25.1% of funnel calls and removes 32.4% of the walk's encodes, and routing
  reads adds no further wire saving while applying a pyO3 round-trip to 17.43M
  reads (+5.8s over the capture-only arm). All four ADR-0004 contract surfaces
  (plugins, identity/handles, astmerge, cache/daemon) were exercised and measured
  satisfiable, so the binding constraint is arithmetic, not contract. ADR-0006 is
  the draft successor and awaits the maintainer's accept/reject; the wall-clock
  leg is explicitly deferred, not measured. CI green on the PR (pr-gate plus all
  four parity jobs), merge `58d32ab24`, lane worktree and branch removed by the
  coordinator after merge.
- `#1674` (`a5bb83244`, PR #1695) — feat: the G1.2 node-shadow fidelity audit and
  the first expression-node read flip. The audit
  (`docs/plans/2026-09-15-g12-node-shadow-fidelity-audit.md`) is generated from
  source by `misc/g12_node_shadow_audit.py` (Python slots by `ast` over
  `mypy/nodes.py`, shadow fields by `ast` over the two mirror modules, Rust
  records by a text scan) with a `--check` mode that pins the committed tables to
  the derived state. 213 slot rows: `served` 76, `served (wire)` 8,
  `partial (marker only)` 42, `partial (class/fullname only)` 6,
  `AST wire (structural)` 41, `gap` 40. The audit floors the
  `rust_snapshot_definition` candidate (`Var.type` is `partial (marker only)`,
  and serving it needs the F-phase type graph this issue forbids) and picks the
  #1635 aststrip lvalue surgery as the consumer. The flip:
  `rust_aststrip_process_lvalue` serves `is_new_def` + `name` from the shadow and
  performs the class-namespace delete through the G3 store, returning `None` for
  every uncovered shape so the Python tail in `mypy/server/aststrip.py` stays the
  identical fallback. New gap closed: `FieldValue::Text` +
  `rust_node_mirror_capture_field_text` for the `name` slot, seeded at adoption
  (`_seed_name`) and captured for adopted nodes, which is what keeps
  `mypy/renaming.py`'s in-place rename from staling a record. Gate:
  `Options.native_ast_mirror_read` (default off, not in
  `OPTIONS_AFFECTING_CACHE`, wired inside the `native_ast_mirror` branch only),
  `TEST_NATIVE_AST_MIRROR_READ` in the helpers. Tier T2 (default-off read flip).
  `testfinegrained` `747 passed, 27 skipped` in all three states (gates off,
  capture on/read off, capture on/read on); engagement `aststrip.served` 26 /
  `aststrip.deferred` 128; wire delta <=2 events (the flipped path is a
  live-object walk). Merge prep by the coordinator: fixed the stale `-> None`
  annotation on `nodes_mirror.activate` that failed CI's self-check with 8 errors
  in 3 files, rebased onto `58d32ab24` (one additive `stubs/type_kernel.pyi`
  conflict), regenerated the audit tables on the new base, and re-verified the
  rebased tree (cargo `2856 passed / 0 failed / 11 ignored`, testtypes
  `4013 passed / 7 skipped` with the flip gates on, self-check clean, 354 files).
  Filed #1708 for an inherited `NativeSymtableReadFlipSuite` flake observed
  during that verification (raw identity layer answers `handle_of` for a
  recycled address; reproduces on `main` without this branch's gates).
- `c73260211` (PR #1697, coordinator) — docs/handoff, tier T1: the wave-5
  resume point refreshed to the merges that landed minutes after `#1682`, and
  the `#1672`/`#1690` H1d entry added here, so the pinned T4 head and the `main`
  chain reflect the merged state of that morning.
- `5b012830e` (PR #1707, coordinator) — docs, tier T1: the append of the two
  final lane entries above (`#1671`/`58d32ab24`, `#1674`/`a5bb83244`), the head
  line and queue corrected to the merged state, and the new "T4 on the merged
  wave head" section (cargo `2856`/0/11, testtypes `4013`/7, testcheck
  `8198`/15/7, fine-grained `1454`/257, cold self-check 354 files clean, audit
  `--check` clean).
- `f2835c480` (PR #1709, coordinator) — docs/handoff, tier T1: head-line
  refresh to `5b012830e` after the entries PR merged.

#### RefView record-shape contract (#1785, recorded 2026-09-17)

`node_mirror::RefScalars` freezes a `RefExpr`'s scalars at capture time
(`kind: Option<i64>`, `fullname: String`). A post-capture write outside
that shape (`kind` a non-`int`, `fullname` absent) cannot be represented:
`_capture_ref` fails conversion, its `except Exception` keeps the earlier
record, and `RefView`'s served branch answers the stale scalar where the
live fallback answers the new one. The two named sites are
`kind_is_none` and `fullname_opt` (`crates/type_kernel/src/depswalk.rs`),
whose live semantics are a plain `None` check and an optional-string read,
so neither defers.

Verdict: **documented limitation**, not compare-before-answer.
Compare-before-answer re-reads the live slot on every call, which is
exactly the PyO3 cost the channel exists to remove (the F close-out
measured 17.43M routing round-trips at +5.8s), so serving those two sites
would buy nothing over their fallback. The drift is latent by
construction: the writers that would trigger it are out-of-contract
(`RefExpr.kind` carries the `Kind` ints, `_fullname` is a `str`), and the
mode 2 differential (`compare_ref_scalars`) is the detector. A read flip
anywhere on this shape requires zero mismatches on a pinned corpus; if an
in-contract writer ever appears, the fix is to widen the capture contract
(mark the out-of-contract value or drop the record so the live fallback
answers), not to double every served read. Refs: #1785, PR #1783, ocr
session `52a4e3b2`.

##### #1785 detector tests: the mode-2 differential fires (recorded 2026-09-17)

The section above named `compare_ref_scalars` as the detector but pinned no
out-of-contract write, so the detector's fire was asserted nowhere. Two
tests now pin it in `mypy/test/testtypes_native_node_read.py`, one per
named call site, each through the real `get_dependencies` walk:

- `test_the_differential_fires_on_a_present_non_int_kind` (site 1,
  `kind_is_none`): adopt a `MemberExpr`, refresh the record to `kind=None`,
  then write a present non-`int`. `_capture_ref` hands `node.kind` to the
  `Option<i64>` seam, the conversion raises, `except Exception` keeps the
  stale `None`, and `capture_fail.ref` increments by one (asserted). Mode 0
  answers the live `is None` (False), mode 1 answers the stale record
  (True); mode 2 serves, compares, and reports `mismatched > 0`.
- `test_the_differential_fires_on_a_none_fullname` (site 2,
  `fullname_opt`): the walk legs use logical deps with a `CallExpr` rvalue
  and an LDEF-kind defining lvalue, the only shape that reaches
  `RefView::fullname_opt` (`depswalk.rs` line 1196); a mode-1 serve count
  above the `process_lvalue` one (>= 2) proves the tail answered the
  lvalue. The record holds `"main.y"`, then `_fullname` is written to
  `None`. The non-optional `String` field cannot represent it, the capture
  fails (`capture_fail.ref` +1), mode 0 answers the live `None` while mode
  1 answers `Some("main.y")` at that site; mode 2 serves, compares, and
  reports `mismatched > 0`.

Each test first walks the unfaulted record in mode 2 to pin the positive
control (`served > 0`, `mismatched == 0`), so the faulted leg's
`mismatched > 0` is a difference the same walk produces, not a constant.
The deps map is identical across modes for both cases: the drift is
invisible to the native/Python map comparison because the kind branch
early-returns before adding deps (`deps.py` line 684 for site 1, the LDEF
return at line 656 for site 2), which is exactly why the counter
differential is required.

Evidence: node-read suite 15 passed / 0 skipped (13 before + 2);
`testtypes_native_mirror` 344 passed / 5 skipped (3.14 + librt, all
pre-existing); `testdeps.py` (the `deps.test` corpus) 230 passed / 0
skipped. Negative controls: `MYPY_TK_NODE_READ_CONTROL=off` rc=1 (both new
tests fail at the clean-leg `served > 0`), `=desync` rc=1 (the agreeing
corpus test fails on the injected desync). No `Options` default, gate
default, or `CACHE_VERSION` changed; compare-before-answer was not
implemented. Refs: #1785, PR #1783.


#### Load-time seed for the fixed-format reader (#1773, recorded 2026-09-17)

Verified cause: `SymbolTable.read` (`mypy/nodes.py`, the fixed-format cache
reader reached from `load_tree`) built namespaces through the dict
constructor, a C-level path the mirror's `__setitem__` patches never see, and
the #1755 write-time seed needs a recorded write, so every cache-loaded
namespace stayed unadopted and the G3 read flip deferred it
(`defer_no_handle`), which #1765 pinned as the expected deferral.

Fix: kernel entry point `rust_symtable_mirror_seed(owner)`
(`crates/type_kernel/src/symtable_mirror.rs`) takes a finished namespace's
live order exactly like the write-time seed; when seeding fails it pins the
owner and marks it inherited, so the flip defers `Inherited` rather than
answering from a half-populated record or leaving a silent no-handle. Python
wrapper `symtables_mirror.seed_loaded` (gate-off no-op, counts `seed.loaded`
and `capture_fail.seed`, never raises into the deserialization path), called
from `SymbolTable.read` for `size > 0`, which covers module and nested class
namespaces alike. The #1765 pin is inverted to
`test_cached_fixed_format_trees_serve_the_read_flip`, asserting the handle,
the entry count and a zero-defer sweep for every loaded namespace.

Probe (two-run fixed-format corpus, flip verify on): pkg.base handle 27524,
entries 32/32; pkg.use handle 27557, entries 13/13; flip delta 7/7 tables
mirrored, 77 entries served, every defer reason 0, astdiff 2/2 verify.ok.
Provenance across the cache-loading build: seeded_owners 521, seeded_entries
6535, put_entries 12, seed_rejects 0 (the seed mints the whole cache-loaded
universe including typeshed deps); the JSON reader answers through the
capture instead (put_entries 6547, zero seeds), same serving 7/7/77.

Gate evidence, both states: `testcheck` 8144 passed / 69 skipped / 7 xfailed
identical gates on and off; cold self-check (`-p mypy -p mypyc`, num_workers 4
forced by `mypy_self_check.ini`) `Success: no issues found in 373 source
files` both states with mirror + verify forced through an Options wrapper;
fine-grained trio (`testfinegrained` + `testfinegrainedcache` +
`testdaemon`) 1334 passed / 256 skipped identical both states; `testdiff` 79
passed with the step-level flip report at 320/320 tables mirrored, 1810
entries served, defers 0. One evidence caveat, verified as a limitation
rather than a defect: the parallel self-check's sessionfinish dump is all
zeros in the master because the forked workers hold the counters (an
in-process single-file run shows put_entries 33192), so cache-loaded
engagement evidence comes from the single-process probe. Refs: #1773, #1755,
#1765, and the residual line in the `#1670` entry ("cache-loaded tables
defer") is superseded by this seed.

#### Statement-family serving read flip (#1787 PR B, recorded 2026-09-17)

`MYPY_TK_STMT_READ_FLIP` selects a mode (0 off / 1 serve / 2 serve +
differential compare) over the G2 metadata store for the one registered
statement field native code reads live: `Block.is_unreachable`. The
consumers are `depswalk.rs::visit_block` (production, the native deps
walk behind `Options.native_type_kernel`) and
`semanal_visitor.rs::rust_visit_block` (parity-only,
`MYPY_ENABLE_NATIVE_SEMANAL`), both via `node_mirror::serve_stmt_flag`
with the live attribute read as the fallback. Serve contract: a record
answers only as the exact shape the capture wrote (`MetaValue::Bool`);
an unrecorded object, a missing identity handle, or a shape-crossed
record (any other `MetaValue`, e.g. an `Int` landed on the bool field)
defers, so the #1785 record-shape drift class is structurally excluded.
The channel keeps its own `StmtReadState` thread-local with the same
seven-tuple counters as G1.1 (`consulted` / `served` / `deferred_off` /
`deferred_unrecorded` / `compared` / `mismatched` / `compare_errors`),
exposed through `nodes_mirror.set_stmt_read_flip` /
`stmt_read_flip` / `stmt_read_counters` and the six
`rust_node_mirror_*_stmt_*` pyfunctions. Mode 2 compares every served
flag against the live slot; an unreadable or non-`bool` live slot
counts as a compare error *and* a mismatch (capture encodes `Bool` only
for literal `True`/`False`, so such a slot on a served record is an
out-of-contract post-capture mutation). The default stays 0: no
`Options` default changed, no `num_workers` force-on, and the
production walk keeps every read live until the env gate opts in.
`MYPY_TK_STMT_SESSIONFINISH_OUT` dumps
`{"stmt_read": ..., "capture": ...}` from `build()`'s finally (the G3.2
`sessionfinish_dump` pattern) for per-pid corpus evidence.

Evidence: `cargo test -p mypy-type-kernel` 2887 passed / 0 failed /
8 ignored (11 new `g2_serving_tests` + the #1795 `object_of` test);
lane suites 368 passed / 5 skipped (all five pre-existing
environmental: t-strings need 3.14+, librt splice);
`testtypes_native_stmt_read` negative controls fail as designed
(`off` -> "the walker must be answered by the record", `desync` ->
"native/Python deps mismatch", both rc=1); the fine-grained corpus
(`testfinegrained` 747 passed / 27 skipped, 4 xdist pids) in mode 2
served 87 and compared all 87 with 0 mismatches, 0 compare errors,
9739 unrecorded defers over 9826 consults, while the mode-0 baseline
shows served 0 / deferred_off 9826 over the identical consult count;
cold self-check clean (374 files) on the rebuilt extension. The
`--check` audit drift is line-anchor-only (the new G2.1 section shifts
`nodes_mirror.py` anchors); the sibling lane regenerates the tables
(#1791).

#### #1795 verdict: object_of validates identity coherence (recorded 2026-09-17)

`capture_pin` stores pins keyed by identity handle; `object_of`
resolves a handle back to the pinned object. Before this change it
answered purely from `TARGET_PINS`, while `handle_of` delegates to
`identity::handle_of` - so after an identity-only reset
(`rust_mirror_reset`) a stale pin could still resolve, and the two
read-backs disagreed. Verdict: **validate inside `object_of`** (the
pin answers only when `identity::handle_of(pin) == handle`), not a
documented independence. Rationale: the divergence is unreachable in
the production flow either way (`nodes_mirror.reset()` runs before
`types_mirror.reset(preserve_stable=True)` under the same option gate,
so pins never outlive the identity reset there), which makes cost the
only differentiator; the validation is one non-minting thread-local map
lookup on a cold read-back, not the PyO3 round-trip class that made
#1785 reject compare-before-answer; the fail direction is the defer
(`None`), never a wrong object (handles are never re-issued after a
reset and pins keep the referent alive, so a mismatch can only mean the
identity layer moved on); and the Var-key lane inherits the invariant
`object_of(h) = Some(obj) => handle_of(obj) = Some(h)` that its own
mode-2 differential will assert. Tests:
`node_mirror_tests::test_object_of_defers_when_identity_forgets_the_handle`
(Rust) and `NativeNodeMirrorSuite::test_identity_reset_defers_object_of`
(Python, through `rust_mirror_reset(False)`). Refs: #1795, #1787 PR B.

#### G4 expression-family graduation: production read-serving default on (#1860, recorded 2026-09-18)

`Options.native_ast_mirror` and `Options.native_ast_mirror_read` flip
to default `True` (neither is in `OPTIONS_AFFECTING_CACHE`; no shadow
state may enter the cache). The #1836 one-family-one-flip rule is
carried by the capture/serving split, not by new options: **capture
is family-agnostic** (the G1 expression records and the G2
statement/def metadata records ride the one `native_ast_mirror`
gate), while **serving is per-channel**, and only the expression
channels follow `native_ast_mirror_read`: the G1.1 `RefExpr` binding
scalars (`depswalk.rs` through `RefView`) and the G1.2 aststrip
MemberExpr lvalue read. The G2.1 statement channel
(`MYPY_TK_STMT_READ_FLIP`) and the G2.2 Var-key translation
(`MYPY_TK_VAR_KEY_FLIP`) stay env-only default 0; their flips are
follow-up issues.

The wiring is `BuildManager.__init__` calling
`nodes_mirror.set_production_read_flip(capture_active and
options.native_ast_mirror_read)`, on every manager including the off
case (the aststrip rule: a later build in the process must not
inherit a stale mode). Production serves mode 1 only; mode 2
(serve + differential compare) is a measurement mode this entry
point never selects, so a production run cannot end up differential
by default. `MYPY_TK_NODE_READ_FLIP` wins over the option:
`activate()` parses the env before the wiring runs, and the wiring
defers (`read_flip()`, no write) whenever the env var is present, so
measurement arms keep control of the channel. The test harness
stays a differential: `helpers.parse_options` forces both gates from
env (unset = off), so the DataSuites never exercise the default-on
path; default-on is exercised by the self-check, the CLI, and the
tree-pinned probe. The #1839 cross-run kernel leg models the same
resolution (`effective` = env value when present, else the
production default computed from kernel presence and the options),
so an option-armed channel is compared rather than misread as
inert.

The #1859 del contract is restated unchanged and still in force: a
`del` on a tracked G1 slot is out of contract (the five binding
scalars are one snapshot gated by a single presence marker, so the
#1841-style per-field retire cannot cover them without new record
state); no production `del` on a tracked G1 slot exists; the pins
stay in force (`NodeSlotDeletionOutOfContractSuite`), and the G1
write-flip work must flip them knowingly.

The #1836 binding condition rides the claim: the expression family's
**write path is still Python**, and the cross-run differential is
**evidence of agreement, not proof of ownership**.

Evidence: tree-pinned dmypy probe on the corpus (branch vs
unpatched-main control, same extension dirs): branch ran
`read_mode 1` with the node channel serving `4/4` consults (0
mismatched, 0 compared) while the control ran `read_mode 0` with the
same 4 consults deferring (`deferred_off 4`) and identical error
output; capture on the branch recorded `capture_ref 112529` against
an empty control audit, with the statement channel idle
(`deferred_off 5658`, `served 0`) and the Var-key channel untouched.
The #1839 cross-run differential (mirror-on vs mirror-off, legs
kernel/errors/ast/typemap/deferral.build) agreed on all five legs:
errors byte-identical, 49 trees and 49 cache payloads byte-identical,
typemap 6214/6214 entries, deferral.build byte-identical; the
kernel leg shows the on-arm armed through the option default
(`NODE_READ=1 STMT_READ=0 VAR_KEY=0` vs `0/0/0`) and its
`ARMED BUT INERT (0 consulted)` note is expected in batch mode (the
deps walk is fine-grained-only; engagement is the probe's counter
gate). The full T3 battery held both gate states:
`testcheck` 8144 passed / 69 skipped / 7 xfailed in each state
(404 s off, 515 s on); the fine-grained family
(`testfinegrained` + `testfinegrainedcache`) 1296 passed / 256
skipped in each state (102 s off, 70 s on); the reversed-order
isolation run (#336 shape, `testcheck` first in one xdist
invocation with all five gates env-on) 12370 passed / 76 skipped /
7 xfailed; cold self-check `Success: no issues found in 378 source
files` in each state with byte-identical output (the gate-off leg
runs through a tree-pinned `Options.__init__` patch wrapper, since
no CLI flag exists for the gates). The native lane suites passed
both states (gate-off 20 passed on `testtypes_native_node_read`
after the wiring test was corrected to model `activate`'s env
role; gate-on 425 passed / 5 skipped over the five files);
`testcrossrundifferential` 64 passed / 4 subtests with the three
new option-default cases; `cargo fmt --check` and `cargo clippy -D
warnings` clean on `mypy-type-kernel` (no Rust change).
