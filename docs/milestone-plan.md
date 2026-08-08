# Remaining Milestones: Rust Migration Plan

## Current State (August 2026)

### GitHub Language Stats
- Python: 5,895,804 bytes (77.5%)
- Rust: 1,716,197 bytes (22.5%)
- **Target: 20%+ Rust** — met

### Rust Crates (38,459 LOC total)

| Crate | LOC | Description |
|-------|-----|-------------|
| type_kernel | ~38,500 | Core type-checking kernels (42 .rs files) |
| ast_serialize | ~4,000 | Native parser (ruff-based AST serialization) |
| module_resolver | ~3,000 | Native module finder |
| fs_probe | ~2,000 | Filesystem probing |

### Native Gates Status

**Production (default-on, `native_type_kernel=True`):**
- `erase_type`, `remove_instance_last_known_values`
- `join` (active + resolver)
- `mro` (active + resolver)
- `expand_type` (active + resolver, graduated to production via PR #220)
- `typeops` (active only; resolver commented out)
- `semanal` (active, visitor helpers)
- `typeanal` (active, queries)
- `erase_typevars` (active)
- `constraints` (active + resolver)
- `solve` (active + resolver)
- `visitor` (active + types variant)
- `checker` (active, statement visitors + complex visitors)
- `checkexpr` (plugin-hook registry only)
- `argmap` (active)
- `applytype` (resolver + typeinfo map)
- `freshen` (parity-tested)
- `lkv` (last-known-value erasure)
- `cache` (parity-tested)
- `checkmember` (active + resolver: `bind_self_fast` decision gate plus `has_operator`, `meta_has_operator`, `instance_fallback`, `defined_in_superclass`; see PR #325)
- `messages` (active: `append_invariance_notes`, `append_numbers_notes`, `append_union_note`, `pretty_callable`; no resolver needed; see PR #324)
- `checkpattern` (active + resolver: pattern starred/sequence helpers; see PRs #315, #323)
- `checkstrformat` (active: format-string parsing helpers; see PR #297)

**Hardcoded off:**
- `subtype` active flag (`_set_native_subtype_active(False)`) — resolver is wired but the per-call active gate is off. The subtype resolver handles `is_subtype` calls but the Python path still runs the visitor. Enabling the active flag would make `is_subtype` itself defer to Rust.

**Not ported (no Rust counterpart):**
| Python Module | Lines | Role |
|---------------|-------|------|
| `infer.py` | 80 | Type inference for anonymous code |
| `maptype.py` | 109 | Type mapping utilities |

**Stubs (Rust exists but minimal):**
| Rust File | Lines | Python Target |
|-----------|-------|---------------|
| `checkcall.rs` | 813 | `check_call` in checkexpr.py (classifier only, no full port) |
| `checkexpr_functions.rs` | 1,455 | plugin-hook helper registry only |

### Performance (M17 graduation, cold self-check)
- `parse_time`: 4.997s (1.0% reduction from Python)
- `semanal_time`: 1.110s (59.1% reduction)
- `type_check_time`: 2.326s (76.6% reduction)
- **Total: 8.433s (52.4% reduction)** from Python baseline of 17.713s

---

## Remaining Milestones

### M18: Subtype Active Flag Graduation
**Goal:** Enable `_set_native_subtype_active(True)` in build.py.
**Risk:** Low — the subtype resolver is already wired and parity-tested. The active flag controls whether `is_subtype` itself defers to Rust or just uses the Rust resolver for sub-checks. Enabling it means the top-level `is_subtype` call goes through Rust.
**Steps:**
1. Flip `_set_native_subtype_active(False)` to `True` in build.py
2. Run full parity suites (testtypes, testcheck)
3. If green, ship as production default
**LOC impact:** Minimal Rust change, large performance win (subtype is the hottest sub-check in type checking)

### M19: `checkstrformat` Port (DONE)
**Goal:** Port `mypy/checkstrformat.py` (1,115 lines) to Rust.
**Current state:** DONE — `checkstrformat.rs` is 636 lines (PR #297). The pure
format-string parsing helpers (`rust_is_numeric_format_type`,
`rust_parse_conversion_specifiers`, `rust_find_non_escaped_targets`,
`rust_parse_format_value`) are ported and gated behind
`_set_native_strformat_active`. The type-checking logic
(`StringFormatterChecker` methods) stays in Python — it needs the checker,
message builder, and named_type.
**Risk:** Medium — strformat checking is self-contained (printf/scanf format string validation) with minimal plugin interaction.
**LOC impact:** ~600 Rust lines landed.

### M20: `checkmember` Port (Partial)
**Goal:** Port `analyze_member_access` and `analyze_typevar_member_access` from `checkmember.py` (1,594 lines).
**Current state:** PARTIAL — `checkmember.rs` is 927 lines (PR #325). The
`bind_self_fast` decision gate is live (pure helper, no checker state) and four
operator helpers (`has_operator`, `meta_has_operator`, `instance_fallback`,
`defined_in_superclass`) are ported behind the resolver. The method-path fast
path (`analyze_instance_member_access`, `MethodFullname` lookup) is ported
behind the resolver via PR #332. The general `analyze_member_access` /
`analyze_typevar_member_access` port remains open — it is the highest-value
remaining port and the second-hottest dispatch in the type checker.
**Risk:** High — `analyze_member_access` is called from `check_call` (Instance branch), `checkexpr`, and `checkmember` itself. It mutates the AST (`store_type`) and interacts with the plugin (`get_method_hook`).
**Steps:**
1. Port `analyze_member_access` as a Rust resolver (returns the result type + side-effect descriptor, Python applies mutations)
2. Port `analyze_typevar_member_access` (simpler, no mutations)
3. Port `find_typevar_missing` and helpers
4. Wire to build.py gate, parity tests
**LOC impact:** ~1,500 Rust lines. This is the highest-value remaining port after subtype-active — `analyze_member_access` is the second-hottest dispatch in the type checker.
**Note:** The `get_method_hook` plugin interaction means this port should use the same `plugin_call_hook_known_absent` fast-path pattern as the checkexpr plugin-hook snapshot.

### M21: `messages` Port (DONE)
**Goal:** Port error message formatting from `mypy.messages` to Rust.
**Current state:** DONE — `messages.rs` is 2,145 lines (PR #324). Four helper
functions are ported: `append_invariance_notes`, `append_numbers_notes`,
`append_union_note`, `pretty_callable`. They are pure computation (no AST
mutation) and run un-gated (no resolver switch needed).
**Risk:** Medium — message formatting is pure computation (no AST mutation), but the message strings are user-visible and must match exactly.
**LOC impact:** ~2,100 Rust lines landed.

### M22: `checkpattern` Port (DONE)
**Goal:** Port `mypy/checkpattern.py` (885 lines) — PEP 634 pattern matching type checking.
**Current state:** DONE — `checkpattern.rs` is 957 lines (PRs #315, #323). The
pattern starred/sequence helpers are ported and gated behind the pattern
resolver. `PatternChecker` dispatch for the full pattern grammar stays in
Python, but the deepest recursion hot spot (sequence/starred analysis) is Rust.
**Risk:** Medium — pattern matching is self-contained but complex (capture patterns, class patterns, mapping patterns, sequence patterns).
**LOC impact:** ~950 Rust lines landed.

### M23: `expand_type` Resolver Fix (RESOLVED)
**Goal:** Fix the 316 known parity failures in the `expand_type` resolver and uncomment it in build.py.
**Current state:** RESOLVED via PR #220 (commit 14ccc0f901). The resolver is
uncommented in build.py:1261 and graduated to production. Three parity gaps
(named-callable definition loss on env substitution, recursive TypeAliasType
decode loops, O(env) per-call memory growth) were fixed with targeted
`_needs_python`/`_env_substitutes_unsafe` deferrals. Full parity verified:
testcheck 8151/0, testtypes 313/0, testinfer 106/0.
**Risk:** Medium — the failures are in generic substitution edge cases.
**Steps:** (completed)
1. Run expand_type parity suite, categorize failures
2. Fix each failure category
3. Uncomment `_set_native_expand_type_resolver(resolver)` in build.py
4. Full parity suite green
**LOC impact:** Fixes to existing Rust code, no new LOC
**Priority:** Medium — unblocks M20 (checkmember needs expand_type for generic member access)

### M24: `typeops` Resolver Fix
**Goal:** Fix parity failures in the typeops resolver and uncomment it in build.py.
**Current state:** `typeops` active flag is on, resolver commented out.
**Risk:** Low-Medium — typeops is mostly pure computation.
**Steps:**
1. Run typeops parity suite, categorize failures
2. Fix each failure
3. Uncomment `_set_native_typeops_resolver(resolver)` in build.py
4. Full parity suite green
**LOC impact:** Fixes to existing Rust code
**Priority:** Medium — typeops resolver enables the Rust path for `make_simplified_union`, `simple_literal_type`, etc.

### M25: Stage 4 Full Port — `check_call` / `check_callable_call`
**Goal:** Full port of `check_call` arg-binding and type-var inference.
**Current state:** Plugin-hook snapshot shipped (PR #291). `checkcall.rs` has classifier logic (524 lines). The `method_fullname` helper is ported behind the resolver via PR #333. Full `check_call` port not started.
**Numbering note:** Issue #330 carries the "M25 method_fullname" title, distinct from the original M25 `check_call` plan below.
**Risk:** High — see "Stage 4 Design Spike" in `docs/rust-migration-strangler.md` (7 open questions).
**Steps:**
1. Resolve open question 1 (defer-mutation boundary)
2. Resolve open question 2 (plugin hook replay split)
3. Port `check_callable_call` arg-to-formal binding
4. Port `freshen_all_functions_type_vars` / `freeze_all_type_vars`
5. Port error emission (defer-on-error strategy)
6. Wire to build.py gate, parity tests
**LOC impact:** ~2,000-3,000 Rust lines
**Priority:** Highest value but highest risk. Should follow M18-M24 to build confidence and fix the `expand_type` resolver dependency.

---

## Milestone Ordering

```
M18 (subtype active)     ─── lowest risk, immediate perf win
M23 (expand_type fix)    ─── unblocks M20
M24 (typeops fix)        ─── parallel with M23
M19 (checkstrformat)     ─── parallel, self-contained
M20 (checkmember)        ─── highest value after M18
M21 (messages)           ─── parallel, low priority
M22 (checkpattern)       ─── parallel, low priority
M25 (check_call full)    ─── last, depends on M20+M23
```

## Sustaining 20%+ Rust

Current: 1.731M Rust bytes / 7.647M total = 22.64%. Each milestone adds Rust LOC and removes Python LOC from the hot path:

| Milestone | New Rust LOC | Rust bytes added | Est. new % |
|-----------|--------------|-------------------|------------|
| M18 | ~0 (flag flip) | 0 | 20.4% |
| M19 | ~600 (landed) | ~220K total | 22.5% |
| M20 | ~927 (landed) | ~220K total | 22.5% |
| M21 | ~2,145 (landed) | ~220K total | 22.5% |
| M22 | ~957 (landed) | ~220K total | 22.5% |
| M25 | ~2,500 | ~100K | 24.0% |

**Honest note on the 24.8% projection:** the 11-milestone plan projected
1.907M Rust bytes (24.8%), but the issues that closed as umbrellas landed as
self-contained helper subsets, not full module ports (strangler-fig — Python
stays as the fallback behind each gate). Actual increment from the 20.4% start
was ~217K bytes (~5.9K LOC) for the original milestones, plus the M20/M25 method
path ports (PR #332, PR #333), landing at 22.64% (1,731,579 / 7,647,253), not
24.8%. This is still well past the 20% target.

The 20% target is sustainable even without new milestones — the existing 41K Rust LOC is enough. New milestones extend it toward 24%.
