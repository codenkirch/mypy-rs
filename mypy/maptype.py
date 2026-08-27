from __future__ import annotations

import copy
from typing import Any

from mypy.expandtype import expand_type_by_instance
from mypy.nodes import TypeInfo
from mypy.types import (
    AnyType,
    CallableType,
    Instance,
    TupleType,
    Type,
    TypeAliasType,
    TypeOfAny,
    TypeType,
    UnionType,
    get_proper_type,
    has_type_vars,
    read_type,
    read_type_list,
    write_type_list,
)

# Stage 3c type-kernel seam: when type_kernel is importable and a resolver
# is installed, map_instance_to_supertype routes through Rust. Rust returns
# None for any type it does not handle, in which case we fall back to the

# pure-Python path. This is the strangler-fig per-call gate.

# wire-bytes -> decoded+fixed Instance cache for the supertype seam
# (10K calls, ~82% repeat). Copy-on-hit: callers re-apply per-call
# line/column. Cleared per build + on real typeinfo map replacement.
_map_supertype_decode_cache: dict[bytes, Instance] = {}


def _clear_map_supertype_decode_cache() -> None:
    _map_supertype_decode_cache.clear()


try:
    import type_kernel as _type_kernel
    from librt.internal import (
        ReadBuffer as _ReadBuffer,
        WriteBuffer as _WriteBuffer,
    )

    _HAS_TYPE_KERNEL = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _ReadBuffer = None  # type: ignore[assignment,misc]
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _HAS_TYPE_KERNEL = False

# Module-level flag + resolver, set by the build manager from
# `Options.native_type_kernel` at the start of each build. When
# `_native_map_active` is True but `_native_map_resolver` is None, the

# shim falls through to Python.
_native_map_active: bool = False
_native_map_resolver: Any = None


def _set_native_map_active(active: bool) -> None:
    """Called by the build manager to enable/disable the Rust path."""
    global _native_map_active
    _native_map_active = active


def _set_native_map_resolver(resolver: Any) -> None:
    """Install the `NativeTypeResolver` pyclass for the Rust maptype path.

    Called by the build manager (or the parity test suite) after building
    the resolver from the live TypeInfo graph. Pass `None` to clear.
    Shares the same resolver as the subtype/expand paths.
    """
    global _native_map_resolver
    _native_map_resolver = resolver


def _serialize_type(t: Type) -> bytes:
    """Serialize a `Type` to its wire-format bytes for the Rust reader."""
    buf = _WriteBuffer()
    t.write(buf)
    return buf.getvalue()


def _needs_python(typ: Type) -> bool:
    """True if `typ` nests a node a kernel round-trip cannot carry.

    Named callables lose their FuncDef/Decorator definition node, breaking
    error formatting that names the function; TypeAliasType would loop while
    decoding. Both must defer to the pure-Python path.
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
            if p.definition is not None:
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


def _native_map_instance_to_supertype(
    instance: Instance, superclass: TypeInfo
) -> Instance | None:
    """Try the Rust map_instance_to_supertype path; None defers to Python."""
    if not (
        _HAS_TYPE_KERNEL
        and _native_map_active
        and _native_map_resolver is not None
        and superclass.fullname != "builtins.tuple"
        and not _needs_python(instance)
    ):
        return None
    try:
        result = _type_kernel.rust_map_instance_to_supertype(
            _native_map_resolver,
            instance.type.fullname,
            _serialize_type(instance),
            superclass.fullname,
        )
        if result is None:
            return None
        raw = bytes(result)
        cached = _map_supertype_decode_cache.get(raw)
        if cached is not None:
            # Shallow copy: callers re-apply per-call line/column
            # (mirrors expand_type/checkmember memoization).
            fixed = copy.copy(cached)
            fixed.line = instance.line
            fixed.column = instance.column
            return fixed
        decoded = read_type(_ReadBuffer(raw))
        from mypy.wirefixup import fixup_wire_type

        fixed = fixup_wire_type(decoded)
        if isinstance(fixed, Instance):  # type: ignore[misc]
            # The wire format does not carry line/column; decoded types
            # default to line -1. Preserve the input location so derived
            # contexts report errors at the call site.
            fixed.line = instance.line
            fixed.column = instance.column
            _map_supertype_decode_cache[raw] = fixed
            return fixed
        return None
    except (AssertionError, NotImplementedError, ValueError):
        # AssertionError: TypeInfo not yet fixed during semanal.
        # NotImplementedError: unserializable variant.
        # ValueError: decode/read failure.

        # All defer to Python.
        return None


def map_instance_to_supertype(instance: Instance, superclass: TypeInfo) -> Instance:
    """Produce a supertype of `instance` that is an Instance
    of `superclass`, mapping type arguments up the chain of bases.

    If `superclass` is not a nominal superclass of `instance.type`,
    then all type arguments are mapped to 'Any'.
    """
    if instance.type == superclass:
        # Fast path: `instance` already belongs to `superclass`.
        return instance

    if not superclass.type_vars:
        # Fast path: `superclass` has no type variables to map to.
        return Instance(superclass, [])

    # Stage 3c type-kernel seam: try the Rust map path for the hot case
    # (both fast paths above handle the trivial edges). Rust returns None
    # for unsupported cases; we then fall through to pure Python. Mapping

    # to `builtins.tuple` defers too: the namedtuple tuple_fallback
    # special case is not ported, so Rust would return tuple[Any, ...]
    # instead of the element-preserving tuple fallback.
    native = _native_map_instance_to_supertype(instance, superclass)
    if native is not None:
        return native
    return map_instance_to_supertypes(instance, superclass)[0]


def _native_map_step_frontier(
    members: list[Instance], supertype: TypeInfo
) -> list[Instance] | None:
    """Try the Rust whole per-member supertype-mapping loop; None defers
    to the pure-Python step.

    Serializes the frontier `members` as one wire list and calls
    `rust_map_instance_to_supertypes` once for the whole step (instead of
    one Python->Rust round trip per member). Members Rust cannot map are
    marked by the parallel fallback flags and re-run individually in
    Python (`map_instance_to_direct_supertypes`), so a single unsupported
    member (TypeAlias/definition-carrying Callable/ParamSpec nested in a
    member) never blocks the supported members.
    """
    if not (
        _HAS_TYPE_KERNEL
        and _native_map_active
        and _native_map_resolver is not None
        and supertype.fullname != "builtins.tuple"
    ):
        return None
    if not members:
        return []
    # Members whose wire round-trip cannot carry them defer per-member;
    # the rest go to Rust in one call.
    supported_idx = [i for i, m in enumerate(members) if not _needs_python(m)]
    if not supported_idx:
        return None
    supported = [members[i] for i in supported_idx]
    try:
        buf = _WriteBuffer()
        write_type_list(buf, supported)
        result = _type_kernel.rust_map_instance_to_supertypes(
            _native_map_resolver,
            buf.getvalue(),
            supertype.fullname,
        )
        if result is None:
            return None
        encoded_results, flags = result
        if len(flags) != len(supported):
            # Parallel arrays out of sync: defer the whole step.
            return None
        mapped: list[Instance] = []
        if bytes(encoded_results):
            decoded_all = read_type_list(_ReadBuffer(bytes(encoded_results)))
            from mypy.wirefixup import fixup_wire_type

            for decoded in decoded_all:
                fixed = fixup_wire_type(decoded)
                if not isinstance(fixed, Instance):  # type: ignore[misc]
                    return None
                mapped.append(fixed)
        # Reassemble by index: supported members that Rust mapped take the
        # decoded results in order; everything else re-runs in Python.
        out: list[Instance] = []
        mapped_iter = iter(mapped)
        pos = 0
        for i, member in enumerate(members):
            if i in supported_idx:
                if flags[pos]:
                    out.append(next(mapped_iter))
                else:
                    out.extend(map_instance_to_direct_supertypes(member, supertype))
                pos += 1
            else:
                out.extend(map_instance_to_direct_supertypes(member, supertype))
        return out
    except (AssertionError, NotImplementedError, ValueError):
        # All defer to the pure-Python step.
        return None


def map_instance_to_supertypes(instance: Instance, supertype: TypeInfo) -> list[Instance]:
    # FIX: Currently we should only have one supertype per interface, so no
    #      need to return an array
    result: list[Instance] = []
    for path in class_derivation_paths(instance.type, supertype):
        types = [instance]
        for sup in path:
            # Stage 3c type-kernel seam: one Rust call maps the whole
            # frontier `types`; unmappable members fall back per-member.
            # If the step defers (None), keep the pure-Python loop.
            expanded = _native_map_step_frontier(types, sup)
            if expanded is None:
                a: list[Instance] = []
                for t in types:
                    a.extend(map_instance_to_direct_supertypes(t, sup))
                types = a
            else:
                types = expanded
        result.extend(types)
    if result:
        return result
    else:
        # Nothing. Presumably due to an error. Construct a dummy using Any.
        any_type = AnyType(TypeOfAny.from_error)
        return [Instance(supertype, [any_type] * len(supertype.type_vars))]


def class_derivation_paths(typ: TypeInfo, supertype: TypeInfo) -> list[list[TypeInfo]]:
    """Return an array of non-empty paths of direct base classes from
    type to supertype.  Return [] if no such path could be found.

      InterfaceImplementationPaths(A, B) == [[B]] if A inherits B
      InterfaceImplementationPaths(A, C) == [[B, C]] if A inherits B and
                                                        B inherits C
    """
    # FIX: Currently we might only ever have a single path, so this could be
    #      simplified
    result: list[list[TypeInfo]] = []

    for base in typ.bases:
        btype = base.type
        if btype == supertype:
            result.append([btype])
        else:
            # Try constructing a longer path via the base class.
            for path in class_derivation_paths(btype, supertype):
                result.append([btype] + path)

    return result


def map_instance_to_direct_supertypes(instance: Instance, supertype: TypeInfo) -> list[Instance]:
    # FIX: There should only be one supertypes, always.
    typ = instance.type
    result: list[Instance] = []

    for b in typ.bases:
        if b.type == supertype:

            if supertype.fullname == "builtins.tuple" and instance.type.tuple_type:
                if has_type_vars(instance.type.tuple_type):
                    # We special case mapping generic tuple types to tuple base, because for
                    # such tuples fallback can't be calculated before applying type arguments.
                    alias = instance.type.special_alias
                    assert alias is not None
                    if not alias._is_recursive:
                        # Unfortunately we can't support this for generic recursive tuples.
                        # If we skip this special casing we will fall back to tuple[Any, ...].
                        tuple_type = expand_type_by_instance(instance.type.tuple_type, instance)
                        if isinstance(tuple_type, TupleType):
                            # Make the import here to avoid cyclic imports.
                            import mypy.typeops

                            result.append(mypy.typeops.tuple_fallback(tuple_type))
                            continue
                        elif isinstance(tuple_type, Instance):
                            # This can happen after normalizing variadic tuples.
                            result.append(tuple_type)
                            continue

            t = expand_type_by_instance(b, instance)
            assert isinstance(t, Instance)
            result.append(t)

    if result:
        return result
    else:
        # Relationship with the supertype not specified explicitly. Use dynamic
        # type arguments implicitly.
        any_type = AnyType(TypeOfAny.unannotated)
        return [Instance(supertype, [any_type] * len(supertype.type_vars))]
