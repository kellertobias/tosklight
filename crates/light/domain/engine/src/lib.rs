#![forbid(unsafe_code)]
//! Deterministic bridge from fixture attributes and playbacks to immutable DMX universe frames.

mod channel_slots;
mod contribution;
mod contribution_batch;
mod controls;
mod engine;
mod fixture;
mod frame_pool;
mod frame_slots;
mod frame_state;
mod frame_values;
mod lifecycle;
mod model;
mod move_in_black;
mod move_in_black_candidate;
mod move_in_black_runtime;
mod playback;
mod playback_batch;
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
mod render_phases;
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
    CueListPlaybackAction, EnginePlaybackCommand, EnginePlaybackEffect, EnginePlaybackOutcome,
    PlaybackDynamicsProjection, PoolPlaybackAction, VirtualPlaybackAction,
};
pub use playback_batch::{
    PlaybackBatchAction, PlaybackBatchCommand, PlaybackBatchOutcome, PreparedPlaybackBatch,
};
pub use playback_exclusion::PoolPlaybackTransition;
pub use render_phases::{
    accumulated_microseconds, enabled as render_phases_enabled, reset as reset_render_phases,
};

pub(crate) use channel_slots::{ChannelSlotIndex, HeadChannelSlots};
pub(crate) use contribution::{
    EngineContribution, EngineContributionResolver, ResolvedAttributes, ResolvedContributionIndex,
    value_for_ordered_position,
};
pub use contribution::{ResolvedChangedAt, ResolvedValues};
pub(crate) use contribution_batch::{replaces_source, sampled_values};
pub(crate) use fixture::profile_head_owner;
#[allow(unused_imports)]
pub(crate) use frame_pool::FramePool;
#[allow(unused_imports)]
pub(crate) use frame_slots::next_generation;
pub(crate) use frame_slots::{Slot, SlotTable};
#[allow(unused_imports)]
pub(crate) use frame_state::{FrameState, Offer, SlotWinner};
pub use frame_values::FrameValues;

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
pub(crate) use programmer_fade::{
    ProgrammerTransition, ProgrammerTransitionKey, ProgrammerTransitionSource,
};
pub(crate) use render_phases::{RenderPhase, timed};
pub(crate) use runtime_generation::{
    GroupMasterGenerationUpdate, GroupMasterIndex, RuntimeGeneration, group_stage_positions,
};
pub(crate) use safety::{apply_safe_values, apply_safe_values_with_snap};

#[cfg(test)]
mod tests;
