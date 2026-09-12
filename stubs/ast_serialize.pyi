"""Inline stub for the in-tree Rust extension ``ast_serialize``.

The extension is built from ``crates/ast_serialize`` and writes AST wire v5:
the parse entry enforces ``cache_version == 5``, the data dict carries
``ast_wire_version``, and the docstring and custom-typing-module options are
threaded through. The PyPI ``ast-serialize`` wheel pinned in pyproject still
declares the older v4 surface, so mypy's self-check resolves the current
surface here via ``mypy_path`` (same pattern as ``module_resolver.pyi``).
"""

from __future__ import annotations

from typing import TypeAlias, TypedDict, type_check_only
from typing_extensions import NotRequired

__all__ = ["parse"]

_TypeIgnores: TypeAlias = list[tuple[int, list[str]]]

@type_check_only
class ParseError(TypedDict):
    line: int
    column: int
    message: str
    blocker: NotRequired[bool]
    code: NotRequired[str]

@type_check_only
class _ASTData(TypedDict):
    ast_wire_version: int
    is_partial_package: bool
    uses_template_strings: bool
    mypy_ignores: _TypeIgnores
    source_hash: str
    mypy_comments: list[tuple[int, str]]

def parse(
    fnam: str,
    source: str | bytes | None = None,
    skip_function_bodies: bool = False,
    python_version: tuple[int, int] | None = None,
    platform: str | None = None,
    always_true: list[str] | None = None,
    always_false: list[str] | None = None,
    cache_version: int = 0,
    include_docstrings: bool = False,
    custom_typing_module: str | None = None,
) -> tuple[bytes, list[ParseError], _TypeIgnores, bytes, _ASTData]: ...
