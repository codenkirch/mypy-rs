"""Tests for the experimental native mypy parser.

To run these, you will need to manually install ast_serialize from
https://github.com/mypyc/ast_serialize first (see the README for the details).
"""

from __future__ import annotations

import contextlib
import os
import tempfile
import unittest
from collections.abc import Iterator

from librt.internal import ReadBuffer

from mypy import defaults, fastparse, nodes
from mypy.cache import (
    DICT_STR_GEN,
    END_TAG,
    LIST_GEN,
    LIST_INT,
    LITERAL_INT,
    LITERAL_NONE,
    LITERAL_STR,
    LOCATION,
    read_int,
)
from mypy.config_parser import parse_mypy_comments
from mypy.errors import CompileError, Errors
from mypy.nodes import MypyFile, ParseError
from mypy.options import Options
from mypy.parse import parse
from mypy.test.data import DataDrivenTestCase, DataSuite
from mypy.test.helpers import assert_string_arrays_equal
from mypy.util import get_mypy_comments

# ast_serialize may be absent, or a pip-published upstream build that imports
# under the same name yet writes a wire format this nativeparse cannot read.
# Probe it with a full round-trip; skip the native suites on any mismatch.
try:
    from mypy.nativeparse import (
        AST_WIRE_VERSION,
        State,
        deserialize_imports,
        native_parse,
        parse_to_binary_ast,
        read_statements,
    )

    _options = Options()
    _options.python_version = defaults.PYTHON3_VERSION
    _node, _errors, _ignores = native_parse(
        "sentinel.py",
        _options,
        source=b"try:\n    x\nexcept* ValueError:\n    y\nelse:\n    z\nfinally:\n    w\n",
    )
    # Fully deserialize so a wire-format mismatch (e.g. a pip-published
    # upstream ast_serialize that imports but writes an incompatible format)
    # is caught here and the native suites are skipped instead of failing.
    _state = State(_options)
    assert _node.raw_data is not None
    _data = ReadBuffer(_node.raw_data.defs)
    _n = read_int(_data)
    _node.defs = read_statements(_state, _data, _n)
    _node.raw_data = None
    has_nativeparse = True
except Exception:
    has_nativeparse = False


class NativeParserSuite(DataSuite):
    required_out_section = True
    base_path = "."
    files = (
        ["native-parser.test", "native-parser-python311.test", "native-parser-python312.test"]
        if has_nativeparse
        else []
    )

    def run_case(self, testcase: DataDrivenTestCase) -> None:
        test_parser(testcase)


class NativeParserImportsSuite(DataSuite):
    required_out_section = True
    base_path = "."
    files = ["native-parser-imports.test"] if has_nativeparse else []

    def run_case(self, testcase: DataDrivenTestCase) -> None:
        test_parser_imports(testcase)


def test_parser(testcase: DataDrivenTestCase) -> None:
    """Perform a single native parser test case.

    The argument contains the description of the test case.
    """
    options = Options()
    options.hide_error_codes = True

    if testcase.file.endswith("python310.test"):
        options.python_version = (3, 10)
    elif testcase.file.endswith("python311.test"):
        options.python_version = (3, 11)
    elif testcase.file.endswith("python312.test"):
        options.python_version = (3, 12)
    elif testcase.file.endswith("python313.test"):
        options.python_version = (3, 13)
    elif testcase.file.endswith("python314.test"):
        options.python_version = (3, 14)
    else:
        options.python_version = defaults.PYTHON3_VERSION

    source = "\n".join(testcase.input)

    # Apply mypy: comments to options.
    comments = get_mypy_comments(source)
    changes, _ = parse_mypy_comments(comments, options)
    options = options.apply_changes(changes)

    # Check if we should skip function bodies (when ignoring errors)
    skip_function_bodies = "# mypy: ignore-errors=True" in source

    try:
        with temp_source(source) as fnam:
            node, errors, type_ignores = native_parse(fnam, options, None, skip_function_bodies)
            errors += load_tree(node, options)
            node.path = "main"
            a = node.str_with_options(options).split("\n")
            a = [format_error(err) for err in errors] + a
            a = [format_ignore(ignore) for ignore in type_ignores] + a
    except CompileError as e:
        a = e.messages
    assert_string_arrays_equal(
        testcase.output, a, f"Invalid parser output ({testcase.file}, line {testcase.line})"
    )


def format_error(err: ParseError) -> str:
    return f"{err['line']}:{err['column']}: error: {err['message']}"


def format_ignore(ignore: tuple[int, list[str]]) -> str:
    line, codes = ignore
    if not codes:
        return f"ignore: {line}"
    else:
        return f"ignore: {line} [{', '.join(codes)}]"


def load_tree(node: MypyFile, options: Options) -> list[ParseError]:
    """Deserialize full AST from serialized raw data."""
    assert node.raw_data is not None
    state = State(options)
    data = ReadBuffer(node.raw_data.defs)
    n = read_int(data)
    node.defs = read_statements(state, data, n)
    node.imports = deserialize_imports(node.raw_data.imports)
    node.raw_data = None
    return state.errors


def test_parser_imports(testcase: DataDrivenTestCase) -> None:
    """Perform a single native parser imports test case.

    The argument contains the description of the test case.
    This test outputs only reachable import information.
    """
    options = Options()
    options.hide_error_codes = True
    options.python_version = (3, 10)

    source = "\n".join(testcase.input)

    try:
        with temp_source(source) as fnam:
            node, errors, type_ignores = native_parse(fnam, options)
            errors += load_tree(node, options)
            # Extract and format reachable imports
            a = format_reachable_imports(node)
            a = [format_error(err) for err in errors] + a
    except CompileError as e:
        a = e.messages

    assert_string_arrays_equal(
        testcase.output, a, f"Invalid parser output ({testcase.file}, line {testcase.line})"
    )


def format_reachable_imports(node: MypyFile) -> list[str]:
    """Format reachable imports from a MypyFile node.

    Returns a list of strings representing reachable imports with line numbers and flags.
    """
    from mypy.nodes import Import, ImportAll, ImportFrom

    output: list[str] = []

    # Filter for reachable imports (is_unreachable == False)
    reachable_imports = [imp for imp in node.imports if not imp.is_unreachable]

    for imp in reachable_imports:
        line_num = imp.line

        # Collect flags (only show when flag is False/not set)
        flags = []
        if not imp.is_top_level:
            flags.append("not top_level")
        if imp.is_mypy_only:
            flags.append("mypy_only")

        flags_str = " [" + ", ".join(flags) + "]" if flags else ""

        if isinstance(imp, Import):
            # Format: line: import foo [as bar] [flags]
            for module_id, as_id in imp.ids:
                if as_id:
                    output.append(f"{line_num}: import {module_id} as {as_id}{flags_str}")
                else:
                    output.append(f"{line_num}: import {module_id}{flags_str}")
        elif isinstance(imp, ImportFrom):
            # Format: line: from foo import bar, baz [as b] [flags]
            # Handle relative imports
            if imp.relative > 0:
                prefix = "." * imp.relative
                if imp.id:
                    module = f"{prefix}{imp.id}"
                else:
                    module = prefix
            else:
                module = imp.id

            # Group all names together
            name_parts = []
            for name, as_name in imp.names:
                if as_name:
                    name_parts.append(f"{name} as {as_name}")
                else:
                    name_parts.append(name)

            names_str = ", ".join(name_parts)
            output.append(f"{line_num}: from {module} import {names_str}{flags_str}")
        elif isinstance(imp, ImportAll):
            # Format: line: from foo import * [flags]
            # Handle relative imports
            if imp.relative > 0:
                prefix = "." * imp.relative
                if imp.id:
                    module = f"{prefix}{imp.id}"
                else:
                    module = prefix
            else:
                module = imp.id

            output.append(f"{line_num}: from {module} import *{flags_str}")

    return output


def _int_enc(n: int) -> int:
    return (n + 10) << 1


def _locs(start_line: int, start_column: int, end_line: int, end_column: int) -> list[int]:
    return [
        LOCATION,
        _int_enc(start_line),
        _int_enc(start_column),
        _int_enc(end_line - start_line),
        _int_enc(end_column - start_column),
    ]


@unittest.skipUnless(has_nativeparse, "nativeparse not available")
class TestNativeParserBinaryFormat(unittest.TestCase):
    def _assert_trivial_binary_data(self, b: bytes, /) -> None:
        # A quick sanity check to ensure the serialized data looks as expected. Only covers
        # a few AST nodes.
        self.assertEqual(
            list(b),
            (
                [LITERAL_INT, 22, nodes.EXPR_STMT, nodes.CALL_EXPR]
                + [nodes.NAME_EXPR, LITERAL_STR]
                + [_int_enc(5)]
                + list(b"print")
                + _locs(1, 0, 1, 5)
                + [END_TAG, LIST_GEN, 22, nodes.STR_EXPR]
                + [LITERAL_STR, _int_enc(5)]
                + list(b"hello")
                + [0]  # corrupted flag (unescaped surrogate escapes present)
                + _locs(1, 6, 1, 13)
                + [END_TAG]
                # arg_kinds: [ARG_POS]
                + [LIST_INT, 22, _int_enc(0)]
                # arg_names: [None]
                + [LIST_GEN, 22, LITERAL_NONE]
                + _locs(1, 0, 1, 14)
                + [END_TAG]
                + _locs(1, 0, 1, 14)
                + [END_TAG]
            ),
        )

    def test_trivial_binary_data_from_file(self) -> None:
        with temp_source("print('hello')") as fnam:
            b, _, _, _, _, _, _, _ = parse_to_binary_ast(fnam, Options())
            self._assert_trivial_binary_data(b)

    def test_trivial_binary_data_from_string_source(self) -> None:
        b, _, _, _, _, _, _, _ = parse_to_binary_ast("", Options(), "print('hello')")
        self._assert_trivial_binary_data(b)

    def test_trivial_binary_data_from_bytes_source(self) -> None:
        b, _, _, _, _, _, _, _ = parse_to_binary_ast("", Options(), b"print('hello')")
        self._assert_trivial_binary_data(b)

    def test_invalid_bytes_raises(self) -> None:
        with self.assertRaises(UnicodeDecodeError):
            parse_to_binary_ast("", Options(), b"\xff")

    def test_v5_golden_func_def_docstring(self) -> None:
        # Pins the v5 FUNC_DEF_STMT record layout: the docstring field follows
        # the name (present flag, value, corrupted flag) before the parameters.
        options = Options()
        options.include_docstrings = True
        b, _, _, _, _, _, _, _ = parse_to_binary_ast("", options, 'def f():\n    "d"\n')
        self.assertEqual(
            list(b),
            [LITERAL_INT, _int_enc(1), nodes.FUNC_DEF_STMT, LITERAL_STR, _int_enc(1)]
            + list(b"f")
            + [1, LITERAL_STR, _int_enc(1)]
            + list(b"d")
            + [0]
            + [LIST_GEN, _int_enc(0)]  # parameters
            + [nodes.BLOCK, LIST_GEN, _int_enc(1), 0]
            + [nodes.EXPR_STMT, nodes.STR_EXPR, LITERAL_STR, _int_enc(1)]
            + list(b"d")
            + [0]
            + _locs(2, 4, 2, 7)
            + [END_TAG]
            + _locs(2, 4, 2, 7)
            + [END_TAG]
            + [END_TAG]
            + [0, 0, 0]  # is_async, has_type_params, has_return_type
            + _locs(1, 0, 2, 7)
            + [END_TAG],
        )

    def test_v5_golden_class_def_docstring(self) -> None:
        options = Options()
        options.include_docstrings = True
        b, _, _, _, _, _, _, _ = parse_to_binary_ast("", options, 'class C:\n    "d"\n')
        self.assertEqual(
            list(b),
            [LITERAL_INT, _int_enc(1), nodes.CLASS_DEF, LITERAL_STR, _int_enc(1)]
            + list(b"C")
            + [1, LITERAL_STR, _int_enc(1)]
            + list(b"d")
            + [0]
            + [nodes.BLOCK, LIST_GEN, _int_enc(1), 0]
            + [nodes.EXPR_STMT, nodes.STR_EXPR, LITERAL_STR, _int_enc(1)]
            + list(b"d")
            + [0]
            + _locs(2, 4, 2, 7)
            + [END_TAG]
            + _locs(2, 4, 2, 7)
            + [END_TAG]
            + [END_TAG]
            + [LIST_GEN, _int_enc(0)]  # base type expressions
            + [LIST_GEN, _int_enc(0)]  # decorators
            + [0]  # has_type_params
            + [DICT_STR_GEN, _int_enc(0)]  # keywords
            + _locs(1, 0, 2, 7)
            + [END_TAG],
        )


def _collect_docstrings(tree: MypyFile) -> dict[tuple[int, str, str], str | None]:
    docstrings: dict[tuple[int, str, str], str | None] = {}

    def visit(stmts: list[nodes.Statement]) -> None:
        for stmt in stmts:
            if isinstance(stmt, nodes.FuncDef):
                docstrings[(stmt.line, "func", stmt.name)] = stmt.docstring
            elif isinstance(stmt, nodes.ClassDef):
                docstrings[(stmt.line, "class", stmt.name)] = stmt.docstring
                visit(stmt.defs.body)

    visit(tree.defs)
    return docstrings


def _collect_imports(tree: MypyFile) -> list[tuple[object, ...]]:
    shapes: list[tuple[object, ...]] = []
    for imp in tree.imports:
        if isinstance(imp, nodes.Import):
            shapes.append(("Import", tuple(imp.ids)))
        elif isinstance(imp, nodes.ImportFrom):
            shapes.append(("ImportFrom", imp.id, imp.relative, tuple(imp.names)))
        elif isinstance(imp, nodes.ImportAll):
            shapes.append(("ImportAll", imp.id, imp.relative))
    for stmt in tree.defs:
        if isinstance(stmt, nodes.Import):
            shapes.append(("stmt Import", tuple(stmt.ids)))
        elif isinstance(stmt, nodes.ImportFrom):
            shapes.append(("stmt ImportFrom", stmt.id, stmt.relative, tuple(stmt.names)))
        elif isinstance(stmt, nodes.ImportAll):
            shapes.append(("stmt ImportAll", stmt.id, stmt.relative))
    return shapes


def _collect_argument_positions(tree: MypyFile) -> dict[str, list[tuple[str, bool]]]:
    positions: dict[str, list[tuple[str, bool]]] = {}

    def visit(stmts: list[nodes.Statement]) -> None:
        for stmt in stmts:
            if isinstance(stmt, nodes.FuncDef):
                positions[stmt.name] = [(a.variable.name, a.pos_only) for a in stmt.arguments]
            elif isinstance(stmt, nodes.ClassDef):
                visit(stmt.defs.body)

    visit(tree.defs)
    return positions


@unittest.skipUnless(has_nativeparse, "nativeparse not available")
class TestNativeParserOptionParity(unittest.TestCase):
    """Differential checks against the CPython-based parser for wire-v5 options."""

    def _parse_both(self, source: str, options: Options) -> tuple[MypyFile, MypyFile]:
        errors = Errors(options)
        fast = fastparse.parse(
            bytes(source, "utf-8"), fnam="main", module="main", errors=errors, options=options
        )
        native, _errors, _ignores = native_parse("main", options, source)
        load_tree(native, options)
        return fast, native

    def test_docstrings_match_fastparse(self) -> None:
        cases = [
            'def f():\n    """func doc"""\n    ...\n',
            'class C:\n    """class doc"""\n    def m(self):\n        """method doc"""\n        ...\n',
            'class C:\n    r"""raw \\" doc"""\n',
            'class C:\n    "implicit" " concat"\n',
            'class C:\n    b"bytes"\n',
            'class C:\n    f"fstring"\n',
            "class C:\n    1\n",
            "class C:\n    '''triple\n    line'''\n",
            'class C:\n    "esc \\n tab\\t unicode \\u263a"\n',
            'class C:\n    "\\ud800"\n',
        ]
        for include in (True, False):
            for source in cases:
                options = Options()
                options.python_version = (3, 13)
                options.include_docstrings = include
                fast, native = self._parse_both(source, options)
                self.assertEqual(
                    _collect_docstrings(fast), _collect_docstrings(native), (include, source)
                )

    def test_custom_typing_module_translation(self) -> None:
        source = (
            "import foo\n"
            "import foo as bar\n"
            "import foo.baz\n"
            "from foo import T\n"
            "from foo import T as U\n"
            "from foo import *\n"
            "from .foo import y\n"
            "from typing import Any\n"
        )
        for custom in (None, "foo"):
            options = Options()
            options.python_version = (3, 13)
            options.custom_typing_module = custom
            fast, native = self._parse_both(source, options)
            self.assertEqual(_collect_imports(fast), _collect_imports(native), custom)

    def test_pos_only_special_methods_option(self) -> None:
        source = (
            "def f(__x, x): ...\n"
            "class C:\n"
            "    def __init__(self, x): ...\n"
            "    def __getattr__(self, name): ...\n"
            "    def __str__(obj): ...\n"
            "def g(a, /, b, *, c): ...\n"
        )
        for enabled in (True, False):
            options = Options()
            options.python_version = (3, 13)
            options.pos_only_special_methods = enabled
            fast, native = self._parse_both(source, options)
            self.assertEqual(
                _collect_argument_positions(fast), _collect_argument_positions(native), enabled
            )

    def test_transform_source_applied_on_native_branch(self) -> None:
        def transform(source: str | bytes) -> str | bytes:
            text = source if isinstance(source, str) else source.decode()
            return text.replace("1", "'transformed'")

        options = Options()
        options.native_parser = True
        options.python_version = (3, 13)
        options.transform_source = transform

        tree = parse(
            "x = 1\n",
            fnam="main",
            module="main",
            errors=Errors(options),
            options=options,
            eager=True,
        )
        stmt = tree.defs[0]
        assert isinstance(stmt, nodes.AssignmentStmt)
        assert isinstance(stmt.rvalue, nodes.StrExpr)
        assert stmt.rvalue.value == "transformed"

        # The build path passes source=None; the native branch must read the
        # file and still apply the transform (stubgen's semantic pass).
        with temp_source("x = 1\n") as fnam:
            tree = parse(
                None, fnam=fnam, module="main", errors=Errors(options), options=options, eager=True
            )
        stmt = tree.defs[0]
        assert isinstance(stmt, nodes.AssignmentStmt)
        assert isinstance(stmt.rvalue, nodes.StrExpr)
        assert stmt.rvalue.value == "transformed"

    def test_wire_version_mismatch_rejected(self) -> None:
        import ast_serialize

        with self.assertRaises(RuntimeError) as ctx:
            ast_serialize.parse("t.py", "x = 1\n", cache_version=AST_WIRE_VERSION - 1)
        self.assertIn("wire version mismatch", str(ctx.exception))
        result = ast_serialize.parse("t.py", "x = 1\n", cache_version=AST_WIRE_VERSION)
        self.assertEqual(result[4]["ast_wire_version"], AST_WIRE_VERSION)


@contextlib.contextmanager
def temp_source(text: str) -> Iterator[str]:
    with tempfile.TemporaryDirectory() as temp_dir:
        temp_path = os.path.join(temp_dir, "t.py")
        with open(temp_path, "w") as f:
            f.write(text)
        yield temp_path
