"""Shim for the Rust fixed-format cache data writer (G0.5, issue #1566).

`ast_serialize.write_cache_data` mirrors `MypyFile.write` and the
`mypy/nodes.py` node writers (the G0.5 symbol-node port). The Rust writer
receives the live checked tree plus four capture callbacks for the fields
that stay Python-side (type payloads, literals, JSON metadata), and returns
bytes identical to the Python writer. An unsupported shape returns `None`
and `write_cache()` runs the pure-Python writer, so the cache format is
unchanged (`CACHE_VERSION` stays 13) and a divergence cannot be written
silently.

Fine-grained incremental mode never writes cache files, so this shim only
runs on coarse builds (see `State.write_cache`).
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

from mypy.cache import WriteBuffer, write_json, write_literal

if TYPE_CHECKING:
    from mypy.nodes import MypyFile

try:
    import ast_serialize as _ast_serialize

    _HAS_AST_SERIALIZE = hasattr(_ast_serialize, "write_cache_data")
except ImportError:
    _ast_serialize = None  # type: ignore[assignment]
    _HAS_AST_SERIALIZE = False

_native_cache_data_active: bool = False
# Bound on first activation so the capture helpers avoid an import lookup
# per type payload (`mypy.types` imports mypy.nodes, so the import stays
# lazy: importing it here at module load could cycle through mypy.build).
_types: Any = None


def _set_native_cache_data_active(active: bool) -> None:
    """Enable/disable the Rust cache data writer; called by build.py."""
    global _native_cache_data_active, _types
    _native_cache_data_active = active
    if active and _types is None and _HAS_AST_SERIALIZE:
        from mypy import types

        _types = types


def _capture_type_opt(value: Any) -> bytes:
    """`mypy.types.write_type_opt` payload for one type field."""
    if value is None:
        return b"\x02"  # LITERAL_NONE
    buf = WriteBuffer()
    _types._write_type_cached(value, buf)
    return buf.getvalue()


def _capture_type_list(values: Any) -> bytes:
    """`mypy.types.write_type_list` payload for one type list field."""
    buf = WriteBuffer()
    _types.write_type_list(buf, values)
    return buf.getvalue()


def _capture_literal(value: Any) -> bytes:
    """`mypy.cache.write_literal` payload for `Var.final_value`."""
    buf = WriteBuffer()
    write_literal(buf, value)
    return buf.getvalue()


def _capture_json(value: Any) -> bytes:
    """`mypy.cache.write_json` payload for `TypeInfo.metadata`."""
    buf = WriteBuffer()
    write_json(buf, value)
    return buf.getvalue()


def _try_native_write_cache_data(tree: MypyFile) -> bytes | None:
    """Serialize a checked module tree via Rust; None falls back to Python."""
    if not (_HAS_AST_SERIALIZE and _native_cache_data_active) or _types is None:
        return None
    try:
        out: bytes | None = _ast_serialize.write_cache_data(
            tree, _capture_type_opt, _capture_type_list, _capture_literal, _capture_json
        )
    except (AssertionError, NotImplementedError, ValueError, OverflowError):
        return None
    return out
