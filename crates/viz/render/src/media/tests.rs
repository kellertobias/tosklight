use super::*;
use std::sync::Arc;

fn frame(source_id: Uuid, sequence: u64, pixel: u8) -> MediaFrame {
    MediaFrame {
        source_id,
        sequence,
        width: EDGE,
        height: EDGE,
        rgba: vec![pixel; (EDGE * EDGE * 4) as usize],
        persistent: false,
    }
}

#[test]
fn live_frames_hold_for_two_seconds_while_fallbacks_never_expire() {
    let now = Instant::now();
    assert!(available(false, Some(now - LAST_GOOD_HOLD), now));
    assert!(!available(
        false,
        Some(now - LAST_GOOD_HOLD - Duration::from_millis(1)),
        now
    ));
    assert!(available(true, Some(now - Duration::from_secs(60)), now));
}

#[test]
fn sampled_average_drives_projector_colour_without_per_pixel_gpu_lights() {
    let average = average_rgb(&[128, 64, 32, 255].repeat(128));
    assert!((average[0] - 128.0 / 255.0).abs() < 0.001);
    assert!((average[1] - 64.0 / 255.0).abs() < 0.001);
    assert!((average[2] - 32.0 / 255.0).abs() < 0.001);
}

#[test]
fn snapshots_share_adopted_pixels_and_keep_invocation_availability_after_replacement() {
    let id = Uuid::new_v4();
    let now = Instant::now();
    let mut retained = RetainedFrames::default();
    let adopted = retained.prepare(&frame(id, 1, 20)).unwrap().unwrap();
    retained.adopt(Arc::clone(&adopted));
    let updated = HashMap::from([(id, now - LAST_GOOD_HOLD)]);
    let frozen = retained.snapshot(now, &updated, &HashSet::new(), &HashMap::from([(id, 0.25)]));
    assert!(Arc::ptr_eq(&adopted, &frozen.sources()[0].frame));
    assert!(frozen.sources()[0].available);
    assert_eq!(frozen.sources()[0].flicker, 0.25);
    let later = retained.snapshot(
        now + Duration::from_millis(1),
        &updated,
        &HashSet::new(),
        &HashMap::new(),
    );
    assert!(!later.sources()[0].available);
    assert!(
        frozen.sources()[0].available,
        "availability belongs to invocation"
    );
    retained.adopt(retained.prepare(&frame(id, 2, 90)).unwrap().unwrap());
    let latest = retained.snapshot(now, &updated, &HashSet::new(), &HashMap::new());
    assert_eq!(latest.sources()[0].frame.rgba[0], 90);
    assert_eq!(frozen.sources()[0].frame.rgba[0], 20);
    assert_eq!(frozen.sources()[0].frame.sequence, 1);
}

#[test]
fn invalid_duplicate_and_over_budget_frames_do_not_replace_retained_ownership() {
    let mut retained = RetainedFrames::default();
    let id = Uuid::new_v4();
    let original = frame(id, 1, 10);
    let adopted = retained.prepare(&original).unwrap().unwrap();
    retained.adopt(Arc::clone(&adopted));
    assert!(retained.prepare(&frame(id, 1, 99)).unwrap().is_none());
    let mut malformed = frame(id, 2, 99);
    malformed.rgba.clear();
    assert!(retained.prepare(&malformed).is_err());
    for sequence in 1..MAX_SOURCES - 1 {
        retained.adopt(
            retained
                .prepare(&frame(Uuid::new_v4(), u64::from(sequence), 3))
                .unwrap()
                .unwrap(),
        );
    }
    assert!(retained.prepare(&frame(Uuid::new_v4(), 1, 1)).is_err());
    let frozen = retained.snapshot(
        Instant::now(),
        &HashMap::new(),
        &HashSet::new(),
        &HashMap::new(),
    );
    assert_eq!(frozen.sources().len(), (MAX_SOURCES - 1) as usize);
    let preserved = frozen
        .sources()
        .iter()
        .find(|s| s.frame.source_id == id)
        .unwrap();
    assert!(Arc::ptr_eq(&adopted, &preserved.frame));
    assert!(
        retained.prepare(&frame(id, 2, 11)).unwrap().is_some(),
        "existing source can update at budget"
    );
}

#[test]
#[ignore = "requires a real GPU; run explicitly with --ignored --test-threads=1"]
fn frozen_media_restore_preserves_source_fallback_expiry_pixels_and_flicker() {
    let gpu = crate::gpu::Gpu::headless(8, 8).expect("GPU required for atlas restore");
    let mut live = MediaAtlas::new(&gpu.device, &gpu.queue);
    let source = Uuid::new_v4();
    let fallback = Uuid::new_v4();
    let fresh = Uuid::new_v4();
    live.update(&gpu.queue, &frame(source, 1, 20)).unwrap();
    live.update(&gpu.queue, &frame(fallback, 1, 50)).unwrap();
    live.update(&gpu.queue, &frame(fresh, 1, 80)).unwrap();
    live.update(&gpu.queue, &frame(fresh, 2, 100)).unwrap();
    live.updated_at.remove(&source);
    live.updated_at.remove(&fallback);
    let frozen = live.snapshot();
    let expected = live.appearance(Some(source), Some(fallback));
    let expected_flicker = live.appearance(Some(fresh), None).1;
    live.update(&gpu.queue, &frame(source, 2, 200)).unwrap();
    let mut restored = MediaAtlas::new(&gpu.device, &gpu.queue);
    frozen.restore_into(&mut restored, &gpu.queue).unwrap();
    assert_eq!(restored.appearance(Some(source), Some(fallback)), expected);
    assert_eq!(restored.layer_with_fallback(Some(source), None), 0);
    assert_ne!(
        restored.layer_with_fallback(None, Some(source)),
        0,
        "expired source pixels still exist for unconditional fallback lookup"
    );
    assert_eq!(restored.appearance(Some(fresh), None).1, expected_flicker);
    assert!(restored.persistent.contains(&fresh));
    let long_after = Instant::now() + Duration::from_secs(60);
    assert!(available(
        restored.persistent.contains(&fresh),
        restored.updated_at.get(&fresh).copied(),
        long_after
    ));
    assert!(!restored.persistent.contains(&source));
    assert!(!restored.updated_at.contains_key(&source));
    let restored_snapshot = restored.snapshot();
    let source_frame = restored_snapshot
        .sources()
        .iter()
        .find(|s| s.frame.source_id == source)
        .unwrap();
    assert_eq!(source_frame.frame.rgba[0], 20);
    assert!(
        !source_frame.frame.persistent,
        "restore does not mutate original frame metadata"
    );
    assert!(frozen.restore_into(&mut restored, &gpu.queue).is_err());
    assert!(
        live.retained
            .prepare(&frame(source, 2, 200))
            .unwrap()
            .is_none()
    );
}

#[test]
fn immutable_show_fallbacks_freeze_without_gpu_and_do_not_replace_adopted_pixels() {
    let id = Uuid::new_v4();
    let fallback = Uuid::new_v4();
    let mut scene = viz_scene::Scene::default();
    scene.media_sections.push(viz_scene::MediaSection {
        id: Uuid::new_v4(),
        surface_id: Uuid::new_v4(),
        name: "wall".into(),
        source_id: Some(id),
        fallback_source_id: Some(fallback),
        position: glam::Vec3::ZERO,
        rotation_degrees: glam::Vec3::ZERO,
        size: glam::Vec3::ONE,
        crop: viz_scene::MediaCrop {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        },
        kind: viz_scene::MediaSectionKind::Tv {
            bezel_metres: 0.01,
            spill: 0.0,
        },
    });
    for source_id in [id, fallback] {
        scene.media_sources.push(viz_scene::MediaSourceBinding {
            id: source_id,
            server_id: Uuid::nil(),
            host: String::new(),
            port: 0,
            advertised_source_id: 0,
            name: source_id.to_string(),
            aspect_ratio: None,
            fallback_rgba: Some(vec![40; (EDGE * EDGE * 4) as usize]),
        });
    }
    let now = Instant::now();
    let mut retained = RetainedFrames::default();
    let adopted = retained.prepare(&frame(id, 17, 90)).unwrap().unwrap();
    retained.adopt(Arc::clone(&adopted));
    let frozen = retained
        .snapshot(
            now,
            &HashMap::from([(id, now)]),
            &HashSet::new(),
            &HashMap::new(),
        )
        .with_scene_fallbacks(&scene)
        .unwrap();
    assert_eq!(frozen.sources().len(), 2);
    let actual = frozen
        .sources()
        .iter()
        .find(|s| s.frame.source_id == id)
        .unwrap();
    assert!(Arc::ptr_eq(&actual.frame, &adopted));
    assert_eq!(actual.frame.sequence, 17);
    scene.media_sources[1].fallback_rgba.as_mut().unwrap()[0] = 12;
    assert_eq!(
        frozen
            .sources()
            .iter()
            .find(|s| s.frame.source_id == fallback)
            .unwrap()
            .frame
            .rgba[0],
        40
    );
    scene.media_sources[1]
        .fallback_rgba
        .as_mut()
        .unwrap()
        .clear();
    assert!(FrozenMedia::default().with_scene_fallbacks(&scene).is_err());
}
