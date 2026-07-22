from __future__ import annotations

from collections.abc import Callable
from typing import Any

from mypy.nodes import TypeInfo
from mypy.types import Instance
from mypy.typestate import type_state

# Stage 5 type-kernel seam: when the `type_kernel` Rust extension is
# importable and `Options.native_type_kernel` is set, route the pure C3
# linearization through Rust. The Rust path returns `None` for any case it
# does not handle (cycles, a base missing from the snapshot, the `obj_type`
# callback edge at mro.py:34, or an inconsistent merge), in which case we
# fall back to the pure-Python implementation. This is the strangler-fig
# per-call gate, mirroring `erasetype.py` (Stage 1), `subtypes.py` (Stage
# 3c), and `argmap.py` (Stage 4): no behavior change unless the option is
# set, and even then unsupported cases degrade gracefully.
try:
    from type_kernel import rust_linearize_hierarchy as _rust_linearize_hierarchy

    _HAS_TYPE_KERNEL = True
except ImportError:
    _rust_linearize_hierarchy = None  # type: ignore[assignment]
    _HAS_TYPE_KERNEL = False

# Module-level flag + resolver + fullname map, set by the build manager
# from `Options.native_type_kernel` at the start of each build. The hot path
# reads these without an options lookup per call. When `_native_mro_active`
# is True but `_native_mro_resolver` or `_native_mro_typeinfo_map` is None,
# the shim falls through to Python (the resolver isn't wired yet, e.g. in
# tests that only set the flag).
_native_mro_active: bool = False
_native_mro_resolver: Any = None
_native_mro_typeinfo_map: dict[str, TypeInfo] | None = None


def _set_native_mro_active(active: bool) -> None:
    """Called by the build manager to enable/disable the Rust MRO path."""
    global _native_mro_active
    _native_mro_active = active


def _set_native_mro_resolver(resolver: Any, typeinfo_map: dict[str, TypeInfo] | None) -> None:
    """Install the `NativeTypeResolver` pyclass and the fullname -> TypeInfo
    map for the Rust MRO path.

    Called by the build manager (or the parity test suite) after building
    the resolver from the live TypeInfo graph. Pass `None` for the resolver
    to clear. The `typeinfo_map` is built from the same TypeInfo list passed
    to `build_native_resolver`; Rust returns a list of fullnames and Python
    converts each back to a live TypeInfo via this map before assigning
    `info.mro`.
    """
    global _native_mro_resolver, _native_mro_typeinfo_map
    _native_mro_resolver = resolver
    _native_mro_typeinfo_map = typeinfo_map


def calculate_mro(info: TypeInfo, obj_type: Callable[[], Instance] | None = None) -> None:
    """Calculate and set mro (method resolution order).

    Raise MroError if cannot determine mro.
    """
    mro = _rust_or_python_mro(info, obj_type)
    assert mro, f"Could not produce a MRO at all for {info}"
    info.mro = mro
    # The property of falling back to Any is inherited.
    info.fallback_to_any = any(baseinfo.fallback_to_any for baseinfo in info.mro)
    type_state.reset_all_subtype_caches_for(info)


def _rust_or_python_mro(
    info: TypeInfo, obj_type: Callable[[], Instance] | None = None
) -> list[TypeInfo] | None:
    """Return the MRO for `info`, preferring Rust, falling back to Python.

    Returns None only when the Rust path declined AND the Python path
    raised `MroError` (which `calculate_mro` turns into the `assert mro`
    failure mirroring the original). In practice the Python path always
    returns a list (it raises MroError for inconsistency, which we let
    propagate), so the None return is unreachable; the signature keeps
    the type honest for the Rust-decline + Python-MroError path.
    """
    if (
        _HAS_TYPE_KERNEL
        and _native_mro_active
        and _native_mro_resolver is not None
        and _native_mro_typeinfo_map is not None
        and not info.mro  # mro.py:31: skip Rust for a cached MRO; Python
        # short-circuits it too, so calling Rust would be wasted work and
        # would re-walk bases the snapshot already linearized.
    ):
        # `obj_type` is only used by the `not bases and info.fullname !=
        # "builtins.object"` edge (mro.py:34). Rust has no callback, so it
        # returns None for that case and we fall through to Python below.
        result = _rust_linearize_hierarchy(_native_mro_resolver, info.fullname)
        if result is not None:
            mro = _fullnames_to_typeinfos(result, info)
            if mro is not None:
                return mro
            # A fullname was missing from the map (stale resolver); fall
            # through to the pure-Python path, which rebuilds from the live
            # graph.
        # Rust declined (cycle, missing base, obj_type edge, inconsistent
        # merge); fall through to Python, which raises the real MroError on
        # inconsistency.
    return linearize_hierarchy(info, obj_type)


def _fullnames_to_typeinfos(
    fullnames: list[str], info: TypeInfo
) -> list[TypeInfo] | None:
    """Convert a Rust-returned fullname list back to live TypeInfo objects.

    Returns None if any fullname is absent from the installed map, so the
    caller falls through to the pure-Python path. The map is built from the
    same TypeInfo list passed to `build_native_resolver`, so every base that
    the Rust snapshot could resolve should be present; a miss means the
    resolver is stale relative to the live graph (defer to Python).
    """
    assert _native_mro_typeinfo_map is not None  # checked by caller
    mro: list[TypeInfo] = []
    for fullname in fullnames:
        baseinfo = _native_mro_typeinfo_map.get(fullname)
        if baseinfo is None:
            # Defensive: the entry's own fullname should always be in the
            # map (it was passed to build_native_resolver), but a stale
            # resolver can miss a freshly-loaded module.
            return None
        mro.append(baseinfo)
    return mro


class MroError(Exception):
    """Raised if a consistent mro cannot be determined for a class."""


def linearize_hierarchy(
    info: TypeInfo, obj_type: Callable[[], Instance] | None = None
) -> list[TypeInfo]:
    # TODO describe
    if info.mro:
        return info.mro
    bases = info.direct_base_classes()
    if not bases and info.fullname != "builtins.object" and obj_type is not None:
        # Probably an error, add a dummy `object` base class,
        # otherwise MRO calculation may spuriously fail.
        bases = [obj_type().type]
    lin_bases = []
    for base in bases:
        assert base is not None, f"Cannot linearize bases for {info.fullname} {bases}"
        lin_bases.append(linearize_hierarchy(base, obj_type))
    lin_bases.append(bases)
    return [info] + merge(lin_bases)


def merge(seqs: list[list[TypeInfo]]) -> list[TypeInfo]:
    seqs = [s.copy() for s in seqs]
    result: list[TypeInfo] = []
    while True:
        seqs = [s for s in seqs if s]
        if not seqs:
            return result
        for seq in seqs:
            head = seq[0]
            if not [s for s in seqs if head in s[1:]]:
                break
        else:
            raise MroError()
        result.append(head)
        for s in seqs:
            if s[0] is head:
                del s[0]
