"""Utilities for mapping between actual and formal arguments (and their types)."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import TYPE_CHECKING

from mypy import nodes
from mypy.maptype import map_instance_to_supertype
from mypy.types import (
    AnyType,
    Instance,
    ParamSpecType,
    TupleType,
    Type,
    TypedDictType,
    TypeOfAny,
    TypeVarTupleType,
    UnpackType,
    get_proper_type,
)

if TYPE_CHECKING:
    from mypy.infer import ArgumentInferContext

try:
    from librt.internal import WriteBuffer as _ArgMapWriteBuffer

    _HAS_LIBRT = True
except ImportError:
    _ArgMapWriteBuffer = None  # type: ignore[assignment,misc]
    _HAS_LIBRT = False

# Stage 4 type-kernel seam: when the `type_kernel` Rust extension is
# importable and `Options.native_type_kernel` is set, route the pure
# positional/named branches of `map_actuals_to_formals` through Rust. The

# Rust path returns `None` for any call with an ARG_STAR or ARG_STAR2 actual
# (those branches need the `actual_arg_type` callback, which is deferred),
# in which case we fall back to the pure-Python implementation. This is the

# strangler-fig per-call gate, mirroring `erasetype.py` (Stage 1) and
# `subtypes.py` (Stage 3c): no behavior change unless the option is set.
try:
    from type_kernel import (
        rust_expand_actual_type as _rust_expand_actual_type,
        rust_map_actuals_to_formals as _rust_map_actuals_to_formals,
        rust_map_actuals_to_formals_with_types as _rust_map_actuals_to_formals_with_types,
        rust_map_formals_to_actuals as _rust_map_formals_to_actuals,
    )

    _HAS_TYPE_KERNEL = True
except ImportError:
    _rust_expand_actual_type = None  # type: ignore[assignment]
    _rust_map_actuals_to_formals = None  # type: ignore[assignment]
    _rust_map_actuals_to_formals_with_types = None  # type: ignore[assignment]
    _rust_map_formals_to_actuals = None  # type: ignore[assignment]
    _HAS_TYPE_KERNEL = False

# Module-level flag read by the gate below. Set by the build manager from
# `Options.native_type_kernel` at the start of each build, so the hot path
# avoids an options lookup per call.
_native_argmap_active: bool = False

# Decision tags returned by `_rust_expand_actual_type` (mirror expand.rs).
_DECISION_TUPLE = 0
_DECISION_KWARG = 1
_DECISION_PASSTHROUGH = 2
_DECISION_ANY_ERROR = 3


def _set_native_argmap_active(active: bool) -> None:
    """Called by the build manager to enable/disable the Rust argmap path."""
    global _native_argmap_active
    _native_argmap_active = active


def _serialize_actual_type(actual: Type) -> bytes | None:
    """Serialize an actual type for wire inspection; None on write failure.

    Uses `get_proper_type` so the wire carries the resolved
    TypedDictType/TupleType (Python resolves before branching in
    map_actuals_to_formals; the Rust side has no type_state).
    """
    try:
        buf = _ArgMapWriteBuffer()
        get_proper_type(actual).write(buf)
        return buf.getvalue()
    except (AssertionError, NotImplementedError, ValueError):
        return None


def map_actuals_to_formals(
    actual_kinds: list[nodes.ArgKind],
    actual_names: Sequence[str | None] | None,
    formal_kinds: list[nodes.ArgKind],
    formal_names: Sequence[str | None],
    actual_arg_type: Callable[[int], Type],
) -> list[list[int]]:
    """Calculate mapping between actual (caller) args and formals.

    The result contains a list of caller argument indexes mapping to each
    callee argument index, indexed by callee index.

    The actual_arg_type argument should evaluate to the type of the actual
    argument with the given index.
    """
    if _HAS_TYPE_KERNEL and _native_argmap_active:
        # Mirror Python's `assert actual_names is not None` for named kinds:
        # if a named kind is present with no names list, let Python raise the
        # internal error rather than calling Rust with an empty names list.
        has_named = any(k.is_named() for k in actual_kinds)
        if not (has_named and actual_names is None):
            kinds = [int(k.value) for k in actual_kinds]
            names = list(actual_names) if actual_names is not None else []
            fk = [int(k.value) for k in formal_kinds]
            fn = list(formal_names)
            has_star = any(k.is_star() for k in actual_kinds)
            if has_star and _HAS_LIBRT:
                # Serialize each star actual's type so Rust can inspect
                # tuple/TypedDict structure; other actuals get None.
                type_blobs = [
                    _serialize_actual_type(actual_arg_type(ai)) if k.is_star() else None
                    for ai, k in enumerate(actual_kinds)
                ]
                result = _rust_map_actuals_to_formals_with_types(kinds, names, fk, fn, type_blobs)
            else:
                result = _rust_map_actuals_to_formals(kinds, names, fk, fn)
            if result is not None:
                return [list(slot) for slot in result]
            # Rust returned None (star actual present), fall through to Python.
    nformals = len(formal_kinds)
    formal_to_actual: list[list[int]] = [[] for i in range(nformals)]
    ambiguous_actual_kwargs: list[int] = []
    fi = 0
    for ai, actual_kind in enumerate(actual_kinds):
        if actual_kind == nodes.ARG_POS:
            if fi < nformals:
                if not formal_kinds[fi].is_star():
                    formal_to_actual[fi].append(ai)
                    fi += 1
                elif formal_kinds[fi] == nodes.ARG_STAR:
                    formal_to_actual[fi].append(ai)
        elif actual_kind == nodes.ARG_STAR:
            # We need to know the actual type to map varargs.
            actualt = get_proper_type(actual_arg_type(ai))
            if isinstance(actualt, TupleType):
                # A tuple actual maps to a fixed number of formals.
                for _ in range(len(actualt.items)):
                    if fi < nformals:
                        if formal_kinds[fi] != nodes.ARG_STAR2:
                            formal_to_actual[fi].append(ai)
                        else:
                            break
                        if formal_kinds[fi] != nodes.ARG_STAR:
                            fi += 1
            else:
                # Assume that it is an iterable (if it isn't, there will be
                # an error later).
                while fi < nformals:
                    if formal_kinds[fi].is_named(star=True):
                        break
                    else:
                        formal_to_actual[fi].append(ai)
                    if formal_kinds[fi] == nodes.ARG_STAR:
                        break
                    fi += 1
        elif actual_kind.is_named():
            assert actual_names is not None, "Internal error: named kinds without names given"
            name = actual_names[ai]
            if name in formal_names and formal_kinds[formal_names.index(name)] != nodes.ARG_STAR:
                formal_to_actual[formal_names.index(name)].append(ai)
            elif nodes.ARG_STAR2 in formal_kinds:
                formal_to_actual[formal_kinds.index(nodes.ARG_STAR2)].append(ai)
        else:
            assert actual_kind == nodes.ARG_STAR2
            actualt = get_proper_type(actual_arg_type(ai))
            if isinstance(actualt, TypedDictType):
                for name in actualt.items:
                    if name in formal_names:
                        formal_to_actual[formal_names.index(name)].append(ai)
                    elif nodes.ARG_STAR2 in formal_kinds:
                        formal_to_actual[formal_kinds.index(nodes.ARG_STAR2)].append(ai)
            else:
                # We don't exactly know which **kwargs are provided by the
                # caller, so we'll defer until all the other unambiguous
                # actuals have been processed
                ambiguous_actual_kwargs.append(ai)

    if ambiguous_actual_kwargs:
        # Assume the ambiguous kwargs will fill the remaining arguments.
        #
        # TODO: If there are also tuple varargs, we might be missing some potential

        #       matches if the tuple was short enough to not match everything.
        unmatched_formals = [
            fi
            for fi in range(nformals)
            if (
                formal_names[fi]
                and (
                    not formal_to_actual[fi]
                    or actual_kinds[formal_to_actual[fi][0]] == nodes.ARG_STAR
                )
                and formal_kinds[fi] != nodes.ARG_STAR
            )
            or formal_kinds[fi] == nodes.ARG_STAR2
        ]
        for ai in ambiguous_actual_kwargs:
            for fi in unmatched_formals:
                formal_to_actual[fi].append(ai)

    return formal_to_actual


def map_formals_to_actuals(
    actual_kinds: list[nodes.ArgKind],
    actual_names: Sequence[str | None] | None,
    formal_kinds: list[nodes.ArgKind],
    formal_names: list[str | None],
    actual_arg_type: Callable[[int], Type],
) -> list[list[int]]:
    """Calculate the reverse mapping of map_actuals_to_formals."""
    if _HAS_TYPE_KERNEL and _native_argmap_active:
        # Mirror Python's `assert actual_names is not None` for named kinds.
        has_named = any(k.is_named() for k in actual_kinds)
        if not (has_named and actual_names is None):
            result = _rust_map_formals_to_actuals(
                [int(k.value) for k in actual_kinds],
                list(actual_names) if actual_names is not None else [],
                [int(k.value) for k in formal_kinds],
                list(formal_names),
            )
            if result is not None:
                return [list(slot) for slot in result]
            # Rust returned None (star actual present), fall through to Python.
    formal_to_actual = map_actuals_to_formals(
        actual_kinds, actual_names, formal_kinds, formal_names, actual_arg_type
    )
    # Now reverse the mapping.
    actual_to_formal: list[list[int]] = [[] for _ in actual_kinds]
    for formal, actuals in enumerate(formal_to_actual):
        for actual in actuals:
            actual_to_formal[actual].append(formal)
    return actual_to_formal


class ArgTypeExpander:
    """Utility class for mapping actual argument types to formal arguments.

    One of the main responsibilities is to expand caller tuple *args and TypedDict
    **kwargs, and to keep track of which tuple/TypedDict items have already been
    consumed.

    Example:

       def f(x: int, *args: str) -> None: ...
       f(*(1, 'x', 1.1))

    We'd call expand_actual_type three times:

      1. The first call would provide 'int' as the actual type of 'x' (from '1').
      2. The second call would provide 'str' as one of the actual types for '*args'.
      2. The third call would provide 'float' as one of the actual types for '*args'.

    A single instance can process all the arguments for a single call. Each call
    needs a separate instance since instances have per-call state.
    """

    def __init__(self, context: ArgumentInferContext) -> None:
        # Next tuple *args index to use.
        self.tuple_index = 0
        # Keyword arguments in TypedDict **kwargs used.
        self.kwargs_used: set[str] | None = None
        # Type context for `*` and `**` arg kinds.
        self.context = context

    def expand_actual_type(
        self,
        actual_type: Type,
        actual_kind: nodes.ArgKind,
        formal_name: str | None,
        formal_kind: nodes.ArgKind,
        allow_unpack: bool = False,
    ) -> Type:
        """Return the actual (caller) type(s) of a formal argument with the given kinds.

        If the actual argument is a tuple *args, return the next individual tuple item that
        maps to the formal arg.

        If the actual argument is a TypedDict **kwargs, return the next matching typed dict
        value type based on formal argument name and kind.

        This is supposed to be called for each formal, in order. Call multiple times per
        formal if multiple actuals map to a formal.
        """
        original_actual = actual_type
        actual_type = get_proper_type(actual_type)
        if (
            _HAS_TYPE_KERNEL
            and _native_argmap_active
            and _HAS_LIBRT
            and (
                (actual_kind == nodes.ARG_STAR and isinstance(actual_type, TupleType))
                or (actual_kind == nodes.ARG_STAR2 and isinstance(actual_type, TypedDictType))
            )
        ):
            # Stage 4 seam: the structural branches (tuple *args item indexing,
            # TypedDict **kwargs key carving) are pure structure, so Rust
            # resolves them and this shim executes the decision against our

            # own Type objects. Rust returns None for anything needing
            # `is_subtype` (Iterable/Mapping unpacking) or an undecodable
            # blob; fall through to Python then.
            try:
                buf = _ArgMapWriteBuffer()
                actual_type.write(buf)
                kwargs_used = sorted(self.kwargs_used) if self.kwargs_used is not None else []
                result = _rust_expand_actual_type(
                    buf.getvalue(),
                    int(actual_kind.value),
                    formal_name,
                    int(formal_kind.value),
                    allow_unpack,
                    self.tuple_index,
                    kwargs_used,
                )
            except (AssertionError, NotImplementedError, ValueError):
                result = None
            if result is not None:
                decision, name, new_tuple_index, _ = result
                if decision == _DECISION_TUPLE:
                    self.tuple_index = new_tuple_index
                    assert isinstance(actual_type, TupleType)
                    item = actual_type.items[self.tuple_index - 1]
                    if isinstance(item, UnpackType) and not allow_unpack:
                        # An unpack item that doesn't have special handling,
                        # use upper bound as above (mirror Python's branch).
                        unpacked = get_proper_type(item.type)
                        if isinstance(unpacked, TypeVarTupleType):
                            fallback = get_proper_type(unpacked.upper_bound)
                        else:
                            fallback = unpacked
                        assert (
                            isinstance(fallback, Instance)
                            and fallback.type.fullname == "builtins.tuple"
                        )
                        item = fallback.args[0]
                    return item
                elif decision == _DECISION_KWARG:
                    assert name is not None
                    assert isinstance(actual_type, TypedDictType)
                    if self.kwargs_used is None:
                        self.kwargs_used = set()
                    self.kwargs_used.add(name)
                    return actual_type.items[name]
        if actual_kind == nodes.ARG_STAR:
            if isinstance(actual_type, TypeVarTupleType):
                # This code path is hit when *Ts is passed to a callable and various
                # special-handling didn't catch this. The best thing we can do is to use
                # the upper bound.
                actual_type = get_proper_type(actual_type.upper_bound)
            if isinstance(actual_type, Instance) and actual_type.args:
                from mypy.subtypes import is_subtype

                if is_subtype(actual_type, self.context.iterable_type):
                    return map_instance_to_supertype(
                        actual_type, self.context.iterable_type.type
                    ).args[0]
                else:
                    # We cannot properly unpack anything other
                    # than `Iterable` type with `*`.
                    # Just return `Any`, other parts of code would raise

                    # a different error for improper use.
                    return AnyType(TypeOfAny.from_error)
            elif isinstance(actual_type, TupleType):
                # Get the next tuple item of a tuple *arg.
                if self.tuple_index >= len(actual_type.items):
                    # Exhausted a tuple -- continue to the next *args.
                    self.tuple_index = 1
                else:
                    self.tuple_index += 1
                item = actual_type.items[self.tuple_index - 1]
                if isinstance(item, UnpackType) and not allow_unpack:
                    # An unpack item that doesn't have special handling, use upper bound as above.
                    unpacked = get_proper_type(item.type)
                    if isinstance(unpacked, TypeVarTupleType):
                        fallback = get_proper_type(unpacked.upper_bound)
                    else:
                        fallback = unpacked
                    assert (
                        isinstance(fallback, Instance)
                        and fallback.type.fullname == "builtins.tuple"
                    )
                    item = fallback.args[0]
                return item
            elif isinstance(actual_type, ParamSpecType):
                # ParamSpec is valid in *args but it can't be unpacked.
                return actual_type
            else:
                return AnyType(TypeOfAny.from_error)
        elif actual_kind == nodes.ARG_STAR2:
            from mypy.subtypes import is_subtype

            if isinstance(actual_type, TypedDictType):
                if self.kwargs_used is None:
                    self.kwargs_used = set()
                if formal_kind != nodes.ARG_STAR2 and formal_name in actual_type.items:
                    # Lookup type based on keyword argument name.
                    assert formal_name is not None
                else:
                    # Pick an arbitrary item if no specified keyword is expected.
                    formal_name = (set(actual_type.items.keys()) - self.kwargs_used).pop()
                self.kwargs_used.add(formal_name)
                return actual_type.items[formal_name]
            elif isinstance(actual_type, Instance) and is_subtype(
                actual_type, self.context.mapping_type
            ):
                # Only `Mapping` type can be unpacked with `**`.
                # Other types will produce an error somewhere else.
                return map_instance_to_supertype(actual_type, self.context.mapping_type.type).args[
                    1
                ]
            elif isinstance(actual_type, ParamSpecType):
                # ParamSpec is valid in **kwargs but it can't be unpacked.
                return actual_type
            else:
                return AnyType(TypeOfAny.from_error)
        else:
            # No translation for other kinds -- 1:1 mapping.
            return original_actual
