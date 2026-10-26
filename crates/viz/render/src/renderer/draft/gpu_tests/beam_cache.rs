//! Real GPU acceptance for exact cached versus freshly encoded beams. Never run automatically.
use super::*;
fn fresh_equal(
    renderer: &mut Renderer,
    scene: &Scene,
    values: &SceneValues,
    view: &ViewConfiguration,
    cached: &CapturedImage,
) {
    renderer.force_draft_beam_fresh_for_test(true);
    let fresh = capture(renderer, scene, values, view);
    assert!(renderer.stats().draft_beam_cache_miss);
    assert_eq!(cached.width, fresh.width);
    assert_eq!(cached.height, fresh.height);
    assert_eq!(
        cached.rgba, fresh.rgba,
        "cached attachment must reproduce freshly encoded pixels"
    );
    renderer.force_draft_beam_fresh_for_test(false);
}
fn stable(
    renderer: &mut Renderer,
    scene: &Scene,
    values: &SceneValues,
    view: &ViewConfiguration,
) -> CapturedImage {
    let image = capture(renderer, scene, values, view);
    assert!(
        renderer.stats().draft_beam_cache_hit,
        "stable dependencies must reuse"
    );
    image
}
fn miss_then_equal(
    renderer: &mut Renderer,
    scene: &Scene,
    values: &SceneValues,
    view: &ViewConfiguration,
) {
    let changed = capture(renderer, scene, values, view);
    assert!(
        renderer.stats().draft_beam_cache_miss,
        "changed dependency must rerender"
    );
    let hit = stable(renderer, scene, values, view);
    assert_eq!(changed.rgba, hit.rgba);
    fresh_equal(renderer, scene, values, view, &hit);
}
#[test]
#[ignore = "requires explicitly serialized native GPU cache/media/laser acceptance"]
fn draft_beam_cache_reuses_media_and_laser_only_frames_with_exact_fresh_pixels() {
    let mut renderer = Renderer::headless(513, 289).expect("actual native GPU required");
    let (mut scene, mut values, view) = rig();
    values.atmosphere.density = 0.25;
    add_visible_laser(&mut scene, &mut values);
    let source = uuid::Uuid::new_v4();
    scene.media_sections.push(MediaSection {
        id: uuid::Uuid::new_v4(),
        surface_id: uuid::Uuid::new_v4(),
        name: "Cache live panel".into(),
        source_id: Some(source),
        fallback_source_id: None,
        position: Vec3::new(-2.0, 2.0, 0.0),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(1.5, 1.0, 0.04),
        crop: MediaCrop {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        },
        kind: MediaSectionKind::Tv {
            bezel_metres: 0.01,
            spill: 0.0,
        },
    });
    scene.recompute_bounds();
    let upload = |renderer: &mut Renderer, sequence, colour: [u8; 4]| {
        renderer
            .update_media_frame(&crate::media::MediaFrame {
                source_id: source,
                sequence,
                width: crate::media::EDGE,
                height: crate::media::EDGE,
                rgba: colour.repeat((crate::media::EDGE * crate::media::EDGE) as usize),
                persistent: true,
            })
            .unwrap();
    };
    upload(&mut renderer, 1, [230, 10, 10, 255]);
    capture(&mut renderer, &scene, &values, &view);
    renderer.force_draft_legacy_composite_for_test(true);
    capture(&mut renderer, &scene, &values, &view);
    let before = stable(&mut renderer, &scene, &values, &view);
    assert!(
        before
            .rgba
            .chunks_exact(4)
            .filter(|p| p[0].max(p[1]).max(p[2]) > 10)
            .count()
            > 100,
        "nonempty real image"
    );
    upload(&mut renderer, 2, [10, 10, 230, 255]);
    let media = stable(&mut renderer, &scene, &values, &view);
    assert_ne!(
        before.rgba, media.rgba,
        "current live pixels must still change"
    );
    fresh_equal(&mut renderer, &scene, &values, &view, &media);
    let clock = renderer
        .capture(&scene, &values, &view, &crate::Overlay::default(), 17.0)
        .unwrap();
    assert!(renderer.stats().draft_beam_cache_hit);
    assert_eq!(
        media.rgba, clock.rgba,
        "uniform Draft ignores only the unrelated wall clock"
    );
    for point in &mut values.laser_scans[1].points {
        point.colour = [0.0, 0.0, 1.0];
        point.y = 0.15;
    }
    let laser = stable(&mut renderer, &scene, &values, &view);
    assert_ne!(
        media.rgba, laser.rgba,
        "current scan path must still change"
    );
    fresh_equal(&mut renderer, &scene, &values, &view, &laser);
}
#[test]
#[ignore = "requires explicitly serialized native GPU cache invalidation acceptance"]
fn draft_beam_cache_invalidates_camera_light_depth_curtains_assets_resize_and_quality() {
    let mut renderer = Renderer::headless(513, 289).expect("actual native GPU required");
    let (mut scene, mut values, mut view) = rig();
    values.atmosphere.density = 0.25;
    capture(&mut renderer, &scene, &values, &view);
    renderer.force_draft_legacy_composite_for_test(true);
    capture(&mut renderer, &scene, &values, &view);
    stable(&mut renderer, &scene, &values, &view);
    view.camera.position.x += 0.1;
    miss_then_equal(&mut renderer, &scene, &values, &view);
    values.emitters[0].intensity = 0.55;
    miss_then_equal(&mut renderer, &scene, &values, &view);
    values.emitters[0].colour = [1.0, 0.2, 0.1];
    miss_then_equal(&mut renderer, &scene, &values, &view);
    scene.scenery[0].position.y += 0.2;
    miss_then_equal(&mut renderer, &scene, &values, &view);
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Curtain,
        position: Vec3::new(0.0, 2.0, -0.5),
        size: Vec3::new(3.0, 3.0, 0.04),
        colour: [0.02; 3],
        ..Default::default()
    });
    miss_then_equal(&mut renderer, &scene, &values, &view);
    scene.scenery.last_mut().unwrap().position.x += 0.5;
    miss_then_equal(&mut renderer, &scene, &values, &view);
    scene.models.push(
        viz_scene::read_glb(include_bytes!(
            "../../../../../../../assets/models/lamps/led-par-pizza.glb"
        ))
        .unwrap(),
    );
    scene.fixtures[0].model = Some(0);
    miss_then_equal(&mut renderer, &scene, &values, &view);
    // Same slot and scene revision, different actual uploaded geometry resource.
    scene.models[0].parts[0].positions[0][0] += 0.01;
    miss_then_equal(&mut renderer, &scene, &values, &view);
    let (_, beam) = renderer.draft_target_sizes_for_test();
    renderer.set_draft_beam_size_for_test(beam);
    miss_then_equal(&mut renderer, &scene, &values, &view);
    renderer.resize(641, 361);
    miss_then_equal(&mut renderer, &scene, &values, &view);
    view.quality = RenderQuality::Ultra;
    capture(&mut renderer, &scene, &values, &view);
    assert!(!renderer.stats().draft_beam_cache_hit && !renderer.stats().draft_beam_cache_miss);
    view.quality = RenderQuality::Draft;
    miss_then_equal(&mut renderer, &scene, &values, &view);
}
