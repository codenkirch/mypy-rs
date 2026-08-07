# Remaining Milestones: Rust Migration Plan

## Current State (August 2026)

### GitHub Language Stats
- Python: 5,793,773 bytes (79.1%)
- Rust: 1,496,698 bytes (20.4%)
- **Target: 20%+ Rust** — met

### Rust Crates (41,136 LOC total)

| Crate | LOC | Description |
|-------|-----|-------------|
| type_kernel | 32,111 | Core type-checking kernels (34 .rs files) |
| ast_serialize | ~4,000 | Native parser (ruff-based AST serialization) |
| module_resolver | ~3,000 | Native module finder |
| fs_probe | ~2,000 | Filesystem probing |

### Native Gates Status

**Production (default-on, `native_type_kernel=True`):**
- `erase_type`, `remove_instance_last_known_values`
- `join` (active + resolver)
- `mro` (active + resolver)
- `expand_type` (active only; resolver commented out, 316 known failures)
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

**Hardcoded off:**
- `subtype` active flag (`_set_native_subtype_active(False)`) — resolver is wired but the per-call active gate is off. The subtype resolver handles `is_subtype` calls but the Python path still runs the visitor. Enabling the active flag would make `is_subtype` itself defer to Rust.

**Not ported (no Rust counterpart):**
| Python Module | Lines | Role |
|---------------|-------|------|
| `checkmember.py` | 1,594 | Member access resolution (`analyze_member_access`) |
| `checkpattern.py` | 885 | Pattern matching (PEP 634) |
| `infer.py` | 80 | Type inference for anonymous code |
| `maptype.py` | 109 | Type mapping utilities |

**Stubs (Rust exists but minimal):**
| Rust File | Lines | Python Target |
|-----------|-------|---------------|
| `checkstrformat.rs` | 37 | `checkstrformat.py` (1,115 lines) |
| `messages.rs` | 40 | `mypy.messages` (error message formatting) |
| `checkcall.rs` | 524 | `check_call` in checkexpr.py (classifier only, no full port) |

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

### M19: `checkstrformat` Port
**Goal:** Port `mypy/checkstrformat.py` (1,115 lines) to Rust.
**Current state:** `checkstrformat.rs` is a 37-line stub.
**Risk:** Medium — strformat checking is self-contained (printf/scanf format string validation) with minimal plugin interaction.
**Steps:**
1. Port `printf_signature` and `scanf_signature` generation
2. Port `check_str_format` call
3. Add PyO3 function, wire to build.py gate
4. Parity tests via `NativeStrFormatSuite`
**LOC impact:** ~1,000 Rust lines, removes 1,115 Python lines from hot path

### M20: `checkmember` Port (Partial)
**Goal:** Port `analyze_member_access` and `analyze_typevar_member_access` from `checkmember.py` (1,594 lines).
**Risk:** High — `analyze_member_access` is called from `check_call` (Instance branch), `checkexpr`, and `checkmember` itself. It mutates the AST (`store_type`) and interacts with the plugin (`get_method_hook`).
**Steps:**
1. Port `analyze_member_access` as a Rust resolver (returns the result type + side-effect descriptor, Python applies mutations)
2. Port `analyze_typevar_member_access` (simpler, no mutations)
3. Port `find_typevar_missing` and helpers
4. Wire to build.py gate, parity tests
**LOC impact:** ~1,500 Rust lines. This is the highest-value remaining port after subtype-active — `analyze_member_access` is the second-hottest dispatch in the type checker.
**Note:** The `get_method_hook` plugin interaction means this port should use the same `plugin_call_hook_known_absent` fast-path pattern as the checkexpr plugin-hook snapshot.

### M21: `messages` Port
**Goal:** Port error message formatting from `mypy.messages` to Rust.
**Current state:** `messages.rs` is a 40-line stub.
**Risk:** Medium — message formatting is pure computation (no AST mutation), but the message strings are user-visible and must match exactly.
**Steps:**
1. Port `format_type` helpers
2. Port `format_type_distinctly`
3. Port message templates (too many messages, start with the hot ones)
4. Wire to build.py gate
**LOC impact:** ~1,000-2,000 Rust lines depending on scope
**Priority:** Low — message formatting is not on the hot path (errors are rare in correct code)

### M22: `checkpattern` Port
**Goal:** Port `mypy/checkpattern.py` (885 lines) — PEP 634 pattern matching type checking.
**Risk:** Medium — pattern matching is self-contained but complex (capture patterns, class patterns, mapping patterns, sequence patterns).
**Steps:**
1. Port `PatternChecker.visit_pattern` dispatch
2. Port each pattern type handler
3. Wire to build.py gate, parity tests
**LOC impact:** ~800 Rust lines
**Priority:** Low — pattern matching is not on the hot path for most codebases

### M23: `expand_type` Resolver Fix
**Goal:** Fix the 316 known parity failures in the `expand_type` resolver and uncomment it in build.py.
**Current state:** `expand_type` active flag is on (per-call gate works), but the resolver is commented out due to 316 failures.
**Risk:** Medium — the failures are in generic substitution edge cases.
**Steps:**
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
**Current state:** Plugin-hook snapshot shipped (PR #291). `checkcall.rs` has classifier logic (524 lines). Full port not started.
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

Current: 1.49M Rust bytes / 7.31M total = 20.4%. Each milestone adds Rust LOC and removes Python LOC from the hot path:

| Milestone | New Rust LOC | Rust bytes added | Est. new % |
|-----------|--------------|-------------------|------------|
| M18 | ~0 (flag flip) | 0 | 20.4% |
| M19 | ~1,000 | ~40K | 20.9% |
| M20 | ~1,500 | ~60K | 21.6% |
| M21 | ~1,500 | ~60K | 22.3% |
| M22 | ~800 | ~32K | 22.7% |
| M25 | ~2,500 | ~100K | 24.0% |

The 20% target is sustainable even without new milestones — the existing 41K Rust LOC is enough. New milestones extend it toward 24%.
