//! Authoritative Speed Group and Sound-to-Light source-selection semantics.
//!
//! Audio capture and frequency analysis intentionally live outside this module. A browser or
//! attached desk submits timestamped analysis observations; this state machine decides whether
//! they are trustworthy and exposes the single effective rate consumed by chasers and controls.

mod controller;
mod model;

pub use controller::*;
pub use model::*;

#[cfg(test)]
mod tests;
