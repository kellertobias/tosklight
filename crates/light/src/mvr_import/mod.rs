//! Atomic application boundary for importing MVR fixtures into the active show.

mod layers;
mod model;
mod plan;
mod projection;
mod service;

pub use layers::{DEFAULT_PATCH_LAYER, MvrLayerPlan};
pub use model::{
    ActiveMvrImportResult, ApplyActiveMvrImportCommand, MvrImportResolution,
    PreparedActiveMvrImport,
};
pub use plan::resolve_mvr_definition;
pub use service::MvrImportService;

#[cfg(test)]
mod tests;
