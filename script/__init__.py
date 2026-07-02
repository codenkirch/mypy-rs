"""Development command entry points.

These commands are intended to be run through uv, for example:

    uv run test
    uv run pytest -n0 -k test_name
"""

from __future__ import annotations

import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ALL_TEST_GROUPS = (
    "self",
    "self-packaging",
    "lint",
    "pytest-fast",
    "pytest-cmdline",
    "pytest-slow",
    "mypyc-fast",
    "pytest-extra",
    "mypyc-extra",
)


def run_step(name: str, command: Sequence[str], *, env: dict[str, str] | None = None) -> int:
    """Run a named development step and return its process status."""
    print(f"run {name}: {list(command)}")
    result = subprocess.run(command, cwd=ROOT, env=env)
    if result.returncode:
        print(f"\nFAILED: {name}")
    return result.returncode


def require_success(
    name: str, command: Sequence[str], *, env: dict[str, str] | None = None
) -> None:
    status = run_step(name, command, env=env)
    if status:
        raise SystemExit(status)


def test(args: Sequence[str] | None = None) -> None:
    """Run the grouped test suite through runtests.py."""
    if args is None:
        args = sys.argv[1:]
    require_success("test", [sys.executable, "runtests.py", *args])


def pytest(args: Sequence[str] | None = None) -> None:
    """Run pytest directly."""
    if args is None:
        args = sys.argv[1:]
    require_success("pytest", [sys.executable, "-m", "pytest", *args])


def lint(args: Sequence[str] | None = None) -> None:
    """Run the full pre-commit lint suite."""
    if args is None:
        args = sys.argv[1:]
    require_success("lint", ["pre-commit", "run", "--all-files", "--show-diff-on-failure", *args])


def format(args: Sequence[str] | None = None) -> None:
    """Run the formatting hooks over the full tree."""
    if args is None:
        args = sys.argv[1:]
    status = 0
    for hook in ("trailing-whitespace", "end-of-file-fixer", "black", "ruff-check"):
        status = run_step("format", ["pre-commit", "run", hook, "--all-files", *args]) or status
    if status:
        raise SystemExit(status)


def typecheck(args: Sequence[str] | None = None) -> None:
    """Run the self type-checking steps."""
    if args is None:
        args = sys.argv[1:]
    if args:
        require_success("typecheck", [sys.executable, "-m", "mypy", *args])
        return

    steps = [
        ("self", [sys.executable, "runtests.py", "self"]),
        (
            "type-misc",
            [
                sys.executable,
                "-m",
                "mypy",
                "--config-file",
                "mypy_self_check.ini",
                "misc",
                "--exclude",
                "misc/sync-typeshed.py",
            ],
        ),
        (
            "type-plugins",
            [
                sys.executable,
                "-m",
                "mypy",
                "--config-file",
                "mypy_self_check.ini",
                "test-data/unit/plugins",
            ],
        ),
        (
            "type-script",
            [sys.executable, "-m", "mypy", "--config-file", "mypy_self_check.ini", "script"],
        ),
        ("type-mypyc-lib-rt", [sys.executable, "-m", "mypy", "mypyc/lib-rt"]),
    ]
    for name, command in steps:
        require_success(name, command)


def docs(args: Sequence[str] | None = None) -> None:
    """Build the documentation."""
    if args is None:
        args = sys.argv[1:]
    out_dir = ROOT / "docs" / "build" / "html"
    doctree_dir = ROOT / "docs" / "build" / "doctrees"
    command = [
        "sphinx-build",
        "-n",
        "-d",
        str(doctree_dir),
        "docs/source",
        str(out_dir),
        "--color",
        "-W",
        "-bhtml",
        *args,
    ]
    require_success("docs", command)
    if "--version" not in args and "-M" not in args:
        print(f"documentation available under file://{out_dir / 'index.html'}")


def all_checks(args: Sequence[str] | None = None) -> None:
    """Run the broad local verification suite."""
    if args is None:
        args = sys.argv[1:]

    require_success("lock", ["uv", "lock", "--check"])
    typecheck([])
    docs([])
    lint([])
    test(args or ALL_TEST_GROUPS)


def env(args: Sequence[str] | None = None) -> None:
    """Inspect the uv-managed environment or run an arbitrary command."""
    if args is None:
        args = sys.argv[1:]
    if args:
        require_success("env", list(args))
    else:
        require_success("pip-list", [sys.executable, "-m", "pip", "list", "--format=columns"])
        require_success(
            "python-executable", [sys.executable, "-c", "import sys; print(sys.executable)"]
        )


def _usage() -> str:
    return (
        "usage: dev {test,pytest,lint,format,typecheck,docs,env} [args...]\n\n"
        "Run mypy development commands"
    )


def main() -> None:
    commands = {
        "test": test,
        "pytest": pytest,
        "lint": lint,
        "format": format,
        "typecheck": typecheck,
        "docs": docs,
        "env": env,
    }
    if len(sys.argv) < 2 or sys.argv[1] in ("-h", "--help"):
        print(_usage())
        raise SystemExit(0)
    command = sys.argv[1]
    args = sys.argv[2:]
    if args and args[0] == "--":
        args = args[1:]
    try:
        runner = commands[command]
    except KeyError:
        print(_usage(), file=sys.stderr)
        raise SystemExit(f"unknown command: {command}") from None
    runner(args)


__all__ = [
    "all_checks",
    "docs",
    "env",
    "format",
    "lint",
    "main",
    "pytest",
    "run_step",
    "test",
    "typecheck",
]
