//! Latest-value publication from the output boundary to visualization transports.

use arc_swap::ArcSwapOption;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_engine::{RenderOptions, RenderResult};
use light_wire::v2::visualization::{VisualizationLane, VisualizationLaneSnapshot};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum VisualizationProjectionKey {
    Normal,
    Preload(Uuid),
}

struct CachedProjection {
    source_sequence: u64,
    snapshot: Arc<VisualizationLaneSnapshot>,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub(super) struct VisualizationMetrics {
    pub(super) normal_subscribers: u64,
    pub(super) preload_subscribers: u64,
    pub(super) projections: u64,
    pub(super) projection_micros: u64,
    pub(super) payload_bytes: u64,
    pub(super) source_age_millis: u64,
    pub(super) skipped_source_frames: u64,
}

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
    projections: parking_lot::Mutex<HashMap<VisualizationProjectionKey, CachedProjection>>,
    normal_subscribers: AtomicU64,
    preload_subscribers: AtomicU64,
    projection_count: AtomicU64,
    projection_micros: AtomicU64,
    payload_bytes: AtomicU64,
    source_age_millis: AtomicU64,
    skipped_source_frames: AtomicU64,
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

    pub(super) fn projection(
        &self,
        key: VisualizationProjectionKey,
        source: &PublishedVisualizationFrame,
        build: impl FnOnce() -> Result<VisualizationLaneSnapshot, super::ApiError>,
    ) -> Result<Arc<VisualizationLaneSnapshot>, super::ApiError> {
        let mut projections = self.projections.lock();
        if let Some(cached) = projections
            .get(&key)
            .filter(|cached| cached.source_sequence == source.sequence)
        {
            return Ok(Arc::clone(&cached.snapshot));
        }
        if let Some(previous) = projections.get(&key) {
            self.skipped_source_frames.fetch_add(
                source
                    .sequence
                    .saturating_sub(previous.source_sequence)
                    .saturating_sub(1),
                Ordering::Relaxed,
            );
        }
        let started = Instant::now();
        let snapshot = Arc::new(build()?);
        self.projection_count.fetch_add(1, Ordering::Relaxed);
        self.projection_micros
            .store(duration_micros(started.elapsed()), Ordering::Relaxed);
        self.payload_bytes.store(
            serde_json::to_vec(snapshot.as_ref())
                .map(|payload| payload.len() as u64)
                .unwrap_or_default(),
            Ordering::Relaxed,
        );
        self.source_age_millis.store(
            source
                .generated_at
                .elapsed()
                .unwrap_or_default()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
            Ordering::Relaxed,
        );
        projections.insert(
            key,
            CachedProjection {
                source_sequence: source.sequence,
                snapshot: Arc::clone(&snapshot),
            },
        );
        Ok(snapshot)
    }

    pub(super) fn change_subscribers(&self, lane: VisualizationLane, delta: i8) {
        let subscribers = match lane {
            VisualizationLane::Normal => &self.normal_subscribers,
            VisualizationLane::Preload => &self.preload_subscribers,
        };
        if delta > 0 {
            subscribers.fetch_add(delta as u64, Ordering::Relaxed);
        } else {
            subscribers.fetch_sub(delta.unsigned_abs().into(), Ordering::Relaxed);
        }
    }

    pub(super) fn metrics(&self) -> VisualizationMetrics {
        VisualizationMetrics {
            normal_subscribers: self.normal_subscribers.load(Ordering::Relaxed),
            preload_subscribers: self.preload_subscribers.load(Ordering::Relaxed),
            projections: self.projection_count.load(Ordering::Relaxed),
            projection_micros: self.projection_micros.load(Ordering::Relaxed),
            payload_bytes: self.payload_bytes.load(Ordering::Relaxed),
            source_age_millis: self.source_age_millis.load(Ordering::Relaxed),
            skipped_source_frames: self.skipped_source_frames.load(Ordering::Relaxed),
        }
    }
}

fn duration_micros(duration: Duration) -> u64 {
    duration.as_micros().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_output::OutputRoute;
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
    };

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

    #[test]
    fn shares_one_projection_for_the_same_lane_and_source_frame() {
        let hub = VisualizationFrameHub::default();
        hub.publish(&rendered(10), RenderOptions::default());
        let source = hub.latest().unwrap();
        let builds = AtomicUsize::new(0);
        let build = || {
            builds.fetch_add(1, AtomicOrdering::Relaxed);
            Ok(VisualizationLaneSnapshot {
                revision: 10,
                generated_at: "2026-07-27T00:00:00Z".into(),
                grand_master: 1.0,
                blackout: false,
                preload: false,
                values: Vec::new(),
                dynamic_stack: Vec::new(),
                profile_output_values: Vec::new(),
            })
        };

        let first = hub
            .projection(VisualizationProjectionKey::Normal, &source, build)
            .unwrap();
        let second = hub
            .projection(VisualizationProjectionKey::Normal, &source, build)
            .unwrap();

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(builds.load(AtomicOrdering::Relaxed), 1);
        assert_eq!(hub.metrics().projections, 1);
    }

    #[test]
    fn subscriber_metrics_return_to_zero() {
        let hub = VisualizationFrameHub::default();
        hub.change_subscribers(VisualizationLane::Normal, 1);
        hub.change_subscribers(VisualizationLane::Preload, 1);
        assert_eq!(hub.metrics().normal_subscribers, 1);
        assert_eq!(hub.metrics().preload_subscribers, 1);
        hub.change_subscribers(VisualizationLane::Normal, -1);
        hub.change_subscribers(VisualizationLane::Preload, -1);
        assert_eq!(hub.metrics().normal_subscribers, 0);
        assert_eq!(hub.metrics().preload_subscribers, 0);
    }
}
