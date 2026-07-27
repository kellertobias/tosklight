//! Latest-value publication from the output boundary to visualization transports.

use arc_swap::ArcSwapOption;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_engine::{RenderOptions, RenderResult};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::SystemTime,
};

/// An immutable semantic frame known to have crossed the authoritative output boundary.
#[derive(Clone, Debug)]
pub(super) struct PublishedVisualizationFrame {
    pub(super) sequence: u64,
    pub(super) generated_at: SystemTime,
    pub(super) show_revision: u64,
    pub(super) options: RenderOptions,
    pub(super) values: Arc<HashMap<(FixtureId, AttributeKey), AttributeValue>>,
}

/// Capacity-one, non-blocking publication. Slow or disconnected observers cannot apply
/// backpressure to output; they simply see the newest complete frame on their next read.
#[derive(Default)]
pub(super) struct VisualizationFrameHub {
    next_sequence: AtomicU64,
    latest: ArcSwapOption<PublishedVisualizationFrame>,
}

impl VisualizationFrameHub {
    pub(super) fn publish(&self, rendered: &RenderResult, options: RenderOptions) {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        self.latest
            .store(Some(Arc::new(PublishedVisualizationFrame {
                sequence,
                generated_at: SystemTime::now(),
                show_revision: rendered.revision,
                options,
                values: Arc::clone(&rendered.resolved_values),
            })));
    }

    pub(super) fn latest(&self) -> Option<Arc<PublishedVisualizationFrame>> {
        self.latest.load_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_output::OutputRoute;
    use std::collections::HashMap;

    fn rendered(revision: u64) -> RenderResult {
        RenderResult {
            universes: HashMap::new(),
            resolved_values: Arc::new(HashMap::new()),
            patched_slots: HashMap::new(),
            revision,
            routes: Arc::<[OutputRoute]>::from([]),
            automatic_playback_transitions: Vec::new(),
        }
    }

    #[test]
    fn retains_only_the_latest_complete_frame() {
        let hub = VisualizationFrameHub::default();
        hub.publish(&rendered(10), RenderOptions::default());
        hub.publish(&rendered(20), RenderOptions::default());
        hub.publish(&rendered(30), RenderOptions::default());

        let latest = hub.latest().expect("a frame was published");
        assert_eq!(latest.sequence, 3);
        assert_eq!(latest.show_revision, 30);
    }
}
