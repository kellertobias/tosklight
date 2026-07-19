use super::PatchChange;
use crate::{ActionContext, ActionError, ActiveShowUnitOfWork};
use light_core::{FixtureId, Revision, ShowId};
use light_engine::EngineSnapshot;
use light_show::FixtureProfileRevision;

/// Adapters for active-show ownership, exact library reads, and live runtime installation.
pub trait ShowPatchPorts: Send + Sync {
    type UnitOfWork: ActiveShowUnitOfWork;
    type PreparedRuntime;

    fn authorize_patch_read(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn authorize_patch(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.authorize_patch_read(context)
    }

    fn begin_active_show(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError>;

    /// Resolves one exact immutable revision without reading the fixture catalog.
    fn resolve_profile_revision(
        &self,
        profile_id: FixtureId,
        revision: Revision,
    ) -> Result<FixtureProfileRevision, ActionError>;

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError>;

    /// Installation is deliberately infallible: all validation belongs in `prepare_runtime`.
    fn install_runtime(&self, prepared: Self::PreparedRuntime);

    /// Targeted adapter/cache reconciliation must not reopen or recompile the show.
    fn reconcile_patch_change(&self, change: &PatchChange);
}
