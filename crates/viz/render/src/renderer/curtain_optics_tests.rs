//! Opaque curtains stop every lamp, including lamps outside the shadow-map budget.
use super::*;
use glam::{EulerRot, Quat, Vec3};
use viz_scene::*;

const EDGE: u32 = 256;
const TARGET: Vec3 = Vec3::new(0.0, 0.0, 2.0);
const TIERS: [RenderQuality; 4] = [
    RenderQuality::Draft,
    RenderQuality::Standard,
    RenderQuality::High,
    RenderQuality::Ultra,
];

/// Eleven brighter, upward-facing lamps exhaust even Ultra's fixture shadow budget. They do
/// not light the measured floor, so the final lamp isolates the budget-independent blocker.
fn rig(front_light: bool, turn: f32) -> (Scene, SceneValues, ViewConfiguration, Vec3) {
    let rotation = Quat::from_rotation_y(turn.to_radians());
    let target = rotation * TARGET;
    let mut scene = Scene::default();
    let mut values = SceneValues::default();
    for index in 0..12 {
        let source = if index == 11 {
            if front_light {
                Vec3::new(0.0, 3.0, 4.0)
            } else {
                Vec3::new(-3.0, 3.0, -3.0)
            }
        } else {
            Vec3::new(index as f32 * 0.1, 10.0, -10.0)
        };
        let source = rotation * source;
        let direction = if index == 11 {
            (target - source).normalize()
        } else {
            Vec3::Y
        };
        let (x, y, z) = Quat::from_rotation_arc(Vec3::NEG_Y, direction).to_euler(EulerRot::XYZ);
        scene.fixtures.push(FixtureInstance {
            position: source,
            installed_colour: [1.0; 3],
            body: FixtureBody {
                kind: BodyKind::Lantern,
                size: Vec3::splat(0.01),
            },
            ..Default::default()
        });
        scene.emitters.push(EmitterInstance {
            fixture_index: index,
            head_index: 0,
            label: format!("Curtain regression lamp {index}"),
            local_origin: Vec3::ZERO,
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees()),
            pan: None,
            tilt: None,
            beam_angle_degrees: 35.0,
            field_angle_degrees: 35.0,
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
        values.emitters.push(EmitterValues {
            intensity: if index == 11 { 0.5 } else { 1.0 },
            colour: [1.0; 3],
            ..Default::default()
        });
    }
    values.atmosphere.density = 0.0;
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Floor,
        position: Vec3::new(0.0, -0.05, 0.0),
        size: Vec3::new(16.0, 0.1, 16.0),
        colour: [0.5; 3],
        roughness: 1.0,
        ..Default::default()
    });
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Curtain,
        position: Vec3::new(0.0, 2.0, 0.0),
        rotation_degrees: Vec3::new(0.0, turn, 0.0),
        size: Vec3::new(4.0, 4.0, 0.2),
        colour: [0.015; 3],
        roughness: 0.95,
        ..Default::default()
    });
    scene.recompute_bounds();
    let mut view = ViewConfiguration {
        mode: ViewMode::Full3d,
        ambient: 0.0,
        exposure: 1.0,
        floor_grid: false,
        show_labels: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    view.camera.position = rotation * Vec3::new(0.0, 6.0, 8.0);
    view.camera.target = target;
    (scene, values, view, target)
}

fn floor_level(
    renderer: &mut Renderer,
    scene: &Scene,
    values: &SceneValues,
    view: &ViewConfiguration,
    target: Vec3,
    label: &str,
) -> f32 {
    let image = renderer
        .capture(scene, values, view, &crate::Overlay::default(), 0.0)
        .expect("curtain regression capture");
    assert!(
        renderer.shaded_frame().lights.last().unwrap().shadow[0] < 0.0,
        "the measured lamp must remain outside the shadow-map budget"
    );
    if let Some(directory) = std::env::var_os("VIZ_OPTICS_CAPTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let mut ppm = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
        for pixel in image.rgba.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(directory.join(format!("curtain-{label}.ppm")), ppm).unwrap();
    }
    let camera =
        crate::ResolvedCamera::resolve(&view.camera, view.mode, 1.0, scene.framing_bounds());
    let (x, y) = camera
        .project(target, EDGE as f32, EDGE as f32)
        .expect("floor receiver is visible");
    let mut total = 0_u32;
    for dy in -3..=3 {
        for dx in -3..=3 {
            let px = (x.round() as i32 + dx).clamp(0, EDGE as i32 - 1) as usize;
            let py = (y.round() as i32 + dy).clamp(0, EDGE as i32 - 1) as usize;
            let pixel = &image.rgba[(py * EDGE as usize + px) * 4..][..3];
            total += pixel.iter().map(|value| u32::from(*value)).sum::<u32>();
        }
    }
    total as f32 / (49.0 * 3.0)
}

#[test]
fn rear_lamp_outside_shadow_budget_is_blocked_in_every_quality() {
    let Ok(mut renderer) = Renderer::headless(EDGE, EDGE) else {
        eprintln!("native GPU unavailable; curtain pixel check skipped");
        return;
    };
    let (mut scene, values, mut view, target) = rig(false, 0.0);
    for quality in TIERS {
        view.quality = quality;
        let blocked = floor_level(
            &mut renderer,
            &scene,
            &values,
            &view,
            target,
            &format!("rear-{quality:?}"),
        );
        let curtain = scene.scenery.pop().unwrap();
        scene.revision += 1;
        let open = floor_level(
            &mut renderer,
            &scene,
            &values,
            &view,
            target,
            &format!("open-{quality:?}"),
        );
        scene.scenery.push(curtain);
        scene.revision += 1;
        assert!(open > 12.0, "{quality:?}: positive control is dark: {open}");
        assert!(
            blocked < open * 0.12 + 1.0,
            "{quality:?}: rear light crossed opaque curtain: blocked={blocked}, open={open}"
        );
    }
}

#[test]
fn curtain_keeps_front_light_on_the_same_side() {
    let Ok(mut renderer) = Renderer::headless(EDGE, EDGE) else {
        eprintln!("native GPU unavailable; curtain front-light check skipped");
        return;
    };
    let (mut scene, values, mut view, target) = rig(true, 0.0);
    for quality in TIERS {
        view.quality = quality;
        let lit = floor_level(
            &mut renderer,
            &scene,
            &values,
            &view,
            target,
            &format!("front-{quality:?}"),
        );
        let curtain = scene.scenery.pop().unwrap();
        scene.revision += 1;
        let open = floor_level(
            &mut renderer,
            &scene,
            &values,
            &view,
            target,
            &format!("front-open-{quality:?}"),
        );
        scene.scenery.push(curtain);
        scene.revision += 1;
        assert!(open > 12.0, "front-light positive control is dark: {open}");
        assert!(
            lit > open * 0.8,
            "{quality:?}: curtain incorrectly blocked same-side light: lit={lit}, open={open}"
        );
    }
}

#[test]
fn rotated_curtain_blocks_its_face_but_leaves_the_opening_beside_it() {
    let Ok(mut renderer) = Renderer::headless(EDGE, EDGE) else {
        eprintln!("native GPU unavailable; rotated curtain check skipped");
        return;
    };
    let (mut scene, values, mut view, target) = rig(false, 90.0);
    view.quality = RenderQuality::Draft;
    let blocked = floor_level(
        &mut renderer,
        &scene,
        &values,
        &view,
        target,
        "rotated-blocked",
    );
    let curtain = scene.scenery.last_mut().unwrap();
    curtain.position += Quat::from_rotation_y(90_f32.to_radians()) * Vec3::new(6.0, 0.0, 0.0);
    scene.revision += 1;
    let opening = floor_level(
        &mut renderer,
        &scene,
        &values,
        &view,
        target,
        "rotated-opening",
    );
    assert!(
        opening > 12.0,
        "finite curtain blocks the opening beside it: {opening}"
    );
    assert!(
        blocked < opening * 0.12 + 1.0,
        "rotated opaque curtain leaked: blocked={blocked}, opening={opening}"
    );
}

/// The reported symptom is light on the camera-facing folds themselves. An off-axis rear lamp
/// can have a positive N dot L on a visible fold flank; backface rejection alone cannot fix it.
#[test]
fn off_axis_rear_lamp_cannot_illuminate_the_front_of_opaque_folds() {
    let Ok(mut renderer) = Renderer::headless(EDGE, EDGE) else {
        eprintln!("native GPU unavailable; curtain face check skipped");
        return;
    };
    let (mut scene, mut values, mut view, _) = rig(false, 0.0);
    scene.scenery.last_mut().unwrap().colour = [0.4; 3];
    let centre = Vec3::new(0.0, 2.0, 0.0);
    view.camera.position = centre + Vec3::new(0.0, 0.0, 8.0);
    view.camera.target = centre;
    for quality in TIERS {
        view.quality = quality;
        let mut levels = [0.0; 3];
        for (sample, source_z) in [-3.0, -3.0, 3.0].into_iter().enumerate() {
            let source = Vec3::new(-4.0, 2.0, source_z);
            scene.fixtures[11].position = source;
            let (x, y, z) = Quat::from_rotation_arc(Vec3::NEG_Y, (centre - source).normalize())
                .to_euler(EulerRot::XYZ);
            scene.emitters[11].local_orientation_degrees =
                Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees());
            values.emitters[11].intensity = if sample == 0 { 0.0 } else { 0.5 };
            scene.revision += 1;
            let image = renderer
                .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
                .unwrap();
            assert!(renderer.shaded_frame().lights.last().unwrap().shadow[0] < 0.0);
            let camera = crate::ResolvedCamera::resolve(
                &view.camera,
                view.mode,
                1.0,
                scene.framing_bounds(),
            );
            // Inset rectangle lies on the cloth, with no floor, edges or lamp apertures. Taking
            // the whole rectangle includes fold flanks rather than just their frontal crowns.
            let (left, bottom) = camera
                .project(Vec3::new(-1.6, 1.0, 0.0), EDGE as f32, EDGE as f32)
                .unwrap();
            let (right, top) = camera
                .project(Vec3::new(1.6, 3.0, 0.0), EDGE as f32, EDGE as f32)
                .unwrap();
            let mut total = 0_u64;
            let mut count = 0_u64;
            for py in top.ceil() as usize..bottom.floor() as usize {
                for px in left.ceil() as usize..right.floor() as usize {
                    total += u64::from(image.rgba[(py * EDGE as usize + px) * 4]);
                    count += 1;
                }
            }
            levels[sample] = total as f32 / count as f32;
        }
        let [dark, rear, front] = levels;
        assert!(
            front > dark + 5.0,
            "{quality:?}: front lamp did not illuminate cloth: {levels:?}"
        );
        assert!(
            rear < dark + (front - dark) * 0.12 + 1.0,
            "{quality:?}: rear lamp illuminated front-facing fold flanks: dark/rear/front={levels:?}"
        );
    }
}
