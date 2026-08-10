# Remaining Milestones → Final Two Ports

## Current State (verified live, August 2026)

### GitHub Language Stats
- Python: 5,993,751 bytes (75.83%)
- Rust: 1,910,231 bytes (24.17%)
- Total: 7,903,982
- **Target: 20%+ Rust: met and sustained; now 24.17%**

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

**Not ported (no Rust counterpart):**
| Python Module | Lines | Role |
|---------------|-------|------|
| `infer.py` | 80 | Type inference for anonymous code |
| `maptype.py` | 109 | Type mapping utilities |

**Remaining work is tracked per-issue; see `next-big-leap-issues.md`.**

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
applies mutations, gate is off by default (`TEST_NATIVE_TYPE_KERNEL=1`
differential), full parity suite green before merge.

## Sustaining 20%+ Rust

Current: 1,910,231 Rust bytes / 7,903,982 total = 24.17%. The 20%
target rides on the existing ~60K Rust LOC. The checkexpr family and the
parallel tracks push toward 30% as measured by the GitHub languages API.
