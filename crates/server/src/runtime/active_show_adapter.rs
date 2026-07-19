use super::{
    AppState, active_show_objects::reconcile_group_projections,
    show_mutation_backup::ShowMutationBackupPlan,
};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowPorts, ActiveShowUnitOfWork, BackupIdentity,
};
use light_core::ShowId;
use light_engine::{EngineError, EngineSnapshot, PreparedEngineSnapshot};
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
    ShowStore, StoreError,
};

/// Server adapter for generic application-owned active-show mutations.
///
/// The caller holds `AppState::activation_lock` across the service call and any returned targeted
/// reconciliation, keeping the exact active-show identity stable through runtime installation.
#[derive(Clone)]
pub(super) struct ServerActiveShowPorts {
    state: AppState,
    backup_kind: ActiveShowBackupKind,
}

impl ServerActiveShowPorts {
    pub(super) fn new(state: AppState) -> Self {
        Self {
            state,
            backup_kind: ActiveShowBackupKind::OutputRoute,
        }
    }

    pub(super) fn show_objects(state: AppState) -> Self {
        Self {
            state,
            backup_kind: ActiveShowBackupKind::ShowObjects,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ActiveShowBackupKind {
    Patch,
    OutputRoute,
    ShowObjects,
}

pub(super) struct ServerActiveShowUnitOfWork {
    store: ShowStore,
    document: PortableShowDocument,
    backup: ShowMutationBackupPlan,
}

impl ServerActiveShowUnitOfWork {
    pub(super) fn begin(
        state: &AppState,
        show_id: ShowId,
        backup_kind: ActiveShowBackupKind,
    ) -> Result<Self, ActionError> {
        let entry = state.active_show.read().clone().ok_or_else(|| {
            ActionError::new(ActionErrorKind::NotFound, "no active show is loaded")
        })?;
        if entry.id != show_id {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "requested show is not active",
            ));
        }
        let store = ShowStore::open(&entry.path).map_err(|error| store_error(error, None))?;
        let document = store.portable_document().map_err(|error| {
            let revision = store.portable_revision().ok().map(|value| value.value());
            store_error(error, revision)
        })?;
        if document.id() != show_id {
            return Err(ActionError::new(
                ActionErrorKind::Internal,
                "active-show index and show document identities differ",
            )
            .at_revision(document.revision().value()));
        }
        let backup = match backup_kind {
            ActiveShowBackupKind::Patch => ShowMutationBackupPlan::patch(state, &entry),
            ActiveShowBackupKind::OutputRoute => {
                ShowMutationBackupPlan::output_route(state, &entry)
            }
            ActiveShowBackupKind::ShowObjects => {
                ShowMutationBackupPlan::show_objects(state, &entry)
            }
        };
        Ok(Self {
            store,
            document,
            backup,
        })
    }
}

impl ActiveShowUnitOfWork for ServerActiveShowUnitOfWork {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, identity: &BackupIdentity) -> Result<(), ActionError> {
        if identity.show_id != self.document.id() {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "mutation backup identity does not match the active show",
            )
            .at_revision(self.document.revision().value()));
        }
        self.backup.create_mutation(
            &self.store,
            identity,
            Some(self.document.revision().value()),
        )
    }

    fn commit(
        self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        let revision = self.document.revision().value();
        self.store
            .apply_portable_transaction(transaction)
            .map_err(|error| store_error(error, Some(revision)))
    }
}

impl ActiveShowPorts for ServerActiveShowPorts {
    type UnitOfWork = ServerActiveShowUnitOfWork;
    type PreparedRuntime = PreparedEngineSnapshot;

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        ServerActiveShowUnitOfWork::begin(&self.state, show_id, self.backup_kind)
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: light_core::Revision,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        unit.store
            .prepare_object_undo(kind, object_id, expected_object_revision)
            .map_err(|error| store_error(error, None))
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        let revision = snapshot.revision;
        self.state
            .engine
            .prepare_snapshot(snapshot)
            .map_err(|error| engine_error(error, Some(revision)))
    }

    fn install_runtime(&self, prepared: Self::PreparedRuntime) {
        self.state.engine.install_prepared_snapshot(prepared);
    }

    fn reconcile_object_changes(&self, changes: &[ActiveShowObjectChange]) {
        if changes
            .iter()
            .any(|change| change.kind == ActiveShowObjectKind::Group)
        {
            reconcile_group_projections(&self.state);
        }
    }
}

fn store_error(error: StoreError, fallback: Option<u64>) -> ActionError {
    let message = error.to_string();
    let (kind, revision) = match error {
        StoreError::RevisionConflict { current, .. } => {
            (ActionErrorKind::Conflict, fallback.or(Some(current)))
        }
        StoreError::DocumentRevisionConflict { current, .. } => {
            (ActionErrorKind::Conflict, Some(current.value()))
        }
        StoreError::FixtureProfileRevisionConflict { .. } => (ActionErrorKind::Conflict, fallback),
        StoreError::Sql(_) => (ActionErrorKind::Unavailable, fallback),
        StoreError::Uuid(_) | StoreError::Json(_) | StoreError::Invalid(_) => {
            (ActionErrorKind::Invalid, fallback)
        }
    };
    with_revision(ActionError::new(kind, message), revision)
}

fn engine_error(error: EngineError, revision: Option<u64>) -> ActionError {
    with_revision(
        ActionError::new(ActionErrorKind::Invalid, error.to_string()),
        revision,
    )
}

fn with_revision(mut error: ActionError, revision: Option<u64>) -> ActionError {
    if let Some(revision) = revision {
        error = error.at_revision(revision);
    }
    error
}
