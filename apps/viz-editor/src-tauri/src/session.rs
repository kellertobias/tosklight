//! The open document, and the commands the patch sheet drives it with.
//!
//! One document is open at a time, exactly as an operator thinks about it: the window is that
//! show. Opening or creating another replaces it, and the sheet reloads from the new snapshot.

use crate::contract::{ChangeDto, MutationDto, OutcomeDto, SnapshotDto};
use crate::discovery::Discovery;
use crate::recent::RecentShow;
use light_application::MvrImportResolution;
use light_fixture::FixtureLibrary;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use viz_document::{
    LiveDmxInputs, ManagedFallbackReference, MediaLayoutOutcome, MediaLayoutSnapshot,
    MediaObjectIntent,
};
use viz_document::{PaperworkMetadata, PlanningDocument};
use viz_planning::SceneSource;

/// The open document, shared with the visualizer.
///
/// The window and the renderer read the same `SceneSource`, so a fixture patched here is in the
/// next snapshot the visualizer asks for. There is no second copy to keep in step.
#[derive(Default)]
pub struct Session {
    source: SceneSource,
    document_lifecycle: Mutex<()>,
    /// Held for the whole of one operator gesture — every write a command makes — and by every
    /// change Control makes to a bound document, so the two never interleave. Reentrant, so a
    /// gesture made of several commands nests; the count is the nesting depth.
    gesture: parking_lot::ReentrantMutex<std::cell::Cell<u32>>,
    library_path: Mutex<Option<PathBuf>>,
    recent: Mutex<Option<RecentShow>>,
    /// The desk show the open document is bound to, if it came from one.
    pub(crate) binding: Mutex<Option<crate::sync::SyncBinding>>,
    bindings: Mutex<Option<crate::sync::SyncBindingStore>>,
    /// The synchronization of a bound document with its desk.
    engine: Mutex<Option<viz_sync::SyncEngine>>,
    sync_host: Mutex<Option<SyncHost>>,
}

type SyncHost = (
    std::sync::Arc<dyn viz_sync::DocumentHost>,
    tokio::runtime::Handle,
);

/// What the window title bar and the file menu need to know.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub show_id: String,
    pub name: String,
    pub path: String,
    pub fixture_count: usize,
    pub file_name: String,
    pub lighting_designer: String,
    pub show_version: String,
    pub venue: String,
    pub contact_email: String,
    pub contact_phone: String,
    pub project: String,
    pub show_date: String,
    /// The lighting designer's company logo as the show stores it, or empty.
    pub company_logo: String,
    pub last_saved_at: u64,
    pub universe_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperworkInput {
    pub lighting_designer: String,
    pub show_version: String,
    pub venue: String,
    pub contact_email: String,
    pub contact_phone: String,
    pub project: String,
    pub show_date: String,
    /// Absent from a window that predates logos, which leaves no logo.
    #[serde(default)]
    pub company_logo: String,
}

/// One fixture profile the operator can patch from.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryProfile {
    pub id: String,
    pub revision: u32,
    pub manufacturer: String,
    pub name: String,
    pub profile: serde_json::Value,
}

/// One generic body, in the shape the shared fixture-profile editor reads.
#[derive(Debug, Serialize)]
pub struct BodyModelDto {
    pub id: String,
    pub label: String,
    pub group: String,
}

/// One canonical attribute, in the shape the shared fixture-profile editor reads.
#[derive(Debug, Serialize)]
pub struct AttributeDescriptorDto {
    pub id: String,
    pub label: String,
    pub family: light_core::AttributeClass,
    pub value_type: light_core::AttributeValueType,
    pub default_unit: Option<String>,
    pub display_unit: Option<String>,
    pub physical_unit: Option<String>,
    pub cyclic: bool,
    pub recordable: bool,
    pub built_in: bool,
    /// The programmer tab the recommended layout puts the attribute on; the channel editor's
    /// attribute picker narrows by it first.
    pub encoder_group: Option<light_core::EncoderGroup>,
    /// The recommended activation group, which the channel editor offers attributes under.
    pub activation_group_id: Option<String>,
    pub activation_group_label: Option<String>,
}

type Answer<T> = Result<T, String>;

impl Session {
    pub fn set_library_path(&self, path: Option<PathBuf>) {
        *self.library_path.lock() = path;
    }

    pub fn scene_source(&self) -> SceneSource {
        self.source.clone()
    }

    pub fn set_recent_store(&self, recent: RecentShow) {
        *self.recent.lock() = Some(recent);
    }

    /// Where this installation keeps the bindings of documents opened from a desk.
    pub fn set_binding_store(&self, store: crate::sync::SyncBindingStore) {
        *self.bindings.lock() = Some(store);
    }

    /// Where bound documents send their edits from: the engine's host and its runtime.
    pub(crate) fn set_sync_host(
        &self,
        host: std::sync::Arc<dyn viz_sync::DocumentHost>,
        runtime: tokio::runtime::Handle,
    ) {
        *self.sync_host.lock() = Some((host, runtime));
    }

    /// The running synchronization of the open document, if it is bound to a desk.
    pub(crate) fn sync_engine(&self) -> Option<viz_sync::SyncEngine> {
        self.engine.lock().clone()
    }

    /// Opens a show file just copied from a desk, bound to that desk's show. The copy *is* the
    /// desk's show at the moment it was read, so it seeds the confirmed mirror.
    pub(crate) fn open_from_desk(
        &self,
        path: &Path,
        mut binding: crate::sync::SyncBinding,
    ) -> Answer<DocumentSummary> {
        let _lifecycle = self.document_lifecycle.lock();
        let summary = self.open_path_locked(path, None)?;
        binding.acknowledged_show_revision =
            self.with(|document| document.portable_revision().map_err(|e| e.to_string()))?;
        self.set_binding(Some(binding.clone()))?;
        self.start_sync(binding, viz_sync::Start::FreshCopy)?;
        Ok(summary)
    }

    /// Starts synchronizing the open document, when the editor can host a synchronization.
    fn start_sync(&self, binding: crate::sync::SyncBinding, start: viz_sync::Start) -> Answer<()> {
        self.stop_sync();
        let Some((host, runtime)) = self.sync_host.lock().clone() else {
            return Ok(());
        };
        let Some(store) = self.bindings.lock().clone() else {
            return Ok(());
        };
        let _gesture = self.gesture.lock();
        let engine = self.with(|document| {
            viz_sync::SyncEngine::start(document, binding, store, host, &runtime, start)
        })?;
        *self.engine.lock() = Some(engine);
        Ok(())
    }

    fn stop_sync(&self) {
        if let Some(engine) = self.engine.lock().take() {
            engine.stop();
        }
    }

    /// Runs one operator gesture: every document write `action` makes, however many commands it
    /// takes, becomes one synchronized transaction when the outermost gesture ends.
    pub(crate) fn gesture<T>(&self, action: impl FnOnce() -> Answer<T>) -> Answer<T> {
        let depth = self.gesture.lock();
        depth.set(depth.get() + 1);
        let outcome = action();
        depth.set(depth.get() - 1);
        if depth.get() == 0
            && let Some(engine) = self.sync_engine()
        {
            // A failure to journal is reported on the sync status; the edit itself is in the
            // document either way, and is recovered from it on the next start.
            let _ = self.source.with(|document| engine.capture(document));
        }
        outcome
    }

    /// Applies a change Control made, under the gesture lock so it never lands inside an
    /// operator's gesture. Answers whether the document changed.
    pub(crate) fn remote_edit(
        &self,
        edit: &mut dyn FnMut(&PlanningDocument) -> Answer<bool>,
    ) -> Answer<bool> {
        let _gesture = self.gesture.lock();
        let changed = self
            .source
            .with(edit)
            .unwrap_or_else(|| Err("no document is open".to_owned()))?;
        if changed {
            self.source.mark_changed();
        }
        Ok(changed)
    }

    /// Binds the open document to a desk show, or unbinds it, in memory and in the store.
    pub(crate) fn set_binding(&self, binding: Option<crate::sync::SyncBinding>) -> Answer<()> {
        if binding.is_none() {
            self.stop_sync();
        }
        *self.binding.lock() = binding.clone();
        let path = self.with(|document| Ok(document.path().to_path_buf()))?;
        let Some(store) = self.bindings.lock().clone() else {
            return Ok(());
        };
        match binding {
            Some(binding) => store.bind(&path, &binding),
            None => store.unbind(&path),
        }
    }

    pub fn recent_paths(&self) -> Vec<String> {
        self.recent
            .lock()
            .as_ref()
            .map(|recent| {
                recent
                    .list()
                    .into_iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Reopens the show this window had open last time, if it is still there.
    pub fn reopen_recent(&self) {
        let path = self.recent.lock().as_ref().and_then(RecentShow::read);
        if let Some(path) = path
            && let Err(error) = self.open_path(&path, None)
        {
            eprintln!("reopen {}: {error}", path.display());
        }
    }

    /// Opens a show file that arrived from somewhere other than the file dialog — a copy pulled
    /// from a desk, for instance. It is the same open: one document at a time, and this is it.
    pub fn open(&self, path: &Path) -> Answer<DocumentSummary> {
        self.open_path(path, None)
    }

    /// Copies the open document to `path` as a new show and opens the copy, unbound.
    pub(crate) fn fork_to(&self, path: &Path) -> Answer<DocumentSummary> {
        let _lifecycle = self.document_lifecycle.lock();
        self.with(|document| {
            let name = document.name().map_err(|error| error.to_string())?;
            document
                .fork_to(path, &name)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })?;
        if let Some(store) = self.bindings.lock().clone() {
            // A file written over an older bound document at the same path must not inherit it.
            store.unbind(path).ok();
        }
        self.open_path_locked(path, None)
    }

    /// Renames the open document, for a caller that opened it itself rather than through the
    /// window's own rename command.
    pub fn rename_to(&self, name: &str) -> Answer<()> {
        self.change(|document| document.rename(name).map_err(|error| error.to_string()))
    }

    /// The open document's name, for the network record that says what this editor is holding.
    pub fn document_name(&self) -> Option<String> {
        self.source.with(|document| document.name().ok()).flatten()
    }

    fn attach_library(&self, document: PlanningDocument) -> Answer<PlanningDocument> {
        match self.library_path.lock().as_ref() {
            Some(path) if path.exists() => document
                .with_library_at(path)
                .map_err(|error| error.to_string()),
            _ => Ok(document),
        }
    }

    fn open_path(&self, path: &Path, created: Option<&str>) -> Answer<DocumentSummary> {
        let _lifecycle = self.document_lifecycle.lock();
        self.open_path_locked(path, created)
    }

    fn open_path_locked(&self, path: &Path, created: Option<&str>) -> Answer<DocumentSummary> {
        self.stop_sync();
        let document = match created {
            Some(name) => PlanningDocument::create(path, name),
            None => PlanningDocument::open(path),
        }
        .map_err(|error| error.to_string())?;
        let document = self.attach_library(document)?;
        let summary = summarize(&document)?;
        self.source.open(document);
        let binding = self.stored_binding(path);
        *self.binding.lock() = binding.clone();
        if let Some(binding) = binding
            && let Err(error) = self.start_sync(binding, viz_sync::Start::Reopen)
        {
            eprintln!("{} opens without synchronization: {error}", path.display());
        }
        if let Some(recent) = self.recent.lock().as_ref() {
            recent.remember(path);
        }
        Ok(summary)
    }

    /// The stored binding of `path`. A damaged binding leaves the document standalone and says
    /// so, rather than refusing to open the operator's file.
    fn stored_binding(&self, path: &Path) -> Option<crate::sync::SyncBinding> {
        let store = self.bindings.lock().clone()?;
        store.for_document(path).unwrap_or_else(|error| {
            eprintln!("{} opens unbound: {error}", path.display());
            None
        })
    }

    pub(crate) fn with<T>(&self, action: impl FnOnce(&PlanningDocument) -> Answer<T>) -> Answer<T> {
        self.source
            .with(action)
            .unwrap_or_else(|| Err("no document is open".to_owned()))
    }

    /// Run a command that changes the document, and tell the visualizer about it.
    ///
    /// A rig the operator just patched has to appear in the picture now, not on whatever the
    /// renderer's next reconnection would have been.
    ///
    /// Every write is one gesture, or part of the gesture a caller opened around several, so a
    /// bound document journals it for its desk.
    pub(crate) fn change<T>(
        &self,
        action: impl FnOnce(&PlanningDocument) -> Answer<T>,
    ) -> Answer<T> {
        self.gesture(|| {
            let outcome = self.with(action);
            if outcome.is_ok() {
                self.source.mark_changed();
            }
            outcome
        })
    }
}

fn summarize(document: &PlanningDocument) -> Answer<DocumentSummary> {
    let snapshot = document
        .patch_snapshot()
        .map_err(|error| error.to_string())?;
    let paperwork = document
        .paperwork_metadata()
        .map_err(|error| error.to_string())?;
    let universes: HashSet<u16> = snapshot
        .fixtures
        .iter()
        .flat_map(|fixture| fixture.patch.split_patches.iter())
        .filter_map(|split| split.universe)
        .collect();
    let last_saved_at = std::fs::metadata(document.path())
        .and_then(|metadata| metadata.modified())
        .and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .map_err(std::io::Error::other)
        })
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    Ok(DocumentSummary {
        show_id: document.show_id().0.to_string(),
        name: document.name().map_err(|error| error.to_string())?,
        path: document.path().display().to_string(),
        fixture_count: snapshot.fixtures.len(),
        file_name: document
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
        lighting_designer: paperwork.lighting_designer,
        show_version: paperwork.show_version,
        venue: paperwork.venue,
        contact_email: paperwork.contact_email,
        contact_phone: paperwork.contact_phone,
        project: paperwork.project,
        show_date: paperwork.show_date,
        company_logo: paperwork.company_logo,
        last_saved_at,
        universe_count: universes.len(),
    })
}

/// A show created on this computer starts with the lighting designer **Make Default** kept here.
///
/// Nothing is filled in when no designer was made the default, or when the file keeping it cannot be
/// read; the show is created either way.
fn with_lighting_designer_default(
    app: &tauri::AppHandle,
    session: &Session,
) -> Option<DocumentSummary> {
    let dir = crate::portable::app_data_dir(app).ok()?;
    let default = crate::company_logo::read_default(&dir).ok()??;
    session
        .change(|document| {
            let mut paperwork = document
                .paperwork_metadata()
                .map_err(|error| error.to_string())?;
            paperwork.lighting_designer = default.lighting_designer.clone();
            paperwork.contact_phone = default.contact_phone.clone();
            paperwork.contact_email = default.contact_email.clone();
            paperwork.company_logo = default.company_logo.clone();
            document
                .save_paperwork_metadata(&paperwork)
                .map_err(|error| error.to_string())?;
            summarize(document)
        })
        .ok()
}

#[tauri::command]
pub fn create_document(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    discovery: tauri::State<'_, Discovery>,
    path: String,
    name: String,
) -> Answer<DocumentSummary> {
    let summary = session.open_path(Path::new(&path), Some(&name))?;
    let summary = with_lighting_designer_default(&app, &session).unwrap_or(summary);
    discovery.announce_document(Some(summary.name.clone()));
    announce_document_change(&app, &window)?;
    Ok(summary)
}

#[tauri::command]
pub fn open_document(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    discovery: tauri::State<'_, Discovery>,
    path: String,
) -> Answer<DocumentSummary> {
    let summary = session.open_path(Path::new(&path), None)?;
    discovery.announce_document(Some(summary.name.clone()));
    announce_document_change(&app, &window)?;
    Ok(summary)
}

#[tauri::command]
pub fn document_summary(session: tauri::State<'_, Session>) -> Answer<Option<DocumentSummary>> {
    session.source.with(summarize).transpose()
}

#[tauri::command]
pub fn save_document_paperwork(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    paperwork: PaperworkInput,
) -> Answer<DocumentSummary> {
    let summary = session.change(|document| {
        document
            .save_paperwork_metadata(&PaperworkMetadata {
                lighting_designer: paperwork.lighting_designer,
                show_version: paperwork.show_version,
                venue: paperwork.venue,
                contact_email: paperwork.contact_email,
                contact_phone: paperwork.contact_phone,
                project: paperwork.project,
                show_date: paperwork.show_date,
                company_logo: paperwork.company_logo,
            })
            .map_err(|error| error.to_string())?;
        summarize(document)
    })?;
    announce_document_change(&app, &window)?;
    Ok(summary)
}

/// Portable Art-Net and sACN receiver intent for the current planning document.
#[tauri::command]
pub fn live_dmx_inputs(session: tauri::State<'_, Session>) -> Answer<LiveDmxInputs> {
    session.with(|document| {
        document
            .live_dmx_inputs()
            .map_err(|error| error.to_string())
    })
}

/// Validate and atomically replace the current document's receiver intent.
#[tauri::command]
pub fn save_live_dmx_inputs(
    session: tauri::State<'_, Session>,
    inputs: LiveDmxInputs,
) -> Answer<LiveDmxInputs> {
    session.change(|document| {
        document
            .save_live_dmx_inputs(&inputs)
            .map_err(|error| error.to_string())?;
        document
            .live_dmx_inputs()
            .map_err(|error| error.to_string())
    })
}

/// Writes the document to a new file as a different show and continues there.
///
/// The copy gets a new show identity and no desk binding: a Save As is a fork, and nothing done to
/// the copy can ever reach the desk show the original is bound to. The original keeps its binding
/// and its unconfirmed edits for the next time it is opened.
#[tauri::command]
pub fn save_document_as(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    discovery: tauri::State<'_, Discovery>,
    path: String,
) -> Answer<DocumentSummary> {
    let summary = session.fork_to(Path::new(&path))?;
    discovery.announce_document(Some(summary.name.clone()));
    announce_document_change(&app, &window)?;
    Ok(summary)
}

#[tauri::command]
pub fn rename_document(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    discovery: tauri::State<'_, Discovery>,
    name: String,
) -> Answer<()> {
    session.change(|document| document.rename(&name).map_err(|error| error.to_string()))?;
    // The record is what a desk's menu names, so a renamed document is a renamed offer.
    discovery.announce_document(Some(name));
    announce_document_change(&app, &window)?;
    Ok(())
}

#[tauri::command]
pub fn patch_snapshot(session: tauri::State<'_, Session>) -> Answer<SnapshotDto> {
    session.with(|document| {
        document
            .patch_snapshot()
            .map(SnapshotDto::from)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn patch_fixtures(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, crate::cad::CadState>,
    mutation: MutationDto,
) -> Answer<OutcomeDto> {
    apply_patch_mutation(&app, &session, &cad, Some(window.label()), mutation)
}

/// Apply one patch mutation and tell every window that needs to know.
///
/// Both ways into the patch come through here — a window's own command, and the local editing API —
/// because the announcement is the part that is easy to forget and expensive to omit. A mutation
/// that does not announce itself leaves every other window drawing a rig that no longer exists.
///
/// `origin` is the window that asked, when a window asked. It already holds the outcome the call
/// returned, so it is left out of the patch announcement rather than being told its own edit twice.
/// A call from the local API has no originating window, and then nothing is left out.
pub(crate) fn apply_patch_mutation(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &crate::cad::CadState,
    origin: Option<&str>,
    mutation: MutationDto,
) -> Answer<OutcomeDto> {
    let outcome = session.change(|document| {
        let request_id = mutation.request_id.clone();
        let command = mutation.into_command(document.show_id());
        let result = document
            .patch_fixtures(command)
            .map_err(|error| error.to_string())?;
        Ok(OutcomeDto {
            request_id,
            replayed: result.replayed,
            changed: result.changed,
            change: ChangeDto::new(result.change, result.event_sequence),
        })
    })?;
    if outcome.changed {
        // Every window that did not make the edit learns it as its own patch stream would have
        // delivered it, so its sheet applies one delta rather than reloading the whole rig.
        let change = serde_json::to_value(&outcome.change).map_err(|error| error.to_string())?;
        match origin {
            Some(origin) => {
                crate::windows::broadcast(app, origin, crate::windows::PATCH_CHANGE_EVENT, change)?;
            }
            None => {
                crate::windows::broadcast_all(app, crate::windows::PATCH_CHANGE_EVENT, change)?;
            }
        }
        // The rig drawings are the same edit seen from the CAD side, and every window draws them,
        // including the one that patched.
        let revision = session
            .with(|document| document.patch_revision().map_err(|error| error.to_string()))?;
        crate::cad::emit_scene_state_delta(app, session, cad, revision)?;
    }
    Ok(outcome)
}

/// Say that the open document itself changed — a different file, a new name, fresh paperwork.
///
/// The summary is not carried in the event on purpose: a window that hears this reads the session
/// again, which is the same authority it read at startup and cannot go stale in transit.
pub(crate) fn announce_document_change(
    app: &tauri::AppHandle,
    window: &tauri::Window,
) -> Answer<()> {
    crate::windows::broadcast(
        app,
        window.label(),
        crate::windows::DOCUMENT_CHANGED_EVENT,
        (),
    )
}

/// Portable media servers, advertised sources, surfaces, LED modules and projectors.
#[tauri::command]
pub fn media_layout(session: tauri::State<'_, Session>) -> Answer<MediaLayoutSnapshot> {
    session.with(|document| document.media_layout().map_err(|error| error.to_string()))
}

/// Connects to a configured CITP peer and returns its numeric advertised outputs.
#[tauri::command]
pub async fn inspect_citp_server(
    host: String,
    port: u16,
) -> Result<Vec<light_media::MediaPreviewSource>, String> {
    let address = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|error| error.to_string())?
        .next()
        .ok_or_else(|| "CITP host resolved to no address".to_owned())?;
    let mut client = light_media::CitpClient::connect(address, std::time::Duration::from_secs(3))
        .await
        .map_err(|error| error.to_string())?;
    client
        .preview_sources()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn discover_citp_servers() -> Result<Vec<light_media::DiscoveredCitpServer>, String> {
    light_media::discover_servers(std::time::Duration::from_millis(1_500))
        .await
        .map_err(|error| error.to_string())
}

/// One revision-safe and replay-safe media object mutation.
#[tauri::command]
pub fn apply_media_intent(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    intent: MediaObjectIntent,
) -> Answer<MediaLayoutOutcome> {
    apply_media_object_intent(&app, &session, Some(window.label()), intent)
}

/// The media layout edit itself, with no window to tell. Separate so it can be exercised without a
/// running application.
pub(crate) fn change_media_layout(
    session: &Session,
    intent: MediaObjectIntent,
) -> Answer<MediaLayoutOutcome> {
    session.change(|document| {
        document
            .apply_media_intent(intent)
            .map_err(|error| error.to_string())
    })
}

/// One media layout edit, applied the same way whether a window or the local editing API asked.
///
/// Every other window hears that the layout moved and reads it again, so a server added by an
/// external tool appears in an open Media workspace without reopening it. A call with no
/// originating window — the local API — tells every window.
pub(crate) fn apply_media_object_intent(
    app: &tauri::AppHandle,
    session: &Session,
    origin: Option<&str>,
    intent: MediaObjectIntent,
) -> Answer<MediaLayoutOutcome> {
    let outcome = change_media_layout(session, intent)?;
    if outcome.changed {
        match origin {
            Some(origin) => crate::windows::broadcast(
                app,
                origin,
                crate::windows::MEDIA_LAYOUT_CHANGED_EVENT,
                (),
            )?,
            None => {
                crate::windows::broadcast_all(app, crate::windows::MEDIA_LAYOUT_CHANGED_EVENT, ())?
            }
        }
    }
    Ok(outcome)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedMediaFallback {
    reference: ManagedFallbackReference,
    outcome: MediaLayoutOutcome,
}

/// Copies a fallback image into the portable show; the source path is never persisted.
#[tauri::command]
pub fn import_media_fallback(
    session: tauri::State<'_, Session>,
    path: PathBuf,
) -> Answer<ImportedMediaFallback> {
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Fallback image")
        .to_owned();
    session.change(|document| {
        let (reference, outcome) = document
            .import_media_fallback(Uuid::new_v4().to_string(), 0, name, &bytes)
            .map_err(|error| error.to_string())?;
        Ok(ImportedMediaFallback { reference, outcome })
    })
}

/// The patch layers the document already carries.
///
/// Layers travel with the show, so a document the desk wrote opens here with its own layers rather
/// than with one invented default that its fixtures do not belong to.
#[tauri::command]
pub fn patch_layers(session: tauri::State<'_, Session>) -> Answer<Vec<PatchLayerDto>> {
    session.with(|document| {
        let stored = document
            .objects("patch_layer")
            .map_err(|error| error.to_string())?;
        Ok(stored
            .into_iter()
            .filter_map(|object| {
                Some(PatchLayerDto {
                    id: object.id.clone(),
                    name: object.body.get("name")?.as_str()?.to_owned(),
                    order: object.body.get("order").and_then(|order| order.as_i64())? as i32,
                    locked: object
                        .body
                        .get("locked")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                    visible_2d: object
                        .body
                        .get("visible2d")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true),
                    visible_3d: object
                        .body
                        .get("visible3d")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true),
                })
            })
            .collect())
    })
}

/// Store one patch layer in the document, as the desk stores it.
#[tauri::command]
pub fn save_patch_layer(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, crate::cad::CadState>,
    layer: PatchLayerDto,
) -> Answer<PatchLayerDto> {
    if layer.name.trim().is_empty() {
        return Err("a patch layer needs a name".to_owned());
    }
    let saved = session.change(|document| {
        document
            .put_object(
                "patch_layer",
                &layer.id,
                &serde_json::json!({
                    "id": layer.id,
                    "name": layer.name,
                    "order": layer.order,
                    "locked": layer.locked,
                    "visible2d": layer.visible_2d,
                    "visible3d": layer.visible_3d,
                }),
            )
            .map_err(|error| error.to_string())?;
        Ok(layer.clone())
    })?;
    let revision =
        session.with(|document| document.patch_revision().map_err(|error| error.to_string()))?;
    crate::cad::emit_scene_state_delta(&app, &session, &cad, revision)?;
    Ok(saved)
}

/// Remove one patch layer from the document.
///
/// The sheet moves the layer's fixtures to the default layer first, as one patch change, so no
/// fixture is left pointing at a layer that is gone. The default layer itself is where they go, so it
/// is never removed.
#[tauri::command]
pub fn delete_patch_layer(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, crate::cad::CadState>,
    id: String,
) -> Answer<bool> {
    if id == "default" {
        return Err("the default layer cannot be deleted".to_owned());
    }
    let deleted = session.change(|document| {
        document
            .delete_object("patch_layer", &id)
            .map_err(|error| error.to_string())
    })?;
    let revision =
        session.with(|document| document.patch_revision().map_err(|error| error.to_string()))?;
    crate::cad::emit_scene_state_delta(&app, &session, &cad, revision)?;
    Ok(deleted)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PatchLayerDto {
    pub id: String,
    pub name: String,
    pub order: i32,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "default_true", rename = "visible2d")]
    pub visible_2d: bool,
    #[serde(default = "default_true", rename = "visible3d")]
    pub visible_3d: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureVisibilityDto {
    pub fixture_id: Uuid,
    #[serde(default = "default_true")]
    pub visible_2d: bool,
    #[serde(default = "default_true")]
    pub visible_3d: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureNoteDto {
    pub fixture_id: Uuid,
    #[serde(default)]
    pub note: String,
}

#[tauri::command]
pub fn fixture_notes(session: tauri::State<'_, Session>) -> Answer<Vec<FixtureNoteDto>> {
    session.with(|document| {
        document
            .objects("fixture_note")
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|object| serde_json::from_value(object.body).map_err(|error| error.to_string()))
            .collect()
    })
}

#[tauri::command]
pub fn save_fixture_note(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, crate::cad::CadState>,
    note: FixtureNoteDto,
) -> Answer<FixtureNoteDto> {
    let saved = session.change(|document| {
        document
            .put_object(
                "fixture_note",
                &note.fixture_id.to_string(),
                &serde_json::to_value(&note).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        Ok(note.clone())
    })?;
    let revision =
        session.with(|document| document.patch_revision().map_err(|error| error.to_string()))?;
    crate::cad::emit_scene_state_delta(&app, &session, &cad, revision)?;
    Ok(saved)
}

#[tauri::command]
pub fn fixture_visibility(session: tauri::State<'_, Session>) -> Answer<Vec<FixtureVisibilityDto>> {
    session.with(|document| {
        document
            .objects("fixture_visibility")
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|object| serde_json::from_value(object.body).map_err(|error| error.to_string()))
            .collect()
    })
}

#[tauri::command]
pub fn save_fixture_visibility(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, crate::cad::CadState>,
    visibility: FixtureVisibilityDto,
) -> Answer<FixtureVisibilityDto> {
    let saved = session.change(|document| {
        document
            .put_object(
                "fixture_visibility",
                &visibility.fixture_id.to_string(),
                &serde_json::to_value(&visibility).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        Ok(visibility.clone())
    })?;
    let revision =
        session.with(|document| document.patch_revision().map_err(|error| error.to_string()))?;
    crate::cad::emit_scene_state_delta(&app, &session, &cad, revision)?;
    Ok(saved)
}

/// Set one preview value: a Simple-mode parameter, or a raw slot from Full DMX mode.
///
/// Session state of this window. It never reaches the show file, never becomes a preset or a cue,
/// and the visualizer receives it exactly as it receives a universe that arrived over the network.
#[tauri::command]
pub fn set_preview(
    session: tauri::State<'_, Session>,
    set: viz_planning::PreviewSet,
) -> Answer<()> {
    if !session.source.is_open() {
        return Err("no document is open".to_owned());
    }
    session.source.set_preview(set);
    Ok(())
}

/// Return fixtures to their defaults, or every fixture when none are named.
#[tauri::command]
pub fn clear_preview(session: tauri::State<'_, Session>, fixtures: Vec<Uuid>) -> Answer<()> {
    session.source.clear_preview_fixtures(&fixtures);
    Ok(())
}

/// Whether the window is currently driving anything, for the surface that says so.
#[tauri::command]
pub fn preview_is_active(session: tauri::State<'_, Session>) -> bool {
    session.source.preview_is_active()
}

/// This computer's fixture library, when the editor has been pointed at one that exists.
pub(crate) fn open_library(session: &Session) -> Option<FixtureLibrary> {
    let path = session.library_path.lock().clone()?;
    path.exists()
        .then(|| FixtureLibrary::open(&path).ok())
        .flatten()
}

/// The fixtures the operator can patch from, for the sheet's fixture browser.
#[tauri::command]
pub fn library_profiles(session: tauri::State<'_, Session>) -> Answer<Vec<LibraryProfile>> {
    let Some(path) = session.library_path.lock().clone() else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let library = FixtureLibrary::open(&path).map_err(|error| error.to_string())?;
    let profiles = library.profiles().map_err(|error| error.to_string())?;
    profiles
        .into_iter()
        .map(|profile| {
            Ok(LibraryProfile {
                id: profile.id.0.to_string(),
                revision: profile.revision,
                manufacturer: profile.manufacturer.clone(),
                name: profile.name.clone(),
                profile: serde_json::to_value(profile).map_err(|error| error.to_string())?,
            })
        })
        .collect()
}

/// Authoring a fixture is a library edit, not a document edit: the profile belongs to this
/// machine's fixture library and every show opened here patches from it afterwards.
///
/// The library assigns the revision. The caller only states which revision it edited, so two
/// windows editing the same fixture cannot silently overwrite each other.
#[tauri::command]
pub fn save_library_profile(
    session: tauri::State<'_, Session>,
    profile: serde_json::Value,
    expected_revision: u32,
) -> Answer<LibraryProfile> {
    let Some(path) = session.library_path.lock().clone() else {
        return Err("This editor has no fixture library attached.".into());
    };
    let profile: light_fixture::FixtureProfile =
        serde_json::from_value(profile).map_err(|error| error.to_string())?;
    let library = FixtureLibrary::open(&path).map_err(|error| error.to_string())?;
    let saved = library
        .save_profile(profile, expected_revision)
        .map_err(|error| error.to_string())?;
    Ok(LibraryProfile {
        id: saved.id.0.to_string(),
        revision: saved.revision,
        manufacturer: saved.manufacturer.clone(),
        name: saved.name.clone(),
        profile: serde_json::to_value(saved).map_err(|error| error.to_string())?,
    })
}

/// Removes one immutable revision. A show already patched against it keeps its own snapshot.
#[tauri::command]
pub fn delete_library_profile_revision(
    session: tauri::State<'_, Session>,
    id: String,
    revision: u32,
) -> Answer<bool> {
    let Some(path) = session.library_path.lock().clone() else {
        return Err("This editor has no fixture library attached.".into());
    };
    let id = light_core::FixtureId(Uuid::parse_str(&id).map_err(|error| error.to_string())?);
    let library = FixtureLibrary::open(&path).map_err(|error| error.to_string())?;
    library
        .delete_profile(id, revision)
        .map_err(|error| error.to_string())
}

/// The generic bodies a profile can be drawn as, for the editor's Body picker.
#[tauri::command]
pub fn fixture_body_catalogue() -> Vec<BodyModelDto> {
    light_fixture::body_catalogue::BODY_CATALOGUE
        .iter()
        .map(|model| BodyModelDto {
            id: model.id.into(),
            label: model.label.into(),
            group: model.group.label().into(),
        })
        .collect()
}

/// The canonical attribute registry a profile channel names as its role.
///
/// The Architect has no desk to ask, so it reads the same built-in registry the desk configures
/// from. Show-specific custom attributes are a desk concern and are deliberately absent.
#[tauri::command]
pub fn attribute_registry() -> Vec<AttributeDescriptorDto> {
    // No show is open to have arranged its own groups, so the grouping is the recommended one.
    let configuration = light_core::AttributeConfiguration::recommended();
    light_core::ATTRIBUTE_REGISTRY
        .iter()
        .filter(|descriptor| !light_core::built_in_attribute_is_retired(descriptor.id))
        .map(|descriptor| {
            let key = light_core::AttributeKey(descriptor.id.into());
            let group = configuration.activation_group_for(&key);
            let encoder_group = configuration
                .placement_for(&key)
                .map(|placement| placement.group);
            AttributeDescriptorDto {
                id: descriptor.id.into(),
                label: descriptor.label.into(),
                family: descriptor.family,
                value_type: descriptor.value_type,
                default_unit: descriptor.default_unit.map(str::to_owned),
                display_unit: descriptor.display_unit.map(str::to_owned),
                physical_unit: descriptor.physical_unit.map(str::to_owned),
                cyclic: descriptor.cyclic,
                recordable: descriptor.recordable,
                built_in: true,
                encoder_group,
                activation_group_id: group.map(|group| group.id.clone()),
                activation_group_label: group.map(|group| group.label.clone()),
            }
        })
        .collect()
}

mod mvr;
pub use mvr::*;

#[tauri::command]
pub fn recent_documents(session: tauri::State<'_, Session>) -> Vec<String> {
    session.recent_paths()
}

/// Metadata for browsing recent files, without opening or changing the active document.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentDocument {
    path: String,
    last_saved_at: Option<u64>,
}

#[tauri::command]
pub fn recent_document_details(session: tauri::State<'_, Session>) -> Vec<RecentDocument> {
    session
        .recent_paths()
        .into_iter()
        .map(|path| {
            let last_saved_at = std::fs::metadata(&path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs());
            RecentDocument {
                path,
                last_saved_at,
            }
        })
        .collect()
}
