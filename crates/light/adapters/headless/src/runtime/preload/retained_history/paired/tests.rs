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
use light_programmer::ProgrammerRegistry;
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

// Independent fixture for this module's private-plan checks; no production test accessors.
struct Rig {
    clock: Arc<ManualClock>,
    engine: Engine,
    publication: DynamicSnapshotPublication,
    live: DynamicRuntime,
    start: Instant,
    selected: u64,
    key: PendingEpisodeKey,
    controller: Uuid,
}
impl Rig {
    fn new() -> Self {
        Self::configured(definition(), 0)
    }
    fn configured(definition: DynamicDefinition, duration: u64) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(100).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let programmer = programmers.start(SessionId::new()).id;
        let engine = Engine::new(programmers);
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
        live.start(DynamicStartRequest {
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
            activation_duration_millis: duration,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
        publication
            .begin_retained_history(&mut live, &engine.snapshot(), capacity(64))
            .unwrap();
        Self {
            clock,
            engine,
            publication,
            live,
            start: Instant::now(),
            selected: 0,
            key: PendingEpisodeKey {
                activation: Uuid::new_v4(),
                programmer,
                branch: PreloadBranch::BeforeRelease,
            },
            controller,
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
    fn pair<T>(&self) -> PairedPendingHistory<T> {
        PairedPendingHistory::new(
            self.seed(PreloadBranch::BeforeRelease),
            self.seed(PreloadBranch::AfterRelease),
        )
        .unwrap()
    }
    fn control(&mut self, at: u64, control: DynamicControl) {
        self.live
            .apply_recorded_control(TimedDynamicControl {
                at_millis: at,
                control,
            })
            .unwrap();
    }
    fn capture(&mut self, at: i64, sampled: bool) -> Arc<RetainedInputCapture> {
        self.clock
            .set(chrono::DateTime::from_timestamp_millis(at).unwrap());
        let cursor = self.publication.input_capture_cursor().unwrap();
        let frame = RetainedFrameCapture::select(
            self.engine.prepare_output_frame(Default::default()),
            &self.publication,
            self.start + Duration::from_millis(self.selected * 40),
        );
        self.selected += 1;
        let marker = if sampled {
            weighted(&mut self.live, at as u64, 0.2);
            self.live.committed_sample_boundary()
        } else {
            None
        };
        self.publication.retain_accepted_input(
            &self.live,
            frame.retained().unwrap(),
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
    fn plan<T>(
        &self,
        pair: &PairedPendingHistory<T>,
        inputs: &[Arc<RetainedInputCapture>],
    ) -> PreparedPendingPairWindow {
        let (before, after) = pair.positions();
        pair.prepare_window(
            inputs,
            &[],
            &self.controls(before.controls),
            &[],
            &self.controls(after.controls),
            limits(),
        )
        .unwrap()
    }
}
fn weighted(runtime: &mut DynamicRuntime, at: u64, current: f32) -> f32 {
    let samples = runtime.sample_all(at, 40, &transports(), &Current(current));
    assert_eq!(samples.len(), 1);
    let mut total = 0.;
    assert!(
        samples[0]
            .expression
            .visit_legacy_contributions(|_, value, weight| total += value * weight)
    );
    total
}
fn copy_input(input: &RetainedInputCapture) -> RetainedInputCapture {
    RetainedInputCapture {
        from: input.from,
        to: input.to,
        cold: input.cold,
        controls: input.controls,
        frame: input.frame.clone(),
        baseline: input.baseline.clone(),
        speed_transports: input.speed_transports,
        rate: input.rate,
        live_sample: input.live_sample,
    }
}
fn origins() -> DynamicSourceOrigins {
    let mut origins = DynamicSourceOrigins::default();
    let entry = light_engine::ContributionFamilyEntry::new(
        light_engine::ContributionSourceId::programmer(light_core::ProgrammerId::new()),
        light_core::ProgrammerEditStamp {
            changed_at: chrono::DateTime::from_timestamp_millis(100).unwrap(),
            programmer_order: 1,
        },
        light_engine::ContributionFamilyFootprint::Whole,
        light_engine::ContributionFamilyRole::Authored,
    );
    origins
        .bind_static_evidence(
            crate::runtime::dynamic_source_origins::DynamicSourceBinding::StaticBaseline {
                target: FixtureId::new(),
                owner: light_core::programming::ProgrammingOwner::Focus,
            },
            &Arc::new(light_engine::ContributionFamilyEvidence::new(vec![entry])),
        )
        .unwrap();
    origins
}

/// Deliberately returns only After, while retaining observable proof that Before was warmed.
struct Evaluate {
    before: f32,
    after: f32,
    seen: Vec<(f32, f32)>,
    fail_call: Option<usize>,
    panic_call: Option<usize>,
}
impl Evaluate {
    fn new(before: f32, after: f32) -> Self {
        Self {
            before,
            after,
            seen: vec![],
            fail_call: None,
            panic_call: None,
        }
    }
}
impl PendingPairEvaluator<f32> for Evaluate {
    fn evaluate(
        &mut self,
        input: &RetainedInputCapture,
        before: &mut DynamicRuntime,
        before_origins: &mut DynamicSourceOrigins,
        after: &mut DynamicRuntime,
        after_origins: &mut DynamicSourceOrigins,
    ) -> Result<f32, String> {
        let at = input.frame.sampled_at().timestamp_millis() as u64;
        let b = weighted(before, at, self.before);
        let a = weighted(after, at, self.after);
        self.seen.push((b, a));
        // Diagnostic call counts may survive failure; they are not published semantic state.
        if self.fail_call == Some(self.seen.len()) || self.panic_call == Some(self.seen.len()) {
            *before_origins = DynamicSourceOrigins::default();
            *after_origins = DynamicSourceOrigins::default();
            if self.panic_call == Some(self.seen.len()) {
                panic!("pair final encoding panic");
            }
            return Err("pair final encoding failed".into());
        }
        Ok(a)
    }
}

#[test]
fn seeds_require_exact_episode_branches_snapshot_positions_and_live_association() {
    let mut rig = Rig::new();
    for case in 0..5 {
        let before = rig.seed(PreloadBranch::BeforeRelease);
        let mut after = rig.seed(PreloadBranch::AfterRelease);
        match case {
            0 => after.key.branch = PreloadBranch::BeforeRelease,
            1 => after.key.activation = Uuid::new_v4(),
            2 => after.key.programmer = light_core::ProgrammerId::new(),
            3 => after.snapshot = Arc::new((*after.snapshot).clone()),
            4 => {
                after.live_sample = {
                    weighted(&mut after.runtime, 100, 0.4);
                    after.runtime.committed_sample_boundary()
                }
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            PairedPendingHistory::<f32>::new(before, after),
            Err(PendingHistoryGap::InvalidSeed)
        ));
    }
    let before = rig.seed(PreloadBranch::BeforeRelease);
    rig.capture(100, false);
    let after = rig.seed(PreloadBranch::AfterRelease);
    assert!(matches!(
        PairedPendingHistory::<f32>::new(before, after),
        Err(PendingHistoryGap::InvalidSeed)
    ));
}

#[test]
fn both_structural_plans_and_every_capture_arc_are_checked_before_any_replay() {
    let mut rig = Rig::new();
    let mut pair = rig.pair();
    let first = rig.capture(100, true);
    let second = rig.capture(100, true);
    let mut evaluator = Evaluate::new(0.8, 0.3);
    let before = (
        pair.before.runtime.snapshot(),
        pair.after.runtime.snapshot(),
        pair.positions(),
    );
    let foreign = Rig::new();
    let foreign_controls =
        foreign.controls(foreign.seed(PreloadBranch::AfterRelease).position.controls);
    assert!(matches!(
        pair.prepare_window(
            &[first.clone()],
            &[],
            &rig.controls(pair.before.position.controls),
            &[],
            &foreign_controls,
            limits()
        ),
        Err(PendingHistoryGap::ControlInterval)
    ));
    let mut crossed = rig.plan(&pair, &[first, second.clone()]);
    // Equal fields/cursors are insufficient: even the SECOND capture must be the same Arc.
    crossed.after.attempts[1].capture = Arc::new(copy_input(&second));
    let result = pair.consume_window(crossed, &mut evaluator);
    assert!(matches!(
        result.stopped,
        Some(PendingPairStop::Rejected(PendingHistoryGap::InputInterval))
    ));
    assert_eq!(
        (
            pair.before.runtime.snapshot(),
            pair.after.runtime.snapshot(),
            pair.positions()
        ),
        before
    );
    assert!(evaluator.seen.is_empty());
}

#[test]
fn both_branch_tokens_reject_foreign_and_stale_windows_before_replay() {
    let mut rig = Rig::new();
    let mut pair = rig.pair();
    let other = rig.pair::<f32>();
    let input = rig.capture(100, true);
    let mut crossed = rig.plan(&pair, &[input.clone()]);
    crossed.after = rig.plan(&other, &[input.clone()]).after;
    let position = pair.positions();
    let mut evaluator = Evaluate::new(0.8, 0.3);
    assert!(matches!(
        pair.consume_window(crossed, &mut evaluator).stopped,
        Some(PendingPairStop::Rejected(PendingHistoryGap::StaleWindow))
    ));
    assert_eq!(pair.positions(), position);
    assert!(evaluator.seen.is_empty());
    let one = rig.plan(&pair, &[input.clone()]);
    let stale = rig.plan(&pair, &[input]);
    assert_eq!(
        pair.consume_window(one, &mut evaluator).successful_attempts,
        1
    );
    assert!(matches!(
        pair.consume_window(stale, &mut evaluator).stopped,
        Some(PendingPairStop::Rejected(PendingHistoryGap::StaleWindow))
    ));
    assert_eq!(evaluator.seen.len(), 1);
}

#[test]
fn before_stays_warm_when_result_uses_only_after_and_resume_keeps_each_held_current() {
    let mut rig = Rig::configured(definition(), 400);
    let mut pair = rig.pair();
    let mut evaluator = Evaluate::new(0.8, 0.3);
    let first = rig.capture(500, true);
    let plan = rig.plan(&pair, &[first]);
    assert_eq!(
        pair.consume_window(plan, &mut evaluator)
            .successful_attempts,
        1
    );
    rig.control(
        500,
        DynamicControl::Pause {
            controller: rig.controller,
            paused: true,
            resume: None,
        },
    );
    evaluator.before = 0.1;
    evaluator.after = 0.9;
    let paused = rig.capture(900, true);
    let plan = rig.plan(&pair, &[paused]);
    assert_eq!(
        pair.consume_window(plan, &mut evaluator)
            .successful_attempts,
        1
    );
    assert_eq!(evaluator.seen[1], (0.8, 0.3));
    rig.control(
        900,
        DynamicControl::Pause {
            controller: rig.controller,
            paused: false,
            resume: Some(ActivationPolicy::JoinSyncNow),
        },
    );
    let resumed = rig.capture(1100, true);
    let plan = rig.plan(&pair, &[resumed]);
    assert_eq!(
        pair.consume_window(plan, &mut evaluator)
            .successful_attempts,
        1
    );
    let (before, after) = evaluator.seen[2];
    assert!((before - 0.45).abs() < 1e-6);
    assert!((after - 0.6).abs() < 1e-6);
    assert!((pair.last_success().unwrap().value - 0.6).abs() < 1e-6);
    assert_ne!(
        pair.last_success().unwrap().before_sample,
        pair.last_success().unwrap().after_sample
    );
    assert!((weighted(&mut rig.live, 1100, 0.2) - 0.2).abs() < 1e-6);
}

#[test]
fn failed_after_replay_keeps_before_controls_without_consuming_and_retry_starts_at_actual_positions()
 {
    let mut rig = Rig::new();
    struct RemoveAfter(Uuid);
    impl PendingPairEvaluator<f32> for RemoveAfter {
        fn evaluate(
            &mut self,
            input: &RetainedInputCapture,
            before: &mut DynamicRuntime,
            _: &mut DynamicSourceOrigins,
            after: &mut DynamicRuntime,
            _: &mut DynamicSourceOrigins,
        ) -> Result<f32, String> {
            let at = input.frame.sampled_at().timestamp_millis() as u64;
            weighted(before, at, 0.8);
            let value = weighted(after, at, 0.3);
            // A legitimate branch-local reconciliation, committed with both first samples.
            after
                .off_controller_by_id(self.0, at, 0, 0)
                .map_err(|error| error.to_string())?;
            Ok(value)
        }
    }
    let mut pair = rig.pair();
    let first = rig.capture(100, true);
    let plan = rig.plan(&pair, &[first]);
    assert_eq!(
        pair.consume_window(plan, &mut RemoveAfter(rig.controller))
            .successful_attempts,
        1
    );
    let initial = pair.positions();
    rig.control(
        200,
        DynamicControl::Update {
            controller: rig.controller,
            size: Some(0.4),
            speed: None,
            phase: None,
        },
    );
    let input = rig.capture(200, true);
    let mut evaluator = Evaluate::new(0.8, 0.3);
    let plan = rig.plan(&pair, &[input.clone()]);
    let result = pair.consume_window(plan, &mut evaluator);
    assert!(matches!(
        result.stopped,
        Some(PendingPairStop::Replay {
            branch: PreloadBranch::AfterRelease,
            gap: PendingHistoryGap::Replay(ref reason),
        }) if !reason.is_empty()
    ));
    assert_eq!(result.consumed_attempts, 0);
    assert_eq!(pair.before.position.controls, input.controls);
    assert_eq!(pair.after.position.controls, initial.1.controls);
    assert_eq!(pair.before.position.inputs, initial.0.inputs);
    assert_eq!(pair.after.position.inputs, initial.1.inputs);
    assert_eq!(pair.before.runtime.controllers()[0].1.size, 0.4);
    assert!(evaluator.seen.is_empty());
    let accepted_before = pair.before.runtime.snapshot();
    let after_unchanged = pair.after.runtime.snapshot();
    // Retry from the accepted Before cursor produces an EMPTY Before interval. Deterministic
    // After failure remains a passive gap; no new authority is invented to repair it.
    let retry = rig.plan(&pair, &[input.clone()]);
    assert!(retry.before.attempts[0].replay.is_empty());
    assert!(matches!(
        pair.consume_window(retry, &mut evaluator).stopped,
        Some(PendingPairStop::Replay {
            branch: PreloadBranch::AfterRelease,
            gap: PendingHistoryGap::Replay(ref reason),
        }) if !reason.is_empty()
    ));
    assert_eq!(pair.before.runtime.snapshot(), accepted_before);
    assert_eq!(pair.after.runtime.snapshot(), after_unchanged);
    assert!(evaluator.seen.is_empty());
    assert!(matches!(
        pair.prepare_window(
            &[input],
            &[],
            &rig.controls(initial.0.controls),
            &[],
            &rig.controls(initial.1.controls),
            limits()
        ),
        Err(PendingHistoryGap::ControlInterval)
    ));
}

#[test]
fn final_error_and_panic_rollback_both_samples_catalogues_and_result_but_consume_the_live_anchor() {
    for panic in [false, true] {
        let mut rig = Rig::new();
        let mut before = rig.seed(PreloadBranch::BeforeRelease);
        let mut after = rig.seed(PreloadBranch::AfterRelease);
        before.origins = origins();
        after.origins = origins();
        let mut pair = PairedPendingHistory::new(before, after).unwrap();
        let mut evaluator = Evaluate::new(0.8, 0.3);
        let first = rig.capture(100, true);
        let plan = rig.plan(&pair, &[first.clone()]);
        assert_eq!(
            pair.consume_window(plan, &mut evaluator)
                .successful_attempts,
            1
        );
        let snapshots = (
            pair.before.runtime.snapshot(),
            pair.after.runtime.snapshot(),
        );
        let markers = (
            pair.before.runtime.committed_sample_boundary(),
            pair.after.runtime.committed_sample_boundary(),
        );
        let catalogues = (pair.before.origins.clone(), pair.after.origins.clone());
        evaluator.before = 0.1;
        evaluator.after = 0.9;
        if panic {
            evaluator.panic_call = Some(2);
        } else {
            evaluator.fail_call = Some(2);
        }
        let failed = rig.capture(200, true);
        let plan = rig.plan(&pair, &[failed.clone()]);
        let result = pair.consume_window(plan, &mut evaluator);
        assert_eq!(
            (
                result.consumed_attempts,
                result.successful_attempts,
                result.failed_attempts.len()
            ),
            (1, 0, 1)
        );
        assert_eq!(result.failed_attempts[0].input, failed.to);
        assert_eq!(
            (
                pair.before.runtime.snapshot(),
                pair.after.runtime.snapshot()
            ),
            snapshots
        );
        assert_eq!(
            (
                pair.before.runtime.committed_sample_boundary(),
                pair.after.runtime.committed_sample_boundary()
            ),
            markers
        );
        assert!(pair.before.origins.shares_storage(&catalogues.0));
        assert!(pair.after.origins.shares_storage(&catalogues.1));
        assert_eq!(
            (pair.before.position.inputs, pair.after.position.inputs),
            (failed.to, failed.to)
        );
        assert!(Arc::ptr_eq(&pair.last_success().unwrap().capture, &first));
        // The failed Live anchor maps to each branch's retained success, so Pause holds its own
        // .8/.3 history and does not become a false missing-anchor gap or copy Live's .2.
        rig.control(200, DynamicControl::GlobalPause(true));
        let paused = rig.capture(300, true);
        let plan = rig.plan(&pair, &[paused]);
        assert_eq!(
            pair.consume_window(plan, &mut evaluator)
                .successful_attempts,
            1
        );
        assert_eq!(evaluator.seen[2], (0.8, 0.3));
    }
}

#[test]
fn selected_attempts_are_all_evaluated_but_only_latest_success_is_retained() {
    let mut rig = Rig::new();
    let mut pair = rig.pair();
    let inputs = [
        rig.capture(100, true),
        rig.capture(140, true),
        rig.capture(180, true),
    ];
    let mut evaluator = Evaluate::new(0.8, 0.3);
    evaluator.fail_call = Some(2);
    let plan = rig.plan(&pair, &inputs);
    let outcome = pair.consume_window(plan, &mut evaluator);
    assert_eq!(
        (
            outcome.consumed_attempts,
            outcome.successful_attempts,
            outcome.failed_attempts.len()
        ),
        (3, 2, 1)
    );
    assert_eq!(evaluator.seen.len(), 3);
    assert!(Arc::ptr_eq(
        &pair.last_success().unwrap().capture,
        &inputs[2]
    ));
    assert_eq!(
        (pair.before.position.inputs, pair.after.position.inputs),
        (inputs[2].to, inputs[2].to)
    );
}

#[test]
fn actual_second_branch_staged_sampling_error_rolls_back_the_completed_first_sample() {
    use light_core::programming::ProgrammingComponent;
    use light_dynamics::{
        DynamicFamilyRepresentation, DynamicLaneBody, DynamicPresetSourceBinding,
        DynamicRuntimeError, DynamicSamplingScratch, DynamicValue, DynamicValueAddress,
        DynamicValueSource, DynamicValueSourceResolver, MaxMinConfiguration, PeriodicFunction,
        ProgrammingLaneBody, ProgrammingLaneConfiguration, PwmShape,
    };
    struct Typed(f32);
    impl DynamicValueSourceResolver for Typed {
        fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
            Some(DynamicValue::Scalar(self.0))
        }
        fn preset(
            &self,
            _: &DynamicPresetSourceBinding,
            _: Uuid,
            _: FixtureId,
        ) -> Option<DynamicValue> {
            None
        }
    }
    fn staged(
        runtime: &mut DynamicRuntime,
        at: u64,
        current: f32,
    ) -> Result<usize, DynamicRuntimeError> {
        let sources = Typed(current);
        runtime.sample_all_programming_staged(
            at,
            40,
            &transports(),
            &Current(0.5),
            &sources,
            None,
            &mut DynamicSamplingScratch::default(),
            |_, deferred| {
                let completed = deferred.complete(&sources)?;
                let count = completed.samples().len();
                Ok((completed, count))
            },
        )
    }
    struct Staged {
        invalid_after: bool,
        completed_before: usize,
    }
    impl PendingPairEvaluator<usize> for Staged {
        fn evaluate(
            &mut self,
            input: &RetainedInputCapture,
            before: &mut DynamicRuntime,
            _: &mut DynamicSourceOrigins,
            after: &mut DynamicRuntime,
            _: &mut DynamicSourceOrigins,
        ) -> Result<usize, String> {
            let at = input.frame.sampled_at().timestamp_millis() as u64;
            self.completed_before = staged(before, at, 0.4).map_err(|error| error.to_string())?;
            staged(after, at, if self.invalid_after { f32::NAN } else { 0.6 })
                .map_err(|error| error.to_string())
        }
    }
    let mut definition = definition();
    let mut focus = definition.lanes[0].clone();
    focus.id = Uuid::new_v4();
    focus.body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: DynamicValueSource::Current,
            maximum: DynamicValueSource::Value {
                value: DynamicValue::Scalar(0.8),
            },
            function: PeriodicFunction::LinearUp,
            size: 1.,
            pwm: PwmShape::default(),
        }),
    });
    definition.lanes.push(focus);
    let mut rig = Rig::configured(definition, 0);
    let mut pair = rig.pair();
    let mut evaluator = Staged {
        invalid_after: false,
        completed_before: 0,
    };
    let first = rig.capture(100, false);
    let plan = rig.plan(&pair, &[first.clone()]);
    assert_eq!(
        pair.consume_window(plan, &mut evaluator)
            .successful_attempts,
        1
    );
    let snapshots = (
        pair.before.runtime.snapshot(),
        pair.after.runtime.snapshot(),
    );
    let markers = (
        pair.before.runtime.committed_sample_boundary(),
        pair.after.runtime.committed_sample_boundary(),
    );
    evaluator.invalid_after = true;
    evaluator.completed_before = 0;
    let failed = rig.capture(200, false);
    let plan = rig.plan(&pair, &[failed.clone()]);
    let outcome = pair.consume_window(plan, &mut evaluator);
    assert_eq!(
        evaluator.completed_before, 2,
        "Before's real legacy and Focus samples completed first"
    );
    assert_eq!(
        (
            outcome.consumed_attempts,
            outcome.successful_attempts,
            outcome.failed_attempts.len()
        ),
        (1, 0, 1)
    );
    assert!(!outcome.failed_attempts[0].detail.is_empty());
    assert_eq!(
        (
            pair.before.runtime.snapshot(),
            pair.after.runtime.snapshot()
        ),
        snapshots
    );
    assert_eq!(
        (
            pair.before.runtime.committed_sample_boundary(),
            pair.after.runtime.committed_sample_boundary()
        ),
        markers
    );
    assert_eq!(
        (pair.before.position.inputs, pair.after.position.inputs),
        (failed.to, failed.to)
    );
    assert!(Arc::ptr_eq(&pair.last_success().unwrap().capture, &first));
}

#[test]
fn random_history_advances_in_both_branches_and_batch_failure_matches_sequential_and_skipped_sample()
 {
    // Random requires a Speed Group. Only this test advances its authoritative transport;
    // the shared fixture intentionally keeps phase zero for the other Current/Resume tests.
    fn advancing(at: u64) -> [DynamicSpeedTransport; 5] {
        let mut values = transports();
        for value in &mut values {
            value.phase_reference_millis = at;
            value.beat_phase = (at as f64 / 1_000.0).rem_euclid(1.0);
        }
        values
    }
    fn random_sample(
        runtime: &mut DynamicRuntime,
        at: u64,
        speed: &[DynamicSpeedTransport; 5],
    ) -> f32 {
        let samples = runtime.sample_all(at, 40, speed, &Current(0.0));
        assert_eq!(samples.len(), 1);
        let mut total = 0.;
        assert!(
            samples[0]
                .expression
                .visit_legacy_contributions(|_, value, weight| total += value * weight)
        );
        total
    }
    fn capture(rig: &mut Rig, at: u64) -> Arc<RetainedInputCapture> {
        rig.clock
            .set(chrono::DateTime::from_timestamp_millis(at as i64).unwrap());
        let cursor = rig.publication.input_capture_cursor().unwrap();
        let frame = RetainedFrameCapture::select(
            rig.engine.prepare_output_frame(Default::default()),
            &rig.publication,
            rig.start + Duration::from_millis(rig.selected * 40),
        );
        rig.selected += 1;
        let speed = advancing(at);
        random_sample(&mut rig.live, at, &speed);
        let marker = rig.live.committed_sample_boundary();
        rig.publication.retain_accepted_input(
            &rig.live,
            frame.retained().unwrap(),
            &[],
            &speed,
            40,
            marker,
        );
        rig.publication
            .input_captures_since(cursor)
            .unwrap()
            .remove(0)
    }
    #[derive(Default)]
    struct RandomEvaluator {
        seen: Vec<(f32, f32)>,
    }
    impl PendingPairEvaluator<f32> for RandomEvaluator {
        fn evaluate(
            &mut self,
            input: &RetainedInputCapture,
            before: &mut DynamicRuntime,
            _: &mut DynamicSourceOrigins,
            after: &mut DynamicRuntime,
            _: &mut DynamicSourceOrigins,
        ) -> Result<f32, String> {
            let at = input.frame.sampled_at().timestamp_millis() as u64;
            assert_eq!(input.speed_transports[0].phase_reference_millis, at);
            let before = random_sample(before, at, &input.speed_transports);
            let after = random_sample(after, at, &input.speed_transports);
            self.seen.push((before, after));
            if self.seen.len() == 2 {
                return Err("second Random attempt fails after sampling both branches".into());
            }
            Ok(after)
        }
    }
    let mut random = definition();
    let group = Uuid::new_v4();
    random.lanes[0].legacy_mut().unwrap().mode = light_dynamics::DynamicLaneMode::Random;
    random.lanes[0].random_group_id = Some(group);
    random
        .random_groups
        .push(light_dynamics::DynamicRandomGroup {
            id: group,
            seed: 17,
            range: light_dynamics::DynamicRandomRange::LegacyScalar {
                low: light_dynamics::ScalarSource::Value { value: 0.0 },
                high: light_dynamics::ScalarSource::Value { value: 1.0 },
            },
            decision_interval_millis: 100,
            start_probability: 1.0,
            mean_duration_millis: 250,
            duration_spread_millis: 30,
            attack_ratio: 0.1,
            decay_ratio: 0.5,
        });
    let mut rig = Rig::configured(random, 0);
    let mut batch = rig.pair();
    let mut sequential = rig.pair();
    let mut reference = rig.seed(PreloadBranch::BeforeRelease).runtime;
    let initial_random = reference.snapshot().instances[0].random_streams.clone();
    let inputs = [
        capture(&mut rig, 100),
        capture(&mut rig, 200),
        capture(&mut rig, 300),
        capture(&mut rig, 400),
    ];
    let mut batch_evaluator = RandomEvaluator::default();
    let mut sequential_evaluator = RandomEvaluator::default();
    let plan = rig.plan(&batch, &inputs);
    let outcome = batch.consume_window(plan, &mut batch_evaluator);
    assert_eq!(
        (
            outcome.consumed_attempts,
            outcome.successful_attempts,
            outcome.failed_attempts.len()
        ),
        (4, 3, 1)
    );
    for input in &inputs {
        let plan = rig.plan(&sequential, &[input.clone()]);
        sequential.consume_window(plan, &mut sequential_evaluator);
    }
    assert_eq!(batch_evaluator.seen, sequential_evaluator.seen);
    assert_eq!(
        batch.before.runtime.snapshot(),
        sequential.before.runtime.snapshot()
    );
    assert_eq!(
        batch.after.runtime.snapshot(),
        sequential.after.runtime.snapshot()
    );
    // Independent reference never sampled the failed 200ms attempt. This detects leaked
    // Random decisions/held state even when batched and sequential coordinators share a bug.
    random_sample(&mut reference, 100, &advancing(100));
    let first_random = reference.snapshot().instances[0].random_streams.clone();
    random_sample(&mut reference, 300, &advancing(300));
    let expected = random_sample(&mut reference, 400, &advancing(400));
    let final_random = reference.snapshot().instances[0].random_streams.clone();
    assert!(!final_random.is_empty());
    assert_ne!(final_random, initial_random);
    assert_ne!(
        final_random, first_random,
        "later successful attempts must advance Random history"
    );
    assert_eq!(batch.before.runtime.snapshot(), reference.snapshot());
    assert_eq!(batch.after.runtime.snapshot(), reference.snapshot());
    assert!((batch.last_success().unwrap().value - expected).abs() < 1e-6);
    assert_eq!(batch_evaluator.seen.len(), 4);
    assert!(Arc::ptr_eq(
        &batch.last_success().unwrap().capture,
        &inputs[3]
    ));
}

#[test]
fn cold_definition_and_start_reach_both_next_samples_using_each_pending_current() {
    let mut rig = Rig::new();
    let mut pair = rig.pair();
    let mut evaluator = Evaluate::new(0.8, 0.3);
    let first = rig.capture(100, true);
    let plan = rig.plan(&pair, &[first]);
    assert_eq!(
        pair.consume_window(plan, &mut evaluator)
            .successful_attempts,
        1
    );
    assert_eq!(evaluator.seen[0], (0.8, 0.3));
    let original = rig.live.snapshot().instances[0].clone();
    let mut controller = original.controllers[0].clone();
    controller.activated_at_millis = 100;
    rig.control(
        100,
        DynamicControl::Off {
            controller: rig.controller,
            delay: 0,
            duration: 0,
        },
    );

    let previous = rig.engine.snapshot();
    let from = pair.positions().0.cold;
    let boundary = rig.publication.cold_boundary(&rig.live).unwrap();
    let mut replacement = definition();
    replacement.name = "Paired cold Current to constant".into();
    // Fixed/StartNow deliberately uses elapsed clock time. This test harness's speed-group
    // transport is stationary at phase zero; it must not hide definition interpolation.
    replacement.speed = DynamicSpeed::Fixed {
        duration_millis: 1_000,
    };
    replacement.default_activation = ActivationPolicy::StartNow;
    replacement.lanes[0].legacy_mut().unwrap().keyframes.points[1].source =
        light_dynamics::ScalarSource::Value { value: 0.65 };
    let mut destination = (*previous).clone();
    destination.revision += 1;
    destination.dynamics = vec![replacement.clone()].into();
    rig.engine.replace_snapshot(destination).unwrap();
    let destination = rig.engine.snapshot();
    rig.live
        .install_definitions(destination.dynamics.iter().cloned())
        .unwrap();
    rig.live
        .refresh_native_color_models(destination.native_color_sources.clone())
        .unwrap();
    rig.control(
        100,
        DynamicControl::Start(Box::new(DynamicStartRequest {
            definition_id: replacement.id,
            controller,
            target_scope: DynamicTargetScope {
                ordered_targets: original.targets,
            },
            stage_positions: Default::default(),
            inherited_spatial_mapping: None,
            now_millis: 100,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })),
    );
    let prepared = boundary.prepare(previous, destination.clone(), &rig.live);
    rig.publication
        .installed_with_cold_event(destination.clone(), Some(prepared));
    let events = rig.publication.cold_generations_since(from).unwrap();
    assert_eq!(events.len(), 1);
    let input = rig.capture(350, true);
    assert!((weighted(&mut rig.live, 350, 0.2) - 0.425).abs() < 1e-6);
    let live_before = (rig.live.snapshot(), rig.live.committed_sample_boundary());
    evaluator.before = 0.1;
    evaluator.after = 0.9;
    let positions = pair.positions();
    let plan = pair
        .prepare_window(
            &[input.clone()],
            &events,
            &rig.controls(positions.0.controls),
            &events,
            &rig.controls(positions.1.controls),
            limits(),
        )
        .unwrap();
    let outcome = pair.consume_window(plan, &mut evaluator);
    assert!(outcome.stopped.is_none(), "{:?}", outcome.stopped);
    assert_eq!(
        (outcome.consumed_attempts, outcome.successful_attempts),
        (1, 1)
    );
    // At 250ms of a 1s cycle, Current@0 -> .65@.5 is halfway. These reject stale
    // old held .8/.3, old-definition fresh .1/.9, and copying Live's .425 result.
    assert!((evaluator.seen[1].0 - 0.375).abs() < 1e-6);
    assert!((evaluator.seen[1].1 - 0.775).abs() < 1e-6);
    assert!((pair.last_success().unwrap().value - 0.775).abs() < 1e-6);
    for branch in [&pair.before, &pair.after] {
        assert!(Arc::ptr_eq(&branch.snapshot, &destination));
        assert_eq!(branch.position.cold, input.cold);
        assert_eq!(branch.position.controls, input.controls);
        let snapshot = branch.runtime.snapshot();
        assert_eq!(snapshot.instances.len(), 1);
        assert_eq!(
            branch
                .runtime
                .instance_definition(snapshot.instances[0].id)
                .unwrap()
                .as_ref(),
            &replacement
        );
    }
    let result = pair.last_success().unwrap();
    assert_ne!(result.before_sample, result.after_sample);
    assert_ne!(result.before_sample, input.live_sample);
    assert_ne!(result.after_sample, input.live_sample);
    assert_eq!(
        (rig.live.snapshot(), rig.live.committed_sample_boundary()),
        live_before
    );
}
