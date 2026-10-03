use super::*;
use crate::runtime::dynamic_snapshot_publication::{
    DynamicSnapshotPublication, RetainedFrameCapture,
};
use light_core::{AttributeKey, FixtureId, ManualClock, SessionId};
use light_dynamics::{
    ActivationPolicy, DynamicControl, DynamicController, DynamicControllerSource,
    DynamicDefinition, DynamicSpeed, DynamicSpeedTransport, DynamicStartRequest,
    DynamicTargetScope, Rational, ScalarSourceResolver, SpeedGroup, TimedDynamicControl,
};
use light_engine::Engine;
use light_programmer::{
    PreloadPlaybackQueueAction, PreloadPlaybackQueueSurface, ProgrammerRegistry,
};
use std::time::{Duration, Instant};

fn capacity(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn limits() -> PendingHistoryLimits {
    PendingHistoryLimits {
        attempts: capacity(16),
        cold_changes: capacity(16),
        controls: capacity(64),
    }
}
fn definition() -> DynamicDefinition {
    let mut definition: DynamicDefinition = serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Pending history",
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "intensity", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type": "current"}, "interpolation": "linear"},
                {"position": 0.5, "source": {"type": "current"}, "interpolation": "linear"}
            ]},
            "max_min": {"minimum": {"type": "value", "value": 0.0},
                "maximum": {"type": "value", "value": 1.0}, "function": "sinus"},
            "middle_amplitude": {"middle": {"type": "current"}, "amplitude": 0.5, "function": "sinus"},
            "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
        }],
        "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.0,
            "span_degrees": 0.0, "block_size": 1, "repeats": 1,
            "wings": false, "anchors_degrees": []},
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    })).unwrap();
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    definition
}
fn transports() -> [DynamicSpeedTransport; 5] {
    [DynamicSpeedTransport {
        effective_bpm: 60.,
        phase_origin_millis: 0,
        phase_reference_millis: 0,
        beat_phase: 0.,
        phase_advancing: true,
    }; 5]
}
struct Current(f32);
impl ScalarSourceResolver for Current {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        Some(self.0)
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
fn sample(runtime: &mut DynamicRuntime, at: u64, value: f32) -> f32 {
    let samples = runtime.sample_all(at, 40, &transports(), &Current(value));
    assert_eq!(samples.len(), 1);
    let mut values = Vec::new();
    samples[0]
        .expression
        .visit_legacy_contributions(|_, value, _| values.push(value));
    assert!(!values.is_empty());
    values[0]
}
struct Evaluator {
    current: f32,
    fail: bool,
    panic: bool,
    calls: usize,
    seen: Vec<(PreloadBranch, bool)>,
}
impl Evaluator {
    fn new(current: f32) -> Self {
        Self {
            current,
            fail: false,
            panic: false,
            calls: 0,
            seen: vec![],
        }
    }
}
impl PendingAttemptEvaluator<f32> for Evaluator {
    fn evaluate(
        &mut self,
        key: PendingEpisodeKey,
        input: &RetainedInputCapture,
        runtime: &mut DynamicRuntime,
        origins: &mut DynamicSourceOrigins,
    ) -> Result<f32, String> {
        self.calls += 1;
        self.seen
            .push((key.branch, runtime.snapshot().global_paused));
        let value = sample(
            runtime,
            input.frame.sampled_at().timestamp_millis() as u64,
            self.current,
        );
        if self.fail || self.panic {
            // Catalogue changes share the attempt boundary with actual sampled runtime history.
            *origins = DynamicSourceOrigins::default();
            if self.panic {
                panic!("injected Pending evaluator panic");
            }
            return Err("injected Pending sample failure".into());
        }
        Ok(value)
    }
}
struct Rig {
    clock: Arc<ManualClock>,
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    publication: DynamicSnapshotPublication,
    live: DynamicRuntime,
    start: Instant,
    selected: u64,
    key: PendingEpisodeKey,
    controller: Uuid,
    request: DynamicStartRequest,
}
impl Rig {
    fn new() -> Self {
        Self::with_activation_duration(0)
    }
    fn with_activation_duration(activation_duration_millis: u64) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(100).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        let programmer = programmers.start(session).id;
        let engine = Engine::new(programmers.clone());
        let definition = definition();
        engine
            .replace_snapshot(EngineSnapshot {
                dynamics: vec![definition.clone()].into(),
                ..Default::default()
            })
            .unwrap();
        let publication = DynamicSnapshotPublication::new(engine.snapshot());
        let mut live = DynamicRuntime::default();
        live.install_definitions([definition.clone()]).unwrap();
        let controller = Uuid::new_v4();
        let request = DynamicStartRequest {
            definition_id: definition.id,
            controller: DynamicController {
                id: controller,
                source: DynamicControllerSource::physical_playback(1),
                priority: 10,
                activated_at_millis: 0,
                size: 1.,
                speed_multiplier: 1.,
                phase_offset_degrees: 0.,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: vec![FixtureId::new()],
            },
            stage_positions: Default::default(),
            inherited_spatial_mapping: None,
            now_millis: 0,
            activation_delay_millis: 0,
            activation_duration_millis,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        };
        live.start(request.clone()).unwrap();
        publication
            .begin_retained_history(&mut live, &engine.snapshot(), capacity(64))
            .unwrap();
        Self {
            clock,
            engine,
            programmers,
            session,
            publication,
            live,
            start: Instant::now(),
            selected: 0,
            key: PendingEpisodeKey {
                activation: Uuid::new_v4(),
                programmer,
                branch: PreloadBranch::AfterRelease,
            },
            controller,
            request,
        }
    }
    fn seed(&self, branch: PreloadBranch) -> PendingHistorySeed {
        let mut live = self.live.fork_for_cold_install();
        let (cold, controls) = self
            .publication
            .begin_retained_history(&mut live, &self.engine.snapshot(), capacity(64))
            .unwrap();
        PendingHistorySeed {
            key: PendingEpisodeKey { branch, ..self.key },
            runtime: self.live.fork_for_pending_preview(),
            origins: Default::default(),
            snapshot: self.engine.snapshot(),
            position: PendingHistoryPosition {
                inputs: self.publication.input_capture_cursor().unwrap(),
                cold,
                controls,
            },
            live_sample: self.live.committed_sample_boundary(),
        }
    }
    fn control(&mut self, control: DynamicControl) {
        self.control_at(100, control);
    }
    fn control_at(&mut self, at_millis: u64, control: DynamicControl) {
        self.live
            .apply_recorded_control(TimedDynamicControl { at_millis, control })
            .unwrap();
    }
    fn capture(&mut self, sampled: bool) -> Arc<RetainedInputCapture> {
        let cursor = self.publication.input_capture_cursor().unwrap();
        let frame = self.engine.prepare_output_frame(Default::default());
        let at = frame.sampled_at().timestamp_millis() as u64;
        let capture = RetainedFrameCapture::select(
            frame,
            &self.publication,
            self.start + Duration::from_millis(self.selected * 40),
        );
        self.selected += 1;
        let marker = if sampled {
            sample(&mut self.live, at, 0.2);
            self.live.committed_sample_boundary()
        } else {
            None
        };
        self.publication.retain_accepted_input(
            &self.live,
            capture.retained().unwrap(),
            &[],
            &transports(),
            40,
            marker,
        );
        self.publication
            .input_captures_since(cursor)
            .unwrap()
            .remove(0)
    }
    fn controls(&self, from: DynamicControlCursor) -> DynamicControlBatch {
        self.live.controls_since(from).unwrap().unwrap()
    }
    fn cold(&mut self, control: DynamicControl) -> Arc<ColdGenerationEvent> {
        self.cold_definitions(control, None)
    }
    fn cold_definitions(
        &mut self,
        control: DynamicControl,
        definitions: Option<Vec<DynamicDefinition>>,
    ) -> Arc<ColdGenerationEvent> {
        let previous = self.engine.snapshot();
        let mut detached = self.live.fork_for_cold_install();
        let (from, _) = self
            .publication
            .begin_retained_history(&mut detached, &previous, capacity(64))
            .unwrap();
        let boundary = self.publication.cold_boundary(&self.live).unwrap();
        let mut destination = (*previous).clone();
        destination.revision += 1;
        if let Some(definitions) = definitions {
            destination.dynamics = definitions.into();
        }
        self.engine.replace_snapshot(destination).unwrap();
        let destination = self.engine.snapshot();
        self.live
            .install_definitions(destination.dynamics.iter().cloned())
            .unwrap();
        self.control(control);
        let prepared = boundary.prepare(previous, destination.clone(), &self.live);
        self.publication
            .installed_with_cold_event(destination, Some(prepared));
        self.publication
            .cold_generations_since(from)
            .unwrap()
            .remove(0)
    }
}
fn consume(
    rig: &Rig,
    history: &mut DetachedPendingHistory<f32>,
    inputs: &[Arc<RetainedInputCapture>],
    cold: &[Arc<ColdGenerationEvent>],
    evaluator: &mut Evaluator,
) -> PendingWindowOutcome {
    let controls = rig.controls(history.position().controls);
    let window = history
        .prepare_window(history.key, inputs, cold, &controls, limits())
        .unwrap();
    history.consume_window(window, evaluator)
}
fn copy_input(input: &RetainedInputCapture) -> RetainedInputCapture {
    RetainedInputCapture {
        from: input.from,
        to: input.to,
        cold: input.cold,
        controls: input.controls,
        frame: Arc::clone(&input.frame),
        baseline: Arc::clone(&input.baseline),
        speed_transports: input.speed_transports,
        rate: input.rate,
        live_sample: input.live_sample,
    }
}

#[test]
fn selected_same_time_attempts_run_once_and_control_tail_waits() {
    let mut rig = Rig::new();
    let mut history = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let first = rig.capture(true);
    let second = rig.capture(true);
    assert_ne!(first.live_sample, second.live_sample);
    assert_eq!(first.frame.sampled_at(), second.frame.sampled_at());
    rig.control(DynamicControl::GlobalPause(true)); // beyond second input's barrier
    let mut evaluator = Evaluator::new(0.8);
    let result = consume(
        &rig,
        &mut history,
        &[first.clone(), second.clone()],
        &[],
        &mut evaluator,
    );
    assert_eq!(result.consumed_attempts, 2);
    assert_eq!(result.successful_attempts, 2);
    assert!(result.stopped.is_none());
    assert!(!history.runtime.snapshot().global_paused);
    assert_eq!(history.position().controls, second.controls);
    assert_eq!(history.last_success().unwrap().value, 0.8);
    assert!(Arc::ptr_eq(
        &history.last_success().unwrap().capture,
        &second
    ));
    assert!(
        history
            .prepare_window(
                history.key,
                &[first],
                &[],
                &rig.controls(history.position().controls),
                limits()
            )
            .is_err()
    );
    assert_eq!(evaluator.calls, 2);
}

#[test]
fn failed_attempt_consumes_live_anchor_but_pause_retains_pending_success() {
    let mut rig = Rig::new();
    let mut history = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let mut evaluator = Evaluator::new(0.8);
    let first = rig.capture(true);
    consume(&rig, &mut history, &[first], &[], &mut evaluator);
    let prior = history.runtime.snapshot();
    let prior_marker = history.runtime.committed_sample_boundary();
    let previous_result = history.last_success().unwrap().capture.to;
    let failed = rig.capture(true);
    evaluator.current = 0.9;
    evaluator.fail = true;
    let result = consume(&rig, &mut history, &[failed.clone()], &[], &mut evaluator);
    assert_eq!(result.failed_attempts.len(), 1);
    assert_eq!(result.failed_attempts[0].input, failed.to);
    assert_eq!(
        result.failed_attempts[0].detail,
        "injected Pending sample failure"
    );
    assert_eq!(history.position().inputs, failed.to);
    assert_eq!(history.runtime.snapshot(), prior);
    assert_eq!(history.runtime.committed_sample_boundary(), prior_marker);
    assert_eq!(history.last_success().unwrap().capture.to, previous_result);
    rig.control(DynamicControl::GlobalPause(true)); // anchored to the consumed failed attempt
    let paused = rig.capture(true);
    evaluator.fail = false;
    evaluator.current = 0.1;
    let result = consume(&rig, &mut history, &[paused], &[], &mut evaluator);
    assert!(result.stopped.is_none());
    assert_eq!(result.successful_attempts, 1);
    assert_eq!(history.last_success().unwrap().value, 0.8);
    assert!(history.runtime.snapshot().global_paused);
    assert_eq!(
        sample(&mut rig.live, 100, 0.1),
        0.2,
        "Live retained its own Current"
    );
}

#[test]
fn failed_evaluation_keeps_newly_accepted_controls_and_panic_rolls_back() {
    let mut rig = Rig::new();
    let mut seed = rig.seed(PreloadBranch::AfterRelease);
    let entry = light_engine::ContributionFamilyEntry::new(
        light_engine::ContributionSourceId::programmer(rig.key.programmer),
        light_core::ProgrammerEditStamp {
            changed_at: chrono::DateTime::from_timestamp_millis(100).unwrap(),
            programmer_order: 1,
        },
        light_engine::ContributionFamilyFootprint::Whole,
        light_engine::ContributionFamilyRole::Authored,
    );
    seed.origins
        .bind_static_evidence(
            crate::runtime::dynamic_source_origins::DynamicSourceBinding::StaticBaseline {
                target: FixtureId::new(),
                owner: light_core::programming::ProgrammingOwner::Focus,
            },
            &Arc::new(light_engine::ContributionFamilyEvidence::new(vec![entry])),
        )
        .unwrap();
    let mut history = DetachedPendingHistory::new(seed).unwrap();
    rig.control(DynamicControl::Update {
        controller: rig.controller,
        size: Some(0.4),
        speed: None,
        phase: None,
    });
    let input = rig.capture(true);
    let original_marker = history.runtime.committed_sample_boundary();
    let original_origins = history.origins.clone();
    let mut evaluator = Evaluator::new(0.7);
    evaluator.panic = true;
    let result = consume(&rig, &mut history, &[input.clone()], &[], &mut evaluator);
    assert_eq!(result.failed_attempts.len(), 1);
    assert_eq!(history.position().controls, input.controls);
    assert_eq!(history.position().inputs, input.to);
    assert_eq!(history.runtime.controllers()[0].1.size, 0.4);
    assert_eq!(history.runtime.committed_sample_boundary(), original_marker);
    assert!(history.origins.shares_storage(&original_origins));
    assert!(history.last_success().is_none());
}

#[test]
fn structural_late_gap_rejects_whole_window_before_any_evaluation() {
    let mut rig = Rig::new();
    let history =
        DetachedPendingHistory::<f32>::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    rig.control(DynamicControl::GlobalPause(true));
    let first = rig.capture(true);
    let second = rig.capture(true);
    let mut wrong = copy_input(&second);
    wrong.from = first.from;
    let before = history.runtime.snapshot();
    let position = history.position();
    assert!(matches!(
        history.prepare_window(
            history.key,
            &[first, Arc::new(wrong)],
            &[],
            &rig.controls(position.controls),
            limits()
        ),
        Err(PendingHistoryGap::InputInterval)
    ));
    assert_eq!(history.runtime.snapshot(), before);
    assert_eq!(history.position(), position);
}

#[test]
fn cold_and_controls_follow_cursor_order_even_at_equal_time() {
    let mut rig = Rig::new();
    let mut history = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let initial = rig.capture(true);
    let mut evaluator = Evaluator::new(0.8);
    consume(&rig, &mut history, &[initial], &[], &mut evaluator);
    let old_instance = history.runtime.snapshot().instances[0].id;
    let held = history.last_success().unwrap().branch_sample;
    // All five controls have identical acceptance times. They are deliberately noncommutative:
    // pause the old .8 history, remove it at cold A, then start a new clock before cold B edits it.
    rig.control(DynamicControl::Pause {
        controller: rig.controller,
        paused: true,
        resume: None,
    });
    let first_cold = rig.cold(DynamicControl::Off {
        controller: rig.controller,
        delay: 0,
        duration: 0,
    });
    let mut restart = rig.request.clone();
    restart.now_millis = 100;
    restart.controller.activated_at_millis = 100;
    rig.control(DynamicControl::Start(Box::new(restart)));
    let new_instance = rig.live.snapshot().instances[0].id;
    assert_ne!(old_instance, new_instance);
    let second_cold = rig.cold(DynamicControl::Update {
        controller: rig.controller,
        size: Some(0.5),
        speed: None,
        phase: None,
    });
    rig.control(DynamicControl::Update {
        controller: rig.controller,
        size: Some(0.6),
        speed: None,
        phase: None,
    });
    let input = rig.capture(true);
    evaluator.current = 0.3;
    let result = consume(
        &rig,
        &mut history,
        &[input.clone()],
        &[first_cold, second_cold],
        &mut evaluator,
    );
    assert!(result.stopped.is_none());
    assert_eq!(history.runtime.snapshot().instances[0].id, new_instance);
    assert_eq!(
        history.runtime.snapshot().instances[0].started_at_millis,
        100
    );
    assert_eq!(history.runtime.controllers()[0].1.size, 0.6);
    assert!(
        (history.last_success().unwrap().value - 0.3).abs() < 1e-6,
        "Off/Start must discard the old .8 held Current instead of preserving its phase history"
    );
    assert_ne!(history.last_success().unwrap().branch_sample, held);
    assert_eq!(history.position().cold, input.cold);
    assert!(Arc::ptr_eq(&history.snapshot, &input.frame.snapshot()));
}

#[test]
fn failed_cold_replay_rolls_back_ordinary_prefix_and_consumes_no_attempt() {
    let mut rig = Rig::new();
    let mut seed = rig.seed(PreloadBranch::AfterRelease);
    seed.runtime
        .off_controller_by_id(rig.controller, 0, 0, 0)
        .unwrap();
    let entry = light_engine::ContributionFamilyEntry::new(
        light_engine::ContributionSourceId::programmer(rig.key.programmer),
        light_core::ProgrammerEditStamp {
            changed_at: chrono::DateTime::from_timestamp_millis(100).unwrap(),
            programmer_order: 1,
        },
        light_engine::ContributionFamilyFootprint::Whole,
        light_engine::ContributionFamilyRole::Authored,
    );
    seed.origins
        .bind_static_evidence(
            crate::runtime::dynamic_source_origins::DynamicSourceBinding::StaticBaseline {
                target: FixtureId::new(),
                owner: light_core::programming::ProgrammingOwner::Focus,
            },
            &Arc::new(light_engine::ContributionFamilyEvidence::new(vec![entry])),
        )
        .unwrap();
    let origins = seed.origins.clone();
    let mut history = DetachedPendingHistory::new(seed).unwrap();
    rig.control(DynamicControl::GlobalPause(true));
    let cold = rig.cold(DynamicControl::Update {
        controller: rig.controller,
        size: Some(0.4),
        speed: None,
        phase: None,
    });
    let input = rig.capture(true);
    let before = history.runtime.snapshot();
    let position = history.position();
    let snapshot = Arc::clone(&history.snapshot);
    let mut evaluator = Evaluator::new(0.8);
    let result = consume(&rig, &mut history, &[input], &[cold], &mut evaluator);
    assert!(matches!(result.stopped, Some(PendingHistoryGap::Replay(_))));
    assert_eq!(result.consumed_attempts, 0);
    assert_eq!(evaluator.calls, 0);
    assert_eq!(history.runtime.snapshot(), before);
    assert_eq!(history.position(), position);
    assert!(Arc::ptr_eq(&history.snapshot, &snapshot));
    assert!(history.origins.shares_storage(&origins));
    assert_eq!(history.origins.snapshot(), origins.snapshot());
}

#[test]
fn missing_ordinary_or_cold_anchor_and_instance_anchor_are_explicit_gaps() {
    for cold in [false, true] {
        let mut rig = Rig::new();
        let history =
            DetachedPendingHistory::<f32>::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
        sample(&mut rig.live, 100, 0.2); // unselected anchor, equal time is not evidence
        let events = if cold {
            vec![rig.cold(DynamicControl::GlobalPause(true))]
        } else {
            rig.control(DynamicControl::GlobalPause(true));
            vec![]
        };
        let input = rig.capture(true);
        assert!(matches!(
            history.prepare_window(
                history.key,
                &[input],
                &events,
                &rig.controls(history.position().controls),
                limits()
            ),
            Err(PendingHistoryGap::MissingAnchor)
        ));
    }
    let mut rig = Rig::new();
    let history =
        DetachedPendingHistory::<f32>::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let input = rig.capture(false);
    let instance = rig.live.snapshot().instances[0].id;
    let _ = rig
        .live
        .sample(instance, 100, 1_000, 40, &Current(0.2))
        .unwrap();
    let mut bad = copy_input(&input);
    bad.live_sample = rig.live.committed_sample_boundary();
    assert!(matches!(
        history.prepare_window(
            history.key,
            &[Arc::new(bad)],
            &[],
            &rig.controls(history.position().controls),
            limits()
        ),
        Err(PendingHistoryGap::InstanceAnchor)
    ));
}

#[test]
fn before_after_own_results_and_prepared_windows_cannot_cross_branches() {
    let mut rig = Rig::new();
    let mut before = DetachedPendingHistory::new(rig.seed(PreloadBranch::BeforeRelease)).unwrap();
    let mut after = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let input = rig.capture(true);
    let controls = rig.controls(before.position().controls);
    let wrong = before
        .prepare_window(before.key, &[input.clone()], &[], &controls, limits())
        .unwrap();
    let mut right_eval = Evaluator::new(0.3);
    assert_eq!(
        after.consume_window(wrong, &mut right_eval).stopped,
        Some(PendingHistoryGap::StaleWindow)
    );
    assert_eq!(right_eval.calls, 0);
    consume(
        &rig,
        &mut before,
        &[input.clone()],
        &[],
        &mut Evaluator::new(0.8),
    );
    consume(&rig, &mut after, &[input], &[], &mut right_eval);
    assert_eq!(before.last_success().unwrap().value, 0.8);
    assert_eq!(after.last_success().unwrap().value, 0.3);
    assert_ne!(
        before.last_success().unwrap().branch_sample,
        after.last_success().unwrap().branch_sample
    );
}

#[test]
fn none_marker_preserves_prior_association_and_unsampled_seed_is_explicit() {
    let mut rig = Rig::new();
    let mut history = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let unsampled = rig.capture(false);
    assert_eq!(unsampled.live_sample, None);
    consume(
        &rig,
        &mut history,
        &[unsampled],
        &[],
        &mut Evaluator::new(0.8),
    );
    assert_eq!(history.live_sample, None);
    let sampled = rig.capture(true);
    let marker = sampled.live_sample;
    consume(
        &rig,
        &mut history,
        &[sampled],
        &[],
        &mut Evaluator::new(0.8),
    );
    let unsampled = rig.capture(false);
    consume(
        &rig,
        &mut history,
        &[unsampled],
        &[],
        &mut Evaluator::new(0.8),
    );
    assert_eq!(history.live_sample, marker);
    rig.control(DynamicControl::GlobalPause(true));
    let paused = rig.capture(true);
    assert!(
        consume(&rig, &mut history, &[paused], &[], &mut Evaluator::new(0.1))
            .stopped
            .is_none()
    );
}

#[test]
fn queue_snapshot_epoch_and_size_validation_precede_processing() {
    let mut rig = Rig::new();
    let history =
        DetachedPendingHistory::<f32>::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let input = rig.capture(false);
    let controls = rig.controls(history.position().controls);
    let mut other_programmer = copy_input(&input);
    let foreign = Engine::new(ProgrammerRegistry::default());
    other_programmer.frame = Arc::new(foreign.prepare_output_frame(Default::default()));
    assert!(matches!(
        history.prepare_window(
            history.key,
            &[Arc::new(other_programmer)],
            &[],
            &controls,
            limits()
        ),
        Err(PendingHistoryGap::WrongEpisode)
    ));
    let mut key = history.key;
    key.activation = Uuid::new_v4();
    assert!(matches!(
        history.prepare_window(key, &[input.clone()], &[], &controls, limits()),
        Err(PendingHistoryGap::WrongEpisode)
    ));
    assert!(matches!(
        history.prepare_window(
            history.key,
            &[input.clone(), input.clone()],
            &[],
            &controls,
            PendingHistoryLimits {
                attempts: capacity(1),
                ..limits()
            }
        ),
        Err(PendingHistoryGap::LimitExceeded)
    ));
    let mut bad_snapshot = copy_input(&input);
    let alien = Engine::new(rig.programmers.clone());
    alien.replace_snapshot((*history.snapshot).clone()).unwrap();
    bad_snapshot.frame = Arc::new(alien.prepare_output_frame(Default::default()));
    assert!(matches!(
        history.prepare_window(
            history.key,
            &[Arc::new(bad_snapshot)],
            &[],
            &controls,
            limits()
        ),
        Err(PendingHistoryGap::SnapshotMismatch)
    ));
    rig.programmers.arm_preload(rig.session, true);
    assert!(rig.programmers.queue_preload_playback_action(
        rig.session,
        1,
        None,
        PreloadPlaybackQueueAction::Go,
        PreloadPlaybackQueueSurface::Physical
    ));
    let queued = rig.capture(false);
    assert!(matches!(
        history.prepare_window(
            history.key,
            &[input.clone(), queued],
            &[],
            &controls,
            limits()
        ),
        Err(PendingHistoryGap::QueueContextUnavailable)
    ));
    rig.publication.installed(rig.engine.snapshot()); // even an identical show Arc resets lineage
    let reset = rig.capture(false);
    assert!(matches!(
        history.prepare_window(history.key, &[reset], &[], &controls, limits()),
        Err(PendingHistoryGap::InputInterval)
    ));
}

#[test]
// The reversed range is the invalid bound under test.
#[allow(clippy::reversed_empty_ranges)]
fn immutable_control_ranges_keep_exact_cursors_and_reject_invalid_bounds() {
    let mut rig = Rig::new();
    let from = rig.live.control_cursor().unwrap();
    rig.control(DynamicControl::GlobalPause(true));
    rig.control(DynamicControl::GlobalPause(false));
    let batch = rig.controls(from);
    assert_eq!(batch.len(), 2);
    let left = batch.range(0..1).unwrap();
    let right = batch.range(1..2).unwrap();
    assert_eq!(left.from(), batch.from());
    assert_eq!(left.to(), right.from());
    assert_eq!(right.to(), batch.to());
    assert_eq!(batch.offset_of(left.to()), Some(1));
    assert_eq!(batch.offset_of(batch.to()), Some(2));
    assert!(batch.range(2..1).is_err());
    assert!(batch.range(0..3).is_err());
    assert!(batch.range(2..2).unwrap().is_empty());
    let other = Rig::new();
    assert_eq!(batch.offset_of(other.live.control_cursor().unwrap()), None);
    assert_eq!(batch.len(), 2);
}

#[test]
fn constructor_rejects_live_recording_and_unrelated_seed_marker() {
    let mut rig = Rig::new();
    let mut recording = rig.seed(PreloadBranch::AfterRelease);
    recording.runtime = rig.live.fork_for_cold_install();
    assert!(matches!(
        DetachedPendingHistory::<f32>::new(recording),
        Err(PendingHistoryGap::InvalidSeed)
    ));
    sample(&mut rig.live, 100, 0.2);
    let mut unrelated = rig.seed(PreloadBranch::AfterRelease);
    sample(&mut unrelated.runtime, 100, 0.8);
    assert!(matches!(
        DetachedPendingHistory::<f32>::new(unrelated),
        Err(PendingHistoryGap::InvalidSeed)
    ));
    let mut nil = rig.seed(PreloadBranch::AfterRelease);
    nil.key.activation = Uuid::nil();
    assert!(matches!(
        DetachedPendingHistory::<f32>::new(nil),
        Err(PendingHistoryGap::InvalidSeed)
    ));
}

#[test]
fn retained_input_arc_survives_source_eviction_and_replay_does_not_retain_every_result() {
    let mut rig = Rig::new();
    let mut history = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let seed_input = history.position().inputs;
    let seed_control = history.position().controls;
    let first = rig.capture(false);
    let held_batch = rig.controls(seed_control);
    for _ in 0..65 {
        rig.capture(false);
    }
    assert!(rig.publication.input_captures_since(seed_input).is_err());
    let window = history
        .prepare_window(history.key, &[first.clone()], &[], &held_batch, limits())
        .unwrap();
    let mut evaluator = Evaluator::new(0.8);
    assert_eq!(
        history
            .consume_window(window, &mut evaluator)
            .successful_attempts,
        1
    );
    assert!(Arc::ptr_eq(
        &history.last_success().unwrap().capture,
        &first
    ));
    // A later surviving entry cannot skip the evicted selected attempts.
    let latest = rig.capture(false);
    assert!(matches!(
        history.prepare_window(history.key, &[latest], &[], &held_batch, limits()),
        Err(PendingHistoryGap::InputInterval)
    ));
}

#[test]
fn divergent_cold_fork_records_cannot_be_combined_by_matching_cursors_and_anchors() {
    let mut rig = Rig::new();
    let history =
        DetachedPendingHistory::<f32>::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let mut other = rig.live.fork_for_cold_install();
    let cold = rig.cold(DynamicControl::GlobalPause(true));
    other
        .apply_recorded_control(TimedDynamicControl {
            at_millis: 100,
            control: DynamicControl::Update {
                controller: rig.controller,
                size: Some(0.4),
                speed: None,
                phase: None,
            },
        })
        .unwrap();
    assert_eq!(other.control_cursor(), rig.live.control_cursor());
    let (_, _, cold_batch) = cold.replay_inputs().unwrap();
    let divergent = other
        .controls_since(history.position().controls)
        .unwrap()
        .unwrap();
    assert_eq!(cold_batch.from(), divergent.from());
    assert_eq!(cold_batch.to(), divergent.to());
    assert_eq!(
        cold_batch.preceding_samples().collect::<Vec<_>>(),
        divergent.preceding_samples().collect::<Vec<_>>()
    );
    assert!(!cold_batch.shares_records(&divergent));
    assert!(cold_batch.shares_records(&cold_batch.clone()));
    rig.live = other;
    let input = rig.capture(false);
    assert!(matches!(
        history.prepare_window(history.key, &[input], &[cold], &divergent, limits()),
        Err(PendingHistoryGap::ControlInterval)
    ));
}

#[test]
fn coordinator_installs_changed_definition_before_its_start_and_samples_new_value() {
    let mut rig = Rig::new();
    let mut history = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let initial = rig.capture(true);
    consume(
        &rig,
        &mut history,
        &[initial],
        &[],
        &mut Evaluator::new(0.8),
    );
    rig.control(DynamicControl::Off {
        controller: rig.controller,
        delay: 0,
        duration: 0,
    });
    let mut replacement = definition();
    replacement.name = "New cold definition with constant endpoint".into();
    for point in &mut replacement.lanes[0].legacy_mut().unwrap().keyframes.points {
        point.source = light_dynamics::ScalarSource::Value { value: 0.65 };
    }
    let mut request = rig.request.clone();
    request.definition_id = replacement.id;
    request.now_millis = 100;
    request.controller.activated_at_millis = 100;
    let event = rig.cold_definitions(
        DynamicControl::Start(Box::new(request)),
        Some(vec![replacement.clone()]),
    );
    let input = rig.capture(true);
    let result = consume(
        &rig,
        &mut history,
        &[input],
        &[event],
        &mut Evaluator::new(0.1),
    );
    assert!(result.stopped.is_none());
    assert_eq!(result.successful_attempts, 1);
    let instance = history.runtime.snapshot().instances[0].id;
    assert_eq!(
        history
            .runtime
            .instance_definition(instance)
            .unwrap()
            .as_ref(),
        &replacement
    );
    assert!(
        (history.last_success().unwrap().value - 0.65).abs() < 1e-6,
        "the first selected attempt must use the newly installed definition, not .8 held history or .1 Current"
    );
}

#[test]
fn synchronized_resume_blends_pending_held_current_without_copying_live() {
    struct Weighted(f32);
    impl PendingAttemptEvaluator<f32> for Weighted {
        fn evaluate(
            &mut self,
            _: PendingEpisodeKey,
            input: &RetainedInputCapture,
            runtime: &mut DynamicRuntime,
            _: &mut DynamicSourceOrigins,
        ) -> Result<f32, String> {
            let at = input.frame.sampled_at().timestamp_millis() as u64;
            let samples = runtime.sample_all(at, 40, &transports(), &Current(self.0));
            assert_eq!(samples.len(), 1);
            let mut total = 0.0;
            assert!(
                samples[0]
                    .expression
                    .visit_legacy_contributions(|_, value, weight| {
                        total += value * weight;
                    })
            );
            Ok(total)
        }
    }
    let mut rig = Rig::with_activation_duration(400);
    let mut history = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let mut evaluator = Weighted(0.8);
    rig.clock
        .set(chrono::DateTime::from_timestamp_millis(500).unwrap());
    let input = rig.capture(true);
    let window = history
        .prepare_window(
            history.key,
            &[input],
            &[],
            &rig.controls(history.position.controls),
            limits(),
        )
        .unwrap();
    assert_eq!(
        history
            .consume_window(window, &mut evaluator)
            .successful_attempts,
        1
    );
    rig.control_at(
        500,
        DynamicControl::Pause {
            controller: rig.controller,
            paused: true,
            resume: None,
        },
    );
    rig.clock
        .set(chrono::DateTime::from_timestamp_millis(900).unwrap());
    let paused = rig.capture(true);
    evaluator.0 = 0.1;
    let window = history
        .prepare_window(
            history.key,
            &[paused],
            &[],
            &rig.controls(history.position.controls),
            limits(),
        )
        .unwrap();
    assert_eq!(
        history
            .consume_window(window, &mut evaluator)
            .successful_attempts,
        1
    );
    assert!((history.last_success().unwrap().value - 0.8).abs() < 1e-6);
    rig.control_at(
        900,
        DynamicControl::Pause {
            controller: rig.controller,
            paused: false,
            resume: Some(ActivationPolicy::JoinSyncNow),
        },
    );
    rig.clock
        .set(chrono::DateTime::from_timestamp_millis(1100).unwrap());
    let resumed = rig.capture(true);
    let window = history
        .prepare_window(
            history.key,
            &[resumed],
            &[],
            &rig.controls(history.position.controls),
            limits(),
        )
        .unwrap();
    let outcome = history.consume_window(window, &mut evaluator);
    assert!(outcome.stopped.is_none());
    assert_eq!(outcome.successful_attempts, 1);
    assert!(
        (history.last_success().unwrap().value - 0.45).abs() < 1e-6,
        "halfway Resume must blend Pending held .8 with Pending Current .1"
    );
    let live_samples = rig.live.sample_all(1100, 40, &transports(), &Current(0.2));
    let mut live_value = 0.0;
    assert!(
        live_samples[0]
            .expression
            .visit_legacy_contributions(|_, value, weight| live_value += value * weight)
    );
    assert!((live_value - 0.2).abs() < 1e-6);
}

#[path = "tests/native_presets.rs"]
mod native_presets;
