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

from typing import Any

from librt.internal import ReadBuffer

from mypy.nodes import FakeInfo
from mypy.types import (
    AnyType,
    CallableType,
    Instance,
    Parameters,
    ParamSpecType,
    Type,
    TypeAliasType,
    TypeType,
    TypeVarLikeType,
    TypeVarTupleType,
    TypeVarType,
    get_proper_type,
)

# type_visitor needs to be imported after types
import mypy.type_visitor  # ruff: isort: skip
from mypy.type_visitor import TypeTranslator

_wire_typeinfo_map: dict[str, Any] | None = None
_last_real_map: dict[str, Any] | None = None
_wire_alias_map: dict[str, Any] | None = None


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
    parity suites), so it is cleared only by an explicit ``None`` (build
    teardown / suite teardown), never by the per-SCC resolver resets.
    """
    global _wire_alias_map
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
    """

    def __init__(self) -> None:
        super().__init__()
        self._var_by_id: dict[tuple[int, int, str], TypeVarLikeType] = {}

    def _canonical(self, t: TypeVarLikeType, key: tuple[int, int, str]) -> TypeVarLikeType:
        existing = self._var_by_id.get(key)
        if existing is None:
            self._var_by_id[key] = t
            return t
        return existing

    @staticmethod
    def _key(t: TypeVarLikeType) -> tuple[int, int, str]:
        return (t.id.raw_id, t.id.meta_level, t.id.namespace)

    def visit_type_var(self, t: TypeVarType, /) -> Type:
        if not t.id.is_meta_var():
            return t
        return self._canonical(t, self._key(t))

    def visit_param_spec(self, t: ParamSpecType, /) -> Type:
        if not t.id.is_meta_var():
            return t
        return self._canonical(t, self._key(t))

    def visit_type_var_tuple(self, t: TypeVarTupleType, /) -> Type:
        if not t.id.is_meta_var():
            return t
        return self._canonical(t, self._key(t))

    def visit_type_alias_type(self, t: TypeAliasType, /) -> Type:
        return t

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


def canonicalize_fresh_vars(typ: Type) -> Type:
    """Re-unify fresh meta-var occurrences by id (wire-path identity repair)."""
    from mypy.types import instance_cache  # noqa: F401  (import side effect)

    result = typ.accept(_FreshVarCanonicalizer())
    return result


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
