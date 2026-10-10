mod legacy_profiles;
mod model;
mod placement;
mod ports;
mod prepare;
mod profiles;
mod programming_replacement;
mod projection;
mod query;
mod record_index;
mod records;
mod replacement;
mod replay;
mod service;
mod update;
mod validation;
mod vector_spread;

pub use model::{
    PatchChange, PatchFixtureAxis, PatchFixtureCandidate, PatchFixtureProjection,
    PatchFixtureUpdateAction, PatchFixtureUpdateIntent, PatchFixturesCommand, PatchFixturesResult,
    PatchHeadReplacement, PatchModeProjection, PatchOperatorAddressOverride, PatchPlacementIntent,
    PatchProfileRevisionProjection, PatchRootProgramReplacement, PatchSnapshot,
    PatchSplitPlacementIntent, PatchSplitPlacementMode, PatchVectorAxis, PatchVectorKind,
    PatchVectorSpreadIntent,
};
pub use ports::{PatchPerformancePhase, ShowPatchPorts};
pub(crate) use prepare::{StagedPatch, stage_patch_command};
pub use programming_replacement::PatchProgrammingReplacement;
pub(crate) use query::fixture_projection;
pub use service::ShowPatchService;

#[cfg(test)]
mod tests;
