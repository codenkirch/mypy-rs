"""Aggregate the node-shadow session dumps of one cache-loaded step (#1825).

Each build under `MYPY_TK_STMT_SESSIONFINISH_OUT` writes a `sessionfinish`
dump: a `{pid}`-expanded JSON file per process holding the statement
channel's read counters (`stmt_read`), the Python audit counters
(`capture`, non-empty only with `MYPY_TK_AST_MIRROR_AUDIT=1`) and the live
metadata entry count (`meta_entries`).

This script is the engagement gate of a cache-loaded step. It exits
non-zero when the run seeded no field record and attributed no capture to
a cache reader, so a step that passes while the cache path was never
walked - or while the store was never armed - fails instead of reading as
a clean run.

No dump file at all is a failure, not a total of zero: zero files means no
build wrote evidence.

Usage:

    MYPY_TK_STMT_SESSIONFINISH_OUT='/tmp/dumps/stmt-{pid}.json' \
        .venv/bin/python -m pytest mypy/test/testfinegrainedcache.py -q
    .venv/bin/python misc/def_cache_seed_report.py '/tmp/dumps/stmt-*.json'
"""

from __future__ import annotations

import glob
import json
import sys

# Both thresholds are the step's reason to exist: the seed must have taken
# field records, and the capture must be attributed to a cache reader
# rather than to the parse path.
_REQUIRED = {
    "meta_seeded.cache_fixed": "the fixed-format cache reader seeded nothing",
    "meta_written.cache_fixed": "no capture was attributed to the fixed-format reader",
}

_PREFIXES = ("meta_seeded.", "meta_written.", "meta_seed_loaded", "meta_seed_rejected")


def _totals(paths: list[str]) -> dict[str, int]:
    totals: dict[str, int] = {}
    for path in paths:
        with open(path, encoding="utf8") as dump:
            report = json.load(dump)
        for section in ("capture", "stmt_read"):
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
            f"no sessionfinish dump matched {argv[1:]}: the measured step never ran "
            "(is the output env var set, and was the store armed?)",
            file=sys.stderr,
        )
        return 1
    totals = _totals(paths)
    print(f"sessionfinish dumps: {len(paths)}")
    for key in sorted(totals):
        if key.startswith(tuple("capture." + p for p in _PREFIXES) + ("capture.meta_seed",)):
            print(f"  {key}: {totals[key]}")
    for key in sorted(k for k in totals if k.startswith("stmt_read.")):
        print(f"  {key}: {totals[key]}")
    failures = [
        reason for key, reason in _REQUIRED.items() if totals.get("capture." + key, 0) <= 0
    ]
    if failures:
        print("cache-loaded seed evidence missing: " + "; ".join(failures), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
