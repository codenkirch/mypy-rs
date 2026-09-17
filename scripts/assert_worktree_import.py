"""Pre-flight: prove which mypy source tree this interpreter would import.

Replacement for the old PYTHONPATH + `python -c "import mypy"` incantation,
which the shared venv defeats through two channels (#1789):

1. The PEP 660 editable install puts an editable finder on sys.meta_path.
   Its mapping points at whatever tree last installed into the venv, which
   may be a stale or deleted worktree.
2. `python -c` puts the current directory at sys.path[0], shadowing every
   PYTHONPATH entry, so the check silently reports the tree you are
   standing in instead of the tree you asked about.

This script strips both channels of accident, puts the requested tree first
on sys.path, imports mypy, and exits 0 only if `mypy.__file__` resolves
inside that tree. PYTHONPATH is preserved, so scratch extension dirs on it
keep working. Run it with the same venv interpreter and PYTHONPATH the real
suite or probe run will use.

Usage:
    .venv/bin/python scripts/assert_worktree_import.py <tree-under-test>

Exit status: 0 on match (prints the resolved mypy path), 1 on mismatch or
import failure, 2 on usage error.
"""

from __future__ import annotations

import os
import sys


def _is_editable_finder(finder: object) -> bool:
    # pip appends the finder *class* to sys.meta_path, so the entry itself
    # can be a type; check both shapes and both name sources.
    cls = finder if isinstance(finder, type) else type(finder)
    return "editable" in cls.__name__.lower() or "editable" in cls.__module__.lower()


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <tree-under-test>", file=sys.stderr)
        return 2
    expected_root = os.path.realpath(os.path.abspath(sys.argv[1]))

    sys.meta_path[:] = [f for f in sys.meta_path if not _is_editable_finder(f)]
    # The cwd must never silently decide the tree: drop the '' entry and
    # absolute spellings of the cwd. Relative spellings survive, but the
    # requested tree is inserted first, so they cannot win.
    cwd_entries = {"", os.getcwd(), os.path.realpath(os.getcwd())}
    sys.path[:] = [p for p in sys.path if p not in cwd_entries]
    sys.path.insert(0, expected_root)

    try:
        import mypy
    except ImportError as exc:
        print(
            f"FAIL: mypy is not importable with the editable finder stripped\n  ImportError: {exc}",
            file=sys.stderr,
        )
        return 1

    mypy_file = getattr(mypy, "__file__", None)
    imported_root = os.path.dirname(os.path.dirname(mypy_file)) if mypy_file else None
    if imported_root is None or os.path.realpath(imported_root) != expected_root:
        print(
            "FAIL: mypy resolved outside the requested tree (#1789)\n"
            f"  requested tree: {expected_root}\n"
            f"  imported mypy:  {mypy_file}",
            file=sys.stderr,
        )
        return 1

    print(f"OK: mypy resolves inside {expected_root}")
    print(f"    mypy.__file__ = {mypy_file}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
