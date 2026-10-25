//! Exercise the actual shared surface/beam optical gate on the native GPU.
use super::*;
use glam::Vec3;
use viz_scene::*;

fn scene() -> Scene {
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
        lamp_response: Default::default(),
        position: Vec3::new(0.0, 4.0, 0.0),
        installed_colour: [1.0; 3],
        body: FixtureBody {
            kind: BodyKind::Lantern,
            size: Vec3::splat(0.2),
        },
        ..Default::default()
    });
    scene.emitters.push(EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Optical gate".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 50.0,
        field_angle_degrees: 50.0,
        optics: EmitterOptics {
            sharpness: 1.0,
            uniformity: 1.0,
            ..Default::default()
        },
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Floor,
        position: Vec3::new(0.0, -0.05, 0.0),
        size: Vec3::new(12.0, 0.1, 12.0),
        colour: [0.4; 3],
        roughness: 1.0,
        ..Default::default()
    });
    scene.recompute_bounds();
    scene
}

fn view() -> ViewConfiguration {
    let mut view = ViewConfiguration {
        ambient: 0.0,
        floor_grid: false,
        show_labels: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    view.camera.position = Vec3::new(0.0, 5.0, 6.0);
    view.camera.target = Vec3::ZERO;
    view
}

fn capture(renderer: &mut Renderer, scene: &Scene, value: EmitterValues) -> CapturedImage {
    renderer
        .capture(
            scene,
            &SceneValues {
                emitters: vec![value],
                ..Default::default()
            },
            &view(),
            &crate::Overlay::default(),
            0.0,
        )
        .unwrap()
}

fn lit_pixels(image: &CapturedImage) -> usize {
    image
        .rgba
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 30)
        .count()
}

fn evidence(name: &str, image: &CapturedImage) {
    if let Some(directory) = std::env::var_os("VIZ_OPTICS_CAPTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let mut ppm = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
        for pixel in image.rgba.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(directory.join(format!("{name}.ppm")), ppm).unwrap();
    }
}

#[test]
fn two_gobo_wheels_mask_together_and_rotate_independently() {
    let Ok(mut renderer) = Renderer::headless(256, 256) else {
        eprintln!("native GPU unavailable; optical pixel check skipped");
        return;
    };
    let mut scene = scene();
    for axis in 0..2 {
        scene.gobo_artwork.push(GoboArtwork {
            edge: 64,
            mask: (0..4096)
                .map(|pixel| {
                    if (if axis == 0 { pixel % 64 } else { pixel / 64 }) < 32 {
                        255
                    } else {
                        0
                    }
                })
                .collect(),
        });
    }
    scene.emitters[0].optics.gobo_wheels = vec![
        vec![
            GoboSlot::default(),
            GoboSlot {
                name: "Left half".into(),
                artwork: Some(0),
            },
        ],
        vec![
            GoboSlot::default(),
            GoboSlot {
                name: "Top half".into(),
                artwork: Some(1),
            },
        ],
    ];
    let mut value = EmitterValues {
        lamp_response: None,
        intensity: 0.5,
        colour: [1.0; 3],
        gobo_wheels: vec![OpticalWheelValues::default(); 2],
        ..Default::default()
    };
    let open = capture(&mut renderer, &scene, value.clone());
    value.gobo_wheels[0].position = 0.75;
    let first = capture(&mut renderer, &scene, value.clone());
    value.gobo_wheels[0].position = 0.0;
    value.gobo_wheels[1].position = 0.75;
    let second = capture(&mut renderer, &scene, value.clone());
    value.gobo_wheels[0].position = 0.75;
    let combined = capture(&mut renderer, &scene, value.clone());
    evidence("gobo-combined", &combined);
    let counts = [
        lit_pixels(&open),
        lit_pixels(&first),
        lit_pixels(&second),
        lit_pixels(&combined),
    ];
    assert!(
        counts[3] > 50 && counts[3] < counts[1] * 3 / 4 && counts[3] < counts[2] * 3 / 4,
        "both wheels must mask the pool, not replace or ignore one: {counts:?}"
    );
    value.gobo_wheels[1].rotation = 0.25;
    let rotated = capture(&mut renderer, &scene, value);
    evidence("gobo-second-rotated", &rotated);
    assert_ne!(
        combined.rgba, rotated.rgba,
        "second wheel rotation must change projection"
    );
}

#[test]
fn authored_prism_wheels_render_distinct_linear_and_radial_copies() {
    let Ok(mut renderer) = Renderer::headless(256, 256) else {
        return;
    };
    let mut scene = scene();
    scene.emitters[0].optics.prism_wheels = vec![
        vec![
            PrismSlot::default(),
            PrismSlot {
                facets: 3,
                linear: true,
                spread_degrees: 12.0,
            },
        ],
        vec![
            PrismSlot::default(),
            PrismSlot {
                facets: 5,
                linear: false,
                spread_degrees: 12.0,
            },
        ],
    ];
    let mut value = EmitterValues {
        lamp_response: None,
        intensity: 0.5,
        colour: [1.0; 3],
        prism_wheels: vec![OpticalWheelValues::default(); 2],
        ..Default::default()
    };
    value.prism_wheels[0].position = 0.75;
    let linear = capture(&mut renderer, &scene, value.clone());
    value.prism_wheels[0].position = 0.0;
    value.prism_wheels[1].position = 0.75;
    let radial = capture(&mut renderer, &scene, value.clone());
    evidence("prism-linear", &linear);
    evidence("prism-radial", &radial);
    assert!(lit_pixels(&linear) > 100 && lit_pixels(&radial) > 100);
    assert_ne!(
        linear.rgba, radial.rgba,
        "prism representation belongs to its wheel"
    );
    value.prism_wheels[1].rotation = 0.125;
    let rotated = capture(&mut renderer, &scene, value);
    assert_ne!(
        radial.rgba, rotated.rgba,
        "selected prism rotates independently"
    );
}

/// A 45-degree fixture used to lose the far half of its pool at an axis-derived sphere.
#[test]
fn a_tilted_pool_lights_the_floor_beyond_its_axis_range() {
    let Ok(mut renderer) = Renderer::headless(256, 256) else {
        eprintln!("native GPU unavailable; range pixel check skipped");
        return;
    };
    let mut scene = scene();
    scene.emitters[0].local_orientation_degrees.x = -45.0;
    scene.scenery[0].size = Vec3::new(24.0, 0.1, 24.0);
    scene.recompute_bounds();
    let target = Vec3::new(0.0, 0.0, 7.0);
    let mut view = view();
    view.camera.position = Vec3::new(0.0, 12.0, 10.0);
    view.camera.target = Vec3::new(0.0, 0.0, 4.0);
    let values = SceneValues {
        emitters: vec![EmitterValues {
            lamp_response: None,
            intensity: 1.0,
            colour: [1.0; 3],
            ..Default::default()
        }],
        ..Default::default()
    };
    let image = renderer
        .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
        .unwrap();
    evidence("tilted-pool-range", &image);
    let level = receiver_level(&image, &scene, &view, target);
    assert!(level > 30.0, "the far pool must stay lit, measured {level}");
}

fn receiver_level(
    image: &CapturedImage,
    scene: &Scene,
    view: &ViewConfiguration,
    target: Vec3,
) -> f32 {
    let camera =
        crate::ResolvedCamera::resolve(&view.camera, view.mode, 1.0, scene.framing_bounds());
    let (x, y) = camera
        .project(target, image.width as f32, image.height as f32)
        .unwrap();
    let mut sum = 0_u32;
    for dy in -2..=2 {
        for dx in -2..=2 {
            let px = (x.round() as i32 + dx).clamp(0, image.width as i32 - 1) as usize;
            let py = (y.round() as i32 + dy).clamp(0, image.height as i32 - 1) as usize;
            sum += u32::from(image.rgba[(py * image.width as usize + px) * 4]);
        }
    }
    sum as f32 / 25.0
}

/// The finite workload limit fades smoothly before culling; it must not paint a sharp disc.
#[test]
fn a_horizontal_light_fades_before_its_finite_range_boundary() {
    let Ok(mut renderer) = Renderer::headless(256, 256) else {
        eprintln!("native GPU unavailable; range pixel check skipped");
        return;
    };
    let mut scene = scene();
    scene.emitters[0].local_orientation_degrees.x = -90.0;
    scene.scenery[0].kind = SceneryKind::Wall;
    scene.scenery[0].size = Vec3::new(4.0, 4.0, 0.1);
    let values = SceneValues {
        emitters: vec![EmitterValues {
            lamp_response: None,
            intensity: 1.0,
            colour: [1.0; 3],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut levels = Vec::new();
    for distance in [25.6, 28.8, 31.8, 32.2] {
        let receiver = Vec3::new(0.0, 4.0, distance);
        scene.scenery[0].position = receiver + Vec3::Z * 0.05;
        scene.revision += 1;
        scene.recompute_bounds();
        let mut view = view();
        view.camera.position = receiver - Vec3::Z * 5.0;
        view.camera.target = receiver;
        let image = renderer
            .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
            .unwrap();
        evidence(&format!("range-bound-{distance}"), &image);
        levels.push(receiver_level(&image, &scene, &view, receiver));
    }
    assert!(
        levels[0] > 20.0,
        "the useful throw stays visible: {levels:?}"
    );
    assert!(
        levels[1] > levels[2] + 5.0 && levels[0] > levels[1] + 5.0,
        "range energy must taper before the boundary: {levels:?}"
    );
    assert!(
        levels[2] < 5.0 && levels[3] < 1.0,
        "no bright final disc may remain at the work bound: {levels:?}"
    );
}

/// Both a normal 180-light concert rig and a 300-light overflowing tile must retain every
/// lamp's energy, independent of the parallel culling workgroup's arrival order.
#[test]
fn a_crowded_tile_keeps_every_light_and_stays_stable_across_frames() {
    let Ok(mut renderer) = Renderer::headless(256, 256) else {
        eprintln!("native GPU unavailable; crowded tile pixel check skipped");
        return;
    };
    let mut scene = scene();
    scene.fixtures[0].body.size = Vec3::splat(0.001);
    let mut view = view();
    view.quality = RenderQuality::Draft;
    let mut values = SceneValues {
        emitters: vec![EmitterValues {
            lamp_response: None,
            intensity: 0.8,
            colour: [1.0; 3],
            ..Default::default()
        }],
        ..Default::default()
    };
    let reference = renderer
        .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
        .unwrap();
    let expected = receiver_level(&reference, &scene, &view, Vec3::ZERO);
    assert!(
        expected > 40.0,
        "the reference receiver must be lit: {expected}"
    );
    let emitter = scene.emitters[0].clone();
    for count in [180, 300] {
        scene.emitters = vec![emitter.clone(); count];
        scene.revision += 1;
        values.emitters = vec![
            EmitterValues {
                lamp_response: None,
                intensity: 0.8 / count as f32,
                colour: [1.0; 3],
                ..Default::default()
            };
            count
        ];
        for frame in 0..6 {
            let image = renderer
                .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
                .unwrap();
            assert_eq!(renderer.shaded_frame().lights.len(), count);
            let measured = receiver_level(&image, &scene, &view, Vec3::ZERO);
            assert!(
                (measured - expected).abs() <= 2.0,
                "tile with {count} lights must preserve all light on frame {frame}: {measured}, expected {expected}"
            );
        }
    }
}

/// Crossing only the raster proxy must not halve the physical haze integral. The cone mesh
/// is deliberately wider than the analytic light cone, so both cameras see the same shaft.
#[test]
fn haze_energy_is_continuous_when_the_camera_enters_the_widened_proxy() {
    let Ok(mut renderer) = Renderer::headless(256, 256) else {
        eprintln!("native GPU unavailable; beam proxy pixel check skipped");
        return;
    };
    let mut scene = scene();
    scene.scenery.clear();
    scene.recompute_bounds();
    let mut view = view();
    view.quality = RenderQuality::High;
    let mut values = SceneValues {
        emitters: vec![EmitterValues {
            lamp_response: None,
            intensity: 0.8,
            colour: [1.0; 3],
            ..Default::default()
        }],
        ..Default::default()
    };
    values.atmosphere.density = 1.0;
    view.fog_variation.lamp_cloudiness = 0.0;
    let mut levels = Vec::new();
    // At y=2 the true cone radius is about 0.93m and the raster proxy radius is 1.26m.
    for x in [1.4, 1.1, 0.0] {
        view.camera.position = Vec3::new(x, 2.0, 0.0);
        view.camera.target = Vec3::new(-1.0, 2.0, 0.0);
        let image = renderer
            .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
            .unwrap();
        levels.push(receiver_level(&image, &scene, &view, view.camera.target));
    }
    assert!(
        levels[0] > 20.0,
        "the outside camera must see a visible shaft: {levels:?}"
    );
    assert!(
        (levels[0] - levels[1]).abs() <= 3.0,
        "crossing the proxy without entering physical haze must not change energy: {levels:?}"
    );
    assert!(
        levels[2] > 5.0 && levels[2] < levels[1] - 3.0,
        "a camera inside the true cone must still see its remaining partial shaft: {levels:?}"
    );
}

/// Wide proxies can extend past the view's far plane even though the ray's near haze is lit.
/// Exit-face rasterization must retain coverage there; the analytic integral still stops at depth.
#[test]
fn a_wide_beam_stays_visible_when_its_exit_proxy_is_beyond_the_far_plane() {
    let Ok(mut renderer) = Renderer::headless(256, 256) else {
        eprintln!("native GPU unavailable; far-clipped beam pixel check skipped");
        return;
    };
    let mut scene = scene();
    scene.scenery.clear();
    scene.emitters[0].beam_angle_degrees = 160.0;
    scene.emitters[0].field_angle_degrees = 160.0;
    scene.recompute_bounds();
    let mut view = view();
    view.quality = RenderQuality::High;
    view.fog_variation.lamp_cloudiness = 0.0;
    view.camera.position = Vec3::new(30.0, -6.0, 0.0);
    view.camera.target = Vec3::new(0.0, -6.0, 0.0);
    let camera =
        crate::ResolvedCamera::resolve(&view.camera, view.mode, 1.0, scene.framing_bounds());
    assert_eq!(camera.far, 60.0);
    let mut values = SceneValues {
        emitters: vec![EmitterValues {
            lamp_response: None,
            intensity: 1.0,
            colour: [1.0; 3],
            ..Default::default()
        }],
        ..Default::default()
    };
    values.atmosphere.density = 1.0;
    let image = renderer
        .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
        .unwrap();
    let level = receiver_level(&image, &scene, &view, view.camera.target);
    assert!(
        level > 8.0,
        "the visible near haze must survive far clipping of its proxy: {level}"
    );
}
