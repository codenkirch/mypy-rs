"""Aggregate the symtable read-flip session dumps of one CI step (#1765).

Each build under `MYPY_TK_SYMTABLE_SESSIONFINISH_OUT` writes a
`sessionfinish` dump: a `{pid}`-expanded JSON file per process, holding the
process-lifetime read-flip counters (`flip`) and the consumer counters
(`astdiff`). This script is the engagement gate of the step that runs the
flip. It exits non-zero when the flip served no table, so a step that
passes while the namespace shadow was never armed fails instead.

No dump file at all is a failure, not a total of zero: zero files means no
build wrote evidence, which must not read as a clean run.
"""

from __future__ import annotations

import glob
import json
import sys

# Both thresholds are the step's reason to exist: the store must have
# mirrored a table, and the consumer must have served a snapshot from it.
_REQUIRED = {
    "flip.tables_mirrored": "the read flip mirrored no table",
    "astdiff.served": "astdiff served no snapshot from the store",
}


def _totals(paths: list[str]) -> dict[str, int]:
    totals: dict[str, int] = {}
    for path in paths:
        with open(path, encoding="utf8") as dump:
            report = json.load(dump)
        for section in ("flip", "astdiff"):
            for key, value in report.get(section, {}).items():
                totals[f"{section}.{key}"] = totals.get(f"{section}.{key}", 0) + int(value)
    return totals


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(f"usage: {argv[0]} <dump-glob> [dump-glob ...]", file=sys.stderr)
        return 2
    paths: list[str] = []
    for pattern in argv[1:]:
        paths.extend(glob.glob(pattern))
    if not paths:
        print(
            "no sessionfinish dump matched "
            f"{argv[1:]}: the read flip never ran (is the output env var set?)",
            file=sys.stderr,
        )
        return 1
    totals = _totals(paths)
    print(f"sessionfinish dumps: {len(paths)}")
    for key in sorted(totals):
        print(f"  {key}: {totals[key]}")
    failures = [reason for key, reason in _REQUIRED.items() if totals.get(key, 0) <= 0]
    if failures:
        print("read-flip engagement evidence missing: " + "; ".join(failures), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
