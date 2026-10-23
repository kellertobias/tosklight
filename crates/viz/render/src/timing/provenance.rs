//! Submission identity and one-shot ownership of an asynchronous GPU readback.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TimingContext {
    pub scene_revision: u64,
    pub show_id: Option<u128>,
    pub mode: viz_scene::ViewMode,
    pub quality: viz_scene::RenderQuality,
    pub size: (u32, u32),
    pub render_size: (u32, u32),
    pub generation: u64,
}

impl TimingContext {
    /// A return to an earlier view is a new scope, not a match for its delayed old sample.
    pub fn following(mut self, previous: Option<Self>) -> Self {
        self.generation = previous.map_or(0, |old| old.generation);
        if previous != Some(self) {
            self.generation = self.generation.saturating_add(1);
        }
        self
    }
}

#[derive(Default)]
pub(super) struct ReadbackOwnership {
    pending: Option<TimingContext>,
    mapped: bool,
    completed: Option<TimingContext>,
}

impl ReadbackOwnership {
    pub fn in_flight(&self) -> bool {
        self.pending.is_some()
    }

    pub fn submitted(&mut self, context: TimingContext) {
        self.pending = Some(context);
        self.mapped = false;
    }

    pub fn request_mapping(&mut self) -> bool {
        if self.pending.is_none() || self.mapped {
            return false;
        }
        self.mapped = true;
        true
    }

    pub fn completed(&mut self) {
        self.completed = self.pending.take();
        self.mapped = false;
    }

    pub fn failed(&mut self) {
        self.pending = None;
        self.completed = None;
        self.mapped = false;
    }

    pub fn sample_matches(&self, context: TimingContext) -> bool {
        self.completed == Some(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(generation: u64) -> TimingContext {
        TimingContext {
            scene_revision: 1,
            show_id: Some(1),
            mode: viz_scene::ViewMode::Full3d,
            quality: viz_scene::RenderQuality::Ultra,
            size: (1920, 1080),
            render_size: (1920, 1080),
            generation,
        }
    }

    #[test]
    fn actual_scope_advancement_rejects_returned_views_and_all_boundaries() {
        let first = context(0).following(None);
        let stable = context(0).following(Some(first));
        assert_eq!(stable, first);
        for changed in [
            TimingContext {
                mode: viz_scene::ViewMode::Lines3d,
                ..context(0)
            },
            TimingContext {
                quality: viz_scene::RenderQuality::Draft,
                ..context(0)
            },
            TimingContext {
                size: (960, 540),
                ..context(0)
            },
            TimingContext {
                render_size: (960, 540),
                ..context(0)
            },
            TimingContext {
                scene_revision: 2,
                ..context(0)
            },
            TimingContext {
                show_id: Some(2),
                ..context(0)
            },
        ] {
            let next = changed.following(Some(first));
            assert_eq!(next.generation, first.generation + 1);
            let returned = context(0).following(Some(next));
            assert_eq!(returned.generation, first.generation + 2);
            assert_ne!(returned, first);
        }
    }

    #[test]
    fn delayed_readback_maps_once_and_retains_submitted_context() {
        let mut state = ReadbackOwnership::default();
        assert!(!state.request_mapping());
        state.submitted(context(1));
        assert!(state.request_mapping());
        assert!(!state.request_mapping());
        assert!(state.in_flight());
        assert!(!state.sample_matches(context(1)));
        state.completed();
        assert!(!state.in_flight());
        assert!(state.sample_matches(context(1)));
        // Leaving and returning to the same view must not attribute the old sample to the new slice.
        assert!(!state.sample_matches(context(3)));
        state.submitted(context(3));
        assert!(state.request_mapping());
        assert!(state.sample_matches(context(1)));
        state.completed();
        assert!(state.sample_matches(context(3)));
        assert!(!state.sample_matches(context(1)));
    }

    #[test]
    fn late_sample_cannot_cross_quality_size_or_show_boundaries() {
        let mut state = ReadbackOwnership::default();
        state.submitted(context(1));
        state.completed();
        for changed in [
            TimingContext {
                quality: viz_scene::RenderQuality::Draft,
                ..context(1)
            },
            TimingContext {
                mode: viz_scene::ViewMode::Lines3d,
                ..context(1)
            },
            TimingContext {
                size: (960, 540),
                ..context(1)
            },
            TimingContext {
                scene_revision: 2,
                ..context(1)
            },
        ] {
            assert!(!state.sample_matches(changed));
        }
    }

    #[test]
    fn failed_mapping_discards_prior_sample_and_pending_ownership() {
        let mut state = ReadbackOwnership::default();
        state.submitted(context(1));
        state.completed();
        state.submitted(context(2));
        assert!(state.request_mapping());
        state.failed();
        assert!(!state.in_flight());
        assert!(!state.request_mapping());
        assert!(!state.sample_matches(context(1)));
    }
}
