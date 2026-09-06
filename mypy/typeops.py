"""Miscellaneous type operations and helpers for use during type checking.

NOTE: These must not be accessed from mypy.nodes or mypy.types to avoid import
      cycles. These must not be called from the semantic analysis main pass
      since these may assume that MROs are ready.
"""

from __future__ import annotations

import itertools
from collections.abc import Callable, Iterable, Sequence
from typing import Any, Final, TypeVar, cast

from mypy.checker_state import checker_state
from mypy.copytype import copy_type
from mypy.expandtype import (
    _resync_definitions as _resync_wire_definitions,
    expand_type,
    expand_type_by_instance,
)
from mypy.lookup import lookup_stdlib_typeinfo
from mypy.maptype import map_instance_to_supertype
from mypy.modules_state import modules_state
from mypy.nodes import (
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    SYMBOL_FUNCBASE_TYPES,
    Decorator,
    Expression,
    FuncBase,
    FuncDef,
    FuncItem,
    OverloadedFuncDef,
    StrExpr,
    SymbolNode,
    TypeInfo,
    Var,
)
from mypy.state import state
from mypy.types import (
    ELLIPSIS_TYPE_NAMES,
    NOT_IMPLEMENTED_TYPE_NAMES,
    AnyType,
    CallableType,
    ExtraAttrs,
    FormalArgument,
    FunctionLike,
    Instance,
    LiteralType,
    NoneType,
    NormalizedCallableType,
    Overloaded,
    Parameters,
    ParamSpecType,
    PartialType,
    ProperType,
    TupleType,
    Type,
    TypeAliasType,
    TypedDictType,
    TypeGuardedType,
    TypeOfAny,
    TypeQuery,
    TypeType,
    TypeVarLikeType,
    TypeVarTupleType,
    TypeVarType,
    UninhabitedType,
    UnionType,
    UnpackType,
    _read_mirror_blob,
    _serialize_stats,
    _serialize_stats_on,
    _serialize_with_taint_check,
    _type_wire_cache,
    _wire_cache_enabled,
    flatten_nested_unions,
    get_proper_type,
    get_proper_types,
    instance_cache,
    remove_dups,
)
from mypy.typestate import type_state
from mypy.typetraverser import TypeTraverserVisitor
from mypy.typevars import fill_typevars

# Stage 3e type-kernel seam: when the `type_kernel` Rust extension is
# importable and `Options.native_type_kernel` is set, union / literal
# helpers route through Rust, falling back to Python on `None`.
try:
    import type_kernel as _type_kernel
    from librt.internal import ReadBuffer as _ReadBuffer, WriteBuffer as _WriteBuffer

    from mypy.types import read_type as _read_type

    _HAS_TYPE_KERNEL = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _ReadBuffer = None  # type: ignore[assignment,misc]
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _read_type = None  # type: ignore[assignment]
    _HAS_TYPE_KERNEL = False

_native_typeops_active: bool = False
_native_typeops_resolver: Any = None

# Decision tags returned by `_type_kernel.rust_classify_type_object_type`
# (#1059); must match TYPE_OBJECT_* in crates/type_kernel/src/typeops.rs.
NATIVE_TYPE_OBJECT_ERROR_INIT = 0
NATIVE_TYPE_OBJECT_ERROR_NEW = 1
NATIVE_TYPE_OBJECT_INIT = 2
NATIVE_TYPE_OBJECT_NEW = 3
NATIVE_TYPE_OBJECT_TIE_ANY = 4


def _needs_python(typ: Type, *, definition_gate: bool = True) -> bool:
    """True if `typ` nests a node a kernel round-trip cannot carry.

    Mirrors `mypy.expandtype._needs_python`: named callables lose their
    FuncDef/Decorator definition node (breaking error formatting that names
    the function), and recursive TypeAliasType would loop while decoding.
    Callers that re-stamp dropped ``definition`` links after the round-trip
    (the #1207 seams, mirroring the #1169 expandtype pattern) pass
    ``definition_gate=False``. Both must defer to the pure-Python path.

    Fresh meta-vars round-trip fine and need no branch: wire-decode seams
    re-unify split occurrences via canonicalize_fresh_vars (#1198).
    """
    stack: list[Type] = [typ]
    visited: set[int] = set()
    while stack:
        t = stack.pop()
        p = get_proper_type(t)
        if id(p) in visited:
            continue
        visited.add(id(p))
        if isinstance(p, CallableType):
            if definition_gate and p.definition is not None:
                return True
            stack.append(p.ret_type)
            stack.extend(p.arg_types)
            stack.append(p.fallback)
        elif isinstance(p, TypeAliasType):
            return True
        elif isinstance(p, Instance):
            stack.extend(p.args)
        elif isinstance(p, UnionType):
            stack.extend(p.items)
        elif isinstance(p, TupleType):
            stack.extend(p.items)
        elif isinstance(p, TypeType):
            stack.append(p.item)
    return False


def _set_native_typeops_active(active: bool) -> None:
    global _native_typeops_active
    _native_typeops_active = active


def _set_native_typeops_resolver(resolver: Any) -> None:
    global _native_typeops_resolver
    _native_typeops_resolver = resolver


def _has_mutated_truthiness(t: Type) -> bool:
    """Does ``t`` (recursively) carry non-default truthiness flags?

    Wire serialization drops ``can_be_true``/``can_be_false``, so any
    type whose flags were explicitly mutated (e.g. by ``narrow_declared_type``
    setting ``can_be_false = False``) would be corrupted by the roundtrip.
    When this returns True, ``make_simplified_union`` must defer to Python
    to avoid wrong dedup survivor selection in ``_remove_redundant_union_items``.
    """
    # -1 = unset (lazy = default). 0/1 = cached. Lazy init sets
    # to default; explicit mutation makes it differ. Compare against
    # default, not != -1, since some types default to False.
    cbt = t._can_be_true
    cbf = t._can_be_false
    if cbt != -1 and bool(cbt) != t.can_be_true_default():
        return True
    if cbf != -1 and bool(cbf) != t.can_be_false_default():
        return True
    # Recurse into union items (flattened by make_simplified_union).
    proper = get_proper_type(t)
    if isinstance(proper, UnionType):
        return any(_has_mutated_truthiness(item) for item in proper.items)
    return False


# Argless built-in instances serialize to fixed bytes; mirroring the
# fast path in checker.py / checkexpr.py / subtypes.py.
_BUILTIN_INSTANCE_BYTES: Final[dict[str, bytes]] = {
    "builtins.str": b"\x50\x53",
    "builtins.function": b"\x50\x54",
    "builtins.int": b"\x50\x55",
    "builtins.bool": b"\x50\x56",
    "builtins.object": b"\x50\x57",
}


def _serialize_type(t: Type) -> bytes:
    """Serialize a `Type` to its wire-format bytes for the Rust reader."""
    key = id(t)
    if _wire_cache_enabled():
        entry = _type_wire_cache.get(key)
        if entry is not None and entry[0] is t:
            return entry[1]
    if type(t) is Instance:
        fn = t.type.fullname
        if (
            not t.args
            and not t.last_known_value
            and not t.extra_attrs
            and fn in _BUILTIN_INSTANCE_BYTES
        ):
            return _BUILTIN_INSTANCE_BYTES[fn]
    # Phase F2 (#1393, slice 5): mirror-blob read on the expensive miss
    # path, same seam contract as the subtypes funnel.
    blob = _read_mirror_blob(t)
    if blob is not None:
        if _serialize_stats_on:
            _serialize_stats["mirror"] += 1
        return blob
    buf = _WriteBuffer()
    result, saw_tvar = _serialize_with_taint_check(t, buf)
    if not saw_tvar and _wire_cache_enabled() and (not isinstance(t, Instance) or t.type_ref is None):  # type: ignore[misc]
        _type_wire_cache[key] = (t, result)
    return result


def _serialize_type_list(items: Sequence[Type]) -> bytes:
    buf = _WriteBuffer()
    from mypy.types import write_type_list

    write_type_list(buf, items)
    return buf.getvalue()


# bytes -> decoded wire-Type cache for the typeops seam.  Byte-identical
# blobs repeat heavily (~95% of 128K calls), so memoizing read_type +
# fixup_wire_type cuts decode cost.
_typeops_decode_cache: dict[bytes, ProperType] = {}
_typeops_decode_list_cache: dict[bytes, list[Type]] = {}


def _clear_typeops_decode_cache() -> None:
    _typeops_decode_cache.clear()
    _typeops_decode_list_cache.clear()


def _deserialize_type(data: bytes) -> ProperType | None:
    """Deserialize wire bytes to a Type, fixing type_ref strings.

    Returns None if any type_ref cannot be resolved to a live TypeInfo
    (so the caller defers to Python). A non-None result is structurally
    a proper-type tree: this seam only passes fully-resolved Rust-built
    types, and fixup_wire_type defers on any TypeAliasType.
    """
    cached = _typeops_decode_cache.get(data)
    if cached is not None:
        return cached
    from mypy.types import instance_cache
    from mypy.wirefixup import fixup_wire_type

    decoded = _read_type(_ReadBuffer(data))
    # Clear instance_cache primitives after read_type so NOT_READY
    # singletons cannot leak into later builds (mirrors _typeanal_decode).
    instance_cache.int_type = None
    instance_cache.str_type = None
    instance_cache.bool_type = None
    instance_cache.object_type = None
    instance_cache.function_type = None
    fixed = fixup_wire_type(decoded)
    if fixed is None:
        return None
    fixed = cast(ProperType, fixed)
    _typeops_decode_cache[data] = fixed
    return fixed


def _deserialize_type_list(data: bytes) -> list[Type] | None:
    """Deserialize wire bytes to a list of Types, fixing type_ref strings.

    Returns None if any type_ref cannot be resolved to a live TypeInfo
    (so the caller defers to Python). Decoding opts into the ErasedType
    wire tag (122): the rru seam runs with keep_erased=True and its
    output may legitimately carry ErasedType nodes. The flag is set on
    mypy.types itself (read there as a module global); this decoder's
    only caller is the rru seam, whose cache is list-value-only and
    never stamped, so a shared flag-on cache is safe.
    """
    cached = _typeops_decode_list_cache.get(data)
    if cached is not None:
        return cached
    from mypy import types as types_mod
    from mypy.types import instance_cache, read_type_list
    from mypy.wirefixup import fixup_wire_type

    old = types_mod._ALLOW_WIRE_ERASED_TYPE
    types_mod._ALLOW_WIRE_ERASED_TYPE = True
    try:
        decoded = read_type_list(_ReadBuffer(data))
    finally:
        types_mod._ALLOW_WIRE_ERASED_TYPE = old
    # Same NOT_READY-singleton hygiene as _deserialize_type.
    instance_cache.int_type = None
    instance_cache.str_type = None
    instance_cache.bool_type = None
    instance_cache.object_type = None
    instance_cache.function_type = None
    result: list[Type] = []
    for item in decoded:
        fixed = fixup_wire_type(item)
        if fixed is None:
            return None
        result.append(fixed)
    if result:
        _typeops_decode_list_cache[data] = result
    return result


def is_recursive_pair(s: Type, t: Type) -> bool:
    """Is this a pair of recursive types?

    There may be more cases, and we may be forced to use e.g. has_recursive_types()
    here, but this function is called in very hot code, so we try to keep it simple
    and return True only in cases we know may have problems.
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        # Only recursive alias pairs (or the TupleType-fallback arm, which
        # also needs a recursive alias) can be recursive, so skip the wire
        # round-trip for non-alias pairs: the Python fallback is False.
        if isinstance(s, TypeAliasType) or isinstance(t, TypeAliasType):
            try:
                result = _type_kernel.rust_is_recursive_pair(
                    _serialize_type(s), _serialize_type(t), _native_typeops_resolver
                )
                if result is not None:
                    return result
            except (AssertionError, NotImplementedError):
                pass
    if isinstance(s, TypeAliasType) and s.is_recursive:
        return (
            isinstance(get_proper_type(t), (Instance, UnionType))
            or isinstance(t, TypeAliasType)
            and t.is_recursive
            # Tuple types are special, they can cause an infinite recursion even if
            # the other type is not recursive, because of the tuple fallback that is
            # calculated "on the fly".
            or isinstance(get_proper_type(s), TupleType)
        )
    if isinstance(t, TypeAliasType) and t.is_recursive:
        return (
            isinstance(get_proper_type(s), (Instance, UnionType))
            or isinstance(s, TypeAliasType)
            and s.is_recursive
            # Same as above.
            or isinstance(get_proper_type(t), TupleType)
        )
    return False


def tuple_fallback(typ: TupleType) -> Instance:
    """Return fallback type for a tuple."""
    info = typ.partial_fallback.type
    if info.fullname != "builtins.tuple":
        return typ.partial_fallback
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            result = _type_kernel.rust_tuple_fallback(
                _serialize_type(typ), _native_typeops_resolver
            )
            if result is not None:
                decoded = _deserialize_type(bytes(result))
                if decoded is not None and isinstance(decoded, Instance):
                    return decoded
        except (AssertionError, NotImplementedError, ValueError):
            pass
    items = []
    for item in typ.items:
        if isinstance(item, UnpackType):
            unpacked_type = get_proper_type(item.type)
            if isinstance(unpacked_type, TypeVarTupleType):
                unpacked_type = get_proper_type(unpacked_type.upper_bound)
            if (
                isinstance(unpacked_type, Instance)
                and unpacked_type.type.fullname == "builtins.tuple"
            ):
                items.append(unpacked_type.args[0])
            else:
                raise NotImplementedError
        else:
            items.append(item)
    return Instance(
        info,
        # Note: flattening recursive unions is dangerous, since it can fool recursive
        # types optimization in subtypes.py and go into infinite recursion.
        [make_simplified_union(items, handle_recursive=False)],
        extra_attrs=typ.partial_fallback.extra_attrs,
    )


def get_self_type(func: CallableType, def_info: TypeInfo) -> Type | None:
    default_self = fill_typevars(def_info)
    if isinstance(get_proper_type(func.ret_type), UninhabitedType):
        return func.ret_type
    elif func.arg_types and func.arg_types[0] != default_self and func.arg_kinds[0] == ARG_POS:
        return func.arg_types[0]
    else:
        return None


def _type_object_type_rust_head(
    info: TypeInfo, classified: tuple[Any, ...], allow_cache: bool
) -> ProperType:
    """Apply the arbitration tag returned by `rust_classify_type_object_type`.

    `classified` is `(tag, is_new, special_sig, uncached, method)`; the
    uncached bit is re-checked here so `allow_cache` can be passed
    through unmodified. Python applies every side effect: the
    invalid-class-definition Any, fallback construction, the
    universal-callable tie arm, the already-native
    `type_object_type_from_function` tail, the tuple `special_sig`
    fixup, and the cache write.
    """
    tag, is_new, special_sig, uncached, method = classified
    if uncached:
        allow_cache = False
    if tag in (NATIVE_TYPE_OBJECT_ERROR_INIT, NATIVE_TYPE_OBJECT_ERROR_NEW):
        # Must be an invalid class definition.
        return AnyType(TypeOfAny.from_error)
    if info.metaclass_type is not None:
        fallback = info.metaclass_type
    else:
        type_type = lookup_stdlib_typeinfo("builtins.type", modules_state.modules)
        fallback = Instance(type_type, [])
    if tag == NATIVE_TYPE_OBJECT_TIE_ANY:
        # Both are defined by object with a bogus base class:
        # construct a universal callable as the prototype.
        any_type = AnyType(TypeOfAny.special_form)
        if instance_cache.function_type is None:
            function_typeinfo = lookup_stdlib_typeinfo("builtins.function", modules_state.modules)
            instance_cache.function_type = Instance(function_typeinfo, [])
        sig = CallableType(
            arg_types=[any_type, any_type],
            arg_kinds=[ARG_STAR, ARG_STAR2],
            arg_names=["_args", "_kwds"],
            ret_type=any_type,
            is_bound=True,
            fallback=instance_cache.function_type,
        )
        result = class_callable(sig, info, None, fallback, None, is_new=False)
        if allow_cache and state.strict_optional:
            info.type_object_type = result
        return result
    # tag is NATIVE_TYPE_OBJECT_INIT or NATIVE_TYPE_OBJECT_NEW.
    if isinstance(method, FuncBase):
        t = function_type(method, fallback)
    else:
        assert isinstance(method.type, ProperType)
        assert isinstance(method.type, FunctionLike)  # is_valid_constructor() ensures this
        t = method.type
    result = type_object_type_from_function(t, info, method.info, fallback, is_new)
    if special_sig:
        assert isinstance(result, CallableType)
        result = result.copy_modified(special_sig="tuple")
    # Only write cached result is strict_optional=True, otherwise we may get
    # inconsistent behaviour because of union simplification.
    if allow_cache and state.strict_optional:
        info.type_object_type = result
    return result


def type_object_type(
    info: TypeInfo, named_type: Callable[[str], Instance] | None = None
) -> ProperType:
    """Return the type of a type object.

    For a generic type G with type variables T and S the type is generally of form

      Callable[..., G[T, S]]

    where ... are argument types for the __init__/__new__ method (without the self
    argument). Also, the fallback type will be 'type' instead of 'function'.
    Note: we keep the unused `named_type` argument to avoid breaking plugins.
    """
    allow_cache = (
        checker_state.type_checker is not None
        and checker_state.type_checker.allow_constructor_cache
    )

    if info.type_object_type is not None:
        if allow_cache:
            return info.type_object_type
        info.type_object_type = None

    # Native type_kernel seam (#1059): Rust decides the init-vs-new-vs-tie
    # arbitration (plus the tuple special_sig and cache-write policy) from
    # the live TypeInfo; Python applies every side effect in the shim below.
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            classified = _type_kernel.rust_classify_type_object_type(info)
        except (AssertionError, NotImplementedError, ValueError):
            classified = None
        if classified is not None:
            if classified[3]:
                # Uncached: do not cache if the method's type is not ready.
                allow_cache = False
            return _type_object_type_rust_head(info, classified, allow_cache)

    # We take the type from whichever of __init__ and __new__ is first
    # in the MRO, preferring __init__ if there is a tie.
    init_method = info.get("__init__")
    new_method = info.get("__new__")
    if not init_method or not is_valid_constructor(init_method.node):
        # Must be an invalid class definition.
        return AnyType(TypeOfAny.from_error)
    # There *should* always be a __new__ method except the test stubs
    # lack it, so just copy init_method in that situation
    new_method = new_method or init_method
    if not is_valid_constructor(new_method.node):
        # Must be an invalid class definition.
        return AnyType(TypeOfAny.from_error)

    # The two is_valid_constructor() checks ensure this.
    assert isinstance(new_method.node, (SYMBOL_FUNCBASE_TYPES, Decorator))
    assert isinstance(init_method.node, (SYMBOL_FUNCBASE_TYPES, Decorator))

    init_index = info.mro.index(init_method.node.info)
    new_index = info.mro.index(new_method.node.info)

    if info.metaclass_type is not None:
        fallback = info.metaclass_type
    else:
        type_type = lookup_stdlib_typeinfo("builtins.type", modules_state.modules)
        fallback = Instance(type_type, [])

    if init_index < new_index:
        method: FuncBase | Decorator = init_method.node
        is_new = False
    elif init_index > new_index:
        method = new_method.node
        is_new = True
    else:
        if init_method.node.info.fullname == "builtins.object":
            # Both are defined by object.  But if we've got a bogus
            # base class, we can't know for sure, so check for that.
            if info.fallback_to_any:
                # Construct a universal callable as the prototype.
                any_type = AnyType(TypeOfAny.special_form)
                if instance_cache.function_type is None:
                    function_typeinfo = lookup_stdlib_typeinfo(
                        "builtins.function", modules_state.modules
                    )
                    instance_cache.function_type = Instance(function_typeinfo, [])
                sig = CallableType(
                    arg_types=[any_type, any_type],
                    arg_kinds=[ARG_STAR, ARG_STAR2],
                    arg_names=["_args", "_kwds"],
                    ret_type=any_type,
                    is_bound=True,
                    fallback=instance_cache.function_type,
                )
                result: FunctionLike = class_callable(
                    sig, info, None, fallback, None, is_new=False
                )
                if allow_cache and state.strict_optional:
                    info.type_object_type = result
                return result

        # Otherwise prefer __init__ in a tie. It isn't clear that this
        # is the right thing, but __new__ caused problems with
        # typeshed (#5647).
        method = init_method.node
        is_new = False
    # Construct callable type based on signature of __init__. Adjust
    # return type and insert type arguments.
    if isinstance(method, FuncBase):
        if isinstance(method, OverloadedFuncDef) and not method.type:
            # Do not cache if the type is not ready. Same logic for decorators is
            # achieved in early return above because is_valid_constructor() is False.
            allow_cache = False
        t = function_type(method, fallback)
    else:
        assert isinstance(method.type, ProperType)
        assert isinstance(method.type, FunctionLike)  # is_valid_constructor() ensures this
        t = method.type
    result = type_object_type_from_function(t, info, method.info, fallback, is_new)
    # Tuple constructor in typeshed is imprecise (and precise one is impossible to express),
    # so we special-case constructors for tuple types. Note we skip the tuple class itself
    # as a micro-optimization, since it is unlikely one would write tuple((1, 2)).
    if method.info.fullname == "builtins.tuple" and info.fullname != "builtins.tuple":
        assert isinstance(result, CallableType)
        result = result.copy_modified(special_sig="tuple")
    # Only write cached result is strict_optional=True, otherwise we may get
    # inconsistent behaviour because of union simplification.
    if allow_cache and state.strict_optional:
        info.type_object_type = result
    return result


def is_valid_constructor(n: SymbolNode | None) -> bool:
    """Does this node represents a valid constructor method?

    This includes normal functions, overloaded functions, and decorators
    that return a callable type.
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            result = _type_kernel.rust_is_valid_constructor(n)
            if result is not None:
                return result
        except (AssertionError, NotImplementedError, ValueError):
            pass
    if isinstance(n, SYMBOL_FUNCBASE_TYPES):
        return True
    if isinstance(n, Decorator):
        return isinstance(get_proper_type(n.type), FunctionLike)
    return False


def _restamp_composite_definitions(
    signature: FunctionLike, decoded: FunctionLike
) -> FunctionLike | None:
    """Re-stamp ``definition`` links the type-object round-trip dropped (#1207).

    The #1169 pattern, specialized to the type-object composite: the
    pure-Python body (bind_self, map_type_from_supertype, class_callable)
    preserves ``CallableType.definition`` through copy_modified, so the
    result carries exactly the input ``signature``'s definitions, per
    overload item. Pair positionally; ``None`` defers to the pure-Python
    body when the shapes diverge.
    """
    if isinstance(signature, CallableType) and isinstance(decoded, CallableType):
        return decoded.copy_modified(definition=signature.definition)
    if isinstance(signature, Overloaded) and isinstance(decoded, Overloaded):
        if len(signature.items) != len(decoded.items):
            return None
        items = [
            item.copy_modified(definition=orig.definition)
            for orig, item in zip(signature.items, decoded.items)
        ]
        return Overloaded(items)
    return None


def type_object_type_from_function(
    signature: FunctionLike, info: TypeInfo, def_info: TypeInfo, fallback: Instance, is_new: bool
) -> FunctionLike:
    # #492 composite seam: Rust mirrors the whole body on the wire signature,
    # defers on unhandled shapes, and rebuilds the final object via copy_modified
    # (non-wire fields survive); #1207 relaxes the gate and re-stamps links.
    special_sig_seam: str | None = "dict" if def_info.fullname == "builtins.dict" else None
    default_ret_seam = fill_typevars(info)
    if (
        _HAS_TYPE_KERNEL
        and _native_typeops_active
        and _native_typeops_resolver is not None
        and not _needs_python(signature, definition_gate=False)
    ):
        from mypy.wirefixup import resync_var_identities

        try:
            result = _type_kernel.rust_type_object_type_from_function(
                _serialize_type(signature),
                info,
                def_info,
                _serialize_type(fallback),
                is_new,
                state.strict_optional,
                type_state.infer_unions,
                _native_typeops_resolver,
            )
            if result is not None:
                decoded = _deserialize_type(bytes(result))
                if decoded is not None and isinstance(decoded, FunctionLike):
                    # Expansion may leave leftover TypeVars; re-link their
                    # identities to the live originals. A resync defer (None)
                    # falls back to the pure-Python body.
                    resynced = resync_var_identities(signature, decoded, [default_ret_seam])
                    if resynced is not None and isinstance(
                        resynced, FunctionLike
                    ):  # type: ignore[misc]
                        fixed = _restamp_composite_definitions(signature, resynced)
                        if fixed is not None:
                            if isinstance(fixed, CallableType):
                                return fixed.copy_modified(
                                    special_sig=special_sig_seam, instance_type=default_ret_seam
                                )
                            ov_items = []
                            for item in fixed.items:
                                assert isinstance(item, CallableType)
                                ov_items.append(
                                    item.copy_modified(
                                        special_sig=special_sig_seam,
                                        instance_type=default_ret_seam,
                                    )
                                )
                            return Overloaded(ov_items)
        except (AssertionError, NotImplementedError, ValueError):
            pass

    # We first need to record all non-trivial (explicit) self types in __init__,
    # since they will not be available after we bind them. Note, we use explicit
    # self-types only in the defining class, similar to __new__ (but not exactly the same,

    # see comment in class_callable below). This is mostly useful for annotating library
    # classes such as subprocess.Popen.
    if not is_new and not info.is_newtype:
        orig_self_types = [get_self_type(it, def_info) for it in signature.items]
    else:
        orig_self_types = [None] * len(signature.items)

    # The __init__ method might come from a generic superclass 'def_info'
    # with type variables that do not map identically to the type variables of
    # the class 'info' being constructed. For example:

    #
    #   class A(Generic[T]):
    #       def __init__(self, x: T) -> None: ...

    #   class B(A[List[T]]):
    #      ...
    #

    # We need to map B's __init__ to the type (List[T]) -> None.
    signature = bind_self(
        signature,
        original_type=fill_typevars(info),
        is_classmethod=is_new,
        # Explicit instance self annotations have special handling in class_callable(),
        # we don't need to bind any type variables in them if they are generic.
        ignore_instances=True,
    )
    signature = cast(FunctionLike, map_type_from_supertype(signature, info, def_info))

    special_sig: str | None = None
    if def_info.fullname == "builtins.dict":
        # Special signature!
        special_sig = "dict"

    if isinstance(signature, CallableType):
        return class_callable(
            signature, info, def_info, fallback, special_sig, is_new, orig_self_types[0]
        )
    else:
        # Overloaded __init__/__new__.
        assert isinstance(signature, Overloaded)
        items: list[CallableType] = []
        for item, orig_self in zip(signature.items, orig_self_types):
            items.append(
                class_callable(item, info, def_info, fallback, special_sig, is_new, orig_self)
            )
        return Overloaded(items)


def class_callable(
    init_type: CallableType,
    info: TypeInfo,
    def_info: TypeInfo | None,
    type_type: Instance,
    special_sig: str | None,
    is_new: bool,
    orig_self_type: Type | None = None,
) -> CallableType:
    """Create a type object type based on the signature of __init__."""
    # #492 follow-up native seam: Rust makes the ret_type decision and
    # combines the type variables (pure once the two resolver-backed subtype
    # booleans are known). Python computes those booleans (already native)

    # and rebuilds the live CallableType so non-wire fields survive.
    # instance_type MUST stay the live fill_typevars(info) result.
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        init_ret_type = get_proper_type(init_type.ret_type)
        orig_self_proper = get_proper_type(orig_self_type) if orig_self_type is not None else None
        explicit_type = init_ret_type if is_new else orig_self_proper
        default_ret_type = fill_typevars(info)
        from mypy.subtypes import is_equivalent, is_subtype

        is_eq = False
        if is_new and explicit_type is not None:
            # Default return type in the class where the constructor method
            # was defined.
            default_def_ret_type = (
                fill_typevars(def_info) if def_info is not None else default_ret_type
            )
            is_eq = is_equivalent(default_def_ret_type, explicit_type, ignore_type_params=True)
        is_st = False
        if (
            isinstance(explicit_type, (Instance, TupleType, UninhabitedType, LiteralType))
            and isinstance(default_ret_type, Instance)
            and not default_ret_type.type.is_protocol
        ):
            is_st = is_subtype(explicit_type, default_ret_type, ignore_type_params=True)
        try:
            result = _type_kernel.rust_class_callable(
                _serialize_type(init_type),
                _serialize_type(explicit_type) if explicit_type is not None else None,
                _serialize_type(default_ret_type),
                is_new,
                is_eq,
                is_st,
                info,
            )
            if result is not None:
                ret_type_blob, var_blobs = result
                ret_type = _deserialize_type(bytes(ret_type_blob))
                if (
                    ret_type is not None
                    and isinstance(ret_type, Instance)
                    and ret_type.type is not info
                    and ret_type.type.fullname == info.fullname
                ):
                    # Fine-grained regression guard (mirror of fill_typevars'):
                    # re-home a decoded ret_type naming the caller's own class
                    # to the live TypeInfo so its identity survives merge_asts.
                    ret_type = Instance(
                        info,
                        ret_type.args,
                        last_known_value=ret_type.last_known_value,
                        extra_attrs=ret_type.extra_attrs,
                    )
                if ret_type is not None:
                    rust_variables: list[TypeVarLikeType] = []
                    deferred = False
                    for blob in var_blobs:
                        decoded_var = _deserialize_type(bytes(blob))
                        if decoded_var is None:
                            deferred = True
                            break
                        rust_variables.append(cast(TypeVarLikeType, decoded_var))
                    if not deferred:
                        return init_type.copy_modified(
                            ret_type=ret_type,
                            fallback=type_type,
                            name=info.name,
                            variables=rust_variables,
                            special_sig=special_sig,
                            instance_type=default_ret_type,
                        )
        except (AssertionError, NotImplementedError, ValueError):
            pass

    variables: list[TypeVarLikeType] = []
    variables.extend(info.defn.type_vars)
    variables.extend(init_type.variables)

    from mypy.subtypes import is_equivalent, is_subtype

    init_ret_type = get_proper_type(init_type.ret_type)
    orig_self_type = get_proper_type(orig_self_type)
    default_ret_type = fill_typevars(info)
    # Default return type in the class where constructor method was defined.
    default_def_ret_type = fill_typevars(def_info) if def_info is not None else default_ret_type
    explicit_type = init_ret_type if is_new else orig_self_type
    if (
        is_new
        and explicit_type is not None
        # Legacy quirk: an explicit __new__ return of a non-subclass C is
        # ignored when building the constructor type for a subclass D(C),
        # so keep D's default (fill_typevars) return instead.
        and (
            isinstance(explicit_type, AnyType)
            and explicit_type.type_of_any != TypeOfAny.unannotated
            or not is_equivalent(default_def_ret_type, explicit_type, ignore_type_params=True)
        )
    ):
        ret_type = explicit_type
    elif (
        isinstance(explicit_type, (Instance, TupleType, UninhabitedType, LiteralType))
        # We have to skip protocols, because it can be a subtype of a return type
        # by accident. Like `Hashable` is a subtype of `object`. See #11799
        and isinstance(default_ret_type, Instance)
        and not default_ret_type.type.is_protocol
        # Use the declared self in __init__ if it is a subtype of what we would use otherwise.
        and is_subtype(explicit_type, default_ret_type, ignore_type_params=True)
    ):
        ret_type = explicit_type
    else:
        ret_type = default_ret_type

    return init_type.copy_modified(
        ret_type=ret_type,
        fallback=type_type,
        name=info.name,
        variables=variables,
        special_sig=special_sig,
        instance_type=default_ret_type,
    )


def map_type_from_supertype(typ: Type, sub_info: TypeInfo, super_info: TypeInfo) -> Type:
    """Map type variables in a type defined in a supertype context to be valid
    in the subtype context. Assume that the result is unique; if more than
    one type is possible, return one of the alternatives.

    For example, assume

      class D(Generic[S]): ...
      class C(D[E[T]], Generic[T]): ...

    Now S in the context of D would be mapped to E[T] in the context of C.
    """
    # Native composite seam (#492 family): the whole body below is one Rust
    # call, deferred when `typ` cannot cross the wire. #1207 relaxes the
    # definition gate; the shim re-stamps the dropped definition links.
    if (
        _HAS_TYPE_KERNEL
        and _native_typeops_active
        and _native_typeops_resolver is not None
        and not _needs_python(typ, definition_gate=False)
    ):
        try:
            result = _type_kernel.rust_map_type_from_supertype(
                _native_typeops_resolver,
                sub_info,
                super_info,
                _serialize_type(typ),
                state.strict_optional,
            )
            if result is not None:
                # Relink contract (#1309, mirroring expand_type_by_instance):
                # the seam returns leftover TypeVars and surviving aliases;
                # the shim re-links them and defers on anything unmatchable.
                from mypy.types import instance_cache
                from mypy.wirefixup import (
                    canonicalize_fresh_vars,
                    fixup_wire_type,
                    resync_var_identities,
                )

                decoded = _read_type(_ReadBuffer(bytes(result)))
                instance_cache.int_type = None
                instance_cache.str_type = None
                instance_cache.bool_type = None
                instance_cache.object_type = None
                instance_cache.function_type = None
                fixed = None
                if decoded is not None:
                    fixed = fixup_wire_type(decoded, resolve_aliases=True)
                    if fixed is not None:
                        fixed = canonicalize_fresh_vars(fixed)
                        if isinstance(fixed, ProperType):
                            fixed.line = typ.line
                            fixed.column = typ.column
                            if isinstance(fixed, CallableType):
                                fixed.fallback.line = fixed.line
                        inst_type_env = fill_typevars(sub_info)
                        if isinstance(inst_type_env, TupleType):
                            inst_type_env = tuple_fallback(inst_type_env)
                        env_values = inst_type_env.args
                        relinked = resync_var_identities(typ, fixed, env_values)
                        if relinked is None:
                            fixed = None
                        else:
                            fixed = _resync_wire_definitions(typ, relinked)
                if fixed is not None:
                    return fixed
        except (AssertionError, NotImplementedError, ValueError):
            # AssertionError: TypeInfo not yet fixed during semanal.
            # NotImplementedError: unserializable variant.
            # ValueError: decode/read failure.
            pass
    # Create the type of self in subtype, of form t[a1, ...].
    inst_type = fill_typevars(sub_info)
    if isinstance(inst_type, TupleType):
        inst_type = tuple_fallback(inst_type)
    # Map the type of self to supertype. This gets us a description of the
    # supertype type variables in terms of subtype variables, i.e. t[t1, ...]
    # so that any type variables in tN are to be interpreted in subtype

    # context.
    inst_type = map_instance_to_supertype(inst_type, super_info)
    # Finally expand the type variables in type with those in the previously
    # constructed type. Note that both type and inst_type may have type
    # variables, but in type they are interpreted in supertype context while

    # in inst_type they are interpreted in subtype context. This works even if
    # the names of type variables in supertype and subtype overlap.
    return expand_type_by_instance(typ, inst_type)


def supported_self_type(
    typ: ProperType, allow_callable: bool = True, allow_instances: bool = True
) -> bool:
    """Is this a supported kind of explicit self-types?

    Currently, this means an X or Type[X], where X is an instance or
    a type variable with an instance upper bound.
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            result = _type_kernel.rust_supported_self_type(
                _serialize_type(typ), _native_typeops_resolver, allow_callable, allow_instances
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError, ValueError):
            pass
    if isinstance(typ, TypeType):
        return supported_self_type(typ.item)
    if allow_callable and isinstance(typ, CallableType):
        # Special case: allow class callable instead of Type[...] as cls annotation,
        # as well as callable self for callback protocols.
        return True
    return isinstance(typ, TypeVarType) or (
        allow_instances and isinstance(typ, Instance) and typ != fill_typevars(typ.type)
    )


F = TypeVar("F", bound=FunctionLike)


def bind_self(
    method: F,
    original_type: Type | None = None,
    is_classmethod: bool = False,
    ignore_instances: bool = False,
) -> F:
    """Return a copy of `method`, with the type of its first parameter (usually
    self or cls) bound to original_type.

    If the type of `self` is a generic type (T, or Type[T] for classmethods),
    instantiate every occurrence of type with original_type in the rest of the
    signature and in the return type.

    original_type is the type of E in the expression E.copy(). It is None in
    compatibility checks. In this case we treat it as the erasure of the
    declared type of self.

    This way we can express "the type of self". For example:

    T = TypeVar('T', bound='A')
    class A:
        def copy(self: T) -> T: ...

    class B(A): pass

    b = B().copy()  # type: B

    """
    if isinstance(method, Overloaded):
        items = [
            bind_self(c, original_type, is_classmethod, ignore_instances) for c in method.items
        ]
        return cast(F, Overloaded(items))
    assert isinstance(method, CallableType)
    func: CallableType = method
    if not func.arg_types:
        # Invalid method, return something.
        return method
    if func.arg_kinds[0] in (ARG_STAR, ARG_STAR2):
        # The signature is of the form 'def foo(*args, ...)'.
        # In this case we shouldn't drop the first arg,
        # since func will be absorbed by the *args.

        # TODO: infer bounds on the type of *args?

        # In the case of **kwargs we should probably emit an error, but
        # for now we simply skip it, to avoid crashes down the line.
        return method
    self_param_type = get_proper_type(func.arg_types[0])

    variables: Sequence[TypeVarLikeType]
    # #492 native seam: for a non-generic CallableType the whole strip path
    # is one Rust call that decides the case is handled. The decoded result
    # is only a "handled" signal; the final object is built through

    # copy_modified on the live object so non-wire fields (special_sig,
    # from_type_type, definition, line/column) survive the roundtrip. Rust
    # defers (None) on variables / star-args / empty args, so the typevar

    # path below is untouched.
    if not func.variables and _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            result = _type_kernel.rust_bind_self(_serialize_type(func))
            if result is not None:
                decoded = _deserialize_type(bytes(result))
                if decoded is not None and isinstance(decoded, CallableType):
                    return cast(
                        F,
                        func.copy_modified(
                            arg_types=func.arg_types[1:],
                            arg_kinds=func.arg_kinds[1:],
                            arg_names=func.arg_names[1:],
                            variables=func.variables,
                            is_bound=True,
                        ),
                    )
        except (AssertionError, NotImplementedError, ValueError):
            pass
    # Having a def __call__(self: Callable[...], ...) can cause infinite recursion. Although
    # this special-casing looks not very principled, there is nothing meaningful we can
    # infer

    # from such definition, since it is inherently indefinitely recursive.
    allow_callable = func.name is None or not func.name.startswith("__call__ of")
    if func.variables and supported_self_type(
        self_param_type, allow_callable=allow_callable, allow_instances=not ignore_instances
    ):
        from mypy.infer import infer_type_arguments

        if original_type is None:
            # TODO: type check method override (see #7861).
            original_type = erase_to_bound(self_param_type)
        original_type = get_proper_type(original_type)

        # Find which of method type variables appear in the type of "self".
        self_ids = {tv.id for tv in get_all_type_vars(self_param_type)}
        self_vars = [tv for tv in func.variables if tv.id in self_ids]

        # Solve for these type arguments using the actual class or instance type.
        typeargs = infer_type_arguments(
            self_vars, self_param_type, original_type, is_supertype=True, erase_types=False
        )
        if (
            is_classmethod
            and any(isinstance(get_proper_type(t), UninhabitedType) for t in typeargs)
            and isinstance(original_type, (Instance, TypeVarType, TupleType))
        ):
            # In case we call a classmethod through an instance x, fallback to type(x).
            typeargs = infer_type_arguments(
                self_vars,
                self_param_type,
                TypeType(original_type),
                is_supertype=True,
                erase_types=False,
            )

        # Update the method signature with the solutions found.
        # Technically, some constraints might be unsolvable, make them Never.
        to_apply = [t if t is not None else UninhabitedType() for t in typeargs]
        func = expand_type(func, {tv.id: arg for tv, arg in zip(self_vars, to_apply)})
        variables = [v for v in func.variables if v not in self_vars]
    else:
        variables = func.variables

    res = func.copy_modified(
        arg_types=func.arg_types[1:],
        arg_kinds=func.arg_kinds[1:],
        arg_names=func.arg_names[1:],
        variables=variables,
        is_bound=True,
    )
    return cast(F, res)


def erase_to_bound(t: Type) -> Type:
    # TODO: use value restrictions to produce a union?
    t = get_proper_type(t)
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            result = _type_kernel.rust_erase_to_bound(_serialize_type(t))
            if result is not None:
                decoded = _deserialize_type(bytes(result))
                if decoded is not None:
                    return decoded
        except (AssertionError, NotImplementedError, ValueError):
            pass
    if isinstance(t, TypeVarType):
        return t.upper_bound
    if isinstance(t, TypeType):
        if isinstance(t.item, TypeVarType):
            return TypeType.make_normalized(t.item.upper_bound, is_type_form=t.is_type_form)
    return t


def callable_corresponding_argument(
    typ: NormalizedCallableType | Parameters, model: FormalArgument
) -> FormalArgument | None:
    """Return the argument of a function that corresponds to `model`"""

    by_name = typ.argument_by_name(model.name)
    by_pos = typ.argument_by_position(model.pos)
    if by_name is None and by_pos is None:
        return None
    if by_name is not None and by_pos is not None:
        if by_name == by_pos:
            return by_name
        # If we're dealing with an optional pos-only and an optional
        # name-only arg, merge them.  This is the case for all functions
        # taking both *args and **args, or a pair of functions like so:

        # def right(a: int = ...) -> None: ...
        # def left(x: int = ..., /, *, a: int = ...) -> None: ...
        from mypy.meet import meet_types

        if (
            not (by_name.required or by_pos.required)
            and by_pos.name is None
            and by_name.pos is None
            # This is not principled, but prevents a crash. It's weird to have a FormalArgument
            # that has an UnpackType.
            and not isinstance(by_name.typ, UnpackType)
            and not isinstance(by_pos.typ, UnpackType)
        ):
            return FormalArgument(
                by_name.name, by_pos.pos, meet_types(by_name.typ, by_pos.typ), False
            )
        return by_name

    return by_name if by_name is not None else by_pos


def simple_literal_type(t: ProperType | None) -> Instance | None:
    """Extract the underlying fallback Instance type for a simple Literal"""
    if (
        _HAS_TYPE_KERNEL
        and _native_typeops_active
        and _native_typeops_resolver is not None
        and t is not None
    ):
        try:
            # (decided, value) wire answer (issue #1101 protocol, #1295): a
            # decided not-a-literal answer skips the body below; only an
            # exception (undecodable blob / stale extension) falls through.
            decided, result = _type_kernel.rust_simple_literal_type(_serialize_type(t))
            if decided:
                if result is None:
                    return None
                decoded = _deserialize_type(bytes(result))
                if isinstance(decoded, Instance):
                    return decoded
                # Unreachable: the fallback of a wire literal is always an
                # Instance blob; fall through defensively.
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(t, Instance) and t.last_known_value is not None:
        t = t.last_known_value
    if isinstance(t, LiteralType):
        return t.fallback
    return None


def is_simple_literal(t: ProperType) -> bool:
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            result = _type_kernel.rust_is_simple_literal(
                _serialize_type(t), _native_typeops_resolver
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(t, LiteralType):
        return t.fallback.type.is_enum or t.fallback.type.fullname == "builtins.str"
    if isinstance(t, Instance):
        return t.last_known_value is not None and isinstance(t.last_known_value.value, str)
    return False


def make_simplified_union(
    items: Sequence[Type],
    line: int = -1,
    column: int = -1,
    *,
    keep_erased: bool = False,
    contract_literals: bool = True,
    handle_recursive: bool = True,
) -> ProperType:
    """Build union type with redundant union items removed.

    If only a single item remains, this may return a non-union type.

    Examples:

    * [int, str] -> Union[int, str]
    * [int, object] -> object
    * [int, int] -> int
    * [int, Any] -> Union[int, Any] (Any types are not simplified away!)
    * [Any, Any] -> Any
    * [int, Union[bytes, str]] -> Union[int, bytes, str]

    Note: This must NOT be used during semantic analysis, since TypeInfos may not
          be fully initialized.

    The keep_erased flag is used for type inference against union types
    containing type variables. If set to True, keep all ErasedType items.

    The contract_literals flag indicates whether we need to contract literal types
    back into a sum type. Set it to False when called by try_expanding_sum_type_
    to_union().
    """
    if (
        _HAS_TYPE_KERNEL
        and _native_typeops_active
        and _native_typeops_resolver is not None
        and len(items) > 1
        and not any(_has_mutated_truthiness(item) for item in items)
    ):
        try:
            result = _type_kernel.rust_make_simplified_union(
                _serialize_type_list(items),
                line,
                column,
                keep_erased,
                contract_literals,
                handle_recursive,
                state.strict_optional,
                _native_typeops_resolver,
            )
            if result is not None:
                decoded = _deserialize_type(bytes(result))
                # Defer when a type_ref is unresolvable: returning None here
                # crashes semanal's check_type_arguments on .accept().
                if decoded is None:
                    raise NotImplementedError("unresolvable type_ref in simplified union")
                return decoded
        except (AssertionError, NotImplementedError):
            pass
    # Step 1: expand all nested unions
    items = flatten_nested_unions(items, handle_recursive=handle_recursive)

    # Step 2: fast path for single item
    if len(items) == 1:
        return get_proper_type(items[0])

    # Step 3: remove redundant unions
    simplified_set: Sequence[Type] = _remove_redundant_union_items(items, keep_erased)

    # Step 4: If more than one literal exists in the union, try to simplify
    if (
        contract_literals
        and sum(isinstance(get_proper_type(item), LiteralType) for item in simplified_set) > 1
    ):
        simplified_set = try_contracting_literals_in_union(simplified_set)

    result = get_proper_type(UnionType.make_union(simplified_set, line, column))

    nitems = len(items)
    if nitems > 1 and (
        nitems > 2 or not (type(items[0]) is NoneType or type(items[1]) is NoneType)
    ):
        # Step 5: At last, we erase any (inconsistent) extra attributes on instances.

        # Initialize with None instead of an empty set as a micro-optimization. The set
        # is needed very rarely, so we try to avoid constructing it.
        extra_attrs_set: set[ExtraAttrs] | None = None
        for item in items:
            instance = try_getting_instance_fallback(item)
            if instance and instance.extra_attrs:
                if extra_attrs_set is None:
                    extra_attrs_set = {instance.extra_attrs}
                else:
                    extra_attrs_set.add(instance.extra_attrs)

        # Code below is awkward, because we don't want the extra checks to affect
        # performance in the common case.
        erase_extra = False
        if extra_attrs_set is not None:
            fallback = try_getting_instance_fallback(result)
            if fallback is None:
                return result
            if len(extra_attrs_set) > 1:  # This case is too tricky to handle.
                erase_extra = True
            else:
                # Check that all relevant items have the extra attributes.
                for item in items:
                    instance = try_getting_instance_fallback(item)
                    if instance and instance.type == fallback.type and not instance.extra_attrs:
                        erase_extra = True
                        break
            if erase_extra:
                fallback.extra_attrs = None

    return result


def _remove_redundant_union_items(items: list[Type], keep_erased: bool) -> list[Type]:
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            # The wire drops can_be_true/can_be_false: pass them as a
            # per-item blob (flags + mutated bit). A widened survivor comes
            # back marked in result[2]; the shim rebuilds that slot with
            # true_or_false(items[src]) on the live object.
            flags_blob = bytearray()
            mutated_any = False
            for item in items:
                flags_blob += bytes(
                    (
                        1 if item.can_be_true else 0,
                        1 if item.can_be_false else 0,
                        1 if _has_mutated_truthiness(item) else 0,
                    )
                )
                mutated_any = mutated_any or bool(flags_blob[-1])
            result = _type_kernel.rust_remove_redundant_union_items(
                _serialize_type_list(items),
                bytes(flags_blob),
                keep_erased,
                state.strict_optional,
                _native_typeops_resolver,
            )
            if result is not None:
                out, prov, widen = bytes(result[0]), result[1], result[2]
                decoded = _deserialize_type_list(out)
                if decoded is not None:
                    # Widened survivors: a duplicate's truthiness raised a
                    # survivor, so Python replaces the slot with
                    # true_or_false(orig_item), a flags-reset copy.
                    if mutated_any or any(widen):
                        # decoded may be a shared cache entry, copy first.
                        decoded = list(decoded)
                        for i, src in enumerate(prov):
                            if widen[i]:
                                decoded[i] = true_or_false(items[src])
                            elif _has_mutated_truthiness(items[src]):
                                decoded[i] = items[src]
                    return decoded
        except (AssertionError, NotImplementedError, ValueError):
            # Unserializable variant or decode/read failure: defer.
            pass
    from mypy.subtypes import is_proper_subtype

    # The first pass through this loop, we check if later items are subtypes of earlier
    # items.
    # The second pass through this loop, we check if earlier items are subtypes of later

    # items
    # (by reversing the remaining items)
    for _direction in range(2):
        new_items: list[Type] = []
        # seen is a map from a type to its index in new_items
        seen: dict[ProperType, int] = {}
        unduplicated_literal_fallbacks: set[Instance] | None = None
        for ti in items:
            proper_ti = get_proper_type(ti)

            # UninhabitedType is always redundant
            if isinstance(proper_ti, UninhabitedType):
                continue

            duplicate_index = -1
            # Quickly check if we've seen this type
            if proper_ti in seen:
                duplicate_index = seen[proper_ti]
            elif (
                isinstance(proper_ti, LiteralType)
                and unduplicated_literal_fallbacks is not None
                and proper_ti.fallback in unduplicated_literal_fallbacks
            ):
                # This is an optimisation for unions with many LiteralType
                # We've already checked for exact duplicates. This means that any super type of
                # the LiteralType must be a super type of its fallback. If we've gone through

                # the expensive loop below and found no super type for a previous LiteralType
                # with the same fallback, we can skip doing that work again and just add the type
                # to new_items
                pass
            else:
                # If not, check if we've seen a supertype of this type
                for j, tj in enumerate(new_items):
                    proper_tj = get_proper_type(tj)
                    # If tj is an Instance with a last_known_value, do not remove proper_ti
                    # (unless it's an instance with the same last_known_value)
                    if (
                        isinstance(proper_tj, Instance)
                        and proper_tj.last_known_value is not None
                        and not (
                            isinstance(proper_ti, Instance)
                            and proper_tj.last_known_value == proper_ti.last_known_value
                        )
                    ):
                        continue

                    if is_proper_subtype(
                        ti, tj, keep_erased_types=keep_erased, ignore_promotions=True
                    ):
                        duplicate_index = j
                        break
            if duplicate_index != -1:
                # If deleted subtypes had more general truthiness, use that
                orig_item = new_items[duplicate_index]
                if not orig_item.can_be_true and ti.can_be_true:
                    new_items[duplicate_index] = true_or_false(orig_item)
                elif not orig_item.can_be_false and ti.can_be_false:
                    new_items[duplicate_index] = true_or_false(orig_item)
            else:
                # We have a non-duplicate item, add it to new_items
                seen[proper_ti] = len(new_items)
                new_items.append(ti)
                if isinstance(proper_ti, LiteralType):
                    if unduplicated_literal_fallbacks is None:
                        unduplicated_literal_fallbacks = set()
                    unduplicated_literal_fallbacks.add(proper_ti.fallback)

        items = new_items
        if len(items) <= 1:
            break
        items.reverse()

    return items


def _get_type_method_ret_type(t: ProperType, *, name: str) -> Type | None:
    # For Enum literals the ret_type can change based on the Enum
    # we need to check the type of the enum rather than the literal
    if isinstance(t, LiteralType) and t.is_enum_literal():
        t = t.fallback

    if isinstance(t, Instance):
        sym = t.type.get(name)
        if sym:
            sym_type = get_proper_type(sym.type)
            if isinstance(sym_type, CallableType):
                return sym_type.ret_type

    return None


def _interpret_truthiness_result(disc: tuple[int, object], t: ProperType) -> ProperType | None:
    """Map a Rust truthiness discriminator to a live Python `ProperType`.

    Returns `None` if the discriminator can't be interpreted (the caller
    falls through to the Python path). Tags:
      0=Uninhabited, 1=NoneType, 2=SameType, 3=CopyTrueOnly,
      4=CopyFalseOnly, 5=CopyReset, 6=LiteralEmptyStr(fallback_bytes),
      7=LiteralZero(fallback_bytes), 8=UnionNarrow(item_discs).
    """
    tag = disc[0]
    if tag == 0:
        return UninhabitedType(line=t.line, column=t.column)
    elif tag == 1:
        return NoneType(line=t.line)
    elif tag == 2:
        return t
    elif tag == 3:
        new_t = copy_type(t)
        new_t.can_be_false = False
        return new_t
    elif tag == 4:
        new_t = copy_type(t)
        new_t.can_be_true = False
        return new_t
    elif tag == 5:
        new_t = copy_type(t)
        new_t.can_be_true = new_t.can_be_true_default()
        new_t.can_be_false = new_t.can_be_false_default()
        return new_t
    elif tag == 6:
        fallback = _deserialize_type(bytes(disc[1]))  # type: ignore[call-overload]
        if isinstance(fallback, Instance):
            return LiteralType("", fallback=fallback)
        return None
    elif tag == 7:
        fallback = _deserialize_type(bytes(disc[1]))  # type: ignore[call-overload]
        if isinstance(fallback, Instance):
            return LiteralType(0, fallback=fallback)
        return None
    elif tag == 8:
        item_discs: list[tuple[int, object]] = disc[1]  # type: ignore[assignment]
        new_items: list[Type] = []
        for i, item_disc in enumerate(item_discs):
            if not isinstance(t, UnionType):
                return None
            item = t.items[i]
            item_proper = get_proper_type(item)
            result = _interpret_truthiness_result(item_disc, item_proper)
            if result is None:
                return None
            new_items.append(result)
        return make_simplified_union(new_items, line=t.line, column=t.column)
    return None


def true_only(t: Type) -> ProperType:
    """
    Restricted version of t with only True-ish values
    """
    t = get_proper_type(t)

    # Steps 1-2 + union recursion read the LIVE can_be_true/can_be_false
    # flags, which the wire does not carry and copy_type may have mutated
    # on union items; Rust only decides the step-4 dunder leaf.
    if not t.can_be_true:
        return UninhabitedType(line=t.line, column=t.column)
    elif not t.can_be_false:
        return t
    elif isinstance(t, UnionType):
        new_items = [true_only(item) for item in t.items]
        can_be_true_items = [item for item in new_items if item.can_be_true]
        return make_simplified_union(can_be_true_items, line=t.line, column=t.column)
    else:
        if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
            try:
                result = _type_kernel.rust_true_only(_serialize_type(t), _native_typeops_resolver)
                if result is not None:
                    interpreted = _interpret_truthiness_result(result, t)
                    if interpreted is not None:
                        return interpreted
            except (AssertionError, NotImplementedError):
                pass
        ret_type = _get_type_method_ret_type(t, name="__bool__") or _get_type_method_ret_type(
            t, name="__len__"
        )

        if ret_type and not ret_type.can_be_true:
            return UninhabitedType(line=t.line, column=t.column)

        new_t = copy_type(t)
        new_t.can_be_false = False
        return new_t


def false_only(t: Type) -> ProperType:
    """
    Restricted version of t with only False-ish values
    """
    t = get_proper_type(t)

    # Steps 1-2 and union recursion (step 3) read LIVE flags (see
    # true_only); they stay in Python. Rust only decides the step 4-6 leaf.
    if not t.can_be_false:
        if state.strict_optional:
            return UninhabitedType(line=t.line)
        else:
            return NoneType(line=t.line)
    elif not t.can_be_true:
        return t
    elif isinstance(t, UnionType):
        new_items = [false_only(item) for item in t.items]
        can_be_false_items = [item for item in new_items if item.can_be_false]
        return make_simplified_union(can_be_false_items, line=t.line, column=t.column)
    elif isinstance(t, Instance) and t.type.fullname in ("builtins.str", "builtins.bytes"):
        return LiteralType("", fallback=t)
    elif isinstance(t, Instance) and t.type.fullname == "builtins.int":
        return LiteralType(0, fallback=t)
    else:
        if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
            try:
                result = _type_kernel.rust_false_only(
                    _serialize_type(t), state.strict_optional, _native_typeops_resolver
                )
                if result is not None:
                    interpreted = _interpret_truthiness_result(result, t)
                    if interpreted is not None:
                        return interpreted
            except (AssertionError, NotImplementedError):
                pass
        ret_type = _get_type_method_ret_type(t, name="__bool__") or _get_type_method_ret_type(
            t, name="__len__"
        )

        if ret_type:
            if not ret_type.can_be_false:
                return UninhabitedType(line=t.line)
        elif isinstance(t, Instance):
            if (t.type.is_final or t.type.is_enum) and state.strict_optional:
                return UninhabitedType(line=t.line)
        elif isinstance(t, LiteralType) and t.is_enum_literal() and state.strict_optional:
            return UninhabitedType(line=t.line)

        new_t = copy_type(t)
        new_t.can_be_true = False
        return new_t


def true_or_false(t: Type) -> ProperType:
    """
    Unrestricted version of t with both True-ish and False-ish values
    """
    t = get_proper_type(t)

    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            result = _type_kernel.rust_true_or_false(_serialize_type(t), _native_typeops_resolver)
            if result is not None:
                interpreted = _interpret_truthiness_result(result, t)
                if interpreted is not None:
                    return interpreted
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(t, UnionType):
        new_items = [true_or_false(item) for item in t.items]
        return make_simplified_union(new_items, line=t.line, column=t.column)

    new_t = copy_type(t)
    new_t.can_be_true = new_t.can_be_true_default()
    new_t.can_be_false = new_t.can_be_false_default()
    return new_t


def erase_def_to_union_or_bound(tdef: TypeVarLikeType) -> Type:
    # TODO(PEP612): fix for ParamSpecType
    if isinstance(tdef, ParamSpecType):
        return AnyType(TypeOfAny.from_error)
    if isinstance(tdef, TypeVarType) and tdef.values:
        return make_simplified_union(tdef.values)
    else:
        return tdef.upper_bound


def erase_to_union_or_bound(typ: TypeVarType) -> ProperType:
    if typ.values:
        return make_simplified_union(typ.values)
    else:
        return get_proper_type(typ.upper_bound)


def function_type(func: FuncBase, fallback: Instance) -> FunctionLike:
    # #747 seam: Rust mirrors the whole body (typed passthrough, callable_type
    # self-binding, broken-overload dummy). Passthrough returns `func.type`
    # live; built arms restore line/column/definition via copy_modified.
    if _HAS_TYPE_KERNEL and _native_typeops_active and func.type is not None:
        # Pre-check (the #789 pattern): the passthrough arm is `func.type`
        # truthy -> return it, decided by Rust on the same condition. Skip
        # the serialize + PyO3 call for the dominant passthrough case.
        assert isinstance(func.type, FunctionLike)
        return func.type
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            result = _type_kernel.rust_function_type(func, _serialize_type(fallback))
            if result is not None:
                is_passthrough, wire_bytes = result
                if is_passthrough:
                    # Typed passthrough: same as `if func.type: return func.type`.
                    assert isinstance(func.type, FunctionLike)
                    return func.type
                decoded = _deserialize_type(bytes(wire_bytes))
                if decoded is not None and isinstance(decoded, CallableType):
                    # callable_type arm: Python passes fdef.line/column and
                    # definition=fdef for FuncDef (error-message naming).
                    definition: SymbolNode | None = func if isinstance(func, FuncDef) else None
                    return decoded.copy_modified(
                        line=func.line,
                        column=func.column,
                        name=func.name,
                        implicit=True,
                        definition=definition,
                    )
                elif isinstance(decoded, Overloaded):
                    # Broken overload: rebuild the inner dummy with the
                    # overload's line; Python builds it with no name, so do
                    # not copy func.name here.
                    item = decoded.items[0].copy_modified(line=func.line, implicit=False)
                    return Overloaded([item])
        except (AssertionError, NotImplementedError, ValueError):
            pass
    if func.type:
        assert isinstance(func.type, FunctionLike)
        return func.type
    else:
        # Implicit type signature with dynamic types.
        if isinstance(func, FuncItem):
            return callable_type(func, fallback)
        else:
            # Either a broken overload, or decorated overload type is not ready.
            # TODO: make sure the caller defers if possible.
            assert isinstance(func, OverloadedFuncDef)
            any_type = AnyType(TypeOfAny.from_error)
            dummy = CallableType(
                [any_type, any_type],
                [ARG_STAR, ARG_STAR2],
                [None, None],
                any_type,
                fallback,
                line=func.line,
                is_ellipsis_args=True,
            )
            # Return an Overloaded, because some callers may expect that
            # an OverloadedFuncDef has an Overloaded type.
            return Overloaded([dummy])


def callable_type(
    fdef: FuncItem, fallback: Instance, ret_type: Type | None = None
) -> CallableType:
    # TODO: somewhat unfortunate duplication with prepare_method_signature in semanal
    if fdef.info and fdef.has_self_or_cls_argument and fdef.arg_names:
        self_type: Type = fill_typevars(fdef.info)
        if fdef.is_class or fdef.name == "__new__":
            self_type = TypeType.make_normalized(self_type)
        args = [self_type] + [AnyType(TypeOfAny.unannotated)] * (len(fdef.arg_names) - 1)
    else:
        args = [AnyType(TypeOfAny.unannotated)] * len(fdef.arg_names)

    return CallableType(
        args,
        fdef.arg_kinds,
        fdef.arg_names,
        ret_type or AnyType(TypeOfAny.unannotated),
        fallback,
        name=fdef.name,
        line=fdef.line,
        column=fdef.column,
        implicit=True,
        # We need this for better error messages, like missing `self` note:
        definition=fdef if isinstance(fdef, FuncDef) else None,
    )


def try_getting_str_literals(expr: Expression, typ: Type) -> list[str] | None:
    """If the given expression or type corresponds to a string literal
    or a union of string literals, returns a list of the underlying strings.
    Otherwise, returns None.

    Specifically, this function is guaranteed to return a list with
    one or more strings if one of the following is true:

    1. 'expr' is a StrExpr
    2. 'typ' is a LiteralType containing a string
    3. 'typ' is a UnionType containing only LiteralType of strings
    """
    if isinstance(expr, StrExpr):
        return [expr.value]

    # TODO: See if we can eliminate this function and call the below one directly
    return try_getting_str_literals_from_type(typ)


def try_getting_str_literals_from_type(typ: Type) -> list[str] | None:
    """If the given expression or type corresponds to a string Literal
    or a union of string Literals, returns a list of the underlying strings.
    Otherwise, returns None.

    For example, if we had the type 'Literal["foo", "bar"]' as input, this function
    would return a list of strings ["foo", "bar"].
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            wire = _type_kernel.rust_try_getting_str_literals_from_type(_serialize_type(typ))
            if wire is not None:
                decided, result = wire
                if decided:
                    return result
        except (AssertionError, NotImplementedError):
            pass
    return try_getting_literals_from_type(typ, str, "builtins.str")


def try_getting_int_literals_from_type(typ: Type) -> list[int] | None:
    """If the given expression or type corresponds to an int Literal
    or a union of int Literals, returns a list of the underlying ints.
    Otherwise, returns None.

    For example, if we had the type 'Literal[1, 2, 3]' as input, this function
    would return a list of ints [1, 2, 3].
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            wire = _type_kernel.rust_try_getting_int_literals_from_type(_serialize_type(typ))
            if wire is not None:
                decided, result = wire
                if decided:
                    return result
        except (AssertionError, NotImplementedError):
            pass
    return try_getting_literals_from_type(typ, int, "builtins.int")


T = TypeVar("T")


def try_getting_literals_from_type(
    typ: Type, target_literal_type: type[T], target_fullname: str
) -> list[T] | None:
    """If the given expression or type corresponds to a Literal or
    union of Literals where the underlying values correspond to the given
    target type, returns a list of those underlying values. Otherwise,
    returns None.
    """
    if (
        _HAS_TYPE_KERNEL
        and _native_typeops_active
        and target_literal_type is bool
        and target_fullname == "builtins.bool"
    ):
        try:
            wire = _type_kernel.rust_try_getting_bool_literals_from_type(_serialize_type(typ))
            if wire is not None:
                decided, result = wire
                if decided:
                    return result  # type: ignore[return-value]
        except (AssertionError, NotImplementedError):
            pass
    typ = get_proper_type(typ)
    if isinstance(typ, Instance) and typ.last_known_value is not None:
        possible_literals: list[Type] = [typ.last_known_value]
    elif isinstance(typ, UnionType):
        possible_literals = list(typ.items)
    else:
        possible_literals = [typ]

    literals: list[T] = []
    for lit in get_proper_types(possible_literals):
        if isinstance(lit, LiteralType) and lit.fallback.type.fullname == target_fullname:
            val = lit.value
            if isinstance(val, target_literal_type):
                literals.append(val)
            else:
                return None
        else:
            return None
    return literals


def is_literal_type_like(t: Type | None) -> bool:
    """Returns 'true' if the given type context is potentially either a LiteralType,
    a Union of LiteralType, or something similar.
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active and t is not None:
        # Expand before the seam: Python's canonical entry expands at the
        # top of every recursive call, and the wire form cannot carry an
        # unexpanded TypeAliasType (Rust would defer on it).
        try:
            result = _type_kernel.rust_is_literal_type_like(
                _serialize_type(get_proper_type(t)), _native_typeops_resolver
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    t = get_proper_type(t)
    if t is None:
        return False
    elif isinstance(t, LiteralType):
        return True
    elif isinstance(t, UnionType):
        return any(is_literal_type_like(item) for item in t.items)
    elif isinstance(t, TypeVarType):
        return is_literal_type_like(t.upper_bound) or any(
            is_literal_type_like(item) for item in t.values
        )
    else:
        return False


def is_singleton_identity_type(typ: ProperType) -> bool:
    """
    Returns True if every value of this type is identical to every other value of this type,
    as judged by the `is` operator.

    Note that this is not true of certain LiteralType, such as Literal[100001] or Literal["string"]
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            result = _type_kernel.rust_is_singleton_identity_type(
                _serialize_type(typ), _native_typeops_resolver
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError, ValueError):
            pass
    if isinstance(typ, NoneType):
        return True
    if isinstance(typ, Instance):
        return (
            (typ.type.is_enum and len(typ.type.enum_members) == 1)
            or (typ.type.fullname in ELLIPSIS_TYPE_NAMES)
            or (typ.type.fullname in NOT_IMPLEMENTED_TYPE_NAMES)
        )
    if isinstance(typ, LiteralType):
        return typ.is_enum_literal() or isinstance(typ.value, bool)
    if isinstance(typ, TypeType) and isinstance(typ.item, Instance) and typ.item.type.is_final:
        return True
    if isinstance(typ, FunctionLike) and typ.is_type_obj() and typ.type_object().is_final:
        return True
    return False


def is_singleton_equality_type(typ: ProperType) -> bool:
    """
    Returns True if every value of this type compares equal to every other value of this type,
    as judged by the `==` operator.
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            result = _type_kernel.rust_is_singleton_equality_type(
                _serialize_type(typ), _native_typeops_resolver
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError, ValueError):
            pass
    return isinstance(typ, LiteralType) or is_singleton_identity_type(typ)


def try_expanding_sum_type_to_union(typ: Type, target_fullname: str | None) -> Type:
    """Attempts to recursively expand any enum Instances with the given target_fullname
    into a Union of all of its component LiteralTypes.

    For example, if we have:

        class Color(Enum):
            RED = 1
            BLUE = 2
            YELLOW = 3

        class Status(Enum):
            SUCCESS = 1
            FAILURE = 2
            UNKNOWN = 3

    ...and if we call `try_expanding_sum_type_to_union(Union[Color, Status], 'module.Color')`,
    this function will return Literal[Color.RED, Color.BLUE, Color.YELLOW, Status].
    """
    typ = get_proper_type(typ)

    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            # Wire serialization drops can_be_true/can_be_false. The
            # expanded result feeds true_only/false_only/make_simplified_union
            # which rely on these flags for narrowing. Defer to Python

            # when the input carries mutated truthiness.
            if not _has_mutated_truthiness(typ):
                result = _type_kernel.rust_try_expanding_sum_type_to_union(
                    _serialize_type(typ),
                    target_fullname,
                    state.strict_optional,
                    _native_typeops_resolver,
                )
                if result is not None:
                    decoded = _deserialize_type(bytes(result))
                    if decoded is not None:
                        return decoded
        except (AssertionError, NotImplementedError):
            pass

    if isinstance(typ, UnionType):
        # Non-empty enums cannot subclass each other so simply removing duplicates is enough.
        items = [
            try_expanding_sum_type_to_union(item, target_fullname)
            for item in remove_dups(flatten_nested_unions(typ.relevant_items()))
        ]
        return UnionType.make_union(items)

    if isinstance(typ, Instance) and (
        target_fullname is None or typ.type.fullname == target_fullname
    ):
        if typ.type.fullname == "builtins.bool":
            return UnionType([LiteralType(True, typ), LiteralType(False, typ)])

        if typ.type.is_enum:
            items = [LiteralType(name, typ) for name in typ.type.enum_members]
            if not items:
                return typ
            return UnionType.make_union(items)

    return typ


def try_contracting_literals_in_union(types: Sequence[Type]) -> list[ProperType]:
    """Contracts any literal types back into a sum type if possible.

    Requires a flattened union and does not descend into children.

    Will replace the first instance of the literal with the sum type and
    remove all others.

    If we call `try_contracting_union(Literal[Color.RED, Color.BLUE, Color.YELLOW])`,
    this function will return Color.

    We also treat `Literal[True, False]` as `bool`.
    """
    # Native seam: bool + enum contraction runs in Rust, mirroring the
    # pure-Python body below. Reads `enum_members` from the resolver
    # snapshot; defers (None) when the fallback fullname has no snapshot or

    # an item cannot be serialized (e.g. TypeAliasType). Callers flatten
    # nested unions before invoking, so item types are all proper.
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            raw = _type_kernel.rust_try_contracting_literals_in_union(
                _serialize_type_list(list(types)), _native_typeops_resolver
            )
            if raw is not None:
                out = [_deserialize_type(bytes(b)) for b in raw]
                if None not in out:
                    return out  # type: ignore[return-value]  # list[ProperType]
        except (AssertionError, NotImplementedError, ValueError):
            pass
    proper_types = [get_proper_type(typ) for typ in types]
    sum_types: dict[str, tuple[set[Any], list[int]]] = {}
    marked_for_deletion = set()
    for idx, typ in enumerate(proper_types):
        if isinstance(typ, LiteralType):
            fullname = typ.fallback.type.fullname
            if typ.fallback.type.is_enum or isinstance(typ.value, bool):
                if fullname not in sum_types:
                    sum_types[fullname] = (
                        (
                            set(typ.fallback.type.enum_members)
                            if typ.fallback.type.is_enum
                            else {True, False}
                        ),
                        [],
                    )
                literals, indexes = sum_types[fullname]
                literals.discard(typ.value)
                indexes.append(idx)
                if not literals:
                    first, *rest = indexes
                    proper_types[first] = typ.fallback
                    marked_for_deletion |= set(rest)
    return list(
        itertools.compress(
            proper_types, [(i not in marked_for_deletion) for i in range(len(proper_types))]
        )
    )


def coerce_to_literal(typ: Type) -> Type:
    """Recursively converts any Instances that have a last_known_value or are
    instances of enum types with a single value into the corresponding LiteralType.
    """
    # Native seam: union mapping, last-known-value extraction, and the
    # single-member-enum -> LiteralType conversion all run in Rust. Enum
    # members are read live (resolver-installed live TypeInfo map), so the

    # native path never uses a stale snapshot. Defers when the live info map
    # is unavailable, or on a TypeAliasType (no wire target).
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            result = _type_kernel.rust_coerce_to_literal(
                _serialize_type(typ), _native_typeops_resolver
            )
            if result is not None:
                decoded = _deserialize_type(bytes(result))
                if decoded is not None:
                    return decoded
        except (AssertionError, NotImplementedError, ValueError):
            pass
    original_type = typ
    typ = get_proper_type(typ)
    if isinstance(typ, UnionType):
        new_items = [coerce_to_literal(item) for item in typ.items]
        return UnionType.make_union(new_items)
    elif isinstance(typ, Instance):
        if typ.last_known_value:
            return typ.last_known_value
        elif typ.type.is_enum:
            enum_values = typ.type.enum_members
            if len(enum_values) == 1:
                return LiteralType(value=enum_values[0], fallback=typ)
    return original_type


def _rust_type_vars(tp: Type) -> list[TypeVarLikeType] | None:
    """Rust extraction of type vars; None defers to Python.

    Note that this cannot power `get_all_type_vars`: the polymorphic-call
    inference path (`PolyTranslator.collect_vars` in applytype.py) stores the
    returned type variables into a callable's `variables` and later
    `freeze_all_type_vars` (checkexpr.py) mutates `tv.id.meta_level` in place.
    Pure Python returns the same live objects embedded in the type tree, so the
    freeze reaches every occurrence; deserialized copies would mutate only
    themselves and leave live metavariables unresolved. `get_type_vars`
    consumers are membership/hash-only, so a value-returning seam is safe there.
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            if _native_typeops_resolver is not None:
                # Alias-bearing trees expand through the resolver snapshot
                # (rust_get_type_vars); the plain byte entry defers on them.
                result = _type_kernel.rust_get_type_vars_live(
                    _native_typeops_resolver, _serialize_type(tp), False
                )
            else:
                result = _type_kernel.rust_get_type_vars(_serialize_type(tp), False)
        except (AssertionError, NotImplementedError):
            return None
        if result is not None:
            out: list[TypeVarLikeType] = []
            for blob in result:
                decoded = _deserialize_type(bytes(blob))
                if decoded is None:
                    return None
                out.append(decoded)  # type: ignore[arg-type]
            return out
    return None


def get_type_vars(tp: Type) -> list[TypeVarType]:
    rust = _rust_type_vars(tp)
    if rust is not None:
        return cast("list[TypeVarType]", rust)
    return cast("list[TypeVarType]", tp.accept(TypeVarExtractor()))


def get_all_type_vars(tp: Type) -> list[TypeVarLikeType]:
    # Not routed through Rust: see the mutation-sensitivity note in
    # _rust_type_vars above.
    return tp.accept(TypeVarExtractor(include_all=True))


class TypeVarExtractor(TypeQuery[list[TypeVarLikeType]]):
    def __init__(self, include_all: bool = False) -> None:
        super().__init__()
        self.include_all = include_all

    def strategy(self, items: Iterable[list[TypeVarLikeType]]) -> list[TypeVarLikeType]:
        out = []
        for item in items:
            out.extend(item)
        return out

    def visit_type_var(self, t: TypeVarType) -> list[TypeVarLikeType]:
        return [t]

    def visit_param_spec(self, t: ParamSpecType) -> list[TypeVarLikeType]:
        return [t] if self.include_all else []

    def visit_type_var_tuple(self, t: TypeVarTupleType) -> list[TypeVarLikeType]:
        return [t] if self.include_all else []


def freeze_all_type_vars(member_type: Type) -> None:
    member_type.accept(FreezeTypeVarsVisitor())


class FreezeTypeVarsVisitor(TypeTraverserVisitor):
    def visit_callable_type(self, t: CallableType) -> None:
        for v in t.variables:
            v.id.meta_level = 0
        super().visit_callable_type(t)


def custom_special_method(typ: Type, name: str, check_all: bool = False) -> bool:
    """Does this type have a custom special method such as __format__() or __eq__()?

    If check_all is True ensure all items of a union have a custom method, not just some.
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            result = _type_kernel.rust_custom_special_method(
                _serialize_type(typ), name, check_all, _native_typeops_resolver
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError, ValueError):
            pass
    typ = get_proper_type(typ)
    if isinstance(typ, Instance):
        method = typ.type.get(name)
        if method and isinstance(method.node, (SYMBOL_FUNCBASE_TYPES, Decorator, Var)):
            if method.node.info:
                return not method.node.info.fullname.startswith(("builtins.", "typing."))
        return False
    if isinstance(typ, UnionType):
        if check_all:
            return all(custom_special_method(t, name, check_all) for t in typ.items)
        return any(custom_special_method(t, name) for t in typ.items)
    if isinstance(typ, TupleType):
        return custom_special_method(tuple_fallback(typ), name, check_all)
    if isinstance(typ, FunctionLike) and typ.is_type_obj():
        # Look up __method__ on the metaclass for class objects.
        return custom_special_method(typ.fallback, name, check_all)
    if isinstance(typ, TypeType) and isinstance(typ.item, Instance):
        if typ.item.type.metaclass_type:
            # Look up __method__ on the metaclass for class objects.
            return custom_special_method(typ.item.type.metaclass_type, name, check_all)
    if isinstance(typ, AnyType):
        # Avoid false positives in uncertain cases.
        return True
    # TODO: support other types (see ExpressionChecker.has_member())?
    return False


def _rust_separate_union_literals(
    t: UnionType,
) -> tuple[Sequence[LiteralType], Sequence[Type]] | None:
    """Rust partition of a union into literal vs non-literal items.

    Returns None to defer to Python when the wire cannot round-trip a shape.
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            result = _type_kernel.rust_separate_union_literals(_serialize_type(t))
        except (AssertionError, NotImplementedError):
            return None
        if result is None:
            # Rust could not classify an item (e.g. an alias target that may
            # resolve to a literal); Python's get_proper_type expansion handles
            # it, so defer the whole partition.
            return None
        literal_blobs, union_blobs = result
        literal_items: list[LiteralType] = []
        for blob in literal_blobs:
            decoded = _deserialize_type(bytes(blob))
            if not isinstance(decoded, LiteralType):
                return None
            literal_items.append(decoded)
        union_items: list[Type] = []
        for blob in union_blobs:
            if (decoded := _deserialize_type(bytes(blob))) is None:
                return None
            union_items.append(decoded)
        return literal_items, union_items
    return None


def separate_union_literals(t: UnionType) -> tuple[Sequence[LiteralType], Sequence[Type]]:
    """Separate literals from other members in a union type."""
    rust = _rust_separate_union_literals(t)
    if rust is not None:
        return rust
    literal_items = []
    union_items = []

    for item in t.items:
        proper = get_proper_type(item)
        if isinstance(proper, LiteralType):
            literal_items.append(proper)
        else:
            union_items.append(item)

    return literal_items, union_items


def try_getting_instance_fallback(typ: Type) -> Instance | None:
    """Returns the Instance fallback for this type if one exists or None."""
    # Fast paths for get_proper_type + the dispatch tail: raw
    # NoneType/AnyType decide None (the largest measured defer buckets)
    # and TypeGuardedType unwraps like types.py:4068.
    cls = type(typ)
    if cls is NoneType or cls is AnyType:
        return None
    if cls is TypeGuardedType:
        typ = cast(TypeGuardedType, typ).type_guard
    if _HAS_TYPE_KERNEL and _native_typeops_active and _native_typeops_resolver is not None:
        try:
            # Issue #1101 decided-None protocol: (True, blob) is the
            # fallback Instance bytes, (True, None) means the
            # `else: return None` tail decided natively, None defers.
            result = _type_kernel.rust_try_getting_instance_fallback(
                _serialize_type(typ), _native_typeops_resolver
            )
            if result is not None:
                decided, blob = result
                if blob is not None:
                    decoded = _deserialize_type(bytes(blob))
                    if isinstance(decoded, Instance):
                        return decoded
                elif decided:
                    return None
        except (AssertionError, NotImplementedError, ValueError):
            pass
    typ = get_proper_type(typ)
    if isinstance(typ, Instance):
        return typ
    elif isinstance(typ, LiteralType):
        return typ.fallback
    elif isinstance(typ, (NoneType, AnyType)):
        return None  # Fast path for None, which is common
    elif isinstance(typ, FunctionLike):
        return typ.fallback
    elif isinstance(typ, TypeVarType):
        return try_getting_instance_fallback(typ.upper_bound)
    elif isinstance(typ, TupleType):
        return typ.partial_fallback
    elif isinstance(typ, TypedDictType):
        return typ.fallback
    return None


def fixup_partial_type(typ: Type) -> Type:
    """Convert a partial type that we couldn't resolve into something concrete.

    This means, for None we make it Optional[Any], and for anything else we
    fill in all of the type arguments with Any.
    """
    if not isinstance(typ, PartialType):
        return typ
    if typ.type is None:
        return UnionType.make_union([AnyType(TypeOfAny.unannotated), NoneType()])
    else:
        return Instance(typ.type, [AnyType(TypeOfAny.unannotated)] * len(typ.type.type_vars))


def _is_disjoint_base(info: TypeInfo) -> bool:
    # It either has the @disjoint_base decorator or defines nonempty __slots__.
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            return _type_kernel.rust_is_disjoint_base(info)
        except (AssertionError, NotImplementedError):
            pass
    if info.is_disjoint_base:
        return True
    if not info.slots:
        return False
    own_slots = {
        slot
        for slot in info.slots
        if not any(
            base_info.type.slots is not None and slot in base_info.type.slots
            for base_info in info.bases
        )
    }
    return bool(own_slots)


def _get_disjoint_base_of(instance: Instance) -> TypeInfo | None:
    """Returns the disjoint base of the given instance, if it exists."""
    if _is_disjoint_base(instance.type):
        return instance.type
    for base in instance.type.mro:
        if _is_disjoint_base(base):
            return base
    return None


def can_have_shared_disjoint_base(instances: list[Instance]) -> bool:
    """Returns whether the given instances can share a disjoint base.

    This means that a child class of these classes can exist at runtime.
    """
    if _HAS_TYPE_KERNEL and _native_typeops_active:
        try:
            return _type_kernel.rust_can_have_shared_disjoint_base(instances)
        except (AssertionError, NotImplementedError):
            pass
    # Ignore None disjoint bases (which are `object`).
    disjoint_bases = [
        base for instance in instances if (base := _get_disjoint_base_of(instance)) is not None
    ]
    if not disjoint_bases:
        # All are `object`.
        return True

    candidate = disjoint_bases[0]
    for base in disjoint_bases[1:]:
        if candidate.has_base(base.fullname):
            continue
        elif base.has_base(candidate.fullname):
            candidate = base
        else:
            return False
    return True
