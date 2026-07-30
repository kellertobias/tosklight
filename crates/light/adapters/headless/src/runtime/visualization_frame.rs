//! Latest-value publication from the output boundary to visualization transports.

use arc_swap::ArcSwapOption;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_engine::{RenderOptions, RenderResult};
use light_wire::v2::{
    preload_values::ProgrammingPreloadAttributeValue,
    visualization::{
        VisualizationLane, VisualizationLaneDelta, VisualizationLaneSnapshot, VisualizationScope,
        VisualizationValue, VisualizationValueKey,
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
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(super) const VISUALIZATION_SOURCE_SAMPLE_INTERVAL: Duration = Duration::from_millis(40);
const DYNAMIC_STACK_PUBLICATION_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum VisualizationProjectionKey {
    Normal {
        include_dynamic_stack: bool,
    },
    Preload {
        session_id: Uuid,
        include_dynamic_stack: bool,
    },
}

impl VisualizationProjectionKey {
    const fn includes_dynamic_stack(self) -> bool {
        match self {
            Self::Normal {
                include_dynamic_stack,
            }
            | Self::Preload {
                include_dynamic_stack,
                ..
            } => include_dynamic_stack,
        }
    }
}

pub(super) struct ProjectedVisualizationFrame {
    pub(super) source_sequence: u64,
    pub(super) previous_source_sequence: Option<u64>,
    pub(super) lane_source_sequence: u64,
    pub(super) source_generated_at: SystemTime,
    pub(super) snapshot: Arc<VisualizationLaneSnapshot>,
    pub(super) delta: Arc<VisualizationLaneDelta>,
    dynamic_stack_generated_at: Instant,
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
    pub(super) stream_serializations: u64,
    pub(super) stream_serialization_micros: u64,
    pub(super) stream_payload_bytes: u64,
    pub(super) stream_sends: u64,
    pub(super) stream_send_micros: u64,
    pub(super) stream_send_failures: u64,
    pub(super) stream_queue_depth: u64,
    pub(super) stream_queue_drops: u64,
}

/// An immutable semantic frame known to have crossed the authoritative output boundary.
#[derive(Clone, Debug)]
pub(super) struct PublishedVisualizationFrame {
    pub(super) sequence: u64,
    pub(super) generated_at: SystemTime,
    pub(super) scope: VisualizationScope,
    pub(super) show_revision: u64,
    pub(super) options: RenderOptions,
    pub(super) values: Arc<HashMap<(FixtureId, AttributeKey), AttributeValue>>,
    pub(super) profile_visualization_values:
        Arc<HashMap<(FixtureId, AttributeKey), AttributeValue>>,
}

/// Capacity-one, non-blocking publication. Slow or disconnected observers cannot apply
/// backpressure to output; they simply see the newest complete frame on their next read.
#[derive(Default)]
pub(super) struct VisualizationFrameHub {
    next_sequence: AtomicU64,
    next_projection_sequence: AtomicU64,
    latest: ArcSwapOption<PublishedVisualizationFrame>,
    sampled: ArcSwapOption<PublishedVisualizationFrame>,
    projections:
        parking_lot::Mutex<HashMap<VisualizationProjectionKey, Arc<ProjectedVisualizationFrame>>>,
    projection_claims: parking_lot::Mutex<HashMap<VisualizationProjectionKey, u64>>,
    normal_subscribers: AtomicU64,
    preload_subscribers: AtomicU64,
    subscriber_notify: Notify,
    subscriber_generation: AtomicU64,
    source_notify: Notify,
    sample_notify: Notify,
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
    stream_serializations: AtomicU64,
    stream_serialization_micros: AtomicU64,
    stream_payload_bytes: AtomicU64,
    stream_sends: AtomicU64,
    stream_send_micros: AtomicU64,
    stream_send_failures: AtomicU64,
    stream_queue_depth: AtomicU64,
    stream_queue_drops: AtomicU64,
}

impl VisualizationFrameHub {
    pub(super) fn publish(
        &self,
        rendered: &RenderResult,
        options: RenderOptions,
        scope: VisualizationScope,
    ) {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        self.latest
            .store(Some(Arc::new(PublishedVisualizationFrame {
                sequence,
                generated_at: SystemTime::now(),
                scope,
                show_revision: rendered.revision,
                options,
                values: Arc::clone(&rendered.resolved_values),
                profile_visualization_values: Arc::clone(&rendered.profile_visualization_values),
            })));
        self.source_notify.notify_one();
    }

    pub(super) fn latest(&self) -> Option<Arc<PublishedVisualizationFrame>> {
        self.latest.load_full()
    }

    pub(super) fn sampled(&self) -> Option<Arc<PublishedVisualizationFrame>> {
        self.sampled.load_full()
    }

    pub(super) async fn wait_for_sample_after(
        &self,
        sequence: u64,
    ) -> Arc<PublishedVisualizationFrame> {
        loop {
            let notified = self.sample_notify.notified();
            if let Some(source) = self.sampled()
                && source.sequence > sequence
            {
                return source;
            }
            notified.await;
        }
    }

    fn sample_latest(&self) -> bool {
        if self.normal_subscribers.load(Ordering::Relaxed) == 0
            && self.preload_subscribers.load(Ordering::Relaxed) == 0
        {
            return false;
        }
        if let Some(source) = self.latest() {
            if self
                .sampled()
                .is_some_and(|sampled| sampled.sequence == source.sequence)
            {
                return false;
            }
            self.sampled.store(Some(source));
            self.sample_notify.notify_waiters();
            return true;
        }
        false
    }

    pub(super) async fn run_sampler(
        self: Arc<Self>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<()> {
        loop {
            while !self.has_subscribers() {
                tokio::select! {
                    _ = cancellation.cancelled() => return Ok(()),
                    _ = self.subscriber_notify.notified() => {},
                }
            }
            self.sample_latest();
            let generation = self.subscriber_generation.load(Ordering::Relaxed);
            let mut next_due = tokio::time::Instant::now() + VISUALIZATION_SOURCE_SAMPLE_INTERVAL;
            let mut due_for_new_source = false;
            while self.has_subscribers() {
                let source_notified = self.source_notify.notified();
                tokio::select! {
                    _ = cancellation.cancelled() => return Ok(()),
                    _ = tokio::time::sleep_until(next_due), if !due_for_new_source => {
                        if self.sample_latest() {
                            next_due =
                                tokio::time::Instant::now() + VISUALIZATION_SOURCE_SAMPLE_INTERVAL;
                        } else {
                            // Do not consume a cadence slot without a new
                            // authoritative source. The next publication can be
                            // sampled immediately while actual samples remain
                            // capped at ten hertz.
                            due_for_new_source = true;
                        }
                    },
                    _ = source_notified => {
                        if due_for_new_source && self.sample_latest() {
                            due_for_new_source = false;
                            next_due =
                                tokio::time::Instant::now() + VISUALIZATION_SOURCE_SAMPLE_INTERVAL;
                        }
                    },
                    _ = self.subscriber_notify.notified() => {
                        if !self.has_subscribers()
                            || self.subscriber_generation.load(Ordering::Relaxed) != generation
                        {
                            break;
                        }
                    },
                }
            }
        }
    }

    fn has_subscribers(&self) -> bool {
        self.normal_subscribers.load(Ordering::Relaxed) != 0
            || self.preload_subscribers.load(Ordering::Relaxed) != 0
    }

    pub(super) fn projection(
        &self,
        key: VisualizationProjectionKey,
        source: &PublishedVisualizationFrame,
        build: impl FnOnce(bool) -> Result<VisualizationLaneSnapshot, super::ApiError>,
    ) -> Result<Arc<ProjectedVisualizationFrame>, super::ApiError> {
        let previous = {
            let projections = self.projections.lock();
            if let Some(cached) = projections
                .get(&key)
                .filter(|cached| cached.source_sequence == source.sequence)
            {
                return Ok(Arc::clone(cached));
            }
            projections.get(&key).cloned()
        };
        let started = Instant::now();
        let refresh_dynamic_stack = key.includes_dynamic_stack()
            && previous.as_ref().is_none_or(|projection| {
                projection.dynamic_stack_generated_at.elapsed()
                    >= DYNAMIC_STACK_PUBLICATION_INTERVAL
            });
        let mut snapshot = build(refresh_dynamic_stack)?;
        if key.includes_dynamic_stack()
            && !refresh_dynamic_stack
            && let Some(previous) = previous.as_ref()
        {
            snapshot.dynamic_stack = previous.snapshot.dynamic_stack.clone();
        }
        let dynamic_stack_generated_at = if refresh_dynamic_stack {
            Instant::now()
        } else {
            previous.as_ref().map_or_else(Instant::now, |projection| {
                projection.dynamic_stack_generated_at
            })
        };
        let (lane_source_sequence, source_generated_at) = match key {
            VisualizationProjectionKey::Normal { .. } => (source.sequence, source.generated_at),
            VisualizationProjectionKey::Preload { .. } => {
                let generated_at = SystemTime::now();
                snapshot.generated_at =
                    chrono::DateTime::<chrono::Utc>::from(generated_at).to_rfc3339();
                (
                    self.next_projection_sequence
                        .fetch_add(1, Ordering::Relaxed)
                        + 1,
                    generated_at,
                )
            }
        };
        let snapshot = Arc::new(snapshot);
        let mut previous_source_sequence = previous
            .as_ref()
            .map(|projection| projection.source_sequence);
        let mut delta = Arc::new(lane_delta(
            previous
                .as_ref()
                .map(|projection| projection.snapshot.as_ref()),
            &snapshot,
        ));
        let mut projections = self.projections.lock();
        if let Some(cached) = projections
            .get(&key)
            .filter(|cached| cached.source_sequence == source.sequence)
        {
            return Ok(Arc::clone(cached));
        }
        if projections
            .get(&key)
            .map(|projection| projection.source_sequence)
            != previous_source_sequence
        {
            previous_source_sequence = projections
                .get(&key)
                .map(|projection| projection.source_sequence);
            delta = Arc::new(lane_delta(
                projections
                    .get(&key)
                    .map(|projection| projection.snapshot.as_ref()),
                &snapshot,
            ));
        }
        if let Some(previous_source_sequence) = previous_source_sequence {
            self.skipped_source_frames.fetch_add(
                source
                    .sequence
                    .saturating_sub(previous_source_sequence)
                    .saturating_sub(1),
                Ordering::Relaxed,
            );
        }
        self.projection_count.fetch_add(1, Ordering::Relaxed);
        self.projection_micros
            .store(duration_micros(started.elapsed()), Ordering::Relaxed);
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
            previous_source_sequence,
            lane_source_sequence,
            source_generated_at,
            snapshot: Arc::clone(&snapshot),
            delta,
            dynamic_stack_generated_at,
        });
        projections.insert(key, Arc::clone(&projection));
        Ok(projection)
    }

    pub(super) fn change_subscribers(&self, lane: VisualizationLane, delta: i8) {
        let was_inactive = !self.has_subscribers();
        let subscribers = match lane {
            VisualizationLane::Normal => &self.normal_subscribers,
            VisualizationLane::Preload => &self.preload_subscribers,
        };
        if delta > 0 {
            subscribers.fetch_add(delta as u64, Ordering::Relaxed);
            if was_inactive {
                self.subscriber_generation.fetch_add(1, Ordering::Relaxed);
                self.sample_latest();
            }
            self.subscriber_notify.notify_one();
        } else {
            let decrement = u64::from(delta.unsigned_abs());
            let _ = subscribers.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_sub(decrement))
            });
            self.subscriber_notify.notify_one();
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

    pub(super) fn record_stream_serialization(&self, duration: Duration, payload_bytes: u64) {
        self.stream_serializations.fetch_add(1, Ordering::Relaxed);
        self.stream_serialization_micros
            .store(duration_micros(duration), Ordering::Relaxed);
        self.stream_payload_bytes
            .store(payload_bytes, Ordering::Relaxed);
        self.payload_bytes.store(payload_bytes, Ordering::Relaxed);
    }

    pub(super) fn record_stream_queue_push(&self, replaced_pending: bool) {
        if replaced_pending {
            self.stream_queue_drops.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stream_queue_depth.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn record_stream_queue_take(&self) {
        let _ =
            self.stream_queue_depth
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |depth| {
                    Some(depth.saturating_sub(1))
                });
    }

    pub(super) fn record_stream_send(&self, duration: Duration, succeeded: bool) {
        self.stream_sends.fetch_add(1, Ordering::Relaxed);
        self.stream_send_micros
            .store(duration_micros(duration), Ordering::Relaxed);
        if !succeeded {
            self.stream_send_failures.fetch_add(1, Ordering::Relaxed);
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
            snapshot_requests: self.snapshot_requests.load(Ordering::Relaxed),
            snapshot_projection_micros: self.snapshot_projection_micros.load(Ordering::Relaxed),
            snapshot_serialization_micros: self
                .snapshot_serialization_micros
                .load(Ordering::Relaxed),
            snapshot_payload_bytes: self.snapshot_payload_bytes.load(Ordering::Relaxed),
            snapshot_source_frame: self.snapshot_source_frame.load(Ordering::Relaxed),
            snapshot_source_age_millis: self.snapshot_source_age_millis.load(Ordering::Relaxed),
            stream_serializations: self.stream_serializations.load(Ordering::Relaxed),
            stream_serialization_micros: self.stream_serialization_micros.load(Ordering::Relaxed),
            stream_payload_bytes: self.stream_payload_bytes.load(Ordering::Relaxed),
            stream_sends: self.stream_sends.load(Ordering::Relaxed),
            stream_send_micros: self.stream_send_micros.load(Ordering::Relaxed),
            stream_send_failures: self.stream_send_failures.load(Ordering::Relaxed),
            stream_queue_depth: self.stream_queue_depth.load(Ordering::Relaxed),
            stream_queue_drops: self.stream_queue_drops.load(Ordering::Relaxed),
        }
    }
}

pub(super) fn lane_delta(
    previous: Option<&VisualizationLaneSnapshot>,
    current: &VisualizationLaneSnapshot,
) -> VisualizationLaneDelta {
    let previous_values = previous.map(|snapshot| value_index(&snapshot.values));
    let previous_profile = previous.map(|snapshot| value_index(&snapshot.profile_output_values));
    VisualizationLaneDelta {
        scope: current.scope,
        revision: current.revision,
        generated_at: current.generated_at.clone(),
        grand_master: current.grand_master,
        blackout: current.blackout,
        preload: current.preload,
        values: changed_values(previous_values.as_ref(), &current.values),
        removed_values: removed_values(previous_values.as_ref(), &current.values),
        dynamic_stack: previous
            .filter(|previous| previous.dynamic_stack == current.dynamic_stack)
            .map_or_else(|| Some(current.dynamic_stack.clone()), |_| None),
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
    let mut removed = previous
        .into_iter()
        .flat_map(HashMap::keys)
        .filter(|(fixture_id, attribute)| !current.contains(&(*fixture_id, attribute.as_str())))
        .map(|(fixture_id, attribute)| VisualizationValueKey {
            fixture_id: *fixture_id,
            attribute: attribute.clone(),
        })
        .collect::<Vec<_>>();
    removed.sort_by(|left, right| {
        left.fixture_id
            .cmp(&right.fixture_id)
            .then_with(|| left.attribute.cmp(&right.attribute))
    });
    removed
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
            profile_visualization_values: Arc::new(HashMap::new()),
            patched_slots: HashMap::new(),
            revision,
            routes: Arc::<[OutputRoute]>::from([]),
            automatic_playback_transitions: Vec::new(),
        }
    }

    fn scope(show_id: Uuid) -> VisualizationScope {
        VisualizationScope {
            show_id: Some(show_id),
        }
    }

    fn snapshot(
        scope: VisualizationScope,
        dynamic_stack: Vec<light_wire::v2::visualization::VisualizationDynamicStackEntry>,
    ) -> VisualizationLaneSnapshot {
        VisualizationLaneSnapshot {
            scope,
            revision: 10,
            generated_at: "2026-07-27T00:00:00Z".into(),
            grand_master: 1.0,
            blackout: false,
            preload: false,
            values: Vec::new(),
            dynamic_stack,
            profile_output_values: Vec::new(),
        }
    }

    fn dynamic_stack_entry() -> light_wire::v2::visualization::VisualizationDynamicStackEntry {
        serde_json::from_value(serde_json::json!({
            "fixture_id": Uuid::new_v4(),
            "attribute": "intensity",
            "entry_type": "dynamic",
            "priority": 0,
            "changed_at_millis": 1,
            "source": "Dynamic",
            "dynamic_id": null,
            "pool_number": 1,
            "name": "Pulse",
            "runtime_instance_id": null,
            "controller_id": null,
            "lane_id": null,
            "size": 1.0,
            "activation_mix": 1.0,
            "paused": false,
            "hidden": false,
            "pending": false,
            "winning": true,
            "value": null,
            "resolved_value": null
        }))
        .expect("test Dynamic stack entry is valid")
    }

    #[test]
    fn retains_only_the_latest_complete_frame() {
        let hub = VisualizationFrameHub::default();
        let scope = scope(Uuid::new_v4());
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        hub.publish(&rendered(20), RenderOptions::default(), scope);
        hub.publish(&rendered(30), RenderOptions::default(), scope);

        let latest = hub.latest().expect("a frame was published");
        assert_eq!(latest.sequence, 3);
        assert_eq!(latest.show_revision, 30);
        assert_eq!(latest.scope, scope);
    }

    #[test]
    fn shares_one_projection_for_the_same_lane_and_source_frame() {
        let hub = VisualizationFrameHub::default();
        let scope = scope(Uuid::new_v4());
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        let source = hub.latest().unwrap();
        let builds = AtomicUsize::new(0);
        let build = |_| {
            builds.fetch_add(1, AtomicOrdering::Relaxed);
            Ok(VisualizationLaneSnapshot {
                scope,
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
            .projection(
                VisualizationProjectionKey::Normal {
                    include_dynamic_stack: false,
                },
                &source,
                build,
            )
            .unwrap();
        let second = hub
            .projection(
                VisualizationProjectionKey::Normal {
                    include_dynamic_stack: false,
                },
                &source,
                build,
            )
            .unwrap();

        assert!(Arc::ptr_eq(&first, &second));
        assert!(first.previous_source_sequence.is_none());
        assert_eq!(builds.load(AtomicOrdering::Relaxed), 1);
        assert_eq!(hub.metrics().projections, 1);
    }

    #[test]
    fn dynamic_stack_refresh_is_capped_at_fixture_sheet_cadence() {
        let hub = VisualizationFrameHub::default();
        let scope = scope(Uuid::new_v4());
        let key = VisualizationProjectionKey::Normal {
            include_dynamic_stack: true,
        };
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        let first_source = hub.latest().unwrap();
        hub.projection(key, &first_source, |refresh| {
            assert!(refresh);
            Ok(snapshot(scope, vec![dynamic_stack_entry()]))
        })
        .unwrap();

        hub.publish(&rendered(10), RenderOptions::default(), scope);
        let second_source = hub.latest().unwrap();
        let second = hub
            .projection(key, &second_source, |refresh| {
                assert!(!refresh);
                Ok(snapshot(scope, Vec::new()))
            })
            .unwrap();
        assert_eq!(second.snapshot.dynamic_stack.len(), 1);
        assert!(second.delta.dynamic_stack.is_none());

        std::thread::sleep(DYNAMIC_STACK_PUBLICATION_INTERVAL);
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        let third_source = hub.latest().unwrap();
        let third = hub
            .projection(key, &third_source, |refresh| {
                assert!(refresh);
                Ok(snapshot(scope, Vec::new()))
            })
            .unwrap();
        assert_eq!(third.delta.dynamic_stack, Some(Vec::new()));
    }

    #[test]
    fn sampler_publishes_one_shared_source_only_while_subscribed() {
        let hub = VisualizationFrameHub::default();
        let scope = scope(Uuid::new_v4());
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        hub.sample_latest();
        assert!(hub.sampled().is_none());

        hub.change_subscribers(VisualizationLane::Normal, 1);
        hub.sample_latest();
        assert_eq!(hub.sampled().unwrap().sequence, 1);

        hub.publish(&rendered(20), RenderOptions::default(), scope);
        assert_eq!(hub.sampled().unwrap().sequence, 1);
        hub.sample_latest();
        assert_eq!(hub.sampled().unwrap().sequence, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn sampler_parks_without_subscribers_and_resumes_on_first_claim() {
        let hub = Arc::new(VisualizationFrameHub::default());
        let cancellation = CancellationToken::new();
        let sampler = tokio::spawn(Arc::clone(&hub).run_sampler(cancellation.clone()));
        let scope = scope(Uuid::new_v4());
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(hub.sampled().is_none());

        hub.change_subscribers(VisualizationLane::Normal, 1);
        tokio::task::yield_now().await;
        assert_eq!(hub.sampled().unwrap().sequence, 1);

        hub.change_subscribers(VisualizationLane::Normal, -1);
        hub.publish(&rendered(20), RenderOptions::default(), scope);
        tokio::time::advance(Duration::from_secs(1)).await;
        assert_eq!(hub.sampled().unwrap().sequence, 1);

        cancellation.cancel();
        sampler.await.unwrap().unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn sampler_notifies_waiters_on_each_new_shared_sample() {
        let hub = Arc::new(VisualizationFrameHub::default());
        let cancellation = CancellationToken::new();
        let sampler = tokio::spawn(Arc::clone(&hub).run_sampler(cancellation.clone()));
        let scope = scope(Uuid::new_v4());
        hub.publish(&rendered(10), RenderOptions::default(), scope);

        let first_waiter = {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move { hub.wait_for_sample_after(0).await.sequence })
        };
        hub.change_subscribers(VisualizationLane::Normal, 1);
        assert_eq!(first_waiter.await.unwrap(), 1);

        let second_waiter = {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move { hub.wait_for_sample_after(1).await.sequence })
        };
        hub.publish(&rendered(20), RenderOptions::default(), scope);
        tokio::task::yield_now().await;
        assert!(!second_waiter.is_finished());

        tokio::time::advance(VISUALIZATION_SOURCE_SAMPLE_INTERVAL).await;
        assert_eq!(second_waiter.await.unwrap(), 2);

        hub.change_subscribers(VisualizationLane::Normal, -1);
        cancellation.cancel();
        sampler.await.unwrap().unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn sampler_uses_the_next_source_when_a_cadence_deadline_precedes_publication() {
        let hub = Arc::new(VisualizationFrameHub::default());
        let cancellation = CancellationToken::new();
        let sampler = tokio::spawn(Arc::clone(&hub).run_sampler(cancellation.clone()));
        let scope = scope(Uuid::new_v4());
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        hub.change_subscribers(VisualizationLane::Normal, 1);
        assert_eq!(hub.wait_for_sample_after(0).await.sequence, 1);
        tokio::task::yield_now().await;

        let next_waiter = {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move { hub.wait_for_sample_after(1).await.sequence })
        };
        tokio::time::advance(VISUALIZATION_SOURCE_SAMPLE_INTERVAL).await;
        tokio::task::yield_now().await;
        assert!(!next_waiter.is_finished());

        hub.publish(&rendered(20), RenderOptions::default(), scope);
        assert_eq!(next_waiter.await.unwrap(), 2);

        hub.change_subscribers(VisualizationLane::Normal, -1);
        cancellation.cancel();
        sampler.await.unwrap().unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn reactivated_sampler_starts_a_fresh_shared_cadence() {
        let hub = Arc::new(VisualizationFrameHub::default());
        let cancellation = CancellationToken::new();
        let sampler = tokio::spawn(Arc::clone(&hub).run_sampler(cancellation.clone()));
        let scope = scope(Uuid::new_v4());
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        hub.change_subscribers(VisualizationLane::Normal, 1);
        assert_eq!(hub.wait_for_sample_after(0).await.sequence, 1);
        tokio::task::yield_now().await;

        tokio::time::advance(VISUALIZATION_SOURCE_SAMPLE_INTERVAL / 2).await;
        hub.change_subscribers(VisualizationLane::Normal, -1);
        hub.publish(&rendered(20), RenderOptions::default(), scope);
        hub.change_subscribers(VisualizationLane::Normal, 1);
        assert_eq!(hub.wait_for_sample_after(1).await.sequence, 2);
        tokio::task::yield_now().await;

        hub.publish(&rendered(30), RenderOptions::default(), scope);
        let next_waiter = {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move { hub.wait_for_sample_after(2).await.sequence })
        };
        tokio::time::advance(VISUALIZATION_SOURCE_SAMPLE_INTERVAL - Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert!(!next_waiter.is_finished());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert_eq!(next_waiter.await.unwrap(), 3);

        hub.change_subscribers(VisualizationLane::Normal, -1);
        cancellation.cancel();
        sampler.await.unwrap().unwrap();
    }

    #[test]
    fn preload_projection_uses_its_own_authoritative_source_identity_and_timestamp() {
        let hub = VisualizationFrameHub::default();
        let scope = scope(Uuid::new_v4());
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        let source = hub.latest().unwrap();
        let stale_timestamp = "2020-01-01T00:00:00Z";

        let projection = hub
            .projection(
                VisualizationProjectionKey::Preload {
                    session_id: Uuid::new_v4(),
                    include_dynamic_stack: false,
                },
                &source,
                |_| {
                    Ok(VisualizationLaneSnapshot {
                        scope,
                        revision: 10,
                        generated_at: stale_timestamp.into(),
                        grand_master: 1.0,
                        blackout: false,
                        preload: true,
                        values: Vec::new(),
                        dynamic_stack: Vec::new(),
                        profile_output_values: Vec::new(),
                    })
                },
            )
            .unwrap();

        assert_eq!(projection.lane_source_sequence, 1);
        assert_ne!(projection.snapshot.generated_at, stale_timestamp);
        assert_eq!(
            projection.snapshot.generated_at,
            chrono::DateTime::<chrono::Utc>::from(projection.source_generated_at).to_rfc3339()
        );
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
        let key = VisualizationProjectionKey::Preload {
            session_id: Uuid::new_v4(),
            include_dynamic_stack: false,
        };
        let scope = scope(Uuid::new_v4());
        hub.change_projection_claim(key, 1);
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        let source = hub.latest().unwrap();
        let builds = AtomicUsize::new(0);
        let mut build = |_| {
            builds.fetch_add(1, AtomicOrdering::Relaxed);
            Ok(VisualizationLaneSnapshot {
                scope,
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
    fn removed_values_have_deterministic_fixture_and_attribute_order() {
        let scope = scope(Uuid::new_v4());
        let fixture_a = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
        let fixture_b = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
        let previous = VisualizationLaneSnapshot {
            scope,
            revision: 1,
            generated_at: "2026-07-27T00:00:00Z".into(),
            grand_master: 1.0,
            blackout: false,
            preload: false,
            values: vec![
                VisualizationValue {
                    fixture_id: fixture_b,
                    attribute: "tilt".into(),
                    value: ProgrammingPreloadAttributeValue::Normalized(0.5),
                },
                VisualizationValue {
                    fixture_id: fixture_a,
                    attribute: "pan".into(),
                    value: ProgrammingPreloadAttributeValue::Normalized(0.5),
                },
                VisualizationValue {
                    fixture_id: fixture_a,
                    attribute: "intensity".into(),
                    value: ProgrammingPreloadAttributeValue::Normalized(0.5),
                },
            ],
            dynamic_stack: Vec::new(),
            profile_output_values: Vec::new(),
        };
        let current = VisualizationLaneSnapshot {
            values: Vec::new(),
            revision: 2,
            ..previous.clone()
        };

        let delta = lane_delta(Some(&previous), &current);

        assert_eq!(
            delta.removed_values,
            vec![
                VisualizationValueKey {
                    fixture_id: fixture_a,
                    attribute: "intensity".into(),
                },
                VisualizationValueKey {
                    fixture_id: fixture_a,
                    attribute: "pan".into(),
                },
                VisualizationValueKey {
                    fixture_id: fixture_b,
                    attribute: "tilt".into(),
                },
            ]
        );
    }

    #[test]
    fn projection_work_cannot_block_latest_frame_publication() {
        let hub = Arc::new(VisualizationFrameHub::default());
        let scope = scope(Uuid::new_v4());
        hub.publish(&rendered(10), RenderOptions::default(), scope);
        let source = hub.latest().unwrap();
        let (projection_started_tx, projection_started_rx) = mpsc::channel();
        let (release_projection_tx, release_projection_rx) = mpsc::channel();
        let projection_hub = Arc::clone(&hub);
        let projection = thread::spawn(move || {
            projection_hub
                .projection(
                    VisualizationProjectionKey::Normal {
                        include_dynamic_stack: false,
                    },
                    &source,
                    |_| {
                        projection_started_tx.send(()).unwrap();
                        release_projection_rx.recv().unwrap();
                        Ok(VisualizationLaneSnapshot {
                            scope,
                            revision: 10,
                            generated_at: "2026-07-27T00:00:00Z".into(),
                            grand_master: 1.0,
                            blackout: false,
                            preload: false,
                            values: Vec::new(),
                            dynamic_stack: Vec::new(),
                            profile_output_values: Vec::new(),
                        })
                    },
                )
                .unwrap();
        });
        projection_started_rx.recv().unwrap();

        let (published_tx, published_rx) = mpsc::channel();
        let publisher_hub = Arc::clone(&hub);
        thread::spawn(move || {
            publisher_hub.publish(&rendered(20), RenderOptions::default(), scope);
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
