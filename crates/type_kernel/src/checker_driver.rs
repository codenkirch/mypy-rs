//! H1 checker-driver counter store (#1861).
//!
//! The Rust driver that lands in slice 2 will own the build-path
//! `check_first_pass` / `visit_block` / `check_second_pass` loops, and the
//! gate-off Python path will count the **same** events through the
//! `MYPY_TK_H1_STATS` env. The cross-run differential (issue #1861, plan
//! `docs/plans/2026-09-18-phase-h1-implementation-plan.md` §4.2) requires
//! that both sides report the same totals for the same corpus: a Rust
//! driver that reports `statements_dispatched = 90_000` while the gate-off
//! Python path reports `91_000` is a differential that is not comparing
//! what it claims. This module is the Rust half of that surface.
//!
//! Mode 0 (default): the counters exist but no event bumps them; this is
//! the base-commit contract, which the negative control in the plan
//! requires. Mode 1: the driver runs and bumps the counters. Mode 2: the
//! driver also counts `mismatched` against the Python-side reference
//! tally for each event (slice 2 will define the reference).
//!
//! The counter names mirror the §4.2 table in the plan verbatim. No name
//! may be removed or reinterpreted without updating the plan.

#![allow(non_local_definitions)]

use std::cell::RefCell;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// The nine H1 driver counters.
///
/// `driver_entered` proves the Rust loop ran at all — a structural zero
/// here is the "unreachable branch" failure family named in AGENTS.md.
/// `statements_dispatched` is the driver's core throughput number and
/// must equal the gate-off Python total on the same corpus.
/// `visit_block_iterations` is the inner-loop count the prior brief
/// called the "thing the probe would actually move."
/// `callbacks_emitted` / `callbacks_raised` survive the abnormal path: a
/// Python callback that raised must still have bumped `callbacks_emitted`
/// before the raise propagated, and `callbacks_raised` records the
/// raise. The `bailouts_to_python_loop` counter is reserved for the
/// daemon path, which #1861 explicitly defers out of this slice.
#[derive(Default)]
struct DriverCounters {
    /// 0 off (default), 1 driver active, 2 driver active + compare.
    mode: u8,
    /// Bumped once per `check_first_pass` entry that reaches Rust.
    driver_entered: u64,
    /// Bumped once per statement the driver dispatched to a Python callback.
    statements_dispatched: u64,
    /// Bumped once per Python callback the driver invoked.
    callbacks_emitted: u64,
    /// Bumped when a Python callback raised and the driver propagated.
    callbacks_raised: u64,
    /// Reserved for the daemon path (bails to Python in this slice).
    bailouts_to_python_loop: u64,
    /// Bumped once per `defer_node` the driver triggered.
    deferred_nodes_deferred: u64,
    /// Bumped once per `mark_unreachable` the driver triggered.
    unreachable_marked: u64,
    /// Bumped once per `break` taken out of the statement loop.
    breaks_taken: u64,
    /// Bumped once per `visit_block` iteration the driver drove.
    visit_block_iterations: u64,
}

/// `(driver_entered, statements_dispatched, callbacks_emitted,
/// callbacks_raised, bailouts_to_python_loop, deferred_nodes_deferred,
/// unreachable_marked, breaks_taken, visit_block_iterations)`.
pub(crate) type DriverCounters9 = (u64, u64, u64, u64, u64, u64, u64, u64, u64);

thread_local! {
    static DRIVER_STATE: RefCell<DriverCounters> = RefCell::new(DriverCounters::default());
}

fn driver_mode() -> u8 {
    DRIVER_STATE.with(|c| c.borrow().mode)
}

fn set_driver_mode(mode: u8) -> PyResult<u8> {
    if mode > 2 {
        return Err(PyValueError::new_err(format!(
            "checker driver mode must be 0, 1 or 2, got {mode}"
        )));
    }
    DRIVER_STATE.with(|c| c.borrow_mut().mode = mode);
    Ok(driver_mode())
}

/// Bump one of the nine counters by event kind. Kind values must stay in
/// lockstep with `KIND_*` in `mypy/checker_driver.py`; a mismatched bump
/// would silently undercount and read as "the driver is doing less work
/// than the Python path" on the cross-run differential.
///
/// Kinds:
/// - 0 = `driver_entered`
/// - 1 = `statements_dispatched`
/// - 2 = `callbacks_emitted`
/// - 3 = `callbacks_raised`
/// - 4 = `bailouts_to_python_loop`
/// - 5 = `deferred_nodes_deferred`
/// - 6 = `unreachable_marked`
/// - 7 = `breaks_taken`
/// - 8 = `visit_block_iterations`
fn record(kind: u8) {
    DRIVER_STATE.with(|c| {
        let mut s = c.borrow_mut();
        match kind {
            0 => s.driver_entered += 1,
            1 => s.statements_dispatched += 1,
            2 => s.callbacks_emitted += 1,
            3 => s.callbacks_raised += 1,
            4 => s.bailouts_to_python_loop += 1,
            5 => s.deferred_nodes_deferred += 1,
            6 => s.unreachable_marked += 1,
            7 => s.breaks_taken += 1,
            8 => s.visit_block_iterations += 1,
            _ => {
                // A future kind panics loudly rather than silently
                // undercounting: a probe that can undercount is worse than
                // no probe (AGENTS.md measurement-code rule).
                unreachable!("checker driver record: unknown kind {kind}")
            }
        }
    });
}

fn read_counters() -> DriverCounters9 {
    DRIVER_STATE.with(|c| {
        let s = c.borrow();
        (
            s.driver_entered,
            s.statements_dispatched,
            s.callbacks_emitted,
            s.callbacks_raised,
            s.bailouts_to_python_loop,
            s.deferred_nodes_deferred,
            s.unreachable_marked,
            s.breaks_taken,
            s.visit_block_iterations,
        )
    })
}

/// Clear the counters; the mode is deliberately kept — a reset must not
/// silently stop the driver, mirroring the existing G-family
/// `flip_reset` discipline.
fn reset() {
    DRIVER_STATE.with(|c| {
        let mode = c.borrow().mode;
        *c.borrow_mut() = DriverCounters {
            mode,
            ..DriverCounters::default()
        };
    });
}

#[pyfunction]
pub(crate) fn rust_checker_driver_mode() -> u8 {
    driver_mode()
}

#[pyfunction]
pub(crate) fn rust_checker_driver_set_mode(mode: u8) -> PyResult<u8> {
    set_driver_mode(mode)
}

#[pyfunction]
pub(crate) fn rust_checker_driver_record(kind: u8) {
    record(kind)
}

#[pyfunction]
pub(crate) fn rust_checker_driver_counters() -> DriverCounters9 {
    read_counters()
}

#[pyfunction]
pub(crate) fn rust_checker_driver_reset() {
    reset()
}

pub(crate) fn register_registry(m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(rust_checker_driver_mode, m)?)?;
    m.add_function(wrap_pyfunction!(rust_checker_driver_set_mode, m)?)?;
    m.add_function(wrap_pyfunction!(rust_checker_driver_record, m)?)?;
    m.add_function(wrap_pyfunction!(rust_checker_driver_counters, m)?)?;
    m.add_function(wrap_pyfunction!(rust_checker_driver_reset, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_defaults_to_zero_and_counters_are_zero() {
        assert_eq!(driver_mode(), 0);
        let c = read_counters();
        assert_eq!(c, (0u64, 0, 0, 0, 0, 0, 0, 0, 0));
    }

    #[test]
    fn set_mode_rejects_unknown() {
        set_driver_mode(0).unwrap();
        assert!(set_driver_mode(3).is_err());
        assert!(set_driver_mode(2).is_ok());
        set_driver_mode(0).unwrap();
    }

    #[test]
    fn record_bumps_the_matching_counter() {
        reset();
        record(0);
        record(1);
        record(1);
        record(8);
        record(8);
        record(8);
        let c = read_counters();
        assert_eq!(c.0, 1); // driver_entered
        assert_eq!(c.1, 2); // statements_dispatched
        assert_eq!(c.8, 3); // visit_block_iterations
    }

    #[test]
    fn reset_keeps_mode_clears_counters() {
        set_driver_mode(2).unwrap();
        record(1);
        record(1);
        reset();
        assert_eq!(driver_mode(), 2);
        assert_eq!(read_counters().1, 0);
        set_driver_mode(0).unwrap();
    }
}
