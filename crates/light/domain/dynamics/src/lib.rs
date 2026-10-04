#![forbid(unsafe_code)]
//! Portable Dynamic definitions and deterministic scalar-lane evaluation.
//!
//! Every lane is evaluated independently. Shared instance clocks, phase maps, and Random-group
//! streams coordinate scalar lanes without creating a multi-attribute value path.

mod angle_pair;
mod canonical_migration;
mod evaluate;
mod lane;
mod model;
mod phase;
mod programming;
mod runtime;
mod spatial;
mod tracking;
mod validation;

#[cfg(test)]
mod tests;

pub use canonical_migration::migrate_canonical_attributes;
pub use evaluate::{DynamicEvaluationContext, DynamicEvaluator, ScalarSourceResolver};
pub use lane::*;
pub use model::*;
pub use phase::{PhasePosition, SpatialPosition, project_phase, project_ranked_phase};
pub use programming::*;
pub use runtime::{
    CompletedChunk, CompletedDynamicSamples, DeferredTypedSampling, DynamicControl,
    DynamicControlBatch, DynamicControlCursor, DynamicControlJournal, DynamicControlLogError,
    DynamicControlOutcome, DynamicController, DynamicControllerLaneSelection,
    DynamicControllerOutputGateSnapshot, DynamicControllerSource,
    DynamicControllerTransitionSnapshot, DynamicHeldPayload, DynamicHeldSampleSnapshot,
    DynamicInstancePresetSources, DynamicInstanceSnapshot, DynamicLaneSelection,
    DynamicNativeModelResolver, DynamicNativeSourceStatus, DynamicOutputFrameScratch,
    DynamicRandomPulseSnapshot, DynamicRandomStreamSnapshot, DynamicRuntime, DynamicRuntimeError,
    DynamicRuntimeSample, DynamicRuntimeSnapshot, DynamicSampleBoundary, DynamicSampleScope,
    DynamicSamplingScratch, DynamicSpeedTransport, DynamicStartRequest, DynamicTargetLanes,
    DynamicTargetScope, LegacyDynamicSample, NativeColorModelCapability,
    NativeColorModelUnavailable, NativeColorUnavailableReason, PreparedDynamicDefinitions,
    PreparedDynamicPresetSources, TimedDynamicControl, normalize_legacy_programmer_controller_ids,
    replay_dynamic_controls,
};
pub use spatial::{
    DynamicSelectionShape, DynamicSpatialMappingOverride, OverrideStage, Position3d,
    ProjectedSpatialPosition, ProjectionKind, ProjectionPreset, RadarSweep, RadialDirection,
    RankDirection, RankedSelection, SpatialMappingError, SpatialMappingWarning, SpatialProjection,
    SpatialSelectionMapping, SpatialSelectionShape, SpatialTarget, Vector3,
    evaluate_dynamic_spatial_mapping, evaluate_spatial_mapping, project_spatial_positions,
};
pub use tracking::*;
pub use validation::{
    DynamicAliasingWarning, DynamicValidationError, aliasing_warning, validate_definition,
};
