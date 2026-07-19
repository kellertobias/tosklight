mod errors;

use self::errors::{engine_error, fixture_error, store_error};
use super::{AppState, show_mutation_backup::ShowMutationBackupPlan};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowUnitOfWork, BackupIdentity, PatchChange,
    ShowPatchPorts,
};
use light_core::{FixtureId, Revision, ShowId};
use light_engine::{EngineSnapshot, PreparedEngineSnapshot};
use light_show::{
    FixtureProfileRevision, PortableShowCommit, PortableShowDocument, PortableShowTransaction,
    ShowStore,
};
use parking_lot::RwLock;
use std::{collections::HashSet, sync::Arc};

/// Runtime adapter for the application-owned active-show patch workflow.
///
/// Callers hold `AppState::activation_lock` across the complete service call. That lock prevents
/// the active show from changing between this adapter's exact identity check, its atomic commit,
/// and the subsequent live-engine installation.
#[derive(Clone)]
pub(super) struct ServerShowPatchPorts {
    state: AppState,
    current_revision: Arc<RwLock<Option<u64>>>,
}

impl ServerShowPatchPorts {
    pub(super) fn new(state: AppState) -> Self {
        Self {
            state,
            current_revision: Arc::new(RwLock::new(None)),
        }
    }

    fn current_revision(&self) -> Option<u64> {
        *self.current_revision.read()
    }

    fn remember_revision(&self, revision: u64) {
        *self.current_revision.write() = Some(revision);
    }
}

pub(super) struct ServerShowPatchUnitOfWork {
    store: ShowStore,
    document: PortableShowDocument,
    backup: ShowMutationBackupPlan,
}

impl ActiveShowUnitOfWork for ServerShowPatchUnitOfWork {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, identity: &BackupIdentity) -> Result<(), ActionError> {
        if identity.show_id != self.document.id() {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "patch backup identity does not match the active show",
            )
            .at_revision(self.document.revision().value()));
        }
        self.backup.create_patch(
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

impl ShowPatchPorts for ServerShowPatchPorts {
    type UnitOfWork = ServerShowPatchUnitOfWork;
    type PreparedRuntime = PreparedEngineSnapshot;

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        let entry = self.state.active_show.read().clone().ok_or_else(|| {
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
        self.remember_revision(document.revision().value());
        if document.id() != show_id {
            return Err(ActionError::new(
                ActionErrorKind::Internal,
                "active-show index and show document identities differ",
            )
            .at_revision(document.revision().value()));
        }
        Ok(ServerShowPatchUnitOfWork {
            store,
            document,
            backup: ShowMutationBackupPlan::patch(&self.state, &entry),
        })
    }

    fn resolve_profile_revision(
        &self,
        profile_id: FixtureId,
        revision: Revision,
    ) -> Result<FixtureProfileRevision, ActionError> {
        let library_revision = u32::try_from(revision).map_err(|_| {
            at_current_revision(
                ActionError::new(
                    ActionErrorKind::Invalid,
                    "fixture profile revision exceeds the library revision range",
                ),
                self.current_revision(),
            )
        })?;
        let profile = self
            .state
            .fixture_library
            .lock()
            .profile_revision_document(profile_id, library_revision)
            .map_err(|error| fixture_error(error, self.current_revision()))?
            .ok_or_else(|| {
                at_current_revision(
                    ActionError::new(
                        ActionErrorKind::NotFound,
                        "fixture profile revision is not available",
                    ),
                    self.current_revision(),
                )
            })?;
        FixtureProfileRevision::new(profile_id, revision, profile)
            .map_err(|error| store_error(error, self.current_revision()))
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.state
            .engine
            .prepare_snapshot(snapshot)
            .map_err(|error| engine_error(error, self.current_revision()))
    }

    fn install_runtime(&self, prepared: Self::PreparedRuntime) {
        self.state.engine.install_prepared_snapshot(prepared);
    }

    fn reconcile_patch_change(&self, change: &PatchChange) {
        let fixture_ids = affected_fixture_ids(change);
        {
            let mut cache = self.state.media_cache.lock();
            for fixture_id in &fixture_ids {
                cache.clear_fixture(&fixture_id.0.to_string());
            }
        }
        let mut statuses = self.state.media_status.write();
        for fixture_id in fixture_ids {
            statuses.remove(&fixture_id);
        }
    }
}

fn affected_fixture_ids(change: &PatchChange) -> HashSet<FixtureId> {
    change
        .fixtures
        .iter()
        .map(|fixture| fixture.patch.fixture_id)
        .chain(change.removed_fixture_ids.iter().copied())
        .collect()
}

fn at_current_revision(mut error: ActionError, revision: Option<u64>) -> ActionError {
    if let Some(revision) = revision {
        error = error.at_revision(revision);
    }
    error
}
