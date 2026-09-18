"""Test cases for mypy types and type operations."""

from __future__ import annotations

import os
import re
from collections.abc import Sequence
from types import SimpleNamespace
from typing import Any, TypeVar
from unittest import TestCase, skipUnless

from mypy.erasetype import _set_native_erase_active, erase_type, remove_instance_last_known_values
from mypy.test.helpers import _env_gate

# Mirror testcheck.py: flip the type-kernel gate from the env var so the
# unit tests exercise the Rust path when TEST_NATIVE_TYPE_KERNEL is set.
# Unset exercises the default (Python) path; =1 exercises the Rust path.
_set_native_erase_active(_env_gate("TEST_NATIVE_TYPE_KERNEL"))
from mypy.mro import _set_native_mro_active

_set_native_mro_active(_env_gate("TEST_NATIVE_TYPE_KERNEL"))
from mypy.checkstrformat import _set_native_strformat_active

_set_native_strformat_active(_env_gate("TEST_NATIVE_TYPE_KERNEL"))
T = TypeVar("T")
from mypy.indirection import TypeIndirectionVisitor
from mypy.join import join_types
from mypy.meet import is_overlapping_types, meet_types, narrow_declared_type
from mypy.nodes import (
    ARG_NAMED,
    ARG_OPT,
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    CONTRAVARIANT,
    COVARIANT,
    INVARIANT,
    ArgKind,
    Block,
    CallExpr,
    ClassDef,
    Expression,
    MypyFile,
    NameExpr,
    SymbolTable,
    TypeInfo,
    Var,
)
from mypy.options import Options
from mypy.plugins.common import find_shallow_matching_overload_item
from mypy.state import state
from mypy.subtypes import is_more_precise, is_proper_subtype, is_same_type, is_subtype
from mypy.test.helpers import Suite, _env_gate, assert_equal, assert_type, skip
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
from mypy.checker import TypeChecker


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
        # make_simplified_union against differently restricted truthiness types drops
        # restrictions.
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
    _WriteBuffer = None  # type: ignore[assignment,misc]
    _HAS_TYPE_KERNEL_WIRE = False

_HAS_TYPE_KERNEL = _type_kernel is not None

_NATIVE_WIRE_ENABLED = _env_gate("TEST_NATIVE_TYPE_KERNEL") and _HAS_TYPE_KERNEL_WIRE

_NATIVE_SEMANAL_LOOKUP_ENABLED = _env_gate("TEST_NATIVE_TYPE_KERNEL") and _HAS_TYPE_KERNEL_WIRE


class FreshVarCanonicalizerSuite(Suite):
    """Regression tests for fresh meta-var identity repair (wirefixup).

    Python's in-memory freshen returns the *same* fresh TypeVar object for
    every occurrence of a given id; downstream inference compares
    metavariables by object identity. The wire round-trip splits each
    occurrence into a distinct object sharing an id, so
    ``canonicalize_fresh_vars`` must re-unify them. These tests are pure
    Python and run without the native extension.
    """

    def setUp(self) -> None:
        self.fx = TypeFixture()
        any_type = AnyType(TypeOfAny.special_form)
        self.fresh1 = TypeVarType("T", "T", TypeVarId(1, meta_level=1), [], self.fx.o, any_type)
        self.fresh2 = TypeVarType("S", "S", TypeVarId(2, meta_level=1), [], self.fx.o, any_type)
        self.declared = TypeVarType(
            "D", "D", TypeVarId(3), [], self.fx.o, any_type  # meta_level=0
        )

    def test_same_occurrences_share_one_object(self) -> None:
        from mypy.wirefixup import canonicalize_fresh_vars

        c = CallableType(
            [self.fresh1, self.fresh1],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fresh2,
            self.fx.function,
            variables=[self.fresh1, self.fresh2],
        )
        fixed = canonicalize_fresh_vars(c)
        assert isinstance(fixed, CallableType)  # type: ignore[misc]
        # Same raw id must map to the same object, not equal-but-distinct.
        assert fixed.arg_types[0] is fixed.arg_types[1]
        assert fixed.arg_types[0] is fixed.variables[0]
        assert fixed.ret_type is fixed.variables[1]

    def test_distinct_ids_stay_distinct(self) -> None:
        from mypy.wirefixup import canonicalize_fresh_vars

        c = CallableType(
            [self.fresh1, self.fresh2],
            [ARG_POS, ARG_POS],
            [None, None],
            self.fresh1,
            self.fx.function,
            variables=[self.fresh1, self.fresh2],
        )
        fixed = canonicalize_fresh_vars(c)
        assert isinstance(fixed, CallableType)  # type: ignore[misc]
        assert fixed.arg_types[0] is fixed.ret_type
        assert fixed.arg_types[0] is not fixed.arg_types[1]

    def test_declared_vars_untouched(self) -> None:
        from mypy.wirefixup import canonicalize_fresh_vars

        c = CallableType(
            [self.declared],
            [ARG_POS],
            [None],
            self.declared,
            self.fx.function,
            variables=[self.declared],
        )
        fixed = canonicalize_fresh_vars(c)
        assert isinstance(fixed, CallableType)  # type: ignore[misc]
        # meta_level=0 vars are ordinary declared vars; leave each as-is.
        assert fixed.arg_types[0] is self.declared
        assert fixed.ret_type is self.declared


# Stage 3b parity suite: round-trips `mypy.types.Type` through the binary
# wire format and asserts that the Rust reader, with a TypeInfo resolver
# built from the live Python TypeInfo graph, produces the same `str(t)` as

# the Python `TypeStrVisitor`. Gated by `TEST_NATIVE_TYPE_KERNEL=1` plus
# the presence of the `type_kernel` extension; skipped otherwise. This
# closes the Stage 3a deferred renderings (prefix-strip on builtins.*,


# enum-literal `value_repr`, bytes-literal `value_repr`, the `[()]`
# variadic-tuple branch) and proves the resolver protocol end-to-end.


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
    ALLOWED_GET_PROPER_TYPES = 9  # +1: _needs_definitions walks non-recursive aliases

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


def _rru_flags_blob(items: Sequence[Type]) -> bytes:
    # Per-item truthiness blob for rust_remove_redundant_union_items:
    # (can_be_true, can_be_false, mutated) triples, computed like the
    # shim in mypy.typeops._remove_redundant_union_items.
    from mypy.typeops import _has_mutated_truthiness

    blob = bytearray()
    for it in items:
        blob += bytes(
            (
                1 if it.can_be_true else 0,
                1 if it.can_be_false else 0,
                1 if _has_mutated_truthiness(it) else 0,
            )
        )
    return bytes(blob)


def _base_infos(fx: TypeFixture) -> list[TypeInfo]:
    """All fixture TypeInfos for the resolver + object."""
    return [getattr(fx, n) for n in dir(fx) if n.endswith("i") and _is_type_info(getattr(fx, n))]


# (Removed: orphan M8g visit_instance parity block — class header was never
# added, so its `setUp`/`tearDown`/`_collect_type_infos`/tests shadowed the
# outer NativeUninhabitedSuite and tripped `no-redef` on self-check.)


# Stage 5 parity suite: exercises `mro::rust_linearize_hierarchy` through the
# public `calculate_mro` entry. Each test builds a live TypeInfo graph with
# explicit `bases` and empty `mro`, builds a `NativeTypeResolver` snapshot

# from it, installs it via `_set_native_mro_resolver`, and asserts the MRO
# `calculate_mro` assigns matches the expected C3 order. Gated by
# `TEST_NATIVE_TYPE_KERNEL=1` plus the `type_kernel` extension; skipped


# otherwise (the Python path is exercised by the existing suites).


# Stage 6b parity suite: compares Python vs Rust format-string parsing.
# Gated by TEST_NATIVE_TYPE_KERNEL=1 plus the type_kernel extension.
try:
    import type_kernel as _strformat_type_kernel  # noqa: F401

    _HAS_TYPE_KERNEL_STRFORMAT_TEST = True
except ImportError:
    _HAS_TYPE_KERNEL_STRFORMAT_TEST = False

_NATIVE_STRFORMAT_ENABLED = (
    bool(os.environ.get("TEST_NATIVE_TYPE_KERNEL")) and _HAS_TYPE_KERNEL_STRFORMAT_TEST
)


# ---------------------------------------------------------------------------
# NativeCheckexprSuite — M8c: visit_yield_expr / visit_conditional_expr /
# visit_star_expr ports to Rust (checkexpr_functions.rs)

# ---------------------------------------------------------------------------


# NativeGetMemberFlagsSuite: differential parity for the Rust port of
# `get_member_flags` (subtypes.py:1773-1812). Toggles the subtype gate
# on/off, asserting native and pure-Python paths agree (live PyO3 reads).


# NativeAnyCausesOverloadAmbiguitySuite: differential parity for the Rust
# port of `any_causes_overload_ambiguity` (checkexpr.py:8233-8308).


# Mirrors NativeCombineSignaturesSuite: toggle the checkexpr gate on/off and
# assert the native and pure-Python paths agree on the same items/actuals.


def strict_optional_flag() -> bool:
    """The `strict_optional` flag threaded into the constraint seam."""
    from mypy.state import state

    return state.strict_optional


class _FakeNode:
    """Minimal stand-in for a MypyFile with `.names` for builtins tests."""

    def __init__(self, names: dict[str, Any]) -> None:
        self.names = names


class _RaisingFinalValue:
    """`is_final` value whose `__bool__` raises `exc`.

    The inner arm of `rust_classify_final_super` calls `is_true()` (the
    Python truthiness protocol) on the already-read attribute value. The
    issue #1470 `_BrokenFinalVar` shape raises on the getattr itself and
    cannot reach that arm, so the #1477 pins need `is_final` to *return* a
    value whose truthiness raises.
    """

    def __init__(self, exc: type[Exception] = AttributeError) -> None:
        self._exc = exc

    def __bool__(self) -> bool:
        raise self._exc("is_final bool")


class _BrokenFinalValueVar(Var):
    """Var whose `is_final` returns a value with a raising `__bool__`."""

    def __init__(self, name: str, exc: type[Exception] = AttributeError) -> None:
        super().__init__(name)
        self.is_final = _RaisingFinalValue(exc)  # type: ignore[assignment]


class _BrokenFinalVar(Var):
    """Var stand-in whose `is_final` read raises `exc`.

    Built for the issue #1470 deferral + error-class pins: an
    AttributeError on the read must defer (Ok(None)) so the shim re-runs
    the pure-Python body, while any other exception must re-propagate
    identically on both gates. `Var.is_final` is a plain attribute (set by
    assignment in `__init__`, never read), so the override only fires when
    the seam or the pure body reads it.
    """

    def __init__(self, name: str, exc: type[Exception] = AttributeError) -> None:
        self._broken_exc = exc
        super().__init__(name)
        self.is_final = False

    def __getattribute__(self, name: str) -> Any:
        if name == "_broken_exc":
            return super().__getattribute__(name)
        if name == "is_final":
            raise super().__getattribute__("_broken_exc")(name)
        return super().__getattribute__(name)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
def _build_native_variance_resolver(infos: list[TypeInfo]) -> Any:
    """Build a `NativeTypeResolver` snapshot over the given live TypeInfos
    (the class plus the fixture graph it subclasses) so the Rust subtype
    and callable-compat seams resolve every type_ref the `infer_variance`
    member types exercise."""
    return _type_kernel.build_native_resolver(infos, [])


class _FakeTypeInfo:
    """Minimal TypeInfo stand-in carrying just the fields the Rust seam reads.

    The semanal_visitor seams only read `fullname` (and `is_enum` for the
    TypeInfo is_type_ref branch), so a tiny fake keeps the suite hermetic and
    independent of the resolver / TypeFixture graph.
    """

    def __init__(self, fullname: str, is_enum: bool = False) -> None:
        self.fullname = fullname
        self.is_enum = is_enum


class _ValidVarArgStubChk:
    """Minimal TypeChecker stand-in for the var-arg predicate tests.

    Only `named_type` / `named_generic_type` are needed: the two
    `ExpressionChecker.is_valid_*_var_arg` methods are the only things the
    differential touches.
    """

    def __init__(self, infos: dict[str, TypeInfo]) -> None:
        self._infos = infos

    def named_type(self, name: str) -> Instance:
        return Instance(self._infos[name], [])

    def named_generic_type(self, name: str, args: list[Type]) -> Instance:
        return Instance(self._infos[name], tuple(args))


def _chk(in_checked: bool) -> SimpleNamespace:
    return SimpleNamespace(in_checked_function=lambda: in_checked)


class _FakeScope:
    """Minimal `Scope` stand-in exposing `stack` and `active_class()`."""

    def __init__(self, module: bool, cls: bool = False) -> None:
        self.stack = [object()] if module else [object(), object()]
        self._cls = cls

    def active_class(self) -> bool:
        return self._cls


class _FakeMsg:
    def __init__(self) -> None:
        self.records: list[tuple[str, object]] = []

    def invalid_signature_for_special_method(
        self, typ: object, context: object, name: str
    ) -> None:
        self.records.append(("invalid_signature", name))


class _FakeChecker:
    """Wires the real `check_getattr_method` onto fake scope/msg state."""

    def __init__(self, scope: _FakeScope, fx: TypeFixture) -> None:

        self.scope = scope
        self.msg = _FakeMsg()
        self._failures: list[str] = []
        self._checker_cls = TypeChecker
        self._fx = fx

    def named_type(self, name: str) -> Instance:
        if "[" in name:
            raise AssertionError(f"named_type got parameterized {name}")
        return Instance(self._fx.make_type_info(name), [])

    def fail(self, msg: str, ctx: object) -> None:
        self._failures.append(msg)


def _run_check(
    scope: _FakeScope, typ: object, name: str
) -> tuple[list[str], list[tuple[str, object]]]:
    """Call the real `check_getattr_method` body against a fake checker."""

    chk = _FakeChecker(scope, TypeFixture())
    TypeChecker.check_getattr_method(chk, typ, context=None, name=name)  # type: ignore[arg-type]
    return chk._failures, chk.msg.records


def expect_fail_check(module: bool, name: str, ok: bool, cls: bool) -> bool:
    """Does a check with these facts produce any failure record?"""
    if module and name == "__getattribute__":
        return True
    if not module and not cls:
        return False
    return not ok


def _make_getattr_sig(ok: bool, cls: bool = False) -> object:
    """A `__getattr__` signature matching the arity the method expects."""
    from mypy.nodes import ARG_POS
    from mypy.test.typefixture import TypeFixture
    from mypy.types import AnyType, CallableType, Instance, TypeOfAny

    fx = TypeFixture()
    args: list[Type] = [Instance(fx.make_type_info("builtins.str"), [])]
    if cls:
        args = [AnyType(TypeOfAny.special_form), Instance(fx.make_type_info("builtins.str"), [])]
    if not ok:
        args[0 if not cls else 1] = Instance(fx.make_type_info("builtins.int"), [])
    return CallableType(
        args,
        [ARG_POS] * len(args),
        [None] * len(args),
        AnyType(TypeOfAny.special_form),
        Instance(fx.make_type_info("builtins.function"), []),
    )


class _FindMemberTailReached(Exception):
    """Raised by the fake checker msg when find_member reaches its tail."""


class _FindMemberTailMsg:
    """Fake checker msg: entering filter_errors marks the tail handoff."""

    def filter_errors(self, *args: Any, **kwargs: Any) -> Any:
        raise _FindMemberTailReached


_FIND_MEMBER_TAIL = "<find_member_tail_reached>"


try:
    import type_kernel as _splice_kernel

    _SPLICE_EXT_OK = True
except ImportError:
    _splice_kernel = None  # type: ignore[assignment]
    _SPLICE_EXT_OK = False


def _splice_funnel_active() -> bool:
    """write_raw_bytes is new in the librt fork; with PyPI librt the
    wire-cache splice path is inert and the splice-hit tests skip."""
    from mypy.types import write_raw_bytes

    return _SPLICE_EXT_OK and bool(write_raw_bytes)


_SPLICE_ACTIVE = _splice_funnel_active()


class _BrokenAttrInfo(TypeInfo):
    """TypeInfo stand-in whose getattr of one chosen attribute raises.

    Built for the wave-48 classifier-seam deferral pins (issue #1466): a
    malformed live TypeInfo whose attribute is unreadable. The seams must
    map the failed read to a deferral (Ok(None)) so the shim re-runs the
    pure-Python body, which raises the identical exception on both sides
    of the gate. The `exc` class pins the error-class boundary: only
    AttributeError defers, anything else re-propagates.
    """

    def __init__(
        self, fullname: str, broken_attr: str, exc: type[Exception] = AttributeError
    ) -> None:

        cd = ClassDef(fullname.rsplit(".", 1)[-1], Block([]), None, [])
        cd.fullname = fullname
        self._broken_attr = broken_attr
        self._broken_exc = exc
        super().__init__(SymbolTable(), cd, fullname.rsplit(".", 1)[0])

    def __getattribute__(self, name: str) -> Any:
        broken = super().__getattribute__("_broken_attr")
        exc = super().__getattribute__("_broken_exc")
        if name == "_broken_attr":
            return broken
        if name == "_broken_exc":
            return exc
        if name == broken:
            raise exc(name)
        return super().__getattribute__(name)


# H1d cluster (#1672): live-object decision heads on the checker gate.
# Each suite pins the seam payload directly, then drives the real method
# gate-off vs gate-on. None of the four seams serializes anything.


class _H1dBinderStub:
    """`ConditionalTypeBinder` stand-in: only the suppression query is read."""

    def __init__(self, suppressed: bool) -> None:
        self._suppressed = suppressed

    def is_unreachable_warning_suppressed(self) -> bool:
        return self._suppressed


class _H1dCheckerStub:
    """Attribute bag standing in for `TypeChecker` in direct seam calls."""

    def __init__(
        self,
        options: Options,
        dynamic_funcs: list[bool],
        current_node_deferred: bool,
        binder: _H1dBinderStub,
    ) -> None:
        self.options = options
        self.dynamic_funcs = dynamic_funcs
        self.current_node_deferred = current_node_deferred
        self.binder = binder


class _H1dScopeCheckerStub:
    """`TypeChecker` stand-in carrying only `scope` and `tree`."""

    def __init__(self, scope: Any, tree: Any) -> None:
        self.scope = scope
        self.tree = tree


def _h1d_module(fullname: str) -> MypyFile:
    """A bare `MypyFile` with `fullname` assigned (the bare ctor omits it)."""
    module = MypyFile([], [])
    module._fullname = fullname
    return module


def _h1d_names(expressions: Sequence[Expression]) -> list[str]:
    """Stable identity fingerprint for a list of expression nodes."""
    return [f"{type(e).__name__}:{id(e)}" for e in expressions]
