"""Resolver-focused microbenchmark: Python resolver vs native resolver.

Calls FindModuleCache.find_module() directly on every import that appears in
the real source corpus (mypy + mypyc + bundled typeshed), bypassing type
checking entirely. This isolates module-resolution throughput, the part the
Rust migration actually touches.

Three columns:
  * python       — per-call find_module on the Python resolver.
  * native       — per-call find_module routing through the Rust resolver
                   (one PyO3 boundary crossing per module id).
  * native batch — one resolve_many call resolving all ids in a single PyO3
                   boundary crossing, proving whether hoisting the per-file
                   import set into one Rust call closes the per-module
                   boundary-overhead gap.
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
from mypy.native_resolve import make_resolver, resolve_modules

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


def run_once_batched(imports: list[str], resolver) -> float:
    """One resolve_many call for the whole import set.

    The resolver is built once and reused across iterations (mirroring how
    FindModuleCache holds a long-lived NativeResolver). The bench corpus has
    no per-module `# mypy: follow-untyped-imports` overrides, so a uniform
    False is correct.
    """
    ids_with_follow = [(mod, False) for mod in imports]
    t0 = time.perf_counter()
    resolve_modules(resolver, ids_with_follow)
    return time.perf_counter() - t0


def main() -> None:
    imports = sorted(collect_imports())
    print(f"Corpus: {len(imports)} unique top-level imports")
    print()

    # Build one long-lived native resolver for the batched column, mirroring how
    # FindModuleCache._ensure_native_resolver constructs it once and reuses it.
    batch_cache = make_cache(native=True)
    batch_cache._ensure_native_resolver()
    batch_resolver = batch_cache._native_resolver

    # Warm all three paths once.
    run_once(imports[:20], native=False)
    run_once(imports[:20], native=True)
    run_once_batched(imports[:20], batch_resolver)

    py_times: list[float] = []
    nat_times: list[float] = []
    bat_times: list[float] = []
    iterations = 5
    for i in range(iterations):
        pt = run_once(imports, native=False)
        nt = run_once(imports, native=True)
        bt = run_once_batched(imports, batch_resolver)
        py_times.append(pt)
        nat_times.append(nt)
        bat_times.append(bt)
        print(f"  iter {i+1}: python={pt:.4f}s  native={nt:.4f}s  batched={bt:.4f}s")

    best_py = min(py_times)
    best_nat = min(nat_times)
    best_bat = min(bat_times)
    print()
    print(f"Python resolver       best-of-{iterations}: {best_py:.4f}s")
    print(f"Native resolver       best-of-{iterations}: {best_nat:.4f}s")
    print(f"Native (batched)      best-of-{iterations}: {best_bat:.4f}s")
    print()
    print(f"Per-module (python):       {best_py / len(imports) * 1e6:.1f} µs")
    print(f"Per-module (native):        {best_nat / len(imports) * 1e6:.1f} µs")
    print(f"Per-module (native batch):  {best_bat / len(imports) * 1e6:.1f} µs")
    print()
    for label, best in (("native", best_nat), ("native batch", best_bat)):
        delta = best - best_py
        pct = (delta / best_py) * 100
        if delta < 0:
            print(f"{label:12} is {-pct:.1f}% faster than python ({-delta:.4f}s saved)")
        elif delta > 0:
            print(f"{label:12} is {pct:.1f}% slower than python ({delta:.4f}s added)")
        else:
            print(f"{label:12} identical to python")


if __name__ == "__main__":
    main()

