#![forbid(unsafe_code)]
//! User-scoped selection and programmer state, shared by all of a user's sessions.

mod command_state;
mod groups;
pub mod highlight;
mod history;
mod preload;
mod presets;
mod registry;
mod selection;
mod sessions;
mod state;
mod transactions;
mod values;

pub mod command_line;

pub use command_state::{
    CommandLineReplaceError, CommandLineState, CommandTarget, ProgrammerInteractionState,
    ProgrammerInteractionVersion,
};
pub use groups::{
    DerivedGroup, FrozenGroup, GroupDefinition, GroupProgrammerValue,
    merge_ordered_group_membership, resolve_group,
};
pub use highlight::{
    HighlightAction, HighlightError, HighlightFixture, HighlightMode, HighlightRegistry,
    HighlightSelectionWrite, HighlightState, HighlightTransition, OSC_REPEAT_GUARD,
    is_duplicate_osc_action,
};
pub use preload::PreloadPlaybackAction;
pub use presets::{Preset, PresetAddress, PresetFamily, PresetStoreMode};
pub use registry::ProgrammerRegistry;
pub use selection::{
    ProgrammerSelection, SelectionExpression, SelectionReference, SelectionReplaceError,
    SelectionRule, apply_selection_rule, resolve_selection_references,
};
pub use state::{
    ProgrammerFixtureUpdate, ProgrammerGroupUpdate, ProgrammerSnapshot, ProgrammerState,
    ProgrammerUpdateContent, TransientProgrammerAction,
};
pub use transactions::ProgrammerTransactionSnapshot;

#[cfg(test)]
mod tests;
