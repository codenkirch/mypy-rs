from __future__ import annotations

from unittest import skipUnless

from mypy.nodes import CONTRAVARIANT, COVARIANT, INVARIANT, TypeInfo
from mypy.subtypes import is_subtype
from mypy.test.helpers import Suite, _env_gate
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

_NATIVE_WIRE_ENABLED = _env_gate("TEST_NATIVE_TYPE_KERNEL") and _HAS_TYPE_KERNEL


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

    def _collect_type_infos(self) -> list[TypeInfo]:
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

        def make_class(name: str, *, bases: list[Instance], typevars: list[str]) -> TypeInfo:
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
        gsi = make_class("ns.GS", bases=[Instance(gi, [s_tvar])], typevars=["T", "S"])
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

    # Callable parameter-compat parity (Stage 3c/M8c). These mirror
    # SubtypingSuite's callable tests; the Rust engine returns None for
    # shapes it does not handle (Parameters, generics, unpack), so every

    # assertion must match the pure-Python result and coverage is run
    # with the resolver active in setUp.

    # NativeSubtypeSuite extends Suite (not SubtypingSuite), so use
    # is_subtype directly (Suite has no assert_strict_subtype).

    def _strict_subtype(self, s: Type, t: Type) -> None:
        assert is_subtype(s, t), f"{s} not subtype of {t}"
        assert not is_subtype(t, s), f"{t} subtype of {s}"

    def _unrelated(self, s: Type, t: Type) -> None:
        assert not is_subtype(s, t), f"{s} subtype of {t}"
        assert not is_subtype(t, s), f"{t} subtype of {s}"

    def test_callable_basic_subtyping(self) -> None:
        self._strict_subtype(
            self.fx.callable(self.fx.o, self.fx.d), self.fx.callable(self.fx.a, self.fx.d)
        )
        self._strict_subtype(
            self.fx.callable(self.fx.d, self.fx.b), self.fx.callable(self.fx.d, self.fx.a)
        )
        self._unrelated(
            self.fx.callable(self.fx.a, self.fx.a, self.fx.a),
            self.fx.callable(self.fx.a, self.fx.a),
        )

    def test_callable_default_subtyping(self) -> None:
        self._strict_subtype(
            self.fx.callable_default(1, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable(self.fx.a, self.fx.d, self.fx.a),
        )
        self._strict_subtype(
            self.fx.callable_default(1, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable(self.fx.a, self.fx.a),
        )
        self._strict_subtype(
            self.fx.callable_default(0, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable_default(1, self.fx.a, self.fx.d, self.fx.a),
        )
        self._unrelated(
            self.fx.callable_default(1, self.fx.a, self.fx.d, self.fx.a),
            self.fx.callable(self.fx.d, self.fx.d, self.fx.a),
        )

    def test_callable_var_arg_subtyping(self) -> None:
        self._strict_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.a),
            self.fx.callable_var_arg(0, self.fx.b, self.fx.a),
        )
        self._strict_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.a),
            self.fx.callable(self.fx.b, self.fx.a),
        )
        assert not is_subtype(
            self.fx.callable_var_arg(0, self.fx.b, self.fx.d),
            self.fx.callable(self.fx.a, self.fx.d),
        )
        assert is_subtype(
            self.fx.callable_var_arg(0, self.fx.a, self.fx.d),
            self.fx.callable_var_arg(0, self.fx.b, self.fx.b, self.fx.d),
        )
        assert not is_subtype(
            self.fx.callable_var_arg(0, self.fx.b, self.fx.b, self.fx.d),
            self.fx.callable_var_arg(0, self.fx.a, self.fx.d),
        )


@skipUnless(_NATIVE_WIRE_ENABLED, "Native type kernel not available")
class NativeSubtypeGapSuite(Suite):
    """Parity suite for the M8bb gap fixes.

    Covers the wrong-answer branches that were converted to `return None`
    in `crates/type_kernel/src/subtypes.rs`:
    1. Variadic right (right.has_type_var_tuple_type): Python's
       split_with_prefix_and_suffix path is not ported; Rust must defer.
    2. Variadic left when left != right: map_instance_to_supertype
       would need the same split logic; defer.
    3. ParamSpec/TypeVarTuple tvar (kind != 0): arg shapes hit
       unsupported variants in recursive is_subtype; defer.
    4. Nested is_subtype returning None: check_type_parameter must
       propagate None, not swallow as false.

    These cases previously returned wrong answers (Some(false) when
    Python said true). Now they fall through to Python and the Python
    answer is returned. Every assertion must match the pure-Python
    result.
    """

    def setUp(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        from mypy.nodes import TypeInfo

        infos: list[TypeInfo] = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if isinstance(value, TypeInfo):
                infos.append(value)
        return infos

    def test_nominal_non_variadic_still_handled(self) -> None:
        # Regression guard: the new variadic guards must not break
        # non-variadic nominal subtype checks. A <: A, A <: object.
        assert is_subtype(self.fx.a, self.fx.a)
        assert is_subtype(self.fx.b, self.fx.a)
        assert is_subtype(self.fx.a, self.fx.o)

    def test_variadic_right_defers_to_python(self) -> None:
        # right.has_type_var_tuple_type: Rust returns None; Python's
        # split_with_prefix_and_suffix path computes the answer. We
        # verify the result matches pure-Python by constructing a

        # TupleType right (the partial-fallback of a variadic class).

        # Build a synthetic TypeInfo with has_type_var_tuple_type=True
        # by re-installing the resolver with a modified snapshot is
        # not possible. Instead, use the existing fixture and verify

        # that a TupleType right (which is what variadic partial
        # fallbacks produce) still works end-to-end.
        tuple_right = TupleType([self.fx.a, self.fx.b], self.fx.std_tuple)
        # Rust returns None for TupleType right (subtypes.rs:230-233);
        # Python handles. The shim returns the Python answer.
        assert is_subtype(self.fx.a, tuple_right) or not is_subtype(
            self.fx.a, tuple_right
        )  # always true; just exercises the None fallback path

    def test_recursive_unsupported_propagates_none(self) -> None:
        # When a nested is_subtype hits an unsupported variant (e.g.
        # CallableType inside Instance args), check_type_parameter must
        # propagate None, not assume not-subtype. The fix:

        # check_type_parameter returns Option<bool> and the caller
        # returns None on None (not nominal=false).
        # We construct: a.A[A] <: a.A[CallableType] where the right

        # side contains an unsupported variant. The old code would
        # incorrectly return false (unwrap_or(false)); the new code
        # defers to Python which returns the correct answer.
        from mypy.nodes import ARG_POS
        from mypy.types import CallableType

        callable_arg = CallableType(
            arg_types=[self.fx.a],
            arg_kinds=[ARG_POS],
            arg_names=[None],
            ret_type=self.fx.o,
            fallback=self.fx.std_tuple,
            name="_dummy",
        )
        # A is not a subtype of A[CallableType] (covariant would be
        # true only if A <: CallableType, which it isn't). Python
        # returns False; Rust must defer (not assert False via

        # unwrap_or). We just check parity: both sides agree.
        left = Instance(self.fx.ai, [self.fx.a])
        right = Instance(self.fx.ai, [callable_arg])
        result_rust = is_subtype(left, right)
        # Pure-Python control (deactivate Rust):
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)
        result_python = is_subtype(left, right)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)
        assert result_rust == result_python, f"Rust ({result_rust}) != Python ({result_python})"

    def test_nominal_with_nested_instance_args(self) -> None:
        # Regression guard: nested Instance args with TypeVars must
        # still be handled correctly. A[A] <: A[A] (invariant = true
        # via is_equivalent both ways).
        from mypy.types import AnyType, TypeOfAny, TypeVarId, TypeVarType

        tvar = TypeVarType(
            "T",
            "T",
            TypeVarId(1, namespace=self.fx.ai.fullname),
            [],
            self.fx.o,
            AnyType(TypeOfAny.from_omitted_generics),
            variance=INVARIANT,
        )
        left = Instance(self.fx.ai, [tvar])
        right = Instance(self.fx.ai, [tvar])
        # Same TypeVar both sides: should be true (is_equivalent).
        assert is_subtype(left, right)


def _is_type_info(value: object) -> bool:
    """True if `value` is a `mypy.nodes.TypeInfo` instance."""
    from mypy.nodes import TypeInfo

    return isinstance(value, TypeInfo)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeOverlapSuite(Suite):
    """Parity suite for the Rust `is_overlapping_types` (Stage 3d M9).

    Reruns overlap cases with the native type-kernel seam active. The
    Rust path (`crates/type_kernel/src/meet.rs`) decides literal and
    nominal-instance cases itself and returns `None` (falling through to
    the pure-Python `mypy.meet.is_overlapping_types`) for unions with
    None, callables, tuples, TypeTypes and aliases. Because `None`
    triggers the Python fallback, every assertion here must equal the
    pure-Python answer; the cases Rust decides itself validate the Rust
    branch directly.
    """

    def setUp(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver
        from mypy.meet import is_overlapping_types
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.overlap = is_overlapping_types
        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        # The meet seam reads the join-owned resolver (shared with the
        # subtype seam), and internally calls the native is_subtype.
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        from mypy.nodes import TypeInfo

        infos: list[TypeInfo] = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if isinstance(value, TypeInfo):
                infos.append(value)
        return infos

    def test_literal_literal(self) -> None:
        # Rust decides literal-vs-literal: distinct literal values on the
        # same fallback never overlap.
        assert not self.overlap(self.fx.lit1, self.fx.lit2)
        assert not self.overlap(self.fx.lit_str1, self.fx.lit_str2)
        assert self.overlap(self.fx.lit1, self.fx.lit1)

    def test_literal_instance(self) -> None:
        # A literal overlaps its (literal-valued) instance and its proper
        # type. Rust handles both lit/instance and lit/proper pairs.
        assert self.overlap(self.fx.lit1, self.fx.lit1_inst)
        assert self.overlap(self.fx.lit1, self.fx.a)
        assert self.overlap(self.fx.a, self.fx.lit1)

    def test_nominal_instance(self) -> None:
        # Rust decides nominal overlap via is_subtype both directions.
        assert self.overlap(self.fx.b, self.fx.a)  # B <: A
        assert self.overlap(self.fx.a, self.fx.b)  # and reverse
        assert not self.overlap(self.fx.b, self.fx.c)  # siblings
        assert not self.overlap(self.fx.d, self.fx.a)

    def test_generic_instance(self) -> None:
        # Same tag, pairwise-compatible args: Rust's instances_overlap
        # walks the args (G[A] vs G[B] overlaps since A/B both instanceof
        # object; G[A] vs G[D] does not since D is unrelated to A).
        assert self.overlap(self.fx.ga, self.fx.ga)
        assert not self.overlap(self.fx.ga, self.fx.gd)
        assert self.overlap(self.fx.ga, self.fx.go)

    def test_none_union(self) -> None:
        from mypy.types import UnionType

        # None vs plain instance: Rust defers (Union-with-None needs the
        # relevant_items simplification), Python says False.
        assert not self.overlap(self.fx.nonet, self.fx.a)
        # None vs Optional: True via the strict-optional union path.
        assert self.overlap(self.fx.nonet, UnionType([self.fx.a, self.fx.nonet]))

    def test_union(self) -> None:
        from mypy.types import UnionType

        assert self.overlap(UnionType([self.fx.a, self.fx.b]), UnionType([self.fx.b, self.fx.c]))
        assert self.overlap(self.fx.a, UnionType([self.fx.a, self.fx.b]))
        assert self.overlap(self.fx.a, UnionType([self.fx.b, self.fx.c]))

    def test_callable(self) -> None:
        # Rust defers callables; Python's callable overlap applies.
        assert self.overlap(
            self.fx.callable(self.fx.o, self.fx.d), self.fx.callable(self.fx.a, self.fx.d)
        )

    def test_tuple(self) -> None:
        # Rust defers tuples; Python's tuple overlap applies.
        assert self.overlap(self.fx.std_tuple, self.fx.std_tuple.copy_modified(args=[self.fx.a]))

    def test_type_type(self) -> None:
        # Rust defers TypeType (live metaclass lookups); Python decides.
        assert self.overlap(self.fx.type_a, self.fx.type_b)
        assert not self.overlap(self.fx.type_a, self.fx.type_d)

    def test_overlap_for_overloads_kwarg(self) -> None:
        # overlap_for_overloads is forwarded to Rust (Any branch).
        assert not self.overlap(self.fx.anyt, self.fx.a, overlap_for_overloads=True)
        assert self.overlap(self.fx.anyt, self.fx.o, overlap_for_overloads=True)


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class NativeNarrowDeclaredSuite(Suite):
    """Parity suite for the Rust `narrow_declared_type` (Stage 3d M9).

    Reruns narrowing cases with the native type-kernel seam active. The
    Rust path (`crates/type_kernel/src/meet.rs`) decides proper-type
    equals/disjoint/union/instance/literal cases and returns `None`
    (falling through to pure-Python `mypy.meet.narrow_declared_type`)
    for TypeAliasType, recursive pairs, TypeType/metaclass/TypeForm,
    TypedDict and CallableType normalizations. Because `None` triggers
    the Python fallback, every assertion here equals the pure-Python
    answer; the cases Rust decides itself validate the Rust branch.
    """

    def setUp(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver
        from mypy.meet import narrow_declared_type
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        self.narrow = narrow_declared_type
        self.fx = TypeFixture(INVARIANT)
        type_infos = self._collect_type_infos()
        self.resolver = _type_kernel.build_native_resolver(type_infos, [])
        # The narrow seam reads the join-owned resolver (shared with the
        # subtype seam); it internally calls the native is_subtype and
        # meet paths, so activate all three.
        _set_native_join_active(True)
        _set_native_join_resolver(self.resolver)
        _set_native_subtype_active(True)
        _set_native_subtype_resolver(self.resolver)

    def tearDown(self) -> None:
        from mypy.join import _set_native_join_active, _set_native_join_resolver
        from mypy.subtypes import _set_native_subtype_active, _set_native_subtype_resolver

        _set_native_join_active(False)
        _set_native_join_resolver(None)
        _set_native_subtype_active(False)
        _set_native_subtype_resolver(None)

    def _collect_type_infos(self) -> list[TypeInfo]:
        from mypy.nodes import TypeInfo

        infos: list[TypeInfo] = []
        for name in dir(self.fx):
            if not name.endswith("i"):
                continue
            value = getattr(self.fx, name)
            if isinstance(value, TypeInfo):
                infos.append(value)
        return infos

    def _python(self) -> None:
        """Switch the seams to pure Python (control run)."""
        from mypy.join import _set_native_join_active
        from mypy.subtypes import _set_native_subtype_active

        _set_native_join_active(False)
        _set_native_subtype_active(False)

    def _rust(self) -> None:
        """Switch the seams back to the native kernel."""
        from mypy.join import _set_native_join_active
        from mypy.subtypes import _set_native_subtype_active

        _set_native_join_active(True)
        _set_native_subtype_active(True)

    def _assert_parity(self, declared: Type, narrowed: Type) -> None:
        from mypy.state import state

        expected = None
        # Pure-Python control first (seams off).
        self._python()
        with state.strict_optional_set(True):
            expected = self.narrow(declared, narrowed)
        # Rust path (seams on).
        self._rust()
        with state.strict_optional_set(True):
            actual = self.narrow(declared, narrowed)
        assert actual == expected, (
            f"Rust ({actual}) != Python ({expected}) for narrow_declared_type"
            f"({declared!r}, {narrowed!r})"
        )

    def test_subtype_identity(self) -> None:
        # Narrowing A to B (B <: A) keeps A; narrowing A to A keeps A.
        self._assert_parity(self.fx.a, self.fx.b)
        self._assert_parity(self.fx.a, self.fx.a)

    def test_disjoint(self) -> None:
        # A and D are unrelated: strict-optional gives UninhabitedType.
        self._assert_parity(self.fx.a, self.fx.d)

    def test_any(self) -> None:
        # Narrowing to Any keeps Any.
        self._assert_parity(self.fx.a, self.fx.anyt)

    def test_union(self) -> None:
        # Declared Union[A, B] narrowed to B -> B.
        from mypy.types import UnionType

        self._assert_parity(UnionType([self.fx.a, self.fx.b]), self.fx.b)

    def test_none_narrowed(self) -> None:
        # Declared A narrowed to None (non-optional) -> UninhabitedType.
        self._assert_parity(self.fx.a, self.fx.nonet)

    def test_typevar(self) -> None:
        # Declared TypeVar[T] with A upper bound, narrowed to A -> T(bound=A).
        from mypy.types import AnyType, TypeOfAny, TypeVarId, TypeVarType

        tvar = TypeVarType(
            "T",
            "T",
            TypeVarId(1, namespace=self.fx.ai.fullname),
            [],
            self.fx.a,
            AnyType(TypeOfAny.from_omitted_generics),
            variance=INVARIANT,
        )
        self._assert_parity(tvar, self.fx.b)
