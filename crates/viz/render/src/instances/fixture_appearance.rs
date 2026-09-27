//! Appearance carried by real fixture model parts: scanner mirrors and discrete RGB diodes.

use super::*;
use viz_scene::{FixtureModel, ModelPart, ModelPartKind};

pub(super) fn scanner_mirror_pivot(model: &FixtureModel) -> Option<Vec3> {
    let mirror = model.parts.iter().find(|part| part.name == "head-mirror")?;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for position in &mirror.positions {
        let position = Vec3::from_array(*position);
        min = min.min(position);
        max = max.max(position);
    }
    (min.x <= max.x).then_some((min + max) * 0.5)
}

/// A scanner's chassis stays fixed; only its gimbal and mirror turn. A mirror turns
/// half as far as the outgoing beam. This is a mechanical preview, not ray reflection.
pub(super) fn scanner_part_transform(
    model: &FixtureModel,
    part: &ModelPart,
    base: Mat4,
    pan_degrees: f32,
    tilt_degrees: f32,
) -> Option<Mat4> {
    let pivot = scanner_mirror_pivot(model)?;
    let pan = Mat4::from_rotation_y((pan_degrees * 0.5).to_radians());
    let tilt = Mat4::from_rotation_x((tilt_degrees * 0.5).to_radians());
    let rotation = match part.kind {
        ModelPartKind::Base => return Some(base),
        ModelPartKind::Yoke => pan,
        ModelPartKind::Head => pan * tilt,
    };
    Some(base * Mat4::from_translation(pivot) * rotation * Mat4::from_translation(-pivot))
}

/// A fixed scanner's beam leaves the mirror, rather than its upstream lens.
pub(super) fn scanner_emitter_pose(
    model: &FixtureModel,
    fixture: &FixtureInstance,
    emitter: &EmitterInstance,
    pan_degrees: f32,
    tilt_degrees: f32,
    zoom: f32,
    points: &[viz_scene::PointPose],
) -> Option<EmitterPose> {
    let pivot = scanner_mirror_pivot(model)? * model.scale_to(fixture.body.size);
    let (position, mount) = fixture.placed_by(points);
    // The lamp shines down onto the resting 45-degree mirror, which turns it
    // toward the front of the fixture. Beam steering is the full deflection.
    let orientation = mount
        * Quat::from_rotation_y(pan_degrees.to_radians())
        * Quat::from_rotation_x(tilt_degrees.to_radians())
        * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    Some(EmitterPose {
        origin: position + mount * pivot,
        direction: (orientation * Vec3::NEG_Y).normalize(),
        half_angle: emitter.cone_half_angle(zoom),
        orientation,
    })
}

pub(super) fn has_discrete_rgb_sources(model: &FixtureModel) -> bool {
    ["source-red", "source-green", "source-blue"]
        .iter()
        .all(|name| model.parts.iter().any(|part| part.name == *name))
}

/// Light only the matching diode glass. The aggregate intensity already includes
/// the brightest additive channel, so divide it out before applying a diode drive.
pub(super) fn source_part_emission(
    part: &ModelPart,
    raw_rgb: [f32; 3],
    visible_intensity: f32,
    installed_colour: Vec3,
) -> Vec3 {
    let primary = match part.name.as_str() {
        "source-red" => 0,
        "source-green" => 1,
        "source-blue" => 2,
        _ => return Vec3::ZERO,
    };
    let peak = raw_rgb.into_iter().fold(0.0_f32, f32::max);
    if peak <= 0.0 {
        return Vec3::ZERO;
    }
    let mut colour = Vec3::ZERO;
    colour[primary] = raw_rgb[primary].clamp(0.0, 1.0) / peak;
    colour * installed_colour * visible_intensity.clamp(0.0, 1.0) * APERTURE_RADIANCE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_scene(model: FixtureModel) -> Scene {
        let mut scene = Scene::default();
        scene.fixtures.push(FixtureInstance {
            body: viz_scene::FixtureBody {
                size: model.extent * 2.0,
                kind: viz_scene::BodyKind::Lantern,
            },
            installed_colour: [1.0; 3],
            model: Some(0),
            patched: true,
            ..Default::default()
        });
        scene.emitters.push(EmitterInstance {
            fixture_index: 0,
            head_index: 0,
            label: "Source".into(),
            local_origin: model.emitter_anchor.unwrap(),
            tilt_pivot: model.head_pivot,
            local_orientation_degrees: Vec3::ZERO,
            pan: Some(viz_scene::MotionAxis {
                axis: Vec3::Y,
                min_degrees: -90.0,
                max_degrees: 90.0,
            }),
            tilt: Some(viz_scene::MotionAxis {
                axis: Vec3::X,
                min_degrees: -90.0,
                max_degrees: 90.0,
            }),
            beam_angle_degrees: 12.0,
            field_angle_degrees: 20.0,
            optics: viz_scene::EmitterOptics::default(),
            kind: EmitterKind::Beam,
            cells: viz_scene::EmitterLayoutCells::single(),
            laser: None,
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        });
        scene.models.push(model);
        scene.recompute_bounds();
        scene
    }

    #[test]
    fn full_frame_keeps_scanner_chassis_fixed_and_exports_the_mirror_beam() {
        let scene = fixture_scene(scanner());
        let values_at = |pan, tilt| SceneValues {
            emitters: vec![EmitterValues {
                pan,
                tilt,
                intensity: 1.0,
                ..Default::default()
            }],
            ..Default::default()
        };
        let a_values = values_at(0.5, 0.5);
        let b_values = values_at(0.85, 0.2);
        let a = build(&scene, &a_values, &FrameStyle::default());
        let b = build(&scene, &b_values, &FrameStyle::default());
        for (index, part) in scene.models[0].parts.iter().enumerate() {
            let transform = |frame: &FrameInstances| {
                frame
                    .meshes
                    .iter()
                    .find(|(kind, _)| *kind == MeshKind::ModelPart(0, index as u32))
                    .unwrap()
                    .1[0]
                    .model
            };
            if part.kind == ModelPartKind::Base {
                assert_eq!(
                    transform(&a),
                    transform(&b),
                    "fixed scanner part {} moved",
                    part.name
                );
            } else {
                assert_ne!(
                    transform(&a),
                    transform(&b),
                    "mirror part {} failed to move",
                    part.name
                );
            }
        }
        assert!(a.poses[0].origin.distance(b.poses[0].origin) < 1e-6);
        assert!(a.poses[0].direction.distance(b.poses[0].direction) > 0.1);
        let pivot = scanner_mirror_pivot(&scene.models[0]).unwrap();
        assert!(a.poses[0].origin.distance(pivot) < 1e-6);
        assert!(a.poses[0].direction.distance(Vec3::Z) < 1e-6);
        let exported = semantic_lights(&scene, &b_values);
        assert!(exported[0].origin.distance(b.poses[0].origin) < 1e-6);
        assert!(exported[0].direction.distance(b.poses[0].direction) < 1e-6);
    }

    #[test]
    fn native_capture_shows_separate_pizza_primary_glass() {
        let Ok(mut renderer) = crate::Renderer::headless(384, 384) else {
            eprintln!("GPU unavailable; fixture geometry/value tests remain authoritative");
            return;
        };
        let pizza = viz_scene::read_glb(include_bytes!(
            "../../../../../assets/models/lamps/led-par-pizza.glb"
        ))
        .unwrap();
        let mut scene = fixture_scene(pizza);
        scene.emitters[0].pan = None;
        scene.emitters[0].tilt = None;
        let mut view = viz_scene::ViewConfiguration {
            ambient: 0.025,
            floor_grid: false,
            background: Some([0.0; 3]),
            ..Default::default()
        };
        view.camera.target = scene.models[0].emitter_anchor.unwrap();
        view.camera.position = view.camera.target + Vec3::new(0.12, -0.6, 0.24);
        for (name, raw, colour) in [
            ("pizza-red", [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
            ("pizza-green", [0.0, 1.0, 0.0], [0.0, 1.0, 0.0]),
        ] {
            let values = SceneValues {
                emitters: vec![EmitterValues {
                    intensity: 1.0,
                    source_primaries: raw,
                    colour,
                    ..Default::default()
                }],
                ..Default::default()
            };
            let image = renderer
                .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
                .unwrap();
            let channel = if name == "pizza-red" { 0 } else { 1 };
            let coloured = image
                .rgba
                .chunks_exact(4)
                .filter(|pixel| {
                    pixel[channel] > 80 && pixel[channel].saturating_sub(pixel[1 - channel]) > 30
                })
                .count();
            assert!(
                coloured > 80,
                "native {name} capture lost coloured diode faces: {coloured}"
            );
            write_capture(name, &image);
        }
        let mut renderer = crate::Renderer::headless(384, 384).unwrap();
        let scene = fixture_scene(scanner());
        view.ambient = 0.5;
        view.camera.target = scanner_mirror_pivot(&scene.models[0]).unwrap();
        view.camera.target.y += 0.15;
        view.camera.position = view.camera.target + Vec3::new(0.45, 0.30, 1.25);
        let mut scanner_images = Vec::new();
        for (name, pan, tilt) in [("scanner-home", 0.5, 0.5), ("scanner-steered", 0.85, 0.2)] {
            let values = SceneValues {
                emitters: vec![EmitterValues {
                    intensity: 0.0,
                    pan,
                    tilt,
                    ..Default::default()
                }],
                ..Default::default()
            };
            let image = renderer
                .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
                .unwrap();
            write_capture(name, &image);
            scanner_images.push(image.rgba);
        }
        assert_ne!(
            scanner_images[0], scanner_images[1],
            "scanner mirror must visibly move"
        );
    }

    fn write_capture(name: &str, image: &crate::CapturedImage) {
        if let Some(directory) = std::env::var_os("VIZ_FIXTURE_CAPTURE_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            let mut ppm = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
            for pixel in image.rgba.chunks_exact(4) {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(
                std::path::Path::new(&directory).join(format!("{name}.ppm")),
                ppm,
            )
            .unwrap();
        }
    }

    fn scanner() -> FixtureModel {
        viz_scene::read_glb(include_bytes!(
            "../../../../../assets/models/lamps/scanner-mirror-spot.glb"
        ))
        .unwrap()
    }

    #[test]
    fn scanner_keeps_chassis_fixed_and_mirror_pivot_stationary() {
        let model = scanner();
        let pivot = scanner_mirror_pivot(&model).unwrap();
        let chassis = model
            .parts
            .iter()
            .find(|part| part.name == "scanner-chassis")
            .unwrap();
        let mirror = model
            .parts
            .iter()
            .find(|part| part.name == "head-mirror")
            .unwrap();
        assert_eq!(
            scanner_part_transform(&model, chassis, Mat4::IDENTITY, 60.0, 80.0),
            Some(Mat4::IDENTITY)
        );
        let transform = scanner_part_transform(&model, mirror, Mat4::IDENTITY, 60.0, 80.0).unwrap();
        assert!(transform.transform_point3(pivot).distance(pivot) < 1e-6);
        let expected = Quat::from_rotation_y(30.0_f32.to_radians())
            * Quat::from_rotation_x(40.0_f32.to_radians());
        assert!(
            transform
                .transform_vector3(Vec3::Z)
                .distance(expected * Vec3::Z)
                < 1e-6
        );
    }

    #[test]
    fn real_pizza_sources_have_separate_primary_glass_and_do_not_double_dim() {
        let model = viz_scene::read_glb(include_bytes!(
            "../../../../../assets/models/lamps/led-par-pizza.glb"
        ))
        .unwrap();
        assert!(has_discrete_rgb_sources(&model));
        let red = model
            .parts
            .iter()
            .find(|part| part.name == "source-red")
            .unwrap();
        let green = model
            .parts
            .iter()
            .find(|part| part.name == "source-green")
            .unwrap();
        let blue = model
            .parts
            .iter()
            .find(|part| part.name == "source-blue")
            .unwrap();
        // Preserve the 31 real domes: eleven red, ten green and ten blue, each
        // retaining the same tessellation. No invented aperture replaces them.
        assert_eq!(red.indices.len() * 10, green.indices.len() * 11);
        assert_eq!(green.indices.len(), blue.indices.len());
        let red_light = source_part_emission(red, [0.5, 0.0, 0.0], 0.5, Vec3::ONE);
        assert!((red_light.x - APERTURE_RADIANCE * 0.5).abs() < 1e-6);
        assert_eq!(red_light.y, 0.0);
        assert_eq!(
            source_part_emission(green, [0.5, 0.0, 0.0], 0.5, Vec3::ONE),
            Vec3::ZERO
        );
        assert_eq!(
            source_part_emission(red, [0.0; 3], 1.0, Vec3::ONE),
            Vec3::ZERO
        );
        let green_light = source_part_emission(green, [0.5, 0.25, 0.0], 0.25, Vec3::ONE);
        assert!((green_light.y - APERTURE_RADIANCE * 0.125).abs() < 1e-6);
    }
}
