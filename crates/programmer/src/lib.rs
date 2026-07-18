#![forbid(unsafe_code)]
//! User-scoped selection and programmer state, shared by all of a user's sessions.

mod command_state;
mod groups;
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

pub use command_state::{CommandLineReplaceError, CommandLineState, CommandTarget};
pub use groups::{
    DerivedGroup, FrozenGroup, GroupDefinition, GroupProgrammerValue,
    merge_ordered_group_membership, resolve_group,
};
pub use preload::PreloadPlaybackAction;
pub use presets::{Preset, PresetAddress, PresetFamily, PresetStoreMode};
pub use registry::ProgrammerRegistry;
pub use selection::{
    ProgrammerSelection, SelectionExpression, SelectionReference, SelectionRule,
    apply_selection_rule, resolve_selection_references,
};
pub use state::{
    ProgrammerFixtureUpdate, ProgrammerGroupUpdate, ProgrammerSnapshot, ProgrammerState,
    ProgrammerUpdateContent, TransientProgrammerAction,
};
pub use transactions::ProgrammerTransactionSnapshot;

#[cfg(test)]
mod tests;
