"""Inline stub for the in-tree Rust extension ``type_kernel``.

The extension is built from ``crates/type_kernel`` and loaded as a bare
``.so`` (no PyPI package, no ``py.typed``), so mypy's self-check cannot
discover its types. This stub mirrors the ``#[pyfunction]`` surface defined
in ``crates/type_kernel/src/lib.rs`` and is found via ``mypy_path``.

Stage 1 of the type-kernel migration exposes a single function,
``erase_type``, which mirrors ``mypy.erasetype.EraseTypeVisitor``. It
returns ``None`` for any type it does not handle, signalling the Python
caller to fall back to the pure-Python visitor (the strangler-fig per-call
gate).
"""

from __future__ import annotations

from typing import Optional

from mypy.types import ProperType, Type

__all__ = ["erase_type"]


def erase_type(typ: Type) -> Optional[ProperType]: ...
