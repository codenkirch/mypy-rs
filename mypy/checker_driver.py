"""H1 checker-driver gate and counter mirror (#1861).

The Python half of the H1 evidence surface. The Rust driver
(``crates/type_kernel/src/checker_driver.rs``) owns the driver counters
when the traversal gate is on; on the gate-off path this module counts the
same loop events so the cross-run differential compares like with like
(issue #1861, plan `docs/plans/2026-09-18-phase-h1-implementation-plan.md`
section 4.2).

Counting is armed by ``MYPY_TK_H1_STATS``: the call sites in
``mypy/checker.py`` are guarded by a module-level boolean computed at
import, so production pays one boolean test and nothing else. The
``record_*`` functions bump unconditionally; a caller that does not want
the count simply does not call them.

The counter names mirror the Rust ``DriverCounters9`` field order
verbatim. No name may be removed or reinterpreted without updating both
sides and the plan: a name mismatch silently undercounts, which is the
"probe that can undercount is worse than no probe" failure family in
AGENTS.md.
"""

from __future__ import annotations

import json
import os
import sys

# Event names, in the same order as `DriverCounters9` in checker_driver.rs.
EVENT_NAMES = (
    "driver_entered",
    "statements_dispatched",
    "callbacks_emitted",
    "callbacks_raised",
    "bailouts_to_python_loop",
    "deferred_nodes_deferred",
    "unreachable_marked",
    "breaks_taken",
    "visit_block_iterations",
)

STATS_ENV = "MYPY_TK_H1_STATS"
SESSIONFINISH_ENV = "MYPY_TK_H1_SESSIONFINISH_OUT"

# The Python-side counters. Plain ints in a dict so the dump is a direct
# serialization and a reset cannot miss a field.
_python_counts: dict[str, int] = {name: 0 for name in EVENT_NAMES}

# What the production wiring last asked this channel to do. Mirrors the
# G-family `production_*_flip()` contract: a consumer reads the recorded
# decision, not a re-derivation from options.
_production_traversal = False

try:  # The extension may be absent in a pure-Python checkout.
    from type_kernel import (
        rust_checker_driver_counters as _rust_counters,
        rust_checker_driver_mode as _rust_mode,
        rust_checker_driver_record as _rust_record,
        rust_checker_driver_reset as _rust_reset,
        rust_checker_driver_set_mode as _rust_set_mode,
    )
except ImportError:  # pragma: no cover - exercised by the gate-off lane
    _rust_counters = None  # type: ignore[assignment]
    _rust_mode = None  # type: ignore[assignment]
    _rust_record = None  # type: ignore[assignment]
    _rust_reset = None  # type: ignore[assignment]
    _rust_set_mode = None  # type: ignore[assignment]


def stats_enabled() -> bool:
    """Whether ``MYPY_TK_H1_STATS`` arms the Python-side counters."""
    return os.environ.get(STATS_ENV) is not None


def reset_stats() -> None:
    """Zero the Python-side counters and the Rust store (mode kept)."""
    for name in EVENT_NAMES:
        _python_counts[name] = 0
    if _rust_reset is not None:
        _rust_reset()


# --- Python-side records (the gate-off path) -------------------------------


def record_driver_entered() -> None:
    _python_counts["driver_entered"] += 1


def record_statements_dispatched() -> None:
    _python_counts["statements_dispatched"] += 1


def record_visit_block_iterations() -> None:
    _python_counts["visit_block_iterations"] += 1


def record_deferred_nodes_deferred() -> None:
    _python_counts["deferred_nodes_deferred"] += 1


def record_unreachable_marked() -> None:
    _python_counts["unreachable_marked"] += 1


def record_breaks_taken() -> None:
    _python_counts["breaks_taken"] += 1


def python_counters() -> dict[str, int]:
    """A copy of the Python-side counters, keyed by event name."""
    return dict(_python_counts)


# --- Rust-side records (the gate-on path, slice 2) -------------------------


def driver_mode() -> int:
    """The Rust driver's serving mode (0 off, 1 drive, 2 drive + compare)."""
    return _rust_mode() if _rust_mode is not None else 0


def set_driver_mode(mode: int) -> int:
    """Set the Rust driver mode; only 0, 1 and 2 exist."""
    if _rust_set_mode is None:
        raise RuntimeError("checker driver extension (type_kernel) is not available")
    return _rust_set_mode(mode)


def record_driver_event(kind: int) -> None:
    """Bump one Rust driver counter by kind index (see the ``record`` table)."""
    if _rust_record is not None:
        _rust_record(kind)


def rust_counters() -> dict[str, int] | None:
    """The Rust driver's counters keyed by event name, or None without the ext."""
    if _rust_counters is None:
        return None
    return dict(zip(EVENT_NAMES, _rust_counters()))


# --- production wiring -----------------------------------------------------


def set_production_checker_traversal(serve: bool) -> int:
    """Record, and wire into Rust, the H1 mode a production build runs.

    ``serve`` is the build wiring's decision (``Options.native_checker_traversal``
    once slice 2 consumes it). Called on every manager, so a later build
    cannot inherit a mode from an earlier one in the process (the
    aststrip/var-key rule). Returns the resulting Rust mode.
    """
    global _production_traversal
    _production_traversal = serve
    if _rust_set_mode is None:
        return 0
    return _rust_set_mode(1 if serve else 0)


def production_checker_traversal() -> bool:
    """Whether the production wiring last asked this channel to drive."""
    return _production_traversal


# --- session evidence ------------------------------------------------------


def sessionfinish() -> dict[str, dict[str, int] | None]:
    """The H1 driver evidence of one finished session.

    Two sections keyed by the same event names: ``python`` (the gate-off
    path's counters) and ``rust`` (the driver's counters, or None when the
    extension is absent). The cross-run differential compares the two.
    """
    return {"python": python_counters(), "rust": rust_counters()}


def sessionfinish_dump() -> None:
    """Write the session report where ``MYPY_TK_H1_SESSIONFINISH_OUT`` asks.

    Called at the end of ``mypy.build.build``; no-op without the env (and
    ``{pid}`` expands to the process id so each xdist worker writes its
    own). Never raises: the call site sits in a ``finally`` whose job is to
    preserve the in-flight build exception.
    """
    out = os.environ.get(SESSIONFINISH_ENV)
    if not out:
        return
    out = out.replace("{pid}", str(os.getpid()))
    try:
        with open(out, "w") as f:
            json.dump(sessionfinish(), f, indent=1, sort_keys=True)
    except Exception as err:
        print(f"checker-driver: sessionfinish dump failed: {err}", file=sys.stderr)
