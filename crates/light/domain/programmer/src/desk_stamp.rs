//! Revision and generation stamps for the desk's one Programmer.
//!
//! These were maps keyed by the operator, from when a desk could hold several Programmers.
//! A desk has one, so each map held exactly one entry — and keying a read on the identity a
//! caller happened to present meant a legacy identity read a revision of zero for a Programmer
//! that was several revisions in. Making the stamp a single value removes the key, and with it
//! the possibility of reading the wrong one.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// A monotonic counter belonging to the desk.
///
/// Cheap to read from the render and command paths, which ask for a generation far more often
/// than anything advances one.
#[derive(Clone, Default, Debug)]
pub(crate) struct DeskStamp(Arc<AtomicU64>);

impl DeskStamp {
    /// A stamp starting from a known value, as a staged snapshot of the desk does.
    pub(crate) fn seeded(value: u64) -> Self {
        Self(Arc::new(AtomicU64::new(value)))
    }

    /// The current value. Zero before anything has advanced it.
    pub(crate) fn get(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    /// Advance by one and return the new value.
    pub(crate) fn advance(&self) -> u64 {
        self.0.fetch_add(1, Ordering::AcqRel).saturating_add(1)
    }

    /// Adopt a value, as committing a staged snapshot back onto the desk does.
    pub(crate) fn set(&self, value: u64) {
        self.0.store(value, Ordering::Release);
    }

    /// Return the desk to a fresh runtime, as a startup or test-bench rebuild does.
    pub(crate) fn clear(&self) {
        self.0.store(0, Ordering::Release);
    }
}
