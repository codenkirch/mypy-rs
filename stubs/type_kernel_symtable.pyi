"""Native seam stubs for the symtable area (split from type_kernel.pyi, #1677)."""

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



# Phase G3.0a (#1581): namespace dual-write capture shadow. One record
# per (owner table handle, name) with generation + seq; `refresh_flags`
# updates every record referencing an adopted node.
def rust_symtable_mirror_put(
    owner: Any,
    name: str,
    symbol: Any,
    kind: int,
    node_fullname: str | None,
    module_public: bool,
    module_hidden: bool,
    implicit: bool,
    plugin_generated: bool,
    no_serialize: bool,
    cross_ref: str | None,
) -> tuple[int, int, int, int]: ...

def rust_symtable_mirror_delete(owner: Any, name: str) -> bool: ...

def rust_symtable_mirror_refresh_flags(
    node: Any,
    kind: int,
    node_fullname: str | None,
    module_public: bool,
    module_hidden: bool,
    implicit: bool,
    plugin_generated: bool,
    no_serialize: bool,
    cross_ref: str | None,
) -> bool: ...

def rust_symtable_mirror_lookup(owner: Any, name: str) -> dict[str, Any] | None: ...

def rust_symtable_mirror_entry_count(owner: Any) -> int: ...

def rust_symtable_mirror_total_entry_count() -> int: ...

def rust_symtable_mirror_names(owner: Any) -> list[str]: ...

def rust_symtable_mirror_generation(owner: Any) -> int | None: ...

def rust_symtable_mirror_reset() -> int: ...

def rust_symtable_mirror_handle_of(obj: Any) -> int | None: ...

# Phase G3.1 (#1670): read-flip evidence counters for the mirror gate.
def rust_symtable_mirror_flip_counts() -> dict[str, int]: ...

def rust_symtable_mirror_flip_counts_reset() -> int: ...

def rust_symtable_mirror_meta_put(
    info: Any,
    bases_count: int,
    mro_count: int,
    metaclass_fullname: str | None,
    fullname: str | None,
    names_table: Any,
) -> int: ...

def rust_symtable_mirror_meta_put_field(
    info: Any,
    field: str,
    value: str,
) -> int: ...

def rust_symtable_mirror_meta_lookup(info: Any) -> dict[str, Any] | None: ...

def rust_symtable_mirror_meta_delete(info: Any) -> bool: ...

def rust_symtable_mirror_meta_entry_count() -> int: ...


__all__ = [
    "rust_symtable_mirror_put",
    "rust_symtable_mirror_delete",
    "rust_symtable_mirror_refresh_flags",
    "rust_symtable_mirror_lookup",
    "rust_symtable_mirror_entry_count",
    "rust_symtable_mirror_total_entry_count",
    "rust_symtable_mirror_names",
    "rust_symtable_mirror_generation",
    "rust_symtable_mirror_reset",
    "rust_symtable_mirror_handle_of",
    "rust_symtable_mirror_flip_counts",
    "rust_symtable_mirror_flip_counts_reset",
    "rust_symtable_mirror_meta_put",
    "rust_symtable_mirror_meta_put_field",
    "rust_symtable_mirror_meta_lookup",
    "rust_symtable_mirror_meta_delete",
    "rust_symtable_mirror_meta_entry_count",
]
