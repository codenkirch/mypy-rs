# Next Big Leap: Detailed Issue Plan + Rust Percentage Estimate

## Current State

| Metric | Value |
|--------|-------|
| Rust bytes (GitHub) | 1,499,215 |
| Python bytes (GitHub) | 5,793,773 |
| Rust % | 20.5% |
| Rust LOC (crates/) | 41,136 |
| Rust files | 34 .rs files in type_kernel + ast_serialize + module_resolver + fs_probe |

## What "the next big leap" means

The 20% target is met. The next big leap targets **25-30% Rust** by porting
the remaining unported hot-path and mid-weight Python modules. This document
breaks down each issue with scope, risk, estimated Rust LOC/bytes, and
estimated Python bytes removed or made redundant.

## Estimation methodology

GitHub language stats are byte-based. A Rust port adds Rust bytes and
may remove Python bytes (if the Python code is deleted) or leave them
(if the Python code stays as a fallback). The strangler-fig approach
keeps Python code as fallback, so Python bytes mostly stay. The Rust %
increase comes primarily from adding Rust bytes, not removing Python.

Rule of thumb: 1 line of Rust averages ~40 bytes (including whitespace,
comments, formatting). This is calibrated from existing crates:

- type_kernel: 32,111 LOC / 1,182,021 bytes = 36.8 bytes/line
- ast_serialize: ~4,000 LOC / 197,260 bytes = ~49 bytes/line
- module_resolver: ~3,000 LOC / 118,305 bytes = ~39 bytes/line
- Overall: 41,136 LOC / 1,499,215 bytes = 36.4 bytes/line

Use **37 bytes/line** as the estimate.

---

## Issue #296: M18 — Subtype active flag graduation

**Scope:** Flip `_set_native_subtype_active(False)` to `True` in build.py.
The subtype resolver is already wired and parity-tested. The active flag
controls whether the top-level `is_subtype` call itself defers to Rust.

**Python target:** `mypy/subtypes.py` (108,145 bytes, 2,491 lines)

**Rust work:** ~0 new LOC (flag flip only, existing code in `subtypes.rs`)

**Risk:** Low. Resolver already parity-tested. Active flag just makes the
top-level call use Rust instead of only sub-checks.

**Parity gates:**
- testtypes: 313 passed baseline
- testcheck: 8151 passed baseline
- Fine-grained / daemon / cache: 1333 passed baseline

**Estimate:**
- New Rust bytes: 0
- Python bytes removed: 0 (Python stays as fallback)
- New Rust %: 20.5% (unchanged, but enables performance win)

---

## Issue #297: M19 — Port `checkstrformat` (printf/scanf format strings)

**Scope:** Port `mypy/checkstrformat.py` (46,444 bytes, 1,115 lines) to Rust.

**Python target:** `mypy/checkstrformat.py`

**Current Rust:** `checkstrformat.rs` is a 37-line stub.

**Rust work:** Port `check_str_format`, `printf_signature`, `scanf_signature`,
`parse_format` (literal extraction + validation). Self-contained module,
minimal plugin interaction.

**Estimated Rust LOC:** 1,000
**Estimated Rust bytes:** 37,000

**Risk:** Medium. Self-contained but format string parsing is fiddly
(precision, width, length modifier, conversion flags).

**Parity gates:** testcheck (printf/scanf test cases), new `NativeStrFormatSuite`.

---

## Issue #298: M20 — Port `checkmember` (member access resolution)

**Scope:** Port `mypy/checkmember.py` (64,518 bytes, 1,594 lines) to Rust.
This is `analyze_member_access` — the second-hottest dispatch in the type
checker after `check_call`.

**Python target:** `mypy/checkmember.py`

**Current Rust:** None.

**Rust work:** Port `analyze_member_access`, `analyze_typevar_member_access`,
`find_typevar_missing`, `analyze_class_attribute_access`,
`analyze_instance_attribute_access`, `analyze_decorator_member_access`.
Must handle plugin hook (`get_method_hook`) via the existing
`plugin_call_hook_known_absent` fast-path pattern.

**Estimated Rust LOC:** 1,500
**Estimated Rust bytes:** 55,500

**Risk:** High. `analyze_member_access` mutates AST (`store_type`), interacts
with plugin hooks, and is called from `check_call` (Instance branch),
`checkexpr`, and recursively. Must use the "Rust returns result +
descriptor, Python applies mutations" pattern.

**Dependencies:** M23 (expand_type resolver fix) is RESOLVED (PR #220) —
`analyze_member_access` calls `expand_type` for generic member access. M20
can proceed.

**Parity gates:** testcheck, testtypes, fine-grained.

---

## Issue #299: M21 — Port `messages` (error message formatting)

**Scope:** Port `mypy/messages.py` (141,669 bytes, 3,536 lines) to Rust.

**Python target:** `mypy/messages.py`

**Current Rust:** `messages.rs` is a 40-line stub.

**Rust work:** Port `format_type`, `format_type_distinctly`, `format_args`,
`format_callable`, error message templates. This is the largest unported
module by bytes. Message strings are user-visible and must match exactly.

**Estimated Rust LOC:** 2,500
**Estimated Rust bytes:** 92,500

**Risk:** Medium. Pure computation (no AST mutation), but large surface
area and exact string matching required. Start with hot messages, defer
rare ones.

**Parity gates:** testcheck (error message comparison), new
`NativeMessagesSuite`.

---

## Issue #300: M22 — Port `checkpattern` (PEP 634 pattern matching)

**Scope:** Port `mypy/checkpattern.py` (36,820 bytes, 885 lines) to Rust.

**Python target:** `mypy/checkpattern.py`

**Current Rust:** None.

**Rust work:** Port `PatternChecker.visit_pattern` dispatch and all pattern
type handlers: capture, class, mapping, sequence, AS, OR, literal, value.

**Estimated Rust LOC:** 800
**Estimated Rust bytes:** 29,600

**Risk:** Medium. Self-contained but complex pattern dispatch. No plugin
interaction.

**Parity gates:** testcheck (pattern matching tests).

---

## Issue #301: M23 — Fix `expand_type` resolver (316 known failures) — RESOLVED

**Status:** RESOLVED via PR #220 (commit 14ccc0f901, merged Aug 4 2026).
The resolver is uncommented in build.py:1261 and graduated to production.

**Scope:** Fix the 316 parity failures in the `expand_type` resolver and
uncomment `_set_native_expand_type_resolver(resolver)` in build.py.

**What was fixed:** Three parity gaps in the Python shim
(`_needs_python`/`_env_substitutes_unsafe` in expandtype.py):
- Named callables substituted from the env lose their FuncDef/Decorator
  definition node on a wire round-trip, corrupting error formatting.
- Recursive TypeAliasType would loop during decode, hanging testcheck.
- Walking every env value per expand call was O(env), blowing up RSS to
  20GB+ on big-literal tests.

**Current Rust:** `expandtype.rs` (805 lines) exists. Resolver active.

**Rust work:** Fix failure categories in existing code. No new LOC expected,
just bug fixes.

**Estimated Rust LOC:** 0 (fixes only)
**Estimated Rust bytes:** 0

**Risk:** Medium. Failures are in generic substitution edge cases.

**Parity gates:** testcheck (8151 passed/0 failed), testtypes (313/0),
testinfer (106/0).

**Why it matters:** Unblocks M20 (checkmember needs expand_type).

---

## Issue #302: M24 — Fix `typeops` resolver

**Scope:** Fix parity failures in the typeops resolver and uncomment
`_set_native_typeops_resolver(resolver)` in build.py.

**Current Rust:** `typeops.rs` (1,984 lines) exists. Resolver commented out.

**Rust work:** Fix failure categories. Minimal new LOC.

**Estimated Rust LOC:** 0 (fixes only)
**Estimated Rust bytes:** 0

**Risk:** Low-Medium. typeops is mostly pure computation.

**Parity gates:** `NativeTypeOpsSuite`, testcheck.

---

## Issue #303: M25 — Full `check_call` port (Stage 4)

**Scope:** Full port of `check_call` arg-binding and type-var inference from
`mypy/checkexpr.py` / `mypy/checkcall.py` (if split). This is the hottest
dispatch in the type checker.

**Python target:** `check_call`, `check_callable_call`,
`check_overload_call`, `check_union_call`, `check_any_type_call` in
checkexpr.py (~3,000 lines of the 7,334-line file).

**Current Rust:** `checkcall.rs` (524 lines) has classifier logic only.

**Rust work:** Port arg-to-formal binding, `freshen_all_function_type_vars`,
`freeze_all_type_vars`, error emission (defer-on-error strategy), overload
resolution. Must handle 4 plugin hooks (see Stage 4 Design Spike in
migration doc).

**Estimated Rust LOC:** 2,500
**Estimated Rust bytes:** 92,500

**Risk:** High. Touches type-var inference, `store_type`, plugin callbacks.
7 open design questions documented in migration doc.

**Dependencies:** M20 (checkmember) should land first — check_call's
Instance branch calls `analyze_member_access`.

**Parity gates:** testcheck, testtypes, fine-grained, daemon, cache.

---

## Issue #304: M26 — Port `traverser` (AST traversal)

**Scope:** Port `mypy/traverser.py` (34,061 bytes, 1,167 lines) to Rust.
This is the default AST visitor/traverser that walks all node types.

**Python target:** `mypy/traverser.py`

**Current Rust:** `traverser.rs` (420 lines) exists but minimal.

**Rust work:** Complete the traverser to cover all node types. The existing
420-line stub handles a subset.

**Estimated Rust LOC:** 800 (additional)
**Estimated Rust bytes:** 29,600

**Risk:** Low. Pure traversal, no computation, no mutation, no plugins.

**Parity gates:** testcheck (traversal is exercised everywhere).

---

## Issue #305: M27 — Port `suggestions` (auto-fix suggestions)

**Scope:** Port `mypy/suggestions.py` (39,056 bytes, 1,068 lines) to Rust.
Generates "did you mean..." suggestions for type errors.

**Python target:** `mypy/suggestions.py`

**Current Rust:** None.

**Rust work:** Port `get_suggestion`, `rank_suggestions`, levenshtein
distance, fuzzy matching.

**Estimated Rust LOC:** 900
**Estimated Rust bytes:** 33,300

**Risk:** Low. Pure computation, no AST mutation, no plugins.

**Parity gates:** testcheck (suggestion tests).

---

## Issue #306: M28 — Port `server/deps` (dependency tracking)

**Scope:** Port `mypy/server/deps.py` (50,004 bytes, 1,142 lines) to Rust.
Computes fine-grained dependency records for incremental mode.

**Python target:** `mypy/server/deps.py`

**Current Rust:** None.

**Rust work:** Port `DependencyVisitor`, `get_dependencies`,
`get_fine_grained_deps`. The dependency-records extraction
(`BuildManager.all_imported_modules_in_file`) is already ported via
module_resolver; this is the broader visitor.

**Estimated Rust LOC:** 1,000
**Estimated Rust bytes:** 37,000

**Risk:** Medium. Must match daemon/incremental semantics exactly.

**Parity gates:** Fine-grained suite (1333 tests), daemon, cache.

---

## Summary Table

| Issue | Milestone | New Rust LOC | New Rust bytes | Risk | Dependencies |
|-------|-----------|-------------|----------------|------|--------------|
| #296 | M18 subtype active | 0 | 0 | Low | None |
| #297 | M19 checkstrformat | 1,000 | 37,000 | Medium | None |
| #298 | M20 checkmember | 1,500 | 55,500 | High | M23 |
| #299 | M21 messages | 2,500 | 92,500 | Medium | None |
| #300 | M22 checkpattern | 800 | 29,600 | Medium | None |
| #301 | M23 expand_type fix | 0 | 0 | Medium | None |
| #302 | M24 typeops fix | 0 | 0 | Low-Med | None |
| #303 | M25 check_call full | 2,500 | 92,500 | High | M20 |
| #304 | M26 traverser | 800 | 29,600 | Low | None |
| #305 | M27 suggestions | 900 | 33,300 | Low | None |
| #306 | M28 server/deps | 1,000 | 37,000 | Medium | None |
| **Total** | | **11,000** | **407,000** | | |

## Percentage Estimate After All Milestones

```
Current Rust bytes:   1,499,215
+ New Rust bytes:       407,000
= Future Rust bytes:  1,906,215

Current Python bytes: 5,793,773
(Python stays as fallback — strangler-fig keeps it)
= Python bytes:       5,793,773

Total bytes:          7,699,988
Rust %:               1,906,215 / 7,699,988 = 24.8%
```

**After all 11 milestones: ~25% Rust.**

That's a 4.3 percentage point increase from 20.5% to ~24.8%.

### What would push past 30%?

To reach 30% Rust (2,310,000 bytes), we'd need an additional ~404,000
Rust bytes beyond M18-M28. Candidates:

| Module | Python bytes | Est. Rust LOC | Est. Rust bytes |
|--------|-------------|---------------|-----------------|
| `nodes.py` (AST node definitions) | 188,604 | 3,000 | 111,000 |
| `types.py` (Type class hierarchy) | ~160,000 | 2,500 | 92,500 |
| `server/update.py` (fine-grained update) | 54,315 | 1,000 | 37,000 |
| `plugins/attrs.py` | 46,051 | 800 | 29,600 |
| `plugins/dataclasses.py` | 47,106 | 800 | 29,600 |
| `plugin.py` (plugin manager) | 36,250 | 700 | 25,900 |
| `dmypy_server.py` | 46,509 | 900 | 33,300 |
| `stubgen.py` + `stubgenc.py` | 118,390 | 2,000 | 74,000 |

However, `nodes.py` and `types.py` are the "widely shared mutable object
graphs" that the migration plan explicitly says NOT to port first — they
are plugin-visible and would break the strangler-fig approach. The plugin
and server modules are lower risk but lower value.

A 25% target is realistic for the next big leap. 30%+ would require
porting `nodes.py`/`types.py` (high risk) or the tooling modules
(`stubgen`, `dmypy_server`), which is a separate effort.

## Recommended Execution Order

```
Phase 1 (parallel, no deps):
  #296 M18 subtype active (flag flip)
  #297 M19 checkstrformat
  #301 M23 expand_type fix (DONE — PR #220)
  #302 M24 typeops fix
  #304 M26 traverser
  #305 M27 suggestions

Phase 2 (after M23 — M23 is done, can proceed):
  #298 M20 checkmember
  #299 M21 messages
  #300 M22 checkpattern

Phase 3 (after M20):
  #303 M25 check_call full

Phase 4 (after M25):
  #306 M28 server/deps
```

Phase 1 has 6 independent issues — dispatch as parallel subagents.
Phase 2 has 3 independent issues — parallel.
Phase 3 is the highest-risk single issue.
Phase 4 depends on the full check_call port for correct dependency tracking.
