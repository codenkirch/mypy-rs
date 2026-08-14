# Rust Migration: Plan Ports Complete

## Current State (verified live, August 2026)

### GitHub Language Stats
- Python: 6,284,312 bytes (66.6%) *(GH API, 2026-08-14)*
- Rust: 3,156,702 bytes (33.4%) *(GH API, 2026-08-14)*
- Total: 9,441,014
- **Target: 20%+ Rust: met and sustained; 33.4% on the GitHub languages
  API (Phase D, 2026-08-14), above the 30% target (D4).**

### Native Gates Status

**Production (default-on via `native_type_kernel`):**
- `erase_type`, `remove_instance_last_known_values` (erasetype)
- `join`, `mro`, `expand_type`, `constraints`, `solve`, `join_types`,
  `meet` (active + resolver, full parity closed)
- `typeops` (active + resolver, production)
- `semanal`, `typeanal`, `erase_typevars` (active; typeanal
  ParamSpec/TypeVarTuple wire, PR #364)
- `visitor`, `checker` (active; checker narrowing seam restored, PR #395)
- `checkexpr` (plugin-hook registry + `star_expr`,
  `conditional_expr_join`, PR #365)
- `argmap`, `applytype` (active/resolver)
- `freshen`, `lkv`, `cache` (parity-tested; NOT_READY instance_cache
  poison fix, PR #399)
- `messages` (active: `format_type` family + notes helpers +
  suggestions, `callable_name`, PRs #324, #397)
- `checkmember` (active + resolver: `bind_self_fast`, operator helpers,
  method fast path; PRs #325, #332)
- `maptype` (active + resolver: `map_instance_to_supertype` hot-path shim,
  PR #427 — the plan's final module port)
- `checkpattern` (active + resolver; PRs #315, #323)
- `checkstrformat` (active; PR #297)
- `serverdeps`, `server` (get_type_triggers PR #318; fine-grained
  triggers PR #372; server update helpers PR #396)
- `dmypy_server` (Server.check, command_run, command_recheck,
  merge_hook_results; PR #374, #358)
- plugins (attrs PR #370, dataclasses PR #375/#400, plugin hook dispatch
  PR #373, stubgen collectors PR #401)
- `stubgenc` AliasPrinter parity render (PR #366)

**Subtype active:** SHIPPED (resolver + active flag in production, M18).

**Ports complete (no un-ported modules remain):**
- `infer.py` — closed without a port (#426): `infer_type_arguments` is
  2 lines of glue over the native `rust_infer_constraints_full` →
  `rust_solve_constraints` path, both already active in production.
- `maptype.py` — landed as the final module port (#425, PR #427) via the
  strangler-fig seam: `rust_map_instance_to_supertype` +
  `rust_class_derivation_paths` + `rust_map_instance_to_direct_supertypes`
  reuse the `subtypes::map_instance_to_supertype` derivation primitive and
  the shared wire codec; the `builtins.tuple` tuple_fallback edge defers
  to Python.

**Remaining work is per-issue optimization within ported modules; see
`next-big-leap-issues.md`.**

### Performance (M17 graduation, cold self-check)
- `parse_time`: 4.997s (1.0% reduction from Python)
- `semanal_time`: 1.110s (59.1% reduction)
- `type_check_time`: 2.326s (76.6% reduction)
- **Total: 8.433s (52.4% reduction)** from Python baseline of 17.713s

---

## The current active ports (checkexpr family)

The checkexpr hot path is the remaining production seam. It is ported
**serially** (all issues touch `mypy/checkexpr.py`; one checkexpr
speed-coder at a time, each rebased on fresh `main`).

| Issue | Port | Region |
|-------|------|--------|
| #386 | conditional-expression join remainder (`_combined_context`) | ~6562 |
| #385 | container literal fast paths (`check_lst_expr`, `fast_dict_type`, ...) | ~5729 |
| #384 | operator fallback checking (`visit_op_expr` helpers) | ~4112 |
| #382 | generic type-argument inference driver (`infer_function_type_arguments`) | ~2644 |
| #380 | `check_callable_call` arg-binding tail | ~1936 |
| #381 | `check_argument_types` / `check_arg` | ~3137 |
| #383 | overload dispatch (`check_overload_call`) | ~3315 |

Order is smallest-cheapest first (#386, #385), then the medium region
(#384, #382), then the high-risk binding zone (#380, #381, #383).

## Parallel tracks (independent of checkexpr)

| Track | Issue | PR | State |
|-------|-------|----|-------|
| dmypy_server pure helpers | #389 | #398 | in rebase/CI |
| semanal portable pure helpers | #391 | #404 | in CI |
| plugin common/functools pure helpers | #394 | - | scoped; low portable surface |

Each follows the same contract: Rust returns result or `None`, Python
applies mutations, parity is verified as a `TEST_NATIVE_TYPE_KERNEL=1`
differential (the kernel is default-on; the env var drives the
head-to-head comparison), and the full parity suite is green before
merge.

## Rust %

Current (2026-08-14, GitHub languages API): 3,156,702 Rust bytes /
9,441,014 total = 33.4%, above the 30% target (Phase D, D4).
