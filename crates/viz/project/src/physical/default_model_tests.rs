use crate::{PatchedFixture, PhysicalInstance};
use glam::{Mat4, Vec3};
use std::{path::Path, sync::Arc};
use uuid::Uuid;
use viz_scene::{EmitterValues, ModelPartKind, SceneValues};

fn fixture(profile: light_fixture::FixtureProfile, mode_id: Uuid, scale: f32) -> PatchedFixture {
    PatchedFixture {
        fixture_id: Uuid::new_v4(),
        name: "Model alignment".into(),
        number: Some(1),
        profile: Arc::new(profile),
        mode_id,
        instances: vec![PhysicalInstance {
            instance_id: Uuid::new_v4(),
            name: "Model alignment".into(),
            split_patches: vec![(1, Some((1, 1)))],
            position: Vec3::new(2., 5., -3.),
            rotation_degrees: Vec3::new(0., 37., 0.),
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.,
            shaper_angle: None,
            installed_appearance: Default::default(),
            scenery_size_metres: None,
            scenery_options: Default::default(),
            model_scale: scale,
            color_calibration: None,
            position_calibration: None,
        }],
    }
}

#[test]
fn authored_motion_keeps_every_shipped_default_lens_attached_at_every_scale() {
    for package in [
        "robe--robin-dls-profile",
        "jb-lighting--jbled-a7",
        "martin--mac-300",
        "cameo--auro-spot-z300",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/fixture-library")
            .join(format!("{package}.toskfixture"));
        let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
        for mode in &profile.modes {
            for scale in [0.5, 1., 2.] {
                let patch = fixture(profile.clone(), mode.id, scale);
                let compiled = crate::compile(&[patch]);
                let scene = &compiled.scene;
                let drawn = &scene.fixtures[0];
                let model = &scene.models[drawn.model.unwrap() as usize];
                let physical = &scene.physical_positions[0];
                assert!(
                    model
                        .parts
                        .iter()
                        .filter(|p| p.kind == ModelPartKind::Head)
                        .all(|p| p.geometry_node_id.is_some())
                );
                assert!((model.scale_to(drawn.body.size) - scale).abs() < 1e-5);
                let (node, rest) = physical.model_lens_rest[0].expect("real model lens rest frame");
                assert!(
                    rest.transform_point3(Vec3::ZERO)
                        .abs_diff_eq(model.emitter_anchor.unwrap(), 1e-6)
                );
                let mut values = SceneValues {
                    emitters: vec![EmitterValues::default(); scene.emitters.len()],
                    ..Default::default()
                };
                values.reconcile_physical_positions(scene);
                for angle in [0., 31., -67.] {
                    for (index, axis) in values.physical_positions[0].axes.iter_mut().enumerate() {
                        axis.has_position = true;
                        axis.known = true;
                        axis.motion.position_degrees = angle + index as f32 * 17.;
                    }
                    values.apply_calibrated_motion(scene, 0.);
                    let delta = values.physical_positions[0].node_deltas[node].unwrap();
                    let expected = delta * rest;
                    let actual = values.emitters[0].physical_pose.unwrap().local.unwrap();
                    assert!(
                        actual.abs_diff_eq(expected, 1e-5),
                        "{package} {} {scale} {angle}",
                        mode.name
                    );
                    // This is the renderer's exact model-part transform: instance scale is applied
                    // after the authored graph delta, to both the real glass and the light source.
                    let glass = (Mat4::from_scale(Vec3::splat(scale)) * delta)
                        .transform_point3(model.emitter_anchor.unwrap());
                    assert!(glass.abs_diff_eq(actual.transform_point3(Vec3::ZERO) * scale, 1e-5));
                }
            }
        }
    }
}

#[test]
fn packaged_custom_model_keeps_its_authored_lens_and_rest_frame() {
    use base64::Engine;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/martin--mac-300.toskfixture");
    let mut profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let model = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/models/lamps/moving-head-led-wash-400.glb"),
    )
    .unwrap();
    profile.model_asset = Some(format!(
        "data:model/gltf-binary;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(model)
    ));
    let mode = profile.modes[0].id;
    let compiled = crate::compile(&[fixture(profile, mode, 2.)]);
    assert!(
        compiled.scene.physical_positions[0]
            .model_lens_rest
            .is_empty()
    );
}

#[test]
fn physical_default_keeps_cad_line_drawings_at_the_fitted_body_size() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/martin--mac-300.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode = profile.modes[0].id;
    let compiled = crate::compile(&[fixture(profile, mode, 1.)]);
    let source = crate::read_shipped_model(&crate::default_model::MOVING_LED_WASH).unwrap();
    let expected = crate::plan_drawing::model_drawing_artwork(
        "moving-head-led-wash-400",
        source.scale_to(compiled.scene.fixtures[0].body.size),
    );
    assert!(!expected.is_empty());
    for drawing in expected {
        let actual = compiled
            .scene
            .plan_artwork
            .iter()
            .find(|p| p.view == drawing.view)
            .unwrap();
        assert_eq!(actual.vertices.len(), drawing.vertices.len());
        for (actual, expected) in actual.vertices.iter().zip(&drawing.vertices) {
            assert!(Vec3::from_array(*actual).abs_diff_eq(Vec3::from_array(*expected), 1e-6));
        }
    }
}
