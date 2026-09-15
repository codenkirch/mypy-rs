# Spike #1629: Pass-1-Only Solve Split Viability

## Decision: NO-GO

Pass-2 type-argument inference is the dominant pattern in the
production-like self-check corpus (79.8% of generic calls). Skipping
it would break inference for the majority of generic call sites.
A pass-1-only split is not viable as a default fast path.

## Method

Env-gated audit (`MYPY_TK_1629_AUDIT=1`) instrumented into
`ExpressionChecker.infer_function_type_arguments` (checkexpr.py:3666)
counting 10 decision points: total calls, pass-1-only vs pass-2-needed,
native vs Python pass-1, pass-2 fast path vs full re-solve, pass-1
already-solved when entering pass-2, and polymorphic fallback.

Two corpora measured:

1. Cold self-check (353 files, `-n0 --no-incremental`, production-like)
2. Testcheck subset (1859 tests, `-k "infer or generic or overload or
   lambda or callable"`, single process)

The testtypes/testinfer light suites do not exercise the full
type-checking call path (they test sub-functions in isolation), so
they produced zero IFTA events and are not reported.

## Raw Counts

### Cold self-check

| Counter | Value | % of total |
|---|---|---|
| ifta_total | 697 | 100% |
| ifta_pass1_only | 141 | 20.2% |
| ifta_pass2_needed | 556 | 79.8% |
| ifta_pass1_native | 57 | 8.2% (40.4% of pass-1-only) |
| ifta_pass1_python_fallback | 640 | 91.8% |
| ifta_pass2_entered | 556 | 79.8% |
| ifta_pass2_fast_path | 60 | 10.8% of pass-2 |
| ifta_pass2_full_resolve | 496 | 89.2% of pass-2 |
| ifta_pass2_entered_but_pass1_solved | 60 | 10.8% of pass-2 |
| ifta_poly_fallback | 18 | 2.6% |

### Testcheck subset (infer/generic/overload/lambda/callable)

| Counter | Value | % of total |
|---|---|---|
| ifta_total | 2294 | 100% |
| ifta_pass1_only | 2025 | 88.3% |
| ifta_pass2_needed | 269 | 11.7% |
| ifta_pass1_native | 0 | 0% |
| ifta_pass1_python_fallback | 2294 | 100% |
| ifta_pass2_entered | 269 | 11.7% |
| ifta_pass2_fast_path | 45 | 16.7% of pass-2 |
| ifta_pass2_full_resolve | 224 | 83.3% of pass-2 |
| ifta_pass2_entered_but_pass1_solved | 47 | 17.5% of pass-2 |
| ifta_poly_fallback | 492 | 21.4% |

## Analysis

### Why pass-2 dominates on the self-check

Pass-2 fires when `ArgInferSecondPassQuery` returns True for any formal
argument type, meaning the formal has a type variable in a callable
return type (e.g. `Callable[[T], S]` where T, S are type variables).
The self-check codebase heavily uses higher-order functions, callbacks,
and generic callables with dependent type variables — the mypy/my own
type checker is a large generic Python codebase. This makes pass-2 the
majority pattern (79.8%), not an edge case.

The testcheck subset shows the opposite (11.7% pass-2) because the
selected tests exercise a broader mix of inference patterns, many of
which are simple single-variable generics that pass-1 resolves
completely.

### Pass-2 does real work

Of the 556 pass-2 calls on the self-check, 496 (89.2%) proceed to a
full re-solve: pass-2 re-infers argument types using the pass-1
partial solution as context, then re-solves constraints. Only 60
(10.8%) hit the fast path where `not callee_type.is_generic()` after
applying pass-1 results (pass-2 adds nothing).

The `ifta_pass2_entered_but_pass1_solved` counter (60) matches the
fast-path count (60) exactly, confirming that the fast path fires
precisely when pass-1 already solved all type variables. Pass-2 is not
redundant in the remaining 89.2% — it contributes new constraints
from lambda/callable arguments whose types could only be inferred
after pass-1 resolved the outer type variables.

### Native IFTA engagement

The native IFTA seam (`_rust_infer_function_type_arguments`) only
engages when `2 not in arg_pass_nums` (line 3702). On the self-check,
57/141 pass-1-only calls (40.4%) went native; the rest deferred to
Python. On the testcheck subset, 0/2025 went native — the native seam
deferred on every call, likely because the test corpus exercises
shapes the kernel cannot yet decide (ParamSpec, Unpack, aliases).

This means the native IFTA seam currently has limited production
engagement. Widening it is a separate concern from the pass-1-only
split question.

### Polymorphic fallback

The polymorphic fallback (line 3809, `allow_polymorphic=True`) fires
after pass-2 when any inferred arg is None/Uninhabited or carries
callee type variables. It runs on 2.6% (self-check) / 21.4% (testcheck)
of calls — a separate third solve pass that is not part of this
spike's scope.

## Conclusion

A pass-1-only solve split is **not viable** as a default fast path.
Pass-2 is the dominant inference pattern on the production-like
self-check corpus (79.8% of calls), and it does real work (full
re-solve) in 89.2% of those cases. Skipping pass-2 would produce
incomplete or incorrect type inference for the majority of generic
call sites.

### Minor optimization opportunity

The 10.8% of pass-2 calls that hit the fast path (pass-1 already
solved everything) could be skipped if pass-1's result is checked
for completeness before entering pass-2. This would save the
`apply_generic_arguments` + `is_generic()` check and the function
call overhead, but it is a marginal optimization, not the structural
"pass-1-only split" the issue envisions.

### Related work

The native IFTA seam (`_rust_infer_function_type_arguments`) already
handles the pass-1-only case when `2 not in arg_pass_nums`. Widening
its engagement rate (currently 40% on self-check, 0% on testcheck)
is the more productive direction, independent of the pass-2 question.
