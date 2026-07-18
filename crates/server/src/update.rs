//! Pure planning for the Shift+Record Update workflow.
//!
//! This module deliberately has no `AppState`, database, or transport dependency. Callers resolve
//! the authoritative active playback/Cue context, build a preview, and persist the resulting
//! [`AtomicUpdatePlan`] as one normal revision-checked object write. Planning borrows programmer
//! content and never consumes it.

mod cue;
mod error;
mod group;
mod incoming;
mod model;
mod plan;
mod preset;
mod target;

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
mod tests;
