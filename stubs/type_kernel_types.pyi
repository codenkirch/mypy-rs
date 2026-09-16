"""Native seam stubs for the types area (split from type_kernel.pyi, #1677)."""

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


class NativeTypeResolver:
    len: int
    alias_len: int
    def render_dict(self) -> dict[str, object]: ...
    def update(
        self,
        type_infos: list[TypeInfo],
        aliases: list[TypeAlias],
        modules: Any = None,
        ctor_blobs: dict[str, bytes] | None = None,
    ) -> tuple[int, int]: ...
    def set_live_typeinfo_map(self, typeinfo_map: dict[str, TypeInfo] | None) -> None: ...

def erase_type(typ: Type) -> ProperType | None: ...

def remove_instance_last_known_values(typ: Type) -> Type | None: ...

def shallow_erase_type_for_equality(typ: Type) -> ProperType | None: ...

def read_type_to_str(data: bytes) -> str: ...

def read_alias_recursion_flag(data: bytes) -> bool | None: ...

def build_resolver(type_infos: list[TypeInfo]) -> dict[str, object]: ...

def read_type_to_str_with_resolver(data: bytes, resolver: dict[str, object]) -> str: ...

def build_native_resolver(
    type_infos: list[TypeInfo],
    aliases: list[TypeAlias],
    modules: Any = None,
    ctor_blobs: dict[str, bytes] | None = None,
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
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
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
    infer_unions: bool = False,
) -> list[int]: ...

def rust_subtype_tvar_tuple_right(
    left: bytes, right: bytes, proper_subtype: bool, resolver: NativeTypeResolver
) -> bool | None: ...

def rust_variadic_tuple_subtype(
    left: bytes, right: bytes, proper_subtype: bool, resolver: NativeTypeResolver
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

def rust_object_or_any_from_type(typ: bytes, resolver: NativeTypeResolver) -> bytes | None: ...

def rust_combine_similar_callables(
    t: bytes, s: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...

def rust_object_from_instance(instance: bytes, resolver: NativeTypeResolver) -> str | None: ...

def rust_join_sorted_key(t_bytes: bytes) -> int | None: ...

def rust_narrow_declared_type(
    declared: bytes, narrowed: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> bytes | None: ...

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
    resolver: NativeTypeResolver, type_bytes: bytes, env_bytes: bytes, strict_optional: bool
) -> bytes | None: ...

def rust_expand_type_by_instance(
    resolver: NativeTypeResolver, type_bytes: bytes, instance_bytes: bytes, strict_optional: bool
) -> bytes | None: ...

def rust_remove_trivial(types_bytes: bytes, strict_optional: bool) -> bytes | None: ...

def rust_freshen_function_type_vars(
    start_raw_id: int, callee_bytes: bytes
) -> tuple[int, bytes] | None: ...

def rust_map_instance_to_supertype(
    resolver: NativeTypeResolver, instance_ref: str, instance_args: bytes, supertype_ref: str
) -> bytes | None: ...

def rust_class_derivation_paths(
    resolver: NativeTypeResolver, typ_ref: str, supertype_ref: str
) -> list[list[str]] | None: ...

def rust_map_instance_to_direct_supertypes(
    resolver: NativeTypeResolver, instance_ref: str, instance_args: bytes, supertype_ref: str
) -> list[bytes] | None: ...

def rust_freshen_all_functions_type_vars(
    start_raw_id: int,
    type_bytes: bytes,
    strict_optional: bool,
    resolver: NativeTypeResolver,
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

def rust_simple_literal_type(type_bytes: bytes) -> tuple[bool, bytes | None]: ...

def rust_is_simple_literal(type_bytes: bytes, resolver: NativeTypeResolver) -> bool | None: ...

def rust_is_literal_type_like(
    type_bytes: bytes, resolver: NativeTypeResolver | None = None
) -> bool | None: ...

def rust_try_getting_str_literals_from_type(
    type_bytes: bytes,
) -> tuple[bool, list[str] | None]: ...

def rust_try_getting_int_literals_from_type(
    type_bytes: bytes,
) -> tuple[bool, list[int] | None]: ...

def rust_try_getting_bool_literals_from_type(
    type_bytes: bytes,
) -> tuple[bool, list[bool] | None]: ...

def rust_try_getting_instance_fallback(
    t_bytes: bytes, resolver: NativeTypeResolver
) -> tuple[bool, bytes | None] | None: ...

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

def rust_has_explicit_any(type_bytes: bytes) -> bool | None: ...

def rust_has_any_from_unimported_type(type_bytes: bytes) -> bool | None: ...

def rust_collect_all_inner_types(type_bytes: bytes) -> list[bytes] | None: ...

def rust_make_optional_type(type_bytes: bytes) -> bytes | None: ...

def rust_unknown_unpack(type_bytes: bytes) -> bool | None: ...

def rust_has_explicit_any_live(resolver: NativeTypeResolver, type_bytes: bytes) -> bool | None: ...

def rust_has_any_from_unimported_type_live(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bool | None: ...

def rust_has_any_from_unimported_type_live_noresolver(typ: Any) -> bool | None: ...

def rust_collect_all_inner_types_live(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> list[bytes] | None: ...

def rust_make_optional_type_live(
    resolver: NativeTypeResolver, type_bytes: bytes
) -> bytes | None: ...

def rust_unknown_unpack_live(resolver: NativeTypeResolver, type_bytes: bytes) -> bool | None: ...

def rust_apply_generic_arguments(
    resolver: NativeTypeResolver,
    callable_bytes: bytes,
    orig_types_bytes: bytes,
    skip_unsatisfied: bool,
    strict_optional: bool,
) -> bytes | None: ...

def rust_has_no_typevars(type_bytes: bytes) -> bool | None: ...

def rust_get_target_type(
    tvar_wire: bytes,
    type_wire: bytes,
    skip_unsatisfied: bool,
    same_type_ok: bool | None,
    bound_ok: bool | None,
    value_subtypes: list[bool] | None,
    narrow_matrix: list[bool] | None,
) -> tuple[int, int] | None: ...

def rust_classify_type_object_type(info: Any) -> tuple[int, bool, bool, bool, Any] | None: ...

def rust_is_typevar_default_recursive(tv_fname: str, start: object) -> bool | None: ...

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
    strict_optional: bool,
    infer_polymorphic: bool,
) -> list[bytes] | None: ...

def rust_select_trivial(options_bytes: bytes) -> bytes | None: ...

def rust_exclude_non_meta_vars(option_bytes: bytes) -> bytes | None: ...

def rust_is_similar_constraints(x_bytes: bytes, y_bytes: bytes) -> bool | None: ...

def rust_any_constraints(
    options_bytes: bytes, eager: bool, strict_optional: bool, resolver: NativeTypeResolver
) -> list[bytes] | None: ...

def rust_repack_callable_args(
    callable_bytes: bytes, resolver: NativeTypeResolver
) -> list[bytes] | None: ...

def rust_merge_with_any(constraint_bytes: bytes) -> bool | None: ...

def rust_filter_satisfiable(
    option_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
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
    strict_optional: bool,
) -> bytes | None: ...

def rust_infer_callable_arguments_constraints(
    resolver: NativeTypeResolver,
    template_bytes: bytes,
    actual_bytes: bytes,
    direction: int,
    strict_optional: bool,
) -> bytes | None: ...

def rust_is_type_type(tp_bytes: bytes) -> bool | None: ...

def rust_skip_reverse_union_constraints(constraints_bytes: bytes) -> bytes | None: ...

def rust_unwrap_type_type(tp_bytes: bytes) -> bytes | None: ...

def rust_solve_one(
    lowers: list[bytes],
    uppers: list[bytes],
    infer_unions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[int, bytes | None] | None: ...

def rust_is_trivial_bound(type_bytes: bytes, allow_tuple: bool) -> bool | None: ...

def rust_find_linear(
    constraint_bytes: bytes,
) -> tuple[bool, tuple[int, int, str] | None] | None: ...

def rust_separate_union_literals(type_bytes: bytes) -> tuple[list[bytes], list[bytes]] | None: ...

def rust_get_type_vars(type_bytes: bytes, include_all: bool) -> list[bytes] | None: ...

def rust_get_type_vars_live(
    resolver: NativeTypeResolver, type_bytes: bytes, include_all: bool
) -> list[bytes] | None: ...

def rust_get_vars(
    target_bytes: bytes, vars: list[tuple[int, int, str]]
) -> list[tuple[int, int, str]] | None: ...

def rust_is_callable_protocol(resolver: NativeTypeResolver, t_bytes: bytes) -> bool | None: ...

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
    iterable_type: bytes | None,
    mapping_type: bytes | None,
) -> bytes | None: ...

def rust_solve_dependent(
    vars_bytes: list[bytes],
    constraints_bytes: list[bytes],
    infer_unions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> tuple[int, bytes | None, bytes | None] | None: ...

def rust_callables_compatible(
    left_bytes: bytes,
    right_bytes: bytes,
    proper_subtype: bool,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
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
    infer_unions: bool = False,
) -> bool | None: ...

def rust_are_args_compatible(
    left: Any,
    right: Any,
    ignore_pos_arg_names: bool,
    allow_partial_overlap: bool,
    allow_imprecise_kinds: bool,
) -> int | None: ...

def rust_classify_type_parameter(left: Any, variance: int, proper_subtype: bool) -> int | None: ...

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
    infer_unions: bool = False,
) -> bool | None: ...

def rust_is_same_type(
    a_bytes: bytes,
    b_bytes: bytes,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
) -> bool | None: ...

def rust_all_same_types(
    items_bytes: list[bytes],
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
) -> bool | None: ...

def rust_is_more_precise(
    left_bytes: bytes,
    right_bytes: bytes,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
) -> bool | None: ...

def rust_is_erased_instance(t_bytes: bytes) -> bool | None: ...

def rust_has_underscore_prefix(name: str) -> bool: ...

def rust_try_restrict_literal_union(
    t_bytes: bytes, s_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> list[bytes] | None: ...

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

def rust_coerce_to_literal(type_bytes: bytes, resolver: NativeTypeResolver) -> bytes | None: ...

def rust_is_singleton_identity_type(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...

def rust_is_singleton_equality_type(
    type_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...


# Issue #578: typeanal_queries functions (live PyO3 objects)
def rust_validate_instance(t: Any, fail: Any, indexed: bool) -> bool | None: ...

def rust_detect_diverging_alias(node: Any, target: Any) -> bool | None: ...

def rust_find_self_type(typ: Any, lookup: Any) -> bool | None: ...

def rust_find_self_type_live(
    resolver: NativeTypeResolver, typ: Any, lookup: Any
) -> bool | None: ...

def rust_check_vec_type_args(args: Any, ctx: Any, api: Any) -> bool | None: ...

def rust_check_unpacks_in_list(items: Any) -> tuple[list[int], int | None] | None: ...


# mypy/types.py — callable formal-arg introspection + copy_modified.
def rust_callable_formal_arguments(
    typ_bytes: bytes,
) -> list[tuple[str | None, int | None, bool]] | None: ...

def rust_callable_argument_by_name(
    typ_bytes: bytes, name: str | None
) -> tuple[str | None, int | None, bool, int] | None: ...

def rust_callable_argument_by_position(
    typ_bytes: bytes, position: int | None
) -> tuple[str | None, int | None, bool, int] | None: ...

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
    implicit: bool, allow_tuple_literal: bool, items_len: int
) -> int | None: ...

def rust_classify_raw_expression_type(
    report_invalid_types: bool, base_type_name: str, note_is_none: bool
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
    arg_count: int, arg0_is_type_list: bool, arg0_is_ellipsis: bool, disallow_any_generics: bool
) -> int | None: ...

def rust_classify_type_guard_arg(fullname: str, args_len: int, is_typeis: bool) -> int | None: ...

def rust_adjust_tuple(left_bytes: bytes, r_bytes: bytes) -> Any: ...

def rust_analyze_unbound_without_info(
    is_var_any: bool,
    allow_type_any: bool,
    is_type_instance: bool,
    is_type_type_any: bool,
    unbound_tvar: Any,
    allow_unbound_tvars: bool,
    is_enum_member: bool,
    defining_literal: Any,
    is_new_style: bool,
    tail_kind: int,
    name: str,
) -> Any: ...

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
    t_bytes: bytes, strict_optional: bool, resolver: NativeTypeResolver
) -> Any: ...

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

def rust_detach_callable(typ_bytes: bytes, class_type_vars_bytes: bytes) -> Any: ...

def rust_expand_callable_variants(type_bytes: bytes, strict_optional: bool) -> Any: ...

def rust_expand_tuple_if_possible(tup_bytes: bytes, target: Any) -> Any: ...

def rust_get_possible_variants(typ_bytes: bytes, resolver: NativeTypeResolver) -> Any: ...

def rust_instantiate_type_alias(
    node: Any, arg_blobs: Any, no_args: Any, empty_tuple_index: Any, analyzing_tvar_def: Any
) -> int | None: ...

def rust_is_enum_overlapping_union(
    x_bytes: bytes, y_bytes: bytes, resolver: NativeTypeResolver
) -> Any: ...

def rust_is_literal_in_union(x_bytes: bytes, y_bytes: bytes) -> Any: ...

def rust_is_none_object_overlap(t1_bytes: bytes, t2_bytes: bytes) -> Any: ...

def rust_is_object(t_bytes: bytes) -> Any: ...

def rust_is_overlapping_erased_types(
    left_bytes: bytes,
    right_bytes: bytes,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: NativeTypeResolver,
) -> Any: ...

def rust_is_tuple(t_bytes: bytes) -> Any: ...

def rust_try_contracting_literals_in_union(
    type_list_bytes: bytes, resolver: NativeTypeResolver
) -> Any: ...

def rust_type_object_type_from_function(
    signature_bytes: bytes,
    info: Any,
    def_info: Any,
    fallback_bytes: bytes,
    is_new: bool,
    strict_optional: bool,
    infer_unions: bool,
    resolver: NativeTypeResolver,
) -> Any: ...

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

def rust_erase_return_self_types(typ_bytes: bytes, self_type_bytes: bytes) -> bytes | None: ...

def rust_fill_typevars_with_any(typ: Any) -> bytes | None: ...

def rust_infer_variance_member(
    member_type_bytes: bytes,
    self_type_bytes: bytes,
    object_type_bytes: bytes,
    raw_id: int,
    resolver: NativeTypeResolver,
    infer_unions: bool = False,
) -> int | None: ...

def rust_is_better(
    t_bytes: bytes, s_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...

def rust_is_descriptor(
    resolver: NativeTypeResolver, type_bytes: bytes, strict_optional: bool
) -> bool | None: ...

def rust_is_disjoint_base(info: Any) -> bool: ...

def rust_is_recursive_pair(
    s_bytes: bytes, t_bytes: bytes, resolver: NativeTypeResolver
) -> bool | None: ...

def rust_is_valid_constructor(n: Any) -> bool: ...

def rust_map_instance_to_supertypes(
    resolver: NativeTypeResolver, items_wire: bytes, supertype_ref: str
) -> tuple[bytes, list[bool]] | None: ...

def rust_match_generic_callables(
    num_vars: int, start_raw_id: int, t_bytes: bytes, s_bytes: bytes
) -> tuple[int, bytes, bytes] | None: ...


# F reopening experiment (#1671): Rust-owned `Instance` field storage.
# Handles are minted by rust_view_put through the shared identity service;
# an encode is served only while its stamp still matches the caller's.
def rust_view_put(
    obj: Any,
    fullname: str,
    args: list[Any],
    arg_handles: list[int],
    fixed_up: bool,
    args_tvar_clean: bool,
    stamp: int,
) -> int: ...

def rust_view_encode(handle: int, stamp: int, live_fullname: str) -> bytes | None: ...

def rust_view_args(handle: int, stamp: int) -> tuple[Any, ...] | None: ...

def rust_view_touch(handle: int) -> bool: ...

def rust_view_reset() -> int: ...

def rust_view_count() -> int: ...

def rust_view_stats() -> tuple[int, int, int]: ...


__all__ = [
    "NativeTypeResolver",
    "erase_type",
    "remove_instance_last_known_values",
    "shallow_erase_type_for_equality",
    "read_type_to_str",
    "read_alias_recursion_flag",
    "build_resolver",
    "read_type_to_str_with_resolver",
    "build_native_resolver",
    "read_type_to_str_with_native_resolver",
    "rust_is_subtype",
    "rust_is_subtype_batch",
    "rust_subtype_tvar_tuple_right",
    "rust_variadic_tuple_subtype",
    "rust_trivial_join",
    "rust_trivial_meet",
    "rust_join_types",
    "rust_join_instances",
    "rust_meet_types",
    "rust_join_tuples",
    "rust_meet_tuples",
    "rust_object_or_any_from_type",
    "rust_combine_similar_callables",
    "rust_object_from_instance",
    "rust_join_sorted_key",
    "rust_narrow_declared_type",
    "rust_map_actuals_to_formals",
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
    "rust_freshen_all_functions_type_vars",
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
    "rust_has_explicit_any",
    "rust_has_any_from_unimported_type",
    "rust_collect_all_inner_types",
    "rust_make_optional_type",
    "rust_unknown_unpack",
    "rust_has_explicit_any_live",
    "rust_has_any_from_unimported_type_live",
    "rust_has_any_from_unimported_type_live_noresolver",
    "rust_collect_all_inner_types_live",
    "rust_make_optional_type_live",
    "rust_unknown_unpack_live",
    "rust_apply_generic_arguments",
    "rust_has_no_typevars",
    "rust_get_target_type",
    "rust_classify_type_object_type",
    "rust_is_typevar_default_recursive",
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
    "rust_filter_imprecise_kinds",
    "rust_infer_directed_arg_constraints",
    "rust_infer_callable_arguments_constraints",
    "rust_is_type_type",
    "rust_skip_reverse_union_constraints",
    "rust_unwrap_type_type",
    "rust_solve_one",
    "rust_is_trivial_bound",
    "rust_find_linear",
    "rust_separate_union_literals",
    "rust_get_type_vars",
    "rust_get_type_vars_live",
    "rust_get_vars",
    "rust_is_callable_protocol",
    "rust_solve_constraints",
    "rust_infer_function_type_arguments",
    "rust_solve_dependent",
    "rust_callables_compatible",
    "rust_are_parameters_compatible",
    "rust_are_args_compatible",
    "rust_classify_type_parameter",
    "rust_is_overlapping_types",
    "rust_is_equivalent",
    "rust_is_same_type",
    "rust_all_same_types",
    "rust_is_more_precise",
    "rust_is_erased_instance",
    "rust_has_underscore_prefix",
    "rust_try_restrict_literal_union",
    "rust_tuple_fallback",
    "rust_bind_self",
    "rust_fill_typevars",
    "rust_class_callable",
    "rust_function_type",
    "rust_callable_type",
    "rust_erase_to_bound",
    "rust_map_type_from_supertype",
    "rust_coerce_to_literal",
    "rust_is_singleton_identity_type",
    "rust_is_singleton_equality_type",
    "rust_validate_instance",
    "rust_detect_diverging_alias",
    "rust_find_self_type",
    "rust_find_self_type_live",
    "rust_check_vec_type_args",
    "rust_check_unpacks_in_list",
    "rust_callable_formal_arguments",
    "rust_callable_argument_by_name",
    "rust_callable_argument_by_position",
    "rust_callable_is_generic",
    "rust_callable_is_kw_arg",
    "rust_callable_is_var_arg",
    "rust_callable_max_possible_positional_args",
    "rust_callable_min_args",
    "rust_can_be_false_default",
    "rust_can_be_true_default",
    "rust_can_be_false_default_live",
    "rust_can_be_true_default_live",
    "rust_tuple_length",
    "rust_union_length",
    "rust_type_analyze",
    "rust_classify_special_unbound",
    "rust_classify_tuple_type_implicit",
    "rust_classify_raw_expression_type",
    "rust_classify_check_warn_deprecated",
    "rust_classify_analyze_callable_type",
    "rust_classify_type_guard_arg",
    "rust_adjust_tuple",
    "rust_analyze_unbound_without_info",
    "rust_are_related_types",
    "rust_are_tuples_overlapping",
    "rust_are_typed_dicts_overlapping",
    "rust_builtin_item_type",
    "rust_classify_type_with_info",
    "rust_classify_unbound_front",
    "rust_detach_callable",
    "rust_expand_callable_variants",
    "rust_expand_tuple_if_possible",
    "rust_get_possible_variants",
    "rust_instantiate_type_alias",
    "rust_is_enum_overlapping_union",
    "rust_is_literal_in_union",
    "rust_is_none_object_overlap",
    "rust_is_object",
    "rust_is_overlapping_erased_types",
    "rust_is_tuple",
    "rust_try_contracting_literals_in_union",
    "rust_type_object_type_from_function",
    "rust_classify_literal_param",
    "rust_erase_return_self_types",
    "rust_fill_typevars_with_any",
    "rust_infer_variance_member",
    "rust_is_better",
    "rust_is_descriptor",
    "rust_is_disjoint_base",
    "rust_is_recursive_pair",
    "rust_is_valid_constructor",
    "rust_map_instance_to_supertypes",
    "rust_match_generic_callables",
    "rust_view_put",
    "rust_view_encode",
    "rust_view_args",
    "rust_view_touch",
    "rust_view_reset",
    "rust_view_count",
    "rust_view_stats",
]
