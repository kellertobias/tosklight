//! Exercise the actual shared surface/beam optical gate on the native GPU.
use super::*;
use glam::Vec3;
use viz_scene::*;

fn scene() -> Scene {
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
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
