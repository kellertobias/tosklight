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
                    stage_position_set: false,
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
            stage_position_set: None,
            copy_stage_position_set: None,
            id: ids[0],
            position_millimetres: [500, 2_250, 3_000],
            rotation_degrees: [0.0; 3],
        },
        EntityTransform {
            stage_position_set: None,
            copy_stage_position_set: None,
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
            stage_position_set: None,
            copy_stage_position_set: None,
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

fn attachment_test_session() -> (Session, PathBuf, [Uuid; 2]) {
    attachment_test_session_with_mount(true)
}

fn attachment_test_session_with_mount(declared: bool) -> (Session, PathBuf, [Uuid; 2]) {
    let (session, path, ids) = transform_session();
    session
        .change(|document| {
            let snapshot = document
                .patch_snapshot()
                .map_err(|error| error.to_string())?;
            let mut candidates = Vec::new();
            for fixture in &snapshot.fixtures {
                let index = usize::from(fixture.patch.fixture_id.0 != ids[0]);
                let mut profile = snapshot.profile_revisions[0].profile_snapshot.clone();
                let profile_id = Uuid::new_v4();
                profile["id"] = serde_json::json!(profile_id);
                if index == 0 {
                    profile["patch_policy"] = serde_json::json!("visual_only");
                    for mode in profile["modes"].as_array_mut().unwrap() {
                        mode["channels"] = serde_json::json!([]);
                        for split in mode["splits"].as_array_mut().unwrap() {
                            split["footprint"] = serde_json::json!(0);
                        }
                    }
                    profile["scenery"] = serde_json::json!({"kind":"truss","chords":1,
                    "default_size_metres":{"x":2.0,"y":0.05,"z":0.05},
                    "minimum_size_metres":{"x":2.0,"y":0.05,"z":0.05},
                    "maximum_size_metres":{"x":2.0,"y":0.05,"z":0.05}});
                } else if declared {
                    profile["mounting"] = serde_json::json!({"hardware":"clamp",
                    "centre_millimetres":{"x":0,"y":0,"z":0},
                    "half_extent_millimetres":{"x":30,"y":30,"z":30},
                    "pipe_millimetres":{"x":0,"y":0,"z":0}});
                }
                if index != 0 && !declared {
                    profile.as_object_mut().unwrap().remove("mounting");
                }
                document
                    .retain_fixture_profile(profile)
                    .map_err(|error| error.to_string())?;
                let mut profile_ref = fixture.profile;
                profile_ref.profile_id = FixtureId(profile_id);
                let mut patch = fixture.patch.clone();
                patch.name = if index == 0 {
                    "Attachment truss"
                } else {
                    "Attachment lamp"
                }
                .into();
                patch.location = FixtureLocation { x: 0, y: 0, z: 0 };
                patch.multipatch.clear();
                if index == 0 {
                    patch.scenery_size_metres = Some(FixtureVector {
                        x: 2000.0,
                        y: 50.0,
                        z: 50.0,
                    });
                    patch.universe = None;
                    patch.address = None;
                    for split in &mut patch.split_patches {
                        split.universe = None;
                        split.address = None;
                    }
                }
                candidates.push(PatchFixtureCandidate {
                    profile: profile_ref,
                    patch,
                });
            }
            document
                .patch_fixtures(PatchFixturesCommand {
                    show_id: document.show_id(),
                    fixtures: candidates,
                    remove_fixture_ids: Vec::new(),
                    placements: Vec::new(),
                    vector_spreads: Vec::new(),
                    fixture_updates: Vec::new(),
                })
                .map_err(|error| error.to_string())?;
            Ok(())
        })
        .unwrap();
    (session, path, ids)
}

fn attachment_gesture(
    session: &Session,
    cad: &super::CadState,
    ids: Vec<Uuid>,
) -> super::TransformOutcome {
    let revision = session
        .with(|document| document.patch_revision().map_err(|error| error.to_string()))
        .unwrap();
    super::transform::commit(
        session,
        cad,
        super::TransformIntent {
            expected_scene_revision: revision,
            entity_ids: ids,
            delta_millimetres: [10, 0, 0],
            spread: false,
            snap_to_mounts: true,
        },
    )
    .unwrap()
}

#[test]
fn snapped_truss_gesture_cannot_persist_self_attachment() {
    let (session, path, ids) = attachment_test_session();
    let result = attachment_gesture(&session, &super::CadState::default(), vec![ids[0]]);
    assert!(
        result.attachments.is_empty(),
        "a connector snap is not a lamp mount"
    );
    assert!(super::attachments(&session).unwrap().is_empty());
    drop(session);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn snapped_lamp_and_selected_truss_cannot_attach_to_moved_peer() {
    let (session, path, ids) = attachment_test_session();
    let result = attachment_gesture(&session, &super::CadState::default(), ids.to_vec());
    assert!(
        result.attachments.is_empty(),
        "all moving logical roots are excluded"
    );
    drop(session);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn snapped_fixture_without_declared_mount_cannot_acquire_attachment() {
    let (session, path, ids) = attachment_test_session_with_mount(false);
    attachment_gesture(&session, &super::CadState::default(), vec![ids[1]]);
    assert!(
        super::attachments(&session).unwrap().is_empty(),
        "legacy drawn clamp is not persisted mounting authority"
    );
    drop(session);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn snapped_declared_lamp_mounts_stationary_pipe_and_cleans_up_outside_reach() {
    let (session, path, ids) = attachment_test_session();
    let cad = super::CadState::default();
    let result = attachment_gesture(&session, &cad, vec![ids[1]]);
    assert_eq!(result.attachments.len(), 1);
    assert_eq!(result.attachments[0].fixture_id, ids[1]);
    assert_eq!(result.attachments[0].truss_member_id, ids[0]);
    let stored = serde_json::to_value(super::attachments(&session).unwrap()).unwrap();
    drop(session);
    let session = Session::default();
    session.open(&path).unwrap();
    assert_eq!(
        serde_json::to_value(super::attachments(&session).unwrap()).unwrap(),
        stored
    );
    let revision = session
        .with(|document| document.patch_revision().map_err(|error| error.to_string()))
        .unwrap();
    super::transform::commit(
        &session,
        &cad,
        super::TransformIntent {
            expected_scene_revision: revision,
            entity_ids: vec![ids[1]],
            delta_millimetres: [0, 100, 0],
            spread: false,
            snap_to_mounts: true,
        },
    )
    .unwrap();
    assert!(
        super::attachments(&session).unwrap().is_empty(),
        "declared30mm clip does not reach100mmawaypipe"
    );
    drop(session);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn snapped_attachment_gesture_history_restores_undo_and_redo_exactly() {
    let (session, path, ids) = attachment_test_session();
    let cad = super::CadState::default();
    let moved = attachment_gesture(&session, &cad, vec![ids[1]]);
    let record = super::history::last_move(&cad.history.lock());
    assert!(record.before_attachments.is_empty());
    assert_eq!(record.after_attachments.len(), 1);
    let undo =
        super::history::restore_move(&session, moved.scene_revision, &record, false).unwrap();
    assert!(undo.attachments.is_empty());
    assert_eq!(undo.transforms[0].position_millimetres, [0, 0, 0]);
    let redo = super::history::restore_move(&session, undo.scene_revision, &record, true).unwrap();
    assert_eq!(
        serde_json::to_value(&redo.attachments).unwrap(),
        serde_json::to_value(&moved.attachments).unwrap()
    );
    assert_eq!(redo.transforms[0].position_millimetres, [10, 0, 0]);
    assert!(
        super::history::restore_move(&session, undo.scene_revision, &record, false).is_err(),
        "stale history cannot replay"
    );
    drop(session);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn snapped_declared_clamp_uses_rotated_stationary_pipe_geometry() {
    let (session, path, ids) = attachment_test_session();
    let revision = session
        .with(|document| document.patch_revision().map_err(|error| error.to_string()))
        .unwrap();
    apply_transforms(
        &session,
        revision,
        &[
            EntityTransform {
                stage_position_set: None,
                copy_stage_position_set: None,
                id: ids[0],
                position_millimetres: [0, 0, 0],
                rotation_degrees: [0.0, 0.0, 90.0],
            },
            EntityTransform {
                stage_position_set: None,
                copy_stage_position_set: None,
                id: ids[1],
                position_millimetres: [0, 600, 0],
                rotation_degrees: [0.0; 3],
            },
        ],
    )
    .unwrap();
    let result = attachment_gesture(&session, &super::CadState::default(), vec![ids[1]]);
    assert_eq!(
        result.attachments.len(),
        1,
        "lamp along rotated pipe is within declared clip reach"
    );
    assert_eq!(result.attachments[0].truss_member_id, ids[0]);
    drop(session);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn translated_legacy_authorship_is_explicit_and_rotation_preserves_absence() {
    assert_eq!(super::authored_placement(None, true), Some(true));
    assert_eq!(super::authored_placement(None, false), None);
    assert_eq!(super::authored_placement(Some(false), true), Some(true));
    assert_eq!(super::authored_placement(Some(false), false), Some(false));
}

#[test]
fn root_gesture_undo_restores_independent_copy_placement_markers() {
    let (session, path, ids) = transform_session();
    let before = session
        .with(|d| {
            let patch = d.patch_snapshot().map_err(|e| e.to_string())?;
            super::selected_transforms(&patch, &BTreeSet::from([ids[0]]))
        })
        .unwrap();
    assert_eq!(before[0].stage_position_set, Some(false));
    assert!(
        before[0]
            .copy_stage_position_set
            .as_ref()
            .unwrap()
            .values()
            .all(|v| !v)
    );
    let cad = super::CadState::default();
    let revision = session
        .with(|d| d.patch_revision().map_err(|e| e.to_string()))
        .unwrap();
    super::transform::commit(
        &session,
        &cad,
        super::TransformIntent {
            expected_scene_revision: revision,
            entity_ids: vec![ids[0]],
            delta_millimetres: [100, 0, 0],
            snap_to_mounts: false,
            spread: false,
        },
    )
    .unwrap();
    let record = super::history::last_move(&cad.history.lock());
    let moved = session
        .with(|d| {
            let patch = d.patch_snapshot().map_err(|e| e.to_string())?;
            super::selected_transforms(&patch, &BTreeSet::from([ids[0]]))
        })
        .unwrap();
    assert_eq!(moved[0].stage_position_set, Some(true));
    assert!(
        moved[0]
            .copy_stage_position_set
            .as_ref()
            .unwrap()
            .values()
            .all(|v| *v)
    );
    let revision = session
        .with(|d| d.patch_revision().map_err(|e| e.to_string()))
        .unwrap();
    super::history::restore_move(&session, revision, &record, false).unwrap();
    let restored = session
        .with(|d| {
            let patch = d.patch_snapshot().map_err(|e| e.to_string())?;
            super::selected_transforms(&patch, &BTreeSet::from([ids[0]]))
        })
        .unwrap();
    assert_eq!(restored[0].stage_position_set, before[0].stage_position_set);
    assert_eq!(
        restored[0].copy_stage_position_set,
        before[0].copy_stage_position_set
    );
    assert_eq!(
        restored[0].position_millimetres,
        before[0].position_millimetres
    );
    let _ = std::fs::remove_file(path);
}
