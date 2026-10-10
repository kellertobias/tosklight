//! Canonical scene and command boundary for the web-owned CAD planning surface.
//!
//! The canvases are deliberately only views. Selection, fixture transforms, revisions and mount
//! relationships live here beside the open planning document, so closing a window cannot lose
//! show data and a stale drag cannot overwrite a newer Patch edit. Every editor window shows the
//! same scene, which is why the deltas below are broadcast rather than sent to one window.

mod aim;
pub mod history;
mod layer_visibility;
pub mod numeric_placement;
mod profile_drawing;
mod profile_lookup;
mod scenery;
mod transform;

use crate::contract::{FixtureDto, MutationDto};
use crate::session::Session;
use aim::{CadAim, aim_lamps};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use history::{History, TransformRecord};
use layer_visibility::{
    fixture_notes, hidden_fixture_ids, hidden_layers, locked_layers, selectable_ids,
};
use light_application::PatchSnapshot;
use parking_lot::Mutex;
use profile_drawing::{CadDrawing, drawing_id, drawings, new_drawings};
use profile_lookup::position_points;
use scenery::{
    CadMounting, CadScenery, cad_mounting, cad_scenery, connect_chains, entity_size, profile_label,
};
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
    #[serde(flatten)]
    pub aim: CadAim,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenery: Option<CadScenery>,
    /// The clip this fixture hangs by, at the size it is drawn: what a pipe has to reach to rig it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mounting: Option<CadMounting>,
    /// A 3D model the operator imported into this show, rather than a shipped Venue object.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub imported_model: bool,
    /// The 3D Point this placement follows, as "ID · name". The plan draws the placement where it
    /// was rigged — a point's live pose is desk state it does not read — so this is a note beside
    /// the drawing. Absent against the stage, or when the point has left the show.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_reference: Option<String>,
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
    /// Whether snapping was on for this move: a moved lamp near a truss is then recorded as mounted
    /// on it. The position itself arrives already snapped from the CAD view.
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
    // One operator gesture: every write below becomes one synchronized transaction.
    session.gesture(|| transform::transform(&app, &session, &cad, intent))
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
    let all_entities = aim_lamps(connect_chains(entities(&patch, &locked, &notes)), &patch);
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
            // A fixture this delta adds needs its model drawing with it; without it the plan
            // draws a plain box until a later snapshot arrives. `new_drawings` sends only what
            // the cache did not already hold.
            drawings: new_drawings(&patch, cad),
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
    aim_lamps(
        connect_chains(entities(snapshot, locked_layers, notes)),
        snapshot,
    )
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
    let points = position_points(snapshot);
    snapshot
        .fixtures
        .iter()
        .flat_map(|fixture| {
            let profile = profiles.get(&(
                fixture.profile.profile_id.0,
                fixture.profile.profile_revision,
            ));
            let snapshot = profile.map(|profile| &profile.profile_snapshot);
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
            let fixture_profile = profile.map_or_else(
                || "Unknown fixture".to_owned(),
                |profile| profile_label(&profile.profile_snapshot),
            );
            let mode_id = fixture.profile.mode_id.to_string();
            let mode = profile_lookup::mode_name(profile, &mode_id);
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
                size_millimetres: entity_size(snapshot, &fixture.patch, id),
                output_direction: output_direction(rotation),
                aim: CadAim::default(),
                scenery: cad_scenery(snapshot, &fixture.patch.scenery_options),
                mounting: cad_mounting(snapshot, entity_size(snapshot, &fixture.patch, id)),
                imported_model: snapshot.is_some_and(profile_lookup::imported_model),
                position_reference: profile_lookup::position_reference(&points, &fixture.patch),
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

#[cfg(test)]
mod tests;
