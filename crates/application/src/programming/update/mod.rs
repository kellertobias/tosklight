//! Programming-owned preview and mutation boundary for the Shift+Record Update workflow.
//!
//! The feature captures narrow Programmer input, resolves live Cue context, and commits one
//! lossless active-show transaction without depending on server `AppState` or transport types.
//! Its planner submodules remain pure: they borrow Programmer content and never consume it.

mod active_show;
mod contracts;
mod cue;
mod error;
mod group;
mod incoming;
mod menu;
mod model;
mod plan;
mod preset;
mod resolution;
mod target;

pub(crate) use contracts::ProgrammingUpdateMenuInput;
pub use contracts::{
    ProgrammingUpdateCommand, ProgrammingUpdateMenuEntry, ProgrammingUpdateObjectReference,
    ProgrammingUpdateOutcome, ProgrammingUpdatePorts, ProgrammingUpdatePreviewRequest,
    ProgrammingUpdatePreviewResult, ProgrammingUpdateProjection, ProgrammingUpdateResult,
    ProgrammingUpdateTargetRequest, ProgrammingUpdateTargetsRequest,
    ProgrammingUpdateTargetsResult,
};
pub use cue::{plan_cue_update, preview_cue_update};
pub use error::UpdateError;
pub use group::{plan_group_update, preview_group_update};
pub use model::{
    CueIdentity, CueSource, CueUpdateMode, ExistingContentMode, UpdateAddress,
    UpdateConfirmationBehavior, UpdateConfirmationPath, UpdateIgnoreReason, UpdateItemOutcome,
    UpdateMenuEntry, UpdateMode, UpdatePreview, UpdatePreviewItem, UpdateSettings,
    UpdateTargetFamily, UpdateTargetFilter, UpdateTargetIdentity, filter_update_menu,
};
pub use plan::{AtomicUpdatePlan, PlannedUpdateObject, UpdateResult};
pub use preset::{plan_preset_update, preview_preset_update};
pub use target::{ActiveCueContext, CueTargetRequest, ResolvedCueTarget, resolve_cue_target};

#[cfg(test)]
mod application_tests;
#[cfg(test)]
mod tests;
