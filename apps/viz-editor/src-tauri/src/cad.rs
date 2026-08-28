//! Canonical scene and command boundary for the web-owned CAD planning surface.
//!
//! The canvases are deliberately only views. Selection, fixture transforms, revisions and mount
//! relationships live here beside the open planning document, so closing a window cannot lose
//! show data and a stale drag cannot overwrite a newer Patch edit. Every editor window shows the
//! same scene, which is why the deltas below are broadcast rather than sent to one window.

use crate::contract::{FixtureDto, MutationDto};
use crate::session::Session;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use light_application::PatchSnapshot;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
};
use tauri::Emitter;
use uuid::Uuid;

pub const SCENE_DELTA_EVENT: &str = "cad-scene-delta";
pub const SELECTION_DELTA_EVENT: &str = "cad-selection-delta";

#[tauri::command]
pub fn cad_export_pdf(path: String, bytes_base64: String) -> Result<(), String> {
    let bytes = STANDARD
        .decode(bytes_base64)
        .map_err(|error| format!("The PDF data is invalid: {error}"))?;
    if !bytes.starts_with(b"%PDF-") {
        return Err("The exported document is not a PDF".to_string());
    }
    fs::write(&path, bytes).map_err(|error| format!("Could not save {path}: {error}"))
}

#[derive(Default)]
pub struct CadState {
    selection: Mutex<SelectionState>,
    history: Mutex<History>,
    drawings: Mutex<HashMap<String, Option<CadDrawing>>>,
}

#[derive(Default)]
struct SelectionState {
    revision: u64,
    ids: Vec<Uuid>,
}

#[derive(Default)]
struct History {
    undo: Vec<TransformRecord>,
    redo: Vec<TransformRecord>,
}

#[derive(Clone)]
struct TransformRecord {
    before: Vec<EntityTransform>,
    after: Vec<EntityTransform>,
    before_attachments: Vec<RigAttachment>,
    after_attachments: Vec<RigAttachment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityTransform {
    pub id: Uuid,
    pub position_millimetres: [i32; 3],
    pub rotation_degrees: [f32; 3],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadEntity {
    /// Unique physical placement: the root fixture ID or one multi-patch instance ID.
    pub id: Uuid,
    /// Shared programming/selection identity for the root and every multi-patch instance.
    pub logical_fixture_id: Uuid,
    pub name: String,
    pub fixture_number: Option<u32>,
    pub fixture_display_id: String,
    pub dmx_address: String,
    pub fixture_profile: String,
    pub mode: String,
    pub note: String,
    pub kind: String,
    pub fixture_type: String,
    pub drawing_id: String,
    pub layer_id: String,
    pub selectable: bool,
    pub position_millimetres: [i32; 3],
    pub rotation_degrees: [f32; 3],
    pub size_millimetres: [f32; 3],
    pub output_direction: [f32; 3],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadProjection {
    pub view: String,
    pub svg: String,
    pub view_box_millimetres: [f32; 4],
    pub origin_millimetres: [f32; 2],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadDrawing {
    pub id: String,
    pub projections: Vec<CadProjection>,
    pub live_meshes: Vec<CadLiveMesh>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadLiveMesh {
    pub pose: String,
    pub triangles: Vec<CadLiveTriangle>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadLiveTriangle {
    pub points_millimetres: [[f32; 3]; 3],
    pub colour: [f32; 3],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RigAttachment {
    pub fixture_id: Uuid,
    pub truss_member_id: Uuid,
    pub mounting_point_id: String,
    pub local_transform: EntityTransform,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadSceneSnapshot {
    pub show_id: Uuid,
    pub scene_revision: u64,
    pub selection_revision: u64,
    pub entities: Vec<CadEntity>,
    pub drawings: Vec<CadDrawing>,
    pub selected_ids: Vec<Uuid>,
    pub attachments: Vec<RigAttachment>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadSceneDelta {
    pub scene_revision: u64,
    pub upserted: Vec<CadEntity>,
    pub drawings: Vec<CadDrawing>,
    pub removed_ids: Vec<Uuid>,
    pub attachments: Vec<RigAttachment>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionDelta {
    pub revision: u64,
    pub selected_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionIntent {
    pub expected_revision: u64,
    pub selected_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformIntent {
    pub expected_scene_revision: u64,
    pub entity_ids: Vec<Uuid>,
    pub delta_millimetres: [i32; 3],
    #[serde(default)]
    pub snap_to_mounts: bool,
    #[serde(default)]
    pub spread: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformOutcome {
    pub scene_revision: u64,
    pub transforms: Vec<EntityTransform>,
    pub attachments: Vec<RigAttachment>,
}

#[tauri::command]
pub fn cad_scene_snapshot(
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
) -> Result<CadSceneSnapshot, String> {
    snapshot(&session, &cad)
}

#[tauri::command]
pub fn cad_replace_selection(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    intent: SelectionIntent,
) -> Result<SelectionDelta, String> {
    let known = selectable_ids(&session)?;
    let mut selection = cad.selection.lock();
    if selection.revision != intent.expected_revision {
        return Err(format!(
            "CAD selection changed at revision {}; refresh before replacing revision {}",
            selection.revision, intent.expected_revision
        ));
    }
    let mut unique = BTreeSet::new();
    selection.ids = intent
        .selected_ids
        .into_iter()
        .filter(|id| known.contains(id) && unique.insert(*id))
        .collect();
    selection.revision = selection.revision.saturating_add(1);
    let delta = SelectionDelta {
        revision: selection.revision,
        selected_ids: selection.ids.clone(),
    };
    drop(selection);
    session
        .scene_source()
        .set_selection(delta.selected_ids.clone());
    app.emit(SELECTION_DELTA_EVENT, &delta)
        .map_err(|error| error.to_string())?;
    Ok(delta)
}

#[tauri::command]
pub fn cad_transform(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    intent: TransformIntent,
) -> Result<TransformOutcome, String> {
    if intent.entity_ids.is_empty() {
        return Err("Select at least one fixture or venue object to move".to_owned());
    }
    let ordered_ids = intent.entity_ids;
    let ids: BTreeSet<Uuid> = ordered_ids.iter().copied().collect();
    if ids.len() != ordered_ids.len() {
        return Err("A CAD transform cannot contain the same entity more than once".to_owned());
    }
    if !ids.is_subset(&selectable_ids(&session)?) {
        return Err("One or more selected CAD entities belong to a locked layer".to_owned());
    }
    let before = session.with(|document| {
        let patch = document
            .patch_snapshot()
            .map_err(|error| error.to_string())?;
        if patch.patch_revision.value() != intent.expected_scene_revision {
            return Err(format!(
                "The rig changed at revision {}; refresh before committing revision {}",
                patch.patch_revision.value(),
                intent.expected_scene_revision
            ));
        }
        selected_transforms(&patch, &ids)
    })?;
    if before.len() != ids.len() {
        return Err("One or more selected CAD entities no longer exist".to_owned());
    }
    let before_attachments = attachments(&session)?
        .into_iter()
        .filter(|attachment| ids.contains(&attachment.fixture_id))
        .collect::<Vec<_>>();
    let mut after = moved_transforms(
        &before,
        &ordered_ids,
        intent.delta_millimetres,
        intent.spread,
    );
    let anchor = (intent.spread && ordered_ids.len() > 1)
        .then(|| {
            before
                .iter()
                .find(|item| item.id == ordered_ids[0])
                .cloned()
        })
        .flatten();
    if intent.snap_to_mounts {
        snap_transforms(&session, &mut after)?;
        if let Some(anchor) = anchor
            && let Some(first) = after.iter_mut().find(|item| item.id == anchor.id)
        {
            *first = anchor;
        }
    }
    let revision = apply_transforms(&session, intent.expected_scene_revision, &after)?;
    let changed_attachments = if intent.snap_to_mounts {
        snap_attachments(&session, &after)?
    } else {
        clear_attachments(&session, &after)?;
        Vec::new()
    };
    let after_attachments = attachments(&session)?
        .into_iter()
        .filter(|attachment| ids.contains(&attachment.fixture_id))
        .collect::<Vec<_>>();
    let mut history = cad.history.lock();
    history.undo.push(TransformRecord {
        before,
        after: after.clone(),
        before_attachments,
        after_attachments,
    });
    history.redo.clear();
    drop(history);
    emit_scene_delta(&app, &session, &cad, revision, Vec::new())?;
    Ok(TransformOutcome {
        scene_revision: revision,
        transforms: after,
        attachments: changed_attachments,
    })
}

fn moved_transforms(
    before: &[EntityTransform],
    ordered_ids: &[Uuid],
    delta_millimetres: [i32; 3],
    spread: bool,
) -> Vec<EntityTransform> {
    let positions = ordered_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect::<HashMap<_, _>>();
    before
        .iter()
        .cloned()
        .map(|mut transform| {
            let index = positions[&transform.id];
            let factor = if spread && ordered_ids.len() > 1 {
                index as f64 / (ordered_ids.len() - 1) as f64
            } else {
                1.0
            };
            for axis in 0..3 {
                let delta = (delta_millimetres[axis] as f64 * factor).round() as i32;
                transform.position_millimetres[axis] =
                    transform.position_millimetres[axis].saturating_add(delta);
            }
            transform
        })
        .collect()
}

#[tauri::command]
pub fn cad_undo(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    expected_scene_revision: u64,
) -> Result<TransformOutcome, String> {
    let record = cad
        .history
        .lock()
        .undo
        .pop()
        .ok_or_else(|| "There is no CAD move to undo".to_owned())?;
    match apply_transforms(&session, expected_scene_revision, &record.before) {
        Ok(revision) => {
            restore_attachments(&session, &record.before, &record.before_attachments)?;
            cad.history.lock().redo.push(record.clone());
            emit_scene_delta(&app, &session, &cad, revision, Vec::new())?;
            Ok(TransformOutcome {
                scene_revision: revision,
                transforms: record.before,
                attachments: attachments(&session)?,
            })
        }
        Err(error) => {
            cad.history.lock().undo.push(record);
            Err(error)
        }
    }
}

#[tauri::command]
pub fn cad_redo(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    expected_scene_revision: u64,
) -> Result<TransformOutcome, String> {
    let record = cad
        .history
        .lock()
        .redo
        .pop()
        .ok_or_else(|| "There is no CAD move to redo".to_owned())?;
    match apply_transforms(&session, expected_scene_revision, &record.after) {
        Ok(revision) => {
            restore_attachments(&session, &record.after, &record.after_attachments)?;
            cad.history.lock().undo.push(record.clone());
            emit_scene_delta(&app, &session, &cad, revision, Vec::new())?;
            Ok(TransformOutcome {
                scene_revision: revision,
                transforms: record.after,
                attachments: attachments(&session)?,
            })
        }
        Err(error) => {
            cad.history.lock().redo.push(record);
            Err(error)
        }
    }
}

pub fn emit_scene_delta(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &CadState,
    scene_revision: u64,
    removed_ids: Vec<Uuid>,
) -> Result<(), String> {
    prune_selection(app, session, cad)?;
    let current = snapshot(session, cad)?;
    app.emit(
        SCENE_DELTA_EVENT,
        CadSceneDelta {
            scene_revision,
            upserted: current.entities,
            drawings: current.drawings,
            removed_ids,
            attachments: current.attachments,
        },
    )
    .map_err(|error| error.to_string())
}

/// Emit only document-owned CAD state. Layer locks and visibility do not change drawings, so this
/// deliberately avoids rebuilding every model projection on the UI action's critical path.
pub fn emit_scene_state_delta(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &CadState,
    scene_revision: u64,
) -> Result<(), String> {
    prune_selection(app, session, cad)?;
    let patch =
        session.with(|document| document.patch_snapshot().map_err(|error| error.to_string()))?;
    let locked = locked_layers(session)?;
    let hidden_fixtures = hidden_fixture_ids(session, "visible2d")?;
    let hidden_layers = hidden_layers(session, "visible2d")?;
    let notes = fixture_notes(session)?;
    let all_entities = entities(&patch, &locked, &notes);
    let removed_ids = all_entities
        .iter()
        .filter(|entity| {
            hidden_fixtures.contains(&entity.logical_fixture_id)
                || hidden_layers.contains(&entity.layer_id)
        })
        .map(|entity| entity.id)
        .collect();
    app.emit(
        SCENE_DELTA_EVENT,
        CadSceneDelta {
            scene_revision,
            upserted: all_entities
                .into_iter()
                .filter(|entity| {
                    !hidden_fixtures.contains(&entity.logical_fixture_id)
                        && !hidden_layers.contains(&entity.layer_id)
                })
                .collect(),
            drawings: Vec::new(),
            removed_ids,
            attachments: attachments(session)?,
        },
    )
    .map_err(|error| error.to_string())
}

fn prune_selection(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &CadState,
) -> Result<(), String> {
    let known = selectable_ids(session)?;
    let mut selection = cad.selection.lock();
    let previous = selection.ids.len();
    selection.ids.retain(|id| known.contains(id));
    if selection.ids.len() != previous {
        selection.revision = selection.revision.saturating_add(1);
        let delta = SelectionDelta {
            revision: selection.revision,
            selected_ids: selection.ids.clone(),
        };
        session
            .scene_source()
            .set_selection(delta.selected_ids.clone());
        app.emit(SELECTION_DELTA_EVENT, delta)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn snapshot(session: &Session, cad: &CadState) -> Result<CadSceneSnapshot, String> {
    let patch =
        session.with(|document| document.patch_snapshot().map_err(|error| error.to_string()))?;
    let selection = cad.selection.lock();
    let locked = locked_layers(session)?;
    let hidden_fixtures = hidden_fixture_ids(session, "visible2d")?;
    let hidden_layers = hidden_layers(session, "visible2d")?;
    Ok(CadSceneSnapshot {
        show_id: patch.show_id.0,
        scene_revision: patch.patch_revision.value(),
        selection_revision: selection.revision,
        entities: visible_entities(
            &patch,
            &locked,
            &hidden_fixtures,
            &hidden_layers,
            &fixture_notes(session)?,
        ),
        drawings: drawings(&patch, cad),
        selected_ids: selection.ids.clone(),
        attachments: attachments(session)?,
    })
}

fn visible_entities(
    snapshot: &PatchSnapshot,
    locked_layers: &BTreeSet<String>,
    hidden_fixtures: &BTreeSet<Uuid>,
    hidden_layers: &BTreeSet<String>,
    notes: &HashMap<Uuid, String>,
) -> Vec<CadEntity> {
    entities(snapshot, locked_layers, notes)
        .into_iter()
        .filter(|entity| {
            !hidden_fixtures.contains(&entity.logical_fixture_id)
                && !hidden_layers.contains(&entity.layer_id)
        })
        .collect()
}

fn entities(
    snapshot: &PatchSnapshot,
    locked_layers: &BTreeSet<String>,
    notes: &HashMap<Uuid, String>,
) -> Vec<CadEntity> {
    let profiles = snapshot
        .profile_revisions
        .iter()
        .map(|profile| ((profile.profile_id.0, profile.profile_revision), profile))
        .collect::<HashMap<_, _>>();
    snapshot
        .fixtures
        .iter()
        .flat_map(|fixture| {
            let profile = profiles.get(&(
                fixture.profile.profile_id.0,
                fixture.profile.profile_revision,
            ));
            let dimensions = profile
                .map(|profile| dimensions(&profile.profile_snapshot))
                .unwrap_or([500.0, 500.0, 500.0]);
            let fixture_type = profile
                .map(|profile| profile.fixture_type.as_str())
                .unwrap_or("fixture");
            let kind = profile
                .map(|profile| match profile.patch_policy {
                    light_fixture::PatchPolicy::VisualOnly => "venue",
                    _ => fixture_type,
                })
                .unwrap_or(fixture_type)
                .to_owned();
            let logical_fixture_id = fixture.patch.fixture_id.0;
            let fixture_profile = profile
                .map(|profile| {
                    let manufacturer = profile
                        .profile_snapshot
                        .get("manufacturer")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    let name = profile
                        .profile_snapshot
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Unknown fixture");
                    format!("{manufacturer} {name}").trim().to_owned()
                })
                .unwrap_or_else(|| "Unknown fixture".to_owned());
            let mode_id = fixture.profile.mode_id.to_string();
            let mode = profile
                .and_then(|profile| {
                    profile
                        .profile_snapshot
                        .get("modes")?
                        .as_array()?
                        .iter()
                        .find(|mode| {
                            mode.get("id").and_then(serde_json::Value::as_str)
                                == Some(mode_id.as_str())
                        })
                })
                .and_then(|mode| mode.get("name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Unknown mode")
                .to_owned();
            let note = notes.get(&logical_fixture_id).cloned().unwrap_or_default();
            let common = |id: Uuid,
                          name: String,
                          location: &light_fixture::FixtureLocation,
                          rotation: &light_fixture::FixtureVector,
                          dmx_address: String| CadEntity {
                id,
                logical_fixture_id,
                name,
                fixture_number: fixture.patch.fixture_number,
                fixture_display_id: fixture
                    .patch
                    .fixture_number
                    .map(|number| number.to_string())
                    .or_else(|| {
                        fixture
                            .patch
                            .virtual_fixture_number
                            .map(|number| format!("0.{number}"))
                    })
                    .unwrap_or_else(|| "—".to_owned()),
                dmx_address,
                fixture_profile: fixture_profile.clone(),
                mode: mode.clone(),
                note: note.clone(),
                kind: kind.clone(),
                fixture_type: fixture_type.to_owned(),
                drawing_id: profile.map_or_else(
                    || format!("unknown:{}", fixture.patch.fixture_id.0),
                    |profile| drawing_id(profile, fixture.profile.mode_id),
                ),
                layer_id: fixture.patch.layer_id.clone(),
                selectable: !locked_layers.contains(&fixture.patch.layer_id),
                position_millimetres: [location.x, location.y, location.z],
                rotation_degrees: [rotation.x, rotation.y, rotation.z],
                size_millimetres: dimensions,
                output_direction: output_direction(rotation),
            };
            let visual_only = profile.is_some_and(|profile| {
                profile.patch_policy == light_fixture::PatchPolicy::VisualOnly
            });
            let address = |universe: Option<u16>, address: Option<u16>| {
                if visual_only {
                    "Visual only".to_owned()
                } else if let (Some(universe), Some(address)) = (universe, address) {
                    format!("{universe}.{address}")
                } else {
                    "Unpatched".to_owned()
                }
            };
            let root_address = if fixture.patch.split_patches.len() > 1 {
                fixture
                    .patch
                    .split_patches
                    .iter()
                    .map(|patch| {
                        format!(
                            "S{} {}",
                            patch.split,
                            address(patch.universe, patch.address)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" / ")
            } else {
                address(fixture.patch.universe, fixture.patch.address)
            };
            std::iter::once(common(
                logical_fixture_id,
                fixture.patch.name.clone(),
                &fixture.patch.location,
                &fixture.patch.rotation,
                root_address,
            ))
            .chain(fixture.patch.multipatch.iter().map(|instance| {
                common(
                    instance.id,
                    if instance.name.trim().is_empty() {
                        fixture.patch.name.clone()
                    } else {
                        instance.name.clone()
                    },
                    &instance.location,
                    &instance.rotation,
                    address(instance.universe, instance.address),
                )
            }))
            .collect::<Vec<_>>()
        })
        .collect()
}

fn locked_layers(session: &Session) -> Result<BTreeSet<String>, String> {
    session.with(|document| {
        document
            .objects("patch_layer")
            .map(|objects| {
                objects
                    .into_iter()
                    .filter(|object| {
                        object
                            .body
                            .get("locked")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false)
                    })
                    .map(|object| object.id)
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

fn hidden_layers(session: &Session, property: &str) -> Result<BTreeSet<String>, String> {
    session.with(|document| {
        document
            .objects("patch_layer")
            .map(|objects| {
                objects
                    .into_iter()
                    .filter(|object| {
                        !object
                            .body
                            .get(property)
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true)
                    })
                    .map(|object| object.id)
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

fn hidden_fixture_ids(session: &Session, property: &str) -> Result<BTreeSet<Uuid>, String> {
    session.with(|document| {
        document
            .objects("fixture_visibility")
            .map(|objects| {
                objects
                    .into_iter()
                    .filter(|object| {
                        !object
                            .body
                            .get(property)
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true)
                    })
                    .filter_map(|object| {
                        object
                            .body
                            .get("fixtureId")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|id| Uuid::parse_str(id).ok())
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

fn fixture_notes(session: &Session) -> Result<HashMap<Uuid, String>, String> {
    session.with(|document| {
        document
            .objects("fixture_note")
            .map(|objects| {
                objects
                    .into_iter()
                    .filter_map(|object| {
                        let fixture_id = object
                            .body
                            .get("fixtureId")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|id| Uuid::parse_str(id).ok())?;
                        let note = object
                            .body
                            .get("note")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        Some((fixture_id, note))
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

fn selectable_ids(session: &Session) -> Result<BTreeSet<Uuid>, String> {
    let locked = locked_layers(session)?;
    let hidden_fixtures = hidden_fixture_ids(session, "visible2d")?;
    let hidden_layers = hidden_layers(session, "visible2d")?;
    session.with(|document| {
        document
            .patch_snapshot()
            .map(|snapshot| {
                snapshot
                    .fixtures
                    .into_iter()
                    .filter(|fixture| {
                        !locked.contains(&fixture.patch.layer_id)
                            && !hidden_layers.contains(&fixture.patch.layer_id)
                            && !hidden_fixtures.contains(&fixture.patch.fixture_id.0)
                    })
                    .map(|fixture| fixture.patch.fixture_id.0)
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

fn drawing_id(
    profile: &light_application::PatchProfileRevisionProjection,
    mode_id: Uuid,
) -> String {
    format!(
        "{}:{}:{}:{}",
        profile.profile_id.0, profile.profile_revision, profile.content_digest, mode_id
    )
}

fn drawings(snapshot: &PatchSnapshot, cad: &CadState) -> Vec<CadDrawing> {
    let mut cache = cad.drawings.lock();
    let profiles = snapshot
        .profile_revisions
        .iter()
        .map(|profile| ((profile.profile_id.0, profile.profile_revision), profile))
        .collect::<HashMap<_, _>>();
    snapshot
        .fixtures
        .iter()
        .filter_map(|fixture| {
            let stored = profiles.get(&(
                fixture.profile.profile_id.0,
                fixture.profile.profile_revision,
            ))?;
            let id = drawing_id(stored, fixture.profile.mode_id);
            cache
                .entry(id.clone())
                .or_insert_with(|| {
                    drawing(&id, &stored.profile_snapshot, Some(fixture.profile.mode_id))
                })
                .clone()
        })
        .collect()
}

fn drawing(id: &str, snapshot: &serde_json::Value, mode_id: Option<Uuid>) -> Option<CadDrawing> {
    let profile = serde_json::from_value::<light_fixture::FixtureProfile>(snapshot.clone()).ok()?;
    let live_meshes = viz_project::generate_live_projection_meshes_for_mode(&profile, mode_id)
        .unwrap_or_default()
        .into_iter()
        .map(|mesh| CadLiveMesh {
            pose: match mesh.pose {
                viz_project::LiveProjectionPose::Top => "top",
                viz_project::LiveProjectionPose::Elevation => "elevation",
            }
            .to_owned(),
            triangles: mesh
                .triangles
                .into_iter()
                .map(|triangle| CadLiveTriangle {
                    points_millimetres: triangle.points_millimetres,
                    colour: triangle.colour,
                })
                .collect(),
        })
        .collect();
    let generated;
    let projections = if let Some(projections) = profile.projection_assets.as_ref() {
        projections
    } else {
        generated = viz_project::generate_profile_projections(&profile).ok()?;
        &generated
    };
    let projections = projections
        .views
        .iter()
        .filter_map(|projection| {
            let encoded = projection
                .artwork_asset
                .strip_prefix("data:image/svg+xml;base64,")?;
            let svg = String::from_utf8(STANDARD.decode(encoded).ok()?).ok()?;
            Some(CadProjection {
                view: projection.view.wire().to_owned(),
                svg,
                view_box_millimetres: projection.view_box_millimetres,
                origin_millimetres: projection.origin_millimetres,
            })
        })
        .collect::<Vec<_>>();
    (!projections.is_empty()).then(|| CadDrawing {
        id: id.to_owned(),
        projections,
        live_meshes,
    })
}

fn dimensions(profile: &serde_json::Value) -> [f32; 3] {
    let physical = profile.get("physical").unwrap_or(profile);
    [
        number(physical, "width_millimetres", 500.0),
        number(physical, "depth_millimetres", 500.0),
        number(physical, "height_millimetres", 500.0),
    ]
}

fn number(value: &serde_json::Value, key: &str, fallback: f32) -> f32 {
    value
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .map_or(fallback, |value| value as f32)
        .max(20.0)
}

fn output_direction(rotation: &light_fixture::FixtureVector) -> [f32; 3] {
    let yaw = rotation.z.to_radians();
    let pitch = rotation.x.to_radians();
    [
        yaw.sin() * pitch.cos(),
        yaw.cos() * pitch.cos(),
        -pitch.sin(),
    ]
}

fn selected_transforms(
    snapshot: &PatchSnapshot,
    ids: &BTreeSet<Uuid>,
) -> Result<Vec<EntityTransform>, String> {
    Ok(snapshot
        .fixtures
        .iter()
        .filter(|fixture| ids.contains(&fixture.patch.fixture_id.0))
        .map(|fixture| EntityTransform {
            id: fixture.patch.fixture_id.0,
            position_millimetres: [
                fixture.patch.location.x,
                fixture.patch.location.y,
                fixture.patch.location.z,
            ],
            rotation_degrees: [
                fixture.patch.rotation.x,
                fixture.patch.rotation.y,
                fixture.patch.rotation.z,
            ],
        })
        .collect())
}

fn apply_transforms(
    session: &Session,
    expected_revision: u64,
    transforms: &[EntityTransform],
) -> Result<u64, String> {
    let desired = transforms
        .iter()
        .map(|transform| (transform.id, transform))
        .collect::<HashMap<_, _>>();
    session.change(|document| {
        let snapshot = document
            .patch_snapshot()
            .map_err(|error| error.to_string())?;
        if snapshot.patch_revision.value() != expected_revision {
            return Err(format!(
                "The rig changed at revision {}; refresh before committing revision {}",
                snapshot.patch_revision.value(),
                expected_revision
            ));
        }
        let fixtures = snapshot
            .fixtures
            .into_iter()
            .filter_map(|fixture| {
                let transform = desired.get(&fixture.patch.fixture_id.0)?;
                let mut fixture = FixtureDto::from(fixture);
                let delta = [
                    transform.position_millimetres[0] - fixture.location.x,
                    transform.position_millimetres[1] - fixture.location.y,
                    transform.position_millimetres[2] - fixture.location.z,
                ];
                fixture.location.x = transform.position_millimetres[0];
                fixture.location.y = transform.position_millimetres[1];
                fixture.location.z = transform.position_millimetres[2];
                fixture.rotation.x = transform.rotation_degrees[0];
                fixture.rotation.y = transform.rotation_degrees[1];
                fixture.rotation.z = transform.rotation_degrees[2];
                for instance in &mut fixture.multipatch {
                    instance.location.x = instance.location.x.saturating_add(delta[0]);
                    instance.location.y = instance.location.y.saturating_add(delta[1]);
                    instance.location.z = instance.location.z.saturating_add(delta[2]);
                }
                Some(fixture)
            })
            .collect::<Vec<_>>();
        if fixtures.len() != desired.len() {
            return Err("One or more selected CAD entities no longer exist".to_owned());
        }
        let command = MutationDto {
            request_id: Uuid::new_v4().to_string(),
            fixtures,
            remove_fixture_ids: Vec::new(),
            placements: Vec::new(),
        }
        .into_command(document.show_id());
        document
            .patch_fixtures_at(command, expected_revision)
            .map(|outcome| outcome.change.patch_revision.value())
            .map_err(|error| error.to_string())
    })
}

fn attachments(session: &Session) -> Result<Vec<RigAttachment>, String> {
    session.with(|document| {
        document
            .objects("rig_attachment")
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|object| serde_json::from_value(object.body).map_err(|error| error.to_string()))
            .collect()
    })
}

fn snap_attachments(
    session: &Session,
    moved: &[EntityTransform],
) -> Result<Vec<RigAttachment>, String> {
    let scene =
        session.with(|document| document.patch_snapshot().map_err(|error| error.to_string()))?;
    let entities = entities(&scene, &BTreeSet::new(), &fixture_notes(session)?);
    let trusses = entities
        .iter()
        .filter(|entity| {
            entity.kind == "venue"
                && (entity.name.to_lowercase().contains("truss")
                    || entity.name.to_lowercase().contains("pipe"))
        })
        .collect::<Vec<_>>();
    let mut created = Vec::new();
    for fixture in moved {
        let Some(truss) = trusses.iter().min_by_key(|truss| {
            let dy = fixture.position_millimetres[1] - truss.position_millimetres[1];
            let dz = fixture.position_millimetres[2] - truss.position_millimetres[2];
            i64::from(dy).pow(2) + i64::from(dz).pow(2)
        }) else {
            continue;
        };
        let dy = fixture.position_millimetres[1] - truss.position_millimetres[1];
        let dz = fixture.position_millimetres[2] - truss.position_millimetres[2];
        if i64::from(dy).pow(2) + i64::from(dz).pow(2) > 500_i64.pow(2) {
            continue;
        }
        let offset = fixture.position_millimetres[0] - truss.position_millimetres[0];
        if offset.abs() as f32 > truss.size_millimetres[0] / 2.0 + 250.0 {
            continue;
        }
        let attachment = RigAttachment {
            fixture_id: fixture.id,
            truss_member_id: truss.id,
            mounting_point_id: format!("{}:x:{}", truss.id, (offset / 100) * 100),
            local_transform: EntityTransform {
                id: fixture.id,
                position_millimetres: [offset, dy, dz],
                rotation_degrees: fixture.rotation_degrees,
            },
        };
        session.change(|document| {
            document
                .put_object(
                    "rig_attachment",
                    &fixture.id.to_string(),
                    &serde_json::to_value(&attachment).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())
        })?;
        created.push(attachment);
    }
    let attached = created
        .iter()
        .map(|attachment| attachment.fixture_id)
        .collect::<BTreeSet<_>>();
    for fixture in moved {
        if !attached.contains(&fixture.id) {
            session.change(|document| {
                document
                    .delete_object("rig_attachment", &fixture.id.to_string())
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            })?;
        }
    }
    Ok(created)
}

fn clear_attachments(session: &Session, moved: &[EntityTransform]) -> Result<(), String> {
    for fixture in moved {
        session.change(|document| {
            document
                .delete_object("rig_attachment", &fixture.id.to_string())
                .map(|_| ())
                .map_err(|error| error.to_string())
        })?;
    }
    Ok(())
}

fn restore_attachments(
    session: &Session,
    moved: &[EntityTransform],
    stored: &[RigAttachment],
) -> Result<(), String> {
    clear_attachments(session, moved)?;
    for attachment in stored {
        session.change(|document| {
            document
                .put_object(
                    "rig_attachment",
                    &attachment.fixture_id.to_string(),
                    &serde_json::to_value(attachment).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())
        })?;
    }
    Ok(())
}

/// Snap only to declared member identities and keep the relationship explicit. The 500 mm
/// attraction radius is intentionally conservative; advanced spacing/distribution belongs to a
/// later Rig Planner slice.
fn snap_transforms(session: &Session, moved: &mut [EntityTransform]) -> Result<(), String> {
    let scene =
        session.with(|document| document.patch_snapshot().map_err(|error| error.to_string()))?;
    let entities = entities(&scene, &BTreeSet::new(), &fixture_notes(session)?);
    let trusses = entities
        .iter()
        .filter(|entity| {
            entity.kind == "venue"
                && (entity.name.to_lowercase().contains("truss")
                    || entity.name.to_lowercase().contains("pipe"))
        })
        .collect::<Vec<_>>();
    for fixture in moved {
        if trusses
            .iter()
            .any(|truss| truss.logical_fixture_id == fixture.id)
        {
            continue;
        }
        let Some(truss) = trusses.iter().min_by_key(|truss| {
            let dy = fixture.position_millimetres[1] - truss.position_millimetres[1];
            let dz = fixture.position_millimetres[2] - truss.position_millimetres[2];
            i64::from(dy).pow(2) + i64::from(dz).pow(2)
        }) else {
            continue;
        };
        let dy = fixture.position_millimetres[1] - truss.position_millimetres[1];
        let dz = fixture.position_millimetres[2] - truss.position_millimetres[2];
        let offset = fixture.position_millimetres[0] - truss.position_millimetres[0];
        if i64::from(dy).pow(2) + i64::from(dz).pow(2) <= 500_i64.pow(2)
            && offset.abs() as f32 <= truss.size_millimetres[0] / 2.0 + 250.0
        {
            fixture.position_millimetres[1] = truss.position_millimetres[1];
            fixture.position_millimetres[2] = truss.position_millimetres[2];
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        EntityTransform, apply_transforms, drawing, entities, fixture_notes, locked_layers,
        moved_transforms, output_direction, selectable_ids,
    };
    use crate::session::Session;
    use light_application::{PatchFixtureCandidate, PatchFixturesCommand};
    use light_core::{FixtureId, Revision};
    use light_fixture::{
        FixtureLocation, FixtureProfile, FixtureVector, MultiPatchInstance, PatchedFixturePatch,
        PatchedFixtureProfileReference, SplitPatch,
    };
    use light_show::{FixtureProfileRevision, ShowStore};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use uuid::Uuid;
    use viz_document::PlanningDocument;

    #[test]
    fn light_direction_is_a_normalised_plan_vector() {
        let direction = output_direction(&FixtureVector {
            x: 0.0,
            y: 0.0,
            z: 90.0,
        });
        assert!((direction[0] - 1.0).abs() < 0.0001);
        assert!(direction[1].abs() < 0.0001);
    }

    #[test]
    fn cad_drawing_generates_all_model_views_when_a_package_has_no_cached_projection() {
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/fixture-library/venue--stage-railing-2-m.toskfixture");
        let mut profile = light_fixture::read_fixture_package(&std::fs::read(package).unwrap())
            .expect("fixture package reads");
        profile.projection_assets = None;

        let mode_id = profile.modes[0].id;
        let drawing = drawing(
            "robe-dls:1",
            &serde_json::to_value(profile).unwrap(),
            Some(mode_id),
        )
        .expect("embedded model produces a CAD drawing");

        assert_eq!(drawing.projections.len(), 5);
        assert_eq!(drawing.live_meshes.len(), 2);
        assert!(
            drawing
                .live_meshes
                .iter()
                .all(|mesh| mesh.triangles.len() > 10),
            "both deterministic poses retain live depth geometry"
        );
        assert!(
            drawing
                .projections
                .iter()
                .all(|projection| projection.svg.matches("<path").count() > 10),
            "each direction contains representative opaque model surfaces"
        );
    }
    fn transform_session() -> (Session, PathBuf, [Uuid; 2]) {
        let path = std::env::temp_dir().join(format!("cad-transform-{}.show", Uuid::new_v4()));
        let document = PlanningDocument::create(&path, "CAD transform test").unwrap();
        let mut profile = FixtureProfile::blank();
        profile.revision = 1;
        profile.manufacturer = "Acme".into();
        profile.name = "CAD light".into();
        let profile_id = profile.id;
        let mode_id = profile.modes[0].id;
        ShowStore::open(&path)
            .unwrap()
            .insert_fixture_profile_revision(
                &FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap())
                    .unwrap(),
            )
            .unwrap();
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        let fixtures = ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let multipatch = if index == 0 {
                    (1..=3)
                        .map(|copy| MultiPatchInstance {
                            id: Uuid::new_v4(),
                            name: format!("CAD light 1 segment {copy}"),
                            universe: None,
                            address: None,
                            split_patches: vec![SplitPatch {
                                split: 1,
                                universe: None,
                                address: None,
                            }],
                            location: FixtureLocation {
                                x: copy * 1_000,
                                y: 2_000,
                                z: 3_000,
                            },
                            ..Default::default()
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                PatchFixtureCandidate {
                    profile: PatchedFixtureProfileReference {
                        profile_id,
                        profile_revision: Revision::from(1_u64),
                        mode_id,
                    },
                    patch: PatchedFixturePatch {
                        fixture_id: FixtureId(*id),
                        fixture_number: Some(index as u32 + 1),
                        virtual_fixture_number: None,
                        name: format!("CAD light {}", index + 1),
                        universe: Some(1),
                        address: Some(index as u16 + 1),
                        split_patches: vec![SplitPatch {
                            split: 1,
                            universe: Some(1),
                            address: Some(index as u16 + 1),
                        }],
                        layer_id: "default".into(),
                        note: None,
                        position_master: None,
                        direct_control: None,
                        internal_bindings: Default::default(),
                        location: FixtureLocation {
                            x: index as i32 * 1_000,
                            y: 2_000,
                            z: 3_000,
                        },
                        rotation: FixtureVector::default(),
                        logical_heads: Vec::new(),
                        multipatch,
                        group_masters_enabled: true,
                        grand_master_enabled: true,
                        invert_pan: false,
                        invert_tilt: false,
                        bracket_angle: 0.0,
                        shaper_angle: None,
                        installed_appearance: Default::default(),
                        move_in_black_enabled: true,
                        move_in_black_delay_millis: 0,
                        highlight_overrides: BTreeMap::new(),
                        freeze: Default::default(),
                    },
                }
            })
            .collect();
        document
            .patch_fixtures(PatchFixturesCommand {
                show_id: document.show_id(),
                fixtures,
                remove_fixture_ids: Vec::new(),
                placements: Vec::new(),
                vector_spreads: Vec::new(),
                fixture_updates: Vec::new(),
            })
            .unwrap();
        drop(document);
        let session = Session::default();
        session.open(&path).unwrap();
        (session, path, ids)
    }

    #[test]
    fn one_revision_checked_command_moves_a_group_and_rejects_a_stale_repeat() {
        let (session, path, ids) = transform_session();
        let before = session
            .with(|document| document.patch_revision().map_err(|error| error.to_string()))
            .unwrap();
        let moved = [
            EntityTransform {
                id: ids[0],
                position_millimetres: [500, 2_250, 3_000],
                rotation_degrees: [0.0; 3],
            },
            EntityTransform {
                id: ids[1],
                position_millimetres: [1_500, 2_250, 3_000],
                rotation_degrees: [0.0; 3],
            },
        ];
        let after = apply_transforms(&session, before, &moved).unwrap();
        assert_eq!(after, before + 1, "one group drag is one Patch revision");
        let snapshot = session
            .with(|document| document.patch_snapshot().map_err(|error| error.to_string()))
            .unwrap();
        let cad_entities = entities(
            &snapshot,
            &BTreeSet::new(),
            &std::collections::HashMap::new(),
        );
        let first_instances = cad_entities
            .iter()
            .filter(|entity| entity.logical_fixture_id == ids[0])
            .collect::<Vec<_>>();
        assert_eq!(
            first_instances.len(),
            4,
            "root and all three copies are drawn"
        );
        assert_eq!(
            first_instances
                .iter()
                .map(|entity| entity.id)
                .collect::<BTreeSet<_>>()
                .len(),
            4,
            "every physical placement remains independently hittable"
        );
        let x = |id| {
            snapshot
                .fixtures
                .iter()
                .find(|fixture| fixture.patch.fixture_id.0 == id)
                .unwrap()
                .patch
                .location
                .x
        };
        assert_eq!(
            x(ids[1]) - x(ids[0]),
            1_000,
            "relative spacing survives the group move"
        );
        let moved_fixture = snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.patch.fixture_id.0 == ids[0])
            .unwrap();
        assert_eq!(
            moved_fixture
                .patch
                .multipatch
                .iter()
                .map(|copy| copy.location.x)
                .collect::<Vec<_>>(),
            vec![1_500, 2_500, 3_500],
            "one logical move translates every physical copy by the same delta"
        );
        assert!(
            apply_transforms(&session, before, &moved)
                .unwrap_err()
                .contains("rig changed"),
            "the old revision cannot overwrite the committed move"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn spread_transform_interpolates_in_explicit_selection_order() {
        let ids = [
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        let before = ids
            .iter()
            .map(|id| EntityTransform {
                id: *id,
                position_millimetres: [1_000, 2_000, 3_000],
                rotation_degrees: [0.0; 3],
            })
            .collect::<Vec<_>>();
        let ordered = [ids[2], ids[0], ids[3], ids[1]];
        let after = moved_transforms(&before, &ordered, [900, -300, 120], true);
        let position = |id| {
            after
                .iter()
                .find(|transform| transform.id == id)
                .unwrap()
                .position_millimetres
        };

        assert_eq!(position(ids[2]), [1_000, 2_000, 3_000]);
        assert_eq!(position(ids[0]), [1_300, 1_900, 3_040]);
        assert_eq!(position(ids[3]), [1_600, 1_800, 3_080]);
        assert_eq!(position(ids[1]), [1_900, 1_700, 3_120]);

        let pair = moved_transforms(&before[..2], &ids[..2], [-400, 200, 0], true);
        assert_eq!(pair[0].position_millimetres, [1_000, 2_000, 3_000]);
        assert_eq!(pair[1].position_millimetres, [600, 2_200, 3_000]);
        let ordinary = moved_transforms(&before[..1], &ids[..1], [50, 60, 70], false);
        assert_eq!(ordinary[0].position_millimetres, [1_050, 2_060, 3_070]);
    }

    #[test]
    fn locked_layers_are_not_selectable_by_native_cad_commands() {
        let (session, path, ids) = transform_session();
        session
            .change(|document| {
                document
                    .put_object(
                        "patch_layer",
                        "default",
                        &serde_json::json!({
                            "id": "default",
                            "name": "Stage",
                            "order": 0,
                            "locked": true,
                        }),
                    )
                    .map_err(|error| error.to_string())?;
                Ok(())
            })
            .unwrap();

        let selectable = selectable_ids(&session).unwrap();
        assert!(ids.iter().all(|id| !selectable.contains(id)));
        let locks = locked_layers(&session).unwrap();
        assert_eq!(locks, BTreeSet::from(["default".to_owned()]));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn fixture_notes_default_blank_and_project_into_every_physical_instance() {
        let (session, path, ids) = transform_session();
        assert!(fixture_notes(&session).unwrap().is_empty());
        session
            .change(|document| {
                document
                    .put_object(
                        "fixture_note",
                        &ids[0].to_string(),
                        &serde_json::json!({
                            "fixtureId": ids[0],
                            "note": "Use secondary safety",
                        }),
                    )
                    .map_err(|error| error.to_string())?;
                Ok(())
            })
            .unwrap();
        let snapshot = session
            .with(|document| document.patch_snapshot().map_err(|error| error.to_string()))
            .unwrap();
        let notes = fixture_notes(&session).unwrap();
        let projected = entities(&snapshot, &BTreeSet::new(), &notes);
        assert!(
            projected
                .iter()
                .filter(|entity| entity.logical_fixture_id == ids[0])
                .all(|entity| entity.note == "Use secondary safety")
        );
        assert!(
            projected
                .iter()
                .filter(|entity| entity.logical_fixture_id == ids[1])
                .all(|entity| entity.note.is_empty())
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn fixture_and_layer_visibility_remove_entities_from_2d_selection() {
        let (session, path, ids) = transform_session();
        session
            .change(|document| {
                document
                    .put_object(
                        "fixture_visibility",
                        &ids[0].to_string(),
                        &serde_json::json!({
                            "fixtureId": ids[0],
                            "visible2d": false,
                            "visible3d": true,
                        }),
                    )
                    .map_err(|error| error.to_string())?;
                Ok(())
            })
            .unwrap();
        let selectable = selectable_ids(&session).unwrap();
        assert!(!selectable.contains(&ids[0]));
        assert!(selectable.contains(&ids[1]));

        session
            .change(|document| {
                document
                    .put_object(
                        "patch_layer",
                        "default",
                        &serde_json::json!({
                            "id": "default",
                            "name": "Stage",
                            "order": 0,
                            "visible2d": false,
                        }),
                    )
                    .map_err(|error| error.to_string())?;
                Ok(())
            })
            .unwrap();
        assert!(selectable_ids(&session).unwrap().is_empty());
        let _ = std::fs::remove_file(path);
    }
}
