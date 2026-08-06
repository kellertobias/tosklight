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
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use viz_document::PlanningDocument;
use viz_planning::SceneSource;

/// The open document, shared with the visualizer.
///
/// The window and the renderer read the same `SceneSource`, so a fixture patched here is in the
/// next snapshot the visualizer asks for. There is no second copy to keep in step.
#[derive(Default)]
pub struct Session {
    source: SceneSource,
    library_path: Mutex<Option<PathBuf>>,
    recent: Mutex<Option<RecentShow>>,
}

/// What the window title bar and the file menu need to know.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub show_id: String,
    pub name: String,
    pub path: String,
    pub fixture_count: usize,
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
        let document = match created {
            Some(name) => PlanningDocument::create(path, name),
            None => PlanningDocument::open(path),
        }
        .map_err(|error| error.to_string())?;
        let document = self.attach_library(document)?;
        let summary = summarize(&document)?;
        self.source.open(document);
        if let Some(recent) = self.recent.lock().as_ref() {
            recent.remember(path);
        }
        Ok(summary)
    }

    fn with<T>(&self, action: impl FnOnce(&PlanningDocument) -> Answer<T>) -> Answer<T> {
        self.source
            .with(action)
            .unwrap_or_else(|| Err("no document is open".to_owned()))
    }

    /// Run a command that changes the document, and tell the visualizer about it.
    ///
    /// A rig the operator just patched has to appear in the picture now, not on whatever the
    /// renderer's next reconnection would have been.
    fn change<T>(&self, action: impl FnOnce(&PlanningDocument) -> Answer<T>) -> Answer<T> {
        let outcome = self.with(action);
        if outcome.is_ok() {
            self.source.mark_changed();
        }
        outcome
    }
}

fn summarize(document: &PlanningDocument) -> Answer<DocumentSummary> {
    let snapshot = document
        .patch_snapshot()
        .map_err(|error| error.to_string())?;
    Ok(DocumentSummary {
        show_id: document.show_id().0.to_string(),
        name: document.name().map_err(|error| error.to_string())?,
        path: document.path().display().to_string(),
        fixture_count: snapshot.fixtures.len(),
    })
}

#[tauri::command]
pub fn create_document(
    session: tauri::State<'_, Session>,
    discovery: tauri::State<'_, Discovery>,
    path: String,
    name: String,
) -> Answer<DocumentSummary> {
    let summary = session.open_path(Path::new(&path), Some(&name))?;
    discovery.announce_document(Some(summary.name.clone()));
    Ok(summary)
}

#[tauri::command]
pub fn open_document(
    session: tauri::State<'_, Session>,
    discovery: tauri::State<'_, Discovery>,
    path: String,
) -> Answer<DocumentSummary> {
    let summary = session.open_path(Path::new(&path), None)?;
    discovery.announce_document(Some(summary.name.clone()));
    Ok(summary)
}

#[tauri::command]
pub fn document_summary(session: tauri::State<'_, Session>) -> Answer<Option<DocumentSummary>> {
    session.source.with(summarize).transpose()
}

/// Writes a complete copy of the document. The result is an ordinary show file the desk opens.
#[tauri::command]
pub fn save_document_as(session: tauri::State<'_, Session>, path: String) -> Answer<()> {
    session.with(|document| {
        document
            .save_as(Path::new(&path))
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn rename_document(
    session: tauri::State<'_, Session>,
    discovery: tauri::State<'_, Discovery>,
    name: String,
) -> Answer<()> {
    session.change(|document| document.rename(&name).map_err(|error| error.to_string()))?;
    // The record is what a desk's menu names, so a renamed document is a renamed offer.
    discovery.announce_document(Some(name));
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
    session: tauri::State<'_, Session>,
    mutation: MutationDto,
) -> Answer<OutcomeDto> {
    session.change(|document| {
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
                })
            })
            .collect())
    })
}

/// Store one patch layer in the document, as the desk stores it.
#[tauri::command]
pub fn save_patch_layer(
    session: tauri::State<'_, Session>,
    layer: PatchLayerDto,
) -> Answer<PatchLayerDto> {
    if layer.name.trim().is_empty() {
        return Err("a patch layer needs a name".to_owned());
    }
    session.change(|document| {
        document
            .put_object(
                "patch_layer",
                &layer.id,
                &serde_json::json!({
                    "id": layer.id,
                    "name": layer.name,
                    "order": layer.order,
                }),
            )
            .map_err(|error| error.to_string())?;
        Ok(layer.clone())
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PatchLayerDto {
    pub id: String,
    pub name: String,
    pub order: i32,
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

#[tauri::command]
pub fn export_mvr(session: tauri::State<'_, Session>, path: String) -> Answer<usize> {
    session.with(|document| {
        let export = document.export_mvr().map_err(|error| error.to_string())?;
        std::fs::write(Path::new(&path), &export.data).map_err(|error| error.to_string())?;
        Ok(export.summary.fixtures)
    })
}

/// What the archive holds and how it lands here, before anything is written.
#[tauri::command]
pub fn preview_mvr(session: tauri::State<'_, Session>, path: String) -> Answer<MvrPreviewDto> {
    session.with(|document| {
        let archive = read_archive(&path)?;
        let preview = document
            .preview_mvr(&archive)
            .map_err(|error| error.to_string())?;
        Ok(MvrPreviewDto {
            fixtures: preview
                .fixtures
                .into_iter()
                .map(|fixture| MvrPreviewFixtureDto {
                    uuid: fixture.uuid.to_string(),
                    name: fixture.name,
                    gdtf_spec: fixture.gdtf_spec,
                    gdtf_mode: fixture.gdtf_mode,
                    universe: fixture.universe,
                    address: fixture.address,
                    matched: fixture.matched,
                    conflicted: fixture.conflicted,
                })
                .collect(),
            scenery: preview.scenery,
            missing_profiles: preview.missing_profiles,
            address_conflicts: preview.address_conflicts,
        })
    })
}

/// Imports the archive, with whatever the operator decided about the fixtures that needed a
/// decision. A fixture with no decision keeps the desk's own default handling.
#[tauri::command]
pub fn import_mvr(
    session: tauri::State<'_, Session>,
    path: String,
    resolutions: HashMap<String, ResolutionDto>,
) -> Answer<MvrImportReport> {
    let resolutions = decode_resolutions(resolutions)?;
    session.change(|document| {
        let archive = read_archive(&path)?;
        let outcome = document
            .import_mvr(archive, resolutions.clone())
            .map_err(|error| error.to_string())?;
        Ok(MvrImportReport {
            imported_fixtures: outcome.imported_fixtures,
            unresolved_fixtures: outcome.unresolved_fixtures,
            warnings: outcome.warnings,
        })
    })
}

fn read_archive(path: &str) -> Answer<light_mvr::MvrDocument> {
    let data = std::fs::read(Path::new(path)).map_err(|error| error.to_string())?;
    PlanningDocument::read_mvr(&data).map_err(|error| error.to_string())
}

/// The operator's decisions, as the import service understands them. An action it does not know
/// is refused rather than quietly treated as the default.
fn decode_resolutions(
    resolutions: HashMap<String, ResolutionDto>,
) -> Answer<HashMap<Uuid, MvrImportResolution>> {
    resolutions
        .into_iter()
        .map(|(uuid, resolution)| {
            let uuid = Uuid::parse_str(&uuid)
                .map_err(|error| format!("{uuid} is not a fixture identifier: {error}"))?;
            let decision = match resolution.action.as_str() {
                "import" => MvrImportResolution::Import,
                "skip" => MvrImportResolution::Skip,
                "import_unpatched" => MvrImportResolution::ImportUnpatched,
                "replace" => MvrImportResolution::Replace,
                "address" => MvrImportResolution::Address {
                    universe: resolution.universe.ok_or("an address needs a universe")?,
                    address: resolution.address.ok_or("an address needs an address")?,
                },
                other => return Err(format!("{other} is not an import resolution")),
            };
            Ok((uuid, decision))
        })
        .collect()
}

#[derive(Debug, Deserialize)]
pub struct ResolutionDto {
    pub action: String,
    pub universe: Option<u16>,
    pub address: Option<u16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MvrPreviewDto {
    pub fixtures: Vec<MvrPreviewFixtureDto>,
    pub scenery: usize,
    pub missing_profiles: Vec<String>,
    pub address_conflicts: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MvrPreviewFixtureDto {
    pub uuid: String,
    pub name: String,
    pub gdtf_spec: String,
    pub gdtf_mode: String,
    pub universe: Option<u16>,
    pub address: Option<u16>,
    pub matched: bool,
    pub conflicted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MvrImportReport {
    pub imported_fixtures: usize,
    pub unresolved_fixtures: usize,
    pub warnings: Vec<String>,
}
