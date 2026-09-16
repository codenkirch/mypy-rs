"""Native seam stubs for the checker area (split from type_kernel.pyi, #1677)."""

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


class PluginHookRegistry:
    def __init__(self, hooks: dict[str, list[str]]) -> None: ...
    def has_call_hook(self, fullname: str) -> bool: ...
    def has_hook_for(self, hook_method_name: str, fullname: str) -> bool: ...
    def has_hook(self, fullname: str) -> bool: ...
    def len(self) -> int: ...
    def is_empty(self) -> bool: ...

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

def rust_join_type_list(
    type_blobs: list[bytes], strict_optional: bool, resolver: NativeTypeResolver
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

def rust_can_be_narrowed_with_len(typ: bytes, resolver: NativeTypeResolver) -> bool | None: ...

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

def rust_has_erased_component(type_bytes: bytes, resolver: NativeTypeResolver) -> bool | None: ...

def rust_allow_fast_container_literal(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bool | None: ...

def rust_analyze_cond_branch(
    resolver: NativeTypeResolver, branch: bytes | None, known_type: bytes | None
) -> bytes | None: ...

def rust_has_bytes_component(resolver: NativeTypeResolver, type_bytes: bytes) -> bool | None: ...

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
    strict_optional: bool,
    arg_names: list[str | None] | None = None,
    strict: bool = True,
    infer_unions: bool = False,
    typeobj_gate_fails: list[int] | None = None,
) -> int | None: ...

def rust_find_matching_overload_items(
    resolver: NativeTypeResolver,
    items_bytes: list[bytes],
    template_bytes: bytes,
    strict_optional: bool,
    infer_unions: bool = False,
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

def rust_classify_classvar_super(base_node: Any, node_is_classvar: bool) -> int | None: ...

def rust_classify_all_supers_gate(
    lvalue_node: Any, lvalue_line: int, lvalue_kind: int | None, mdef: int
) -> tuple[int, list[int]] | None: ...

def rust_classify_check_lvalue(
    lvalue: Any, allow_redefinition: bool, is_definition: bool
) -> int | None: ...

def rust_classify_new_signature(is_metaclass: bool, is_instance_ret: bool) -> int | None: ...

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

def rust_is_writable_attribute(node: Any) -> bool | None: ...

def rust_is_defined_in_base_class(var: Any) -> bool | None: ...

def rust_is_definition(node: Any) -> bool | None: ...

def rust_can_widen_in_scope(name: Any, orig_type: Any, scope: Any) -> bool | None: ...

def rust_is_overloaded_item(node: Any, statement: Any) -> bool | None: ...

def rust_is_self_member_ref(memberexpr: Any) -> bool | None: ...

def rust_is_len_of_tuple(expr: Any) -> bool | None: ...

def rust_is_literal_enum(parent_type: Any, member_type: Any) -> bool | None: ...

def rust_check_exit_return_type(defn: Any) -> bool | None: ...

def rust_check_final_deletable(typ: Any) -> list[str] | None: ...

def rust_is_base_class(t: Any, s: Any) -> bool | None: ...

def rust_classify_find_member(
    name: str, itype: Any, is_operator: bool, class_obj: bool
) -> int | None: ...

def rust_check_for_untyped_decorator(
    disallow_untyped_decorators: bool,
    func_type_bytes: bytes | None,
    dec_type: Any,
    current_node_deferred: bool,
) -> bool | None: ...

def rust_check_explicit_override_decorator(defn: Any, found_method_base_classes: Any) -> bool: ...

def rust_check_match_args(type_bytes: bytes) -> bool | None: ...

def rust_is_valid_defaultdict_partial_value_type(
    type_bytes: bytes, old_type_inference: bool
) -> bool | None: ...

def rust_is_assignable_slot(lvalue: Any, typ: Any) -> bool | None: ...

def rust_is_noop_for_reachability(stmt: Any) -> bool | None: ...

def rust_classify_unbound_return_typevar(type_bytes: bytes) -> int | None: ...

def rust_check_untyped_after_decorator(
    disallow_any_decorated: bool,
    is_stub: bool,
    current_node_deferred: bool,
    type_bytes: bytes,
    resolver: Any,
) -> bool | None: ...

def rust_check_incompatible_property_override(e: Any) -> bool | None: ...

def rust_classify_find_isinstance_head(
    callee: Any, args_len: int, literal_ok: bool
) -> int | None: ...

def rust_classify_match_subject_head(
    subject: Any, subject_dummy_is_none: bool
) -> int | None: ...

def rust_classify_range_int_gate(expr: Any) -> int | None: ...

def rust_classify_comparison_operands(
    literal_kinds: list[int],
    operand_flags: list[tuple[bool, bool, bool, bool, bool]],
    operand_wires: list[bytes],
    resolver: NativeTypeResolver,
) -> list[bool] | None: ...

def rust_classify_type_check_raise(
    type_bytes: bytes, callee_fullname: str | None
) -> int | None: ...

def rust_classify_type_range(t: Any) -> tuple[int, bool] | None: ...

def rust_classify_typeobj_gate(callee: Any) -> int | None: ...

def rust_check_call_head(
    callable_node: Any, callee: Any, enum_bases: Any
) -> tuple[bool, int | None]: ...

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

def rust_classify_return_stmt_pre(return_type_bytes: bytes, is_lambda: bool) -> bool | None: ...

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

def rust_is_type_type_context(resolver: NativeTypeResolver, type_bytes: bytes) -> bool | None: ...

def rust_try_getting_literal(type_bytes: bytes) -> bytes | None: ...

def rust_is_string_literal(type_bytes: bytes) -> bool | None: ...

def rust_is_untyped_decorator(typ: Any) -> bool | None: ...

def rust_is_typeddict_type_context(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bool | None: ...

def rust_conditional_expr_join(
    if_bytes: bytes,
    else_bytes: bytes,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
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
) -> bytes | Literal[False] | None: ...

def rust_tuple_context_matches(elements_tags: list[int], ctx_bytes: bytes) -> bool | None: ...

def rust_build_tuple_type(items_bytes: list[bytes], seen_unpack: int) -> bytes | None: ...

def rust_star_expr(type_bytes: bytes) -> bytes | None: ...

def rust_resolve_plugin_hook(
    registry: Any, callable_name: str, plugin_list: Any, hook_method_name: str
) -> Any | None: ...

def rust_method_fullname(
    resolver: NativeTypeResolver, type_bytes: bytes, method_name: str
) -> str | None: ...

def rust_is_enum_callable_base(callable_node: Expression | None, enum_bases: Any) -> bool: ...

def rust_classify_protocol_test_callee(callee: Expression, n_args: int) -> str | None: ...

def rust_classify_call(callee_bytes: bytes) -> int | None: ...

def rust_classify_typeddict_call(args: list[Expression], arg_kinds: list[int]) -> int | None: ...

def rust_classify_reveal_imported(
    kind: int, is_imported: bool, unimported_reveal_enabled: bool
) -> str | None: ...

def rust_refers_to_typeddict(base: Any, target_bytes: bytes | None = None) -> bool: ...

def rust_classify_super_arg_types(chk: Any, super_expr: Any) -> int | None: ...

def rust_compute_arg_context_indices(
    arg_kinds: list[int],
    formal_to_actual: list[list[int]],
    args_len: int,
    callee_arg_types_len: int,
) -> list[int] | None: ...

def rust_classify_visit_op_expr(expr: Any) -> int | None: ...

def rust_classify_check_arg(
    caller_type_bytes: bytes, is_subtype: bool, has_abstract_type_part: bool
) -> int | None: ...

def rust_classify_simple_assignment(
    lvalue_type_bytes: bytes | None,
    is_stub: bool,
    rvalue_is_ellipsis: bool,
    has_inferred: bool,
    inferred_is_argument: bool,
    simple_rvalue: bool,
) -> int | None: ...

def rust_classify_check_assignment(
    lvalue: Any, lvalue_type: Any | None, has_inferred: bool, active_class: bool
) -> tuple[int, int] | None: ...

def rust_classify_check_boolean_op(
    op_is_and: bool,
    right_always: bool,
    right_unreachable: bool,
    left_map_values: list[bytes],
    right_map_values: list[bytes],
    expanded_left_bytes: bytes,
    can_be_true: bool,
    can_be_false: bool,
    strict_optional: bool,
    restricted_uninhabited: bool | None,
    resolver: NativeTypeResolver,
) -> tuple[int, bool, bool, int] | None: ...

def rust_classify_index_with_type(
    left_type: Any, chk: Any, expand_variadic: bool
) -> int | None: ...

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
    arg_kinds: list[int],
    formal_to_actual: list[list[int]],
    strict: bool,
    infer_unions: bool,
    strict_optional: bool,
    iterable_type: bytes | None,
    mapping_type: bytes | None,
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
    resolver: NativeTypeResolver, arg_type_bytes: list[bytes], target_bytes: list[bytes]
) -> bool | None: ...

def rust_bind_self_fast(method_bytes: bytes) -> bytes | None: ...

def rust_classify_member_access(resolver: NativeTypeResolver, type_bytes: bytes) -> int | None: ...

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

def rust_find_non_escaped_targets(format_value: str) -> tuple[int, list[tuple[str, int]]]: ...

def rust_parse_format_value(
    format_value: str,
) -> tuple[
    int,
    list[
        tuple[str, int, str | None, str, str, str, str, str | None, bool, str | None, str | None]
    ],
]: ...

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
    types_bytes: list[bytes], star_pos: int | None, num_patterns: int, resolver: NativeTypeResolver
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

def rust_classify_sequence_pattern_head(
    type_bytes: bytes,
    star_pos: int | None,
    required_patterns: int,
    non_seq_bytes: bytes,
    sequence_bytes: bytes,
    iterable_bytes: bytes,
    resolver: NativeTypeResolver,
) -> int | None: ...

def rust_classify_sequence_tuple_result(
    new_bytes: list[bytes],
    rest_bytes: list[bytes],
    resolver: NativeTypeResolver,
) -> tuple[bool, int, int, list[bool]] | None: ...

def rust_classify_mapping_rest(
    type_bytes: bytes,
    mapping_bytes: bytes,
    resolver: NativeTypeResolver,
) -> int | None: ...

def rust_filter_or_match_types(
    match_bytes: list[bytes],
    resolver: NativeTypeResolver,
) -> list[int] | None: ...

def rust_is_unreachable_map(type_bytes_list: list[bytes]) -> bool | None: ...

def rust_stmt_outcome(node_bytes: bytes) -> str | None: ...

def rust_type_requires_usage(type_bytes: bytes, resolver: NativeTypeResolver) -> int | None: ...

def rust_with_exit_suppresses(type_bytes: bytes, strict_optional: bool) -> bool: ...

def rust_try_handler_union(type_bytes: bytes, strict_optional: bool) -> list[bytes] | None: ...

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
    infer_unions: bool = False,
) -> int | None: ...

def rust_restrict_subtype_away(
    t_bytes: bytes,
    s_bytes: bytes,
    consider_runtime_isinstance: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
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
    fields_bytes: bytes, class_fullname: str, init_name: str, add_order: bool
) -> bytes | None: ...

def rust_dataclass_transform(
    fields_bytes: bytes,
    class_fullname: str,
    decorator_init: bool,
    decorator_eq: bool,
    decorator_order: bool,
    decorator_frozen: bool,
) -> bytes | None: ...

def rust_dataclass_post_init_transform(
    fields_bytes: bytes, class_fullname: str
) -> bytes | None: ...

def rust_find_shallow_matching_overload_item(overload: Any, call: Any) -> int | None: ...

def rust_infer_condition_value(expr: Expression, options: Any) -> int: ...

def rust_infer_pattern_value(pattern: Any) -> int: ...

def rust_assert_will_always_fail(stmt: Any, options: Any) -> bool: ...

def rust_consider_sys_version_info(expr: Expression, pyversion: tuple[int, ...]) -> int: ...

def rust_consider_sys_platform(expr: Expression, platform: str) -> int: ...

def rust_is_sys_attr(expr: Expression, name: str) -> bool: ...

def rust_contains_sys_version_info(
    expr: Expression,
) -> None | int | tuple[int | None, int | None]: ...

def rust_contains_int_or_tuple_of_ints(expr: Expression) -> None | int | tuple[int, ...]: ...

def rust_fixed_comparison(left: Any, op: str, right: Any) -> int: ...

def rust_analyze_member_access(
    resolver: NativeTypeResolver,
    name: str,
    typ_bytes: bytes,
    self_type_bytes: bytes,
    is_lvalue: bool,
    is_super: bool,
    is_operator: bool,
    is_self: bool,
    preserve_type_var_ids: bool,
    start_raw_id: int,
    strict_optional: bool,
    plugin: object | None = None,
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
) -> tuple[int, bool, list[bytes | None]] | None: ...

def rust_analyze_none_member_access(
    resolver: NativeTypeResolver,
    name: str,
    typ_bytes: bytes,
    self_type_bytes: bytes | None,
    is_lvalue: bool,
    is_super: bool,
    preserve_type_var_ids: bool,
    start_raw_id: int,
    strict_optional: bool,
) -> tuple[int, bool, bytes] | None: ...

def rust_analyze_typeddict_access(
    resolver: NativeTypeResolver, name: str, typ_bytes: bytes, strict_optional: bool
) -> bytes | None: ...

def rust_analyze_enum_class_attribute_access(
    resolver: NativeTypeResolver, instance_bytes: bytes, name: str
) -> bytes | None: ...

def rust_analyze_descriptor_access(
    resolver: NativeTypeResolver, descriptor_bytes: bytes, is_lvalue: bool, strict_optional: bool
) -> bytes | None: ...

def rust_descriptor_has_get_set(
    resolver: NativeTypeResolver, descriptor_bytes: bytes
) -> tuple[bool, bool] | None: ...

def rust_classify_type_type_member_access(typ: Any) -> int | None: ...

def rust_is_instance_var(var: Var) -> bool | None: ...

def rust_check_final_member(info: TypeInfo, name: str) -> bool | None: ...

def rust_classify_analyze_var(
    name: str,
    var: Var,
    itype_bytes: bytes,
    is_lvalue: bool,
    no_deferral: bool,
    is_operator: bool,
    resolver: NativeTypeResolver,
) -> int | None: ...

def rust_check_self_arg(
    resolver: NativeTypeResolver,
    functype_bytes: bytes,
    dispatched_arg_type_bytes: bytes,
    is_classmethod: bool,
    name: str,
    strict_optional: bool,
    infer_unions: bool = False,
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

def rust_check_argument_count(
    formal_kinds: list[int],
    has_param_spec: bool,
    special_sig: str | None,
    actual_kinds: list[int],
    actual_names: list[str | None],
    actual_shapes: list[int],
    actual_item_counts: list[int],
    formal_to_actual: list[list[int]],
    object_type_present: bool,
    callable_name: str | None,
    in_checked_function: bool,
) -> tuple[bool, list[tuple[int, int, int]], bool] | None: ...

def rust_combine_function_signatures(
    resolver: NativeTypeResolver,
    types_bytes: list[bytes],
    start_raw_id: int,
    strict_optional: bool,
) -> tuple[int, bytes] | None: ...

def rust_arg_approximate_similarity(
    actual_bytes: bytes,
    formal_bytes: bytes,
    strict_optional: bool,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
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

def rust_group_comparison_operands(
    ops_and_indices: list[tuple[str, int, int]],
    literal_hashes: dict[int, int],
    operators_to_group: list[str],
) -> list[tuple[str, list[int]]]: ...

def rust_is_valid_inferred_type(
    typ_bytes: bytes,
    is_lvalue_final: bool,
    is_lvalue_member: bool,
    allow_redefinition: bool,
    resolver: NativeTypeResolver,
) -> bool | None: ...

def rust_has_custom_eq_checks(typ_bytes: bytes, resolver: NativeTypeResolver) -> bool | None: ...

def rust_copy_modified(typ_bytes: bytes, field: str, value_bytes: bytes) -> bytes | None: ...

def rust_decorator_is_dynamic(dec: Any) -> bool: ...

def rust_func_has_self_or_cls_argument(func: Any) -> bool: ...

def rust_func_item_is_dynamic(func: Any) -> bool: ...

def rust_overloaded_is_dynamic(func: Any) -> bool: ...

def rust_typeinfo_is_generic(info: Any) -> bool: ...

def rust_typeinfo_is_metaclass(info: Any, precise: bool) -> bool: ...

def rust_typeinfo_has_base(info: Any, fullname: str) -> bool: ...

def rust_classify_class_pattern_ranges(
    typ_bytes: bytes, class_ref_node: Any
) -> list[int] | None: ...

def rust_classify_class_pattern_alias_gate(class_ref_node: Any) -> bool | None: ...

def rust_classify_class_pattern_keywords(
    match_arg_names: list[str | None],
    num_positionals: int,
    keyword_keys: list[str],
) -> list[tuple[int, int]] | None: ...

def rust_any_causes_overload_ambiguity(
    resolver: NativeTypeResolver,
    items_bytes: list[bytes],
    return_types_bytes: list[bytes],
    arg_types_bytes: list[bytes],
    arg_kinds: Any,
    arg_names: Any,
    infer_unions: bool,
    strict_optional: bool,
) -> bool | None: ...

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
    object_type_bytes: bytes, callable_name: Any, member: Any, has_object_type: bool
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

def rust_equality_value_info(t_bytes: bytes, resolver: NativeTypeResolver) -> Any: ...

def rust_find_isinstance_join() -> Any: ...

def rust_find_possibly_undefined(node: Any, type_map: Any, options: Any, names: Any) -> Any: ...

def rust_get_property_type(t: Any) -> Any: ...

def rust_infer_value_type() -> Any: ...

def rust_is_classmethod_node(node: Any) -> bool | None: ...

def rust_is_equality_ambiguous_for_narrowing(
    left_bytes: bytes, right_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...

def rust_is_more_general_arg_prefix(
    t_bytes: bytes, s_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bool | None: ...

def rust_is_node_static(node: Any) -> bool | None: ...

def rust_is_same_arg_prefix(
    t_bytes: bytes, s_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
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
    types_bytes: list[bytes], start_raw_id: Any, strict_optional: bool
) -> Any: ...

def rust_narrow_type() -> Any: ...

def rust_overload_can_never_match(
    signature_bytes: bytes, other_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bool | None: ...

def rust_partial_type_inference() -> Any: ...

def rust_partition_equality_ambiguous_types(
    current_bytes: bytes,
    target_bytes: bytes,
    is_identity: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...

def rust_remove_redundant_union_items(
    type_list_bytes: bytes,
    truthiness_bytes: bytes,
    keep_erased: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...

def rust_serialize_fields(fields: Any) -> Any: ...

def rust_should_dispatch_union_call(
    object_type_bytes: bytes, callable_name: Any, member: Any
) -> Any: ...

def rust_supported_self_type(
    type_bytes: bytes, resolver: NativeTypeResolver, allow_callable: bool, allow_instances: bool
) -> bool | None: ...

def rust_analyze_instance_member_dispatch(
    resolver: NativeTypeResolver,
    instance_bytes: bytes,
    name: str,
    override_info: str | None,
    self_type_bytes: bytes | None,
    _no_deferral: bool,
    preserve_type_var_ids: bool,
    start_raw_id: int,
    strict_optional: bool,
) -> tuple[int, bool, bytes] | None: ...

def rust_classify_metaclass_compat(info: Any) -> int | None: ...

def rust_covers_at_runtime(
    item_bytes: bytes,
    supertype_bytes: bytes,
    strict_optional: bool,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
) -> bool | None: ...

def rust_get_member_flags(
    info: Any,
    name: str,
    class_obj: bool,
    extra_attrs: Any | None,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> list[int] | None: ...

def rust_is_valid_keyword_var_arg(
    type_bytes: bytes, dict_str_keys_ok: bool, skag_str_ok: bool, skag_never_ok: bool
) -> bool | None: ...

def rust_is_valid_var_arg(type_bytes: bytes, iterable_ok: bool) -> bool | None: ...

def rust_classify_enum(
    info: Any, is_stub: bool, tree_fullname: str, enum_bases: list[str]
) -> tuple[int, list[str]] | None: ...

def rust_always_returns_none(node: Expression, info: TypeInfo | None) -> bool | None: ...

def rust_infer_operator_assignment_method(
    typ: Any, method: str, in_ops: bool
) -> tuple[bool, str] | None: ...

def rust_is_type_like(node: Any) -> bool | None: ...

def rust_should_report_unreachable_issues(chk: Any) -> bool | None: ...

def rust_refers_to_different_scope(name: Any, scope: Any, tree: Any) -> bool | None: ...

def rust_flatten_lvalues(lvalues: list[Expression]) -> list[Expression] | None: ...

def rust_literal_int_expr(
    type_maps: list[dict[Expression, Type]], expr: Expression
) -> tuple[int, int | None] | None: ...


__all__ = [
    "PluginHookRegistry",
    "rust_is_protocol_implementation",
    "rust_join_type_list",
    "rust_narrow_type_by_identity_equality",
    "rust_narrow_with_len",
    "rust_can_be_narrowed_with_len",
    "rust_has_any_type",
    "rust_has_abstract_type",
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
    "rust_check_overload_call",
    "rust_find_matching_overload_items",
    "rust_is_typed_callable",
    "rust_is_private",
    "rust_is_operator_method",
    "rust_are_argument_counts_overlapping",
    "rust_check_overlapping_overloads",
    "rust_classify_final_super",
    "rust_classify_check_final",
    "rust_classify_classvar_super",
    "rust_classify_all_supers_gate",
    "rust_classify_check_lvalue",
    "rust_classify_new_signature",
    "rust_classify_getattr_method",
    "rust_classify_func_def_override",
    "rust_classify_enum_new",
    "rust_classify_enum_bases",
    "rust_is_final_enum_value",
    "rust_is_writable_attribute",
    "rust_is_defined_in_base_class",
    "rust_is_definition",
    "rust_can_widen_in_scope",
    "rust_is_overloaded_item",
    "rust_is_self_member_ref",
    "rust_is_len_of_tuple",
    "rust_is_literal_enum",
    "rust_check_exit_return_type",
    "rust_check_final_deletable",
    "rust_is_base_class",
    "rust_classify_find_member",
    "rust_check_for_untyped_decorator",
    "rust_check_explicit_override_decorator",
    "rust_check_match_args",
    "rust_is_valid_defaultdict_partial_value_type",
    "rust_is_assignable_slot",
    "rust_is_noop_for_reachability",
    "rust_classify_unbound_return_typevar",
    "rust_check_untyped_after_decorator",
    "rust_check_incompatible_property_override",
    "rust_classify_find_isinstance_head",
    "rust_classify_match_subject_head",
    "rust_classify_range_int_gate",
    "rust_classify_comparison_operands",
    "rust_classify_type_check_raise",
    "rust_classify_type_range",
    "rust_classify_typeobj_gate",
    "rust_check_call_head",
    "rust_classify_rvalue_count",
    "rust_classify_truthy_type",
    "rust_classify_missing_annotations",
    "rust_classify_return_stmt_variant",
    "rust_classify_return_stmt_pre",
    "rust_classify_return_stmt_post",
    "rust_is_type_type_context",
    "rust_try_getting_literal",
    "rust_is_string_literal",
    "rust_is_untyped_decorator",
    "rust_is_typeddict_type_context",
    "rust_conditional_expr_join",
    "rust_conditional_types",
    "rust_container_type",
    "rust_tuple_context_matches",
    "rust_build_tuple_type",
    "rust_star_expr",
    "rust_resolve_plugin_hook",
    "rust_method_fullname",
    "rust_is_enum_callable_base",
    "rust_classify_protocol_test_callee",
    "rust_classify_call",
    "rust_classify_typeddict_call",
    "rust_classify_reveal_imported",
    "rust_refers_to_typeddict",
    "rust_classify_super_arg_types",
    "rust_compute_arg_context_indices",
    "rust_classify_visit_op_expr",
    "rust_classify_check_arg",
    "rust_classify_simple_assignment",
    "rust_classify_check_assignment",
    "rust_classify_check_boolean_op",
    "rust_classify_index_with_type",
    "rust_calibrate_type_obj_return",
    "rust_normalize_callable",
    "rust_check_callable_call",
    "rust_real_union",
    "rust_solve_generic_call",
    "rust_get_arg_infer_passes",
    "rust_possible_none_type_var_overlap",
    "rust_bind_self_fast",
    "rust_classify_member_access",
    "rust_instance_fallback",
    "rust_has_operator",
    "rust_meta_has_operator",
    "rust_defined_in_superclass",
    "rust_analyze_instance_member_access",
    "rust_analyze_member_method",
    "rust_analyze_conversion_specifiers",
    "rust_is_numeric_format_type",
    "rust_parse_conversion_specifiers",
    "rust_parse_placeholder_format",
    "rust_find_non_escaped_targets",
    "rust_parse_format_value",
    "rust_is_uninhabited",
    "rust_get_match_arg_names",
    "rust_get_type_range",
    "rust_should_self_match",
    "rust_can_match_sequence",
    "rust_contract_starred_pattern_types",
    "rust_expand_starred_pattern_types",
    "rust_construct_sequence_child",
    "rust_classify_sequence_pattern_head",
    "rust_classify_sequence_tuple_result",
    "rust_classify_mapping_rest",
    "rust_filter_or_match_types",
    "rust_is_unreachable_map",
    "rust_stmt_outcome",
    "rust_type_requires_usage",
    "rust_with_exit_suppresses",
    "rust_try_handler_union",
    "rust_classify_except_handler_tests",
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
    "rust_restrict_subtype_away",
    "rust_custom_special_method",
    "rust_get_protocol_member",
    "rust_transform_attrs",
    "rust_dataclass_transform",
    "rust_dataclass_post_init_transform",
    "rust_find_shallow_matching_overload_item",
    "rust_infer_condition_value",
    "rust_infer_pattern_value",
    "rust_assert_will_always_fail",
    "rust_consider_sys_version_info",
    "rust_consider_sys_platform",
    "rust_is_sys_attr",
    "rust_contains_sys_version_info",
    "rust_contains_int_or_tuple_of_ints",
    "rust_fixed_comparison",
    "rust_analyze_member_access",
    "rust_analyze_union_member_access",
    "rust_analyze_none_member_access",
    "rust_analyze_typeddict_access",
    "rust_analyze_enum_class_attribute_access",
    "rust_analyze_descriptor_access",
    "rust_descriptor_has_get_set",
    "rust_classify_type_type_member_access",
    "rust_is_instance_var",
    "rust_check_final_member",
    "rust_classify_analyze_var",
    "rust_check_self_arg",
    "rust_expand_without_binding",
    "rust_expand_and_bind_callable",
    "rust_add_class_tvars",
    "rust_check_argument_count",
    "rust_combine_function_signatures",
    "rust_arg_approximate_similarity",
    "rust_visit_tuple_index_helper",
    "rust_visit_tuple_slice_helper",
    "rust_try_getting_int_literals",
    "rust_visit_temp_node",
    "rust_visit_promote_expr",
    "rust_visit_paramspec_expr",
    "rust_visit_type_var_tuple_expr",
    "rust_visit_newtype_expr",
    "rust_and_conditional_maps",
    "rust_or_conditional_maps",
    "rust_group_comparison_operands",
    "rust_is_valid_inferred_type",
    "rust_has_custom_eq_checks",
    "rust_copy_modified",
    "rust_decorator_is_dynamic",
    "rust_func_has_self_or_cls_argument",
    "rust_func_item_is_dynamic",
    "rust_overloaded_is_dynamic",
    "rust_typeinfo_is_generic",
    "rust_typeinfo_is_metaclass",
    "rust_typeinfo_has_base",
    "rust_classify_class_pattern_ranges",
    "rust_classify_class_pattern_alias_gate",
    "rust_classify_class_pattern_keywords",
    "rust_any_causes_overload_ambiguity",
    "rust_check_argument_types_plan",
    "rust_check_arguments",
    "rust_check_call_expr_callable_name",
    "rust_dangerous_comparison",
    "rust_equality_value_info",
    "rust_find_isinstance_join",
    "rust_find_possibly_undefined",
    "rust_get_property_type",
    "rust_infer_value_type",
    "rust_is_classmethod_node",
    "rust_is_equality_ambiguous_for_narrowing",
    "rust_is_more_general_arg_prefix",
    "rust_is_node_static",
    "rust_is_same_arg_prefix",
    "rust_is_unsafe_overlapping_overload_signatures",
    "rust_merge_typevars_in_callables_by_name",
    "rust_narrow_type",
    "rust_overload_can_never_match",
    "rust_partial_type_inference",
    "rust_partition_equality_ambiguous_types",
    "rust_remove_redundant_union_items",
    "rust_serialize_fields",
    "rust_should_dispatch_union_call",
    "rust_supported_self_type",
    "rust_analyze_instance_member_dispatch",
    "rust_classify_metaclass_compat",
    "rust_covers_at_runtime",
    "rust_get_member_flags",
    "rust_is_valid_keyword_var_arg",
    "rust_is_valid_var_arg",
    "rust_classify_enum",
    "rust_always_returns_none",
    "rust_infer_operator_assignment_method",
    "rust_is_type_like",
    "rust_should_report_unreachable_issues",
    "rust_refers_to_different_scope",
    "rust_flatten_lvalues",
    "rust_literal_int_expr",
]
