use super::PatchChange;
use crate::{ActionContext, ActionError, ActiveShowPorts};
use light_core::{FixtureId, Revision};
use light_show::FixtureProfileRevision;
use std::time::Duration;

/// Stable phase names emitted by the real Patch application boundary.
///
/// Production adapters may ignore these observations. The released performance probe records
/// them so a regression can be attributed without changing Patch command or event semantics.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PatchPerformancePhase {
    BoundaryValidation,
    SnapshotLoad,
    ConflictDetection,
    ProfileResolutionAndPlacement,
    CandidatePreparation,
    Compile,
    RuntimePreparation,
    Backup,
    Persistence,
    RuntimeInstall,
    ProjectionReconcile,
    EventPublication,
}

/// Adapters for active-show ownership, exact library reads, and live runtime installation.
pub trait ShowPatchPorts: ActiveShowPorts {
    fn authorize_patch_read(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn authorize_patch(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.authorize_patch_read(context)
    }

    /// Resolves one exact immutable revision without reading the fixture catalog.
    fn resolve_profile_revision(
        &self,
        profile_id: FixtureId,
        revision: Revision,
    ) -> Result<FixtureProfileRevision, ActionError>;

    /// Targeted adapter/cache reconciliation must not reopen or recompile the show.
    fn reconcile_patch_change(&self, change: &PatchChange);

    /// Observes one Patch phase without making diagnostics part of the command outcome.
    fn record_patch_performance_phase(&self, _phase: PatchPerformancePhase, _elapsed: Duration) {}
}
