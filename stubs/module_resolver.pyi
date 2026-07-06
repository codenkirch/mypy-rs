"""Inline stub for the in-tree Rust extension ``module_resolver``.

The extension is built from ``crates/module_resolver`` and loaded as a bare
``.so`` (no PyPI package, no ``py.typed``), so mypy's self-check cannot
discover its types the way it discovers the ``ast_serialize`` wheel. This
stub mirrors the ``#[pymethods]`` surface defined in
``crates/module_resolver/src/lib.rs`` and is found via ``mypy_path``.
"""

from __future__ import annotations

from typing import Optional

__all__ = ["NativeResolver"]

_StdlibVersionEntry = tuple[str, tuple[int, int], Optional[tuple[int, int]]]

class NativeResolver:
    def __init__(
        self,
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
    def resolve(
        self, id: str, use_typeshed: bool, follow_untyped_imports: bool
    ) -> tuple[int, Optional[str], bool]: ...
    def compute_dep_records(
        self,
        file_id: str,
        file_path: str,
        imports: list[tuple[int, str, int, list[tuple[str, Optional[str]]], int, bool, bool, bool, bool]],
        known_modules: set[str],
    ) -> tuple[list[tuple[int, str, int]], Optional[tuple[int, str]]]: ...
