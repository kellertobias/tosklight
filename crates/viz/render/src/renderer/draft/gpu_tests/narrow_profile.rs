//! Shared radial-profile acceptance, independently initialized Draft and Ultra at 2 degrees.
use super::*;

#[test]
#[ignore = "requires explicitly serialized native GPU corrected narrow-profile pixel acceptance"]
fn physical_two_degree_shaft_has_visible_core_in_draft_and_ultra() {
    for quality in [RenderQuality::Draft, RenderQuality::Ultra] {
        let mut renderer = Renderer::headless(961, 541).expect("requested native GPU required");
        let (mut scene, mut values, mut view) = rig();
        scene.revision = 19;
        scene.emitters[0].beam_angle_degrees = 2.0;
        scene.emitters[0].field_angle_degrees = 2.0;
        scene.emitters[0].optics.sharpness = 0.92;
        scene.emitters[0].optics.uniformity = 0.95;
        values.emitters[0].intensity = 1.0;
        values.atmosphere.density = 0.8;
        view.quality = quality;
        view.fog_variation = FogVariation {
            lamp_cloudiness: 0.0,
            lamp_turbulence: 0.0,
            laser_cloudiness: 0.0,
            laser_turbulence: 0.0,
        };
        view.camera.position = Vec3::new(0.35, 2.3, 0.6);
        view.camera.target = Vec3::new(0.0, 2.0, 0.0);
        let image = capture(&mut renderer, &scene, &values, &view);
        values.atmosphere.density = 0.0;
        let clear = capture(&mut renderer, &scene, &values, &view);
        let mut visible = 0;
        let mut maximum = 0;
        for (pixel, clear) in image.rgba.chunks_exact(4).zip(clear.rgba.chunks_exact(4)) {
            let signal = i16::from(pixel[0]) - i16::from(clear[0]);
            visible += usize::from(signal > 8);
            maximum = maximum.max(signal);
        }
        println!(
            "corrected physical 2-degree {quality:?}: shaftMax={maximum} pixelsOver8={visible}"
        );
        assert!(
            visible > 30 && maximum > 8,
            "corrected narrow shaft must have real visible core"
        );
    }
}
