use super::{
    AppState, HighlightInstallPolicy, PlaybackInstallPolicy, ProgrammingInstallOwner,
    install_prepared_snapshot_with_selection_refresh, show_mutation_backup::ShowMutationBackupPlan,
};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowPorts, ActiveShowUnitOfWork,
    BackupIdentity,
};
use light_core::ShowId;
use light_engine::{EngineError, EngineSnapshot, PreparedEngineSnapshot};
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
    ShowStore, StoreError,
};

#[cfg(test)]
#[derive(Default)]
pub(super) struct ActiveShowLifecyclePause {
    state: std::sync::Mutex<ActiveShowLifecyclePauseState>,
    changed: std::sync::Condvar,
}

#[cfg(test)]
#[derive(Default)]
struct ActiveShowLifecyclePauseState {
    armed: bool,
    started: bool,
    released: bool,
}

#[cfg(test)]
impl ActiveShowLifecyclePause {
    pub(super) fn arm(&self) {
        let mut state = self.state.lock().unwrap();
        *state = ActiveShowLifecyclePauseState {
            armed: true,
            started: false,
            released: false,
        };
    }

    pub(super) fn wait_until_started(&self) {
        let state = self.state.lock().unwrap();
        let (state, _) = self
            .changed
            .wait_timeout_while(state, std::time::Duration::from_secs(5), |state| {
                !state.started
            })
            .unwrap();
        assert!(
            state.started,
            "active-show lifecycle did not reach its test pause"
        );
    }

    pub(super) fn release(&self) {
        let mut state = self.state.lock().unwrap();
        state.released = true;
        self.changed.notify_all();
    }

    pub(super) fn pause_if_armed(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.armed {
            return;
        }
        state.started = true;
        self.changed.notify_all();
        while !state.released {
            state = self.changed.wait(state).unwrap();
        }
        state.armed = false;
    }
}

/// Server adapter for generic application-owned active-show mutations.
///
/// The caller holds `AppState::activation_lock` across the service call and any returned targeted
/// reconciliation, keeping the exact active-show identity stable through runtime installation.
#[derive(Clone)]
pub(super) struct ServerActiveShowPorts {
    state: AppState,
    backup_kind: ActiveShowBackupKind,
    programming_owner: Option<ProgrammingInstallOwner>,
}

impl ServerActiveShowPorts {
    pub(super) fn new(state: AppState) -> Self {
        Self {
            state,
            backup_kind: ActiveShowBackupKind::OutputRoute,
            programming_owner: None,
        }
    }

    pub(super) fn show_objects(state: AppState) -> Self {
        Self {
            state,
            backup_kind: ActiveShowBackupKind::ShowObjects,
            programming_owner: None,
        }
    }

    pub(super) fn show_objects_with_programming_owner(
        state: AppState,
        owner: ProgrammingInstallOwner,
    ) -> Self {
        Self {
            state,
            backup_kind: ActiveShowBackupKind::ShowObjects,
            programming_owner: Some(owner),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ActiveShowBackupKind {
    Patch,
    OutputRoute,
    ShowObjects,
}

pub(crate) struct ServerActiveShowUnitOfWork {
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

    pub(super) fn prepare_object_undo(
        &self,
        kind: &str,
        object_id: &str,
        expected_object_revision: light_core::Revision,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        self.store
            .prepare_object_undo(kind, object_id, expected_object_revision)
            .map_err(|error| store_error(error, None))
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
        &mut self,
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
        unit.prepare_object_undo(kind, object_id, expected_object_revision)
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

    fn install_runtime(&self, context: &ActionContext, prepared: Self::PreparedRuntime) {
        install_prepared_snapshot_with_selection_refresh(
            &self.state,
            context,
            prepared,
            self.programming_owner,
            PlaybackInstallPolicy::Preserve,
            HighlightInstallPolicy::Reconcile,
        );
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
