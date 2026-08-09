# Remaining Milestones → Final Two Ports

## Current State (verified live, August 2026)

### GitHub Language Stats
- Python: 5,919,276 bytes (77.4%)
- Rust: 1,731,654 bytes (22.56%)
- Total: 7,675,140
- **Target: 20%+ Rust — met and sustained**

### Native Gates Status

**Production (default-on via `native_type_kernel`):**
- `erase_type`, `remove_instance_last_known_values` (erasetype)
- `join`, `mro`, `expand_type`, `constraints`, `solve` (active + resolver)
- `typeops` (active + resolver, production)
- `semanal`, `typeanal`, `erase_typevars` (active)
- `visitor`, `checker` (active)
- `checkexpr` (plugin-hook registry only)
- `argmap`, `applytype` (active/resolver)
- `freshen`, `lkv`, `cache` (parity-tested)
- `messages` (active: `format_type` family + notes helpers + suggestions; PR #324 + follow-ups)
- `checkmember` (active + resolver: `bind_self_fast`, operator helpers, method fast path; PRs #325, #332)
- `checkpattern` (active + resolver; PRs #315, #323)
- `checkstrformat` (active; PR #297)
- `serverdeps` (`get_type_triggers`; PR #318)

**Subtype active:** SHIPPED — resolver + active flag in production (M18).

**Not ported (no Rust counterpart):**
| Python Module | Lines | Role |
|---------------|-------|------|
| `infer.py` | 80 | Type inference for anonymous code |
| `maptype.py` | 109 | Type mapping utilities |

**Remaining seam stubs:**
| Rust File | Lines | Python Target | Status |
|-----------|-------|---------------|--------|
| `checkcall.rs` | 524 | `check_callable_call` in checkexpr.py | helpers only → **#341** |
| `checkmember.rs` | 927 | `analyze_member_access` general | method fast path only → **#342** |

### Performance (M17 graduation, cold self-check)
- `parse_time`: 4.997s (1.0% reduction from Python)
- `semanal_time`: 1.110s (59.1% reduction)
- `type_check_time`: 2.326s (76.6% reduction)
- **Total: 8.433s (52.4% reduction)** from Python baseline of 17.713s

---

## M25 (final port): `check_callable_call` arg-binding — #341

**Goal:** Port the `check_callable_call` arg-to-formal binding and
type-var inference tail from `mypy/checkexpr.py` (~line 1936) to Rust,
plus the `argmap.py` binding helpers it needs.

**Current state:** `checkcall.rs` (524 L) has decision helpers only
(`rust_classify_call`, `rust_normalize_callable`, `rust_real_union`,
`rust_possible_none_type_var_overlap`). Plugin-hook snapshot shipped
(PR #291); `method_fullname` behind resolver (PR #333).

**Risk:** High — arg binding touches `store_type`, `self.accept`,
4 plugin hooks, and type-var freshen/freeze.

**Steps:**
1. Add `_set_native_checkcall_active` gate, wire from build.py (mirror
   the erasetype/typeops gate propagation pattern)
2. Port the binding tail: Rust returns bound args + inferred type vars,
   Python applies `store_type`/accept/plugin/message side effects
3. Defer to Python on any plugin-visible or Python-only path (return
   None → caller falls back)
4. Differential parity: full suite with and without
   `TEST_NATIVE_TYPE_KERNEL=1`
5. New `NativeCheckCallSuite` parity tests (mirror `NativeTypeOpsSuite`)

**LOC impact:** ~1,600 Rust lines (~60,000 bytes).

## M20 (final port): `analyze_member_access` general — #342

**Goal:** Complete the `analyze_member_access` general dispatch and
`analyze_class_attribute_access` port. The Instance fast path and
operator helpers are done; the general path (type vars, class attrs,
plugins, `store_type`) stays in Python.

**Current state:** `checkmember.rs` (927 L) — `bind_self_fast` gate,
operator helpers, `MethodFullname` method fast path (PRs #325, #332).

**Risk:** High — general path mutates AST (`store_type`), calls
`get_method_hook`, dispatches to `analyze_class_attribute_access`.

**Steps:**
1. Port the general dispatch behind the existing
   `_native_checkmember_active` gate/resolver
2. `analyze_typevar_member_access` first (simpler, no mutations)
3. `analyze_class_attribute_access` + `find_typevar_missing` + helpers
4. Plugin hooks via `plugin_call_hook_known_absent` fast-path
5. Differential parity (kernel on/off), fine-grained + daemon included

**LOC impact:** ~1,500 Rust lines (~55,000 bytes).

---

## Milestone Ordering (remaining)

```
#341 check_callable_call (fg343)  ─── parallel
#342 analyze_member_access  (fg344)  ─── parallel
docs refresh               (fg342)  ─── parallel, merge first
```

Both ports are independent; no ordering constraint between them.

## Sustaining 20%+ Rust

Current: 1,731,654 Rust bytes / 7,675,140 total = 22.56%. Each of the
two final ports adds ~55-60K Rust bytes:

| Port | New Rust bytes | Est. new % |
|------|----------------|------------|
| #341 check_callable_call | ~60,000 | ~23.3% |
| #342 analyze_member_access | ~55,000 | ~24.0% |
| both + Python deletion of replaced paths | ~115,000 | ~24-25.5% |

The 20% target is sustainable without new ports — the existing ~41K Rust
LOC carries it. The two final ports extend it toward 25%.