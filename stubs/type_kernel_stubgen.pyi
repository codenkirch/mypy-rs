"""Native seam stubs for the stubgen area (split from type_kernel.pyi, #1677)."""

from __future__ import annotations

from collections.abc import Callable, Sized
from typing import Any, Literal, TypeVar

from mypy.nodes import (
    AssignmentStmt,
    Block,
    CallExpr,
    DataclassTransformSpec,
    Decorator,
    Expression,
    FuncDef,
    Lvalue,
    MemberExpr,
    MypyFile,
    NameExpr,
    Node,
    OverloadedFuncDef,
    RefExpr,
    SymbolNode,
    SymbolTable,
    SymbolTableNode,
    TypeAlias,
    TypeInfo,
    Var,
)
from mypy.types import CallableType, Instance, ProperType, TupleType, Type, TypeVarLikeType


T = TypeVar("T")


def rust_stubgen_render(expr: Expression) -> str | None: ...

def rust_stubgen_render_type_args(items: list[Expression]) -> str | None: ...

def rust_get_assigned_names(lvalues: list[Expression]) -> list[str]: ...

def rust_is_none_expr(expr: Expression) -> bool: ...

def rust_is_pybind11_overloaded_function_docstring(docstring: str, name: str) -> bool: ...

def rust_method_name_sort_key(name: str) -> tuple[int, str]: ...

def rust_stubgen_get_qualified_name(node: Expression) -> str: ...

def rust_stubgen_str_type_tag(node: Expression, can_be_incomplete: bool) -> int | None: ...

def rust_stubgen_str_default(node: Expression) -> tuple[str, bool] | None: ...

def rust_is_generator_return_type(
    typ_bytes: bytes, is_coroutine: bool, strict_optional: bool, resolver: NativeTypeResolver
) -> bool | None: ...

def rust_is_async_generator_return_type(
    typ_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bool | None: ...

def rust_get_generator_yield_type(
    return_type_bytes: bytes,
    is_coroutine: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bytes | None: ...

def rust_get_generator_receive_type(
    return_type_bytes: bytes,
    is_coroutine: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bytes | None: ...

def rust_get_generator_return_type(
    return_type_bytes: bytes,
    is_coroutine: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bytes | None: ...

def rust_get_coroutine_return_type(return_type_bytes: bytes) -> bytes | None: ...


__all__ = [
    "rust_stubgen_render",
    "rust_stubgen_render_type_args",
    "rust_get_assigned_names",
    "rust_is_none_expr",
    "rust_is_pybind11_overloaded_function_docstring",
    "rust_method_name_sort_key",
    "rust_stubgen_get_qualified_name",
    "rust_stubgen_str_type_tag",
    "rust_stubgen_str_default",
    "rust_is_generator_return_type",
    "rust_is_async_generator_return_type",
    "rust_get_generator_yield_type",
    "rust_get_generator_receive_type",
    "rust_get_generator_return_type",
    "rust_get_coroutine_return_type",
]
