"""Type checking of attribute access"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any, Final, TypeVar, cast

import copy

from mypy import message_registry, state
from mypy.checker_shared import TypeCheckerSharedApi
from mypy.erasetype import erase_typevars
from mypy.expandtype import (
    expand_self_type,
    expand_type_by_instance,
    freshen_all_functions_type_vars,
)
from mypy.lookup import lookup_stdlib_typeinfo
from mypy.maptype import map_instance_to_supertype
from mypy.meet import is_overlapping_types
from mypy.messages import MessageBuilder
from mypy.modules_state import modules_state
from mypy.nodes import (
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    EXCLUDED_ENUM_ATTRIBUTES,
    SYMBOL_FUNCBASE_TYPES,
    Context,
    Decorator,
    Expression,
    FuncBase,
    FuncDef,
    IndexExpr,
    MypyFile,
    NameExpr,
    OverloadedFuncDef,
    SymbolTable,
    TempNode,
    TypeAlias,
    TypeInfo,
    TypeVarLikeExpr,
    Var,
    is_final_node,
)
from mypy.plugin import AttributeContext
from mypy.subtypes import is_subtype
from mypy.typeops import (
    bind_self,
    erase_to_bound,
    freeze_all_type_vars,
    function_type,
    get_all_type_vars,
    make_simplified_union,
    supported_self_type,
    tuple_fallback,
)
from mypy.types import (
    AnyType,
    CallableType,
    _encode_no_arg_instance,
    _serialize_stats,
    _serialize_stats_on,
    DeletedType,
    FunctionLike,
    Instance,
    _serialize_with_taint_check,
    _type_wire_cache,
    _wire_cache_enabled,
    LiteralType,
    NoneType,
    Overloaded,
    ParamSpecType,
    PartialType,
    ProperType,
    TupleType,
    Type,
    TypeAliasType,
    TypedDictType,
    TypeOfAny,
    TypeType,
    TypeVarId,
    TypeVarLikeType,
    TypeVarTupleType,
    TypeVarType,
    UninhabitedType,
    UnionType,
    get_proper_type,
    instance_cache,
)

# M20: type-kernel seam for checkmember. When the `type_kernel` Rust
# extension is importable and `Options.native_type_kernel` is set,
# `bind_self_fast` and `_analyze_member_access` dispatch route through

# Rust. The Rust path returns `None` for any type it does not handle, in
# which case we fall back to the pure-Python implementation. This is the
# strangler-fig per-call gate — no behavior change unless the option is

# explicitly enabled.
#
# Import the buffer + read_type helpers and the 12 pre-existing checkmember

# kernels. Issue-#476 adds five new kernels; four of them (check_self_arg,
# expand_without_binding, expand_and_bind_callable, add_class_tvars) have a
# known parity gap: the wire round-trip drops the CallableType.definition

# link used by error-message rendering and Self-typevar solving, so their
# gates defer to Python via `is not None` checks. They import real when
# available so the Rust bytes still count toward the migration;

# `descriptor_has_get_set` is parity-clean (0 failures) and fully active.
try:
    from librt.internal import (
        ReadBuffer as _CheckMemberReadBuffer,
        WriteBuffer as _CheckMemberWriteBuffer,
    )
    from type_kernel import (
        rust_analyze_descriptor_access as _rust_analyze_descriptor_access,
        rust_analyze_enum_class_attribute_access as _rust_analyze_enum_class_attribute_access,
        rust_analyze_instance_member_access as _rust_analyze_instance_member_access,
        rust_analyze_instance_member_dispatch as _rust_analyze_instance_member_dispatch,
        rust_analyze_member_access as _rust_analyze_member_access,
        rust_analyze_member_method as _rust_analyze_member_method,
        rust_analyze_none_member_access as _rust_analyze_none_member_access,
        rust_analyze_typeddict_access as _rust_analyze_typeddict_access,
        rust_analyze_union_member_access as _rust_analyze_union_member_access,
        rust_bind_self_fast as _rust_bind_self_fast,
        rust_classify_type_type_member_access as _rust_classify_type_type_member_access,
        rust_defined_in_superclass as _rust_defined_in_superclass,
        rust_has_operator as _rust_has_operator,
        rust_instance_fallback as _rust_instance_fallback,
        rust_meta_has_operator as _rust_meta_has_operator,
        rust_is_instance_var as _rust_is_instance_var,
    )

    from mypy.types import read_type as _checkmember_read_type

    _HAS_TYPE_KERNEL = True
except ImportError:
    _rust_bind_self_fast = None  # type: ignore[assignment]
    _rust_instance_fallback = None  # type: ignore[assignment]
    _rust_has_operator = None  # type: ignore[assignment]
    _rust_meta_has_operator = None  # type: ignore[assignment]
    _rust_is_instance_var = None  # type: ignore[assignment]
    _rust_defined_in_superclass = None  # type: ignore[assignment]
    _rust_classify_type_type_member_access = None  # type: ignore[assignment]
    _rust_analyze_member_access = None  # type: ignore[assignment]
    _rust_analyze_member_method = None  # type: ignore[assignment]
    _rust_analyze_instance_member_access = None  # type: ignore[assignment]
    _rust_analyze_instance_member_dispatch = None  # type: ignore[assignment]
    _rust_analyze_union_member_access = None  # type: ignore[assignment]
    _rust_analyze_none_member_access = None  # type: ignore[assignment]
    _rust_analyze_typeddict_access = None  # type: ignore[assignment]
    _rust_analyze_enum_class_attribute_access = None  # type: ignore[assignment]
    _rust_analyze_descriptor_access = None  # type: ignore[assignment]
    _CheckMemberReadBuffer = None  # type: ignore[assignment,misc]
    _CheckMemberWriteBuffer = None  # type: ignore[assignment,misc]
    _checkmember_read_type = None  # type: ignore[assignment]
    _HAS_TYPE_KERNEL = False

# Issue-#476 kernels. `descriptor_has_get_set` is parity-clean and active.
# The other four were deferred in #484 because the wire format drops
# CallableType.definition; wirefixup now re-links definition by name + arity

# (issue #485), so they are active again via the inner try-block below.
# Any kernels that fail to import fall back to Python (None gates).
_rust_check_self_arg: Any = None
_rust_expand_without_binding: Any = None
_rust_expand_and_bind_callable: Any = None
_rust_add_class_tvars: Any = None
_rust_descriptor_has_get_set: Any = None
if _HAS_TYPE_KERNEL:
    try:
        from type_kernel import (
            rust_descriptor_has_get_set as _rust_descriptor_has_get_set,
        )
    except ImportError:
        pass
    try:
        from type_kernel import (
            rust_check_self_arg as _rust_check_self_arg,
            rust_expand_without_binding as _rust_expand_without_binding,
            rust_expand_and_bind_callable as _rust_expand_and_bind_callable,
            rust_add_class_tvars as _rust_add_class_tvars,
        )
    except ImportError:
        pass

_native_checkmember_active: bool = False


def _set_native_checkmember_active(active: bool) -> None:
    """Called by the build manager to enable/disable the Rust path."""
    global _native_checkmember_active
    _native_checkmember_active = active


_native_checkmember_resolver: Any = None


def _set_native_checkmember_resolver(resolver: Any) -> None:
    """Install/clear the NativeTypeResolver shared with the checkmember kernel."""
    global _native_checkmember_resolver
    _native_checkmember_resolver = resolver


# Decision tags from `rust_classify_type_type_member_access`; must match
# `TT_*` constants in crates/type_kernel/src/checkmember.rs.
NATIVE_TT_NONE = 0
NATIVE_TT_ITEM_INSTANCE = 1
NATIVE_TT_ITEM_ANY = 2
NATIVE_TT_TV_UB_INSTANCE = 3
NATIVE_TT_TV_UB_UNION = 4
NATIVE_TT_TV_UB_TUPLE = 5
NATIVE_TT_TV_UB_ANY = 6
NATIVE_TT_TV_UB_OTHER = 7
NATIVE_TT_ITEM_TUPLE = 8
NATIVE_TT_ITEM_FUNC_TYPEOBJ = 9
NATIVE_TT_ITEM_FUNC_NOT_TYPEOBJ = 10
NATIVE_TT_ITEM_TYPE_TYPE_INSTANCE = 11
NATIVE_TT_ITEM_TYPE_TYPE_OTHER = 12


_BUILTIN_INSTANCE_BYTES: Final[dict[str, bytes]] = {
    "builtins.str": b"\x50\x53",
    "builtins.function": b"\x50\x54",
    "builtins.int": b"\x50\x55",
    "builtins.bool": b"\x50\x56",
    "builtins.object": b"\x50\x57",
}


# bytes -> fixed Type cache for checkmember deserialize, split into
# freeze/non-freeze pools: freeze callers (bind_self_fast and friends)
# mutate CallableType.variables in place and share nothing with member path.
_deser_caches: dict[bool, dict[bytes, Type]] = {False: {}, True: {}}
_deser_cache_hits: dict[bool, list[int]] = {False: [0], True: [0]}
_deser_cache_misses: dict[bool, list[int]] = {False: [0], True: [0]}


def _clear_deser_cache() -> None:
    for cache in _deser_caches.values():
        cache.clear()
    for hits in _deser_cache_hits.values():
        hits[0] = 0
    for misses in _deser_cache_misses.values():
        misses[0] = 0


def _serialize_type_for_checkmember(t: Type) -> bytes:
    if _serialize_stats_on:
        _serialize_stats["calls"] += 1
    key = id(t)
    if _wire_cache_enabled():
        entry = _type_wire_cache.get(key)
        if entry is not None and entry[0] is t:
            if _serialize_stats_on:
                _serialize_stats["hits"] += 1
            return entry[1]
    if type(t) is Instance:
        fn = t.type.fullname
        if (
            not t.args
            and not t.last_known_value
            and not t.extra_attrs
            and fn in _BUILTIN_INSTANCE_BYTES
        ):
            if _serialize_stats_on:
                _serialize_stats["builtin"] += 1
            return _BUILTIN_INSTANCE_BYTES[fn]
    fast = _encode_no_arg_instance(t, _CheckMemberWriteBuffer)
    if fast is not None:
        if _wire_cache_enabled() and t.type_ref is None:  # type: ignore[misc]
            if _serialize_stats_on:
                _serialize_stats["writes"] += 1
                _serialize_stats["bytes"] += len(fast)
            _type_wire_cache[key] = (t, fast)
        return fast
    buf = _CheckMemberWriteBuffer()
    result, saw_tvar = _serialize_with_taint_check(t, buf)
    if saw_tvar:
        if _serialize_stats_on:
            _serialize_stats["tvar"] += 1
    elif _wire_cache_enabled() and (not isinstance(t, Instance) or t.type_ref is None):  # type: ignore[misc]
        if _serialize_stats_on:
            _serialize_stats["writes"] += 1
            _serialize_stats["bytes"] += len(result)
        _type_wire_cache[key] = (t, result)
    return result


def _deserialize_type_for_checkmember(data: bytes, freeze: bool = False) -> Type | None:
    """Decode wire bytes, resolving type_ref to live TypeInfo via wirefixup.

    Returns None when a type_ref is unresolvable so callers defer to Python.
    ``freeze`` selects the cache pool: callers that run
    ``freeze_all_type_vars`` on the result (bind_self_fast and friends)
    must not share objects with member-access callers, whose returned
    non-frozen objects the freeze would mutate in place.
    """
    from mypy.types import instance_cache
    from mypy.wirefixup import canonicalize_fresh_vars, check_no_fake_info, fixup_wire_type

    _hits = _deser_cache_hits[freeze]
    _dc = _deser_caches[freeze].get(data)
    if _dc is not None:
        _hits[0] += 1
        # Shallow copy: callers mutate top-level line/column on the
        # decoded object; identical bytes must not cross-contaminate
        # call sites that apply different values.
        return copy.copy(_dc)

    _deser_cache_misses[freeze][0] += 1

    decoded = _checkmember_read_type(_CheckMemberReadBuffer(data))
    # Clear instance_cache primitives after read_type so NOT_READY
    # singletons cannot leak into later builds (mirrors typeops.py).
    instance_cache.int_type = None
    instance_cache.str_type = None
    instance_cache.bool_type = None
    instance_cache.object_type = None
    instance_cache.function_type = None
    fixed = fixup_wire_type(decoded)
    if fixed is None or not check_no_fake_info(fixed):
        # A decoded tree carrying a residual fake TypeInfo (stale wire
        # typeinfo entry across fine-grained refreshes) must not enter
        # the type graph; defer to Python.
        return None
    # Wire round-trip loses fresh meta-var identity; re-unify
    # occurrences before returning (mirrors expandtype.py:463-467).
    fixed = canonicalize_fresh_vars(fixed)
    if fixed is not None:
        _deser_caches[freeze][data] = fixed
    return fixed


def _restore_definition(original: Type, decoded: Type) -> Type:
    """Copy the ``definition`` link from ``original`` onto ``decoded``.

    The wire format drops ``CallableType.definition`` (only used for error
    messages). The ``_TypeRefFixer._match_definition`` fallback tries to
    relink it from the fallback TypeInfo's symbol table, but for methods
    the fallback is ``builtins.function``, not the defining class, so the
    lookup fails. This helper restores the link directly from the original
    live object, mirroring how ``bind_self_fast`` preserves non-wire fields
    via ``copy_modified`` on the original object.
    """
    from mypy.types import CallableType, Overloaded

    if isinstance(original, CallableType) and isinstance(decoded, CallableType):  # type: ignore[misc]
        if original.definition is not None and decoded.definition is None:
            return decoded.copy_modified(definition=original.definition)
    elif isinstance(original, Overloaded) and isinstance(decoded, Overloaded):  # type: ignore[misc]
        if len(original.items) == len(decoded.items):
            new_items = []
            changed = False
            for orig, dec in zip(original.items, decoded.items):
                if orig.definition is not None and dec.definition is None:
                    new_items.append(dec.copy_modified(definition=orig.definition))
                    changed = True
                else:
                    new_items.append(dec)
            if changed:
                return Overloaded(new_items)
    elif isinstance(original, Overloaded) and isinstance(decoded, CallableType):  # type: ignore[misc]
        # check_self_arg filters an Overloaded to a single CallableType;
        # find the matching item by arg-type signature to restore its
        # definition link.
        for orig in original.items:
            if (
                orig.definition is not None
                and len(orig.arg_types) == len(decoded.arg_types)
            ):
                return decoded.copy_modified(definition=orig.definition)
    return decoded


def _restore_native_method_definition(
    name: str, typ: Type, decoded: Type
) -> Type:
    """Best-effort relink of ``definition`` for a native-decoded method type.

    The general member-access seam resolves a fallback to an Instance and
    dispatches the method branch natively, so the decoded result is a
    method signature whose ``definition`` the wire format dropped. That link
    drives error formatting (e.g. ``pretty_callable`` re-inserting the bound
    ``self`` arg in overload-variant notes), so mirror the instance/union
    seams by restoring it from the method node on the resolved class.
    Returns ``decoded`` unchanged when no method node is resolvable.
    """
    from mypy.types import CallableType, Instance, TupleType

    info = None
    if isinstance(typ, Instance):
        info = typ.type
    elif isinstance(typ, TupleType):
        from mypy.typeops import tuple_fallback

        fallback = tuple_fallback(typ)
        if isinstance(fallback, Instance):
            info = fallback.type
    elif isinstance(typ, (LiteralType, CallableType)):
        fallback = typ.fallback
        if isinstance(fallback, Instance):
            info = fallback.type
    if info is None:
        return decoded
    method = info.get_method(name) if name else None
    if (
        method is not None
        and not isinstance(method, Decorator)
        and getattr(method, "type", None) is not None
    ):
        return _restore_definition(method.type, decoded)
    return decoded


class MemberContext:
    """Information and objects needed to type check attribute access.

    Look at the docstring of analyze_member_access for more information.
    """

    def __init__(
        self,
        *,
        is_lvalue: bool,
        is_super: bool,
        is_operator: bool,
        original_type: Type,
        context: Context,
        chk: TypeCheckerSharedApi,
        self_type: Type | None = None,
        module_symbol_table: SymbolTable | None = None,
        no_deferral: bool = False,
        is_self: bool = False,
        rvalue: Expression | None = None,
        suppress_errors: bool = False,
        preserve_type_var_ids: bool = False,
    ) -> None:
        self.is_lvalue = is_lvalue
        self.is_super = is_super
        self.is_operator = is_operator
        self.original_type = original_type
        self.self_type = self_type or original_type
        self.context = context  # Error context
        self.chk = chk
        self.msg = chk.msg
        self.module_symbol_table = module_symbol_table
        self.no_deferral = no_deferral
        self.is_self = is_self
        if rvalue is not None:
            assert is_lvalue
        self.rvalue = rvalue
        self.suppress_errors = suppress_errors
        # This attribute is only used to preserve old protocol member access logic.
        # It is needed to avoid infinite recursion in cases involving self-referential
        # generic methods, see find_member() for details. Do not use for other purposes!
        self.preserve_type_var_ids = preserve_type_var_ids

    def named_type(self, name: str) -> Instance:
        return self.chk.named_type(name)

    def not_ready_callback(self, name: str, context: Context) -> None:
        self.chk.handle_cannot_determine_type(name, context)

    def fail(self, msg: str) -> None:
        if not self.suppress_errors:
            self.msg.fail(msg, self.context)

    def copy_modified(
        self,
        *,
        self_type: Type | None = None,
        is_lvalue: bool | None = None,
        original_type: Type | None = None,
    ) -> MemberContext:
        mx = MemberContext(
            is_lvalue=self.is_lvalue,
            is_super=self.is_super,
            is_operator=self.is_operator,
            original_type=self.original_type,
            context=self.context,
            chk=self.chk,
            self_type=self.self_type,
            module_symbol_table=self.module_symbol_table,
            no_deferral=self.no_deferral,
            rvalue=self.rvalue,
            suppress_errors=self.suppress_errors,
            preserve_type_var_ids=self.preserve_type_var_ids,
        )
        if self_type is not None:
            mx.self_type = self_type
        if is_lvalue is not None:
            mx.is_lvalue = is_lvalue
        if original_type is not None:
            mx.original_type = original_type
        return mx


def analyze_member_access(
    name: str,
    typ: Type,
    context: Context,
    *,
    is_lvalue: bool,
    is_super: bool,
    is_operator: bool,
    original_type: Type,
    chk: TypeCheckerSharedApi,
    override_info: TypeInfo | None = None,
    in_literal_context: bool = False,
    self_type: Type | None = None,
    module_symbol_table: SymbolTable | None = None,
    no_deferral: bool = False,
    is_self: bool = False,
    rvalue: Expression | None = None,
    suppress_errors: bool = False,
) -> Type:
    """Return the type of attribute 'name' of 'typ'.

    The actual implementation is in '_analyze_member_access' and this docstring
    also applies to it.

    This is a general operation that supports various different variations:

      1. lvalue or non-lvalue access (setter or getter access)
      2. supertype access when using super() (is_super == True and
         'override_info' should refer to the supertype)

    'original_type' is the most precise inferred or declared type of the base object
    that we have available. When looking for an attribute of 'typ', we may perform
    recursive calls targeting the fallback type, and 'typ' may become some supertype
    of 'original_type'. 'original_type' is always preserved as the 'typ' type used in
    the initial, non-recursive call. The 'self_type' is a component of 'original_type'
    to which generic self should be bound (a narrower type that has a fallback to instance).
    Currently, this is used only for union types.

    'module_symbol_table' is passed to this function if 'typ' is actually a module,
    and we want to keep track of the available attributes of the module (since they
    are not available via the type object directly)

    'rvalue' can be provided optionally to infer better setter type when is_lvalue is True,
    most notably this helps for descriptors with overloaded __set__() method.

    'suppress_errors' will skip any logic that is only needed to generate error messages.
    Note that this more of a performance optimization, one should not rely on this to not
    show any messages, as some may be show e.g. by callbacks called here,
    use msg.filter_errors(), if needed.
    """
    mx = MemberContext(
        is_lvalue=is_lvalue,
        is_super=is_super,
        is_operator=is_operator,
        original_type=original_type,
        context=context,
        chk=chk,
        self_type=self_type,
        module_symbol_table=module_symbol_table,
        no_deferral=no_deferral,
        is_self=is_self,
        rvalue=rvalue,
        suppress_errors=suppress_errors,
    )
    result = _analyze_member_access(name, typ, mx, override_info)
    possible_literal = get_proper_type(result)
    if (
        in_literal_context
        and isinstance(possible_literal, Instance)
        and possible_literal.last_known_value is not None
    ):
        return possible_literal.last_known_value
    else:
        return result


def _analyze_member_access(
    name: str, typ: Type, mx: MemberContext, override_info: TypeInfo | None = None
) -> Type:
    typ = get_proper_type(typ)
    # M20: gate the general dispatch path through Rust when the kernel
    # is active.  Rust handles pure type-transform branches (AnyType,
    # DeletedType, UninhabitedType, TupleType fallback recursion,

    # Literal/Callable/Overloaded fallback recursion,
    # ParamSpec/TypeVarTuple fallback recursion).  Returns None (Python
    # None) for branches needing plugin state, union construction, error

    # reporting, or resolver lookups: Python falls through.  The
    # isinstance gate below also skips types Rust always defers on.
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _native_checkmember_resolver is not None
        and _rust_analyze_member_access is not None
        and not isinstance(
            typ,
            (Instance, UnionType, TypeType, TypedDictType, NoneType, DeletedType, TypeAliasType),
        )
    ):
        try:
            result = _rust_analyze_member_access(
                _native_checkmember_resolver,
                name,
                _serialize_type_for_checkmember(typ),
                _serialize_type_for_checkmember(mx.self_type),
                mx.is_lvalue,
                mx.is_super,
                mx.preserve_type_var_ids,
                TypeVarId.next_raw_id,
                state.state.strict_optional,
            )
            if result is not None:
                next_raw_id, changed, wire_bytes = result
                if changed:
                    TypeVarId.next_raw_id = next_raw_id
                decoded = _deserialize_type_for_checkmember(bytes(wire_bytes))
                if decoded is not None:
                    # The wire round-trip drops .definition (drives error
                    # formatting, e.g. re-inserting the bound self arg in
                    # overload-variant notes). Restore it for method results.
                    if isinstance(decoded, (CallableType, Overloaded)):
                        decoded = _restore_native_method_definition(name, typ, decoded)
                    if isinstance(decoded, ProperType):
                        decoded.line = typ.line
                        decoded.column = typ.column
                        if isinstance(decoded, CallableType):
                            decoded.fallback.line = decoded.line
                    return decoded
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(typ, Instance):
        return analyze_instance_member_access(name, typ, mx, override_info)
    elif isinstance(typ, AnyType):
        # The base object has dynamic type.
        return AnyType(TypeOfAny.from_another_any, source_any=typ)
    elif isinstance(typ, UnionType):
        return analyze_union_member_access(name, typ, mx)
    elif isinstance(typ, FunctionLike) and typ.is_type_obj():
        return analyze_type_callable_member_access(name, typ, mx)
    elif isinstance(typ, TypeType):
        return analyze_type_type_member_access(name, typ, mx, override_info)
    elif isinstance(typ, TupleType):
        # Actually look up from the fallback instance type.
        return _analyze_member_access(name, tuple_fallback(typ), mx, override_info)
    elif isinstance(typ, (LiteralType, FunctionLike)):
        # Actually look up from the fallback instance type.
        return _analyze_member_access(name, typ.fallback, mx, override_info)
    elif isinstance(typ, TypedDictType):
        return analyze_typeddict_access(name, typ, mx, override_info)
    elif isinstance(typ, NoneType):
        return analyze_none_member_access(name, typ, mx)
    elif isinstance(typ, TypeVarLikeType):
        if isinstance(typ, TypeVarType) and typ.values:
            return _analyze_member_access(
                name, make_simplified_union(typ.values), mx, override_info
            )
        return _analyze_member_access(name, typ.upper_bound, mx, override_info)
    elif isinstance(typ, DeletedType):
        if not mx.suppress_errors:
            mx.msg.deleted_as_rvalue(typ, mx.context)
        return AnyType(TypeOfAny.from_error)
    elif isinstance(typ, UninhabitedType):
        attr_type = UninhabitedType()
        attr_type.ambiguous = typ.ambiguous
        return attr_type
    return report_missing_attribute(mx.original_type, typ, name, mx)


def may_be_awaitable_attribute(
    name: str, typ: Type, mx: MemberContext, override_info: TypeInfo | None = None
) -> bool:
    """Check if the given type has the attribute when awaited."""
    if mx.chk.checking_missing_await:
        # Avoid infinite recursion.
        return False
    with mx.chk.checking_await_set(), mx.msg.filter_errors() as local_errors:
        aw_type = mx.chk.get_precise_awaitable_type(typ, local_errors)
        if aw_type is None:
            return False
        _ = _analyze_member_access(
            name, aw_type, mx.copy_modified(self_type=aw_type), override_info
        )
        return not local_errors.has_new_errors()


def report_missing_attribute(
    original_type: Type,
    typ: Type,
    name: str,
    mx: MemberContext,
    override_info: TypeInfo | None = None,
) -> Type:
    if mx.suppress_errors:
        return AnyType(TypeOfAny.from_error)
    error_code = mx.msg.has_no_attr(original_type, typ, name, mx.context, mx.module_symbol_table)
    if not mx.msg.prefer_simple_messages():
        if may_be_awaitable_attribute(name, typ, mx, override_info):
            mx.msg.possible_missing_await(mx.context, error_code)
    return AnyType(TypeOfAny.from_error)


# The several functions that follow implement analyze_member_access for various
# types and aren't documented individually.


def analyze_instance_member_access(
    name: str, typ: Instance, mx: MemberContext, override_info: TypeInfo | None
) -> Type:
    info = typ.type
    if override_info:
        info = override_info

    method = info.get_method(name)

    if name == "__init__" and not mx.is_super and not info.is_final:
        if not method or not method.is_final:
            # Accessing __init__ in statically typed code would compromise
            # type safety unless used via super() or the method/class is final.
            mx.fail(message_registry.CANNOT_ACCESS_INIT)
            return AnyType(TypeOfAny.from_error)

    # The base object has an instance type.

    if (
        state.find_occurrences
        and info.name == state.find_occurrences[0]
        and name == state.find_occurrences[1]
        and not mx.suppress_errors
    ):
        mx.msg.note("Occurrence of '{}.{}'".format(*state.find_occurrences), mx.context)

    # M20 kernel dispatch (#805): the whole method branch is one Rust
    # call for a plain FuncBase (freshen, static/trivial-self tail,
    # generic tail). Deferred: Decorator, property, lvalue/super, plugin.
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _native_checkmember_resolver is not None
        and _rust_analyze_instance_member_dispatch is not None
        and method is not None
        and not isinstance(method, Decorator)
        and not method.is_property
        and not mx.is_super
        and not mx.is_lvalue
    ):
        try:
            result = _rust_analyze_instance_member_dispatch(
                _native_checkmember_resolver,
                _serialize_type_for_checkmember(typ),
                name,
                override_info.fullname if override_info else None,
                _serialize_type_for_checkmember(mx.self_type),
                mx.no_deferral,
                mx.preserve_type_var_ids,
                TypeVarId.next_raw_id,
                state.state.strict_optional,
            )
            if result is not None:
                next_raw_id, changed, wire_bytes = result
                if changed:
                    TypeVarId.next_raw_id = next_raw_id
                decoded = _deserialize_type_for_checkmember(bytes(wire_bytes))
                if decoded is not None and isinstance(decoded, ProperType):
                    decoded.line = typ.line
                    decoded.column = typ.column
                    if isinstance(decoded, CallableType):
                        decoded.fallback.line = decoded.line
                method_sig = getattr(method, "type", None)
                if method_sig is not None:
                    decoded = _restore_definition(method_sig, decoded)
                instance_cache.int_type = None
                instance_cache.str_type = None
                instance_cache.bool_type = None
                instance_cache.object_type = None
                instance_cache.function_type = None
                if decoded is not None:
                    return decoded
        except (AssertionError, NotImplementedError):
            pass

    # Look up the member. First look up the method dictionary.
    if method and not isinstance(method, Decorator):
        if mx.is_super and not mx.suppress_errors:
            validate_super_call(method, mx)

        if method.is_property:
            assert isinstance(method, OverloadedFuncDef)
            getter = method.items[0]
            assert isinstance(getter, Decorator)
            if mx.is_lvalue and getter.var.is_settable_property:
                mx.chk.warn_deprecated(method.setter, mx.context)
            return analyze_var(name, getter.var, typ, mx)

        if mx.is_lvalue and not mx.suppress_errors:
            mx.msg.cant_assign_to_method(mx.context)
        if not isinstance(method, OverloadedFuncDef):
            signature = function_type(method, mx.named_type("builtins.function"))
        else:
            if method.type is None:
                # Overloads may be not ready if they are decorated. Handle this in same
                # manner as we would handle a regular decorated function: defer if possible.
                if not mx.no_deferral and method.items:
                    mx.not_ready_callback(method.name, mx.context)
                return AnyType(TypeOfAny.special_form)
            assert isinstance(method.type, Overloaded)
            signature = method.type
        if not mx.preserve_type_var_ids:
            signature = freshen_all_functions_type_vars(signature)
        # M20 native seam: for a static, non-overloaded method the whole
        # map -> expand -> freeze tail is replaceable by one Rust call.
        # Rust defers (None) for overloads, bound signatures, non-Instance

        # types, unresolvable derivation paths, and any expand result that
        # still carries type variables. Freshening already happened above,
        # and a fully successful Rust expand is already frozen, so

        # freeze_all_type_vars is a no-op on that path.
        if (
            _HAS_TYPE_KERNEL
            and _native_checkmember_active
            and _native_checkmember_resolver is not None
            and isinstance(method, FuncDef)
            and (method.is_static or method.is_trivial_self)
            and not mx.is_super
            and not mx.is_lvalue
            and _rust_analyze_instance_member_access is not None
        ):
            try:
                result = _rust_analyze_instance_member_access(
                    _native_checkmember_resolver,
                    _serialize_type_for_checkmember(typ),
                    _serialize_type_for_checkmember(signature),
                    method.info.fullname,
                    state.state.strict_optional,
                    method.is_trivial_self,
                )
                if result is not None:
                    decoded = _deserialize_type_for_checkmember(bytes(result))
                    # The wire format does not carry line/column; decoded
                    # types default to line -1. Preserve the input type's
                    # location so derived contexts report errors at the

                    # call site instead of a phantom line 0/-1.
                    if decoded is not None and isinstance(decoded, ProperType):
                        decoded.line = typ.line
                        decoded.column = typ.column
                        if isinstance(decoded, CallableType):
                            decoded.fallback.line = decoded.line
        # The wire round-trip drops `.name` and `.definition`; both drive
        # error-message formatting, so restore them from the pre-seam
        # signature for identical bound-method notes.
                        if isinstance(decoded, CallableType) and isinstance(signature, CallableType):
                            decoded.name = signature.name
                            decoded.definition = signature.definition
                    # Clear the process-global primitive decode singletons
                    # after a read so NOT_READY Instances cannot leak into
                    # later builds.
                    instance_cache.int_type = None
                    instance_cache.str_type = None
                    instance_cache.bool_type = None
                    instance_cache.object_type = None
                    instance_cache.function_type = None
                    if decoded is not None:
                        return decoded
            except (AssertionError, NotImplementedError):
                # AssertionError: TypeInfo not yet fixed during semanal.
                # NotImplementedError: unserializable variant.
                # Both defer to Python.
                pass
        if not method.is_static:
            # M20 follow-up: non-trivial plain FuncDef methods dispatched on
            # the exact defining class bind+expand in Rust; the remaining
            # cases defer to the pure-Python tail below.
            if (
                isinstance(method, FuncDef)
                and not method.is_trivial_self
                and not mx.is_super
                and not mx.is_lvalue
                and _HAS_TYPE_KERNEL
                and _native_checkmember_active
                and _native_checkmember_resolver is not None
                and _rust_analyze_member_method is not None
            ):
                try:
                    result = _rust_analyze_member_method(
                        _native_checkmember_resolver,
                        _serialize_type_for_checkmember(typ),
                        _serialize_type_for_checkmember(signature),
                        method.info.fullname,
                        _serialize_type_for_checkmember(mx.self_type),
                        name,
                        state.state.strict_optional,
                        method.is_class,
                    )
                    if result is not None:
                        decoded = _deserialize_type_for_checkmember(bytes(result))
                        if decoded is not None and isinstance(decoded, ProperType):
                            decoded.line = typ.line
                            decoded.column = typ.column
                            if isinstance(decoded, CallableType):
                                decoded.fallback.line = decoded.line
                                if isinstance(signature, CallableType):
                                    decoded.name = signature.name
                                    decoded.definition = signature.definition
                        instance_cache.int_type = None
                        instance_cache.str_type = None
                        instance_cache.bool_type = None
                        instance_cache.object_type = None
                        instance_cache.function_type = None
                        if decoded is not None:
                            return decoded
                except (AssertionError, NotImplementedError):
                    pass
            if isinstance(method, (FuncDef, OverloadedFuncDef)) and method.is_trivial_self:
                signature = bind_self_fast(signature, mx.self_type)
            else:
                signature = check_self_arg(
                    signature, mx.self_type, method.is_class, mx.context, name, mx.msg
                )
                signature = bind_self(signature, mx.self_type, is_classmethod=method.is_class)
        typ = map_instance_to_supertype(typ, method.info)
        member_type = expand_type_by_instance(signature, typ)
        freeze_all_type_vars(member_type)
        return member_type
    else:
        # Not a method.
        return analyze_member_var_access(name, typ, info, mx)


def validate_super_call(node: FuncBase, mx: MemberContext) -> None:
    unsafe_super = False
    if isinstance(node, FuncDef) and node.is_trivial_body:
        unsafe_super = True
    elif isinstance(node, OverloadedFuncDef):
        if node.impl:
            impl = node.impl if isinstance(node.impl, FuncDef) else node.impl.func
            unsafe_super = impl.is_trivial_body
        elif not node.is_property and node.items:
            assert isinstance(node.items[0], Decorator)
            unsafe_super = node.items[0].func.is_trivial_body
    if unsafe_super:
        mx.msg.unsafe_super(node.name, node.info.name, mx.context)


def analyze_type_callable_member_access(name: str, typ: FunctionLike, mx: MemberContext) -> Type:
    # Class attribute.
    # TODO super?
    instance_type = typ.items[0].get_instance_type(force_fallback=True)
    if isinstance(instance_type, Instance):
        if not mx.is_operator:
            # When Python sees an operator (eg `3 == 4`), it automatically translates that
            # into something like `int.__eq__(3, 4)` instead of `(3).__eq__(4)` as an
            # optimization.

            #
            # While it normally it doesn't matter which of the two versions are used, it
            # does cause inconsistencies when working with classes. For example, translating

            # `int == int` to `int.__eq__(int)` would not work since `int.__eq__` is meant to
            # compare two int _instances_. What we really want is `type(int).__eq__`, which
            # is meant to compare two types or classes.

            #
            # This check makes sure that when we encounter an operator, we skip looking up
            # the corresponding method in the current instance to avoid this edge case.

            # See https://github.com/python/mypy/pull/1787 for more info.
            # TODO: do not rely on same type variables being present in all constructor overloads.
            result = analyze_class_attribute_access(
                instance_type,
                name,
                mx,
                original_vars=typ.items[0].variables,
                mcs_fallback=typ.fallback,
            )
            if result:
                return result
        # Look up from the 'type' type.
        return _analyze_member_access(name, typ.fallback, mx)
    else:
        assert False, f"Unexpected type {instance_type!r}"


def analyze_type_type_member_access(
    name: str, typ: TypeType, mx: MemberContext, override_info: TypeInfo | None
) -> Type:
    # Similar to analyze_type_callable_attribute_access.
    item = None
    fallback = mx.named_type("builtins.type")
    # Issue-#957: classify the 9-way dispatch head in Rust
    # (checkmember.rs); terminal branches stay here. None falls
    # through to the pure-Python body below.
    tag = None
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _rust_classify_type_type_member_access is not None
    ):
        try:
            tag = _rust_classify_type_type_member_access(typ)
        except (AssertionError, NotImplementedError, ValueError, TypeError):
            tag = None
        if tag is not None:
            if tag == NATIVE_TT_ITEM_INSTANCE:
                item = typ.item
            elif tag == NATIVE_TT_ITEM_ANY:
                with mx.msg.filter_errors():
                    return _analyze_member_access(name, fallback, mx, override_info)
            elif tag == NATIVE_TT_TV_UB_INSTANCE:
                item = get_proper_type(typ.item.upper_bound)
            elif tag == NATIVE_TT_TV_UB_UNION:
                upper_bound = get_proper_type(typ.item.upper_bound)
                return _analyze_member_access(
                    name,
                    TypeType.make_normalized(
                        upper_bound, line=typ.line, column=typ.column
                    ),
                    mx,
                    override_info,
                )
            elif tag == NATIVE_TT_TV_UB_TUPLE:
                item = tuple_fallback(get_proper_type(typ.item.upper_bound))
            elif tag == NATIVE_TT_TV_UB_ANY:
                with mx.msg.filter_errors():
                    return _analyze_member_access(name, fallback, mx, override_info)
            elif tag == NATIVE_TT_ITEM_TUPLE:
                item = tuple_fallback(typ.item)
            elif tag == NATIVE_TT_ITEM_FUNC_TYPEOBJ:
                item = typ.item.fallback
            elif tag == NATIVE_TT_ITEM_TYPE_TYPE_INSTANCE:
                item = typ.item.item.type.metaclass_type
            # tags NONE / TV_UB_OTHER / FUNC_NOT_TYPEOBJ /
            # TYPE_TYPE_OTHER: item stays None, fall through to tail.
    if item is None and tag is None:
        if isinstance(typ.item, Instance):
            item = typ.item
        elif isinstance(typ.item, AnyType):
            with mx.msg.filter_errors():
                return _analyze_member_access(name, fallback, mx, override_info)
        elif isinstance(typ.item, TypeVarType):
            upper_bound = get_proper_type(typ.item.upper_bound)
            if isinstance(upper_bound, Instance):
                item = upper_bound
            elif isinstance(upper_bound, UnionType):
                return _analyze_member_access(
                    name,
                    TypeType.make_normalized(
                        upper_bound, line=typ.line, column=typ.column
                    ),
                    mx,
                    override_info,
                )
            elif isinstance(upper_bound, TupleType):
                item = tuple_fallback(upper_bound)
            elif isinstance(upper_bound, AnyType):
                with mx.msg.filter_errors():
                    return _analyze_member_access(name, fallback, mx, override_info)
        elif isinstance(typ.item, TupleType):
            item = tuple_fallback(typ.item)
        elif isinstance(typ.item, FunctionLike) and typ.item.is_type_obj():
            item = typ.item.fallback
        elif isinstance(typ.item, TypeType):
            # Access member on metaclass object via Type[Type[C]]
            if isinstance(typ.item.item, Instance):
                item = typ.item.item.type.metaclass_type
    ignore_messages = False

    if item is not None:
        fallback = item.type.metaclass_type or fallback

    if item and not mx.is_operator:
        # See comment above for why operators are skipped
        result = analyze_class_attribute_access(
            item, name, mx, mcs_fallback=fallback, override_info=override_info
        )
        if result:
            if not (isinstance(get_proper_type(result), AnyType) and item.type.fallback_to_any):
                return result
            else:
                # We don't want errors on metaclass lookup for classes with Any fallback
                ignore_messages = True

    with mx.msg.filter_errors(filter_errors=ignore_messages):
        return _analyze_member_access(name, fallback, mx, override_info)


def analyze_union_member_access(name: str, typ: UnionType, mx: MemberContext) -> Type:
    with mx.msg.disable_type_names():
        # M20: gate the union-map through Rust when the kernel is active.
        # Rust maps relevant_items and returns per-item results; this shim
        # joins via make_simplified_union. Defer: property / Var / lvalue.
        if (
            _HAS_TYPE_KERNEL
            and _native_checkmember_active
            and _native_checkmember_resolver is not None
            and _rust_analyze_union_member_access is not None
        ):
            try:
                result = _rust_analyze_union_member_access(
                    _native_checkmember_resolver,
                    _serialize_type_for_checkmember(typ),
                    name,
                    mx.is_lvalue,
                    mx.is_super,
                    mx.no_deferral,
                    mx.preserve_type_var_ids,
                    TypeVarId.next_raw_id,
                    state.state.strict_optional,
                )
                if result is not None:
                    next_raw_id, changed, per_item = result
                    if changed:
                        TypeVarId.next_raw_id = next_raw_id
                    relevant = typ.relevant_items()
                    if len(per_item) == len(relevant):
                        decoded_items = []
                        for subtype, item_bytes in zip(relevant, per_item):
                            decoded = _deserialize_type_for_checkmember(bytes(item_bytes))
                            if decoded is None:
                                break
                            if isinstance(subtype, Instance):
                                method = subtype.type.get_method(name)
                                if (
                                    method is not None
                                    and not isinstance(method, Decorator)
                                    and getattr(method, "type", None) is not None
                                ):
                                    decoded = _restore_definition(method.type, decoded)
                            decoded_items.append(decoded)
                        else:
                            for decoded in decoded_items:
                                if isinstance(decoded, ProperType):
                                    decoded.line = typ.line
                                    decoded.column = typ.column
                                    if isinstance(decoded, CallableType):
                                        decoded.fallback.line = decoded.line
                            return make_simplified_union(decoded_items)
            except (AssertionError, NotImplementedError, ValueError):
                pass
        results = []
        for subtype in typ.relevant_items():
            # Self types should be bound to every individual item of a union.
            item_mx = mx.copy_modified(self_type=subtype)
            results.append(_analyze_member_access(name, subtype, item_mx))
    return make_simplified_union(results)


def analyze_none_member_access(name: str, typ: NoneType, mx: MemberContext) -> Type:
    # M20: gate the NoneType branch through Rust. __bool__ returns a pure
    # CallableType (ret=Literal[False]); any other name recurses on
    # builtins.object. Defer (None) when the recursion defers.
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _native_checkmember_resolver is not None
        and _rust_analyze_none_member_access is not None
    ):
        try:
            result = _rust_analyze_none_member_access(
                _native_checkmember_resolver,
                name,
                _serialize_type_for_checkmember(typ),
                state.state.strict_optional,
            )
            if result is not None:
                decoded = _deserialize_type_for_checkmember(bytes(result))
                if decoded is not None:
                    if isinstance(decoded, ProperType):
                        decoded.line = typ.line
                        decoded.column = typ.column
                        if isinstance(decoded, CallableType):
                            decoded.fallback.line = decoded.line
                    return decoded
        except (AssertionError, NotImplementedError):
            pass
    if name == "__bool__":
        literal_false = LiteralType(False, fallback=mx.named_type("builtins.bool"))
        return CallableType(
            arg_types=[],
            arg_kinds=[],
            arg_names=[],
            ret_type=literal_false,
            fallback=mx.named_type("builtins.function"),
        )
    else:
        return _analyze_member_access(name, mx.named_type("builtins.object"), mx)


def analyze_member_var_access(
    name: str, itype: Instance, info: TypeInfo, mx: MemberContext
) -> Type:
    """Analyse attribute access that does not target a method.

    This is logically part of analyze_member_access and the arguments are similar.

    original_type is the type of E in the expression E.var
    """
    # It was not a method. Try looking up a variable.
    node = info.get(name)
    v = node.node if node else None

    mx.chk.warn_deprecated(v, mx.context)

    vv = v
    is_trivial_self = False
    if isinstance(vv, Decorator):
        # The associated Var node of a decorator contains the type.
        v = vv.var
        is_trivial_self = vv.func.is_trivial_self and not vv.decorators
        if mx.is_super and not mx.suppress_errors:
            validate_super_call(vv.func, mx)
    if isinstance(v, FuncDef):
        assert False, "Did not expect a function"
    if isinstance(v, MypyFile):
        # Special case: accessing module on instances is allowed, but will not
        # be recorded by semantic analyzer.
        mx.chk.module_refs.add(v.fullname)

    if isinstance(vv, (TypeInfo, TypeAlias, MypyFile, TypeVarLikeExpr)):
        # If the associated variable is a TypeInfo synthesize a Var node for
        # the purposes of type checking.  This enables us to type check things
        # like accessing class attributes on an inner class. Similar we allow

        # using qualified type aliases in runtime context. For example:
        #     class C:
        #         A = List[int]

        #     x = C.A() <- this is OK
        typ = mx.chk.expr_checker.analyze_static_reference(vv, mx.context, mx.is_lvalue)
        v = Var(name, type=typ)
        v.info = info

    if isinstance(v, Var):
        implicit = info[name].implicit

        # An assignment to final attribute is always an error,
        # independently of types.
        if mx.is_lvalue and not mx.chk.get_final_context():
            check_final_member(name, info, mx.msg, mx.context)

        return analyze_var(name, v, itype, mx, implicit=implicit, is_trivial_self=is_trivial_self)
    elif (
        not v
        and name not in ["__getattr__", "__setattr__", "__getattribute__"]
        and not mx.is_operator
        and mx.module_symbol_table is None
    ):
        # Above we skip ModuleType.__getattr__ etc. if we have a
        # module symbol table, since the symbol table allows precise
        # checking.
        if not mx.is_lvalue:
            for method_name in ("__getattribute__", "__getattr__"):
                method = info.get_method(method_name)

                # __getattribute__ is defined on builtins.object and returns Any, so without
                # the guard this search will always find object.__getattribute__ and conclude
                # that the attribute exists
                if method and method.info.fullname != "builtins.object":
                    bound_method = analyze_decorator_or_funcbase_access(
                        defn=method, itype=itype, name=method_name, mx=mx
                    )
                    typ = map_instance_to_supertype(itype, method.info)
                    getattr_type = get_proper_type(expand_type_by_instance(bound_method, typ))
                    if isinstance(getattr_type, CallableType):
                        result = getattr_type.ret_type
                    else:
                        result = getattr_type

                    # Call the attribute hook before returning.
                    fullname = f"{method.info.fullname}.{name}"
                    # Stage 4/C3: skip the Python attribute-hook chain when
                    # the registry proves no DefaultPlugin hook matches.
                    from mypy.checkexpr import plugin_hook_known_absent

                    if not plugin_hook_known_absent("get_attribute_hook", fullname):
                        hook = mx.chk.plugin.get_attribute_hook(fullname)
                    else:
                        hook = None
                    if hook:
                        result = hook(
                            AttributeContext(
                                get_proper_type(mx.original_type),
                                result,
                                mx.is_lvalue,
                                mx.context,
                                mx.chk,
                            )
                        )
                    return result
        else:
            setattr_meth = info.get_method("__setattr__")
            if setattr_meth and setattr_meth.info.fullname != "builtins.object":
                bound_type = analyze_decorator_or_funcbase_access(
                    defn=setattr_meth,
                    itype=itype,
                    name="__setattr__",
                    mx=mx.copy_modified(is_lvalue=False),
                )
                typ = map_instance_to_supertype(itype, setattr_meth.info)
                setattr_type = get_proper_type(expand_type_by_instance(bound_type, typ))
                if isinstance(setattr_type, CallableType) and len(setattr_type.arg_types) > 0:
                    return setattr_type.arg_types[-1]

    if itype.type.fallback_to_any:
        return AnyType(TypeOfAny.special_form)

    # Could not find the member.
    if itype.extra_attrs and name in itype.extra_attrs.attrs:
        # For modules use direct symbol table lookup.
        if not itype.extra_attrs.mod_name:
            return itype.extra_attrs.attrs[name]

    if mx.is_super and not mx.suppress_errors:
        mx.msg.undefined_in_superclass(name, mx.context)
        return AnyType(TypeOfAny.from_error)
    else:
        ret = report_missing_attribute(mx.original_type, itype, name, mx)
        # Avoid paying double jeopardy if we can't find the member due to --no-implicit-reexport
        if (
            mx.module_symbol_table is not None
            and name in mx.module_symbol_table
            and not mx.module_symbol_table[name].module_public
        ):
            v = mx.module_symbol_table[name].node
            e = NameExpr(name)
            e.set_line(mx.context)
            e.node = v
            return mx.chk.expr_checker.analyze_ref_expr(e, lvalue=mx.is_lvalue)
        return ret


def check_final_member(name: str, info: TypeInfo, msg: MessageBuilder, ctx: Context) -> None:
    """Give an error if the name being assigned was declared as final."""
    for base in info.mro:
        sym = base.names.get(name)
        if sym and is_final_node(sym.node):
            msg.cant_assign_to_final(name, attr_assign=True, ctx=ctx)


def analyze_descriptor_access(descriptor_type: Type, mx: MemberContext) -> Type:
    """Type check descriptor access.

    Arguments:
        descriptor_type: The type of the descriptor attribute being accessed
            (the type of ``f`` in ``a.f`` when ``f`` is a descriptor).
        mx: The current member access context.
    Return:
        The return type of the appropriate ``__get__/__set__`` overload for the descriptor.
    """
    instance_type = get_proper_type(mx.self_type)
    orig_descriptor_type = descriptor_type
    descriptor_type = get_proper_type(descriptor_type)

    if isinstance(descriptor_type, UnionType) or not isinstance(descriptor_type, Instance):
        # M20: gated descriptor head. Rust maps a UnionType item-wise
        # and joins, and passes a non-lvalue, no-__get__ descriptor
        # (Instance/TupleType) through; a __get__-bearing one defers.
        if (
            _HAS_TYPE_KERNEL
            and _native_checkmember_active
            and _native_checkmember_resolver is not None
            and _rust_analyze_descriptor_access is not None
        ):
            try:
                result = _rust_analyze_descriptor_access(
                    _native_checkmember_resolver,
                    _serialize_type_for_checkmember(descriptor_type),
                    mx.is_lvalue,
                    state.state.strict_optional,
                )
                if result is not None:
                    decoded = _deserialize_type_for_checkmember(bytes(result))
                    if decoded is not None:
                        if isinstance(decoded, ProperType):
                            decoded.line = descriptor_type.line
                            decoded.column = descriptor_type.column
                        return decoded
            except (AssertionError, NotImplementedError):
                pass
    if isinstance(descriptor_type, UnionType):
        # Map the access over union types
        return make_simplified_union(
            [analyze_descriptor_access(typ, mx) for typ in descriptor_type.items]
        )
    elif not isinstance(descriptor_type, Instance):
        return orig_descriptor_type

    if not mx.is_lvalue:
        # M20: gate the __get__ presence check through Rust. Rust reads
        # member presence from the resolver snapshots; defer (None) when
        # the class snapshot is missing.
        rust_decided = False
        if (
            _HAS_TYPE_KERNEL
            and _native_checkmember_active
            and _native_checkmember_resolver is not None
            and _rust_descriptor_has_get_set is not None
        ):
            try:
                result = _rust_descriptor_has_get_set(
                    _native_checkmember_resolver,
                    _serialize_type_for_checkmember(descriptor_type),
                )
                if result is not None:
                    rust_decided = True
                    has_get, _has_set = result
                    if not has_get:
                        return orig_descriptor_type
            except (AssertionError, NotImplementedError):
                pass
        if not rust_decided and not descriptor_type.type.has_readable_member("__get__"):
            return orig_descriptor_type

    # We do this check first to accommodate for descriptors with only __set__ method.
    # If there is no __set__, we type-check that the assigned value matches
    # the return type of __get__. This doesn't match the python semantics,

    # (which allow you to override the descriptor with any value), but preserves
    # the type of accessing the attribute (even after the override).
    if mx.is_lvalue and descriptor_type.type.has_readable_member("__set__"):
        return analyze_descriptor_assign(descriptor_type, mx)

    if mx.is_lvalue and not descriptor_type.type.has_readable_member("__get__"):
        # This turned out to be not a descriptor after all.
        return orig_descriptor_type

    dunder_get = descriptor_type.type.get_method("__get__")
    if dunder_get is None:
        mx.fail(
            message_registry.DESCRIPTOR_GET_NOT_CALLABLE.format(
                descriptor_type.str_with_options(mx.msg.options)
            )
        )
        return AnyType(TypeOfAny.from_error)

    bound_method = analyze_decorator_or_funcbase_access(
        defn=dunder_get,
        itype=descriptor_type,
        name="__get__",
        mx=mx.copy_modified(self_type=descriptor_type),
    )

    typ = map_instance_to_supertype(descriptor_type, dunder_get.info)
    dunder_get_type = expand_type_by_instance(bound_method, typ)

    if isinstance(instance_type, FunctionLike) and instance_type.is_type_obj():
        owner_type = instance_type.items[0].get_instance_type()
        instance_type = NoneType()
    elif isinstance(instance_type, TypeType):
        owner_type = instance_type.item
        instance_type = NoneType()
    else:
        owner_type = instance_type

    callable_name = mx.chk.expr_checker.method_fullname(descriptor_type, "__get__")
    dunder_get_type = mx.chk.expr_checker.transform_callee_type(
        callable_name,
        dunder_get_type,
        [
            TempNode(instance_type, context=mx.context),
            TempNode(TypeType.make_normalized(owner_type), context=mx.context),
        ],
        [ARG_POS, ARG_POS],
        mx.context,
        object_type=descriptor_type,
    )

    _, inferred_dunder_get_type = mx.chk.expr_checker.check_call(
        dunder_get_type,
        [
            TempNode(instance_type, context=mx.context),
            TempNode(TypeType.make_normalized(owner_type), context=mx.context),
        ],
        [ARG_POS, ARG_POS],
        mx.context,
        object_type=descriptor_type,
        callable_name=callable_name,
    )

    # Search for possible deprecations:
    mx.chk.warn_deprecated(dunder_get, mx.context)

    inferred_dunder_get_type = get_proper_type(inferred_dunder_get_type)
    if isinstance(inferred_dunder_get_type, AnyType):
        # check_call failed, and will have reported an error
        return inferred_dunder_get_type

    if not isinstance(inferred_dunder_get_type, CallableType):
        mx.fail(
            message_registry.DESCRIPTOR_GET_NOT_CALLABLE.format(
                descriptor_type.str_with_options(mx.msg.options)
            )
        )
        return AnyType(TypeOfAny.from_error)

    return inferred_dunder_get_type.ret_type


def analyze_descriptor_assign(descriptor_type: Instance, mx: MemberContext) -> Type:
    instance_type = get_proper_type(mx.self_type)
    dunder_set = descriptor_type.type.get_method("__set__")
    if dunder_set is None:
        mx.fail(
            message_registry.DESCRIPTOR_SET_NOT_CALLABLE.format(
                descriptor_type.str_with_options(mx.msg.options)
            ).value
        )
        return AnyType(TypeOfAny.from_error)

    bound_method = analyze_decorator_or_funcbase_access(
        defn=dunder_set,
        itype=descriptor_type,
        name="__set__",
        mx=mx.copy_modified(is_lvalue=False, self_type=descriptor_type),
    )
    typ = map_instance_to_supertype(descriptor_type, dunder_set.info)
    dunder_set_type = expand_type_by_instance(bound_method, typ)

    callable_name = mx.chk.expr_checker.method_fullname(descriptor_type, "__set__")
    rvalue = mx.rvalue or TempNode(AnyType(TypeOfAny.special_form), context=mx.context)
    dunder_set_type = mx.chk.expr_checker.transform_callee_type(
        callable_name,
        dunder_set_type,
        [TempNode(instance_type, context=mx.context), rvalue],
        [ARG_POS, ARG_POS],
        mx.context,
        object_type=descriptor_type,
    )

    # For non-overloaded setters, type-check like a regular assignment.
    # We first infer the type by using the rvalue as type context.
    type_context = rvalue
    with mx.msg.filter_errors():
        _, inferred_dunder_set_type = mx.chk.expr_checker.check_call(
            dunder_set_type,
            [TempNode(instance_type, context=mx.context), type_context],
            [ARG_POS, ARG_POS],
            mx.context,
            object_type=descriptor_type,
            callable_name=callable_name,
        )

    # And now we in fact type check the call, to show errors related to wrong arguments
    # count, etc., replacing the type context for non-overloaded setters only.
    inferred_dunder_set_type = get_proper_type(inferred_dunder_set_type)
    if isinstance(inferred_dunder_set_type, CallableType):
        type_context = TempNode(AnyType(TypeOfAny.special_form), context=mx.context)
    mx.chk.expr_checker.check_call(
        dunder_set_type,
        [TempNode(instance_type, context=mx.context), type_context],
        [ARG_POS, ARG_POS],
        mx.context,
        object_type=descriptor_type,
        callable_name=callable_name,
    )

    # Search for possible deprecations:
    mx.chk.warn_deprecated(dunder_set, mx.context)

    # In the following cases, a message already will have been recorded in check_call.
    if (not isinstance(inferred_dunder_set_type, CallableType)) or (
        len(inferred_dunder_set_type.arg_types) < 2
    ):
        return AnyType(TypeOfAny.from_error)
    return inferred_dunder_set_type.arg_types[1]


def is_instance_var(var: Var) -> bool:
    """Return if var is an instance variable according to PEP 526."""
    if _HAS_TYPE_KERNEL and _native_checkmember_active and _rust_is_instance_var is not None:
        result = _rust_is_instance_var(var)
        if result is not None:
            return result
    return (
        # check the type_info node is the var (not a decorated function, etc.)
        var.name in var.info.names
        and var.info.names[var.name].node is var
        and not var.is_classvar
        # variables without annotations are treated as classvar
        and not var.is_inferred
    )


def analyze_var(
    name: str,
    var: Var,
    itype: Instance,
    mx: MemberContext,
    *,
    implicit: bool = False,
    is_trivial_self: bool = False,
) -> Type:
    """Analyze access to an attribute via a Var node.

    This is conceptually part of analyze_member_access and the arguments are similar.
    itype is the instance type in which attribute should be looked up
    original_type is the type of E in the expression E.var
    if implicit is True, the original Var was created as an assignment to self
    if is_trivial_self is True, we can use fast path for bind_self().
    """
    # Found a member variable.
    original_itype = itype
    itype = map_instance_to_supertype(itype, var.info)
    if var.is_settable_property and mx.is_lvalue:
        typ: Type | None = var.setter_type
        if typ is None and var.is_ready:
            # Existing synthetic properties may not set setter type. Fall back to getter.
            typ = var.type
    else:
        typ = var.type
    if typ:
        if isinstance(typ, PartialType):
            return mx.chk.handle_partial_var_type(typ, mx.is_lvalue, var, mx.context)
        if mx.is_lvalue and not mx.suppress_errors:
            if var.is_property and not var.is_settable_property:
                mx.msg.read_only_property(name, itype.type, mx.context)
            if var.is_classvar:
                mx.msg.cant_assign_to_classvar(name, mx.context)
        # This is the most common case for variables, so start with this.
        result = expand_without_binding(typ, var, itype, original_itype, mx)

        # A non-None value indicates that we should actually bind self for this variable.
        call_type: ProperType | None = None
        if var.is_initialized_in_class and (not is_instance_var(var) or mx.is_operator):
            typ = get_proper_type(typ)
            if isinstance(typ, FunctionLike) and not typ.is_type_obj():
                call_type = typ
            elif var.is_property:
                deco_mx = mx.copy_modified(original_type=typ, self_type=typ, is_lvalue=False)
                call_type = get_proper_type(_analyze_member_access("__call__", typ, deco_mx))
            else:
                call_type = typ

        # Bound variables with callable types are treated like methods
        # (these are usually method aliases like __rmul__ = __mul__).
        if isinstance(call_type, FunctionLike) and not call_type.is_type_obj():
            if mx.is_lvalue and not var.is_property and not mx.suppress_errors:
                mx.msg.cant_assign_to_method(mx.context)

        # Bind the self type for each callable component (when needed).
        if call_type and not var.is_staticmethod:
            bound_items = []
            for ct in call_type.items if isinstance(call_type, UnionType) else [call_type]:
                p_ct = get_proper_type(ct)
                if isinstance(p_ct, FunctionLike) and (not p_ct.bound() or var.is_property):
                    item = expand_and_bind_callable(p_ct, var, itype, name, mx, is_trivial_self)
                else:
                    item = expand_without_binding(ct, var, itype, original_itype, mx)
                bound_items.append(item)
            result = UnionType.make_union(bound_items)
    else:
        if not var.is_ready and not mx.no_deferral:
            mx.not_ready_callback(var.name, mx.context)
        # Implicit 'Any' type.
        result = AnyType(TypeOfAny.special_form)
    fullname = f"{var.info.fullname}.{name}"
    # Stage 4/C3: skip the Python attribute-hook chain when the registry
    # proves no DefaultPlugin hook matches (per-attribute-access hot path).
    from mypy.checkexpr import plugin_hook_known_absent

    if not plugin_hook_known_absent("get_attribute_hook", fullname):
        hook = mx.chk.plugin.get_attribute_hook(fullname)
    else:
        hook = None

    if var.info.is_enum and not mx.is_lvalue:
        if name in var.info.enum_members and name not in {"name", "value"}:
            enum_literal = LiteralType(name, fallback=itype)
            result = itype.copy_modified(last_known_value=enum_literal)
        elif (
            isinstance(p_result := get_proper_type(result), Instance)
            and p_result.type.fullname == "enum.nonmember"
            and p_result.args
        ):
            # Unwrap nonmember similar to class-level access
            result = p_result.args[0]
    if result and not (implicit or var.info.is_protocol and is_instance_var(var)):
        result = analyze_descriptor_access(result, mx)
    if hook:
        result = hook(
            AttributeContext(
                get_proper_type(mx.original_type), result, mx.is_lvalue, mx.context, mx.chk
            )
        )
    return result


def expand_without_binding(
    typ: Type, var: Var, itype: Instance, original_itype: Instance, mx: MemberContext
) -> Type:
    # M20: gate the pure expand path through Rust. Rust handles the case
    # where preserve_type_var_ids is False and var.info.self_type is None
    # (expand_self_type returns typ unchanged). Defer (None) otherwise.
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _native_checkmember_resolver is not None
        and _rust_expand_without_binding is not None
        and not mx.is_self
        and not mx.is_super
    ):
        try:
            has_self_type = var.info.self_type is not None and not var.is_property
            if not has_self_type:
                result = _rust_expand_without_binding(
                    _serialize_type_for_checkmember(typ),
                    _serialize_type_for_checkmember(itype),
                    mx.preserve_type_var_ids,
                    False,  # has_self_type
                    TypeVarId.next_raw_id,
                    state.state.strict_optional,
                    _native_checkmember_resolver,
                )
                if result is not None:
                    next_raw_id, changed, wire_bytes = result
                    if changed:
                        TypeVarId.next_raw_id = next_raw_id
                    decoded = _deserialize_type_for_checkmember(bytes(wire_bytes), freeze=True)
                    if decoded is not None:
                        freeze_all_type_vars(decoded)
                        return _restore_definition(typ, decoded)
        except (AssertionError, NotImplementedError):
            pass
    if not mx.preserve_type_var_ids:
        typ = freshen_all_functions_type_vars(typ)
    typ = expand_self_type_if_needed(typ, mx, var, original_itype)
    expanded = expand_type_by_instance(typ, itype)
    freeze_all_type_vars(expanded)
    return expanded


def expand_and_bind_callable(
    functype: FunctionLike,
    var: Var,
    itype: Instance,
    name: str,
    mx: MemberContext,
    is_trivial_self: bool,
) -> Type:
    # M20: gate the trivial_self path through Rust. Rust handles
    # is_trivial_self=True + not is_property + no self_type + not is_self/super.
    # Defer (None) for non-trivial paths (check_self_arg + bind_self) and

    # property extraction.
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _native_checkmember_resolver is not None
        and is_trivial_self
        and not var.is_property
        and not mx.is_self
        and not mx.is_super
        and _rust_expand_and_bind_callable is not None
    ):
        try:
            has_self_type = var.info.self_type is not None
            if not has_self_type:
                result = _rust_expand_and_bind_callable(
                    _serialize_type_for_checkmember(functype),
                    _serialize_type_for_checkmember(itype),
                    is_trivial_self,
                    var.is_property,
                    mx.preserve_type_var_ids,
                    TypeVarId.next_raw_id,
                    state.state.strict_optional,
                    _native_checkmember_resolver,
                )
                if result is not None:
                    next_raw_id, changed, wire_bytes = result
                    if changed:
                        TypeVarId.next_raw_id = next_raw_id
                    decoded = _deserialize_type_for_checkmember(bytes(wire_bytes), freeze=True)
                    if decoded is not None:
                        freeze_all_type_vars(decoded)
                        return _restore_definition(functype, decoded)
        except (AssertionError, NotImplementedError):
            pass
    if not mx.preserve_type_var_ids:
        functype = freshen_all_functions_type_vars(functype)
    typ = get_proper_type(expand_self_type(var, functype, mx.self_type))
    assert isinstance(typ, FunctionLike)
    if is_trivial_self:
        typ = bind_self_fast(typ, mx.self_type)
    else:
        typ = check_self_arg(typ, mx.self_type, var.is_classmethod, mx.context, name, mx.msg)
        typ = bind_self(typ, mx.self_type, var.is_classmethod)
    expanded = expand_type_by_instance(typ, itype)
    freeze_all_type_vars(expanded)
    if not var.is_property:
        return expanded
    if isinstance(expanded, Overloaded):
        # Legacy way to store settable properties is with overloads. Also in case it is
        # an actual overloaded property, selecting first item that passed check_self_arg()
        # is a good approximation, long-term we should use check_call() inference below.
        if not expanded.items:
            # A broken overload, error should be already reported.
            return AnyType(TypeOfAny.from_error)
        expanded = expanded.items[0]
    assert isinstance(expanded, CallableType), expanded
    if var.is_settable_property and mx.is_lvalue and var.setter_type is not None:
        if expanded.variables:
            type_ctx = mx.rvalue or TempNode(AnyType(TypeOfAny.special_form), context=mx.context)
            _, inferred_expanded = mx.chk.expr_checker.check_call(
                expanded, [type_ctx], [ARG_POS], mx.context
            )
            expanded = get_proper_type(inferred_expanded)
            assert isinstance(expanded, CallableType)
        if not expanded.arg_types:
            # This can happen when accessing invalid property from its own body,
            # error will be reported elsewhere.
            return AnyType(TypeOfAny.from_error)
        return expanded.arg_types[0]
    else:
        return expanded.ret_type


def expand_self_type_if_needed(
    t: Type, mx: MemberContext, var: Var, itype: Instance, is_class: bool = False
) -> Type:
    """Expand special Self type in a backwards compatible manner.

    This should ensure that mixing old-style and new-style self-types work
    seamlessly. Also, re-bind new style self-types in subclasses if needed.
    """
    original = get_proper_type(mx.self_type)
    if not (mx.is_self or mx.is_super):
        repl = mx.self_type
        if is_class:
            if isinstance(original, TypeType):
                repl = original.item
            elif isinstance(original, CallableType):
                # Problematic access errors should have been already reported.
                repl = erase_typevars(original.ret_type)
            else:
                repl = itype
        return expand_self_type(var, t, repl)
    elif supported_self_type(
        # Support compatibility with plain old style T -> T and Type[T] -> T only.
        get_proper_type(mx.self_type),
        allow_instances=False,
        allow_callable=False,
    ):
        repl = mx.self_type
        if is_class and isinstance(original, TypeType):
            repl = original.item
        return expand_self_type(var, t, repl)
    elif (
        mx.is_self
        and itype.type != var.info
        # If an attribute with Self-type was defined in a supertype, we need to
        # rebind the Self type variable to Self type variable of current class...
        and itype.type.self_type is not None
        # ...unless `self` has an explicit non-trivial annotation.
        and itype == mx.chk.scope.active_self_type()
    ):
        return expand_self_type(var, t, itype.type.self_type)
    else:
        return t


def check_self_arg(
    functype: FunctionLike,
    dispatched_arg_type: Type,
    is_classmethod: bool,
    context: Context,
    name: str,
    msg: MessageBuilder,
) -> FunctionLike:
    """Check that an instance has a valid type for a method with annotated 'self'.

    For example if the method is defined as:
        class A:
            def f(self: S) -> T: ...
    then for 'x.f' we check that type(x) <: S. If the method is overloaded, we select
    only overloads items that satisfy this requirement. If there are no matching
    overloads, an error is generated.
    """
    items = functype.items
    if not items:
        return functype
    # M20: gate the overload filtering through Rust. Rust mirrors the
    # two-pass filter (Instance overlap special-case + is_subtype check)
    # and defers (None) for any case it cannot decide or that needs error

    # reporting (no_formal_self, incompatible_self_argument).
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _native_checkmember_resolver is not None
        and _rust_check_self_arg is not None
    ):
        try:
            result = _rust_check_self_arg(
                _native_checkmember_resolver,
                _serialize_type_for_checkmember(functype),
                _serialize_type_for_checkmember(dispatched_arg_type),
                is_classmethod,
                name,
                state.state.strict_optional,
            )
            if result is not None:
                next_raw_id, changed, wire_bytes = result
                if changed:
                    from mypy.types import TypeVarId
                    TypeVarId.next_raw_id = next_raw_id
                decoded = _deserialize_type_for_checkmember(bytes(wire_bytes), freeze=True)
                if decoded is not None:
                    if isinstance(decoded, ProperType):
                        decoded.line = context.line
                        decoded.column = context.column
                    freeze_all_type_vars(decoded)
                    return _restore_definition(functype, decoded)  # type: ignore[return-value]
        except (AssertionError, NotImplementedError):
            pass
    new_items = []
    if is_classmethod:
        dispatched_arg_type = TypeType.make_normalized(dispatched_arg_type)
    p_dispatched_arg_type = get_proper_type(dispatched_arg_type)

    for item in items:
        if not item.arg_types or item.arg_kinds[0] not in (ARG_POS, ARG_STAR):
            # No positional first (self) argument (*args is okay).
            msg.no_formal_self(name, item, context)
            # This is pretty bad, so just return the original signature if
            # there is at least one such error.
            return functype
        selfarg = get_proper_type(item.arg_types[0])
        if isinstance(selfarg, Instance) and isinstance(p_dispatched_arg_type, Instance):
            if selfarg.type is p_dispatched_arg_type.type and selfarg.args:
                if not is_overlapping_types(p_dispatched_arg_type, selfarg):
                    # This special casing is needed since `actual <: erased(template)`
                    # logic below doesn't always work, and a more correct approach may
                    # be tricky.
                    continue
        new_items.append(item)

    if new_items:
        items = new_items
        new_items = []

    for item in items:
        selfarg = get_proper_type(item.arg_types[0])
        # This matches similar special-casing in bind_self(), see more details there.
        self_callable = name == "__call__" and isinstance(selfarg, CallableType)
        if self_callable or is_subtype(
            dispatched_arg_type,
            # This level of erasure matches the one in checker.check_func_def(),
            # better keep these two checks consistent.
            erase_typevars(erase_to_bound(selfarg)),
            # This is to work around the fact that erased ParamSpec and TypeVarTuple
            # callables are not always compatible with non-erased ones both ways.
            always_covariant=any(
                not isinstance(tv, TypeVarType) for tv in get_all_type_vars(selfarg)
            ),
            ignore_pos_arg_names=True,
        ):
            new_items.append(item)
        elif isinstance(selfarg, ParamSpecType):
            # TODO: This is not always right. What's the most reasonable thing to do here?
            new_items.append(item)
        elif isinstance(selfarg, TypeVarTupleType):
            raise NotImplementedError
    if not new_items:
        # Choose first item for the message (it may be not very helpful for overloads).
        msg.incompatible_self_argument(
            name, dispatched_arg_type, items[0], is_classmethod, context
        )
        return functype
    if len(new_items) == 1:
        return new_items[0]
    return Overloaded(new_items)


def analyze_class_attribute_access(
    itype: Instance,
    name: str,
    mx: MemberContext,
    *,
    mcs_fallback: Instance,
    override_info: TypeInfo | None = None,
    original_vars: Sequence[TypeVarLikeType] | None = None,
) -> Type | None:
    """Analyze access to an attribute on a class object.

    itype is the return type of the class object callable, original_type is the type
    of E in the expression E.var, original_vars are type variables of the class callable
    (for generic classes).
    """
    info = itype.type
    if override_info:
        info = override_info

    fullname = f"{info.fullname}.{name}"
    hook = mx.chk.plugin.get_class_attribute_hook(fullname)

    node = info.get(name)
    if not node:
        if itype.extra_attrs and name in itype.extra_attrs.attrs:
            # For modules use direct symbol table lookup.
            if not itype.extra_attrs.mod_name:
                return itype.extra_attrs.attrs[name]
        if info.fallback_to_any or info.meta_fallback_to_any:
            return apply_class_attr_hook(mx, hook, AnyType(TypeOfAny.special_form))
        return None

    if (
        isinstance(node.node, Var)
        and not node.node.is_classvar
        and not hook
        and mcs_fallback.type.get(name)
    ):
        # If the same attribute is declared on the metaclass and the class but with
        # different types,
        # and the attribute on the class is not a ClassVar,

        # the type of the attribute on the metaclass should take priority

        # over the type of the attribute on the class,
        # when the attribute is being accessed from the class object itself.
        #

        # Return `None` here to signify that the name should be looked up
        # on the class object itself rather than the instance.
        return None

    mx.chk.warn_deprecated(node.node, mx.context)

    is_decorated = isinstance(node.node, Decorator)
    is_method = is_decorated or isinstance(node.node, FuncBase)
    if mx.is_lvalue and not mx.suppress_errors:
        if is_method:
            mx.msg.cant_assign_to_method(mx.context)
        if isinstance(node.node, TypeInfo):
            mx.fail(message_registry.CANNOT_ASSIGN_TO_TYPE)

    # Refuse class attribute access if slot defined
    if info.slots and name in info.slots:
        mx.fail(message_registry.CLASS_VAR_CONFLICTS_SLOTS.format(name))

    if node.implicit and isinstance(node.node, Var):
        if node.node.is_final:
            # If a final attribute was declared on `self` in `__init__`, then it
            # can't be accessed on the class object.
            mx.fail(message_registry.CANNOT_ACCESS_FINAL_INSTANCE_ATTR.format(node.node.name))
        elif not mx.is_lvalue and not defined_in_superclass(info, name):
            mx.fail(message_registry.CANNOT_ACCESS_INSTANCE_ONLY_ATTR.format(node.node.name))

    # An assignment to final attribute on class object is also always an error,
    # independently of types.
    if mx.is_lvalue and not mx.chk.get_final_context():
        check_final_member(name, info, mx.msg, mx.context)

    if info.is_enum and not (mx.is_lvalue or is_decorated or is_method):
        enum_class_attribute_type = analyze_enum_class_attribute_access(itype, name, mx)
        if enum_class_attribute_type:
            return apply_class_attr_hook(mx, hook, enum_class_attribute_type)

    t = node.type
    if t:
        if isinstance(t, PartialType):
            symnode = node.node
            assert isinstance(symnode, Var)
            return apply_class_attr_hook(
                mx, hook, mx.chk.handle_partial_var_type(t, mx.is_lvalue, symnode, mx.context)
            )

        # Find the class where method/variable was defined.
        if isinstance(node.node, Decorator):
            super_info: TypeInfo | None = node.node.var.info
        elif isinstance(node.node, (Var, SYMBOL_FUNCBASE_TYPES)):
            super_info = node.node.info
        else:
            super_info = None

        # Map the type to how it would look as a defining class. For example:
        #     class C(Generic[T]): ...
        #     class D(C[Tuple[T, S]]): ...

        #     D[int, str].method()
        # Here itype is D[int, str], isuper is C[Tuple[int, str]].
        if not super_info:
            isuper = None
        else:
            isuper = map_instance_to_supertype(itype, super_info)

        if isinstance(node.node, Var):
            assert isuper is not None
            object_type = get_proper_type(mx.self_type)
            # Check if original variable type has type variables. For example:
            #     class C(Generic[T]):
            #         x: T

            #     C.x  # Error, ambiguous access
            #     C[int].x  # Also an error, since C[int] is same as C at runtime
            # Exception is Self type wrapped in ClassVar, that is safe.
            prohibit_self = not node.node.is_classvar
            def_vars = set(node.node.info.defn.type_vars)
            if prohibit_self and node.node.info.self_type:
                def_vars.add(node.node.info.self_type)
            # Exception: access on Type[...], including first argument of class methods is OK.
            prohibit_generic = not isinstance(object_type, TypeType) or node.implicit
            if prohibit_generic and def_vars & set(get_all_type_vars(t)):
                if node.node.is_classvar:
                    message = message_registry.GENERIC_CLASS_VAR_ACCESS
                else:
                    message = message_registry.GENERIC_INSTANCE_VAR_CLASS_ACCESS
                mx.fail(message)
            t = expand_self_type_if_needed(t, mx, node.node, itype, is_class=True)
            t = expand_type_by_instance(t, isuper)
            # Erase non-mapped variables, but keep mapped ones, even if there is an error.
            # In the above example this means that we infer following types:
            #     C.x -> Any

            #     C[int].x -> int
            if prohibit_generic:
                erase_vars = set(itype.type.defn.type_vars)
                if prohibit_self and itype.type.self_type:
                    erase_vars.add(itype.type.self_type)
                t = erase_typevars(t, {tv.id for tv in erase_vars})

        is_classmethod = (
            (is_decorated and cast(Decorator, node.node).func.is_class)
            or (isinstance(node.node, SYMBOL_FUNCBASE_TYPES) and node.node.is_class)
            or isinstance(node.node, Var)
            and node.node.is_classmethod
        )
        t = get_proper_type(t)
        is_trivial_self = False
        if isinstance(node.node, Decorator):
            # Use fast path if there are trivial decorators like @classmethod or @property
            is_trivial_self = node.node.func.is_trivial_self and not node.node.decorators
        elif isinstance(node.node, (FuncDef, OverloadedFuncDef)):
            is_trivial_self = node.node.is_trivial_self
        if (
            isinstance(t, FunctionLike)
            and is_classmethod
            and not is_trivial_self
            and not t.bound()
        ):
            t = check_self_arg(t, mx.self_type, False, mx.context, name, mx.msg)
        t = add_class_tvars(
            t,
            isuper,
            is_classmethod,
            mx,
            original_vars=original_vars,
            is_trivial_self=is_trivial_self,
        )
        if is_decorated:
            t = expand_self_type_if_needed(
                t, mx, cast(Decorator, node.node).var, itype, is_class=is_classmethod
            )

        result = t
        # __set__ is not called on class objects.
        if not mx.is_lvalue:
            result = analyze_descriptor_access(result, mx)

        return apply_class_attr_hook(mx, hook, result)
    elif isinstance(node.node, Var):
        mx.not_ready_callback(name, mx.context)
        return AnyType(TypeOfAny.special_form)

    if isinstance(node.node, (TypeInfo, TypeAlias, MypyFile, TypeVarLikeExpr)):
        # TODO: should we apply class plugin here (similar to instance access)?
        return mx.chk.expr_checker.analyze_static_reference(node.node, mx.context, mx.is_lvalue)

    if is_decorated:
        assert isinstance(node.node, Decorator)
        if node.node.type:
            return apply_class_attr_hook(mx, hook, node.node.type)
        else:
            mx.not_ready_callback(name, mx.context)
            return AnyType(TypeOfAny.from_error)
    else:
        assert isinstance(node.node, SYMBOL_FUNCBASE_TYPES)
        typ = function_type(node.node, mx.named_type("builtins.function"))
        # Note: if we are accessing class method on class object, the cls argument is bound.
        # Annotated and/or explicit class methods go through other code paths above, for
        # unannotated implicit class methods we do this here.
        if node.node.is_class:
            typ = bind_self_fast(typ)
        return apply_class_attr_hook(mx, hook, typ)


def apply_class_attr_hook(
    mx: MemberContext, hook: Callable[[AttributeContext], Type] | None, result: Type
) -> Type | None:
    if hook:
        result = hook(
            AttributeContext(
                get_proper_type(mx.original_type), result, mx.is_lvalue, mx.context, mx.chk
            )
        )
    return result


def analyze_enum_class_attribute_access(
    itype: Instance, name: str, mx: MemberContext
) -> Type | None:
    # Skip these since Enum will remove it
    if name in EXCLUDED_ENUM_ATTRIBUTES:
        return report_missing_attribute(mx.original_type, itype, name, mx)

    node = itype.type.get(name)
    if node and node.type:
        proper = get_proper_type(node.type)
        # Support `A = nonmember(1)` function call and decorator.
        if (
            isinstance(proper, Instance)
            and proper.type.fullname == "enum.nonmember"
            and proper.args
        ):
            return proper.args[0]

    # M20: gate the enum_literal tail through Rust. Rust handles only the
    # final branch (name in enum_members -> itype.copy_modified(last_known_value=
    # LiteralType(name, fallback=itype))). The EXCLUDED and nonmember paths

    # above need checker state / node types not in the snapshot — both run
    # in Python before this gate. Defer (None) when the class snapshot is
    # missing or name is not an enum member.
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _native_checkmember_resolver is not None
        and _rust_analyze_enum_class_attribute_access is not None
    ):
        try:
            result = _rust_analyze_enum_class_attribute_access(
                _native_checkmember_resolver,
                _serialize_type_for_checkmember(itype),
                name,
            )
            if result is not None:
                decoded = _deserialize_type_for_checkmember(bytes(result))
                if decoded is not None:
                    if isinstance(decoded, ProperType):
                        decoded.line = itype.line
                        decoded.column = itype.column
                    return decoded
        except (AssertionError, NotImplementedError):
            pass

    if name not in itype.type.enum_members:
        return None

    enum_literal = LiteralType(name, fallback=itype)
    return itype.copy_modified(last_known_value=enum_literal)


def analyze_typeddict_access(
    name: str, typ: TypedDictType, mx: MemberContext, override_info: TypeInfo | None
) -> Type:
    # M20: gate the __delitem__ branch through Rust (pure CallableType).
    # __setitem__ needs checker state; the fallback branch recurses on an
    # Instance (defers). Both defer to Python.
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _native_checkmember_resolver is not None
        and _rust_analyze_typeddict_access is not None
    ):
        try:
            result = _rust_analyze_typeddict_access(
                _native_checkmember_resolver,
                name,
                _serialize_type_for_checkmember(typ),
                state.state.strict_optional,
            )
            if result is not None:
                decoded = _deserialize_type_for_checkmember(bytes(result))
                if decoded is not None:
                    if isinstance(decoded, ProperType):
                        decoded.line = typ.line
                        decoded.column = typ.column
                        if isinstance(decoded, CallableType):
                            decoded.fallback.line = decoded.line
                    return decoded
        except (AssertionError, NotImplementedError):
            pass
    if name == "__setitem__":
        if isinstance(mx.context, IndexExpr):
            # Since we can get this during `a['key'] = ...`
            # it is safe to assume that the context is `IndexExpr`.
            item_type, key_names = mx.chk.expr_checker.visit_typeddict_index_expr(
                typ, mx.context.index, setitem=True
            )
            assigned_readonly_keys = typ.readonly_keys & key_names
            if assigned_readonly_keys and not mx.suppress_errors:
                mx.msg.readonly_keys_mutated(assigned_readonly_keys, context=mx.context)
        else:
            # It can also be `a.__setitem__(...)` direct call.
            # In this case `item_type` can be `Any`,
            # because we don't have args available yet.

            # TODO: check in `default` plugin that `__setitem__` is correct.
            item_type = AnyType(TypeOfAny.implementation_artifact)
        return CallableType(
            arg_types=[mx.chk.named_type("builtins.str"), item_type],
            arg_kinds=[ARG_POS, ARG_POS],
            arg_names=[None, None],
            ret_type=NoneType(),
            fallback=mx.chk.named_type("builtins.function"),
            name=name,
        )
    elif name == "__delitem__":
        return CallableType(
            arg_types=[mx.chk.named_type("builtins.str")],
            arg_kinds=[ARG_POS],
            arg_names=[None],
            ret_type=NoneType(),
            fallback=mx.chk.named_type("builtins.function"),
            name=name,
        )
    return _analyze_member_access(name, typ.fallback, mx, override_info)


def add_class_tvars(
    t: ProperType,
    isuper: Instance | None,
    is_classmethod: bool,
    mx: MemberContext,
    original_vars: Sequence[TypeVarLikeType] | None = None,
    is_trivial_self: bool = False,
) -> Type:
    """Instantiate type variables during analyze_class_attribute_access,
    e.g T and Q in the following:

    class A(Generic[T]):
        @classmethod
        def foo(cls: Type[Q]) -> Tuple[T, Q]: ...

    class B(A[str]): pass
    B.foo()

    Args:
        t: Declared type of the method (or property)
        isuper: Current instance mapped to the superclass where method was defined, this
            is usually done by map_instance_to_supertype()
        is_classmethod: True if this method is decorated with @classmethod
        original_vars: Type variables of the class callable on which the method was accessed
        is_trivial_self: if True, we can use fast path for bind_self().
    Returns:
        Expanded method type with added type variables (when needed).
    """
    # TODO: verify consistency between Q and T

    # We add class type variables if the class method is accessed on class object
    # without applied type arguments, this matches the behavior of __init__().
    # For example (continuing the example in docstring):

    #     A       # The type of callable is def [T] () -> A[T], _not_ def () -> A[Any]
    #     A[int]  # The type of callable is def () -> A[int]
    # and

    #     A.foo       # The type is generic def [T] () -> Tuple[T, A[T]]
    #     A[int].foo  # The type is non-generic def () -> Tuple[int, A[int]]
    #

    # This behaviour is useful for defining alternative constructors for generic classes.
    # To achieve such behaviour, we add the class type variables that are still free
    # (i.e. appear in the return type of the class object on which the method was accessed).

    # M20: gate the classmethod + trivial_self path through Rust. Rust handles
    # the CallableType path (freshen + bind_self_fast + expand + copy_modified)
    # and the Overloaded recursion. Defer (None) for non-classmethod, non-trivial,

    # already-bound, or property paths.
    if (
        _HAS_TYPE_KERNEL
        and _native_checkmember_active
        and _native_checkmember_resolver is not None
        and _rust_add_class_tvars is not None
        and is_classmethod
        and is_trivial_self
        and not mx.is_self
        and not mx.is_super
    ):
        try:
            tvars = original_vars if original_vars is not None else []
            # Serialize original_vars as a wire-format type list.
            orig_vars_buf = _CheckMemberWriteBuffer()
            from mypy.types import write_type_list
            write_type_list(orig_vars_buf, list(tvars))
            orig_vars_bytes = orig_vars_buf.getvalue()
            isuper_bytes = (
                _serialize_type_for_checkmember(isuper) if isuper is not None else b""
            )
            result = _rust_add_class_tvars(
                _native_checkmember_resolver,
                _serialize_type_for_checkmember(t),
                isuper_bytes,
                is_classmethod,
                is_trivial_self,
                mx.preserve_type_var_ids,
                orig_vars_bytes,
                TypeVarId.next_raw_id,
                state.state.strict_optional,
            )
            if result is not None:
                next_raw_id, changed, wire_bytes = result
                if changed:
                    TypeVarId.next_raw_id = next_raw_id
                decoded = _deserialize_type_for_checkmember(bytes(wire_bytes), freeze=True)
                if decoded is not None:
                    freeze_all_type_vars(decoded)
                    return _restore_definition(t, decoded)
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(t, CallableType):
        tvars = original_vars if original_vars is not None else []
        if not mx.preserve_type_var_ids:
            t = freshen_all_functions_type_vars(t)
        if is_classmethod and not t.is_bound:
            if is_trivial_self:
                t = bind_self_fast(t, mx.self_type)
            else:
                t = bind_self(t, mx.self_type, is_classmethod=True)
        if isuper is not None:
            t = expand_type_by_instance(t, isuper)
        freeze_all_type_vars(t)
        return t.copy_modified(variables=list(tvars) + list(t.variables))
    elif isinstance(t, Overloaded):
        return Overloaded(
            [
                cast(
                    CallableType,
                    add_class_tvars(item, isuper, is_classmethod, mx, original_vars=original_vars),
                )
                for item in t.items
            ]
        )
    if isuper is not None:
        t = expand_type_by_instance(t, isuper)
    return t


def analyze_decorator_or_funcbase_access(
    defn: Decorator | FuncBase, itype: Instance, name: str, mx: MemberContext
) -> Type:
    """Analyzes the type behind method access.

    The function itself can possibly be decorated.
    See: https://github.com/python/mypy/issues/10409
    """
    if isinstance(defn, Decorator):
        return analyze_var(name, defn.var, itype, mx)
    typ = function_type(defn, mx.chk.named_type("builtins.function"))
    if isinstance(defn, (FuncDef, OverloadedFuncDef)) and defn.is_trivial_self:
        return bind_self_fast(typ, mx.self_type)
    typ = check_self_arg(typ, mx.self_type, defn.is_class, mx.context, name, mx.msg)
    return bind_self(typ, original_type=mx.self_type, is_classmethod=defn.is_class)


F = TypeVar("F", bound=FunctionLike)


def bind_self_fast(method: F, original_type: Type | None = None) -> F:
    """Return a copy of `method`, with the type of its first parameter (usually
    self or cls) bound to original_type.

    This is a faster version of mypy.typeops.bind_self() that can be used for methods
    with trivial self/cls annotations.
    """
    if _HAS_TYPE_KERNEL and _native_checkmember_active and _rust_bind_self_fast is not None:
        # Rust path is a pure handled decision; its strip rebuild is
        # deterministic from Python attrs, so skip the round-trip and reuse
        # the Python body directly. Overloaded recursion stays native.
        if isinstance(method, CallableType):
            if not method.arg_types:
                return method
            if method.arg_kinds[0] in (ARG_STAR, ARG_STAR2):
                return method
            return cast(
                F,
                method.copy_modified(
                    arg_types=method.arg_types[1:],
                    arg_kinds=method.arg_kinds[1:],
                    arg_names=method.arg_names[1:],
                    is_bound=True,
                ),
            )
        result = _rust_bind_self_fast(_serialize_type_for_checkmember(method))
        if result is not None:
            decoded = _deserialize_type_for_checkmember(bytes(result))
            if decoded is not None:
                if isinstance(method, CallableType) and isinstance(decoded, CallableType):  # type: ignore[misc]
                    if not method.arg_types or method.arg_kinds[0] in (ARG_STAR, ARG_STAR2):
                        return method
                    return cast(
                        F,
                        method.copy_modified(
                            arg_types=method.arg_types[1:],
                            arg_kinds=method.arg_kinds[1:],
                            arg_names=method.arg_names[1:],
                            is_bound=True,
                        ),
                    )
                elif isinstance(method, Overloaded) and isinstance(decoded, Overloaded):  # type: ignore[misc]
                    if not method.items:
                        return method
                    items: list[CallableType] = []
                    for c in method.items:
                        bound = bind_self_fast(c, original_type)
                        items.append(bound)
                    return cast(F, Overloaded(items))
    if isinstance(method, Overloaded):
        items = [bind_self_fast(c, original_type) for c in method.items]
        return cast(F, Overloaded(items))
    assert isinstance(method, CallableType)
    if not method.arg_types:
        # Invalid method, return something.
        return method
    if method.arg_kinds[0] in (ARG_STAR, ARG_STAR2):
        # See typeops.py for details.
        return method
    return method.copy_modified(
        arg_types=method.arg_types[1:],
        arg_kinds=method.arg_kinds[1:],
        arg_names=method.arg_names[1:],
        is_bound=True,
    )


def has_operator(typ: Type, op_method: str) -> bool:
    """Does type have operator with the given name?

    Note: this follows the rules for operator access, in particular:
    * __getattr__ is not considered
    * for class objects we only look in metaclass
    * instance level attributes (i.e. extra_attrs) are not considered
    """
    # This is much faster than analyze_member_access, and so using
    # it first as a filter is important for performance. This is mostly relevant
    # in situations where we can't expect that method is likely present,

    # e.g. for __OP__ vs __rOP__.
    typ = get_proper_type(typ)

    if _HAS_TYPE_KERNEL and _native_checkmember_active and _native_checkmember_resolver is not None and _rust_has_operator is not None:
        try:
            # The Rust path expands TypeVarLikeType internally (values_or_bound)
            # and defers (None) for any case it cannot decide, e.g. a
            # TypeVarType with a non-empty value restriction.
            result = _rust_has_operator(
                _native_checkmember_resolver,
                _serialize_type_for_checkmember(typ),
                op_method,
                state.state.strict_optional,
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass

    if isinstance(typ, TypeVarLikeType):
        typ = typ.values_or_bound()
    if isinstance(typ, AnyType):
        return True
    if isinstance(typ, UnionType):
        return all(has_operator(x, op_method) for x in typ.relevant_items())
    if isinstance(typ, FunctionLike) and typ.is_type_obj():
        return typ.fallback.type.has_readable_member(op_method)
    if isinstance(typ, TypeType):
        # Type[Union[X, ...]] is always normalized to Union[Type[X], ...],
        # so we don't need to care about unions here, but we need to care about
        # Type[T], where upper bound of T is a union.
        item = typ.item
        if isinstance(item, TypeVarType):
            item = item.values_or_bound()
        if isinstance(item, UnionType):
            return all(meta_has_operator(x, op_method) for x in item.relevant_items())
        return meta_has_operator(item, op_method)
    return instance_fallback(typ).type.has_readable_member(op_method)


def instance_fallback(typ: ProperType) -> Instance:
    if _HAS_TYPE_KERNEL and _native_checkmember_active and _rust_instance_fallback is not None:
        try:
            result = _rust_instance_fallback(_serialize_type_for_checkmember(typ))
            if result is not None:
                decoded = _deserialize_type_for_checkmember(bytes(result))
                # Rust mirrors Python: Literal/TypedDict return their fallback
                # (always an Instance); a TupleType whose partial fallback is
                # not an Instance already deferred. Only trust an Instance.
                if decoded is not None and isinstance(decoded, Instance):  # type: ignore[misc]
                    return decoded
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(typ, Instance):
        return typ
    if isinstance(typ, TupleType):
        return tuple_fallback(typ)
    if isinstance(typ, (LiteralType, TypedDictType)):
        return typ.fallback
    if instance_cache.object_type is None:
        object_typeinfo = lookup_stdlib_typeinfo("builtins.object", modules_state.modules)
        instance_cache.object_type = Instance(object_typeinfo, [])
    return instance_cache.object_type


def meta_has_operator(item: Type, op_method: str) -> bool:
    item = get_proper_type(item)
    if _HAS_TYPE_KERNEL and _native_checkmember_active and _native_checkmember_resolver is not None and _rust_meta_has_operator is not None:
        try:
            result = _rust_meta_has_operator(
                _native_checkmember_resolver,
                _serialize_type_for_checkmember(item),
                op_method,
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(item, AnyType):
        return True
    item = instance_fallback(item)
    meta = item.type.metaclass_type
    if meta is None:
        type_type = lookup_stdlib_typeinfo("builtins.type", modules_state.modules)
        meta = Instance(type_type, [])
    return meta.type.has_readable_member(op_method)


def defined_in_superclass(info: TypeInfo, name: str) -> bool:
    """Check if a variable has an explicit value at class level in any of superclasses."""
    if _HAS_TYPE_KERNEL and _native_checkmember_active and _native_checkmember_resolver is not None and _rust_defined_in_superclass is not None:
        try:
            result = _rust_defined_in_superclass(
                _native_checkmember_resolver, info.fullname, name
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    for base in info.mro[1:]:
        if (node := base.names.get(name)) is not None:
            if not node.implicit and isinstance(node.node, Var) and node.node.has_explicit_value:
                return True
    return False
