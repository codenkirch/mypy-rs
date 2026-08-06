"""Shared helpers for wire-round-trip kernel paths.

The native type kernel serializes Type objects to a wire format, processes
them in Rust, and deserializes the result via `read_type`. Deserialization
produces Instances whose `.type` is `NOT_READY` (a FakeInfo) and whose
`type_ref` holds the fullname string. These must be resolved to live
TypeInfo objects before the result can re-enter the type graph, or
FakeInfo.__getattribute__ raises AssertionError downstream.

This module provides the shared `_TypeRefFixer` (a TypeTranslator that
resolves type_ref strings to live TypeInfo) and `fixup_wire_type` (the
convenience entry point). The fixer defers to Python (returns None) when
any type_ref is absent from the fullname -> TypeInfo map, so callers
can fall back gracefully.

The fixer mutates Instances in place. This is intentional: wire compact
tags (INSTANCE_STR etc.) return shared cached Instance objects, so
mutating the cache entry resolves it for all future callers. Without
in-place mutation, a later `read_type` returning the same cached
Instance would get a FakeInfo, leaking into the type graph.
"""

from __future__ import annotations

from typing import Any

from mypy.nodes import FakeInfo
from mypy.types import (
    CallableType,
    Instance,
    Parameters,
    ParamSpecType,
    Type,
    TypeAliasType,
    TypeType,
    TypeVarTupleType,
    TypeVarType,
)

# type_visitor needs to be imported after types
import mypy.type_visitor  # ruff: isort: skip
from mypy.type_visitor import TypeTranslator

_wire_typeinfo_map: dict[str, Any] | None = None
# fullname -> mypy.nodes.TypeAlias. Installed at runtime, same lifecycle
# as the typeinfo map; used to fix up recursive TypeAliasType nodes whose
# alias target carries unresolved type_refs.
_wire_alias_map: dict[str, Any] | None = None


def set_wire_typeinfo_map(typeinfo_map: dict[str, Any] | None) -> None:
    """Install the fullname -> TypeInfo map shared by all wire-round-trip paths."""
    global _wire_typeinfo_map
    _wire_typeinfo_map = typeinfo_map


def set_wire_alias_map(alias_map: dict[str, Any] | None) -> None:
    """Install the fullname -> TypeAlias map for recursive alias fixup."""
    global _wire_alias_map
    _wire_alias_map = alias_map


def fixup_wire_type(typ: Type) -> Type | None:
    """Resolve type_ref strings in a wire-decoded Type to live TypeInfo.

    Returns None if the typeinfo map is unset or any Instance's type_ref
    is absent, so the caller can defer to Python.
    """
    if _wire_typeinfo_map is None:
        return None
    fixer = _TypeRefFixer(_wire_typeinfo_map, _wire_alias_map)
    result = typ.accept(fixer)
    return None if fixer.missing else result


def fix_alias_recursive(fixer: TypeTranslator, t: TypeAliasType) -> Type:
    """Fix a wire-decoded TypeAliasType's alias node + recursive target.

    Used by the per-module `_TypeRefFixer` copies (join, expandtype,
    applytype) so recursive aliases resolve identically everywhere.
    `fixer` must expose `missing` and `typeinfo_map`; `t.type_ref` is
    resolved against `_wire_alias_map`.
    """
    if getattr(fixer, "missing", False):
        return t
    if t.alias is None and t.type_ref is not None and _wire_alias_map is not None:
        alias = _wire_alias_map.get(t.type_ref)
        if alias is None:
            fixer.missing = True
            return t
        t.alias = alias
    # Base visitor recursion covers args; the recursive target needs
    # explicit descent so its Instances' type_refs get fixed.
    if getattr(fixer, "_seen_aliases", None) is not None:
        seen: set[int] = getattr(fixer, "_seen_aliases")
        if id(t.alias) in seen:
            return t
    if t.alias is not None:
        target = t.alias.target.accept(fixer)
        if fixer.missing:
            return t
        if target is not t.alias.target:
            t.alias.target = target  # type: ignore[assignment]
    return t


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
        # Guard against infinite recursion on self-referential aliases:
        # Expr's target contains Expr, so target traversal re-enters
        # visit_type_alias_type on the same TypeAlias node.
        self._seen_aliases: set[int] = set()

    def visit_instance(self, t: Instance, /) -> Type:
        if t.type_ref is not None:
            info = self.typeinfo_map.get(t.type_ref)
            if info is None:
                self.missing = True
                return t
            # Mutate in place: wire compact tags (INSTANCE_STR etc.)
            # return shared cached Instance objects. Mutating resolves
            # the cache entry so future read_type calls get live TypeInfo.
            t.type = info
            t.type_ref = None
        if self.missing:
            return t
        result = super().visit_instance(t)
        if isinstance(result, Instance) and result.extra_attrs is not None:
            attrs = {k: v.accept(self) for k, v in result.extra_attrs.attrs.items()}
            if self.missing:
                return t
            extra = result.extra_attrs.copy()
            extra.attrs = attrs
            result.extra_attrs = extra
        return result

    def visit_callable_type(self, t: CallableType, /) -> Type:
        if self.missing:
            return t
        fallback = t.fallback.accept(self)
        if self.missing:
            return t
        result = super().visit_callable_type(t)
        if not isinstance(result, CallableType):
            return result
        # translate_variables returns vars as-is (base translator
        # skips them). Visit each variable so upper_bound/values
        # get TypeInfo fixed up.
        variables = [v.accept(self) for v in result.variables]
        if self.missing:
            return t
        # Base translator also skips type_guard/type_is.
        type_guard = None
        if t.type_guard is not None:
            tg = t.type_guard.accept(self)
            if self.missing:
                return t
            type_guard = tg
        type_is = None
        if t.type_is is not None:
            ti = t.type_is.accept(self)
            if self.missing:
                return t
            type_is = ti
        return result.copy_modified(
            fallback=fallback,
            variables=variables,
            type_guard=type_guard,
            type_is=type_is,
        )

    def visit_type_type(self, t: TypeType, /) -> Type:
        if self.missing:
            return t
        return super().visit_type_type(t)

    def visit_type_var(self, t: TypeVarType, /) -> Type:
        if self.missing:
            return t
        upper_bound = t.upper_bound.accept(self)
        if self.missing:
            return t
        values = [v.accept(self) for v in t.values]
        if self.missing:
            return t
        default = t.default.accept(self)
        if self.missing:
            return t
        return t.copy_modified(upper_bound=upper_bound, values=values, default=default)

    def visit_param_spec(self, t: ParamSpecType, /) -> Type:
        if self.missing:
            return t
        default = t.default.accept(self)
        if self.missing:
            return t
        prefix = t.prefix.accept(self)
        if self.missing:
            return t
        assert isinstance(prefix, Parameters)
        # copy_modified keeps upper_bound (it is determined by flavor).
        return t.copy_modified(default=default, prefix=prefix)

    def visit_type_var_tuple(self, t: TypeVarTupleType, /) -> Type:
        if self.missing:
            return t
        tuple_fallback = t.tuple_fallback.accept(self)
        if self.missing:
            return t
        upper_bound = t.upper_bound.accept(self)
        if self.missing:
            return t
        default = t.default.accept(self)
        if self.missing:
            return t
        result = t.copy_modified(upper_bound=upper_bound, default=default)
        # copy_modified keeps tuple_fallback; swap it on the fresh copy.
        assert isinstance(result, TypeVarTupleType)
        result.tuple_fallback = tuple_fallback
        return result

    def visit_type_alias_type(self, t: TypeAliasType, /) -> Type:
        if self.missing:
            return t
        args = [a.accept(self) for a in t.args]
        if self.missing:
            return t
        seen = self._seen_aliases
        if t.alias is not None and id(t.alias) in seen:
            return t
        result = fix_alias_recursive(self, t)
        if self.missing:
            return t
        if t.alias is not None:
            seen.add(id(t.alias))
        return result


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
