from __future__ import annotations

from collections.abc import Callable, Iterable, Iterator
from contextlib import contextmanager
from typing import Any, Final, TypeAlias as _TypeAlias, TypeVar, cast

import mypy.applytype
import mypy.constraints
import mypy.typeops
from mypy.checker_state import checker_state
from mypy.erasetype import erase_type
from mypy.expandtype import (
    expand_self_type,
    expand_type,
    expand_type_by_instance,
    freshen_function_type_vars,
)
from mypy.maptype import map_instance_to_supertype

# Circular import; done in the function instead.
# import mypy.solve
from mypy.nodes import (
    ARG_STAR,
    ARG_STAR2,
    CONTRAVARIANT,
    COVARIANT,
    INVARIANT,
    VARIANCE_NOT_READY,
    Context,
    Decorator,
    FakeInfo,
    FuncBase,
    OverloadedFuncDef,
    TypeInfo,
    Var,
)
from mypy.options import Options
from mypy.state import state
from mypy.types import (
    MYPYC_NATIVE_INT_NAMES,
    TUPLE_LIKE_INSTANCE_NAMES,
    TYPED_NAMEDTUPLE_NAMES,
    AnyType,
    CallableType,
    DeletedType,
    ErasedType,
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
    TypeOfAny,
    TypeType,
    TypeVarLikeType,
    TypeVarTupleType,
    TypeVarType,
    TypeVisitor,
    UnboundType,
    UninhabitedType,
    UnionType,
    UnpackType,
    _serialize_with_taint_check,
    _type_wire_cache,
    _wire_cache_enabled,
    find_unpack_in_list,
    flatten_nested_unions,
    get_proper_type,
    is_named_instance,
    split_with_prefix_and_suffix,
)
from mypy.types_utils import flatten_types
from mypy.typestate import SubtypeKind, type_state
from mypy.typevars import fill_typevars, fill_typevars_with_any

# Stage 3c (M8b) type-kernel seam: when type_kernel is importable,
# native_type_kernel is set, and a resolver is installed, the nominal
# is_subtype path routes through Rust. Rust returns None for unhandled.
try:
    import type_kernel as _type_kernel
    from librt.internal import ReadBuffer as _ReadBuffer, WriteBuffer as _WriteBuffer
    from type_kernel import (
        rust_are_args_compatible as _rust_are_args_compatible,
        rust_classify_find_member as _rust_classify_find_member,
        rust_classify_type_parameter as _rust_classify_type_parameter,
        rust_infer_variance_member as _rust_infer_variance_member,
    )

    from mypy.types import read_type as _read_type

    _HAS_TYPE_KERNEL = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _ReadBuffer = None  # type: ignore[assignment,misc]
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _read_type = None  # type: ignore[assignment]
    _rust_infer_variance_member = None  # type: ignore[assignment]
    _rust_are_args_compatible = None  # type: ignore[assignment]
    _rust_classify_find_member = None  # type: ignore[assignment]
    _rust_classify_type_parameter = None  # type: ignore[assignment]
    _HAS_TYPE_KERNEL = False

# Module-level flag + resolver, set by the build manager from
# `Options.native_type_kernel` at the start of each build. The hot path
# reads these without an options lookup per call. When `_native_subtype_active`

# is True but `_native_subtype_resolver` is None, the shim falls through to
# Python (the resolver isn't wired yet, e.g. in tests that only set the flag).
_native_subtype_active: bool = False
_native_subtype_resolver: Any = None


def _set_native_subtype_active(active: bool) -> None:
    """Called by the build manager to enable/disable the Rust subtype path.

    Also resets the batch accumulator: a stale buffer from a previous
    build must never answer a later build (per-build context and resolver
    differ).
    """
    global _native_subtype_active, _subtype_batch, _subtype_answers
    _native_subtype_active = active
    _subtype_batch = []
    _subtype_answers = {}


def _set_native_subtype_resolver(resolver: Any) -> None:
    """Install the `NativeTypeResolver` pyclass for the Rust subtype path.

    Called by the build manager (or the parity test suite) after building
    the resolver from the live TypeInfo graph. Pass `None` to clear.
    """
    global _native_subtype_resolver
    _native_subtype_resolver = resolver


def _native_variance_member_active() -> bool:
    """Guard for the `infer_variance` member seam."""
    return (
        _HAS_TYPE_KERNEL
        and _native_subtype_active
        and _native_subtype_resolver is not None
        and _rust_infer_variance_member is not None
    )


def _native_erase_return_self_types_active() -> bool:
    """Guard for the `erase_return_self_types` seam."""
    return _HAS_TYPE_KERNEL and _native_subtype_active


# Decision tags returned by `_rust_are_args_compatible`; must match the
# `KIND_ARE_ARGS_*` constants in crates/type_kernel/src/subtypes.rs.
NATIVE_ARE_ARGS_FALSE = 0
NATIVE_ARE_ARGS_TRUE = 1
NATIVE_ARE_ARGS_CALL_IS_COMPAT = 2

# Decision tags returned by `_rust_classify_find_member` (#1074); must
# match the TAG_* constants in crates/type_kernel/src/findmember.rs.
NATIVE_FIND_MEMBER_PROCEED = 0
NATIVE_FIND_MEMBER_ANY_SPECIAL_FORM = 1
NATIVE_FIND_MEMBER_EXTRA_ATTR = 2
NATIVE_FIND_MEMBER_NOT_FOUND = 3


def _native_find_member_prelude_active() -> bool:
    """Guard for the `find_member` name-resolution prelude seam."""
    return (
        _HAS_TYPE_KERNEL
        and _native_subtype_active
        and _native_subtype_resolver is not None
        and _rust_classify_find_member is not None
    )


def _native_are_args_compatible_active() -> bool:
    """Guard for the `are_args_compatible` dispatch seam."""
    return _HAS_TYPE_KERNEL and _native_subtype_active and _rust_are_args_compatible is not None


def _native_params_compat_nested_proper(
    is_compat: Callable[[Type, Type], bool], ignore_pos_arg_names: bool
) -> bool | None:
    """Nested proper-subtype flag when `is_compat` matches the kernel's fixed
    nested subtype semantics, or None to defer to the pure-Python body.

    The kernel's `are_parameters_compatible` re-enters subtype checks through
    `crate::subtypes::is_subtype` with a fixed SubtypeContext (only
    ignore_pos_arg_names / strict flags / the proper flag carried), so only
    resolver-backed callbacks with exactly those semantics may engage:
    module-level `is_subtype` / `is_proper_subtype` (default context), or
    `SubtypeVisitor._is_subtype` over a context whose other flags are all
    default. Everything else (erasure predicates, overlap checks, flipped
    compat closures, TypeChecker callbacks, test stubs) defers.
    """
    fn = getattr(is_compat, "__func__", is_compat)
    slf = getattr(is_compat, "__self__", None)
    if slf is None:
        # Module-level callbacks build a default SubtypeContext per call, so a
        # non-default top-level ignore_pos_arg_names would leak into nested
        # callable comparisons on the Rust side but not the Python side.
        if not ignore_pos_arg_names:
            if fn is is_subtype:
                return False
            if fn is is_proper_subtype:
                return True
        return None
    if fn is SubtypeVisitor._is_subtype and isinstance(slf, SubtypeVisitor):
        ctx = slf.subtype_context
        if (
            ctx.ignore_type_params
            or ctx.ignore_pos_arg_names != ignore_pos_arg_names
            or ctx.ignore_declared_variance
            or ctx.always_covariant
            or ctx.ignore_promotions
            or ctx.erase_instances
            or ctx.keep_erased_types
        ):
            return None
        return slf.proper_subtype
    return None


# Decision tags returned by `_rust_classify_type_parameter`; must match
# the `KIND_TYPEPARAM_*` constants in crates/type_kernel/src/subtypes.rs.
NATIVE_TYPEPARAM_SUBTYPE = 0
NATIVE_TYPEPARAM_SUBTYPE_SWAP = 1
NATIVE_TYPEPARAM_PROPER_SUBTYPE = 2
NATIVE_TYPEPARAM_PROPER_SWAP = 3
NATIVE_TYPEPARAM_SAME = 4
NATIVE_TYPEPARAM_EQUIVALENT = 5


def _native_type_parameter_active() -> bool:
    """Guard for the `check_type_parameter` dispatch seam."""
    return (
        _HAS_TYPE_KERNEL and _native_subtype_active and _rust_classify_type_parameter is not None
    )


def _native_infer_variance_member(
    typ: Type, self_type: Type, object_type: Instance, tvar: TypeVarLikeType
) -> int | None:
    """Run the Rust per-member `infer_variance` decision, or None to defer.

    Serializes `typ` (the `find_member` result), the self type, and the
    object Instance, plus the candidate typevar's raw_id. The Rust side
    applies the wire subset of `erase_return_self_types`, `expand_type`,
    and the two `is_subtype` calls and returns a co/contra flip bitmask.
    """
    if isinstance(typ, TypeAliasType):
        # The wire subset defers TypeAliasType; skip serialization cost.
        return None
    try:
        typ_bytes = _serialize_type(typ)
        self_bytes = _serialize_type(self_type)
        object_bytes = _serialize_type(object_type)
    except Exception:
        return None
    try:
        return _rust_infer_variance_member(
            typ_bytes, self_bytes, object_bytes, tvar.id.raw_id, _native_subtype_resolver
        )
    except Exception:
        return None


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
    buf = _WriteBuffer()
    result, saw_tvar = _serialize_with_taint_check(t, buf)
    if (
        not saw_tvar
        and _wire_cache_enabled()
        and (not isinstance(t, Instance) or t.type_ref is None)  # type: ignore[misc]
    ):
        _type_wire_cache[key] = (t, result)
    return result


# wire-bytes -> decoded+fixed Type cache for the subtype-seam decodes
# (171K calls, ~85% identical bytes); callers only read the result.
_subtype_decode_cache: dict[bytes, ProperType] = {}


def _clear_subtype_decode_cache() -> None:
    _subtype_decode_cache.clear()


def _deserialize_type(data: bytes) -> ProperType | None:
    """Deserialize wire bytes to a Type, fixing type_ref strings.

    Returns None if any type_ref cannot be resolved to a live TypeInfo
    (so the caller defers to Python). A non-None result is structurally a
    proper-type tree: fixup_wire_type defers on any TypeAliasType.
    """
    from mypy.types import instance_cache
    from mypy.wirefixup import fixup_wire_type

    cached = _subtype_decode_cache.get(data)
    if cached is not None:
        return cached
    decoded = _read_type(_ReadBuffer(data))
    instance_cache.int_type = None
    instance_cache.str_type = None
    instance_cache.bool_type = None
    instance_cache.object_type = None
    instance_cache.function_type = None
    fixed = fixup_wire_type(decoded)
    if fixed is not None:
        proper = cast(ProperType, fixed)
        _subtype_decode_cache[data] = proper
        return proper
    return fixed


def _restore_protocol_member_definition(left: Instance, member: str, decoded: ProperType) -> Type:
    """Restore ``definition`` links lost in the wire round-trip.

    The Rust seam returns a bound (self-stripped, ``is_bound``) FunctionLike
    matching ``bind_self`` in ``find_node_type``, but the wire format drops
    ``CallableType.definition``. Error rendering (``pretty_callable`` /
    ``get_first_arg``) needs the live ``FuncDef`` to re-add the ``self``
    parameter name, so copy definitions from the live symbol's ``node.type``
    (``_restore_definition`` mirrors the same idea).
    """
    from mypy.checkmember import _restore_definition
    from mypy.types import CallableType, FunctionLike, Overloaded

    if not isinstance(decoded, FunctionLike):
        return decoded
    sym = left.type.get(member)
    node = sym.node if sym else None
    if node is None:
        return decoded
    node_type = getattr(node, "type", None)
    if not isinstance(node_type, (CallableType, Overloaded)):
        return decoded
    return _restore_definition(node_type, decoded)


# Batch accumulator for the native `_is_subtype` path (measurement: ~171K
# calls, ~85K identical-pair repeats). Pairs are buffered as
# (left_bytes, right_bytes, ctx_key) and flushed in one

# `rust_is_subtype_batch` call once the threshold is reached, or when a
# new context key arrives (the batch call takes a single shared flag
# set). `_set_native_subtype_active` resets the buffer at each build

# start so stale pairs never answer a later build; only pairs that
# passed the exact single-pair gate are buffered, so a batched answer is
# semantically identical to the single-pair `rust_is_subtype` call.
_SUBTYPE_BATCH_THRESHOLD: Final = 512
_subtype_batch: list[tuple[bytes, bytes, tuple[bool, ...]]] = []
# Persistent answer cache keyed by (left_bytes, right_bytes, ctx_key):
# every pair Rust decided this build lands here, so an identical repeat
# (the ~85K) hits the dict instead of a fresh single-pair Rust call.
_subtype_answers: dict[tuple[bytes, bytes, tuple[bool, ...]], bool] = {}


def _clear_subtype_batch() -> None:
    """Drop buffered pairs (never flush-answer them) at build boundaries.

    Called alongside `_clear_subtype_decode_cache` on daemon recheck and
    manager reset: bytes serialized against a previous build's TypeInfo
    graph must not be answered against a later build's resolver.
    """
    global _subtype_batch, _subtype_answers
    _subtype_batch = []
    _subtype_answers = {}


def _strict_concatenate_flag(subtype_context: SubtypeContext) -> bool:
    """Same strict_concatenate computation the single-pair shim uses."""
    if subtype_context.options is None:
        return False
    return bool(subtype_context.options.extra_checks or subtype_context.options.strict_concatenate)


def _flush_subtype_batch() -> dict[tuple[bytes, bytes, tuple[bool, ...]], bool]:
    """Flush buffered pairs through `rust_is_subtype_batch`.

    Returns a dict mapping (left_bytes, right_bytes, ctx_key) -> decided
    bool for the pairs Rust answered; pairs Rust deferred (-1) are left
    out so the caller falls through to the single-pair path. Clears the
    buffer even when the Rust call raises (the caller re-runs those pairs
    singly). Also folds every decided answer into the build-global
    `_subtype_answers` so an identical later pair under the same flags is
    answered from the dict — except code 3 answers (true via a registry
    consult cut): those hold only for the derivation that took the cut,
    so they are returned to the caller but never persisted, mirroring
    Python's assuming-matrix hit which is likewise not cached.
    """
    global _subtype_batch, _subtype_answers
    if not _subtype_batch:
        return {}
    pairs = _subtype_batch
    _subtype_batch = []
    flat: list[bytes] = []
    for left_b, right_b, _ in pairs:
        flat.append(left_b)
        flat.append(right_b)
    # All buffered pairs share one ctx key (the accumulating caller only
    # buffers under the key it holds), so take the first entry's flags.
    _, _, ctx_key = pairs[0]
    (
        ignore_type_params,
        ignore_declared_variance,
        always_covariant,
        ignore_promotions,
        proper_subtype,
        strict_optional,
        ignore_pos_arg_names,
        strict_concatenate,
    ) = ctx_key
    try:
        answers = _type_kernel.rust_is_subtype_batch(
            flat,
            ignore_type_params,
            ignore_declared_variance,
            always_covariant,
            ignore_promotions,
            proper_subtype,
            strict_optional,
            ignore_pos_arg_names,
            strict_concatenate,
            _native_subtype_resolver,
        )
    except (AssertionError, NotImplementedError):
        # Unserializable variant reached the Rust edge; nothing was
        # answered, fall through per-pair.
        return {}
    result: dict[tuple[bytes, bytes, tuple[bool, ...]], bool] = {}
    cacheable: dict[tuple[bytes, bytes, tuple[bool, ...]], bool] = {}
    for (left_b, right_b, k), answer in zip(pairs, answers):
        if answer == 1:
            result[(left_b, right_b, k)] = True
            cacheable[(left_b, right_b, k)] = True
        elif answer == 0:
            result[(left_b, right_b, k)] = False
            cacheable[(left_b, right_b, k)] = False
        elif answer == 3:
            # True via a consult cut: valid only for this derivation,
            # returned to the caller but never persisted.
            result[(left_b, right_b, k)] = True
    _subtype_answers.update(cacheable)
    return result


# Flags for detected protocol members
IS_SETTABLE: Final = 1
IS_CLASSVAR: Final = 2
IS_CLASS_OR_STATIC: Final = 3
IS_VAR: Final = 4
IS_EXPLICIT_SETTER: Final = 5

TypeParameterChecker: _TypeAlias = Callable[[Type, Type, int, bool, "SubtypeContext"], bool]


class SubtypeContext:
    def __init__(
        self,
        *,
        # Non-proper subtype flags
        ignore_type_params: bool = False,
        ignore_pos_arg_names: bool = False,
        ignore_declared_variance: bool = False,
        # Supported for both proper and non-proper
        always_covariant: bool = False,
        ignore_promotions: bool = False,
        # Proper subtype flags
        erase_instances: bool = False,
        keep_erased_types: bool = False,
        options: Options | None = None,
    ) -> None:
        self.ignore_type_params = ignore_type_params
        self.ignore_pos_arg_names = ignore_pos_arg_names
        self.ignore_declared_variance = ignore_declared_variance
        self.always_covariant = always_covariant
        self.ignore_promotions = ignore_promotions
        self.erase_instances = erase_instances
        self.keep_erased_types = keep_erased_types
        self.options = options

    def check_context(self, proper_subtype: bool) -> None:
        # Historically proper and non-proper subtypes were defined using different helpers
        # and different visitors. Check if flag values are such that we definitely support.
        if proper_subtype:
            assert not self.ignore_pos_arg_names and not self.ignore_declared_variance
        else:
            assert not self.erase_instances and not self.keep_erased_types


def is_subtype(
    left: Type,
    right: Type,
    *,
    subtype_context: SubtypeContext | None = None,
    ignore_type_params: bool = False,
    ignore_pos_arg_names: bool = False,
    ignore_declared_variance: bool = False,
    always_covariant: bool = False,
    ignore_promotions: bool = False,
    options: Options | None = None,
) -> bool:
    """Is 'left' subtype of 'right'?

    Also consider Any to be a subtype of any type, and vice versa. This
    recursively applies to components of composite types (List[int] is subtype
    of List[Any], for example).

    type_parameter_checker is used to check the type parameters (for example,
    A with B in is_subtype(C[A], C[B]). The default checks for subtype relation
    between the type arguments (e.g., A and B), taking the variance of the
    type var into account.
    """
    if left == right:
        return True
    if subtype_context is None:
        subtype_context = SubtypeContext(
            ignore_type_params=ignore_type_params,
            ignore_pos_arg_names=ignore_pos_arg_names,
            ignore_declared_variance=ignore_declared_variance,
            always_covariant=always_covariant,
            ignore_promotions=ignore_promotions,
            options=options,
        )
    else:
        assert (
            not ignore_type_params
            and not ignore_pos_arg_names
            and not ignore_declared_variance
            and not always_covariant
            and not ignore_promotions
            and options is None
        ), "Don't pass both context and individual flags"
    if type_state.is_assumed_subtype(left, right):
        return True
    if mypy.typeops.is_recursive_pair(left, right):
        # This case requires special care because it may cause infinite recursion.
        # Our view on recursive types is known under a fancy name of iso-recursive mu-types.

        # Roughly this means that a recursive type is defined as an alias where right hand side
        # can refer to the type as a whole, for example:
        #     A = Union[int, Tuple[A, ...]]

        # and an alias unrolled once represents the *same type*, in our case all these represent
        # the same type:

        #    A
        #    Union[int, Tuple[A, ...]]
        #    Union[int, Tuple[Union[int, Tuple[A, ...]], ...]]

        # The algorithm for subtyping is then essentially under the assumption that
        # left <: right,
        # check that get_proper_type(left) <: get_proper_type(right). On the example

        # above,

        # If we start with:
        #     A = Union[int, Tuple[A, ...]]
        #     B = Union[int, Tuple[B, ...]]

        # When checking if A <: B we push pair (A, B) onto 'assuming' stack, then when
        # after few steps we come back to initial call is_subtype(A, B) and
        # immediately return True.
        with pop_on_exit(type_state.get_assumptions(is_proper=False), left, right):
            return _is_subtype(left, right, subtype_context, proper_subtype=False)
    return _is_subtype(left, right, subtype_context, proper_subtype=False)


def is_proper_subtype(
    left: Type,
    right: Type,
    *,
    subtype_context: SubtypeContext | None = None,
    ignore_promotions: bool = False,
    erase_instances: bool = False,
    keep_erased_types: bool = False,
) -> bool:
    """Is left a proper subtype of right?

    For proper subtypes, there's no need to rely on compatibility due to
    Any types. Every usable type is a proper subtype of itself.

    If erase_instances is True, erase left instance *after* mapping it to supertype
    (this is useful for runtime isinstance() checks). If keep_erased_types is True,
    do not consider ErasedType a subtype of all types (used by type inference against unions).
    """
    if left == right:
        return True
    if subtype_context is None:
        subtype_context = SubtypeContext(
            ignore_promotions=ignore_promotions,
            erase_instances=erase_instances,
            keep_erased_types=keep_erased_types,
        )
    else:
        assert not ignore_promotions and not erase_instances and not keep_erased_types, (
            "Don't pass both context and individual flags"
        )
    if type_state.is_assumed_proper_subtype(left, right):
        return True
    if mypy.typeops.is_recursive_pair(left, right):
        # Same as for non-proper subtype, see detailed comment there for explanation.
        with pop_on_exit(type_state.get_assumptions(is_proper=True), left, right):
            return _is_subtype(left, right, subtype_context, proper_subtype=True)
    return _is_subtype(left, right, subtype_context, proper_subtype=True)


def is_equivalent(
    a: Type,
    b: Type,
    *,
    ignore_type_params: bool = False,
    ignore_pos_arg_names: bool = False,
    options: Options | None = None,
    subtype_context: SubtypeContext | None = None,
) -> bool:
    if (
        _HAS_TYPE_KERNEL
        and _native_subtype_active
        and _native_subtype_resolver is not None
        and subtype_context is None
    ):
        try:
            result = _type_kernel.rust_is_equivalent(
                _serialize_type(a),
                _serialize_type(b),
                ignore_type_params,
                state.strict_optional,
                _native_subtype_resolver,
            )
        except (AssertionError, NotImplementedError):
            result = None
        if result is not None:
            return result
    return is_subtype(
        a,
        b,
        ignore_type_params=ignore_type_params,
        ignore_pos_arg_names=ignore_pos_arg_names,
        options=options,
        subtype_context=subtype_context,
    ) and is_subtype(
        b,
        a,
        ignore_type_params=ignore_type_params,
        ignore_pos_arg_names=ignore_pos_arg_names,
        options=options,
        subtype_context=subtype_context,
    )


def is_same_type(
    a: Type, b: Type, ignore_promotions: bool = True, subtype_context: SubtypeContext | None = None
) -> bool:
    """Are these types proper subtypes of each other?

    This means types may have different representation (e.g. an alias, or
    a non-simplified union) but are semantically exchangeable in all contexts.
    """
    # First, use fast path for some common types. This is performance-critical.
    if (
        type(a) is Instance
        and type(b) is Instance
        and a.type == b.type
        and len(a.args) == len(b.args)
        and a.last_known_value is b.last_known_value
    ):
        return all(is_same_type(x, y) for x, y in zip(a.args, b.args))
    elif (
        isinstance(a, TypeVarType)
        and isinstance(b, TypeVarType)
        and a.id == b.id
        and a.upper_bound == b.upper_bound
    ):
        return True

    if (
        _HAS_TYPE_KERNEL
        and _native_subtype_active
        and _native_subtype_resolver is not None
        and subtype_context is None
    ):
        try:
            result = _type_kernel.rust_is_same_type(
                _serialize_type(a),
                _serialize_type(b),
                ignore_promotions,
                state.strict_optional,
                _native_subtype_resolver,
            )
        except (AssertionError, NotImplementedError):
            result = None
        if result is not None:
            return result

    # Note that using ignore_promotions=True (default) makes types like int and int64
    # considered not the same type (which is the case at runtime).

    # Also Union[bool, int] (if it wasn't simplified before) will be different
    # from plain int, etc.
    return is_proper_subtype(
        a, b, ignore_promotions=ignore_promotions, subtype_context=subtype_context
    ) and is_proper_subtype(
        b, a, ignore_promotions=ignore_promotions, subtype_context=subtype_context
    )


# This is a common entry point for subtyping checks (both proper and non-proper).
# Never call this private function directly, use the public versions.
def _is_subtype(
    left: Type, right: Type, subtype_context: SubtypeContext, proper_subtype: bool
) -> bool:
    subtype_context.check_context(proper_subtype)
    orig_right = right
    orig_left = left
    left = get_proper_type(left)
    right = get_proper_type(right)

    # Fast paths for trivial cases that skip serialization and the
    # visitor entirely. Safe for both proper and non-proper subtype.
    if left is right:
        return True
    if isinstance(left, Instance) and isinstance(right, Instance):
        if not left.args and not right.args:
            if left.type.fullname == right.type.fullname:
                return True
            if right.type.fullname == "builtins.object":
                return True
    elif isinstance(left, NoneType) and isinstance(right, NoneType):
        return True

    # Note: Unpack type should not be a subtype of Any, since it may represent
    # multiple types. This should always go through the visitor, to check arity.
    if (
        not proper_subtype
        and isinstance(right, (AnyType, UnboundType, ErasedType))
        and not isinstance(left, UnpackType)
    ):
        # TODO: should we consider all types proper subtypes of UnboundType and/or
        # ErasedType as we do for non-proper subtyping.
        return True

    # Cases specific w.r.t. right type are easier to handle before entering the
    # SubtypeVisitor.
    # Currently, these include Union types and TypeVarType with values.
    if isinstance(right, UnionType) and not isinstance(left, UnionType):
        # Normally, when 'left' is not itself a union, the only way
        # 'left' can be a subtype of the union 'right' is if it is a
        # subtype of one of the items making up the union.
        if proper_subtype:
            is_subtype_of_item = any(
                is_proper_subtype(orig_left, item, subtype_context=subtype_context)
                for item in right.items
            )
        else:
            is_subtype_of_item = any(
                is_subtype(orig_left, item, subtype_context=subtype_context)
                for item in right.items
            )
        # Recombine rhs literal types, to make an enum type a subtype
        # of a union of all enum items as literal types. Only do it if
        # the previous check didn't succeed, since recombining can be

        # expensive.
        # `bool` is a special case, because `bool` is `Literal[True, False]`.
        if (
            not is_subtype_of_item
            and isinstance(left, Instance)
            and (left.type.is_enum or left.type.fullname == "builtins.bool")
        ):
            right = UnionType(
                mypy.typeops.try_contracting_literals_in_union(flatten_nested_unions(right.items))
            )
            if proper_subtype:
                is_subtype_of_item = any(
                    is_proper_subtype(orig_left, item, subtype_context=subtype_context)
                    for item in right.items
                )
            else:
                is_subtype_of_item = any(
                    is_subtype(orig_left, item, subtype_context=subtype_context)
                    for item in right.items
                )
        # However, if 'left' is a type variable T, T might also have
        # an upper bound which is itself a union. This case will be
        # handled below by the SubtypeVisitor. We have to check both

        # possibilities, to handle both cases like T <: Union[T, U]
        # and cases like T <: B where B is the upper bound of T and is
        # a union. (See #2314.)
        if not isinstance(left, TypeVarType):
            return is_subtype_of_item
        elif is_subtype_of_item:
            return True
        # otherwise, fall through

    if isinstance(right, TypeVarType) and right.values and not isinstance(left, TypeVarType):
        if proper_subtype:
            if all(
                is_proper_subtype(orig_left, v, subtype_context=subtype_context)
                for v in right.values
            ):
                return True
        elif all(is_subtype(orig_left, v, subtype_context=subtype_context) for v in right.values):
            return True

    # Stage 3c (M8b): try the Rust nominal-instance path. By this point
    # `left`/`right` are proper types and the AnyType/UnionType/
    # TypeVarType-with-values right short-circuits have fired, matching

    # the Rust `is_subtype` contract. Rust returns `None` for any case
    # it does not handle (generics needing `expand_type_by_instance`,
    # protocols, tuples, callables, etc.); we then fall through to

    # `SubtypeVisitor`. Mirrors `erasetype.py:80-86`.
    if (
        _HAS_TYPE_KERNEL
        and _native_subtype_active
        and _native_subtype_resolver is not None
        and not subtype_context.erase_instances
        and not subtype_context.keep_erased_types
        and not isinstance(left, ErasedType)
        and not isinstance(right, ErasedType)
    ):
        try:
            left_bytes = _serialize_type(left)
            right_bytes = _serialize_type(right)
        except (AssertionError, NotImplementedError):
            left_bytes = None
            right_bytes = None
        if left_bytes is not None and right_bytes is not None:
            # Batch-accumulator path: queue the pair and flush when the
            # threshold is reached or the flag set changes. Each pair is
            # answered by `rust_is_subtype_batch` with exactly the flags

            # the single-pair call below would use; the batched answer is
            # identical to the single-pair answer. A also-buffered
            # identical pair is answered from the same dict (dedup).
            ctx_key: tuple[bool, ...] = (
                subtype_context.ignore_type_params,
                subtype_context.ignore_declared_variance,
                subtype_context.always_covariant,
                subtype_context.ignore_promotions,
                proper_subtype,
                state.strict_optional,
                subtype_context.ignore_pos_arg_names,
                _strict_concatenate_flag(subtype_context),
            )
            if _subtype_batch:
                prev_flags = _subtype_batch[0][2]
                if prev_flags != ctx_key:
                    # The buffered pairs use a different flag set than this
                    # call; flush them first so this pair's buffer slots
                    # start fresh. The batch call takes one shared flag

                    # set, so pairs can only be batched under one key;
                    # the just-flushed dict holds other-flag answers only.
                    _flush_subtype_batch()
            if (left_bytes, right_bytes, ctx_key) in _subtype_answers:
                # Decided this build under this exact flag set; identical
                # bytes under identical flags give the identical answer
                # (the cache is reset at build boundaries).
                return _subtype_answers[(left_bytes, right_bytes, ctx_key)]
            _subtype_batch.append((left_bytes, right_bytes, ctx_key))
            if len(_subtype_batch) < _SUBTYPE_BATCH_THRESHOLD:
                # Not flushed yet: the current pair's answer is not known;
                # fall through to the single-pair path for THIS call so the
                # caller still gets a bool.

                # The buffered duplicate is what the next identical call
                # will look up.
                pass
            else:
                answers = _flush_subtype_batch()
                if (left_bytes, right_bytes, ctx_key) in answers:
                    return answers[(left_bytes, right_bytes, ctx_key)]
        # Verify the still-serializable assertions and run the single-pair
        # path when the batch path did not decide (deferred pair, or the
        # buffer did not include this pair).
        try:
            result = _type_kernel.rust_is_subtype(
                _serialize_type(left),
                _serialize_type(right),
                subtype_context.ignore_type_params,
                subtype_context.ignore_declared_variance,
                subtype_context.always_covariant,
                subtype_context.ignore_promotions,
                proper_subtype,
                state.strict_optional,
                subtype_context.ignore_pos_arg_names,
                (
                    (
                        subtype_context.options.extra_checks
                        or subtype_context.options.strict_concatenate
                    )
                    if subtype_context.options
                    else False
                ),
                _native_subtype_resolver,
            )
        except (AssertionError, NotImplementedError):
            # Type tree contains an unserializable variant (e.g.
            # TypeGuardedType nested in a Union). Defer to Python.
            result = None
        if result is not None:
            return result
    return left.accept(SubtypeVisitor(orig_right, subtype_context, proper_subtype))


def check_type_parameter(
    left: Type, right: Type, variance: int, proper_subtype: bool, subtype_context: SubtypeContext
) -> bool:
    # Issue #998: Rust classifies the variance-dispatch head (including
    # the ambiguous-Uninhabited invariant-to-covariant upgrade) into a
    # leaf tag; Python applies the leaf call. None defers to the body.
    if _native_type_parameter_active():
        tag = _rust_classify_type_parameter(left, variance, proper_subtype)
        if tag == NATIVE_TYPEPARAM_SUBTYPE:
            return is_subtype(left, right, subtype_context=subtype_context)
        if tag == NATIVE_TYPEPARAM_SUBTYPE_SWAP:
            return is_subtype(right, left, subtype_context=subtype_context)
        if tag == NATIVE_TYPEPARAM_PROPER_SUBTYPE:
            return is_proper_subtype(left, right, subtype_context=subtype_context)
        if tag == NATIVE_TYPEPARAM_PROPER_SWAP:
            return is_proper_subtype(right, left, subtype_context=subtype_context)
        if tag == NATIVE_TYPEPARAM_SAME:
            return is_same_type(
                left, right, ignore_promotions=False, subtype_context=subtype_context
            )
        if tag == NATIVE_TYPEPARAM_EQUIVALENT:
            return is_equivalent(left, right, subtype_context=subtype_context)
    # It is safe to consider empty collection literals and similar as covariant, since
    # such type can't be stored in a variable, see checker.is_valid_inferred_type().
    if variance == INVARIANT:
        p_left = get_proper_type(left)
        if isinstance(p_left, UninhabitedType) and p_left.ambiguous:
            variance = COVARIANT
    # If variance hasn't been inferred yet, we are lenient and default to
    # covariance. This shouldn't happen often, but it's very difficult to
    # avoid these cases altogether.
    if variance == COVARIANT or variance == VARIANCE_NOT_READY:
        if proper_subtype:
            return is_proper_subtype(left, right, subtype_context=subtype_context)
        else:
            return is_subtype(left, right, subtype_context=subtype_context)
    elif variance == CONTRAVARIANT:
        if proper_subtype:
            return is_proper_subtype(right, left, subtype_context=subtype_context)
        else:
            return is_subtype(right, left, subtype_context=subtype_context)
    else:
        if proper_subtype:
            # We pass ignore_promotions=False because it is a default for subtype checks.
            # The actual value will be taken from the subtype_context, and it is whatever
            # the original caller passed.
            return is_same_type(
                left, right, ignore_promotions=False, subtype_context=subtype_context
            )
        else:
            return is_equivalent(left, right, subtype_context=subtype_context)


class SubtypeVisitor(TypeVisitor[bool]):
    __slots__ = (
        "right",
        "orig_right",
        "proper_subtype",
        "subtype_context",
        "options",
        "_subtype_kind",
    )

    def __init__(self, right: Type, subtype_context: SubtypeContext, proper_subtype: bool) -> None:
        self.right = get_proper_type(right)
        self.orig_right = right
        self.proper_subtype = proper_subtype
        self.subtype_context = subtype_context
        self.options = subtype_context.options
        self._subtype_kind = SubtypeVisitor.build_subtype_kind(subtype_context, proper_subtype)

    @staticmethod
    def build_subtype_kind(subtype_context: SubtypeContext, proper_subtype: bool) -> SubtypeKind:
        return (
            state.strict_optional,
            proper_subtype,
            subtype_context.ignore_type_params,
            subtype_context.ignore_pos_arg_names,
            subtype_context.ignore_declared_variance,
            subtype_context.always_covariant,
            subtype_context.ignore_promotions,
            subtype_context.erase_instances,
            subtype_context.keep_erased_types,
        )

    def _is_subtype(self, left: Type, right: Type) -> bool:
        if self.proper_subtype:
            return is_proper_subtype(left, right, subtype_context=self.subtype_context)
        return is_subtype(left, right, subtype_context=self.subtype_context)

    def _all_subtypes(self, lefts: Iterable[Type], rights: Iterable[Type]) -> bool:
        return all(self._is_subtype(li, ri) for (li, ri) in zip(lefts, rights))

    # visit_x(left) means: is left (which is an instance of X) a subtype of right?

    def visit_unbound_type(self, left: UnboundType) -> bool:
        # This can be called if there is a bad type annotation. The result probably
        # doesn't matter much but by returning True we simplify these bad types away
        # from unions, which could filter out some bogus messages.
        return True

    def visit_any(self, left: AnyType) -> bool:
        return isinstance(self.right, AnyType) if self.proper_subtype else True

    def visit_none_type(self, left: NoneType) -> bool:
        if state.strict_optional:
            if isinstance(self.right, NoneType) or is_named_instance(
                self.right, "builtins.object"
            ):
                return True
            if isinstance(self.right, Instance) and self.right.type.is_protocol:
                members = self.right.type.protocol_members
                # None is compatible with Hashable (and other similar protocols). This is
                # slightly sloppy since we don't check the signature of "__hash__".
                # None is also compatible with `SupportsStr` protocol.
                return not members or all(member in ("__hash__", "__str__") for member in members)
            return False
        else:
            return True

    def visit_uninhabited_type(self, left: UninhabitedType) -> bool:
        return True

    def visit_erased_type(self, left: ErasedType) -> bool:
        # This may be encountered during type inference. The result probably doesn't
        # matter much.
        # TODO: it actually does matter, figure out more principled logic about this.
        return not self.subtype_context.keep_erased_types

    def visit_deleted_type(self, left: DeletedType) -> bool:
        return True

    def visit_instance(self, left: Instance) -> bool:
        if type(left.type) is FakeInfo:
            # Wire NOT_READY instances (FakeInfo .type) must never crash
            # subtyping: treat as non-subtype and defer to the caller.
            return True
        if left.type.fallback_to_any and not self.proper_subtype:
            # NOTE: `None` is a *non-subclassable* singleton, therefore no class
            # can by a subtype of it, even with an `Any` fallback.

            # This special case is needed to treat descriptors in classes with
            # dynamic base classes correctly, see #5456.
            return not isinstance(self.right, NoneType)
        right = self.right
        if isinstance(right, TupleType) and right.partial_fallback.type.is_enum:
            return self._is_subtype(left, mypy.typeops.tuple_fallback(right))
        if isinstance(right, TupleType):
            if len(right.items) == 1:
                # Non-normalized Tuple type (may be left after semantic analysis
                # because semanal_typearg visitor is not a type translator).
                item = right.items[0]
                if isinstance(item, UnpackType):
                    unpacked = get_proper_type(item.type)
                    if isinstance(unpacked, Instance):
                        return self._is_subtype(left, unpacked)
            if left.type.has_base(right.partial_fallback.type.fullname):
                if not self.proper_subtype:
                    # Special cases to consider:
                    #   * Plain tuple[Any, ...] instance is a subtype of all tuple types.

                    #   * Foo[*tuple[Any, ...]] (normalized) instance is a subtype of all
                    #     tuples with fallback to Foo (e.g. for variadic NamedTuples).
                    mapped = map_instance_to_supertype(left, right.partial_fallback.type)
                    if is_erased_instance(mapped):
                        if (
                            mapped.type.fullname == "builtins.tuple"
                            or mapped.type.has_type_var_tuple_type
                        ):
                            return True
            return False
        if isinstance(right, TypeVarTupleType):
            # tuple[Any, ...] is like Any in the world of tuples (see special case above).
            if left.type.has_base("builtins.tuple"):
                mapped = map_instance_to_supertype(left, right.tuple_fallback.type)
                if isinstance(get_proper_type(mapped.args[0]), AnyType):
                    return not self.proper_subtype
        if isinstance(right, Instance):
            if type(right.type) is FakeInfo:
                # Conservative: never run cached/hashing code paths on an
                # instance whose .type is a wire NOT_READY placeholder.
                return False
            if type_state.is_cached_subtype_check(self._subtype_kind, left, right):
                return True
            if type_state.is_cached_negative_subtype_check(self._subtype_kind, left, right):
                return False
            if not self.subtype_context.ignore_promotions and not right.type.is_protocol:
                for base in left.type.mro:
                    if base._promote and any(
                        self._is_subtype(p, self.right) for p in base._promote
                    ):
                        type_state.record_subtype_cache_entry(self._subtype_kind, left, right)
                        return True
                # Special case: Low-level integer types are compatible with 'int'. We can't
                # use promotions, since 'int' is already promoted to low-level integer types,
                # and we can't have circular promotions.
                if left.type.alt_promote and left.type.alt_promote.type is right.type:
                    return True
            rname = right.type.fullname
            # Always try a nominal check if possible,
            # there might be errors that a user wants to silence *once*.

            # NamedTuples are a special case, because `NamedTuple` is not listed
            # in `TypeInfo.mro`, so when `(a: NamedTuple) -> None` is used,
            # we need to check for `is_named_tuple` property
            if (
                left.type.has_base(rname)
                or rname == "builtins.object"
                or (
                    rname in TYPED_NAMEDTUPLE_NAMES
                    and any(l.is_named_tuple for l in left.type.mro)
                )
            ) and not self.subtype_context.ignore_declared_variance:
                # Map left type to corresponding right instances.
                t = map_instance_to_supertype(left, right.type)
                if self.subtype_context.erase_instances:
                    erased = erase_type(t)
                    assert isinstance(erased, Instance)
                    t = erased
                nominal = True
                if right.type.has_type_var_tuple_type:
                    # For variadic instances we simply find the correct type argument mappings,
                    # all the heavy lifting is done by the tuple subtyping.
                    assert right.type.type_var_tuple_prefix is not None
                    assert right.type.type_var_tuple_suffix is not None
                    prefix = right.type.type_var_tuple_prefix
                    suffix = right.type.type_var_tuple_suffix
                    tvt = right.type.defn.type_vars[prefix]
                    assert isinstance(tvt, TypeVarTupleType)
                    fallback = tvt.tuple_fallback
                    left_prefix, left_middle, left_suffix = split_with_prefix_and_suffix(
                        t.args, prefix, suffix
                    )
                    right_prefix, right_middle, right_suffix = split_with_prefix_and_suffix(
                        right.args, prefix, suffix
                    )
                    left_args = (
                        left_prefix + (TupleType(list(left_middle), fallback),) + left_suffix
                    )
                    right_args = (
                        right_prefix + (TupleType(list(right_middle), fallback),) + right_suffix
                    )
                    if not self.proper_subtype and is_erased_instance(t):
                        return True
                    if len(left_args) != len(right_args):
                        return False
                    type_params = zip(left_args, right_args, right.type.defn.type_vars)
                else:
                    type_params = zip(t.args, right.args, right.type.defn.type_vars)
                if not self.subtype_context.ignore_type_params:
                    tried_infer = False
                    for lefta, righta, tvar in type_params:
                        if isinstance(tvar, TypeVarType):
                            if tvar.variance == VARIANCE_NOT_READY and not tried_infer:
                                infer_class_variances(right.type)
                                tried_infer = True
                            if (
                                self.subtype_context.always_covariant
                                and tvar.variance == INVARIANT
                            ):
                                variance = COVARIANT
                            else:
                                variance = tvar.variance
                            if not check_type_parameter(
                                lefta, righta, variance, self.proper_subtype, self.subtype_context
                            ):
                                nominal = False
                        else:
                            # TODO: everywhere else ParamSpecs are handled as invariant.
                            if not check_type_parameter(
                                lefta, righta, COVARIANT, self.proper_subtype, self.subtype_context
                            ):
                                nominal = False
                if nominal:
                    type_state.record_subtype_cache_entry(self._subtype_kind, left, right)
                else:
                    type_state.record_negative_subtype_cache_entry(self._subtype_kind, left, right)
                return nominal
            if right.type.is_protocol and is_protocol_implementation(
                left, right, proper_subtype=self.proper_subtype, options=self.options
            ):
                return True
            # We record negative cache entry here, and not in the protocol check like we do for
            # positive cache, to avoid accidentally adding a type that is not a structural
            # subtype, but is a nominal subtype (involving type: ignore override).
            type_state.record_negative_subtype_cache_entry(self._subtype_kind, left, right)
            return False
        if isinstance(right, TypeType):
            item = right.item
            if isinstance(item, TupleType):
                item = mypy.typeops.tuple_fallback(item)
            # TODO: this is a bit arbitrary, we should only skip Any-related cases.
            if not self.proper_subtype:
                if is_named_instance(left, "builtins.type"):
                    return self._is_subtype(TypeType(AnyType(TypeOfAny.special_form)), right)
                if left.type.is_metaclass():
                    if isinstance(item, AnyType):
                        return True
                    if isinstance(item, Instance):
                        return is_named_instance(item, "builtins.object")
        if isinstance(right, LiteralType) and left.last_known_value is not None:
            return self._is_subtype(left.last_known_value, right)
        if isinstance(right, FunctionLike):
            # Special case: Instance can be a subtype of Callable / Overloaded.
            call = find_member("__call__", left, left, is_operator=True)
            if call:
                return self._is_subtype(call, right)
            return False
        else:
            return False

    def visit_type_var(self, left: TypeVarType) -> bool:
        right = self.right
        if isinstance(right, TypeVarType) and left.id == right.id:
            # Fast path for most common case.
            if left.upper_bound == right.upper_bound:
                return True
            # Corner case for self-types in classes generic in type vars
            # with value restrictions.
            if left.id.is_self():
                return True
            return self._is_subtype(left.upper_bound, right.upper_bound)
        if left.values and self._is_subtype(UnionType.make_union(left.values), right):
            return True
        return self._is_subtype(left.upper_bound, self.right)

    def visit_param_spec(self, left: ParamSpecType) -> bool:
        right = self.right
        if (
            isinstance(right, ParamSpecType)
            and right.id == left.id
            and right.flavor == left.flavor
        ):
            return self._is_subtype(left.prefix, right.prefix)
        if isinstance(right, Parameters) and are_trivial_parameters(right):
            return True
        return self._is_subtype(left.upper_bound, self.right)

    def visit_type_var_tuple(self, left: TypeVarTupleType) -> bool:
        right = self.right
        if isinstance(right, TypeVarTupleType) and right.id == left.id:
            return left.min_len >= right.min_len
        return self._is_subtype(left.upper_bound, self.right)

    def visit_unpack_type(self, left: UnpackType) -> bool:
        # TODO: Ideally we should not need this (since it is not a real type).
        # Instead callers (upper level types) should handle it when it appears in type list.
        if isinstance(self.right, UnpackType):
            return self._is_subtype(left.type, self.right.type)
        if isinstance(self.right, Instance) and self.right.type.fullname == "builtins.object":
            return True
        return False

    def visit_parameters(self, left: Parameters) -> bool:
        if isinstance(self.right, Parameters):
            # Native `are_parameters_compatible` seam (Stage 3c/M8c). Returns
            # None for shapes the engine cannot decide (generic parameters,
            # meet_types merge, nested is_subtype deferral); else Python fallback.
            if (
                _HAS_TYPE_KERNEL
                and _native_subtype_active
                and _native_subtype_resolver is not None
            ):
                try:
                    result = _type_kernel.rust_are_parameters_compatible(
                        _serialize_type(left),
                        _serialize_type(self.right),
                        False,  # is_proper_subtype
                        self.subtype_context.ignore_pos_arg_names,
                        False,  # allow_partial_overlap
                        False,  # strict_concatenate_check
                        state.strict_optional,
                        self.proper_subtype,  # nested comparisons
                        _native_subtype_resolver,
                    )
                except (AssertionError, NotImplementedError):
                    result = None
                if result is not None:
                    return result
            return are_parameters_compatible(
                left,
                self.right,
                is_compat=self._is_subtype,
                # TODO: this should pass the current value, but then couple tests fail.
                is_proper_subtype=False,
                ignore_pos_arg_names=self.subtype_context.ignore_pos_arg_names,
            )
        elif isinstance(self.right, Instance):
            return self.right.type.fullname == "builtins.object"
        else:
            return False

    def visit_callable_type(self, left: CallableType) -> bool:
        right = self.right
        if isinstance(right, CallableType):
            if left.type_guard is not None and right.type_guard is not None:
                if not self._is_subtype(left.type_guard, right.type_guard):
                    return False
            elif left.type_is is not None and right.type_is is not None:
                # For TypeIs we have to check both ways; it is unsafe to pass
                # a TypeIs[Child] when a TypeIs[Parent] is expected, because
                # if the narrower returns False, we assume that the narrowed value is

                # *not* a Parent.
                if not self._is_subtype(left.type_is, right.type_is) or not self._is_subtype(
                    right.type_is, left.type_is
                ):
                    return False
            elif right.type_guard is not None and left.type_guard is None:
                # This means that one function has `TypeGuard` and other does not.
                # They are not compatible. See https://github.com/python/mypy/issues/11307
                return False
            elif right.type_is is not None and left.type_is is None:
                # Similarly, if one function has `TypeIs` and the other does not,
                # they are not compatible.
                return False
            # Native callable-compat engine (Stage 3c/M8c). The kernel has no
            # constraint solver, so the is_callable_compatible prelude runs
            # here (#1279); unhandled shapes defer to the Python engine below.
            norm_left = left.with_unpacked_kwargs().with_normalized_var_args()
            norm_right = right.with_unpacked_kwargs().with_normalized_var_args()
            ignore_pos = self.subtype_context.ignore_pos_arg_names or (
                norm_left.implicit or norm_right.implicit
            )
            if (
                _HAS_TYPE_KERNEL
                and _native_subtype_active
                and _native_subtype_resolver is not None
            ):
                if norm_right.is_type_obj() and not norm_left.is_type_obj():
                    return False
                if norm_left.variables:
                    unified = unify_generic_callable(norm_left, norm_right, ignore_return=False)
                    if unified is None:
                        return False
                    # Kernel gates on left.variables but never reads them;
                    # dropping them skips its unify defer.
                    norm_left = unified.copy_modified(variables=[])
                try:
                    result = _type_kernel.rust_callables_compatible(
                        _serialize_type(norm_left),
                        _serialize_type(norm_right),
                        self.proper_subtype,
                        ignore_pos,
                        (
                            (self.options.extra_checks or self.options.strict_concatenate)
                            if self.options
                            else False
                        ),
                        state.strict_optional,
                        _native_subtype_resolver,
                    )
                except (AssertionError, NotImplementedError):
                    result = None
                if result is not None:
                    return result
                # The shim already unified norm_left, so pass the normalized
                # pair below; re-unifying the original left would allocate a
                # second round of fresh tvar ids and shift their numbering.
            return is_callable_compatible(
                norm_left,
                norm_right,
                is_compat=self._is_subtype,
                is_proper_subtype=self.proper_subtype,
                ignore_pos_arg_names=ignore_pos,
                strict_concatenate=(
                    (self.options.extra_checks or self.options.strict_concatenate)
                    if self.options
                    else False
                ),
            )
        elif isinstance(right, Overloaded):
            return all(self._is_subtype(left, item) for item in right.items)
        elif isinstance(right, Instance):
            if right.type.is_protocol and "__call__" in right.type.protocol_members:
                # OK, a callable can implement a protocol with a `__call__` member.
                call = find_member("__call__", right, right, is_operator=True)
                assert call is not None
                if self._is_subtype(left, call):
                    if len(right.type.protocol_members) == 1:
                        return True
                    if is_protocol_implementation(left.fallback, right, skip=["__call__"]):
                        return True
            if right.type.is_protocol and left.is_type_obj():
                instance_type = left.get_instance_type(force_fallback=True)
                if isinstance(instance_type, Instance) and is_protocol_implementation(
                    instance_type, right, proper_subtype=self.proper_subtype, class_obj=True
                ):
                    return True
            return self._is_subtype(left.fallback, right)
        elif isinstance(right, TypeType):
            # This is unsound, we don't check the __init__ signature.
            return left.is_type_obj() and self._is_subtype(left.get_instance_type(), right.item)
        else:
            return False

    def visit_tuple_type(self, left: TupleType) -> bool:
        right = self.right
        if isinstance(right, Instance):
            if is_named_instance(right, "typing.Sized"):
                return True
            elif is_named_instance(right, TUPLE_LIKE_INSTANCE_NAMES):
                if right.args:
                    iter_type = right.args[0]
                else:
                    if self.proper_subtype:
                        return False
                    iter_type = AnyType(TypeOfAny.special_form)
                if is_named_instance(right, "builtins.tuple") and isinstance(
                    get_proper_type(iter_type), AnyType
                ):
                    # TODO: We shouldn't need this special case. This is currently needed
                    #       for isinstance(x, tuple), though it's unclear why.
                    return True
                for li in left.items:
                    if isinstance(li, UnpackType):
                        unpack = get_proper_type(li.type)
                        if isinstance(unpack, TypeVarTupleType):
                            unpack = get_proper_type(unpack.upper_bound)
                        assert (
                            isinstance(unpack, Instance)
                            and unpack.type.fullname == "builtins.tuple"
                        )
                        li = unpack.args[0]
                    if not self._is_subtype(li, iter_type):
                        return False
                return True
            elif self._is_subtype(left.partial_fallback, right) and self._is_subtype(
                mypy.typeops.tuple_fallback(left), right
            ):
                return True
            elif right.type.is_protocol and is_protocol_implementation(
                left, right, proper_subtype=self.proper_subtype
            ):
                # Special-case protocols to get precise binding of self type for
                # custom tuple types like NamedTuples.
                return True
            return False
        elif isinstance(right, TupleType):
            # If right has a variadic unpack this needs special handling. If there is a TypeVarTuple
            # unpack, item count must coincide. If the left has variadic unpack but right
            # doesn't have one, we will fall through to False down the line.
            if self.variadic_tuple_subtype(left, right):
                return True
            if len(left.items) != len(right.items):
                return False
            if any(not self._is_subtype(l, r) for l, r in zip(left.items, right.items)):
                return False
            if is_named_instance(right.partial_fallback, "builtins.tuple"):
                # No need to verify fallback. This is useful since the calculated fallback
                # may be inconsistent due to how we calculate joins between unions vs.

                # non-unions. For example, join(int, str) == object, whereas
                # join(Union[int, C], Union[str, C]) == Union[int, str, C].
                return True
            if is_named_instance(left.partial_fallback, "builtins.tuple"):
                # Again, no need to verify. At this point we know the right fallback
                # is a subclass of tuple, so if left is plain tuple, it cannot be a subtype.
                return False
            # At this point we know both fallbacks are non-tuple.
            return self._is_subtype(left.partial_fallback, right.partial_fallback)
        else:
            return False

    def variadic_tuple_subtype(self, left: TupleType, right: TupleType) -> bool:
        """Check subtyping between two potentially variadic tuples.

        Most non-trivial cases here are due to variadic unpacks like *tuple[X, ...],
        we handle such unpacks as infinite unions Tuple[()] | Tuple[X] | Tuple[X, X] | ...

        Note: the cases where right is fixed or has *Ts unpack should be handled
        by the caller.
        """
        right_unpack_index = find_unpack_in_list(right.items)
        if right_unpack_index is None:
            # This case should be handled by the caller.
            return False
        right_unpack = right.items[right_unpack_index]
        assert isinstance(right_unpack, UnpackType)
        right_unpacked = get_proper_type(right_unpack.type)
        if not isinstance(right_unpacked, Instance):
            # This case should be handled by the caller.
            return False
        assert right_unpacked.type.fullname == "builtins.tuple"
        right_item = right_unpacked.args[0]
        right_prefix = right_unpack_index
        right_suffix = len(right.items) - right_prefix - 1
        left_unpack_index = find_unpack_in_list(left.items)
        if left_unpack_index is None:
            # Simple case: left is fixed, simply find correct mapping to the right
            # (effectively selecting item with matching length from an infinite union).
            if len(left.items) < right_prefix + right_suffix:
                return False
            prefix, middle, suffix = split_with_prefix_and_suffix(
                tuple(left.items), right_prefix, right_suffix
            )
            if not all(
                self._is_subtype(li, ri) for li, ri in zip(prefix, right.items[:right_prefix])
            ):
                return False
            if right_suffix and not all(
                self._is_subtype(li, ri) for li, ri in zip(suffix, right.items[-right_suffix:])
            ):
                return False
            return all(self._is_subtype(li, right_item) for li in middle)
        else:
            if len(left.items) < len(right.items):
                # There are some items on the left that will never have a matching length
                # on the right.
                return False
            left_prefix = left_unpack_index
            left_suffix = len(left.items) - left_prefix - 1
            left_unpack = left.items[left_unpack_index]
            assert isinstance(left_unpack, UnpackType)
            left_unpacked = get_proper_type(left_unpack.type)
            if not isinstance(left_unpacked, Instance):
                # *Ts unpack can't be split, except if it is all mapped to Anys or objects.
                if self.is_top_type(right_item):
                    right_prefix_types, middle, right_suffix_types = split_with_prefix_and_suffix(
                        tuple(right.items), left_prefix, left_suffix
                    )
                    if not all(
                        self.is_top_type(ri) or isinstance(ri, UnpackType) for ri in middle
                    ):
                        return False
                    # Also check the tails match as well.
                    return self._all_subtypes(
                        left.items[:left_prefix], right_prefix_types
                    ) and self._all_subtypes(left.items[-left_suffix:], right_suffix_types)
                return False
            assert left_unpacked.type.fullname == "builtins.tuple"
            left_item = left_unpacked.args[0]

            # The most tricky case with two variadic unpacks we handle similar to union
            # subtyping: *each* item on the left, must be a subtype of *some* item on the right.

            # For this we first check the "asymptotic case", i.e. that both unpacks a subtypes,
            # and then check subtyping for all finite overlaps.
            if not self._is_subtype(left_item, right_item):
                return False
            max_overlap = max(0, right_prefix - left_prefix, right_suffix - left_suffix)
            for overlap in range(max_overlap + 1):
                repr_items = left.items[:left_prefix] + [left_item] * overlap
                if left_suffix:
                    repr_items += left.items[-left_suffix:]
                left_repr = left.copy_modified(items=repr_items)
                if not self._is_subtype(left_repr, right):
                    return False
            return True

    def is_top_type(self, typ: Type) -> bool:
        if not self.proper_subtype and isinstance(get_proper_type(typ), AnyType):
            return True
        return is_named_instance(typ, "builtins.object")

    def visit_typeddict_type(self, left: TypedDictType) -> bool:
        right = self.right
        if isinstance(right, Instance):
            return self._is_subtype(left.fallback, right)
        elif isinstance(right, TypedDictType):
            if left == right:
                return True  # Fast path

            # A closed type must remain closed
            if right.is_closed and not left.is_closed:
                return False

            # Perform fast key-based checks before recursing into value types
            for name, l, r in left.zipall(right):
                # Required keys must remain required
                if r.required and not l.required:
                    return False
                # Mutable keys must remain mutable
                if r.mutable and not l.mutable:
                    return False
                # Mutable optional keys must also remain optional,
                # to retain the ability to delete them
                if r.mutable and not r.required and l.required:
                    return False

            for name, l, r in left.zipall(right):
                if r.mutable:
                    # None will only be used for missing ReadOnly[object] keys
                    assert r.typ is not None
                    # Guaranteed by "mutable keys must remain mutable" check
                    assert l.typ is not None

                    # TODO: should we pass on the full subtype_context here and below?
                    if self.proper_subtype:
                        check = is_same_type(l.typ, r.typ)
                    else:
                        check = is_equivalent(
                            l.typ,
                            r.typ,
                            ignore_type_params=self.subtype_context.ignore_type_params,
                            options=self.options,
                        )
                else:
                    # Read-only items behave covariantly
                    if r.typ is None:
                        check = True
                    elif l.typ is None:
                        # Keys cannot be dropped in an open subtype
                        # as this would implicitly widen the type constraint
                        check = False
                    else:
                        check = self._is_subtype(l.typ, r.typ)
                if not check:
                    return False
            # (NOTE: Fallbacks don't matter.)
            return True
        else:
            return False

    def visit_literal_type(self, left: LiteralType) -> bool:
        if isinstance(self.right, LiteralType):
            return left == self.right
        else:
            return self._is_subtype(left.fallback, self.right)

    def visit_overloaded(self, left: Overloaded) -> bool:
        right = self.right
        if isinstance(right, Instance):
            if right.type.is_protocol and "__call__" in right.type.protocol_members:
                # same as for CallableType
                call = find_member("__call__", right, right, is_operator=True)
                assert call is not None
                if self._is_subtype(left, call):
                    if len(right.type.protocol_members) == 1:
                        return True
                    if is_protocol_implementation(left.fallback, right, skip=["__call__"]):
                        return True
            return self._is_subtype(left.fallback, right)
        elif isinstance(right, CallableType):
            for item in left.items:
                if self._is_subtype(item, right):
                    return True
            return False
        elif isinstance(right, Overloaded):
            if left == self.right:
                # When it is the same overload, then the types are equal.
                return True

            # Ensure each overload on the right side (the supertype) is accounted for.
            previous_match_left_index = -1
            matched_overloads = set()

            for right_item in right.items:
                found_match = False

                for left_index, left_item in enumerate(left.items):
                    subtype_match = self._is_subtype(left_item, right_item)

                    # Order matters: we need to make sure that the index of
                    # this item is at least the index of the previous one.
                    if subtype_match and previous_match_left_index <= left_index:
                        previous_match_left_index = left_index
                        found_match = True
                        matched_overloads.add(left_index)
                        break
                    else:
                        # If this one overlaps with the supertype in any way, but it wasn't
                        # an exact match, then it's a potential error.
                        strict_concat = (
                            (self.options.extra_checks or self.options.strict_concatenate)
                            if self.options
                            else False
                        )
                        if left_index not in matched_overloads and (
                            is_callable_compatible(
                                left_item,
                                right_item,
                                is_compat=self._is_subtype,
                                is_proper_subtype=self.proper_subtype,
                                ignore_return=True,
                                ignore_pos_arg_names=self.subtype_context.ignore_pos_arg_names,
                                strict_concatenate=strict_concat,
                            )
                            or is_callable_compatible(
                                right_item,
                                left_item,
                                is_compat=self._is_subtype,
                                is_proper_subtype=self.proper_subtype,
                                ignore_return=True,
                                ignore_pos_arg_names=self.subtype_context.ignore_pos_arg_names,
                                strict_concatenate=strict_concat,
                            )
                        ):
                            return False

                if not found_match:
                    return False
            return True
        elif isinstance(right, UnboundType):
            return True
        elif isinstance(right, TypeType):
            # All the items must have the same type object status, so
            # it's sufficient to query only (any) one of them.
            # This is unsound, we don't check all the __init__ signatures.
            return left.is_type_obj() and self._is_subtype(left.items[0], right)
        else:
            return False

    def visit_union_type(self, left: UnionType) -> bool:
        if isinstance(self.right, Instance):
            literal_types: set[Instance] = set()
            # avoid redundant check for union of literals
            for item in left.relevant_items():
                p_item = get_proper_type(item)
                lit_type = mypy.typeops.simple_literal_type(p_item)
                if lit_type is not None:
                    if lit_type in literal_types:
                        continue
                    literal_types.add(lit_type)
                    item = lit_type
                if not self._is_subtype(item, self.orig_right):
                    return False
            return True

        elif isinstance(self.right, UnionType):
            # prune literals early to avoid nasty quadratic behavior which would
            # otherwise arise when checking
            # subtype relationships between slightly different narrowings of an Enum

            # we achieve O(N+M) instead of O(N*M)

            fast_check: set[ProperType] = set()

            for item in flatten_types(self.right.relevant_items()):
                p_item = get_proper_type(item)
                fast_check.add(p_item)
                if isinstance(p_item, Instance) and p_item.last_known_value is not None:
                    fast_check.add(p_item.last_known_value)

            for item in left.relevant_items():
                p_item = get_proper_type(item)
                if p_item in fast_check:
                    continue
                lit_type = mypy.typeops.simple_literal_type(p_item)
                if lit_type in fast_check:
                    continue
                if not self._is_subtype(item, self.orig_right):
                    return False
            return True

        return all(self._is_subtype(item, self.orig_right) for item in left.items)

    def visit_partial_type(self, left: PartialType) -> bool:
        # This is indeterminate as we don't really know the complete type yet.
        if self.proper_subtype:
            # TODO: What's the right thing to do here?
            return False
        if left.type is None:
            # Special case, partial `None`. This might happen when defining
            # class-level attributes with explicit `None`.
            # We can still recover from this.

            # https://github.com/python/mypy/issues/11105
            return self.visit_none_type(NoneType())
        raise RuntimeError(f'Partial type "{left}" cannot be checked with "issubtype()"')

    def visit_type_type(self, left: TypeType) -> bool:
        right = self.right
        if left.is_type_form:
            if isinstance(right, TypeType):
                if not right.is_type_form:
                    return False
                return self._is_subtype(left.item, right.item)
            if isinstance(right, Instance):
                if right.type.fullname == "builtins.object":
                    return True
                return False
            return False
        else:  # not left.is_type_form
            if isinstance(right, TypeType):
                return self._is_subtype(left.item, right.item)
            if isinstance(right, Overloaded) and right.is_type_obj():
                # Same as in other direction: if it's a constructor callable, all
                # items should belong to the same class' constructor, so it's enough
                # to check one of them.
                return self._is_subtype(left, right.items[0])
            if isinstance(right, CallableType):
                if self.proper_subtype and not right.is_type_obj():
                    # We can't accept `Type[X]` as a *proper* subtype of Callable[P, X]
                    # since this will break transitivity of subtyping.
                    return False
                item = left.item
                # Note: self-types are special since returning `Self` is more precise
                # than its upper bound, so we don't unwrap TypeVar to its bound here.
                if isinstance(item, TupleType):
                    item = mypy.typeops.tuple_fallback(item)
                if isinstance(item, Instance):
                    constructor = mypy.typeops.type_object_type(item.type)
                    constructor = expand_type_by_instance(constructor, item)
                    if isinstance(constructor, CallableType):
                        # Only consider return type to match historic behavior (see below).
                        return self._is_subtype(constructor.ret_type, right.ret_type)
                    elif isinstance(constructor, Overloaded):
                        return all(
                            self._is_subtype(c.ret_type, right.ret_type) for c in constructor.items
                        )
                # This is unsound, we don't check the __init__ signature.
                return self._is_subtype(left.item, right.ret_type)

            if isinstance(right, Instance):
                if right.type.fullname in ["builtins.object", "builtins.type"]:
                    # TODO: Strictly speaking, the type builtins.type is considered equivalent to
                    #       Type[Any]. However, this would break the is_proper_subtype check in
                    #       conditional_types for cases like isinstance(x, type) when the type

                    #       of x is Type[int]. It's unclear what's the right way to address this.
                    return True
                item = left.item
                if isinstance(item, TypeVarType):
                    item = get_proper_type(item.upper_bound)
                if isinstance(item, Instance):
                    if right.type.is_protocol and is_protocol_implementation(
                        item, right, proper_subtype=self.proper_subtype, class_obj=True
                    ):
                        return True
                    metaclass = item.type.metaclass_type
                    return metaclass is not None and self._is_subtype(metaclass, right)
            return False

    def visit_type_alias_type(self, left: TypeAliasType) -> bool:
        assert False, f"This should be never called, got {left}"


T = TypeVar("T", bound=Type)


@contextmanager
def pop_on_exit(stack: list[tuple[T, T]], left: T, right: T) -> Iterator[None]:
    stack.append((left, right))
    yield
    stack.pop()


# In-flight recursive-protocol pairs, keyed by the wire bytes both sides
# already share plus the proper-subtype flag; see protocol_pair_in_flight.
# Refcounted, since nested frames can legally hold the same pair twice.
_PROTOCOL_PAIRS_IN_FLIGHT: Final[dict[tuple[bytes, bytes, bool], int]] = {}


def protocol_pair_in_flight(left: bytes, right: bytes, proper: bool) -> bool:
    """Registry consult used by the Rust protocol kernel (no Python callers).

    Mirrors the `assuming` matrix lifetime of is_protocol_implementation
    for the Rust kernel (#1344): when a Python member loop defers a
    sub-check into a native chain that re-derives the same pair, the Rust
    thread-local assuming stack is empty, and this registry is what lets
    the kernel cut exactly where the matrix scan would. Keys are the wire
    bytes both sides already share (see _serialize_type, including the
    builtin-instance short-circuit) plus the proper flag.

    A miss is always safe: it only costs re-derivation, never a wrong
    verdict (a missing cut defers to the actual member loop).
    """
    return (left, right, proper) in _PROTOCOL_PAIRS_IN_FLIGHT


def is_protocol_implementation(
    left: Instance | TupleType,
    right: Instance,
    proper_subtype: bool = False,
    class_obj: bool = False,
    skip: list[str] | None = None,
    options: Options | None = None,
) -> bool:
    """Check whether 'left' implements the protocol 'right'.

    If 'proper_subtype' is True, then check for a proper subtype.
    Treat recursive protocols by using the 'assuming' structural subtype matrix
    (in sparse representation, i.e. as a list of pairs (subtype, supertype)),
    see also comment in nodes.TypeInfo. When we enter a check for classes
    (A, P), defined as following::

      class P(Protocol):
          def f(self) -> P: ...
      class A:
          def f(self) -> A: ...

    this results in A being a subtype of P without infinite recursion.
    On every false result, we pop the assumption, thus avoiding an infinite recursion
    as well.
    """
    assert right.type.is_protocol
    if skip is None:
        skip = []
    # Preserve original left type for precise self-type binding. Only tuple types are
    # supported for now.
    original_left = left
    if isinstance(left, TupleType):
        left = mypy.typeops.tuple_fallback(left)
    # We need to record this check to generate protocol fine-grained dependencies.
    type_state.record_protocol_subtype_check(left.type, right.type)
    # nominal subtyping currently ignores '__init__' and '__new__' signatures
    members_not_to_check = {"__init__", "__new__"}
    members_not_to_check.update(skip)
    # Trivial check that circumvents the bug described in issue 9771:
    if left.type.is_protocol:
        members_right = set(right.type.protocol_members) - members_not_to_check
        members_left = set(left.type.protocol_members) - members_not_to_check
        if not members_right.issubset(members_left):
            return False
    assuming = right.type.assuming_proper if proper_subtype else right.type.assuming
    for l, r in reversed(assuming):
        if l == left and r == right:
            return True
    # Register the in-flight pair for the Rust kernel (#1344): key on the
    # post-fallback left (the Instance natively re-derived). A
    # serialization failure just skips registration.
    pair_key: tuple[bytes, bytes, bool] | None = None
    if _HAS_TYPE_KERNEL and _native_subtype_active:
        try:
            pair_key = (_serialize_type(left), _serialize_type(right), proper_subtype)
            count = _PROTOCOL_PAIRS_IN_FLIGHT.get(pair_key, 0)
            _PROTOCOL_PAIRS_IN_FLIGHT[pair_key] = count + 1
        except (AssertionError, NotImplementedError):
            pair_key = None
    try:
        with pop_on_exit(assuming, left, right):
            for member in right.type.protocol_members:
                if member in members_not_to_check:
                    continue
                ignore_names = member != "__call__"  # __call__ can be passed kwargs
                # The third argument below indicates to what self type is bound.
                # We always bind self to the subtype. (Similarly to nominal types).
                supertype = find_member(member, right, original_left)
                assert supertype is not None

                subtype = get_protocol_member(left, original_left, member, class_obj)
                # Useful for debugging:
                # print(member, 'of', left, 'has type', subtype)
                # print(member, 'of', right, 'has type', supertype)
                if not subtype:
                    return False
                if not proper_subtype:
                    # Nominal check currently ignores arg names
                    # NOTE: If we ever change this, be sure to also change the call to
                    # SubtypeVisitor.build_subtype_kind(...) down below.
                    is_compat = is_subtype(
                        subtype, supertype, ignore_pos_arg_names=ignore_names, options=options
                    )
                else:
                    is_compat = is_proper_subtype(subtype, supertype)
                if not is_compat:
                    return False
                if isinstance(get_proper_type(subtype), NoneType) and isinstance(
                    get_proper_type(supertype), CallableType
                ):
                    # We want __hash__ = None idiom to work even without --strict-optional
                    return False
                subflags = get_member_flags(member, left, class_obj=class_obj)
                superflags = get_member_flags(member, right)
                if IS_SETTABLE in superflags:
                    # Check opposite direction for settable attributes.
                    if IS_EXPLICIT_SETTER in superflags:
                        supertype = find_member(member, right, original_left, is_lvalue=True)
                    if IS_EXPLICIT_SETTER in subflags:
                        subtype = get_protocol_member(
                            left, original_left, member, class_obj, is_lvalue=True
                        )
                    # At this point we know attribute is present on subtype, otherwise we
                    # would return False above.
                    assert supertype is not None and subtype is not None
                    if not is_subtype(supertype, subtype, options=options):
                        return False
                if IS_SETTABLE in superflags and IS_SETTABLE not in subflags:
                    return False
                if not class_obj:
                    if IS_SETTABLE not in superflags:
                        if IS_CLASSVAR in superflags and IS_CLASSVAR not in subflags:
                            return False
                    elif (IS_CLASSVAR in subflags) != (IS_CLASSVAR in superflags):
                        return False
                else:
                    if IS_VAR in superflags and IS_CLASSVAR not in subflags:
                        # Only class variables are allowed for class object access.
                        return False
                    if IS_CLASSVAR in superflags:
                        # This can be never matched by a class object.
                        return False
                # This rule is copied from nominal check in checker.py
                if IS_CLASS_OR_STATIC in superflags and IS_CLASS_OR_STATIC not in subflags:
                    return False
    finally:
        if pair_key is not None:
            count = _PROTOCOL_PAIRS_IN_FLIGHT[pair_key] - 1
            if count:
                _PROTOCOL_PAIRS_IN_FLIGHT[pair_key] = count
            else:
                del _PROTOCOL_PAIRS_IN_FLIGHT[pair_key]

    if not proper_subtype:
        # Nominal check currently ignores arg names, but __call__ is special for protocols
        ignore_names = right.type.protocol_members != ["__call__"]
    else:
        ignore_names = False
    subtype_kind = SubtypeVisitor.build_subtype_kind(
        subtype_context=SubtypeContext(ignore_pos_arg_names=ignore_names),
        proper_subtype=proper_subtype,
    )
    type_state.record_subtype_cache_entry(subtype_kind, left, right)
    return True


def get_protocol_member(
    left: Instance, original_left: Type, member: str, class_obj: bool, is_lvalue: bool = False
) -> Type | None:
    if (
        _native_find_member_prelude_active()
        and not class_obj
        and not is_lvalue
        and member != "__call__"
        and left.extra_attrs is not None
    ):
        # The kernel defers gpm on extra_attrs-bearing instances (module
        # types); resolve the miss/hit through the #1074 prelude classifier
        # (`__call__` is excluded: the metaclass case at the bottom fires).
        try:
            tag = _rust_classify_find_member(member, left, False, False)
        except (AssertionError, NotImplementedError, ValueError, AttributeError):
            tag = None
        if tag == NATIVE_FIND_MEMBER_EXTRA_ATTR:
            assert left.extra_attrs is not None
            return left.extra_attrs.attrs[member]
        if tag == NATIVE_FIND_MEMBER_NOT_FOUND:
            return None
    if _HAS_TYPE_KERNEL and _native_subtype_active and _native_subtype_resolver is not None:
        try:
            result = _type_kernel.rust_get_protocol_member(
                _serialize_type(left),
                _serialize_type(original_left),
                member,
                class_obj,
                is_lvalue,
                _native_subtype_resolver,
            )
            if result is not None:
                if len(result) == 0:
                    return None
                decoded = _deserialize_type(bytes(result))
                if decoded is not None:
                    return _restore_protocol_member_definition(left, member, decoded)
        except (AssertionError, NotImplementedError, ValueError, AttributeError):
            pass
    if member == "__call__" and class_obj:
        # Special case: class objects always have __call__ that is just the constructor.
        return mypy.typeops.type_object_type(left.type)

    if member == "__call__" and left.type.is_metaclass(precise=True):
        # Special case: we want to avoid falling back to metaclass __call__
        # if constructor signature didn't match, this can cause many false negatives.
        return None

    subtype = find_member(member, left, original_left, class_obj=class_obj, is_lvalue=is_lvalue)
    if isinstance(subtype, PartialType):
        subtype = (
            NoneType()
            if subtype.type is None
            else Instance(
                subtype.type, [AnyType(TypeOfAny.unannotated)] * len(subtype.type.type_vars)
            )
        )
    return subtype


def find_member(
    name: str,
    itype: Instance,
    subtype: Type,
    *,
    is_operator: bool = False,
    class_obj: bool = False,
    is_lvalue: bool = False,
) -> Type | None:
    type_checker = checker_state.type_checker
    if type_checker is None:
        # Unfortunately, there are many scenarios where someone calls is_subtype() before
        # type checking phase. In this case we fallback to old (incomplete) logic.
        # TODO: reduce number of such cases (e.g. semanal_typeargs, post-semanal plugins).
        return find_member_simple(
            name, itype, subtype, is_operator=is_operator, class_obj=class_obj, is_lvalue=is_lvalue
        )

    if _native_find_member_prelude_active():
        # Issue #1074: Rust classifies the name-resolution prelude (the
        # info.get miss path, the dunder-accessor scan, and the miss
        # verdicts); the checkmember tail below stays Python.
        try:
            tag = _rust_classify_find_member(name, itype, is_operator, class_obj)
        except (AssertionError, NotImplementedError, ValueError, AttributeError):
            tag = None
        if tag == NATIVE_FIND_MEMBER_PROCEED:
            pass
        elif tag == NATIVE_FIND_MEMBER_ANY_SPECIAL_FORM:
            return AnyType(TypeOfAny.special_form)
        elif tag == NATIVE_FIND_MEMBER_EXTRA_ATTR:
            # Rust decided the hit; fetch the live Type here so it never
            # crosses the seam.
            assert itype.extra_attrs is not None
            return itype.extra_attrs.attrs[name]
        elif tag == NATIVE_FIND_MEMBER_NOT_FOUND:
            return None

    # We don't use ATTR_DEFINED error code below (since missing attributes can cause various
    # other error codes), instead we perform quick node lookup with all the fallbacks.
    info = itype.type
    sym = info.get(name)
    node = sym.node if sym else None
    if not node:
        name_not_found = True
        if (
            name not in ["__getattr__", "__setattr__", "__getattribute__"]
            and not is_operator
            and not class_obj
            and itype.extra_attrs is None  # skip ModuleType.__getattr__
        ):
            for method_name in ("__getattribute__", "__getattr__"):
                method = info.get_method(method_name)
                if method and method.info.fullname != "builtins.object":
                    name_not_found = False
                    break
        if name_not_found:
            if info.fallback_to_any or class_obj and info.meta_fallback_to_any:
                return AnyType(TypeOfAny.special_form)
            if itype.extra_attrs and name in itype.extra_attrs.attrs:
                return itype.extra_attrs.attrs[name]
            return None

    from mypy.checkmember import (
        MemberContext,
        analyze_class_attribute_access,
        analyze_instance_member_access,
    )

    mx = MemberContext(
        is_lvalue=is_lvalue,
        is_super=False,
        is_operator=is_operator,
        original_type=TypeType.make_normalized(itype) if class_obj else itype,
        self_type=TypeType.make_normalized(subtype) if class_obj else subtype,
        context=Context(),  # all errors are filtered, but this is a required argument
        chk=type_checker,
        suppress_errors=True,
        # Avoid infinite recursion with protocols like
        # class P(Protocol[T]): def combine(self, other: P[S]) -> P[Tuple[T, S]]: ...
        # Protocols can't use the assumption stack here (it would grow indefinitely).
        preserve_type_var_ids=True,
    )
    with type_checker.msg.filter_errors(filter_deprecated=True):
        if class_obj:
            fallback = itype.type.metaclass_type or mx.named_type("builtins.type")
            return analyze_class_attribute_access(itype, name, mx, mcs_fallback=fallback)
        else:
            return analyze_instance_member_access(name, itype, mx, info)


def find_member_simple(
    name: str,
    itype: Instance,
    subtype: Type,
    *,
    is_operator: bool = False,
    class_obj: bool = False,
    is_lvalue: bool = False,
) -> Type | None:
    """Find the type of member by 'name' in 'itype's TypeInfo.

    Find the member type after applying type arguments from 'itype', and binding
    'self' to 'subtype'. Return None if member was not found.
    """
    info = itype.type
    method = info.get_method(name)
    if method:
        if isinstance(method, Decorator):
            return find_node_type(method.var, itype, subtype, class_obj=class_obj)
        if method.is_property:
            assert isinstance(method, OverloadedFuncDef)
            dec = method.items[0]
            assert isinstance(dec, Decorator)
            # Pass on is_lvalue flag as this may be a property with different setter type.
            return find_node_type(
                dec.var, itype, subtype, class_obj=class_obj, is_lvalue=is_lvalue
            )
        return find_node_type(method, itype, subtype, class_obj=class_obj)
    else:
        # don't have such method, maybe variable or decorator?
        node = info.get(name)
        v = node.node if node else None
        if isinstance(v, Var):
            return find_node_type(v, itype, subtype, class_obj=class_obj)
        if (
            not v
            and name not in ["__getattr__", "__setattr__", "__getattribute__"]
            and not is_operator
            and not class_obj
            and itype.extra_attrs is None  # skip ModuleType.__getattr__
        ):
            for method_name in ("__getattribute__", "__getattr__"):
                # Normally, mypy assumes that instances that define __getattr__ have all
                # attributes with the corresponding return type. If this will produce
                # many false negatives, then this could be prohibited for

                # structural subtyping.
                method = info.get_method(method_name)
                if method and method.info.fullname != "builtins.object":
                    if isinstance(method, Decorator):
                        getattr_type = get_proper_type(find_node_type(method.var, itype, subtype))
                    else:
                        getattr_type = get_proper_type(find_node_type(method, itype, subtype))
                    if isinstance(getattr_type, CallableType):
                        return getattr_type.ret_type
                    return getattr_type
        if itype.type.fallback_to_any or class_obj and itype.type.meta_fallback_to_any:
            return AnyType(TypeOfAny.special_form)
        if isinstance(v, TypeInfo):
            # PEP 544 doesn't specify anything about such use cases. So we just try
            # to do something meaningful (at least we should not crash).
            return TypeType(fill_typevars_with_any(v))
    if itype.extra_attrs and name in itype.extra_attrs.attrs:
        return itype.extra_attrs.attrs[name]
    return None


def get_member_flags(name: str, itype: Instance, class_obj: bool = False) -> set[int]:
    """Detect whether a member 'name' is settable, whether it is an
    instance or class variable, and whether it is class or static method.

    The flags are defined as following:
    * IS_SETTABLE: whether this attribute can be set, not set for methods and
      non-settable properties;
    * IS_CLASSVAR: set if the variable is annotated as 'x: ClassVar[t]';
    * IS_CLASS_OR_STATIC: set for methods decorated with @classmethod or
      with @staticmethod.
    """
    if _HAS_TYPE_KERNEL and _native_subtype_active and _native_subtype_resolver is not None:
        try:
            result = _type_kernel.rust_get_member_flags(
                itype.type,
                name,
                class_obj,
                itype.extra_attrs,
                state.strict_optional,
                _native_subtype_resolver,
            )
            if result is not None:
                return {int(flag) for flag in result}
        except (AssertionError, NotImplementedError, ValueError):
            pass
    info = itype.type
    method = info.get_method(name)
    setattr_meth = info.get_method("__setattr__")
    if method:
        if isinstance(method, Decorator):
            if method.var.is_staticmethod or method.var.is_classmethod:
                return {IS_CLASS_OR_STATIC}
            elif method.var.is_property:
                return {IS_VAR}
        elif method.is_property:  # this could be settable property
            assert isinstance(method, OverloadedFuncDef)
            dec = method.items[0]
            assert isinstance(dec, Decorator)
            if dec.var.is_settable_property or setattr_meth:
                flags = {IS_VAR, IS_SETTABLE}
                if dec.var.setter_type is not None:
                    flags.add(IS_EXPLICIT_SETTER)
                return flags
            else:
                return {IS_VAR}
        return set()  # Just a regular method
    node = info.get(name)
    if not node:
        if setattr_meth:
            return {IS_SETTABLE}
        if itype.extra_attrs and name in itype.extra_attrs.attrs:
            flags = set()
            if name not in itype.extra_attrs.immutable:
                flags.add(IS_SETTABLE)
            return flags
        return set()
    v = node.node
    # just a variable
    if isinstance(v, Var):
        if v.is_property:
            return {IS_VAR}
        flags = {IS_VAR}
        if not v.is_final:
            flags.add(IS_SETTABLE)
        # TODO: define cleaner rules for class vs instance variables.
        if v.is_classvar and not is_descriptor(v.type):
            flags.add(IS_CLASSVAR)
        if class_obj and v.is_inferred:
            flags.add(IS_CLASSVAR)
        return flags
    return set()


def is_descriptor(typ: Type | None) -> bool:
    if (
        typ is not None
        and _HAS_TYPE_KERNEL
        and _native_subtype_active
        and _native_subtype_resolver is not None
    ):
        try:
            type_bytes = _serialize_type(typ)
        except Exception:
            type_bytes = None
        if type_bytes is not None:
            try:
                result = _type_kernel.rust_is_descriptor(
                    _native_subtype_resolver, type_bytes, state.strict_optional
                )
            except (AssertionError, NotImplementedError):
                result = None
            if result is not None:
                return result
    typ = get_proper_type(typ)
    if isinstance(typ, Instance):
        return typ.type.get("__get__") is not None
    if isinstance(typ, UnionType):
        return all(is_descriptor(item) for item in typ.relevant_items())
    return False


def find_node_type(
    node: Var | FuncBase,
    itype: Instance,
    subtype: Type,
    class_obj: bool = False,
    is_lvalue: bool = False,
) -> Type:
    """Find type of a variable or method 'node' (maybe also a decorated method).
    Apply type arguments from 'itype', and bind 'self' to 'subtype'.
    """
    from mypy.typeops import bind_self

    if isinstance(node, FuncBase):
        typ: Type | None = mypy.typeops.function_type(
            node, fallback=Instance(itype.type.mro[-1], [])
        )
    else:
        # This part and the one below are simply copies of the logic from checkmember.py.
        if node.is_settable_property and is_lvalue:
            typ = node.setter_type
            if typ is None and node.is_ready:
                typ = node.type
        else:
            typ = node.type
        if typ is not None:
            typ = expand_self_type(node, typ, subtype)
    p_typ = get_proper_type(typ)
    if typ is None:
        return AnyType(TypeOfAny.from_error)
    # We don't need to bind 'self' for static methods, since there is no 'self'.
    if isinstance(node, FuncBase) or (
        isinstance(p_typ, FunctionLike)
        and node.is_initialized_in_class
        and not node.is_staticmethod
    ):
        assert isinstance(p_typ, FunctionLike)
        if class_obj and not (
            node.is_class if isinstance(node, FuncBase) else node.is_classmethod
        ):
            # Don't bind instance methods on class objects.
            signature = p_typ
        else:
            signature = bind_self(
                p_typ, subtype, is_classmethod=isinstance(node, Var) and node.is_classmethod
            )
        if node.is_property and not class_obj:
            assert isinstance(signature, CallableType)
            if (
                isinstance(node, Var)
                and node.is_settable_property
                and is_lvalue
                and node.setter_type is not None
            ):
                typ = signature.arg_types[0]
            else:
                typ = signature.ret_type
        else:
            typ = signature
    itype = map_instance_to_supertype(itype, node.info)
    typ = expand_type_by_instance(typ, itype)
    return typ


def non_method_protocol_members(tp: TypeInfo) -> list[str]:
    """Find all non-callable members of a protocol."""

    assert tp.is_protocol
    result: list[str] = []
    anytype = AnyType(TypeOfAny.special_form)
    instance = Instance(tp, [anytype] * len(tp.defn.type_vars))

    for member in tp.protocol_members:
        typ = get_proper_type(find_member(member, instance, instance))
        if not isinstance(typ, (Overloaded, CallableType)):
            result.append(member)
    return result


def is_callable_compatible(
    left: CallableType,
    right: CallableType,
    *,
    is_compat: Callable[[Type, Type], bool],
    is_proper_subtype: bool,
    is_compat_return: Callable[[Type, Type], bool] | None = None,
    ignore_return: bool = False,
    ignore_pos_arg_names: bool = False,
    check_args_covariantly: bool = False,
    allow_partial_overlap: bool = False,
    strict_concatenate: bool = False,
) -> bool:
    """Is the left compatible with the right, using the provided compatibility check?

    is_compat:
        The check we want to run against the parameters.

    is_compat_return:
        The check we want to run against the return type.
        If None, use the 'is_compat' check.

    check_args_covariantly:
        If true, check if the left's args is compatible with the right's
        instead of the other way around (contravariantly).

        This function is mostly used to check if the left is a subtype of the right which
        is why the default is to check the args contravariantly. However, it's occasionally
        useful to check the args using some other check, so we leave the variance
        configurable.

        For example, when checking the validity of overloads, it's useful to see if
        the first overload alternative has more precise arguments than the second.
        We would want to check the arguments covariantly in that case.

        Note! The following two function calls are NOT equivalent:

            is_callable_compatible(f, g, is_compat=is_subtype, check_args_covariantly=False)
            is_callable_compatible(g, f, is_compat=is_subtype, check_args_covariantly=True)

        The two calls are similar in that they both check the function arguments in
        the same direction: they both run `is_subtype(argument_from_g, argument_from_f)`.

        However, the two calls differ in which direction they check things like
        keyword arguments. For example, suppose f and g are defined like so:

            def f(x: int, *y: int) -> int: ...
            def g(x: int) -> int: ...

        In this case, the first call will succeed and the second will fail: f is a
        valid stand-in for g but not vice-versa.

    allow_partial_overlap:
        By default this function returns True if and only if *all* calls to left are
        also calls to right (with respect to the provided 'is_compat' function).

        If this parameter is set to 'True', we return True if *there exists at least one*
        call to left that's also a call to right.

        In other words, we perform an existential check instead of a universal one;
        we require left to only overlap with right instead of being a subset.

        For example, suppose we set 'is_compat' to some subtype check and compare following:

            f(x: float, y: str = "...", *args: bool) -> str
            g(*args: int) -> str

        This function would normally return 'False': f is not a subtype of g.
        However, we would return True if this parameter is set to 'True': the two
        calls are compatible if the user runs "f_or_g(3)". In the context of that
        specific call, the two functions effectively have signatures of:

            f2(float) -> str
            g2(int) -> str

        Here, f2 is a valid subtype of g2 so we return True.

        Specifically, if this parameter is set this function will:

        -   Ignore optional arguments on either the left or right that have no
            corresponding match.
        -   No longer mandate optional arguments on either side are also optional
            on the other.
        -   No longer mandate that if right has a *arg or **kwarg that left must also
            have the same.

        Note: when this argument is set to True, this function becomes "symmetric" --
        the following calls are equivalent:

            is_callable_compatible(f, g,
                                   is_compat=some_check,
                                   check_args_covariantly=False,
                                   allow_partial_overlap=True)
            is_callable_compatible(g, f,
                                   is_compat=some_check,
                                   check_args_covariantly=True,
                                   allow_partial_overlap=True)

        If the 'some_check' function is also symmetric, the two calls would be equivalent
        whether or not we check the args covariantly.
    """
    # Normalize both types before comparing them.
    left = left.with_unpacked_kwargs().with_normalized_var_args()
    right = right.with_unpacked_kwargs().with_normalized_var_args()

    if is_compat_return is None:
        is_compat_return = is_compat

    # If either function is implicitly typed, ignore positional arg names too
    if left.implicit or right.implicit:
        ignore_pos_arg_names = True

    # Non-type cannot be a subtype of type.
    if right.is_type_obj() and not left.is_type_obj() and not allow_partial_overlap:
        return False

    # A callable L is a subtype of a generic callable R if L is a
    # subtype of every type obtained from R by substituting types for
    # the variables of R. We can check this by simply leaving the

    # generic variables of R as type variables, effectively varying
    # over all possible values.

    # It's okay even if these variables share ids with generic
    # type variables of L, because generating and solving
    # constraints for the variables of L to make L a subtype of R

    # (below) treats type variables on the two sides as independent.
    if left.variables:
        # Apply generic type variables away in left via type inference.
        unified = unify_generic_callable(left, right, ignore_return=ignore_return)
        if unified is None:
            return False
        left = unified

    # Check return types.
    if not ignore_return and not is_compat_return(left.ret_type, right.ret_type):
        return False

    if check_args_covariantly:
        is_compat = flip_compat_check(is_compat)

    if not strict_concatenate and (left.from_concatenate or right.from_concatenate):
        strict_concatenate_check = False
    else:
        strict_concatenate_check = True

    return are_parameters_compatible(
        left,
        right,
        is_compat=is_compat,
        is_proper_subtype=is_proper_subtype,
        ignore_pos_arg_names=ignore_pos_arg_names,
        allow_partial_overlap=allow_partial_overlap,
        strict_concatenate_check=strict_concatenate_check,
    )


def are_trivial_parameters(param: Parameters | NormalizedCallableType) -> bool:
    param_star = param.var_arg()
    param_star2 = param.kw_arg()
    return (
        param.arg_kinds == [ARG_STAR, ARG_STAR2]
        and param_star is not None
        and isinstance(get_proper_type(param_star.typ), AnyType)
        and param_star2 is not None
        and isinstance(get_proper_type(param_star2.typ), AnyType)
    )


def is_trivial_suffix(param: Parameters | NormalizedCallableType) -> bool:
    param_star = param.var_arg()
    param_star2 = param.kw_arg()
    return (
        param.arg_kinds[-2:] == [ARG_STAR, ARG_STAR2]
        and param_star is not None
        and isinstance(get_proper_type(param_star.typ), AnyType)
        and param_star2 is not None
        and isinstance(get_proper_type(param_star2.typ), AnyType)
    )


def are_parameters_compatible(
    left: Parameters | NormalizedCallableType,
    right: Parameters | NormalizedCallableType,
    *,
    is_compat: Callable[[Type, Type], bool],
    is_proper_subtype: bool,
    ignore_pos_arg_names: bool = False,
    allow_partial_overlap: bool = False,
    strict_concatenate_check: bool = False,
) -> bool:
    """Helper function for is_callable_compatible, used for Parameter compatibility"""
    # Native type_kernel seam (#1066): same kernel decision as the
    # visit_parameters / meet-overlap shims; see the nested-is_compat
    # classifier below. None (defer) falls through to the body.
    nested_proper = _native_params_compat_nested_proper(is_compat, ignore_pos_arg_names)
    if (
        nested_proper is not None
        and _HAS_TYPE_KERNEL
        and _native_subtype_active
        and _native_subtype_resolver is not None
    ):
        try:
            result = _type_kernel.rust_are_parameters_compatible(
                _serialize_type(left),
                _serialize_type(right),
                is_proper_subtype,
                ignore_pos_arg_names,
                allow_partial_overlap,
                strict_concatenate_check,
                state.strict_optional,
                nested_proper,
                _native_subtype_resolver,
            )
        except (AssertionError, NotImplementedError):
            result = None
        if result is not None:
            return result

    if right.is_ellipsis_args and not is_proper_subtype:
        return True

    left_star = left.var_arg()
    left_star2 = left.kw_arg()
    right_star = right.var_arg()
    right_star2 = right.kw_arg()

    # Treat "def _(*a: Any, **kw: Any) -> X" similarly to "Callable[..., X]"
    if are_trivial_parameters(right) and not is_proper_subtype:
        return True
    trivial_suffix = is_trivial_suffix(right) and not is_proper_subtype

    trivial_vararg_suffix = False
    if (
        right.arg_kinds[-1:] == [ARG_STAR]
        and isinstance(get_proper_type(right.arg_types[-1]), AnyType)
        and not is_proper_subtype
        and all(k.is_positional(star=True) for k in left.arg_kinds)
    ):
        # Similar to how (*Any, **Any) is considered a supertype of all callables, we consider
        # (*Any) a supertype of all callables with positional arguments. This is needed in
        # particular because we often refuse to try type inference if actual type is not

        # a subtype of erased template type.
        trivial_vararg_suffix = True

    # Match up corresponding arguments and check them for compatibility. In
    # every pair (argL, argR) of corresponding arguments from L and R, argL must
    # be "more general" than argR if L is to be a subtype of R.

    # Arguments are corresponding if they either share a name, share a position,
    # or both. If L's corresponding argument is ambiguous, L is not a subtype of R.

    # If left has one corresponding argument by name and another by position,
    # consider them to be one "merged" argument (and not ambiguous) if they're
    # both optional, they're name-only and position-only respectively, and they

    # have the same type.  This rule allows functions with (*args, **kwargs) to
    # properly stand in for the full domain of formal arguments that they're
    # used for in practice.

    # Every argument in R must have a corresponding argument in L, and every
    # required argument in L must have a corresponding argument in R.

    # Phase 1: Confirm every argument in R has a corresponding argument in L.

    # Phase 1a: If left and right can both accept an infinite number of args,
    #           their types must be compatible.

    #           Furthermore, if we're checking for compatibility in all cases,
    #           we confirm that if R accepts an infinite number of arguments,
    #           L must accept the same.
    def _incompatible(left_arg: FormalArgument | None, right_arg: FormalArgument | None) -> bool:
        if right_arg is None:
            return False
        if left_arg is None:
            return not allow_partial_overlap and not trivial_suffix
        return not is_compat(right_arg.typ, left_arg.typ)

    if (
        _incompatible(left_star, right_star)
        and not trivial_vararg_suffix
        or _incompatible(left_star2, right_star2)
    ):
        return False

    # Phase 1b: Check non-star args: for every arg right can accept, left must
    #           also accept. The only exception is if we are allowing partial
    #           overlaps: in that case, we ignore optional args on the right.
    for right_arg in right.formal_arguments():
        left_arg = mypy.typeops.callable_corresponding_argument(left, right_arg)
        if left_arg is None:
            if allow_partial_overlap and not right_arg.required:
                continue
            return False
        if not are_args_compatible(
            left_arg,
            right_arg,
            is_compat,
            ignore_pos_arg_names=ignore_pos_arg_names,
            allow_partial_overlap=allow_partial_overlap,
            allow_imprecise_kinds=right.imprecise_arg_kinds,
        ):
            return False

    if trivial_suffix:
        # For trivial right suffix we *only* check that every non-star right argument
        # has a valid match on the left.
        return True

    # Phase 1c: Check var args. Right has an infinite series of optional positional
    #           arguments. Get all further positional args of left, and make sure
    #           they're more general than the corresponding member in right.

    # TODO: handle suffix in UnpackType (i.e. *args: *Tuple[Ts, X, Y]).
    if right_star is not None and not trivial_vararg_suffix:
        # Synthesize an anonymous formal argument for the right
        right_by_position = right.try_synthesizing_arg_from_vararg(None)
        assert right_by_position is not None

        i = right_star.pos
        assert i is not None
        while i < len(left.arg_kinds) and left.arg_kinds[i].is_positional():
            if allow_partial_overlap and left.arg_kinds[i].is_optional():
                break

            left_by_position = left.argument_by_position(i)
            assert left_by_position is not None

            if not are_args_compatible(
                left_by_position,
                right_by_position,
                is_compat,
                ignore_pos_arg_names=ignore_pos_arg_names,
                allow_partial_overlap=allow_partial_overlap,
            ):
                return False
            i += 1

    # Phase 1d: Check kw args. Right has an infinite series of optional named
    #           arguments. Get all further named args of left, and make sure
    #           they're more general than the corresponding member in right.
    if right_star2 is not None:
        right_names = {name for name in right.arg_names if name is not None}
        left_only_names = set()
        for name, kind in zip(left.arg_names, left.arg_kinds):
            if (
                name is None
                or kind.is_star()
                or name in right_names
                or not strict_concatenate_check
            ):
                continue
            left_only_names.add(name)

        # Synthesize an anonymous formal argument for the right
        right_by_name = right.try_synthesizing_arg_from_kwarg(None)
        assert right_by_name is not None

        for name in left_only_names:
            left_by_name = left.argument_by_name(name)
            assert left_by_name is not None

            if allow_partial_overlap and not left_by_name.required:
                continue

            if not are_args_compatible(
                left_by_name,
                right_by_name,
                is_compat,
                ignore_pos_arg_names=ignore_pos_arg_names,
                allow_partial_overlap=allow_partial_overlap,
            ):
                return False

    # Phase 2: Left must not impose additional restrictions.
    #          (Every required argument in L must have a corresponding argument in R)
    #          Note: we already checked the *arg and **kwarg arguments in phase 1a.
    for left_arg in left.formal_arguments():
        right_by_name = (
            right.argument_by_name(left_arg.name) if left_arg.name is not None else None
        )

        right_by_pos = (
            right.argument_by_position(left_arg.pos) if left_arg.pos is not None else None
        )

        # If the left hand argument corresponds to two right-hand arguments,
        # neither of them can be required.
        if (
            right_by_name is not None
            and right_by_pos is not None
            and right_by_name != right_by_pos
            and (right_by_pos.required or right_by_name.required)
            and strict_concatenate_check
            and not right.imprecise_arg_kinds
        ):
            return False

        # All *required* left-hand arguments must have a corresponding
        # right-hand argument.  Optional args do not matter.
        if left_arg.required and right_by_pos is None and right_by_name is None:
            return False

    return True


def are_args_compatible(
    left: FormalArgument,
    right: FormalArgument,
    is_compat: Callable[[Type, Type], bool],
    *,
    ignore_pos_arg_names: bool,
    allow_partial_overlap: bool,
    allow_imprecise_kinds: bool = False,
) -> bool:
    # Native type_kernel seam: classify the pure decision in Rust
    # (subtypes.rs); the trailing is_compat call stays here. None falls
    # through to the pure-Python body below.
    if _native_are_args_compatible_active():
        try:
            tag = _rust_are_args_compatible(
                left,
                right,
                bool(ignore_pos_arg_names),
                bool(allow_partial_overlap),
                bool(allow_imprecise_kinds),
            )
        except (AssertionError, NotImplementedError, ValueError, TypeError):
            tag = None
        if tag == NATIVE_ARE_ARGS_FALSE:
            return False
        if tag == NATIVE_ARE_ARGS_TRUE:
            return True
        # NATIVE_ARE_ARGS_CALL_IS_COMPAT falls through to the tail.

    if left.required and right.required:
        # If both arguments are required allow_partial_overlap has no effect.
        allow_partial_overlap = False

    def is_different(
        left_item: object | None, right_item: object | None, allow_overlap: bool
    ) -> bool:
        """Checks if the left and right items are different.

        If the right item is unspecified (e.g. if the right callable doesn't care
        about what name or position its arg has), we default to returning False.

        If we're allowing partial overlap, we also default to returning False
        if the left callable also doesn't care."""
        if right_item is None:
            return False
        if allow_overlap and left_item is None:
            return False
        return left_item != right_item

    # If right has a specific name it wants this argument to be, left must
    # have the same.
    if is_different(left.name, right.name, allow_partial_overlap):
        # But pay attention to whether we're ignoring positional arg names
        if not ignore_pos_arg_names or right.pos is None:
            return False

    # If right is at a specific position, left must have the same.
    # TODO: partial overlap logic is flawed for positions.
    # We disable it to avoid false positives at a cost of few false negatives.
    if is_different(left.pos, right.pos, allow_overlap=False) and not allow_imprecise_kinds:
        return False

    # If right's argument is optional, left's must also be
    # (unless we're relaxing the checks to allow potential
    # rather than definite compatibility).
    if not allow_partial_overlap and not right.required and left.required:
        return False

    # If we're allowing partial overlaps and neither arg is required,
    # the types don't actually need to be the same
    if allow_partial_overlap and not left.required and not right.required:
        return True

    # Left must have a more general type
    return is_compat(right.typ, left.typ)


def flip_compat_check(is_compat: Callable[[Type, Type], bool]) -> Callable[[Type, Type], bool]:
    def new_is_compat(left: Type, right: Type) -> bool:
        return is_compat(right, left)

    return new_is_compat


def unify_generic_callable(
    type: NormalizedCallableType,
    target: NormalizedCallableType,
    ignore_return: bool,
    return_constraint_direction: int | None = None,
) -> NormalizedCallableType | None:
    """Try to unify a generic callable type with another callable type.

    Return unified CallableType if successful; otherwise, return None.
    """
    import mypy.solve

    if set(type.type_var_ids()) & {v.id for v in mypy.typeops.get_all_type_vars(target)}:
        # Overload overlap check does nasty things like unifying in opposite direction.
        # This can easily create type variable clashes, so we need to refresh.
        type = freshen_function_type_vars(type)

    if return_constraint_direction is None:
        return_constraint_direction = mypy.constraints.SUBTYPE_OF

    constraints: list[mypy.constraints.Constraint] = []
    # There is some special logic for inference in callables, so better use them
    # as wholes instead of picking separate arguments.
    cs = mypy.constraints.infer_constraints(
        type.copy_modified(ret_type=UninhabitedType()),
        target.copy_modified(ret_type=UninhabitedType()),
        mypy.constraints.SUBTYPE_OF,
        skip_neg_op=True,
    )
    constraints.extend(cs)
    if not ignore_return:
        c = mypy.constraints.infer_constraints(
            type.ret_type, target.ret_type, return_constraint_direction
        )
        constraints.extend(c)
    inferred_vars, _ = mypy.solve.solve_constraints(
        type.variables, constraints, allow_polymorphic=True
    )
    if None in inferred_vars:
        return None
    non_none_inferred_vars = cast(list[Type], inferred_vars)
    had_errors = False

    def report(*args: Any) -> None:
        nonlocal had_errors
        had_errors = True

    # This function may be called by the solver, so we need to allow erased types here.
    # We anyway allow checking subtyping between other types containing <Erased>
    # (probably also because solver needs subtyping). See also comment in

    # ExpandTypeVisitor.visit_erased_type().
    applied = mypy.applytype.apply_generic_arguments(
        type, non_none_inferred_vars, report, context=target
    )
    if had_errors:
        return None
    return cast(NormalizedCallableType, applied)


def try_restrict_literal_union(t: UnionType, s: Type) -> list[Type] | None:
    """Return the items of t, excluding any occurrence of s, if and only if
      - t only contains simple literals
      - s is a simple literal

    Otherwise, returns None
    """
    if _HAS_TYPE_KERNEL and _native_subtype_active and _native_subtype_resolver is not None:
        try:
            result = _type_kernel.rust_try_restrict_literal_union(
                _serialize_type(t),
                _serialize_type(s),
                state.strict_optional,
                _native_subtype_resolver,
            )
        except (AssertionError, NotImplementedError):
            result = None
        if result is not None:
            decoded: list[Type] = []
            for item_bytes in result:
                item = _deserialize_type(bytes(item_bytes))
                if item is None:
                    return None
                decoded.append(item)
            return decoded

    ps = get_proper_type(s)
    if not mypy.typeops.is_simple_literal(ps):
        return None

    new_items: list[Type] = []
    for i in t.relevant_items():
        pi = get_proper_type(i)
        if not mypy.typeops.is_simple_literal(pi):
            return None
        if pi != ps:
            new_items.append(i)
    return new_items


def restrict_subtype_away(t: Type, s: Type, *, consider_runtime_isinstance: bool = True) -> Type:
    """Return t minus s for runtime type assertions.

    If we can't determine a precise result, return a supertype of the
    ideal result (just t is a valid result).

    This is used for type inference of runtime type checks such as
    isinstance(). Currently, this just removes elements of a union type.
    """
    if _HAS_TYPE_KERNEL and _native_subtype_active and _native_subtype_resolver is not None:
        try:
            from mypy.state import state

            result = _type_kernel.rust_restrict_subtype_away(
                _serialize_type(t),
                _serialize_type(s),
                consider_runtime_isinstance,
                state.strict_optional,
                _native_subtype_resolver,
            )
            if result is not None:
                decoded = _deserialize_type(bytes(result))
                if decoded is not None:
                    return decoded
        except (AssertionError, NotImplementedError, ValueError, AttributeError):
            pass
    p_t = get_proper_type(t)
    if isinstance(p_t, UnionType):
        new_items = try_restrict_literal_union(p_t, s)
        if new_items is None:
            new_items = [
                restrict_subtype_away(
                    item, s, consider_runtime_isinstance=consider_runtime_isinstance
                )
                for item in p_t.relevant_items()
            ]
        return UnionType.make_union(
            [item for item in new_items if not isinstance(get_proper_type(item), UninhabitedType)]
        )
    elif isinstance(p_t, TypeVarType):
        return p_t.copy_modified(upper_bound=restrict_subtype_away(p_t.upper_bound, s))

    if consider_runtime_isinstance:
        if covers_at_runtime(t, s):
            return UninhabitedType()
        else:
            return t
    else:
        if is_proper_subtype(t, s, ignore_promotions=True):
            return UninhabitedType()
        if is_proper_subtype(t, s, ignore_promotions=True, erase_instances=True):
            return UninhabitedType()
        return t


def covers_at_runtime(item: Type, supertype: Type) -> bool:
    """Will isinstance(item, supertype) always return True at runtime?"""
    if _HAS_TYPE_KERNEL and _native_subtype_active and _native_subtype_resolver is not None:
        try:
            from mypy.state import state

            result = _type_kernel.rust_covers_at_runtime(
                _serialize_type(item),
                _serialize_type(supertype),
                state.strict_optional,
                _native_subtype_resolver,
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError, ValueError, AttributeError):
            pass
    item = get_proper_type(item)
    supertype = get_proper_type(supertype)

    # Since runtime type checks will ignore type arguments, erase the types.
    if not (isinstance(supertype, FunctionLike) and supertype.is_type_obj()):
        supertype = erase_type(supertype)
    if is_proper_subtype(
        erase_type(item), supertype, ignore_promotions=True, erase_instances=True
    ):
        return True
    if isinstance(supertype, Instance):
        if supertype.type.is_protocol:
            # TODO: Implement more robust support for runtime isinstance() checks, see issue #3827.
            if is_proper_subtype(item, supertype, ignore_promotions=True):
                return True
        if isinstance(item, TypedDictType):
            # Special case useful for selecting TypedDicts from unions using isinstance(x, dict).
            if supertype.type.fullname == "builtins.dict":
                return True
        elif isinstance(item, TypeVarType):
            if is_proper_subtype(item.upper_bound, supertype, ignore_promotions=True):
                return True
        elif isinstance(item, Instance) and supertype.type.fullname == "builtins.int":
            # "int" covers all native int types
            if item.type.fullname in MYPYC_NATIVE_INT_NAMES:
                return True
    # TODO: Add more special cases.
    return False


def is_more_precise(left: Type, right: Type, *, ignore_promotions: bool = False) -> bool:
    """Check if left is a more precise type than right.

    A left is a proper subtype of right, left is also more precise than
    right. Also, if right is Any, left is more precise than right, for
    any left.
    """
    # TODO Should List[int] be more precise than List[Any]?
    right = get_proper_type(right)
    if isinstance(right, AnyType):
        return True
    if _HAS_TYPE_KERNEL and _native_subtype_active and _native_subtype_resolver is not None:
        try:
            result = _type_kernel.rust_is_more_precise(
                _serialize_type(left),
                _serialize_type(right),
                ignore_promotions,
                state.strict_optional,
                _native_subtype_resolver,
            )
        except (AssertionError, NotImplementedError):
            result = None
        if result is not None:
            return result
    return is_proper_subtype(left, right, ignore_promotions=ignore_promotions)


def all_non_object_members(info: TypeInfo) -> set[str]:
    members = set(info.names)
    for base in info.mro[1:-1]:
        members.update(base.names)
    return members


def infer_variance(info: TypeInfo, i: int) -> bool:
    """Infer the variance of the ith type variable of a generic class.

    Return True if successful. This can fail if some inferred types aren't ready.
    """
    object_type = Instance(info.mro[-1], [])

    for variance in COVARIANT, CONTRAVARIANT, INVARIANT:
        tv = info.defn.type_vars[i]
        assert isinstance(tv, TypeVarType)
        if tv.variance != VARIANCE_NOT_READY:
            continue
        tv.variance = variance
        co = True
        contra = True
        tvar = info.defn.type_vars[i]
        self_type = fill_typevars(info)
        for member in all_non_object_members(info):
            # __mypy-replace is an implementation detail of the dataclass plugin
            if member in ("__init__", "__new__", "__mypy-replace"):
                continue

            if isinstance(self_type, TupleType):
                self_type = mypy.typeops.tuple_fallback(self_type)
            flags = get_member_flags(member, self_type)
            settable = IS_SETTABLE in flags

            node = info[member].node
            if isinstance(node, Var):
                if node.type is None:
                    tv.variance = VARIANCE_NOT_READY
                    return False
                if has_underscore_prefix(member):
                    # Special case to avoid false positives (and to pass conformance tests)
                    settable = False

            # TODO: handle settable properties with setter type different from getter.
            typ = find_member(member, self_type, self_type)
            if typ:
                # It's okay for a method in a generic class with a contravariant type
                # variable to return a generic instance of the class, if it doesn't involve
                # variance (i.e. values of type variables are propagated). Our normal rules

                # would disallow this. Replace such return types with 'Any' to allow this.

                # This could probably be more lenient (e.g. allow self type be nested, don't
                # require all type arguments to be identical to self_type), but this will
                # hopefully cover the vast majority of such cases, including Self.
                if _native_variance_member_active():
                    # Native per-member decision: erase_return_self_types +
                    # expand_type + 2x is_subtype folded into a co/contra mask.
                    code = _native_infer_variance_member(typ, self_type, object_type, tvar)
                    if code is not None:
                        if code & 1:
                            co = False
                        if code & 2:
                            contra = False
                        if code & 2 and settable:
                            co = False
                        continue

                typ = erase_return_self_types(typ, self_type)

                typ2 = expand_type(typ, {tvar.id: object_type})
                if not is_subtype(typ, typ2):
                    co = False
                if not is_subtype(typ2, typ):
                    contra = False
                    if settable:
                        co = False

        # Infer variance from base classes, in case they have explicit variances
        for base in info.bases:
            base2 = expand_type(base, {tvar.id: object_type})
            if not is_subtype(base, base2):
                co = False
            if not is_subtype(base2, base):
                contra = False

        if co:
            v = COVARIANT
        elif contra:
            v = CONTRAVARIANT
        else:
            v = INVARIANT
        if v == variance:
            break
        tv.variance = VARIANCE_NOT_READY
    return True


def has_underscore_prefix(name: str) -> bool:
    if _HAS_TYPE_KERNEL and _native_subtype_active:
        return _type_kernel.rust_has_underscore_prefix(name)
    return name.startswith("_") and not (name.startswith("__") and name.endswith("__"))


def infer_class_variances(info: TypeInfo) -> bool:
    if not info.defn.type_args:
        return True
    tvs = info.defn.type_vars
    success = True
    for i, tv in enumerate(tvs):
        if isinstance(tv, TypeVarType) and tv.variance == VARIANCE_NOT_READY:
            if not infer_variance(info, i):
                success = False
    return success


def erase_return_self_types(typ: Type, self_type: Instance) -> Type:
    """If a typ is function-like and returns self_type, replace return type with Any."""
    proper_type = get_proper_type(typ)
    if isinstance(proper_type, CallableType):
        if _native_erase_return_self_types_active():
            try:
                result = _type_kernel.rust_erase_return_self_types(
                    _serialize_type(proper_type), _serialize_type(self_type)
                )
            except (AssertionError, NotImplementedError):
                result = None
            if result is not None:
                decoded = _deserialize_type(bytes(result))
                if decoded is not None:
                    return decoded
        ret = get_proper_type(proper_type.ret_type)
        if isinstance(ret, Instance) and ret == self_type:
            return proper_type.copy_modified(ret_type=AnyType(TypeOfAny.implementation_artifact))
    elif isinstance(proper_type, Overloaded):
        return Overloaded(
            [
                cast(CallableType, erase_return_self_types(it, self_type))
                for it in proper_type.items
            ]
        )
    return typ


def is_erased_instance(t: Instance) -> bool:
    """Is this an instance where all args are Any types?"""
    if _HAS_TYPE_KERNEL and _native_subtype_active:
        try:
            result = _type_kernel.rust_is_erased_instance(_serialize_type(t))
        except (AssertionError, NotImplementedError):
            result = None
        if result is not None:
            return result
    if not t.args:
        return False
    for arg in t.args:
        if isinstance(arg, UnpackType):
            unpacked = get_proper_type(arg.type)
            if not isinstance(unpacked, Instance):
                return False
            assert unpacked.type.fullname == "builtins.tuple"
            if not isinstance(get_proper_type(unpacked.args[0]), AnyType):
                return False
        elif not isinstance(get_proper_type(arg), AnyType):
            return False
    return True
