use super::*;
use viz_scene::{FixtureBody, FixtureModel, ModelPart, ModelPartKind, MotionAxis};

fn rig() -> (Scene, SceneValues, FrameStyle) {
    let mut scene = Scene::default();
    scene.models.push(FixtureModel {
        extent: Vec3::splat(0.25),
        head_pivot: Vec3::new(0.0, 0.2, 0.0),
        has_head: true,
        parts: [
            ModelPartKind::Base,
            ModelPartKind::Yoke,
            ModelPartKind::Head,
        ]
        .into_iter()
        .map(|kind| ModelPart {
            kind,
            name: format!("{kind:?}"),
            positions: vec![[0.0, 0.0, 0.0], [0.2, 0.0, 0.0], [0.0, 0.3, 0.0]],
            indices: vec![0, 1, 2],
            colour: [0.2; 3],
            ..Default::default()
        })
        .collect(),
        ..Default::default()
    });
    for index in 0..2 {
        scene.fixtures.push(FixtureInstance {
            fixture_id: viz_scene::uuid::Uuid::from_u128(index as u128 + 1),
            instance_id: viz_scene::uuid::Uuid::from_u128(index as u128 + 10),
            model: Some(0),
            patched: false,
            position: Vec3::new(index as f32, 6.0, 0.0),
            installed_colour: [1.0; 3],
            body: FixtureBody::default(),
            ..Default::default()
        });
        let mut emitter = crate::instances::tests::head();
        emitter.fixture_index = index;
        emitter.pan = Some(MotionAxis {
            axis: Vec3::Y,
            min_degrees: -270.0,
            max_degrees: 270.0,
        });
        emitter.tilt = Some(MotionAxis {
            axis: Vec3::X,
            min_degrees: -135.0,
            max_degrees: 135.0,
        });
        scene.emitters.push(emitter);
    }
    let values = SceneValues {
        emitters: vec![
            EmitterValues {
                intensity: 1.0,
                colour: [1.0; 3],
                ..Default::default()
            };
            2
        ],
        ..Default::default()
    };
    let style = FrameStyle {
        quality: viz_scene::RenderQuality::Draft,
        draw_beams: false,
        draw_aim_lines: true,
        emitter_apertures: false,
        floor_grid: false,
        crowd_person_budget: 0,
        effect_particle_budget: 0,
        media_content: false,
        ..Default::default()
    };
    (scene, values, style)
}

#[test]
fn camera_clocks_and_requested_quality_reuse_retained_geometry() {
    let (scene, mut values, mut style) = rig();
    let mut cache = TechnicalLinesInstances::default();
    let frame = cache.update(&scene, &values, &style);
    assert!(frame.meshes.iter().any(
        |(kind, instances)| matches!(kind, MeshKind::ModelPart(_, _)) && instances.len() == 2
    ));
    let mesh_ptr = frame.meshes[0].1.as_ptr();
    let revision = cache.revision();
    values.frame += 1;
    values.newest_input_micros = 100;
    style.camera_position = Some(Vec3::new(5.0, 7.0, 9.0));
    style.quality = viz_scene::RenderQuality::Ultra;
    assert_eq!(
        cache.update(&scene, &values, &style).meshes[0].1.as_ptr(),
        mesh_ptr
    );
    assert_eq!(cache.revision(), revision);
    assert_eq!(cache.scenery.builds, 1);
    assert!(cache.bodies.iter().all(|body| body.builds == 1));
}

#[test]
fn one_moving_fixture_reuses_other_fixture_and_static_bundles() {
    let (scene, mut values, style) = rig();
    let mut cache = TechnicalLinesInstances::default();
    cache.update(&scene, &values, &style);
    let unchanged = cache.bodies[1].frame.meshes[0].1.as_ptr();
    let moving_before = cache.bodies[0]
        .frame
        .meshes
        .iter()
        .find(|(kind, _)| *kind == MeshKind::ModelPart(0, 1))
        .unwrap()
        .1[0]
        .model;
    values.emitters[0].pan = 0.75;
    cache.update(&scene, &values, &style);
    assert_eq!(cache.bodies[0].builds, 2);
    assert_eq!(cache.emitters[0].builds, 2);
    assert_eq!(cache.bodies[1].builds, 1);
    assert_eq!(cache.emitters[1].builds, 1);
    assert_eq!(cache.scenery.builds, 1);
    assert_eq!(cache.bodies[1].frame.meshes[0].1.as_ptr(), unchanged);
    assert_ne!(
        cache.bodies[0]
            .frame
            .meshes
            .iter()
            .find(|(kind, _)| *kind == MeshKind::ModelPart(0, 1))
            .unwrap()
            .1[0]
            .model,
        moving_before
    );
}

#[test]
fn cached_output_preserves_existing_models_pose_selection_and_unpatched_fixtures() {
    let (scene, mut values, style) = rig();
    values
        .selected_fixtures
        .insert(scene.fixtures[0].fixture_id);
    let expected = build(&scene, &values, &style);
    let mut cache = TechnicalLinesInstances::default();
    let actual = cache.update(&scene, &values, &style);
    assert_eq!(actual.meshes.len(), expected.meshes.len());
    for ((actual_kind, actual), (expected_kind, expected)) in
        actual.meshes.iter().zip(&expected.meshes)
    {
        assert_eq!(actual_kind, expected_kind);
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.model, expected.model);
            assert_eq!(actual.normal0, expected.normal0);
            assert_eq!(actual.normal1, expected.normal1);
            assert_eq!(actual.normal2, expected.normal2);
        }
    }
    assert_eq!(
        bytemuck::cast_slice::<LineVertex, u8>(&actual.lines),
        bytemuck::cast_slice::<LineVertex, u8>(&expected.lines)
    );
    assert_eq!(actual.poses.len(), 2);
    for (actual, expected) in actual.poses.iter().zip(&expected.poses) {
        assert_eq!(actual.origin, expected.origin);
        assert_eq!(actual.orientation, expected.orientation);
    }
    assert!(actual.lights.is_empty() && actual.beams.is_empty() && actual.lasers.is_empty());
}

#[test]
fn same_index_model_replacement_theme_guides_and_selection_invalidate_content() {
    let (mut scene, mut values, mut style) = rig();
    let mut cache = TechnicalLinesInstances::default();
    cache.update(&scene, &values, &style);
    let revision = cache.revision();
    scene.models[0].head_pivot.y += 0.4;
    cache.update(&scene, &values, &style);
    assert!(cache.revision() > revision);
    let revision = cache.revision();
    style.aim_guides = true;
    style.faint_ink = Vec3::new(0.2, 0.3, 0.4);
    values.emitters[0].intensity = 0.0;
    values
        .selected_fixtures
        .insert(scene.fixtures[0].fixture_id);
    cache.update(&scene, &values, &style);
    assert!(cache.revision() > revision);
    assert_eq!(cache.bodies[0].builds, 3);
    assert!(!cache.frame.lines.is_empty());
}

#[test]
fn removed_scene_content_is_not_retained_and_show_identity_invalidates() {
    let (mut scene, mut values, style) = rig();
    let mut cache = TechnicalLinesInstances::default();
    cache.update(&scene, &values, &style);
    let revision = cache.revision();
    scene.show_id = Some(viz_scene::uuid::Uuid::from_u128(99));
    cache.update(&scene, &values, &style);
    assert!(cache.revision() > revision);
    scene.fixtures.clear();
    scene.emitters.clear();
    values.emitters.clear();
    let frame = cache.update(&scene, &values, &style);
    assert!(frame.meshes.is_empty() && frame.poses.is_empty() && frame.lines.is_empty());
    assert!(cache.bodies.is_empty() && cache.emitters.is_empty());
}

#[test]
fn lasers_keep_pose_and_aim_without_scans_lights_or_apertures() {
    let (mut scene, values, style) = rig();
    scene.emitters[0].kind = EmitterKind::Laser;
    let mut cache = TechnicalLinesInstances::default();
    let frame = cache.update(&scene, &values, &style);
    assert_eq!(frame.poses.len(), 2);
    assert_eq!(frame.lines.len(), 4);
    assert!(frame.lights.is_empty() && frame.lasers.is_empty() && frame.beams.is_empty());
    assert!(
        frame
            .meshes
            .iter()
            .all(|(kind, _)| matches!(kind, MeshKind::ModelPart(_, _)))
    );
}

#[test]
fn optical_animation_without_pose_or_line_change_reuses_bundles() {
    let (scene, mut values, style) = rig();
    let mut cache = TechnicalLinesInstances::default();
    cache.update(&scene, &values, &style);
    let revision = cache.revision();
    values.emitters[0].gobo_rotation = 0.75;
    values.emitters[0].prism_rotation = 0.5;
    values.emitters[0].gobo = 0.4;
    values.emitters[0].focus = 0.2;
    cache.update(&scene, &values, &style);
    assert_eq!(cache.revision(), revision);
    assert!(cache.bodies.iter().all(|entry| entry.builds == 1));
    assert!(cache.emitters.iter().all(|entry| entry.builds == 1));
}

#[test]
fn changing_dimmer_colour_and_primaries_never_rebuilds_body_or_changes_body_bytes() {
    let (scene, mut values, style) = rig();
    let mut cache = TechnicalLinesInstances::default();
    cache.update(&scene, &values, &style);
    let before = bytemuck::cast_slice::<MeshInstance, u8>(&cache.frame.meshes[0].1).to_vec();
    values.emitters[0].intensity = 0.4;
    values.emitters[0].colour = [0.2, 0.3, 0.4];
    values.emitters[0].source_primaries = [0.9, 0.5, 0.3];
    cache.update(&scene, &values, &style);
    assert_eq!(cache.bodies[0].builds, 1);
    assert_eq!(cache.bodies[1].builds, 1);
    assert_eq!(cache.emitters[0].builds, 2);
    assert_eq!(
        bytemuck::cast_slice::<MeshInstance, u8>(&cache.frame.meshes[0].1),
        before
    );
}
