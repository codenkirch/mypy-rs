"""G2.2: the `Var` binder-key handle translation and its differential.

`mypy/nodes_mirror` mints a store handle for the `Var` a `RefExpr` binds
(`_capture_ref` pins the target), and `mypy/literals.py` emits that handle in
the narrowing key (`("Var", handle)`) when the var-key flip serves, keeping
the live object key when it defers. `extract_var_from_literal_hash` reverses
the handle through `rust_node_mirror_object_of`. Mode 0 (the default) keeps
the live object key, mode 1 emits the handle, mode 2 emits it and compares
every translated key against the live-key computation.

The suite has four jobs:

1. Pin the translation contract: the live key in mode 0, the handle in modes
   1/2, and the deferral when a `Var` was never pinned (an unresolvable key
   must stay the live object, never key a lookup on the wrong node).
2. Pin the soundness invariants: `object_of(handle)` returns the identical
   object, no two live `Var`s share a handle, and mint order does not change
   key equality (equality is the object, not the ordinal).
3. Keep its own green result falsifiable. `MYPY_TK_VAR_KEY_CONTROL=share`
   makes every `Var` translate to one victim's handle, so the identity
   assertion must fail on a mutated run while the positive path stays green.
4. Report the mode-2 counters, whose `deferred == 0` is what proves one key
   space rather than a mix of handle keys and live-object keys.
"""

from __future__ import annotations

import os
from typing import Any

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from unittest import skipUnless

from mypy.nodes import GDEF, NameExpr, Var
from mypy.test.helpers import Suite
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED

# Harness mutations, so a mutation run needs no edit and the unmutated run is
# the control: "" (none) or "share" (every `Var` translates to one handle).
_CONTROL_ENV = "MYPY_TK_VAR_KEY_CONTROL"


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class VarKeyTranslationSuite(Suite):
    """The `Var`-key translation contract, its invariants, and its counters."""

    def setUp(self) -> None:
        from mypy import nodes_mirror

        self._m = nodes_mirror
        self._k = _type_kernel
        self._m.activate(audit=True)
        self._m.set_var_key_flip(0)
        self._m.reset(clear_counts=True)
        self.control = os.environ.get(_CONTROL_ENV, "")
        if self.control not in ("", "share"):
            raise AssertionError(f"unknown {_CONTROL_ENV}={self.control!r}")
        self._install_control()

    def tearDown(self) -> None:
        self._m.set_var_key_flip(0)
        self._m._uninstall_var_key_hooks()
        self._m.reset(clear_counts=True)

    def _install_control(self) -> None:
        """Apply the harness mutation, if any (see `_CONTROL_ENV`).

        `share` replaces the translate hook so every `Var` emits one pinned
        victim's handle: the key still translates, but it no longer identifies
        its own `Var`, which is the invariant the identity test asserts. It is
        re-applied after every `set_var_key_flip`, which installs the real
        hook.
        """
        if self.control != "share":
            return
        from mypy import literals

        if getattr(self, "_victim", None) is None:
            self._victim = Var("victim")
            self._pin(self._victim)
        handle = self._k.rust_node_mirror_handle_of(self._victim)
        literals.set_var_key_hooks(lambda _var: handle, self._m._resolve_var_key)

    # -- fixtures --

    def _pin(self, var: Var) -> None:
        """Mint the capture-time pin the production `RefExpr` write mints."""
        expr = NameExpr(var.name)
        expr.kind = GDEF
        expr.node = var

    def _key(self, var: Var, mode: int) -> Any:
        """The narrowing key for a `NameExpr` bound to `var` in `mode`."""
        self._m.set_var_key_flip(mode)
        self._install_control()
        expr = NameExpr(var.name)
        expr.node = var
        from mypy.literals import literal_hash

        return literal_hash(expr)

    def _unpinned_key(self, var: Var, mode: int) -> Any:
        """A key for a `Var` no capture pinned: the live object must stay."""
        self._m.set_var_key_flip(mode)
        self._install_control()
        expr = NameExpr(var.name)
        # Bypass the patched `__setattr__` so the binding write never pins.
        self._m._ORIG_SETATTR(expr, "node", var)
        from mypy.literals import literal_hash

        return literal_hash(expr)

    def _extract(self, key: Any) -> Var | None:
        from mypy.literals import extract_var_from_literal_hash

        return extract_var_from_literal_hash(key)

    # -- the translation contract --

    def test_mode_zero_keeps_the_live_object_key(self) -> None:
        var = Var("x")
        self._pin(var)
        key = self._key(var, 0)
        assert key == ("Var", var), "mode 0 must keep the live object key"
        counters = self._m.var_key_counters()
        assert counters["served"] == 0
        assert counters["deferred_off"] == 1
        assert counters["compared"] == 0

    def test_a_pinned_var_serves_its_handle(self) -> None:
        var = Var("x")
        self._pin(var)
        handle = self._k.rust_node_mirror_handle_of(var)
        assert handle is not None
        key = self._key(var, 1)
        assert key == ("Var", handle), "mode 1 must serve the store handle"
        assert self._k.rust_node_mirror_object_of(handle) is var
        assert self._extract(key) is var

    def test_mode_two_compares_every_translated_key(self) -> None:
        var = Var("x")
        self._pin(var)
        key = self._key(var, 2)
        assert key[0] == "Var" and isinstance(key[1], int)
        counters = self._m.var_key_counters()
        assert counters["served"] == 1
        assert counters["compared"] == counters["served"], "mode 2 must compare every key"
        assert counters["mismatched"] == 0
        assert counters["deferred"] == 0, "a pinned Var must not defer"

    def test_an_unpinned_var_keeps_the_live_key(self) -> None:
        unpinned = Var("u")
        # The constructor's own capture gave it an identity handle, but no
        # `RefExpr` binding pinned it, so the handle cannot resolve it back.
        handle = self._k.rust_node_mirror_handle_of(unpinned)
        assert handle is not None
        assert self._k.rust_node_mirror_object_of(handle) is None, "no pin to resolve"
        key = self._unpinned_key(unpinned, 1)
        assert key == ("Var", unpinned), "an unpinned Var must keep the live key"
        assert self._extract(key) is unpinned
        counters = self._m.var_key_counters()
        assert counters["served"] == 0
        assert counters["deferred_unrecorded"] == 1
        assert counters["deferred"] == 1

    def test_extract_accepts_both_key_shapes(self) -> None:
        var = Var("x")
        self._pin(var)
        handle = self._k.rust_node_mirror_handle_of(var)
        assert self._extract(("Var", var)) is var
        assert self._extract(("Var", handle)) is var
        # A handle that no longer resolves leaves the key opaque, exactly as
        # a non-Var element does - never a wrong Var.
        other = Var("y")
        self._pin(other)
        other_handle = self._k.rust_node_mirror_handle_of(other)
        assert other_handle != handle
        self._m.reset()  # drop the pins; the identity registry stays valid
        assert self._extract(("Var", handle)) is None
        assert self._extract(("Var", other_handle)) is None

    # -- the soundness invariants --

    def test_object_of_returns_the_identical_object(self) -> None:
        var = Var("x")
        self._pin(var)
        handle = self._k.rust_node_mirror_handle_of(var)
        assert handle is not None
        resolved = self._k.rust_node_mirror_object_of(handle)
        assert resolved is var, "the read-back must be identity, not equality"

    def test_no_two_live_vars_share_a_handle(self) -> None:
        a, b = Var("a"), Var("b")
        self._pin(a)
        self._pin(b)
        ha = self._k.rust_node_mirror_handle_of(a)
        hb = self._k.rust_node_mirror_handle_of(b)
        assert ha is not None and hb is not None
        assert ha != hb, "injectivity: distinct Vars, distinct handles"
        assert self._k.rust_node_mirror_object_of(ha) is a
        assert self._k.rust_node_mirror_object_of(hb) is b
        # The keys are distinct too, so a dict cannot conflate them.
        assert self._key(a, 1) != self._key(b, 1)

    def test_mint_order_does_not_change_key_equality(self) -> None:
        a, b, c = Var("a"), Var("b"), Var("c")
        first = self._key(a, 1)
        # Mint unrelated Vars in between; the key for `a` must be unchanged,
        # because equality is the object, not the mint ordinal.
        self._pin(b)
        self._pin(c)
        again = self._key(a, 1)
        assert again == first, "mint order must not change a Var's key"
        assert self._key(b, 1) != first

    def test_a_translated_key_is_a_usable_dict_key(self) -> None:
        var = Var("x")
        self._pin(var)
        key = self._key(var, 2)
        live_key = ("Var", var)
        # The translated key is hashable and looks up its own entry, and the
        # live-key computation resolves through the same Var.
        d = {key: var}
        assert d[key] is var
        assert d.get(key) is self._extract(live_key)
        assert {live_key: 1}.get(live_key) == 1

    # -- the gate's own state handling (#1779) --

    def test_an_out_of_range_mode_is_refused(self) -> None:
        try:
            self._m.set_var_key_flip(3)
        except ValueError:
            return
        raise AssertionError("mode 3 must be refused, not silently accepted")

    def test_the_raw_seam_refuses_an_out_of_range_mode(self) -> None:
        try:
            self._k.rust_node_mirror_set_var_key_mode(3)
        except ValueError:
            return
        raise AssertionError("the raw seam must refuse mode 3")

    def test_a_mode_set_before_registration_is_still_readable(self) -> None:
        saved_active = self._m._active
        saved_kernel = self._m._kernel_mod
        try:
            self._m._active = False
            self._m._kernel_mod = None
            assert self._m.set_var_key_flip(1) == 1
            assert self._m.var_key_flip() == 1, "the set mode must be readable back"
            assert self._m.var_key_counters() != {}, "the counters must see the store"
        finally:
            self._m.set_var_key_flip(0)
            self._m._active = saved_active
            self._m._kernel_mod = saved_kernel

    def test_a_malformed_mode_env_fails_activate_with_no_state_change(self) -> None:
        env_name = self._m._VAR_KEY_FLIP_ENV
        saved_active = self._m._active
        saved_env = os.environ.get(env_name)
        try:
            for bad in ("nonsense", "7"):
                self._m._active = False
                os.environ[env_name] = bad
                raised = False
                try:
                    self._m.activate()
                except ValueError:
                    raised = True
                assert raised, f"{env_name}={bad!r} must be refused"
                assert self._m._active is False, "a refused activate must change no state"
            os.environ.pop(env_name, None)
            assert self._m.activate() is True, "the retry must not be swallowed"
            assert self._m.var_key_flip() == 0
        finally:
            self._m._active = saved_active
            if saved_env is None:
                os.environ.pop(env_name, None)
            else:
                os.environ[env_name] = saved_env

    # -- the invariant the control drives red --

    def test_a_translated_key_resolves_to_its_own_var(self) -> None:
        var = Var("x")
        other = Var("y")
        self._pin(other)
        self._pin(var)
        key = self._key(var, 2)
        resolved = self._extract(key)
        # A shared or recycled handle makes this resolve to `other`; the
        # `share` control must turn this assertion red.
        assert resolved is var, "the translated key must resolve to its own Var"
        assert resolved is not other
