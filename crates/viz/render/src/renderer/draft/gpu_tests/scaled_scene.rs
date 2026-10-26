//! Native image acceptance for the production reduced-resolution scene and physical-size final image.
use super::*;

fn add_media(renderer: &mut Renderer, scene: &mut Scene) {
    let source = uuid::Uuid::new_v4();
    let edge = crate::media::EDGE;
    let rgba = (0..edge)
        .flat_map(|y| {
            (0..edge).flat_map(move |x| match (x < edge / 2, y < edge / 2) {
                (true, true) => [230, 10, 10, 255],
                (false, true) => [10, 230, 10, 255],
                (true, false) => [10, 10, 230, 255],
                (false, false) => [230, 230, 10, 255],
            })
        })
        .collect();
    renderer
        .update_media_frame(&crate::media::MediaFrame {
            source_id: source,
            sequence: 1,
            width: edge,
            height: edge,
            rgba,
            persistent: true,
        })
        .unwrap();
    scene.media_sections.push(MediaSection {
        id: uuid::Uuid::new_v4(),
        surface_id: uuid::Uuid::new_v4(),
        name: "Scaled scene asymmetric media witness".into(),
        source_id: Some(source),
        fallback_source_id: None,
        position: Vec3::new(2.4, 2.0, 1.4),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(1.6, 1.2, 0.04),
        crop: MediaCrop {
            left: 0.1,
            top: 0.05,
            width: 0.8,
            height: 0.7,
        },
        kind: MediaSectionKind::Led {
            columns: 1,
            rows: 1,
            occupied_cells: vec![0],
            module_size: [1.6, 1.2],
            module_gap: [0.0, 0.0],
            pixel_pitch_millimetres: 3.9,
        },
    });
}
fn assert_media(image: &CapturedImage, scene: &Scene, view: &ViewConfiguration) {
    for (x, y, expected) in [
        (-0.4, 0.3, [true, false, false]),
        (0.4, 0.3, [false, true, false]),
        (-0.4, -0.3, [false, false, true]),
        (0.4, -0.3, [true, true, false]),
    ] {
        let rgb = receiver(image, scene, view, Vec3::new(2.4 + x, 2.0 + y, 1.43));
        for channel in 0..3 {
            assert!(
                if expected[channel] {
                    rgb[channel] > 60.0
                } else {
                    rgb[channel] < 40.0
                },
                "media corner/crop or viewport changed at ({x},{y}): {rgb:?}"
            );
        }
    }
}

#[test]
#[ignore = "requires explicitly serialized native GPU scaled Draft media/depth/laser acceptance"]
fn scaled_draft_preserves_output_depth_media_lasers_and_odd_resize() {
    let mut renderer = Renderer::headless(513, 289).expect("requested native GPU required");
    let (mut scene, mut values, mut view) = rig();
    scene.scenery.clear();
    values.atmosphere.density = 0.5;
    view.camera.position = Vec3::new(0.0, 2.0, 6.0);
    view.camera.target = Vec3::new(0.0, 2.0, 0.0);
    let open = capture(&mut renderer, &scene, &values, &view);
    assert!(receiver(&open, &scene, &view, view.camera.target)[0] > 10.0);
    let wall = SceneryObject {
        kind: SceneryKind::Wall,
        position: Vec3::new(0.0, 2.0, 1.1),
        size: Vec3::new(1.2, 5.0, 0.1),
        colour: [0.0; 3],
        roughness: 1.0,
        ..Default::default()
    };
    add_media(&mut renderer, &mut scene);
    add_visible_laser(&mut scene, &mut values);
    scene.recompute_bounds();
    for (width, height) in [(513, 289), (641, 361), (321, 181)] {
        renderer.resize(width, height);
        values.emitters.last_mut().unwrap().intensity = 0.0;
        scene.scenery.push(wall.clone());
        scene.recompute_bounds();
        let baseline = capture(&mut renderer, &scene, &values, &view);
        assert_eq!((baseline.width, baseline.height), (width, height));
        assert_eq!(renderer.stats().render_scale, 0.25);
        let scaled = (
            ((width as f32) * 0.25).round() as u32,
            ((height as f32) * 0.25).round() as u32,
        );
        assert_eq!(
            renderer.draft_target_sizes_for_test(),
            (scaled, (scaled.0.div_ceil(4), scaled.1.div_ceil(4)))
        );
        assert!(!renderer.draft_composite_state_for_test().0);
        assert!(
            receiver(&baseline, &scene, &view, view.camera.target)
                .iter()
                .all(|value| *value < 3.0),
            "shafts leaked through the foreground wall"
        );
        assert_media(&baseline, &scene, &view);
        // The foreground wall deliberately hides the laser path too. Remove it for the
        // positive laser witness; compare against another open frame with identical media.
        scene.scenery.clear();
        scene.recompute_bounds();
        let baseline = capture(&mut renderer, &scene, &values, &view);
        values.emitters.last_mut().unwrap().intensity = 1.0;
        let laser = capture(&mut renderer, &scene, &values, &view);
        let (fused, count) = renderer.draft_composite_state_for_test();
        assert!(!fused && count > 0);
        assert!(
            laser
                .rgba
                .chunks_exact(4)
                .zip(baseline.rgba.chunks_exact(4))
                .any(|(a, b)| i16::from(a[1]) - i16::from(b[1]) > 8),
            "scaled scene lost visible laser energy"
        );
    }
}
