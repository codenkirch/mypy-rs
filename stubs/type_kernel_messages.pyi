"""Native seam stubs for the messages area (split from type_kernel.pyi, #1677)."""

from __future__ import annotations

from typing import Any, TypeVar

from mypy.nodes import Expression

T = TypeVar("T")

from type_kernel_types import NativeTypeResolver

# Issue #1101: (decided, value) wire answer. decided=True means Rust
# answered (value may be None = a genuine no-result); Python only falls
# back on an exception.
def rust_constant_fold_expr(
    expr: Expression, cur_mod_id: str
) -> tuple[bool, int | bool | float | complex | str | None]: ...
def rust_format_messages_default(
    error_tuples: list[tuple[str | None, int, int, int, int, str, str, str | None]],
    show_column_numbers: bool,
    show_error_end: bool,
    hide_error_codes: bool,
) -> list[str]: ...
def rust_format_messages_default_pretty(
    error_tuples: list[tuple[str | None, int, int, int, int, str, str, str | None]],
    source_lines: list[str] | None,
    show_column_numbers: bool,
    show_error_end: bool,
    hide_error_codes: bool,
    pretty: bool,
) -> list[str]: ...
def rust_callable_name(name: str) -> str | None: ...
def rust_for_function(name: str) -> str | None: ...
def rust_invalid_index_type(index_str: str, expected_str: str, base_str: str) -> str: ...
def rust_missing_named_argument(name: str, for_func: str) -> str: ...
def rust_signatures_incompatible(method: str, other_method: str) -> str: ...
def rust_signature_incompatible_with_supertype(name: str, target: str) -> str: ...
def rust_classify_has_no_attr(
    member: str,
    is_instance: bool,
    is_function_like: bool,
    is_type_obj: bool,
    is_union: bool,
    is_typevar: bool,
    typevar_bound_is_union: bool,
    has_readable_member: bool,
    instance_fullname: str,
    are_type_names_disabled: bool,
    instance_has_names: bool,
    module_private: bool,
    instance_names: list[str],
    module_public_names: list[str],
) -> tuple[int, str, list[str]]: ...
def rust_too_few_arguments(
    prefer_simple: bool,
    argument_names: list[str | None] | None,
    callee_arg_names: list[str | None],
    callee_min_args: int,
    callee_name: str | None,
    for_func: str,
) -> str | None: ...
def rust_too_many_arguments(prefer_simple: bool, for_func: str) -> str: ...
def rust_too_many_positional_arguments(prefer_simple: bool, for_func: str) -> str: ...
def rust_undefined_in_superclass(member: str) -> str: ...
def rust_unexpected_keyword_argument_for_function(
    for_func: str, name: str, matches: list[str] | None
) -> str: ...
def rust_wrong_number_values_to_unpack(provided: int, expected: int) -> str: ...
def rust_format_key_list(keys: list[str], short: bool) -> str: ...
def rust_quote_type_string(type_string: str) -> str: ...
def rust_capitalize(s: str) -> str: ...
def rust_pretty_seq(args: list[str], conjunction: str) -> str: ...
def rust_format_string_list(lst: list[str]) -> str | None: ...
def rust_format_item_name_list(items: list[str]) -> str: ...
def rust_wrong_type_arg_count(low: int, high: int, act: str, name: str) -> str: ...
def rust_strip_quotes(s: str) -> str: ...
def rust_extract_type(name: str) -> str: ...
def rust_variance_string(variance: int) -> str: ...
def rust_format_type_bare(
    type_bytes: bytes,
    resolver: NativeTypeResolver,
    verbosity: int,
    module_names: bool,
    use_star_unpack: bool,
    reveal_verbose_types: bool = ...,
    pretty_wire_safe: bool = ...,
) -> str | None: ...
def rust_format_type(
    type_bytes: bytes,
    resolver: NativeTypeResolver,
    verbosity: int,
    module_names: bool,
    use_star_unpack: bool,
    reveal_verbose_types: bool = ...,
    pretty_wire_safe: bool = ...,
) -> str | None: ...
def rust_format_type_distinctly(
    type_bytes_list: list[bytes],
    resolver: NativeTypeResolver,
    bare: bool,
    use_star_unpack: bool,
    reveal_verbose_types: bool = ...,
    pretty_wire_safe: bool = ...,
    hints: list[tuple[str | None, str | None] | None] = ...,
) -> list[str] | None: ...
def rust_append_invariance_notes(
    arg_bytes: bytes, expected_bytes: bytes, resolver: NativeTypeResolver
) -> list[str] | None: ...
def rust_append_invariance_notes_live(
    arg_type: Any,
    expected_type: Any,
    arg_subtype_result: bool | None,
    key_same_result: bool | None,
    val_subtype_result: bool | None,
) -> list[str] | None: ...
def rust_append_numbers_notes(expected_bytes: bytes) -> list[str] | None: ...
def rust_append_numbers_notes_live(expected_type: Any) -> list[str] | None: ...
def rust_append_union_note(
    arg_bytes: bytes, expected_bytes: bytes, resolver: NativeTypeResolver, use_star_unpack: bool
) -> list[str] | None: ...
def rust_pretty_callable(
    callable_bytes: bytes,
    resolver: NativeTypeResolver,
    reveal_verbose_types: bool,
    use_star_unpack: bool,
) -> str | None: ...
def rust_best_matches(current: str, options: list[str], n: int) -> list[str]: ...
def rust_count_stats(messages: list[str]) -> tuple[int, int, int]: ...
def rust_find_type_overlaps(type_bytes_list: list[bytes]) -> list[str] | None: ...
def rust_make_inferred_type_note(
    subtype_bytes: bytes, supertype_bytes: bytes, arg_results: list[bool], context: Any
) -> bool: ...
def rust_make_inferred_type_note_live(
    subtype: Any, supertype: Any, arg_results: list[bool], context: Any
) -> bool: ...

__all__ = [
    "rust_constant_fold_expr",
    "rust_format_messages_default",
    "rust_format_messages_default_pretty",
    "rust_callable_name",
    "rust_for_function",
    "rust_invalid_index_type",
    "rust_missing_named_argument",
    "rust_signatures_incompatible",
    "rust_signature_incompatible_with_supertype",
    "rust_classify_has_no_attr",
    "rust_too_few_arguments",
    "rust_too_many_arguments",
    "rust_too_many_positional_arguments",
    "rust_undefined_in_superclass",
    "rust_unexpected_keyword_argument_for_function",
    "rust_wrong_number_values_to_unpack",
    "rust_format_key_list",
    "rust_quote_type_string",
    "rust_capitalize",
    "rust_pretty_seq",
    "rust_format_string_list",
    "rust_format_item_name_list",
    "rust_wrong_type_arg_count",
    "rust_strip_quotes",
    "rust_extract_type",
    "rust_variance_string",
    "rust_format_type_bare",
    "rust_format_type",
    "rust_format_type_distinctly",
    "rust_append_invariance_notes",
    "rust_append_invariance_notes_live",
    "rust_append_numbers_notes",
    "rust_append_numbers_notes_live",
    "rust_append_union_note",
    "rust_pretty_callable",
    "rust_best_matches",
    "rust_count_stats",
    "rust_find_type_overlaps",
    "rust_make_inferred_type_note",
    "rust_make_inferred_type_note_live",
]
