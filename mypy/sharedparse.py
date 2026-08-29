from __future__ import annotations

from typing import Final

"""Shared logic between our three mypy parser files."""

try:
    from type_kernel import (
        rust_argument_elide_name as _rust_argument_elide_name,
        rust_special_function_elide_names as _rust_special_function_elide_names,
    )

    _SHAREDPARSE_HAS_KERNEL = True
except ImportError:
    _rust_special_function_elide_names = None  # type: ignore[assignment]
    _rust_argument_elide_name = None  # type: ignore[assignment]
    _SHAREDPARSE_HAS_KERNEL = False

_native_sharedparse_active: bool = False


def _set_native_sharedparse_active(active: bool) -> None:
    global _native_sharedparse_active
    _native_sharedparse_active = active


_NON_BINARY_MAGIC_METHODS: Final = {
    "__abs__",
    "__call__",
    "__complex__",
    "__contains__",
    "__buffer__",
    "__del__",
    "__delattr__",
    "__delitem__",
    "__enter__",
    "__exit__",
    "__float__",
    "__getattr__",
    "__getattribute__",
    "__getitem__",
    "__hex__",
    "__init__",
    "__init_subclass__",
    "__int__",
    "__invert__",
    "__iter__",
    "__len__",
    "__long__",
    "__neg__",
    "__new__",
    "__oct__",
    "__pos__",
    "__release_buffer__",
    "__repr__",
    "__reversed__",
    "__setattr__",
    "__setitem__",
    "__str__",
}

MAGIC_METHODS_ALLOWING_KWARGS: Final = {
    "__init__",
    "__init_subclass__",
    "__new__",
    "__call__",
    "__setattr__",
}

BINARY_MAGIC_METHODS: Final = {
    "__add__",
    "__and__",
    "__divmod__",
    "__eq__",
    "__floordiv__",
    "__ge__",
    "__gt__",
    "__iadd__",
    "__iand__",
    "__idiv__",
    "__ifloordiv__",
    "__ilshift__",
    "__imatmul__",
    "__imod__",
    "__imul__",
    "__ior__",
    "__ipow__",
    "__irshift__",
    "__isub__",
    "__itruediv__",
    "__ixor__",
    "__le__",
    "__lshift__",
    "__lt__",
    "__matmul__",
    "__mod__",
    "__mul__",
    "__ne__",
    "__or__",
    "__pow__",
    "__radd__",
    "__rand__",
    "__rdiv__",
    "__rfloordiv__",
    "__rlshift__",
    "__rmatmul__",
    "__rmod__",
    "__rmul__",
    "__ror__",
    "__rpow__",
    "__rrshift__",
    "__rshift__",
    "__rsub__",
    "__rtruediv__",
    "__rxor__",
    "__sub__",
    "__truediv__",
    "__xor__",
}

assert not (_NON_BINARY_MAGIC_METHODS & BINARY_MAGIC_METHODS)

MAGIC_METHODS: Final = _NON_BINARY_MAGIC_METHODS | BINARY_MAGIC_METHODS

MAGIC_METHODS_POS_ARGS_ONLY: Final = MAGIC_METHODS - MAGIC_METHODS_ALLOWING_KWARGS


def special_function_elide_names(name: str) -> bool:
    if _SHAREDPARSE_HAS_KERNEL and _native_sharedparse_active:
        try:
            return _rust_special_function_elide_names(name)
        except (AssertionError, NotImplementedError):
            pass
    return name in MAGIC_METHODS_POS_ARGS_ONLY


def argument_elide_name(name: str | None) -> bool:
    if _SHAREDPARSE_HAS_KERNEL and _native_sharedparse_active:
        try:
            return _rust_argument_elide_name(name)
        except (AssertionError, NotImplementedError):
            pass
    return name is not None and name.startswith("__") and not name.endswith("__")
