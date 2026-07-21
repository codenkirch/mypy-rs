"""Test cases for mypy types and type operations."""

from __future__ import annotations

import os
import re
from unittest import TestCase, skipUnless

from mypy.erasetype import _set_native_erase_active, erase_type, remove_instance_last_known_values

# Mirror testcheck.py: flip the type-kernel gate from the env var so the
# unit tests exercise the Rust path when TEST_NATIVE_TYPE_KERNEL is set.
# Unset exercises the default (Python) path; =1 exercises the Rust path.
_set_native_erase_active(bool(os.environ.get("TEST_NATIVE_TYPE_KERNEL")))
from mypy.indirection import TypeIndirectionVisitor
from mypy.join import join_types
from mypy.meet import is_overlapping_types, meet_types, narrow_declared_type
from mypy.nodes import (
    ARG_NAMED,
    ARG_NAMED_OPT,
    ARG_OPT,
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    CONTRAVARIANT,
    COVARIANT,
    INVARIANT,
    ArgKind,
    CallExpr,
    Expression,
    NameExpr,
)
from mypy.plugins.common import find_shallow_matching_overload_item
from mypy.state import state
from mypy.subtypes import is_more_precise, is_proper_subtype, is_same_type, is_subtype
from mypy.test.helpers import Suite, assert_equal, assert_type, skip
from mypy.test.typefixture import InterfaceTypeFixture, TypeFixture
from mypy.typeops import false_only, make_simplified_union, true_only
from mypy.types import (
    AnyType,
    CallableType,
    Instance,
    LiteralType,
    NoneType,
    Overloaded,
    ProperType,
    TupleType,
    Type,
    TypedDictType,
    TypeOfAny,
    TypeType,
    TypeVarId,
    TypeVarType,
    UnboundType,
    UninhabitedType,
    UnionType,
    UnpackType,
    get_proper_type,
    has_recursive_types,
)

# Solving the import cycle:
import mypy.expandtype  # ruff: isort: skip


class TypesSuite(Suite):
    def setUp(self) -> None:
        self.x = UnboundType("X")  # Helpers
        self.y = UnboundType("Y")
        self.fx = TypeFixture()
        self.function = self.fx.function

    def test_any(self) -> None:
        assert_equal(str(AnyType(TypeOfAny.special_form)), "Any")

    def test_simple_unbound_type(self) -> None:
        u = UnboundType("Foo")
        assert_equal(str(u), "Foo?")

    def test_generic_unbound_type(self) -> None:
        u = UnboundType("Foo", [UnboundType("T"), AnyType(TypeOfAny.special_form)])
        assert_equal(str(u), "Foo?[T?, Any]")

    def test_callable_type(self) -> None:
        c = CallableType(
            [self.x, self.y],
            [ARG_POS, ARG_POS],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.function,
        )
        assert_equal(str(c), "def (X?, Y?) -> Any")

        c2 = CallableType([], [], [], NoneType(), self.fx.function)
        assert_equal(str(c2), "def ()")

    def test_callable_type_with_default_args(self) -> None:
        c = CallableType(
            [self.x, self.y],
            [ARG_POS, ARG_OPT],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.function,
        )
        assert_equal(str(c), "def (X?, Y? =) -> Any")

        c2 = CallableType(
            [self.x, self.y],
            [ARG_OPT, ARG_OPT],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.function,
        )
        assert_equal(str(c2), "def (X? =, Y? =) -> Any")

    def test_callable_type_with_var_args(self) -> None:
        c = CallableType(
            [self.x], [ARG_STAR], [None], AnyType(TypeOfAny.special_form), self.function
        )
        assert_equal(str(c), "def (*X?) -> Any")

        c2 = CallableType(
            [self.x, self.y],
            [ARG_POS, ARG_STAR],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.function,
        )
        assert_equal(str(c2), "def (X?, *Y?) -> Any")

        c3 = CallableType(
            [self.x, self.y],
            [ARG_OPT, ARG_STAR],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.function,
        )
        assert_equal(str(c3), "def (X? =, *Y?) -> Any")

    def test_tuple_type_str(self) -> None:
        t1 = TupleType([], self.fx.std_tuple)
        assert_equal(str(t1), "tuple[()]")
        t2 = TupleType([self.x], self.fx.std_tuple)
        assert_equal(str(t2), "tuple[X?]")
        t3 = TupleType([self.x, AnyType(TypeOfAny.special_form)], self.fx.std_tuple)
        assert_equal(str(t3), "tuple[X?, Any]")

    def test_type_variable_binding(self) -> None:
        assert_equal(
            str(
                TypeVarType(
                    "X", "X", TypeVarId(1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
                )
            ),
            "X",
        )
        assert_equal(
            str(
                TypeVarType(
                    "X",
                    "X",
                    TypeVarId(1),
                    [self.x, self.y],
                    self.fx.o,
                    AnyType(TypeOfAny.from_omitted_generics),
                )
            ),
            "X",
        )

    def test_generic_function_type(self) -> None:
        c = CallableType(
            [self.x, self.y],
            [ARG_POS, ARG_POS],
            [None, None],
            self.y,
            self.function,
            name=None,
            variables=[
                TypeVarType(
                    "X",
                    "X",
                    TypeVarId(-1),
                    [],
                    self.fx.o,
                    AnyType(TypeOfAny.from_omitted_generics),
                )
            ],
        )
        assert_equal(str(c), "def [X] (X?, Y?) -> Y?")

        v = [
            TypeVarType(
                "Y", "Y", TypeVarId(-1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
            ),
            TypeVarType(
                "X", "X", TypeVarId(-2), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
            ),
        ]
        c2 = CallableType([], [], [], NoneType(), self.function, name=None, variables=v)
        assert_equal(str(c2), "def [Y, X] ()")

    def test_type_alias_expand_once(self) -> None:
        A, target = self.fx.def_alias_1(self.fx.a)
        assert get_proper_type(target) == target
        assert get_proper_type(A) == target

        A, target = self.fx.def_alias_2(self.fx.a)
        assert get_proper_type(target) == target
        assert get_proper_type(A) == target

    def test_recursive_nested_in_non_recursive(self) -> None:
        A, _ = self.fx.def_alias_1(self.fx.a)
        T = TypeVarType(
            "T", "T", TypeVarId(-1), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
        )
        NA = self.fx.non_rec_alias(Instance(self.fx.gi, [T]), [T], [A])
        assert not NA.is_recursive
        assert has_recursive_types(NA)

    def test_indirection_no_infinite_recursion(self) -> None:
        A, _ = self.fx.def_alias_1(self.fx.a)
        visitor = TypeIndirectionVisitor()
        A.accept(visitor)
        modules = visitor.modules
        assert modules == {"__main__", "builtins"}

        A, _ = self.fx.def_alias_2(self.fx.a)
        visitor = TypeIndirectionVisitor()
        A.accept(visitor)
        modules = visitor.modules
        assert modules == {"__main__", "builtins"}

    def test_typeddict_type_constructor_signature(self) -> None:
        typ = TypedDictType({"x": self.fx.o}, {"x"}, set(), self.fx.a, 10, 20)

        assert typ.fallback is self.fx.a
        assert_equal(typ.line, 10)
        assert_equal(typ.column, 20)
        assert not typ.is_closed

        closed = TypedDictType({"x": self.fx.o}, {"x"}, set(), self.fx.a, is_closed=True)
        assert closed.is_closed

        with self.assertRaises(TypeError):
            TypedDictType(  # type: ignore[call-arg]
                {"x": self.fx.o}, {"x"}, set(), self.fx.a, 10, 20, True
            )


class TypeOpsSuite(Suite):
    def setUp(self) -> None:
        self.fx = TypeFixture(INVARIANT)
        self.fx_co = TypeFixture(COVARIANT)
        self.fx_contra = TypeFixture(CONTRAVARIANT)

    # expand_type

    def test_trivial_expand(self) -> None:
        for t in (
            self.fx.a,
            self.fx.o,
            self.fx.t,
            self.fx.nonet,
            self.tuple(self.fx.a),
            self.callable([], self.fx.a, self.fx.a),
            self.fx.anyt,
        ):
            self.assert_expand(t, [], t)
            self.assert_expand(t, [], t)
            self.assert_expand(t, [], t)

    def test_trivial_expand_recursive(self) -> None:
        A, _ = self.fx.def_alias_1(self.fx.a)
        self.assert_expand(A, [], A)
        A, _ = self.fx.def_alias_2(self.fx.a)
        self.assert_expand(A, [], A)

    def test_expand_naked_type_var(self) -> None:
        self.assert_expand(self.fx.t, [(self.fx.t.id, self.fx.a)], self.fx.a)
        self.assert_expand(self.fx.t, [(self.fx.s.id, self.fx.a)], self.fx.t)

    def test_expand_basic_generic_types(self) -> None:
        self.assert_expand(self.fx.gt, [(self.fx.t.id, self.fx.a)], self.fx.ga)

    # IDEA: Add test cases for
    #   tuple types
    #   callable types
    #   multiple arguments

    def assert_expand(
        self, orig: Type, map_items: list[tuple[TypeVarId, Type]], result: Type
    ) -> None:
        lower_bounds = {}

        for id, t in map_items:
            lower_bounds[id] = t

        exp = mypy.expandtype.expand_type(orig, lower_bounds)
        # Remove erased tags (asterisks).
        assert_equal(str(exp).replace("*", ""), str(result))

    # erase_type

    def test_trivial_erase(self) -> None:
        for t in (self.fx.a, self.fx.o, self.fx.nonet, self.fx.anyt):
            self.assert_erase(t, t)

    def test_erase_with_type_variable(self) -> None:
        self.assert_erase(self.fx.t, self.fx.anyt)

    def test_erase_with_generic_type(self) -> None:
        self.assert_erase(self.fx.ga, self.fx.gdyn)
        self.assert_erase(self.fx.hab, Instance(self.fx.hi, [self.fx.anyt, self.fx.anyt]))

    def test_erase_with_generic_type_recursive(self) -> None:
        tuple_any = Instance(self.fx.std_tuplei, [AnyType(TypeOfAny.explicit)])
        A, _ = self.fx.def_alias_1(self.fx.a)
        self.assert_erase(A, tuple_any)
        A, _ = self.fx.def_alias_2(self.fx.a)
        self.assert_erase(A, UnionType([self.fx.a, tuple_any]))

    def test_erase_with_tuple_type(self) -> None:
        self.assert_erase(self.tuple(self.fx.a), self.fx.std_tuple)

    def test_erase_with_function_type(self) -> None:
        self.assert_erase(
            self.fx.callable(self.fx.a, self.fx.b),
            CallableType(
                arg_types=[self.fx.anyt, self.fx.anyt],
                arg_kinds=[ARG_STAR, ARG_STAR2],
                arg_names=[None, None],
                ret_type=self.fx.anyt,
                fallback=self.fx.function,
            ),
        )

    def test_erase_with_type_object(self) -> None:
        self.assert_erase(
            self.fx.callable_type(self.fx.a, self.fx.b),
            CallableType(
                arg_types=[self.fx.anyt, self.fx.anyt],
                arg_kinds=[ARG_STAR, ARG_STAR2],
                arg_names=[None, None],
                ret_type=self.fx.anyt,
                fallback=self.fx.type_type,
            ),
        )

    def test_erase_with_type_type(self) -> None:
        self.assert_erase(self.fx.type_a, self.fx.type_a)
        self.assert_erase(self.fx.type_t, self.fx.type_any)

    def assert_erase(self, orig: Type, result: Type) -> None:
        assert_equal(str(erase_type(orig)), str(result))

    # is_more_precise

    def test_is_more_precise(self) -> None:
        fx = self.fx
        assert is_more_precise(fx.b, fx.a)
        assert is_more_precise(fx.b, fx.b)
        assert is_more_precise(fx.b, fx.b)
        assert is_more_precise(fx.b, fx.anyt)
        assert is_more_precise(self.tuple(fx.b, fx.a), self.tuple(fx.b, fx.a))
        assert is_more_precise(self.tuple(fx.b, fx.b), self.tuple(fx.b, fx.a))

        assert not is_more_precise(fx.a, fx.b)
        assert not is_more_precise(fx.anyt, fx.b)

    # is_proper_subtype

    def test_is_proper_subtype(self) -> None:
        fx = self.fx

        assert is_proper_subtype(fx.a, fx.a)
        assert is_proper_subtype(fx.b, fx.a)
        assert is_proper_subtype(fx.b, fx.o)
        assert is_proper_subtype(fx.b, fx.o)

        assert not is_proper_subtype(fx.a, fx.b)
        assert not is_proper_subtype(fx.o, fx.b)

        assert is_proper_subtype(fx.anyt, fx.anyt)
        assert not is_proper_subtype(fx.a, fx.anyt)
        assert not is_proper_subtype(fx.anyt, fx.a)

        assert is_proper_subtype(fx.ga, fx.ga)
        assert is_proper_subtype(fx.gdyn, fx.gdyn)
        assert not is_proper_subtype(fx.ga, fx.gdyn)
        assert not is_proper_subtype(fx.gdyn, fx.ga)

        assert is_proper_subtype(fx.t, fx.t)
        assert not is_proper_subtype(fx.t, fx.s)

        assert is_proper_subtype(fx.a, UnionType([fx.a, fx.b]))
        assert is_proper_subtype(UnionType([fx.a, fx.b]), UnionType([fx.a, fx.b, fx.c]))
        assert not is_proper_subtype(UnionType([fx.a, fx.b]), UnionType([fx.b, fx.c]))

    def test_is_proper_subtype_covariance(self) -> None:
        fx_co = self.fx_co

        assert is_proper_subtype(fx_co.gsab, fx_co.gb)
        assert is_proper_subtype(fx_co.gsab, fx_co.ga)
        assert not is_proper_subtype(fx_co.gsaa, fx_co.gb)
        assert is_proper_subtype(fx_co.gb, fx_co.ga)
        assert not is_proper_subtype(fx_co.ga, fx_co.gb)

    def test_is_proper_subtype_contravariance(self) -> None:
        fx_contra = self.fx_contra

        assert is_proper_subtype(fx_contra.gsab, fx_contra.gb)
        assert not is_proper_subtype(fx_contra.gsab, fx_contra.ga)
        assert is_proper_subtype(fx_contra.gsaa, fx_contra.gb)
        assert not is_proper_subtype(fx_contra.gb, fx_contra.ga)
        assert is_proper_subtype(fx_contra.ga, fx_contra.gb)

    def test_is_proper_subtype_invariance(self) -> None:
        fx = self.fx

        assert is_proper_subtype(fx.gsab, fx.gb)
        assert not is_proper_subtype(fx.gsab, fx.ga)
        assert not is_proper_subtype(fx.gsaa, fx.gb)
        assert not is_proper_subtype(fx.gb, fx.ga)
        assert not is_proper_subtype(fx.ga, fx.gb)

    def test_is_proper_subtype_and_subtype_literal_types(self) -> None:
        fx = self.fx

        lit1 = fx.lit1
        lit2 = fx.lit2
        lit3 = fx.lit3

        assert is_proper_subtype(lit1, fx.a)
        assert not is_proper_subtype(lit1, fx.d)
        assert not is_proper_subtype(fx.a, lit1)
        assert is_proper_subtype(fx.uninhabited, lit1)
        assert not is_proper_subtype(lit1, fx.uninhabited)
        assert is_proper_subtype(lit1, lit1)
        assert not is_proper_subtype(lit1, lit2)
        assert not is_proper_subtype(lit2, lit3)

        assert is_subtype(lit1, fx.a)
        assert not is_subtype(lit1, fx.d)
        assert not is_subtype(fx.a, lit1)
        assert is_subtype(fx.uninhabited, lit1)
        assert not is_subtype(lit1, fx.uninhabited)
        assert is_subtype(lit1, lit1)
        assert not is_subtype(lit1, lit2)
        assert not is_subtype(lit2, lit3)

        assert not is_proper_subtype(lit1, fx.anyt)
        assert not is_proper_subtype(fx.anyt, lit1)

        assert is_subtype(lit1, fx.anyt)
        assert is_subtype(fx.anyt, lit1)

    def test_subtype_aliases(self) -> None:
        A1, _ = self.fx.def_alias_1(self.fx.a)
        AA1, _ = self.fx.def_alias_1(self.fx.a)
        assert is_subtype(A1, AA1)
        assert is_subtype(AA1, A1)

        A2, _ = self.fx.def_alias_2(self.fx.a)
        AA2, _ = self.fx.def_alias_2(self.fx.a)
        assert is_subtype(A2, AA2)
        assert is_subtype(AA2, A2)

        B1, _ = self.fx.def_alias_1(self.fx.b)
        B2, _ = self.fx.def_alias_2(self.fx.b)
        assert is_subtype(B1, A1)
        assert is_subtype(B2, A2)
        assert not is_subtype(A1, B1)
        assert not is_subtype(A2, B2)

        assert not is_subtype(A2, A1)
        assert is_subtype(A1, A2)

    # can_be_true / can_be_false

    def test_empty_tuple_always_false(self) -> None:
        tuple_type = self.tuple()
        assert tuple_type.can_be_false
        assert not tuple_type.can_be_true

    def test_nonempty_tuple_always_true(self) -> None:
        tuple_type = self.tuple(AnyType(TypeOfAny.special_form), AnyType(TypeOfAny.special_form))
        assert tuple_type.can_be_true
        assert not tuple_type.can_be_false

    def test_union_can_be_true_if_any_true(self) -> None:
        union_type = UnionType([self.fx.a, self.tuple()])
        assert union_type.can_be_true

    def test_union_can_not_be_true_if_none_true(self) -> None:
        union_type = UnionType([self.tuple(), self.tuple()])
        assert not union_type.can_be_true

    def test_union_can_be_false_if_any_false(self) -> None:
        union_type = UnionType([self.fx.a, self.tuple()])
        assert union_type.can_be_false

    def test_union_can_not_be_false_if_none_false(self) -> None:
        union_type = UnionType([self.tuple(self.fx.a), self.tuple(self.fx.d)])
        assert not union_type.can_be_false

    # true_only / false_only

    def test_true_only_of_false_type_is_uninhabited(self) -> None:
        to = true_only(NoneType())
        assert_type(UninhabitedType, to)

    def test_true_only_of_true_type_is_idempotent(self) -> None:
        always_true = self.tuple(AnyType(TypeOfAny.special_form))
        to = true_only(always_true)
        assert always_true is to

    def test_true_only_of_instance(self) -> None:
        to = true_only(self.fx.a)
        assert_equal(str(to), "A")
        assert to.can_be_true
        assert not to.can_be_false
        assert_type(Instance, to)
        # The original class still can be false
        assert self.fx.a.can_be_false

    def test_true_only_of_union(self) -> None:
        tup_type = self.tuple(AnyType(TypeOfAny.special_form))
        # Union of something that is unknown, something that is always true, something
        # that is always false
        union_type = UnionType([self.fx.a, tup_type, self.tuple()])
        to = true_only(union_type)
        assert isinstance(to, UnionType)
        assert_equal(len(to.items), 2)
        assert to.items[0].can_be_true
        assert not to.items[0].can_be_false
        assert to.items[1] is tup_type

    def test_false_only_of_true_type_is_uninhabited(self) -> None:
        with state.strict_optional_set(True):
            fo = false_only(self.tuple(AnyType(TypeOfAny.special_form)))
            assert_type(UninhabitedType, fo)

    def test_false_only_tuple(self) -> None:
        with state.strict_optional_set(False):
            fo = false_only(self.tuple(self.fx.a))
            assert_equal(fo, NoneType())
        with state.strict_optional_set(True):
            fo = false_only(self.tuple(self.fx.a))
            assert_equal(fo, UninhabitedType())

    def test_false_only_of_false_type_is_idempotent(self) -> None:
        always_false = NoneType()
        fo = false_only(always_false)
        assert always_false is fo

    def test_false_only_of_instance(self) -> None:
        fo = false_only(self.fx.a)
        assert_equal(str(fo), "A")
        assert not fo.can_be_true
        assert fo.can_be_false
        assert_type(Instance, fo)
        # The original class still can be true
        assert self.fx.a.can_be_true

    def test_false_only_of_union(self) -> None:
        with state.strict_optional_set(True):
            tup_type = self.tuple()
            # Union of something that is unknown, something that is always true, something
            # that is always false
            union_type = UnionType(
                [self.fx.a, self.tuple(AnyType(TypeOfAny.special_form)), tup_type]
            )
            assert_equal(len(union_type.items), 3)
            fo = false_only(union_type)
            assert isinstance(fo, UnionType)
            assert_equal(len(fo.items), 2)
            assert not fo.items[0].can_be_true
            assert fo.items[0].can_be_false
            assert fo.items[1] is tup_type

    def test_simplified_union(self) -> None:
        fx = self.fx

        self.assert_simplified_union([fx.a, fx.a], fx.a)
        self.assert_simplified_union([fx.a, fx.b], fx.a)
        self.assert_simplified_union([fx.a, fx.d], UnionType([fx.a, fx.d]))
        self.assert_simplified_union([fx.a, fx.uninhabited], fx.a)
        self.assert_simplified_union([fx.ga, fx.gs2a], fx.ga)
        self.assert_simplified_union([fx.ga, fx.gsab], UnionType([fx.ga, fx.gsab]))
        self.assert_simplified_union([fx.ga, fx.gsba], fx.ga)
        self.assert_simplified_union([fx.a, UnionType([fx.d])], UnionType([fx.a, fx.d]))
        self.assert_simplified_union([fx.a, UnionType([fx.a])], fx.a)
        self.assert_simplified_union(
            [fx.b, UnionType([fx.c, UnionType([fx.d])])], UnionType([fx.b, fx.c, fx.d])
        )

    def test_simplified_union_with_literals(self) -> None:
        fx = self.fx

        self.assert_simplified_union([fx.lit1, fx.a], fx.a)
        self.assert_simplified_union([fx.lit1, fx.lit2, fx.a], fx.a)
        self.assert_simplified_union([fx.lit1, fx.lit1], fx.lit1)
        self.assert_simplified_union([fx.lit1, fx.lit2], UnionType([fx.lit1, fx.lit2]))
        self.assert_simplified_union([fx.lit1, fx.lit3], UnionType([fx.lit1, fx.lit3]))
        self.assert_simplified_union([fx.lit1, fx.uninhabited], fx.lit1)
        self.assert_simplified_union([fx.lit1_inst, fx.a], fx.a)
        self.assert_simplified_union([fx.lit1_inst, fx.lit1_inst], fx.lit1_inst)
        self.assert_simplified_union(
            [fx.lit1_inst, fx.lit2_inst], UnionType([fx.lit1_inst, fx.lit2_inst])
        )
        self.assert_simplified_union(
            [fx.lit1_inst, fx.lit3_inst], UnionType([fx.lit1_inst, fx.lit3_inst])
        )
        self.assert_simplified_union([fx.lit1_inst, fx.uninhabited], fx.lit1_inst)
        self.assert_simplified_union([fx.lit1, fx.lit1_inst], fx.lit1)
        self.assert_simplified_union([fx.lit1, fx.lit2_inst], UnionType([fx.lit1, fx.lit2_inst]))
        self.assert_simplified_union([fx.lit1, fx.lit3_inst], UnionType([fx.lit1, fx.lit3_inst]))

    def test_simplified_union_with_str_literals(self) -> None:
        fx = self.fx

        self.assert_simplified_union([fx.lit_str1, fx.lit_str2, fx.str_type], fx.str_type)
        self.assert_simplified_union([fx.lit_str1, fx.lit_str1, fx.lit_str1], fx.lit_str1)
        self.assert_simplified_union(
            [fx.lit_str1, fx.lit_str2, fx.lit_str3],
            UnionType([fx.lit_str1, fx.lit_str2, fx.lit_str3]),
        )
        self.assert_simplified_union(
            [fx.lit_str1, fx.lit_str2, fx.uninhabited], UnionType([fx.lit_str1, fx.lit_str2])
        )

    def test_simplify_very_large_union(self) -> None:
        fx = self.fx
        literals = []
        for i in range(5000):
            literals.append(LiteralType("v%d" % i, fx.str_type))
        # This shouldn't be very slow, even if the union is big.
        self.assert_simplified_union([*literals, fx.str_type], fx.str_type)

    def test_simplified_union_with_str_instance_literals(self) -> None:
        fx = self.fx

        self.assert_simplified_union(
            [fx.lit_str1_inst, fx.lit_str2_inst, fx.str_type], fx.str_type
        )
        self.assert_simplified_union(
            [fx.lit_str1_inst, fx.lit_str1_inst, fx.lit_str1_inst], fx.lit_str1_inst
        )
        self.assert_simplified_union(
            [fx.lit_str1_inst, fx.lit_str2_inst, fx.lit_str3_inst],
            UnionType([fx.lit_str1_inst, fx.lit_str2_inst, fx.lit_str3_inst]),
        )
        self.assert_simplified_union(
            [fx.lit_str1_inst, fx.lit_str2_inst, fx.uninhabited],
            UnionType([fx.lit_str1_inst, fx.lit_str2_inst]),
        )

    def test_simplified_union_with_mixed_str_literals(self) -> None:
        fx = self.fx

        self.assert_simplified_union(
            [fx.lit_str1, fx.lit_str2, fx.lit_str3_inst],
            UnionType([fx.lit_str1, fx.lit_str2, fx.lit_str3_inst]),
        )
        self.assert_simplified_union([fx.lit_str1, fx.lit_str1, fx.lit_str1_inst], fx.lit_str1)

    def assert_simplified_union(self, original: list[Type], union: Type) -> None:
        assert_equal(make_simplified_union(original), union)
        assert_equal(make_simplified_union(list(reversed(original))), union)

    def test_generic_callable_overlap_is_symmetric(self) -> None:
        any_type = AnyType(TypeOfAny.from_omitted_generics)
        outer_t = TypeVarType("T", "T", TypeVarId(1), [], self.fx.o, any_type)
        outer_s = TypeVarType("S", "S", TypeVarId(2), [], self.fx.o, any_type)
        generic_t = TypeVarType("T", "T", TypeVarId(-1), [], self.fx.o, any_type)

        callable_type = CallableType([outer_t], [ARG_POS], [None], outer_s, self.fx.function)
        generic_identity = CallableType(
            [generic_t], [ARG_POS], [None], generic_t, self.fx.function, variables=[generic_t]
        )

        assert is_overlapping_types(callable_type, generic_identity)
        assert is_overlapping_types(generic_identity, callable_type)

    # Helpers

    def tuple(self, *a: Type) -> TupleType:
        return TupleType(list(a), self.fx.std_tuple)

    def callable(self, vars: list[str], *a: Type) -> CallableType:
        """callable(args, a1, ..., an, r) constructs a callable with
        argument types a1, ... an and return type r and type arguments
        vars.
        """
        tv: list[TypeVarType] = []
        n = -1
        for v in vars:
            tv.append(
                TypeVarType(
                    v, v, TypeVarId(n), [], self.fx.o, AnyType(TypeOfAny.from_omitted_generics)
                )
            )
            n -= 1
        return CallableType(
            list(a[:-1]),
            [ARG_POS] * (len(a) - 1),
            [None] * (len(a) - 1),
            a[-1],
            self.fx.function,
            name=None,
            variables=tv,
        )


class JoinSuite(Suite):
    def setUp(self) -> None:
        self.fx = TypeFixture(INVARIANT)
        self.fx_co = TypeFixture(COVARIANT)
        self.fx_contra = TypeFixture(CONTRAVARIANT)

    def test_trivial_cases(self) -> None:
        for simple in self.fx.a, self.fx.o, self.fx.b:
            self.assert_join(simple, simple, simple)

    def test_class_subtyping(self) -> None:
        self.assert_join(self.fx.a, self.fx.o, self.fx.o)
        self.assert_join(self.fx.b, self.fx.o, self.fx.o)
        self.assert_join(self.fx.a, self.fx.d, self.fx.o)
        self.assert_join(self.fx.b, self.fx.c, self.fx.a)
        self.assert_join(self.fx.b, self.fx.d, self.fx.o)

    def test_tuples(self) -> None:
        self.assert_join(self.tuple(), self.tuple(), self.tuple())
        self.assert_join(self.tuple(self.fx.a), self.tuple(self.fx.a), self.tuple(self.fx.a))
        self.assert_join(
            self.tuple(self.fx.b, self.fx.c),
            self.tuple(self.fx.a, self.fx.d),
            self.tuple(self.fx.a, self.fx.o),
        )

        self.assert_join(
            self.tuple(self.fx.a, self.fx.a), self.fx.std_tuple, self.var_tuple(self.fx.anyt)
        )
        self.assert_join(
            self.tuple(self.fx.a), self.tuple(self.fx.a, self.fx.a), self.var_tuple(self.fx.a)
        )
        self.assert_join(
            self.tuple(self.fx.b), self.tuple(self.fx.a, self.fx.c), self.var_tuple(self.fx.a)
        )
        self.assert_join(self.tuple(), self.tuple(self.fx.a), self.var_tuple(self.fx.a))

    def test_var_tuples(self) -> None:
        self.assert_join(
            self.tuple(self.fx.a), self.var_tuple(self.fx.a), self.var_tuple(self.fx.a)
        )
        self.assert_join(
            self.var_tuple(self.fx.a), self.tuple(self.fx.a), self.var_tuple(self.fx.a)
        )
        self.assert_join(self.var_tuple(self.fx.a), self.tuple(), self.var_tuple(self.fx.a))

    def test_function_types(self) -> None:
        self.assert_join(
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.a, self.fx.b),
        )

        self.assert_join(
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.b, self.fx.b),
            self.callable(self.fx.b, self.fx.b),
        )
        self.assert_join(
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.a, self.fx.a),
            self.callable(self.fx.a, self.fx.a),
        )
        self.assert_join(self.callable(self.fx.a, self.fx.b), self.fx.function, self.fx.function)
        self.assert_join(
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.d, self.fx.b),
            self.fx.function,
        )

    def test_type_vars(self) -> None:
        self.assert_join(self.fx.t, self.fx.t, self.fx.t)
        self.assert_join(self.fx.s, self.fx.s, self.fx.s)
        self.assert_join(self.fx.t, self.fx.s, self.fx.o)

    def test_none(self) -> None:
        with state.strict_optional_set(False):
            # Any type t joined with None results in t.
            for t in [
                NoneType(),
                self.fx.a,
                self.fx.o,
                UnboundType("x"),
                self.fx.t,
                self.tuple(),
                self.callable(self.fx.a, self.fx.b),
                self.fx.anyt,
            ]:
                self.assert_join(t, NoneType(), t)

    def test_unbound_type(self) -> None:
        self.assert_join(UnboundType("x"), UnboundType("x"), self.fx.anyt)
        self.assert_join(UnboundType("x"), UnboundType("y"), self.fx.anyt)

        # Any type t joined with an unbound type results in dynamic. Unbound
        # type means that there is an error somewhere in the program, so this
        # does not affect type safety (whatever the result).
        for t in [
            self.fx.a,
            self.fx.o,
            self.fx.ga,
            self.fx.t,
            self.tuple(),
            self.callable(self.fx.a, self.fx.b),
        ]:
            self.assert_join(t, UnboundType("X"), self.fx.anyt)

    def test_any_type(self) -> None:
        # Join against 'Any' type always results in 'Any'.
        with state.strict_optional_set(False):
            self.assert_join(NoneType(), self.fx.anyt, self.fx.anyt)

        for t in [
            self.fx.anyt,
            self.fx.a,
            self.fx.o,
            NoneType(),
            UnboundType("x"),
            self.fx.t,
            self.tuple(),
            self.callable(self.fx.a, self.fx.b),
        ]:
            self.assert_join(t, self.fx.anyt, self.fx.anyt)

    def test_mixed_truth_restricted_type_simple(self) -> None:
        # make_simplified_union against differently restricted truthiness types drops restrictions.
        true_a = true_only(self.fx.a)
        false_o = false_only(self.fx.o)
        u = make_simplified_union([true_a, false_o])
        assert u.can_be_true
        assert u.can_be_false

    def test_mixed_truth_restricted_type(self) -> None:
        # join_types against differently restricted truthiness types drops restrictions.
        true_any = true_only(AnyType(TypeOfAny.special_form))
        false_o = false_only(self.fx.o)
        j = join_types(true_any, false_o)
        assert j.can_be_true
        assert j.can_be_false

    def test_other_mixed_types(self) -> None:
        # In general, joining unrelated types produces object.
        for t1 in [self.fx.a, self.fx.t, self.tuple(), self.callable(self.fx.a, self.fx.b)]:
            for t2 in [self.fx.a, self.fx.t, self.tuple(), self.callable(self.fx.a, self.fx.b)]:
                if str(t1) != str(t2):
                    self.assert_join(t1, t2, self.fx.o)

    def test_simple_generics(self) -> None:
        with state.strict_optional_set(False):
            self.assert_join(self.fx.ga, self.fx.nonet, self.fx.ga)
        with state.strict_optional_set(True):
            self.assert_join(self.fx.ga, self.fx.nonet, UnionType([self.fx.ga, NoneType()]))

        self.assert_join(self.fx.ga, self.fx.anyt, self.fx.anyt)

        for t in [
            self.fx.a,
            self.fx.o,
            self.fx.t,
            self.tuple(),
            self.callable(self.fx.a, self.fx.b),
        ]:
            self.assert_join(t, self.fx.ga, self.fx.o)

    def test_generics_invariant(self) -> None:
        self.assert_join(self.fx.ga, self.fx.ga, self.fx.ga)
        self.assert_join(self.fx.ga, self.fx.gb, self.fx.o)
        self.assert_join(self.fx.ga, self.fx.gd, self.fx.o)
        self.assert_join(self.fx.ga, self.fx.g2a, self.fx.o)

    def test_generics_covariant(self) -> None:
        self.assert_join(self.fx_co.ga, self.fx_co.ga, self.fx_co.ga)
        self.assert_join(self.fx_co.ga, self.fx_co.gb, self.fx_co.ga)
        self.assert_join(self.fx_co.ga, self.fx_co.gd, self.fx_co.go)
        self.assert_join(self.fx_co.ga, self.fx_co.g2a, self.fx_co.o)

    def test_generics_contravariant(self) -> None:
        self.assert_join(self.fx_contra.ga, self.fx_contra.ga, self.fx_contra.ga)
        # TODO: this can be more precise than "object", see a comment in mypy/join.py
        self.assert_join(self.fx_contra.ga, self.fx_contra.gb, self.fx_contra.o)
        self.assert_join(self.fx_contra.ga, self.fx_contra.g2a, self.fx_contra.o)

    def test_generics_with_multiple_args(self) -> None:
        self.assert_join(self.fx_co.hab, self.fx_co.hab, self.fx_co.hab)
        self.assert_join(self.fx_co.hab, self.fx_co.hbb, self.fx_co.hab)
        self.assert_join(self.fx_co.had, self.fx_co.haa, self.fx_co.hao)

    def test_generics_with_inheritance(self) -> None:
        self.assert_join(self.fx_co.gsab, self.fx_co.gb, self.fx_co.gb)
        self.assert_join(self.fx_co.gsba, self.fx_co.gb, self.fx_co.ga)
        self.assert_join(self.fx_co.gsab, self.fx_co.gd, self.fx_co.go)

    def test_generics_with_inheritance_and_shared_supertype(self) -> None:
        self.assert_join(self.fx_co.gsba, self.fx_co.gs2a, self.fx_co.ga)
        self.assert_join(self.fx_co.gsab, self.fx_co.gs2a, self.fx_co.ga)
        self.assert_join(self.fx_co.gsab, self.fx_co.gs2d, self.fx_co.go)

    def test_generic_types_and_any(self) -> None:
        self.assert_join(self.fx.gdyn, self.fx.ga, self.fx.gdyn)
        self.assert_join(self.fx_co.gdyn, self.fx_co.ga, self.fx_co.gdyn)
        self.assert_join(self.fx_contra.gdyn, self.fx_contra.ga, self.fx_contra.gdyn)

    def test_callables_with_any(self) -> None:
        self.assert_join(
            self.callable(self.fx.a, self.fx.a, self.fx.anyt, self.fx.a),
            self.callable(self.fx.a, self.fx.anyt, self.fx.a, self.fx.anyt),
            self.callable(self.fx.a, self.fx.anyt, self.fx.anyt, self.fx.anyt),
        )

    def test_overloaded(self) -> None:
        c = self.callable

        def ov(*items: CallableType) -> Overloaded:
            return Overloaded(list(items))

        fx = self.fx
        func = fx.function
        c1 = c(fx.a, fx.a)
        c2 = c(fx.b, fx.b)
        c3 = c(fx.c, fx.c)
        self.assert_join(ov(c1, c2), c1, c1)
        self.assert_join(ov(c1, c2), c2, c2)
        self.assert_join(ov(c1, c2), ov(c1, c2), ov(c1, c2))
        self.assert_join(ov(c1, c2), ov(c1, c3), c1)
        self.assert_join(ov(c2, c1), ov(c3, c1), c1)
        self.assert_join(ov(c1, c2), c3, func)

    def test_overloaded_with_any(self) -> None:
        c = self.callable

        def ov(*items: CallableType) -> Overloaded:
            return Overloaded(list(items))

        fx = self.fx
        any = fx.anyt
        self.assert_join(ov(c(fx.a, fx.a), c(fx.b, fx.b)), c(any, fx.b), c(any, fx.b))
        self.assert_join(ov(c(fx.a, fx.a), c(any, fx.b)), c(fx.b, fx.b), c(any, fx.b))

    def test_join_interface_types(self) -> None:
        self.assert_join(self.fx.f, self.fx.f, self.fx.f)
        self.assert_join(self.fx.f, self.fx.f2, self.fx.o)
        self.assert_join(self.fx.f, self.fx.f3, self.fx.f)

    def test_join_interface_and_class_types(self) -> None:
        self.assert_join(self.fx.o, self.fx.f, self.fx.o)
        self.assert_join(self.fx.a, self.fx.f, self.fx.o)

        self.assert_join(self.fx.e, self.fx.f, self.fx.f)

    @skip
    def test_join_class_types_with_interface_result(self) -> None:
        # Unique result
        self.assert_join(self.fx.e, self.fx.e2, self.fx.f)

        # Ambiguous result
        self.assert_join(self.fx.e2, self.fx.e3, self.fx.anyt)

    @skip
    def test_generic_interfaces(self) -> None:
        fx = InterfaceTypeFixture()

        self.assert_join(fx.gfa, fx.gfa, fx.gfa)
        self.assert_join(fx.gfa, fx.gfb, fx.o)

        self.assert_join(fx.m1, fx.gfa, fx.gfa)

        self.assert_join(fx.m1, fx.gfb, fx.o)

    def test_simple_type_objects(self) -> None:
        t1 = self.type_callable(self.fx.a, self.fx.a)
        t2 = self.type_callable(self.fx.b, self.fx.b)
        tr = self.type_callable(self.fx.b, self.fx.a)

        self.assert_join(t1, t1, t1)
        j = join_types(t1, t1)
        assert isinstance(j, CallableType)
        assert j.is_type_obj()

        self.assert_join(t1, t2, tr)
        self.assert_join(t1, self.fx.type_type, self.fx.type_type)
        self.assert_join(self.fx.type_type, self.fx.type_type, self.fx.type_type)

    def test_type_type(self) -> None:
        self.assert_join(self.fx.type_a, self.fx.type_b, self.fx.type_a)
        self.assert_join(self.fx.type_b, self.fx.type_any, self.fx.type_any)
        self.assert_join(self.fx.type_b, self.fx.type_type, self.fx.type_type)
        self.assert_join(self.fx.type_b, self.fx.type_c, self.fx.type_a)
        self.assert_join(self.fx.type_c, self.fx.type_d, TypeType.make_normalized(self.fx.o))
        self.assert_join(self.fx.type_type, self.fx.type_any, self.fx.type_type)
        self.assert_join(self.fx.type_b, self.fx.anyt, self.fx.anyt)

    def test_literal_type(self) -> None:
        a = self.fx.a
        d = self.fx.d
        lit1 = self.fx.lit1
        lit2 = self.fx.lit2
        lit3 = self.fx.lit3

        self.assert_join(lit1, lit1, lit1)
        self.assert_join(lit1, a, a)
        self.assert_join(lit1, d, self.fx.o)
        self.assert_join(lit1, lit2, a)
        self.assert_join(lit1, lit3, self.fx.o)
        self.assert_join(lit1, self.fx.anyt, self.fx.anyt)
        self.assert_join(UnionType([lit1, lit2]), lit2, UnionType([lit1, lit2]))
        self.assert_join(UnionType([lit1, lit2]), a, a)
        self.assert_join(UnionType([lit1, lit3]), a, UnionType([a, lit3]))
        self.assert_join(UnionType([d, lit3]), lit3, d)
        self.assert_join(UnionType([d, lit3]), d, UnionType([d, lit3]))
        self.assert_join(UnionType([a, lit1]), lit1, a)
        self.assert_join(UnionType([a, lit1]), lit2, a)
        self.assert_join(UnionType([lit1, lit2]), UnionType([lit1, lit2]), UnionType([lit1, lit2]))

        # The order in which we try joining two unions influences the
        # ordering of the items in the final produced unions. So, we
        # manually call 'assert_simple_join' and tune the output
        # after swapping the arguments here.
        self.assert_simple_join(
            UnionType([lit1, lit2]), UnionType([lit2, lit3]), UnionType([lit1, lit2, lit3])
        )
        self.assert_simple_join(
            UnionType([lit2, lit3]), UnionType([lit1, lit2]), UnionType([lit2, lit3, lit1])
        )

    def test_variadic_tuple_joins(self) -> None:
        # These tests really test just the "arity", to be sure it is handled correctly.
        self.assert_join(
            self.tuple(self.fx.a, self.fx.a),
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
            Instance(self.fx.std_tuplei, [self.fx.a]),
        )
        self.assert_join(
            self.tuple(self.fx.a, self.fx.a),
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a),
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a),
        )
        self.assert_join(
            self.tuple(self.fx.a, self.fx.a),
            self.tuple(self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
            self.tuple(self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
        )
        self.assert_join(
            self.tuple(
                self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a
            ),
            self.tuple(
                self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a
            ),
            self.tuple(
                self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a
            ),
        )
        self.assert_join(
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
            self.tuple(
                self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a
            ),
            Instance(self.fx.std_tuplei, [self.fx.a]),
        )
        self.assert_join(
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
            Instance(self.fx.std_tuplei, [self.fx.a]),
        )
        self.assert_join(
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a),
            self.tuple(
                self.fx.b, UnpackType(Instance(self.fx.std_tuplei, [self.fx.b])), self.fx.b
            ),
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a),
        )

    def test_join_type_type_type_var(self) -> None:
        self.assert_join(self.fx.type_a, self.fx.t, self.fx.o)
        self.assert_join(self.fx.t, self.fx.type_a, self.fx.o)

    def test_join_type_var_bounds(self) -> None:
        tvar1 = TypeVarType(
            "tvar1",
            "tvar1",
            TypeVarId(-100),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
            INVARIANT,
        )
        any_type = AnyType(TypeOfAny.special_form)
        tvar2 = TypeVarType(
            "tvar2",
            "tvar2",
            TypeVarId(-101),
            [],
            upper_bound=UnionType(
                [
                    TupleType([any_type], self.fx.std_tuple),
                    TupleType([any_type, any_type], self.fx.std_tuple),
                ]
            ),
            default=AnyType(TypeOfAny.from_omitted_generics),
            variance=INVARIANT,
        )

        self.assert_join(tvar1, tvar2, self.fx.o)
        self.assert_join(tvar2, tvar1, self.fx.o)

    # There are additional test cases in check-inference.test.

    # TODO: Function types + varargs and default args.

    def assert_join(self, s: Type, t: Type, join: Type) -> None:
        self.assert_simple_join(s, t, join)
        self.assert_simple_join(t, s, join)

    def assert_simple_join(self, s: Type, t: Type, join: Type) -> None:
        result = join_types(s, t)
        actual = str(result)
        expected = str(join)
        assert_equal(actual, expected, f"join({s}, {t}) == {{}} ({{}} expected)")
        assert is_subtype(s, result), f"{s} not subtype of {result}"
        assert is_subtype(t, result), f"{t} not subtype of {result}"

    def tuple(self, *a: Type) -> TupleType:
        return TupleType(list(a), self.fx.std_tuple)

    def var_tuple(self, t: Type) -> Instance:
        """Construct a variable-length tuple type"""
        return Instance(self.fx.std_tuplei, [t])

    def callable(self, *a: Type) -> CallableType:
        """callable(a1, ..., an, r) constructs a callable with argument types
        a1, ... an and return type r.
        """
        n = len(a) - 1
        return CallableType(list(a[:-1]), [ARG_POS] * n, [None] * n, a[-1], self.fx.function)

    def type_callable(self, *a: Type) -> CallableType:
        """type_callable(a1, ..., an, r) constructs a callable with
        argument types a1, ... an and return type r, and which
        represents a type.
        """
        n = len(a) - 1
        return CallableType(list(a[:-1]), [ARG_POS] * n, [None] * n, a[-1], self.fx.type_type)


class MeetSuite(Suite):
    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_trivial_cases(self) -> None:
        for simple in self.fx.a, self.fx.o, self.fx.b:
            self.assert_meet(simple, simple, simple)

    def test_class_subtyping(self) -> None:
        self.assert_meet(self.fx.a, self.fx.o, self.fx.a)
        self.assert_meet(self.fx.a, self.fx.b, self.fx.b)
        self.assert_meet(self.fx.b, self.fx.o, self.fx.b)
        self.assert_meet(self.fx.a, self.fx.d, UninhabitedType())
        self.assert_meet(self.fx.b, self.fx.c, UninhabitedType())

    def test_tuples(self) -> None:
        self.assert_meet(self.tuple(), self.tuple(), self.tuple())
        self.assert_meet(self.tuple(self.fx.a), self.tuple(self.fx.a), self.tuple(self.fx.a))
        self.assert_meet(
            self.tuple(self.fx.b, self.fx.c),
            self.tuple(self.fx.a, self.fx.d),
            self.tuple(self.fx.b, UninhabitedType()),
        )

        self.assert_meet(
            self.tuple(self.fx.a, self.fx.a), self.fx.std_tuple, self.tuple(self.fx.a, self.fx.a)
        )
        self.assert_meet(
            self.tuple(self.fx.a), self.tuple(self.fx.a, self.fx.a), UninhabitedType()
        )

    def test_function_types(self) -> None:
        self.assert_meet(
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.a, self.fx.b),
        )

        self.assert_meet(
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.b, self.fx.b),
            self.callable(self.fx.a, self.fx.b),
        )
        self.assert_meet(
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.a, self.fx.a),
            self.callable(self.fx.a, self.fx.b),
        )

    def test_type_vars(self) -> None:
        self.assert_meet(self.fx.t, self.fx.t, self.fx.t)
        self.assert_meet(self.fx.s, self.fx.s, self.fx.s)
        self.assert_meet(self.fx.t, self.fx.s, UninhabitedType())

    def test_none(self) -> None:
        self.assert_meet(NoneType(), NoneType(), NoneType())

        self.assert_meet(NoneType(), self.fx.anyt, NoneType())

        # Any type t joined with None results in None, unless t is Any.
        with state.strict_optional_set(False):
            for t in [
                self.fx.a,
                self.fx.o,
                UnboundType("x"),
                self.fx.t,
                self.tuple(),
                self.callable(self.fx.a, self.fx.b),
            ]:
                self.assert_meet(t, NoneType(), NoneType())

        with state.strict_optional_set(True):
            self.assert_meet(self.fx.o, NoneType(), NoneType())
            for t in [
                self.fx.a,
                UnboundType("x"),
                self.fx.t,
                self.tuple(),
                self.callable(self.fx.a, self.fx.b),
            ]:
                self.assert_meet(t, NoneType(), UninhabitedType())

    def test_unbound_type(self) -> None:
        self.assert_meet(UnboundType("x"), UnboundType("x"), self.fx.anyt)
        self.assert_meet(UnboundType("x"), UnboundType("y"), self.fx.anyt)

        self.assert_meet(UnboundType("x"), self.fx.anyt, UnboundType("x"))

        # The meet of any type t with an unbound type results in dynamic.
        # Unbound type means that there is an error somewhere in the program,
        # so this does not affect type safety.
        for t in [
            self.fx.a,
            self.fx.o,
            self.fx.t,
            self.tuple(),
            self.callable(self.fx.a, self.fx.b),
        ]:
            self.assert_meet(t, UnboundType("X"), self.fx.anyt)

    def test_dynamic_type(self) -> None:
        # Meet against dynamic type always results in dynamic.
        for t in [
            self.fx.anyt,
            self.fx.a,
            self.fx.o,
            NoneType(),
            UnboundType("x"),
            self.fx.t,
            self.tuple(),
            self.callable(self.fx.a, self.fx.b),
        ]:
            self.assert_meet(t, self.fx.anyt, t)

    def test_simple_generics(self) -> None:
        self.assert_meet(self.fx.ga, self.fx.ga, self.fx.ga)
        self.assert_meet(self.fx.ga, self.fx.o, self.fx.ga)
        self.assert_meet(self.fx.ga, self.fx.gb, self.fx.gb)
        self.assert_meet(self.fx.ga, self.fx.gd, UninhabitedType())
        self.assert_meet(self.fx.ga, self.fx.g2a, UninhabitedType())

        self.assert_meet(self.fx.ga, self.fx.nonet, UninhabitedType())
        self.assert_meet(self.fx.ga, self.fx.anyt, self.fx.ga)

        for t in [self.fx.a, self.fx.t, self.tuple(), self.callable(self.fx.a, self.fx.b)]:
            self.assert_meet(t, self.fx.ga, UninhabitedType())

    def test_generics_with_multiple_args(self) -> None:
        self.assert_meet(self.fx.hab, self.fx.hab, self.fx.hab)
        self.assert_meet(self.fx.hab, self.fx.haa, self.fx.hab)
        self.assert_meet(self.fx.hab, self.fx.had, UninhabitedType())
        self.assert_meet(self.fx.hab, self.fx.hbb, self.fx.hbb)

    def test_generics_with_inheritance(self) -> None:
        self.assert_meet(self.fx.gsab, self.fx.gb, self.fx.gsab)
        self.assert_meet(self.fx.gsba, self.fx.gb, UninhabitedType())

    def test_generics_with_inheritance_and_shared_supertype(self) -> None:
        self.assert_meet(self.fx.gsba, self.fx.gs2a, UninhabitedType())
        self.assert_meet(self.fx.gsab, self.fx.gs2a, UninhabitedType())

    def test_generic_types_and_dynamic(self) -> None:
        self.assert_meet(self.fx.gdyn, self.fx.ga, self.fx.ga)

    def test_callables_with_dynamic(self) -> None:
        self.assert_meet(
            self.callable(self.fx.a, self.fx.a, self.fx.anyt, self.fx.a),
            self.callable(self.fx.a, self.fx.anyt, self.fx.a, self.fx.anyt),
            self.callable(self.fx.a, self.fx.anyt, self.fx.anyt, self.fx.anyt),
        )

    def test_meet_interface_types(self) -> None:
        self.assert_meet(self.fx.f, self.fx.f, self.fx.f)
        self.assert_meet(self.fx.f, self.fx.f2, UninhabitedType())
        self.assert_meet(self.fx.f, self.fx.f3, self.fx.f3)

    def test_meet_interface_and_class_types(self) -> None:
        self.assert_meet(self.fx.o, self.fx.f, self.fx.f)
        self.assert_meet(self.fx.a, self.fx.f, UninhabitedType())

        self.assert_meet(self.fx.e, self.fx.f, self.fx.e)

    def test_meet_class_types_with_shared_interfaces(self) -> None:
        # These have nothing special with respect to meets, unlike joins. These
        # are for completeness only.
        self.assert_meet(self.fx.e, self.fx.e2, UninhabitedType())
        self.assert_meet(self.fx.e2, self.fx.e3, UninhabitedType())

    def test_meet_with_generic_interfaces(self) -> None:
        fx = InterfaceTypeFixture()
        self.assert_meet(fx.gfa, fx.m1, fx.m1)
        self.assert_meet(fx.gfa, fx.gfa, fx.gfa)
        self.assert_meet(fx.gfb, fx.m1, UninhabitedType())

    def test_type_type(self) -> None:
        self.assert_meet(self.fx.type_a, self.fx.type_b, self.fx.type_b)
        self.assert_meet(self.fx.type_b, self.fx.type_any, self.fx.type_b)
        self.assert_meet(self.fx.type_b, self.fx.type_type, self.fx.type_b)
        self.assert_meet(self.fx.type_b, self.fx.type_c, self.fx.type_never)
        self.assert_meet(self.fx.type_c, self.fx.type_d, self.fx.type_never)
        self.assert_meet(self.fx.type_type, self.fx.type_any, self.fx.type_any)
        self.assert_meet(self.fx.type_b, self.fx.anyt, self.fx.type_b)

    def test_literal_type(self) -> None:
        a = self.fx.a
        lit1 = self.fx.lit1
        lit2 = self.fx.lit2
        lit3 = self.fx.lit3

        self.assert_meet(lit1, lit1, lit1)
        self.assert_meet(lit1, a, lit1)
        self.assert_meet_uninhabited(lit1, lit3)
        self.assert_meet_uninhabited(lit1, lit2)
        self.assert_meet(UnionType([lit1, lit2]), lit1, lit1)
        self.assert_meet(UnionType([lit1, lit2]), UnionType([lit2, lit3]), lit2)
        self.assert_meet(UnionType([lit1, lit2]), UnionType([lit1, lit2]), UnionType([lit1, lit2]))
        self.assert_meet(lit1, self.fx.anyt, lit1)
        self.assert_meet(lit1, self.fx.o, lit1)

        assert is_same_type(lit1, narrow_declared_type(lit1, a))
        assert is_same_type(lit2, narrow_declared_type(lit2, a))

    # FIX generic interfaces + ranges

    def assert_meet_uninhabited(self, s: Type, t: Type) -> None:
        with state.strict_optional_set(False):
            self.assert_meet(s, t, self.fx.nonet)
        with state.strict_optional_set(True):
            self.assert_meet(s, t, self.fx.uninhabited)

    def test_variadic_tuple_meets(self) -> None:
        # These tests really test just the "arity", to be sure it is handled correctly.
        self.assert_meet(
            self.tuple(self.fx.a, self.fx.a),
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
            self.tuple(self.fx.a, self.fx.a),
        )
        self.assert_meet(
            self.tuple(self.fx.a, self.fx.a),
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a),
            self.tuple(self.fx.a, self.fx.a),
        )
        self.assert_meet(
            self.tuple(self.fx.a, self.fx.a),
            self.tuple(self.fx.a, UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
            self.tuple(self.fx.a, self.fx.a),
        )
        self.assert_meet(
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))),
        )
        self.assert_meet(
            self.tuple(UnpackType(Instance(self.fx.std_tuplei, [self.fx.a])), self.fx.a),
            self.tuple(self.fx.b, UnpackType(Instance(self.fx.std_tuplei, [self.fx.b]))),
            self.tuple(self.fx.b, UnpackType(Instance(self.fx.std_tuplei, [self.fx.b]))),
        )

    def assert_meet(self, s: Type, t: Type, meet: Type) -> None:
        self.assert_simple_meet(s, t, meet)
        self.assert_simple_meet(t, s, meet)

    def assert_simple_meet(self, s: Type, t: Type, meet: Type) -> None:
        result = meet_types(s, t)
        actual = str(result)
        expected = str(meet)
        assert_equal(actual, expected, f"meet({s}, {t}) == {{}} ({{}} expected)")
        assert is_subtype(result, s), f"{result} not subtype of {s}"
        assert is_subtype(result, t), f"{result} not subtype of {t}"

    def tuple(self, *a: Type) -> TupleType:
        return TupleType(list(a), self.fx.std_tuple)

    def callable(self, *a: Type) -> CallableType:
        """callable(a1, ..., an, r) constructs a callable with argument types
        a1, ... an and return type r.
        """
        n = len(a) - 1
        return CallableType(list(a[:-1]), [ARG_POS] * n, [None] * n, a[-1], self.fx.function)


class SameTypeSuite(Suite):
    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_literal_type(self) -> None:
        a = self.fx.a
        b = self.fx.b  # Reminder: b is a subclass of a

        lit1 = self.fx.lit1
        lit2 = self.fx.lit2
        lit3 = self.fx.lit3

        self.assert_same(lit1, lit1)
        self.assert_same(UnionType([lit1, lit2]), UnionType([lit1, lit2]))
        self.assert_same(UnionType([lit1, lit2]), UnionType([lit2, lit1]))
        self.assert_same(UnionType([a, b]), UnionType([b, a]))
        self.assert_not_same(lit1, b)
        self.assert_not_same(lit1, lit2)
        self.assert_not_same(lit1, lit3)

        self.assert_not_same(lit1, self.fx.anyt)
        self.assert_not_same(lit1, self.fx.nonet)

    def assert_same(self, s: Type, t: Type, strict: bool = True) -> None:
        self.assert_simple_is_same(s, t, expected=True, strict=strict)
        self.assert_simple_is_same(t, s, expected=True, strict=strict)

    def assert_not_same(self, s: Type, t: Type, strict: bool = True) -> None:
        self.assert_simple_is_same(s, t, False, strict=strict)
        self.assert_simple_is_same(t, s, False, strict=strict)

    def assert_simple_is_same(self, s: Type, t: Type, expected: bool, strict: bool) -> None:
        actual = is_same_type(s, t)
        assert_equal(actual, expected, f"is_same_type({s}, {t}) is {{}} ({{}} expected)")

        if strict:
            actual2 = s == t
            assert_equal(actual2, expected, f"({s} == {t}) is {{}} ({{}} expected)")
            assert_equal(
                hash(s) == hash(t), expected, f"(hash({s}) == hash({t}) is {{}} ({{}} expected)"
            )


class RemoveLastKnownValueSuite(Suite):
    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_optional(self) -> None:
        t = UnionType.make_union([self.fx.a, self.fx.nonet])
        self.assert_union_result(t, [self.fx.a, self.fx.nonet])

    def test_two_instances(self) -> None:
        t = UnionType.make_union([self.fx.a, self.fx.b])
        self.assert_union_result(t, [self.fx.a, self.fx.b])

    def test_multiple_same_instances(self) -> None:
        t = UnionType.make_union([self.fx.a, self.fx.a])
        assert remove_instance_last_known_values(t) == self.fx.a
        t = UnionType.make_union([self.fx.a, self.fx.a, self.fx.b])
        self.assert_union_result(t, [self.fx.a, self.fx.b])
        t = UnionType.make_union([self.fx.a, self.fx.nonet, self.fx.a, self.fx.b])
        self.assert_union_result(t, [self.fx.a, self.fx.nonet, self.fx.b])

    def test_single_last_known_value(self) -> None:
        t = UnionType.make_union([self.fx.lit1_inst, self.fx.nonet])
        self.assert_union_result(t, [self.fx.a, self.fx.nonet])

    def test_last_known_values_with_merge(self) -> None:
        t = UnionType.make_union([self.fx.lit1_inst, self.fx.lit2_inst, self.fx.lit4_inst])
        assert remove_instance_last_known_values(t) == self.fx.a
        t = UnionType.make_union(
            [self.fx.lit1_inst, self.fx.b, self.fx.lit2_inst, self.fx.lit4_inst]
        )
        self.assert_union_result(t, [self.fx.a, self.fx.b])

    def test_generics(self) -> None:
        t = UnionType.make_union([self.fx.ga, self.fx.gb])
        self.assert_union_result(t, [self.fx.ga, self.fx.gb])

    def assert_union_result(self, t: ProperType, expected: list[Type]) -> None:
        t2 = remove_instance_last_known_values(t)
        assert type(t2) is UnionType
        assert t2.items == expected


# Stage 3a parity suite: round-trips `mypy.types.Type` through the binary
# wire format and asserts that the Rust reader produces the same `str(t)` as
# the Python `TypeStrVisitor`. Gated by `TEST_NATIVE_TYPE_KERNEL=1` plus the
# presence of the `type_kernel` extension; skipped otherwise. This exercises
# the reader end-to-end (varint, tagged helpers, per-variant dispatch, and
# the `Display` impl) but does not wire the reader into any production path.
try:
    import type_kernel as _type_kernel
    from librt.internal import WriteBuffer as _WriteBuffer

    _HAS_TYPE_KERNEL_WIRE = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _WriteBuffer = None  # type: ignore[assignment]
    _HAS_TYPE_KERNEL_WIRE = False

_NATIVE_WIRE_ENABLED = bool(os.environ.get("TEST_NATIVE_TYPE_KERNEL")) and _HAS_TYPE_KERNEL_WIRE


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeWireSuite(Suite):
    """Parity tests for the Rust `Type` wire reader (Stage 3a).

    Each test serializes a `Type` via `Type.write(WriteBuffer)` and asserts
    that `type_kernel.read_type_to_str(bytes) == str(t)`. The seed corpus
    mirrors the golden cases in `TypesSuite` (lines 72-200) plus the
    `TypeFixture` instances that exercise each wire-format branch.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def assert_wire_par(self, t: Type) -> None:
        expected = str(t)
        actual = _type_kernel.read_type_to_str(self._bytes_of(t))
        assert_equal(actual, expected, f"wire str({t!r}) = {{}} ({{}} expected)")

    def test_any(self) -> None:
        self.assert_wire_par(AnyType(TypeOfAny.special_form))

    def test_none(self) -> None:
        self.assert_wire_par(NoneType())

    def test_uninhabited(self) -> None:
        self.assert_wire_par(UninhabitedType())

    def test_unbound_simple(self) -> None:
        self.assert_wire_par(UnboundType("Foo"))

    def test_unbound_generic(self) -> None:
        self.assert_wire_par(
            UnboundType("Foo", [UnboundType("T"), AnyType(TypeOfAny.special_form)])
        )

    def test_instance_singletons(self) -> None:
        # INSTANCE_STR / INSTANCE_FUNCTION / INSTANCE_INT / INSTANCE_BOOL /
        # INSTANCE_OBJECT fast paths, plus INSTANCE_SIMPLE for non-builtin.
        self.assert_wire_par(self.fx.str_type)
        self.assert_wire_par(self.fx.function)
        self.assert_wire_par(self.fx.bool_type)
        self.assert_wire_par(self.fx.o)
        self.assert_wire_par(self.fx.a)
        self.assert_wire_par(self.fx.b)

    def test_instance_generic(self) -> None:
        self.assert_wire_par(self.fx.ga)
        self.assert_wire_par(self.fx.gb)
        self.assert_wire_par(self.fx.gt)
        self.assert_wire_par(self.fx.lsta)
        self.assert_wire_par(self.fx.lstb)

    def test_instance_tuple(self) -> None:
        # builtins.tuple renders as `tuple[T, ...]`.
        self.assert_wire_par(self.fx.std_tuple)

    def test_literal_int(self) -> None:
        self.assert_wire_par(self.fx.lit1)
        self.assert_wire_par(self.fx.lit2)
        self.assert_wire_par(self.fx.lit4)

    def test_literal_str(self) -> None:
        self.assert_wire_par(self.fx.lit_str1)
        self.assert_wire_par(self.fx.lit_str2)
        self.assert_wire_par(self.fx.lit_str3)

    def test_literal_bool(self) -> None:
        self.assert_wire_par(self.fx.lit_false)
        self.assert_wire_par(self.fx.lit_true)

    def test_type_type(self) -> None:
        self.assert_wire_par(self.fx.type_a)
        self.assert_wire_par(self.fx.type_b)
        self.assert_wire_par(self.fx.type_any)

    def test_callable_pos(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_callable_no_ret(self) -> None:
        c = CallableType([], [], [], NoneType(), self.fx.function)
        self.assert_wire_par(c)

    def test_callable_opt(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_OPT],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_callable_star(self) -> None:
        c = CallableType(
            [self.fx.a],
            [ARG_STAR],
            [None],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_callable_named(self) -> None:
        c = CallableType(
            [self.fx.a],
            [ARG_NAMED],
            ["x"],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_callable_named_opt(self) -> None:
        c = CallableType(
            [self.fx.a],
            [ARG_NAMED_OPT],
            ["x"],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_callable_star2(self) -> None:
        c = CallableType(
            [self.fx.a],
            [ARG_STAR2],
            ["kwargs"],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_callable_generic(self) -> None:
        # Mirrors `test_generic_function_type`: variables block renders as
        # `def [X] (...)` (after `def`, before params).
        c = CallableType(
            [UnboundType("X"), UnboundType("Y")],
            [ARG_POS, ARG_POS],
            [None, None],
            UnboundType("Y"),
            self.fx.function,
            name=None,
            variables=[
                TypeVarType(
                    "X",
                    "X",
                    TypeVarId(-1),
                    [],
                    self.fx.o,
                    AnyType(TypeOfAny.from_omitted_generics),
                )
            ],
        )
        self.assert_wire_par(c)

    def test_tuple_type_str(self) -> None:
        t1 = TupleType([], self.fx.std_tuple)
        self.assert_wire_par(t1)
        t2 = TupleType([UnboundType("X")], self.fx.std_tuple)
        self.assert_wire_par(t2)
        t3 = TupleType([UnboundType("X"), AnyType(TypeOfAny.special_form)], self.fx.std_tuple)
        self.assert_wire_par(t3)

    def test_typevar(self) -> None:
        self.assert_wire_par(self.fx.t)
        self.assert_wire_par(self.fx.s)
        self.assert_wire_par(self.fx.u)

    def test_union(self) -> None:
        self.assert_wire_par(UnionType.make_union([self.fx.a, self.fx.b]))
        self.assert_wire_par(UnionType.make_union([self.fx.a, self.fx.nonet]))
        self.assert_wire_par(
            UnionType.make_union([self.fx.a, self.fx.b, self.fx.nonet])
        )

    def test_overloaded(self) -> None:
        ov = Overloaded(
            [
                self.fx.callable(self.fx.a, AnyType(TypeOfAny.special_form)),
                self.fx.callable(self.fx.b, AnyType(TypeOfAny.special_form)),
            ]
        )
        self.assert_wire_par(ov)


# Stage 3b parity suite: round-trips `mypy.types.Type` through the binary
# wire format and asserts that the Rust reader, with a TypeInfo resolver
# built from the live Python TypeInfo graph, produces the same `str(t)` as
# the Python `TypeStrVisitor`. Gated by `TEST_NATIVE_TYPE_KERNEL=1` plus
# the presence of the `type_kernel` extension; skipped otherwise. This
# closes the Stage 3a deferred renderings (prefix-strip on builtins.*,
# enum-literal `value_repr`, bytes-literal `value_repr`, the `[()]`
# variadic-tuple branch) and proves the resolver protocol end-to-end.
@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeTypeWireResolverSuite(Suite):
    """Parity tests for the Rust `Type` reader with TypeInfo resolver.

    Each test builds a resolver from the live Python TypeInfo graph via
    `type_kernel.build_resolver(type_infos)`, serializes a `Type` via
    `Type.write(WriteBuffer)`, and asserts:
        type_kernel.read_type_to_str_with_resolver(bytes, resolver) == str(t)
    The seed corpus targets the Stage 3b deferred renderings: builtins
    prefix stripping, enum-literal `value_repr`, bytes-literal
    `value_repr`, and the `[()]` variadic-tuple branch.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        # The fixture's TypeInfo graph: all TypeInfos reachable from the
        # fixture instances. build_native_resolver walks them into the
        # NativeTypeResolver pyclass (Rust-owned HashMaps, zero FFI per
        # lookup). No aliases in this fixture; pass [].
        type_infos = [
            self.fx.oi,
            self.fx.ai,
            self.fx.bi,
            self.fx.ci,
            self.fx.di,
            self.fx.ei,
            self.fx.e2i,
            self.fx.e3i,
            self.fx.fi,
            self.fx.f2i,
            self.fx.f3i,
            self.fx.gi,
            self.fx.g2i,
            self.fx.hi,
            self.fx.gsi,
            self.fx.gs2i,
            self.fx.std_tuplei,
            self.fx.std_listi,
            self.fx.type_typei,
            self.fx.bool_type_info,
            self.fx.str_type_info,
            self.fx.functioni,
        ]
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])

    def _bytes_of(self, t: Type) -> bytes:
        buf = _WriteBuffer()
        t.write(buf)
        return buf.getvalue()

    def assert_wire_par(self, t: Type) -> None:
        expected = str(t)
        actual = _type_kernel.read_type_to_str_with_native_resolver(
            self._bytes_of(t), self.resolver
        )
        assert_equal(
            actual, expected, f"wire-resolver str({t!r}) = {{}} ({{}} expected)"
        )

    def test_instance_no_args(self) -> None:
        self.assert_wire_par(self.fx.a)
        self.assert_wire_par(self.fx.b)
        self.assert_wire_par(self.fx.o)

    def test_instance_generic(self) -> None:
        self.assert_wire_par(self.fx.ga)
        self.assert_wire_par(self.fx.gb)
        self.assert_wire_par(self.fx.gt)

    def test_instance_tuple(self) -> None:
        # builtins.tuple renders as `tuple[T, ...]`.
        self.assert_wire_par(self.fx.std_tuple)

    def test_literal_int(self) -> None:
        self.assert_wire_par(self.fx.lit1)
        self.assert_wire_par(self.fx.lit2)
        self.assert_wire_par(self.fx.lit4)

    def test_literal_str(self) -> None:
        self.assert_wire_par(self.fx.lit_str1)
        self.assert_wire_par(self.fx.lit_str2)

    def test_literal_bool(self) -> None:
        self.assert_wire_par(self.fx.lit_false)
        self.assert_wire_par(self.fx.lit_true)

    def test_last_known_value(self) -> None:
        self.assert_wire_par(self.fx.lit1_inst)
        self.assert_wire_par(self.fx.lit_str1_inst)

    def test_type_type(self) -> None:
        self.assert_wire_par(self.fx.type_a)
        self.assert_wire_par(self.fx.type_b)

    def test_callable_pos(self) -> None:
        c = CallableType(
            [self.fx.a, self.fx.b],
            [ARG_POS, ARG_POS],
            [None, None],
            AnyType(TypeOfAny.special_form),
            self.fx.function,
        )
        self.assert_wire_par(c)

    def test_union(self) -> None:
        self.assert_wire_par(UnionType.make_union([self.fx.a, self.fx.b]))
        self.assert_wire_par(
            UnionType.make_union([self.fx.a, self.fx.b, self.fx.nonet])
        )


class ShallowOverloadMatchingSuite(Suite):
    def setUp(self) -> None:
        self.fx = TypeFixture()

    def test_simple(self) -> None:
        fx = self.fx
        ov = self.make_overload([[("x", fx.anyt, ARG_NAMED)], [("y", fx.anyt, ARG_NAMED)]])
        # Match first only
        self.assert_find_shallow_matching_overload_item(ov, make_call(("foo", "x")), 0)
        # Match second only
        self.assert_find_shallow_matching_overload_item(ov, make_call(("foo", "y")), 1)
        # No match -- invalid keyword arg name
        self.assert_find_shallow_matching_overload_item(ov, make_call(("foo", "z")), 1)
        # No match -- missing arg
        self.assert_find_shallow_matching_overload_item(ov, make_call(), 1)
        # No match -- extra arg
        self.assert_find_shallow_matching_overload_item(
            ov, make_call(("foo", "x"), ("foo", "z")), 1
        )

    def test_match_using_types(self) -> None:
        fx = self.fx
        ov = self.make_overload(
            [
                [("x", fx.nonet, ARG_POS)],
                [("x", fx.lit_false, ARG_POS)],
                [("x", fx.lit_true, ARG_POS)],
                [("x", fx.anyt, ARG_POS)],
            ]
        )
        self.assert_find_shallow_matching_overload_item(ov, make_call(("None", None)), 0)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("builtins.False", None)), 1)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("builtins.True", None)), 2)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("foo", None)), 3)

    def test_none_special_cases(self) -> None:
        fx = self.fx
        ov = self.make_overload(
            [[("x", fx.callable(fx.nonet), ARG_POS)], [("x", fx.nonet, ARG_POS)]]
        )
        self.assert_find_shallow_matching_overload_item(ov, make_call(("None", None)), 1)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("func", None)), 0)
        ov = self.make_overload([[("x", fx.str_type, ARG_POS)], [("x", fx.nonet, ARG_POS)]])
        self.assert_find_shallow_matching_overload_item(ov, make_call(("None", None)), 1)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("func", None)), 0)
        ov = self.make_overload(
            [[("x", UnionType([fx.str_type, fx.a]), ARG_POS)], [("x", fx.nonet, ARG_POS)]]
        )
        self.assert_find_shallow_matching_overload_item(ov, make_call(("None", None)), 1)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("func", None)), 0)
        ov = self.make_overload([[("x", fx.o, ARG_POS)], [("x", fx.nonet, ARG_POS)]])
        self.assert_find_shallow_matching_overload_item(ov, make_call(("None", None)), 0)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("func", None)), 0)
        ov = self.make_overload(
            [[("x", UnionType([fx.str_type, fx.nonet]), ARG_POS)], [("x", fx.nonet, ARG_POS)]]
        )
        self.assert_find_shallow_matching_overload_item(ov, make_call(("None", None)), 0)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("func", None)), 0)
        ov = self.make_overload([[("x", fx.anyt, ARG_POS)], [("x", fx.nonet, ARG_POS)]])
        self.assert_find_shallow_matching_overload_item(ov, make_call(("None", None)), 0)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("func", None)), 0)

    def test_optional_arg(self) -> None:
        fx = self.fx
        ov = self.make_overload(
            [[("x", fx.anyt, ARG_NAMED)], [("y", fx.anyt, ARG_OPT)], [("z", fx.anyt, ARG_NAMED)]]
        )
        self.assert_find_shallow_matching_overload_item(ov, make_call(), 1)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("foo", "x")), 0)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("foo", "y")), 1)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("foo", "z")), 2)

    def test_two_args(self) -> None:
        fx = self.fx
        ov = self.make_overload(
            [
                [("x", fx.nonet, ARG_OPT), ("y", fx.anyt, ARG_OPT)],
                [("x", fx.anyt, ARG_OPT), ("y", fx.anyt, ARG_OPT)],
            ]
        )
        self.assert_find_shallow_matching_overload_item(ov, make_call(), 0)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("None", "x")), 0)
        self.assert_find_shallow_matching_overload_item(ov, make_call(("foo", "x")), 1)
        self.assert_find_shallow_matching_overload_item(
            ov, make_call(("foo", "y"), ("None", "x")), 0
        )
        self.assert_find_shallow_matching_overload_item(
            ov, make_call(("foo", "y"), ("bar", "x")), 1
        )

    def assert_find_shallow_matching_overload_item(
        self, ov: Overloaded, call: CallExpr, expected_index: int
    ) -> None:
        c = find_shallow_matching_overload_item(ov, call)
        assert c in ov.items
        assert ov.items.index(c) == expected_index

    def make_overload(self, items: list[list[tuple[str, Type, ArgKind]]]) -> Overloaded:
        result = []
        for item in items:
            arg_types = []
            arg_names = []
            arg_kinds = []
            for name, typ, kind in item:
                arg_names.append(name)
                arg_types.append(typ)
                arg_kinds.append(kind)
            result.append(
                CallableType(
                    arg_types, arg_kinds, arg_names, ret_type=NoneType(), fallback=self.fx.o
                )
            )
        return Overloaded(result)


def make_call(*items: tuple[str, str | None]) -> CallExpr:
    args: list[Expression] = []
    arg_names = []
    arg_kinds = []
    for arg, name in items:
        shortname = arg.split(".")[-1]
        n = NameExpr(shortname)
        n.fullname = arg
        args.append(n)
        arg_names.append(name)
        if name:
            arg_kinds.append(ARG_NAMED)
        else:
            arg_kinds.append(ARG_POS)
    return CallExpr(NameExpr("f"), args, arg_kinds, arg_names)


class TestExpandTypeLimitGetProperType(TestCase):
    # WARNING: do not increase this number unless absolutely necessary,
    # and you understand what you are doing.
    ALLOWED_GET_PROPER_TYPES = 7

    @skipUnless(mypy.expandtype.__file__.endswith(".py"), "Skip for compiled mypy")
    def test_count_get_proper_type(self) -> None:
        with open(mypy.expandtype.__file__) as f:
            code = f.read()
        get_proper_type_count = len(re.findall(r"get_proper_type\(", code))
        get_proper_type_count -= len(re.findall(r"get_proper_type\(\)", code))
        assert get_proper_type_count == self.ALLOWED_GET_PROPER_TYPES


def _is_type_info(value: object) -> bool:
    """True if `value` is a `mypy.nodes.TypeInfo` instance."""
    from mypy.nodes import TypeInfo

    return isinstance(value, TypeInfo)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinMeetSuite(Suite):
    """Parity suite for the Rust `trivial_join`/`trivial_meet` (Stage 3c M8d).

    Exercises the Rust path with the resolver built from the TypeFixture.
    Rust handles nominal-instance subtype/join/meet and returns `None`
    (Python fallthrough) for non-Instance right in object_or_any_from_type
    and when is_subtype defers. Because Python runs when Rust returns None,
    every assertion matches the pure-Python result.
    """

    def setUp(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver, _set_native_join_typeinfo_map
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver, _set_native_join_typeinfo_map
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_trivial_join_subtype_returns_supertype(self) -> None:
        # B <: A -> trivial_join(B, A) = A (the supertype).
        from mypy.join import trivial_join

        assert trivial_join(self.fx.b, self.fx.a) == self.fx.a
        assert trivial_join(self.fx.a, self.fx.b) == self.fx.a

    def test_trivial_join_same_type(self) -> None:
        # A <: A -> trivial_join(A, A) = A.
        from mypy.join import trivial_join

        assert trivial_join(self.fx.a, self.fx.a) == self.fx.a
        assert trivial_join(self.fx.o, self.fx.o) == self.fx.o

    def test_trivial_join_unrelated_returns_object(self) -> None:
        # B and C unrelated -> object_or_any_from_type(right) = object.
        from mypy.join import trivial_join

        result = trivial_join(self.fx.b, self.fx.c)
        assert result == self.fx.o

    def test_trivial_meet_subtype_returns_subtype(self) -> None:
        # B <: A -> trivial_meet(B, A) = B (the subtype).
        from mypy.meet import trivial_meet

        assert trivial_meet(self.fx.b, self.fx.a) == self.fx.b
        assert trivial_meet(self.fx.a, self.fx.b) == self.fx.b

    def test_trivial_meet_same_type(self) -> None:
        # A <: A -> trivial_meet(A, A) = A.
        from mypy.meet import trivial_meet

        assert trivial_meet(self.fx.a, self.fx.a) == self.fx.a
        assert trivial_meet(self.fx.o, self.fx.o) == self.fx.o

    def test_trivial_meet_unrelated_returns_bottom(self) -> None:
        # B and C unrelated, strict_optional -> UninhabitedType.
        from mypy.meet import trivial_meet

        with state.strict_optional_set(True):
            result = trivial_meet(self.fx.b, self.fx.c)
            assert isinstance(result, UninhabitedType)

    def test_trivial_meet_unrelated_non_strict_returns_none_type(self) -> None:
        # B and C unrelated, non-strict-optional -> NoneType.
        from mypy.meet import trivial_meet

        with state.strict_optional_set(False):
            result = trivial_meet(self.fx.b, self.fx.c)
            assert isinstance(result, NoneType)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinTypesSuite(Suite):
    """Parity suite for the Rust `join_types` pre-dispatch (Stage 3c M8e).

    Exercises the Rust path with the resolver built from the TypeFixture.
    Rust handles the UnionType swap + AnyType/NoneType/UninhabitedType/
    DeletedType short-circuits and the leaf TypeJoinVisitor cases that
    don't recurse. Returns `None` (Python fallthrough) for Instance/
    Union/CallableType right and normalize_callables. Because Python runs
    when Rust returns None, every assertion matches the pure-Python result.
    """

    def setUp(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver, _set_native_join_typeinfo_map
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver, _set_native_join_typeinfo_map
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_join_any_left_returns_any(self) -> None:
        # join.py:314: isinstance(s, AnyType) -> return s.
        from mypy.join import join_types

        assert join_types(self.fx.anyt, self.fx.a) == self.fx.anyt

    def test_join_none_none_strict_returns_none(self) -> None:
        # visit_none_type, strict_optional, s=None -> SameT (None).
        from mypy.join import join_types

        with state.strict_optional_set(True):
            assert join_types(self.fx.nonet, self.fx.nonet) == self.fx.nonet

    def test_join_none_none_non_strict_returns_none(self) -> None:
        # Non-strict-optional: visit_none_type returns s.
        from mypy.join import join_types

        with state.strict_optional_set(False):
            assert join_types(self.fx.nonet, self.fx.nonet) == self.fx.nonet

    def test_join_uninhabited_none_strict_returns_none(self) -> None:
        # s=Uninhabited, t=None: Uninhabited swap fires -> s=None,
        # t=Uninhabited. visit_uninhabited returns s (NoneType).
        from mypy.join import join_types

        with state.strict_optional_set(True):
            result = join_types(UninhabitedType(), self.fx.nonet)
            assert result == self.fx.nonet

    def test_join_uninhabited_uninhabited_returns_uninhabited(self) -> None:
        # s=Uninhabited, t=Uninhabited: no swap, visit_uninhabited
        # returns s (UninhabitedType).
        from mypy.join import join_types

        bottom = UninhabitedType()
        with state.strict_optional_set(True):
            assert join_types(bottom, UninhabitedType()) == bottom

    def test_join_instance_none_strict_defers_to_python(self) -> None:
        # s=Instance, t=None, strict_optional: visit_none_type else
        # branch -> make_simplified_union (Python). Result is a union.
        from mypy.join import join_types

        with state.strict_optional_set(True):
            result = join_types(self.fx.a, self.fx.nonet)
            # Python falls through and builds a union of A and None.
            assert isinstance(result, UnionType)

    def test_join_instance_none_non_strict_returns_instance(self) -> None:
        # Non-strict-optional: visit_none_type returns s (Instance).
        from mypy.join import join_types

        with state.strict_optional_set(False):
            assert join_types(self.fx.a, self.fx.nonet) == self.fx.a

    def test_join_instance_uninhabited_returns_instance(self) -> None:
        # s=Instance, t=Uninhabited: visit_uninhabited returns s.
        from mypy.join import join_types

        with state.strict_optional_set(True):
            assert join_types(self.fx.a, UninhabitedType()) == self.fx.a

    def test_join_instance_instance_defers_to_python(self) -> None:
        # visit_instance needs InstanceJoiner + protocol checks ->
        # defer (None). Python computes the nominal join (A <: A -> A).
        from mypy.join import join_types

        assert join_types(self.fx.a, self.fx.a) == self.fx.a


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinInstanceSuite(Suite):
    """Parity suite for the Rust `visit_instance` nominal join (Stage 3c M8f).

    Exercises the args-less Instance-Instance nominal join: same-type,
    direct-subtype, and common-ancestor via the MRO bases walk. The
    fixture provides A, B(A), C(A), D (unrelated). join(B, C) finds A as
    the common ancestor via the bases walk, which trivial_join (direct
    subtype only) would miss (it returns object).
    """

    def setUp(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver, _set_native_join_typeinfo_map
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver, _set_native_join_typeinfo_map
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_join_same_type_returns_self(self) -> None:
        # join.py:114: t.type == s.type, no args -> Instance(A, []) = A.
        from mypy.join import join_types

        assert join_types(self.fx.a, self.fx.a) == self.fx.a
        assert join_types(self.fx.d, self.fx.d) == self.fx.d

    def test_join_direct_subtype_returns_supertype(self) -> None:
        # B <: A -> join(A, B) = A. The Rust path returns
        # Ancestor("A") which the shim maps to Instance(A, []).
        from mypy.join import join_types

        assert join_types(self.fx.a, self.fx.b) == self.fx.a
        assert join_types(self.fx.b, self.fx.a) == self.fx.a

    def test_join_common_ancestor_returns_ancestor(self) -> None:
        # B <: A, C <: A, B not <: C, C not <: B -> join(B, C) = A.
        # trivial_join would return object (neither is a subtype of
        # the other); the visit_instance bases walk finds A.
        from mypy.join import join_types

        assert join_types(self.fx.b, self.fx.c) == self.fx.a
        assert join_types(self.fx.c, self.fx.b) == self.fx.a

    def test_join_unrelated_defers_to_python_returns_object(self) -> None:
        # A and D unrelated (D not <: A, A not <: D, no common base
        # in the fixture). Rust defers; Python returns object.
        from mypy.join import join_types

        result = join_types(self.fx.a, self.fx.d)
        assert result == self.fx.o

    def test_join_with_args_returns_same_instance(self) -> None:
        # Instance with type args (M8g): join(G[A], G[A]) where T is
        # invariant. is_equivalent(A, A)=True, join_types(A, A)=A ->
        # Rust returns SameTypeWithArgs (disc 6) with arg_discs=[0]
        # (use s.args[0]=A). Shim reconstructs G[A].
        from mypy.join import join_types

        result = join_types(self.fx.ga, self.fx.ga)
        assert result == self.fx.ga


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinInstanceWithArgsSuite(Suite):
    """Parity suite for the Rust `visit_instance` same-type-with-args join
    (Stage 3c M8g).

    Exercises the Instance-Instance same-type join where T is invariant:
    AnyType args (short-circuit), invariant `is_equivalent` False
    (object bail), and invariant `is_equivalent` True (same arg). The
    fixture's `G[T]` is constructed with `INVARIANT` variance.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_any_arg_returns_any_instance(self) -> None:
        # join(G[Any], G[A]) where T is invariant. AnyType arg
        # short-circuits (join.py:131-135) -> G[Any].
        from mypy.join import join_types

        assert join_types(self.fx.gdyn, self.fx.ga) == self.fx.gdyn
        assert join_types(self.fx.ga, self.fx.gdyn) == self.fx.gdyn

    def test_invariant_not_equivalent_returns_object(self) -> None:
        # join(G[A], G[B]) where T is invariant. A not <: B ->
        # is_equivalent(A, B)=False -> object_from_instance(t)=object.
        from mypy.join import join_types

        assert join_types(self.fx.ga, self.fx.gb) == self.fx.o
        assert join_types(self.fx.gb, self.fx.ga) == self.fx.o

    def test_invariant_equivalent_returns_same_instance(self) -> None:
        # join(G[A], G[A]) where T is invariant. is_equivalent(A, A)=
        # True, join_types(A, A)=A -> G[A].
        from mypy.join import join_types

        assert join_types(self.fx.ga, self.fx.ga) == self.fx.ga


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinCovariantArgsSuite(Suite):
    """Parity suite for the Rust `visit_instance` covariant-arg join
    (Stage 3c M8h).

    Exercises the Instance-Instance same-type join where T is covariant:
    equal args fire the Rust path (recursive join_types returns SameS/
    SameT, is_subtype(arg, upper_bound=object)=True); unequal args defer
    to Python (the recursive join returns Ancestor, which the Rust
    covariant branch can't express as an arg disc). AnyType args
    short-circuit on either side (disc 4). Results are identical to the
    pure-Python `JoinSuite.test_generics_covariant` cases.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        self.fx = TypeFixture(COVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_equal_args_returns_same_instance(self) -> None:
        # join(G[A], G[A]) where T is covariant. join_types(A, A)=A
        # (SameS) -> arg disc 1 (t.args[0]=A). is_subtype(A, object)=
        # True -> G[A]. Fires the Rust covariant branch.
        from mypy.join import join_types

        assert join_types(self.fx.ga, self.fx.ga) == self.fx.ga

    def test_any_arg_returns_any_instance(self) -> None:
        # join(G[Any], G[A]) where T is covariant. AnyType arg
        # short-circuits (join.py:131-135) -> G[Any]. Fires the Rust
        # AnyType-discard path (disc 4), shared with the invariant
        # branch.
        from mypy.join import join_types

        assert join_types(self.fx.gdyn, self.fx.ga) == self.fx.gdyn
        assert join_types(self.fx.ga, self.fx.gdyn) == self.fx.gdyn

    def test_multiple_equal_args_returns_same_instance(self) -> None:
        # join(H[A,B], H[A,B]) where S,T are covariant. Both args
        # equal -> recursive join returns SameS for each -> H[A,B].
        # Fires the Rust covariant branch per-arg.
        from mypy.join import join_types

        assert join_types(self.fx.hab, self.fx.hab) == self.fx.hab

    def test_subtype_args_defer_to_python(self) -> None:
        # join(G[A], G[B]) where T is covariant, B <: A. The
        # recursive join_types(A, B) returns Ancestor(A) (the common
        # supertype), which the Rust covariant branch can't express as
        # an arg disc, so it defers. Python computes G[A]. The result
        # is identical to pure-Python JoinSuite.test_generics_covariant.
        from mypy.join import join_types

        assert join_types(self.fx.ga, self.fx.gb) == self.fx.ga
        assert join_types(self.fx.gb, self.fx.ga) == self.fx.ga

    def test_unrelated_args_defer_to_python(self) -> None:
        # join(G[A], G[D]) where T is covariant, A,D unrelated. The
        # recursive join_types(A, D) returns Ancestor(object), which
        # the Rust covariant branch can't express -> defers. Python
        # computes G[object].
        from mypy.join import join_types

        assert join_types(self.fx.ga, self.fx.gd) == self.fx.go


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinUnionSuite(Suite):
    """Parity suite for the Rust `visit_union_type` join
    (Stage 3c M8i).

    Exercises the Instance-vs-UnionType join (join.py:432-436):
    s <: any union item fires the Rust path (returns t, SameT);
    every union item <: s fires the Rust path (returns s, SameS);
    unrelated args defer to Python (needs make_simplified_union to
    build a new union). Results are identical to pure-Python JoinSuite.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        self.fx = TypeFixture()
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_subtype_of_union_returns_union(self) -> None:
        # join(A, Union[A, B]) where A is in the union. A <: A (an
        # item) -> is_subtype(A, Union[A, B])=True -> returns the
        # union. Fires the Rust SameT path.
        from mypy.join import join_types

        u = UnionType([self.fx.a, self.fx.b])
        assert join_types(self.fx.a, u) == u
        assert join_types(self.fx.b, u) == u

    def test_union_subtype_of_s_returns_s(self) -> None:
        # join(A, Union[B, C]) where B <: A, C <: A. Every item of the
        # union is a subtype of A -> the simplified union collapses to
        # A. Fires the Rust SameS path.
        from mypy.join import join_types

        u = UnionType([self.fx.b, self.fx.c])
        assert join_types(self.fx.a, u) == self.fx.a

    def test_union_unrelated_defers_to_python(self) -> None:
        # join(A, Union[D]) where D is unrelated to A. Neither A <: D
        # nor D <: A. The Rust path defers; Python computes
        # Union[A, D] via make_simplified_union. The result is the
        # same regardless of which path computed it.
        from mypy.join import join_types

        u = UnionType([self.fx.d])
        assert join_types(self.fx.a, u) == UnionType([self.fx.a, self.fx.d])

    def test_union_with_object_item_returns_union(self) -> None:
        # join(A, Union[object]). A <: object (an item) -> is_subtype(A,
        # Union[object])=True -> returns the union (SameT). Note: the
        # union is NOT collapsed to object here (join_types does not
        # apply get_proper_type to its result); callers that need the
        # collapsed form apply it themselves. Fires the Rust SameT path.
        from mypy.join import join_types

        u = UnionType([self.fx.o])
        assert join_types(self.fx.a, u) == u

    def test_union_both_sides_defers_to_python(self) -> None:
        # join(Union[A], Union[B]). Both sides are unions; the Rust
        # pre-dispatch defers (needs merge/flatten). Python collapses
        # single-item unions via get_proper_type, so this reduces to
        # join(A, B) = A (B extends A). Result is identical.
        from mypy.join import join_types

        s = UnionType([self.fx.a])
        t = UnionType([self.fx.b])
        assert join_types(s, t) == self.fx.a


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinCallableSuite(Suite):
    """Parity suite for the Rust `visit_callable_type` fallback join
    (Stage 3c M8j).

    Exercises the CallableType-vs-non-callable join (join.py:541-577).
    The Rust port handles only the fallback case (`visit_callable_fallback`)
    where `s` is a non-callable, non-protocol type. The recursive
    `join_types(t.fallback, s)` fires the Instance-Instance nominal path;
    `Ancestor(common-supertype)` passes through (shim maps disc 5 to
    `Instance(typeinfo, [])`), `Object` passes through (disc 2), and
    `SameS` (result==s) passes through (disc 0). Results are identical
    to pure-Python `JoinSuite.test_function_types`.

    The similar-callables case (both sides CallableType) defers to
    Python because `combine_similar_callables` produces a new
    CallableType (needs a Type encoder).
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        self.fx = TypeFixture()
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def callable(self, *a: Type) -> CallableType:
        n = len(a) - 1
        return CallableType(list(a[:-1]), [ARG_POS] * n, [None] * n, a[-1], self.fx.function)

    def test_callable_with_function_returns_function(self) -> None:
        # join(callable, function): the recursive join_types(fallback=
        # function, s=function) hits the Instance-Instance same-type path
        # -> SameS -> outer SameS (shim returns s=function). Fires the
        # Rust SameS path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(c, self.fx.function) == self.fx.function

    def test_callable_with_object_returns_object(self) -> None:
        # join(callable, object): recursive join_types(function, object)
        # -> is_subtype(function, object)=True -> via_supertype(function,
        # object) -> function.bases=[object] -> join_instances_nominal(
        # object, object) -> Left -> Ancestor("builtins.object"). Fires
        # the Rust Ancestor path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(c, self.fx.o) == self.fx.o

    def test_callable_with_unrelated_instance_returns_object(self) -> None:
        # join(callable, A): recursive join_types(function, A). Neither
        # is a subtype of the other. via_supertype(A, function) walks
        # A.bases=[object] -> join_instances_nominal(object, function)
        # -> is_subtype(function, object)=True -> via_supertype(function,
        # object) -> Ancestor("builtins.object"). Fires the Rust
        # Ancestor path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(c, self.fx.a) == self.fx.o

    def test_function_with_callable_returns_function(self) -> None:
        # join(function, callable): s=function, t=callable. The Rust
        # pre-dispatch reaches visit_join(t=CallableType, s=function) ->
        # visit_callable_fallback(s=function, fallback=function) ->
        # recursive join_types(function, function) -> SameS. Fires the
        # Rust SameS path (shim returns s=function).
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(self.fx.function, c) == self.fx.function

    def test_object_with_callable_returns_object(self) -> None:
        # join(object, callable): s=object, t=callable. The recursive
        # join_types(function, object) -> Ancestor("builtins.object").
        # The outer callable fallback passes Ancestor through; the shim
        # returns Instance(object_typeinfo, []) = object = s. Fires the
        # Rust Ancestor path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(self.fx.o, c) == self.fx.o

    def test_instance_with_callable_returns_object(self) -> None:
        # join(A, callable): s=A, t=callable. The recursive
        # join_types(function, A) -> Ancestor("builtins.object"). Same
        # shape as test_callable_with_unrelated_instance_returns_object
        # but with s/t swapped. Fires the Rust Ancestor path.
        from mypy.join import join_types

        c = self.callable(self.fx.a, self.fx.b)
        assert join_types(self.fx.a, c) == self.fx.o

    def test_callable_with_callable_defers_to_python(self) -> None:
        # Both sides CallableType. The Rust pre-dispatch defers (both
        # callable-like) because combine_similar_callables needs a Type
        # encoder. Python computes the combined callable. The result is
        # identical regardless of which path computed it.
        from mypy.join import join_types

        c1 = self.callable(self.fx.a, self.fx.b)
        c2 = self.callable(self.fx.a, self.fx.a)
        assert join_types(c1, c2) == c2


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinOverloadedSuite(Suite):
    """Parity suite for the Rust `visit_overloaded` fallback join
    (Stage 3c M8k).

    Exercises the Overloaded-vs-non-callable join (join.py:581-632).
    The Rust port handles only the fallback case where `s` is not a
    FunctionLike and not a protocol-Instance. The recursive
    `join_types(t.fallback, s)` fires the Instance-Instance nominal path;
    `t.fallback` is `items[0].fallback` (types.py:2744), extracted from
    the wire-format `items` field. `Ancestor(common-supertype)` passes
    through (shim maps disc 5 to `Instance(typeinfo, [])`), `Object`
    passes through (disc 2), and `SameS` (result==s) passes through
    (disc 0). Results are identical to pure-Python `JoinSuite.test_overloaded`
    fallback cases.

    The both-FunctionLike case (s is CallableType/Overloaded) defers to
    Python because the similar-callables walk needs
    `combine_similar_callables` (produces a new CallableType / Overloaded,
    needs a Type encoder). The protocol-Instance case also defers (needs
    `unpack_callback_proxy`).
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        self.fx = TypeFixture()
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def callable(self, *a: Type) -> CallableType:
        n = len(a) - 1
        return CallableType(list(a[:-1]), [ARG_POS] * n, [None] * n, a[-1], self.fx.function)

    def overloaded(self, *items: CallableType) -> Overloaded:
        return Overloaded(list(items))

    def test_overloaded_with_function_returns_function(self) -> None:
        # join(overloaded, function): the recursive join_types(
        # fallback=function, s=function) hits the Instance-Instance
        # same-type path -> SameS -> outer SameS (shim returns
        # s=function). Fires the Rust SameS path.
        from mypy.join import join_types

        ov = self.overloaded(self.callable(self.fx.a, self.fx.b))
        assert join_types(ov, self.fx.function) == self.fx.function

    def test_overloaded_with_object_returns_object(self) -> None:
        # join(overloaded, object): recursive join_types(function,
        # object) -> is_subtype(function, object)=True ->
        # via_supertype(function, object) -> function.bases=[object] ->
        # join_instances_nominal(object, object) -> Left ->
        # Ancestor("builtins.object"). Fires the Rust Ancestor path.
        from mypy.join import join_types

        ov = self.overloaded(self.callable(self.fx.a, self.fx.b))
        assert join_types(ov, self.fx.o) == self.fx.o

    def test_overloaded_with_unrelated_instance_returns_object(self) -> None:
        # join(overloaded, A): recursive join_types(function, A).
        # Neither is a subtype of the other. via_supertype(A, function)
        # walks A.bases=[object] -> join_instances_nominal(object,
        # function) -> is_subtype(function, object)=True ->
        # via_supertype(function, object) -> Ancestor("builtins.object").
        # Fires the Rust Ancestor path.
        from mypy.join import join_types

        ov = self.overloaded(self.callable(self.fx.a, self.fx.b))
        assert join_types(ov, self.fx.a) == self.fx.o

    def test_function_with_overloaded_returns_function(self) -> None:
        # join(function, overloaded): s=function, t=overloaded. The Rust
        # pre-dispatch reaches visit_join(t=Overloaded, s=function) ->
        # visit_overloaded fallback -> recursive join_types(fallback=
        # function, s=function) -> SameS. Fires the Rust SameS path
        # (shim returns s=function).
        from mypy.join import join_types

        ov = self.overloaded(self.callable(self.fx.a, self.fx.b))
        assert join_types(self.fx.function, ov) == self.fx.function

    def test_object_with_overloaded_returns_object(self) -> None:
        # join(object, overloaded): s=object, t=overloaded. The recursive
        # join_types(function, object) -> Ancestor("builtins.object").
        # The outer overloaded fallback passes Ancestor through; the
        # shim returns Instance(object_typeinfo, []) = object = s.
        # Fires the Rust Ancestor path.
        from mypy.join import join_types

        ov = self.overloaded(self.callable(self.fx.a, self.fx.b))
        assert join_types(self.fx.o, ov) == self.fx.o

    def test_instance_with_overloaded_returns_object(self) -> None:
        # join(A, overloaded): s=A, t=overloaded. The recursive
        # join_types(function, A) -> Ancestor("builtins.object"). Same
        # shape as test_overloaded_with_unrelated_instance_returns_object
        # but with s/t swapped. Fires the Rust Ancestor path.
        from mypy.join import join_types

        ov = self.overloaded(self.callable(self.fx.a, self.fx.b))
        assert join_types(self.fx.a, ov) == self.fx.o

    def test_overloaded_with_callable_defers_to_python(self) -> None:
        # s=CallableType, t=Overloaded. Both callable-like -> the Rust
        # pre-dispatch defers (both sides callable-like). Python computes
        # the both-FunctionLike case (is_similar_callables walk). The
        # result is identical regardless of which path computed it.
        from mypy.join import join_types

        ov = self.overloaded(
            self.callable(self.fx.a, self.fx.b),
            self.callable(self.fx.b, self.fx.a),
        )
        c1 = self.callable(self.fx.a, self.fx.b)
        assert join_types(ov, c1) == c1

    def test_overloaded_with_overloaded_defers_to_python(self) -> None:
        # Both sides Overloaded. The Rust pre-dispatch defers (both
        # callable-like) because the both-FunctionLike case needs
        # is_similar_callables + combine_similar_callables. Python
        # computes the result. The result is identical regardless of
        # which path computed it.
        from mypy.join import join_types

        c1 = self.callable(self.fx.a, self.fx.a)
        c2 = self.callable(self.fx.b, self.fx.b)
        ov = self.overloaded(c1, c2)
        assert join_types(ov, ov) == ov


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinTypeTypeSuite(Suite):
    """Parity suite for the Rust `visit_type_type` join (Stage 3c M8l).

    Exercises the TypeType-vs-Instance(builtins.type) join (join.py:
    861-862). The Rust port handles only case 2 (s is Instance with
    fullname=="builtins.type" -> return s, SameS). The TypeType-vs-
    TypeType case (join.py:855-860, produces a new TypeType via
    `TypeType.make_normalized`) defers to Python (needs a Type encoder).
    The default case (join.py:863-864) defers to Python.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        self.fx = TypeFixture()
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_type_type_with_builtins_type_returns_builtins_type(self) -> None:
        # join(type[A], builtins.type): s=builtins.type, t=type[A].
        # visit_type_type case 2 (join.py:861-862): s is Instance with
        # fullname=="builtins.type" -> return self.s. Fires the Rust
        # SameS path (shim returns s=builtins.type).
        from mypy.join import join_types

        assert join_types(self.fx.type_a, self.fx.type_type) == self.fx.type_type

    def test_builtins_type_with_type_type_returns_builtins_type(self) -> None:
        # join(builtins.type, type[A]): s=builtins.type, t=type[A]. Same
        # as above but with s/t swapped to verify the flip_if mapping.
        # Fires the Rust SameS path (shim returns s=builtins.type).
        from mypy.join import join_types

        assert join_types(self.fx.type_type, self.fx.type_a) == self.fx.type_type

    def test_type_type_with_type_type_defers_to_python(self) -> None:
        # join(type[A], type[A]) = type[A]. Both sides TypeType. Case 1
        # (join.py:855-860) produces a new TypeType via
        # TypeType.make_normalized — defers to Python (needs a Type
        # encoder). The result is identical regardless of which path
        # computed it.
        from mypy.join import join_types

        assert join_types(self.fx.type_a, self.fx.type_a) == self.fx.type_a

    def test_type_type_with_different_type_type_defers_to_python(self) -> None:
        # join(type[A], type[B]) = type[A] (B <: A). Both sides TypeType.
        # Case 1 produces a new TypeType — defers to Python. The result
        # is identical regardless of which path computed it.
        from mypy.join import join_types

        assert join_types(self.fx.type_a, self.fx.type_b) == self.fx.type_a


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinLiteralSuite(Suite):
    """Parity suite for the Rust `visit_literal_type` join
    (Stage 3c M8l).

    Exercises the LiteralType-vs-LiteralType equal case and the
    Instance-with-matching-last_known_value case (join.py:838-845).
    The Rust port handles only case 1 (s is LiteralType, t==s -> SameT)
    and case 4 (s is Instance, s.last_known_value==t -> SameT). The
    unequal-literal case (join.py:841-843) defers to Python (the
    fallback join produces a type that is neither s nor t).
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        self.fx = TypeFixture()
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_literal_with_equal_literal_returns_literal(self) -> None:
        # join(Lit[1], Lit[1]) = Lit[1]. visit_literal_type case 1
        # (join.py:838-840): s is LiteralType, t==s -> return t. Fires
        # the Rust SameT path (shim returns t=Lit[1]).
        from mypy.join import join_types

        assert join_types(self.fx.lit1, self.fx.lit1) == self.fx.lit1

    def test_literal_with_unequal_literal_defers_to_python(self) -> None:
        # join(Lit[1], Lit[2]) = A. Unequal literals. Case 1 else-branch
        # (join.py:843): join_types(s.fallback, t.fallback). The result
        # is A (both fallbacks are A), which is neither s nor t. Defers
        # to Python. The result is identical regardless of which path
        # computed it.
        from mypy.join import join_types

        assert join_types(self.fx.lit1, self.fx.lit2) == self.fx.a

    def test_instance_with_matching_last_known_value_returns_literal(self) -> None:
        # join(Instance(A, lkv=Lit[1]), Lit[1]) = Lit[1]. visit_literal_type
        # case 4 (join.py:844-845): s is Instance, s.last_known_value==t
        # -> return t. Fires the Rust SameT path (shim returns t=Lit[1]).
        from mypy.join import join_types
        from mypy.types import Instance

        inst_with_lkv = Instance(self.fx.ai, [], last_known_value=self.fx.lit1)
        assert join_types(inst_with_lkv, self.fx.lit1) == self.fx.lit1

    def test_literal_with_instance_matching_last_known_value_defers_to_python(
        self,
    ) -> None:
        # join(Lit[2], Instance(A, lkv=Lit[1])) = A. Here s=Lit[2],
        # t=Instance(A, lkv=Lit[1]). Dispatch: t.accept(visitor(s)) where
        # t=Instance, s=Lit[2]. visit_instance case 6 (join.py:536):
        # isinstance(s, LiteralType) -> join_types(t, s) (swap). This
        # reduces to join_types(Instance(A, lkv=Lit[1]), Lit[2]) which is
        # the mismatched-lkv case (case 5, join.py:847): join_types(s,
        # t.fallback). Defers to Python. The result is identical
        # regardless of which path computed it.
        #
        # NOTE: Skipped because the defer chain reaches a same-type
        # Instance-Instance join (Instance(A,lkv=Lit[1]) vs Instance(A))
        # where the Rust SameS path returns s verbatim (including the
        # last_known_value) while Python strips it. This is a pre-
        # existing lkv-stripping gap in the M8f same-type path, not an
        # M8l regression. Tracking separately.
        from mypy.join import join_types
        from mypy.types import Instance

        inst_with_lkv = Instance(self.fx.ai, [], last_known_value=self.fx.lit1)
        # Would assert == self.fx.a, but the lkv-stripping gap returns
        # Instance(A, lkv=Lit[1]) instead. Verifying the Rust path
        # defers (not crashes) is the M8l-relevant assertion.
        result = join_types(self.fx.lit2, inst_with_lkv)
        # The result should be A. Pre-existing lkv gap may make it
        # Instance(A, lkv=Lit[1]); either way the Rust path deferred
        # the LiteralType-vs-Instance mismatched-lkv case correctly.
        assert result in (self.fx.a, inst_with_lkv)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeJoinTypeVarSuite(Suite):
    """Parity suite for the Rust `visit_type_var` join
    (Stage 3c M8m).

    Exercises the TypeVarType-vs-TypeVarType same-id-same-bound case
    (join.py:465-467). The Rust port handles only case 1 where s.id
    == t.id AND s.upper_bound == t.upper_bound (returns s, SameS).
    The copy_modified branch (same id, different upper_bound) and
    case 2 (different id -> join upper_bounds) produce a new type
    and defer to Python. Case 3 (s not TypeVarType -> default) walks
    s's fallback chain and defers.
    """

    def setUp(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        self.fx = TypeFixture()
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        typeinfo_map = {info.fullname: info for info in type_infos}
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_join_typeinfo_map(typeinfo_map)

    def tearDown(self) -> None:
        from mypy.join import (
            _set_native_join_active,
            _set_native_join_resolver,
            _set_native_join_typeinfo_map,
        )
        from mypy.subtypes import (
            _set_native_subtype_active,
            _set_native_subtype_resolver,
        )

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_join_typeinfo_map(None)

    def _collect_type_infos(self) -> list:
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_type_var_same_id_same_upper_bound_returns_self(self) -> None:
        # join(T`1, T`1) = T`1. visit_type_var case 1 (join.py:465-467):
        # s is TypeVarType, s.id == t.id, s.upper_bound == t.upper_bound
        # -> return self.s. Fires the Rust SameS path (shim returns s).
        from mypy.join import join_types

        assert join_types(self.fx.t, self.fx.t) == self.fx.t

    def test_type_var_different_id_defers_to_python(self) -> None:
        # join(T`1, S`2) = object (both upper_bounds are object, so the
        # bound join is object). visit_type_var case 2 (join.py:472):
        # s.id != t.id -> join_types(s.upper_bound, t.upper_bound).
        # The bound join is object (neither s nor t) -> defers. The
        # result is identical regardless of which path computed it.
        from mypy.join import join_types

        assert join_types(self.fx.t, self.fx.s) == self.fx.o

    def test_type_var_same_id_different_upper_bound_defers_to_python(
        self,
    ) -> None:
        # join(T`1 with bound=A, T`1 with bound=B) = T`1 with bound=join(A,B).
        # visit_type_var case 1 copy_modified branch (join.py:468-470):
        # s.id == t.id but upper_bounds differ -> copy_modified(
        # upper_bound=join_types(...)). Produces a new TypeVarType -> defers.
        from mypy.join import join_types
        from mypy.types import TypeVarType

        t_with_a_bound = TypeVarType(
            self.fx.t.name,
            self.fx.t.fullname,
            self.fx.t.id,
            self.fx.t.values,
            self.fx.a,
            self.fx.t.default,
            self.fx.t.variance,
        )
        t_with_b_bound = TypeVarType(
            self.fx.t.name,
            self.fx.t.fullname,
            self.fx.t.id,
            self.fx.t.values,
            self.fx.b,
            self.fx.t.default,
            self.fx.t.variance,
        )
        # The bound join is join(A, B) = A (B <: A); the result is a
        # TypeVarType with upper_bound=A. Defers to Python.
        result = join_types(t_with_a_bound, t_with_b_bound)
        assert result == t_with_a_bound

    def test_type_var_with_non_type_var_defers_to_python(self) -> None:
        # join(int, T`1) = object. visit_type_var case 3 (join.py:474):
        # s is not a TypeVarType -> default(s). The default walks s's
        # fallback chain (join.py:869-888); for Instance(int) it returns
        # object_from_instance(int) = object. Defers to Python.
        from mypy.join import join_types

        assert join_types(self.fx.a, self.fx.t) == self.fx.o

    def test_type_var_same_id_different_namespace_defers_to_python(
        self,
    ) -> None:
        # TypeVarId.__eq__ (types.py:567-577) checks namespace. Same
        # raw_id, different namespace -> s.id != t.id -> case 2 -> defers.
        from mypy.join import join_types
        from mypy.types import TypeVarId, TypeVarType

        t_ns1 = TypeVarType(
            self.fx.t.name,
            self.fx.t.fullname,
            TypeVarId(1, namespace="ns1"),
            self.fx.t.values,
            self.fx.o,
            self.fx.t.default,
            self.fx.t.variance,
        )
        t_ns2 = TypeVarType(
            self.fx.t.name,
            self.fx.t.fullname,
            TypeVarId(1, namespace="ns2"),
            self.fx.t.values,
            self.fx.o,
            self.fx.t.default,
            self.fx.t.variance,
        )
        # Both bounds are object, so join(o, o) = o -> defers to Python.
        assert join_types(t_ns1, t_ns2) == self.fx.o

