mod errors;
mod programming_replacement;

use self::errors::{engine_error, fixture_error, store_error};
use super::PreparedOutputSnapshot;
use super::{
    ActiveShowBackupKind, AppState, HighlightInstallPolicy, PlaybackInstallPolicy,
    ServerActiveShowUnitOfWork, install_prepared_snapshot_with_selection_refresh,
};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowPorts, ActiveShowUnitOfWork,
    BackupIdentity, PatchChange, ShowPatchPorts,
};
use light_core::{FixtureId, Revision, ShowId};
use light_engine::EngineSnapshot;
use light_show::{
    FixtureProfileRevision, PortableShowCommit, PortableShowDocument, PortableShowTransaction,
};
use parking_lot::RwLock;
use std::{collections::HashSet, sync::Arc};

/// Runtime adapter for the application-owned active-show patch workflow.
///
/// Each application-ordered lifecycle owns an active-show coordinator permit. Snapshot lifecycles
/// release it before immutable fixture-library planning begins; transaction lifecycles retain it
/// through commit, runtime installation, reconciliation, and event publication.
#[derive(Clone)]
pub(super) struct ServerShowPatchPorts {
    state: AppState,
    current_patch_revision: Arc<RwLock<Option<u64>>>,
    activation_held: bool,
    programming_replacement: Arc<RwLock<programming_replacement::PreparedProgrammingReplacement>>,
}

impl ServerShowPatchPorts {
    pub(super) fn new(state: AppState) -> Self {
        Self {
            state,
            current_patch_revision: Arc::new(RwLock::new(None)),
            activation_held: false,
            programming_replacement: Arc::new(RwLock::new(Default::default())),
        }
    }

    pub(super) fn with_activation_held(state: AppState) -> Self {
        Self {
            state,
            current_patch_revision: Arc::new(RwLock::new(None)),
            activation_held: true,
            programming_replacement: Arc::new(RwLock::new(Default::default())),
        }
    }

    pub(super) const fn state(&self) -> &AppState {
        &self.state
    }

    fn current_patch_revision(&self) -> Option<u64> {
        *self.current_patch_revision.read()
    }

    fn remember_patch_revision(&self, revision: u64) {
        *self.current_patch_revision.write() = Some(revision);
    }
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct PatchProfileResolutionPause {
    state: std::sync::Mutex<PatchProfileResolutionPauseState>,
    changed: std::sync::Condvar,
}

#[cfg(test)]
#[derive(Default)]
struct PatchProfileResolutionPauseState {
    armed: bool,
    started: bool,
    released: bool,
}

#[cfg(test)]
impl PatchProfileResolutionPause {
    pub(super) fn arm(&self) {
        let mut state = self.state.lock().unwrap();
        state.armed = true;
        state.started = false;
        state.released = false;
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
            "server patch profile resolution did not start"
        );
    }

    pub(super) fn release(&self) {
        let mut state = self.state.lock().unwrap();
        state.released = true;
        self.changed.notify_all();
    }

    fn pause_if_armed(&self) {
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

pub(super) struct ServerShowPatchUnitOfWork {
    inner: ServerActiveShowUnitOfWork,
    patch_revision: u64,
}

impl ServerShowPatchUnitOfWork {
    pub(super) const fn active_show_unit(&self) -> &ServerActiveShowUnitOfWork {
        &self.inner
    }
}

impl ActiveShowUnitOfWork for ServerShowPatchUnitOfWork {
    fn document(&self) -> &PortableShowDocument {
        self.inner.document()
    }

    fn backup(&mut self, identity: &BackupIdentity) -> Result<(), ActionError> {
        self.inner
            .backup(identity)
            .map_err(|error| at_patch_revision(error, self.patch_revision))
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        self.inner
            .commit(transaction)
            .map_err(|error| at_patch_revision(error, self.patch_revision))
    }
}

impl ActiveShowPorts for ServerShowPatchPorts {
    type UnitOfWork = ServerShowPatchUnitOfWork;
    type PreparedRuntime = PreparedOutputSnapshot;

    fn run_active_show_lifecycle<T>(
        &self,
        _context: &ActionContext,
        _show_id: ShowId,
        operation: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError> {
        #[cfg(test)]
        self.state.active_show.pause_patch_lifecycle_if_armed();
        if self.activation_held {
            return self.state.programming.run_active_show_boundary(operation);
        }
        let _activation = self.state.active_show.acquire_blocking();
        self.state.programming.run_active_show_boundary(operation)
    }

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        *self.programming_replacement.write() = Default::default();
        let unit =
            ServerActiveShowUnitOfWork::begin(&self.state, show_id, ActiveShowBackupKind::Patch)?;
        let patch_revision = unit.document().patch_revision().value();
        self.remember_patch_revision(patch_revision);
        Ok(ServerShowPatchUnitOfWork {
            inner: unit,
            patch_revision,
        })
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: Revision,
    ) -> Result<light_show::PortableShowObjectUndo, ActionError> {
        unit.inner
            .prepare_object_undo(kind, object_id, expected_object_revision)
            .map_err(|error| at_current_patch_revision(error, self.current_patch_revision()))
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        let prepared = self.state.output.prepare_snapshot(snapshot);
        prepared
            .map(|prepared| {
                prepared
                    .with_replacement_migrations(self.programming_replacement.read().plans.clone())
            })
            .map_err(|error| engine_error(error, self.current_patch_revision()))
    }

    fn install_runtime(&self, context: &ActionContext, prepared: Self::PreparedRuntime) {
        install_prepared_snapshot_with_selection_refresh(
            &self.state,
            context,
            prepared,
            None,
            PlaybackInstallPolicy::Preserve,
            HighlightInstallPolicy::Reconcile,
        );
    }

    fn finalize_runtime<T>(
        &self,
        context: &ActionContext,
        prepared: Self::PreparedRuntime,
        persist: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError> {
        let committed =
            super::engine_selection_refresh::finalize_prepared_snapshot_with_selection_refresh(
                &self.state,
                context,
                prepared,
                None,
                || {
                    let committed = persist()?;
                    // The enclosing lifecycle still holds the Programmer mutation gate. Captured
                    // exact IDs/orders cannot change between preflight and this post-commit step.
                    for migration in &self.programming_replacement.read().programmers {
                        self.state.programming.apply_replacement_migration_with_history(
                            migration.before.id, &migration.current, &migration.undo, &migration.redo,
                        ).expect("replacement programmer/history migration was validated under the held mutation boundary");
                    }
                    Ok(committed)
                },
            )
            .map_err(|error| at_current_patch_revision(error, self.current_patch_revision()))?;
        // Publish sparse read-model updates only after the coherent output generation is live.
        for migration in &self.programming_replacement.read().programmers {
            self.state
                .programming
                .publish_replacement_migration_values(context, &migration.before);
        }
        Ok(committed)
    }
}

impl ShowPatchPorts for ServerShowPatchPorts {
    fn prepare_programming_replacements(
        &self,
        plans: &[light_application::PatchProgrammingReplacement],
    ) -> Result<(), ActionError> {
        *self.programming_replacement.write() =
            programming_replacement::capture(&self.state, plans)?;
        Ok(())
    }

    fn resolve_profile_revision(
        &self,
        profile_id: FixtureId,
        revision: Revision,
    ) -> Result<FixtureProfileRevision, ActionError> {
        #[cfg(test)]
        self.state
            .active_show
            .patch_profile_resolution_probe()
            .pause_if_armed();
        let library_revision = u32::try_from(revision).map_err(|_| {
            at_current_patch_revision(
                ActionError::new(
                    ActionErrorKind::Invalid,
                    "fixture profile revision exceeds the library revision range",
                ),
                self.current_patch_revision(),
            )
        })?;
        let profile = self
            .state
            .installation
            .fixture_profile_revision_document(profile_id, library_revision)
            .map_err(|error| fixture_error(error, self.current_patch_revision()))?
            .ok_or_else(|| {
                at_current_patch_revision(
                    ActionError::new(
                        ActionErrorKind::NotFound,
                        "fixture profile revision is not available",
                    ),
                    self.current_patch_revision(),
                )
            })?;
        FixtureProfileRevision::new(profile_id, revision, profile)
            .map_err(|error| store_error(error, self.current_patch_revision()))
    }

    fn reconcile_patch_change(&self, change: &PatchChange) {
        let fixture_ids = affected_fixture_ids(change);
        self.state.media.invalidate_fixtures(fixture_ids);
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

fn at_current_patch_revision(mut error: ActionError, revision: Option<u64>) -> ActionError {
    if let Some(revision) = revision {
        error = error.at_revision(revision);
    }
    error
}

fn at_patch_revision(mut error: ActionError, patch_revision: u64) -> ActionError {
    error.current_revision = Some(patch_revision);
    error
}
