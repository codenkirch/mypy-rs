"""Native seam stubs for the misc area (split from type_kernel.pyi, #1677)."""

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



# Issue #533: pure utility functions from util.py
def rust_is_dunder(name: str, exclude_special: bool = ...) -> bool: ...

def rust_is_sunder(name: str) -> bool: ...

def rust_split_module_names(mod_name: str) -> list[str]: ...

def rust_module_prefix(modules: list[str], target: str) -> str | None: ...

def rust_split_target(modules: list[str], target: str) -> tuple[str, str] | None: ...

def rust_short_type(obj: object) -> str: ...

def rust_find_python_encoding(text: bytes) -> tuple[str, int]: ...

def rust_bytes_to_human_readable_repr(b: bytes) -> str: ...

def rust_decode_python_encoding(source: bytes) -> str: ...

def rust_trim_source_line(
    line: str, max_len: int, col: int, min_width: int
) -> tuple[str, int]: ...

def rust_get_mypy_comments(source: str) -> list[tuple[int, str]]: ...

def rust_get_prefix(fullname: str) -> str: ...

def rust_correct_relative_import(
    cur_mod_id: str, relative: int, target: str, is_cur_package_init_file: bool
) -> tuple[str, bool]: ...

def rust_unmangle(name: str) -> str: ...

def rust_get_unique_redefinition_name(name: str, existing: list[str]) -> str: ...

def rust_split_words(msg: str) -> list[str]: ...

def rust_soft_wrap(msg: str, max_len: int, first_offset: int, num_indent: int = ...) -> str: ...

def rust_hash_digest(data: bytes) -> str: ...

def rust_hash_digest_bytes(data: bytes) -> bytes: ...

def rust_hash_path_stem(s: str) -> int: ...

def rust_is_sub_path_normabs(path: str, dir: str) -> bool: ...

def rust_is_typeshed_file(typeshed_dir: str | None, *, file: str) -> bool: ...

def rust_is_stdlib_file(typeshed_dir: str | None, *, file: str) -> bool: ...

def rust_is_stub_package_file(file: str) -> bool: ...

def rust_unnamed_function(name: str | None) -> bool: ...

def rust_time_spent_us(t0: int) -> int: ...

def rust_plural_s(s: int | Sized) -> str: ...

def rust_json_dumps(obj: object, debug: bool = ...) -> bytes: ...

class IdMapper:
    def __init__(self) -> None: ...
    def id(self, o: object) -> int: ...
    def __len__(self) -> int: ...


__all__ = [
    "rust_is_dunder",
    "rust_is_sunder",
    "rust_split_module_names",
    "rust_module_prefix",
    "rust_split_target",
    "rust_short_type",
    "rust_find_python_encoding",
    "rust_bytes_to_human_readable_repr",
    "rust_decode_python_encoding",
    "rust_trim_source_line",
    "rust_get_mypy_comments",
    "rust_get_prefix",
    "rust_correct_relative_import",
    "rust_unmangle",
    "rust_get_unique_redefinition_name",
    "rust_split_words",
    "rust_soft_wrap",
    "rust_hash_digest",
    "rust_hash_digest_bytes",
    "rust_hash_path_stem",
    "rust_is_sub_path_normabs",
    "rust_is_typeshed_file",
    "rust_is_stdlib_file",
    "rust_is_stub_package_file",
    "rust_unnamed_function",
    "rust_time_spent_us",
    "rust_plural_s",
    "rust_json_dumps",
    "IdMapper",
]
