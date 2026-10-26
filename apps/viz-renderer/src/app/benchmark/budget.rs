//! Effective CPU frame plans, separate from delayed GPU samples and CLI quality intent.
use viz_render::FrameStats;
use viz_scene::RenderQuality;

#[derive(Default)]
pub(super) struct Budget {
    scale: Option<(f32, f32)>,
    steps: Option<(u32, u32)>,
    shadows: Option<(u32, u32)>,
    beams: Option<(u32, u32)>,
    qualities: Vec<&'static str>,
    width: Option<(u32, u32)>,
    height: Option<(u32, u32)>,
    draft_beam_cache_hit_frames: u64,
    draft_beam_cache_miss_frames: u64,
}
fn range<T: PartialOrd + Copy>(current: &mut Option<(T, T)>, value: T) {
    *current = Some(match *current {
        Some((low, high)) => (
            if value < low { value } else { low },
            if value > high { value } else { high },
        ),
        None => (value, value),
    });
}
impl Budget {
    pub(super) fn record(&mut self, stats: &FrameStats) {
        if stats.render_scale.is_finite() && stats.render_scale > 0.0 {
            range(&mut self.scale, stats.render_scale);
        }
        range(&mut self.steps, stats.volumetric_steps);
        range(&mut self.shadows, stats.shadow_budget);
        range(&mut self.beams, stats.beams);
        self.draft_beam_cache_hit_frames = self
            .draft_beam_cache_hit_frames
            .saturating_add(u64::from(stats.draft_beam_cache_hit));
        self.draft_beam_cache_miss_frames = self
            .draft_beam_cache_miss_frames
            .saturating_add(u64::from(stats.draft_beam_cache_miss));
    }
    pub(super) fn observe_view(&mut self, quality: RenderQuality, size: (u32, u32)) {
        if !self.qualities.contains(&quality.wire()) {
            self.qualities.push(quality.wire());
        }
        range(&mut self.width, size.0);
        range(&mut self.height, size.1);
    }
    pub(super) fn report(&self, requested: Option<RenderQuality>) -> serde_json::Value {
        serde_json::json!({
            "requestedQuality": requested.map(RenderQuality::wire).unwrap_or("provider/local preferences"),
            "observedQualities": self.qualities,
            "physicalWidthRange": self.width,
            "physicalHeightRange": self.height,
            "renderScaleRange": self.scale,
            "volumetricStepsRange": self.steps,
            "shadowBudgetRange": self.shadows,
            "beamsRange": self.beams,
            "draftBeamCacheHitFrames": self.draft_beam_cache_hit_frames,
            "draftBeamCacheMissFrames": self.draft_beam_cache_miss_frames,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_counts_preserve_reuse_rebuilds_and_excluded_frames_separately() {
        let mut budget = Budget::default();
        let empty = budget.report(None);
        assert_eq!(empty["draftBeamCacheHitFrames"], 0);
        assert_eq!(empty["draftBeamCacheMissFrames"], 0);
        // Includes initial build, consecutive reuse, invalidation and ineligible views.
        for (hit, miss) in [
            (false, false),
            (false, true),
            (true, false),
            (true, false),
            (false, false),
            (false, true),
            (true, false),
        ] {
            budget.record(&FrameStats {
                draft_beam_cache_hit: hit,
                draft_beam_cache_miss: miss,
                ..Default::default()
            });
        }
        let report = budget.report(Some(RenderQuality::Draft));
        assert_eq!(report["draftBeamCacheHitFrames"], 3);
        assert_eq!(report["draftBeamCacheMissFrames"], 2);
        // Excluded frames must not be inferred as misses or hits.
        budget.record(&FrameStats::default());
        assert_eq!(budget.report(Some(RenderQuality::Draft)), report);
    }
    #[test]
    fn effective_budget_reports_actual_ranges_independently_of_requested_quality() {
        let mut budget = Budget::default();
        for (scale, steps, shadows, beams, size) in [
            (0.5, 12, 2, 149, (1920, 1080)),
            (0.25, 4, 1, 160, (2880, 1800)),
        ] {
            budget.record(&FrameStats {
                render_scale: scale,
                volumetric_steps: steps,
                shadow_budget: shadows,
                beams,
                ..Default::default()
            });
            budget.observe_view(RenderQuality::Draft, size);
        }
        let report = budget.report(Some(RenderQuality::High));
        assert_eq!(report["requestedQuality"], "high");
        assert_eq!(report["observedQualities"], serde_json::json!(["draft"]));
        for (key, expected) in [
            ("renderScaleRange", serde_json::json!([0.25, 0.5])),
            ("volumetricStepsRange", serde_json::json!([4, 12])),
            ("shadowBudgetRange", serde_json::json!([1, 2])),
            ("beamsRange", serde_json::json!([149, 160])),
            ("physicalWidthRange", serde_json::json!([1920, 2880])),
            ("physicalHeightRange", serde_json::json!([1080, 1800])),
        ] {
            assert_eq!(report[key], expected, "{key}");
        }
    }
    #[test]
    fn absent_and_invalid_scale_is_unavailable_while_zero_work_remains_explicit() {
        let mut budget = Budget::default();
        assert!(budget.report(None)["renderScaleRange"].is_null());
        budget.record(&FrameStats {
            render_scale: f32::NAN,
            ..Default::default()
        });
        let report = budget.report(None);
        assert!(report["renderScaleRange"].is_null());
        assert_eq!(report["beamsRange"], serde_json::json!([0, 0]));
        assert!(report["physicalWidthRange"].is_null());
    }
}
