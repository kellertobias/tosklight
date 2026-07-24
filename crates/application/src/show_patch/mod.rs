mod legacy_profiles;
mod model;
mod placement;
mod ports;
mod prepare;
mod profiles;
mod projection;
mod query;
mod record_index;
mod records;
mod replay;
mod service;
mod validation;

pub use model::{
    PatchChange, PatchFixtureCandidate, PatchFixtureProjection, PatchFixturesCommand,
    PatchFixturesResult, PatchModeProjection, PatchOperatorAddressOverride, PatchPlacementIntent,
    PatchProfileRevisionProjection, PatchSnapshot, PatchSplitPlacementIntent,
    PatchSplitPlacementMode,
};
pub use ports::ShowPatchPorts;
pub use service::ShowPatchService;

#[cfg(test)]
mod tests;
