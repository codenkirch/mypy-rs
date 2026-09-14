"""Pattern checker. This file is conceptually part of TypeChecker."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterator
from typing import Final, NamedTuple

from mypy import message_registry
from mypy.checker_shared import TypeCheckerSharedApi, TypeRange
from mypy.checkmember import analyze_member_access
from mypy.expandtype import expand_type_by_instance
from mypy.join import join_types
from mypy.literals import literal_hash
from mypy.maptype import map_instance_to_supertype
from mypy.meet import narrow_declared_type
from mypy.messages import MessageBuilder
from mypy.nodes import ARG_POS, Expression, NameExpr, TempNode, TypeAlias, Var
from mypy.options import Options
from mypy.patterns import (
    AsPattern,
    ClassPattern,
    MappingPattern,
    OrPattern,
    Pattern,
    SequencePattern,
    SingletonPattern,
    StarredPattern,
    ValuePattern,
)
from mypy.plugin import Plugin
from mypy.subtypes import is_subtype
from mypy.typeops import (
    coerce_to_literal,
    make_simplified_union,
    try_getting_str_literals_from_type,
    tuple_fallback,
)
from mypy.types import (
    AnyType,
    FunctionLike,
    Instance,
    NoneType,
    ProperType,
    TupleType,
    Type,
    TypedDictType,
    TypeOfAny,
    TypeType,
    TypeVarTupleType,
    TypeVarType,
    UninhabitedType,
    UnionType,
    UnpackType,
    callable_with_ellipsis,
    find_unpack_in_list,
    get_proper_type,
    split_with_prefix_and_suffix,
)
from mypy.typevars import fill_typevars, fill_typevars_with_any
from mypy.visitor import PatternVisitor

# M22 type-kernel seam: with `type_kernel` importable and
# `Options.native_type_kernel` set, standalone helpers route through Rust;
# Rust returns None for unhandled cases so Python falls back (strangler-fig).
try:
    import type_kernel as _type_kernel
    from librt.internal import WriteBuffer as _WriteBuffer

    _HAS_TYPE_KERNEL = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _HAS_TYPE_KERNEL = False

# Module-level flag, set by the build manager from Options.native_type_kernel.
_native_checkpattern_active: bool = False


def _set_native_checkpattern_active(active: bool) -> None:
    """Called by the build manager to enable/disable the Rust path."""
    global _native_checkpattern_active
    _native_checkpattern_active = active


def _serialize_type(t: Type) -> bytes:
    buf = _WriteBuffer()
    t.write(buf)
    return buf.getvalue()


# Branch tags returned by rust_classify_class_pattern_ranges (issue #987).
_CLASS_PATTERN_FAIL: Final = 0
_CLASS_PATTERN_TYPE_OBJ: Final = 1
_CLASS_PATTERN_CALLABLE_VAR: Final = 2
_CLASS_PATTERN_TYPE_TYPE: Final = 3
_CLASS_PATTERN_ANY: Final = 4

# Branch tags for rust_classify_sequence_pattern_head (issue #1633): the
# step 1-3 dispatch of visit_sequence_pattern. The four early-non-match
# tags all map to early_non_match() in the shim.
_SEQ_CANNOT_MATCH: Final = 0
_SEQ_UNION: Final = 1
_SEQ_FIXED_TOO_FEW: Final = 2
_SEQ_FIXED_TOO_MANY: Final = 3
_SEQ_FIXED_OK: Final = 4
_SEQ_VARIADIC_TOO_MANY: Final = 5
_SEQ_VARIADIC_OK: Final = 6
_SEQ_ANY: Final = 7
_SEQ_ITERABLE: Final = 8
_SEQ_OTHER: Final = 9
_SEQ_EARLY_NON_MATCH_TAGS: Final = frozenset(
    {_SEQ_CANNOT_MATCH, _SEQ_FIXED_TOO_FEW, _SEQ_FIXED_TOO_MANY, _SEQ_VARIADIC_TOO_MANY}
)

# Rest-type tags for rust_classify_sequence_tuple_result (issue #1633).
_SEQ_REST_ALL: Final = 0
_SEQ_REST_SINGLE: Final = 1
_SEQ_REST_KEEP: Final = 2

# Branch tags for rust_classify_mapping_rest (issue #1633).
_MAP_INSTANCE: Final = 0
_MAP_DICT_FALLBACK: Final = 1

# Violation tags for rust_classify_class_pattern_keywords (issue #1633).
_CLS_KW_TOO_MANY: Final = 1
_CLS_KW_MATCHES_POSITIONAL: Final = 2
_CLS_KW_DUPLICATE: Final = 3


def _deserialize_type_list(result: list[bytes] | list[list[int]]) -> list[Type] | None:
    from mypy.typeops import _deserialize_type

    out: list[Type] = []
    for b in result:
        blob = bytes(b) if isinstance(b, list) else b
        t = _deserialize_type(blob)
        if t is None:
            return None
        out.append(t)
    return out


self_match_type_names: Final = [
    "builtins.bool",
    "builtins.bytearray",
    "builtins.bytes",
    "builtins.dict",
    "builtins.float",
    "builtins.frozenset",
    "builtins.int",
    "builtins.list",
    "builtins.set",
    "builtins.str",
    "builtins.tuple",
]

non_sequence_match_type_names: Final = ["builtins.str", "builtins.bytes", "builtins.bytearray"]


# For every Pattern a PatternType can be calculated, by recursively calculating
# the PatternTypes of the sub-patterns first.
# Using the PatternType data the match subject and captures can be narrowed.
class PatternType(NamedTuple):
    type: Type  # The type the match subject can be narrowed to
    rest_type: Type  # The remaining type if the pattern didn't match
    captures: dict[Expression, Type]  # The variables captured by the pattern


class PatternChecker(PatternVisitor[PatternType]):
    """Pattern checker.

    This class checks if a pattern can match a type, what the type can be narrowed to, and what
    type capture patterns should be inferred as.
    """

    # Some services are provided by a TypeChecker instance.
    chk: TypeCheckerSharedApi
    # This is shared with TypeChecker, but stored also here for convenience.
    msg: MessageBuilder
    # Currently unused
    plugin: Plugin
    # The expression being matched against the pattern
    subject: Expression

    subject_type: Type
    # Type of the subject to check the (sub)pattern against
    type_context: list[Type]
    # Types matching self instead of __match_args__ when used as a class pattern
    # Filled in from self_match_type_names
    self_match_types: list[Type]
    # Types that are sequences, but don't match sequence patterns. Filled in from
    # non_sequence_match_type_names
    non_sequence_match_types: list[Type]

    options: Options

    def __init__(
        self, chk: TypeCheckerSharedApi, msg: MessageBuilder, plugin: Plugin, options: Options
    ) -> None:
        self.chk = chk
        self.msg = msg
        self.plugin = plugin

        self.type_context = []
        self.self_match_types = self.generate_types_from_names(self_match_type_names)
        self.non_sequence_match_types = self.generate_types_from_names(
            non_sequence_match_type_names
        )
        self.options = options

    def accept(self, o: Pattern, type_context: Type) -> PatternType:
        self.type_context.append(type_context)
        result = o.accept(self)
        self.type_context.pop()

        return result

    def visit_as_pattern(self, o: AsPattern) -> PatternType:
        current_type = self.type_context[-1]
        if o.pattern is not None:
            pattern_type = self.accept(o.pattern, current_type)
            typ, rest_type, type_map = pattern_type
        else:
            typ, rest_type, type_map = current_type, UninhabitedType(), {}

        if not is_uninhabited(typ) and o.name is not None:
            typ, _ = self.chk.conditional_types_with_intersection(
                current_type, [get_type_range(typ)], o, default=current_type
            )
            if not is_uninhabited(typ):
                type_map[o.name] = typ

        return PatternType(typ, rest_type, type_map)

    def _filter_or_match_types(self, pattern_types: list[PatternType]) -> list[int]:
        """Indices of or-alternatives whose match type is inhabited (#1633).

        Rust folds the per-alternative `is_uninhabited` checks into one
        call; the union itself stays with `make_simplified_union` (already
        native under its own gate), so no union path forks here. The shim
        maps indices back onto the live PatternTypes, so no decoded type
        crosses back.
        """
        if _HAS_TYPE_KERNEL and _native_checkpattern_active:
            from mypy.subtypes import _native_subtype_resolver

            resolver = _native_subtype_resolver
            if resolver is not None:
                try:
                    kept = _type_kernel.rust_filter_or_match_types(
                        [_serialize_type(p.type) for p in pattern_types], resolver
                    )
                    if kept is not None:
                        return [int(i) for i in kept]
                except (AssertionError, NotImplementedError):
                    pass
        return [i for i, p in enumerate(pattern_types) if not is_uninhabited(p.type)]

    def visit_or_pattern(self, o: OrPattern) -> PatternType:
        current_type = self.type_context[-1]

        #
        # Check all the subpatterns
        #
        pattern_types = []
        for pattern in o.patterns:
            pattern_type = self.accept(pattern, current_type)
            pattern_types.append(pattern_type)
            if not is_uninhabited(pattern_type.type):
                current_type = pattern_type.rest_type

        #
        # Collect the final type
        #
        kept = self._filter_or_match_types(pattern_types)
        types = [pattern_types[i].type for i in kept]

        #
        # Check the capture types
        #
        capture_types: dict[Var, list[tuple[Expression, Type]]] = defaultdict(list)
        # Collect captures from the first subpattern
        for expr, typ in pattern_types[0].captures.items():
            node = get_var(expr)
            capture_types[node].append((expr, typ))

        # Check if other subpatterns capture the same names
        for i, pattern_type in enumerate(pattern_types[1:]):
            vars = {get_var(expr) for expr, _ in pattern_type.captures.items()}
            if capture_types.keys() != vars:
                self.msg.fail(message_registry.OR_PATTERN_ALTERNATIVE_NAMES, o.patterns[i])
            for expr, typ in pattern_type.captures.items():
                node = get_var(expr)
                capture_types[node].append((expr, typ))

        captures: dict[Expression, Type] = {}
        for capture_list in capture_types.values():
            typ = UninhabitedType()
            for _, other in capture_list:
                typ = make_simplified_union([typ, other])

            captures[capture_list[0][0]] = typ

        union_type = make_simplified_union(types)
        return PatternType(union_type, current_type, captures)

    def visit_value_pattern(self, o: ValuePattern) -> PatternType:
        current_type = self.type_context[-1]
        typ = self.chk.expr_checker.accept(o.expr)
        typ = coerce_to_literal(typ)
        node = TempNode(current_type)
        # Value patterns are essentially a syntactic sugar on top of `if x == Value`.
        # They should be treated equivalently.
        ok_map, rest_map = self.chk.narrow_type_by_identity_equality(
            "==", [node, TempNode(typ)], [current_type, typ], [0, 1], {0}
        )
        ok_type = ok_map.get(node, current_type) if ok_map is not None else UninhabitedType()
        rest_type = rest_map.get(node, current_type) if rest_map is not None else UninhabitedType()
        return PatternType(ok_type, rest_type, {})

    def visit_singleton_pattern(self, o: SingletonPattern) -> PatternType:
        current_type = self.type_context[-1]
        value: bool | None = o.value
        if isinstance(value, bool):
            typ = self.chk.expr_checker.infer_literal_expr_type(value, "builtins.bool")
        elif value is None:
            typ = NoneType()
        else:
            assert False

        narrowed_type, rest_type = self.chk.conditional_types_with_intersection(
            current_type, [get_type_range(typ)], o, default=current_type
        )
        return PatternType(narrowed_type, rest_type, {})

    def _classify_sequence_head(
        self, current_type: ProperType, star_position: int | None, required_patterns: int
    ) -> int:
        """Step 1-3 dispatch of visit_sequence_pattern (#1633).

        Rust decides the branch in one call (running the already-ported
        can_match_sequence logic inline); the shim only routes. A `None`
        answer runs the verbatim Python head below, so routing agrees on
        both paths by construction.
        """
        if _HAS_TYPE_KERNEL and _native_checkpattern_active and self.non_sequence_match_types:
            from mypy.subtypes import _native_subtype_resolver

            resolver = _native_subtype_resolver
            if resolver is not None:
                non_seq_union = UnionType.make_union(self.non_sequence_match_types)
                sequence = self.chk.named_type("typing.Sequence")
                iterable = self.chk.named_generic_type(
                    "typing.Iterable", [AnyType(TypeOfAny.special_form)]
                )
                try:
                    tag = _type_kernel.rust_classify_sequence_pattern_head(
                        _serialize_type(current_type),
                        star_position,
                        required_patterns,
                        _serialize_type(non_seq_union),
                        _serialize_type(sequence),
                        _serialize_type(iterable),
                        resolver,
                    )
                    if tag is not None:
                        return int(tag)
                except (AssertionError, NotImplementedError):
                    pass
        return self._sequence_head_python(current_type, star_position, required_patterns)

    def _sequence_head_python(
        self, current_type: ProperType, star_position: int | None, required_patterns: int
    ) -> int:
        if not self.can_match_sequence(current_type):
            return _SEQ_CANNOT_MATCH
        if isinstance(current_type, UnionType):
            return _SEQ_UNION
        if isinstance(current_type, TupleType):
            if find_unpack_in_list(current_type.items) is None:
                size_diff = len(current_type.items) - required_patterns
                if size_diff < 0:
                    return _SEQ_FIXED_TOO_FEW
                if size_diff > 0 and star_position is None:
                    return _SEQ_FIXED_TOO_MANY
                return _SEQ_FIXED_OK
            if len(current_type.items) - 1 > required_patterns and star_position is None:
                return _SEQ_VARIADIC_TOO_MANY
            return _SEQ_VARIADIC_OK
        if isinstance(current_type, AnyType):
            return _SEQ_ANY
        if isinstance(current_type, Instance) and self.chk.type_is_iterable(current_type):
            return _SEQ_ITERABLE
        return _SEQ_OTHER

    def _classify_sequence_tuple_result(
        self, new_inner_types: list[Type], rest_inner_types: list[Type]
    ) -> tuple[bool, int, int, list[bool]]:
        """Step-5 arbitration for a fixed tuple in visit_sequence_pattern.

        Returns (new_uninhabited, rest_tag, rest_index, rest_mask); the
        mask lets the shim rebuild the single-conditional tuple without
        re-querying is_uninhabited per item.
        """
        if _HAS_TYPE_KERNEL and _native_checkpattern_active:
            from mypy.subtypes import _native_subtype_resolver

            resolver = _native_subtype_resolver
            if resolver is not None:
                try:
                    decided = _type_kernel.rust_classify_sequence_tuple_result(
                        [_serialize_type(t) for t in new_inner_types],
                        [_serialize_type(t) for t in rest_inner_types],
                        resolver,
                    )
                    if decided is not None:
                        new_uninh, rest_tag, rest_idx, mask = decided
                        return (bool(new_uninh), int(rest_tag), int(rest_idx), list(mask))
                except (AssertionError, NotImplementedError):
                    pass
        new_uninhabited = any(is_uninhabited(t) for t in new_inner_types)
        mask = [is_uninhabited(t) for t in rest_inner_types]
        num_always_match = sum(mask)
        if num_always_match == len(rest_inner_types):
            return (new_uninhabited, _SEQ_REST_ALL, -1, mask)
        if num_always_match == len(rest_inner_types) - 1:
            idx = next(i for i, u in enumerate(mask) if not u)
            return (new_uninhabited, _SEQ_REST_SINGLE, idx, mask)
        return (new_uninhabited, _SEQ_REST_KEEP, -1, mask)

    def visit_sequence_pattern(self, o: SequencePattern) -> PatternType:
        #
        # Step 1. Check for existence of a starred pattern
        #
        current_type = get_proper_type(self.type_context[-1])
        star_positions = [i for i, p in enumerate(o.patterns) if isinstance(p, StarredPattern)]
        star_position: int | None = None
        if len(star_positions) == 1:
            star_position = star_positions[0]
        elif len(star_positions) >= 2:
            assert False, "Parser should prevent multiple starred patterns"
        required_patterns = len(o.patterns)
        if star_position is not None:
            required_patterns -= 1

        head = self._classify_sequence_head(current_type, star_position, required_patterns)
        if head in _SEQ_EARLY_NON_MATCH_TAGS:
            return self.early_non_match()

        #
        # Step 2. If we have a union, recurse and return the combined result
        #
        if head == _SEQ_UNION:
            assert isinstance(current_type, UnionType)
            match_types: list[Type] = []
            rest_types: list[Type] = []
            captures_list: dict[Expression, list[Type]] = {}

            if star_position is not None:
                star_pattern = o.patterns[star_position]
                assert isinstance(star_pattern, StarredPattern)
                star_expr = star_pattern.capture
            else:
                star_expr = None

            for t in current_type.items:
                match_type, rest_type, captures = self.accept(o, t)
                match_types.append(match_type)
                rest_types.append(rest_type)
                if not is_uninhabited(match_type):
                    for expr, typ in captures.items():
                        p_typ = get_proper_type(typ)
                        if expr not in captures_list:
                            captures_list[expr] = []
                        # Avoid adding in a list[Never] for empty list captures
                        if (
                            expr == star_expr
                            and isinstance(p_typ, Instance)
                            and p_typ.type.fullname == "builtins.list"
                            and is_uninhabited(p_typ.args[0])
                        ):
                            continue
                        captures_list[expr].append(typ)

            return PatternType(
                make_simplified_union(match_types),
                make_simplified_union(rest_types),
                {expr: make_simplified_union(types) for expr, types in captures_list.items()},
            )

        #
        # Step 3. Get inner types of original type (sizes decided by head).
        #
        unpack_index = None
        if head == _SEQ_FIXED_OK:
            assert isinstance(current_type, TupleType)
            inner_types: list[Type] = current_type.items
            assert find_unpack_in_list(inner_types) is None
        elif head == _SEQ_VARIADIC_OK:
            assert isinstance(current_type, TupleType)
            inner_types = current_type.items
            unpack_index = find_unpack_in_list(inner_types)
            assert unpack_index is not None
            normalized_inner_types = []
            for it in inner_types:
                # Unfortunately, it is not possible to "split" the TypeVarTuple
                # into individual items, so we just use its upper bound for the whole
                # analysis instead.
                if isinstance(it, UnpackType) and isinstance(it.type, TypeVarTupleType):
                    it = UnpackType(it.type.upper_bound)
                normalized_inner_types.append(it)
            inner_types = normalized_inner_types
            current_type = current_type.copy_modified(items=normalized_inner_types)
        elif head == _SEQ_ANY:
            assert isinstance(current_type, AnyType)
            inner_type = AnyType(TypeOfAny.from_another_any, current_type)
            inner_types = [inner_type] * len(o.patterns)
        elif head == _SEQ_ITERABLE:
            assert isinstance(current_type, Instance)
            inner_type = self.chk.iterable_item_type(current_type, o)
            inner_types = [inner_type] * len(o.patterns)
        else:
            assert head == _SEQ_OTHER
            inner_type = self.chk.named_type("builtins.object")
            inner_types = [inner_type] * len(o.patterns)

        #
        # Step 4. Match inner patterns
        #
        contracted_new_inner_types: list[Type] = []
        contracted_rest_inner_types: list[Type] = []
        captures = {}  # dict[Expression, Type]

        contracted_inner_types = self.contract_starred_pattern_types(
            inner_types, star_position, required_patterns
        )
        for p, t in zip(o.patterns, contracted_inner_types):
            pattern_type = self.accept(p, t)
            typ, rest, type_map = pattern_type
            contracted_new_inner_types.append(typ)
            contracted_rest_inner_types.append(rest)
            self.update_type_map(captures, type_map)

        new_inner_types = self.expand_starred_pattern_types(
            contracted_new_inner_types, star_position, len(inner_types), unpack_index is not None
        )
        rest_inner_types = self.expand_starred_pattern_types(
            contracted_rest_inner_types, star_position, len(inner_types), unpack_index is not None
        )

        #
        # Step 5. Calculate new type
        #
        new_type: Type
        rest_type = current_type
        if isinstance(current_type, TupleType) and unpack_index is None:
            new_uninhabited, rest_tag, rest_idx, rest_mask = self._classify_sequence_tuple_result(
                new_inner_types, rest_inner_types
            )
            if new_uninhabited:
                new_type = UninhabitedType()
            else:
                new_type = TupleType(new_inner_types, current_type.partial_fallback)

            if rest_tag == _SEQ_REST_ALL:
                # All subpatterns always match, so we can apply negative narrowing
                rest_type = UninhabitedType()
            elif rest_tag == _SEQ_REST_SINGLE:
                assert rest_idx >= 0
                # Exactly one subpattern may conditionally match, the rest always match.
                # We can apply negative narrowing to this one position.
                rest_type = TupleType(
                    [
                        curr if uninh else rest
                        for curr, rest, uninh in zip(inner_types, rest_inner_types, rest_mask)
                    ],
                    current_type.partial_fallback,
                )
        elif isinstance(current_type, TupleType):
            # For variadic tuples it is too tricky to match individual items like for fixed
            # tuples, so we instead try to narrow the entire type.
            # TODO: use more precise narrowing when possible (e.g. for identical shapes).
            new_tuple_type = TupleType(new_inner_types, current_type.partial_fallback)
            new_type, _ = self.chk.conditional_types_with_intersection(
                new_tuple_type, [get_type_range(current_type)], o, default=new_tuple_type
            )
            if (
                star_position is not None
                and required_patterns <= len(inner_types) - 1
                and all(is_uninhabited(rest) for rest in rest_inner_types)
            ):
                rest_type = UninhabitedType()
        else:
            new_inner_type = UninhabitedType()
            for typ in new_inner_types:
                new_inner_type = join_types(new_inner_type, typ)
            new_type = self.construct_sequence_child(current_type, new_inner_type)
            new_type, possible_rest_type = self.chk.conditional_types_with_intersection(
                current_type, [get_type_range(new_type)], o, default=current_type
            )
            if star_position is not None and len(o.patterns) == 1:
                # Match cannot be refuted, so narrow the remaining type
                rest_type = possible_rest_type

        return PatternType(new_type, rest_type, captures)

    def contract_starred_pattern_types(
        self, types: list[Type], star_pos: int | None, num_patterns: int
    ) -> list[Type]:
        """
        Contracts a list of types in a sequence pattern depending on the position of a starred
        capture pattern.

        For example if the sequence pattern [a, *b, c] is matched against types [bool, int, str,
        bytes] the contracted types are [bool, Union[int, str], bytes].

        If star_pos in None the types are returned unchanged.
        """
        if _HAS_TYPE_KERNEL and _native_checkpattern_active:
            from mypy.subtypes import _native_subtype_resolver

            resolver = _native_subtype_resolver
            if resolver is not None:
                try:
                    result = _type_kernel.rust_contract_starred_pattern_types(
                        [_serialize_type(t) for t in types], star_pos, num_patterns, resolver
                    )
                    if result is not None:
                        decoded = _deserialize_type_list([bytes(b) for b in result])
                        if decoded is not None:
                            return decoded
                except (AssertionError, NotImplementedError):
                    pass
        unpack_index = find_unpack_in_list(types)
        if unpack_index is not None:
            # Variadic tuples require "re-shaping" to match the requested pattern.
            unpack = types[unpack_index]
            assert isinstance(unpack, UnpackType)
            unpacked = get_proper_type(unpack.type)
            # This should be guaranteed by the normalization in the caller.
            assert isinstance(unpacked, Instance) and unpacked.type.fullname == "builtins.tuple"
            if star_pos is None:
                missing = num_patterns - len(types) + 1
                new_types = types[:unpack_index]
                new_types += [unpacked.args[0]] * missing
                new_types += types[unpack_index + 1 :]
                return new_types
            prefix, middle, suffix = split_with_prefix_and_suffix(
                tuple([UnpackType(unpacked) if isinstance(t, UnpackType) else t for t in types]),
                star_pos,
                num_patterns - star_pos,
            )
            new_middle = []
            for m in middle:
                # The existing code expects the star item type, rather than the type of
                # the whole tuple "slice".
                if isinstance(m, UnpackType):
                    new_middle.append(unpacked.args[0])
                else:
                    new_middle.append(m)
            return list(prefix) + [make_simplified_union(new_middle)] + list(suffix)
        else:
            if star_pos is None:
                return types
            new_types = types[:star_pos]
            star_length = len(types) - num_patterns
            new_types.append(make_simplified_union(types[star_pos : star_pos + star_length]))
            new_types += types[star_pos + star_length :]
            return new_types

    def expand_starred_pattern_types(
        self, types: list[Type], star_pos: int | None, num_types: int, original_unpack: bool
    ) -> list[Type]:
        """Undoes the contraction done by contract_starred_pattern_types.

        For example if the sequence pattern is [a, *b, c] and types [bool, int, str] are extended
        to length 4 the result is [bool, int, int, str].
        """
        if _HAS_TYPE_KERNEL and _native_checkpattern_active:
            from mypy.subtypes import _native_subtype_resolver

            resolver = _native_subtype_resolver
            if resolver is not None:
                try:
                    result = _type_kernel.rust_expand_starred_pattern_types(
                        [_serialize_type(t) for t in types],
                        star_pos,
                        num_types,
                        original_unpack,
                        resolver,
                    )
                    if result is not None:
                        decoded = _deserialize_type_list([bytes(b) for b in result])
                        if decoded is not None:
                            return decoded
                except (AssertionError, NotImplementedError):
                    pass
        if star_pos is None:
            return types
        if original_unpack:
            # In the case where original tuple type has an unpack item, it is not practical
            # to coerce pattern type back to the original shape (and may not even be possible),
            # so we only restore the type of the star item.
            res = []
            for i, t in enumerate(types):
                if i != star_pos or is_uninhabited(t):
                    res.append(t)
                else:
                    res.append(UnpackType(self.chk.named_generic_type("builtins.tuple", [t])))
            return res
        new_types = types[:star_pos]
        star_length = num_types - len(types) + 1
        new_types += [types[star_pos]] * star_length
        new_types += types[star_pos + 1 :]

        return new_types

    def visit_starred_pattern(self, o: StarredPattern) -> PatternType:
        captures: dict[Expression, Type] = {}
        if o.capture is not None:
            list_type = self.chk.named_generic_type("builtins.list", [self.type_context[-1]])
            captures[o.capture] = list_type
        return PatternType(self.type_context[-1], UninhabitedType(), captures)

    def _classify_mapping_rest(self, current_type: ProperType, mapping: Type) -> int:
        """The `o.rest` branch of visit_mapping_pattern (#1633).

        Rust folds the `is_subtype(current, Mapping)` check and the
        `Instance` test into one tag; the shim builds the rest type from
        live TypeInfos (`map_instance_to_supertype` needs them).
        """
        if _HAS_TYPE_KERNEL and _native_checkpattern_active:
            from mypy.subtypes import _native_subtype_resolver

            resolver = _native_subtype_resolver
            if resolver is not None:
                try:
                    tag = _type_kernel.rust_classify_mapping_rest(
                        _serialize_type(current_type), _serialize_type(mapping), resolver
                    )
                    if tag is not None:
                        return int(tag)
                except (AssertionError, NotImplementedError):
                    pass
        if is_subtype(current_type, mapping) and isinstance(current_type, Instance):
            return _MAP_INSTANCE
        return _MAP_DICT_FALLBACK

    def visit_mapping_pattern(self, o: MappingPattern) -> PatternType:
        current_type = get_proper_type(self.type_context[-1])
        can_match = True
        captures: dict[Expression, Type] = {}
        for key, value in zip(o.keys, o.values):
            inner_type = self.get_mapping_item_type(o, current_type, key)
            if inner_type is None:
                can_match = False
                inner_type = self.chk.named_type("builtins.object")
            pattern_type = self.accept(value, inner_type)
            if is_uninhabited(pattern_type.type):
                can_match = False
            else:
                self.update_type_map(captures, pattern_type.captures)

        if o.rest is not None:
            mapping = self.chk.named_type("typing.Mapping")
            if self._classify_mapping_rest(current_type, mapping) == _MAP_INSTANCE:
                assert isinstance(current_type, Instance)
                mapping_inst = map_instance_to_supertype(current_type, mapping.type)
                dict_typeinfo = self.chk.lookup_typeinfo("builtins.dict")
                rest_type = Instance(dict_typeinfo, mapping_inst.args)
            else:
                object_type = self.chk.named_type("builtins.object")
                rest_type = self.chk.named_generic_type(
                    "builtins.dict", [object_type, object_type]
                )

            captures[o.rest] = rest_type

        else_type = current_type
        if can_match:
            # We can't narrow the type here, as Mapping key is invariant.
            new_type = self.type_context[-1]
            if not o.keys:
                # Match cannot be refuted, so narrow the remaining type
                mapping = self.chk.named_type("typing.Mapping")
                if_type, else_type = self.chk.conditional_types_with_intersection(
                    current_type,
                    [TypeRange(mapping, is_upper_bound=False)],
                    o,
                    default=current_type,
                )
                if not isinstance(current_type, AnyType):
                    new_type = if_type
        else:
            new_type = UninhabitedType()
        return PatternType(new_type, else_type, captures)

    def get_mapping_item_type(
        self, pattern: MappingPattern, mapping_type: Type, key: Expression
    ) -> Type | None:
        mapping_type = get_proper_type(mapping_type)
        if isinstance(mapping_type, TypedDictType):
            with self.msg.filter_errors() as local_errors:
                result: Type | None = self.chk.expr_checker.visit_typeddict_index_expr(
                    mapping_type, key
                )[0]
                has_local_errors = local_errors.has_new_errors()
            # If we can't determine the type statically fall back to treating it as a normal
            # mapping
            if has_local_errors:
                with self.msg.filter_errors() as local_errors:
                    result = self.get_simple_mapping_item_type(pattern, mapping_type, key)

                    if local_errors.has_new_errors():
                        result = None
        else:
            with self.msg.filter_errors():
                result = self.get_simple_mapping_item_type(pattern, mapping_type, key)
        return result

    def get_simple_mapping_item_type(
        self, pattern: MappingPattern, mapping_type: Type, key: Expression
    ) -> Type:
        result, _ = self.chk.expr_checker.check_method_call_by_name(
            "__getitem__", mapping_type, [key], [ARG_POS], pattern
        )
        return result

    def _is_generic_type_alias(self, type_info: object) -> bool:
        """The generic-alias gate of visit_class_pattern (#1633).

        Rust reads the live class-ref node (`TypeAlias` + `no_args`);
        a missing node is a plain non-alias.
        """
        if _HAS_TYPE_KERNEL and _native_checkpattern_active:
            try:
                decided = _type_kernel.rust_classify_class_pattern_alias_gate(type_info)
                if decided is not None:
                    return bool(decided)
            except (AssertionError, NotImplementedError):
                pass
        return isinstance(type_info, TypeAlias) and not type_info.no_args

    def _classify_class_pattern_keywords(
        self, match_arg_names: list[str | None], num_positionals: int, keyword_keys: list[str]
    ) -> list[tuple[int, int]]:
        """Positional-count and duplicate arbitration (#1633).

        Rust returns violations in emission order as (tag, keyword index)
        pairs (`_CLS_KW_TOO_MANY` carries -1); the shim applies the
        `msg.fail` calls, which stay Python-side (message text is the
        testcheck exact-match contract).
        """
        if _HAS_TYPE_KERNEL and _native_checkpattern_active:
            try:
                violations = _type_kernel.rust_classify_class_pattern_keywords(
                    match_arg_names, num_positionals, keyword_keys
                )
                if violations is not None:
                    return [(int(tag), int(idx)) for tag, idx in violations]
            except (AssertionError, NotImplementedError):
                pass
        if num_positionals > len(match_arg_names):
            return [(_CLS_KW_TOO_MANY, -1)]
        match_arg_set = {name for name in match_arg_names[:num_positionals] if name is not None}
        seen: set[str] = set()
        out: list[tuple[int, int]] = []
        for i, key in enumerate(keyword_keys):
            if key in match_arg_set:
                out.append((_CLS_KW_MATCHES_POSITIONAL, i))
            elif key in seen:
                out.append((_CLS_KW_DUPLICATE, i))
            seen.add(key)
        return out

    def visit_class_pattern(self, o: ClassPattern) -> PatternType:
        current_type = get_proper_type(self.type_context[-1])

        #
        # Check class type
        #
        type_info = o.class_ref.node
        if self._is_generic_type_alias(type_info):
            self.msg.fail(message_registry.CLASS_PATTERN_GENERIC_TYPE_ALIAS, o)
            return self.early_non_match()

        typ = self.chk.expr_checker.accept(o.class_ref)
        type_ranges = self.get_class_pattern_type_ranges(typ, o)
        if type_ranges is None:
            return self.early_non_match()
        typ = UnionType.make_union([t.item for t in type_ranges])

        new_type, rest_type = self.chk.conditional_types_with_intersection(
            current_type, type_ranges, o, default=current_type
        )
        if is_uninhabited(new_type):
            return self.early_non_match()
        # TODO: Do I need this?
        narrowed_type = narrow_declared_type(current_type, new_type)

        #
        # Convert positional to keyword patterns
        #
        keyword_pairs: list[tuple[str | None, Pattern]] = []
        # Default for the non-tuple arm and the self-match fall-through:
        # no known names, so no positional collision is possible.
        match_arg_names: list[str | None] = [None] * len(o.positionals)

        captures: dict[Expression, Type] = {}

        if len(o.positionals) != 0:
            if self.should_self_match(typ):
                if len(o.positionals) > 1:
                    self.msg.fail(message_registry.CLASS_PATTERN_TOO_MANY_POSITIONAL_ARGS, o)
                pattern_type = self.accept(o.positionals[0], narrowed_type)
                if not is_uninhabited(pattern_type.type):
                    return PatternType(
                        pattern_type.type,
                        join_types(rest_type, pattern_type.rest_type),
                        pattern_type.captures,
                    )
                captures = pattern_type.captures
            else:
                with self.msg.filter_errors() as local_errors:
                    match_args_type = analyze_member_access(
                        "__match_args__",
                        typ,
                        o,
                        is_lvalue=False,
                        is_super=False,
                        is_operator=False,
                        original_type=typ,
                        chk=self.chk,
                    )
                    has_local_errors = local_errors.has_new_errors()
                if has_local_errors:
                    self.msg.fail(
                        message_registry.MISSING_MATCH_ARGS.format(
                            typ.str_with_options(self.options)
                        ),
                        o,
                    )
                    return self.early_non_match()

                proper_match_args_type = get_proper_type(match_args_type)
                if isinstance(proper_match_args_type, TupleType):
                    match_arg_names = get_match_arg_names(proper_match_args_type)
                else:
                    match_arg_names = [None] * len(o.positionals)

                for arg_name, pos in zip(match_arg_names, o.positionals):
                    keyword_pairs.append((arg_name, pos))

        #
        # Check for duplicate patterns (violations decided natively).
        #
        for key, value in zip(o.keyword_keys, o.keyword_values):
            keyword_pairs.append((key, value))
        has_duplicates = False
        for vtag, vidx in self._classify_class_pattern_keywords(
            match_arg_names, len(o.positionals), o.keyword_keys
        ):
            if vtag == _CLS_KW_TOO_MANY:
                self.msg.fail(message_registry.CLASS_PATTERN_TOO_MANY_POSITIONAL_ARGS, o)
                return self.early_non_match()
            value = o.keyword_values[vidx]
            key = o.keyword_keys[vidx]
            if vtag == _CLS_KW_MATCHES_POSITIONAL:
                self.msg.fail(
                    message_registry.CLASS_PATTERN_KEYWORD_MATCHES_POSITIONAL.format(key), value
                )
                has_duplicates = True
            else:
                assert vtag == _CLS_KW_DUPLICATE
                self.msg.fail(
                    message_registry.CLASS_PATTERN_DUPLICATE_KEYWORD_PATTERN.format(key), value
                )
                has_duplicates = True

        if has_duplicates:
            return self.early_non_match()

        #
        # Check keyword patterns
        #
        can_match = True
        for keyword, pattern in keyword_pairs:
            key_type: Type | None = None
            with self.msg.filter_errors() as local_errors:
                if keyword is not None:
                    key_type = analyze_member_access(
                        keyword,
                        narrowed_type,
                        pattern,
                        is_lvalue=False,
                        is_super=False,
                        is_operator=False,
                        original_type=new_type,
                        chk=self.chk,
                    )
                else:
                    key_type = AnyType(TypeOfAny.from_error)
                has_local_errors = local_errors.has_new_errors()
            if has_local_errors or key_type is None:
                key_type = AnyType(TypeOfAny.from_error)
                if not (type_info and type_info.fullname == "builtins.object"):
                    self.msg.fail(
                        message_registry.CLASS_PATTERN_UNKNOWN_KEYWORD.format(
                            typ.str_with_options(self.options), keyword
                        ),
                        pattern,
                    )
                elif keyword is not None:
                    new_type = self.chk.add_any_attribute_to_type(new_type, keyword)

            inner_type, inner_rest_type, inner_captures = self.accept(pattern, key_type)
            if is_uninhabited(inner_type):
                can_match = False
            else:
                self.update_type_map(captures, inner_captures)
                if not is_uninhabited(inner_rest_type):
                    rest_type = current_type

        if not can_match:
            new_type = UninhabitedType()
        return PatternType(new_type, rest_type, captures)

    def get_class_pattern_type_ranges(self, typ: Type, o: ClassPattern) -> list[TypeRange] | None:
        # Native type_kernel seam (issue #987): Rust classifies each leaf
        # item (union recursion on the wire) into a branch tag; the TypeRange
        # construction and the fail below stay on live objects.
        if _HAS_TYPE_KERNEL and _native_checkpattern_active:
            tags = None
            try:
                tags = _type_kernel.rust_classify_class_pattern_ranges(
                    _serialize_type(typ), o.class_ref.node
                )
            except (AssertionError, NotImplementedError):
                tags = None
            if tags is not None:
                return self._class_pattern_ranges_from_tags(typ, o, tags)
        p_typ = get_proper_type(typ)

        if isinstance(p_typ, UnionType):
            type_ranges = []
            for item in p_typ.items:
                type_range = self.get_class_pattern_type_ranges(item, o)
                if type_range is not None:
                    type_ranges.extend(type_range)
            if not type_ranges:
                return None
            return type_ranges

        if isinstance(p_typ, FunctionLike) and p_typ.is_type_obj():
            typ = fill_typevars_with_any(p_typ.type_object())
            return [TypeRange(typ, is_upper_bound=False)]
        if (
            isinstance(o.class_ref.node, Var)
            and o.class_ref.node.type is not None
            and o.class_ref.node.fullname == "typing.Callable"
        ):
            # Create a `Callable[..., Any]`
            fallback = self.chk.named_type("builtins.function")
            any_type = AnyType(TypeOfAny.unannotated)
            typ = callable_with_ellipsis(any_type, ret_type=any_type, fallback=fallback)
            return [TypeRange(typ, is_upper_bound=False)]
        if isinstance(p_typ, TypeType):
            typ = p_typ.item
            return [TypeRange(p_typ.item, is_upper_bound=True)]
        if isinstance(p_typ, AnyType):
            return [TypeRange(p_typ, is_upper_bound=False)]

        self.msg.fail(
            message_registry.CLASS_PATTERN_TYPE_REQUIRED.format(
                typ.str_with_options(self.options)
            ),
            o,
        )
        return None

    def _class_pattern_leaves(self, typ: Type) -> Iterator[Type]:
        # Flatten union items in pre-order, mirroring the Rust classifier's
        # recursion; the raw (pre-get_proper_type) item is kept so the fail
        # message formats exactly what the Python body would format.
        p_typ = get_proper_type(typ)
        if isinstance(p_typ, UnionType):
            for item in p_typ.items:
                yield from self._class_pattern_leaves(item)
        else:
            yield typ

    def _class_pattern_ranges_from_tags(
        self, typ: Type, o: ClassPattern, tags: list[int]
    ) -> list[TypeRange] | None:
        out: list[TypeRange] = []
        for item, tag in zip(self._class_pattern_leaves(typ), tags):
            if tag == _CLASS_PATTERN_FAIL:
                self.msg.fail(
                    message_registry.CLASS_PATTERN_TYPE_REQUIRED.format(
                        item.str_with_options(self.options)
                    ),
                    o,
                )
                continue
            p_item = get_proper_type(item)
            if tag == _CLASS_PATTERN_TYPE_OBJ:
                # Tag TYPE_OBJ is only emitted for class-object callables
                # (FunctionLike with is_type_obj()).
                assert isinstance(p_item, FunctionLike)
                out.append(
                    TypeRange(fill_typevars_with_any(p_item.type_object()), is_upper_bound=False)
                )
            elif tag == _CLASS_PATTERN_CALLABLE_VAR:
                fallback = self.chk.named_type("builtins.function")
                any_type = AnyType(TypeOfAny.unannotated)
                out.append(
                    TypeRange(
                        callable_with_ellipsis(any_type, ret_type=any_type, fallback=fallback),
                        is_upper_bound=False,
                    )
                )
            elif tag == _CLASS_PATTERN_TYPE_TYPE:
                # Tag TYPE_TYPE is only emitted for TypeType leaves.
                assert isinstance(p_item, TypeType)
                out.append(TypeRange(p_item.item, is_upper_bound=True))
            else:
                out.append(TypeRange(p_item, is_upper_bound=False))
        if not out:
            return None
        return out

    def should_self_match(self, typ: Type) -> bool:
        typ = get_proper_type(typ)
        if isinstance(typ, TupleType):
            typ = typ.partial_fallback
        if isinstance(typ, AnyType):
            return False
        has_match_args = isinstance(typ, Instance) and typ.type.get("__match_args__") is not None
        if has_match_args:
            # Named tuples and other subtypes of builtins that define __match_args__
            # should not self match.
            return False
        if _HAS_TYPE_KERNEL and _native_checkpattern_active and self.self_match_types:
            from mypy.subtypes import _native_subtype_resolver

            resolver = _native_subtype_resolver
            if resolver is not None:
                try:
                    union = UnionType.make_union(self.self_match_types)
                    result = _type_kernel.rust_should_self_match(
                        _serialize_type(typ), has_match_args, _serialize_type(union), resolver
                    )
                    if result is not None:
                        return result
                except (AssertionError, NotImplementedError):
                    pass
        for other in self.self_match_types:
            if is_subtype(typ, other):
                return True
        return False

    def can_match_sequence(self, typ: ProperType) -> bool:
        if isinstance(typ, AnyType):
            return True
        if isinstance(typ, UnionType):
            return any(self.can_match_sequence(get_proper_type(item)) for item in typ.items)
        if _HAS_TYPE_KERNEL and _native_checkpattern_active and self.non_sequence_match_types:
            from mypy.subtypes import _native_subtype_resolver

            resolver = _native_subtype_resolver
            if resolver is not None:
                sequence = self.chk.named_type("typing.Sequence")
                try:
                    non_seq_union = UnionType.make_union(self.non_sequence_match_types)
                    result = _type_kernel.rust_can_match_sequence(
                        _serialize_type(typ),
                        _serialize_type(non_seq_union),
                        _serialize_type(sequence),
                        resolver,
                    )
                    if result is not None:
                        return result
                except (AssertionError, NotImplementedError):
                    pass
        for other in self.non_sequence_match_types:
            # We have to ignore promotions, as memoryview should match, but bytes,
            # which it can be promoted to, shouldn't
            if is_subtype(typ, other, ignore_promotions=True):
                return False
        sequence = self.chk.named_type("typing.Sequence")
        # If the static type is more general than sequence the actual type could still match
        return is_subtype(typ, sequence) or is_subtype(sequence, typ)

    def generate_types_from_names(self, type_names: list[str]) -> list[Type]:
        types: list[Type] = []
        for name in type_names:
            try:
                types.append(self.chk.named_type(name))
            except KeyError as e:
                # Some built in types are not defined in all test cases
                if not name.startswith("builtins."):
                    raise e
        return types

    def update_type_map(
        self, original_type_map: dict[Expression, Type], extra_type_map: dict[Expression, Type]
    ) -> None:
        # Unneeded if TypeMap used literal hashes instead of expressions, as
        # noted in the TODO above its definition
        already_captured = {literal_hash(expr) for expr in original_type_map}
        for expr, typ in extra_type_map.items():
            if literal_hash(expr) in already_captured:
                node = get_var(expr)
                self.msg.fail(
                    message_registry.MULTIPLE_ASSIGNMENTS_IN_PATTERN.format(node.name), expr
                )
            else:
                original_type_map[expr] = typ

    def construct_sequence_child(self, outer_type: Type, inner_type: Type) -> Type:
        """
        If outer_type is a child class of typing.Sequence returns a new instance of
        outer_type, that is a Sequence of inner_type. If outer_type is not a child class of
        typing.Sequence just returns a Sequence of inner_type

        For example:
        construct_sequence_child(List[int], str) = List[str]

        TODO: this doesn't make sense. For example if one has class S(Sequence[int], Generic[T])
        or class T(Sequence[Tuple[T, T]]), there is no way any of those can map to Sequence[str].
        """
        proper_type = get_proper_type(outer_type)
        if _HAS_TYPE_KERNEL and _native_checkpattern_active and isinstance(proper_type, Instance):
            from mypy.subtypes import _native_subtype_resolver

            resolver = _native_subtype_resolver
            if resolver is not None:
                sequence = self.chk.named_generic_type("typing.Sequence", [inner_type])
                empty_type = fill_typevars(proper_type.type)
                try:
                    result = _type_kernel.rust_construct_sequence_child(
                        _serialize_type(outer_type),
                        _serialize_type(empty_type),
                        _serialize_type(sequence),
                        resolver,
                    )
                    if result is not None:
                        decoded = _deserialize_type_list([bytes(result)])
                        if decoded is not None:
                            return decoded[0]
                except (AssertionError, NotImplementedError):
                    pass
        if isinstance(proper_type, TypeVarType):
            new_bound = self.construct_sequence_child(proper_type.upper_bound, inner_type)
            return proper_type.copy_modified(upper_bound=new_bound)
        if isinstance(proper_type, AnyType):
            return outer_type
        if isinstance(proper_type, UnionType):
            types = [
                self.construct_sequence_child(item, inner_type)
                for item in proper_type.items
                if self.can_match_sequence(get_proper_type(item))
            ]
            return make_simplified_union(types)
        sequence = self.chk.named_generic_type("typing.Sequence", [inner_type])
        if is_subtype(outer_type, self.chk.named_type("typing.Sequence")):
            if isinstance(proper_type, TupleType):
                proper_type = tuple_fallback(proper_type)
            assert isinstance(proper_type, Instance)
            empty_type = fill_typevars(proper_type.type)
            partial_type = expand_type_by_instance(empty_type, sequence)
            return expand_type_by_instance(partial_type, proper_type)
        else:
            return sequence

    def early_non_match(self) -> PatternType:
        return PatternType(UninhabitedType(), self.type_context[-1], {})


def get_match_arg_names(typ: TupleType) -> list[str | None]:
    if _HAS_TYPE_KERNEL and _native_checkpattern_active:
        from mypy.subtypes import _native_subtype_resolver

        resolver = _native_subtype_resolver
        if resolver is not None:
            try:
                result = _type_kernel.rust_get_match_arg_names(_serialize_type(typ), resolver)
                if result is not None:
                    return result
            except (AssertionError, NotImplementedError):
                pass
    args: list[str | None] = []
    for item in typ.items:
        values = try_getting_str_literals_from_type(item)
        if values is None or len(values) != 1:
            args.append(None)
        else:
            args.append(values[0])
    return args


def get_var(expr: Expression) -> Var:
    """
    Warning: this in only true for expressions captured by a match statement.
    Don't call it from anywhere else
    """
    assert isinstance(expr, NameExpr), expr
    node = expr.node
    assert isinstance(node, Var), node
    return node


def get_type_range(typ: Type) -> TypeRange:
    typ = get_proper_type(typ)
    if _HAS_TYPE_KERNEL and _native_checkpattern_active:
        try:
            result = _type_kernel.rust_get_type_range(_serialize_type(typ))
            if result is True:
                # Rust says: the bool LKV should be unwrapped for the
                # TypeRange. Rust only returns True when typ is an Instance
                # carrying a bool LKV, so the asserts below always hold.
                assert isinstance(typ, Instance)
                lkv = typ.last_known_value
                assert lkv is not None
                return TypeRange(lkv, is_upper_bound=False)
            if result is False:
                return TypeRange(typ, is_upper_bound=False)
        except (AssertionError, NotImplementedError):
            pass
    if (
        isinstance(typ, Instance)
        and typ.last_known_value
        and isinstance(typ.last_known_value.value, bool)
    ):
        typ = typ.last_known_value
    return TypeRange(typ, is_upper_bound=False)


def is_uninhabited(typ: Type) -> bool:
    if _HAS_TYPE_KERNEL and _native_checkpattern_active:
        from mypy.subtypes import _native_subtype_resolver

        resolver = _native_subtype_resolver
        if resolver is not None:
            try:
                result = _type_kernel.rust_is_uninhabited(_serialize_type(typ), resolver)
                if result is not None:
                    return result
            except (AssertionError, NotImplementedError):
                pass
    return isinstance(get_proper_type(typ), UninhabitedType)
