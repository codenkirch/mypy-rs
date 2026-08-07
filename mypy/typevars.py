from __future__ import annotations

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
# is active, has_no_typevars routes through Rust. Rust returns None for
# unsupported cases (TypeAliasType, UnboundType); we fall back to Python.
try:
    import type_kernel as _type_kernel
    from librt.internal import ReadBuffer as _ReadBuffer
    from librt.internal import WriteBuffer as _WriteBuffer

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


def fill_typevars(typ: TypeInfo) -> Instance | TupleType:
    """For a non-generic type, return instance type representing the type.

    For a generic G type with parameters T1, .., Tn, return G[T1, ..., Tn].
    """
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
