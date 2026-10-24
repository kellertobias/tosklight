use super::*;
use crate::ultimate::UltimateEligibility;
use viz_scene::{RenderQuality, ViewMode};

#[test]
fn every_plan_view_refuses_ultimate_with_an_explicit_native_alternative() {
    let scene = Scene::default();
    let values = SceneValues::default();
    let media = FrozenMedia::default();
    let mut count = 0;
    for mode in ViewMode::ALL.into_iter().filter(|mode| mode.is_plot()) {
        let view = ViewConfiguration {
            mode,
            quality: RenderQuality::Draft,
            ..Default::default()
        };
        let image = ImageManifest::build(&scene, &values, &view, 1.25, &media).unwrap();
        let eligibility = UltimateEligibility::inspect_image(&image).unwrap();
        assert!(
            !eligibility.is_supported(),
            "plot mode {mode:?} must be explicit"
        );
        assert!(eligibility.reasons().iter().any(|reason| {
            reason.contains("2D plan symbols/outlines") && reason.contains("choose Ultra")
        }));
        // Building the alternate-engine chooser must leave the native capture settings intact.
        assert_eq!(view.mode, mode);
        assert_eq!(view.quality, RenderQuality::Draft);
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn shaded_capture_from_each_3d_view_remains_eligible_at_live_draft_quality() {
    let scene = Scene::default();
    let values = SceneValues::default();
    let media = FrozenMedia::default();
    let mut count = 0;
    for mode in ViewMode::ALL.into_iter().filter(|mode| !mode.is_plot()) {
        let view = ViewConfiguration {
            mode,
            quality: RenderQuality::Draft,
            ..Default::default()
        };
        let image = ImageManifest::build(&scene, &values, &view, 1.25, &media).unwrap();
        let eligibility = UltimateEligibility::inspect_image(&image).unwrap();
        assert!(
            eligibility.is_supported(),
            "{mode:?}: {:?}",
            eligibility.reasons()
        );
        assert_eq!(view.mode, mode);
        assert_eq!(view.quality, RenderQuality::Draft);
        count += 1;
    }
    assert_eq!(count, 3);
}
