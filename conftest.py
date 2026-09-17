from __future__ import annotations

import os
import os.path


# Load-bearing (#1789): a suite must exercise the tree this conftest lives
# in. The shared venv's editable finder and the cwd sys.path entry can both
# resolve `mypy` to a foreign tree, green-lighting a hollow run. Fail early.
def _assert_mypy_source_tree() -> None:
    expected_root = os.path.dirname(os.path.abspath(__file__))
    if not os.path.isfile(os.path.join(expected_root, "mypy", "__init__.py")):
        # This conftest is not part of a mypy source tree (e.g. tests shipped
        # inside an installed package); nothing to compare against.
        return
    try:
        import mypy
    except ImportError as exc:
        raise AssertionError(
            "mypy is not importable at all (#1789); the shared venv's editable\n"
            "install may point at a deleted tree.\n"
            f"  tree under test: {expected_root}\n"
            f"  ImportError: {exc}"
        ) from exc
    mypy_file = getattr(mypy, "__file__", None)
    if mypy_file is None or os.path.realpath(
        os.path.dirname(os.path.dirname(mypy_file))
    ) != os.path.realpath(expected_root):
        raise AssertionError(
            "mypy resolved outside the tree under test (#1789): the suite would\n"
            "report a hollow green against a foreign tree.\n"
            f"  tree under test: {os.path.realpath(expected_root)}\n"
            f"  imported mypy:   {mypy_file}\n"
            "  Fix: run pytest from the tree under test, or strip the venv's\n"
            "  editable finder and put the tree first on PYTHONPATH\n"
            "  (scripts/assert_worktree_import.py checks this standalone)."
        )


_assert_mypy_source_tree()

pytest_plugins = ["mypy.test.data"]


def pytest_configure(config):
    mypy_source_root = os.path.dirname(os.path.abspath(__file__))
    if os.getcwd() != mypy_source_root:
        os.chdir(mypy_source_root)


# This function name is special to pytest.  See
# https://docs.pytest.org/en/latest/how-to/writing_plugins.html
def pytest_addoption(parser) -> None:
    parser.addoption(
        "--bench", action="store_true", default=False, help="Enable the benchmark test runs"
    )
