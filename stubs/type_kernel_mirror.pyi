"""Native seam stubs for the mirror area (split from type_kernel.pyi, #1677)."""

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


def rust_has_type_vars(type_bytes: bytes) -> bool: ...

def rust_has_recursive_types(type_bytes: bytes) -> bool | None: ...

def rust_is_literal_type(
    type_bytes: bytes, fallback_fullname: str, value_kind: str, value_payload: str
) -> bool: ...

def rust_is_unannotated_any(type_bytes: bytes) -> bool: ...

def rust_remove_dups(type_bytes_list: list[bytes]) -> list[bytes] | None: ...

def rust_type_vars_as_args(type_bytes_list: list[bytes]) -> list[bytes]: ...

def rust_callable_with_ellipsis(
    any_bytes: bytes, ret_bytes: bytes, fallback_bytes: bytes
) -> bytes | None: ...

def rust_find_unpack_in_list(type_bytes_list: list[bytes]) -> int: ...

def rust_split_with_prefix_and_suffix(
    type_bytes_list: list[bytes], prefix: int, suffix: int
) -> tuple[list[bytes], list[bytes], list[bytes]]: ...

def rust_flatten_nested_unions(
    type_bytes_list: list[bytes],
    handle_type_alias_type: bool,
    handle_recursive: bool,
    resolver: NativeTypeResolver | None,
    row_expansions: list[bytes | None] = [],
) -> list[bytes] | None: ...

def rust_flatten_nested_tuples(
    type_bytes_list: list[bytes],
    handle_recursive: bool,
    resolver: NativeTypeResolver | None,
) -> list[bytes] | None: ...

def rust_copy_type(type_bytes: bytes) -> bytes | None: ...

def rust_has_return_statement(node_bytes: bytes) -> bool | None: ...

def rust_has_str_expression(node_bytes: bytes) -> bool: ...

def rust_has_yield_expression(node_bytes: bytes) -> bool: ...

def rust_has_yield_from_expression(node_bytes: bytes) -> bool: ...

def rust_has_await_expression(node_bytes: bytes) -> bool: ...

def rust_count_return_statements(node_bytes: bytes) -> int: ...

def rust_count_yield_expressions(node_bytes: bytes) -> int: ...

def rust_count_yield_from_expressions(node_bytes: bytes) -> int: ...

def rust_count_name_and_member_expressions(node_bytes: bytes) -> tuple[int, int]: ...

def rust_count_return_statements_and_flags(node_bytes: bytes) -> tuple[int, int]: ...

def rust_count_all_returns(node_bytes: bytes) -> int: ...

def rust_count_non_extension_handlers(node_bytes: bytes) -> int: ...

def rust_count_non_literal_handlers(node_bytes: bytes) -> int: ...

def rust_has_yield_return(node_bytes: bytes) -> bool: ...

def rust_has_complex_slice(node_bytes: bytes) -> bool: ...

def rust_is_global_expr(node_bytes: bytes) -> bool: ...

def rust_has_await_in_generator(node_bytes: bytes) -> bool: ...

def rust_node_mirror_capture_ref(
    obj: Any,
    kind: int | None,
    node_fullname: str | None,
    fullname: str,
    is_new_def: bool,
    is_inferred_def: bool,
) -> int: ...

def rust_node_mirror_capture_analyzed(obj: Any, analyzed_kind: str | None) -> int: ...

def rust_node_mirror_ref(
    handle: int,
) -> tuple[int | None, str | None, str, bool, bool] | None: ...

def rust_node_mirror_analyzed(handle: int) -> tuple[bool, str | None] | None: ...

def rust_node_mirror_captures(handle: int) -> tuple[int, int] | None: ...

def rust_node_mirror_drop(handle: int) -> bool: ...

def rust_node_mirror_reset() -> int: ...

def rust_node_mirror_entry_count() -> int: ...

def rust_node_mirror_handle_of(obj: Any) -> int | None: ...

def rust_node_mirror_capture_field_kind(obj: Any, field: str, kind: str | None) -> int: ...

def rust_node_mirror_capture_flag(obj: Any, field: str, value: bool) -> int: ...

def rust_node_mirror_capture_field_name(obj: Any, field: str, name: str | None) -> int: ...

def rust_node_mirror_capture_field_kinds(
    obj: Any, field: str, kinds: list[str | None]
) -> int: ...

def rust_node_mirror_field(handle: int, field: str) -> tuple[str, Any] | None: ...

def rust_node_mirror_fields(handle: int) -> list[str] | None: ...

def rust_node_mirror_field_captures(handle: int) -> int | None: ...

def rust_node_mirror_capture_field_wire(
    obj: Any, field: str, kind: str | None, wire: bytes
) -> int: ...

def rust_node_mirror_field_wire(
    handle: int, field: str
) -> tuple[str | None, bytes] | None: ...

def rust_node_mirror_capture_meta(
    obj: Any,
    field: str,
    kind: str,
    text: str | None = None,
    num: int | None = None,
    items: list[str] | None = None,
) -> int: ...

def rust_node_mirror_meta(
    handle: int,
) -> dict[str, tuple[str, str | None, int | None, list[str] | None]] | None: ...

def rust_node_mirror_meta_captures(handle: int) -> int | None: ...

def rust_node_mirror_meta_drop(handle: int) -> bool: ...

def rust_node_mirror_meta_reset() -> int: ...

def rust_node_mirror_meta_entry_count() -> int: ...

def rust_mirror_handle_of(obj: Any) -> int | None: ...

def rust_get_subexpressions(root: Any) -> list[Any] | None: ...

def rust_strip_ref_expr(node: Any) -> bool | None: ...

def rust_aststrip_process_lvalue(type_info: Any, lvalue: Any) -> bool | None: ...

def rust_node_mirror_capture_field_text(obj: Any, field: str, value: str) -> int: ...


__all__ = [
    "rust_has_type_vars",
    "rust_has_recursive_types",
    "rust_is_literal_type",
    "rust_is_unannotated_any",
    "rust_remove_dups",
    "rust_type_vars_as_args",
    "rust_callable_with_ellipsis",
    "rust_find_unpack_in_list",
    "rust_split_with_prefix_and_suffix",
    "rust_flatten_nested_unions",
    "rust_flatten_nested_tuples",
    "rust_copy_type",
    "rust_has_return_statement",
    "rust_has_str_expression",
    "rust_has_yield_expression",
    "rust_has_yield_from_expression",
    "rust_has_await_expression",
    "rust_count_return_statements",
    "rust_count_yield_expressions",
    "rust_count_yield_from_expressions",
    "rust_count_name_and_member_expressions",
    "rust_count_return_statements_and_flags",
    "rust_count_all_returns",
    "rust_count_non_extension_handlers",
    "rust_count_non_literal_handlers",
    "rust_has_yield_return",
    "rust_has_complex_slice",
    "rust_is_global_expr",
    "rust_has_await_in_generator",
    "rust_node_mirror_capture_ref",
    "rust_node_mirror_capture_analyzed",
    "rust_node_mirror_ref",
    "rust_node_mirror_analyzed",
    "rust_node_mirror_captures",
    "rust_node_mirror_drop",
    "rust_node_mirror_reset",
    "rust_node_mirror_entry_count",
    "rust_node_mirror_handle_of",
    "rust_node_mirror_capture_field_kind",
    "rust_node_mirror_capture_flag",
    "rust_node_mirror_capture_field_name",
    "rust_node_mirror_capture_field_kinds",
    "rust_node_mirror_field",
    "rust_node_mirror_fields",
    "rust_node_mirror_field_captures",
    "rust_node_mirror_capture_field_wire",
    "rust_node_mirror_field_wire",
    "rust_node_mirror_capture_meta",
    "rust_node_mirror_meta",
    "rust_node_mirror_meta_captures",
    "rust_node_mirror_meta_drop",
    "rust_node_mirror_meta_reset",
    "rust_node_mirror_meta_entry_count",
    "rust_mirror_handle_of",
    "rust_get_subexpressions",
    "rust_strip_ref_expr",
    "rust_aststrip_process_lvalue",
    "rust_node_mirror_capture_field_text",
]
