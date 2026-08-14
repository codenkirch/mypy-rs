# Next Big Leap: Detailed Issue Plan + Rust Percentage Estimate

## Current State (verified live, 2026-08-14)

| Metric | Value |
|--------|-------|
| Rust bytes (GitHub) | 3,156,702 |
| Python bytes (GitHub) | 6,284,312 |
| Rust % (GitHub) | 33.4% |
| Total bytes | 9,441,014 |

The 20% target is met and sustained; 33.4% exceeds the 30% target (D4).
**The migration plan's module port
list is exhausted**: `maptype.py` landed as the final port (PR #427) and
`infer.py` closed without a port (#426, its wrapper is glue over the
already-native constraints + solve path). The checkexpr-family slice ports
(#380-#387) all landed, plus the semanal/plugin/daemon helper tracks
(#389, #391) and the crash fixes (#429/#430). Overload-error-reporting
helper `arg_approximate_similarity` ported as #432 (PR #433) with a
kernel-ON/OFF differential of 911=911 and full-testtypes+testcheck 8726
passed. The checker generator/coroutine return-type cluster
(`is_generator_return_type`, yield/receive/return extraction,
`get_coroutine_return_type`) ported as #434 (PR #435), which also fixed a
shared `is_subtype` parity gap (`is_proper_subtype(Instance, Any)` now
decides False instead of deferring); full kernel-ON vs OFF 8726=8726.

Remaining work targets deeper coverage inside ported modules (checkexpr
family, semanal scope handling) and the last large lever `semanal.py`
symbol resolution + scope handling (#348) — the next plateau is
**~28-30%** via those.

## Roadmap summary (from the 11-milestone plan)

The M18-M28 plan is complete. Most M-issues shipped as self-contained
helper subsets behind gates (strangler-fig, Python stays as fallback).
The two original "final ports" #341 (check_callable_call) and #342
(analyze_member_access general) were re-scoped into the issue-per-port
family (#380-#387) and all landed; the module-level "final two"
(`infer.py`, `maptype.py`) are resolved as described above.

## The active ports

### checkexpr family (serial, one speed-coder at a time)

All issues touch `mypy/checkexpr.py`, so they are executed serially, each
rebased on fresh `main`:

| Issue | Port | Python region | Risk |
|-------|------|---------------|------|
| #386 | conditional-expression join remainder (`_combined_context`) | ~6562 | low |
| #385 | container literal fast paths (`check_lst_expr`, `fast_dict_type`, `tuple_context_matches`) | ~5729 | medium |
| #384 | operator fallback checking (`visit_op_expr`, `check_op`, `check_op_reversible`, `lookup_operator`, `check_method_call`) | ~4112 | medium |
| #382 | generic type-argument inference driver (`infer_function_type_arguments`, pass2) | ~2644 | medium |
| #380 | `check_callable_call` arg-binding tail | ~1936 | high |
| #381 | `check_argument_types` / `check_arg` | ~3137 | high |
| #383 | overload dispatch (`check_overload_call` family) | ~3315 | high |

**Contract for every port:** Rust returns a result record or `None`; on
`None` Python falls back to the pure-Python visitor (strangler-fig
per-call gate, `_CHECKEXPR_HAS_TYPE_KERNEL and _native_checkexpr_active`).
Defer to Python on any plugin-visible, mutation-bearing (`store_type`),
message-raising, or Python-only path. Differential parity:
`TEST_NATIVE_TYPE_KERNEL=1` vs unset, full
testtypes+testcheck+testfinegrained green at `-n4` before merge.

### Parallel tracks (independent of checkexpr)

| Issue | Port | PR | State |
|-------|------|----|-------|
| #389 | dmypy_server pure helpers (`start.map`, `all.sources`, response metadata) | #398 | rebased, CI |
| #391 | semanal portable pure helpers (`rust_is_init_only`, `rust_erase_func_annotations`, `rust_get_deprecated`, `rust_get_name_repr_of_expr`) | #404 | CI |
| #394 | plugin common/functools pure helpers | - | scoped to `parse_bool` / `require_bool_literal_argument` (deadco); most live helpers are AST-walking and out of scope |
| #387 | checker narrowing remainder (identity equality, binder) | - | unblocked after PR #395 |

## Percentage path to ~30%

Rust % is measured as `Rust / (Python + Rust)` bytes from the GitHub
languages API. Each 40-60K Rust byte port moves the needle by roughly
0.5-0.7 percentage points while Python bytes stay constant. The checkexpr
family is the largest remaining contiguous Python hot path
(`checkexpr.py` is ~58KB). Completing it, plus the parallel tracks,
lands ~26-27%. The only clearly-weighted candidates that push past that:

| Module | Python bytes | Notes |
|--------|-------------|-------|
| `semanal.py` symbol resolution + scope handling | large | **#348** (biggest single lever) |
| `nodes.py` / `types.py` | 188K / 160K | explicitly out of scope (shared mutable, plugin-visible) |

`nodes.py` and `types.py` are the "widely shared mutable object graphs"
the migration plan says NOT to port first. A true 30%+ requires either
those (AST/Type reimplementation) or #348 (semanal symbol resolution
+ scope handling, the next milestone after the checkexpr family lands).

## Recommended execution order

```
Phase 1 (parallel now):
  #398 dmypy port merge (in CI)
  #404 semanal port merge (in CI)
  docs refresh (#379, this PR)
  #386 checkexpr beachhead dispatch

Phase 2 (serial checkexpr chain, each on fresh main):
  #386 -> #385 -> #384 -> #382 -> #380 -> #381 -> #383

Phase 3 (parallel, after checkexpr clears):
  #387 checker narrowing
  #394 narrow plugin-scope port (or close as low-value)
  #348 semanal symbol resolution + scope handling

Phase 4:
  Measure final Rust % from the languages API; update this doc.
```

Every port follows the same contract: Rust returns result/`None`, Python
applies mutations, gate off by default (`TEST_NATIVE_TYPE_KERNEL=1`
differential), full parity suite green before merge.
