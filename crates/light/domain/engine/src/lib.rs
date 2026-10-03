#![forbid(unsafe_code)]
//! Deterministic bridge from fixture attributes and playbacks to immutable DMX universe frames.

mod physical_projection;
pub use physical_projection::{PhysicalForwardFrame, PhysicalInstanceOutput, PhysicalModelSupport};
mod channel_slots;
mod color_report;
mod contribution;
mod contribution_batch;
mod controls;
mod cue_preview;
mod engine;
mod fixture;
mod frame_pool;
mod frame_slots;
mod frame_state;
mod frame_token;
pub use frame_token::{CapturedFrameLane, CapturedFrameToken};
mod frame_values;
mod group_plan;
mod group_programming;
mod lifecycle;
mod model;
mod native_position_projection;
pub use native_position_projection::PositionNativeWrite;
mod native_family_footprint;
mod native_family_projection;
mod optics_readout;
pub use native_family_projection::FamilyNativeWrite;
mod native_raw;
pub use native_raw::{CapturedNativeRaw, ProfileHeadDestination, profile_head_destinations};
mod mount_projection;
pub use mount_projection::{FixtureMountFrame, ResolvedFixtureMount};
mod native_color_sources;
pub use native_color_sources::{
    CapturedDirectColor, DirectColorObservation, NativeColorSourceCatalog,
    NativeColorSourceRevision, NativeColorSourceRevisionKey,
};
mod output_continuity;
pub use output_continuity::OutputContinuityState;
mod move_in_black;
mod move_in_black_candidate;
mod move_in_black_runtime;
mod playback;
mod playback_batch;
mod playback_exclusion;
mod point_projection;
mod position_adoption;
pub use position_adoption::{PositionCommandedOwnerReadout, PositionCommandedReadout};
mod position_freeze_capture;
mod preload_sources;
pub use preload_sources::{
    CapturedDynamicProgrammerLane, CapturedDynamicProgrammerRow, PreparedPreloadSources,
    capture_dynamic_programmer_rows,
};
mod preload_frame;
mod prepared_frame;
mod prepared_geometry;
pub use prepared_geometry::PreparedFrameGeometry;
mod prepared_static_family;
pub use preload_frame::{
    PreloadBranch, PreloadFrameState, PreparedPreloadFrame, RenderedPreloadFrame,
};
pub use prepared_frame::PreparedOutputFrame;
pub use prepared_static_family::{
    FamilyProjectionEvidence, FamilyProjectionMaster, FamilyProjectionMetadata,
    PreparedStaticFamilyFrame,
};
mod profile_blackout;
mod profile_color;
mod profile_encoding;
mod profile_projection;
mod profile_projection_plan;
mod profile_value_index;
mod programmer_fade;
mod programmer_memo;
mod programmer_release;
mod programmer_resolution;
mod render;
mod render_phases;
mod resolution;
mod runtime_generation;
mod safety;
mod tracked_positions;
mod value_pool;
mod visualization;
pub use visualization::{ObservedSourceFrame, ProfileVisualizationProjection};

pub use color_report::{ColorReportHead, HeadColorReport};
pub use contribution_batch::{
    ContributionBatch, ContributionFamilyEntry, ContributionFamilyEvidence,
    ContributionFamilyFootprint, ContributionFamilyRole, ContributionOrigin,
    ContributionProgrammerLane, ContributionReleaseCutoff, ContributionSample,
    ContributionSequenceMaster, ContributionSourceDescriptor, ContributionSourceId,
};
pub use cue_preview::{CuePreviewState, TrackedCueValue, cue_preview_state, render_fixture_slots};
pub use engine::Engine;
pub use lifecycle::{
    FinalizedEngineSnapshot, GroupMasterPreparation, PreparedEngineSnapshot, PreparedGroupMaster,
};
pub use model::{
    EngineError, EngineSnapshot, MoveInBlackDiagnostic, MoveInBlackPosition, MoveInBlackState,
    RenderOptions, RenderResult,
};
pub use playback::{
    CueListPlaybackAction, EnginePlaybackCommand, EnginePlaybackEffect, EnginePlaybackOutcome,
    PlaybackDynamicsProjection, PoolPlaybackAction, VirtualPlaybackAction,
};
pub use playback_batch::{
    PlaybackBatchAction, PlaybackBatchCommand, PlaybackBatchOutcome, PreparedPlaybackBatch,
    PreparedPreloadPlaybackBatch,
};
pub use playback_exclusion::PoolPlaybackTransition;
pub use point_projection::{POINT_AXIS_METRES, ResolvedPointPose};
pub use render_phases::{
    accumulated_microseconds, enabled as render_phases_enabled, reset as reset_render_phases,
};
pub use tracked_positions::{
    TrackedInputFrame, TrackedOverride, TrackedPointInput, TrackedSampleIdentity,
};

pub(crate) use channel_slots::{ChannelSlotIndex, HeadChannelSlots};
pub(crate) use contribution::{
    EngineContribution, EngineContributionResolver, ResolvedAttributes, ResolvedContributionIndex,
};
pub use contribution::{ResolvedChangedAt, ResolvedValues};
pub(crate) use contribution_batch::{replaces_source, sampled_values};
pub(crate) use fixture::profile_head_owner;
#[allow(unused_imports)]
pub(crate) use frame_pool::FramePool;
pub use frame_slots::FrameAddresser;
#[allow(unused_imports)]
pub(crate) use frame_slots::next_generation;
pub(crate) use frame_slots::{Slot, SlotTable};
#[allow(unused_imports)]
pub(crate) use frame_state::{FrameState, Offer, SlotWinner};
pub use frame_values::FrameValues;
pub use value_pool::{Pooled, Reusable, ValuePool};

/// One profile head's values while it is being projected.
///
/// Read several times per channel and rebuilt per head, with keys that never come from outside
/// this desk, so they are hashed for speed rather than against an adversary.
pub(crate) type HeadValues =
    rustc_hash::FxHashMap<light_core::AttributeKey, light_core::AttributeValue>;
pub(crate) type HeadSequenceMasters =
    rustc_hash::FxHashMap<light_core::AttributeKey, contribution::ApplicableSequenceMaster>;
pub(crate) use move_in_black_candidate::PreparedCandidate;
pub(crate) use move_in_black_runtime::{MoveInBlackKey, MoveInBlackRuntime};
pub(crate) use profile_blackout::blackout_raw;
pub(crate) use profile_color::{channel_visual_level, profile_visual_color};
pub(crate) use profile_encoding::ProfileEncodingIndex;
pub(crate) use profile_projection::{
    AxisInversion, ResolvedProfileFixtureOutput, encode_profile_split, resolve_profile_fixture,
};
pub(crate) use profile_projection_plan::{FixtureProjectionPlan, ProfileProjectionIndex};
pub(crate) use profile_value_index::ProfileValueIndex;
pub(crate) use programmer_fade::{ProgrammerTransitionKey, ProgrammerTransitionSource};
pub(crate) use render_phases::{RenderPhase, timed};
pub(crate) use runtime_generation::{
    GroupMasterGenerationUpdate, GroupMasterIndex, GroupMasterLevels, RuntimeGeneration,
    group_stage_positions,
};
pub(crate) use safety::{apply_safe_values, apply_safe_values_with_snap};

#[cfg(test)]
mod tests;
