"""Shared helpers for wire-round-trip kernel paths.

The native type kernel serializes Type objects to a wire format, processes
them in Rust, and deserializes the result via `read_type`. Deserialization
produces Instances whose `.type` is `NOT_READY` (a FakeInfo) and whose
`type_ref` holds the fullname string. These must be resolved to live
TypeInfo objects before the result can re-enter the type graph, or
FakeInfo.__getattribute__ raises AssertionError downstream.

This module provides the shared `_TypeRefFixer` (a TypeTranslator that
resolves type_ref strings to live TypeInfo/TypeAlias) and `fixup_wire_type`
(the convenience entry point). The fixer defers to Python (returns None)
when any type_ref is absent from the fullname -> TypeInfo / -> TypeAlias
maps, so callers can fall back gracefully.

The fixer mutates Instances in place. This is intentional: wire compact
tags (INSTANCE_STR etc.) return shared cached Instance objects, so
mutating the cache entry resolves it for all future callers. Without
in-place mutation, a later `read_type` returning the same cached
Instance would get a FakeInfo, leaking into the type graph.
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, TypeVar, cast

from librt.internal import ReadBuffer

from mypy.nodes import FakeInfo
from mypy.types import (
    AnyType,
    CallableType,
    Instance,
    LiteralType,
    Overloaded,
    Parameters,
    ParamSpecType,
    TupleType,
    Type,
    TypeAliasType,
    TypedDictType,
    TypeType,
    TypeVarLikeType,
    TypeVarTupleType,
    TypeVarType,
    UnionType,
    UnpackType,
    get_proper_type,
)

# type_visitor needs to be imported after types
import mypy.type_visitor  # ruff: isort: skip
from mypy.type_visitor import TypeTranslator

_wire_typeinfo_map: dict[str, Any] | None = None
_last_real_map: dict[str, Any] | None = None
_wire_alias_map: dict[str, Any] | None = None
_last_alias_map: dict[str, Any] | None = None


def clear_wire_decode_caches() -> None:
    """Invalidate every Python-side wire-decode cache.

    Called by `set_wire_typeinfo_map` when a brand-new map identity
    replaces the previous one, by `BuildManager._clear_native_resolvers`
    on the per-build reset, and by `BuildManager._refresh_native_wirefixup_maps`
    when merge_asts re-homes map entries in place (same dict identity, so
    the identity check there would not clear anything).
    """
    from mypy.checker import _clear_checker_deser_cache
    from mypy.checkexpr import _clear_argtypes_plan_cache
    from mypy.checkmember import _clear_deser_cache
    from mypy.erasetype import _clear_erase_decode_cache
    from mypy.expandtype import _clear_expand_decode_cache
    from mypy.maptype import _clear_map_supertype_decode_cache
    from mypy.meet import _clear_narrow_decode_cache
    from mypy.subtypes import _clear_subtype_batch, _clear_subtype_decode_cache
    from mypy.typeops import _clear_typeops_decode_cache
    from mypy.typevars import _clear_typevars_decode_cache

    _clear_deser_cache()
    _clear_checker_deser_cache()
    _clear_erase_decode_cache()
    _clear_argtypes_plan_cache()
    _clear_typeops_decode_cache()
    _clear_typevars_decode_cache()
    _clear_map_supertype_decode_cache()
    _clear_subtype_decode_cache()
    # Serialized subtype answer pairs are keyed on the previous map's
    # TypeInfo graph; a per-build reset (or a merge_asts re-home) must
    # drop them alongside the decode caches.
    _clear_subtype_batch()
    _clear_narrow_decode_cache()
    _clear_expand_decode_cache()


def set_wire_typeinfo_map(typeinfo_map: dict[str, Any] | None) -> None:
    """Install the fullname -> TypeInfo map shared by all wire-round-trip paths.

    Invalidate the checkmember deserialize cache only when a real map
    replaces a *different* real map: a brand-new TypeInfo map can resolve
    the same wire bytes to different live TypeInfo objects (parity tests
    build fresh TypeInfos per case).  ``None`` resets (saved in
    ``_last_real_map`` so they never erase the accumulated map identity)
    are only semanal-side gates clearing the resolver snapshot; the
    accumulated map itself is stable and growing (never re-created), so
    cached resolutions stay valid across SCCs and clear on the SCC cycle
    would destroy the hit rate.  The daemon path re-creates the map;
    ``BuildManager._clear_native_resolvers`` calls ``_clear_deser_cache``
    explicitly for that.
    """
    global _wire_typeinfo_map, _last_real_map
    if typeinfo_map is None:
        _wire_typeinfo_map = None
        return
    if _last_real_map is not None and typeinfo_map is not _last_real_map:
        # A brand-new map identity (parity tests per case, daemon
        # rechecks): cached decodes from the previous map must not
        # survive, they would resolve into stale TypeInfo objects.
        clear_wire_decode_caches()
    _last_real_map = typeinfo_map
    _wire_typeinfo_map = typeinfo_map


def set_wire_alias_map(alias_map: dict[str, Any] | None) -> None:
    """Install the fullname -> TypeAlias map for resolved decoded aliases.

    Mirrors `set_wire_typeinfo_map`: the map lives for the lifetime of the
    resolver it was derived from (per-build in the daemon, per-case in the
    parity suites). A brand-new map identity clears the Python-side decode
    caches: cached decodes may carry aliases re-linked from the previous
    map, which would point at stale TypeAlias nodes. An explicit ``None``
    (build teardown / suite teardown) also clears.
    """
    global _wire_alias_map, _last_alias_map
    if _last_alias_map is not None and alias_map is not _last_alias_map:
        clear_wire_decode_caches()
    _last_alias_map = alias_map
    _wire_alias_map = alias_map


def fixup_wire_type(typ: Type, *, resolve_aliases: bool = False) -> Type | None:
    """Resolve type_ref strings in a wire-decoded Type to live objects.

    Returns None if the typeinfo map is unset or any Instance's type_ref
    is absent, so the caller can defer to Python. Decoded TypeAliasTypes
    resolve through the alias map (see ``set_wire_alias_map``) only when
    ``resolve_aliases`` is set: the alias-scoped consumers (the typeanal
    passthrough seam) want the decoded alias re-linked to its live
    TypeAlias node, while every other seam must keep its previous
    defer-on-alias behavior (a decoded alias whose args carry live
    typevars must not be frozen into the graph by, e.g., checkmember's
    self-arg path).
    """
    if _wire_typeinfo_map is None:
        # No typeinfo map (erasetype/typeops seam): cannot resolve
        # NOT_READY singletons, so evict them; a poisoned cache Instance
        # must never leak into the type graph via a later read_type.
        fixup_instance_cache()
        return None
    alias_map = _wire_alias_map if resolve_aliases else None
    fixer = _TypeRefFixer(_wire_typeinfo_map, alias_map)
    result = typ.accept(fixer)
    # Also fixup shared instance_cache singletons that read_type may
    # have populated with NOT_READY instances: a failed fixup (returning
    # None) must not leave NOT_READY singletons leaking into later calls.
    fixup_instance_cache()
    return None if fixer.missing else result


def decode_source_any(data: ReadBuffer) -> AnyType | None:
    """Read a wire AnyType blob for `solve_one` kind=3 (Any-absorption).

    Structurally mirrors `AnyType.read` (types.py:1463-1473) up to and
    including `END_TAG`, returning just the `source_any` AnyType (the Rust
    seam emits `AnyType(from_another_any, source_any=...)`, whose own
    `type_of_any`/`missing_import_name` are canonical). Returns None for
    `LITERAL_NONE` so the Python shim skips the `from_another_any`
    construction and falls through to the pure-Python body instead.
    """
    from mypy.cache import LITERAL_NONE, read_tag
    from mypy.types import ANY_TYPE

    tag = read_tag(data)
    if tag == LITERAL_NONE:
        return None
    assert tag == ANY_TYPE
    # Delegate to `AnyType.read`, which consumes the remaining
    # `source_any`/`type_of_any`/`missing_import_name`/`END_TAG` fields
    # without a leading class tag.
    source_any_type = AnyType.read(data)
    return source_any_type.source_any


def fixup_instance_cache() -> None:
    """Fix up shared instance_cache singletons if they carry type_refs.

    read_type populates instance_cache with NOT_READY Instances when it
    encounters INSTANCE_OBJECT/INSTANCE_STR/etc. tags. These must be
    resolved to live TypeInfo before they leak into the type graph.

    Unresolvable singletons are evicted (not left in place): the cached
    Instance may be wired into a type (e.g. copy_type's TypeShallowCopier
    reuses the shared object), and its type_ref is never cleared in the
    Python fallback. Keeping it would poison the shared cache for the
    next read_type and the fixture cache with a FakeInfo object.

    With no typeinfo map installed, every NOT_READY singleton is
    unresolvable, so all are evicted.
    """
    from mypy.types import instance_cache

    for attr in ("str_type", "function_type", "int_type", "bool_type", "object_type"):
        inst = getattr(instance_cache, attr)
        if inst is None:
            continue
        if inst.type_ref is None:
            # Already a live TypeInfo (or never wire-decoded): keep.
            continue
        if _wire_typeinfo_map is None:
            # Nothing to resolve against: evict so the NOT_READY object
            # cannot leak into the graph via a later read_type.
            setattr(instance_cache, attr, None)
            continue
        info = _wire_typeinfo_map.get(inst.type_ref)
        if info is not None and not isinstance(info, FakeInfo):
            inst.type = info
            inst.type_ref = None
        else:
            # Unresolvable (absent or FakeInfo placeholder): evict so a
            # NOT_READY singleton wired to FakeInfo cannot survive. A
            # later read_type recreates it and this pass fixes it up.
            setattr(instance_cache, attr, None)


class _FreshVarCanonicalizer(TypeTranslator):
    """Re-unify fresh (meta-level > 0) type variable occurrences by id.

    Python's in-memory fresh paths (e.g. ``expand_type`` with a tvmap built
    by ``freshen_function_type_vars``) return the *same* fresh TypeVar
    object for every occurrence of a given id, so downstream inference that
    compares metavariables by object identity sees them as one variable.
    A wire round-trip loses that: each occurrence materializes a distinct
    object carrying the same id, splitting metavariables and breaking
    inference. This pass makes all occurrences of a given fresh id share a
    single object, restoring Python-observable identity.

    Variables in ``seed`` pre-register their identity: occurrences in the
    tree unify onto those exact objects, keeping callers' expectations
    that the variables slot and occurrences share objects (e.g. freeze).
    """

    def __init__(self, seed: Sequence[TypeVarLikeType] | None = None) -> None:
        super().__init__()
        self._var_by_id: dict[tuple[int, int, str], TypeVarLikeType] = {}
        if seed is not None:
            for tv in seed:
                self._var_by_id.setdefault(self._key(tv), tv)

    def _canonical(self, t: TypeVarLikeType, key: tuple[int, int, str]) -> TypeVarLikeType:
        existing = self._var_by_id.get(key)
        if existing is None:
            self._var_by_id[key] = t
            return t
        if existing == t:
            return existing
        # Same id but different content (env-expanded occurrence, e.g. a
        # ParamSpec whose prefix grew via Concatenate): an expansion result,
        # not a wire-split copy. Leave it alone for downstream inference.
        return t

    @staticmethod
    def _key(t: TypeVarLikeType) -> tuple[int, int, str]:
        return (t.id.raw_id, t.id.meta_level, t.id.namespace)

    def visit_type_var(self, t: TypeVarType, /) -> Type:
        if not t.id.is_meta_var():
            return t
        return self._canonical(t, self._key(t))

    def visit_type_alias_type(self, t: TypeAliasType, /) -> Type:
        # Fresh vars also occur as alias arguments (e.g. Pairs[Self] in a
        # method signature); re-unify them with the seed/tree occurrences
        # so freeze-in-place and id-keyed substitution see one object.

        # Never descends into ``t.alias.target`` (recursive-alias safe),
        # matching the alias handling of the identity canonicalizer below.
        if not t.args:
            return t
        args = [arg.accept(self) for arg in t.args]
        return t.copy_modified(args=args)

    def visit_param_spec(self, t: ParamSpecType, /) -> Type:
        if not t.id.is_meta_var():
            return t
        return self._canonical(t, self._key(t))

    def visit_type_var_tuple(self, t: TypeVarTupleType, /) -> Type:
        if not t.id.is_meta_var():
            return t
        return self._canonical(t, self._key(t))

    def visit_callable_type(self, t: CallableType, /) -> Type:
        # Base translator leaves `variables`, `type_guard`, `type_is`
        # untranslated; traverse them so fresh vars inside are unified.
        result = get_proper_type(super().visit_callable_type(t))
        if not isinstance(result, CallableType):
            return result
        variables = [v.accept(self) for v in result.variables]
        type_guard = t.type_guard.accept(self) if t.type_guard is not None else None
        type_is = t.type_is.accept(self) if t.type_is is not None else None
        return result.copy_modified(
            variables=variables, type_guard=type_guard, type_is=type_is  # type: ignore[arg-type]
        )


def canonicalize_fresh_vars_reported(
    typ: Type, seed: Sequence[TypeVarLikeType] | None = None
) -> tuple[Type, bool]:
    """Canonicalize and report whether any fresh meta-var was unified.

    Variables in ``seed`` pre-register their identity: occurrences in the
    tree unify onto those exact objects, keeping the variables slot and
    occurrences shared for callers that mutate vars in place (freeze).
    Callers that cache decoded trees must not store fresh-var-bearing
    results: the repair shares one object per id, so a cached tree would
    let one caller's in-place freeze leak into later callers of the
    identical blob.
    """
    canonicalizer = _FreshVarCanonicalizer(seed)
    result = typ.accept(canonicalizer)
    return result, bool(canonicalizer._var_by_id)


def canonicalize_fresh_vars_reported_list(types: list[Type]) -> tuple[list[Type], bool]:
    """Canonicalize a decoded list with one shared unifier, per identity.

    A single canonicalizer instance unifies fresh ids across the items
    (the same fresh var can appear in several items), and reports whether
    any fresh var was seen: such trees must stay out of shared caches.
    """
    canonicalizer = _FreshVarCanonicalizer()
    result: list[Type] = []
    has_fresh = False
    for t in types:
        result.append(t.accept(canonicalizer))
        has_fresh = has_fresh or bool(canonicalizer._var_by_id)
    return result, has_fresh


def canonicalize_fresh_vars(typ: Type, seed: Sequence[TypeVarLikeType] | None = None) -> Type:
    """Re-unify fresh meta-var occurrences by id (wire-path identity repair)."""
    result, _ = canonicalize_fresh_vars_reported(typ, seed)
    return result


def _collect_typevar_likes(t: Type, seed: dict[tuple[int, int, str], TypeVarLikeType]) -> None:
    """Collect every TypeVar-like node in the raw tree of `t` into `seed`.

    Raw-tree walk (no get_proper_type): the seed must hold the exact
    objects Python's ExpandTypeVisitor would return by identity, i.e.
    vars occurring as-is plus vars nested inside upper bounds, defaults,
    value lists, prefixes and callable variables slots. Non-recursive
    alias targets are walked with a cycle guard; a match exposes the
    alias target's vars under the alias's own tree.
    """
    alias_seen: set[int] = set()
    stack: list[Type] = [t]
    while stack:
        cur = stack.pop()
        if isinstance(cur, TypeVarLikeType):
            seed.setdefault((cur.id.raw_id, cur.id.meta_level, cur.id.namespace), cur)
            stack.append(cur.upper_bound)
            stack.append(cur.default)
            if isinstance(cur, TypeVarType):
                stack.extend(cur.values)
            elif isinstance(cur, ParamSpecType):
                stack.append(cur.prefix)
            elif isinstance(cur, TypeVarTupleType):
                stack.append(cur.tuple_fallback)
        elif isinstance(cur, CallableType):  # type: ignore[misc]
            stack.extend(cur.arg_types)
            stack.append(cur.ret_type)
            stack.append(cur.fallback)
            stack.extend(cur.variables)
            if cur.instance_type is not None:
                stack.append(cur.instance_type)
        elif isinstance(cur, Parameters):
            stack.extend(cur.arg_types)
            stack.extend(cur.variables)
        elif isinstance(cur, Overloaded):  # type: ignore[misc]
            stack.extend(cur.items)
        elif isinstance(cur, Instance):  # type: ignore[misc]
            stack.extend(cur.args)
        elif isinstance(cur, UnionType):  # type: ignore[misc]
            stack.extend(cur.items)
        elif isinstance(cur, TupleType):  # type: ignore[misc]
            stack.extend(cur.items)
            stack.append(cur.partial_fallback)
        elif isinstance(cur, TypedDictType):  # type: ignore[misc]
            stack.extend(cur.items.values())
            stack.append(cur.fallback)
        elif isinstance(cur, TypeType):  # type: ignore[misc]
            stack.append(cur.item)
        elif isinstance(cur, UnpackType):
            stack.append(cur.type)
        elif isinstance(cur, TypeAliasType):
            stack.extend(cur.args)
            if cur.alias is not None and id(cur.alias) not in alias_seen:
                alias_seen.add(id(cur.alias))
                stack.append(cur.alias.target)


class _VarIdentityCanonicalizer(TypeTranslator):
    """Re-link wire-decoded TypeVar-like nodes to their live originals.

    Python's ExpandTypeVisitor preserves TypeVar identity: an unbound var
    returns the original occurrence (expandtype.py visit_type_var) and a
    bound var returns the original env-value object. The wire round-trip
    hands back fresh copies, so downstream consumers that compare vars by
    identity or mutate them in place would observe a split.

    This pass seeds a canonical map from the ORIGINAL tree plus the env
    values (all vars reachable, including bounds/defaults), then replaces
    each decoded occurrence whose (raw_id, meta_level, namespace) matches
    AND is structurally equal to a seeded original with that original.
    Structurally unequal nodes are genuine expansion results and keep the
    decoded copy; matches are deterministic per key, independent of the
    call, so callers may cache the pre-repair decoded shape.

    Meta vars without a seed entry behave like `_FreshVarCanonicalizer`
    (share the first occurrence). A non-meta var absent from the seed is
    a collection gap: `missing_seed` is set and the caller defers.
    """

    def __init__(self, typ: Type, env_values: Sequence[Type]) -> None:
        super().__init__()
        self._typ = typ
        self._env_values = env_values
        self._seed: dict[tuple[int, int, str], TypeVarLikeType] | None = None
        self.missing_seed = False

    def _canon(self, t: TypeVarLikeType) -> Type:
        if self._seed is None:
            self._seed = {}
            _collect_typevar_likes(self._typ, self._seed)
            for v in self._env_values:
                _collect_typevar_likes(v, self._seed)
        existing = self._seed.get((t.id.raw_id, t.id.meta_level, t.id.namespace))
        if existing is None:
            if not t.id.is_meta_var():
                self.missing_seed = True
            self._seed[(t.id.raw_id, t.id.meta_level, t.id.namespace)] = t
            return t
        if existing == t:
            return existing
        return t

    def visit_type_var(self, t: TypeVarType, /) -> Type:
        return self._canon(t)

    def visit_param_spec(self, t: ParamSpecType, /) -> Type:
        return self._canon(t)

    def visit_type_var_tuple(self, t: TypeVarTupleType, /) -> Type:
        return self._canon(t)

    def visit_type_alias_type(self, t: TypeAliasType, /) -> Type:
        return t

    def visit_callable_type(self, t: CallableType, /) -> Type:
        # Base translator leaves `variables`, `type_guard`, `type_is`
        # untranslated; traverse them so var occurrences inside are relinked.
        result = get_proper_type(super().visit_callable_type(t))
        if not isinstance(result, CallableType):
            return result
        variables = [v.accept(self) for v in result.variables]
        type_guard = t.type_guard.accept(self) if t.type_guard is not None else None
        type_is = t.type_is.accept(self) if t.type_is is not None else None
        return result.copy_modified(
            variables=variables, type_guard=type_guard, type_is=type_is  # type: ignore[arg-type]
        )


def contains_typevar_like(typ: Type) -> bool:
    """Does the decoded tree contain any TypeVar-like node?

    Mirrors the Rust `result_has_typevar` scan. Decoded var-bearing
    results only appear on the identity-repair path, so callers use this
    to cheaply skip the seed-walk repair for the common var-free case.
    """
    stack: list[Type] = [typ]
    while stack:
        cur = stack.pop()
        if isinstance(cur, TypeVarLikeType):
            return True
        if isinstance(cur, CallableType):  # type: ignore[misc]
            stack.extend(cur.arg_types)
            stack.append(cur.ret_type)
            stack.append(cur.fallback)
            stack.extend(cur.variables)
        elif isinstance(cur, Overloaded):  # type: ignore[misc]
            stack.extend(cur.items)
        elif isinstance(cur, Parameters):
            stack.extend(cur.arg_types)
            stack.extend(cur.variables)
        elif isinstance(cur, Instance):  # type: ignore[misc]
            stack.extend(cur.args)
        elif isinstance(cur, UnionType):  # type: ignore[misc]
            stack.extend(cur.items)
        elif isinstance(cur, TupleType):  # type: ignore[misc]
            stack.extend(cur.items)
            stack.append(cur.partial_fallback)
        elif isinstance(cur, TypedDictType):  # type: ignore[misc]
            stack.extend(cur.items.values())
            stack.append(cur.fallback)
        elif isinstance(cur, TypeType):  # type: ignore[misc]
            stack.append(cur.item)
        elif isinstance(cur, UnpackType):
            stack.append(cur.type)
        elif isinstance(cur, TypeAliasType):
            stack.extend(cur.args)
        elif isinstance(cur, LiteralType):  # type: ignore[misc]
            stack.append(cur.fallback)
    return False


def resync_var_identities(typ: Type, decoded: Type, env_values: Sequence[Type]) -> Type | None:
    """Re-link wire-decoded TypeVar-like occurrences to their live originals.

    Python's ExpandTypeVisitor preserves TypeVar identity; a wire
    round-trip does not. Seeds from the ORIGINAL tree plus the env
    values, replaces structurally-equal decoded occurrences with their
    originals, and returns None to defer when a non-meta var has no
    seeded original (a collection gap, e.g. an alias target the walk
    could not see). See `_VarIdentityCanonicalizer`.
    """
    if not contains_typevar_like(decoded):
        return decoded
    cand = _VarIdentityCanonicalizer(typ, env_values)
    result = decoded.accept(cand)
    if cand.missing_seed:
        return None
    return result


class _ReceiverTvarResyncer(TypeTranslator):
    """Re-link decoded receiver-arg tvar occurrences to the live objects.

    Python's pure IAMA tail substitutes receiver-argument tvars by
    returning the mapped receiver's variable objects themselves
    (ExpandTypeVisitor returns the env value), so the result shares
    object identity with the caller's receiver. The survivors-gate
    widening (wave32) rides fresh (meta_level > 0) receiver-arg vars
    through the wire; without a relink each decoded occurrence is a
    doppelganger, and downstream solve-freshening that fuses identities
    in one object collapses the inference target to `Never` (issue
    #1286 regression). This pass seeds from the mapped receiver's
    arguments — the same `recv` keys the Rust gate consults — and
    replaces key-matching occurrences with the live objects.

    Keys: full (raw_id, meta_level, namespace) for TypeVarType, and
    (raw_id, namespace) for ParamSpecType / TypeVarTupleType, whose wire
    form drops `id.meta_level` entirely (a decoded ParamSpec carries
    meta 0 regardless of the live var). This ignores meta for TVLs on
    purpose, mirroring the -1 sentinel in `collect_tvar_keys`.
    Variables-entry leftovers are deliberately NOT relinked (they are
    frozen method tvars whose decoded-copy status is the pre-widening
    status quo). Unmatched ids pass through: the Rust gate guarantees
    every decoded tvar survivor is a variables entry or a key-matched
    receiver-arg rider.
    """

    def __init__(self, receiver_args: Sequence[Type]) -> None:
        super().__init__()
        self._vars: dict[tuple[int, int, str], TypeVarType] = {}
        self._tvls: dict[tuple[int, str], TypeVarLikeType] = {}
        stack: list[Type] = list(receiver_args)
        while stack:
            cur = stack.pop()
            if isinstance(cur, TypeVarLikeType):
                if isinstance(cur, TypeVarType):
                    self._vars.setdefault(
                        (cur.id.raw_id, cur.id.meta_level, cur.id.namespace), cur
                    )
                else:
                    self._tvls.setdefault((cur.id.raw_id, cur.id.namespace), cur)
                stack.append(cur.upper_bound)
                stack.append(cur.default)
                if isinstance(cur, TypeVarType):
                    stack.extend(cur.values)
                elif isinstance(cur, ParamSpecType):
                    stack.append(cur.prefix)
                elif isinstance(cur, TypeVarTupleType):
                    stack.append(cur.tuple_fallback)
            elif isinstance(cur, CallableType):  # type: ignore[misc]
                stack.extend(cur.arg_types)
                stack.append(cur.ret_type)
                stack.append(cur.fallback)
                stack.extend(cur.variables)
                if cur.instance_type is not None:
                    stack.append(cur.instance_type)
            elif isinstance(cur, Parameters):
                stack.extend(cur.arg_types)
                stack.extend(cur.variables)
            elif isinstance(cur, Overloaded):  # type: ignore[misc]
                stack.extend(cur.items)
            elif isinstance(cur, Instance):  # type: ignore[misc]
                stack.extend(cur.args)
            elif isinstance(cur, UnionType):  # type: ignore[misc]
                stack.extend(cur.items)
            elif isinstance(cur, TupleType):  # type: ignore[misc]
                stack.extend(cur.items)
                stack.append(cur.partial_fallback)
            elif isinstance(cur, TypedDictType):  # type: ignore[misc]
                stack.extend(cur.items.values())
                stack.append(cur.fallback)
            elif isinstance(cur, TypeType):  # type: ignore[misc]
                stack.append(cur.item)
            elif isinstance(cur, UnpackType):
                stack.append(cur.type)
            elif isinstance(cur, TypeAliasType):
                stack.extend(cur.args)

    def _var(self, t: TypeVarLikeType) -> Type:
        if isinstance(t, TypeVarType):
            return self._vars.get((t.id.raw_id, t.id.meta_level, t.id.namespace), t)
        return self._tvls.get((t.id.raw_id, t.id.namespace), t)

    def visit_type_var(self, t: TypeVarType, /) -> Type:
        return self._var(t)

    def visit_param_spec(self, t: ParamSpecType, /) -> Type:
        return self._var(t)

    def visit_type_var_tuple(self, t: TypeVarTupleType, /) -> Type:
        return self._var(t)

    def visit_type_alias_type(self, t: TypeAliasType, /) -> Type:
        # Receiver-arg riders also occur as alias arguments (e.g.
        # Pairs[Self]-shaped signatures); descend into alias args but
        # never into t.alias.target (recursive-alias safe).
        if not t.args:
            return t
        return t.copy_modified(args=[arg.accept(self) for arg in t.args])

    def visit_callable_type(self, t: CallableType, /) -> Type:
        # Base translator leaves `variables`, `type_guard`, `type_is`
        # untranslated; traverse them so riders inside are relinked.
        result = get_proper_type(super().visit_callable_type(t))
        if not isinstance(result, CallableType):
            return result
        new_variables = [v.accept(self) for v in result.variables]
        type_guard = t.type_guard.accept(self) if t.type_guard is not None else None
        type_is = t.type_is.accept(self) if t.type_is is not None else None
        return result.copy_modified(
            variables=new_variables, type_guard=type_guard, type_is=type_is  # type: ignore[arg-type]
        )


_T = TypeVar("_T", bound="Type")


def resync_receiver_arg_tvars(decoded: _T, receiver_args: Sequence[Type]) -> _T:
    """Re-link decoded receiver-arg tvar occurrences to the live objects.

    Pairs with the wave32 IAMA survivors-gate widening: at every IAMA
    decode sink the Python shim seeds this pass with the mapped
    receiver's arguments so decoded riders regain Python's identity
    semantics. Cheap when the decoded tree carries no tvar at all.
    """
    if not contains_typevar_like(decoded):
        return decoded
    return cast("_T", decoded.accept(_ReceiverTvarResyncer(receiver_args)))


class _TypeRefFixer(TypeTranslator):
    """Resolve wire-decoded Instance.type_ref to live TypeInfo.

    Mutates Instances in place: wire compact tags (INSTANCE_STR etc.)
    return SHARED cached Instances, so in-place mutation resolves the
    cache entry for all future callers. Without it, a later read_type
    returning the same cached Instance would get a FakeInfo.
    Sets self.missing when a type_ref is absent from the map so the
    caller can defer to Python.
    """

    def __init__(
        self, typeinfo_map: dict[str, Any], alias_map: dict[str, Any] | None = None
    ) -> None:
        super().__init__()
        self.typeinfo_map = typeinfo_map
        self.alias_map = alias_map
        self.missing = False

    def visit_instance(self, t: Instance, /) -> Type:
        if t.type_ref is not None:
            info = self.typeinfo_map.get(t.type_ref)
            if info is None or isinstance(info, FakeInfo):
                # FakeInfo .type (NOT_READY placeholder, e.g. the
                # builtins.bool cache singleton): defer to Python,
                # never wire a fake TypeInfo into the graph.
                self.missing = True
                return t
            # Mutate in place: wire compact tags (INSTANCE_STR etc.)
            # return shared cached Instance objects. Mutating resolves
            # the cache entry so future read_type calls get live TypeInfo.
            t.type = info
            t.type_ref = None
        if self.missing:
            return t
        result = get_proper_type(super().visit_instance(t))
        if isinstance(result, Instance) and result.extra_attrs is not None:
            attrs = {k: v.accept(self) for k, v in result.extra_attrs.attrs.items()}
            if self.missing:
                return t  # type: ignore[unreachable]
            extra = result.extra_attrs.copy()
            extra.attrs = attrs
            result.extra_attrs = extra
        return result

    def visit_callable_type(self, t: CallableType, /) -> Type:
        if self.missing:
            return t
        fallback = t.fallback.accept(self)
        if self.missing:
            return t  # type: ignore[unreachable]
        result = get_proper_type(super().visit_callable_type(t))
        if not isinstance(result, CallableType):
            return result
        # translate_variables returns vars as-is (base translator
        # skips them). Visit each variable so upper_bound/values
        # get TypeInfo fixed up.
        variables = [v.accept(self) for v in result.variables]
        if self.missing:
            return t  # type: ignore[unreachable]
        # Base translator also skips type_guard/type_is.
        type_guard = None
        if t.type_guard is not None:
            tg = t.type_guard.accept(self)
            if self.missing:
                return t  # type: ignore[unreachable]
            type_guard = tg
        type_is = None
        if t.type_is is not None:
            ti = t.type_is.accept(self)
            if self.missing:
                return t  # type: ignore[unreachable]
            type_is = ti
        result = result.copy_modified(
            fallback=fallback,  # type: ignore[arg-type]
            variables=variables,  # type: ignore[arg-type]
            type_guard=type_guard,
            type_is=type_is,
        )
        # The wire format drops CallableType.definition (only used for
        # error messages); re-link it so messages render `f(self)` and
        # self-typevar solving sees the original argument. Match the

        # containing class's symbol table by name + arity, mirroring
        # fixup.py's FuncDef/OverloadedFuncDef linking.
        if result.definition is None and not self.missing:
            definition = self._match_definition(result)
            if definition is not None:
                result = result.copy_modified(definition=definition)
        return result

    def _match_definition(self, t: CallableType) -> Any:
        """Find the live FuncDef/OverloadedFuncDef item for a decoded callable.

        Uses the (already fixed-up) fallback TypeInfo's symbol table and
        matches on name + argument arity, mirroring how fixup.py links
        ``func.type.definition = func`` and ``typ.definition = item`` for
        overloads. Returns None when no match exists (e.g. builtins
        primitives) so definition stays None and callers fall back.
        """
        info = t.fallback.type
        if isinstance(info, FakeInfo) or not getattr(info, "names", None):
            return None
        from mypy.nodes import Decorator, FuncDef, OverloadedFuncDef
        from mypy.types import Overloaded

        if t.name is None:
            return None
        lookup_name = t.name.split(" of ")[0] if " of " in t.name else t.name
        node = info.names.get(lookup_name)
        if node is None:
            node = info.names.get(t.name)
        if node is None:
            return None
        sym = node.node
        if isinstance(sym, Decorator):
            # Decorator wraps a FuncDef; its .type is the decorated callable.
            ctyp = get_proper_type(sym.type) if sym.type else None
            if isinstance(ctyp, CallableType) and len(ctyp.arg_types) == len(t.arg_types):
                return sym.func
            return None
        if isinstance(sym, FuncDef):
            ctyp = sym.type
            if isinstance(ctyp, CallableType) and len(ctyp.arg_types) == len(t.arg_types):
                return sym
            return None
        if isinstance(sym, OverloadedFuncDef) and isinstance(sym.type, Overloaded):
            for item in sym.items:
                item_typ = getattr(item, "type", None)
                if isinstance(item, Decorator):
                    item_typ = item.var.type if item.var else item.func.type
                cp = get_proper_type(item_typ) if item_typ is not None else None
                if isinstance(cp, CallableType) and len(cp.arg_types) == len(t.arg_types):
                    return item
            return None
        return None

    def visit_type_type(self, t: TypeType, /) -> Type:
        if self.missing:
            return t
        return super().visit_type_type(t)

    def visit_type_var(self, t: TypeVarType, /) -> Type:
        if self.missing:
            return t
        upper_bound = t.upper_bound.accept(self)
        if self.missing:
            return t  # type: ignore[unreachable]
        values = [v.accept(self) for v in t.values]
        if self.missing:
            return t  # type: ignore[unreachable]
        default = t.default.accept(self)
        if self.missing:
            return t  # type: ignore[unreachable]
        return t.copy_modified(upper_bound=upper_bound, values=values, default=default)

    def visit_param_spec(self, t: ParamSpecType, /) -> Type:
        if self.missing:
            return t
        # Visit upper_bound first: it may be a shared instance_cache
        # singleton (INSTANCE_OBJECT compact tag) whose FakeInfo must be
        # resolved in place before check_no_fake_info scans the tree.
        _ = t.upper_bound.accept(self)
        if self.missing:
            return t  # type: ignore[unreachable]
        default = t.default.accept(self)
        if self.missing:
            return t  # type: ignore[unreachable]
        prefix = t.prefix.accept(self)
        if self.missing:
            return t  # type: ignore[unreachable]
        assert isinstance(prefix, Parameters)
        # copy_modified keeps upper_bound (it is determined by flavor).
        return t.copy_modified(default=default, prefix=prefix)

    def visit_type_var_tuple(self, t: TypeVarTupleType, /) -> Type:
        if self.missing:
            return t
        tuple_fallback = t.tuple_fallback.accept(self)
        if self.missing:
            return t  # type: ignore[unreachable]
        upper_bound = t.upper_bound.accept(self)
        if self.missing:
            return t  # type: ignore[unreachable]
        default = t.default.accept(self)
        if self.missing:
            return t  # type: ignore[unreachable]
        result = t.copy_modified(upper_bound=upper_bound, default=default)
        # copy_modified keeps tuple_fallback; swap it on the fresh copy.
        assert isinstance(result, TypeVarTupleType)
        result.tuple_fallback = tuple_fallback  # type: ignore[assignment]
        return result

    def visit_type_alias_type(self, t: TypeAliasType, /) -> Type:
        if self.missing:
            return t
        # Decoded aliases have alias=None and a type_ref string. Resolve
        # the ref to a live TypeAlias when a map is installed (mirrors
        # fixup.py); otherwise defer to Python (unfixed alias breaks str).
        if t.type_ref is not None:
            if self.alias_map is None:
                self.missing = True
                return t
            alias = self.alias_map.get(t.type_ref)
            if alias is None:
                self.missing = True
                return t
            t.alias = alias
            t.type_ref = None
        args = [a.accept(self) for a in t.args]
        if self.missing:
            return t  # type: ignore[unreachable]
        return t.copy_modified(args=args)


class _FakeInfoGuard(mypy.type_visitor.TypeQuery[bool]):
    """Detect residual fake TypeInfos (unfixed wire type_refs) in a tree."""

    def __init__(self) -> None:
        super().__init__()
        # Decoded aliases have no alias node to expand.
        self.skip_alias_target = True
        # Instance.__hash__ dereferences `.type`, which raises on fake
        # infos, so track visited instances by id() instead.
        self.seen_instance_ids: set[int] = set()

    def strategy(self, items: list[bool]) -> bool:
        return any(items)

    def visit_instance(self, t: Instance, /) -> bool:
        # `type()` bypasses FakeInfo.__getattribute__, which raises.
        if type(t.type) is FakeInfo:
            return True
        if id(t) in self.seen_instance_ids:
            return False
        self.seen_instance_ids.add(id(t))
        result = super().visit_instance(t)
        if result:
            return True
        if t.extra_attrs is not None:
            return any(v.accept(self) for v in t.extra_attrs.attrs.values())
        return False

    def visit_callable_type(self, t: CallableType, /) -> bool:
        # Base TypeQuery skips fallback, variables, type_guard, type_is.
        if super().visit_callable_type(t):
            return True
        if t.fallback.accept(self):
            return True
        if any(v.accept(self) for v in t.variables):
            return True
        return bool(
            (t.type_guard is not None and t.type_guard.accept(self))
            or (t.type_is is not None and t.type_is.accept(self))
        )

    def visit_type_var_tuple(self, t: TypeVarTupleType, /) -> bool:
        # Base TypeQuery skips tuple_fallback.
        if super().visit_type_var_tuple(t):
            return True
        return t.tuple_fallback.accept(self)


def check_no_fake_info(t: Type) -> bool:
    """Return True when the tree has no residual fake TypeInfos."""
    return not t.accept(_FakeInfoGuard())
