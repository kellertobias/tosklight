//! Fixture-independent, recordable programming contracts.
//!
//! Owners are whole values. Components name edits inside those values, and native channel
//! bindings name an adapter's output. None is a substitute for the other. Rich payloads are
//! shared by the frame store; editing creates a new value without changing retained cues.
mod color;
mod components;
mod direct_adoption;
mod direct_capture;
#[cfg(test)]
mod direct_capture_tests;
mod edit;
#[cfg(test)]
mod edit_tests;
mod field_trace;
mod group;
mod memory;
mod native_arithmetic;
mod owned_context;
mod pending_transition;
mod position;
mod ranks;
mod scalar;
mod spread;
#[cfg(test)]
mod tests;
mod transition;
#[cfg(test)]
mod transition_scale_tests;
#[cfg(test)]
mod transition_tests;
mod value;
mod virtual_color;

pub use color::*;
pub use components::*;
pub use direct_adoption::*;
pub use direct_capture::*;
pub use edit::*;
pub use field_trace::*;
pub use group::*;
pub use native_arithmetic::{scale_native_delta, scale_native_delta_wide};
pub use owned_context::*;
pub use pending_transition::*;
pub use position::*;
pub use scalar::*;
pub use spread::*;
pub use transition::*;
pub(crate) use value::deserialize_intent;
pub use value::{
    ProgrammingValueScope, independent_programming_component, validate_programming_entries,
    validate_targeted_programming_entries,
};
pub use virtual_color::*;

/// Explicit cutover marker for semantic show programming (independent of fixture profile schema).
pub const PROGRAMMING_CONTRACT_VERSION: u16 = 1;
/// Feature gate for stable live Cue-to-Preset references. Literal semantic values remain v1.
pub const LIVE_PRESET_REFERENCE_CONTRACT: u16 = 2;
/// Independent Cuelist numbering, distinct from physical Playback definitions.
pub const INDEPENDENT_CUELIST_POOL_CONTRACT: u16 = 3;
pub const SUPPORTED_PROGRAMMING_CONTRACT: u16 = INDEPENDENT_CUELIST_POOL_CONTRACT;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct IntentError(pub String);

pub(super) fn require(condition: bool, message: &str) -> Result<(), IntentError> {
    condition
        .then_some(())
        .ok_or_else(|| IntentError(message.into()))
}
