from __future__ import annotations

from typing import cast

from mypy.erasetype import erase_typevars
from mypy.nodes import TypeInfo
from mypy.types import (
    Instance,
    ParamSpecType,
    ProperType,
    TupleType,
    Type,
    TypeOfAny,
    TypeVarLikeType,
    TypeVarTupleType,
    TypeVarType,
    UnpackType,
    read_type,
)
from mypy.typevartuples import erased_vars

# Stage 6c type-kernel seam: when type_kernel is importable and the gate
# is active, has_no_typevars and fill_typevars route through Rust. Rust
# returns None for unsupported cases; we fall back to Python.
try:
    import type_kernel as _type_kernel
    from librt.internal import ReadBuffer as _ReadBuffer, WriteBuffer as _WriteBuffer

    _HAS_TYPE_KERNEL = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _ReadBuffer = None  # type: ignore[assignment,misc]
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _HAS_TYPE_KERNEL = False

_native_typevars_active: bool = False


def _set_native_typevars_active(active: bool) -> None:
    global _native_typevars_active
    _native_typevars_active = active


# bytes -> decoded wire-Type cache for the typevars seam.  Byte-identical
# blobs repeat heavily (~97% of 90K calls), so memoizing read_type +
# fixup_wire_type cuts decode cost.  Mirrors other deser caches.
_typevars_decode_cache: dict[bytes, ProperType] = {}


def _clear_typevars_decode_cache() -> None:
    _typevars_decode_cache.clear()


def _native_decode_well_formed(data: bytes) -> ProperType | None:
    """Decode wire bytes for the fill_typevars seam.

    Mirrors mypy/typeops.py::_deserialize_type plus a check_no_fake_info
    guard: the global wire typeinfo map can hold stale or fake entries
    across fine-grained refreshes, so a decoded tree with any residual
    fake TypeInfo defers to Python. A non-None result is structurally a
    proper-type tree: this seam only passes fully-resolved Rust-built
    types, and fixup_wire_type defers on any TypeAliasType.
    """
    cached = _typevars_decode_cache.get(data)
    if cached is not None:
        return cached
    from mypy.types import instance_cache
    from mypy.wirefixup import check_no_fake_info, fixup_wire_type

    decoded = read_type(_ReadBuffer(data))
    # Clear instance_cache primitives after read_type so NOT_READY
    # singletons cannot leak into later builds (mirrors _typeanal_decode).
    instance_cache.int_type = None
    instance_cache.str_type = None
    instance_cache.bool_type = None
    instance_cache.object_type = None
    instance_cache.function_type = None
    fixed = fixup_wire_type(decoded)
    if fixed is None or not check_no_fake_info(fixed):
        return None
    proper = cast(ProperType, fixed)
    _typevars_decode_cache[data] = proper
    return proper


def fill_typevars(typ: TypeInfo) -> Instance | TupleType:
    """For a non-generic type, return instance type representing the type.

    For a generic G type with parameters T1, .., Tn, return G[T1, ..., Tn].
    """
    # Native seam: Rust yields only the rebuilt tvar-arg list via the
    # wire round-trip; the root Instance (and named-tuple wrapper) are
    # rebuilt on the live `typ` so a stale wire-map entry cannot leak.
    if _HAS_TYPE_KERNEL and _native_typevars_active:
        try:
            result = _type_kernel.rust_fill_typevars(typ)
            if result is not None:
                decoded = _native_decode_well_formed(bytes(result))
                if decoded is not None and isinstance(decoded, (Instance, TupleType)):
                    root = decoded if isinstance(decoded, Instance) else decoded.partial_fallback
                    inst = Instance(typ, root.args)
                    if typ.tuple_type is None:
                        return inst
                    return typ.tuple_type.copy_modified(fallback=inst)
        except (AssertionError, NotImplementedError, ValueError):
            pass
    tvs: list[Type] = []
    # TODO: why do we need to keep both typ.type_vars and typ.defn.type_vars?
    for i in range(len(typ.defn.type_vars)):
        tv: TypeVarLikeType | UnpackType = typ.defn.type_vars[i]
        # Change the line number
        if isinstance(tv, TypeVarType):
            tv = tv.copy_modified(line=-1, column=-1)
        elif isinstance(tv, TypeVarTupleType):
            tv = UnpackType(
                TypeVarTupleType(
                    tv.name,
                    tv.fullname,
                    tv.id,
                    tv.upper_bound,
                    tv.tuple_fallback,
                    tv.default,
                    line=-1,
                    column=-1,
                )
            )
        else:
            assert isinstance(tv, ParamSpecType)
            tv = ParamSpecType(
                tv.name,
                tv.fullname,
                tv.id,
                tv.flavor,
                tv.upper_bound,
                tv.default,
                line=-1,
                column=-1,
            )
        tvs.append(tv)
    inst = Instance(typ, tvs)
    # TODO: do we need to also handle typeddict_type here and below?
    if typ.tuple_type is None:
        return inst
    return typ.tuple_type.copy_modified(fallback=inst)


def fill_typevars_with_any(typ: TypeInfo) -> Instance | TupleType:
    """Apply a correct number of Any's as type arguments to a type."""
    # Native seam: Rust builds the erased Instance (and the named-tuple
    # variant) on the live `typ`; a decoded TupleType proves Rust ran the
    # tuple-erasure predicate, so its fallback carries the erased args.
    if _HAS_TYPE_KERNEL and _native_typevars_active:
        try:
            result = _type_kernel.rust_fill_typevars_with_any(typ)
            if result is not None:
                decoded = _native_decode_well_formed(bytes(result))
                if decoded is not None:
                    if isinstance(decoded, TupleType):
                        inst = Instance(typ, decoded.partial_fallback.args)
                        if typ.tuple_type is not None:
                            return typ.tuple_type.copy_modified(fallback=inst)
                    elif isinstance(decoded, Instance):
                        return Instance(typ, decoded.args)
        except (AssertionError, NotImplementedError, ValueError):
            pass
    inst = Instance(typ, erased_vars(typ.defn.type_vars, TypeOfAny.special_form))
    if typ.tuple_type is None:
        return inst
    erased_tuple_type = erase_typevars(typ.tuple_type, {tv.id for tv in typ.defn.type_vars})
    assert isinstance(erased_tuple_type, ProperType)
    if isinstance(erased_tuple_type, TupleType):
        return typ.tuple_type.copy_modified(fallback=inst)
    return inst


def has_no_typevars(typ: Type) -> bool:
    # Test if type contains type variables by erasing and comparing.
    # We use equality comparison __eq__ defined for types.
    # Note: cannot use is_same_type or identity 'is' comparison.
    if _HAS_TYPE_KERNEL and _native_typevars_active:
        try:
            buf = _WriteBuffer()
            typ.write(buf)
            result = _type_kernel.rust_has_no_typevars(buf.getvalue())
            if result is not None:
                return result
        except (NotImplementedError, AssertionError):
            pass
    return typ == erase_typevars(typ)
