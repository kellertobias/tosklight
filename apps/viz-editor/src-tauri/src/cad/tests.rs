use super::profile_drawing::drawing;
use super::{
    EntityTransform, apply_transforms, entities, fixture_notes, locked_layers, moved_transforms,
    output_direction, selectable_ids,
};
use crate::session::Session;
use light_application::{PatchFixtureCandidate, PatchFixturesCommand};
use light_core::{FixtureId, Revision};
use light_fixture::{
    FixtureLocation, FixtureProfile, FixtureVector, MultiPatchInstance, PatchedFixturePatch,
    PatchedFixtureProfileReference, SplitPatch,
};
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
pub(super) fn transform_session() -> (Session, PathBuf, [Uuid; 2]) {
    let path = std::env::temp_dir().join(format!("cad-transform-{}.show", Uuid::new_v4()));
    let document = PlanningDocument::create(&path, "CAD transform test").unwrap();
    let mut profile = FixtureProfile::blank();
    profile.revision = 1;
    profile.manufacturer = "Acme".into();
    profile.name = "CAD light".into();
    let profile_id = profile.id;
    let mode_id = profile.modes[0].id;
    document
        .retain_fixture_profile(serde_json::to_value(profile).unwrap())
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
                    model_scale: None,
                    scenery_options: Default::default(),
                    scenery_size_metres: None,
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
                    position_calibration: None,
                    color_calibration: None,
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
