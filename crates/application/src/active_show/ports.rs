use super::ActiveShowObjectChange;
use crate::{ActionContext, ActionError};
use light_core::ShowId;
use light_engine::EngineSnapshot;
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
};

/// One already-open active-show mutation boundary.
pub trait ActiveShowUnitOfWork {
    fn document(&self) -> &PortableShowDocument;

    fn backup(&mut self, identity: &BackupIdentity) -> Result<(), ActionError>;

    /// Persists the prepared transaction while retaining the open unit until runtime installation,
    /// adapter reconciliation, and event publication finish. Adapters may keep lifecycle guards
    /// in the unit to prevent the active-show identity changing during that post-commit sequence.
    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError>;
}

/// Unique operator-visible identity for the one pre-mutation safety backup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupIdentity {
    pub show_id: ShowId,
    pub correlation_id: uuid::Uuid,
    pub request_id: String,
}

/// Runtime adapters needed by generic active-show mutations.
pub trait ActiveShowPorts: Send + Sync {
    type UnitOfWork: ActiveShowUnitOfWork;
    type PreparedRuntime;

    fn authorize_mutation(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    /// Runs one complete application-ordered active-show lifecycle. Adapters that need a broader
    /// installation-level guard acquire it here, before the application ordering gate, and retain
    /// it through persistence, runtime installation, reconciliation, and event publication.
    fn run_active_show_lifecycle<T>(
        &self,
        _context: &ActionContext,
        _show_id: ShowId,
        operation: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError> {
        operation()
    }

    fn begin_active_show(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError>;

    /// Reads the exact previous raw body from the already-open active-show boundary without
    /// changing current state or consuming history.
    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: light_core::Revision,
    ) -> Result<PortableShowObjectUndo, ActionError>;

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError>;

    /// Installation is deliberately infallible: every fallible step precedes persistence.
    fn install_runtime(&self, prepared: Self::PreparedRuntime);

    /// Reconciles adapter-owned projections after the exact committed runtime is installed.
    fn reconcile_object_changes(&self, _changes: &[ActiveShowObjectChange]) {}
}
