#![forbid(unsafe_code)]
//! Portable Dynamic definitions and deterministic scalar-lane evaluation.
//!
//! Every lane is evaluated independently. Shared instance clocks, phase maps, and Random-group
//! streams coordinate scalar lanes without creating a multi-attribute value path.

mod canonical_migration;
mod evaluate;
mod model;
mod phase;
mod runtime;
mod spatial;
mod validation;

#[cfg(test)]
mod tests;

pub use canonical_migration::migrate_canonical_attributes;
pub use evaluate::{DynamicEvaluationContext, DynamicEvaluator, ScalarSourceResolver};
pub use model::*;
pub use phase::{PhasePosition, SpatialPosition, project_phase, project_ranked_phase};
pub use runtime::{
    DynamicController, DynamicControllerSource, DynamicControllerTransitionSnapshot,
    DynamicInstanceSnapshot, DynamicRandomPulseSnapshot, DynamicRandomStreamSnapshot,
    DynamicRuntime, DynamicRuntimeError, DynamicRuntimeSample, DynamicRuntimeSnapshot,
    DynamicSpeedTransport, DynamicStartRequest, DynamicTargetScope,
};
pub use spatial::{
    DynamicSelectionShape, DynamicSpatialMappingOverride, OverrideStage, Position3d,
    ProjectedSpatialPosition, ProjectionKind, ProjectionPreset, RadarSweep, RadialDirection,
    RankDirection, RankedSelection, SpatialMappingError, SpatialMappingWarning, SpatialProjection,
    SpatialSelectionMapping, SpatialSelectionShape, SpatialTarget, Vector3,
    evaluate_dynamic_spatial_mapping, evaluate_spatial_mapping, project_spatial_positions,
};
pub use validation::{
    DynamicAliasingWarning, DynamicValidationError, aliasing_warning, validate_definition,
};
