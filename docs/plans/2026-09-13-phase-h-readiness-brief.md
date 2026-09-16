# Phase H readiness brief (wave 72-B / issue #1584)

Date: 2026-09-13. Read-only analysis after the G3.0a-d merge (PR #1585).

## 1. What H1 requires that does not exist yet

### The checker traversal contract

`TypeChecker` (checker.py:1223, 12,685 lines) extends
`NodeVisitor[None]`. The core loop is `visit_block` (checker.py:4599-4620):
for each statement, check binder reachability, then `self.accept(s)`
which calls `s.accept(self)` dispatching to `visit_*`. The binder
(`ConditionalTypeBinder`, binder.py:151, 737 lines) is the live mutable
state: it tracks conditional types, reachability, break/continue frames.

H1 needs a Rust-side traversal driver that:
- Walks the AST statement list in order.
- At each statement, invokes a Python callback for the visit_* body
  (binder mutation, expression checking, message emission, plugin hooks).
- Manages the binder frame stack natively (push/pop on block entry/exit,
  break/continue, unreachable tracking).

### Which visit_* bodies could run as Rust traversal with Python callbacks

**Traversal-only (no Python state mutation beyond binder):**
- `visit_pass_stmt` (10703): no-op. Already trivial.
- `visit_break_stmt` (7832): `self.binder.handle_break()`. One binder call.
- `visit_continue_stmt` (7835): `self.binder.handle_continue()`. One binder call.
- `visit_import` / `visit_import_all` (4573-4576): delegate to `check_import`.
- `visit_expression_stmt` (6790-6800): calls `expr_checker.accept`, then
  `type_requires_usage` check + fail. The accept is the hot path (already
  native via checkexpr seams); the fail is a Python callback.

**Partially ported (already have rust_ seams for decision heads):**
- `visit_assignment_stmt` (4658): `rust_classify_check_assignment` ported
  (issue #1090); the tail (binder.assign_type, check_simple_assignment)
  stays Python-side.
- `visit_return_stmt` (6802): `rust_classify_return_stmt_*` ported
  (issue #1004); accept + check_subtype tail stays Python-side.
- `visit_if_stmt` (7079): conditional narrowing uses
  `find_isinstance_check` (partially ported, #1086); binder frame push/pop
  + analyze_cond_branch stay Python-side.
- `visit_match_stmt` (7839): pattern checking partially ported
  (checkpattern.rs); the match-case traversal + binder frame management
  stays Python-side.
- `visit_class_def` (3973): many rust_classify_* seams (all_supers_gate,
  final_super, classvar_super, enum_*, metaclass_compat, etc.); the
  class body traversal + TypeInfo mutation stays Python-side.
- `visit_func_def` (2260): rust_classify_func_def_override, missing_annotations,
  new_signature ported; the function body traversal + scope push/pop
  stays Python-side.

**Cannot move to Rust traversal (live Python object mutation, plugin hooks):**
- `visit_decorator` (7591): plugin `get_class_decorator_hook` /
  `get_function_decorator_hook` calls mutate the FuncDef/ClassDef in place.
- `visit_with_stmt` (7748): plugin `get_base_class_hook` for
  `contextlib.contextmanager` etc. mutates TypeInfo bases.
- `visit_try_stmt` (7233): exception handler analysis mutates the binder
  with complex frame merge semantics that depend on live TypeInfo MRO
  walks.
- `visit_for_stmt` (7461): iterator type inference + binder frame
  management + `analyze_cond_branch` for the else clause.

### What does not exist yet

1. **Rust binder.** The `ConditionalTypeBinder` is pure Python. An H1
   driver needs a Rust-side binder that tracks the same conditional
   type map, reachability flags, and frame stack. The binder's data
   model (expression-keyed type maps, union/intersection merge) is
   complex but algorithmically self-contained (737 lines). It does not
   depend on TypeInfo mutation or plugin hooks.

2. **Traversal driver.** No Rust code walks the AST statement list.
   The existing `ReturnSeeker` traverser (traverser.rs, #1030) is a
   read-only visitor; H1 needs a stateful traversal that drives
   Python callbacks.

3. **Callback contract.** The Python `visit_*` methods need to be
   callable from Rust as callbacks. Today the dispatch goes
   `s.accept(self)` -> Python vtable; H1 inverts it to Rust driver
   -> Python callback per statement kind.

## 2. Minimum G state H1 needs

**H1 does NOT require G4 graduation.** The checker reads AST nodes
(Statement/Expression) but never mutates them. The binder tracks
types (which are Phase F objects), but the binder can stay Python-side
as a callback target even if the traversal driver is Rust.

The minimum G state for a first H1 vertical slice:
- **G1 expression nodes: shadow only (already landed, G1.0a/b).**
  The checker's `accept` dispatches to `expr_checker.accept` which
  reads expression node fields. These are already captured in the G1
  shadow. A read flip is not required because the Rust driver can
  call back to Python for expression checking.
- **G2 statement nodes: shadow only (already landed, G2.0).**
  The Rust driver needs to read statement kind, child block lists,
  and a few scalar fields (is_unreachable, line). These are already
  in the G2 shadow record. Again, a read flip is not required if
  the Rust driver calls back to Python for the visit_* body.
- **G3 symbol tables: not required for H1.** The checker reads
  `TypeInfo.names` for member lookups, but those go through the
  already-native subtype/checkmember resolver seams, not direct
  symbol table access.

**Falsifiable graduation signal:** a first H1 slice can be
demonstrated with the current G1.0b + G2.0 shadow state (record-only,
default-off) by having the Rust traversal driver read shadow records
for dispatch and call back to Python for visit_* bodies. The signal
is: does the Rust traversal + Python callback path match the current
`visit_block` -> `accept` -> `visit_*` path on testcheck + testtypes +
fine-grained + self-check?

## 3. First vertical slice candidate for H1

**`visit_block` traversal with native binder frame management.**

Scope: move the `visit_block` loop (checker.py:4599-4620) into a Rust
driver that:
1. Reads the block's `is_unreachable` flag (G2 shadow record).
2. Iterates `b.body` statements (reads the statement list from the
   G2 shadow).
3. For each statement, checks binder reachability (calls a Rust
   binder) and either marks unreachable or dispatches to the Python
   `visit_*` callback.
4. Manages the `expr_cache.clear()` after each statement (Python
   callback, one-liner).

The binder frame management (push on block entry, pop on exit,
break/continue handling) can be split into a second slice.

**Parity surface:** testcheck 8,198/15/7/0 exact, testtypes 3,483/7,
fine-grained 747/27 + daemon 38, cold self-check clean. The gate is
`Options.native_checker_traversal` default-off + `TEST_NATIVE_CHECKER_TRAVERSAL`
in test helpers.

**Expected cost/benefit:** the `visit_block` loop itself is a
dispatch overhead measurement, not a computation win. The per-statement
cost is dominated by the visit_* body (expression checking, subtype
checks), which is already native. A Rust traversal driver saves the
Python vtable dispatch (~1us per statement) but adds PyO3 callback
overhead (~1-2us per statement). Net: likely within noise, same as
Phase F. The real H1 win would come from moving the binder to Rust
(eliminating the Python binder's dict operations on every assignment),
but that is a larger slice.

**Recommendation:** the `visit_block` slice is a valid falsifiable
probe but has a low expected payoff. File it only if a second slice
(native binder) is also scoped, because the traversal alone is
overhead-neutral.

## 4. H3: message formatting

### What blocks moving off live-object formatting

`mypy/messages.py` (4,212 lines) already has significant native seams:
- `rust_format_type` / `rust_format_type_bare` / `rust_format_type_distinctly`
  (messages.py:3345, 3377, 3529): type formatting.
- `rust_classify_has_no_attr` (messages.py:394): the 11-arm
  special-case front for attribute errors.
- `rust_make_inferred_type_note` (messages.py:4144): inferred-type note.
- `rust_format_string_list` / `rust_format_item_name_list` /
  `rust_format_key_list` (messages.py:3880, 3898, 4184): list formatting.

What remains Python-side and blocks full H3:
1. **`format_type` depends on live TypeInfo.** The native seam reads
   the TypeInfo `fullname` for Instance rendering. Without a G4
   graduation (Rust-owned TypeInfo storage), the Rust formatter must
   call back to Python for every Instance name lookup. This is the
   same F-phase blocker: the type graph is Python-canonical.
2. **Error message strings.** Many messages are constructed from
   live Python objects via `.format()` or f-strings. The Rust seams
   port the formatting of type strings, but the message templates
   themselves (e.g. `INCOMPATIBLE_RETURN_VALUE_TYPE`) stay Python-side
   because they are keyed by error codes in `message_registry`.
3. **`pretty_callable` definition dependency.** The native
   `pretty_callable` drops the `definition` field (a live FuncDef),
   so named callables render nameless unless the shim re-stamps the
   definition (#1169, #1455). This is an F-phase identity issue.

**H3 is blocked on the same F-phase graduation that blocked F4.** The
messages kernel is mostly native, but the remaining Python-side work
is all live-object identity: TypeInfo names, FuncDef definitions, and
error-code-keyed message templates. These cannot move to Rust without
either G4 graduation (AST storage flip) or F4 graduation (type graph
storage flip), both of which are retired unclaimed (#1573).

## 5. Risk register deltas and ordered H sequence

### Risk register deltas

- **Binder correctness.** The `ConditionalTypeBinder` is the most
  algorithmically complex piece that H1 would move. Its union/intersection
  merge semantics are parity-critical: a wrong merge silently changes
  type narrowing, producing wrong answers, not crashes. The binder
  has 737 lines and no existing Rust port. Gate: full fine-grained
  suite (747 tests) + testtypes parity differential.
- **Callback overhead.** A Rust driver calling Python callbacks per
  statement is the Phase F pattern: PyO3 round-trip cost may exceed the
  Python vtable dispatch it replaces. Gate: work-share measurement
  within 10% of baseline, same as the F-phase gate.
- **Plugin hook timing.** Plugins mutate AST nodes during semanal,
  not during the checker pass. So H1 (checker driver) has minimal
  plugin interaction (only `get_function_decorator_hook` etc. in
  `visit_decorator`). H2 (semanal driver) is where plugin timing
  becomes critical.
- **Deferral lifecycle.** The semanal deferral loop (`self.defer()`
  -> re-process on next iteration) is a convergence protocol that
  depends on live symbol table state. H2 cannot move the deferral
  driver to Rust without G3.1 (symbol table read flip) + G4
  graduation. This is the hardest H sub-phase.

### Ordered H sequence

1. **H1a: `visit_block` traversal probe** (optional, low payoff).
   Rust traversal of the statement list with Python callbacks for
   visit_* bodies. Gate: `native_checker_traversal`, default-off.
   Signal: does the overhead break even?

2. **H1b: native binder** (the real H1 win). Port
   `ConditionalTypeBinder` to Rust. The binder's data model
   (expression-keyed type maps, frame stack) is self-contained.
   This is the largest single H slice (~737 lines of Python ->
   Rust) but also the highest payoff: every assignment, branch,
   and loop incurs binder operations. Gate: same as H1a.

3. **H1c: checker driver graduation.** With a native binder and
   traversal driver, the checker's `visit_*` bodies become Python
   callbacks over a Rust traversal + Rust binder. The complex
   visit_* bodies (try/except, match, with) stay Python callbacks.
   Graduation claim: "checker traversal runs in Rust."

4. **H2: semanal driver** (after G3.1 + G4). The semanal scope
   stack, deferral loop, and symbol table mutation are all
   Python-canonical. H2 cannot start until G3 graduates (symbol
   table read flip) and G4 graduates (AST storage flip). This is
   blocked on the same F-phase graduation that is retired unclaimed.

5. **H3: message formatting** (blocked on F4/G4 graduation). The
   messages kernel is mostly native. The remaining Python-side work
   is live-object identity (TypeInfo names, FuncDef definitions).
   No H3 slice can start until the type/AST storage flip graduates.

### Honest assessment

**H1 is startable now** with the current G1.0b + G2.0 shadow state,
but the expected payoff is within noise (same as Phase F). The binder
port (H1b) is the only H1 slice with a real performance lever, and it
is a 737-line algorithmic port with high parity risk.

**H2 and H3 are blocked** on G4 graduation, which requires the same
>=10% work-share win that Phase F could not deliver. The Phase F
close-out (#1573) applies unchanged: without a replacement-view
prototype that clears the win bar, H2 and H3 are not startable.

**Recommendation:** file H1b (native binder) as the first H slice if
the binder's dict operations are a measured bottleneck. Otherwise,
H is not startable until the storage-flip graduation bar is met, and
the next productive work is G3.1 (symbol table read flip) or the
performance bottleneck (per-SCC resolver cost).


## Corrections (appended 2026-09-17, from the wave-8 Phase-H design lane, #1770)

Five statements in the body above are wrong or unverified, each with evidence in
#1770. Read that issue before using this brief; the summary is kept here only so
the wrong claims are not reused:

1. **A Rust binder already exists** — `crates/type_kernel/src/binder.rs` with
   `mypy/binder.py:248` and `Options.native_binder` — so the binder is not a
   greenfield step.
2. **It is metadata-only**: types stay Python-side.
3. **It has no recorded ratio anywhere** in the tracked record.
4. **"The checker never mutates AST nodes" is false**: there are 42 node/`Var`
   write sites (e.g. `var.is_inferred` at `mypy/checker.py:6337`, `defn.type` at
   `:1783`, `lvalue_node.type` at `:4980`). A designer relying on the old claim
   would build the wrong differential.
5. **"`visit_decorator` calls class-decorator hooks" is false**: `self.plugin`
   appears three times in `checker.py` with zero hook calls; every such hook is in
   semanal.

Sequencing also changed: H1 is **not** next. It waits on a `Var` handle scheme
from the G-family read flips, because `mypy/literals.py:204` keys narrowing on
the live `Var` object. See #1770 for the order, the cross-run differential shape,
the counters and the negative control.
