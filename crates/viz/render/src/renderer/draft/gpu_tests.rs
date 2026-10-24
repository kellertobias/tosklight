//! Explicitly requested native-GPU pixel acceptance; ignored in ordinary CPU-only runs.
use super::super::{CapturedImage, FrameContext, Renderer};
use glam::{EulerRot, Quat, Vec3};
use viz_scene::*;

fn rig() -> (Scene, SceneValues, ViewConfiguration) {
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
        position: Vec3::new(0.0, 4.0, 0.0),
        installed_colour: [1.0; 3],
        body: FixtureBody {
            kind: BodyKind::Lantern,
            size: Vec3::splat(0.002),
        },
        ..Default::default()
    });
    scene.emitters.push(EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Draft acceptance source".into(),
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
    let mut values = SceneValues {
        emitters: vec![EmitterValues {
            intensity: 0.8,
            colour: [1.0; 3],
            ..Default::default()
        }],
        ..Default::default()
    };
    values.atmosphere.density = 0.0;
    let mut view = ViewConfiguration {
        quality: RenderQuality::Draft,
        ambient: 0.0,
        floor_grid: false,
        show_labels: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    view.camera.position = Vec3::new(0.0, 5.0, 6.0);
    view.camera.target = Vec3::ZERO;
    (scene, values, view)
}
fn capture(
    renderer: &mut Renderer,
    scene: &Scene,
    values: &SceneValues,
    view: &ViewConfiguration,
) -> CapturedImage {
    renderer
        .capture(scene, values, view, &crate::Overlay::default(), 0.0)
        .expect("requested Draft GPU capture")
}
fn camera(scene: &Scene, view: &ViewConfiguration, image: &CapturedImage) -> crate::ResolvedCamera {
    crate::ResolvedCamera::resolve(
        &view.camera,
        view.mode,
        image.width as f32 / image.height as f32,
        scene.framing_bounds(),
    )
}
fn pixel_patch(image: &CapturedImage, x: f32, y: f32, radius: i32) -> [f32; 3] {
    let mut sum = [0.0; 3];
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let px = (x.round() as i32 + dx).clamp(0, image.width as i32 - 1) as usize;
            let py = (y.round() as i32 + dy).clamp(0, image.height as i32 - 1) as usize;
            for (channel, total) in sum.iter_mut().enumerate() {
                *total += f32::from(image.rgba[(py * image.width as usize + px) * 4 + channel]);
            }
        }
    }
    sum.map(|value| value / (radius * 2 + 1).pow(2) as f32)
}
fn receiver(
    image: &CapturedImage,
    scene: &Scene,
    view: &ViewConfiguration,
    target: Vec3,
) -> [f32; 3] {
    let (x, y) = camera(scene, view, image)
        .project(target, image.width as f32, image.height as f32)
        .expect("acceptance receiver visible");
    pixel_patch(image, x, y, 2)
}

#[test]
#[ignore = "requires explicitly serialized native GPU acceptance"]
fn actual_par_odd_physical_geometry_and_backend_switches_preserve_hq() {
    let mut renderer = Renderer::headless(513, 289).expect("requested native GPU required");
    let (mut scene, values, mut view) = rig();
    scene.models.push(
        viz_scene::read_glb(include_bytes!(
            "../../../../../../assets/models/lamps/led-par-pizza.glb"
        ))
        .unwrap(),
    );
    scene.fixtures[0].model = Some(0);
    view.ambient = 0.25;
    view.camera.position = Vec3::new(0.65, 4.35, 0.85);
    view.camera.target = scene.fixtures[0].position;
    let draft_a = capture(&mut renderer, &scene, &values, &view);
    assert_eq!(
        (draft_a.width, draft_a.height, renderer.samples()),
        (513, 289, 1)
    );
    assert_eq!(renderer.stats().render_scale, 1.0);
    assert!(
        draft_a
            .rgba
            .chunks_exact(4)
            .filter(|pixel| pixel[..3].iter().any(|value| *value > 20))
            .count()
            > 100,
        "actual GLB fixture must remain visible"
    );
    view.quality = RenderQuality::High;
    let high_a = capture(&mut renderer, &scene, &values, &view);
    view.quality = RenderQuality::Draft;
    let draft_b = capture(&mut renderer, &scene, &values, &view);
    assert_eq!(
        draft_a.rgba, draft_b.rgba,
        "returning from High must preserve Draft pixels"
    );
    view.quality = RenderQuality::High;
    let high_b = capture(&mut renderer, &scene, &values, &view);
    assert_eq!(
        high_a.rgba, high_b.rgba,
        "Draft must not mutate the regular High backend"
    );
    let device = renderer.device();
    let target = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Draft target metadata acceptance"),
            size: wgpu::Extent3d {
                width: 513,
                height: 289,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: renderer.output_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());
    let context = FrameContext {
        device,
        queue: renderer.queue(),
        size: (513, 289),
        format: renderer.output_format(),
        output: &target,
        timer: None,
    };
    let media = crate::media::MediaAtlas::new(device, renderer.queue());
    let backend = super::DraftRenderer::new(&context, &media);
    let geometry = backend.scene.targets.hdr.texture().size();
    let beams = backend.scene.draft.as_ref().unwrap().beam.texture().size();
    assert_eq!(
        (geometry.width, geometry.height, backend.scene.samples),
        (513, 289, 1)
    );
    assert_eq!((beams.width, beams.height), (257, 145));
    assert_eq!(
        backend.scene.targets.bloom_a.texture(),
        backend.scene.targets.shaded.texture()
    );
    assert_eq!(
        backend.scene.targets.bloom_b.texture(),
        backend.scene.targets.shaded.texture()
    );
}

/// Brighter upward sources consume both atlas slots without contributing to the measured shaft.
fn add_shadow_budget_decoys(scene: &mut Scene, values: &mut SceneValues) {
    let emitter = scene.emitters[0].clone();
    for index in 0..2 {
        scene.fixtures.push(FixtureInstance {
            position: Vec3::new(-20.0 + index as f32, 20.0, -20.0),
            installed_colour: [1.0; 3],
            body: FixtureBody {
                kind: BodyKind::Lantern,
                size: Vec3::splat(0.002),
            },
            ..Default::default()
        });
        let mut decoy = emitter.clone();
        decoy.fixture_index = (scene.fixtures.len() - 1) as u32;
        decoy.local_orientation_degrees = Vec3::new(180.0, 0.0, 0.0);
        scene.emitters.push(decoy);
        values.emitters.push(EmitterValues {
            intensity: 1.0,
            colour: [1.0; 3],
            ..Default::default()
        });
    }
}
#[test]
#[ignore = "requires explicitly serialized native GPU acceptance"]
fn uniform_shaft_rgb_intensity_and_inside_camera_remain_visible() {
    let mut renderer = Renderer::headless(257, 257).expect("requested native GPU required");
    let (mut scene, mut values, mut view) = rig();
    scene.scenery.clear();
    add_shadow_budget_decoys(&mut scene, &mut values);
    scene.recompute_bounds();
    values.atmosphere.density = 0.35;
    view.camera.position = Vec3::new(1.4, 2.0, 0.0);
    view.camera.target = Vec3::new(-1.0, 2.0, 0.0);
    let mut levels = Vec::new();
    for intensity in [0.15, 0.4, 0.8] {
        values.emitters[0].intensity = intensity;
        let image = capture(&mut renderer, &scene, &values, &view);
        assert!(
            renderer.shaded_frame().lights[0].shadow[0] < 0.0,
            "measured shaft must exercise analytic path"
        );
        levels.push(receiver(&image, &scene, &view, view.camera.target)[0]);
    }
    assert!(
        levels[0] > 1.0 && levels[1] > levels[0] + 1.0 && levels[2] > levels[1] + 1.0,
        "intensity must remain monotonic: {levels:?}"
    );
    for channel in 0..3 {
        values.emitters[0].colour = [0.0; 3];
        values.emitters[0].colour[channel] = 1.0;
        let image = capture(&mut renderer, &scene, &values, &view);
        let rgb = receiver(&image, &scene, &view, view.camera.target);
        assert!(
            rgb[channel] > 2.0
                && (0..3)
                    .filter(|other| *other != channel)
                    .all(|other| rgb[other] < rgb[channel] * 0.15 + 1.0),
            "shaft RGB must follow source: {rgb:?}"
        );
    }
    values.emitters[0].colour = [1.0; 3];
    view.camera.position = Vec3::new(0.0, 2.0, 0.0);
    let image = capture(&mut renderer, &scene, &values, &view);
    assert!(
        receiver(&image, &scene, &view, view.camera.target)[0] > 2.0,
        "inside camera must retain remaining lit shaft"
    );
}

#[test]
#[ignore = "requires explicitly serialized native GPU acceptance"]
fn off_camera_opaque_box_shadow_moves_and_restores_without_revision_change() {
    let mut renderer = Renderer::headless(257, 257).expect("requested native GPU required");
    let (mut scene, values, mut view) = rig();
    view.camera.position = Vec3::new(0.0, 0.6, 5.0);
    view.camera.target = Vec3::ZERO;
    view.camera.fov_degrees = 20.0;
    let open = capture(&mut renderer, &scene, &values, &view);
    let positive = receiver(&open, &scene, &view, Vec3::ZERO)[0];
    assert!(positive > 20.0, "receiver positive control: {positive}");
    let mut high_view = view.clone();
    high_view.quality = RenderQuality::High;
    let high_open = capture(&mut renderer, &scene, &values, &high_view);
    let high_positive = receiver(&high_open, &scene, &high_view, Vec3::ZERO)[0];
    assert!(
        high_positive > 20.0,
        "High receiver positive control: {high_positive}"
    );
    let box_position = Vec3::new(0.0, 2.0, 0.0);
    assert!(
        camera(&scene, &view, &open)
            .project(box_position, 257.0, 257.0)
            .is_none(),
        "caster must be outside camera view"
    );
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Prop,
        position: box_position,
        size: Vec3::new(1.5, 0.3, 1.5),
        colour: [0.0; 3],
        roughness: 1.0,
        ..Default::default()
    });
    let revision = scene.revision;
    let high_blocked = capture(&mut renderer, &scene, &values, &high_view);
    let high_dark = receiver(&high_blocked, &scene, &high_view, Vec3::ZERO)[0];
    assert!(
        high_dark < high_positive * 0.2 + 2.0,
        "regular High physically confirms the same off-camera caster: {high_dark} vs {high_positive}"
    );
    let blocked = capture(&mut renderer, &scene, &values, &view);
    let dark = receiver(&blocked, &scene, &view, Vec3::ZERO)[0];
    assert!(
        dark < positive * 0.2 + 2.0,
        "off-camera geometry must block light: {dark} vs {positive}"
    );
    scene.scenery[1].position.x = 5.0;
    let moved = capture(&mut renderer, &scene, &values, &view);
    assert!((receiver(&moved, &scene, &view, Vec3::ZERO)[0] - positive).abs() < 3.0);
    scene.scenery[1].position = box_position;
    let restored = capture(&mut renderer, &scene, &values, &view);
    assert!((receiver(&restored, &scene, &view, Vec3::ZERO)[0] - dark).abs() < 3.0);
    assert_eq!(
        scene.revision, revision,
        "test must exercise content-based invalidation"
    );
}

#[test]
#[ignore = "requires explicitly serialized native GPU acceptance"]
fn crowded_tiles_and_high_index_colour_sentinel_keep_all_light_energy() {
    let mut renderer = Renderer::headless(257, 257).expect("requested native GPU required");
    let (mut scene, mut values, view) = rig();
    let image = capture(&mut renderer, &scene, &values, &view);
    let baseline = receiver(&image, &scene, &view, Vec3::ZERO)[0];
    assert!(baseline > 20.0);
    let emitter = scene.emitters[0].clone();
    for count in [180, 300] {
        scene.emitters = vec![emitter.clone(); count];
        values.emitters = vec![
            EmitterValues {
                intensity: 0.8 / count as f32,
                colour: [1.0; 3],
                ..Default::default()
            };
            count
        ];
        for _ in 0..3 {
            let image = capture(&mut renderer, &scene, &values, &view);
            assert_eq!(renderer.shaded_frame().lights.len(), count);
            assert!(
                (receiver(&image, &scene, &view, Vec3::ZERO)[0] - baseline).abs() < 3.0,
                "overflow tile must preserve combined energy"
            );
        }
    }
    for value in &mut values.emitters {
        value.intensity = 0.0002;
        value.colour = [1.0, 0.0, 0.0];
    }
    values.emitters[299].intensity = 0.8;
    values.emitters[299].colour = [0.0, 0.0, 1.0];
    let image = capture(&mut renderer, &scene, &values, &view);
    let rgb = receiver(&image, &scene, &view, Vec3::ZERO);
    assert!(
        rgb[2] > 30.0 && rgb[2] > rgb[0] * 1.5,
        "index299 blue source must survive tile overflow: {rgb:?}"
    );
}

#[test]
#[ignore = "requires explicitly serialized native GPU acceptance"]
fn curtain_blocks_rear_lamp_beyond_two_cached_shadow_slots() {
    let mut renderer = Renderer::headless(257, 257).expect("requested native GPU required");
    let (mut scene, mut values, mut view) = rig();
    let target = Vec3::new(0.0, 0.0, 2.0);
    scene.fixtures[0].position = Vec3::new(-3.0, 3.0, -3.0);
    let direction = (target - scene.fixtures[0].position).normalize();
    let (x, y, z) = Quat::from_rotation_arc(Vec3::NEG_Y, direction).to_euler(EulerRot::XYZ);
    scene.emitters[0].local_orientation_degrees =
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees());
    values.emitters[0].intensity = 0.5;
    add_shadow_budget_decoys(&mut scene, &mut values);
    view.camera.position = Vec3::new(0.0, 6.0, 8.0);
    view.camera.target = target;
    let open = capture(&mut renderer, &scene, &values, &view);
    assert!(renderer.shaded_frame().lights[0].shadow[0] < 0.0);
    let positive = receiver(&open, &scene, &view, target)[0];
    assert!(positive > 12.0, "curtain positive control: {positive}");
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Curtain,
        position: Vec3::new(0.0, 2.0, 0.0),
        size: Vec3::new(4.0, 4.0, 0.2),
        colour: [0.015; 3],
        roughness: 0.95,
        ..Default::default()
    });
    let blocked = capture(&mut renderer, &scene, &values, &view);
    let dark = receiver(&blocked, &scene, &view, target)[0];
    assert!(
        dark < positive * 0.15 + 1.0,
        "curtain cannot transmit the unbudgeted lamp: {dark} vs {positive}"
    );
}

#[test]
#[ignore = "requires explicitly serialized native GPU acceptance"]
fn half_resolution_beams_do_not_leak_across_full_resolution_opaque_silhouette() {
    let mut renderer = Renderer::headless(513, 289).expect("requested native GPU required");
    let (mut scene, mut values, mut view) = rig();
    scene.scenery.clear();
    values.atmosphere.density = 0.5;
    view.camera.position = Vec3::new(0.0, 2.0, 6.0);
    view.camera.target = Vec3::new(0.0, 2.0, 0.0);
    let open = capture(&mut renderer, &scene, &values, &view);
    let positive = receiver(&open, &scene, &view, view.camera.target)[0];
    assert!(
        positive > 10.0,
        "background shaft positive control: {positive}"
    );
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Wall,
        position: Vec3::new(0.0, 2.0, 1.1),
        size: Vec3::new(1.2, 5.0, 0.1),
        colour: [0.0; 3],
        roughness: 1.0,
        ..Default::default()
    });
    let blocked = capture(&mut renderer, &scene, &values, &view);
    for x in [-0.55, 0.0, 0.55] {
        let rgb = receiver(&blocked, &scene, &view, Vec3::new(x, 2.0, 1.15));
        assert!(
            rgb.iter().all(|value| *value < 3.0),
            "foreground/inner silhouette cannot receive background shaft: x={x}, {rgb:?}"
        );
    }
    let outside = receiver(&blocked, &scene, &view, Vec3::new(0.85, 2.0, 0.0))[0];
    assert!(
        outside > 3.0,
        "shaft beside opaque silhouette must remain visible: {outside}"
    );
}

#[test]
fn independent_curtain_interval_union_preserves_overlaps_touching_and_input_order() {
    let intervals = [
        (2.0_f64, 4.0_f64),
        (3.0, 5.0),
        (5.0, 6.0),
        (8.0, 9.0),
        (8.2, 8.5),
    ];
    let mut expected = 0.0;
    for sample in 0..10_000 {
        let t = (f64::from(sample) + 0.5) * 0.001;
        if !intervals.iter().any(|(a, b)| t >= *a && t <= *b) {
            expected += 0.001;
        }
    }
    for mut input in [intervals.to_vec(), intervals.into_iter().rev().collect()] {
        input.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cursor: f64 = 0.0;
        let mut visible: f64 = 0.0;
        for (a, b) in input {
            if a > cursor {
                visible += a - cursor;
            }
            cursor = cursor.max(b);
        }
        visible += 10.0 - cursor;
        assert!((visible - expected).abs() < 1e-8);
        assert!(
            (visible - 5.0).abs() < 1e-8,
            "overlap must never subtract light twice"
        );
    }
}
