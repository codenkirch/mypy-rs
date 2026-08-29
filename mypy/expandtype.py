from __future__ import annotations

import copy
from collections.abc import Iterable, Mapping
from typing import Any, Final, TypeVar, cast, overload

from mypy.nodes import ARG_STAR, ArgKind, FakeInfo, Var
from mypy.state import state
from mypy.types import (
    ANY_STRATEGY,
    AnyType,
    BoolTypeQuery,
    CallableType,
    DeletedType,
    ErasedType,
    FunctionLike,
    Instance,
    LiteralType,
    NoneType,
    Overloaded,
    Parameters,
    ParamSpecFlavor,
    ParamSpecType,
    PartialType,
    ProperType,
    TrivialSyntheticTypeTranslator,
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
    UnboundType,
    UninhabitedType,
    UnionType,
    UnpackType,
    _serialize_with_taint_check,
    _type_wire_cache,
    _wire_cache_enabled,
    flatten_nested_unions,
    get_proper_type,
    read_type,
    split_with_prefix_and_suffix,
)
from mypy.typevartuples import split_with_instance

# Solving the import cycle:
import mypy.type_visitor  # ruff: isort: skip

# WARNING: these functions should never (directly or indirectly) depend on
# is_subtype(), meet_types(), join_types() etc.
# TODO: add a static dependency test for this.

# Stage 3c type-kernel seam: when type_kernel is importable and a resolver
# is installed, expand_type routes through Rust. Rust returns None for any
# type it does not handle, in which case we fall back to the pure-Python

# visitor. This is the strangler-fig per-call gate.


# wire-bytes -> decoded+fixed Type cache for the expand_type seam
# (74K calls, ~98% repeat). Copy-on-hit: callers apply per-call
# line/column. Cleared per build + on real typeinfo map replacement.
_expand_type_decode_cache: dict[bytes, Type] = {}
_expand_remove_trivial_cache: dict[bytes, list[Type]] = {}


def _clear_expand_decode_cache() -> None:
    _expand_type_decode_cache.clear()
    _expand_remove_trivial_cache.clear()


def _canonicalize_fresh_type_list(types: list[Type]) -> tuple[list[Type], bool]:
    """Wire-path identity repair for a decoded list; reports fresh-var presence."""
    from mypy.wirefixup import canonicalize_fresh_vars_reported_list

    return canonicalize_fresh_vars_reported_list(types)


def _needs_python(typ: Type, *, definition_gate: bool = True, meta_gate: bool = False) -> bool:
    """True if `typ` nests a node a kernel round-trip cannot carry.

    Callers that re-stamp dropped ``definition`` links after the round-trip
    via ``_resync_definitions`` pass ``definition_gate=False``; callers
    without a re-stamp path (env values, remove_trivial) keep the gate.
    freshen_function_type_vars also keeps the gate (#1220): its decoded
    result re-enters error reporting as a plugin context, where nested
    wire-decoded types carry no locations.
    Callers whose decoded result is a partial list that cannot be
    re-unified with an enclosing variables slot (remove_trivial) pass
    ``meta_gate=True``: a decoded fresh (meta) type var is a distinct
    object, so in-place id mutation by ``freeze_all_type_vars`` would
    miss it. Recursive TypeAliasType would loop while decoding and must
    defer.
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
        elif isinstance(p, (ParamSpecType, TypeVarTupleType)):
            # TypeVarTuple/ParamSpec always defer: the Rust remove_trivial
            # dedups structurally and can drop distinct unresolved
            # `Unpack[tuple[Never, ...]]` items, corrupting union results.
            return True
        elif isinstance(p, TypeVarType):
            # Meta relax: whole-tree callers re-unify via canonicalize_fresh_vars
            # and may take fresh (meta) vars across the wire. Partial-list
            # callers pass meta_gate=True (decoded meta vars cannot re-unify).
            if p.has_default() or (meta_gate and p.id.meta_level > 0):
                return True
        elif isinstance(p, UnpackType):
            # Walk through Unpack: `Unpack[tuple[Never, ...]]` nests a
            # TypeVarTupleType only inside the wrapped type.
            stack.append(p.type)
        elif isinstance(p, Instance):
            stack.extend(p.args)
        elif isinstance(p, UnionType):
            stack.extend(p.items)
        elif isinstance(p, TupleType):
            stack.extend(p.items)
        elif isinstance(p, TypeType):
            stack.append(p.item)
    return False


def _contains_alias_raw(t: Type) -> bool:
    """True if any node in the raw type tree is a TypeAliasType.

    Unlike ``_needs_python`` (which walks ``get_proper_type`` and so hides
    non-recursive aliases behind their expansion), this walks the raw tree,
    mirroring the Rust ``result_contains_typealias`` scan.
    """
    stack: list[Type] = [t]
    visited: set[int] = set()
    while stack:
        typ = stack.pop()
        if id(typ) in visited:
            continue
        visited.add(id(typ))
        if isinstance(typ, TypeAliasType):
            return True
        # Deliberate raw-tree walk (no get_proper_type expansion): detect the
        # literal alias nodes the substitution would round-trip through wire.
        if isinstance(typ, Instance):  # type: ignore[misc]
            stack.extend(typ.args)
        elif isinstance(typ, CallableType):  # type: ignore[misc]
            stack.append(typ.ret_type)
            stack.extend(typ.arg_types)
            for v in typ.variables:
                stack.append(v.upper_bound)
                stack.append(v.default)
        elif isinstance(typ, UnionType):  # type: ignore[misc]
            stack.extend(typ.items)
        elif isinstance(typ, TupleType):  # type: ignore[misc]
            stack.extend(typ.items)
        elif isinstance(typ, TypeType):  # type: ignore[misc]
            stack.append(typ.item)
        elif isinstance(typ, UnpackType):
            stack.append(typ.type)
    return False


def _env_substitutes_unsafe(typ: Type, env: Mapping[TypeVarId, Type]) -> bool:
    """True if substituting `env` into `typ` would introduce an unsafe node.

    Only the env entries for typevars actually referenced by `typ` are
    substituted by expand_type, so only those values can lose a definition
    or alias on a kernel round-trip. Walking just those bounds the cost to
    the substituted subset, keeping large unrelated envs (e.g. big dict
    literals) on the fast kernel path.
    """
    used: set[tuple[int, str]] = set()
    stack: list[Type] = [typ]
    visited: set[int] = set()
    while stack:
        t = stack.pop()
        p = get_proper_type(t)
        if id(p) in visited:
            continue
        visited.add(id(p))
        if isinstance(p, TypeVarType):
            used.add((p.id.raw_id, p.id.namespace))
        elif isinstance(p, Instance):
            stack.extend(p.args)
        elif isinstance(p, CallableType):
            for v in p.variables:
                used.add((v.id.raw_id, v.id.namespace))
            stack.append(p.ret_type)
            stack.extend(p.arg_types)
        elif isinstance(p, UnionType):
            stack.extend(p.items)
        elif isinstance(p, TupleType):
            stack.extend(p.items)
        elif isinstance(p, TypeType):
            stack.append(p.item)
    return any(
        (v.raw_id, v.namespace) in used
        # Env values carrying a TypeAliasType stay on the Python path: the
        # substituted value would round-trip the wire as a decoded alias
        # node; keeping the original live node by identity is safer.
        and (_needs_python(t) or _contains_alias_raw(t))
        for v, t in env.items()
    )


try:
    import type_kernel as _type_kernel
    from librt.internal import (
        ReadBuffer as _ReadBuffer,
        WriteBuffer as _WriteBuffer,
        write_int as _write_int_bare,
        write_str as _write_str_tagged,
    )

    _HAS_TYPE_KERNEL = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _ReadBuffer = None  # type: ignore[assignment,misc]
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _write_int_bare = None  # type: ignore[assignment]
    _write_str_tagged = None  # type: ignore[assignment]
    _HAS_TYPE_KERNEL = False

# Module-level flag + resolver, set by the build manager from
# `Options.native_type_kernel` at the start of each build. When
# `_native_expand_type_active` is True but `_native_expand_type_resolver`

# is None, the shim falls through to Python.
_native_expand_type_active: bool = False
_native_expand_type_resolver: Any = None
# fullname -> TypeInfo map, used to resolve wire-decoded `type_ref`
# strings to live TypeInfo. Shares the same map the join path installs.
_native_expand_type_typeinfo_map: dict[str, Any] | None = None


def _set_native_expand_type_active(active: bool) -> None:
    """Called by the build manager to enable/disable the Rust path."""
    global _native_expand_type_active
    _native_expand_type_active = active


def _set_native_expand_type_resolver(resolver: Any) -> None:
    """Install the `NativeTypeResolver` pyclass for the Rust expand path.

    Called by the build manager (or the parity test suite) after building
    the resolver from the live TypeInfo graph. Pass `None` to clear.
    Shares the same resolver as the subtype/join paths.
    """
    global _native_expand_type_resolver
    _native_expand_type_resolver = resolver


def _set_native_expand_type_typeinfo_map(typeinfo_map: dict[str, Any] | None) -> None:
    """Install the fullname -> TypeInfo map for type_ref resolution.

    Delegates to the shared wirefixup module. Kept for build.py
    compatibility (build.py calls this alongside the join setter).
    """
    global _native_expand_type_typeinfo_map
    _native_expand_type_typeinfo_map = typeinfo_map
    from mypy.wirefixup import set_wire_typeinfo_map

    set_wire_typeinfo_map(typeinfo_map)


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
    if not saw_tvar and _wire_cache_enabled() and (type(t) is not Instance or t.type_ref is None):
        _type_wire_cache[key] = (t, result)
    return result


def _serialize_env(env: Mapping[TypeVarId, Type]) -> bytes:
    """Serialize a `Mapping[TypeVarId, Type]` to the env wire format.

    Layout: count (bare int) + pairs of (TypeVarId raw_id bare int +
    TypeVarId meta_level bare int + TypeVarId namespace tagged str +
    Type blob). Mirrors the Rust `decode_env` reader in expandtype.rs.
    """
    buf = _WriteBuffer()
    _write_int_bare(buf, len(env))
    for tv_id, typ in env.items():
        _write_int_bare(buf, tv_id.raw_id)
        _write_int_bare(buf, tv_id.meta_level)
        _write_str_tagged(buf, tv_id.namespace)
        typ.write(buf)
    return buf.getvalue()


@overload
def expand_type(typ: CallableType, env: Mapping[TypeVarId, Type]) -> CallableType: ...


@overload
def expand_type(typ: ProperType, env: Mapping[TypeVarId, Type]) -> ProperType: ...


@overload
def expand_type(typ: Type, env: Mapping[TypeVarId, Type]) -> Type: ...


def expand_type(typ: Type, env: Mapping[TypeVarId, Type]) -> Type:
    """Substitute any type variable references in a type given by a type
    environment.
    """
    # Stage 3c type-kernel seam: try the Rust expand_type path. Rust
    # returns None for unsupported cases (ParamSpec, TypeAliasType, etc.);
    # we then fall through to the pure-Python visitor. Mirrors the
    # erasetype.py strangler-fig contract.
    if (
        _HAS_TYPE_KERNEL
        and _native_expand_type_active
        and _native_expand_type_resolver is not None
        and not _needs_python(typ, definition_gate=False)
        and not _env_substitutes_unsafe(typ, env)
    ):
        try:
            result = _type_kernel.rust_expand_type(
                _native_expand_type_resolver,
                _serialize_type(typ),
                _serialize_env(env),
                state.strict_optional,
            )
            if result is not None:
                raw = bytes(result)
                cached = _expand_type_decode_cache.get(raw)
                if cached is not None and isinstance(cached, ProperType):
                    # Shallow copy: callers mutate top-level line/column;
                    # identical blobs must not cross-contaminate sites
                    # applying different locations.
                    fixed = copy.copy(cached)
                    fixed.line = typ.line
                    fixed.column = typ.column
                    if isinstance(fixed, CallableType):
                        fixed.fallback.line = fixed.line
                    fixed = _resync_definitions(typ, fixed)
                    if fixed is not None:
                        return fixed
                else:
                    decoded = read_type(_ReadBuffer(raw))
                    from mypy.wirefixup import canonicalize_fresh_vars_reported, fixup_wire_type

                    # resolve_aliases=True re-links wire-decoded TypeAliasType
                    # nodes to live TypeAlias nodes; an alias missing from the
                    # per-build map defers (None) to the pure-Python body.
                    fixed = fixup_wire_type(decoded, resolve_aliases=True)
                    # The wire format does not carry line/column; decoded
                    # types default to line -1. Preserve the input type's
                    # location so derived contexts (e.g. plugin

                    # default_return_type) report errors at the call site
                    # instead of a phantom line 0/-1.
                    if fixed is not None and isinstance(fixed, ProperType):
                        fixed.line = typ.line
                        fixed.column = typ.column
                        if isinstance(fixed, CallableType):
                            fixed.fallback.line = fixed.line
                    # Clear the process-global primitive decode singletons after
                    # a read so NOT_READY Instances cannot leak into later builds
                    # (read_type lazily fills instance_cache with

                    # Instance(NOT_READY, []) singletons for str/int/bool/etc).
                    from mypy.types import instance_cache

                    instance_cache.int_type = None
                    instance_cache.str_type = None
                    instance_cache.bool_type = None
                    instance_cache.object_type = None
                    instance_cache.function_type = None
                    if fixed is not None:
                        # The wire round-trip splits fresh meta-var occurrences
                        # into distinct objects: re-unify them before a downstream
                        # in-place freeze, and keep those trees out of the cache.
                        fixed, has_fresh = canonicalize_fresh_vars_reported(fixed)
                        if not has_fresh:
                            # Cache the definition-less decoded shape; each
                            # call re-stamps definitions from its own input
                            # type (see the cached branch above).
                            _expand_type_decode_cache[raw] = fixed
                        fixed = _resync_definitions(typ, fixed)
                        if fixed is not None:
                            return fixed
        except (NotImplementedError, AssertionError):
            # AssertionError: TypeInfo not yet fixed during semanal.
            # NotImplementedError: unserializable variant.
            # Both defer to Python.
            pass
    return typ.accept(ExpandTypeVisitor(env))


def _needs_definitions(typ: Type) -> bool:
    """True if any callable anywhere in `typ` carries a definition node."""
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
        elif isinstance(p, Overloaded):
            stack.extend(p.items)
        elif isinstance(p, Instance):
            stack.extend(p.args)
        elif isinstance(p, UnionType):
            stack.extend(p.items)
        elif isinstance(p, TupleType):
            stack.extend(p.items)
        elif isinstance(p, TypeType):
            stack.append(p.item)
        elif isinstance(p, UnpackType):
            stack.append(p.type)
    return False


def _resync_definitions(original: Type, decoded: Type) -> Type | None:
    """Re-stamp ``definition`` links the wire round-trip dropped.

    The wire format carries no ``CallableType.definition``; the pure-Python
    visitor preserves those links and a lost one changes error messages
    that name the function (``pretty_callable``). Walk ``original`` and
    ``decoded`` in parallel wherever substitution preserves structure and
    copy definitions onto the decoded nodes. Return None when the trees
    diverge below an original node that still carries definitions, so the
    caller defers to the pure-Python visitor (parity over speed).
    """
    if not _needs_definitions(original):
        return decoded
    return _resync_definitions_inner(original, decoded)


def _resync_definitions_inner(original: Type, decoded: Type) -> Type | None:
    """Pairwise implementation of `_resync_definitions`.

    Runs on raw nodes: the callers' gates exclude TypeAliasType inputs
    (the kernel defers any result carrying an alias node), so no alias
    unwrapping is needed here.
    """
    o = original
    d = decoded
    if type(o) is CallableType and type(d) is CallableType:
        if len(o.arg_types) != len(d.arg_types):
            return None
        new_def = o.definition if o.definition is not None else d.definition
        ret = _resync_definitions_inner(o.ret_type, d.ret_type)
        if ret is None:
            return None
        new_args = []
        for po, pd in zip(o.arg_types, d.arg_types):
            pa = _resync_definitions_inner(po, pd)
            if pa is None:
                return None
            new_args.append(pa)
        return d.copy_modified(definition=new_def, ret_type=ret, arg_types=new_args)
    if type(o) is Overloaded and type(d) is Overloaded:
        if len(o.items) != len(d.items):
            return None
        new_items = []
        for po, pd in zip(o.items, d.items):
            pi = _resync_definitions_inner(po, pd)
            if pi is None:
                return None
            new_items.append(pi)
        return Overloaded(new_items)  # type: ignore[arg-type]
    if type(o) is Overloaded and type(d) is CallableType:
        # ETBI on an overload can return a callable after bind_self; the
        # legacy top-level heuristic picked a same-arity item definition.
        for orig in o.items:
            if orig.definition is not None and len(orig.arg_types) == len(d.arg_types):
                return d.copy_modified(definition=orig.definition)
        return None
    if type(o) is Instance and type(d) is Instance:
        if len(o.args) != len(d.args):
            return None
        new_args = []
        for po, pd in zip(o.args, d.args):
            pa = _resync_definitions_inner(po, pd)
            if pa is None:
                return None
            new_args.append(pa)
        return d.copy_modified(args=new_args)
    if type(o) is UnionType and type(d) is UnionType:
        if len(o.items) != len(d.items):
            return None
        new_items = []
        for po, pd in zip(o.items, d.items):
            pi = _resync_definitions_inner(po, pd)
            if pi is None:
                return None
            new_items.append(pi)
        return UnionType(new_items)
    if type(o) is TupleType and type(d) is TupleType:
        if len(o.items) != len(d.items):
            return None
        new_items = []
        for po, pd in zip(o.items, d.items):
            pi = _resync_definitions_inner(po, pd)
            if pi is None:
                return None
            new_items.append(pi)
        return d.copy_modified(items=new_items)
    if type(o) is TypeType and type(d) is TypeType:
        pi = _resync_definitions_inner(o.item, d.item)
        if pi is None:
            return None
        return TypeType.make_normalized(pi)
    if type(o) is UnpackType and type(d) is UnpackType:
        pi = _resync_definitions_inner(o.type, d.type)
        if pi is None:
            return None
        return UnpackType(pi)
    if type(o) is TypeVarType and type(d) is TypeVarType:
        # Only an upper bound can nest other typevars and thus callables.
        pb = _resync_definitions_inner(o.upper_bound, d.upper_bound)
        if pb is None:
            return None
        return d.copy_modified(upper_bound=pb)
    # Divergent node kinds (leaf types): nothing to re-stamp. The top-level
    # check proved no unpaired definitions sit below a pairing node, and a
    # leaf-level kind mismatch means no callable is involved at all.
    return d


@overload
def expand_type_by_instance(typ: CallableType, instance: Instance) -> CallableType: ...


@overload
def expand_type_by_instance(typ: ProperType, instance: Instance) -> ProperType: ...


@overload
def expand_type_by_instance(typ: Type, instance: Instance) -> Type: ...


def expand_type_by_instance(typ: Type, instance: Instance) -> Type:
    """Substitute type variables in type using values from an Instance.
    Type variables are considered to be bound by the class declaration."""
    if not instance.args and not instance.type.has_type_var_tuple_type:
        return typ
    else:
        # Stage 3c type-kernel seam: try Rust expand_type_by_instance.
        # Rust returns None for unsupported cases (TypeVarTuple, arg
        # count mismatch, unmatched typevars); falls back to Python.
        if (
            _HAS_TYPE_KERNEL
            and _native_expand_type_active
            and _native_expand_type_resolver is not None
            and not _needs_python(typ, definition_gate=False)
            and not any(
                _needs_python(a, definition_gate=False)
                for a in instance.args
            )
        ):
            try:
                result = _type_kernel.rust_expand_type_by_instance(
                    _native_expand_type_resolver,
                    _serialize_type(typ),
                    _serialize_type(instance),
                    state.strict_optional,
                )
                if result is not None:
                    decoded = read_type(_ReadBuffer(bytes(result)))
                    from mypy.wirefixup import canonicalize_fresh_vars, fixup_wire_type

                    fixed = fixup_wire_type(decoded)
                    # The wire round-trip splits fresh meta-var occurrences
                    # into distinct objects: re-unify them so a downstream
                    # in-place freeze touches every occurrence (pre-stamping).
                    if fixed is not None:
                        fixed = canonicalize_fresh_vars(fixed)
                    # The wire format does not carry line/column; decoded
                    # types default to line -1. Preserve the input type's
                    # location so derived contexts report errors at the
                    # call site instead of a phantom line 0/-1.
                    if fixed is not None and isinstance(fixed, ProperType):
                        fixed.line = typ.line
                        fixed.column = typ.column
                        if isinstance(fixed, CallableType):
                            fixed.fallback.line = fixed.line
                        # Definitions are dropped by the wire round-trip;
                        # re-stamp from the pre-seam type (None defers
                        # to the Python visitor).
                        fixed = _resync_definitions(typ, fixed)
                    from mypy.types import instance_cache

                    instance_cache.int_type = None
                    instance_cache.str_type = None
                    instance_cache.bool_type = None
                    instance_cache.object_type = None
                    instance_cache.function_type = None
                    if fixed is not None:
                        return fixed
            except (NotImplementedError, AssertionError):
                # AssertionError: TypeInfo not yet fixed during semanal.
                # NotImplementedError: unserializable variant.
                # Both defer to Python.
                pass
        variables: dict[TypeVarId, Type] = {}
        if instance.type.has_type_var_tuple_type:
            assert instance.type.type_var_tuple_prefix is not None
            assert instance.type.type_var_tuple_suffix is not None

            args_prefix, args_middle, args_suffix = split_with_instance(instance)
            tvars_prefix, tvars_middle, tvars_suffix = split_with_prefix_and_suffix(
                tuple(instance.type.defn.type_vars),
                instance.type.type_var_tuple_prefix,
                instance.type.type_var_tuple_suffix,
            )
            tvar = tvars_middle[0]
            assert isinstance(tvar, TypeVarTupleType)
            variables = {tvar.id: TupleType(list(args_middle), tvar.tuple_fallback)}
            instance_args = args_prefix + args_suffix
            tvars = tvars_prefix + tvars_suffix
        else:
            tvars = tuple(instance.type.defn.type_vars)
            instance_args = instance.args

        for binder, arg in zip(tvars, instance_args):
            assert isinstance(binder, TypeVarLikeType)
            variables[binder.id] = arg

        result = expand_type(typ, variables)
        # The pure-Python visitor's union path re-decodes ret occurrences
        # through the native remove_trivial seam: re-unify them so callers
        # that mutate the vars in place (freeze) see every occurrence.
        from mypy.wirefixup import canonicalize_fresh_vars

        return canonicalize_fresh_vars(result)


F = TypeVar("F", bound=FunctionLike)


def freshen_function_type_vars(callee: F) -> F:
    """Substitute fresh type variables for generic function type variables.

    The definition gate stays ON for this seam (#1220): its result is handed
    to plugin FunctionContext hooks as ``ctx.default_return_type`` and
    re-enters error reporting via ``extract_callable_type(...,
    ctx=ctx.default_return_type)`` -> ``analyze_member_access(context=...)``.
    A wire-decoded result carries no nested locations, so a locally-built
    error loses its line number (functools.partial path, "int" not callable).
    Keep deferring on any definition until positions below the root are
    re-established.
    """
    if isinstance(callee, CallableType):
        if not callee.is_generic():
            return callee
        # Stage 3c type-kernel seam (mirrors expand_type's strangler-fig
        # contract); see the docstring for the #1220 definition-gate scope.
        if (
            _HAS_TYPE_KERNEL
            and _native_expand_type_active
            and _native_expand_type_resolver is not None
            and not _needs_python(callee)
        ):
            try:
                result = _type_kernel.rust_freshen_function_type_vars(
                    TypeVarId.next_raw_id, _serialize_type(callee)
                )
                if result is not None:
                    next_raw_id, serialized = result
                    TypeVarId.next_raw_id = next_raw_id
                    decoded = read_type(_ReadBuffer(bytes(serialized)))
                    from mypy.wirefixup import fixup_wire_type

                    fixed = fixup_wire_type(decoded)
                    # The wire format drops line/column; preserve the input type's
                    # location so derived contexts report errors at the
                    # call site instead of a phantom line 0/-1.
                    if fixed is not None and isinstance(fixed, ProperType):
                        fixed.line = callee.line
                        fixed.column = callee.column
                        if isinstance(fixed, CallableType):
                            fixed.fallback.line = fixed.line
                    # Clear the process-global primitive decode singletons
                    # after a read so NOT_READY Instances cannot leak into
                    # later builds (see expand_type).
                    from mypy.types import instance_cache

                    instance_cache.int_type = None
                    instance_cache.str_type = None
                    instance_cache.bool_type = None
                    instance_cache.object_type = None
                    instance_cache.function_type = None
                    if fixed is not None:
                        from mypy.wirefixup import canonicalize_fresh_vars

                        # Wire round-trip loses fresh meta-var identity;
                        # re-unify occurrences before returning.
                        fixed = canonicalize_fresh_vars(fixed)
                        # Definitions are dropped by the wire round-trip;
                        # re-stamp from the input (None = unpairable,
                        # defer to the Python path below).
                        fixed = _resync_definitions(callee, fixed)
                        if fixed is not None:
                            return cast(F, fixed)
            except (AssertionError, NotImplementedError, ValueError, AttributeError):
                # Defer to Python: semanal TypeInfo-not-fixed asserts,
                # unserializable variants, failed wire reads, FakeInfo
                # attribute access.
                pass
        tvs = []
        tvmap: dict[TypeVarId, Type] = {}
        for v in callee.variables:
            tv = v.new_unification_variable(v)
            tvs.append(tv)
            tvmap[v.id] = tv
            if tv.has_default():
                # Point to fresh ids in case defaults depend on previous variables.
                tv.default = expand_type(tv.default, tvmap)
        fresh = expand_type(callee, tvmap).copy_modified(variables=tvs)
        from mypy.wirefixup import canonicalize_fresh_vars

        # The expand_type kernel seam re-decodes occurrences as distinct
        # objects: unify them onto the freshened tvs (pre-registered via
        # seed) so the variables slot and occurrences share identity for
        # downstream in-place freeze.
        return cast(F, canonicalize_fresh_vars(fresh, seed=tvs))
    else:
        assert isinstance(callee, Overloaded)
        fresh_overload = Overloaded([freshen_function_type_vars(item) for item in callee.items])
        return cast(F, fresh_overload)


class HasGenericCallable(BoolTypeQuery):
    def __init__(self) -> None:
        super().__init__(ANY_STRATEGY)

    def visit_callable_type(self, t: CallableType) -> bool:
        return t.is_generic() or super().visit_callable_type(t)


# Share a singleton since this is performance sensitive
has_generic_callable: Final = HasGenericCallable()


T = TypeVar("T", bound=Type)


def freshen_all_functions_type_vars(t: T) -> T:
    result: Type
    has_generic_callable.reset()
    if not t.accept(has_generic_callable):
        return t  # Fast path to avoid expensive freshening
    else:
        # Stage 3c type-kernel seam: try the Rust freshen path first. Rust
        # returns None for unsupported cases (Overloaded, ParamSpec vars,
        # TypeAliasType), then we fall through to the pure-Python visitor.
        if (
            _HAS_TYPE_KERNEL
            and _native_expand_type_active
            and _native_expand_type_resolver is not None
            and not _needs_python(t, definition_gate=False)
        ):
            try:
                call = _type_kernel.rust_freshen_all_functions_type_vars(
                    TypeVarId.next_raw_id, _serialize_type(t), state.strict_optional
                )
                if call is not None:
                    next_raw_id, changed, serialized = call
                    if changed:
                        TypeVarId.next_raw_id = next_raw_id
                        decoded = read_type(_ReadBuffer(bytes(serialized)))
                        from mypy.wirefixup import fixup_wire_type

                        fixed = fixup_wire_type(decoded)
                        # The wire format has no line/column; decoded types
                        # default to -1. Preserve the input type's location so
                        # derived contexts report errors at the call site.
                        if fixed is not None and isinstance(fixed, ProperType):
                            fixed.line = t.line
                            fixed.column = t.column
                            if isinstance(fixed, CallableType):
                                fixed.fallback.line = fixed.line
                        # Clear the process-global primitive decode
                        # singletons after a read (see expand_type).
                        from mypy.types import instance_cache

                        instance_cache.int_type = None
                        instance_cache.str_type = None
                        instance_cache.bool_type = None
                        instance_cache.object_type = None
                        instance_cache.function_type = None
                        if fixed is not None:
                            from mypy.wirefixup import canonicalize_fresh_vars

                            # Wire round-trip loses fresh meta-var identity;
                            # re-unify occurrences before returning.
                            fixed = canonicalize_fresh_vars(fixed)
                            # Re-stamp wire-dropped definitions (None =
                            # unpairable, defer to the Python path below).
                            fixed = _resync_definitions(t, fixed)
                            if fixed is not None:
                                return cast(T, fixed)
            except (NotImplementedError, AssertionError):
                pass
        result = t.accept(FreshenCallableVisitor())
        assert isinstance(result, type(t))
        return result


class FreshenCallableVisitor(mypy.type_visitor.TypeTranslator):
    def visit_callable_type(self, t: CallableType) -> Type:
        result = super().visit_callable_type(t)
        assert isinstance(result, ProperType) and isinstance(result, CallableType)
        return freshen_function_type_vars(result)

    def visit_type_alias_type(self, t: TypeAliasType) -> Type:
        # Same as for ExpandTypeVisitor
        return t.copy_modified(args=[arg.accept(self) for arg in t.args])


class ExpandTypeVisitor(TrivialSyntheticTypeTranslator):
    """Visitor that substitutes type variables with values."""

    variables: Mapping[TypeVarId, Type]  # TypeVar id -> TypeVar value

    def __init__(self, variables: Mapping[TypeVarId, Type]) -> None:
        super().__init__()
        self.variables = variables

    def visit_unbound_type(self, t: UnboundType) -> Type:
        return t

    def visit_any(self, t: AnyType) -> Type:
        return t

    def visit_none_type(self, t: NoneType) -> Type:
        return t

    def visit_uninhabited_type(self, t: UninhabitedType) -> Type:
        return t

    def visit_deleted_type(self, t: DeletedType) -> Type:
        return t

    def visit_erased_type(self, t: ErasedType) -> Type:
        # This may happen during type inference if some function argument
        # type is a generic callable, and its erased form will appear in inferred
        # constraints, then solver may check subtyping between them, which will trigger

        # unify_generic_callables(), this is why we can get here. Another example is
        # when inferring type of lambda in generic context, the lambda body contains
        # a generic method in generic class.
        return t

    def visit_instance(self, t: Instance) -> Type:
        if len(t.args) == 0:
            return t

        args = self.expand_type_tuple_with_unpack(t.args)

        if isinstance(t.type, FakeInfo):
            # The type checker expands function definitions and bodies
            # if they depend on constrained type variables but the body
            # might contain a tuple type comment (e.g., # type: (int, float)),

            # in which case 't.type' is not yet available.

            # See: https://github.com/python/mypy/issues/16649
            return t.copy_modified(args=args)

        if t.type.fullname == "builtins.tuple":
            # Normalize Tuple[*Tuple[X, ...], ...] -> Tuple[X, ...]
            arg = args[0]
            if isinstance(arg, UnpackType) and not (
                isinstance(arg.type, TypeAliasType) and arg.type.is_recursive
            ):
                unpacked = get_proper_type(arg.type)
                if isinstance(unpacked, Instance):
                    assert unpacked.type.fullname == "builtins.tuple"
                    args = list(unpacked.args)
        return t.copy_modified(args=args)

    def visit_type_var(self, t: TypeVarType) -> Type:
        # Normally upper bounds can't contain other type variables, the only exception is
        # special type variable Self`0 <: C[T, S], where C is the class where Self is used.
        if t.id.is_self():
            t = t.copy_modified(upper_bound=t.upper_bound.accept(self))
        repl = self.variables.get(t.id, t)
        if isinstance(repl, ProperType) and isinstance(repl, Instance):
            # TODO: do we really need to do this?
            # If I try to remove this special-casing ~40 tests fail on reveal_type().
            return repl.copy_modified(last_known_value=None)
        return repl

    def visit_param_spec(self, t: ParamSpecType) -> Type:
        # Set prefix to something empty, so we don't duplicate it below.
        repl = self.variables.get(t.id, t.copy_modified(prefix=Parameters([], [], [])))
        if isinstance(repl, ParamSpecType):
            return repl.copy_modified(
                flavor=t.flavor,
                prefix=t.prefix.copy_modified(
                    arg_types=self.expand_types(t.prefix.arg_types) + repl.prefix.arg_types,
                    arg_kinds=t.prefix.arg_kinds + repl.prefix.arg_kinds,
                    arg_names=t.prefix.arg_names + repl.prefix.arg_names,
                ),
            )
        elif isinstance(repl, Parameters):
            assert isinstance(t.upper_bound, ProperType) and isinstance(t.upper_bound, Instance)
            if t.flavor == ParamSpecFlavor.BARE:
                return Parameters(
                    self.expand_types(t.prefix.arg_types) + repl.arg_types,
                    t.prefix.arg_kinds + repl.arg_kinds,
                    t.prefix.arg_names + repl.arg_names,
                    variables=[*t.prefix.variables, *repl.variables],
                    imprecise_arg_kinds=repl.imprecise_arg_kinds,
                )
            elif t.flavor == ParamSpecFlavor.ARGS:
                assert all(k.is_positional() for k in t.prefix.arg_kinds)
                return self._possible_callable_varargs(
                    repl, list(t.prefix.arg_types), t.upper_bound
                )
            else:
                assert t.flavor == ParamSpecFlavor.KWARGS
                return self._possible_callable_kwargs(repl, t.upper_bound)
        else:
            # We could encode Any as trivial parameters etc., but it would be too verbose.
            # TODO: assert this is a trivial type, like Any, Never, or object.
            return repl

    @classmethod
    def _possible_callable_varargs(
        cls, repl: Parameters, required_prefix: list[Type], tuple_type: Instance
    ) -> ProperType:
        """Given a callable, extract all parameters that can be passed as `*args`.

        This builds a union of all (possibly variadic) tuples representing all possible
        argument sequences that can be passed positionally. Each such tuple starts with
        all required (pos-only without a default) arguments, followed by some prefix
        of other arguments that can be passed positionally.
        """
        required_posargs = required_prefix
        if repl.variables:
            # We will tear the callable apart, do not leak type variables
            return tuple_type
        optional_posargs: list[Type] = []
        for kind, name, type in zip(repl.arg_kinds, repl.arg_names, repl.arg_types):
            if kind == ArgKind.ARG_POS and name is None:
                if optional_posargs:
                    # May happen following Unpack expansion without kinds correction
                    required_posargs += optional_posargs
                    optional_posargs = []
                required_posargs.append(type)
            elif kind.is_positional():
                optional_posargs.append(type)
            elif kind == ArgKind.ARG_STAR:
                if isinstance(type, UnpackType):
                    optional_posargs.append(type)
                else:
                    optional_posargs.append(UnpackType(Instance(tuple_type.type, [type])))
                break
        return UnionType.make_union(
            [
                TupleType(required_posargs + optional_posargs[:i], fallback=tuple_type)
                for i in range(len(optional_posargs) + 1)
            ]
        )

    @classmethod
    def _possible_callable_kwargs(cls, repl: Parameters, dict_type: Instance) -> ProperType:
        """Given a callable, extract all parameters that can be passed as `**kwargs`.

        If the function only accepts **kwargs, this will be a `dict[str, KwargsValueType]`.
        Otherwise, this will be a `TypedDict` containing all explicit args and ignoring
        `**kwargs` (until PEP 728 `extra_items` is supported). TypedDict entries will
        be required iff the corresponding argument is kw-only and has no default.
        """
        if repl.variables:
            # We will tear the callable apart, do not leak type variables
            return dict_type
        kwargs = {}
        required_names = set()
        extra_items: Type | None = None
        for kind, name, type in zip(repl.arg_kinds, repl.arg_names, repl.arg_types):
            if kind == ArgKind.ARG_NAMED and name is not None:
                kwargs[name] = type
                required_names.add(name)
            elif kind == ArgKind.ARG_STAR2:
                # Unpack[TypedDict] is normalized early, it isn't stored as Unpack
                extra_items = type
            elif not kind.is_star() and name is not None:
                kwargs[name] = type
        if not kwargs and extra_items is not None:
            return Instance(dict_type.type, [dict_type.args[0], extra_items])
        # TODO: when PEP 728 `extra_items` is implemented, pass extra_items below.
        is_closed = extra_items is None
        return TypedDictType(kwargs, required_names, set(), dict_type, is_closed=is_closed)

    def visit_type_var_tuple(self, t: TypeVarTupleType) -> Type:
        # Sometimes solver may need to expand a type variable with (a copy of) itself
        # (usually together with other TypeVars, but it is hard to filter out TypeVarTuples).
        repl = self.variables.get(t.id, t)
        if isinstance(repl, TypeVarTupleType):
            return repl
        elif isinstance(repl, ProperType) and isinstance(repl, (AnyType, UninhabitedType)):
            # Some failed inference scenarios will try to set all type variables to Never.
            # Instead of being picky and require all the callers to wrap them,
            # do this here instead.

            # Note: most cases when this happens are handled in expand unpack below, but
            # in rare cases (e.g. ParamSpec containing Unpack star args) it may be skipped.
            return t.tuple_fallback.copy_modified(args=[repl])
        raise NotImplementedError

    def visit_unpack_type(self, t: UnpackType) -> Type:
        # It is impossible to reasonably implement visit_unpack_type, because
        # unpacking inherently expands to something more like a list of types.

        # Relevant sections that can call unpack should call expand_unpack()
        # instead.
        # However, if the item is a variadic tuple, we can simply carry it over.

        # In particular, if we expand A[*tuple[T, ...]] with substitutions {T: str},
        # it is hard to assert this without getting proper type. Another important
        # example is non-normalized types when called from semanal.py.
        return UnpackType(t.type.accept(self))

    def expand_unpack(self, t: UnpackType) -> list[Type]:
        assert isinstance(t.type, TypeVarTupleType)
        repl = get_proper_type(self.variables.get(t.type.id, t.type))
        if isinstance(repl, UnpackType):
            repl = get_proper_type(repl.type)
        if isinstance(repl, TupleType):
            return repl.items
        elif (
            isinstance(repl, Instance)
            and repl.type.fullname == "builtins.tuple"
            or isinstance(repl, TypeVarTupleType)
        ):
            return [UnpackType(typ=repl)]
        elif isinstance(repl, (AnyType, UninhabitedType)):
            # Replace *Ts = Any with *Ts = *tuple[Any, ...] and same for Never.
            # These types may appear here as a result of user error or failed inference.
            return [UnpackType(t.type.tuple_fallback.copy_modified(args=[repl]))]
        else:
            raise RuntimeError(f"Invalid type replacement to expand: {repl}")

    def visit_parameters(self, t: Parameters) -> Type:
        return t.copy_modified(arg_types=self.expand_types(t.arg_types))

    def interpolate_args_for_unpack(self, t: CallableType, var_arg: UnpackType) -> list[Type]:
        star_index = t.arg_kinds.index(ARG_STAR)
        prefix = self.expand_types(t.arg_types[:star_index])
        suffix = self.expand_types(t.arg_types[star_index + 1 :])

        var_arg_type = get_proper_type(var_arg.type)
        new_unpack: Type
        if isinstance(var_arg_type, TupleType):
            # We have something like Unpack[Tuple[Unpack[Ts], X1, X2]]
            expanded_tuple = var_arg_type.accept(self)
            assert isinstance(expanded_tuple, ProperType) and isinstance(expanded_tuple, TupleType)
            expanded_items = expanded_tuple.items
            fallback = var_arg_type.partial_fallback
            new_unpack = UnpackType(TupleType(expanded_items, fallback))
        elif isinstance(var_arg_type, TypeVarTupleType):
            # We have plain Unpack[Ts]
            fallback = var_arg_type.tuple_fallback
            expanded_items = self.expand_unpack(var_arg)
            new_unpack = UnpackType(TupleType(expanded_items, fallback))
        # Since get_proper_type() may be called in semanal.py before callable
        # normalization happens, we need to also handle non-normal cases here.
        elif isinstance(var_arg_type, Instance):
            # we have something like Unpack[Tuple[Any, ...]]
            new_unpack = UnpackType(var_arg.type.accept(self))
        else:
            # We have invalid type in Unpack. This can happen when expanding aliases
            # to Callable[[*Invalid], Ret]
            new_unpack = AnyType(TypeOfAny.from_error, line=var_arg.line, column=var_arg.column)
        return prefix + [new_unpack] + suffix

    def visit_callable_type(self, t: CallableType) -> CallableType:
        param_spec = t.param_spec()
        if param_spec is not None:
            repl = self.variables.get(param_spec.id)
            # If a ParamSpec in a callable type is substituted with a
            # callable type, we can't use normal substitution logic,
            # since ParamSpec is actually split into two components

            # *P.args and **P.kwargs in the original type. Instead, we
            # must expand both of them with all the argument types,
            # kinds and names in the replacement. The return type in

            # the replacement is ignored.
            if isinstance(repl, Parameters):
                # We need to expand both the types in the prefix and the ParamSpec itself
                expanded = t.copy_modified(
                    arg_types=self.expand_types(t.arg_types[:-2]) + repl.arg_types,
                    arg_kinds=t.arg_kinds[:-2] + repl.arg_kinds,
                    arg_names=t.arg_names[:-2] + repl.arg_names,
                    ret_type=t.ret_type.accept(self),
                    type_guard=(t.type_guard.accept(self) if t.type_guard is not None else None),
                    type_is=(t.type_is.accept(self) if t.type_is is not None else None),
                    imprecise_arg_kinds=(t.imprecise_arg_kinds or repl.imprecise_arg_kinds),
                    variables=[*repl.variables, *t.variables],
                )
                var_arg = expanded.var_arg()
                if var_arg is not None and isinstance(var_arg.typ, UnpackType):
                    # Sometimes we get new unpacks after expanding ParamSpec.
                    expanded.normalize_trivial_unpack()
                return expanded
            elif isinstance(repl, ParamSpecType):
                # We're substituting one ParamSpec for another; this can mean that the prefix
                # changes, e.g. substitute Concatenate[int, P] in place of Q.
                prefix = repl.prefix
                clean_repl = repl.copy_modified(prefix=Parameters([], [], []))
                return t.copy_modified(
                    arg_types=self.expand_types(t.arg_types[:-2])
                    + prefix.arg_types
                    + [
                        clean_repl.with_flavor(ParamSpecFlavor.ARGS),
                        clean_repl.with_flavor(ParamSpecFlavor.KWARGS),
                    ],
                    arg_kinds=t.arg_kinds[:-2] + prefix.arg_kinds + t.arg_kinds[-2:],
                    arg_names=t.arg_names[:-2] + prefix.arg_names + t.arg_names[-2:],
                    ret_type=t.ret_type.accept(self),
                    from_concatenate=t.from_concatenate or bool(repl.prefix.arg_types),
                    imprecise_arg_kinds=(t.imprecise_arg_kinds or prefix.imprecise_arg_kinds),
                )

        var_arg = t.var_arg()
        needs_normalization = False
        if var_arg is not None and isinstance(var_arg.typ, UnpackType):
            needs_normalization = True
            arg_types = self.interpolate_args_for_unpack(t, var_arg.typ)
        else:
            arg_types = self.expand_types(t.arg_types)
        instance_type = None
        if t.instance_type is not None:
            instance_type = t.instance_type.accept(self)
            assert isinstance(instance_type, ProperType)
        expanded = t.copy_modified(
            arg_types=arg_types,
            ret_type=t.ret_type.accept(self),
            type_guard=t.type_guard.accept(self) if t.type_guard is not None else None,
            type_is=t.type_is.accept(self) if t.type_is is not None else None,
            instance_type=instance_type,
        )
        if needs_normalization:
            return expanded.with_normalized_var_args()
        return expanded

    def visit_overloaded(self, t: Overloaded) -> Type:
        items: list[CallableType] = []
        for item in t.items:
            new_item = item.accept(self)
            assert isinstance(new_item, ProperType)
            assert isinstance(new_item, CallableType)
            items.append(new_item)
        return Overloaded(items)

    def expand_type_list_with_unpack(self, typs: list[Type]) -> list[Type]:
        """Expands a list of types that has an unpack."""
        items: list[Type] = []
        for item in typs:
            if isinstance(item, UnpackType) and isinstance(item.type, TypeVarTupleType):
                items.extend(self.expand_unpack(item))
            else:
                items.append(item.accept(self))
        return items

    def expand_type_tuple_with_unpack(self, typs: tuple[Type, ...]) -> list[Type]:
        """Expands a tuple of types that has an unpack."""
        # Micro-optimization: Specialized variant of expand_type_list_with_unpack
        items: list[Type] = []
        for item in typs:
            if isinstance(item, UnpackType) and isinstance(item.type, TypeVarTupleType):
                items.extend(self.expand_unpack(item))
            else:
                items.append(item.accept(self))
        return items

    def visit_tuple_type(self, t: TupleType) -> Type:
        items = self.expand_type_list_with_unpack(t.items)
        if len(items) == 1:
            # Normalize Tuple[*Tuple[X, ...]] -> Tuple[X, ...]
            item = items[0]
            if isinstance(item, UnpackType) and not (
                isinstance(item.type, TypeAliasType) and item.type.is_recursive
            ):
                unpacked = get_proper_type(item.type)
                if isinstance(unpacked, Instance):
                    # expand_type() may be called during semantic analysis, before
                    # invalid unpacks are fixed.
                    if unpacked.type.fullname != "builtins.tuple":
                        return t.partial_fallback.accept(self)
                    if t.partial_fallback.type.fullname != "builtins.tuple":
                        # If it is a subtype (like named tuple) we need to preserve it,
                        # this essentially mimics the logic in tuple_fallback().
                        return t.partial_fallback.accept(self)
                    return unpacked
        fallback = t.partial_fallback.accept(self)
        assert isinstance(fallback, ProperType) and isinstance(fallback, Instance)
        return t.copy_modified(items=items, fallback=fallback)

    def visit_typeddict_type(self, t: TypedDictType) -> Type:
        if cached := self.get_cached(t):
            return cached
        fallback = t.fallback.accept(self)
        assert isinstance(fallback, ProperType) and isinstance(fallback, Instance)
        result = t.copy_modified(item_types=self.expand_types(t.items.values()), fallback=fallback)
        self.set_cached(t, result)
        return result

    def visit_literal_type(self, t: LiteralType) -> Type:
        # TODO: Verify this implementation is correct
        return t

    def visit_union_type(self, t: UnionType) -> Type:
        # Use cache to avoid O(n**2) or worse expansion of types during translation
        # (only for large unions, since caching adds overhead)
        use_cache = len(t.items) > 3
        if use_cache and (cached := self.get_cached(t)):
            return cached

        expanded = self.expand_types(t.items)
        # After substituting for type variables in t.items, some resulting types
        # might be subtypes of others, however calling  make_simplified_union()
        # can cause recursion, so we just remove strict duplicates.
        simplified = UnionType.make_union(
            remove_trivial(flatten_nested_unions(expanded)), t.line, t.column
        )
        # This call to get_proper_type() is unfortunate but is required to preserve
        # the invariant that ProperType will stay ProperType after applying expand_type(),
        # otherwise a single item union of a type alias will break it. Note this should not

        # cause infinite recursion since pathological aliases like A = Union[A, B] are
        # banned at the semantic analysis level.
        result = get_proper_type(simplified)

        if use_cache:
            self.set_cached(t, result)
        return result

    def visit_partial_type(self, t: PartialType) -> Type:
        return t

    def visit_type_type(self, t: TypeType) -> Type:
        # TODO: Verify that the new item type is valid (instance or
        # union of instances or Any).  Sadly we can't report errors
        # here yet.
        item = t.item.accept(self)
        return TypeType.make_normalized(item, is_type_form=t.is_type_form)

    def visit_type_alias_type(self, t: TypeAliasType) -> Type:
        # Target of the type alias cannot contain type variables (not bound by the type
        # alias itself), so we just expand the arguments.
        if len(t.args) == 0:
            return t
        args = self.expand_type_list_with_unpack(t.args)
        # TODO: normalize if target is Tuple, and args are [*tuple[X, ...]]?
        return t.copy_modified(args=args)

    def expand_types(self, types: Iterable[Type]) -> list[Type]:
        a: list[Type] = []
        for t in types:
            a.append(t.accept(self))
        return a


@overload
def expand_self_type(var: Var, typ: ProperType, replacement: ProperType) -> ProperType: ...


@overload
def expand_self_type(var: Var, typ: Type, replacement: Type) -> Type: ...


def expand_self_type(var: Var, typ: Type, replacement: Type) -> Type:
    """Expand appearances of Self type in a variable type."""
    if var.info.self_type is not None and not var.is_property:
        return expand_type(typ, {var.info.self_type.id: replacement})
    return typ


def remove_trivial(types: Iterable[Type]) -> list[Type]:
    """Make trivial simplifications on a list of types without calling is_subtype().

    This makes following simplifications:
        * Remove bottom types (taking into account strict optional setting)
        * Remove everything else if there is an `object`
        * Remove strict duplicate types
    """
    # Stage 3c type-kernel seam: try the Rust remove_trivial path. Rust
    # returns None for read/write failures, then we fall through to the
    # pure-Python loop (the strangler-fig per-call contract).
    types_list = list(types)
    if (
        _HAS_TYPE_KERNEL
        and _native_expand_type_active
        and not any(_needs_python(t, meta_gate=True) for t in types_list)
    ):
        try:
            from mypy.types import read_type_list, write_type_list

            buf = _WriteBuffer()
            write_type_list(buf, types_list)
            result = _type_kernel.rust_remove_trivial(buf.getvalue(), state.strict_optional)
            if result is not None:
                raw = bytes(result)
                cached = _expand_remove_trivial_cache.get(raw)
                if cached is not None:
                    return cached
                from mypy.wirefixup import fixup_wire_type

                decoded = read_type_list(_ReadBuffer(raw))
                # Clear the process-global primitive decode singletons
                # after a read so NOT_READY Instances cannot leak into
                # later builds (see expand_type).
                from mypy.types import instance_cache

                instance_cache.int_type = None
                instance_cache.str_type = None
                instance_cache.bool_type = None
                instance_cache.object_type = None
                instance_cache.function_type = None
                fixed_types: list[Type] = []
                for item in decoded:
                    fixed = fixup_wire_type(item)
                    if fixed is None:
                        break
                    fixed_types.append(fixed)
                else:
                    # Same identity repair + cache policy as expand_type:
                    # fresh-var trees stay out of the shared cache.
                    fixed_types, has_fresh = _canonicalize_fresh_type_list(fixed_types)
                    if not has_fresh:
                        _expand_remove_trivial_cache[raw] = fixed_types
                    return fixed_types
        except (AssertionError, NotImplementedError, ValueError, AttributeError):
            # Defer to Python: semanal TypeInfo-not-fixed asserts,
            # unserializable variants, failed wire reads, FakeInfo
            # attribute access.
            pass
    removed_none = False
    new_types = []
    all_types = set()
    for t in types_list:
        p_t = get_proper_type(t)
        if isinstance(p_t, UninhabitedType):
            continue
        if isinstance(p_t, NoneType) and not state.strict_optional:
            removed_none = True
            continue
        if isinstance(p_t, Instance) and p_t.type.fullname == "builtins.object":
            return [p_t]
        if p_t not in all_types:
            new_types.append(t)
            all_types.add(p_t)
    if new_types:
        return new_types
    if removed_none:
        return [NoneType()]
    return [UninhabitedType()]
