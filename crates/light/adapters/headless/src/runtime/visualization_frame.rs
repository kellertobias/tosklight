//! Latest-value publication from the output boundary to visualization transports.

use arc_swap::ArcSwapOption;
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

/// Source state sampled by the same successful transaction as its semantic output. Readers
/// borrow this retained frame instead of joining values to a newer mutable runtime/catalogue.
#[derive(Debug)]
pub(in crate::runtime) struct FrameDynamicSources {
    pub(in crate::runtime) sample_boundary: Option<light_dynamics::DynamicSampleBoundary>,
    pub(in crate::runtime) runtime: light_dynamics::DynamicRuntimeSnapshot,
    pub(in crate::runtime) samples: Vec<light_dynamics::DynamicRuntimeSample>,
    pub(in crate::runtime) origins: Arc<super::dynamic_source_origins::DynamicSourceOrigins>,
    pub(in crate::runtime) programmer_values:
        Arc<Vec<(Uuid, i16, light_dynamics::DynamicAddressValue)>>,
    pub(in crate::runtime) cue_values: Arc<[light_playback::ActiveCueDynamicValue]>,
    /// Present only when sampling already evaluated the captured ordinary source. Publishing
    /// source evidence must never trigger another resolution merely to fill an observer cache.
    /// Holds the finalized frame; the programmed parameters are read through `raw_values`.
    pub(in crate::runtime) ordinary: Option<light_engine::FrameValues>,
    /// TL-659: the earliest start this frame carries (`CommittedDynamicOutput`).
    pub(in crate::runtime) change_lead_start: Option<i64>,
}

/// Carries one completed render and its matching sources to the Hold/publication boundary.
#[derive(Debug)]
pub(in crate::runtime) struct RenderedSemanticFrame {
    pub(in crate::runtime) rendered: RenderResult,
    pub(in crate::runtime) options: RenderOptions,
    pub(in crate::runtime) dynamics: Option<Arc<FrameDynamicSources>>,
}

impl RenderedSemanticFrame {
    /// TL-659: the earliest Cue or Dynamic start this frame carries, if any.
    pub(in crate::runtime) fn change_lead_start(&self) -> Option<i64> {
        self.dynamics.as_ref()?.change_lead_start
    }

    /// A direct engine render supplies no Dynamic source proof. Never fill this absence from
    /// the current global runtime, which may already belong to another output frame.
    #[cfg(test)]
    pub(in crate::runtime) fn untraced(rendered: RenderResult, options: RenderOptions) -> Self {
        Self {
            rendered,
            options,
            dynamics: None,
        }
    }
}

#[path = "visualization_frame/projection_key.rs"]
mod projection_key;
pub(super) use projection_key::VisualizationProjectionKey;

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
    pub(super) generation: u64,
    pub(super) sampled_at: chrono::DateTime<chrono::Utc>,
    pub(super) source_snapshot: Arc<light_engine::EngineSnapshot>,
    pub(super) tracking: Arc<light_engine::TrackedInputFrame>,
    pub(super) points: Arc<light_engine::Pooled<Vec<light_engine::ResolvedPointPose>>>,
    pub(super) mounts: Arc<light_engine::Pooled<light_engine::FixtureMountFrame>>,
    pub(super) generated_at: SystemTime,
    pub(super) scope: VisualizationScope,
    pub(super) show_revision: u64,
    pub(super) options: RenderOptions,
    pub(super) values: light_engine::FrameValues,
    pub(super) dynamics: Option<Arc<FrameDynamicSources>>,
    pub(super) physical: Arc<light_engine::Pooled<light_engine::PhysicalForwardFrame>>,
    pub(super) profile_visualization_values:
        Arc<light_engine::Pooled<light_engine::ResolvedValues>>,
}

impl PublishedVisualizationFrame {
    pub(super) fn identity(&self) -> light_wire::v2::output_control::OutputFrameIdentity {
        light_wire::v2::output_control::OutputFrameIdentity {
            generation: self.generation,
            sequence: self.sequence,
            sampled_at: self.sampled_at.to_rfc3339(),
            tracking: Some(light_wire::v2::output_control::OutputTrackingIdentity {
                show_id: self.tracking.show_id.map(|id| id.0),
                configuration_generation: self.tracking.configuration_generation,
                point_generation: self.tracking.point_generation,
                source_generation: self.tracking.source_generation,
                accepted_sequence: self.tracking.accepted_sequence,
                sampled_at_millis: self.tracking.sampled_at_millis,
            }),
        }
    }
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
    /// Publish `completed`. The frame it replaces (often the last reference to its samples and
    /// values) is freed on `pool` when there is one (TL-639 round 6).
    pub(super) fn publish(
        &self,
        completed: &RenderedSemanticFrame,
        scope: VisualizationScope,
        pool: Option<&light_engine::parallel::OutputPool>,
    ) {
        let rendered = &completed.rendered;
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let previous = self.latest.swap(Some(Arc::new(PublishedVisualizationFrame {
            sequence,
            generation: rendered.generation,
            sampled_at: rendered.sampled_at,
            source_snapshot: Arc::clone(&rendered.source_snapshot),
            tracking: Arc::clone(&rendered.tracking),
            points: Arc::clone(&rendered.points),
            mounts: Arc::clone(&rendered.mounts),
            generated_at: SystemTime::now(),
            scope,
            show_revision: rendered.revision,
            options: completed.options,
            values: rendered.resolved_values.clone(),
            dynamics: completed.dynamics.clone(),
            physical: Arc::clone(&rendered.physical),
            profile_visualization_values: Arc::clone(&rendered.profile_visualization_values),
        })));
        self.source_notify.notify_one();
        if let (Some(pool), Some(previous)) = (pool, previous) {
            pool.drop_later(previous);
        }
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
#[path = "visualization_frame/tests.rs"]
mod tests;
