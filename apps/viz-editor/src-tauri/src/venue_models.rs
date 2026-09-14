//! Venue models the operator brings in themselves: a hall, a stage, a set, as a GLB, glTF, 3MF
//! or OBJ file, kept in the show as one GLB (see `crate::model_import`).
//!
//! A venue model is not a fixture-library entry. Importing one wraps the file in a visual-only
//! profile that lives only in the open show — every Venue object already is a visual-only
//! fixture, so the patch sheet, layers, the CAD views, the Visualizer and the desk's Stage all
//! place and draw it with nothing new to learn — and places one object from it at the stage origin.

use crate::contract::{FixtureDto, MutationDto};
use crate::session::{Session, apply_patch_mutation};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use light_fixture::{FixtureProfile, MAX_FIXTURE_MODEL_BYTES};
use serde::Serialize;
use std::path::Path;
use uuid::Uuid;

/// The manufacturer imported models are listed under, apart from the shipped Venue objects.
pub const IMPORTED_MANUFACTURER: &str = "Imported models";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VenueModelImport {
    pub fixture_id: String,
    pub name: String,
    pub triangles: usize,
}

/// A visual-only profile wrapping one GLB, and how many triangles it draws.
#[derive(Debug)]
pub struct VenueModelProfile {
    pub profile: FixtureProfile,
    pub triangles: usize,
}

/// Import a 3D model from the operator's disk as a venue object of the open show.
///
/// A glTF, 3MF or OBJ is converted to a self-contained GLB first, reading the files it names from
/// beside it; a GLB is taken as it is.
#[tauri::command]
pub fn import_venue_model(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, crate::cad::CadState>,
    path: String,
    layer_id: Option<String>,
) -> Result<VenueModelImport, String> {
    let path = Path::new(&path);
    let size = std::fs::metadata(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?
        .len();
    if size > MAX_FIXTURE_MODEL_BYTES as u64 {
        return Err(format!(
            "{} is {} MB; a 3D model may be at most {} MB.",
            file_name(path),
            size.div_ceil(1024 * 1024),
            MAX_FIXTURE_MODEL_BYTES / (1024 * 1024)
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Venue model")
        .to_owned();
    let source = file_name(path);
    let bytes =
        crate::model_import::to_glb(&source, &bytes, |relative| read_sibling(path, relative))
            .map_err(|error| error.to_string())?;
    let model = venue_model_profile(&name, &source, &bytes)?;
    let profile_id = model.profile.id.0;
    let mode_id = model.profile.modes[0].id;

    let virtual_number = session.with(|document| {
        let snapshot = document
            .patch_snapshot()
            .map_err(|error| error.to_string())?;
        Ok(first_free_virtual_number(
            snapshot
                .fixtures
                .iter()
                .filter_map(|fixture| fixture.patch.virtual_fixture_number),
        ))
    })?;
    let profile = serde_json::to_value(&model.profile).map_err(|error| error.to_string())?;
    // The patch resolves a profile from the show before the library, so once the show holds it the
    // placement below needs no library entry at all.
    session.change(|document| {
        document
            .retain_fixture_profile(profile)
            .map_err(|error| error.to_string())
    })?;

    let fixture_id = Uuid::new_v4();
    let fixture: FixtureDto = serde_json::from_value(serde_json::json!({
        "fixtureId": fixture_id,
        "fixtureNumber": null,
        "virtualFixtureNumber": virtual_number,
        "name": name,
        "profileId": profile_id,
        "profileRevision": 1,
        "modeId": mode_id,
        "splitPatches": [{ "split": 1, "universe": null, "address": null }],
        "layerId": layer_id.filter(|layer| !layer.trim().is_empty()).unwrap_or_else(|| "default".into()),
        "location": { "x": 0, "y": 0, "z": 0 },
        "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 },
    }))
    .map_err(|error| error.to_string())?;
    apply_patch_mutation(
        &app,
        &session,
        &cad,
        None,
        MutationDto {
            request_id: Uuid::new_v4().to_string(),
            fixtures: vec![fixture],
            remove_fixture_ids: Vec::new(),
            placements: Vec::new(),
        },
    )?;
    Ok(VenueModelImport {
        fixture_id: fixture_id.to_string(),
        name,
        triangles: model.triangles,
    })
}

/// Wrap a GLB in a visual-only profile sized to the model, so it is drawn at the size it was built.
///
/// The Visualizer fits a model to its profile's physical size; declaring the model's own bounds
/// keeps that fit at exactly 1:1, so a 20 m hall stays 20 m.
pub fn venue_model_profile(
    name: &str,
    source: &str,
    bytes: &[u8],
) -> Result<VenueModelProfile, String> {
    light_fixture::validate_glb_model(bytes).map_err(|error| format!("{source}: {error}"))?;
    let model = viz_scene::read_glb_with_limit(bytes, viz_scene::VENUE_MODEL_MAX_TRIANGLES)
        .map_err(|error| format!("{source}: {}", error.0))?;
    let size = model.extent * 2.0 * 1000.0;
    let profile: FixtureProfile = serde_json::from_value(serde_json::json!({
        "schema_version": 3,
        "id": Uuid::new_v4(),
        "revision": 1,
        "manufacturer": IMPORTED_MANUFACTURER,
        "name": name,
        "short_name": name,
        "fixture_type": "venue",
        "patch_policy": "visual_only",
        "notes": format!("Imported into this show from {source}. It is not part of the fixture library."),
        "model_asset": format!("data:model/gltf-binary;base64,{}", STANDARD.encode(bytes)),
        "model_units": "metres",
        "physical": {
            "width_millimetres": size.x,
            "height_millimetres": size.y,
            "depth_millimetres": size.z,
            "weight_kilograms": null,
            "power_watts": null,
            "connectors": "",
            "light_source": "",
            "color_temperature_kelvin": null,
            "color_rendering_index": null,
            "luminous_output_lumens": null,
            "lens": "",
            "beam_angle_degrees": null
        },
        "optics": {},
        "modes": [{
            "id": Uuid::new_v4(),
            "name": "Model",
            "notes": "",
            "splits": [{ "number": 1, "footprint": 0 }],
            "heads": [{ "id": Uuid::new_v4(), "name": "Visual", "master_shared": true, "split": 1 }],
            "channels": [],
            "color_systems": [],
            "control_actions": [],
            "geometry": { "nodes": [], "emitters": [] }
        }],
        "hazardous": false,
        "direct_control_protocols": [],
        "signal_loss_policy": { "type": "hold_last" },
        "reserved_source": null
    }))
    .map_err(|error| format!("{source}: {error}"))?;
    profile
        .validate()
        .map_err(|error| format!("{source}: {error}"))?;
    Ok(VenueModelProfile {
        triangles: model.triangle_count(),
        profile,
    })
}

/// The lowest `0.x` number no Venue object uses yet.
fn first_free_virtual_number(used: impl Iterator<Item = u32>) -> u32 {
    let used: std::collections::BTreeSet<u32> = used.collect();
    (1..).find(|number| !used.contains(number)).unwrap_or(1)
}

/// A file a model names — a glTF buffer or texture, an OBJ material library — beside the model.
fn read_sibling(model: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let sibling = model
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(relative);
    let size = std::fs::metadata(&sibling)
        .map_err(|error| error.to_string())?
        .len();
    if size > MAX_FIXTURE_MODEL_BYTES as u64 {
        return Err(format!(
            "it is {} MB; a 3D model may be at most {} MB",
            size.div_ceil(1024 * 1024),
            MAX_FIXTURE_MODEL_BYTES / (1024 * 1024)
        ));
    }
    std::fs::read(&sibling).map_err(|error| error.to_string())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("The model")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GLB holding one triangle spanning 20 m across, 4 m up and 10 m deep.
    fn triangle_glb() -> Vec<u8> {
        let positions: [f32; 9] = [0.0, 0.0, 0.0, 20.0, 4.0, 0.0, 0.0, 0.0, 10.0];
        let mut binary: Vec<u8> = positions
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        binary.extend([0_u16, 1, 2].iter().flat_map(|index| index.to_le_bytes()));
        while !binary.len().is_multiple_of(4) {
            binary.push(0);
        }
        let mut json = serde_json::json!({
            "asset": { "version": "2.0" },
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 0 }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "indices": 1 }] }],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                  "min": [0.0, 0.0, 0.0], "max": [20.0, 4.0, 10.0] },
                { "bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR" }
            ],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
                { "buffer": 0, "byteOffset": 36, "byteLength": 6 }
            ],
            "buffers": [{ "byteLength": binary.len() }]
        })
        .to_string()
        .into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let total = 12 + 8 + json.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(&binary);
        glb
    }

    #[test]
    fn a_glb_becomes_a_visual_only_profile_the_size_of_the_model() {
        let imported = venue_model_profile("Hall", "hall.glb", &triangle_glb()).expect("imported");
        let profile = &imported.profile;
        assert_eq!(imported.triangles, 1);
        assert_eq!(profile.manufacturer, IMPORTED_MANUFACTURER);
        assert_eq!(profile.name, "Hall");
        assert_eq!(profile.patch_policy, light_fixture::PatchPolicy::VisualOnly);
        assert!(
            profile.scenery.is_none(),
            "a model is drawn from its GLB, not generated"
        );
        assert!(
            profile
                .model_asset
                .as_deref()
                .is_some_and(|asset| asset.starts_with("data:model/gltf-binary;base64,"))
        );
        let physical = serde_json::to_value(&profile.physical).expect("physical");
        assert_eq!(physical["width_millimetres"], 20_000.0);
        assert_eq!(physical["height_millimetres"], 4_000.0);
        assert_eq!(physical["depth_millimetres"], 10_000.0);
        assert_eq!(profile.modes.len(), 1);
        assert!(profile.modes[0].channels.is_empty());
    }

    #[test]
    fn a_file_that_is_not_a_glb_is_refused_by_name() {
        let error = venue_model_profile("Notes", "notes.glb", b"not a model").expect_err("refused");
        assert!(error.starts_with("notes.glb: "), "{error}");
        assert!(error.contains("GLB"), "{error}");
    }

    #[test]
    fn the_venue_limit_is_far_above_a_lamp_body() {
        let error = viz_scene::read_glb_with_limit(&triangle_glb(), 0).expect_err("over the limit");
        assert!(
            error.0.contains("more than the 0 this model may use"),
            "{}",
            error.0
        );
    }

    #[test]
    fn the_next_venue_object_takes_the_lowest_free_number() {
        assert_eq!(first_free_virtual_number([].into_iter()), 1);
        assert_eq!(first_free_virtual_number([1, 2, 4].into_iter()), 3);
    }
}
