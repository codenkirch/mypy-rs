# Rust Port Swarm Candidates (2026-08-19b)

Third-wave batch. The first wave (#675-#680), slice 48 (#672), and wave-2
(#691 conditional_types, #692 equality_ambiguity) have landed. This wave
targets the NEXT confirmed pure-Python bodies that remain ungated. Unlike
the 08-19 wave (all small leaves in checker.py), this wave is the
"medium-body" tier: single functions of 3-13K Python bytes that are still
pure-computation enough to strangler-port, each worth roughly 10-18K Rust
bytes.

Goal context: push the GitHub languages-API Rust share from ~36.4% (local,
post-691/692) toward 40%. At a calibrated Rust:Python body ratio of
~2.2x (18.9K Rust from 6.5K Python for conditional_types; 10.4K from 5.7K
for comparison_group), each candidate below moves the share meaningfully.

NOTE (verified 2026-08-19): the small-leaf surface is exhausted (08-19 doc
predicted this). The remaining opportunities are exactly these: (a) big
module-level arity/alias/override algebra in typeanal.py, typeops.py,
checker.py; (b) pure type-predicate bodies in checkexpr.py that call only
native or gated sub-seams; (c) the classifier FRONT of semanal's
type-expression parsing. Most big visitor-method bodies in checkexpr/
checker/semanal are state-bound (msg, binder, scope, deferral) and are
flagged HIGH where applicable; they are listed last, not as primary
targets.

## How the seam works (all candidates)

Strangler-fig per-call gate. Python serializes the input `Type` graph to
the binary wire format, calls a `type_kernel` PyO3 function, and if Rust
returns a value (not `None`) uses it; on `None` or
`AssertionError`/`NotImplementedError`/`ValueError` it falls back to the
pure-Python body (which stays in place, untouched).

- Wire read/write: `crates/type_kernel/src/wire.rs` (`read_type`,
  `read_type_list`, `write_type`, `write_type_list`), Python side
  `mypy/types.py`.
- Live `TypeInfo` snapshot: `crates/type_kernel/src/typeinfo.rs`
  (`TypeResolver` / `NativeTypeResolver`).
- Gates already wired in `mypy/build.py`: `_set_native_checker_active`,
  `_set_native_checker_stmts_active`, `_set_native_typeops_active`,
  `_set_native_checkexpr_active`, `_set_native_semanal_active`,
  `_set_native_semanal_visitor_active`, `_set_native_typeanal_active`,
  `_set_native_messages_active`, plus the resolvers. No `build.py` change
  is needed for any candidate below.
- Several live-object functions need `TypeInfo`/`TypeAlias`/`FuncDef`
  attribute reads. Prefer PyO3 attribute reads on the LIVE object passed
  in (pattern: `rust_class_callable` reads `info.defn.type_vars`
  directly), or pass the needed scalars/fullnames from the Python shim as
  plain args. Avoid extending the wire codec for node bodies (they are not
  wire-serializable).

## Build / parity (MANDATORY)

The venv site-packages `.so` is stale. ANY parity run MUST use a fresh
scratch build:

```bash
SCRATCH=/private/tmp/mypy-rs-swarm-<candidate>
mkdir -p "$SCRATCH"
cargo rustc -p mypy-type-kernel --features extension-module --lib \
  --crate-type cdylib --release -- -C link-arg=-undefined -C link-arg=dynamic_lookup
cp target/release/libtype_kernel.dylib \
  "$SCRATCH/type_kernel.cpython-314-darwin.so"
PYTHONPATH="$SCRATCH" TEST_NATIVE_TYPE_KERNEL=1 \
  .venv/bin/python -m pytest -n0 mypy/test/testtypes.py -q -k <Suite>
```

Env facts:
- venv python: `.venv/bin/python` (3.14.5, SOABI `cpython-314-darwin`).
- NEVER `maturin develop` (installs bogus `mypy-0.1.0`).
- `TEST_NATIVE_TYPE_KERNEL=1` is the parity differential (kernel default-on;
  harness compares Rust vs pure-Python head-to-head).
- New modules: add `mod <name>;` to `crates/type_kernel/src/lib.rs` in the
  module block (~44-116, keep alphabetical) plus one `add_function!` line
  before `Ok(())`.

Native seams available to call as `PyObject` calls from the shim (all
verified native-backed): `is_subtype` / `is_proper_subtype`,
`is_overlapping_types`, `restrict_subtype_away`, `make_simplified_union`,
`try_expanding_sum_type_to_union`, `detach_callable`, `expand_type`,
`freshen_function_type_vars`, `map_formals_to_actuals`,
`map_instance_to_supertype`, `flatten_nested_unions`,
`flatten_nested_tuples`, `try_getting_literal`, `has_bytes_component`,
`has_erased_component`, `are_argument_counts_overlapping` (argmap),
`overload_can_never_match` / `is_more_general_arg_prefix` (native since
08-19), `is_unsafe_overlapping_overload_signatures` (native since 08-19),
`partition_equality_ambiguous_types` (native since 08-19),
`is_typevar_default_recursive`, `detect_diverging_alias`, `wrong_type_arg_count`.

## Summary table

| # | Python fn(s) | file:lines | Python bytes | est Rust bytes | Rust module (new?) | gate | risk |
|---|---|---|---|---|---|---|---|
| 1 | `instantiate_type_alias` | typeanal.py:2217-2388 | 7318 | ~16K | `typealias_instantiate.rs` (new) | `_native_typeanal_active` | MED |
| 2 | `dangerous_comparison` | checkexpr.py:5008-5137 | 6286 | ~14K | `dangerous_comparison.rs` (new) | `_native_checkexpr_active` | MED |
| 3 | `check_overlapping_overloads` (decision loop) | checker.py:1537-1658 | 6147 | ~14K | `overload_override.rs` (new) | `_native_checker_active` | MED |
| 4 | `type_object_type` (+ `type_object_type_from_function`) | typeops.py:283-394, 410-460 | 7378 | ~14K | `typeops.rs` (existing) | `_native_typeops_active` | MED |
| 5 | `try_parse_as_type_expression` (classifier front) | semanal.py:8906-9049 | 7404 (front ~58%) | ~8-12K | `semanal_typeexpr.rs` (new) | `_native_semanal_active` | MED |
| 6 | `check_argument_types` (arg-expansion phase) | checkexpr.py:3664-3795 | 6814 | ~12K | `checkexpr_argtypes.rs` (new) | `_native_checkexpr_active` | MED-HIGH |
| 7 | `analyze_unbound_type_without_type_info` | typeanal.py:986-1107 | 5869 | ~11K | `typeanal_unbound.rs` (new) | `_native_typeanal_active` | MED-HIGH |
| 8 | `clean_up_bases_and_infer_type_variables` | semanal.py:2709-2804 | 4592 | ~10K | `semanal_bases.rs` (new) | `_native_semanal_active` | MED |

Candidates 5+8 touch different regions of semanal.py and different Rust
module files; 1+7 touch different regions of typeanal.py and different Rust
module files; 2+6 touch different regions of checkexpr.py and different
Rust module files. All 8 Rust module files are pairwise disjoint. The
disjoint-assignment plan is at the bottom.

---

## Candidate 1: `instantiate_type_alias`

- Python: `mypy/typeanal.py:2217-2388` (module-level function).
- Python body: 7318 bytes.
- Rust module: NEW `crates/type_kernel/src/typealias_instantiate.rs`. Add
  `mod typealias_instantiate;` to lib.rs.
- Gate: `_native_typeanal_active` + resolver.
- Risk: MED. Reason: module-level, bounded arity algebra, but reads live
  `TypeAlias` node fields (`alias_tvars`, `target`, `tvar_tuple_index`,
  `name`) and takes `MsgCallback` closures for `fail`/`note`.
- Behavior spec (port precisely):
  - Signature `instantiate_type_alias(node: TypeAlias, args: list[Type],
    fail, note, no_args: bool, ctx, options, *, unexpanded_type=None,
    disallow_any=False, use_standard_error=False, empty_tuple_index=False,
    analyzing_tvar_def=False) -> tuple[Type, bool]`.
  - `args = flatten_nested_tuples(args)` (native); if old_args and now
    empty, set `empty_tuple_index = True`.
  - If any `unknown_unpack(a)` (native `rust_unknown_unpack`): return
    `set_any_tvars(node, [], ..., special_form=True)` -> Defer (None),
    keep the Python body owning `set_any_tvars`.
  - `no_args` rewrite when target is `builtins.tuple` Instance and args.
  - Arity checks: `max_tv_count = len(node.alias_tvars)`,
    `act_len = len(args)`; bare-alias -> `set_any_tvars(...)` (defer);
    `max_tv_count == 0`/`act_len == 0` -> Instance/TypeAliasType
    (constructible in Rust, wire-safe); `max_tv_count == 0` and `no_args`
    -> Instance(args); `tvar_tuple_index is None` vs present branches.
  - `fill_typevars` path -> `set_any_tvars` (defer).
  - TypeVarTuple split check (`find_unpack_in_list`) -> error (defer the
    bool decision to Python, or compute locally on wire types).
  - End: build `TypeAliasType(node, args, ...)`; FlexibleAlias expansion
    -> return `exp.args[-1]`.
  - Simplest correct seam: Rust computes the arity/result SHAPE decision
    and returns either (a) a `TypeAliasType`/`Instance` wire result with a
    boolean, or (b) `None` to defer the whole call to Python. Since
    `set_any_tvars` produces side-effectful error output, any path that
    would call it MUST defer (None). The gap is small: the max_tv_count==0
    success paths and the FlexibleAlias path are pure and worth keeping in
    Rust; the rest is thin.
  - Needs `find_unpack_in_list` reimplementation (small, pure) or a wire
    helper returning the unpack index.
- Parity suite: `NativeInstantiateTypeAliasSuite` in
  `mypy/test/testtypes.py`. Cases: non-generic alias, bare `List`,
  `Text = str` no_args, generic alias with all args, missing args (defer),
  TypeVarTuple alias with split (defer), FlexibleAlias unwrap. Compare
  `(str(t), used_default)` gate off vs on.
- Import line: `rust_instantiate_type_alias as _rust_instantiate_type_alias`.

## Candidate 2: `dangerous_comparison`

- Python: `mypy/checkexpr.py:5008-5137` (method on ExpressionChecker).
- Python body: 6286 bytes.
- Rust module: NEW `crates/type_kernel/src/dangerous_comparison.rs`. Add
  `mod dangerous_comparison;` to lib.rs.
- Gate: `_native_checkexpr_active` + resolver.
- Risk: MED. Reason: pure type-predicate decision tree; every dependent
  sub-seam is native (`is_overlapping_types`, `try_getting_literal`,
  `has_bytes_component`, `map_instance_to_supertype`, `get_proper_type`,
  `custom_special_method` is typeops but a name-set check that can be
  re-derived). Requires 4 scalars from the checker (options/binder state).
- Behavior spec:
  - Returns `bool` (is the comparison dangerous).
  - `if not strict_equality: return False` (pass from shim).
  - `custom_special_method(left, "__eq__")` /
    `custom_special_method(right, "__eq__")` and `not identity_check` ->
    True-return False (pass shim-computed bools, or port the
    `custom_special_method` decision: reads `tp.type.mro` for a
    `__eq__`/`__ne__` member that is not a builtin placeholder; the
    fullname allowlist is small. Simpler: shim passes
    `has_custom_eq(left)/has_custom_eq(right)` for the common cases).
  - `prefer_literal`: `try_getting_literal(left/right)` (native wire).
  - `unreachable_warning_suppressed` (binder state) -> pass as bool.
  - `strict_equality_for_none` option + NoneType checks -> pure.
  - Optional/Union stripping: `remove_optional` on both when both Union.
  - bytes/bytearray handled via `has_bytes_component` (native).
  - Instance pairs: the OVERLAPPING_TYPES_ALLOWLIST / Mapping /
    list/tuple / bytes-allowlist special cases: read `left.type.fullname`
    and `right.type.has_base(...)` via resolver, recurse with
    `map_instance_to_supertype` (native) on AbstractSet/Mapping. The
    `self.chk.lookup_typeinfo("typing.AbstractSet")` lookup must come from
    the shim (pass the two TypeInfo fullnames, or have the shim pass the
    mapped supertype args directly).
  - Literal/Instance bytes cases -> pure fullname/has_base checks.
  - Final: `not is_overlapping_types(left, right, ignore_promotions=False)`
    (native) with a `seen_types` recursion guard (set of (Type, Type)
    pairs; pass a stable serialized key or an id-pair set built in Rust
    over wire types).
  - Cleanest seam: Python shim computes `strict_equality`,
    `strict_equality_for_none`, `unreachable_suppressed`,
    `has_custom_eq_left`, `has_custom_eq_right`, and the
    AbstractSet/Mapping TypeInfo lookups, then calls
    `_rust_dangerous_comparison(left, right, original_container,
    identity_check, prefer_literal, scalars, fullname_map, resolver)`.
    Rust returns `bool`; None deferred only on resolver miss.
- Parity suite: `NativeDangerousComparisonSuite`. Cases: literal str vs
  int True, `Optional[X]` vs `Optional[Y]` non-overlapping True, `x is
  None` safe False, custom `__eq__` suppress False, bytes-contains False,
  `AbstractSet[int]` vs `set[str]` via Mapping recursion, bool-vs-bool
  False, list vs tuple rejection.
- Import line: `rust_dangerous_comparison as _rust_dangerous_comparison`.

## Candidate 3: `check_overlapping_overloads` (decision loop)

- Python: `mypy/checker.py:1537-1658` (method on TypeChecker).
- Python body: 6147 bytes.
- Rust module: NEW `crates/type_kernel/src/overload_override.rs`. Add
  `mod overload_override;` to lib.rs.
- Gate: `_native_checker_active` + resolver.
- Risk: MED. Reason: the decision loop is pure pairwise algebra, but the
  two hot predicates it calls were natively ported in the 08-19 wave
  (`overload_can_never_match`, `is_unsafe_overlapping_overload_signatures`),
  so Rust can call them as `PyObject` calls or re-derive; the error
  emission (`self.msg.*`) stays Python.
- Behavior spec:
  - Inputs (prepared by the Python shim): list of `sig` CallableTypes per
    item, `impl_type` or None, `is_descriptor_get` bool, `class_type_vars`
    list. The shim does the `extract_callable_type` / `defn.impl`
    unwrapping and the `state.strict_optional_set(True)` wrapper.
  - Rust port of the body:
    - For each `(sig1, sig2)` pair with
      `are_argument_counts_overlapping(sig1, sig2)` (native argmap):
      compute `will_never_match = overload_can_never_match(sig1, sig2)`
      and `unsafe = is_unsafe_overlapping_overload_signatures(sig1, sig2,
      type_vars)` and `flip_note` (needs the reversed
      `is_unsafe_...` + `overload_can_never_match(sig2, sig1)`).
    - For `impl` vs each `sig1`: the callable-compat decisions
      (`is_callable_compatible(impl, sig1, is_compat=is_subtype,
      is_compat_return=..., is_proper_subtype=False)`), the
      `unify_generic_callable` None-guard, the arg/ret subtype checks.
    - Return a structured decision list, e.g. `list[(i, j, kind)]` where
      kind is `never_match` (error at strict-optional off only? no:
      never_match is unconditional), `unsafe_overlap`, `impl_param`,
      `impl_ret`, `impl_typevar_specific`.
  - The Python shim receives the decisions and emits exactly the
    `self.msg.overloaded_*` calls it does today. The pure-Python body is
    untouched and runs on deferral.
  - Simpler fallback scope if the impl-compat tail is too entangled:
    port only the pairwise `overload_can_never_match` +
    `is_unsafe_overlapping_overload_signatures` screening (the diagnostics
    loop) and defers the impl-vs-items tail (it already runs native
    sub-seams on the Python side). Est Rust then ~10K; keep the full loop
    as the stretch goal.
- Parity suite: `NativeOverlappingOverloadsSuite`. Reuse
  `testcheck.py` overload fixtures + a unit suite comparing the emitted
  `(i, j, kind)` decision list gate off vs on before any msg emission.
- Import line: `rust_check_overlapping_overloads as _rust_check_...`.

## Candidate 4: `type_object_type` (+ `type_object_type_from_function`)

- Python: `mypy/typeops.py:283-394` and `410-460`.
- Python body: 283-394 = 5143 bytes; 410-460 = 2235 bytes; total 7378
  bytes for the two-function cluster.
- Rust module: EXISTING `crates/type_kernel/src/typeops.rs` (has
  `rust_make_simplified_union`, `rust_try_expanding_sum_type_to_union`,
  `rust_true_only/rust_false_only`; append `rust_type_object_type` +
  `rust_type_object_type_from_function`).
- Gate: `_native_typeops_active` + resolver.
- Risk: MED. Reason: module-level pure FunctionLike/Instance-builder from a
  live `TypeInfo` (+ `defn`/`init_node`), but touches the
  `info.type_object_type` cache field (must stay on the Python shim) and
  calls `tuple_fallback`, `map_type_from_supertype` (native), and
  `function_type`-like assembly.
- Behavior spec:
  - `type_object_type(info, name, fullname, fallback, is_new)` builds an
    `Instance`/`CallableType` from a TypeInfo: if the info has a
    non-variadic `__init__`/`__new__`/`__call__`, build the callable via
    `type_object_type_from_function`, else `Instance(info, [])` with
    `info.type_object_type` cache handling on the Python side.
  - `type_object_type_from_function` handles the generic case via
    `self._type_object_type_from_function` variants: supertype method
    synth, `map_type_from_supertype(signature, info, def_info)`, classmethod
    binding, `synthesize_callable` (variadic clamp), `tuple_type_object`
    for named tuples (fallback tuple_fallback + `expand_type_by_instance`
    native).
  - Seam: Rust reads live `info.defn`, `info.names["__init__"]`,
    `info.init_node`, `def_info` via PyO3, computes the callable shape,
    returns the wire `CallableType`/`Instance`; the Python shim owns the
    `info.type_object_type` cache assignment, and the shim passes
    `is_new`, `fallback`, `tuple_type` pieces it cannot reconstruct. Defer
    (None) whenever the init/defn graph is not fully resolvable.
- Parity suite: `NativeTypeObjectTypeSuite`. Cases: non-generic class,
    generic class `class C(Generic[T])`, NewType vs class, classmethod
    `__new__`, named tuple, empty `object`, supertype.
- Import lines: `rust_type_object_type as _rust_type_object_type`,
  `rust_type_object_type_from_function as _rust_...`.

## Candidate 5: `try_parse_as_type_expression` (classifier front)

- Python: `mypy/semanal.py:8906-9049`.
- Python body: 7404 bytes total; the pure structural classifier front is
  lines 8929-8999 (~4300 bytes, ~58%).
- Rust module: NEW `crates/type_kernel/src/semanal_typeexpr.rs`. Add
  `mod semanal_typeexpr;` to lib.rs.
- Gate: `_native_semanal_active` (or `_native_semanal_visitor_active`).
- Risk: MED. Reason: pure AST classification (isinstance chain against
  `MaybeTypeExpression`/`NameExpr`/`MemberExpr`/`StrExpr`/`IndexExpr`/
  `OpExpr`) with one lookup-bound sub-branch; the tail
  (`expr_to_analyzed_type`) stays Python.
- Behavior spec:
  - Port the bail-out DECISION as `rust_classify_type_expression(node_tags,
   str_value: str|None, str_isidentifier: bool, has_quotes: bool, ...) ->
   Option<int>` where the int encodes the classification result:
    0 = "definitely not a type expression (set as_type=None)",
    1 = "maybe a type expression (proceed to full parse)",
    2 = "defer to Python" (identifier-string branch needs symbol lookup).
  - The Python shim feeds structural facts computed from the live AST node
    (tags, string value, quote/whitespace/length flags, regex flag from
    `_MULTIPLE_WORDS_NONTYPE_RE` via `re`), plus the IndexExpr leftmost
    resolution (NameExpr w/ Var vs MemberExpr vs other). The shim already
    knows these cheaply; Rust just applies the rule table. If the shim
    decides the branch needs `self.lookup`, it returns defer (None) and
    Python runs the whole body.
  - This is the cleanest and lowest-risk of the semanal pair. The regex
    semantics stay in `re` on the Python side (pass the bool); no Unicode
    re-implementation in Rust.
  - Value: even at ~4300 Python bytes body, Rust lands the full taut
    rule table and removes the per-expression Python classification cost on
    a very hot path.
- Parity suite: `NativeTypeExpressionClassifySuite`. Cases: NameExpr,
  MemberExpr (both defer), `"sentence like this"` string (not-a-type),
  `"a"` identifier string when unresolved (defer), `"foo bar"` no quotes
  (not-a-type), `IndexExpr` over Var base (not-a-type),
  `X | Y` OpExpr (maybe), `X - Y` (not-a-type).
- Import line: `rust_classify_type_expression as _rust_classify_...`.

## Candidate 6: `check_argument_types` (arg-expansion phase)

- Python: `mypy/checkexpr.py:3664-3795`.
- Python body: 6814 bytes.
- Rust module: NEW `crates/type_kernel/src/checkexpr_argtypes.rs`. Add
  `mod checkexpr_argtypes;` to lib.rs.
- Gate: `_native_checkexpr_active` + resolver.
- Risk: MED-HIGH. Reason: the `callee_arg_types`/`callee_arg_kinds`
  derivation (the Unpack/expanded-tuple logic, lines ~3690-3780) is pure
  wire computation, but the per-argument `check_arg(...)` callback and
  `ArgTypeExpander` remain Python.
- Behavior spec:
  - `check_argument_types` first calls `check_var_args_kwargs` (Python),
    then for each formal computes the effective `callee_arg_types` and
    `callee_arg_kinds`, checking a length mismatch against `actuals` and
    reporting too-many/too-few, then drives the `mapper.expand_actual_type`
    + `check_arg` inner loop.
  - Port the arg-expansion phase: given `arg_types`, `arg_kinds`,
    `formal_to_actual`, `callee` (wire CallableType), compute, per formal,
    either the `(callee_arg_types, callee_arg_kinds, actual_types,
    actual_kinds)` plan or the `too_many/too_few` decision. Return a
    compact plan (`list` of per-formal plans + a list of error tags).
  - Python shim consumes the plan, reports the too-many/too-few errors,
    runs the `ArgTypeExpander` + `check_arg` loop unchanged. Pure-Python
    body is the fallback.
  - Defer (None) on Tuple-with-Unpack forms the wire cannot carry (rare).
- Parity suite: `NativeCheckArgumentTypesPlanSuite`. Cases: simple args,
  `*args`/`**kwargs` matching, unpacked `Tuple[Unpack[Ts]]` vs
  `Tuple[Unpack[Ts], int]`, too-many, too-few, named-args mismatch.
- Import line: `rust_check_argument_types_plan as _rust_...`.

## Candidate 7: `analyze_unbound_type_without_type_info`

- Python: `mypy/typeanal.py:986-1107`.
- Python body: 5869 bytes.
- Rust module: NEW `crates/type_kernel/src/typeanal_unbound.rs`. Add
  `mod typeanal_unbound;` to lib.rs.
- Gate: `_native_typeanal_active` (result of `t`/sym classification).
- Risk: MED-HIGH. Reason: mostly error-message building with
  `self.fail`/`self.note` callbacks plus `self.anal_array` (stateful), so
  the pure win is the branch DECISION + the Any/enum/Literal paths.
- Behavior spec:
  - Port the classification: given `sym.node` kind and a few flags
    (`is_var_any`, `is_allow_type_any`, `is_unbound_tvar`,
    `is_allow_unbound_tvars`, `is_enum_member`,
    `is_defining_literal`, name), return one of: `from_unimported Any`,
    `special Any`, keep `t` (unbound tvar), `LiteralType(enum member)`,
    or `defer`. The literal/member fallback Instance construction is wire-
    constructible.
  - The failure tail (message + notes + `copy_modified(args=anal_array)`)
    stays Python (it emits 3 different message families and notes).
  - Est Rust reflects the decision tree + the two or three wire result
    constructors; if the message tail dominates and the shim keeps most
    bytes, reframe this as "branch decision only" (~6K) and drop to
    MED.
- Parity suite: `NativeUnboundWithoutTypeInfoSuite`. Cases: Var-typed Any
  (from unimported), `builtins.type` special Any, unbound tvar with
  `allow_unbound_tvars`, enum member as literal, enum member outside
  literal (defer).
- Import line: `rust_analyze_unbound_without_info as _rust_...`.

## Candidate 8: `clean_up_bases_and_infer_type_variables`

- Python: `mypy/semanal.py:2709-2804`.
- Python body: 4592 bytes.
- Rust module: NEW `crates/type_kernel/src/semanal_bases.rs`. Add
  `mod semanal_bases;` to lib.rs.
- Gate: `_native_semanal_active` + resolver.
- Risk: MED. Reason: pure-ish class-base algebra (mro reachability + type
  var inference from base types) whose dependencies are native
  (`infer_type_arguments`, `get_type_vars` via traverser/constraints, and
  `is_subtype`); the deferral flag (`self.deferred`) stays on Python.
- Behavior spec:
  - `clean_up_bases_and_infer_type_variables` visits `self.type.bases`,
    drops unreachable/deferred-disabled bases (placeholder / self-
    reference), converts `TypeType`-wrapped bases, then infers type var
    substitutions: for each base, it calls the SAME `infer_type_arguments`
    used elsewhere (native) to map the class's own type vars to the base's
    explicit args, and returns the cleaned bases + `(mapped,
    type_vars)` if convertible.
  - Rust returns the cleaned base list (wire) + the inferred tvar
    mapping or None-defer. The shim owns `self.type.bases` mutation and
    `self.deferred` side effects.
  - Reuse the existing constraints/solve native path for
    `infer_type_arguments` (already the resolver-backed path in
    Python); simplest: have Rust compute the clean-and-classify step and
    have Python call its existing `infer_type_arguments` on the result, so
    Rust adds the base-cleanup bytes without re-porting inference.
- Parity suite: `NativeCleanUpBasesSuite`. Cases: single base, generic
  base `class B(A[int])`, self-referential base (drop), placeholder base
  (defer), protocol base.
- Import line: `rust_clean_up_bases as _rust_clean_up_bases`.

---

## Low-priority: whole-module deletion candidates

Checked `mypy/erasetype.py`, `mypy/maptype.py`, `mypy/expandtype.py` for
Python-fallback deletion viability. RESULT: NOT defensible for any of
them. Each is strangler-gated per call and still keeps the FULL pure-Python
visitor/fallback body:

- `erasetype.py`: `erase_type` falls through to `typ.accept(EraseTypeVisitor())`
  on Rust `None`, and the visitor body is complete and live. `erase_typevars`
  similarly falls back. `shallow_erase_type_for_equality` still calls the
  Python `replace_meta_vars` on the meta-var path. Deleting the fallback
  breaks any wire-unsupported type and all no-kernel runs.
- `maptype.py`: `_native_map_instance_to_supertype` defers on
  `builtins.tuple` superclass, `_needs_python` (definition-carrying
  callables, TypeAliasType), and `read_type` failures; the fallback
  `map_instance_to_supertypes` / `class_derivation_paths` /
  `map_instance_to_direct_supertypes` chain is complete and live. Rust also
  cannot emit line/column, so the Python wrapper owns the fast paths
  (`instance.type == superclass`, no-type-vars superclass).
- `expandtype.py`: `expand_type` / `expand_type_by_instance` /
  `freshen_all_functions_type_vars` each keep the full Python visitor with
  real fallback bodies (the Unpack/ParamSpec/Tuple expansion is heavily
  wire-unsupported and stayed Python by design).

These modules are common-code paths that legitimately keep Python fallbacks
for wire-unsupported forms; deleting them would shrink the Python
denominator only by removing defensively-necessary fallbacks, at the cost
of breaking no-kernel/parity runs. SKIP unless the project explicitly
accepts "Rust-only + no fallback" as policy (it does not; the strangler
docs require fallback preservation).

## High-risk: plugin-visible nodes.py/types.py (flag only)

`mypy/nodes.py` and `mypy/types.py` are nominally off-limits (plugin-
visible mutable object graphs). Pure-function bodies found:

- `nodes.py`: `enum_members` (4041-4090, 1861 B), `local_definitions`
  (5496-5532, 2022 B), `check_arg_kinds` (5426-5460, 1352 B),
  `explain_metaclass_conflict` (4155-4190, 1538 B).
- `types.py`: `with_normalized_var_args` (2699-2763, 3440 B), `slice`
  (3137-3191, 2685 B), `formal_arguments` (2532-2568, 1809 B), `copy_modified`
  (2360-2435, 3870 B; ALREADY ported as `rust_copy_modified`).

EXCEPTION-WORTHY only if the project approves one-off:
`types.py:with_normalized_var_args` (CallableType method, 3440 B) and
`types.py:formal_arguments` (1809 B) are pure projection helpers with no
mutation and no identity games. They are still methods on the plugin-visible
`CallableType`/`CallableType.arguments`. Recommend a standalone parity-only
`rust_*` mirror (like `read_type_to_str`, never wired into production)
rather than a gated production seam, unless a maintainer green-lights the
plugin-visible exception. Keep as a separate "high-risk" decision, not in
the main wave.

---

## Disjoint-assignment plan

Favorite ordering for exec spawns (one worktree agent per candidate; all 8
heads can run in parallel; each is a self-contained PR):

| Agent | Candidate | Python region | Rust module file | gate |
|---|---|---|---|---|
| A | 1 `instantiate_type_alias` | typeanal.py:2217-2388 | `typealias_instantiate.rs` (new) | `_native_typeanal_active` |
| B | 2 `dangerous_comparison` | checkexpr.py:5008-5137 | `dangerous_comparison.rs` (new) | `_native_checkexpr_active` |
| C | 3 `check_overlapping_overloads` | checker.py:1537-1658 | `overload_override.rs` (new) | `_native_checker_active` |
| D | 4 `type_object_type` | typeops.py:283-394, 410-460 | `typeops.rs` (existing) | `_native_typeops_active` |
| E | 5 `try_parse_as_type_expression` classifier | semanal.py:8929-8999 (shim at 8906-9049) | `semanal_typeexpr.rs` (new) | `_native_semanal_active` |
| F | 6 `check_argument_types` plan | checkexpr.py:3664-3795 | `checkexpr_argtypes.rs` (new) | `_native_checkexpr_active` |
| G | 7 `analyze_unbound_type_without_type_info` | typeanal.py:986-1107 | `typeanal_unbound.rs` (new) | `_native_typeanal_active` |
| H | 8 `clean_up_bases_and_infer_type_variables` | semanal.py:2709-2804 | `semanal_bases.rs` (new) | `_native_semanal_active` |

Disjointness guarantees:
- Python regions: A/G both in typeanal.py but at 986-1107 and 2217-2388
  (disjoint, ~1K lines apart); B/F both in checkexpr.py but at 5008-5137
  and 3664-3795 (disjoint); E/H both in semanal.py but at 8929-8999 and
  2709-2804 (disjoint). C alone in checker.py; D alone in typeops.py.
- Rust module files: 7 new + `typeops.rs` (existing) only touched by D.
  All pairwise disjoint. Only D risk-adjacent: D appends to `typeops.rs`
  which other waves may also append; coordinate D last or verify no other
  in-flight wave targets `typeops.rs` (the 08-19 wave used new modules
  only, so `typeops.rs` is currently exclusive to D).
- Import-line safety: each Python file's `import`/alias block gets
  distinct `rust_x as _rust_x` lines; no two agents edit the same import
  line region (B/F/C/E/H each own separate files or far-apart regions).
- `lib.rs`: all 7 new modules get one `mod <name>;` line each (keep the
  ~44-116 module block alphabetical); 7 distinct lines, no collision.
- No candidate touches `mypy/types.py` read_type dispatch, wire tags
  (122+ range untouched), or `build.py` (all `_set_native_*` already in
  place).
- Optional chaining: if two of C's sub-decisions prove too entangled,
  C restricts scope (per Candidate 3 note) rather than overlapping with
  another agent's region.

Parity gate before ANY merge:
```
PYTHONPATH=$SCRATCH TEST_NATIVE_TYPE_KERNEL=1 \
  .venv/bin/python -m pytest -n0 mypy/test/testtypes.py -q -k <Suite>
PYTHONPATH=$SCRATCH TEST_NATIVE_TYPE_KERNEL=1 \
  .venv/bin/python -m pytest -n0 mypy/test/testcheck.py -q -k overload
cargo fmt --check && cargo clippy -D warnings
```
Differential parity (TEST_NATIVE_TYPE_KERNEL unset vs 1) identical on the
added suite.
