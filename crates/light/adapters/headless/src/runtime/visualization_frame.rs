//! Latest-value publication from the output boundary to visualization transports.

use arc_swap::ArcSwapOption;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_engine::{RenderOptions, RenderResult};
use light_wire::v2::{
    preload_values::ProgrammingPreloadAttributeValue,
    visualization::{
        VisualizationLane, VisualizationLaneDelta, VisualizationLaneSnapshot, VisualizationValue,
        VisualizationValueKey,
    },
};
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

pub(super) struct ProjectedVisualizationFrame {
    pub(super) source_sequence: u64,
    pub(super) previous_source_sequence: Option<u64>,
    pub(super) snapshot: Arc<VisualizationLaneSnapshot>,
    pub(super) delta: Arc<VisualizationLaneDelta>,
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
    pub(super) snapshot_requests: u64,
    pub(super) snapshot_projection_micros: u64,
    pub(super) snapshot_serialization_micros: u64,
    pub(super) snapshot_payload_bytes: u64,
    pub(super) snapshot_source_frame: u64,
    pub(super) snapshot_source_age_millis: u64,
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
    projections:
        parking_lot::Mutex<HashMap<VisualizationProjectionKey, Arc<ProjectedVisualizationFrame>>>,
    projection_claims: parking_lot::Mutex<HashMap<VisualizationProjectionKey, u64>>,
    normal_subscribers: AtomicU64,
    preload_subscribers: AtomicU64,
    projection_count: AtomicU64,
    projection_micros: AtomicU64,
    payload_bytes: AtomicU64,
    source_age_millis: AtomicU64,
    skipped_source_frames: AtomicU64,
    snapshot_requests: AtomicU64,
    snapshot_projection_micros: AtomicU64,
    snapshot_serialization_micros: AtomicU64,
    snapshot_payload_bytes: AtomicU64,
    snapshot_source_frame: AtomicU64,
    snapshot_source_age_millis: AtomicU64,
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
    ) -> Result<Arc<ProjectedVisualizationFrame>, super::ApiError> {
        let mut projections = self.projections.lock();
        if let Some(cached) = projections
            .get(&key)
            .filter(|cached| cached.source_sequence == source.sequence)
        {
            return Ok(Arc::clone(cached));
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
        let previous = projections.get(&key);
        let delta = Arc::new(lane_delta(
            previous.map(|projection| projection.snapshot.as_ref()),
            &snapshot,
        ));
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
        let projection = Arc::new(ProjectedVisualizationFrame {
            source_sequence: source.sequence,
            previous_source_sequence: previous.map(|projection| projection.source_sequence),
            snapshot: Arc::clone(&snapshot),
            delta,
        });
        projections.insert(key, Arc::clone(&projection));
        Ok(projection)
    }

    pub(super) fn change_subscribers(&self, lane: VisualizationLane, delta: i8) {
        let subscribers = match lane {
            VisualizationLane::Normal => &self.normal_subscribers,
            VisualizationLane::Preload => &self.preload_subscribers,
        };
        if delta > 0 {
            subscribers.fetch_add(delta as u64, Ordering::Relaxed);
        } else {
            let decrement = u64::from(delta.unsigned_abs());
            let _ = subscribers.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_sub(decrement))
            });
        }
    }

    pub(super) fn change_projection_claim(&self, key: VisualizationProjectionKey, delta: i8) {
        let mut claims = self.projection_claims.lock();
        if delta > 0 {
            *claims.entry(key).or_default() += delta as u64;
            return;
        }
        let Some(current) = claims.get_mut(&key) else {
            return;
        };
        *current = current.saturating_sub(u64::from(delta.unsigned_abs()));
        if *current == 0 {
            claims.remove(&key);
            self.projections.lock().remove(&key);
        }
    }

    pub(super) fn record_snapshot_route(
        &self,
        projection_duration: Duration,
        serialization_duration: Duration,
        payload_bytes: u64,
        source: Option<&PublishedVisualizationFrame>,
    ) {
        self.snapshot_requests.fetch_add(1, Ordering::Relaxed);
        self.snapshot_projection_micros
            .store(duration_micros(projection_duration), Ordering::Relaxed);
        self.snapshot_serialization_micros
            .store(duration_micros(serialization_duration), Ordering::Relaxed);
        self.snapshot_payload_bytes
            .store(payload_bytes, Ordering::Relaxed);
        self.snapshot_source_frame.store(
            source.map_or(0, |source| source.sequence),
            Ordering::Relaxed,
        );
        self.snapshot_source_age_millis.store(
            source
                .and_then(|source| source.generated_at.elapsed().ok())
                .map_or(0, |age| age.as_millis().min(u128::from(u64::MAX)) as u64),
            Ordering::Relaxed,
        );
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
            snapshot_requests: self.snapshot_requests.load(Ordering::Relaxed),
            snapshot_projection_micros: self.snapshot_projection_micros.load(Ordering::Relaxed),
            snapshot_serialization_micros: self
                .snapshot_serialization_micros
                .load(Ordering::Relaxed),
            snapshot_payload_bytes: self.snapshot_payload_bytes.load(Ordering::Relaxed),
            snapshot_source_frame: self.snapshot_source_frame.load(Ordering::Relaxed),
            snapshot_source_age_millis: self.snapshot_source_age_millis.load(Ordering::Relaxed),
        }
    }
}

fn lane_delta(
    previous: Option<&VisualizationLaneSnapshot>,
    current: &VisualizationLaneSnapshot,
) -> VisualizationLaneDelta {
    let previous_values = previous.map(|snapshot| value_index(&snapshot.values));
    let previous_profile = previous.map(|snapshot| value_index(&snapshot.profile_output_values));
    VisualizationLaneDelta {
        revision: current.revision,
        generated_at: current.generated_at.clone(),
        grand_master: current.grand_master,
        blackout: current.blackout,
        preload: current.preload,
        values: changed_values(previous_values.as_ref(), &current.values),
        removed_values: removed_values(previous_values.as_ref(), &current.values),
        dynamic_stack: current.dynamic_stack.clone(),
        profile_output_values: changed_values(
            previous_profile.as_ref(),
            &current.profile_output_values,
        ),
        removed_profile_output_values: removed_values(
            previous_profile.as_ref(),
            &current.profile_output_values,
        ),
    }
}

fn value_index(
    values: &[VisualizationValue],
) -> HashMap<(Uuid, String), &ProgrammingPreloadAttributeValue> {
    values
        .iter()
        .map(|value| ((value.fixture_id, value.attribute.clone()), &value.value))
        .collect()
}

fn changed_values(
    previous: Option<&HashMap<(Uuid, String), &ProgrammingPreloadAttributeValue>>,
    current: &[VisualizationValue],
) -> Vec<VisualizationValue> {
    current
        .iter()
        .filter(|value| {
            previous.is_none_or(|previous| {
                previous.get(&(value.fixture_id, value.attribute.clone())) != Some(&&value.value)
            })
        })
        .cloned()
        .collect()
}

fn removed_values(
    previous: Option<&HashMap<(Uuid, String), &ProgrammingPreloadAttributeValue>>,
    current: &[VisualizationValue],
) -> Vec<VisualizationValueKey> {
    let current = current
        .iter()
        .map(|value| (value.fixture_id, value.attribute.as_str()))
        .collect::<std::collections::HashSet<_>>();
    previous
        .into_iter()
        .flat_map(HashMap::keys)
        .filter(|(fixture_id, attribute)| !current.contains(&(*fixture_id, attribute.as_str())))
        .map(|(fixture_id, attribute)| VisualizationValueKey {
            fixture_id: *fixture_id,
            attribute: attribute.clone(),
        })
        .collect()
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
        sync::mpsc,
        thread,
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
        assert!(first.previous_source_sequence.is_none());
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

    #[test]
    fn final_projection_claim_releases_session_specific_cache() {
        let hub = VisualizationFrameHub::default();
        let key = VisualizationProjectionKey::Preload(Uuid::new_v4());
        hub.change_projection_claim(key, 1);
        hub.publish(&rendered(10), RenderOptions::default());
        let source = hub.latest().unwrap();
        let builds = AtomicUsize::new(0);
        let mut build = || {
            builds.fetch_add(1, AtomicOrdering::Relaxed);
            Ok(VisualizationLaneSnapshot {
                revision: 10,
                generated_at: "2026-07-27T00:00:00Z".into(),
                grand_master: 1.0,
                blackout: false,
                preload: true,
                values: Vec::new(),
                dynamic_stack: Vec::new(),
                profile_output_values: Vec::new(),
            })
        };
        hub.projection(key, &source, &mut build).unwrap();
        hub.projection(key, &source, &mut build).unwrap();
        assert_eq!(builds.load(AtomicOrdering::Relaxed), 1);

        hub.change_projection_claim(key, -1);
        hub.change_projection_claim(key, 1);
        hub.projection(key, &source, &mut build).unwrap();
        assert_eq!(builds.load(AtomicOrdering::Relaxed), 2);
    }

    #[test]
    fn projection_work_cannot_block_latest_frame_publication() {
        let hub = Arc::new(VisualizationFrameHub::default());
        hub.publish(&rendered(10), RenderOptions::default());
        let source = hub.latest().unwrap();
        let (projection_started_tx, projection_started_rx) = mpsc::channel();
        let (release_projection_tx, release_projection_rx) = mpsc::channel();
        let projection_hub = Arc::clone(&hub);
        let projection = thread::spawn(move || {
            projection_hub
                .projection(VisualizationProjectionKey::Normal, &source, || {
                    projection_started_tx.send(()).unwrap();
                    release_projection_rx.recv().unwrap();
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
                })
                .unwrap();
        });
        projection_started_rx.recv().unwrap();

        let (published_tx, published_rx) = mpsc::channel();
        let publisher_hub = Arc::clone(&hub);
        thread::spawn(move || {
            publisher_hub.publish(&rendered(20), RenderOptions::default());
            published_tx.send(()).unwrap();
        });
        published_rx
            .recv_timeout(Duration::from_millis(100))
            .expect("publication must not wait for projection work");
        assert_eq!(hub.latest().unwrap().show_revision, 20);

        release_projection_tx.send(()).unwrap();
        projection.join().unwrap();
    }
}
