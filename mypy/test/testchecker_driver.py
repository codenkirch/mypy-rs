"""H1: the checker-traversal driver's counter surface and its gates.

The H1 counter store (`crates/type_kernel/src/checker_driver.rs`) is the
evidence instrument for the driver that lands with slice 2 (issue #1861,
plan `docs/plans/2026-09-18-phase-h1-implementation-plan.md` section 4.2).
The cross-run differential compares gate-on against gate-off on the same
corpus, so both paths must count the same events under the same names:
the Rust `DriverCounters9` field order and this module's `EVENT_NAMES`
must stay in lockstep. This suite pins that, every gate contract, and the
counter discipline itself:

1. The Rust half: modes 0/1/2 only, an unknown mode rejected loudly (a
   mode no path implements must never select silently), each record kind
   bumps exactly its own counter, reset clears counters but keeps mode.
2. The Python half: `EVENT_NAMES` matches the Rust tuple order, the
   `record_*` helpers bump their own counter and no other, and
   `sessionfinish()` reports both halves under those names.
3. The gates: `Options.native_checker_traversal` defaults off and is not
   in `OPTIONS_AFFECTING_CACHE` (it must never invalidate a cache); the
   harness `_env_gate` decodes 0/1 and rejects unknown spellings; the
   production wiring records its decision on every call, the off case
   included, so no later build inherits a stale mode.

The counters are evidence-critical per AGENTS.md: a store that can
silently undercount is worse than no store, so every assertion here has
a sibling that makes it fail (the negative-control discipline the plan's
section 3 requires of the whole lane).
"""

from __future__ import annotations

import json
import os
import tempfile
from unittest import skipUnless

try:
    import type_kernel as _type_kernel
except ImportError:
    _type_kernel = None  # type: ignore[assignment]

from mypy import checker_driver
from mypy.options import OPTIONS_AFFECTING_CACHE, Options
from mypy.test.helpers import Suite, _env_gate
from mypy.test.testtypes import _NATIVE_WIRE_ENABLED


@skipUnless(_NATIVE_WIRE_ENABLED, "requires TEST_NATIVE_TYPE_KERNEL=1 and type_kernel ext")
class CheckerDriverCountersSuite(Suite):
    """The H1 counter store: its Rust half, its Python half, its gates."""

    def setUp(self) -> None:
        checker_driver.reset_stats()
        checker_driver.set_driver_mode(0)
        checker_driver.set_production_checker_traversal(False)
        self.addCleanup(self._restore)

    def _restore(self) -> None:
        checker_driver.reset_stats()
        checker_driver.set_driver_mode(0)
        checker_driver.set_production_checker_traversal(False)

    # --- the Rust half ----------------------------------------------------

    def test_rust_mode_defaults_to_zero(self) -> None:
        assert checker_driver.driver_mode() == 0

    def test_rust_mode_accepts_0_1_2(self) -> None:
        assert checker_driver.set_driver_mode(1) == 1
        assert checker_driver.set_driver_mode(2) == 2
        assert checker_driver.set_driver_mode(0) == 0

    def test_rust_mode_rejects_unknown_loudly(self) -> None:
        # A mode no driver path implements must never select silently.
        try:
            checker_driver.set_driver_mode(3)
        except ValueError:
            pass
        else:
            raise AssertionError("set_driver_mode(3) must raise, not pass")

    def test_rust_record_bumps_exactly_its_counter(self) -> None:
        # The tautology control: if `record` bumped every counter, or none,
        # this fails, so the counter reading is not a constant assertion.
        checker_driver.record_driver_event(1)  # statements_dispatched
        counters = checker_driver.rust_counters()
        assert counters is not None
        assert counters["statements_dispatched"] == 1
        for name in checker_driver.EVENT_NAMES:
            if name != "statements_dispatched":
                assert counters[name] == 0, f"{name} must stay 0 after kind 1"

    def test_rust_record_covers_every_kind(self) -> None:
        for kind, name in enumerate(checker_driver.EVENT_NAMES):
            checker_driver.record_driver_event(kind)
        counters = checker_driver.rust_counters()
        assert counters is not None
        assert counters == dict.fromkeys(checker_driver.EVENT_NAMES, 1)

    def test_rust_reset_clears_counters_but_keeps_mode(self) -> None:
        checker_driver.set_driver_mode(2)
        checker_driver.record_driver_event(1)
        checker_driver.reset_stats()
        assert checker_driver.driver_mode() == 2
        counters = checker_driver.rust_counters()
        assert counters is not None
        assert counters == dict.fromkeys(checker_driver.EVENT_NAMES, 0)
        checker_driver.set_driver_mode(0)

    def test_event_names_match_the_rust_field_order(self) -> None:
        # The cross-run differential keys both halves by these names, so a
        # reorder on one side only would silently undercount the other.
        counters = checker_driver.rust_counters()
        assert counters is not None
        assert tuple(counters) == checker_driver.EVENT_NAMES

    # --- the Python half --------------------------------------------------

    def test_python_records_bump_exactly_their_counter(self) -> None:
        checker_driver.record_statements_dispatched()
        checker_driver.record_visit_block_iterations()
        checker_driver.record_statements_dispatched()
        counters = checker_driver.python_counters()
        assert counters["statements_dispatched"] == 2
        assert counters["visit_block_iterations"] == 1
        for name in checker_driver.EVENT_NAMES:
            if name not in ("statements_dispatched", "visit_block_iterations"):
                assert counters[name] == 0, f"{name} must stay 0"

    def test_reset_stats_clears_both_halves(self) -> None:
        checker_driver.record_statements_dispatched()
        checker_driver.record_driver_event(1)
        checker_driver.reset_stats()
        assert checker_driver.python_counters() == dict.fromkeys(checker_driver.EVENT_NAMES, 0)
        counters = checker_driver.rust_counters()
        assert counters is not None
        assert counters == dict.fromkeys(checker_driver.EVENT_NAMES, 0)

    def test_sessionfinish_reports_both_halves(self) -> None:
        checker_driver.record_statements_dispatched()
        checker_driver.record_driver_event(1)
        report = checker_driver.sessionfinish()
        assert set(report) == {"python", "rust"}
        python_half = report["python"]
        assert python_half is not None
        assert python_half["statements_dispatched"] == 1
        rust_half = report["rust"]
        assert rust_half is not None
        assert rust_half["statements_dispatched"] == 1

    def test_sessionfinish_dump_writes_where_the_env_asks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = os.path.join(tmp, "h1-{pid}.json")
            os.environ[checker_driver.SESSIONFINISH_ENV] = out
            self.addCleanup(os.environ.pop, checker_driver.SESSIONFINISH_ENV, None)
            checker_driver.record_statements_dispatched()
            checker_driver.sessionfinish_dump()
            with open(out.replace("{pid}", str(os.getpid()))) as f:
                dumped = json.load(f)
        assert dumped["python"]["statements_dispatched"] == 1

    def test_sessionfinish_dump_is_a_noop_without_the_env(self) -> None:
        os.environ.pop(checker_driver.SESSIONFINISH_ENV, None)
        # Must not raise and must not write anywhere.
        checker_driver.sessionfinish_dump()

    # --- the gates --------------------------------------------------------

    def test_stats_env_arms_the_python_counters(self) -> None:
        try:
            os.environ.pop(checker_driver.STATS_ENV, None)
            assert checker_driver.stats_enabled() is False
            os.environ[checker_driver.STATS_ENV] = "1"
            assert checker_driver.stats_enabled() is True
        finally:
            os.environ.pop(checker_driver.STATS_ENV, None)

    def test_option_defaults_off_and_never_affects_cache(self) -> None:
        options = Options()
        assert options.native_checker_traversal is False
        # The traversal gate must not invalidate a cache: the differential
        # compares gate states on the same cache.
        assert "native_checker_traversal" not in OPTIONS_AFFECTING_CACHE

    def test_harness_env_gate_decodes_and_rejects(self) -> None:
        try:
            os.environ["TEST_NATIVE_CHECKER_TRAVERSAL"] = "1"
            assert _env_gate("TEST_NATIVE_CHECKER_TRAVERSAL") is True
            os.environ["TEST_NATIVE_CHECKER_TRAVERSAL"] = "0"
            assert _env_gate("TEST_NATIVE_CHECKER_TRAVERSAL") is False
            os.environ["TEST_NATIVE_CHECKER_TRAVERSAL"] = "maybe"
            try:
                _env_gate("TEST_NATIVE_CHECKER_TRAVERSAL")
            except SystemExit:
                pass
            else:
                raise AssertionError("unknown gate spelling must exit, not pass")
        finally:
            os.environ.pop("TEST_NATIVE_CHECKER_TRAVERSAL", None)

    def test_production_wiring_records_every_call(self) -> None:
        # The off case must wire too: a later manager that never sees the
        # option must not inherit the previous build's mode.
        checker_driver.set_production_checker_traversal(True)
        assert checker_driver.production_checker_traversal() is True
        assert checker_driver.driver_mode() == 1
        checker_driver.set_production_checker_traversal(False)
        assert checker_driver.production_checker_traversal() is False
        assert checker_driver.driver_mode() == 0
