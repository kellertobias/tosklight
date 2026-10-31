//! Merging an offline-programmed show copy back into the show it was taken from (TL-858).

mod engine;
mod json;
mod model;
mod renumber;
mod service;

#[cfg(test)]
mod tests;

pub use engine::merge_shows;
pub use model::{
    MergeChoice, MergeConflict, MergeInputs, MergeKindSummary, MergeMacroWarning, MergeObjects,
    MergeOutcome, MergeRenumber, MergeResolutions,
};
pub use service::{OfflineCopy, ShowMergePreview, ShowMergeService};
