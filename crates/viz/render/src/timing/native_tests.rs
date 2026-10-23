//! A real GPU exercises mapping reuse and provenance across actual render submissions.
use crate::Renderer;
use viz_scene::{RenderQuality, Scene, SceneValues, ViewConfiguration};

#[test]
fn native_timestamp_readbacks_continue_and_do_not_cross_quality_boundaries() {
    let Ok(mut renderer) = Renderer::headless(64, 64) else {
        eprintln!("GPU unavailable: native timestamp ownership proof not run");
        return;
    };
    let scene = Scene {
        revision: 1,
        ..Default::default()
    };
    let values = SceneValues::default();
    let mut view = ViewConfiguration::default();
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
    renderer
        .capture(&scene, &values, &view, &overlay, 10.0)
        .expect("next Draft frame");
    assert!(renderer.stats().gpu_sample_matches_frame);
    eprintln!(
        "Native GPU timestamp ownership verified: {} distinct samples; quality boundary rejected",
        ids.len()
    );
}
