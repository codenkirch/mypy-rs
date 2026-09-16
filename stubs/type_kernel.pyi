"""Inline stub for the in-tree Rust extension ``type_kernel``.

The extension is built from ``crates/type_kernel`` and loaded as a bare
``.so`` (no PyPI package, no ``py.typed``), so mypy's self-check cannot
discover its types. This stub mirrors the ``#[pyfunction]`` surface defined
in ``crates/type_kernel/src/lib.rs`` and is found via ``mypy_path``.

Most functions exchange serialized ``mypy.types.Type`` values as opaque
``bytes`` blobs; None signals the Python caller to fall back to the
pure-Python implementation (the strangler-fig per-call gate). Where a
function exposes an opaque PyObject handle (TruthinessOut payloads, the
resolver dict built by ``build_resolver``), the stub types it ``object``
because the Rust side does not promise a concrete Python type.

Stage 1: ``erase_type`` mirrors ``mypy.erasetype.EraseTypeVisitor``.
Stage 2: ``remove_instance_last_known_values`` mirrors
``mypy.erasetype.LastKnownValueEraser``.
"""

from __future__ import annotations

from type_kernel_checker import *  # noqa: F403
from type_kernel_messages import *  # noqa: F403
from type_kernel_mirror import *  # noqa: F403
from type_kernel_misc import *  # noqa: F403
from type_kernel_semanal import *  # noqa: F403
from type_kernel_server import *  # noqa: F403
from type_kernel_stubgen import *  # noqa: F403
from type_kernel_symtable import *  # noqa: F403
from type_kernel_types import *  # noqa: F403
