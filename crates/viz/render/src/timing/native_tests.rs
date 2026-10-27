//! A real GPU exercises mapping reuse and provenance across actual render submissions.
use crate::Renderer;
use viz_scene::{
    RenderQuality, Scene, SceneValues, SceneryKind, SceneryObject, ViewConfiguration, ViewMode,
};

#[test]
fn native_timestamp_readbacks_continue_and_do_not_cross_quality_boundaries() {
    let Ok(mut renderer) = Renderer::headless(64, 64) else {
        eprintln!("GPU unavailable: native timestamp ownership proof not run");
        return;
    };
    let mut scene = Scene {
        revision: 1,
        ..Default::default()
    };
    // Metal may return its counter-error sentinel for empty passes. Exercise real triangles.
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Floor,
        size: glam::Vec3::new(4.0, 0.1, 4.0),
        colour: [0.4; 3],
        ..Default::default()
    });
    scene.recompute_bounds();
    let values = SceneValues::default();
    let mut view = ViewConfiguration {
        mode: ViewMode::Full3d,
        ..Default::default()
    };
    let overlay = crate::Overlay::default();
    let mut ids = std::collections::HashSet::new();
    for frame in 0..8 {
        renderer
            .capture(&scene, &values, &view, &overlay, frame as f32)
            .expect("capture with reusable timing map");
        let stats = renderer.stats();
        if let Some(id) = stats.gpu_sample_id {
            assert!(stats.gpu_sample_matches_frame);
            ids.insert(id);
        }
    }
    if ids.is_empty() {
        eprintln!(
            "GPU has no completed timestamps: timing unsupported/unavailable, not a pass for native timing"
        );
        return;
    }
    assert!(
        ids.len() >= 2,
        "mapping continues over successive completed submissions"
    );
    view.quality = RenderQuality::Draft;
    renderer
        .capture(&scene, &values, &view, &overlay, 9.0)
        .expect("changed quality");
    assert!(
        !renderer.stats().gpu_sample_matches_frame,
        "last HQ timing must not become Draft timing"
    );
    let mut draft_sample = false;
    for frame in 10..30 {
        renderer
            .capture(&scene, &values, &view, &overlay, frame as f32)
            .expect("next Draft frame");
        if renderer.stats().gpu_sample_matches_frame && renderer.stats().gpu_sample_id.is_some() {
            draft_sample = true;
            break;
        }
    }
    assert!(
        draft_sample,
        "a completed Draft sample eventually becomes available without crediting old HQ timing"
    );
    eprintln!(
        "Native GPU timestamp ownership verified: {} distinct samples; quality boundary rejected",
        ids.len()
    );
}

/// Completion still progresses while subsequent actual render submissions omit timing writes.
#[test]
fn native_diagnostic_readback_reuses_after_skipped_frames() {
    let Ok(mut renderer) = Renderer::headless(64, 64) else {
        return;
    };
    renderer.set_gpu_timing_test_cadence(viz_scene::gpu_timing::TimingCadence::OneHz);
    let mut scene = Scene {
        revision: 1,
        ..Default::default()
    };
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Floor,
        size: glam::Vec3::new(4.0, 0.1, 4.0),
        colour: [0.4; 3],
        ..Default::default()
    });
    scene.recompute_bounds();
    let values = SceneValues::default();
    let view = ViewConfiguration {
        mode: ViewMode::Lines3d,
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let mut saw_completed_with_skips = false;
    loop {
        renderer
            .capture(&scene, &values, &view, &crate::Overlay::default(), 0.0)
            .expect("capture through diagnostic admission");
        let Some(coverage) = renderer.gpu_timing_coverage() else {
            return;
        };
        if coverage.valid_samples > 0 && coverage.cadence_skips > 0 {
            saw_completed_with_skips = true;
            assert!(renderer.stats().gpu_sample_matches_frame);
        }
        if coverage.valid_samples >= 2 {
            assert!(saw_completed_with_skips);
            assert!(coverage.frames_considered > coverage.frames_admitted);
            assert!(coverage.elapsed_millis >= 1000);
            eprintln!(
                "Native skipped-frame timestamp reuse {} [{}]: {coverage:?}",
                renderer.adapter_name(),
                renderer.backend()
            );
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "diagnostic timestamps unavailable or failed to progress: {coverage:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
