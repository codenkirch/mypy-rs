from __future__ import annotations

import os
from unittest import skipUnless

from mypy.nodes import CONTRAVARIANT, COVARIANT, INVARIANT
from mypy.subtypes import is_subtype
from mypy.test.helpers import Suite
from mypy.test.typefixture import InterfaceTypeFixture, TypeFixture
from mypy.types import Instance, TupleType, Type, UninhabitedType, UnpackType

# Stage 3c (M8b) parity suite: reruns the nominal-instance subtype cases
# with the Rust is_subtype path active. Rust handles nominal cases and
# falls through to Python on the rest, so results must match. Gated.
try:
    import type_kernel as _type_kernel

    _HAS_TYPE_KERNEL = True
except ImportError:
    _type_kernel = None  # type: ignore[assignment]
    _HAS_TYPE_KERNEL = False

_NATIVE_WIRE_ENABLED = bool(os.environ.get("TEST_NATIVE_TYPE_KERNEL")) and _HAS_TYPE_KERNEL


class SubtypingSuite(Suite):
    def setUp(self) -> None:
        self.fx = TypeFixture(INVARIANT)
        self.fx_contra = TypeFixture(CONTRAVARIANT)
        self.fx_co = TypeFixture(COVARIANT)

    def test_trivial_cases(self) -> None:
        for simple in self.fx_co.a, self.fx_co.o, self.fx_co.b:
            self.assert_subtype(simple, simple)

    def test_instance_subtyping(self) -> None:
        self.assert_strict_subtype(self.fx.a, self.fx.o)
        self.assert_strict_subtype(self.fx.b, self.fx.o)
        self.assert_strict_subtype(self.fx.b, self.fx.a)

        self.assert_not_subtype(self.fx.a, self.fx.d)
        self.assert_not_subtype(self.fx.b, self.fx.c)

    def test_simple_generic_instance_subtyping_invariant(self) -> None:
        self.assert_subtype(self.fx.ga, self.fx.ga)
        self.assert_subtype(self.fx.hab, self.fx.hab)

        self.assert_not_subtype(self.fx.ga, self.fx.g2a)
        self.assert_not_subtype(self.fx.ga, self.fx.gb)
        self.assert_not_subtype(self.fx.gb, self.fx.ga)

    def test_simple_generic_instance_subtyping_covariant(self) -> None:
        self.assert_subtype(self.fx_co.ga, self.fx_co.ga)
        self.assert_subtype(self.fx_co.hab, self.fx_co.hab)

        self.assert_not_subtype(self.fx_co.ga, self.fx_co.g2a)
        self.assert_not_subtype(self.fx_co.ga, self.fx_co.gb)
        self.assert_subtype(self.fx_co.gb, self.fx_co.ga)

    def test_simple_generic_instance_subtyping_contravariant(self) -> None:
        self.assert_subtype(self.fx_contra.ga, self.fx_contra.ga)
        self.assert_subtype(self.fx_contra.hab, self.fx_contra.hab)

        self.assert_not_subtype(self.fx_contra.ga, self.fx_contra.g2a)
        self.assert_subtype(self.fx_contra.ga, self.fx_contra.gb)
        self.assert_not_subtype(self.fx_contra.gb, self.fx_contra.ga)

    def test_generic_subtyping_with_inheritance_invariant(self) -> None:
        self.assert_subtype(self.fx.gsab, self.fx.gb)
        self.assert_not_subtype(self.fx.gsab, self.fx.ga)
        self.assert_not_subtype(self.fx.gsaa, self.fx.gb)

    def test_generic_subtyping_with_inheritance_covariant(self) -> None:
        self.assert_subtype(self.fx_co.gsab, self.fx_co.gb)
        self.assert_subtype(self.fx_co.gsab, self.fx_co.ga)
        self.assert_not_subtype(self.fx_co.gsaa, self.fx_co.gb)

    def test_generic_subtyping_with_inheritance_contravariant(self) -> None:
        self.assert_subtype(self.fx_contra.gsab, self.fx_contra.gb)
        self.assert_not_subtype(self.fx_contra.gsab, self.fx_contra.ga)
        self.assert_subtype(self.fx_contra.gsaa, self.fx_contra.gb)

    def test_interface_subtyping(self) -> None:
        self.assert_subtype(self.fx.e, self.fx.f)
        self.assert_equivalent(self.fx.f, self.fx.f)
        self.assert_not_subtype(self.fx.a, self.fx.f)

    def test_generic_interface_subtyping(self) -> None:
        # TODO make this work
        fx2 = InterfaceTypeFixture()

        self.assert_subtype(fx2.m1, fx2.gfa)
        self.assert_not_subtype(fx2.m1, fx2.gfb)

        self.assert_equivalent(fx2.gfa, fx2.gfa)

    def test_basic_callable_subtyping(self) -> None:
        self.assert_strict_subtype(
            self.fx.callable(self.fx.o, self.fx.d), self.fx.callable(self.fx.a, self.fx.d)
        )
        self.assert_strict_subtype(
            self.fx.callable(self.fx.d, self.fx.b), self.fx.callable(self.fx.d, self.fx.a)
        )

        self.assert_strict_subtype(
            self.fx.callable(self.fx.a, UninhabitedType()), self.fx.callable(self.fx.a, self.fx.a)
        )

        self.assert_unrelated(
            self.fx.callable(self.fx.a, self.fx.a, self.fx.a),
            self.fx.callable(self.fx.a, self.fx.a),
        )

    def test_default_arg_callable_subtyping(self) -> None:
        self.assert_strict_subtype(
            self.fx.callable_default(1, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable(self.fx.a, self.fx.d, self.fx.a),
        )

        self.assert_strict_subtype(
            self.fx.callable_default(1, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable(self.fx.a, self.fx.a),
        )

        self.assert_strict_subtype(
            self.fx.callable_default(0, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable_default(1, self.fx.a, self.fx.d, self.fx.a),
        )

        self.assert_unrelated(
            self.fx.callable_default(1, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable(self.fx.d, self.fx.d, self.fx.a),
        )

        self.assert_unrelated(
            self.fx.callable_default(0, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable_default(1, self.fx.a, self.fx.a, self.fx.a),
        )

        self.assert_unrelated(
            self.fx.callable_default(1, self.fx.a, self.fx.a),
            self.fx.callable(self.fx.a, self.fx.a, self.fx.a),
        )

    def test_var_arg_callable_subtyping_1(self) -> None:
        self.assert_strict_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.a),
            self.fx.callable_var_arg(0, self.fx.b, self.fx.a),
        )

    def test_var_arg_callable_subtyping_2(self) -> None:
        self.assert_strict_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.a),
            self.fx.callable(self.fx.b, self.fx.a),
        )

    def test_var_arg_callable_subtyping_3(self) -> None:
        self.assert_strict_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.a), self.fx.callable(self.fx.a)
        )

    def test_var_arg_callable_subtyping_4(self) -> None:
        self.assert_strict_subtype(
            self.fx.callable_var_arg(1, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable(self.fx.b, self.fx.a),
        )

    def test_var_arg_callable_subtyping_5(self) -> None:
        self.assert_strict_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable(self.fx.b, self.fx.a),
        )

    def test_var_arg_callable_subtyping_6(self) -> None:
        self.assert_strict_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.f, self.fx.d),
            self.fx.callable_var_arg(0, self.fx.b, self.fx.e, self.fx.d),
        )

    def test_var_arg_callable_subtyping_7(self) -> None:
        self.assert_not_subtype(
            self.fx.callable_var_arg(0, self.fx.b, self.fx.d),
            self.fx.callable(self.fx.a, self.fx.d),
        )

    def test_var_arg_callable_subtyping_8(self) -> None:
        self.assert_not_subtype(
            self.fx.callable_var_arg(0, self.fx.b, self.fx.d),
            self.fx.callable_var_arg(0, self.fx.a, self.fx.a, self.fx.d),
        )
        self.assert_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.d),
            self.fx.callable_var_arg(0, self.fx.b, self.fx.b, self.fx.d),
        )

    def test_var_arg_callable_subtyping_9(self) -> None:
        self.assert_not_subtype(
            self.fx.callable_var_arg(0, self.fx.b, self.fx.b, self.fx.d),
            self.fx.callable_var_arg(0, self.fx.a, self.fx.d),
        )
        self.assert_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.a, self.fx.d),
            self.fx.callable_var_arg(0, self.fx.b, self.fx.d),
        )

    def test_type_callable_subtyping(self) -> None:
        self.assert_subtype(self.fx.callable_type(self.fx.d, self.fx.a), self.fx.type_type)

        self.assert_strict_subtype(
            self.fx.callable_type(self.fx.d, self.fx.b), self.fx.callable(self.fx.d, self.fx.a)
        )

        self.assert_strict_subtype(
            self.fx.callable_type(self.fx.a, self.fx.b), self.fx.callable(self.fx.a, self.fx.b)
        )

    def test_type_var_tuple(self) -> None:
        self.assert_subtype(Instance(self.fx.gvi, []), Instance(self.fx.gvi, []))
        self.assert_subtype(
            Instance(self.fx.gvi, [self.fx.a, self.fx.b]),
            Instance(self.fx.gvi, [self.fx.a, self.fx.b]),
        )
        self.assert_not_subtype(
            Instance(self.fx.gvi, [self.fx.a, self.fx.b]),
            Instance(self.fx.gvi, [self.fx.b, self.fx.a]),
        )
        self.assert_not_subtype(
            Instance(self.fx.gvi, [self.fx.a, self.fx.b]), Instance(self.fx.gvi, [self.fx.a])
        )

        self.assert_subtype(
            Instance(self.fx.gvi, [UnpackType(self.fx.ss)]),
            Instance(self.fx.gvi, [UnpackType(self.fx.ss)]),
        )
        self.assert_not_subtype(
            Instance(self.fx.gvi, [UnpackType(self.fx.ss)]),
            Instance(self.fx.gvi, [UnpackType(self.fx.us)]),
        )

        self.assert_not_subtype(
            Instance(self.fx.gvi, [UnpackType(self.fx.ss)]), Instance(self.fx.gvi, [])
        )
        self.assert_not_subtype(
            Instance(self.fx.gvi, [UnpackType(self.fx.ss)]), Instance(self.fx.gvi, [self.fx.anyt])
        )

    def test_type_var_tuple_with_prefix_suffix(self) -> None:
        self.assert_subtype(
            Instance(self.fx.gvi, [self.fx.a, UnpackType(self.fx.ss)]),
            Instance(self.fx.gvi, [self.fx.a, UnpackType(self.fx.ss)]),
        )
        self.assert_subtype(
            Instance(self.fx.gvi, [self.fx.a, self.fx.b, UnpackType(self.fx.ss)]),
            Instance(self.fx.gvi, [self.fx.a, self.fx.b, UnpackType(self.fx.ss)]),
        )
        self.assert_not_subtype(
            Instance(self.fx.gvi, [self.fx.a, UnpackType(self.fx.ss)]),
            Instance(self.fx.gvi, [self.fx.b, UnpackType(self.fx.ss)]),
        )
        self.assert_not_subtype(
            Instance(self.fx.gvi, [self.fx.a, UnpackType(self.fx.ss)]),
            Instance(self.fx.gvi, [self.fx.a, self.fx.b, UnpackType(self.fx.ss)]),
        )

        self.assert_subtype(
            Instance(self.fx.gvi, [UnpackType(self.fx.ss), self.fx.a]),
            Instance(self.fx.gvi, [UnpackType(self.fx.ss), self.fx.a]),
        )
        self.assert_not_subtype(
            Instance(self.fx.gvi, [UnpackType(self.fx.ss), self.fx.a]),
            Instance(self.fx.gvi, [UnpackType(self.fx.ss), self.fx.b]),
        )
        self.assert_not_subtype(
            Instance(self.fx.gvi, [UnpackType(self.fx.ss), self.fx.a]),
            Instance(self.fx.gvi, [UnpackType(self.fx.ss), self.fx.a, self.fx.b]),
        )

        self.assert_subtype(
            Instance(self.fx.gvi, [self.fx.a, self.fx.b, UnpackType(self.fx.ss), self.fx.c]),
            Instance(self.fx.gvi, [self.fx.a, self.fx.b, UnpackType(self.fx.ss), self.fx.c]),
        )
        self.assert_not_subtype(
            Instance(self.fx.gvi, [self.fx.a, self.fx.b, UnpackType(self.fx.ss), self.fx.c]),
            Instance(self.fx.gvi, [self.fx.a, UnpackType(self.fx.ss), self.fx.b, self.fx.c]),
        )

    def test_type_var_tuple_unpacked_variable_length_tuple(self) -> None:
        self.assert_subtype(
            Instance(self.fx.gvi, [self.fx.a, self.fx.a]),
            Instance(self.fx.gvi, [UnpackType(Instance(self.fx.std_tuplei, [self.fx.a]))]),
        )

    def test_fallback_not_subtype_of_tuple(self) -> None:
        self.assert_not_subtype(self.fx.a, TupleType([self.fx.b], fallback=self.fx.a))

    # IDEA: Maybe add these test cases (they are tested pretty well in type
    #       checker tests already):
    #  * more interface subtyping test cases
    #  * more generic interface subtyping test cases
    #  * type variables
    #  * tuple types
    #  * None type
    #  * any type
    #  * generic function types

    def assert_subtype(self, s: Type, t: Type) -> None:
        assert is_subtype(s, t), f"{s} not subtype of {t}"

    def assert_not_subtype(self, s: Type, t: Type) -> None:
        assert not is_subtype(s, t), f"{s} subtype of {t}"

    def assert_strict_subtype(self, s: Type, t: Type) -> None:
        self.assert_subtype(s, t)
        self.assert_not_subtype(t, s)

    def assert_equivalent(self, s: Type, t: Type) -> None:
        self.assert_subtype(s, t)
        self.assert_subtype(t, s)

    def assert_unrelated(self, s: Type, t: Type) -> None:
        self.assert_not_subtype(s, t)
        self.assert_not_subtype(t, s)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeSubtypeSuite(Suite):
    """Parity suite for the Rust nominal-instance `is_subtype` (Stage 3c M8b).

    Reruns the nominal-instance cases from `SubtypingSuite` with the Rust
    path active. The Rust path handles non-generic nominal subtyping and
    same-type arg checks; it returns `None` (fall through to Python) for
    generics needing `map_instance_to_supertype` substitution, protocols,
    tuples, callables, etc. Because the Python fallback runs when Rust
    returns `None`, every assertion must match the pure-Python result.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        # Build the resolver from the fixture's TypeInfos so the Rust
        # path can look up `has_base`, `mro`, `type_vars_with_variance`.
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list:
        # The fixture stores TypeInfo objects on its `*i` attributes.
        infos = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if _is_type_info(value):
                infos.append(value)
        return infos

    def test_trivial_cases(self) -> None:
        for simple in self.fx.a, self.fx.o, self.fx.b:
            assert is_subtype(simple, simple), f"{simple} not subtype of {simple}"

    def test_instance_subtyping(self) -> None:
        # B <: A (nominal, non-generic): Rust handles this.
        assert is_subtype(self.fx.b, self.fx.a)
        assert is_subtype(self.fx.a, self.fx.o)
        assert is_subtype(self.fx.b, self.fx.o)
        # A not <: D, B not <: C: Rust returns False (not protocol).
        assert not is_subtype(self.fx.a, self.fx.d)
        assert not is_subtype(self.fx.b, self.fx.c)

    def test_same_type_no_args_is_subtype(self) -> None:
        # A <: A, object <: object: Rust handles same-type, no args.
        assert is_subtype(self.fx.a, self.fx.a)
        assert is_subtype(self.fx.o, self.fx.o)

    def test_generic_same_type_same_args(self) -> None:
        # G[A] <: G[A] (same type, same args): Rust handles the
        # same-type fast path (no map_instance_to_supertype needed).
        assert is_subtype(self.fx.ga, self.fx.ga)
        assert is_subtype(self.fx.hab, self.fx.hab)

    def test_generic_different_args_invariant_not_subtype(self) -> None:
        # G[A] not <: G[B] (invariant): Rust handles same-type arg check.
        assert not is_subtype(self.fx.ga, self.fx.gb)
        assert not is_subtype(self.fx.gb, self.fx.ga)

    def test_generic_substitution_falls_through(self) -> None:
        # GS[A, B] <: G[B] needs map_instance_to_supertype (generic
        # substitution via expand_type_by_instance). The fixture's
        # TypeVars carry namespace="" (not the class fullname), so the
        # Rust substitution check (tvar.namespace == left.type_ref)
        # does not match and Rust returns None. Python falls through
        # and computes the correct result. This proves the
        # strangler-fig contract: Rust's `None` doesn't change the
        # answer. Real code (class typevars with namespace=class
        # fullname) exercises the Rust substitution path.
        assert is_subtype(self.fx.gsab, self.fx.gb)
        assert not is_subtype(self.fx.gsab, self.fx.ga)

    def test_generic_substitution_with_namespaced_tvar(self) -> None:
        # Real code path: class typevars carry namespace=class.fullname.
        # Build GS[T, S] <: G[S] with namespace set on both the class's
        # defn.type_vars and the base Instance's TypeVar args. The Rust
        # path substitutes tvar.raw_id=2 (S) -> left.args[1] (B),
        # producing G[B], so GS[A, B] <: G[B] holds and GS[A, B] <:
        # G[A] does not.
        from mypy.nodes import Block, ClassDef, SymbolTable, TypeInfo
        from mypy.types import AnyType, TypeOfAny, TypeVarId, TypeVarType

        def make_class(name, *, bases, typevars):
            defn = ClassDef(name, Block([]), None, [])
            defn.fullname = name
            defn.type_vars = [
                TypeVarType(
                    n,
                    n,
                    TypeVarId(i, namespace=name),
                    [],
                    self.fx.o,
                    AnyType(TypeOfAny.from_omitted_generics),
                    variance=INVARIANT,
                )
                for i, n in enumerate(typevars, 1)
            ]
            info = TypeInfo(SymbolTable(), defn, name)
            info.bases = bases
            # mro must include base type infos so has_base() works
            # (nodes.py:4140 walks mro by fullname). Real TypeInfo
            # mro is built by calculate_mro(), but for this test we
            # assemble it manually.
            mro = [info]
            for base in bases:
                if isinstance(base, Instance):
                    mro.extend(base.type.mro)
            if self.fx.oi not in mro:
                mro.append(self.fx.oi)
            info.mro = mro
            return info

        # G[T] with T`1 namespace="ns.G"
        gi = make_class("ns.G", bases=[], typevars=["T"])
        # GS[T, S] <: G[S], base arg references GS's S (raw_id=2)
        s_tvar = TypeVarType(
            "S",
            "S",
            TypeVarId(2, namespace="ns.GS"),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
            variance=INVARIANT,
        )
        gsi = make_class(
            "ns.GS",
            bases=[Instance(gi, [s_tvar])],
            typevars=["T", "S"],
        )
        gsab = Instance(gsi, [self.fx.a, self.fx.b])
        gb = Instance(gi, [self.fx.b])
        ga = Instance(gi, [self.fx.a])
        # Rebuild resolver so Rust sees the new TypeInfos' bases blobs.
        # Must include the fixture's TypeInfos (A, B, object) so the
        # recursive check_type_parameter calls (is_subtype(B, B)) can
        # resolve the Instance type_refs.
        from mypy.subtypes import _set_native_subtype_resolver

        all_infos = [gi, gsi] + self._collect_type_infos()
        resolver = _type_kernel.build_native_resolver(all_infos, [])
        _set_native_subtype_resolver(resolver)
        assert is_subtype(gsab, gb)
        assert not is_subtype(gsab, ga)


def _is_type_info(value: object) -> bool:
    """True if `value` is a `mypy.nodes.TypeInfo` instance."""
    from mypy.nodes import TypeInfo

    return isinstance(value, TypeInfo)
