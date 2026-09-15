#!/usr/bin/env python3
"""Plan the per-module seam-registration split for issue #1677.

`crates/type_kernel/src/lib.rs` carries every `wrap_pyfunction!` registration in
one place, which makes it the merge point for every parallel lane. This tool
groups those registrations by their *defining module* and reports the split, so
the actual move becomes a mechanical transform with a checkable invariant
rather than a hand edit of ~961 sites, 762 of which span multiple lines.

Dry run by default: it prints the plan and verifies counts. `--verify` exits
non-zero if the parsed registration count or the function-name set disagrees
with the file, which is the invariant that matters, because a dropped
`#[pyfunction]` silently nulls an entire seam battery through the single
`try: from type_kernel import (...)` block in the Python modules.

    .venv/bin/python scripts/plan_seam_split.py
    .venv/bin/python scripts/plan_seam_split.py --verify
"""

from __future__ import annotations

import argparse
import re
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LIB_RS = REPO / "crates" / "type_kernel" / "src" / "lib.rs"

REGISTRATION = re.compile(
    r"(?P<recv>[A-Za-z_][A-Za-z0-9_]*)\.add_function\(\s*"
    r"wrap_pyfunction!\(\s*(?P<path>[A-Za-z_][A-Za-z0-9_:]*?)::(?P<fn>[A-Za-z0-9_]+)\s*,",
    re.S,
)


def parse(path: Path) -> list[tuple[str, str, str]]:
    """Return (receiver, defining module, pyfunction name) per registration.

    A path is `mod::fn` or `crate::mod::fn`, so the module is the segment
    immediately before the function. Taking the first segment instead
    mis-attributes every `crate::`-qualified registration while still counting
    the right number of sites, which is why --verify also checks the shape of
    the parsed names and not just the total.
    """
    out: list[tuple[str, str, str]] = []
    for m in REGISTRATION.finditer(path.read_text(encoding="utf-8")):
        segments = m.group("path").split("::")
        out.append((m.group("recv"), segments[-1], m.group("fn")))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--verify", action="store_true", help="fail on count/name mismatch")
    ap.add_argument("--module", help="show only this defining module's functions")
    args = ap.parse_args()

    text = LIB_RS.read_text(encoding="utf-8")
    raw_sites = len(re.findall(r"wrap_pyfunction!", text))
    declared_mods = set(re.findall(r"^\s*(?:pub )?mod ([a-z_][a-z0-9_]*)", text, re.M))
    found = parse(LIB_RS)

    by_module: dict[str, list[str]] = {}
    for _, module, fn in found:
        by_module.setdefault(module, []).append(fn)

    receivers = Counter(recv for recv, _, _ in found)
    names = sorted({fn for _, _, fn in found})
    duplicated = sorted(fn for fn, n in Counter(fn for _, _, fn in found).items() if n > 1)

    print(f"file:            {LIB_RS.relative_to(REPO)}")
    print(f"raw wrap! sites: {raw_sites}")
    print(f"parsed:          {len(found)} across {len(by_module)} defining modules")
    print(f"unique pyfns:    {len(names)}")
    print(f"receivers:       {dict(receivers)}")

    print("\nper-module registration counts (the split units):")
    for module, fns in sorted(by_module.items(), key=lambda kv: (-len(kv[1]), kv[0])):
        print(f"  {module:22s} {len(fns):4d}")

    if args.module:
        print(f"\n{args.module}:")
        for fn in sorted(by_module.get(args.module, [])):
            print(f"  {fn}")

    print("\nproposed transform per module:")
    print("  pub fn register_registry(m: &PyModule) -> PyResult<()> { ... m.add_function... }")
    print("  lib.rs keeps the mod decls and calls each register_registry once")

    ok = True
    if raw_sites != len(found):
        print(
            f"\nFAIL raw sites ({raw_sites}) != parsed ({len(found)}); "
            "the transform would drop registrations",
            file=sys.stderr,
        )
        ok = False
    bad_modules = sorted(set(by_module) - declared_mods)
    if bad_modules:
        print(
            f"\nFAIL modules not declared as `mod` in lib.rs: {bad_modules[:8]}",
            file=sys.stderr,
        )
        ok = False
    names_as_modules = sorted(set(names) & declared_mods)
    if names_as_modules:
        print(
            f"\nFAIL {len(names_as_modules)} parsed 'function' names are module names "
            f"({names_as_modules[:5]}); the function capture grabbed a path segment",
            file=sys.stderr,
        )
        ok = False
    bad_shapes = sorted(fn for fn in names if not re.fullmatch(r"[a-z][a-z0-9_]*", fn))
    if bad_shapes:
        print(f"\nFAIL malformed function names: {bad_shapes[:5]}", file=sys.stderr)
        ok = False
    if duplicated:
        print(f"\nnote: pyfunction names registered more than once: {duplicated}", file=sys.stderr)
    if args.verify and not ok:
        return 1
    if args.verify:
        print(
            f"\nVERIFY OK: {len(found)} sites parsed, all modules declared, "
            "all names well formed"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())