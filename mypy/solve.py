"""Type inference constraint solving"""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable, Sequence
from typing import Any, TypeAlias as _TypeAlias

from mypy.constraints import SUBTYPE_OF, SUPERTYPE_OF, Constraint, infer_constraints, neg_op
from mypy.expandtype import expand_type
from mypy.graph_utils import prepare_sccs, strongly_connected_components, topsort
from mypy.join import join_type_list
from mypy.meet import meet_type_list, meet_types
from mypy.state import state
from mypy.subtypes import is_subtype
from mypy.typeops import get_all_type_vars
from mypy.types import (
    AnyType,
    CallableType,
    DeletedType,
    ErasedType,
    Instance,
    NoneType,
    Overloaded,
    Parameters,
    ParamSpecType,
    ProperType,
    TupleType,
    Type,
    TypeAliasType,
    TypedDictType,
    TypeGuardedType,
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
    get_proper_type,
    has_recursive_types,
    read_type,
)
from mypy.typestate import type_state

# Stage 4b type-kernel seam: when type_kernel is importable and the
# solve gate is on, single-variable constraint solving routes through
# Rust, deferring to Python on join/meet/is_subtype/union gaps.
try:
    import type_kernel as _type_kernel
    from librt.internal import ReadBuffer as _ReadBuffer, WriteBuffer as _WriteBuffer

    _HAS_TYPE_KERNEL = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _ReadBuffer = None  # type: ignore[assignment,misc]
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _HAS_TYPE_KERNEL = False

# Module-level flag, set by the build manager from
# `Options.native_type_kernel` at the start of each build. When active
# but no Rust result, the shim falls through to Python.
_native_solve_active: bool = False


def _set_native_solve_active(active: bool) -> None:
    """Called by the build manager to enable/disable the Rust path."""
    global _native_solve_active
    _native_solve_active = active


# TypeInfo resolver for the Rust path, installed by
# `_build_native_resolvers` alongside the subtype/join resolvers.

# `rust_solve_one` needs it to build `Instance` types (Object/Ancestor
# setop results) and run `is_subtype`. None until a resolver is installed.
_native_solve_resolver: Any = None


def _set_native_solve_resolver(resolver: Any) -> None:
    """Called by the build manager to install the Rust TypeInfo resolver."""
    global _native_solve_resolver
    _native_solve_resolver = resolver


def _serialize_type_list(types: Iterable[Type]) -> list[bytes]:
    """Serialize a `Type` iterable to wire-format blobs for the Rust reader."""
    blobs: list[bytes] = []
    for t in types:
        buf = _WriteBuffer()
        t.write(buf)
        blobs.append(buf.getvalue())
    return blobs


def _serialize_constraint_list(constraints: Iterable[Constraint]) -> list[bytes]:
    """Serialize a `Constraint` iterable to wire-format blobs for the Rust reader."""
    from mypy.constraints import _write_constraint

    blobs: list[bytes] = []
    for c in constraints:
        buf = _WriteBuffer()
        _write_constraint(buf, c)
        blobs.append(buf.getvalue())
    return blobs


def _serialize_constraint(c: Constraint) -> bytes:
    """Serialize a single `Constraint` to wire format for the Rust reader."""
    from mypy.constraints import _write_constraint

    buf = _WriteBuffer()
    _write_constraint(buf, c)
    return buf.getvalue()


def _native_solve_dependent_result(
    result: tuple[Any, Any, Any], originals: dict[TypeVarId, TypeVarLikeType]
) -> tuple[Solutions, list[TypeVarLikeType]] | None:
    """Decode a Rust `rust_solve_dependent` result into (solutions, free_vars).

    `result` is `(num_solved, sol_blob, free_blob)`. A nonzero status or any
    wire-cycle problem returns None, which makes the caller fall back to
    pure Python.
    """
    from mypy.cache import read_int, read_str

    status, sol_blob, free_blob = result
    if status != 0 or sol_blob is None or free_blob is None:
        return None

    solutions: Solutions = {}
    data = _ReadBuffer(bytes(sol_blob))
    count = read_int(data)
    for _ in range(count):
        raw = read_int(data)
        meta = read_int(data)
        ns = read_str(data)
        has_sol = read_int(data)
        tv = TypeVarId(raw, meta, namespace=ns)
        if has_sol == 1:
            from mypy.wirefixup import fixup_wire_type

            decoded = fixup_wire_type(read_type(data))
            if decoded is None:
                return None
            solutions[tv] = decoded
        elif has_sol == 0:
            solutions[tv] = None
        else:
            return None

    free_vars: list[TypeVarLikeType] = []
    data = _ReadBuffer(bytes(free_blob))
    count = read_int(data)
    for _ in range(count):
        raw = read_int(data)
        meta = read_int(data)
        ns = read_str(data)
        flag = read_int(data)
        if flag != 0:
            return None
        tv = TypeVarId(raw, meta, namespace=ns)
        orig = originals.get(tv)
        if orig is None:
            return None
        free_vars.append(orig)
    return solutions, free_vars


_WIRE_SAFE_UNSAFE_TYPES: tuple[type[Type], ...] = (
    TupleType,
    CallableType,
    Overloaded,
    TypeAliasType,
    TypeVarType,
    ParamSpecType,
    TypeVarTupleType,
    TypedDictType,
    UnboundType,
    Parameters,
    TypeType,
    TypeGuardedType,
    ErasedType,
    DeletedType,
)


def _bounds_wire_safe(bounds: Iterable[Type]) -> bool:
    """Whether solve bounds can round-trip through the wire format.

    The wire round-trip rebuilds types as fresh objects, losing
    identity-bearing structure: tuple fallbacks, callable/ParamSpec
    variables, alias nodes, and recursive expansions. Feeding such a
    candidate back into inference breaks identity-dependent checks, so
    only bounds free of those structures take the native path.
    """

    def walk(t: Type) -> bool:
        pt = get_proper_type(t)
        if isinstance(pt, _WIRE_SAFE_UNSAFE_TYPES):
            return False
        if has_recursive_types(pt):
            return False
        if isinstance(pt, UnionType):
            return all(walk(item) for item in pt.items)
        if isinstance(pt, Instance):
            return all(walk(arg) for arg in pt.args)
        return True

    return all(walk(t) for t in bounds)


Bounds: _TypeAlias = "dict[TypeVarId, set[Type]]"
Graph: _TypeAlias = "set[tuple[TypeVarId, TypeVarId]]"
Solutions: _TypeAlias = "dict[TypeVarId, Type | None]"


def solve_constraints(
    original_vars: Sequence[TypeVarLikeType],
    constraints: list[Constraint],
    strict: bool = True,
    allow_polymorphic: bool = False,
    skip_unsatisfied: bool = False,
) -> tuple[list[Type | None], list[TypeVarLikeType]]:
    """Solve type constraints.

    Return the best type(s) for type variables; each type can be None if the value of
    the variable could not be solved.

    If a variable has no constraints, if strict=True then arbitrarily
    pick UninhabitedType as the value of the type variable. If strict=False, pick AnyType.
    If allow_polymorphic=True, then use the full algorithm that can potentially return
    free type variables in solutions (these require special care when applying). Otherwise,
    use a simplified algorithm that just solves each type variable individually if possible.

    The skip_unsatisfied flag matches the same one in applytype.apply_generic_arguments().
    """
    vars = [tv.id for tv in original_vars]
    if not vars:
        return [], []

    originals = {tv.id: tv for tv in original_vars}
    extra_vars: list[TypeVarId] = []
    # Get additional type variables from generic actuals.
    for c in constraints:
        extra_vars.extend([v.id for v in c.extra_tvars if v.id not in vars + extra_vars])
        originals.update({v.id: v for v in c.extra_tvars if v.id not in originals})

    if allow_polymorphic:
        # Constraints inferred from unions require special handling in polymorphic inference.
        constraints = skip_reverse_union_constraints(constraints)

    # Collect a list of constraints for each type variable.
    cmap: dict[TypeVarId, list[Constraint]] = {tv: [] for tv in vars + extra_vars}
    for con in constraints:
        if con.type_var in vars + extra_vars:
            cmap[con.type_var].append(con)

    if allow_polymorphic:
        if constraints:
            solutions, free_vars = solve_with_dependent(
                vars + extra_vars, constraints, vars, originals
            )
        else:
            solutions = {}
            free_vars = []
    else:
        if (
            _HAS_TYPE_KERNEL
            and _native_solve_active
            and _native_solve_resolver is not None
            and constraints
            and type_state.infer_unions
            and state.strict_optional
            and _bounds_wire_safe([c.target for c in constraints])
        ):
            try:
                result = _type_kernel.rust_solve_constraints(
                    _serialize_type_list([originals[tv] for tv in vars + extra_vars]),
                    _serialize_type_list([originals[tv] for tv in vars]),
                    _serialize_constraint_list(constraints),
                    strict,
                    type_state.infer_unions,
                    state.strict_optional,
                    skip_unsatisfied,
                    _native_solve_resolver,
                )
            except (AssertionError, NotImplementedError):
                result = None
            if result is not None:
                out = _native_solve_dependent_result(result, originals)
                if out is not None:
                    solutions_out, _ = out
                    rust_res: list[Type | None] | None = []
                    for v in vars:
                        if v not in solutions_out:
                            rust_res = None
                            break
                        assert rust_res is not None
                        rust_res.append(solutions_out[v])
                    if rust_res is not None:
                        return rust_res, []
        solutions = {}
        free_vars = []
        for tv, cs in cmap.items():
            if not cs:
                continue
            lowers = [c.target for c in cs if c.op == SUPERTYPE_OF]
            uppers = [c.target for c in cs if c.op == SUBTYPE_OF]
            solution = solve_one(lowers, uppers)

            # Do not leak type variables in non-polymorphic solutions.
            if solution is None or not get_vars(
                solution, [tv for tv in extra_vars if tv not in vars]
            ):
                solutions[tv] = solution

    res: list[Type | None] = []
    for v in vars:
        if v in solutions:
            res.append(solutions[v])
        else:
            # No constraints for type variable -- 'UninhabitedType' is the most specific type.
            candidate: Type
            if strict:
                candidate = UninhabitedType()
                candidate.ambiguous = True
            else:
                candidate = AnyType(TypeOfAny.special_form)
            res.append(candidate)

    if not free_vars and not skip_unsatisfied:
        # Most of the validation for solutions is done in applytype.py, but here we can
        # quickly test solutions w.r.t. to upper bounds, and use the latter (if possible),
        # if solutions are actually not valid (due to poor inference context).
        res = pre_validate_solutions(res, original_vars, constraints)

    return res, free_vars


def solve_with_dependent(
    vars: list[TypeVarId],
    constraints: list[Constraint],
    original_vars: list[TypeVarId],
    originals: dict[TypeVarId, TypeVarLikeType],
) -> tuple[Solutions, list[TypeVarLikeType]]:
    """Solve set of constraints that may depend on each other, like T <: List[S].

    The whole algorithm consists of five steps:
      * Propagate via linear constraints and use secondary constraints to get transitive closure
      * Find dependencies between type variables, group them in SCCs, and sort topologically
      * Check that all SCC are intrinsically linear, we can't solve (express) T <: List[T]
      * Variables in leaf SCCs that don't have constant bounds are free (choose one per SCC)
      * Solve constraints iteratively starting from leaves, updating bounds after each step.
    """
    if (
        _HAS_TYPE_KERNEL
        and _native_solve_active
        and _native_solve_resolver is not None
        and constraints
        and type_state.infer_unions
        and state.strict_optional
    ):
        try:
            result = _type_kernel.rust_solve_dependent(
                _serialize_type_list([originals[tv] for tv in vars]),
                _serialize_constraint_list(constraints),
                type_state.infer_unions,
                state.strict_optional,
                _native_solve_resolver,
            )
        except (AssertionError, NotImplementedError):
            result = None
        if result is not None:
            out = _native_solve_dependent_result(result, originals)
            if out is not None:
                return out

    graph, lowers, uppers = transitive_closure(vars, constraints)

    dmap = compute_dependencies(vars, graph, lowers, uppers)
    sccs = list(strongly_connected_components(set(vars), dmap))
    if not all(check_linear(scc, lowers, uppers) for scc in sccs):
        return {}, []
    raw_batches = list(topsort(prepare_sccs(sccs, dmap)))

    free_vars = []
    free_solutions = {}
    for scc in raw_batches[0]:
        # If there are no bounds on this SCC, then the only meaningful solution we can
        # express, is that each variable is equal to a new free variable. For example,
        # if we have T <: S, S <: U, we deduce: T = S = U = <free>.
        if all(not lowers[tv] and not uppers[tv] for tv in scc):
            best_free = choose_free([originals[tv] for tv in scc], original_vars)
            if best_free:
                # TODO: failing to choose may cause leaking type variables,
                # we need to fail gracefully instead.
                free_vars.append(best_free.id)
                free_solutions[best_free.id] = best_free

    # Update lowers/uppers with free vars, so these can now be used
    # as valid solutions.
    for l, u in graph:
        if l in free_vars:
            lowers[u].add(free_solutions[l])
        if u in free_vars:
            uppers[l].add(free_solutions[u])

    # Flatten the SCCs that are independent, we can solve them together,
    # since we don't need to update any targets in between.
    batches = []
    for batch in raw_batches:
        next_bc = []
        for scc in batch:
            next_bc.extend(list(scc))
        batches.append(next_bc)

    solutions: dict[TypeVarId, Type | None] = {}
    for flat_batch in batches:
        res = solve_iteratively(flat_batch, graph, lowers, uppers)
        solutions.update(res)
    return solutions, [free_solutions[tv] for tv in free_vars]


def solve_iteratively(
    batch: list[TypeVarId], graph: Graph, lowers: Bounds, uppers: Bounds
) -> Solutions:
    """Solve transitive closure sequentially, updating upper/lower bounds after each step.

    Transitive closure is represented as a linear graph plus lower/upper bounds for each
    type variable, see transitive_closure() docstring for details.

    We solve for type variables that appear in `batch`. If a bound is not constant (i.e. it
    looks like T :> F[S, ...]), we substitute solutions found so far in the target F[S, ...]
    after solving the batch.

    Importantly, after solving each variable in a batch, we move it from linear graph to
    upper/lower bounds, this way we can guarantee consistency of solutions (see comment below
    for an example when this is important).
    """
    solutions = {}
    s_batch = set(batch)
    while s_batch:
        for tv in sorted(s_batch, key=lambda x: x.raw_id):
            if lowers[tv] or uppers[tv]:
                solvable_tv = tv
                break
        else:
            break
        # Solve each solvable type variable separately.
        s_batch.remove(solvable_tv)
        result = solve_one(lowers[solvable_tv], uppers[solvable_tv])
        solutions[solvable_tv] = result
        if result is None:
            # TODO: support backtracking lower/upper bound choices and order within SCCs.
            # (will require switching this function from iterative to recursive).
            continue

        # Update the (transitive) bounds from graph if there is a solution.
        # This is needed to guarantee solutions will never contradict the initial
        # constraints. For example, consider {T <: S, T <: A, S :> B} with A :> B.

        # If we would not update the uppers/lowers from graph, we would infer T = A, S = B
        # which is not correct.
        for l, u in graph.copy():
            if l == u:
                continue
            if l == solvable_tv:
                lowers[u].add(result)
                graph.remove((l, u))
            if u == solvable_tv:
                uppers[l].add(result)
                graph.remove((l, u))

    # We can update uppers/lowers only once after solving the whole SCC,
    # since uppers/lowers can't depend on type variables in the SCC
    # (and we would reject such SCC as non-linear and therefore not solvable).
    subs = {tv: s for (tv, s) in solutions.items() if s is not None}
    for tv in lowers:
        lowers[tv] = {expand_type(lt, subs) for lt in lowers[tv]}
    for tv in uppers:
        uppers[tv] = {expand_type(ut, subs) for ut in uppers[tv]}
    return solutions


def _join_sorted_key(t: Type) -> int:
    if _HAS_TYPE_KERNEL and _native_solve_active:
        try:
            result = _type_kernel.rust_join_sorted_key(_serialize_type_payload(t))
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    t = get_proper_type(t)
    if isinstance(t, UnionType):
        return -2
    if isinstance(t, NoneType):
        return -1

    if isinstance(t, Overloaded):
        return 1
    return 0


def solve_one(lowers: Iterable[Type], uppers: Iterable[Type]) -> Type | None:
    """Solve constraints by finding by using meets of upper bounds, and joins of lower bounds."""

    candidate: Type | None = None

    # Filter out previous results of failed inference, they will only spoil the current
    # pass...
    new_uppers = []
    for u in uppers:
        pu = get_proper_type(u)
        if not isinstance(pu, UninhabitedType) or not pu.ambiguous:
            new_uppers.append(u)
    uppers = new_uppers

    # ...unless this is the only information we have, then we just pass it on.
    lowers = list(lowers)
    if not uppers and not lowers:
        candidate = UninhabitedType()
        candidate.ambiguous = True
        return candidate

    # Single-bound no-op solves return the bound itself; defer so the
    # original object (identity) survives, since the wire round-trip
    # would rebuild a fresh instance that breaks identity checks.
    _noop = (len(lowers) == 1 and not uppers) or (len(uppers) == 1 and not lowers)

    if (
        _HAS_TYPE_KERNEL
        and _native_solve_active
        and _native_solve_resolver is not None
        and not _noop
    ):
        if _bounds_wire_safe(list(lowers) + list(uppers)):
            try:
                result = _type_kernel.rust_solve_one(
                    _serialize_type_list(lowers),
                    _serialize_type_list(uppers),
                    type_state.infer_unions,
                    state.strict_optional,
                    _native_solve_resolver,
                )
            except (AssertionError, NotImplementedError):
                result = None
            if result is not None:
                kind, blob = result
                if kind == 0:
                    if blob is not None:
                        from mypy.wirefixup import fixup_wire_type

                        decoded = read_type(_ReadBuffer(bytes(blob)))
                        fixed = fixup_wire_type(decoded)
                        if fixed is not None:
                            return fixed
                        # fixup_wire_type defers (e.g. a bound carrying an
                        # unexpanded alias, #1093): fall through to the
                        # Python body instead of reporting "no solution".
                    else:
                        return None
                if kind == 1:
                    if blob is not None:
                        from mypy.wirefixup import fixup_wire_type

                        decoded = read_type(_ReadBuffer(bytes(blob)))
                        fixed = fixup_wire_type(decoded)
                        if fixed is not None:
                            return fixed
                        # Same defer-on-unfixable fall-through as kind 0.
                    else:
                        return None
                if kind == 2:
                    # No bounds at all, ambiguous Never.
                    candidate = UninhabitedType()
                    candidate.ambiguous = True
                    return candidate
                if kind == 3:
                    # Any-absorption. Source_any is the Any side (or
                    # None via LITERAL_NONE), so build AnyType only when real.
                    from mypy.wirefixup import decode_source_any

                    if blob is not None:
                        source_any = decode_source_any(_ReadBuffer(bytes(blob)))
                        if source_any is not None:
                            return AnyType(
                                TypeOfAny.from_another_any, source_any=source_any
                            )

    bottom: Type | None = None
    top: Type | None = None

    # Process each bound separately, and calculate the lower and upper
    # bounds based on constraints. Note that we assume that the constraint
    # targets do not have constraint references.
    if type_state.infer_unions and lowers:
        # This deviates from the general mypy semantics because
        # recursive types are union-heavy in 95% of cases.
        # Retain `None` when no bottoms were provided to avoid bogus `Never` inference.
        bottom = UnionType.make_union(lowers)
    else:
        # The order of lowers is non-deterministic.
        # We attempt to sort lowers because joins are non-associative. For instance:

        # join(join(int, str), int | str) == join(object, int | str) == object
        # join(int, join(str, int | str)) == join(int, int | str)    == int | str
        # Note that joins in theory should be commutative, but in practice some

        # bugs mean this is also a source of non-deterministic type checking
        # results.
        sorted_lowers = sorted(lowers, key=_join_sorted_key)
        if sorted_lowers:
            bottom = join_type_list(sorted_lowers)

    for target in uppers:
        if top is None:
            top = target
        else:
            top = meet_types(top, target)

    p_top = get_proper_type(top)
    p_bottom = get_proper_type(bottom)
    if isinstance(p_top, AnyType) or isinstance(p_bottom, AnyType):
        source_any = top if isinstance(p_top, AnyType) else bottom
        assert isinstance(source_any, ProperType) and isinstance(source_any, AnyType)
        return AnyType(TypeOfAny.from_another_any, source_any=source_any)
    elif bottom is None:
        if top:
            candidate = top
        else:
            # No constraints for type variable
            return None
    elif top is None:
        candidate = bottom
    elif is_subtype(bottom, top):
        candidate = bottom
    else:
        candidate = None
    return candidate


def choose_free(
    scc: list[TypeVarLikeType], original_vars: list[TypeVarId]
) -> TypeVarLikeType | None:
    """Choose the best solution for an SCC containing only type variables.

    This is needed to preserve e.g. the upper bound in a situation like this:
        def dec(f: Callable[[T], S]) -> Callable[[T], S]: ...

        @dec
        def test(x: U) -> U: ...

    where U <: A.
    """

    if len(scc) == 1:
        # Fast path, choice is trivial.
        return scc[0]

    common_upper_bound = meet_type_list([t.upper_bound for t in scc])
    common_upper_bound_p = get_proper_type(common_upper_bound)
    # We include None for when strict-optional is disabled.
    if isinstance(common_upper_bound_p, (UninhabitedType, NoneType)):
        # This will cause to infer Never, which is better than a free TypeVar
        # that has an upper bound Never.
        return None

    values: list[Type] = []
    for tv in scc:
        if isinstance(tv, TypeVarType) and tv.values:
            if values:
                # It is too tricky to support multiple TypeVars with values
                # within the same SCC.
                return None
            values = tv.values.copy()

    if values and not is_trivial_bound(common_upper_bound_p):
        # If there are both values and upper bound present, we give up,
        # since type variables having both are not supported.
        return None

    # For convenience with current type application machinery, we use a stable
    # choice that prefers the original type variables (not polymorphic ones) in SCC.
    best = min(scc, key=lambda x: (x.id not in original_vars, x.id.raw_id))
    if isinstance(best, TypeVarType):
        return best.copy_modified(values=values, upper_bound=common_upper_bound)
    if is_trivial_bound(common_upper_bound_p, allow_tuple=True):
        # TODO: support more cases for ParamSpecs/TypeVarTuples
        return best
    return None


def _serialize_type_payload(tp: Type) -> bytes:
    """Serialize a single `Type` to wire format for the Rust reader."""
    buf = _WriteBuffer()
    tp.write(buf)
    return buf.getvalue()


def is_trivial_bound(tp: ProperType, allow_tuple: bool = False) -> bool:
    if _HAS_TYPE_KERNEL and _native_solve_active:
        try:
            result = _type_kernel.rust_is_trivial_bound(
                _serialize_type_payload(tp), allow_tuple
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(tp, Instance) and tp.type.fullname == "builtins.tuple":
        return allow_tuple and is_trivial_bound(get_proper_type(tp.args[0]))
    return isinstance(tp, Instance) and tp.type.fullname == "builtins.object"


def find_linear(c: Constraint) -> tuple[bool, TypeVarId | None]:
    """Find out if this constraint represent a linear relationship, return target id if yes."""
    if _HAS_TYPE_KERNEL and _native_solve_active:
        try:
            result = _type_kernel.rust_find_linear(_serialize_constraint(c))
        except (AssertionError, NotImplementedError):
            result = None
        if result is not None:
            is_linear, _ = result
            if is_linear:
                # Rust says linear. The wire drops meta_level for ParamSpec /
                # TypeVarTuple targets (it lives only on TypeVarType), so rebuild
                # the id from live objects to match the Python membership check.
                if isinstance(c.origin_type_var, TypeVarType):
                    if isinstance(c.target, TypeVarType):
                        return True, c.target.id
                if isinstance(c.origin_type_var, ParamSpecType):
                    if isinstance(c.target, ParamSpecType) and not c.target.prefix.arg_types:
                        return True, c.target.id
                if isinstance(c.origin_type_var, TypeVarTupleType):
                    target = get_proper_type(c.target)
                    if isinstance(target, TupleType) and len(target.items) == 1:
                        item = target.items[0]
                        if isinstance(item, UnpackType) and isinstance(item.type, TypeVarTupleType):
                            return True, item.type.id
            # Rust's negative answer does not expand `TypeAliasType` targets
            # (Python's `get_proper_type` does), so a `False` here is not
            # authoritative: fall through to the Python body verbatim.
    if isinstance(c.origin_type_var, TypeVarType):
        if isinstance(c.target, TypeVarType):
            return True, c.target.id
    if isinstance(c.origin_type_var, ParamSpecType):
        if isinstance(c.target, ParamSpecType) and not c.target.prefix.arg_types:
            return True, c.target.id
    if isinstance(c.origin_type_var, TypeVarTupleType):
        target = get_proper_type(c.target)
        if isinstance(target, TupleType) and len(target.items) == 1:
            item = target.items[0]
            if isinstance(item, UnpackType) and isinstance(item.type, TypeVarTupleType):
                return True, item.type.id
    return False, None


def transitive_closure(
    tvars: list[TypeVarId], constraints: list[Constraint]
) -> tuple[Graph, Bounds, Bounds]:
    """Find transitive closure for given constraints on type variables.

    Transitive closure gives maximal set of lower/upper bounds for each type variable,
    such that we cannot deduce any further bounds by chaining other existing bounds.

    The transitive closure is represented by:
      * A set of lower and upper bounds for each type variable, where only constant and
        non-linear terms are included in the bounds.
      * A graph of linear constraints between type variables (represented as a set of pairs)
    Such separation simplifies reasoning, and allows an efficient and simple incremental
    transitive closure algorithm that we use here.

    For example if we have initial constraints [T <: S, S <: U, U <: int], the transitive
    closure is given by:
      * {} <: T <: {int}
      * {} <: S <: {int}
      * {} <: U <: {int}
      * {T <: S, S <: U, T <: U}
    """
    uppers: Bounds = defaultdict(set)
    lowers: Bounds = defaultdict(set)
    graph: Graph = {(tv, tv) for tv in tvars}

    remaining = set(constraints)
    while remaining:
        c = remaining.pop()
        # Note that ParamSpec constraint P <: Q may be considered linear only if Q
        # has no prefix,
        # for cases like P <: Concatenate[T, Q] we should consider this non-linear

        # and put {P} and
        # {T, Q} into separate SCCs. Similarly, Ts <: Tuple[*Us] considered linear,
        # while Ts <: Tuple[*Us, U] is non-linear.
        is_linear, target_id = find_linear(c)
        if is_linear and target_id in tvars:
            assert target_id is not None
            if c.op == SUBTYPE_OF:
                lower, upper = c.type_var, target_id
            else:
                lower, upper = target_id, c.type_var
            if (lower, upper) in graph:
                continue
            graph |= {
                (l, u) for l in tvars for u in tvars if (l, lower) in graph and (upper, u) in graph
            }
            for u in tvars:
                if (upper, u) in graph:
                    lowers[u] |= lowers[lower]
            for l in tvars:
                if (l, lower) in graph:
                    uppers[l] |= uppers[upper]
            for lt in lowers[lower]:
                for ut in uppers[upper]:
                    add_secondary_constraints(remaining, lt, ut)
        elif c.op == SUBTYPE_OF:
            if c.target in uppers[c.type_var]:
                continue
            for l in tvars:
                if (l, c.type_var) in graph:
                    uppers[l].add(c.target)
            for lt in lowers[c.type_var]:
                add_secondary_constraints(remaining, lt, c.target)
        else:
            assert c.op == SUPERTYPE_OF
            if c.target in lowers[c.type_var]:
                continue
            for u in tvars:
                if (c.type_var, u) in graph:
                    lowers[u].add(c.target)
            for ut in uppers[c.type_var]:
                add_secondary_constraints(remaining, c.target, ut)
    return graph, lowers, uppers


def add_secondary_constraints(cs: set[Constraint], lower: Type, upper: Type) -> None:
    """Add secondary constraints inferred between lower and upper (in place)."""
    if isinstance(get_proper_type(upper), UnionType) and isinstance(
        get_proper_type(lower), UnionType
    ):
        # When both types are unions, this can lead to inferring spurious constraints,
        # for example Union[T, int] <: S <: Union[T, int] may infer T <: int.
        # To avoid this, just skip them for now.
        return
    # TODO: what if secondary constraints result in inference against polymorphic actual?
    cs.update(set(infer_constraints(lower, upper, SUBTYPE_OF)))
    cs.update(set(infer_constraints(upper, lower, SUPERTYPE_OF)))


def compute_dependencies(
    tvars: list[TypeVarId], graph: Graph, lowers: Bounds, uppers: Bounds
) -> dict[TypeVarId, list[TypeVarId]]:
    """Compute dependencies between type variables induced by constraints.

    If we have a constraint like T <: List[S], we say that T depends on S, since
    we will need to solve for S first before we can solve for T.
    """
    res = {}
    for tv in tvars:
        deps = set()
        for lt in lowers[tv]:
            deps |= get_vars(lt, tvars)
        for ut in uppers[tv]:
            deps |= get_vars(ut, tvars)
        for other in tvars:
            if other == tv:
                continue
            if (tv, other) in graph or (other, tv) in graph:
                deps.add(other)
        res[tv] = list(deps)
    return res


def check_linear(scc: set[TypeVarId], lowers: Bounds, uppers: Bounds) -> bool:
    """Check there are only linear constraints between type variables in SCC.

    Linear are constraints like T <: S (while T <: F[S] are non-linear).
    """
    for tv in scc:
        if any(get_vars(lt, list(scc)) for lt in lowers[tv]):
            return False
        if any(get_vars(ut, list(scc)) for ut in uppers[tv]):
            return False
    return True


def skip_reverse_union_constraints(cs: list[Constraint]) -> list[Constraint]:
    """Avoid ambiguities for constraints inferred from unions during polymorphic inference.

    Polymorphic inference implicitly relies on assumption that a reverse of a linear constraint
    is a linear constraint. This is however not true in presence of union types, for example
    T :> Union[S, int] vs S <: T. Trying to solve such constraints would be detected ambiguous
    as (T, S) form a non-linear SCC. However, simply removing the linear part results in a valid
    solution T = Union[S, int], S = <free>. A similar scenario is when we get T <: Union[T, int],
    such constraints carry no information, and will equally confuse linearity check.

    TODO: a cleaner solution may be to avoid inferring such constraints in first place, but
    this would require passing around a flag through all infer_constraints() calls.
    """
    if _HAS_TYPE_KERNEL and _native_solve_active and _bounds_wire_safe([c.target for c in cs]):
        try:
            buf = _WriteBuffer()
            from mypy.constraints import _write_option

            _write_option(buf, cs)
            raw = _type_kernel.rust_skip_reverse_union_constraints(buf.getvalue())
            if raw is not None:
                data = _ReadBuffer(bytes(raw))
                from mypy.cache import read_int_bare  # type: ignore[attr-defined]

                count = read_int_bare(data)
                result: list[Constraint] = []
                for _ in range(count):
                    from mypy.cache import read_int

                    from mypy.wirefixup import fixup_wire_type

                    origin = read_type(data)
                    origin = fixup_wire_type(origin)
                    if origin is None:
                        raise NotImplementedError("origin unresolvable on wire")
                    op = read_int(data)
                    target = read_type(data)
                    target = fixup_wire_type(target)
                    if target is None:
                        raise NotImplementedError("target unresolvable on wire")
                    result.append(Constraint(origin, op, target))  # type: ignore[arg-type]
                return result
        except (AssertionError, NotImplementedError, ValueError, AttributeError):
            pass
    reverse_union_cs = set()
    for c in cs:
        p_target = get_proper_type(c.target)
        if isinstance(p_target, UnionType):
            for item in p_target.items:
                if isinstance(item, TypeVarType):
                    if item == c.origin_type_var and c.op == SUBTYPE_OF:
                        reverse_union_cs.add(c)
                        continue
                    # These two forms are semantically identical, but are different from
                    # the point of view of Constraint.__eq__().
                    reverse_union_cs.add(Constraint(item, neg_op(c.op), c.origin_type_var))
                    reverse_union_cs.add(Constraint(c.origin_type_var, c.op, item))
    return [c for c in cs if c not in reverse_union_cs]


def get_vars(target: Type, vars: list[TypeVarId]) -> set[TypeVarId]:
    """Find type variables for which we are solving in a target type."""
    if _HAS_TYPE_KERNEL and _native_solve_active and _bounds_wire_safe([target]):
        try:
            result = _type_kernel.rust_get_vars(
                _serialize_type_payload(target),
                [(v.raw_id, v.meta_level, v.namespace) for v in vars],
            )
            if result is not None:
                return {TypeVarId(r, m, namespace=n) for r, m, n in result}
        except (AssertionError, NotImplementedError):
            pass
    return {tv.id for tv in get_all_type_vars(target)} & set(vars)


def pre_validate_solutions(
    solutions: list[Type | None],
    original_vars: Sequence[TypeVarLikeType],
    constraints: list[Constraint],
) -> list[Type | None]:
    """Check is each solution satisfies the upper bound of the corresponding type variable.

    If it doesn't satisfy the bound, check if bound itself satisfies all constraints, and
    if yes, use it instead as a fallback solution.
    """
    new_solutions: list[Type | None] = []
    for t, s in zip(original_vars, solutions):
        if is_callable_protocol(t.upper_bound):
            # This is really ad-hoc, but a proper fix would be much more complex,
            # and otherwise this may cause crash in a relatively common scenario.
            new_solutions.append(s)
            continue
        if s is not None and not is_subtype(s, t.upper_bound):
            bound_satisfies_all = True
            for c in constraints:
                if c.op == SUBTYPE_OF and not is_subtype(t.upper_bound, c.target):
                    bound_satisfies_all = False
                    break
                if c.op == SUPERTYPE_OF and not is_subtype(c.target, t.upper_bound):
                    bound_satisfies_all = False
                    break
            if bound_satisfies_all:
                new_solutions.append(t.upper_bound)
                continue
        new_solutions.append(s)
    return new_solutions


def is_callable_protocol(t: Type) -> bool:
    if (
        _HAS_TYPE_KERNEL
        and _native_solve_active
        and _native_solve_resolver is not None
        and _bounds_wire_safe([t])
    ):
        try:
            result = _type_kernel.rust_is_callable_protocol(
                _native_solve_resolver, _serialize_type_payload(t)
            )
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    proper_t = get_proper_type(t)
    if isinstance(proper_t, Instance) and proper_t.type.is_protocol:
        return "__call__" in proper_t.type.protocol_members
    return False
