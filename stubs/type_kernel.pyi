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

from collections.abc import Callable, Sized
from typing import Any, TypeVar

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
from mypy.types import ProperType, TupleType, Type, TypeVarLikeType

T = TypeVar("T")

__all__ = [
    "rust_find_shallow_matching_overload_item",
    "NativeTypeResolver",
    "PluginHookRegistry",
    "erase_type",
    "remove_instance_last_known_values",
    "shallow_erase_type_for_equality",
    "rust_constant_fold_expr",
    "rust_read_cache_meta",
    "rust_read_cache_meta_ex",
    "read_type_to_str",
    "build_resolver",
    "read_type_to_str_with_resolver",
    "build_native_resolver",
    "read_type_to_str_with_native_resolver",
    "rust_is_subtype",
    "rust_is_subtype_batch",
    "rust_is_protocol_implementation",
    "rust_subtype_tvar_tuple_right",
    "rust_variadic_tuple_subtype",
    "rust_trivial_join",
    "rust_trivial_meet",
    "rust_join_types",
    "rust_join_instances",
    "rust_meet_types",
    "rust_object_or_any_from_type",
    "rust_combine_similar_callables",
    "rust_object_from_instance",
    "rust_narrow_declared_type",
    "rust_narrow_with_len",
    "rust_map_actuals_to_formals",
    "rust_narrow_type_by_identity_equality",
    "rust_map_formals_to_actuals",
    "rust_map_actuals_to_formals_with_types",
    "rust_expand_actual_type",
    "rust_linearize_hierarchy",
    "rust_expand_type",
    "rust_expand_type_by_instance",
    "rust_remove_trivial",
    "rust_freshen_function_type_vars",
    "rust_map_instance_to_supertype",
    "rust_class_derivation_paths",
    "rust_map_instance_to_direct_supertypes",
    "rust_make_simplified_union",
    "rust_simple_literal_type",
    "rust_is_simple_literal",
    "rust_is_literal_type_like",
    "rust_try_getting_str_literals_from_type",
    "rust_try_getting_int_literals_from_type",
    "rust_try_getting_bool_literals_from_type",
    "rust_try_getting_instance_fallback",
    "rust_true_only",
    "rust_false_only",
    "rust_true_or_false",
    "rust_try_expanding_sum_type_to_union",
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
    "rust_unknown_unpack",
    "rust_has_explicit_any_live",
    "rust_has_any_from_unimported_type_live",
    "rust_collect_all_inner_types_live",
    "rust_make_optional_type_live",
    "rust_unknown_unpack_live",
    "rust_flatten_nested_tuples",
    "rust_copy_type",
    "rust_apply_generic_arguments",
    "rust_has_no_typevars",
    "rust_has_abstract_type",
    "rust_has_any_type",
    "rust_has_uninhabited_component",
    "rust_has_ambiguous_uninhabited_component",
    "rust_has_erased_component",
    "rust_allow_fast_container_literal",
    "rust_analyze_cond_branch",
    "rust_has_bytes_component",
    "rust_has_bool_item",
    "rust_is_non_empty_tuple",
    "rust_has_coroutine_decorator",
    "rust_is_async_def",
    "rust_is_duplicate_mapping",
    "rust_is_typed_callable",
    "rust_is_private",
    "rust_is_operator_method",
    "rust_are_argument_counts_overlapping",
    "rust_is_type_type_context",
    "rust_try_getting_literal",
    "rust_is_string_literal",
    "rust_is_untyped_decorator",
    "rust_is_typeddict_type_context",
    "rust_is_typevar_default_recursive",
    "rust_conditional_expr_join",
    "rust_container_type",
    "rust_tuple_context_matches",
    "rust_build_tuple_type",
    "rust_star_expr",
    "rust_resolve_plugin_hook",
    "rust_method_fullname",
    "rust_is_enum_callable_base",
    "rust_classify_protocol_test_callee",
    "rust_format_messages_default",
    "rust_format_messages_default_pretty",
    "rust_infer_constraints",
    "rust_infer_constraints_full",
    "rust_select_trivial",
    "rust_exclude_non_meta_vars",
    "rust_is_similar_constraints",
    "rust_any_constraints",
    "rust_repack_callable_args",
    "rust_merge_with_any",
    "rust_filter_satisfiable",
    "rust_is_same_constraints",
    "rust_classify_call",
    "rust_classify_typeddict_call",
    "rust_classify_reveal_imported",
    "rust_refers_to_typeddict",
    "rust_classify_super_arg_types",
    "rust_classify_visit_op_expr",
    "rust_classify_index_with_type",
    "rust_calibrate_type_obj_return",
    "rust_normalize_callable",
    "rust_check_callable_call",
    "rust_solve_one",
    "rust_check_overload_call",
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
    "rust_check_overlapping_overloads",
    "rust_classify_except_handler_tests",
    "rust_classify_final_super",
    "rust_classify_check_final",
    "rust_classify_classvar_super",
    "rust_classify_check_lvalue",
    "rust_classify_new_signature",
    "rust_classify_func_def_override",
    "rust_classify_enum_new",
    "rust_classify_enum_bases",
    "rust_is_final_enum_value",
    "rust_check_for_untyped_decorator",
    "rust_check_explicit_override_decorator",
    "rust_check_match_args",
    "rust_classify_rvalue_count",
    "rust_classify_truthy_type",
    "rust_classify_missing_annotations",
    "rust_classify_return_stmt_variant",
    "rust_classify_return_stmt_pre",
    "rust_classify_return_stmt_post",
    "rust_classify_lvalue_validity",
    "rust_classify_fixed_args",
    "rust_conditional_types",
    "rust_is_true_literal",
    "rust_is_false_literal",
    "rust_is_literal_none",
    "rust_is_literal_not_implemented",
    "rust_is_static",
    "rust_is_property",
    "rust_is_method",
    "rust_is_empty_generator_function",
    "rust_is_settable_property",
    "rust_is_custom_settable_property",
    "rust_can_have_shared_disjoint_base",
    "rust_check_operator",
    "rust_is_trivial_bound",
    "rust_find_linear",
    "rust_separate_union_literals",
    "rust_get_type_vars",
    "rust_solve_constraints",
    "rust_solve_dependent",
    "rust_replace_implicit_first_type",
    "rust_infer_function_type_arguments",
    "rust_callables_compatible",
    "rust_are_args_compatible",
    "rust_classify_type_parameter",
    "rust_are_parameters_compatible",
    "rust_is_overlapping_types",
    "rust_refers_to_fullname",
    "rust_refers_to_class_or_function",
    "rust_is_trivial_body",
    "rust_find_duplicate",
    "rust_is_valid_replacement",
    "rust_is_same_symbol",
    "rust_names_modified_in_lvalue",
    "rust_names_modified_by_assignment",
    "rust_remove_imported_names_from_symtable",
    "rust_should_wait_rhs",
    "rust_apply_semantic_analyzer_patches",
    "rust_classify_decorators",
    "rust_classify_imports",
    "rust_classify_setup_type_vars",
    "rust_visit_list_set_expr",
    "rust_visit_dict_expr",
    "rust_visit_template_str_expr",
    "rust_visit_unary_expr",
    "rust_visit_comparison_expr",
    "rust_visit_slice_expr",
    "rust_visit_conditional_expr",
    "rust_visit_super_expr",
    "rust_visit_raise_stmt",
    "rust_visit_assert_stmt",
    "rust_visit_operator_assignment_stmt",
    "rust_visit_block",
    "rust_visit_if_stmt",
    "rust_is_valid_del_target",
    "rust_visit_del_stmt",
    "rust_visit_expression_stmt",
    "rust_visit_break_stmt",
    "rust_visit_continue_stmt",
    "rust_visit_global_decl",
    "rust_visit_match_stmt",
    "rust_visit_return_stmt",
    "rust_visit_block_maybe",
    "rust_visit_while_stmt",
    "rust_visit_name_expr",
    "rust_visit_star_expr",
    "rust_visit_as_pattern",
    "rust_visit_or_pattern",
    "rust_visit_value_pattern",
    "rust_visit_sequence_pattern",
    "rust_visit_starred_pattern",
    "rust_visit_mapping_pattern",
    "rust_visit_class_pattern",
    "rust_visit_yield_expr",
    "rust_visit_yield_from_expr",
    "rust_visit_await_expr",
    "rust_visit_try_stmt",
    "rust_visit_op_expr",
    "rust_visit_index_expr",
    "rust_visit_cast_expr",
    "rust_visit_type_form_expr",
    "rust_visit_assert_type_expr",
    "rust_visit_reveal_expr",
    "rust_visit_type_application",
    "rust_visit_list_comprehension",
    "rust_visit_set_comprehension",
    "rust_visit_dictionary_comprehension",
    "rust_visit_generator_expr",
    "rust_visit_lambda_expr",
    "rust_visit_overloaded_func_def",
    "rust_visit_class_def",
    "rust_visit_func_def",
    "rust_visit_nonlocal_decl",
    "rust_visit_for_stmt",
    "rust_visit_with_stmt",
    "rust_visit_assignment_expr",
    "rust_visit_import_all",
    "rust_visit_import_from",
    "rust_visit_assignment_stmt",
    "rust_visit_import",
    "rust_visit_call_expr",
    "rust_visit_type_alias_stmt",
    "rust_lookup",
    "rust_stubgen_render",
    "rust_stubgen_render_type_args",
    "rust_get_assigned_names",
    "rust_is_none_expr",
    "rust_is_pybind11_overloaded_function_docstring",
    "rust_method_name_sort_key",
    "rust_dataclass_transform",
    "rust_dataclass_post_init_transform",
    "rust_classify_member_resolution",
    "rust_classify_simple_literal_type",
    "rust_is_defined_type_param",
    "rust_var_is_typing_special_form",
    "rust_get_typevarlike_declaration",
    "rust_parse_bool",
    "rust_is_mangled_global",
    "rust_is_initial_mangled_global",
    "rust_is_final_redefinition",
    "rust_is_same_var_from_getattr",
    "rust_can_possibly_be_typevarlike_declaration",
    "rust_can_possibly_be_type_form",
    "rust_is_type_ref",
    "rust_can_be_type_alias",
    "rust_check_typevarlike_name",
    "rust_check_decorated_function_is_method",
    "rust_classify_method_signature",
    "rust_extract_typevarlike_name",
    "rust_special_function_elide_names",
    "rust_argument_elide_name",
    "rust_set_callable_name",
    "rust_has_placeholder",
    "rust_calculate_tuple_fallback",
    "rust_find_dataclass_transform_spec",
    "rust_find_shallow_matching_overload_item",
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
    "rust_count_stats",
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
    # Issue #540: modulefinder pure helpers
    "rust_is_init_file",
    "rust_parse_version",
    "rust_mypy_path",
    "rust_typeshed_py_version",
    "rust_default_lib_path",
    "rust_load_stdlib_py_versions",
    "rust_matches_exclude",
    "rust_get_search_dirs",
    "rust_compute_search_paths",
    "RustSearchPaths",
    "RustBuildSource",
    "RustBuildSourceSet",
    "rust_calculate_class_abstract_status",
    "rust_check_protocol_status",
    "rust_calculate_class_vars",
    "rust_add_type_promotion",
    "rust_fixup_type",
    "rust_fixup_type_info",
    "rust_resolve_cross_ref",
    "rust_fixup_symbol_table",
    "rust_fixup_overloaded_func_def",
    "rust_fixup_decorator",
    "rust_get_declaration",
    "rust_infer_condition_value",
    "rust_infer_pattern_value",
    "rust_assert_will_always_fail",
    "rust_consider_sys_version_info",
    "rust_consider_sys_platform",
    "rust_is_sys_attr",
    "rust_contains_sys_version_info",
    "rust_contains_int_or_tuple_of_ints",
    "rust_fixed_comparison",
    "rust_primary_source",
    "rust_check_namedtuple_field_name",
    "rust_verify_requiredness_compatibility",
    "rust_verify_field_against_closed_bases",
    "rust_validate_instance",
    "rust_detect_diverging_alias",
    "rust_find_self_type",
    "rust_check_vec_type_args",
    "rust_check_unpacks_in_list",
    "rust_find_matching_overload_items",
    "IdMapper",
]

class NativeTypeResolver:
    len: int
    alias_len: int
    def render_dict(self) -> dict[str, object]: ...
    def update(
        self, type_infos: list[TypeInfo], aliases: list[TypeAlias]
    ) -> tuple[int, int]: ...
    def set_live_typeinfo_map(self, typeinfo_map: dict[str, TypeInfo] | None) -> None: ...

class PluginHookRegistry:
    def __init__(self, hooks: dict[str, list[str]]) -> None: ...
    def has_call_hook(self, fullname: str) -> bool: ...
    def has_hook_for(self, hook_method_name: str, fullname: str) -> bool: ...
    def has_hook(self, fullname: str) -> bool: ...
    def len(self) -> int: ...
    def is_empty(self) -> bool: ...

def erase_type(typ: Type) -> ProperType | None: ...
def remove_instance_last_known_values(typ: Type) -> Type | None: ...
def shallow_erase_type_for_equality(typ: Type) -> ProperType | None: ...
def rust_constant_fold_expr(
    expr: Expression, cur_mod_id: str
) -> int | bool | float | complex | str | None: ...
def rust_read_cache_meta(blob: bytes) -> dict[str, Any] | None: ...
def rust_read_cache_meta_ex(blob: bytes) -> dict[str, Any] | None: ...
def read_type_to_str(data: bytes) -> str: ...
def build_resolver(type_infos: list[TypeInfo]) -> dict[str, object]: ...
def read_type_to_str_with_resolver(data: bytes, resolver: dict[str, object]) -> str: ...
def build_native_resolver(
    type_infos: list[TypeInfo], aliases: list[TypeAlias], modules: Any = None
) -> NativeTypeResolver: ...
def read_type_to_str_with_native_resolver(data: bytes, resolver: NativeTypeResolver) -> str: ...
def rust_get_type_triggers(typ: Any, use_logical_deps: bool) -> list[str] | None: ...
def rust_attribute_triggers(typ: Any, name: str) -> list[str] | None: ...
def rust_is_subtype(
    left: bytes,
    right: bytes,
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    proper_subtype: bool,
    strict_optional: bool,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...

def rust_is_protocol_implementation(
    left: bytes,
    right: bytes,
    skip: list[str],
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    proper_subtype: bool,
    strict_optional: bool,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...

def rust_is_subtype_batch(
    pairs_bytes: list[bytes],
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    proper_subtype: bool,
    strict_optional: bool,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    resolver: NativeTypeResolver,
) -> list[int]: ...

def rust_subtype_tvar_tuple_right(
    left: bytes,
    right: bytes,
    proper_subtype: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_variadic_tuple_subtype(
    left: bytes,
    right: bytes,
    proper_subtype: bool,
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
def rust_join_instances(
    t: bytes, s: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> tuple[int, str | None, list[int], bytes] | None: ...
def rust_meet_types(
    left: bytes, right: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> tuple[int, str | None, list[int], bytes] | None: ...
def rust_join_tuples(
    s: bytes, t: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_meet_tuples(
    s: bytes, t: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_join_type_list(
    type_blobs: list[bytes], strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_object_or_any_from_type(
    typ: bytes, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_combine_similar_callables(
    t: bytes, s: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_object_from_instance(instance: bytes, resolver: NativeTypeResolver) -> str | None: ...
def rust_join_sorted_key(t_bytes: bytes) -> int | None: ...
def rust_narrow_declared_type(
    declared: bytes, narrowed: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_narrow_type_by_identity_equality(
    expr_type: bytes,
    target_type: bytes,
    comparison: str,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[bytes | None, bytes | None] | None: ...
def rust_narrow_with_len(
    typ: bytes,
    op: str,
    size: int,
    strict_optional: bool,
    precise_tuple: bool,
    resolver: NativeTypeResolver,
) -> tuple[bytes, bytes] | None: ...
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
    resolver: NativeTypeResolver,
    type_bytes: bytes,
    env_bytes: bytes,
    strict_optional: bool,
) -> bytes | None: ...
def rust_expand_type_by_instance(
    resolver: NativeTypeResolver,
    type_bytes: bytes,
    instance_bytes: bytes,
    strict_optional: bool,
) -> bytes | None: ...
def rust_remove_trivial(
    types_bytes: bytes,
    strict_optional: bool,
) -> bytes | None: ...
def rust_freshen_function_type_vars(
    start_raw_id: int,
    callee_bytes: bytes,
) -> tuple[int, bytes] | None: ...
def rust_map_instance_to_supertype(
    resolver: NativeTypeResolver,
    instance_ref: str,
    instance_args: bytes,
    supertype_ref: str,
) -> bytes | None: ...
def rust_class_derivation_paths(
    resolver: NativeTypeResolver,
    typ_ref: str,
    supertype_ref: str,
) -> list[list[str]] | None: ...
def rust_map_instance_to_direct_supertypes(
    resolver: NativeTypeResolver,
    instance_ref: str,
    instance_args: bytes,
    supertype_ref: str,
) -> list[bytes] | None: ...
def rust_freshen_all_functions_type_vars(
    start_raw_id: int,
    type_bytes: bytes,
    strict_optional: bool,
) -> tuple[int, bool, bytes] | None: ...
def rust_make_simplified_union(
    items_bytes: bytes,
    line: int,
    column: int,
    keep_erased: bool,
    contract_literals: bool,
    handle_recursive: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bytes | None: ...
def rust_simple_literal_type(type_bytes: bytes) -> bytes | None: ...
def rust_is_simple_literal(type_bytes: bytes, resolver: NativeTypeResolver) -> bool | None: ...
def rust_is_literal_type_like(type_bytes: bytes) -> bool | None: ...
def rust_try_getting_str_literals_from_type(type_bytes: bytes) -> list[str] | None: ...
def rust_try_getting_int_literals_from_type(type_bytes: bytes) -> list[int] | None: ...
def rust_try_getting_bool_literals_from_type(type_bytes: bytes) -> list[bool] | None: ...
def rust_try_getting_instance_fallback(
    t_bytes: bytes, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_true_only(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> tuple[int, object] | None: ...
def rust_false_only(
    type_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> tuple[int, object] | None: ...
def rust_true_or_false(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> tuple[int, object] | None: ...
def rust_try_expanding_sum_type_to_union(
    type_bytes: bytes,
    target_fullname: str | None,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bytes | None: ...
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
def rust_unknown_unpack(type_bytes: bytes) -> bool | None: ...
def rust_has_explicit_any_live(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bool | None: ...
def rust_has_any_from_unimported_type_live(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bool | None: ...
def rust_collect_all_inner_types_live(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> list[bytes] | None: ...
def rust_make_optional_type_live(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bytes | None: ...
def rust_unknown_unpack_live(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bool | None: ...
def rust_flatten_nested_tuples(
    type_bytes_list: list[bytes], handle_recursive: bool
) -> list[bytes] | None: ...
def rust_copy_type(type_bytes: bytes) -> bytes | None: ...
def rust_apply_generic_arguments(
    resolver: NativeTypeResolver,
    callable_bytes: bytes,
    orig_types_bytes: bytes,
    skip_unsatisfied: bool,
    strict_optional: bool,
) -> bytes | None: ...
def rust_has_no_typevars(type_bytes: bytes) -> bool | None: ...
def rust_has_any_type(
    resolver: NativeTypeResolver, type_bytes: bytes, ignore_in_type_obj: bool
) -> bool | None: ...
def rust_has_abstract_type(
    caller_type: ProperType, callee_type: ProperType, allow_abstract_call: bool
) -> bool | None: ...
def rust_has_uninhabited_component(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_has_ambiguous_uninhabited_component(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_has_erased_component(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_allow_fast_container_literal(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bool | None: ...
def rust_analyze_cond_branch(
    resolver: NativeTypeResolver,
    branch: bytes | None,
    known_type: bytes | None,
) -> bytes | None: ...
def rust_has_bytes_component(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bool | None: ...
def rust_has_bool_item(type_bytes: bytes) -> bool | None: ...
def rust_is_non_empty_tuple(type_bytes: bytes) -> bool | None: ...
def rust_has_coroutine_decorator(type_bytes: bytes) -> bool | None: ...
def rust_is_async_def(type_bytes: bytes) -> bool | None: ...
def rust_is_duplicate_mapping(
    mapping: list[int],
    actual_types: list[bytes],
    actual_kinds: list[int],
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_check_overload_call(
    resolver: NativeTypeResolver,
    targets_bytes: list[bytes],
    arg_types_bytes: list[bytes],
    arg_kinds: list[int],
    arg_names: list[str | None] | None,
    strict_optional: bool,
) -> int | None: ...
def rust_find_matching_overload_items(
    resolver: NativeTypeResolver,
    items_bytes: list[bytes],
    template_bytes: bytes,
    strict_optional: bool,
) -> list[int] | None: ...
def rust_is_typed_callable(type_bytes: bytes) -> bool | None: ...
def rust_is_private(node_name: str) -> bool: ...
def rust_is_operator_method(fullname: str | None) -> bool: ...
def rust_are_argument_counts_overlapping(t_bytes: bytes, s_bytes: bytes) -> bool | None: ...
def rust_check_overlapping_overloads(
    signatures: list[bytes],
    class_type_vars: bytes,
    is_descriptor_get: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> list[tuple[int, int, int, bool]] | None: ...
def rust_classify_final_super(
    base_node: Any,
    node_is_final: bool,
    node_name: str,
    base_fullname: str,
    enum_bases: list[str],
    enum_special_props: list[str],
) -> int | None: ...
def rust_classify_check_final(
    lvalues: Any,
    is_final_decl: bool,
    cls: Any,
    is_stub: bool,
    s_type_is_none: bool,
    is_assignment_stmt: bool,
) -> tuple[bool, list[tuple[str, bool]]] | None: ...
def rust_classify_classvar_super(
    base_node: Any,
    node_is_classvar: bool,
) -> int | None: ...
def rust_classify_check_lvalue(
    lvalue: Any, allow_redefinition: bool, is_definition: bool
) -> int | None: ...
def rust_classify_new_signature(
    is_metaclass: bool, is_instance_ret: bool
) -> int | None: ...
def rust_classify_getattr_method(scope: Any, name: str) -> int | None: ...
def rust_classify_func_def_override(
    is_funcdef: bool,
    orig_type_is_none: bool,
    is_partial: bool,
    partial_type_is_none: bool,
    is_invalid_redefinition: bool,
) -> int: ...
def rust_classify_enum_new(bases: Any) -> list[int] | None: ...
def rust_classify_enum_bases(bases: Any) -> tuple[int, int] | None: ...
def rust_is_final_enum_value(sym: SymbolTableNode, is_stub: bool) -> bool: ...
def rust_check_for_untyped_decorator(
    disallow_untyped_decorators: bool,
    func_type_bytes: bytes | None,
    dec_type_bytes: bytes | None,
    current_node_deferred: bool,
) -> bool | None: ...
def rust_check_explicit_override_decorator(
    defn: Any, found_method_base_classes: Any
) -> bool: ...
def rust_check_match_args(type_bytes: bytes) -> bool | None: ...
def rust_classify_rvalue_count(
    lvalues: Any, rvalue_count: int, rvalue_unpack: int | None
) -> int | None: ...
def rust_classify_truthy_type(t: Any) -> int | None: ...
def rust_classify_missing_annotations(
    is_typeshed_stub: bool,
    warn_incomplete_stub: bool,
    disallow_untyped_defs: bool,
    disallow_incomplete_defs: bool,
    type_tag: int,
    arguments_len: int,
    arg_names: list[str | None],
    is_generator: bool,
    is_coroutine: bool,
    ret_type_bytes: bytes | None,
    arg_type_blobs: list[bytes],
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[int, bool] | None: ...
def rust_classify_return_stmt_variant(is_generator: bool, is_coroutine: bool) -> int: ...
def rust_classify_return_stmt_pre(
    return_type_bytes: bytes, is_lambda: bool
) -> bool | None: ...
def rust_classify_return_stmt_post(
    typ_bytes: bytes | None,
    return_type_bytes: bytes,
    is_async_generator: bool,
    is_generator: bool,
    is_coroutine: bool,
    declared_none_return: bool,
    warn_return_any: bool,
    current_node_deferred: bool,
    name_in_binary_magic: bool,
    expr_is_literal_not_implemented: bool,
    is_lambda: bool,
    in_checked_function: bool,
) -> int | None: ...
def rust_classify_lvalue_validity(node: Any) -> int: ...
def rust_is_type_type_context(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bool | None: ...
def rust_try_getting_literal(type_bytes: bytes) -> bytes | None: ...
def rust_is_string_literal(type_bytes: bytes) -> bool | None: ...
def rust_is_untyped_decorator(type_bytes: bytes) -> bool | None: ...
def rust_is_typeddict_type_context(type_bytes: bytes) -> bool | None: ...
def rust_is_typevar_default_recursive(
    tv_fname: str, start: object
) -> bool | None: ...
def rust_conditional_expr_join(
    if_bytes: bytes, else_bytes: bytes, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_conditional_types(
    current: bytes,
    ranges: bytes | None,
    default: bytes | None,
    consider_runtime_isinstance: bool,
    from_equality: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[bytes | None, bytes | None] | None: ...
def rust_container_type(
    resolver: NativeTypeResolver,
    tag: str,
    elements: list[bytes],
    ctx: list[bytes] | None,
    n_keys: int,
) -> bytes | None: ...
def rust_tuple_context_matches(
    elements_tags: list[int], ctx_bytes: bytes
) -> bool | None: ...
def rust_build_tuple_type(
    items_bytes: list[bytes], seen_unpack: int
) -> bytes | None: ...
def rust_star_expr(type_bytes: bytes) -> bytes | None: ...
def rust_resolve_plugin_hook(
    registry: Any, callable_name: str, plugin_list: Any, hook_method_name: str
) -> Any | None: ...
def rust_method_fullname(
    resolver: NativeTypeResolver, type_bytes: bytes, method_name: str
) -> str | None: ...
def rust_is_enum_callable_base(callable_node: Expression | None, enum_bases: Any) -> bool: ...
def rust_classify_protocol_test_callee(callee: Expression, n_args: int) -> str | None: ...
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
def rust_infer_constraints(
    template_bytes: bytes, actual_bytes: bytes, direction: int
) -> list[bytes] | None: ...
def rust_infer_constraints_full(
    resolver: NativeTypeResolver,
    template_bytes: bytes,
    actual_bytes: bytes,
    direction: int,
    skip_neg_op: bool,
    erase_types: bool,
) -> list[bytes] | None: ...
def rust_select_trivial(options_bytes: bytes) -> bytes | None: ...
def rust_exclude_non_meta_vars(option_bytes: bytes) -> bytes | None: ...
def rust_is_similar_constraints(x_bytes: bytes, y_bytes: bytes) -> bool | None: ...
def rust_any_constraints(
    options_bytes: bytes, eager: bool, resolver: NativeTypeResolver
) -> list[bytes] | None: ...
def rust_repack_callable_args(
    callable_bytes: bytes, resolver: NativeTypeResolver
) -> list[bytes] | None: ...
def rust_merge_with_any(constraint_bytes: bytes) -> bool | None: ...
def rust_filter_satisfiable(
    option_bytes: bytes, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_is_same_constraints(
    x_bytes: bytes, y_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_filter_imprecise_kinds(constraints_bytes: bytes) -> bytes | None: ...
def rust_infer_directed_arg_constraints(
    resolver: NativeTypeResolver,
    left_bytes: bytes,
    right_bytes: bytes,
    direction: int,
) -> bytes | None: ...
def rust_infer_callable_arguments_constraints(
    resolver: NativeTypeResolver,
    template_bytes: bytes,
    actual_bytes: bytes,
    direction: int,
) -> bytes | None: ...
def rust_is_type_type(tp_bytes: bytes) -> bool | None: ...
def rust_skip_reverse_union_constraints(constraints_bytes: bytes) -> bytes | None: ...
def rust_unwrap_type_type(tp_bytes: bytes) -> bytes | None: ...
def rust_classify_call(callee_bytes: bytes) -> int | None: ...
def rust_classify_typeddict_call(
    args: list[Expression], arg_kinds: list[int]
) -> int | None: ...
def rust_classify_reveal_imported(
    kind: int, is_imported: bool, unimported_reveal_enabled: bool
) -> str | None: ...
def rust_refers_to_typeddict(base: Any, target_bytes: bytes | None = None) -> bool: ...
def rust_classify_super_arg_types(chk: Any, super_expr: Any) -> int | None: ...
def rust_classify_visit_op_expr(expr: Any) -> int | None: ...
def rust_classify_index_with_type(left_type: Any, chk: Any, expand_variadic: bool) -> int | None: ...
def rust_calibrate_type_obj_return(callee_bytes: bytes, arg_type_bytes: bytes) -> bytes | None: ...
def rust_normalize_callable(callee_bytes: bytes) -> bytes | None: ...
def rust_check_callable_call(
    resolver: NativeTypeResolver,
    callee_bytes: bytes,
    arg_types_bytes: list[bytes],
    callable_name: str | None,
    object_type_present: bool,
    registry: Any,
    has_user_plugins: bool,
    plugins: Any,
) -> bytes | None: ...
def rust_real_union(
    resolver: NativeTypeResolver, type_bytes: bytes, strict_optional: bool
) -> bool | None: ...
def rust_solve_generic_call(
    resolver: NativeTypeResolver,
    callee_bytes: bytes,
    arg_types_bytes: list[bytes],
    formal_to_actual: list[list[int]],
    strict: bool,
    infer_unions: bool,
    strict_optional: bool,
) -> bytes | None: ...
def rust_get_arg_infer_passes(
    resolver: NativeTypeResolver,
    formal_bytes: list[bytes],
    actual_bytes: list[bytes],
    lambda_flags: list[bool],
    formal_to_actual: list[list[int]],
    num_actuals: int,
) -> list[int] | None: ...
def rust_possible_none_type_var_overlap(
    resolver: NativeTypeResolver,
    arg_type_bytes: list[bytes],
    target_bytes: list[bytes],
) -> bool | None: ...
def rust_bind_self_fast(method_bytes: bytes) -> bytes | None: ...
def rust_classify_member_access(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> int | None: ...
def rust_instance_fallback(type_bytes: bytes) -> bytes | None: ...
def rust_has_operator(
    resolver: NativeTypeResolver, type_bytes: bytes, op_method: str, strict_optional: bool
) -> bool | None: ...
def rust_meta_has_operator(
    resolver: NativeTypeResolver, type_bytes: bytes, op_method: str
) -> bool | None: ...
def rust_defined_in_superclass(
    resolver: NativeTypeResolver, fullname: str, name: str
) -> bool | None: ...
def rust_analyze_instance_member_access(
    resolver: NativeTypeResolver,
    instance: bytes,
    signature: bytes,
    method_fullname: str,
    strict_optional: bool,
    is_trivial_self: bool,
) -> bytes | None: ...
def rust_analyze_member_method(
    resolver: NativeTypeResolver,
    instance: bytes,
    signature: bytes,
    method_fullname: str,
    self_type: bytes,
    name: str,
    strict_optional: bool,
    is_class: bool,
) -> bytes | None: ...
def rust_solve_one(
    lowers: list[bytes],
    uppers: list[bytes],
    infer_unions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[int, bytes | None] | None: ...
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
) -> str | None: ...
def rust_format_type(
    type_bytes: bytes,
    resolver: NativeTypeResolver,
    verbosity: int,
    module_names: bool,
    use_star_unpack: bool,
) -> str | None: ...
def rust_format_type_distinctly(
    type_bytes_list: list[bytes],
    resolver: NativeTypeResolver,
    bare: bool,
    use_star_unpack: bool,
) -> list[str] | None: ...
def rust_append_invariance_notes(
    arg_bytes: bytes,
    expected_bytes: bytes,
    resolver: NativeTypeResolver,
) -> list[str] | None: ...
def rust_append_numbers_notes(expected_bytes: bytes) -> list[str] | None: ...
def rust_append_union_note(
    arg_bytes: bytes,
    expected_bytes: bytes,
    resolver: NativeTypeResolver,
    use_star_unpack: bool,
) -> list[str] | None: ...
def rust_pretty_callable(
    callable_bytes: bytes,
    resolver: NativeTypeResolver,
    reveal_verbose_types: bool,
    use_star_unpack: bool,
) -> str | None: ...
def rust_best_matches(current: str, options: list[str], n: int) -> list[str]: ...
def rust_analyze_conversion_specifiers(
    specs: list[tuple[bool, str, str, str]],
) -> tuple[bool, bool, bool] | None: ...
def rust_is_numeric_format_type(conv_type: str, is_new_style: bool) -> bool: ...
def rust_parse_conversion_specifiers(
    format_str: str,
) -> list[tuple[str, int, str | None, str, str, str, str]]: ...
def rust_parse_placeholder_format(
    format_spec: str,
) -> tuple[str | None, str | None, str | None, bool, bool, str, str | None, str, str] | None: ...
def rust_find_non_escaped_targets(
    format_value: str,
) -> tuple[int, list[tuple[str, int]]]: ...
def rust_parse_format_value(
    format_value: str,
) -> tuple[int, list[tuple[str, int, str | None, str, str, str, str, str | None, bool, str | None, str | None]]]: ...
def rust_is_uninhabited(t_bytes: bytes, resolver: NativeTypeResolver) -> bool | None: ...
def rust_get_match_arg_names(
    t_bytes: bytes, resolver: NativeTypeResolver
) -> list[str | None] | None: ...
def rust_get_type_range(type_bytes: bytes) -> bool | None: ...
def rust_should_self_match(
    type_bytes: bytes,
    has_match_args: bool,
    self_match_types_bytes: bytes,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_can_match_sequence(
    type_bytes: bytes,
    non_seq_types_bytes: bytes,
    sequence_type_bytes: bytes,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_contract_starred_pattern_types(
    types_bytes: list[bytes],
    star_pos: int | None,
    num_patterns: int,
    resolver: NativeTypeResolver,
) -> list[bytes] | None: ...
def rust_expand_starred_pattern_types(
    types_bytes: list[bytes],
    star_pos: int | None,
    num_types: int,
    original_unpack: bool,
    resolver: NativeTypeResolver,
) -> list[bytes] | None: ...
def rust_construct_sequence_child(
    outer_bytes: bytes,
    empty_type_bytes: bytes,
    sequence_bytes: bytes,
    resolver: NativeTypeResolver,
) -> bytes | None: ...
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
def rust_make_any_non_explicit(type_bytes: bytes) -> bytes | None: ...
def rust_make_any_non_unimported(type_bytes: bytes) -> bytes | None: ...
def rust_is_unreachable_map(type_bytes_list: list[bytes]) -> bool | None: ...
def rust_stmt_outcome(node_bytes: bytes) -> str | None: ...
def rust_type_requires_usage(type_bytes: bytes, resolver: NativeTypeResolver) -> int | None: ...
def rust_with_exit_suppresses(type_bytes: bytes, strict_optional: bool) -> bool: ...
def rust_try_handler_union(
    type_bytes: bytes, strict_optional: bool
) -> list[bytes] | None: ...
def rust_classify_except_handler_tests(
    type_bytes_list: list[bytes], resolver: NativeTypeResolver
) -> list[tuple[int, bytes | None]] | None: ...
def rust_is_true_literal(node: Any) -> bool: ...
def rust_is_false_literal(node: Any) -> bool: ...
def rust_is_literal_none(node: Any) -> bool: ...
def rust_is_literal_not_implemented(node: Any) -> bool: ...
def rust_is_static(func: Any) -> bool: ...
def rust_is_property(defn: Any) -> bool: ...
def rust_is_method(node: Any) -> bool: ...
def rust_is_empty_generator_function(func: Any) -> bool: ...
def rust_is_settable_property(defn: Any) -> bool: ...
def rust_is_custom_settable_property(defn: Any) -> bool: ...
def rust_can_have_shared_disjoint_base(instances: list[Any]) -> bool: ...
def rust_check_operator(
    resolver: NativeTypeResolver,
    op_name: str,
    left: bytes,
    right: bytes,
    strict_optional: bool,
) -> int | None: ...
def rust_is_trivial_bound(type_bytes: bytes, allow_tuple: bool) -> bool | None: ...
def rust_find_linear(
    constraint_bytes: bytes,
) -> tuple[bool, tuple[int, int, str] | None] | None: ...
def rust_separate_union_literals(
    type_bytes: bytes,
) -> tuple[list[bytes], list[bytes]] | None: ...
def rust_get_type_vars(type_bytes: bytes, include_all: bool) -> list[bytes] | None: ...
def rust_get_vars(
    target_bytes: bytes, vars: list[tuple[int, int, str]]
) -> list[tuple[int, int, str]] | None: ...
def rust_is_callable_protocol(
    resolver: NativeTypeResolver, t_bytes: bytes
) -> bool | None: ...
def rust_solve_constraints(
    vars_bytes: list[bytes],
    dependent_vars_bytes: list[bytes],
    constraints_bytes: list[bytes],
    strict: bool,
    infer_unions: bool,
    strict_optional: bool,
    skip_unsatisfied: bool,
    resolver: NativeTypeResolver,
) -> tuple[int, bytes | None, bytes | None] | None: ...
def rust_infer_function_type_arguments(
    resolver: NativeTypeResolver,
    callee: bytes,
    arg_types: list[bytes | None],
    arg_kinds: list[int],
    formal_to_actual: list[list[int]],
    strict: bool,
    infer_unions: bool,
    strict_optional: bool,
) -> bytes | None: ...
def rust_solve_dependent(
    vars_bytes: list[bytes],
    constraints_bytes: list[bytes],
    infer_unions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[int, bytes | None, bytes | None] | None: ...
def rust_replace_implicit_first_type(
    sig_bytes: bytes, new_type_bytes: bytes
) -> bytes | None: ...
def rust_callables_compatible(
    left_bytes: bytes,
    right_bytes: bytes,
    proper_subtype: bool,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_are_parameters_compatible(
    left_bytes: bytes,
    right_bytes: bytes,
    is_proper_subtype: bool,
    ignore_pos_arg_names: bool,
    allow_partial_overlap: bool,
    strict_concatenate_check: bool,
    strict_optional: bool,
    nested_proper_subtype: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_are_args_compatible(
    left: Any,
    right: Any,
    ignore_pos_arg_names: bool,
    allow_partial_overlap: bool,
    allow_imprecise_kinds: bool,
) -> int | None: ...
def rust_classify_type_parameter(
    left: Any,
    variance: int,
    proper_subtype: bool,
) -> int | None: ...
def rust_is_overlapping_types(
    left_bytes: bytes,
    right_bytes: bytes,
    ignore_promotions: bool,
    overlap_for_overloads: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_is_equivalent(
    a_bytes: bytes,
    b_bytes: bytes,
    ignore_type_params: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_is_same_type(
    a_bytes: bytes,
    b_bytes: bytes,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_all_same_types(
    items_bytes: list[bytes],
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_is_more_precise(
    left_bytes: bytes,
    right_bytes: bytes,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_is_erased_instance(t_bytes: bytes) -> bool | None: ...
def rust_has_underscore_prefix(name: str) -> bool: ...
def rust_try_restrict_literal_union(
    t_bytes: bytes, s_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> list[bytes] | None: ...
def rust_restrict_subtype_away(
    t_bytes: bytes,
    s_bytes: bytes,
    consider_runtime_isinstance: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bytes | None: ...
def rust_custom_special_method(
    type_bytes: bytes, name: str, check_all: bool, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_get_protocol_member(
    left_bytes: bytes,
    original_left_bytes: bytes,
    member: str,
    class_obj: bool,
    is_lvalue: bool,
    resolver: NativeTypeResolver,
) -> bytes | None: ...
def rust_transform_attrs(
    fields_bytes: bytes,
    class_fullname: str,
    init_name: str,
    add_order: bool,
) -> bytes | None: ...
def rust_tuple_fallback(t_bytes: bytes, resolver: NativeTypeResolver) -> bytes | None: ...
def rust_bind_self(method_bytes: bytes) -> bytes | None: ...
def rust_fill_typevars(typ: TypeInfo) -> bytes | None: ...
def rust_class_callable(
    init_wire: bytes,
    explicit_wire: bytes | None,
    default_ret_wire: bytes,
    is_new: bool,
    is_eq: bool,
    is_st: bool,
    info: TypeInfo,
) -> tuple[bytes, list[bytes]] | None: ...
def rust_function_type(func: Any, fallback_wire: bytes) -> tuple[bool, bytes] | None: ...
def rust_callable_type(
    fdef: Any, fallback_wire: bytes, ret_type_wire: bytes | None
) -> bytes | None: ...
def rust_erase_to_bound(t_bytes: bytes) -> bytes | None: ...
def rust_map_type_from_supertype(
    resolver: NativeTypeResolver,
    sub_info: TypeInfo,
    super_info: TypeInfo,
    type_bytes: bytes,
    strict_optional: bool,
) -> bytes | None: ...
def rust_coerce_to_literal(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_is_singleton_identity_type(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_is_singleton_equality_type(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_refers_to_fullname(node: Expression, fullnames: str | tuple[str, ...]) -> bool: ...
def rust_refers_to_class_or_function(node: Expression) -> bool: ...
def rust_is_trivial_body(block: Block) -> bool: ...
def rust_find_duplicate(list: list[T]) -> T | None: ...
def rust_is_valid_replacement(old: SymbolTableNode, new: SymbolTableNode) -> bool: ...
def rust_is_same_symbol(a: SymbolNode | None, b: SymbolNode | None) -> bool: ...
def rust_names_modified_in_lvalue(lvalue: Lvalue) -> list[NameExpr]: ...
def rust_names_modified_by_assignment(s: AssignmentStmt) -> list[NameExpr]: ...
def rust_remove_imported_names_from_symtable(names: SymbolTable, module: str) -> None: ...
def rust_should_wait_rhs(semanal: Any, rv: Expression) -> bool | None: ...
def rust_apply_semantic_analyzer_patches(patches: list[tuple[int, Callable[[], None]]]) -> None: ...
def rust_classify_decorators(
    decorators: list[Expression],
    name_sets: tuple[
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
    ],
) -> list[str] | None: ...
def rust_classify_class_decorator(
    decorator: Expression,
    name_sets: tuple[
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
        str | tuple[str, ...],
    ],
) -> tuple[str, str | None] | None: ...
def rust_classify_imports(
    ids: list[tuple[str, str | None]],
    is_stub_file: bool,
    implicit_reexport: bool,
    modules: dict[str, MypyFile],
    scope_stack: list[int],
    self_type: TypeInfo | None,
) -> list[tuple[str, str, bool, int | None]] | None: ...
def rust_classify_member_resolution(
    expr: Expression,
    member_expr_cls: type[MemberExpr],
    ref_expr_cls: type[RefExpr],
    mypy_file_cls: type[MypyFile],
    type_info_cls: type[TypeInfo],
    type_alias_cls: type[TypeAlias],
) -> tuple[str | None, SymbolTableNode | None]: ...
def rust_lookup(
    name: str,
    global_decls: set[str],
    globals: SymbolTable,
    nonlocal_decls: set[str],
    locals: list[SymbolTable | None],
    type_names: SymbolTable | None,
    is_func_scope: bool,
) -> tuple[str, SymbolTableNode | None] | None: ...
def rust_stubgen_render(expr: Expression) -> str | None: ...
def rust_stubgen_render_type_args(items: list[Expression]) -> str | None: ...
def rust_get_assigned_names(lvalues: list[Expression]) -> list[str]: ...
def rust_is_none_expr(expr: Expression) -> bool: ...
def rust_is_pybind11_overloaded_function_docstring(docstring: str, name: str) -> bool: ...
def rust_method_name_sort_key(name: str) -> tuple[int, str]: ...
def rust_dataclass_transform(
    fields_bytes: bytes,
    class_fullname: str,
    decorator_init: bool,
    decorator_eq: bool,
    decorator_order: bool,
    decorator_frozen: bool,
) -> bytes | None: ...
def rust_dataclass_post_init_transform(
    fields_bytes: bytes,
    class_fullname: str,
) -> bytes | None: ...
def rust_find_shallow_matching_overload_item(
    overload: Any,
    call: Any,
) -> int | None: ...
def rust_var_is_typing_special_form(node: Any) -> bool: ...
def rust_get_typevarlike_declaration(
    s: AssignmentStmt,
    typevarlike_types: tuple[str, ...],
) -> CallExpr | None: ...
def rust_parse_bool(expr: Expression) -> bool | None: ...
def rust_is_mangled_global(name: str, globals: dict[str, Any]) -> bool: ...
def rust_is_initial_mangled_global(name: str) -> bool: ...
def rust_is_final_redefinition(
    kind: int,
    name: str,
    globals: dict[str, Any],
    type_names: Any,
) -> bool: ...
def rust_is_same_var_from_getattr(a: Any, b: Any) -> bool: ...
def rust_can_possibly_be_typevarlike_declaration(s: AssignmentStmt) -> bool: ...
def rust_can_possibly_be_type_form(s: AssignmentStmt, is_pep_613_annot: bool) -> bool | None: ...
def rust_is_type_ref(rv: Expression, bare: bool) -> bool | None: ...
def rust_can_be_type_alias(
    rv: Expression,
    allow_none: bool,
    is_stub_file: bool,
) -> bool | None: ...
def rust_check_typevarlike_name(
    call: CallExpr,
    name: str,
) -> tuple[bool, str | None] | None: ...
def rust_check_decorated_function_is_method(
    semanal: Any,
) -> bool | None: ...
def rust_classify_method_signature(
    func: Any,
    self_type_wire: bytes | None,
    unanalyzed_kind: int,
    expected_self: bool | None,
    has_self_type: bool,
) -> tuple[bool, bool, int] | None: ...
def rust_extract_typevarlike_name(
    s: AssignmentStmt,
    call: CallExpr,
) -> str | None: ...
def rust_is_defined_type_param(locals: list[SymbolTable | None], name: str) -> bool: ...
def rust_classify_setup_type_vars(
    tvar_defs: list[TypeVarLikeType],
    has_defaults: list[bool],
) -> list[int] | None: ...
def rust_visit_list_set_expr(expr: Expression, semanal: object) -> bool: ...
def rust_visit_dict_expr(expr: Expression, semanal: object) -> bool: ...
def rust_visit_template_str_expr(expr: Expression, semanal: object) -> bool: ...
def rust_visit_unary_expr(expr: Expression, semanal: object) -> bool: ...
def rust_visit_comparison_expr(expr: Expression, semanal: object) -> bool: ...
def rust_visit_slice_expr(expr: Expression, semanal: object) -> bool: ...
def rust_visit_conditional_expr(expr: Expression, semanal: object) -> bool: ...
def rust_visit_super_expr(
    expr: Expression, semanal: object, the_type: object
) -> bool: ...
def rust_visit_raise_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_assert_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_operator_assignment_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_block(block: Block, semanal: object) -> bool: ...
def rust_visit_if_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_is_valid_del_target(expr: Expression) -> bool: ...
def rust_visit_del_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_expression_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_break_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_continue_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_global_decl(stmt: Node, semanal: object) -> bool: ...
def rust_visit_match_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_return_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_block_maybe(block: Block | None, semanal: object) -> bool: ...
def rust_visit_while_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_name_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_star_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_as_pattern(pattern: Node, semanal: object) -> bool: ...
def rust_visit_or_pattern(pattern: Node, semanal: object) -> bool: ...
def rust_visit_value_pattern(pattern: Node, semanal: object) -> bool: ...
def rust_visit_sequence_pattern(pattern: Node, semanal: object) -> bool: ...
def rust_visit_starred_pattern(pattern: Node, semanal: object) -> bool: ...
def rust_visit_mapping_pattern(pattern: Node, semanal: object) -> bool: ...
def rust_visit_class_pattern(pattern: Node, semanal: object) -> bool: ...
def rust_visit_yield_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_yield_from_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_await_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_try_stmt(stmt: Node, semanal: object) -> bool: ...
def rust_visit_op_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_index_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_cast_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_type_form_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_assert_type_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_reveal_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_type_application(expr: Node, semanal: object) -> bool: ...
def rust_visit_list_comprehension(expr: Node, semanal: object) -> bool: ...
def rust_visit_set_comprehension(expr: Node, semanal: object) -> bool: ...
def rust_visit_dictionary_comprehension(expr: Node, semanal: object) -> bool: ...
def rust_visit_generator_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_lambda_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_overloaded_func_def(defn: Node, semanal: object) -> bool: ...
def rust_visit_class_def(defn: Node, semanal: object) -> bool: ...
def rust_visit_func_def(defn: Node, semanal: object) -> bool: ...
def rust_visit_nonlocal_decl(d: Node, semanal: object) -> bool: ...
def rust_visit_for_stmt(s: Node, semanal: object) -> bool: ...
def rust_visit_with_stmt(s: Node, semanal: object) -> bool: ...
def rust_visit_assignment_expr(s: Node, semanal: object) -> bool: ...
def rust_visit_import_all(i: Node, semanal: object) -> bool: ...
def rust_visit_import_from(imp: Node, semanal: object) -> bool: ...
def rust_visit_assignment_stmt(s: Node, semanal: object) -> bool: ...
def rust_visit_import(i: Node, semanal: object) -> bool: ...
def rust_visit_call_expr(expr: Node, semanal: object) -> bool: ...
def rust_visit_type_alias_stmt(s: Node, semanal: object) -> bool: ...
def rust_special_function_elide_names(name: str) -> bool: ...
def rust_argument_elide_name(name: str | None) -> bool: ...
def rust_set_callable_name(sig: Type, fdef: FuncDef) -> ProperType | None: ...
def rust_has_placeholder(typ: Type) -> bool | None: ...
def rust_calculate_tuple_fallback(typ: TupleType) -> Type | None: ...
def rust_find_dataclass_transform_spec(node: Node | None) -> DataclassTransformSpec | None: ...

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
def rust_trim_source_line(line: str, max_len: int, col: int, min_width: int) -> tuple[str, int]: ...
def rust_get_mypy_comments(source: str) -> list[tuple[int, str]]: ...
def rust_get_prefix(fullname: str) -> str: ...
def rust_correct_relative_import(
    cur_mod_id: str, relative: int, target: str, is_cur_package_init_file: bool
) -> tuple[str, bool]: ...
def rust_unmangle(name: str) -> str: ...
def rust_get_unique_redefinition_name(name: str, existing: list[str]) -> str: ...
def rust_count_stats(messages: list[str]) -> tuple[int, int, int]: ...
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

# Issue #568: semanal_classprop functions (live PyO3 objects)
def rust_calculate_class_abstract_status(
    typ: TypeInfo, is_stub_file: bool, errors: Any
) -> None: ...
def rust_check_protocol_status(info: TypeInfo, errors: Any) -> None: ...
def rust_calculate_class_vars(info: TypeInfo) -> None: ...
def rust_add_type_promotion(
    info: TypeInfo,
    module_names: SymbolTable,
    options: Any,
    builtin_names: SymbolTable | None,
) -> None: ...

# Issue #570: fixup functions (live PyO3 objects)
def rust_fixup_type(
    typ: Any, modules: dict[str, MypyFile], allow_missing: bool
) -> bool: ...
def rust_fixup_type_info(
    info: TypeInfo, modules: dict[str, MypyFile], allow_missing: bool
) -> bool: ...
def rust_resolve_cross_ref(
    value: SymbolTableNode, modules: dict[str, MypyFile], allow_missing: bool
) -> bool: ...
def rust_fixup_symbol_table(
    symtab: SymbolTable, modules: dict[str, MypyFile], allow_missing: bool
) -> bool: ...
def rust_fixup_overloaded_func_def(
    o: OverloadedFuncDef, modules: dict[str, MypyFile], allow_missing: bool
) -> bool: ...
def rust_fixup_decorator(
    d: Decorator, modules: dict[str, MypyFile], allow_missing: bool
) -> bool: ...

# Issue #572: binder.get_declaration (live PyO3 object)
def rust_get_declaration(expr: Expression) -> Type | None: ...

# Issue #574: reachability functions (live PyO3 objects)
def rust_infer_condition_value(expr: Expression, options: Any) -> int: ...
def rust_infer_pattern_value(pattern: Any) -> int: ...
def rust_assert_will_always_fail(stmt: Any, options: Any) -> bool: ...
def rust_consider_sys_version_info(expr: Expression, pyversion: tuple[int, ...]) -> int: ...
def rust_consider_sys_platform(expr: Expression, platform: str) -> int: ...
def rust_is_sys_attr(expr: Expression, name: str) -> bool: ...
def rust_contains_sys_version_info(expr: Expression) -> None | int | tuple[int | None, int | None]: ...
def rust_contains_int_or_tuple_of_ints(expr: Expression) -> None | int | tuple[int, ...]: ...
def rust_fixed_comparison(left: Any, op: str, right: Any) -> int: ...

# Issue #576: semanal_typeddict/namedtuple functions (live PyO3 objects)
def rust_primary_source(sources: list[Any]) -> Any: ...
def rust_check_namedtuple_field_name(field: str, seen_names: Any) -> str | None: ...
def rust_verify_requiredness_compatibility(
    field_name: str, source: Any, is_required: bool, primary_source_base: Any
) -> str | None: ...
def rust_verify_field_against_closed_bases(
    field_name: str, closed_bases: Any, primary_source_base: Any
) -> list[str]: ...

# Issue #578: typeanal_queries functions (live PyO3 objects)
def rust_validate_instance(t: Any, fail: Any, indexed: bool) -> bool | None: ...
def rust_detect_diverging_alias(node: Any, target: Any) -> bool | None: ...
def rust_find_self_type(typ: Any, lookup: Any) -> bool | None: ...
def rust_check_vec_type_args(args: Any, ctx: Any, api: Any) -> bool | None: ...
def rust_check_unpacks_in_list(items: Any) -> tuple[list[int], int | None] | None: ...

# Phase D (self-check repair): the 73 functions whose Python call sites were
# discovered missing from this stub, sorted by Python consumption module; the
# Rust `py: Python` GIL token is injected by PyO3, not part of the signature.

# mypy/checkmember.py — member-access resolution (resolver is first).
def rust_analyze_member_access(
    resolver: NativeTypeResolver,
    name: str,
    typ_bytes: bytes,
    self_type_bytes: bytes,
    is_lvalue: bool,
    is_super: bool,
    preserve_type_var_ids: bool,
    start_raw_id: int,
    strict_optional: bool,
) -> tuple[int, bool, bytes] | None: ...
def rust_analyze_union_member_access(
    resolver: NativeTypeResolver,
    union_bytes: bytes,
    name: str,
    is_lvalue: bool,
    is_super: bool,
    _no_deferral: bool,
    preserve_type_var_ids: bool,
    start_raw_id: int,
    strict_optional: bool,
) -> tuple[int, bool, list[bytes]] | None: ...
def rust_analyze_none_member_access(
    resolver: NativeTypeResolver, name: str, typ_bytes: bytes, strict_optional: bool
) -> bytes | None: ...
def rust_analyze_typeddict_access(
    resolver: NativeTypeResolver, name: str, typ_bytes: bytes, strict_optional: bool
) -> bytes | None: ...
def rust_analyze_enum_class_attribute_access(
    resolver: NativeTypeResolver, instance_bytes: bytes, name: str
) -> bytes | None: ...
def rust_analyze_descriptor_access(
    resolver: NativeTypeResolver,
    descriptor_bytes: bytes,
    is_lvalue: bool,
    strict_optional: bool,
) -> bytes | None: ...
def rust_descriptor_has_get_set(
    resolver: NativeTypeResolver, descriptor_bytes: bytes
) -> tuple[bool, bool] | None: ...
def rust_classify_type_type_member_access(typ: Any) -> int | None: ...
def rust_is_instance_var(var: Var) -> bool | None: ...
def rust_check_self_arg(
    resolver: NativeTypeResolver,
    functype_bytes: bytes,
    dispatched_arg_type_bytes: bytes,
    is_classmethod: bool,
    name: str,
    strict_optional: bool,
) -> tuple[int, bool, bytes] | None: ...
def rust_expand_without_binding(
    typ_bytes: bytes,
    itype_bytes: bytes,
    preserve_type_var_ids: bool,
    has_self_type: bool,
    start_raw_id: int,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[int, bool, bytes] | None: ...
def rust_expand_and_bind_callable(
    functype_bytes: bytes,
    itype_bytes: bytes,
    is_trivial_self: bool,
    is_property: bool,
    preserve_type_var_ids: bool,
    start_raw_id: int,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[int, bool, bytes] | None: ...
def rust_add_class_tvars(
    resolver: NativeTypeResolver,
    t_bytes: bytes,
    isuper_bytes: bytes,
    is_classmethod: bool,
    is_trivial_self: bool,
    preserve_type_var_ids: bool,
    original_vars_bytes: bytes,
    start_raw_id: int,
    strict_optional: bool,
) -> tuple[int, bool, bytes] | None: ...

# mypy/checkexpr.py — check_argument_count, overload merge, arg similarity,
# tuple index/slice helpers, int-literal extraction.
def rust_check_argument_count(
    callee_bytes: bytes,
    actual_types_bytes: list[bytes],
    actual_kinds: list[int],
    actual_names: list[str | None],
    formal_to_actual: list[list[int]],
    special_sig: str | None,
    object_type_present: bool,
    callable_name: str | None,
    in_checked_function: bool,
) -> tuple[bool, list[tuple[int, int, int]], bool] | None: ...
def rust_combine_function_signatures(
    resolver: NativeTypeResolver, types_bytes: list[bytes], start_raw_id: int, strict_optional: bool
) -> tuple[int, bytes] | None: ...
def rust_arg_approximate_similarity(
    actual_bytes: bytes, formal_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_visit_tuple_index_helper(
    items_bytes: list[bytes],
    partial_fallback_bytes: bytes,
    n: int,
    line: int,
    column: int,
    min_length: int,
) -> bytes | None: ...
def rust_visit_tuple_slice_helper(
    items_bytes: list[bytes],
    partial_fallback_bytes: bytes,
    begin: int | None,
    end: int | None,
    stride: int | None,
    line: int,
    column: int,
) -> bytes | None: ...
def rust_try_getting_int_literals(type_bytes: bytes) -> list[int] | None: ...
def rust_visit_temp_node(type_bytes: bytes) -> bytes | None: ...
def rust_visit_promote_expr(type_bytes: bytes) -> bytes | None: ...
def rust_visit_paramspec_expr() -> bytes: ...
def rust_visit_type_var_tuple_expr() -> bytes: ...
def rust_visit_newtype_expr() -> bytes: ...

# mypy/checker.py — conditional maps, generator/coroutine return helpers,
# valid-inferred-type query, custom-eq query.
def rust_and_conditional_maps(
    keys1: list[int],
    values1: list[bytes],
    keys2: list[int],
    values2: list[bytes],
    use_meet: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[list[int], list[bytes]] | None: ...
def rust_or_conditional_maps(
    keys1: list[int],
    values1: list[bytes],
    keys2: list[int],
    values2: list[bytes],
    coalesce_any: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[list[int], list[bytes]] | None: ...
def rust_is_generator_return_type(
    typ_bytes: bytes, is_coroutine: bool, strict_optional: bool, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_is_async_generator_return_type(
    typ_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_get_generator_yield_type(
    return_type_bytes: bytes, is_coroutine: bool, strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_get_generator_receive_type(
    return_type_bytes: bytes, is_coroutine: bool, strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_get_generator_return_type(
    return_type_bytes: bytes, is_coroutine: bool, strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...
def rust_get_coroutine_return_type(return_type_bytes: bytes) -> bytes | None: ...
def rust_group_comparison_operands(
    ops_and_indices: list[tuple[str, int, int]],
    literal_hashes: dict[int, int],
    operators_to_group: list[str],
) -> list[tuple[str, list[int]]]: ...
def rust_is_valid_inferred_type(
    typ_bytes: bytes, is_lvalue_final: bool, is_lvalue_member: bool, allow_redefinition: bool
) -> bool | None: ...
def rust_has_custom_eq_checks(typ_bytes: bytes, resolver: NativeTypeResolver) -> bool | None: ...

# mypy/types.py — callable formal-arg introspection + copy_modified.
def rust_callable_formal_arguments(
    typ_bytes: bytes
) -> list[tuple[str | None, int | None, bool]] | None: ...
def rust_callable_argument_by_name(
    typ_bytes: bytes, name: str | None
) -> tuple[str | None, int | None, bool] | None: ...
def rust_callable_argument_by_position(
    typ_bytes: bytes, position: int | None
) -> tuple[str | None, int | None, bool] | None: ...
def rust_callable_is_generic(typ_bytes: bytes) -> bool | None: ...
def rust_callable_is_kw_arg(typ_bytes: bytes) -> bool | None: ...
def rust_callable_is_var_arg(typ_bytes: bytes) -> bool | None: ...
def rust_callable_max_possible_positional_args(typ_bytes: bytes) -> int | None: ...
def rust_callable_min_args(typ_bytes: bytes) -> int | None: ...
def rust_can_be_false_default(typ_bytes: bytes) -> bool | None: ...
def rust_can_be_true_default(typ_bytes: bytes) -> bool | None: ...
def rust_can_be_false_default_live(
    typ_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_can_be_true_default_live(
    typ_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_tuple_length(typ_bytes: bytes) -> int | None: ...
def rust_union_length(typ_bytes: bytes) -> int | None: ...
def rust_copy_modified(typ_bytes: bytes, field: str, value_bytes: bytes) -> bytes | None: ...

# mypy/nodes.py — live-node classification queries (PyO3 objects).
def rust_decorator_is_dynamic(dec: Any) -> bool: ...
def rust_func_has_self_or_cls_argument(func: Any) -> bool: ...
def rust_func_item_is_dynamic(func: Any) -> bool: ...
def rust_overloaded_is_dynamic(func: Any) -> bool: ...
def rust_typeinfo_is_generic(info: Any) -> bool: ...
def rust_typeinfo_is_metaclass(info: Any, precise: bool) -> bool: ...
def rust_typeinfo_has_base(info: Any, fullname: str) -> bool: ...

# mypy/typeanal.py — wire round-trip analysis.
def rust_type_analyze(
    typ_bytes: bytes,
    allow_tuple_literal: bool = False,
    allow_param_spec_literals: bool = False,
    allow_unpack: bool = False,
) -> bytes | None: ...
def rust_classify_special_unbound(
    fullname: str,
    arg_count: int,
    empty_tuple_index: bool,
    allow_typed_dict_special_forms: bool,
    tuple_missing_or_placeholder: bool,
    tuple_ellipsis_form: bool,
    not_in_final: bool,
    not_in_tuple: bool,
    not_in_type: bool,
    not_in_typeform: bool,
    not_in_classvar: bool,
    not_in_never: bool,
    not_in_annotated: bool,
    not_in_required: bool,
    not_in_notrequired: bool,
    not_in_readonly: bool,
    not_in_literal: bool,
    not_in_unpack: bool,
    not_in_self: bool,
    allow_unpack: bool,
) -> int | None: ...
def rust_classify_tuple_type_implicit(
    implicit: bool,
    allow_tuple_literal: bool,
    items_len: int,
) -> int | None: ...
def rust_classify_class_pattern_ranges(
    typ_bytes: bytes,
    class_ref_node: Any,
) -> list[int] | None: ...
def rust_classify_raw_expression_type(
    report_invalid_types: bool,
    base_type_name: str,
    note_is_none: bool,
) -> int | None: ...
def rust_classify_check_warn_deprecated(
    deprecated: str | None,
    is_typeshed_stub: bool,
    api_type_fullname: str | None,
    info_fullname: str,
    info_name: str,
    deprecated_calls_exclude: list[str],
    report_deprecated_as_note: bool,
    import_from_names: list[str],
) -> int | None: ...
def rust_classify_analyze_callable_type(
    arg_count: int,
    arg0_is_type_list: bool,
    arg0_is_ellipsis: bool,
    disallow_any_generics: bool,
) -> int | None: ...

# mypy/semanal.py — decorator/semanal-visitor helpers.
def rust_erase_func_annotations(func: Any) -> None: ...
def rust_get_deprecated(expression: Any) -> str | None: ...
def rust_get_name_repr_of_expr(expr: Any) -> str | None: ...
def rust_is_init_only(node: Any) -> bool: ...
def rust_lookup_qualified(
    resolver: NativeTypeResolver,
    name: str,
    first_sym_kind: int,
    first_sym_fullname: str,
    first_sym_is_any: bool,
) -> tuple[int, str] | None: ...

# mypy/server/deps.py — D2-style import-graph triggers.
def rust_compute_wildcard_triggers(
    changed_names: list[str], package_nesting_level: int
) -> list[str] | None: ...
def rust_compute_target_modules(
    triggers: list[str],
    deps: list[tuple[str, list[str]]],
    up_to_date_modules: list[str],
    module_ids: list[str],
) -> list[str]: ...

# mypy/server/update.py — daemon update helpers.
def rust_dedupe_modules(modules: list[tuple[str, str]]) -> list[tuple[str, str]]: ...
def rust_get_module_to_path_map(graph: Any) -> list[tuple[str, str]]: ...
def rust_get_sources(changed_modules: list[tuple[str, str]], followed: bool) -> list[Any]: ...
def rust_extract_fnam_from_message(message: str) -> str | None: ...
def rust_extract_possible_fnam_from_message(message: str) -> str: ...
def rust_sort_messages_preserving_file_order(
    messages: list[str], prev_messages: list[str]
) -> list[str]: ...
def rust_find_relative_leaf_module(
    modules: list[tuple[str, str]], deps: dict[str, list[str]]
) -> tuple[str, str] | None: ...
def rust_find_unloaded_deps(
    initial: list[str],
    graph: dict[str, tuple[list[str], list[str]]],
    loaded: set[str],
) -> list[str] | None: ...
def rust_target_from_node(module: str, node: Any) -> str | None: ...
def rust_merge_dependencies(new_deps: dict[str, set[str]], deps: dict[str, set[str]]) -> None: ...
def rust_non_trivial_bases(info: Any) -> list[Any]: ...
def rust_has_user_bases(info: Any) -> bool: ...
def rust_compare_symbol_table_snapshots(
    name_prefix: str, snapshot1: dict[str, Any], snapshot2: dict[str, Any]
) -> set[str]: ...
def rust_is_expr_literal_type(node: Any) -> bool | None: ...
def rust_get_partial_instance_type(node: Any) -> Any | None: ...

# mypy/dmypy_server.py — daemon server helpers.
def rust_ignore_suppressed_imports(module: str) -> bool | None: ...
def rust_get_meminfo() -> dict[str, Any] | None: ...
def rust_response_metadata(options: Any) -> dict[str, str] | None: ...
def rust_find_all_sources_in_build(graph: Any, extra: Any) -> list[Any] | None: ...
def rust_add_all_sources_to_changed(sources: Any, changed: Any) -> None: ...
def rust_fix_module_deps(graph: Any) -> None: ...
def rust_filter_out_missing_top_level_packages(
    packages: Any, search_paths: Any, fscache: Any
) -> set[str] | None: ...

class IdMapper:
    def __init__(self) -> None: ...
    def id(self, o: object) -> int: ...
    def __len__(self) -> int: ...

# Issue #540: pure helpers from mypy/modulefinder.py
def rust_is_init_file(path: str) -> bool: ...
def rust_parse_version(version: str) -> tuple[int, int]: ...
def rust_mypy_path() -> list[str]: ...
def rust_typeshed_py_version(options: Any) -> tuple[int, int]: ...
def rust_default_lib_path(
    data_dir: str, pyversion: tuple[int, int], custom_typeshed_dir: str | None
) -> list[str]: ...
def rust_load_stdlib_py_versions(
    custom_typeshed_dir: str | None
) -> dict[str, tuple[tuple[int, int], tuple[int, int] | None]]: ...
def rust_matches_exclude(
    subpath: str, excludes: list[str], fscache: Any, verbose: bool
) -> bool: ...
def rust_get_search_dirs(
    python_executable: str | None
) -> tuple[list[str], list[str]]: ...
def rust_compute_search_paths(
    sources: Any, options: Any, data_dir: str, alt_lib_path: str | None
) -> RustSearchPaths: ...

class RustSearchPaths:
    def __init__(
        self,
        python_path: list[str] = ...,
        mypy_path: list[str] = ...,
        package_path: list[str] = ...,
        typeshed_path: list[str] = ...,
    ) -> None: ...
    @property
    def python_path(self) -> tuple[str, ...]: ...
    @python_path.setter
    def python_path(self, value: list[str]) -> None: ...
    @property
    def mypy_path(self) -> tuple[str, ...]: ...
    @mypy_path.setter
    def mypy_path(self, value: list[str]) -> None: ...
    @property
    def package_path(self) -> tuple[str, ...]: ...
    @package_path.setter
    def package_path(self, value: list[str]) -> None: ...
    @property
    def typeshed_path(self) -> tuple[str, ...]: ...
    @typeshed_path.setter
    def typeshed_path(self, value: list[str]) -> None: ...
    def asdict(self) -> dict[str, tuple[str, ...]]: ...

class RustBuildSource:
    def __init__(
        self,
        path: str | None,
        module: str | None,
        text: str | None = ...,
        base_dir: str | None = ...,
        followed: bool = ...,
    ) -> None: ...
    @property
    def path(self) -> str | None: ...
    @path.setter
    def path(self, value: str | None) -> None: ...
    @property
    def module(self) -> str: ...
    @module.setter
    def module(self, value: str) -> None: ...
    @property
    def text(self) -> str | None: ...
    @text.setter
    def text(self, value: str | None) -> None: ...
    @property
    def base_dir(self) -> str | None: ...
    @base_dir.setter
    def base_dir(self, value: str | None) -> None: ...
    @property
    def followed(self) -> bool: ...
    @followed.setter
    def followed(self, value: bool) -> None: ...

class RustBuildSourceSet:
    def __init__(self, sources: Any) -> None: ...
    @property
    def source_text_present(self) -> bool: ...
    @property
    def source_modules(self) -> dict[str, str]: ...
    @property
    def source_paths(self) -> set[str]: ...
    def is_source(self, file: Any) -> bool: ...

def rust_adjust_tuple(
    left_bytes: bytes,
    r_bytes: bytes,
) -> Any: ...
def rust_analyze_unbound_without_info(
    is_var_any: bool,
    allow_type_any: bool,
    is_type_instance: bool,
    is_type_type_any: bool,
    unbound_tvar: Any,
    allow_unbound_tvars: bool,
    is_enum_member: bool,
    defining_literal: Any,
) -> Any: ...
def rust_any_causes_overload_ambiguity(
    resolver: NativeTypeResolver,
    items_bytes: list[bytes],
    return_types_bytes: list[bytes],
    arg_types_bytes: list[bytes],
    arg_kinds: Any,
    arg_names: Any,
    strict_optional: bool,
) -> bool | None: ...
def rust_are_related_types(
    left_bytes: bytes,
    right_bytes: bytes,
    proper_subtype: bool,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_are_tuples_overlapping(
    left_bytes: bytes,
    right_bytes: bytes,
    strict_optional: bool,
    ignore_promotions: bool,
    overlap_for_overloads: Any,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_are_typed_dicts_overlapping(
    left_bytes: bytes,
    right_bytes: bytes,
    strict_optional: bool,
    ignore_promotions: bool,
    overlap_for_overloads: Any,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_builtin_item_type(
    t_bytes: bytes,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_check_argument_types_plan(
    resolver: NativeTypeResolver,
    arg_type_blobs: list[bytes],
    arg_kinds: list[int],
    formal_to_actual: list[list[int]],
    callee_bytes: bytes,
) -> list[bytes] | None: ...
def rust_check_arguments(
    resolver: NativeTypeResolver,
    callee_bytes: bytes,
    arg_types_bytes: bytes,
    arg_kinds: Any,
    formal_to_actual: Any,
    strict_optional: bool,
    allow_abstract_call: bool,
) -> Any: ...
def rust_check_call_expr_callable_name(
    object_type_bytes: bytes,
    callable_name: Any,
    member: Any,
    has_object_type: bool,
) -> Any: ...
def rust_classify_type_expression(
    node_tags: Any,
    str_value: Any,
    str_isidentifier: Any,
    str_has_quotes: Any,
    str_has_open_bracket: Any,
    str_is_whitespace: Any,
    str_nontype_regex_match: Any,
    index_base_kind: Any,
    index_leftmost_is_name: Any,
    index_node_is_var: Any,
    index_var_is_special: Any,
    op_is_pipe: Any,
) -> int | None: ...
def rust_classify_type_with_info(
    fullname: Any,
    args_len: Any,
    tuple_type_not_none: Any,
    special_alias_not_none: Any,
    typeddict_type_not_none: Any,
) -> Any: ...
def rust_classify_unbound_front(
    node_kind: Any,
    placeholder_becomes_typeinfo: Any,
    final_iteration: Any,
    allow_placeholder: bool,
    has_hook: bool,
    tvar_def_exists: Any,
    tvar_def_in_allowed: Any,
    tvar_def_erased: Any,
    placeholder_in_tvar_params: Any,
    allow_unbound_tvars: bool,
    defining_alias: Any,
    defining_literal: Any,
    param_spec_name_set: Any,
    allow_param_spec_literals: bool,
    has_args: bool,
    alias_type_params_names: Any,
    tname: Any,
    allow_type_var_tuple: int,
    nesting_level: Any,
) -> Any: ...
def rust_clean_up_bases(
    fullname: Any,
    in_protocol_names: Any,
    has_args: bool,
) -> int: ...
def rust_is_magic_base(
    base_expr: Expression,
    namedtuple_names: tuple[str, ...],
    tpdict_names: tuple[str, ...],
) -> bool: ...
def rust_is_core_builtin_class(
    cur_mod_id: str,
    class_name: str,
    core_names: list[str],
) -> bool: ...
def rust_classify_with_metaclass(
    fullname: str | None,
    args_len: int,
    all_positional: bool,
) -> int: ...
def rust_classify_add_metaclass(
    fullname: str | None,
    args_len: int,
    arg_kind_0_positional: bool,
) -> int: ...
def rust_classify_configure_bases(
    bases_wire: list[bytes],
    is_newtypes: list[bool],
    disallow_subclassing_any: bool,
    disallow_any_unimported: bool,
    disallow_any_explicit: bool,
    is_typeshed_stub_file: bool,
) -> list[tuple[int, bool, bool]] | None: ...
def rust_classify_configure_mro(
    info: TypeInfo,
) -> tuple[int, list[int], str | None] | None: ...
def rust_classify_function_signature(
    sig_arg_types_len: int,
    arguments_len: int,
) -> int: ...
def rust_classify_fixed_args(
    args_len: int,
    arg_kinds: list[int],
    numargs: int,
) -> int | None: ...
def rust_classify_simple_literal_type(
    function_stack: bool,
    value_kind: int,
    cur_mod_id: str,
    is_final: bool,
) -> int | None: ...
def rust_create_errors(
    error_tuples: Any,
) -> Any: ...
def rust_dangerous_comparison(
    left_bytes: bytes,
    right_bytes: bytes,
    original_container_bytes: bytes | None,
    python_seen: Any,
    prefer_literal: Any,
    identity_check: Any,
    strict_equality_for_none: bool,
    unreachable_suppressed: Any,
    has_custom_eq_left: bool,
    has_custom_eq_right: bool,
    strict_optional: bool,
    abstract_set_ref: Any,
    abstract_map_ref: Any,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_detach_callable(
    typ_bytes: bytes,
    class_type_vars_bytes: bytes,
) -> Any: ...
def rust_equality_value_info(
    t_bytes: bytes,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_expand_callable_variants(
    type_bytes: bytes,
    strict_optional: bool,
) -> Any: ...
def rust_expand_tuple_if_possible(
    tup_bytes: bytes,
    target: Any,
) -> Any: ...
def rust_extract_meta_info(
    typ: Any,
) -> Any: ...
def rust_find_isinstance_join() -> Any: ...
def rust_find_possibly_undefined(
    node: Any,
    type_map: Any,
    options: Any,
    names: Any,
) -> Any: ...
def rust_get_possible_variants(
    typ_bytes: bytes,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_get_property_type(
    t: Any,
) -> Any: ...
def rust_infer_value_type() -> Any: ...
def rust_instantiate_type_alias(
    node: Any,
    arg_blobs: Any,
    no_args: Any,
    empty_tuple_index: Any,
) -> Any: ...
def rust_is_classmethod_node(
    node: Any,
) -> bool | None: ...
def rust_is_enum_overlapping_union(
    x_bytes: bytes,
    y_bytes: bytes,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_is_equality_ambiguous_for_narrowing(
    left_bytes: bytes,
    right_bytes: bytes,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_is_literal_in_union(
    x_bytes: bytes,
    y_bytes: bytes,
) -> Any: ...
def rust_is_more_general_arg_prefix(
    t_bytes: bytes,
    s_bytes: bytes,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_is_node_static(
    node: Any,
) -> bool | None: ...
def rust_is_none_object_overlap(
    t1_bytes: bytes,
    t2_bytes: bytes,
) -> Any: ...
def rust_is_object(
    t_bytes: bytes,
) -> Any: ...
def rust_is_overlapping_erased_types(
    left_bytes: bytes,
    right_bytes: bytes,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_is_same_arg_prefix(
    t_bytes: bytes,
    s_bytes: bytes,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_is_tuple(
    t_bytes: bytes,
) -> Any: ...
def rust_is_unsafe_overlapping_overload_signatures(
    signature: Any,
    other: Any,
    class_type_vars: Any,
    partial_only: Any,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_merge_typevars_in_callables_by_name(
    types_bytes: list[bytes],
    start_raw_id: Any,
    strict_optional: bool,
) -> Any: ...
def rust_namedtuple_prohibited_names() -> Any: ...
def rust_narrow_type() -> Any: ...
def rust_overload_can_never_match(
    signature_bytes: bytes,
    other_bytes: bytes,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_partial_type_inference() -> Any: ...
def rust_partition_equality_ambiguous_types(
    current_bytes: bytes,
    target_bytes: bytes,
    is_identity: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_process_start_options(
    flags: Any,
    allow_sources: bool,
) -> Any: ...
def rust_remove_path_prefix(
    path: Any,
    prefix: Any,
) -> Any: ...
def rust_remove_redundant_union_items(
    type_list_bytes: bytes,
    keep_erased: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_report_internal_error(
    file: Any,
    line: Any,
    show_traceback: Any,
    mypy_version: Any,
) -> Any: ...
def rust_serialize_fields(
    fields: Any,
) -> Any: ...
def rust_should_dispatch_union_call(
    object_type_bytes: bytes,
    callable_name: Any,
    member: Any,
) -> Any: ...
def rust_sort_within_context(
    errors: Any,
) -> Any: ...
def rust_supported_self_type(
    type_bytes: bytes,
    resolver: NativeTypeResolver,
    allow_callable: bool,
    allow_instances: bool,
) -> bool | None: ...
def rust_transform_copy(
    node: Any,
) -> Any: ...
def rust_try_contracting_literals_in_union(
    type_list_bytes: bytes,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_type_object_type_from_function(
    signature_bytes: bytes,
    info: Any,
    def_info: Any,
    fallback_bytes: bytes,
    is_new: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...
def rust_yield_nonoverlapping_types(
    nonoverlapping_types: Any,
    unreachable_lines: Any,
) -> Any: ...


# Entries missing from earlier merges, recovered from the built extension
# (self-check attr-defined failures, 2026-08-27).
def rust_analyze_instance_member_dispatch(
    resolver: NativeTypeResolver,
    instance_bytes: bytes,
    name: str,
    override_info: str | None,
    self_type_bytes: bytes,
    _no_deferral: bool,
    preserve_type_var_ids: bool,
    start_raw_id: int,
    strict_optional: bool,
) -> tuple[int, bool, bytes] | None: ...
def rust_classify_literal_param(
    is_proper_type: bool,
    is_unbound: bool,
    is_union_pre: bool,
    original_str_expr_is_not_none: bool,
    is_any: bool,
    type_of_any: int,
    is_raw_expr: bool,
    literal_value_is_none: bool,
    simple_name: str,
    is_none_type: bool,
    is_literal: bool,
    is_instance: bool,
    last_known_value_is_none: bool,
    is_union_post: bool,
) -> int: ...
def rust_classify_metaclass_compat(info: Any) -> int | None: ...
def rust_covers_at_runtime(
    item_bytes: bytes, supertype_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bool | None: ...
def rust_erase_return_self_types(typ_bytes: bytes, self_type_bytes: bytes) -> bytes | None: ...
def rust_fill_typevars_with_any(typ: Any) -> bytes | None: ...
def rust_find_type_overlaps(type_bytes_list: list[bytes]) -> list[str] | None: ...
def rust_get_member_flags(
    info: Any,
    name: str,
    class_obj: bool,
    extra_attrs: Any | None,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> list[int] | None: ...
def rust_has_await_in_generator(node_bytes: bytes) -> bool: ...
def rust_infer_variance_member(
    member_type_bytes: bytes,
    self_type_bytes: bytes,
    object_type_bytes: bytes,
    raw_id: int,
    resolver: NativeTypeResolver,
) -> int | None: ...
def rust_is_better(t_bytes: bytes, s_bytes: bytes, resolver: NativeTypeResolver) -> bool | None: ...
def rust_is_descriptor(
    resolver: NativeTypeResolver, type_bytes: bytes, strict_optional: bool
) -> bool | None: ...
def rust_is_disjoint_base(info: Any) -> bool: ...
def rust_is_recursive_pair(
    s_bytes: bytes,
    t_bytes: bytes,
    s_is_recursive: bool,
    t_is_recursive: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...
def rust_is_valid_constructor(n: Any) -> bool: ...
def rust_is_valid_keyword_var_arg(
    type_bytes: bytes, dict_str_keys_ok: bool, skag_str_ok: bool, skag_never_ok: bool
) -> bool | None: ...
def rust_is_valid_var_arg(type_bytes: bytes, iterable_ok: bool) -> bool | None: ...
def rust_make_inferred_type_note(
    subtype_bytes: bytes, supertype_bytes: bytes, arg_results: list[bool], context: Any
) -> bool: ...
def rust_map_instance_to_supertypes(
    resolver: NativeTypeResolver, items_wire: bytes, supertype_ref: str
) -> tuple[bytes, list[bool]] | None: ...
def rust_match_generic_callables(
    num_vars: int, start_raw_id: int, t_bytes: bytes, s_bytes: bytes
) -> tuple[int, bytes, bytes] | None: ...
def rust_classify_enum(
    info: Any, is_stub: bool, tree_fullname: str, enum_bases: list[str]
) -> tuple[int, list[str]] | None: ...
