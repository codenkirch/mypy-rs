"""Inline stub for the in-tree Rust extension ``module_resolver``.

The extension is built from ``crates/module_resolver`` and loaded as a bare
``.so`` (no PyPI package, no ``py.typed``), so mypy's self-check cannot
discover its types the way it discovers the ``ast_serialize`` wheel. This
stub mirrors the ``#[pymethods]`` surface defined in
``crates/module_resolver/src/lib.rs`` (and the ``FsCache`` pyclass in
``crates/module_resolver/src/fs_cache.rs``) and is found via ``mypy_path``.

``module_resolver`` exports two pyclasses:

* ``FsCache`` — transactional memoizing filesystem cache backing
  ``mypy.fscache.FileSystemCache``.
* ``NativeResolver`` — module resolver that reads through a shared
  ``FsCache`` instance, eliminating the dual-cache hazard.
"""

from __future__ import annotations

from typing import Optional

__all__ = ["FsCache", "NativeResolver"]

_StdlibVersionEntry = tuple[str, tuple[int, int], Optional[tuple[int, int]]]


class FsCache:
    def __init__(self) -> None: ...
    def set_package_root(self, package_root: list[str]) -> None: ...
    def flush(self) -> None: ...
    def stat_or_none(
        self, path: str
    ) -> tuple[int, int, float, int, int, int] | None: ...
    def init_under_package_root(self, path: str) -> bool: ...
    def listdir(self, path: str) -> list[str]: ...
    def isfile(self, path: str) -> bool: ...
    def isfile_case(self, path: str, prefix: str) -> bool: ...
    def exists_case(self, path: str, prefix: str) -> bool: ...
    def isdir(self, path: str) -> bool: ...
    def exists(self, path: str, real_only: bool = ...) -> bool: ...
    def read(self, path: str) -> bytes: ...
    def hash_digest(self, path: str) -> str: ...
    def samefile(self, f1: str, f2: str) -> bool: ...


class NativeResolver:
    def __init__(
        self,
        fs_cache: FsCache,
        namespace_packages: bool,
        use_builtins_fixtures: bool,
        python_path: list[str],
        mypy_path: list[str],
        package_path: list[str],
        typeshed_path: list[str],
        python_version: tuple[int, int],
        stdlib_versions: list[_StdlibVersionEntry],
        stub_flat: list[str],
        stub_namespace: list[tuple[str, str]],
    ) -> None: ...
    def flush(self) -> None: ...
    def resolve(
        self, id: str, use_typeshed: bool, follow_untyped_imports: bool
    ) -> tuple[int, Optional[str], bool]: ...
    def resolve_many(
        self, ids_with_follow: list[tuple[str, bool]]
    ) -> list[tuple[int, Optional[str], bool]]: ...
    def compute_dep_records(
        self,
        file_id: str,
        file_path: str,
        imports: list[tuple[int, str, int, list[tuple[str, Optional[str]]], int, bool, bool, bool, bool]],
        known_modules: set[str],
    ) -> tuple[list[tuple[int, str, int]], Optional[tuple[int, str]]]: ...
