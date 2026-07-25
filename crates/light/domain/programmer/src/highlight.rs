//! Authoritative transient Highlight and programmer-selection stepping shared by every control
//! surface.

mod model;
mod operations;
mod registry;
mod selection;
mod state;

pub use model::{
    HighlightAction, HighlightError, HighlightFixture, HighlightMode, HighlightSelectionWrite,
    HighlightState, HighlightTransition, OSC_REPEAT_GUARD, is_duplicate_osc_action,
};
pub use registry::HighlightRegistry;

#[cfg(test)]
mod tests;
