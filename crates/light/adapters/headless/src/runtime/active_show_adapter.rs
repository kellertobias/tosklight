use super::capability_resources::ActiveShowDocumentCache;
use super::{
    ActiveShowRepository, AppState, HighlightInstallPolicy, PlaybackInstallPolicy,
    ProgrammingInstallOwner, install_prepared_snapshot_with_selection_refresh,
    show_mutation_backup::ShowMutationBackupPlan, speed_groups::application_millis,
};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowPorts, ActiveShowUnitOfWork,
    BackupIdentity,
};
use light_core::{SessionId, ShowId};
use light_engine::{EngineError, EngineSnapshot, PreparedEngineSnapshot};
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
    StoreError,
};
use std::sync::Arc;

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
/// The caller holds an active-show coordinator permit across the service call and any returned
/// targeted reconciliation, keeping the exact identity stable through runtime installation.
#[derive(Clone)]
pub(super) struct ServerActiveShowPorts {
    state: AppState,
    backup_kind: ActiveShowBackupKind,
    programming_owner: Option<ProgrammingInstallOwner>,
    /// Acting session for a frozen Group refresh, whose selection is installed inside the owning
    /// Show transaction.
    frozen_selection_session: Option<SessionId>,
}

impl ServerActiveShowPorts {
    pub(super) fn new(state: AppState) -> Self {
        Self {
            state,
            backup_kind: ActiveShowBackupKind::OutputRoute,
            programming_owner: None,
            frozen_selection_session: None,
        }
    }

    pub(super) fn show_objects(state: AppState) -> Self {
        Self {
            state,
            backup_kind: ActiveShowBackupKind::ShowObjects,
            programming_owner: None,
            frozen_selection_session: None,
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
            frozen_selection_session: None,
        }
    }

    pub(super) fn group_management(
        state: AppState,
        owner: ProgrammingInstallOwner,
        session_id: SessionId,
    ) -> Self {
        Self {
            state,
            backup_kind: ActiveShowBackupKind::ShowObjects,
            programming_owner: Some(owner),
            frozen_selection_session: Some(session_id),
        }
    }

    pub(crate) const fn frozen_selection_session(&self) -> Option<SessionId> {
        self.frozen_selection_session
    }

    pub(crate) const fn state(&self) -> &AppState {
        &self.state
    }
}

#[derive(Clone, Copy)]
pub(super) enum ActiveShowBackupKind {
    Patch,
    OutputRoute,
    ShowObjects,
}

/// Drops the cached in-memory document. Required wherever the active show file is replaced or
/// re-identified in place, because such changes do not always bump the portable revision the
/// cache is validated against.
pub(crate) fn invalidate_active_show_document(state: &AppState) {
    state.active_show.clear_document_cache();
}

pub(crate) struct ServerActiveShowUnitOfWork {
    state: AppState,
    store: ActiveShowRepository,
    /// Present for the whole unit; taken only when the unit is dropped (returned to the shared
    /// cache) or discarded after a commit conflict revealed the disk moved underneath it.
    document: Option<PortableShowDocument>,
    cache: ActiveShowDocumentCache,
    backup: ShowMutationBackupPlan,
}

impl ServerActiveShowUnitOfWork {
    pub(super) fn begin(
        state: &AppState,
        show_id: ShowId,
        backup_kind: ActiveShowBackupKind,
    ) -> Result<Self, ActionError> {
        let entry = state.active_show.current().clone().ok_or_else(|| {
            ActionError::new(ActionErrorKind::NotFound, "no active show is loaded")
        })?;
        if entry.id != show_id {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "requested show is not active",
            ));
        }
        let store =
            ActiveShowRepository::open(&entry.path).map_err(|error| store_error(error, None))?;
        let document = Self::current_document(state, &store, show_id)?;
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
            state: state.clone(),
            store,
            document: Some(document),
            cache: state.active_show.document_cache(),
            backup,
        })
    }

    /// Reuses the cached in-memory document when it still matches the store's O(1) portable
    /// revision; any out-of-band portable commit invalidates it and forces a full reload.
    fn current_document(
        state: &AppState,
        store: &ActiveShowRepository,
        show_id: ShowId,
    ) -> Result<PortableShowDocument, ActionError> {
        let cached = state.active_show.document_cache().take();
        if let Some(document) = cached
            && document.id() == show_id
            && store.portable_revision().ok() == Some(document.revision())
        {
            return Ok(document);
        }
        store.portable_document().map_err(|error| {
            let revision = store.portable_revision().ok().map(|value| value.value());
            store_error(error, revision)
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
        self.document
            .as_ref()
            .expect("active-show unit of work retains its document until drop")
    }

    fn backup(&mut self, identity: &BackupIdentity) -> Result<(), ActionError> {
        let revision = self.document().revision().value();
        let show_id = self.document().id();
        if identity.show_id != show_id {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "mutation backup identity does not match the active show",
            )
            .at_revision(revision));
        }
        // Recovery checkpoints run at most once per configured autosave interval (api-rules §8):
        // the first mutation on a show always backs up; later mutations skip the full-file copy
        // until the interval has elapsed. The lock is held across the copy so concurrent
        // mutation paths cannot duplicate a checkpoint.
        let now = application_millis(&self.state);
        let interval_millis = self
            .state
            .installation
            .configuration()
            .autosave_interval_seconds
            .saturating_mul(1_000);
        let mut result = Ok(());
        self.state
            .active_show
            .update_backup_checkpoint(|checkpoint| {
                let due = match *checkpoint {
                    Some((last_show, last_at)) if last_show == show_id => {
                        now.saturating_sub(last_at) >= interval_millis
                    }
                    _ => true,
                };
                if due {
                    result = self
                        .backup
                        .create_mutation(&self.store, identity, Some(revision));
                    if result.is_ok() {
                        *checkpoint = Some((show_id, now));
                    }
                }
            });
        result
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        let revision = self.document().revision().value();
        match self.store.apply_portable_transaction(transaction) {
            Ok(commit) => {
                if let Some(document) = self.document.as_mut() {
                    document.apply_commit(&commit);
                    debug_assert_eq!(document.revision(), commit.revision());
                    self.state.attributes.install_document(document);
                }
                Ok(commit)
            }
            Err(error) => {
                // A commit failure that reached the store means the file may have moved out from
                // under this unit (out-of-band writer); drop the document so the next unit of
                // work reloads instead of reusing a stale copy.
                self.document = None;
                Err(store_error(error, Some(revision)))
            }
        }
    }
}

impl Drop for ServerActiveShowUnitOfWork {
    fn drop(&mut self) {
        // The retained document always reflects the last committed on-disk state (mutations touch
        // it only via `apply_commit` after a successful store commit), so it is safe to hand back
        // for the next unit of work regardless of whether this unit committed.
        if let Some(document) = self.document.take() {
            self.cache.replace(Some(document));
        }
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
            .output
            .prepare_snapshot(snapshot)
            .map_err(|error| engine_error(error, Some(revision)))
    }

    fn normalized_active_snapshot(&self) -> Option<Arc<EngineSnapshot>> {
        Some(self.state.output.snapshot())
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
