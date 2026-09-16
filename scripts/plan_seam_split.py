#!/usr/bin/env python3
"""Audit the per-module seam-registration split for issue #1677.

Before the split, `crates/type_kernel/src/lib.rs` carried every
`wrap_pyfunction!` registration in one place, which made it the merge point
for every parallel lane. Each defining module now owns a
`pub(crate) fn register_registry(m: &PyModule)` at the end of its own file,
and `lib.rs` only calls those functions.

This tool reports the registrations grouped by defining module and enforces
the layout invariant that matters, because a dropped registration silently
nulls an entire seam battery through the single `try: from type_kernel import
(...)` block in the Python modules:

    .venv/bin/python scripts/plan_seam_split.py
    .venv/bin/python scripts/plan_seam_split.py --verify
    .venv/bin/python scripts/plan_seam_split.py --module checker_functions
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

SRC: Path
LIB_RS: Path


def resolve_root(explicit: str | None) -> Path:
    """Locate the mypy-rs checkout this script audits.

    Explicit --root wins. Otherwise prefer the enclosing git checkout (so a
    copy of this script run from anywhere still finds the tree), falling back
    to the script's own location for non-git checkouts.
    """
    if explicit:
        return Path(explicit)
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True
        )
        return Path(out.stdout.strip())
    except (subprocess.CalledProcessError, FileNotFoundError, OSError):
        return Path(__file__).resolve().parent.parent


REGISTER_FN = re.compile(r"pub\(crate\)\s+fn\s+register_registry\s*\(", re.S)
REGISTRATION = re.compile(r"wrap_pyfunction!\(\s*(?P<fn>[A-Za-z0-9_]+)\s*,", re.S)


def split_block(text: str) -> str:
    """Return the text of the file's `register_registry` fn, or ''."""
    start = REGISTER_FN.search(text)
    if start is None:
        return ""
    brace = text.index("{", start.end())
    depth = 0
    for i in range(brace, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[brace : i + 1]
    raise ValueError("unterminated register_registry body")


def parse() -> list[tuple[str, str, str]]:
    """Return (file stem, defining module, pyfunction name) per registration.

    After the split the macro takes the bare function name, because the
    registration lives in the defining module itself; the file stem is the
    defining module.
    """
    out: list[tuple[str, str, str]] = []
    for path in sorted(SRC.glob("*.rs")):
        block = split_block(path.read_text(encoding="utf-8"))
        for m in REGISTRATION.finditer(block):
            out.append((path.stem, path.stem, m.group("fn")))
    return out


def verify(regs: list[tuple[str, str, str]]) -> list[str]:
    """Return a list of invariant violations (empty means clean)."""
    problems: list[str] = []
    if not regs:
        problems.append("no registrations found: every seam would be inert")
    lib = LIB_RS.read_text(encoding="utf-8")
    if "wrap_pyfunction!" in lib:
        problems.append("lib.rs still contains wrap_pyfunction! sites")
    for path in sorted(SRC.glob("*.rs")):
        text = path.read_text(encoding="utf-8")
        in_block = len(REGISTRATION.findall(split_block(text)))
        total = len(REGISTRATION.findall(text))
        if total != in_block:
            problems.append(f"{path.name}: {total - in_block} site(s) outside register_registry")
        if "#[pyfunction" in text and in_block == 0 and path.name != "lib.rs":
            problems.append(f"{path.name}: defines #[pyfunction]s but registers none")
    return problems


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--verify", action="store_true", help="fail on layout violations")
    ap.add_argument("--module", help="show only this defining module's functions")
    ap.add_argument("--root", help="mypy-rs checkout root (default: enclosing git checkout)")
    args = ap.parse_args()

    global SRC, LIB_RS
    root = resolve_root(args.root)
    SRC = root / "crates" / "type_kernel" / "src"
    if not SRC.is_dir():
        print(
            f"error: expected mypy-rs layout absent under {root} "
            "(no crates/type_kernel/src); run from a mypy-rs checkout "
            "or pass --root <path>",
            file=sys.stderr,
        )
        return 2
    LIB_RS = SRC / "lib.rs"

    regs = parse()
    if args.module:
        fns = [fn for _, mod, fn in regs if mod == args.module]
        print(f"{args.module}: {len(fns)} registration(s)")
        for fn in fns:
            print(f"  {fn}")
        return 0

    per_module = Counter(mod for _, mod, _ in regs)
    dupes = [fn for fn, n in Counter(fn for _, _, fn in regs).items() if n > 1]
    print(f"file:            {SRC}/*.rs")
    print(f"registered:      {len(regs)} sites across {len(per_module)} defining modules")
    print(f"unique pyfns:    {len({f for _, _, f in regs})}")
    if dupes:
        print(f"note: pyfunction names registered more than once: {sorted(dupes)}")

    if args.verify:
        problems = verify(regs)
        for p in problems:
            print(f"VERIFY: {p}", file=sys.stderr)
        return 1 if problems else 0

    print()
    print("per-module registration counts (the split units):")
    for mod, count in per_module.most_common():
        print(f"  {mod:24s} {count}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
