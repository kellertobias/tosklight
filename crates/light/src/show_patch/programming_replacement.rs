//! Source-bound replacement migrations; never aliases for future master commands.
mod build;
mod stored;
use crate::{ActionError, ActionErrorKind};
pub(super) use build::build;
pub use light_core::ReplacementRuntimeMigration as PatchProgrammingReplacement;
use light_core::{
    AttributeKey, FixtureId, ReplacementHeadTarget, ReplacementProfileContext,
    ReplacementProgramProjection,
};
use std::collections::{HashMap, HashSet};
pub(super) use stored::stage;
