"""Differential checker oracle: mypy vs pyrefly vs ty on a shared corpus.

Runs all three type checkers (where installed) over the same Python
corpus, parses each tool's diagnostics into a uniform (path, line, col,
code, message) tuple, and buckets the disagreements. This is a triage
hint, not a parity oracle: pyrefly and ty implement their own
gradual-typing semantics and will disagree with mypy on many intentional
calls. Use the report to surface type-semantics edge cases the mypy
suite may not cover before they become parity regressions, not to gate
any PR.

Usage:

    python scripts/diff_checkers.py [--corpus DIR] [--out REPORT.md]

By default the corpus is the repo root (mypy + mypyc + bundled
typeshed), matching scripts/bench_parser.py. Missing tools are skipped
with a header note; mypy itself is always run (the venv mypy is on
PATH when invoked through the project venv).

The script is intentionally dependency-free (stdlib only) so it runs
in any venv that has mypy installed, without requiring pyrefly or ty.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Default corpus mirrors scripts/bench_parser.py: mypy + mypyc +
# bundled typeshed. Excludes the venv, build artifacts, and the
# generated conformance corpus.
DEFAULT_CORPUS_SUBDIRS = ("mypy", "mypyc")
DEFAULT_EXCLUDES = ("/.venv/", "/build/", "/__pycache__/", "/conformance/")


@dataclass(frozen=True)
class Diagnostic:
    """Uniform diagnostic shape across all three checkers.

    The (path, line, col, code) tuple is the join key for disagreement
    bucketing. message is human-readable context only.
    """

    path: str
    line: int
    col: int
    code: str
    message: str


@dataclass
class CheckerResult:
    """One checker's output over the corpus."""

    name: str
    available: bool
    diagnostics: list[Diagnostic] = field(default_factory=list)
    note: str = ""


def collect_corpus(corpus_root: Path) -> list[Path]:
    """Collect .py and .pyi files under corpus_root, excluding venv/build."""
    files: list[Path] = []
    for sub in DEFAULT_CORPUS_SUBDIRS:
        sub_root = corpus_root / sub
        if not sub_root.is_dir():
            continue
        files.extend(sub_root.rglob("*.py"))
        files.extend(sub_root.rglob("*.pyi"))
    # typeshed stubs exercise the .pyi path heavily
    typeshed = corpus_root / "mypy" / "typeshed"
    if typeshed.is_dir():
        files.extend(typeshed.rglob("*.pyi"))
    out: list[Path] = []
    for f in files:
        s = str(f)
        if any(excl in s for excl in DEFAULT_EXCLUDES):
            continue
        out.append(f)
    out.sort()
    return out


def find_executable(name: str) -> str | None:
    """Locate a checker executable on PATH, or None if absent."""
    return shutil.which(name)


def run_mypy(files: list[Path], repo_root: Path) -> CheckerResult:
    """Run mypy with the native type kernel if available.

    Uses --no-error-summary and --show-error-codes for parseable output.
    The native kernel is toggled via MYPYPATH / env, but for the diff
    harness the default venv mypy is fine: the goal is to compare
    against pyrefly/ty, not to differential mypy-native-vs-python
    (that is covered by the parity suites).
    """
    mypy_bin = find_executable("mypy")
    if mypy_bin is None:
        venv_mypy = repo_root / ".venv" / "bin" / "mypy"
        if venv_mypy.exists():
            mypy_bin = str(venv_mypy)
    if mypy_bin is None:
        return CheckerResult(name="mypy", available=False, note="mypy not on PATH")
    # mypy chokes on huge file lists via argv; pass via stdin file list.
    cmd = [
        mypy_bin,
        "--no-error-summary",
        "--show-error-codes",
        "--no-pretty",
        "--no-color-output",
        "--show-column-numbers",
        "--follow-imports=silent",
    ]
    # Write the file list to a tempfile and pass --files-from-stdin would
    # need a here-doc; simpler to batch. mypy accepts a directory or a
    # list of files. Cap at 500 files per invocation to stay under argv
    # limits on macOS (256KB).
    diags: list[Diagnostic] = []
    batch_size = 500
    for i in range(0, len(files), batch_size):
        batch = files[i : i + batch_size]
        proc = subprocess.run(
            cmd + [str(f) for f in batch],
            capture_output=True,
            text=True,
            cwd=str(repo_root),
            timeout=600,
        )
        diags.extend(parse_mypy_output(proc.stdout))
    return CheckerResult(name="mypy", available=True, diagnostics=diags)


# mypy default line format (no --pretty):
#   path:line:col: error: message  [code]
#   path:line:col: warning: message  [code]
MYPY_LINE_RE = re.compile(
    r"^(?P<path>[^:]+):(?P<line>\d+):(?P<col>\d+):\s+"
    r"(?P<severity>error|warning|note):\s+(?P<message>.+?)"
    r"\s+\[(?P<code>[^\]]+)\]\s*$"
)


def parse_mypy_output(stdout: str) -> list[Diagnostic]:
    """Parse mypy's --show-column-numbers output into diagnostics."""
    out: list[Diagnostic] = []
    for line in stdout.splitlines():
        m = MYPY_LINE_RE.match(line)
        if not m:
            continue
        if m.group("severity") == "note":
            # Notes attach to the preceding error; skip for the diff
            # harness to keep the join key stable.
            continue
        out.append(
            Diagnostic(
                path=m.group("path"),
                line=int(m.group("line")),
                col=int(m.group("col")),
                code=m.group("code"),
                message=m.group("message"),
            )
        )
    return out


def run_pyrefly(files: list[Path], repo_root: Path) -> CheckerResult:
    """Run pyrefly check if installed; skip otherwise.

    pyrefly emits JSON via --output-format json. The schema is unstable
    across versions, so this parser is defensive: if the JSON shape does
    not match expectations, fall back to parsing the human-readable
    text format (which reuses the mypy regex).
    """
    pyrefly_bin = find_executable("pyrefly")
    if pyrefly_bin is None:
        return CheckerResult(
            name="pyrefly", available=False, note="pyrefly not installed (expected)"
        )
    cmd = [pyrefly_bin, "check", "--output-format", "json", "--no-banner"]
    diags: list[Diagnostic] = []
    batch_size = 200
    for i in range(0, len(files), batch_size):
        batch = files[i : i + batch_size]
        proc = subprocess.run(
            cmd + [str(f) for f in batch],
            capture_output=True,
            text=True,
            cwd=str(repo_root),
            timeout=600,
        )
        diags.extend(parse_pyrefly_json(proc.stdout, proc.stderr))
    return CheckerResult(name="pyrefly", available=True, diagnostics=diags)


def parse_pyrefly_json(stdout: str, stderr: str) -> list[Diagnostic]:
    """Parse pyrefly's JSON output. Defensive: fall back to text parse."""
    if not stdout.strip():
        return []
    try:
        data = json.loads(stdout)
    except json.JSONDecodeError:
        # Older pyrefly versions emit human-readable text on stdout.
        return parse_mypy_output(stdout)
    diags: list[Diagnostic] = []
    # pyrefly JSON shape: {"errors": [{"path": ..., "diagnostics": [...]}]}
    # The exact key names vary; walk defensively.
    errors = data.get("errors") if isinstance(data, dict) else None
    if errors is None and isinstance(data, list):
        errors = data
    if not isinstance(errors, list):
        return []
    for entry in errors:
        if not isinstance(entry, dict):
            continue
        path = entry.get("path") or entry.get("file") or ""
        for diag in entry.get("diagnostics") or entry.get("errors") or []:
            if not isinstance(diag, dict):
                continue
            loc = diag.get("location") or diag.get("span") or {}
            line = int(loc.get("start_line") or loc.get("line") or 0)
            col = int(loc.get("start_column") or loc.get("column") or 0)
            code = diag.get("code") or diag.get("rule") or "pyrefly"
            message = diag.get("message") or ""
            diags.append(
                Diagnostic(path=path, line=line, col=col, code=str(code), message=message)
            )
    return diags


def run_ty(files: list[Path], repo_root: Path) -> CheckerResult:
    """Run ty check if installed; skip otherwise.

    ty (the ruff type checker) emits JSON via --output-format json.
    """
    ty_bin = find_executable("ty")
    if ty_bin is None:
        return CheckerResult(name="ty", available=False, note="ty not installed (expected)")
    cmd = [ty_bin, "check", "--output-format", "json"]
    diags: list[Diagnostic] = []
    batch_size = 200
    for i in range(0, len(files), batch_size):
        batch = files[i : i + batch_size]
        proc = subprocess.run(
            cmd + [str(f) for f in batch],
            capture_output=True,
            text=True,
            cwd=str(repo_root),
            timeout=600,
        )
        diags.extend(parse_ty_json(proc.stdout, proc.stderr))
    return CheckerResult(name="ty", available=True, diagnostics=diags)


def parse_ty_json(stdout: str, stderr: str) -> list[Diagnostic]:
    """Parse ty's JSON output. Defensive: fall back to text parse."""
    if not stdout.strip():
        return []
    try:
        data = json.loads(stdout)
    except json.JSONDecodeError:
        return parse_mypy_output(stdout)
    diags: list[Diagnostic] = []
    # ty JSON shape: {"diagnostics": [{"file": ..., "range": {...}, ...}]}
    items = data.get("diagnostics") if isinstance(data, dict) else None
    if not isinstance(items, list):
        return []
    for diag in items:
        if not isinstance(diag, dict):
            continue
        path = diag.get("file") or ""
        rng = diag.get("range") or {}
        start = rng.get("start") or {}
        line = int(start.get("line") or 0)
        col = int(start.get("character") or start.get("column") or 0)
        code = diag.get("code") or "ty"
        message = diag.get("message") or ""
        diags.append(
            Diagnostic(path=path, line=line, col=col, code=str(code), message=message)
        )
    return diags


def bucket_disagreements(results: list[CheckerResult]) -> dict[str, list[tuple]]:
    """Bucket diagnostics by (path, line, col) into disagreement classes.

    Buckets:
      - "all_agree": all available checkers report the same location.
        (Informational; not a disagreement.)
      - "all_disagree": location reported by only one checker.
      - "two_of_three": two checkers agree, one does not.
      - "mypy_only": mypy reports a location the others do not.
      - "pyrefly_only", "ty_only": analogously.
    """
    by_loc: dict[tuple, dict[str, Diagnostic]] = defaultdict(dict)
    for r in results:
        if not r.available:
            continue
        for d in r.diagnostics:
            by_loc[(d.path, d.line, d.col)][r.name] = d
    buckets: dict[str, list[tuple]] = defaultdict(list)
    available = [r.name for r in results if r.available]
    for loc, by_checker in by_loc.items():
        reporters = set(by_checker)
        if reporters == set(available):
            if len({d.code for d in by_checker.values()}) == 1:
                buckets["all_agree"].append((loc, by_checker))
            else:
                buckets["all_reported_diff_code"].append((loc, by_checker))
        elif len(reporters) == 1:
            (only,) = reporters
            buckets[f"{only}_only"].append((loc, by_checker))
        else:
            missing = set(available) - reporters
            buckets[f"missing_{'_'.join(sorted(missing))}"].append((loc, by_checker))
    return buckets


def render_report(
    results: list[CheckerResult],
    buckets: dict[str, list[tuple]],
    corpus_size: int,
) -> str:
    """Render the disagreement report as markdown."""
    lines: list[str] = []
    lines.append("# Differential checker report")
    lines.append("")
    lines.append(f"Corpus: {corpus_size} files")
    lines.append("")
    lines.append("## Checker availability")
    lines.append("")
    lines.append("| Checker | Available | Diagnostics | Note |")
    lines.append("|---------|-----------|-------------|------|")
    for r in results:
        lines.append(
            f"| {r.name} | {r.available} | {len(r.diagnostics)} | {r.note} |"
        )
    lines.append("")
    lines.append("## Disagreement buckets")
    lines.append("")
    lines.append("pyrefly and ty implement their own gradual-typing semantics and")
    lines.append("cannot be vendored for mypy parity. Disagreements are *expected*")
    lines.append("and are triage hints, not failures. Use them to surface edge cases")
    lines.append("the mypy suite may not cover before they become parity regressions.")
    lines.append("")
    lines.append("| Bucket | Count |")
    lines.append("|--------|-------|")
    for name in sorted(buckets):
        lines.append(f"| {name} | {len(buckets[name])} |")
    lines.append("")
    # Sample up to 20 entries per non-trivial bucket.
    for name in sorted(buckets):
        entries = buckets[name]
        if not entries or name == "all_agree":
            continue
        lines.append(f"## {name} (sample, max 20)")
        lines.append("")
        for loc, by_checker in entries[:20]:
            path, line, col = loc
            lines.append(f"- `{path}:{line}:{col}`")
            for cname, d in sorted(by_checker.items()):
                lines.append(f"  - {cname} [{d.code}]: {d.message}")
        lines.append("")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--corpus",
        type=Path,
        default=REPO,
        help="Corpus root (default: repo root).",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Output report path (default: stdout).",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Cap corpus size (for smoke testing).",
    )
    args = parser.parse_args(argv)

    files = collect_corpus(args.corpus)
    if args.limit:
        files = files[: args.limit]
    print(f"corpus: {len(files)} files", file=sys.stderr)

    results: list[CheckerResult] = [
        run_mypy(files, REPO),
        run_pyrefly(files, REPO),
        run_ty(files, REPO),
    ]

    buckets = bucket_disagreements(results)
    report = render_report(results, buckets, corpus_size=len(files))

    if args.out:
        args.out.write_text(report)
        print(f"report written to {args.out}", file=sys.stderr)
    else:
        print(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
