//! Scene attachments may be cheaper than presentation; printed lines and HQ jobs stay physical.
use viz_scene::{ViewConfiguration, ViewMode};

pub(super) fn scene_scale(view: &ViewConfiguration, capture_quality: bool, adaptive: f32) -> f32 {
    if capture_quality || view.mode.is_plot() || view.mode == ViewMode::Lines3d {
        1.0
    } else {
        view.quality.resolution_scale() * adaptive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_scene::RenderQuality;

    #[test]
    fn live_draft_reduces_scene_attachments_without_reducing_printed_views_or_hq_jobs() {
        let mut view = ViewConfiguration {
            mode: ViewMode::Full3d,
            quality: RenderQuality::Draft,
            ..Default::default()
        };
        assert_eq!(scene_scale(&view, false, 1.0), 0.25);
        // A frozen HQ job must not inherit the originating window's reduced Draft budget.
        assert_eq!(scene_scale(&view, true, 1.0), 1.0);
        for mode in ViewMode::ALL {
            if mode.is_plot() || mode == ViewMode::Lines3d {
                view.mode = mode;
                assert_eq!(scene_scale(&view, false, 1.0), 1.0, "{mode:?}");
            }
        }
    }

    #[test]
    fn standard_high_and_ultra_keep_their_existing_resolution_and_adaptation() {
        for quality in [
            RenderQuality::Standard,
            RenderQuality::High,
            RenderQuality::Ultra,
        ] {
            let view = ViewConfiguration {
                mode: ViewMode::Full3d,
                quality,
                ..Default::default()
            };
            assert_eq!(scene_scale(&view, false, 1.0), 1.0);
            if quality == RenderQuality::Ultra {
                assert_eq!(scene_scale(&view, false, 0.75), 0.75);
                assert_eq!(scene_scale(&view, true, 0.75), 1.0);
            }
        }
    }
}
