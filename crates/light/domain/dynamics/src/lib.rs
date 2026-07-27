#![forbid(unsafe_code)]
//! Portable Dynamic definitions and deterministic scalar-lane evaluation.
//!
//! Every lane is evaluated independently. Shared instance clocks, phase maps, and Random-group
//! streams coordinate scalar lanes without creating a multi-attribute value path.

mod evaluate;
mod model;
mod phase;
mod runtime;
mod validation;

#[cfg(test)]
mod tests;

pub use evaluate::{DynamicEvaluationContext, DynamicEvaluator, ScalarSourceResolver};
pub use model::*;
pub use phase::{PhasePosition, SpatialPosition, project_phase};
pub use runtime::{
    DynamicController, DynamicControllerSource, DynamicControllerTransitionSnapshot,
    DynamicInstanceSnapshot, DynamicRandomPulseSnapshot, DynamicRandomStreamSnapshot,
    DynamicRuntime, DynamicRuntimeError, DynamicRuntimeSample, DynamicRuntimeSnapshot,
    DynamicSpeedTransport, DynamicStartRequest, DynamicTargetScope,
};
pub use validation::{
    DynamicAliasingWarning, DynamicValidationError, aliasing_warning, validate_definition,
};
