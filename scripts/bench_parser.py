"""Parser-focused microbenchmark: Python (fastparse) vs native parser.

Calls mypy.parse.parse() with eager=True directly on the real source corpus
(mypy + mypyc + bundled typeshed), bypassing type checking entirely. This
isolates parser + AST materialization throughput, the part the Rust migration
actually touches.
"""
from __future__ import annotations

import os
import sys
import time
from pathlib import Path

from mypy.errors import Errors
from mypy.options import Options
from mypy.parse import parse

REPO = Path(__file__).resolve().parent.parent


def collect_corpus() -> list[tuple[str, bytes]]:
    """Collect (path, source_bytes) for every .py and .pyi under the repo."""
    files: list[Path] = []
    for sub in ("mypy", "mypyc"):
        files.extend((REPO / sub).rglob("*.py"))
        files.extend((REPO / sub).rglob("*.pyi"))
    # typeshed stubs exercise the .pyi path heavily
    files.extend((REPO / "mypy/typeshed").rglob("*.pyi"))
    # Exclude the local extension / venv
    files = [f for f in files if "/.venv/" not in str(f)]
    corpus: list[tuple[str, bytes]] = []
    for f in files:
        try:
            src = f.read_bytes()
        except OSError:
            continue
        corpus.append((str(f), src))
    corpus.sort(key=lambda x: x[0])
    return corpus


def make_options(native: bool) -> Options:
    opts = Options()
    opts.native_parser = native
    opts.python_version = (3, 13)
    return opts


def run_once(corpus: list[tuple[str, bytes]], native: bool) -> tuple[float, int]:
    """Parse every file once. Return (elapsed_seconds, error_count)."""
    opts = make_options(native)
    errors = Errors(opts)
    t0 = time.perf_counter()
    err_count = 0
    for path, src in corpus:
        errors.set_file(path, module=None, options=opts, scope=None)
        try:
            parse(src, path, module=None, errors=errors, options=opts, eager=True)
        except Exception as e:
            err_count += 1
            # Don't spam; just count
            if err_count <= 3:
                print(f"  [{ 'native' if native else 'python' }] {path}: {type(e).__name__}: {e}",
                      file=sys.stderr)
    elapsed = time.perf_counter() - t0
    return elapsed, err_count


def main() -> None:
    corpus = collect_corpus()
    total_bytes = sum(len(s) for _, s in corpus)
    print(f"Corpus: {len(corpus)} files, {total_bytes / 1024:.0f} KiB")
    print()

    # Warm both parsers once (imports, caches, etc.)
    run_once(corpus[:20], native=False)
    run_once(corpus[:20], native=True)

    py_times: list[float] = []
    nat_times: list[float] = []
    iterations = 3
    for i in range(iterations):
        pt, _ = run_once(corpus, native=False)
        nt, _ = run_once(corpus, native=True)
        py_times.append(pt)
        nat_times.append(nt)
        print(f"  iter {i+1}: python={pt:.3f}s  native={nt:.3f}s")

    best_py = min(py_times)
    best_nat = min(nat_times)
    print()
    print(f"Python parser best-of-{iterations}: {best_py:.3f}s")
    print(f"Native parser best-of-{iterations}: {best_nat:.3f}s")
    delta = best_nat - best_py
    pct = (delta / best_py) * 100
    if delta < 0:
        print(f"Native is {-pct:.1f}% faster ({-delta:.3f}s saved)")
    elif delta > 0:
        print(f"Native is {pct:.1f}% slower ({delta:.3f}s added)")
    else:
        print("Native and Python are identical")
    print(f"Throughput (python): {total_bytes / 1024 / best_py:.0f} KiB/s")
    print(f"Throughput (native): {total_bytes / 1024 / best_nat:.0f} KiB/s")


if __name__ == "__main__":
    main()
