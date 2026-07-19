#![forbid(unsafe_code)]
//! Deterministic bridge from fixture attributes and playbacks to immutable DMX universe frames.

mod contribution;
mod controls;
mod engine;
mod fixture;
mod legacy_projection;
mod lifecycle;
mod model;
mod move_in_black;
mod move_in_black_candidate;
mod move_in_black_runtime;
mod profile_blackout;
mod profile_color;
mod profile_projection;
mod programmer_fade;
mod render;
mod resolution;
mod safety;
mod visualization;

pub use engine::Engine;
pub use lifecycle::PreparedEngineSnapshot;
pub use model::{
    EngineError, EngineSnapshot, MoveInBlackDiagnostic, MoveInBlackPosition, MoveInBlackState,
    RenderOptions, RenderResult,
};

pub(crate) use contribution::{
    EngineContribution, ResolvedAttributes, resolve_engine_contributions,
    value_for_ordered_position,
};
pub(crate) use fixture::{profile_head_owner, snapshot_attribute_is_snap};
pub(crate) use legacy_projection::{group_scale_for_fixture, render_fixture};
pub(crate) use move_in_black_candidate::PreparedCandidate;
pub(crate) use move_in_black_runtime::{MoveInBlackKey, MoveInBlackRuntime};
pub(crate) use profile_blackout::blackout_raw;
pub(crate) use profile_color::{channel_visual_level, profile_visual_color};
pub(crate) use profile_projection::{render_profile_split, resolve_profile_head};
pub(crate) use programmer_fade::{
    ProgrammerTransition, ProgrammerTransitionKey, ProgrammerTransitionSource,
};
pub(crate) use safety::{apply_safe_values, apply_safe_values_with_snap};

#[cfg(test)]
mod tests;
