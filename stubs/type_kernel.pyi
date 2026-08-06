"""Inline stub for the in-tree Rust extension ``type_kernel``.

The extension is built from ``crates/type_kernel`` and loaded as a bare
``.so`` (no PyPI package, no ``py.typed``), so mypy's self-check cannot
discover its types. This stub mirrors the ``#[pyfunction]`` surface defined
in ``crates/type_kernel/src/lib.rs`` and is found via ``mypy_path``.

Stage 1: ``erase_type`` mirrors ``mypy.erasetype.EraseTypeVisitor``.
Stage 2: ``remove_instance_last_known_values`` mirrors
``mypy.erasetype.LastKnownValueEraser``.

Both return ``None`` for any type they do not handle, signalling the Python
caller to fall back to the pure-Python visitor (the strangler-fig per-call
gate).
"""

from __future__ import annotations

from typing import Optional

from mypy.types import ProperType, Type

__all__ = [
    "erase_type",
    "remove_instance_last_known_values",
    "rust_is_unreachable_map",
    "rust_stmt_outcome",
    "rust_type_requires_usage",
]


def erase_type(typ: Type) -> Optional[ProperType]: ...


def remove_instance_last_known_values(typ: Type) -> Optional[Type]: ...


def rust_type_requires_usage(type_bytes: bytes) -> Optional[int]: ...


def rust_is_unreachable_map(type_bytes_list: list[bytes]) -> Optional[bool]: ...


def rust_stmt_outcome(node_bytes: bytes) -> Optional[str]: ...
