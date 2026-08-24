use super::*;

const MEASUREMENT_CAPACITY: usize = 2_048;
const OUTPUT_EPOCH_CAPACITY: usize = 2_048;
const CAUSAL_ORIGIN_TTL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Serialize)]
pub(super) struct ActionTimingProjection {
    pub(super) action_id: u64,
    pub(super) source: String,
    pub(super) action: String,
    pub(super) request_id: String,
    pub(super) received_output_tick: u64,
    pub(super) acknowledged_output_tick: u64,
    pub(super) first_output_tick: Option<u64>,
    pub(super) acknowledgement_wall_micros: u64,
    pub(super) first_output_wall_micros: Option<u64>,
    pub(super) output_frame_hz: u16,
    pub(super) budget_ticks: u64,
    pub(super) requires_output_frame: bool,
    pub(super) acknowledgement_within_budget: bool,
    pub(super) output_within_budget: Option<bool>,
    pub(super) succeeded: bool,
}

#[derive(Clone)]
pub(super) struct ActionTimingResource {
    inner: Arc<ActionTimingInner>,
}

struct ActionTimingInner {
    next_action_id: AtomicU64,
    output_tick: AtomicU64,
    visibility_epoch: AtomicU64,
    measurements: Mutex<VecDeque<ActionMeasurement>>,
    output_epochs: Mutex<VecDeque<OutputEpoch>>,
    causal_origins: Mutex<HashMap<(String, String), CausalOrigin>>,
    output_wake: Arc<tokio::sync::Notify>,
}

struct CausalOrigin {
    action_id: u64,
    source: String,
    received_output_tick: u64,
    received_at: Instant,
    output_frame_hz: u16,
    osc_feedback: OscActionFeedback,
}

#[derive(Clone)]
pub(super) struct OscActionFeedback {
    pub(super) path: String,
    pub(super) target: SocketAddr,
}

struct ActionMeasurement {
    action_id: u64,
    source: String,
    action: String,
    request_id: String,
    received_output_tick: u64,
    acknowledged_output_tick: u64,
    received_at: Instant,
    acknowledged_at: Instant,
    output_frame_hz: u16,
    visibility_epoch: Option<u64>,
    requires_output_frame: bool,
    succeeded: bool,
}

#[derive(Clone, Copy)]
struct OutputEpoch {
    visibility_epoch: u64,
    output_tick: u64,
    completed_at: Instant,
}

pub(super) struct ActionTimingReceipt {
    resource: ActionTimingResource,
    action_id: u64,
    source: String,
    action: String,
    request_id: String,
    received_output_tick: u64,
    received_at: Instant,
    output_frame_hz: u16,
    may_change_output: bool,
    osc_feedback: Option<OscActionFeedback>,
}

#[derive(Clone, Copy)]
pub(super) struct OutputRenderTiming {
    output_tick: u64,
    visibility_epoch: u64,
}

impl Default for ActionTimingResource {
    fn default() -> Self {
        Self {
            inner: Arc::new(ActionTimingInner {
                next_action_id: AtomicU64::new(0),
                output_tick: AtomicU64::new(0),
                visibility_epoch: AtomicU64::new(0),
                measurements: Mutex::new(VecDeque::with_capacity(MEASUREMENT_CAPACITY)),
                output_epochs: Mutex::new(VecDeque::with_capacity(OUTPUT_EPOCH_CAPACITY)),
                causal_origins: Mutex::new(HashMap::new()),
                output_wake: Arc::new(tokio::sync::Notify::new()),
            }),
        }
    }
}

impl ActionTimingResource {
    pub(super) fn begin(
        &self,
        source: impl Into<String>,
        action: impl Into<String>,
        request_id: impl Into<String>,
        output_frame_hz: u16,
        may_change_output: bool,
    ) -> ActionTimingReceipt {
        ActionTimingReceipt {
            resource: self.clone(),
            action_id: self.inner.next_action_id.fetch_add(1, Ordering::Relaxed) + 1,
            source: source.into(),
            action: action.into(),
            request_id: request_id.into(),
            received_output_tick: self.inner.output_tick.load(Ordering::Acquire),
            received_at: Instant::now(),
            output_frame_hz: output_frame_hz.max(1),
            may_change_output,
            osc_feedback: None,
        }
    }

    pub(super) fn output_wake(&self) -> Arc<tokio::sync::Notify> {
        Arc::clone(&self.inner.output_wake)
    }

    pub(super) fn begin_causal_origin(
        &self,
        session_id: impl Into<String>,
        source: impl Into<String>,
        request_id: impl Into<String>,
        output_frame_hz: u16,
        osc_feedback: OscActionFeedback,
    ) {
        let now = Instant::now();
        let key = (session_id.into(), request_id.into());
        let mut origins = self.inner.causal_origins.lock();
        origins.retain(|_, origin| now.duration_since(origin.received_at) <= CAUSAL_ORIGIN_TTL);
        origins.entry(key).or_insert_with(|| CausalOrigin {
            action_id: self.inner.next_action_id.fetch_add(1, Ordering::Relaxed) + 1,
            source: source.into(),
            received_output_tick: self.inner.output_tick.load(Ordering::Acquire),
            received_at: now,
            output_frame_hz: output_frame_hz.max(1),
            osc_feedback,
        });
    }

    pub(super) fn begin_or_resume(
        &self,
        session_id: impl Into<String>,
        fallback_source: impl Into<String>,
        action: impl Into<String>,
        request_id: impl Into<String>,
        output_frame_hz: u16,
        may_change_output: bool,
    ) -> ActionTimingReceipt {
        let session_id = session_id.into();
        let request_id = request_id.into();
        let action = action.into();
        let now = Instant::now();
        let origin = {
            let mut origins = self.inner.causal_origins.lock();
            origins.retain(|_, origin| now.duration_since(origin.received_at) <= CAUSAL_ORIGIN_TTL);
            origins.remove(&(session_id, request_id.clone()))
        };
        match origin {
            Some(origin) => ActionTimingReceipt {
                resource: self.clone(),
                action_id: origin.action_id,
                source: origin.source,
                action,
                request_id,
                received_output_tick: origin.received_output_tick,
                received_at: origin.received_at,
                output_frame_hz: origin.output_frame_hz,
                may_change_output,
                osc_feedback: Some(origin.osc_feedback),
            },
            None => self.begin(
                fallback_source,
                action,
                request_id,
                output_frame_hz,
                may_change_output,
            ),
        }
    }

    pub(super) fn begin_output_render(&self) -> OutputRenderTiming {
        OutputRenderTiming {
            output_tick: self.inner.output_tick.fetch_add(1, Ordering::AcqRel) + 1,
            visibility_epoch: self.inner.visibility_epoch.load(Ordering::Acquire),
        }
    }

    pub(super) fn complete_output_render(&self, timing: OutputRenderTiming) {
        let mut epochs = self.inner.output_epochs.lock();
        if epochs
            .back()
            .is_some_and(|last| last.visibility_epoch >= timing.visibility_epoch)
        {
            return;
        }
        push_bounded(
            &mut epochs,
            OutputEpoch {
                visibility_epoch: timing.visibility_epoch,
                output_tick: timing.output_tick,
                completed_at: Instant::now(),
            },
            OUTPUT_EPOCH_CAPACITY,
        );
    }

    pub(super) fn snapshot(&self) -> Vec<ActionTimingProjection> {
        let epochs = self
            .inner
            .output_epochs
            .lock()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        self.inner
            .measurements
            .lock()
            .iter()
            .map(|measurement| measurement.projection(&epochs))
            .collect()
    }
}

impl ActionTimingReceipt {
    pub(super) fn acknowledge(self, succeeded: bool) -> ActionTimingProjection {
        self.acknowledge_with_osc_feedback(succeeded).0
    }

    pub(super) fn acknowledge_with_osc_feedback(
        self,
        succeeded: bool,
    ) -> (ActionTimingProjection, Option<OscActionFeedback>) {
        let acknowledged_at = Instant::now();
        let acknowledged_output_tick = self.resource.inner.output_tick.load(Ordering::Acquire);
        let visibility_epoch = (succeeded && self.may_change_output).then(|| {
            let visibility_epoch = self
                .resource
                .inner
                .visibility_epoch
                .fetch_add(1, Ordering::AcqRel)
                + 1;
            self.resource.inner.output_wake.notify_one();
            visibility_epoch
        });
        let measurement = ActionMeasurement {
            action_id: self.action_id,
            source: self.source,
            action: self.action,
            request_id: self.request_id,
            received_output_tick: self.received_output_tick,
            acknowledged_output_tick,
            received_at: self.received_at,
            acknowledged_at,
            output_frame_hz: self.output_frame_hz,
            visibility_epoch,
            requires_output_frame: self.may_change_output,
            succeeded,
        };
        let projection = measurement.projection(&[]);
        push_bounded(
            &mut self.resource.inner.measurements.lock(),
            measurement,
            MEASUREMENT_CAPACITY,
        );
        (projection, self.osc_feedback)
    }
}

impl ActionMeasurement {
    fn projection(&self, epochs: &[OutputEpoch]) -> ActionTimingProjection {
        let output = self.visibility_epoch.and_then(|visibility_epoch| {
            epochs
                .iter()
                .find(|epoch| epoch.visibility_epoch >= visibility_epoch)
        });
        let budget_ticks = action_budget_ticks(self.output_frame_hz);
        let acknowledgement_wall_micros =
            duration_micros(self.acknowledged_at.duration_since(self.received_at));
        let acknowledgement_within_budget = self
            .acknowledged_output_tick
            .saturating_sub(self.received_output_tick)
            <= budget_ticks;
        let first_output_wall_micros = output
            .map(|epoch| duration_micros(epoch.completed_at.duration_since(self.received_at)));
        ActionTimingProjection {
            action_id: self.action_id,
            source: self.source.clone(),
            action: self.action.clone(),
            request_id: self.request_id.clone(),
            received_output_tick: self.received_output_tick,
            acknowledged_output_tick: self.acknowledged_output_tick,
            first_output_tick: output.map(|epoch| epoch.output_tick),
            acknowledgement_wall_micros,
            first_output_wall_micros,
            output_frame_hz: self.output_frame_hz,
            budget_ticks,
            requires_output_frame: self.requires_output_frame,
            acknowledgement_within_budget,
            output_within_budget: self.visibility_epoch.and_then(|_| {
                output.map(|epoch| {
                    epoch.output_tick.saturating_sub(self.received_output_tick) <= budget_ticks
                })
            }),
            succeeded: self.succeeded,
        }
    }
}

pub(super) const fn action_budget_ticks(output_frame_hz: u16) -> u64 {
    if output_frame_hz <= 60 { 2 } else { 4 }
}

fn duration_micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

fn push_bounded<T>(queue: &mut VecDeque<T>, value: T, capacity: usize) {
    if queue.len() == capacity {
        queue.pop_front();
    }
    queue.push_back(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_budget_switches_only_above_sixty_hertz() {
        assert_eq!(action_budget_ticks(44), 2);
        assert_eq!(action_budget_ticks(60), 2);
        assert_eq!(action_budget_ticks(61), 4);
        assert_eq!(action_budget_ticks(100), 4);
        assert_eq!(action_budget_ticks(120), 4);
    }

    #[test]
    fn irregular_wall_cadence_does_not_fail_an_on_time_output_tick() {
        let completed_at = Instant::now();
        let received_at = completed_at - Duration::from_millis(79);
        let measurement = ActionMeasurement {
            action_id: 1,
            source: "websocket".into(),
            action: "command_execute".into(),
            request_id: "irregular-cadence".into(),
            received_output_tick: 41,
            acknowledged_output_tick: 42,
            received_at,
            acknowledged_at: received_at + Duration::from_millis(70),
            output_frame_hz: 44,
            visibility_epoch: Some(1),
            requires_output_frame: true,
            succeeded: true,
        };
        let completed = measurement.projection(&[OutputEpoch {
            visibility_epoch: 1,
            output_tick: 43,
            completed_at,
        }]);

        assert_eq!(completed.acknowledgement_wall_micros, 70_000);
        assert_eq!(completed.first_output_wall_micros, Some(79_000));
        assert!(completed.acknowledgement_within_budget);
        assert_eq!(completed.output_within_budget, Some(true));
    }

    #[test]
    fn short_wall_elapsed_time_does_not_hide_a_late_output_tick() {
        let received_at = Instant::now();
        let measurement = ActionMeasurement {
            action_id: 1,
            source: "websocket".into(),
            action: "command_execute".into(),
            request_id: "late-tick".into(),
            received_output_tick: 41,
            acknowledged_output_tick: 42,
            received_at,
            acknowledged_at: received_at + Duration::from_millis(1),
            output_frame_hz: 44,
            visibility_epoch: Some(1),
            requires_output_frame: true,
            succeeded: true,
        };
        let completed = measurement.projection(&[OutputEpoch {
            visibility_epoch: 1,
            output_tick: 44,
            completed_at: received_at + Duration::from_millis(2),
        }]);

        assert_eq!(completed.first_output_wall_micros, Some(2_000));
        assert_eq!(completed.output_within_budget, Some(false));
    }

    #[test]
    fn output_epoch_is_credited_only_to_a_render_that_started_after_acknowledgement() {
        let timing = ActionTimingResource::default();
        let during_render = timing.begin_output_render();
        let receipt = timing.begin("websocket", "values", "values-1", 60, true);
        let immediate = receipt.acknowledge(true);
        assert_eq!(immediate.first_output_tick, None);

        timing.complete_output_render(during_render);
        assert_eq!(timing.snapshot()[0].first_output_tick, None);

        let following_render = timing.begin_output_render();
        timing.complete_output_render(following_render);
        let completed = &timing.snapshot()[0];
        assert_eq!(completed.first_output_tick, Some(2));
        assert_eq!(completed.output_within_budget, Some(true));
        assert!(completed.requires_output_frame);
    }

    #[test]
    fn projection_only_actions_do_not_require_an_output_frame() {
        let timing = ActionTimingResource::default();
        timing
            .begin("websocket", "selection", "selection-1", 44, false)
            .acknowledge(true);
        let completed = &timing.snapshot()[0];
        assert_eq!(completed.first_output_tick, None);
        assert_eq!(completed.output_within_budget, None);
        assert!(!completed.requires_output_frame);
        assert!(completed.acknowledgement_within_budget);
    }

    #[test]
    fn websocket_action_resumes_only_the_matching_authenticated_origin() {
        let timing = ActionTimingResource::default();
        timing.begin_causal_origin(
            "session-a",
            "osc",
            "encoder-1",
            60,
            OscActionFeedback {
                path: "main".into(),
                target: "127.0.0.1:9000".parse().unwrap(),
            },
        );
        let other_session =
            timing.begin_or_resume("session-b", "websocket", "values", "encoder-1", 60, true);
        assert_eq!(other_session.source, "websocket");
        other_session.acknowledge(true);

        let resumed =
            timing.begin_or_resume("session-a", "websocket", "values", "encoder-1", 60, true);
        assert_eq!(resumed.source, "osc");
        resumed.acknowledge(true);

        let measurements = timing.snapshot();
        assert_eq!(measurements.len(), 2);
        assert_eq!(measurements[0].source, "websocket");
        assert_eq!(measurements[1].source, "osc");
        assert_eq!(measurements[1].request_id, "encoder-1");
    }
}
#[tokio::test]
async fn successful_output_action_wakes_the_scheduler() {
    let timing = ActionTimingResource::default();
    let wake = timing.output_wake();
    let receipt = timing.begin("websocket", "preload_lifecycle", "wake", 44, true);

    receipt.acknowledge(true);

    tokio::time::timeout(Duration::from_millis(10), wake.notified())
        .await
        .expect("successful output action leaves a scheduler wake permit");
}
