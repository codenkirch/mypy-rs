"""Byte-parity tests for the Rust fixed-format cache data writer (#1566).

G0.5 ported the `mypy/nodes.py` `write()` field sets to
`ast_serialize.write_cache_data`. These tests are the frozen-legacy A/B
reference: the Python writer stays the fallback, and every module tree is
compared byte-for-byte against it. The corpus builds a small package plus
its real typeshed dependencies, so overloads, enums, NamedTuples,
TypedDicts, aliases, ParamSpec/TypeVarTuple and decorators are all
covered.

The cached-tree test touches the entry module and rebuilds against the
same cache dir: the dependency trees are then decoded from the cache and
re-encoded, which exercises the decode -> re-encode half of the round
trip plus symbol-table structural equality.
"""

from __future__ import annotations

import os
import tempfile
import unittest
from unittest import mock, skipUnless

from mypy import build
from mypy.cache import WriteBuffer
from mypy.cache_data import (
    _HAS_AST_SERIALIZE,
    _set_native_cache_data_active,
    _try_native_write_cache_data,
)
from mypy.modulefinder import BuildSource
from mypy.nodes import GDEF, MypyFile, PlaceholderNode, SymbolTable, SymbolTableNode
from mypy.options import Options
from mypy.server.astdiff import compare_symbol_table_snapshots, snapshot_symbol_table

_BASE = """\
from __future__ import annotations
import dataclasses
import enum
from typing import Callable, Generic, NamedTuple, Protocol, TypeVar, overload
from typing_extensions import ParamSpec, TypeAlias, TypeVarTuple, Unpack, dataclass_transform

T = TypeVar("T")
P = ParamSpec("P")
Ts = TypeVarTuple("Ts")

class Animal(enum.Enum):
    DOG = 0
    CAT = 1

class Point(NamedTuple):
    x: int
    y: int

class Proto(Protocol):
    def meth(self) -> int: ...

class C(Generic[T]):
    attr: int = 0
    def method(self, x: T) -> T: ...
    @overload
    def ov(self, x: int) -> int: ...
    @overload
    def ov(self, x: str) -> str: ...
    def ov(self, x): return x
    @property
    def prop(self) -> int: return 0
    @prop.setter
    def prop(self, value: int) -> None: ...
    @classmethod
    def cm(cls) -> "C[T]": ...
    @staticmethod
    def sm() -> None: ...

@dataclass_transform()
def transform(cls: type[T]) -> type[T]: ...

@transform
class Wrapped:
    value: int

Alias = list[T]

def generic_fn(*args: Unpack[Ts]) -> None: ...
def pspec_fn(f: Callable[P, int]) -> Callable[P, int]: ...
"""

_USE = """\
from __future__ import annotations
from pkg.base import Animal, C, Point

a: Animal = Animal.DOG
p: Point = Point(1, 2)
c: C[int] = C()
"""

_MAIN = """\
from __future__ import annotations
from pkg.base import C
from pkg.use import a

def main() -> None:
    x: C[str] = C()
"""

# (relative path, module id, source)
_SOURCES = [
    ("pkg/__init__.py", "pkg", ""),
    ("pkg/base.py", "pkg.base", _BASE),
    ("pkg/use.py", "pkg.use", _USE),
    ("main.py", "main", _MAIN),
]


def _python_cache_bytes(tree: MypyFile) -> bytes:
    """Legacy writer output for one tree (the frozen A/B reference)."""
    buf = WriteBuffer()
    tree.write(buf)
    return buf.getvalue()


def _first_diff(left: bytes, right: bytes) -> str:
    limit = min(len(left), len(right))
    for index in range(limit):
        if left[index] != right[index]:
            return f"byte {index}: legacy={left[index]} native={right[index]}"
    return f"lengths legacy={len(left)} native={len(right)}"


def _table_without_builtins(tree: MypyFile) -> SymbolTable:
    # Symbol tables skip `__builtins__` on write, so the decoded table
    # legitimately lacks it; compare everything else.
    table = SymbolTable()
    for key, node in tree.names.items():
        if key != "__builtins__":
            table[key] = node
    return table


@skipUnless(_HAS_AST_SERIALIZE, "requires the ast_serialize extension")
class CacheDataWriterSuite(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = self.tmp.name
        for relative, _, text in _SOURCES:
            path = os.path.join(self.root, relative)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf8") as file:
                file.write(text)
        self.cache_dir = os.path.join(self.root, ".mypy_cache")
        # The build manager sets this flag; set it explicitly so the seam
        # is testable without a prior build in the same process.
        _set_native_cache_data_active(True)

    def _options(self) -> Options:
        options = Options()
        options.show_traceback = True
        options.incremental = True
        options.cache_dir = self.cache_dir
        options.native_type_kernel = True
        # The write-cache dispatch activates from this opt-in option; a
        # build would otherwise reset the module gate set in setUp.
        options.native_cache_data = True
        options.allow_empty_bodies = True
        return options

    def _sources(self, source_texts: bool) -> list[BuildSource]:
        out = []
        for relative, module_id, text in _SOURCES:
            out.append(
                BuildSource(
                    os.path.join(self.root, relative), module_id, text if source_texts else None
                )
            )
        return out

    def _build(self, source_texts: bool) -> build.BuildResult:
        return build.build(sources=self._sources(source_texts), options=self._options())

    def test_live_tree_byte_parity(self) -> None:
        result = self._build(source_texts=True)
        checked = 0
        for module_id, tree in sorted(result.manager.modules.items()):
            legacy = _python_cache_bytes(tree)
            native = _try_native_write_cache_data(tree)
            self.assertIsNotNone(native, f"native writer deferred for {module_id}")
            assert native is not None
            self.assertEqual(
                native, legacy, f"{module_id} byte mismatch: {_first_diff(legacy, native)}"
            )
            checked += 1
        # The corpus pulls in the typeshed dependencies; a count below the
        # four sources means the build short-circuited and the parity
        # sweep would be vacuous.
        self.assertGreater(checked, len(_SOURCES))

    def test_write_cache_uses_native_writer(self) -> None:
        # `build.write_cache` imports the shim per call, so patching the
        # module attribute observes the production path.
        import mypy.cache_data as cache_data

        original = cache_data._try_native_write_cache_data
        observed: list[tuple[str, bytes | None]] = []

        def counting(tree: MypyFile) -> bytes | None:
            out = original(tree)
            observed.append((tree.fullname, out))
            return out

        with mock.patch.object(cache_data, "_try_native_write_cache_data", counting):
            result = self._build(source_texts=True)
        self.assertTrue(result.manager.modules)
        written = {name for name, _ in observed}
        for _, module_id, _ in _SOURCES:
            self.assertIn(module_id, written, f"write_cache did not serialize {module_id}")
        for fullname, out in observed:
            self.assertIsNotNone(out, f"late fallback for {fullname}")

    def test_cached_tree_round_trip_parity(self) -> None:
        first = self._build(source_texts=True)
        before = {
            module_id: snapshot_symbol_table(module_id, _table_without_builtins(tree))
            for module_id, tree in first.manager.modules.items()
            if module_id.startswith("pkg") or module_id == "main"
        }
        # Touch the entry module: run 2 rechecks it and loads the package
        # trees from the cache written by run 1.
        with open(os.path.join(self.root, "main.py"), "a", encoding="utf8") as file:
            file.write("\n# run 2\n")
        second = self._build(source_texts=False)

        checked = 0
        for module_id, tree in sorted(second.manager.modules.items()):
            legacy = _python_cache_bytes(tree)
            native = _try_native_write_cache_data(tree)
            self.assertIsNotNone(native, f"native writer deferred for cached {module_id}")
            assert native is not None
            self.assertEqual(
                native,
                legacy,
                f"cached {module_id} byte mismatch: {_first_diff(legacy, native)}",
            )
            if module_id in ("pkg.base", "pkg.use"):
                # Dependencies of the rechecked entry module come from the
                # cache; `main` itself is re-parsed.
                self.assertTrue(tree.is_cache_skeleton, f"{module_id} was not cache-loaded")
                after = snapshot_symbol_table(module_id, _table_without_builtins(tree))
                diff = compare_symbol_table_snapshots(module_id, before[module_id], after)
                self.assertFalse(diff, f"{module_id} symbol table diff: {sorted(diff)[:10]}")
                checked += 1
        self.assertEqual(checked, 2)

    def test_unsupported_shape_defers(self) -> None:
        tree = MypyFile([], [])
        tree._fullname = "mod"
        tree.names = SymbolTable()
        placeholder = PlaceholderNode("mod.p", tree, line=1)
        tree.names["p"] = SymbolTableNode(GDEF, placeholder)
        # Rust returns None (defer) and the Python writer raises its own
        # NotImplementedError, so the fallback path is the authority.
        self.assertIsNone(_try_native_write_cache_data(tree))
        buf = WriteBuffer()
        with self.assertRaises(NotImplementedError):
            tree.write(buf)
