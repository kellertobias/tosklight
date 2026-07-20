#![forbid(unsafe_code)]
//! Deterministic bridge from fixture attributes and playbacks to immutable DMX universe frames.

mod contribution;
mod contribution_batch;
mod controls;
mod engine;
mod fixture;
mod legacy_projection;
mod lifecycle;
mod model;
mod move_in_black;
mod move_in_black_candidate;
mod move_in_black_runtime;
mod playback;
mod playback_exclusion;
mod profile_blackout;
mod profile_color;
mod profile_encoding;
mod profile_projection;
mod profile_projection_plan;
mod profile_value_index;
mod programmer_fade;
mod programmer_resolution;
mod render;
mod resolution;
mod runtime_generation;
mod safety;
mod visualization;

pub use contribution_batch::{
    ContributionBatch, ContributionSample, ContributionSequenceMaster, ContributionSourceId,
};
pub use engine::Engine;
pub use lifecycle::PreparedEngineSnapshot;
pub use model::{
    EngineError, EngineSnapshot, MoveInBlackDiagnostic, MoveInBlackPosition, MoveInBlackState,
    RenderOptions, RenderResult,
};
pub use playback::{
    CueListPlaybackAction, EnginePlaybackCommand, EnginePlaybackOutcome, PlaybackBatchAction,
    PlaybackBatchCommand, PlaybackBatchOutcome, PlaybackDynamicsProjection, PoolPlaybackAction,
    PreparedPlaybackBatch,
};
pub use playback_exclusion::PoolPlaybackTransition;

pub(crate) use contribution::{
    EngineContribution, EngineContributionResolver, ResolvedAttributes, ResolvedContributionIndex,
    value_for_ordered_position,
};
pub(crate) use contribution_batch::{replaces_source, sampled_values};
pub(crate) use fixture::profile_head_owner;
pub(crate) use legacy_projection::render_fixture;
pub(crate) use move_in_black_candidate::PreparedCandidate;
pub(crate) use move_in_black_runtime::{MoveInBlackKey, MoveInBlackRuntime};
pub(crate) use profile_blackout::blackout_raw;
pub(crate) use profile_color::{channel_visual_level, profile_visual_color};
pub(crate) use profile_encoding::ProfileEncodingIndex;
pub(crate) use profile_projection::{encode_profile_split, resolve_profile_fixture};
pub(crate) use profile_projection_plan::{FixtureProjectionPlan, ProfileProjectionIndex};
pub(crate) use profile_value_index::ProfileValueIndex;
pub(crate) use programmer_fade::{
    ProgrammerTransition, ProgrammerTransitionKey, ProgrammerTransitionSource,
};
pub(crate) use runtime_generation::{
    GroupMasterGenerationUpdate, GroupMasterIndex, RuntimeGeneration,
};
pub(crate) use safety::{apply_safe_values, apply_safe_values_with_snap};

#[cfg(test)]
mod tests;
