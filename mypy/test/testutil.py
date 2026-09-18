from __future__ import annotations

import ast
import atexit
import os
from pathlib import Path
from unittest import TestCase, mock

from mypy.inspections import parse_location
from mypy.util import _generate_junit_contents, get_terminal_width, hard_exit


class TestGetTerminalSize(TestCase):
    def test_get_terminal_size_in_pty_defaults_to_80(self) -> None:
        # when run using a pty, `os.get_terminal_size()` returns `0, 0`
        ret = os.terminal_size((0, 0))
        mock_environ = os.environ.copy()
        mock_environ.pop("COLUMNS", None)
        with mock.patch.object(os, "get_terminal_size", return_value=ret):
            with mock.patch.dict(os.environ, values=mock_environ, clear=True):
                assert get_terminal_width() == 80

    def test_parse_location_windows(self) -> None:
        assert parse_location(r"C:\test.py:1:1") == (r"C:\test.py", [1, 1])
        assert parse_location(r"C:\test.py:1:1:1:1") == (r"C:\test.py", [1, 1, 1, 1])


class TestWriteJunitXml(TestCase):
    def test_junit_pass(self) -> None:
        serious = False
        messages_by_file: dict[str | None, list[str]] = {}
        expected = """<?xml version="1.0" encoding="utf-8"?>
<testsuite errors="0" failures="0" name="mypy" skips="0" tests="1" time="1.230">
  <testcase classname="mypy" file="mypy" line="1" name="mypy-py3.14-test-plat" time="1.230">
  </testcase>
</testsuite>
"""
        result = _generate_junit_contents(
            dt=1.23,
            serious=serious,
            messages_by_file=messages_by_file,
            version="3.14",
            platform="test-plat",
        )
        assert result == expected

    def test_junit_fail_escape_xml_chars(self) -> None:
        serious = False
        messages_by_file: dict[str | None, list[str]] = {
            "file1.py": ["Test failed", "another line < > &"]
        }
        expected = """<?xml version="1.0" encoding="utf-8"?>
<testsuite errors="0" failures="1" name="mypy" skips="0" tests="1" time="1.230">
  <testcase classname="mypy" file="file1.py" line="1" name="mypy-py3.14-test-plat file1.py" time="1.230">
    <failure message="mypy produced messages">Test failed
another line &lt; &gt; &amp;</failure>
  </testcase>
</testsuite>
"""
        result = _generate_junit_contents(
            dt=1.23,
            serious=serious,
            messages_by_file=messages_by_file,
            version="3.14",
            platform="test-plat",
        )
        assert result == expected

    def test_junit_fail_two_files(self) -> None:
        serious = False
        messages_by_file: dict[str | None, list[str]] = {
            "file1.py": ["Test failed", "another line"],
            "file2.py": ["Another failure", "line 2"],
        }
        expected = """<?xml version="1.0" encoding="utf-8"?>
<testsuite errors="0" failures="2" name="mypy" skips="0" tests="2" time="1.230">
  <testcase classname="mypy" file="file1.py" line="1" name="mypy-py3.14-test-plat file1.py" time="1.230">
    <failure message="mypy produced messages">Test failed
another line</failure>
  </testcase>
  <testcase classname="mypy" file="file2.py" line="1" name="mypy-py3.14-test-plat file2.py" time="1.230">
    <failure message="mypy produced messages">Another failure
line 2</failure>
  </testcase>
</testsuite>
"""
        result = _generate_junit_contents(
            dt=1.23,
            serious=serious,
            messages_by_file=messages_by_file,
            version="3.14",
            platform="test-plat",
        )
        assert result == expected

    def test_serious_error(self) -> None:
        serious = True
        messages_by_file: dict[str | None, list[str]] = {None: ["Error line 1", "Error line 2"]}
        expected = """<?xml version="1.0" encoding="utf-8"?>
<testsuite errors="1" failures="0" name="mypy" skips="0" tests="1" time="1.230">
  <testcase classname="mypy" file="mypy" line="1" name="mypy-py3.14-test-plat" time="1.230">
    <failure message="mypy produced messages">Error line 1
Error line 2</failure>
  </testcase>
</testsuite>
"""
        result = _generate_junit_contents(
            dt=1.23,
            serious=serious,
            messages_by_file=messages_by_file,
            version="3.14",
            platform="test-plat",
        )
        assert result == expected


class TestHardExit(TestCase):
    def test_hard_exit_runs_atexit_handlers(self) -> None:
        # os._exit skips atexit handlers, which silently dropped diagnostics
        # registered by instrumentation kernels (#1061); hard_exit must run
        # them explicitly before the hard kill.
        seen: list[int] = []
        atexit.register(lambda: seen.append(1))
        with mock.patch("os._exit") as fake_exit:
            hard_exit(0)
        assert seen == [1]
        fake_exit.assert_called_once_with(0)

    def test_hard_exit_flushes_handler_output(self) -> None:
        # End-to-end #1706 repro: a handler writing to block-buffered
        # stdout loses its output unless hard_exit flushes after the
        # handlers run. Read back via a separate fd (closing would flush).
        import os
        import sys
        import tempfile

        fd, path = tempfile.mkstemp()
        os.close(fd)
        try:
            f = open(path, "w", buffering=8192)
            try:

                def handler() -> None:
                    if not f.closed:
                        f.write("diagnostic\n")

                atexit.register(handler)
                with mock.patch("os._exit") as fake_exit, mock.patch.object(sys, "stdout", f):
                    hard_exit(0)
                with open(path, "rb") as reader:
                    assert reader.read() == b"diagnostic\n"
                fake_exit.assert_called_once_with(0)
            finally:
                f.close()
        finally:
            os.unlink(path)


class TestCollectionGuard(TestCase):
    """No test module may silently collect zero tests (#1701).

    `python_classes`/`python_functions` are empty and the custom collector
    only handles DataSuite subclasses, so a module-level zero-arg
    `def test_*` or a plain `class Test*` with test methods collects
    nothing while pytest still exits 0. This guard fails loudly instead.
    """

    def test_no_silent_zero_collection(self) -> None:
        test_dir = Path(__file__).resolve().parent
        roots = [test_dir, test_dir.parent.parent / "mypyc" / "test"]
        offenders: list[str] = []
        for root in roots:
            for path in sorted(root.glob("test*.py")):
                tree = ast.parse(path.read_text())
                names = {n.name: n for n in tree.body if isinstance(n, ast.ClassDef)}
                for node in tree.body:
                    if isinstance(node, ast.FunctionDef) and node.name.startswith("test_"):
                        args = node.args
                        if (
                            not args.args
                            and not args.posonlyargs
                            and not args.kwonlyargs
                            and args.vararg is None
                            and args.kwarg is None
                        ):
                            offenders.append(f"{path.name}:{node.lineno} {node.name}")
                    elif isinstance(node, ast.ClassDef) and node.name.startswith("Test"):
                        bases = [
                            b.id if isinstance(b, ast.Name) else getattr(b, "attr", "?")
                            for b in node.bases
                        ]
                        collected = self._collectable(node.name, bases, names)
                        if not collected:
                            methods = [
                                m.name
                                for m in node.body
                                if isinstance(m, ast.FunctionDef) and m.name.startswith("test")
                            ]
                            if methods:
                                offenders.append(f"{path.name}:{node.lineno} {node.name}")
        assert not offenders, (
            "test-looking definitions that pytest will not collect "
            "(convert to TestCase/DataSuite or remove): " + ", ".join(offenders)
        )

    @staticmethod
    def _collectable(name: str, bases: list[str], names: dict[str, ast.ClassDef]) -> bool:
        """Whether a Test* class is picked up by a collector."""
        if any(
            b in ("TestCase", "DataSuite", "DataDrivenTestCase", "Suite", "MypycDataSuite")
            for b in bases
        ):
            return True
        return any(
            TestCollectionGuard._collectable(
                base, TestCollectionGuard._bases(names.get(base)), names
            )
            for base in bases
            if base in names
        )

    @staticmethod
    def _bases(node: ast.ClassDef | None) -> list[str]:
        if node is None:
            return []
        return [b.id if isinstance(b, ast.Name) else getattr(b, "attr", "?") for b in node.bases]
