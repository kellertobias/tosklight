//! Assemble one coherent renderer scene from the desk's read models.

use crate::transform::{self, PlacementSource};
use crate::wire::{ObjectRecord, PatchSnapshot, StageLayoutBody};
use glam::Vec3;
use light_fixture::FixtureProfile;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use viz_project::{PatchedFixture, PhysicalInstance, ScenePlan};
use viz_scene::{SceneryKind, SceneryObject};

/// Everything the scene builder reads, gathered by the connection before anything is displayed.
pub struct DeskReadModels {
    pub patch: PatchSnapshot,
    pub stage_layout: StageLayoutBody,
    pub venue_objects: Vec<ObjectRecord>,
    pub show_name: String,
    pub server_identity: String,
}

/// Build the scene and its bindings, plus any warnings the operator must see.
pub fn build(models: &DeskReadModels) -> ScenePlan {
    let profiles = decode_profiles(&models.patch);
    let mut fixtures = Vec::with_capacity(models.patch.fixtures.len());
    let mut warnings = Vec::new();
    // Where every instance's transform actually came from. A rig standing in the wrong place is
    // first of all a question of which of the four sources placed it, so the diagnostics say.
    let mut placements = Vec::with_capacity(models.patch.fixtures.len());
    let mut grid_index = 0_usize;

    for fixture in &models.patch.fixtures {
        let Some(profile) = profiles.get(&(fixture.profile_id, fixture.profile_revision)) else {
            warnings.push(format!(
                "{}: profile revision {} has no snapshot; the fixture is shown as a generic body",
                fixture.name, fixture.profile_revision
            ));
            continue;
        };
        let mut instances = Vec::with_capacity(1 + fixture.multipatch.len());
        let root_placement = transform::resolve(
            models
                .stage_layout
                .positions3d
                .get(&fixture.fixture_id.to_string()),
            fixture.location,
            fixture.rotation,
            models
                .stage_layout
                .positions
                .get(&fixture.fixture_id.to_string()),
            grid_index,
        );
        grid_index += 1;
        placements.push(root_placement.source);
        instances.push(PhysicalInstance {
            instance_id: fixture.fixture_id,
            name: fixture.name.clone(),
            split_patches: split_patches(&fixture.split_patches),
            position: root_placement.position,
            rotation_degrees: root_placement.rotation_degrees,
            invert_pan: fixture.invert_pan,
            invert_tilt: fixture.invert_tilt,
            bracket_angle: fixture.bracket_angle,
            shaper_angle: fixture.shaper_angle,
        });
        for multipatch in &fixture.multipatch {
            // A multi-patch instance shares the logical fixture's values but keeps its own
            // transform, so it never inherits the root's placement.
            let placement = transform::resolve(
                models
                    .stage_layout
                    .positions3d
                    .get(&multipatch.id.to_string()),
                multipatch.location,
                multipatch.rotation,
                None,
                grid_index,
            );
            grid_index += 1;
            placements.push(placement.source);
            instances.push(PhysicalInstance {
                instance_id: multipatch.id,
                name: if multipatch.name.is_empty() {
                    format!("{} \u{2022} multi-patch", fixture.name)
                } else {
                    multipatch.name.clone()
                },
                split_patches: split_patches(&multipatch.split_patches),
                position: placement.position,
                rotation_degrees: placement.rotation_degrees,
                invert_pan: multipatch.invert_pan,
                invert_tilt: multipatch.invert_tilt,
                bracket_angle: multipatch.bracket_angle,
                shaper_angle: multipatch.shaper_angle,
            });
        }
        fixtures.push(PatchedFixture {
            fixture_id: fixture.fixture_id,
            name: fixture.name.clone(),
            number: fixture.fixture_number,
            profile: profile.clone(),
            mode_id: fixture.mode_id,
            instances,
        });
    }

    let mut plan = viz_project::compile(&fixtures);
    plan.warnings.extend(warnings);
    if !placements.is_empty() {
        plan.warnings.push(placement_summary(&placements));
    }
    plan.scene.show_id = Some(models.patch.show_id);
    plan.scene.show_name = models.show_name.clone();
    plan.scene.source_identity = models.server_identity.clone();
    plan.scene.revision = models.patch.patch_revision;
    plan.scene.scenery = build_scenery(&plan.scene, &models.venue_objects);
    plan.scene.recompute_bounds();
    plan
}

fn split_patches(splits: &[crate::wire::SplitAssignment]) -> Vec<(u16, Option<(u16, u16)>)> {
    splits
        .iter()
        .map(|split| {
            (
                split.split,
                match (split.universe, split.address) {
                    (Some(universe), Some(address)) => Some((universe, address)),
                    _ => None,
                },
            )
        })
        .collect()
}

/// Decode the immutable profile snapshots the patch carries.
fn decode_profiles(patch: &PatchSnapshot) -> HashMap<(Uuid, u64), Arc<FixtureProfile>> {
    patch
        .profile_revisions
        .iter()
        .filter_map(|revision| {
            if revision.profile_snapshot.is_null() {
                return None;
            }
            let profile: FixtureProfile =
                serde_json::from_value(revision.profile_snapshot.clone()).ok()?;
            Some((
                (revision.profile_id, revision.profile_revision),
                Arc::new(profile),
            ))
        })
        .collect()
}

/// Visual-only scenery: a stage floor sized to the rig, plus any `Venue` objects the show stores.
///
/// The floor exists because a beam has to land on something for its footprint to be visible.
fn build_scenery(scene: &viz_scene::Scene, venue: &[ObjectRecord]) -> Vec<SceneryObject> {
    let mut scenery = Vec::with_capacity(venue.len() + 2);
    let bounds = scene.bounds;
    // The deck extends well past the rig footprint so a wide wash lands on it instead of
    // spilling off the edge into empty space, where its beam would keep going with nothing to
    // stop it.
    const APRON_METRES: f32 = 16.0;
    let width = if bounds.is_empty() {
        24.0
    } else {
        (bounds.extent().x + APRON_METRES).clamp(12.0, 120.0)
    };
    let depth = if bounds.is_empty() {
        20.0
    } else {
        (bounds.extent().z + APRON_METRES).clamp(12.0, 120.0)
    };
    let centre = if bounds.is_empty() {
        Vec3::ZERO
    } else {
        bounds.centre()
    };
    scenery.push(SceneryObject {
        id: Uuid::nil(),
        name: "Stage floor".into(),
        position: Vec3::new(centre.x, -0.05, centre.z),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(width, 0.1, depth),
        colour: [0.075, 0.075, 0.082],
        roughness: 0.85,
        kind: SceneryKind::Floor,
        chords: 0,
    });
    // No backdrop is invented. A show that wants one places a `Venue` object; anything else
    // would put a surface in the picture that the operator never rigged.
    for object in venue {
        let Some(position) = venue_vector(&object.body, "position") else {
            continue;
        };
        let size = venue_size(&object.body).unwrap_or(Vec3::splat(1.0));
        let kind = venue_kind(&object.body);
        let chords = venue_chords(&object.body, kind);
        scenery.push(SceneryObject {
            id: Uuid::nil(),
            name: object
                .body
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&object.id)
                .to_owned(),
            position,
            rotation_degrees: Vec3::ZERO,
            size,
            colour: match kind {
                SceneryKind::Truss => [0.2, 0.205, 0.215],
                SceneryKind::Wall => [0.1, 0.1, 0.11],
                _ => [0.14, 0.14, 0.15],
            },
            // Wool serge is the matt-est thing in a venue and aluminium the least: a drape
            // that catches the same highlight as the truss over it reads as painted board.
            roughness: match kind {
                SceneryKind::Curtain => 0.96,
                SceneryKind::Truss | SceneryKind::Railing => 0.45,
                _ => 0.8,
            },
            kind,
            chords,
        });
    }
    scenery
}

fn venue_vector(body: &serde_json::Value, key: &str) -> Option<Vec3> {
    let value = body.get(key)?;
    let read = |name: &str| value.get(name).and_then(serde_json::Value::as_f64);
    Some(transform::to_world(
        read("x")? as f32,
        read("y")? as f32,
        read("z")? as f32,
    ))
}

/// A venue object's extent. Sizes take the same axis order as positions but never a sign: an
/// extent is a length, and a negative one would turn the object inside out.
fn venue_size(body: &serde_json::Value) -> Option<Vec3> {
    let value = body.get("size")?;
    let read = |name: &str| value.get(name).and_then(serde_json::Value::as_f64);
    Some(
        Vec3::new(read("x")? as f32, read("z")? as f32, read("y")? as f32)
            .abs()
            .max(Vec3::splat(0.01)),
    )
}

/// What a venue object is, so a truss is drawn as structure rather than as an anonymous prop.
fn venue_kind(body: &serde_json::Value) -> SceneryKind {
    let named = ["kind", "venue_kind", "category", "shape", "type"]
        .into_iter()
        .find_map(|key| body.get(key).and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = body
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let says = |needle: &str| named.contains(needle) || name.contains(needle);
    if says("truss") || says("bar") || says("boom") {
        SceneryKind::Truss
    } else if says("wall") || says("cyc") || says("backdrop") || says("screen") {
        SceneryKind::Wall
    } else if says("riser") || says("deck") || says("rostrum") || says("stage") {
        SceneryKind::Riser
    } else {
        SceneryKind::Prop
    }
}

/// How many chords a truss has. Rigs say this in the name — "3 point", "four-point", "pipe" —
/// and a truss drawn with the wrong number of chords is the wrong truss.
fn venue_chords(body: &serde_json::Value, kind: SceneryKind) -> u8 {
    if kind != SceneryKind::Truss {
        return 0;
    }
    let text = ["kind", "venue_kind", "type", "name"]
        .into_iter()
        .filter_map(|key| body.get(key).and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let explicit = body
        .get("chords")
        .and_then(serde_json::Value::as_u64)
        .map(|chords| chords as u8);
    if let Some(chords) = explicit.filter(|chords| (1..=4).contains(chords)) {
        return chords;
    }
    let says = |needles: [&str; 3]| needles.iter().any(|needle| text.contains(needle));
    if says(["pipe", "1 point", "one point"]) || text.contains("bar") {
        1
    } else if says(["2 point", "two point", "ladder"]) || text.contains("2-point") {
        2
    } else if says(["3 point", "three point", "tri"]) || text.contains("3-point") {
        3
    } else {
        // A truss with nothing to say for itself is the one every rig has most of.
        4
    }
}

/// Placement provenance for the diagnostics surface.
pub fn placement_summary(sources: &[PlacementSource]) -> String {
    let count =
        |wanted: PlacementSource| sources.iter().filter(|source| **source == wanted).count();
    format!(
        "placement: {} from stage layout, {} from patch, {} migrated, {} default grid",
        count(PlacementSource::StageLayout3d),
        count(PlacementSource::PatchLocation),
        count(PlacementSource::MigratedLayout2d),
        count(PlacementSource::DefaultGrid),
    )
}
