"""Measure the def-family store's cache-loaded provenance (#1825).

The evidence generator for the G2.4 slice. Three run shapes over the same
tiny package, in one process, so the counters can be read per build phase
without a wall-clock leg:

1. cold, from source: every record has the parse origin;
2. warm, fixed format (`fixed_format_cache=True`, the default): the
   dependencies come out of the binary cache, which is where the load-time
   seed must fire;
3. warm, JSON (`fixed_format_cache=False`): the same over the JSON cache
   path, whose reader is `SymbolNode.deserialize`.

For each phase it reports the provenance split (origin x mechanism), the
seed's counts, the live entry count, and - for every symbol node of every
cache-skeleton module - how many tracked slots its record holds against how
many the live node has. That last number is the per-field absence measure:
a cache-loaded node left under-covered is what the slice exists to remove.

`--seed-off` mutates the mechanism in process (the wrapper is replaced by a
no-op, the call sites stay) and re-runs the phases: that is the
structural-zero control, and it is the strongest available form of "the
same leg at the implementation base". A tree without `nodes_mirror.seed_loaded`
reports `seed mechanism: ABSENT` and the same zeros, which is the base
reading where the counters do not exist at all.

A warm phase that materialized no cache-skeleton module is reported as a
VOID READING and fails the run: its numbers would otherwise read as "the
cache path seeded nothing" when the cache was never read at all.

Run it with the kernel and the two other extensions on PYTHONPATH:

    PYTHONPATH=/private/tmp/1825-ast:/private/tmp/1825-res:/private/tmp/1825-tk \\
        .venv/bin/python misc/def_cache_seed_probe.py [--seed-off]
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile
from typing import Any

from mypy import build, nodes_mirror
from mypy.modulefinder import BuildSource
from mypy.nodes import MypyFile, SymbolTableNode
from mypy.options import Options

_SOURCES: list[tuple[str, str, str]] = [
    ("pkg/__init__.py", "pkg", ""),
    (
        "pkg/base.py",
        "pkg.base",
        "from typing import overload\n"
        "\n"
        "class Base:\n"
        "    def m(self) -> int: ...\n"
        "\n"
        "@overload\n"
        "def overloaded(x: int) -> int: ...\n"
        "@overload\n"
        "def overloaded(x: str) -> str: ...\n"
        "def overloaded(x): ...\n",
    ),
    (
        "pkg/use.py",
        "pkg.use",
        "from pkg.base import Base, overloaded\n"
        "\n"
        "def deco(f): return f\n"
        "\n"
        "@deco\n"
        "def wrapped(x: int) -> int: return x\n"
        "\n"
        "def use(b: Base) -> int: return overloaded(1)\n",
    ),
    ("main.py", "__main__", "import pkg.use\n"),
]

_ORIGINS = ("parse", "cache_fixed", "cache_json")


def _options(cache_dir: str, *, fixed_format_cache: bool) -> Options:
    options = Options()
    options.incremental = True
    options.cache_dir = cache_dir
    options.fixed_format_cache = fixed_format_cache
    options.allow_empty_bodies = True
    # The AST-mirror store is default-off, so a build has to opt in; the
    # audit flag is what makes the provenance counters readable at all.
    options.native_ast_mirror = True
    os.environ["MYPY_TK_AST_MIRROR_AUDIT"] = "1"
    return options


def _provenance(report: dict[str, int]) -> dict[str, int]:
    keys = [k for k in report if k.startswith(("meta_written.", "meta_seeded."))]
    return {k: report[k] for k in sorted(keys)}


def _seed_counts(report: dict[str, int]) -> dict[str, int]:
    keys = [k for k in report if k.startswith(("meta_seed_loaded", "meta_seed_"))]
    return {k: report[k] for k in sorted(keys)}


def _coverage(modules: dict[str, MypyFile]) -> dict[str, Any]:
    """Tracked versus recorded slots of every cache-loaded def node.

    Walks the symbol table of each cache-skeleton module and materializes
    each node (the deferred `SymbolTableNode.node` read is the same path the
    compiler takes), then compares the record against the tracked set.
    Slots the live node does not have are not absences: they are the
    store's "not recorded" state by contract.
    """
    kernel = nodes_mirror._kernel_mod
    skeleton_modules = 0
    nodes_seen = 0
    per_class: dict[str, list[int]] = {}
    unrecorded: dict[str, list[str]] = {}
    for module_id, tree in sorted(modules.items()):
        if not getattr(tree, "is_cache_skeleton", False):
            continue
        skeleton_modules += 1
        for name, entry in tree.names.items():
            if not isinstance(entry, SymbolTableNode):
                continue
            node = entry.node
            if node is None:
                continue
            tracked = nodes_mirror._meta_tracked(type(node))
            if not tracked:
                continue
            # The identity registry's own lookup, not the id()-keyed Python
            # map: this walk materializes nodes it has not seen before, and a
            # recycled id() would hand back another node's record.
            handle = kernel.rust_node_mirror_handle_of(node)
            record = kernel.rust_node_mirror_meta(handle) if handle is not None else None
            present = {f for f in tracked if hasattr(node, f)}
            label = type(node).__name__
            counts = per_class.setdefault(label, [0, 0, 0])
            counts[0] += 1
            counts[1] += len(present)
            counts[2] += len(record or {})
            nodes_seen += 1
            missing = sorted(f for f in present if f not in (record or {}))
            if missing:
                unrecorded.setdefault(label, []).extend(missing)
    return {
        "skeleton_modules": skeleton_modules,
        "def_nodes": nodes_seen,
        "per_class": {k: tuple(v) for k, v in sorted(per_class.items())},
        "unrecorded": {k: sorted(set(v)) for k, v in sorted(unrecorded.items())},
    }


def _run_phase(
    root: str, cache_dir: str, *, fixed_format_cache: bool, touch: bool, label: str
) -> bool:
    """Build one phase and print its reading; False when the reading is void.

    A warm phase that materialized no cache-skeleton module did not read
    the cache at all, so its provenance and coverage numbers would read as
    "the cache path seeded nothing" while the cache path was never walked.
    That shape is reachable - an unarmed store or a cache directory the
    build could not reuse reproduces it - so it is a failure, not a zero.
    """
    if touch:
        with open(os.path.join(root, "main.py"), "a", encoding="utf8") as handle:
            handle.write("# run 2\n")
    sources = [
        BuildSource(os.path.join(root, rel), module_id, text if not touch else None)
        for rel, module_id, text in _SOURCES
    ]
    nodes_mirror.reset(clear_counts=True)
    before = nodes_mirror.report()
    result = build.build(
        sources=sources, options=_options(cache_dir, fixed_format_cache=fixed_format_cache)
    )
    after = nodes_mirror.report()
    delta = {k: v - before.get(k, 0) for k, v in after.items() if v != before.get(k, 0)}
    report = result.manager.modules
    coverage = _coverage(report)
    print(f"--- {label} ---")
    print(f"errors: {len(result.errors)}")
    print(f"modules: {len(report)}  cache-skeleton modules: {coverage['skeleton_modules']}")
    print(f"provenance: {_provenance(delta)}")
    print(f"seed counts: {_seed_counts(delta)}")
    print(f"live entries: {nodes_mirror._kernel_mod.rust_node_mirror_meta_entry_count()}")
    print(f"cache-loaded def nodes seen: {coverage['def_nodes']}")
    for cls, (nodes, present, recorded) in coverage["per_class"].items():
        print(f"  {cls}: {nodes} nodes, tracked slots present {present}, recorded {recorded}")
    if coverage["unrecorded"]:
        print(f"  UNRECORDED: {coverage['unrecorded']}")
    else:
        print("  UNRECORDED: none")
    print()
    if touch and coverage["skeleton_modules"] == 0:
        print(
            f"VOID READING: {label} is a warm phase but no module was loaded from the "
            "cache, so no number above describes the cache path",
            file=sys.stderr,
        )
        return False
    return True


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--seed-off",
        action="store_true",
        help="replace the seed wrapper with a no-op (the control arm)",
    )
    args = parser.parse_args(argv[1:])

    present = hasattr(nodes_mirror, "seed_loaded")
    print(f"tree: {nodes_mirror.__file__}")
    print(f"seed mechanism: {'present' if present else 'ABSENT (implementation base)'}")
    if args.seed_off and present:
        nodes_mirror.seed_loaded = lambda node: 0
        print("control: seed_off (the wrapper is a no-op, the call sites stay)")
    print()

    with tempfile.TemporaryDirectory() as root:
        for rel, _module_id, text in _SOURCES:
            path = os.path.join(root, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf8") as handle:
                handle.write(text)
        void: list[str] = []
        for label, fixed in (("fixed format", True), ("JSON", False)):
            cache_dir = os.path.join(root, f".cache-{'ff' if fixed else 'json'}")
            for touch, phase in ((False, "cold"), (True, "warm")):
                name = f"{phase} ({label})"
                if not _run_phase(
                    root, cache_dir, fixed_format_cache=fixed, touch=touch, label=name
                ):
                    void.append(name)
    if void:
        print(
            "void phases (a number above does not describe the cache path): " + ", ".join(void),
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
