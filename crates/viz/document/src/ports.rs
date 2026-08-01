//! The planning application's patch boundary.
//!
//! The desk's adapter for the same application services owns an output engine, a playback
//! runtime, a programmer and live sessions, and installs every committed patch into all of them.
//! A planning application owns a file. This adapter therefore implements exactly the persistence
//! half of [`ShowPatchPorts`]: it opens the show, resolves immutable profile revisions, commits
//! the transaction, and installs nothing.
//!
//! Compilation still happens. `prepare_runtime` receives the engine snapshot the application
//! layer built from the candidate document and drops it, so a patch that could not be compiled is
//! still rejected here exactly as it is on a desk — the planning application simply has no
//! runtime to install it into.

use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowPorts, ActiveShowUnitOfWork,
    BackupIdentity, PatchChange, ShowPatchPorts,
};
use light_core::{FixtureId, Revision, ShowId};
use light_engine::EngineSnapshot;
use light_fixture::FixtureLibrary;
use light_show::{
    FixtureProfileRevision, PortableShowCommit, PortableShowDocument, PortableShowObjectUndo,
    PortableShowTransaction, ShowStore, StoreError,
};
use parking_lot::Mutex;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) fn store_error(error: StoreError, revision: Option<u64>) -> ActionError {
    let action = ActionError::new(ActionErrorKind::Internal, error.to_string());
    match revision {
        Some(revision) => action.at_revision(revision),
        None => action,
    }
}

/// The patch boundary for one planning document.
///
/// A `ShowStore` owns a SQLite connection and is opened per unit of work, matching the desk
/// adapter: the ports value itself carries only the paths and stays `Send + Sync`.
pub struct PlanningPorts {
    show_path: PathBuf,
    /// Fixture packages the operator can patch from. A document that only edits fixtures it
    /// already contains does not need one.
    library: Option<Arc<Mutex<FixtureLibrary>>>,
}

impl PlanningPorts {
    pub fn new(show_path: impl Into<PathBuf>) -> Self {
        Self {
            show_path: show_path.into(),
            library: None,
        }
    }

    pub fn with_library(mut self, library: Arc<Mutex<FixtureLibrary>>) -> Self {
        self.library = Some(library);
        self
    }

    pub fn show_path(&self) -> &Path {
        &self.show_path
    }

    pub(crate) fn library(&self) -> Option<&Mutex<FixtureLibrary>> {
        self.library.as_deref()
    }

    fn open(&self) -> Result<ShowStore, ActionError> {
        ShowStore::open(&self.show_path).map_err(|error| store_error(error, None))
    }

    /// Resolves a profile revision the document already retains, so a saved show reopens without
    /// the library that originally supplied its fixtures.
    fn stored_profile_revision(
        &self,
        store: &ShowStore,
        profile_id: FixtureId,
        revision: Revision,
    ) -> Result<Option<FixtureProfileRevision>, ActionError> {
        store
            .resolve_fixture_profile_revision(profile_id, revision)
            .map_err(|error| store_error(error, None))
    }

    /// Falls back to the fixture library for a profile this document has not retained yet, which
    /// is what happens the first time a fixture is patched.
    fn library_profile_revision(
        &self,
        profile_id: FixtureId,
        revision: Revision,
    ) -> Result<Option<FixtureProfileRevision>, ActionError> {
        let Some(library) = self.library.as_ref() else {
            return Ok(None);
        };
        let library_revision = u32::try_from(revision).map_err(|_| {
            ActionError::new(
                ActionErrorKind::Invalid,
                "fixture profile revision exceeds the library revision range",
            )
        })?;
        let document = library
            .lock()
            .profile_revision_document(profile_id, library_revision)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
        let Some(document) = document else {
            return Ok(None);
        };
        FixtureProfileRevision::new(profile_id, revision, document)
            .map(Some)
            .map_err(|error| store_error(error, None))
    }
}

/// One open planning mutation. Dropping it without committing leaves the file untouched.
pub struct PlanningUnitOfWork {
    store: ShowStore,
    document: Option<PortableShowDocument>,
}

impl PlanningUnitOfWork {
    fn begin(ports: &PlanningPorts, show_id: ShowId) -> Result<Self, ActionError> {
        let store = ports.open()?;
        let document = store
            .portable_document()
            .map_err(|error| store_error(error, None))?;
        if document.id() != show_id {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "requested show is not the open planning document",
            )
            .at_revision(document.revision().value()));
        }
        Ok(Self {
            store,
            document: Some(document),
        })
    }
}

impl ActiveShowUnitOfWork for PlanningUnitOfWork {
    fn document(&self) -> &PortableShowDocument {
        self.document
            .as_ref()
            .expect("planning unit of work retains its document until drop")
    }

    /// The planning application takes no pre-mutation file copy.
    ///
    /// A desk mutates the show an audience is watching and checkpoints against that. A planning
    /// document is edited before anything depends on it, and its safety net is the operator's own
    /// save: the file on disk is not replaced until they ask for it.
    fn backup(&mut self, _identity: &BackupIdentity) -> Result<(), ActionError> {
        Ok(())
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
                }
                Ok(commit)
            }
            Err(error) => {
                // The file may have moved under this unit; drop the document so the next unit
                // reloads rather than reusing a stale copy.
                self.document = None;
                Err(store_error(error, Some(revision)))
            }
        }
    }
}

impl ActiveShowPorts for PlanningPorts {
    type UnitOfWork = PlanningUnitOfWork;
    /// Nothing is installed, so nothing is prepared.
    type PreparedRuntime = ();

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        PlanningUnitOfWork::begin(self, show_id)
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: Revision,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        unit.store
            .prepare_object_undo(kind, object_id, expected_object_revision)
            .map_err(|error| store_error(error, None))
    }

    /// Compiling proves the candidate show is renderable; the result is then discarded.
    fn prepare_runtime(
        &self,
        _snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        Ok(())
    }

    fn install_runtime(&self, _context: &ActionContext, _prepared: Self::PreparedRuntime) {}
}

impl ShowPatchPorts for PlanningPorts {
    fn resolve_profile_revision(
        &self,
        profile_id: FixtureId,
        revision: Revision,
    ) -> Result<FixtureProfileRevision, ActionError> {
        let store = self.open()?;
        if let Some(profile) = self.stored_profile_revision(&store, profile_id, revision)? {
            return Ok(profile);
        }
        let profile = self
            .library_profile_revision(profile_id, revision)?
            .ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::NotFound,
                    "fixture profile revision is not available",
                )
            })?;
        // Retain it in the document so this show stays patchable away from the library that
        // supplied it — the same reason a desk show carries its own profile revisions.
        store
            .insert_fixture_profile_revision(&profile)
            .map_err(|error| store_error(error, None))?;
        Ok(profile)
    }

    fn reconcile_patch_change(&self, _change: &PatchChange) {}
}
