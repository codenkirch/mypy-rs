"""End-to-end test cases for the daemon (dmypy).

These are special because they run multiple shell commands.

This also includes some unit tests.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest

from mypy.dmypy_server import filter_out_missing_top_level_packages
from mypy.fscache import FileSystemCache
from mypy.modulefinder import SearchPaths
from mypy.test.config import PREFIX, test_temp_dir
from mypy.test.data import DataDrivenTestCase, DataSuite
from mypy.test.helpers import assert_string_arrays_equal, normalize_error_messages

try:
    import type_kernel  # noqa: F401

    _HAS_TYPE_KERNEL = True
except ImportError:
    _HAS_TYPE_KERNEL = False

# Files containing test cases descriptions.
daemon_files = ["daemon.test"]


class DaemonSuite(DataSuite):
    files = daemon_files

    def run_case(self, testcase: DataDrivenTestCase) -> None:
        try:
            test_daemon(testcase)
        finally:
            # Kill the daemon if it's still running.
            run_cmd("dmypy kill")


def test_daemon(testcase: DataDrivenTestCase) -> None:
    assert testcase.old_cwd is not None, "test was not properly set up"
    for i, step in enumerate(parse_script(testcase.input)):
        cmd = step[0]
        expected_lines = step[1:]
        assert cmd.startswith("$")
        cmd = cmd[1:].strip()
        cmd = cmd.replace("{python}", sys.executable)
        sts, output = run_cmd(cmd)
        output_lines = output.splitlines()
        output_lines = normalize_error_messages(output_lines)
        if sts:
            output_lines.append("== Return code: %d" % sts)
        assert_string_arrays_equal(
            expected_lines,
            output_lines,
            "Command %d (%s) did not give expected output" % (i + 1, cmd),
        )


def parse_script(input: list[str]) -> list[list[str]]:
    """Parse testcase.input into steps.

    Each command starts with a line starting with '$'.
    The first line (less '$') is sent to the shell.
    The remaining lines are expected output.
    """
    steps = []
    step: list[str] = []
    for line in input:
        if line.startswith("$"):
            if step:
                assert step[0].startswith("$")
                steps.append(step)
                step = []
        step.append(line)
    if step:
        steps.append(step)
    return steps


def run_cmd(input: str) -> tuple[int, str]:
    if input[1:].startswith("mypy run --") and "--show-error-codes" not in input:
        input += " --hide-error-codes"
    if input.startswith("dmypy "):
        input = sys.executable + " -m mypy." + input
    if input.startswith("mypy "):
        input = sys.executable + " -m" + input
    env = os.environ.copy()
    # Prepend (not replace) so the Rust extension dirs on PYTHONPATH
    # (ast_serialize, module_resolver) survive into the dmypy subprocess.
    # Overwriting here drops them, and the venv's PyPI `ast-serialize` stub

    # (which has no `parse`) shadows the missing Rust extension.
    env["PYTHONPATH"] = PREFIX + os.pathsep + env.get("PYTHONPATH", "")
    try:
        output = subprocess.check_output(
            input, shell=True, stderr=subprocess.STDOUT, text=True, cwd=test_temp_dir, env=env
        )
        return 0, output
    except subprocess.CalledProcessError as err:
        return err.returncode, err.output


class DaemonUtilitySuite(unittest.TestCase):
    """Unit tests for helpers"""

    def test_filter_out_missing_top_level_packages(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            self.make_file(td, "base/a/")
            self.make_file(td, "base/b.py")
            self.make_file(td, "base/c.pyi")
            self.make_file(td, "base/missing.txt")
            self.make_file(td, "typeshed/d.pyi")
            self.make_file(td, "typeshed/@python2/e")  # outdated
            self.make_file(td, "pkg1/f-stubs")
            self.make_file(td, "pkg2/g-python2-stubs")  # outdated
            self.make_file(td, "mpath/sub/long_name/")

            def makepath(p: str) -> str:
                return os.path.join(td, p)

            search = SearchPaths(
                python_path=(makepath("base"),),
                mypy_path=(makepath("mpath/sub"),),
                package_path=(makepath("pkg1"), makepath("pkg2")),
                typeshed_path=(makepath("typeshed"),),
            )
            fscache = FileSystemCache()
            res = filter_out_missing_top_level_packages(
                {"a", "b", "c", "d", "e", "f", "g", "long_name", "ff", "missing"}, search, fscache
            )
            assert res == {"a", "b", "c", "d", "f", "long_name"}

    def make_file(self, base: str, path: str) -> None:
        fullpath = os.path.join(base, path)
        os.makedirs(os.path.dirname(fullpath), exist_ok=True)
        if not path.endswith("/"):
            with open(fullpath, "w") as f:
                f.write("# test file")


@unittest.skipUnless(_HAS_TYPE_KERNEL, "requires the type_kernel extension")
class NativeDaemonStableHandleSuite(unittest.TestCase):
    """Daemon-stable identity handles (issue #1528).

    A dmypy fine-grained recheck calls `_clear_native_resolvers`, which now
    takes the preserving reset: blob storage and raw ids drop while the
    strong-pin stable layer keeps the handle of every live object, so an
    astmerge-preserved Type re-registers under the handle it already had.
    An object the recheck drops loses its pin at the next preserving reset,
    once nothing else owns it (the pin was the only remaining owner).
    """

    def tearDown(self) -> None:
        if _HAS_TYPE_KERNEL:
            from mypy import types_mirror

            types_mirror.reset(clear_counts=True)

    def test_handle_survives_fine_grained_recheck(self) -> None:
        from mypy import types_mirror
        from mypy.dmypy_server import Server
        from mypy.modulefinder import BuildSource
        from mypy.nodes import Var
        from mypy.options import Options
        from mypy.types import Instance, get_proper_type

        with tempfile.TemporaryDirectory() as td:
            other_path = os.path.join(td, "other.py")
            main_path = os.path.join(td, "main.py")
            with open(other_path, "w", encoding="utf8") as f:
                f.write("a = [1]\n")
            with open(main_path, "w", encoding="utf8") as f:
                f.write("import other\nkeep = [3]\n")

            options = Options()
            options.use_builtins_fixtures = True
            options.native_type_mirror = True
            server = Server(options, os.path.join(td, "status.json"))
            sources = [
                BuildSource(main_path, "main", None),
                BuildSource(other_path, "other", None),
            ]

            def check() -> None:
                res = server.check(
                    sources, export_types=False, is_tty=False, terminal_width=-1
                )
                assert res["status"] == 0, res

            check()
            fg = server.fine_grained_manager
            assert fg is not None
            a_node = fg.manager.modules["other"].names["a"].node
            assert isinstance(a_node, Var)
            a_type = a_node.type
            assert a_type is not None
            a_handle = types_mirror._register_tree(a_type)
            assert a_handle is not None
            # A scratch family object nothing else owns: the recheck's
            # preserving sweep must retire its pin-only entry.
            proper = get_proper_type(a_type)
            assert isinstance(proper, Instance)
            scratch = proper.copy_modified()
            scratch_handle = types_mirror._register_tree(scratch)
            assert scratch_handle is not None and scratch_handle != a_handle
            del scratch
            assert types_mirror._kernel_mod.rust_mirror_stable_alive(scratch_handle)

            # Recheck: only `main` changes, so astmerge preserves `other`'s
            # objects. The same reset sweeps the pin-only scratch entry.
            with open(main_path, "w", encoding="utf8") as f:
                f.write("import other\nvalue = other.a\n")
            check()
            fg = server.fine_grained_manager
            assert fg is not None
            a_after = fg.manager.modules["other"].names["a"].node
            assert isinstance(a_after, Var)
            assert a_after.type is a_type
            # Preserved object: the stable layer kept its handle, blob
            # storage was dropped, and re-registration rebuilds it under
            # the same handle.
            assert types_mirror._kernel_mod.rust_mirror_handle_of(a_type) == a_handle
            assert types_mirror._kernel_mod.rust_mirror_stable_alive(a_handle)
            assert types_mirror._register_tree(a_type) == a_handle
            assert types_mirror._handle_of(a_type) == a_handle
            blob = types_mirror._kernel_mod.rust_mirror_bytes(a_handle)
            assert blob is not None and bytes(blob)
            # Dropped object: the pin-only entry was retired.
            assert not types_mirror._kernel_mod.rust_mirror_stable_alive(scratch_handle)
