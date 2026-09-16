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
