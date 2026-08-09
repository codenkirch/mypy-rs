# Next Big Leap: Detailed Issue Plan + Rust Percentage Estimate

## Current State (verified live)

| Metric | Value |
|--------|-------|
| Rust bytes (GitHub) | 1,731,654 |
| Python bytes (GitHub) | 5,919,276 |
| Rust % | 22.56% |
| Total bytes | 7,675,140 |

The 20% target is met and sustained. The next big leap targets
**25-25.5% Rust** by completing the two remaining hot-path ports.

## What "the next big leap" means

The 11-milestone plan (M18-M28) is essentially complete. Most M-issues
shipped not as full module ports but as self-contained helper subsets
behind gates (strangler-fig — Python stays as fallback). Status of every
milestone:

| Milestone | Status |
|-----------|--------|
| M18 subtype active | SHIPPED — resolver wired + active, production |
| M19 checkstrformat | DONE (PR #297) |
| M20 checkmember | PARTIAL — helpers + method fast path (PRs #325, #332); general `analyze_member_access` remains → **#342** |
| M21 messages | DONE (PR #324) — incl. `format_type`/`format_type_distinctly` family |
| M22 checkpattern | DONE (PRs #315, #323) |
| M23 expand_type fix | DONE (PR #220) |
| M24 typeops resolver | SHIPPED — resolver live in production |
| M25 check_call | PARTIAL — checker/checkcall helpers (PRs #291, #333); `check_callable_call` arg-binding remains → **#341** |
| M26 traverser | DROPPED — low value (#304 closed) |
| M27 suggestions | DONE (messages.rs, `_native_suggestions_active`) |
| M28 server/deps | DONE (PR #318) — `get_type_triggers` + native resolver dep-records |

## The two remaining ports

### Issue #341: `check_callable_call` full arg-binding + type-var inference

**Python target:** `mypy/checkexpr.py` — `check_callable_call`
(~line 1936), plus `infer_function_type_arguments` /
`map_actuals_to_formals` in `mypy/argmap.py`.

**Current Rust:** `checkcall.rs` (524 L) has decision helpers only
(`rust_classify_call`, `rust_normalize_callable`, `rust_real_union`,
`rust_possible_none_type_var_overlap`, lib.rs:323-330). No
`_set_native_checkcall` gate.

**Estimated Rust LOC:** 1,600; **bytes:** ~60,000

**Risk:** High. Arg-to-formal binding, `freshen_all_function_type_vars`
/ `freeze_all_type_vars`, `store_type` side effects, 4 plugin hooks.
Seam pattern: Rust returns result/decision, Python applies
`self.accept`/`store_type`/plugin/message mutations; defer to Python on
any plugin-visible or Python-only path.

**Parity gates:** testcheck (call tests), testtypes, fine-grained.

### Issue #342: `analyze_member_access` general path

**Python target:** `mypy/checkmember.py` — `analyze_member_access`
(~line 235) general dispatch + `analyze_class_attribute_access`
(~line 1251) + helpers.

**Current Rust:** `checkmember.rs` (927 L) — `bind_self_fast` gate,
operator helpers, method-path fast path. General dispatch still Python.

**Estimated Rust LOC:** 1,500; **bytes:** ~55,000

**Risk:** High. Mutates AST (`store_type`), interacts with plugin hooks
(`get_method_hook` via `plugin_call_hook_known_absent` fast-path), called
recursively from `check_call` and `checkexpr`.

**Parity gates:** testcheck, testtypes, fine-grained.

## Percentage Estimate After the Two Ports

```
Current Rust bytes:   1,731,654
+ New Rust bytes:      ~115,000
= Future Rust bytes: ~1,846,654

Python bytes (fallback kept): 5,919,276
Total bytes:                 ~7,790,000
Rust %:                     ~23.7-24%
```

If either port also lets Python-side redundant paths be deleted
(checkexpr.py/checkmember.py bodies replaced by the Rust seam), the
denominator shrinks and the % lands at **~25-25.5%**. Realistic target
for this leap: **24-25.5%**.

## What would push past 25% (future candidates, not in this leap)

| Module | Python bytes | Est. Rust LOC | Est. Rust bytes |
|--------|-------------|---------------|-----------------|
| `nodes.py` (AST node defs) | 188,604 | 3,000 | 111,000 |
| `types.py` (Type hierarchy) | ~160,000 | 2,500 | 92,500 |
| `server/update.py` | 54,315 | 1,000 | 37,000 |
| `plugins/attrs.py` | 46,051 | 800 | 29,600 |
| `plugins/dataclasses.py` | 47,106 | 800 | 29,600 |
| `dmypy_server.py` | 46,509 | 900 | 33,300 |
| `stubgen.py` + `stubgenc.py` | 118,390 | 2,000 | 74,000 |

`nodes.py` and `types.py` are the "widely shared mutable object graphs"
the migration plan explicitly says NOT to port first — plugin-visible,
would break the strangler-fig approach. 30%+ would require either those
(AST/Type reimplementation) or a separate Rust checker engine that
replaces the Python walker entirely.

## Recommended Execution Order

```
Phase 1 (parallel, no deps):
  #341 check_callable_call arg-binding   (fg343)
  #342 analyze_member_access general     (fg344)
  docs refresh                           (fg342, this PR)

Phase 2:
  Measure: post-merge Rust % from github languages API
  Re-evaluate next candidates (beyond 25%) against measured hot spots
```

Both ports are independent and can ship in parallel. Each follows the
same contract: Rust returns result/None, Python applies mutations, gate
off by default (`TEST_NATIVE_TYPE_KERNEL=1` differential), full parity
suite green before merge.