use super::*;

#[test]
fn manual_barn_doors_have_independent_softness_and_full_stroke() {
    let mut scene = Scene::default();
    let mut lamp = fixture();
    lamp.installed_shaper_blades = [0.75, 0.35, 0.1, 0.2];
    lamp.installed_shaper_angles_degrees = [10.0, -20.0, 30.0, 40.0];
    scene.fixtures.push(lamp);
    scene.emitters.push(emitter());
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    let packed = build(&scene, &values, &FrameStyle::default()).lights[0];
    assert_eq!(packed.shapers, [1.5, 0.7, 0.2, 0.4]);
    assert_eq!(packed.shaper_params, [0.04, 0.0, 0.0, 0.0]);
    for (angle, expected) in packed
        .shaper_angles
        .into_iter()
        .zip([10.0_f32, -20.0, 30.0, 40.0])
    {
        assert!((angle - expected.to_radians()).abs() < 1e-6);
    }
    // A profile's actual shaper controls retain the established live stroke and softness.
    scene.emitters[0].shaper_roles = [true; 4];
    values.emitters[0].shaper_blades = [0.12, 0.23, 0.34, 0.45];
    let live = build(&scene, &values, &FrameStyle::default()).lights[0];
    assert_eq!(live.shapers, values.emitters[0].shaper_blades);
    assert_eq!(live.shaper_params, [0.0; 4]);
}
