"""Resolver-focused microbenchmark: Python resolver vs native resolver.

Calls FindModuleCache.find_module() directly on every import that appears in
the real source corpus (mypy + mypyc + bundled typeshed), bypassing type
checking entirely. This isolates module-resolution throughput, the part the
Rust migration actually touches.
"""
from __future__ import annotations

import ast
import os
import sys
import time
from pathlib import Path

from mypy.modulefinder import FindModuleCache, compute_search_paths
from mypy.fscache import FileSystemCache
from mypy.options import Options

REPO = Path(__file__).resolve().parent.parent


def collect_imports() -> set[str]:
    """Walk the corpus and collect every imported module name."""
    imports: set[str] = set()
    files: list[Path] = []
    for sub in ("mypy", "mypyc"):
        files.extend((REPO / sub).rglob("*.py"))
    files = [f for f in files if "/.venv/" not in str(f)]
    for f in files:
        try:
            tree = ast.parse(f.read_bytes())
        except SyntaxError:
            continue
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    imports.add(alias.name.split(".")[0])
            elif isinstance(node, ast.ImportFrom):
                if node.module and node.level == 0:
                    imports.add(node.module.split(".")[0])
    # Always include the core modules the resolver is asked about during a
    # real build, so the corpus isn't artificially tiny.
    imports.update({"builtins", "typing", "abc", "collections", "sys", "os", "io"})
    return imports


def make_cache(native: bool) -> FindModuleCache:
    opts = Options()
    opts.python_version = (3, 13)
    opts.native_resolver = native
    fscache = FileSystemCache()
    # Build search paths the way build.py does: typeshed + cwd.
    data_dir = str(REPO / "mypy")
    sources = []
    search_paths = compute_search_paths(sources, opts, data_dir, alt_lib_path=None)
    return FindModuleCache(search_paths, fscache=fscache, options=opts)


def run_once(imports: list[str], native: bool) -> float:
    cache = make_cache(native)
    t0 = time.perf_counter()
    for mod in imports:
        cache.find_module(mod, fast_path=True)
    return time.perf_counter() - t0


def main() -> None:
    imports = sorted(collect_imports())
    print(f"Corpus: {len(imports)} unique top-level imports")
    print()

    # Warm both resolvers once.
    run_once(imports[:20], native=False)
    run_once(imports[:20], native=True)

    py_times: list[float] = []
    nat_times: list[float] = []
    iterations = 5
    for i in range(iterations):
        pt = run_once(imports, native=False)
        nt = run_once(imports, native=True)
        py_times.append(pt)
        nat_times.append(nt)
        print(f"  iter {i+1}: python={pt:.4f}s  native={nt:.4f}s")

    best_py = min(py_times)
    best_nat = min(nat_times)
    print()
    print(f"Python resolver best-of-{iterations}: {best_py:.4f}s")
    print(f"Native resolver best-of-{iterations}: {best_nat:.4f}s")
    delta = best_nat - best_py
    pct = (delta / best_py) * 100
    if delta < 0:
        print(f"Native is {-pct:.1f}% faster ({-delta:.4f}s saved)")
    elif delta > 0:
        print(f"Native is {pct:.1f}% slower ({delta:.4f}s added)")
    else:
        print("Native and Python are identical")
    print(f"Per-module (python): {best_py / len(imports) * 1e6:.1f} µs")
    print(f"Per-module (native): {best_nat / len(imports) * 1e6:.1f} µs")


if __name__ == "__main__":
    main()
