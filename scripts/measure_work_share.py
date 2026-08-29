#!/usr/bin/env python3
"""Measure the native work share: how much of the self-check corpus's
type-checking work executes in Rust.

The representative migration metric. A strangler-fig port keeps every
Python fallback, so Python byte share never shrinks and byte metrics
understate progress. This script measures the runtime differential
between the pure-Python path and the default-on native path on the cold
self-check, and reports the Rust-absorbed share per phase.

Usage:
    python3 scripts/measure_work_share.py [--pairs N] [--python-only] [--python-cmd PY]

Runs the self-check with mypy_self_check.ini -p mypy -p mypyc twice
per round (cold, --no-incremental, --dump-build-stats, single worker
-n0): once with the kernel off (--no-native-type-kernel) and once with
the native path (prod default). -n0 is required: with parallel workers
each worker dumps its own Stats: block and the per-phase rows never
aggregate back to the driver, so the differential would read garbage.
Each round runs native then python, and the round order makes the
pair's load window as close as possible. The report takes the median
of N rounds per phase. A sanity gate rejects a round where parse_time
differs by more than 20% between the two modes: parsing is the same
code path in both, so a larger delta means the round was polluted by
background load. (M17 baseline and this script agree on reporting a
single serial build.) Parses the first
Stats: block and prints a share table like:

    Phase                  python       native       Rust share
    parse_time             5.046s       4.997s         1.0%
    ...

The share is (python - native) / python per phase. Total is the sum of
the three kernel-relevant phases (parse + semanal + type-check impl),
matching the M17 baseline in docs/remaining-migration-plan.md.

The self-check itself reports pre-existing diagnostics (exit code
nonzero); the script ignores the exit code and succeeds as long as the
Stats: block parses.

Exit code 0 on success; nonzero if either run crashes or no stats
appear.
"""

import argparse
import os
import re
import subprocess
import sys

STAT_ROWS = ("parse_time", "semanal_time", "type_check_time")

STAT_RE = re.compile(r"^(\w+):\s+([\d.]+)\s*$", re.MULTILINE)


def run_self_check(python_cmd: list[str], native: bool, cwd: str) -> dict[str, float]:
    """Run the cold self-check; return the build-stats rows."""
    cmd = python_cmd + [
        "-m",
        "mypy",
        "--config-file",
        "mypy_self_check.ini",
        # Serial: parallel workers each dump their own Stats: block, and
        # per-phase rows never reach the driver. Force n0 above the ini.
        "-n0",
        "--no-incremental",
        "--dump-build-stats",
        "-p",
        "mypy",
        "-p",
        "mypyc",
    ]
    if not native:
        cmd.append("--no-native-type-kernel")
    env = dict(os.environ)
    env.pop("TEST_NATIVE_TYPE_KERNEL", None)
    print(f"=== running self-check {'native' if native else 'python-only'} ===")
    proc = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True)
    stats: dict[str, float] = {}
    combined = proc.stdout + proc.stderr
    # The driver emits a single Stats: block. Parse only the first one so
    # that interleaved worker blocks (parallel mode) or other stdout can
    # never bleed a second set of values into the result.
    chunks = combined.split("Stats:")
    if len(chunks) >= 2:
        for name, value in STAT_RE.findall(chunks[1]):
            if name in STAT_ROWS:
                stats[name] = float(value)
    if not stats:
        print(proc.stderr[-2000:], file=sys.stderr)
        sys.exit(f"self-check ({'native' if native else 'python-only'}) produced no stats")
    return stats


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pairs",
        type=int,
        default=5,
        help="number of native+python round pairs (default: 5; median of pairwise ratios)",
    )
    parser.add_argument(
        "--python-only",
        action="store_true",
        help="only run the python-only self-check (skip the native run)",
    )
    parser.add_argument(
        "--python-cmd",
        default=None,
        help="python interpreter to run mypy with (default: sys.executable)",
    )
    args = parser.parse_args()
    if args.pairs < 1:
        sys.exit("--pairs must be >= 1")

    cwd = os.getcwd()
    python_cmd = [args.python_cmd or sys.executable]
    if not os.path.exists(os.path.join(cwd, "mypy_self_check.ini")):
        sys.exit("run from the repo root (mypy_self_check.ini not found)")

    native_rounds: list[dict[str, float]] = []
    python_rounds: list[dict[str, float]] = []
    rounds = 0
    while rounds < args.pairs:
        if not args.python_only:
            native_rounds.append(run_self_check(python_cmd, native=True, cwd=cwd))
        python_rounds.append(run_self_check(python_cmd, native=False, cwd=cwd))
        # Sanity gate: parsing is identical code in both modes. If the
        # pair's parse_time moves more than 20%, background load polluted
        # this round; drop it and collect another.
        if not args.python_only:
            pv = python_rounds[-1].get("parse_time", 0.0)
            nv = native_rounds[-1].get("parse_time", 0.0)
            if pv and nv:
                drift = abs(pv - nv) / pv
                if drift > 0.20:
                    print(f"round {rounds + 1}: parse drift {drift:.0%} > 20%, skipping (load)")
                    native_rounds.pop()
                    python_rounds.pop()
                    continue
        rounds += 1

    def med(values: list[float]) -> float:
        ordered = sorted(values)
        return ordered[len(ordered) // 2]

    def key_median(rounds_list: list[dict[str, float]], key: str) -> float:
        return med([r.get(key, 0.0) for r in rounds_list])

    def total_of(stats: dict[str, float]) -> float:
        return sum(stats.get(k, 0.0) for k in STAT_ROWS)

    # Median of per-round ratios. Absolute times are load-dependent on a
    # shared machine; the ratio (native vs python in a back-to-back pair)
    # is robust because both modes compete for the same CPU window.
    pairs = list(zip(native_rounds, python_rounds))

    def pair_share(key: str, i: int) -> float:
        pv = pairs[i][1].get(key, 0.0)
        nv = pairs[i][0].get(key, 0.0)
        return (pv - nv) / pv * 100 if pv else 0.0

    def key_share(key: str) -> float:
        return med([pair_share(key, i) for i in range(len(pairs))])

    def total_share() -> float:
        # Total share = same median-of-pairwise-ratios formula on summed rows.
        ratios = []
        for i in range(len(pairs)):
            pv = total_of(pairs[i][1])
            nv = total_of(pairs[i][0])
            ratios.append((pv - nv) / pv * 100 if pv else 0.0)
        return med(ratios)

    # Surface the median absolute seconds so the table stays readable;
    # the reported shares are the median-of-pairwise-ratios values.
    native_stats = {k: key_median(native_rounds, k) for k in STAT_ROWS}
    python_stats = {k: key_median(python_rounds, k) for k in STAT_ROWS}
    native_total = total_of(native_stats) if native_stats else 0.0
    python_total = total_of(python_stats)

    print("\nPhase                  python       native       Rust share (median of ratios)")
    for name in STAT_ROWS:
        pv = python_stats.get(name, 0.0)
        nv = native_stats.get(name, 0.0)
        share = key_share(name)
        print(f"{name:<22} {pv:8.3f}s  {nv:8.3f}s  {share:6.1f}%")
    print("-" * 78)
    print(f"{'total':<22} {python_total:8.3f}s  {native_total:8.3f}s  {total_share():6.1f}%")

    return 0


if __name__ == "__main__":
    sys.exit(main())
