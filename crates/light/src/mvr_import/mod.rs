//! Atomic application boundary for importing MVR fixtures into the active show.

mod bindings;
mod layers;
mod model;
mod occupied;
mod plan;
mod projection;
mod revisions;
mod service;
mod sources;

pub use layers::{DEFAULT_PATCH_LAYER, MvrLayerPlan};
pub use model::{
    ActiveMvrImportResult, ApplyActiveMvrImportCommand, MvrImportResolution,
    PreparedActiveMvrImport,
};
pub use occupied::{
    OccupiedPatch, apply_mvr_primary_address, fixture_occupied_patches, occupied_patches,
    primary_footprint,
};
pub use plan::{
    MvrDocumentImport, mvr_destination_fixture_ids, plan_mvr_document_import,
    resolve_mvr_definition,
};
pub use service::MvrImportService;
pub use sources::{MVR_SOURCE_ARCHIVE_KIND, RetainedMvrSources};

#[cfg(test)]
mod tests;

pub use bindings::{MvrDefinitions, bind_mvr_sources};
pub use revisions::{
    MvrProfileConflict, MvrProfileSlots, mvr_native_profile_slots, mvr_profile_conflicts,
    mvr_profile_identity, reserve_mvr_profiles, reserve_mvr_profiles_with_identity_copies,
};
