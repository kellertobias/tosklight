#![forbid(unsafe_code)]
//! The planning document behind the Viz editor.
//!
//! One `.show` file, opened and mutated through the same application services the lighting desk
//! uses, with none of the desk's runtime. There is no output engine, no playback, no programmer,
//! no OSC and no session model here: patching a fixture validates, compiles and commits exactly as
//! it does on a desk, and then stops.
//!
//! That boundary is the point. The visualizer takes its live values from Art-Net or sACN sent by
//! whatever desk is driving the rig — frequently not this software at all — so the planning
//! application only has to own the rig itself: what fixtures exist, what they are, where they are,
//! and how they are addressed.
//!
//! The file it writes is an ordinary portable show. The desk opens it directly; no conversion,
//! no export step, no second format.

mod media;
mod mvr;
mod ports;
pub mod standalone;

#[cfg(test)]
mod tests;

pub use media::{
    CitpConfiguration, CropRectangle, LedModuleType, ManagedFallbackReference, MediaFallbackAsset,
    MediaIntentAction, MediaLayoutOutcome, MediaLayoutSnapshot, MediaObject, MediaObjectIntent,
    MediaProjector, MediaServer, MediaSource, MediaSurface, MediaSurfaceSection,
    MediaSurfaceSectionKind, MediaTransform, ProjectionScreenMaterial, VersionedMediaObject,
};
pub use mvr::{MvrExport, MvrImportOutcome, MvrPreview, MvrPreviewFixture};
pub use ports::{PlanningPorts, PlanningUnitOfWork};

use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionSource, ActiveShowService, EventBus,
    PatchFixturesCommand, PatchFixturesResult, PatchSnapshot, ShowPatchService,
};
use light_core::ShowId;
use light_fixture::FixtureLibrary;
use light_show::ShowStore;
use parking_lot::Mutex;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("{0}")]
    Store(String),
    #[error("{0}")]
    Fixture(String),
    #[error("{0}")]
    Action(String),
    #[error("{0}")]
    Mvr(String),
}

impl From<ActionError> for DocumentError {
    fn from(error: ActionError) -> Self {
        Self::Action(error.message)
    }
}

impl From<light_show::StoreError> for DocumentError {
    fn from(error: light_show::StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

impl From<light_fixture::FixtureError> for DocumentError {
    fn from(error: light_fixture::FixtureError) -> Self {
        Self::Fixture(error.to_string())
    }
}

/// An open planning document.

pub struct PlanningDocument {
    path: PathBuf,
    show_id: ShowId,
    desk_id: Uuid,
    patch: ShowPatchService,
    ports: PlanningPorts,
}

impl PlanningDocument {
    /// Creates a new show file and opens it.
    pub fn create(path: impl AsRef<Path>, name: &str) -> Result<Self, DocumentError> {
        let path = path.as_ref().to_path_buf();
        let (_store, show_id) = ShowStore::create(&path, name)?;
        Ok(Self::assemble(path, show_id))
    }

    /// Opens an existing show file, whether this application or the desk wrote it.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let path = path.as_ref().to_path_buf();
        let show_id = ShowStore::open(&path)?.id()?;
        Ok(Self::assemble(path, show_id))
    }

    fn assemble(path: PathBuf, show_id: ShowId) -> Self {
        Self {
            ports: PlanningPorts::new(&path),
            path,
            show_id,
            desk_id: Uuid::new_v4(),
            patch: ShowPatchService::new(ActiveShowService::new(EventBus::default())),
        }
    }

    /// Makes a fixture library available for patching fixtures the document does not yet carry.
    pub fn with_library(mut self, library: FixtureLibrary) -> Self {
        self.ports = PlanningPorts::new(&self.path).with_library(Arc::new(Mutex::new(library)));
        self
    }

    /// Opens the shipped or operator fixture packages at `path` and uses them for patching.
    pub fn with_library_at(self, path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let library = FixtureLibrary::open(path)?;
        Ok(self.with_library(library))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn show_id(&self) -> ShowId {
        self.show_id
    }

    pub fn name(&self) -> Result<String, DocumentError> {
        Ok(self.store()?.name()?)
    }

    pub(crate) fn store(&self) -> Result<ShowStore, DocumentError> {
        Ok(ShowStore::open(&self.path)?)
    }

    pub(crate) fn ports(&self) -> &PlanningPorts {
        &self.ports
    }

    /// A planning application has no operator session, so every action is this desk acting on its
    /// own behalf. The identity still travels with the action, exactly as it does on a desk.
    pub(crate) fn context(&self) -> ActionContext {
        ActionContext::system(self.desk_id, ActionSource::UserInterface)
    }

    /// The authoritative patch, including the profile revisions needed to interpret it.
    pub fn patch_snapshot(&self) -> Result<PatchSnapshot, DocumentError> {
        Ok(self
            .patch
            .snapshot(&self.context(), self.show_id, &self.ports)?)
    }

    /// Patches, repatches and removes fixtures as one atomic transaction.
    pub fn patch_fixtures(
        &self,
        command: PatchFixturesCommand,
    ) -> Result<PatchFixturesResult, DocumentError> {
        let expected = self.patch_revision()?;
        let context = self
            .context()
            .with_request_id(Uuid::new_v4().to_string())
            .with_expected_revision(expected);
        Ok(self
            .patch
            .handle(ActionEnvelope { context, command }, &self.ports)?)
    }

    /// Stored show objects of one kind, for consumers that read the document's own records —
    /// output routes, stage layout and venue geometry among them.
    pub fn objects(&self, kind: &str) -> Result<Vec<light_show::VersionedObject>, DocumentError> {
        Ok(self.store()?.objects(kind)?)
    }

    /// Writes one stored show object, creating it when it is not there yet.
    ///
    /// Patch layers travel with the show, so a layer made while planning is the same layer the
    /// desk shows: they are stored objects here exactly as they are there.
    pub fn put_object(
        &self,
        kind: &str,
        id: &str,
        body: &serde_json::Value,
    ) -> Result<(), DocumentError> {
        let store = self.store()?;
        let expected = store
            .objects(kind)?
            .into_iter()
            .find(|object| object.id == id)
            .map_or(light_core::Revision::default(), |object| object.revision);
        store.put_object(kind, id, body, expected)?;
        Ok(())
    }

    /// The portable media layout authored by the standalone Viz product.
    pub fn media_layout(&self) -> Result<MediaLayoutSnapshot, DocumentError> {
        media::snapshot(&self.store()?).map_err(DocumentError::Store)
    }

    /// Applies one replay-safe, revision-checked media object intent atomically.
    pub fn apply_media_intent(
        &self,
        intent: MediaObjectIntent,
    ) -> Result<MediaLayoutOutcome, DocumentError> {
        media::apply(&self.store()?, intent).map_err(DocumentError::Action)
    }

    /// Imports a PNG, JPEG or WebP fallback as immutable bytes inside the portable show.
    ///
    /// Keeping the bytes in the show database makes ordinary save-as/backup/restore exact and
    /// avoids absolute paths or a sidecar directory that could be separated from the show.
    pub fn import_media_fallback(
        &self,
        request_id: impl Into<String>,
        expected_revision: u64,
        name: impl Into<String>,
        bytes: &[u8],
    ) -> Result<(ManagedFallbackReference, MediaLayoutOutcome), DocumentError> {
        media::import_fallback(
            &self.store()?,
            request_id.into(),
            expected_revision,
            name.into(),
            bytes,
        )
        .map_err(DocumentError::Action)
    }

    /// The portable patch revision, which is what patch mutations are versioned against.
    pub fn patch_revision(&self) -> Result<u64, DocumentError> {
        Ok(self.store()?.portable_patch_revision()?.value())
    }

    /// Writes a complete copy of the document to `destination`.
    ///
    /// The result is an ordinary portable show file: the desk opens it through its own show
    /// library with no import step.
    pub fn save_as(&self, destination: impl AsRef<Path>) -> Result<(), DocumentError> {
        Ok(self.store()?.backup_to(destination)?)
    }

    /// Renames the document as the desk's show library will present it.
    pub fn rename(&self, name: &str) -> Result<(), DocumentError> {
        let store = self.store()?;
        let id = store.id()?;
        Ok(store.set_identity(id, name, None)?)
    }
}
