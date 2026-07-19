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

    fn commit(
        self,
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
