from __future__ import annotations

from collections.abc import Callable, Iterable, Sequence
from typing import Any, cast

import mypy.subtypes
from mypy.erasetype import erase_typevars
from mypy.expandtype import expand_type
from mypy.nodes import Context, TypeInfo
from mypy.state import state
from mypy.type_visitor import TypeTranslator
from mypy.typeops import get_all_type_vars
from mypy.types import (
    AnyType,
    CallableType,
    Instance,
    Parameters,
    ParamSpecFlavor,
    ParamSpecType,
    PartialType,
    ProperType,
    TupleType,
    Type,
    TypeAliasType,
    TypeVarId,
    TypeVarLikeType,
    TypeVarTupleType,
    TypeVarType,
    UninhabitedType,
    UnionType,
    UnpackType,
    get_proper_type,
    read_type,
    remove_dups,
)

# type_visitor needs to be imported after types
import mypy.type_visitor  # ruff: isort: skip

# Stage 6c type-kernel seam: apply_generic_arguments routes through Rust.
# Rust returns None for unhandled cases, falling back to pure Python.
# This is the strangler-fig per-call gate.
try:
    import type_kernel as _type_kernel
    from librt.internal import (
        ReadBuffer as _ReadBuffer,
        WriteBuffer as _WriteBuffer,
        write_int as _write_int_bare,
        write_tag as _write_tag,
    )

    _HAS_TYPE_KERNEL = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _ReadBuffer = None  # type: ignore[assignment,misc]
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _write_int_bare = None  # type: ignore[assignment]
    _write_tag = None  # type: ignore[assignment]
    _HAS_TYPE_KERNEL = False

_native_applytype_active: bool = False
_native_applytype_resolver: Any = None
_native_applytype_typeinfo_map: dict[str, Any] | None = None


def _set_native_applytype_active(active: bool) -> None:
    global _native_applytype_active
    _native_applytype_active = active


def _contains_named_callable(types: Sequence[Type | None]) -> bool:
    """True if any type nests a node a kernel round-trip cannot carry.

    The wire format cannot carry FuncDef/Decorator nodes, so a kernel
    round-trip would strip `definition` from a substituted callable arg,
    breaking error formatting that names the function; recursive
    TypeAliasType would loop while decoding. Both defer to Python.
    """
    stack: list[Type] = [t for t in types if t is not None]
    visited: set[int] = set()
    while stack:
        t = get_proper_type(stack.pop())
        if id(t) in visited:
            continue
        visited.add(id(t))
        if isinstance(t, CallableType):
            if t.definition is not None:
                return True
            stack.append(t.ret_type)
            stack.extend(t.arg_types)
            stack.append(t.fallback)
        elif isinstance(t, TypeAliasType):
            return True
        elif isinstance(t, Instance):
            stack.extend(t.args)
        elif isinstance(t, UnionType):
            stack.extend(t.items)
        elif isinstance(t, TupleType):
            stack.extend(t.items)
    return False


def _set_native_applytype_resolver(resolver: Any) -> None:
    global _native_applytype_resolver
    _native_applytype_resolver = resolver


def _set_native_applytype_typeinfo_map(typeinfo_map: dict[str, Any] | None) -> None:
    global _native_applytype_typeinfo_map
    _native_applytype_typeinfo_map = typeinfo_map
    from mypy.wirefixup import set_wire_typeinfo_map

    set_wire_typeinfo_map(typeinfo_map)


def _serialize_type(t: Type) -> bytes:
    buf = _WriteBuffer()
    t.write(buf)
    return buf.getvalue()


def _serialize_optional_type_list(types: Sequence[Type | None]) -> bytes:
    buf = _WriteBuffer()
    _write_int_bare(buf, len(types))
    for t in types:
        if t is None:
            _write_tag(buf, 0)
        else:
            _write_tag(buf, 1)
            t.write(buf)
    return buf.getvalue()


# Wire codec drops TypeVarId.meta_level. Collector snapshots meta_level
# from live inputs and fixer patches it back on decoded output.
# On conflicts we defer to Python.


class _TypeVarMetaCollector(mypy.type_visitor.TypeQuery[dict[tuple[int, str], int]]):
    """Walk a live type tree, mapping (raw_id, namespace) -> meta_level."""

    def strategy(self, items: list[dict[tuple[int, str], int]]) -> dict[tuple[int, str], int]:
        out: dict[tuple[int, str], int] = {}
        for d in items:
            _merge_meta_map(out, d)
        return out

    def _record(self, t: TypeVarLikeType) -> None:
        key = (t.id.raw_id, t.id.namespace)
        self.meta[key] = t.id.meta_level

    def __init__(self) -> None:
        super().__init__()
        self.meta: dict[tuple[int, str], int] = {}

    def visit_type_var(self, t: TypeVarType, /) -> dict[tuple[int, str], int]:
        self._record(t)
        return super().visit_type_var(t)

    def visit_param_spec(self, t: ParamSpecType, /) -> dict[tuple[int, str], int]:
        self._record(t)
        return super().visit_param_spec(t)

    def visit_type_var_tuple(self, t: TypeVarTupleType, /) -> dict[tuple[int, str], int]:
        self._record(t)
        return super().visit_type_var_tuple(t)

    def visit_callable_type(self, t: CallableType, /) -> dict[tuple[int, str], int]:
        # Base TypeQuery skips variables, type_guard and type_is.
        result = super().visit_callable_type(t)
        for v in t.variables:
            _merge_meta_map(self.meta, v.accept(self))
        for extra in (t.type_guard, t.type_is, t.instance_type):
            if extra is not None:
                _merge_meta_map(self.meta, extra.accept(self))
        return result


def _merge_meta_map(dst: dict[tuple[int, str], int], src: dict[tuple[int, str], int]) -> None:
    for k, v in src.items():
        if k in dst and dst[k] != v:
            # Same (raw_id, namespace) with different meta levels:
            # ambiguous, mark conflict so the fixer defers.
            dst[k] = _META_CONFLICT
        else:
            dst[k] = v


_META_CONFLICT = -1


class _TypeVarMetaFixer(mypy.type_visitor.TypeQuery[list[Any]]):
    """Restore meta_level onto a freshly decoded type tree (in place)."""

    def __init__(self, meta: dict[tuple[int, str], int]) -> None:
        super().__init__()
        self.meta = meta
        self.missing = False

    def strategy(self, items: list[list[Any]]) -> list[Any]:
        return []

    def _fix(self, t: TypeVarLikeType) -> None:
        meta = self.meta.get((t.id.raw_id, t.id.namespace))
        if meta is None:
            # Decoded ids default to 0; nothing to restore.
            return
        if meta == _META_CONFLICT:
            self.missing = True
            return
        t.id.meta_level = meta

    def visit_type_var(self, t: TypeVarType, /) -> list[Any]:
        self._fix(t)
        if self.missing:
            return []
        return super().visit_type_var(t)

    def visit_param_spec(self, t: ParamSpecType, /) -> list[Any]:
        self._fix(t)
        if self.missing:
            return []
        return super().visit_param_spec(t)

    def visit_type_var_tuple(self, t: TypeVarTupleType, /) -> list[Any]:
        self._fix(t)
        if self.missing:
            return []
        return super().visit_type_var_tuple(t)

    def visit_callable_type(self, t: CallableType, /) -> list[Any]:
        # Base TypeQuery skips variables, type_guard and type_is.
        result = super().visit_callable_type(t)
        if self.missing:
            return []
        for v in t.variables:
            v.accept(self)
        for extra in (t.type_guard, t.type_is, t.instance_type):
            if extra is not None:
                extra.accept(self)
        return result


def _native_get_target_type(
    tvar: TypeVarLikeType, type: Type, p_type: ProperType, skip_unsatisfied: bool
) -> tuple[int, int] | None:
    """Rust decision head for get_target_type (applytype.py:244-296).

    Returns (tag, match_index) when Rust decided the branch, or None to
    fall back to the pure-Python body. The resolver-backed subtype and
    same-type booleans are computed here (already native) and passed in;
    Rust never re-derives them. The report callback, expand_type, and
    erase_typevars stay Python-side, and the result Type never crosses
    the seam.
    """
    from mypy.subtypes import is_same_type, is_subtype

    same_type_ok: bool | None = None
    bound_ok: bool | None = None
    value_subtypes: list[bool] | None = None
    narrow_matrix: list[bool] | None = None
    if isinstance(tvar, TypeVarType) and tvar.values:
        values = tvar.values
        if isinstance(p_type, AnyType):
            pass
        elif isinstance(p_type, TypeVarType) and p_type.values:
            same_type_ok = all(any(is_same_type(v, v1) for v in values) for v1 in p_type.values)
            if not same_type_ok:
                value_subtypes, narrow_matrix = _target_type_subtype_facts(type, values)
        else:
            value_subtypes, narrow_matrix = _target_type_subtype_facts(type, values)
    elif isinstance(tvar, TypeVarType):
        upper_bound = tvar.upper_bound
        if tvar.name == "Self":
            # Internally constructed Self-types contain class type variables
            # in upper bound, so we need to erase them to avoid false
            # positives. Mirrors the pure-Python body's erase_typevars call.
            upper_bound = erase_typevars(upper_bound)
        bound_ok = is_subtype(type, upper_bound)
    try:
        return _type_kernel.rust_get_target_type(
            _serialize_type(tvar),
            _serialize_type(type),
            skip_unsatisfied,
            same_type_ok,
            bound_ok,
            value_subtypes,
            narrow_matrix,
        )
    except (NotImplementedError, AssertionError, ValueError):
        return None


def _target_type_subtype_facts(
    type: Type, values: list[Type]
) -> tuple[list[bool], list[bool] | None]:
    """Subtype facts for the value-matching fold: per-value plus matrix.

    The narrow matrix (value_i <: value_j, flattened row-major) is only
    computed when more than one value matches, mirroring the lazy
    narrowest-match loop of the pure-Python body.
    """
    from mypy.subtypes import is_subtype

    subs = [is_subtype(type, value) for value in values]
    narrow: list[bool] | None = None
    if sum(subs) > 1:
        n = len(values)
        narrow = [is_subtype(values[i], values[j]) for i in range(n) for j in range(n)]
    return subs, narrow


def get_target_type(
    tvar: TypeVarLikeType,
    type: Type,
    callable: CallableType,
    report_incompatible_typevar_value: Callable[[CallableType, Type, str, Context], None],
    context: Context,
    skip_unsatisfied: bool,
    id_to_type: dict[TypeVarId, Type],
) -> Type | None:
    p_type = get_proper_type(type)
    if _HAS_TYPE_KERNEL and _native_applytype_active:
        rust = _native_get_target_type(tvar, type, p_type, skip_unsatisfied)
        if rust is not None:
            tag, idx = rust
            if tag == 0:  # TAG_EXPAND_DEFAULT
                # Gradually expand defaults, as they may depend on previous type variables.
                return expand_type(tvar.default, id_to_type)
            if tag == 1:  # TAG_PASSTHROUGH
                return type
            if tag == 2:  # TAG_MATCH
                return cast(TypeVarType, tvar).values[idx]
            if tag == 3:  # TAG_SKIP
                return None
            # TAG_REPORT
            report_incompatible_typevar_value(callable, type, tvar.name, context)
            return type
    if isinstance(p_type, UninhabitedType) and p_type.ambiguous and tvar.has_default():
        # Gradually expand defaults, as they may depend on previous type variables.
        return expand_type(tvar.default, id_to_type)
    if isinstance(tvar, ParamSpecType):
        return type
    if isinstance(tvar, TypeVarTupleType):
        return type
    assert isinstance(tvar, TypeVarType)
    values = tvar.values
    if values:
        if isinstance(p_type, AnyType):
            return type
        if isinstance(p_type, TypeVarType) and p_type.values:
            # Allow substituting T1 for T if every allowed value of T1
            # is also a legal value of T.
            if all(any(mypy.subtypes.is_same_type(v, v1) for v in values) for v1 in p_type.values):
                return type
        matching = []
        for value in values:
            if mypy.subtypes.is_subtype(type, value):
                matching.append(value)
        if matching:
            best = matching[0]
            # If there are more than one matching value, we select the narrowest
            for match in matching[1:]:
                if mypy.subtypes.is_subtype(match, best):
                    best = match
            return best
        if skip_unsatisfied:
            return None
        report_incompatible_typevar_value(callable, type, tvar.name, context)
    else:
        upper_bound = tvar.upper_bound
        if tvar.name == "Self":
            # Internally constructed Self-types contain class type variables in upper bound,
            # so we need to erase them to avoid false positives. This is safe because we do
            # not support type variables in upper bounds of user defined types.
            upper_bound = erase_typevars(upper_bound)
        if not mypy.subtypes.is_subtype(type, upper_bound):
            if skip_unsatisfied:
                return None
            report_incompatible_typevar_value(callable, type, tvar.name, context)
    return type


def apply_generic_arguments(
    callable: CallableType,
    orig_types: Sequence[Type | None],
    report_incompatible_typevar_value: Callable[[CallableType, Type, str, Context], None],
    context: Context,
    skip_unsatisfied: bool = False,
) -> CallableType:
    """Apply generic type arguments to a callable type.

    For example, applying [int] to 'def [T] (T) -> T' results in
    'def (int) -> int'.

    Note that each type can be None; in this case, it will not be applied.

    If `skip_unsatisfied` is True, then just skip the types that don't satisfy type variable
    bound or constraints, instead of giving an error.
    """
    # Stage 6c type-kernel seam: route apply_generic_arguments to Rust.
    # Returns None for unsupported cases, falling back to Python.
    # Limited to skip_unsatisfied=True calls (due to TypeVar round-trip).
    if (
        _HAS_TYPE_KERNEL
        and _native_applytype_active
        and _native_applytype_resolver is not None
        and skip_unsatisfied
        and not _contains_named_callable(orig_types)
    ):
        try:
            # Wire codec drops TypeVarId.meta_level. Returning a callable
            # with meta_level=0 for metavariables corrupts later inference.
            # Defer when any input contains metavariables.
            meta: dict[tuple[int, str], int] = {}
            collector = _TypeVarMetaCollector()
            callable.accept(collector)
            meta = collector.meta
            for ot in orig_types:
                if ot is not None:
                    collector = _TypeVarMetaCollector()
                    ot.accept(collector)
                    for k, v in collector.meta.items():
                        if k in meta and meta[k] != v:
                            meta[k] = v  # conflict: anything nonzero defers
                        else:
                            meta[k] = v
            has_meta = any(v != 0 for v in meta.values())
            # Defer generic tuple-subclass constructors:
            # fallback mapping in later arg checks is sensitive to
            # decode differences (check-typevar-tuple regressions).
            ret = get_proper_type(callable.ret_type)
            tuple_subclass_ctor = (
                callable.is_generic()
                and isinstance(ret, TupleType)
                and ret.partial_fallback.type.fullname != "builtins.tuple"
            )
            if not has_meta and not tuple_subclass_ctor:
                result = _type_kernel.rust_apply_generic_arguments(
                    _native_applytype_resolver,
                    _serialize_type(callable),
                    _serialize_optional_type_list(orig_types),
                    skip_unsatisfied,
                    state.strict_optional,
                )
                if result is not None:
                    decoded = read_type(_ReadBuffer(bytes(result)))
                    # Clear instance_cache primitives after read_type so
                    # NOT_READY singletons cannot leak into later builds.
                    from mypy.types import instance_cache

                    instance_cache.int_type = None
                    instance_cache.str_type = None
                    instance_cache.bool_type = None
                    instance_cache.object_type = None
                    instance_cache.function_type = None
                    # Wire round-trip loses line/column/definition.
                    # Restore them from the original callable so error
                    # locations and special-signature dispatch agree.
                    if isinstance(get_proper_type(decoded), CallableType):
                        decoded = cast(CallableType, decoded).copy_modified(
                            line=callable.line,
                            column=callable.column,
                            definition=callable.definition,
                        )
                    if _native_applytype_typeinfo_map is not None:
                        from mypy.wirefixup import check_no_fake_info, fixup_wire_type

                        fixed = fixup_wire_type(decoded)
                        if fixed is not None:
                            assert isinstance(get_proper_type(fixed), CallableType)
                            # Any residual fake TypeInfo crashes later
                            # serialization, so defer instead.
                            if check_no_fake_info(fixed):
                                return cast(CallableType, fixed)
                    else:
                        assert isinstance(get_proper_type(decoded), CallableType)
                        return cast(CallableType, decoded)
        except (NotImplementedError, AssertionError):
            # AssertionError: TypeInfo not yet fixed during semanal.
            # NotImplementedError: unserializable variant.
            # Both defer to Python.
            pass
    tvars = callable.variables
    assert len(orig_types) <= len(tvars)
    # Check that inferred type variable values are compatible with allowed
    # values and bounds.  Also, promote subtype values to allowed values.
    # Create a map from type variable id to target type.
    id_to_type: dict[TypeVarId, Type] = {}

    for tvar, type in zip(tvars, orig_types):
        assert not isinstance(type, PartialType), "Internal error: must never apply partial type"
        if type is None:
            continue

        target_type = get_target_type(
            tvar,
            type,
            callable,
            report_incompatible_typevar_value,
            context,
            skip_unsatisfied,
            id_to_type,
        )
        if target_type is not None:
            id_to_type[tvar.id] = target_type

    # TODO: validate arg_kinds/arg_names for ParamSpec and TypeVarTuple replacements,
    # not just type variable bounds above.
    param_spec = callable.param_spec()
    if param_spec is not None:
        nt = id_to_type.get(param_spec.id)
        if nt is not None:
            # ParamSpec expansion is special-cased, so we need to always expand callable
            # as a whole, not expanding arguments individually.
            callable = expand_type(callable, id_to_type)
            assert isinstance(callable, CallableType)
            return callable.copy_modified(
                variables=[tv for tv in tvars if tv.id not in id_to_type]
            )

    # Apply arguments to argument types.
    var_arg = callable.var_arg()
    if var_arg is not None and isinstance(var_arg.typ, UnpackType):
        # Same as for ParamSpec, callable with variadic types needs to be expanded as a whole.
        callable = expand_type(callable, id_to_type)
        assert isinstance(callable, CallableType)
        return callable.copy_modified(variables=[tv for tv in tvars if tv.id not in id_to_type])
    else:
        callable = callable.copy_modified(
            arg_types=[expand_type(at, id_to_type) for at in callable.arg_types]
        )

    # Apply arguments to TypeGuard and TypeIs if any.
    if callable.type_guard is not None:
        type_guard = expand_type(callable.type_guard, id_to_type)
    else:
        type_guard = None
    if callable.type_is is not None:
        type_is = expand_type(callable.type_is, id_to_type)
    else:
        type_is = None

    # Callable may retain type vars if only some were applied.
    # TODO: move apply_poly() logic here when new inference is universal.
    # With this logic we can add new free variables.
    remaining_tvars: list[TypeVarLikeType] = []
    for tv in tvars:
        if tv.id in id_to_type:
            continue
        if not tv.has_default():
            remaining_tvars.append(tv)
            continue
        # TypeVarLike isn't in id_to_type mapping.
        # Only expand the TypeVar default here.
        typ = expand_type(tv, id_to_type)
        assert isinstance(typ, TypeVarLikeType)
        remaining_tvars.append(typ)

    instance_type = None
    if callable.instance_type is not None:
        instance_type = expand_type(callable.instance_type, id_to_type)
        assert isinstance(instance_type, ProperType)

    return callable.copy_modified(
        ret_type=expand_type(callable.ret_type, id_to_type),
        variables=remaining_tvars,
        type_guard=type_guard,
        type_is=type_is,
        instance_type=instance_type,
    )


def apply_poly(tp: CallableType, poly_tvars: Sequence[TypeVarLikeType]) -> CallableType | None:
    """Make free type variables generic in the type if possible.

    This will translate the type `tp` while trying to create valid bindings for
    type variables `poly_tvars` while traversing the type. This follows the same rules
    as we do during semantic analysis phase, examples:
      * Callable[Callable[[T], T], T] -> def [T] (def (T) -> T) -> T
      * Callable[[], Callable[[T], T]] -> def () -> def [T] (T -> T)
      * List[T] -> None (not possible)
    """
    try:
        return tp.copy_modified(
            arg_types=[t.accept(PolyTranslator(poly_tvars)) for t in tp.arg_types],
            ret_type=tp.ret_type.accept(PolyTranslator(poly_tvars)),
            variables=[],
        )
    except PolyTranslationError:
        return None


class PolyTranslationError(Exception):
    pass


class PolyTranslator(TypeTranslator):
    """Make free type variables generic in the type if possible.

    See docstring for apply_poly() for details.
    """

    def __init__(
        self,
        poly_tvars: Iterable[TypeVarLikeType],
        bound_tvars: frozenset[TypeVarLikeType] = frozenset(),
        seen_aliases: frozenset[TypeInfo] = frozenset(),
    ) -> None:
        super().__init__()
        self.poly_tvars = set(poly_tvars)
        # This is a simplified version of TypeVarScope used during semantic analysis.
        self.bound_tvars = bound_tvars
        self.seen_aliases = seen_aliases

    def collect_vars(self, t: CallableType | Parameters) -> list[TypeVarLikeType]:
        found_vars = []
        for arg in t.arg_types:
            for tv in get_all_type_vars(arg):
                if isinstance(tv, ParamSpecType):
                    normalized: TypeVarLikeType = tv.copy_modified(
                        flavor=ParamSpecFlavor.BARE, prefix=Parameters([], [], [])
                    )
                else:
                    normalized = tv
                if normalized in self.poly_tvars and normalized not in self.bound_tvars:
                    found_vars.append(normalized)
        return remove_dups(found_vars)

    def visit_callable_type(self, t: CallableType) -> Type:
        found_vars = self.collect_vars(t)
        self.bound_tvars |= set(found_vars)
        result = super().visit_callable_type(t)
        self.bound_tvars -= set(found_vars)

        assert isinstance(result, ProperType) and isinstance(result, CallableType)
        result.variables = result.variables + tuple(found_vars)
        return result

    def visit_type_var(self, t: TypeVarType) -> Type:
        if t in self.poly_tvars and t not in self.bound_tvars:
            raise PolyTranslationError()
        return super().visit_type_var(t)

    def visit_param_spec(self, t: ParamSpecType) -> Type:
        if t in self.poly_tvars and t not in self.bound_tvars:
            raise PolyTranslationError()
        return super().visit_param_spec(t)

    def visit_type_var_tuple(self, t: TypeVarTupleType) -> Type:
        if t in self.poly_tvars and t not in self.bound_tvars:
            raise PolyTranslationError()
        return super().visit_type_var_tuple(t)

    def visit_type_alias_type(self, t: TypeAliasType) -> Type:
        if not t.args:
            return t.copy_modified()
        if not t.is_recursive:
            return get_proper_type(t).accept(self)
        # We can't handle polymorphic application for recursive generic aliases
        # without risking an infinite recursion, just give up for now.
        raise PolyTranslationError()

    def visit_instance(self, t: Instance) -> Type:
        if t.type.has_param_spec_type:
            # We need this special-casing to preserve the possibility to store a
            # generic function in an instance type. Things like
            #     forall T . Foo[[x: T], T]

            # are not really expressible in current type system, but this looks like
            # a useful feature, so let's keep it.
            param_spec_index = next(
                i for (i, tv) in enumerate(t.type.defn.type_vars) if isinstance(tv, ParamSpecType)
            )
            p = get_proper_type(t.args[param_spec_index])
            if isinstance(p, Parameters):
                found_vars = self.collect_vars(p)
                self.bound_tvars |= set(found_vars)
                new_args = [a.accept(self) for a in t.args]
                self.bound_tvars -= set(found_vars)

                repl = new_args[param_spec_index]
                assert isinstance(repl, ProperType) and isinstance(repl, Parameters)
                repl.variables = list(repl.variables) + list(found_vars)
                return t.copy_modified(args=new_args)
        # There is the same problem with callback protocols as with aliases
        # (callback protocols are essentially more flexible aliases to callables).
        if t.args and t.type.is_protocol and t.type.protocol_members == ["__call__"]:
            if t.type in self.seen_aliases:
                raise PolyTranslationError()
            call = mypy.subtypes.find_member("__call__", t, t, is_operator=True)
            assert call is not None
            return call.accept(
                PolyTranslator(self.poly_tvars, self.bound_tvars, self.seen_aliases | {t.type})
            )
        return super().visit_instance(t)
