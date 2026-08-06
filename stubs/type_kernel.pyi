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

from typing import Any

from mypy.nodes import TypeAlias, TypeInfo
from mypy.types import ProperType, Type

__all__ = [
    "NativeTypeResolver",
    "PluginHookRegistry",
    "erase_type",
    "remove_instance_last_known_values",
    "rust_read_cache_meta",
    "rust_read_cache_meta_ex",
    "read_type_to_str",
    "build_resolver",
    "read_type_to_str_with_resolver",
    "build_native_resolver",
    "read_type_to_str_with_native_resolver",
    "rust_is_subtype",
    "rust_trivial_join",
    "rust_trivial_meet",
    "rust_join_types",
    "rust_meet_types",
    "rust_map_actuals_to_formals",
    "rust_map_formals_to_actuals",
    "rust_map_actuals_to_formals_with_types",
    "rust_expand_actual_type",
    "rust_linearize_hierarchy",
    "rust_expand_type",
    "rust_make_simplified_union",
    "rust_simple_literal_type",
    "rust_is_simple_literal",
    "rust_true_only",
    "rust_false_only",
    "rust_true_or_false",
    "rust_operator_tables",
    "rust_erase_typevars",
    "rust_replace_meta_vars",
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
    "rust_has_explicit_any",
    "rust_has_any_from_unimported_type",
    "rust_collect_all_inner_types",
    "rust_make_optional_type",
    "rust_flatten_nested_tuples",
    "rust_copy_type",
    "rust_apply_generic_arguments",
    "rust_has_no_typevars",
    "rust_has_any_type",
    "rust_has_uninhabited_component",
    "rust_has_ambiguous_uninhabited_component",
    "rust_allow_fast_container_literal",
    "rust_has_bytes_component",
    "rust_has_bool_item",
    "rust_is_non_empty_tuple",
    "rust_has_coroutine_decorator",
    "rust_is_async_def",
    "rust_is_typed_callable",
    "rust_is_private",
    "rust_is_operator_method",
    "rust_are_argument_counts_overlapping",
    "rust_is_type_type_context",
    "rust_try_getting_literal",
    "rust_is_string_literal",
    "rust_is_untyped_decorator",
    "rust_is_typeddict_type_context",
    "rust_format_messages_default",
    "rust_infer_constraints",
    "rust_classify_call",
    "rust_normalize_callable",
    "rust_solve_one",
    "rust_format_key_list",
    "rust_is_numeric_format_type",
    "rust_has_return_statement",
    "rust_has_str_expression",
    "rust_has_yield_expression",
    "rust_has_yield_from_expression",
    "rust_has_await_expression",
    "rust_count_return_statements",
    "rust_count_yield_expressions",
    "rust_count_name_and_member_expressions",
    "rust_make_any_non_explicit",
    "rust_make_any_non_unimported",
    "rust_is_unreachable_map",
    "rust_stmt_outcome",
    "rust_type_requires_usage",
    "rust_with_exit_suppresses",
    "rust_try_handler_union",
]

class NativeTypeResolver:
    len: int
    alias_len: int
    def render_dict(self) -> dict[str, object]: ...

class PluginHookRegistry:
    def __init__(self, hooks: list[str]) -> None: ...
    def has_hook(self, fullname: str) -> bool: ...
    def len(self) -> int: ...
    def is_empty(self) -> bool: ...

def erase_type(typ: Type) -> ProperType | None: ...
def remove_instance_last_known_values(typ: Type) -> Type | None: ...
def rust_read_cache_meta(blob: bytes) -> dict[str, Any] | None: ...
def rust_read_cache_meta_ex(blob: bytes) -> dict[str, Any] | None: ...
def read_type_to_str(data: bytes) -> str: ...
def build_resolver(type_infos: list[TypeInfo]) -> dict[str, object]: ...
def read_type_to_str_with_resolver(data: bytes, resolver: dict[str, object]) -> str: ...
def build_native_resolver(
    type_infos: list[TypeInfo], aliases: list[TypeAlias]
) -> NativeTypeResolver: ...
def read_type_to_str_with_native_resolver(data: bytes, resolver: NativeTypeResolver) -> str: ...
def rust_is_subtype(
    left: bytes,
    right: bytes,
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    proper_subtype: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_trivial_join(
    left: bytes,
    right: bytes,
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> int | None: ...
def rust_trivial_meet(
    left: bytes,
    right: bytes,
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> int | None: ...
def rust_join_types(
    left: bytes, right: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> tuple[int, str | None, list[int], bytes] | None: ...
def rust_meet_types(
    left: bytes, right: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> tuple[int, str | None, list[int], bytes] | None: ...
def rust_map_actuals_to_formals(
    actual_kinds: list[int],
    actual_names: list[str | None],
    formal_kinds: list[int],
    formal_names: list[str | None],
) -> list[list[int]] | None: ...
def rust_map_formals_to_actuals(
    actual_kinds: list[int],
    actual_names: list[str | None],
    formal_kinds: list[int],
    formal_names: list[str | None],
) -> list[list[int]] | None: ...
def rust_map_actuals_to_formals_with_types(
    actual_kinds: list[int],
    actual_names: list[str | None],
    formal_kinds: list[int],
    formal_names: list[str | None],
    actual_types: list[bytes | None],
) -> list[list[int]] | None: ...
def rust_expand_actual_type(
    actual_type: bytes,
    actual_kind: int,
    formal_name: str | None,
    formal_kind: int,
    allow_unpack: bool,
    tuple_index: int,
    kwargs_used: list[str],
) -> tuple[int, str | None, int, list[str]] | None: ...
def rust_linearize_hierarchy(
    resolver: NativeTypeResolver, info_fullname: str
) -> list[str] | None: ...
def rust_expand_type(
    resolver: NativeTypeResolver, type_bytes: bytes, env_bytes: bytes
) -> bytes | None: ...
def rust_make_simplified_union(
    items_bytes: bytes,
    line: int,
    column: int,
    keep_erased: bool,
    contract_literals: bool,
    handle_recursive: bool,
    resolver: NativeTypeResolver,
) -> bytes | None: ...
def rust_simple_literal_type(type_bytes: bytes) -> bytes | None: ...
def rust_is_simple_literal(type_bytes: bytes, resolver: NativeTypeResolver) -> bool | None: ...
def rust_true_only(type_bytes: bytes) -> tuple[int, object] | None: ...
def rust_false_only(type_bytes: bytes, strict_optional: bool) -> tuple[int, object] | None: ...
def rust_true_or_false(type_bytes: bytes) -> tuple[int, object] | None: ...
def rust_operator_tables() -> dict[str, object]: ...
def rust_erase_typevars(type_bytes: bytes, ids_bytes: bytes) -> bytes | None: ...
def rust_replace_meta_vars(type_bytes: bytes, target_bytes: bytes) -> bytes | None: ...
def rust_has_type_vars(type_bytes: bytes) -> bool: ...
def rust_has_recursive_types(type_bytes: bytes) -> bool | None: ...
def rust_is_literal_type(
    type_bytes: bytes,
    fallback_fullname: str,
    value_kind: str,
    value_payload: str,
) -> bool: ...
def rust_is_unannotated_any(type_bytes: bytes) -> bool: ...
def rust_remove_dups(type_bytes_list: list[bytes]) -> list[bytes]: ...
def rust_type_vars_as_args(type_bytes_list: list[bytes]) -> list[bytes]: ...
def rust_callable_with_ellipsis(
    any_bytes: bytes, ret_bytes: bytes, fallback_bytes: bytes
) -> bytes | None: ...
def rust_find_unpack_in_list(type_bytes_list: list[bytes]) -> int: ...
def rust_split_with_prefix_and_suffix(
    type_bytes_list: list[bytes], prefix: int, suffix: int
) -> tuple[list[bytes], list[bytes], list[bytes]]: ...
def rust_flatten_nested_unions(
    type_bytes_list: list[bytes], handle_type_alias_type: bool, handle_recursive: bool
) -> list[bytes] | None: ...
def rust_has_explicit_any(type_bytes: bytes) -> bool | None: ...
def rust_has_any_from_unimported_type(type_bytes: bytes) -> bool | None: ...
def rust_collect_all_inner_types(type_bytes: bytes) -> list[bytes] | None: ...
def rust_make_optional_type(type_bytes: bytes) -> bytes | None: ...
def rust_flatten_nested_tuples(
    type_bytes_list: list[bytes], handle_recursive: bool
) -> list[bytes] | None: ...
def rust_copy_type(type_bytes: bytes) -> bytes | None: ...
def rust_apply_generic_arguments(
    resolver: NativeTypeResolver,
    callable_bytes: bytes,
    orig_types_bytes: bytes,
    skip_unsatisfied: bool,
) -> bytes | None: ...
def rust_has_no_typevars(type_bytes: bytes) -> bool | None: ...
def rust_has_any_type(type_bytes: bytes, ignore_in_type_obj: bool) -> bool | None: ...
def rust_has_uninhabited_component(type_bytes: bytes) -> bool | None: ...
def rust_has_ambiguous_uninhabited_component(type_bytes: bytes) -> bool | None: ...
def rust_allow_fast_container_literal(type_bytes: bytes) -> bool | None: ...
def rust_has_bytes_component(type_bytes: bytes) -> bool | None: ...
def rust_has_bool_item(type_bytes: bytes) -> bool | None: ...
def rust_is_non_empty_tuple(type_bytes: bytes) -> bool | None: ...
def rust_has_coroutine_decorator(type_bytes: bytes) -> bool | None: ...
def rust_is_async_def(type_bytes: bytes) -> bool | None: ...
def rust_is_typed_callable(type_bytes: bytes) -> bool | None: ...
def rust_is_private(node_name: str) -> bool: ...
def rust_is_operator_method(fullname: str | None) -> bool: ...
def rust_are_argument_counts_overlapping(t_bytes: bytes, s_bytes: bytes) -> bool | None: ...
def rust_is_type_type_context(type_bytes: bytes) -> bool | None: ...
def rust_try_getting_literal(type_bytes: bytes) -> bytes | None: ...
def rust_is_string_literal(type_bytes: bytes) -> bool | None: ...
def rust_is_untyped_decorator(type_bytes: bytes) -> bool | None: ...
def rust_is_typeddict_type_context(type_bytes: bytes) -> bool | None: ...
def rust_format_messages_default(
    error_tuples: list[tuple[str | None, int, int, int, int, str, str, str | None]],
    show_column_numbers: bool,
    show_error_end: bool,
    hide_error_codes: bool,
) -> list[str]: ...
def rust_infer_constraints(
    template_bytes: bytes, actual_bytes: bytes, direction: int
) -> list[bytes] | None: ...
def rust_classify_call(callee_bytes: bytes) -> int | None: ...
def rust_normalize_callable(callee_bytes: bytes) -> bytes | None: ...
def rust_solve_one(
    lowers: list[bytes],
    uppers: list[bytes],
    infer_unions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[int, bytes | None] | None: ...
def rust_format_key_list(keys: list[str], short: bool) -> str: ...
def rust_is_numeric_format_type(conv_type: str, is_new_style: bool) -> bool: ...
def rust_has_return_statement(node_bytes: bytes) -> bool: ...
def rust_has_str_expression(node_bytes: bytes) -> bool: ...
def rust_has_yield_expression(node_bytes: bytes) -> bool: ...
def rust_has_yield_from_expression(node_bytes: bytes) -> bool: ...
def rust_has_await_expression(node_bytes: bytes) -> bool: ...
def rust_count_return_statements(node_bytes: bytes) -> int: ...
def rust_count_yield_expressions(node_bytes: bytes) -> int: ...
def rust_count_name_and_member_expressions(node_bytes: bytes) -> tuple[int, int]: ...
def rust_make_any_non_explicit(type_bytes: bytes) -> bytes | None: ...
def rust_make_any_non_unimported(type_bytes: bytes) -> bytes | None: ...
def rust_is_unreachable_map(type_bytes_list: list[bytes]) -> bool | None: ...
def rust_stmt_outcome(node_bytes: bytes) -> str | None: ...
def rust_type_requires_usage(type_bytes: bytes) -> int | None: ...
def rust_with_exit_suppresses(type_bytes: bytes, strict_optional: bool) -> bool: ...
def rust_try_handler_union(
    type_bytes: bytes, strict_optional: bool
) -> list[bytes] | None: ...
