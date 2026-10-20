//! The bracket hinge a compiled lamp carries into the scene.

use super::*;

fn fresnel(bracket_angle: f32, model_scale: f32) -> PatchedFixture {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Generic".into();
    profile.name = "Fresnel".into();
    profile.fixture_type = "fresnel".into();
    let mode_id = profile.modes[0].id;
    PatchedFixture {
        fixture_id: Uuid::new_v4(),
        name: "Fresnel".into(),
        number: Some(1),
        profile: Arc::new(profile),
        mode_id,
        instances: vec![PhysicalInstance {
            color_calibration: None,
            position_calibration: None,
            model_scale,
            scenery_options: Default::default(),
            scenery_size_metres: None,
            instance_id: Uuid::new_v4(),
            name: "Fresnel".into(),
            split_patches: vec![(1, Some((1, 1)))],
            position: Vec3::new(0.0, 5.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle,
            shaper_angle: None,
            installed_appearance: InstalledFixtureAppearance::default(),
        }],
    }
}

/// A shipped Fresnel turns in its hanging frame about the bolts its manifest records, at the size
/// the model is drawn, so the body, its beam and the CAD all turn about the same point.
#[test]
fn a_shipped_fresnel_carries_its_hinge_at_the_drawn_scale() {
    for scale in [1.0, 2.0] {
        let plan = compile(&[fresnel(45.0, scale)]);
        let fixture = &plan.scene.fixtures[0];
        assert_eq!(fixture.bracket_degrees, 45.0);
        let model =
            &plan.scene.models[fixture.model.expect("drawn from the shipped Fresnel") as usize];
        let hinge = crate::bracket_hinge("fresnel-barn-doors").expect("the Fresnel has a hinge");
        assert_eq!(model.bracket_hinge, Some(hinge));
        let expected = hinge * model.scale_to(fixture.body.size);
        let actual = fixture
            .bracket_hinge
            .expect("the instance carries the hinge");
        assert!(
            (actual - expected).length() < 1e-6,
            "{actual:?} {expected:?}"
        );
        // The emitter leaves the same lens the body is drawn with.
        let lens = plan.scene.emitters[0].local_origin;
        assert!(
            lens.y < actual.y,
            "the lens hangs below the hinge: {lens:?} {actual:?}"
        );
    }
}

fn showcase_fresnel(bracket_angle: f32, model_scale: f32, yaw: f32) -> PatchedFixture {
    let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/prolights--ecl-fresnel-ct-plus-m.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(package).unwrap())
        .expect("the actual Showcase Fresnel package reads");
    let mut fixture = fresnel(bracket_angle, model_scale);
    fixture.mode_id = profile.modes[0].id;
    fixture.profile = Arc::new(profile);
    fixture.instances[0].position = Vec3::new(-4.5, 5.68, 6.12);
    fixture.instances[0].rotation_degrees = Vec3::new(0.0, yaw, 0.0);
    fixture
}

/// The shipped personality has a point emitter at an identity chassis origin. That placeholder
/// must retain its identity and DMX ownership while borrowing the lens of its drawn default body.
#[test]
fn showcase_fresnel_placeholder_leaves_the_drawn_lens_at_every_installed_scale_and_aim() {
    for scale in [0.5, 1.0, 2.0] {
        for (bracket, yaw) in [(0.0, 0.0), (51.613_018, -6.525_083), (64.150_3, 52.584_01)] {
            let patched = showcase_fresnel(bracket, scale, yaw);
            let authored = &patched.profile.geometry.emitters[0];
            let emitter_id = authored.id;
            let plan = compile(&[patched]);
            assert_eq!(plan.scene.emitters.len(), 1);
            assert_eq!(plan.bindings.len(), 1);
            assert_eq!(plan.scene.emitter_ids, vec![emitter_id]);
            let fixture = &plan.scene.fixtures[0];
            let emitter = &plan.scene.emitters[0];
            let model = &plan.scene.models[fixture.model.unwrap() as usize];
            let fitted = model.scale_to(fixture.body.size);
            let lens = model
                .emitter_anchor
                .expect("the drawn body identifies its lens")
                * fitted;
            let hinge = model
                .bracket_hinge
                .expect("the drawn body identifies its bracket")
                * fitted;
            assert!(
                emitter.local_origin.abs_diff_eq(lens, 1e-6),
                "scale {scale}, bracket {bracket}: {:?} versus lens {lens:?}",
                emitter.local_origin
            );
            assert!(fixture.bracket_hinge.unwrap().abs_diff_eq(hinge, 1e-6));
            assert_eq!(emitter.tilt_pivot, Vec3::ZERO);
            assert_eq!(emitter.head_index, 0);
            assert_eq!(emitter.label, "Lens");
            assert_eq!(emitter.beam_angle_degrees, 9.4);
            assert_eq!(emitter.field_angle_degrees, 10.81);
            let aim = viz_scene::euler_degrees(emitter.local_orientation_degrees) * Vec3::NEG_Y;
            assert!(aim.abs_diff_eq(model.emitter_axis.unwrap(), 1e-6));
            assert!((emitter.optics.source.width - 0.128_455_1 * scale).abs() < 1e-6);
            assert!((emitter.optics.source.height - 0.128_455_1 * scale).abs() < 1e-6);
            let (position, rotation) = fixture.placed_by(&[]);
            let mount_rotation = viz_scene::euler_degrees(fixture.rotation_degrees);
            let bracket_rotation = Quat::from_rotation_x(bracket.to_radians());
            let world_lens =
                fixture.position + mount_rotation * (hinge + bracket_rotation * (lens - hinge));
            let world_source = position + rotation * emitter.local_origin;
            assert!(world_source.abs_diff_eq(world_lens, 1e-6));
            let world_axis = mount_rotation * bracket_rotation * model.emitter_axis.unwrap();
            assert!((rotation * aim).abs_diff_eq(world_axis, 1e-6));
            let intensity = plan.bindings[0]
                .intensity
                .as_ref()
                .expect("original head still owns intensity");
            assert_eq!(intensity.logical_universe, 1);
            assert_eq!(intensity.slots, vec![1, 2]);
            assert_eq!(intensity.max_raw, 65_535);
        }
    }
}

#[test]
fn a_fresnels_authored_nonzero_lens_is_not_replaced_by_the_default_body_anchor() {
    let mut fixture = showcase_fresnel(60.0, 1.0, 30.0);
    Arc::get_mut(&mut fixture.profile)
        .unwrap()
        .geometry
        .emitters[0]
        .origin = Vector3 {
        x: 12.0,
        y: -250.0,
        z: 30.0,
    };
    let plan = compile(&[fixture]);
    assert!(
        plan.scene.emitters[0]
            .local_origin
            .abs_diff_eq(Vec3::new(0.012, -0.25, 0.03), 1e-6)
    );
}

#[test]
fn a_fresnels_authored_chassis_pose_is_not_treated_as_an_identity_placeholder() {
    let mut fixture = showcase_fresnel(60.0, 1.0, 30.0);
    Arc::get_mut(&mut fixture.profile).unwrap().geometry.nodes[0]
        .transform
        .translation
        .x = 30.0;
    let plan = compile(&[fixture]);
    let source = plan.scene.emitters[0].local_origin;
    let drawn = &plan.scene.fixtures[0];
    let model = &plan.scene.models[drawn.model.unwrap() as usize];
    let default_lens = model.emitter_anchor.unwrap() * model.scale_to(drawn.body.size);
    assert!(
        !source.abs_diff_eq(default_lens, 1e-4),
        "an authored chassis pose must not be silently moved onto the fallback lens"
    );
}

#[test]
fn a_fresnels_custom_model_does_not_reinterpret_an_authored_zero_origin() {
    use base64::Engine;
    let mut fixture = showcase_fresnel(60.0, 1.0, 30.0);
    let model = crate::default_model::choose(None, "fresnel", Default::default());
    Arc::get_mut(&mut fixture.profile).unwrap().model_asset = Some(format!(
        "data:model/gltf-binary;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(model.bytes)
    ));
    let plan = compile(&[fixture]);
    assert!(plan.scene.fixtures[0].model.is_some());
    assert_eq!(plan.scene.emitters[0].local_origin, Vec3::ZERO);
}
