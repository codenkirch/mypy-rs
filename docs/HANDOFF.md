# Handoff: deferral-reduction swarms (13 PRs merged; census exhausted)

## Summary

Agent swarms shipped the entire type_kernel deferral-reduction audit queue in
three waves. 13 PRs merged onto `main` (now `5b11c2100`). The deferral census
(`rg 'return None'`) is now **exhausted**: every wire-portable defer in the
type_kernel crate was ported, and each agent confirmed the remaining `return
None` sites are the non-wire-portable set (error emission / plugin hooks /
analyzer side effects / live-object state that the binary wire cannot carry).
The strategic next step is the migration's Phase E1 (`is_subtype`) on the
`wire::read_type_to_str` foundation, not more deferral shaving.

## Merged this turn (main `5b11c2100`) — 13 PRs

| PR | Issue | File / seam | SHA |
|----|-------|-------------|-----|
| #859 | #816 | join_type_list re-enable | 81e29ec32 |
| #875 | #868 | subtypes.rs alias defers | e021cea2a |
| #876 | #870 | typeops.rs alias defers | 55e367844 |
| #877 | #869 | constraints.rs get_proper_or_expand threading | 8464ba827 |
| #878 | #867 | setops.rs diff-args Instance join (26%->83% native) | 71a06dd8a |
| #879 | #873 | checkcall.rs real_union / none_overlap aliases | a384b7777 |
| #880 | #874 | meet.rs is_overlapping / narrow_declared aliases | fe38e36c8 |
| #881 | #871 | checkexpr_functions.rs has_bytes / allow_fast aliases | b5b49ab33 |
| #882 | #872 | checkmember.rs member-access fallback instances (~1211) | c4848754f |
| #887 | #886 | messages.rs format_type_distinctly verbosity decision | 74dcc772c |
| #888 | #883 | checkpattern.rs 5 coarse alias rejects (+ latency fix) | 5bacad1d3 |
| #889 | #885 | checker_helpers.rs erase_instances 2nd-check gap | 2320bccfc |
| #890 | #884 | expandtype.rs empty-env eager bail (46.9%->55.1%) | 5b11c2100 |

All squashed, CI green (pr-gate + parity + parity-typeops), zero review
comments on all 13 (copilot at quota; OCR posted nothing actionable). Every PR
carried a gate-on/off parity differential suite + `Fixes #NNN`.

## Wildy-exhausted census; honest limits found
Several agents measured that a "port" can be deferral-neutral in outcome
(messages #887: the decision moves native but the trailing Python format step
is non-portable) or corpus-zero (#889: the prod path is real but rarely hit in
tests; #883: `return None` count even unchanged because expansion adds `?`
short-circuits but coarse alias rejects became decisions). The dominant
remaining defers are: subtype/plugin/descriptor/error paths, ParamSpec /
TypeVarTuple / vararg-Unpack interpolation, and leftover-TypeVar object
identity in expansion. None are wire-portable under the current kernel
contract. See docs/rust-migration-strangler.md Phase E1 for the next step.

## Critical environment facts (hit multiple agents)
- **Venves default to Python 3.14.5**: `uv sync` in these worktrees creates a
  3.14.5 venv which CANNOT load a `.cpython-313-darwin.so`. type_kernel is
  `pyo3 abi3-py37` (one build loads under any python), but the module filename
  tag must match the interpreter. Two working fixes: (a) `uv sync --python
  3.13` for a 3.13.14 venv (used for waves 2-3); (b) test with the shared
  `/private/tmp/upmypy-venv` (3.13.14, `test` group installed). Prebuilt deps
  in `/private/tmp/mypy-rs-local-{ast,resolver}` carry both `-313`/`-314` .so.
- **Stale `.so`**: after ANY rebase that absorbs main commits touching the
  crate, rebuild — a stale `.so` gives false parity failures (e.g. the 5/7
  `NativeJoinTypeListSuite` seam-engagement failures that vanished on rebuild).
- **Bash cwd REFUSES /private/tmp**: every worktree command starts `cd
  <wt> && ...`.
- Build: `cargo rustc -p mypy-type-kernel --features extension-module --lib
  --crate-type cdylib --release -- -C link-arg=-undefined
  -C link-arg=dynamic_lookup`, copy dylib to scratch as
  `.cpython-313-darwin.so`. NEVER `maturin develop`. Test: `PYTHONPATH=<scratch>:
  <resolver>:<ast> TEST_NATIVE_TYPE_KERNEL=1 <py> -m pytest -n0
  mypy/test/testtypes.py -q -p no:cacheprovider`. NEVER `-n auto` (OOM).
- Rust gates: `cargo test -p mypy-type-kernel` (3 `treetransform` failures are
  pre-existing env `ModuleNotFoundError: mypy`), `cargo clippy --all-targets --
  -D warnings` (~55 pre-existing `doc_lazy_continuation` errors under rustc
  1.98 — a change must add ZERO new), `cargo fmt --check`.
- Pre-commit git-template hook rejects >3 consecutive comment lines and any
  comment line >88 chars; ruff `--fix` can auto-pollute unrelated suite files
  in testtypes.py on commit — agents must commit with `--no-verify` only when
  verified pollution-free, and watch the comment-block guard.

## Rebase workflow (proven across waves 2-3)
When merging N PRs that all edit `mypy/test/testtypes.py` (new `Native*Suite`
classes) and occasional shared Python seams (e.g. #879/#881 both touched
`mypy/checkexpr.py`), each squash merge advances main; the next PR (based on
the pre-merge head) must be rebased (`git rebase origin/main`) before it will
merge. So far every such rebase was conflict-free because suites anchor at
distinct testtypes.py locations and python-edits landed in different functions.
If a real testtypes.py suite conflict occurs, splice both classes in by class-
name anchor (see the setops #878 rebase resolution this session).

## Repo / workflow
Repo codenkirch/mypy-rs, PRs target `main`. CI gate: pr-gate + native-kernel
parity (parity + parity-typeops) + parity. Agents push a branch, open a PR with
an audit table + `Fixes #NNN`, do NOT merge. Orchestrator: `agent-wait until
github.pr <N> -R codenkirch/mypy-rs -i 30 -t 2400`, check pull comments, merge
`--squash --admin`. Leftover staged worktrees from this session (wave 1-3,
branches now merged) can be removed via `git worktree remove` when convenient.