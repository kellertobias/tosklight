use super::*;
use crate::runtime::dynamic_snapshot_publication::{
    DynamicSnapshotPublication, RetainedFrameCapture,
};
use crate::runtime::dynamic_source_origins::{
    DynamicFamilySourceProjection, DynamicStaticProgrammerLane, DynamicStaticSource,
};
use crate::runtime::preload::retained_history::paired::PairedPendingHistory;
use crate::runtime::preload::retained_history::{
    PendingEpisodeKey, PendingHistoryLimits, PendingHistoryPosition, PendingHistorySeed,
};
use light_core::{ManualClock, SessionId, programming::*};
use light_dynamics::*;
use light_engine::{FamilyProjectionEvidence, FamilyProjectionMaster};
use light_programmer::ProgrammerRegistry;
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
    rc::Rc,
    time::{Duration, Instant},
};

mod color_direct;
mod optics;
mod physical;
pub(in crate::runtime) mod position;

fn axis(component: ProgrammingComponent) -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(component),
    }
}
fn position(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn pan_definition() -> DynamicDefinition {
    let mut definition = DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 1,
        revision: 1,
        name: "Pan and static Tilt".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::Targetless,
        lanes: vec![DynamicLane {
            id: Uuid::new_v4(),
            body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                address: axis(ProgrammingComponent::Pan),
                configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                    points: [0., 0.5]
                        .map(|position| DynamicKeyframe {
                            position,
                            source: DynamicValueSource::Current,
                            interpolation: light_dynamics::ScalarInterpolation::Linear,
                        })
                        .to_vec(),
                    size: 1.,
                }),
            }),
            speed_multiplier: Rational::ONE,
            width: 1.,
            phase: None,
            random_group_id: None,
        }],
        random_groups: vec![],
        phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: Default::default(),
        phase: PhaseDistribution {
            ordering: PhaseOrdering::Selection,
            offset_degrees: 0.,
            span_degrees: 360.,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: vec![],
        },
        speed: DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::StartNow,
        activation_boundary: ActivationBoundary::Beat,
    };
    definition.normalize_angle_pair();
    definition
}

struct Compatible;
impl HybridFrameResolver for Compatible {
    fn adopt(
        &self,
        _: HybridFrameContext<'_>,
        _: FixtureId,
        _: &AttributeValue,
        _: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("compatible fixture-independent test values must not ask for physical adoption")
    }
}
struct Evidence {
    target: FixtureId,
    owner: ProgrammingOwner,
    value: AttributeValue,
    sources: DynamicFamilySourceProjection,
}
fn observe(
    _: PreloadBranch,
    observation: HybridFamilyObservation<'_>,
) -> Result<(FamilyProjectionMetadata, Evidence), TransitionError> {
    let mut sources = DynamicFamilySourceProjection::default();
    observation.project_fields(
        &ProgrammingFieldScope::for_value(observation.owner, observation.value)?,
        &mut sources,
    )?;
    Ok((
        FamilyProjectionMetadata {
            changed_at: None,
            evidence: FamilyProjectionEvidence::Replace {
                origin: None,
                family_evidence: None,
            },
            master: FamilyProjectionMaster::PreserveBaseline,
        },
        Evidence {
            target: observation.target,
            owner: observation.owner,
            value: observation.value.clone(),
            sources,
        },
    ))
}
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
fn transports() -> [DynamicSpeedTransport; 5] {
    [DynamicSpeedTransport {
        effective_bpm: 73.,
        phase_origin_millis: 0,
        phase_reference_millis: 0,
        beat_phase: 0.,
        phase_advancing: true,
    }; 5]
}
struct Rig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    target: FixtureId,
    clock: Arc<ManualClock>,
    key: PendingEpisodeKey,
    publication: DynamicSnapshotPublication,
    live: RefCell<DynamicRuntime>,
    started: Instant,
    selected: Cell<u64>,
}
impl Rig {
    fn new() -> Self {
        Self::with_numeric_pan(false)
    }
    fn numeric_pan() -> Self {
        Self::with_numeric_pan(true)
    }
    fn with_numeric_pan(numeric_pan: bool) -> Self {
        let mut definition = pan_definition();
        if numeric_pan {
            let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
                unreachable!()
            };
            let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
                unreachable!()
            };
            config.points[1].source = DynamicValueSource::Value {
                value: DynamicValue::Scalar(90.),
            };
            definition.default_activation = ActivationPolicy::JoinSyncNow;
            definition.speed = DynamicSpeed::SpeedGroup {
                group: SpeedGroup::A,
                beats_per_cycle: Rational::ONE,
            };
            assert!(!definition.lanes[0].is_angle_current_passthrough());
            assert!(definition.lanes[1].is_angle_current_passthrough());
        }
        Self::with_definition(definition)
    }
    fn with_definition(definition: DynamicDefinition) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        let programmer = programmers.start(session).id;
        let target = FixtureId::new();
        programmers.set(
            session,
            target,
            ProgrammingOwner::Position.key(),
            position(10., 20.),
        );
        let engine = Engine::with_programming_contract_support(
            programmers.clone(),
            PROGRAMMING_CONTRACT_VERSION,
        );
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                dynamics: vec![definition.clone()].into(),
                ..Default::default()
            })
            .unwrap();
        let publication = DynamicSnapshotPublication::new(engine.snapshot());
        let mut live =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        live.install_definitions([definition.clone()]).unwrap();
        publication
            .begin_retained_history(&mut live, &engine.snapshot(), capacity(64))
            .unwrap();
        programmers.arm_preload(session, true);
        clock.advance_millis(10);
        programmers.set(
            session,
            target,
            ProgrammingOwner::Position.key(),
            position(60., 40.),
        );
        assert!(programmers.apply_dynamic_values(
            session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: target,
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::DynamicOn {
                    instance_link: Uuid::new_v4(),
                    lane_id: definition.lanes[0].id,
                    dynamic: DynamicReference {
                        dynamic_id: Some(definition.id),
                        last_known_pool_number: 1,
                        embedded_fallback: DynamicDefinitionSnapshot {
                            definition: Arc::new(definition)
                        }
                    },
                    overrides: DynamicInstanceOverrides {
                        size: 1.,
                        speed_multiplier: Rational::ONE,
                        phase_offset_degrees: 0.
                    },
                    timing: Default::default()
                }
            }],
            None
        ));
        Self {
            engine,
            programmers,
            session,
            target,
            clock,
            key: PendingEpisodeKey {
                activation: Uuid::new_v4(),
                programmer,
                branch: PreloadBranch::BeforeRelease,
            },
            publication,
            live: RefCell::new(live),
            started: Instant::now(),
            selected: Cell::new(0),
        }
    }
    fn pair(&self) -> PairedPendingHistory<PendingHybridResult<Evidence>> {
        let mut live = self.live.borrow_mut();
        let (cold, controls) = self
            .publication
            .begin_retained_history(&mut live, &self.engine.snapshot(), capacity(64))
            .unwrap();
        let seed = |branch| PendingHistorySeed {
            key: PendingEpisodeKey { branch, ..self.key },
            runtime: live.fork_for_pending_preview(),
            origins: Default::default(),
            snapshot: self.engine.snapshot(),
            position: PendingHistoryPosition {
                inputs: self.publication.input_capture_cursor().unwrap(),
                cold,
                controls,
            },
            live_sample: live.committed_sample_boundary(),
        };
        PairedPendingHistory::new(
            seed(PreloadBranch::BeforeRelease),
            seed(PreloadBranch::AfterRelease),
        )
        .unwrap()
    }
    fn capture(&self) -> Arc<RetainedInputCapture> {
        let cursor = self.publication.input_capture_cursor().unwrap();
        let selected = self.selected.get();
        self.selected.set(selected + 1);
        let frame = RetainedFrameCapture::select(
            self.engine.prepare_output_frame(Default::default()),
            &self.publication,
            self.started + Duration::from_millis(selected * 40),
        );
        self.publication.retain_accepted_input(
            &self.live.borrow(),
            frame.retained().unwrap(),
            &[],
            &transports(),
            37,
            None,
        );
        self.publication
            .input_captures_since(cursor)
            .unwrap()
            .remove(0)
    }
    fn pause(&self) {
        // Match the retained Playback capture as well as its ordered control journal. The
        // real sampler reconciles the authoritative captured pause flag before sampling.
        self.engine
            .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
            .unwrap();
        self.live
            .borrow_mut()
            .apply_recorded_control(TimedDynamicControl {
                at_millis: 1_000_050,
                control: DynamicControl::GlobalPause(true),
            })
            .unwrap();
    }
    fn position(&self, pan: f32, tilt: f32) {
        self.clock.advance_millis(40);
        self.programmers.set(
            self.session,
            self.target,
            ProgrammingOwner::Position.key(),
            position(pan, tilt),
        );
    }
    fn fixed_color(&self) -> AttributeValue {
        // Fixed family composition needs a captured static underlay. This target has no
        // fixture profile defaults, so author that underlay explicitly, as the existing
        // preload_hybrid Fixed Color Release regression does. Release retains it together
        // with the Fixed candidate for Before, and removes it from After.
        self.programmers.set(
            self.session,
            self.target,
            ProgrammingOwner::Color.key(),
            AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                intent: ColorIntent::default(),
            })),
        );
        let color = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent {
                uv: UvIntent { amount: 0.8 },
                ..Default::default()
            },
        }));
        assert!(
            self.programmers.apply_dynamic_values(
                self.session,
                &[light_programmer::DynamicProgrammerValueMutation::Set {
                    fixture_id: self.target,
                    attribute: ProgrammingOwner::Color.key(),
                    value: DynamicSemanticValue::ProgrammingFixAt {
                        mask: ProgrammingFamilyFixAt::from_family(
                            ProgrammingOwner::Color,
                            None,
                            color.clone()
                        )
                        .unwrap(),
                        timing: Default::default()
                    }
                }],
                None
            )
        );
        color
    }
    fn release_color(&self) {
        assert!(self.programmers.apply_dynamic_values(
            self.session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: self.target,
                attribute: ProgrammingOwner::Color.key(),
                value: DynamicSemanticValue::Release
            }],
            None
        ));
    }
    fn consume(
        &self,
        pair: &mut PairedPendingHistory<PendingHybridResult<Evidence>>,
        input: Arc<RetainedInputCapture>,
        evaluator: &mut impl PendingPairEvaluator<PendingHybridResult<Evidence>>,
    ) -> crate::runtime::preload::retained_history::paired::PendingPairWindowOutcome {
        let (before, after) = pair.positions();
        let live = self.live.borrow();
        let before_controls = live.controls_since(before.controls).unwrap().unwrap();
        let after_controls = live.controls_since(after.controls).unwrap().unwrap();
        let window = pair
            .prepare_window(
                &[input],
                &[],
                &before_controls,
                &[],
                &after_controls,
                limits(),
            )
            .unwrap();
        drop(live);
        pair.consume_window(window, evaluator)
    }
}
fn branch_position(branch: &PendingHybridBranch<Evidence>) -> &AttributeValue {
    &branch
        .sidecars
        .iter()
        .find(|row| row.owner == ProgrammingOwner::Position)
        .unwrap()
        .value
}

fn numeric_current_pair(actual: &AttributeValue, pan: f32, tilt: f32) {
    assert_eq!(
        actual,
        &position(pan, tilt),
        "pinned numeric Current and its generated Tilt both use this branch's captured static family"
    );
}

fn numeric_expressions(
    branch: &PendingHybridBranch<Evidence>,
) -> Vec<light_dynamics::DynamicSampleExpression> {
    let expressions = branch
        .sampled
        .samples
        .iter()
        .map(|sample| sample.expression.clone())
        .collect::<Vec<_>>();
    assert!(
        expressions
            .iter()
            .any(|expression| expression.has_angle_numeric()),
        "numeric Current arithmetic must remain symbolic in retained source history"
    );
    expressions
}

#[test]
fn paired_adapter_uses_retained_pending_current_and_keeps_both_histories_warm() {
    let rig = Rig::new();
    let mut pair = rig.pair();
    let mut evaluator =
        RetainedPreloadHybridEvaluator::new(&rig.engine, rig.key.programmer, Compatible, observe);
    let first = rig.capture();
    rig.position(720., -90.); // Must not leak into either branch of the retained attempt.
    assert_eq!(
        rig.consume(&mut pair, first.clone(), &mut evaluator)
            .successful_attempts,
        1
    );
    let first_result = pair.last_success().unwrap();
    assert!(first_result.value.rendered.before_release.is_none());
    assert_eq!(
        branch_position(&first_result.value.before),
        &position(60., 40.)
    );
    assert_eq!(
        branch_position(&first_result.value.after),
        &position(60., 40.)
    );
    assert_eq!(first_result.value.before.sampled.samples.len(), 2);
    assert_eq!(first_result.value.after.sampled.samples.len(), 2);
    assert!(first_result.value.before.requirements.is_empty());
    let evidence = &first_result.value.before.sidecars[0].sources;
    assert!(evidence.entries().unwrap().iter().all(|entry| matches!(
        entry.static_source().map(|s| &s.source),
        Some(DynamicStaticSource::Programmer {
            lane: DynamicStaticProgrammerLane::Preload,
            ..
        })
    )));
    let old_origins = first_result.value.before.origins.clone();
    let old_snapshot = old_origins.snapshot();
    let old_sources = first_result.value.before.sidecars[0].sources.clone();
    let second = rig.capture();
    assert_eq!(
        rig.consume(&mut pair, second, &mut evaluator)
            .successful_attempts,
        1
    );
    assert_eq!(
        branch_position(&pair.last_success().unwrap().value.before),
        &position(720., -90.)
    );
    assert_eq!(
        branch_position(&pair.last_success().unwrap().value.after),
        &position(720., -90.)
    );
    assert_eq!(old_origins.snapshot(), old_snapshot);
    assert!(
        old_sources
            .entries()
            .unwrap()
            .iter()
            .all(|entry| old_origins.get(entry.record().occurrence_id) == Some(entry.record()))
    );
    assert!(
        !old_origins.shares_storage(&pair.last_success().unwrap().value.before.origins),
        "later captured Current source bindings require an independent catalogue"
    );
    assert!(
        rig.live.borrow().snapshot().instances.is_empty(),
        "Pending reconciliation never installs in Live"
    );
}

#[test]
fn hidden_before_history_is_held_when_color_release_later_needs_it() {
    let rig = Rig::numeric_pan();
    let color = rig.fixed_color();
    let mut pair = rig.pair();
    let mut evaluator =
        RetainedPreloadHybridEvaluator::new(&rig.engine, rig.key.programmer, Compatible, observe);
    assert_eq!(
        rig.consume(&mut pair, rig.capture(), &mut evaluator)
            .successful_attempts,
        1
    );
    assert!(
        pair.last_success()
            .unwrap()
            .value
            .rendered
            .before_release
            .is_none()
    );
    let initial = &pair.last_success().unwrap().value;
    assert_eq!(
        initial
            .before
            .sidecars
            .iter()
            .find(|row| row.owner == ProgrammingOwner::Color)
            .map(|row| &row.value),
        Some(&color)
    );
    assert!(
        initial
            .before
            .requirements
            .iter()
            .all(|row| row.owner != ProgrammingOwner::Color)
    );
    let held = [
        numeric_expressions(&pair.last_success().unwrap().value.before),
        numeric_expressions(&pair.last_success().unwrap().value.after),
    ];
    rig.pause();
    rig.position(120., 80.);
    rig.release_color();
    assert_eq!(
        rig.consume(&mut pair, rig.capture(), &mut evaluator)
            .successful_attempts,
        1
    );
    let result = &pair.last_success().unwrap().value;
    assert!(
        result
            .before
            .requirements
            .iter()
            .all(|row| row.owner != ProgrammingOwner::Color),
        "Before has its captured Color underlay and must not report MaterializedEndpoints"
    );
    numeric_current_pair(branch_position(&result.before), 120., 80.);
    numeric_current_pair(branch_position(&result.after), 120., 80.);
    assert_eq!(numeric_expressions(&result.before), held[0]);
    assert_eq!(numeric_expressions(&result.after), held[1]);
    assert_eq!(
        result
            .rendered
            .before_release
            .as_ref()
            .unwrap()
            .values()
            .value(rig.target, &ProgrammingOwner::Color.key()),
        Some(&color)
    );
    assert!(
        result
            .rendered
            .source
            .values()
            .value(rig.target, &ProgrammingOwner::Color.key())
            .is_none()
    );
    let original = result
        .before
        .sidecars
        .iter()
        .find(|row| row.owner == ProgrammingOwner::Color)
        .unwrap();
    assert_eq!(original.target, rig.target);
    assert!(
        !original.sources.entries().unwrap().is_empty(),
        "Before keeps actual Fixed-source evidence"
    );
}

#[test]
fn either_branch_failure_and_real_finalizer_rejection_preserve_the_last_pair() {
    for fail_branch in [
        Some(PreloadBranch::BeforeRelease),
        Some(PreloadBranch::AfterRelease),
        None,
    ] {
        let rig = Rig::numeric_pan();
        let mut pair = rig.pair();
        let failure = Rc::new(Cell::new(None));
        let observed_failure = Rc::clone(&failure);
        let observer = move |branch, observation: HybridFamilyObservation<'_>| {
            if observed_failure.get() == Some(branch) {
                return Err(TransitionError::from(IntentError(
                    "injected branch observer failure".into(),
                )));
            }
            observe(branch, observation)
        };
        let mut evaluator = RetainedPreloadHybridEvaluator::new(
            &rig.engine,
            rig.key.programmer,
            Compatible,
            observer,
        );
        assert_eq!(
            rig.consume(&mut pair, rig.capture(), &mut evaluator)
                .successful_attempts,
            1
        );
        let held = [
            numeric_expressions(&pair.last_success().unwrap().value.before),
            numeric_expressions(&pair.last_success().unwrap().value.after),
        ];
        let accepted = pair.last_success().unwrap().capture.to;
        let before_marker = pair.last_success().unwrap().before_sample;
        let after_marker = pair.last_success().unwrap().after_sample;
        rig.position(120., 80.);
        let failed = rig.capture();
        failure.set(fail_branch);
        evaluator.swap_finalization_tokens = fail_branch.is_none();
        let outcome = rig.consume(&mut pair, failed.clone(), &mut evaluator);
        assert_eq!(outcome.failed_attempts.len(), 1);
        assert_eq!(outcome.consumed_attempts, 1);
        assert_eq!(pair.positions().0.inputs, failed.to);
        assert_eq!(pair.positions().1.inputs, failed.to);
        assert_eq!(pair.last_success().unwrap().capture.to, accepted);
        assert_eq!(pair.last_success().unwrap().before_sample, before_marker);
        assert_eq!(pair.last_success().unwrap().after_sample, after_marker);
        failure.set(None);
        evaluator.swap_finalization_tokens = false;
        rig.pause(); // Hold arithmetic/phase, while both Current axes follow the new frame.
        rig.position(300., 100.);
        assert_eq!(
            rig.consume(&mut pair, rig.capture(), &mut evaluator)
                .successful_attempts,
            1
        );
        let result = &pair.last_success().unwrap().value;
        numeric_current_pair(branch_position(&result.before), 300., 100.);
        numeric_current_pair(branch_position(&result.after), 300., 100.);
        assert_eq!(numeric_expressions(&result.before), held[0]);
        assert_eq!(numeric_expressions(&result.after), held[1]);
    }
}

#[test]
fn adapter_rejects_foreign_programmer_and_uncaptured_queue_before_touching_branches() {
    let rig = Rig::new();
    let capture = rig.capture();
    let mut before = rig.live.borrow().fork_for_pending_preview();
    let mut after = rig.live.borrow().fork_for_pending_preview();
    let mut before_origins = DynamicSourceOrigins::default();
    let mut after_origins = DynamicSourceOrigins::default();
    let before_snapshot = before.snapshot();
    let after_snapshot = after.snapshot();
    let mut foreign = RetainedPreloadHybridEvaluator::new(
        &rig.engine,
        ProgrammerId(Uuid::new_v4()),
        Compatible,
        observe,
    );
    let result = foreign.evaluate(
        &capture,
        &mut before,
        &mut before_origins,
        &mut after,
        &mut after_origins,
    );
    assert!(matches!(result, Err(ref reason) if reason.contains("another Programmer")));
    assert!(rig.programmers.queue_preload_playback_action(
        rig.session,
        1,
        None,
        light_programmer::PreloadPlaybackQueueAction::Go,
        light_programmer::PreloadPlaybackQueueSurface::Physical
    ));
    let queued = rig.capture();
    let mut evaluator =
        RetainedPreloadHybridEvaluator::new(&rig.engine, rig.key.programmer, Compatible, observe);
    let result = evaluator.evaluate(
        &queued,
        &mut before,
        &mut before_origins,
        &mut after,
        &mut after_origins,
    );
    assert!(matches!(result, Err(ref reason) if reason.contains("queue context")));
    assert_eq!(before.snapshot(), before_snapshot);
    assert_eq!(after.snapshot(), after_snapshot);
    assert!(before_origins.snapshot().records.is_empty());
    assert!(after_origins.snapshot().records.is_empty());
}

#[test]
fn non_color_release_keeps_a_distinct_hidden_before_dynamic_until_later_color_release() {
    let rig = Rig::numeric_pan();
    // Use the real Programmer lifecycle solely to construct an active Preload Dynamic.
    // No headless GO transaction, playback installation or production publication is invoked.
    assert!(rig.programmers.activate_preload(rig.session));
    assert!(rig.programmers.arm_preload(rig.session, true));
    rig.clock.advance_millis(40);
    assert!(rig.programmers.apply_dynamic_values(
        rig.session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: rig.target,
            attribute: ProgrammingOwner::Position.key(),
            value: DynamicSemanticValue::Release
        }],
        None
    ));
    let color = rig.fixed_color();
    let mut pair = rig.pair();
    let mut evaluator =
        RetainedPreloadHybridEvaluator::new(&rig.engine, rig.key.programmer, Compatible, observe);
    assert_eq!(
        rig.consume(&mut pair, rig.capture(), &mut evaluator)
            .successful_attempts,
        1
    );
    let hidden = &pair.last_success().unwrap().value;
    assert!(
        hidden.rendered.before_release.is_none(),
        "Position Release needs no visible Color comparison"
    );
    assert_eq!(hidden.before.sampled.after_runtime.instances.len(), 1);
    // Release masks source values; it deliberately keeps linked Dynamic clocks alive.
    assert_eq!(hidden.after.sampled.after_runtime.instances.len(), 1);
    assert_eq!(
        hidden.before.sampled.samples.len(),
        2,
        "hidden Before still samples the active Angle pair"
    );
    assert!(
        hidden.after.sampled.samples.len() < hidden.before.sampled.samples.len(),
        "After lacks released static Current for numeric Pan; its hidden history must differ"
    );
    assert!(
        hidden
            .after
            .sidecars
            .iter()
            .all(|row| row.owner != ProgrammingOwner::Position),
        "an incomplete Angle pair cannot fabricate its released Current partner"
    );
    let held_position = branch_position(&hidden.before).clone();

    rig.pause();
    // The later Color comparison must keep this already-distinct Before history.
    rig.release_color();
    assert_eq!(
        rig.consume(&mut pair, rig.capture(), &mut evaluator)
            .successful_attempts,
        1
    );
    let visible = &pair.last_success().unwrap().value;
    assert_eq!(branch_position(&visible.before), &held_position);
    assert_eq!(visible.after.sampled.after_runtime.instances.len(), 1);
    assert_eq!(
        visible
            .rendered
            .before_release
            .as_ref()
            .unwrap()
            .values()
            .value(rig.target, &ProgrammingOwner::Color.key()),
        Some(&color)
    );
}

fn preset_pan(definition: &mut DynamicDefinition, value: f32) {
    let address = axis(ProgrammingComponent::Pan);
    let source = || DynamicValueSource::Preset {
        preset_id: "3.1".into(),
        address: address.clone(),
        last_valid_by_target: vec![],
        retained: Some(Arc::new(DynamicPresetTemplate {
            universal: Some(position(value, 40.)),
            ..Default::default()
        })),
    };
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: address.clone(),
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: source(),
            maximum: source(),
            function: PeriodicFunction::LinearUp,
            size: 1.,
            pwm: PwmShape::default(),
        }),
    });
}

// Test-only reads of tables deliberately omitted from output-projection snapshots.
// No assertions, fallible operations or runtime mutation after inner.evaluate.
struct PresetAudit<E> {
    inner: E,
    before: Vec<DynamicInstancePresetSources>,
    after: Vec<DynamicInstancePresetSources>,
}
impl<E: PendingPairEvaluator<PendingHybridResult<Evidence>>>
    PendingPairEvaluator<PendingHybridResult<Evidence>> for PresetAudit<E>
{
    fn evaluate(
        &mut self,
        capture: &RetainedInputCapture,
        before: &mut DynamicRuntime,
        before_origins: &mut DynamicSourceOrigins,
        after: &mut DynamicRuntime,
        after_origins: &mut DynamicSourceOrigins,
    ) -> Result<PendingHybridResult<Evidence>, String> {
        let result = self
            .inner
            .evaluate(capture, before, before_origins, after, after_origins)?;
        self.before = before.preset_source_instances();
        self.after = after.preset_source_instances();
        Ok(result)
    }
}

fn assert_preset_branch(
    rig: &Rig,
    branch: &PendingHybridBranch<Evidence>,
    tables: &[DynamicInstancePresetSources],
    value: f32,
    definition: Uuid,
    lane: Uuid,
) {
    use crate::runtime::dynamic_source_origins::{
        DynamicProgrammerSourceLane, DynamicSourceBinding, DynamicSourceOrigin,
    };
    assert_eq!(branch_position(branch), &position(value, 40.));
    assert!(branch.requirements.is_empty());
    assert_eq!(branch.sampled.samples.len(), 2);
    assert_eq!(branch.sampled.after_runtime.instances.len(), 1);
    let instance = &branch.sampled.after_runtime.instances[0];
    assert_eq!(instance.definition.id, definition);
    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert_eq!(table.instance_id, instance.id);
    assert_eq!(table.ordered_targets, vec![rig.target]);
    assert_eq!(table.sources.len(), 2);
    assert_ne!(table.sources[0].id, table.sources[1].id);
    assert_eq!(table.last_valid.len(), 2);
    for slot in [
        DynamicPresetSourceSlot::Minimum,
        DynamicPresetSourceSlot::Maximum,
    ] {
        let occurrence = DynamicPresetSourceOccurrence {
            lane_id: lane,
            slot,
        };
        let binding = table
            .sources
            .iter()
            .find(|s| s.occurrence == Some(occurrence))
            .unwrap();
        assert_eq!(binding.preset_id, "3.1");
        assert_eq!(binding.address, axis(ProgrammingComponent::Pan));
        assert_eq!(
            binding.retained.as_ref().unwrap().universal.as_ref(),
            Some(&position(value, 40.))
        );
        let row = table
            .last_valid
            .iter()
            .find(|row| row.matches(binding))
            .unwrap();
        assert_eq!(
            row.values,
            vec![DynamicValueFallback {
                target: rig.target,
                value: DynamicValue::Scalar(value)
            }]
        );
    }
    let mut pan_origin = None;
    let mut partner_count = 0;
    for sample in &branch.sampled.samples {
        assert_eq!(sample.instance_id, instance.id);
        assert_eq!(sample.target, rig.target);
        let binding = DynamicSourceBinding::Authored {
            instance_id: sample.instance_id,
            controller_id: sample.controller_id,
            target: sample.target,
            lane_id: sample.lane_id,
        };
        if sample.lane_id != lane {
            partner_count += 1;
            assert_eq!(
                sample.expression.angle_current_address(),
                Some(&axis(ProgrammingComponent::Tilt)),
                "the only other lane must be the generated static-Current Tilt partner"
            );
            assert!(branch.origins.binding(&binding).is_none());
            let mut authored = Vec::new();
            sample
                .expression
                .visit_source_occurrences(&mut |id| authored.push(id))
                .unwrap();
            assert!(
                authored.is_empty(),
                "Current Tilt must not inherit Pan authorship"
            );
            continue;
        }
        assert!(
            pan_origin.is_none(),
            "exactly one authored Preset Pan sample"
        );
        let occurrence = branch.origins.binding(&binding).unwrap();
        pan_origin = Some(occurrence);
        let record = branch.origins.get(occurrence).unwrap();
        assert_eq!(record.binding, binding);
        assert!(matches!(&record.origin, DynamicSourceOrigin::Programmer {
            programmer_id, lane: DynamicProgrammerSourceLane::Preload, ..
        } if *programmer_id == rig.key.programmer));
        let mut authored = Vec::new();
        sample
            .expression
            .visit_source_occurrences(&mut |id| authored.push(id))
            .unwrap();
        assert_eq!(authored, vec![occurrence]);
    }
    assert_eq!(partner_count, 1);
    let pan_origin = pan_origin.unwrap();
    let dependency = branch
        .origins
        .binding(&DynamicSourceBinding::StaticBaseline {
            target: rig.target,
            owner: ProgrammingOwner::Position,
        })
        .unwrap();
    assert_ne!(dependency, pan_origin);
    let position_evidence = branch
        .sidecars
        .iter()
        .find(|row| row.owner == ProgrammingOwner::Position)
        .unwrap();
    let entries = position_evidence.sources.entries().unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.record().occurrence_id == pan_origin
                && entry.role() == FamilyTraceRole::Authored)
    );
    assert!(entries.iter().any(|entry|
        entry.record().occurrence_id == dependency
            && entry.role() == FamilyTraceRole::CalculationDependency
            && matches!(entry.static_source().map(|source| &source.source),
                Some(DynamicStaticSource::Programmer { programmer_id, lane: DynamicStaticProgrammerLane::Preload })
                    if *programmer_id == rig.key.programmer)
    ));
    for sidecar in &branch.sidecars {
        for entry in sidecar.sources.entries().unwrap() {
            assert_eq!(
                branch.origins.get(entry.record().occurrence_id),
                Some(entry.record())
            );
        }
    }
}

#[test]
fn retained_preset_capture_precedes_later_definition_and_cold_generation() {
    let mut definition_a = pan_definition();
    preset_pan(&mut definition_a, 25.);
    let definition = definition_a.id;
    let lane = definition_a.lanes[0].id;
    let rig = Rig::with_definition(definition_a.clone());
    let mut pair = rig.pair();
    let mut evaluator = PresetAudit {
        inner: RetainedPreloadHybridEvaluator::new(
            &rig.engine,
            rig.key.programmer,
            Compatible,
            observe,
        ),
        before: vec![],
        after: vec![],
    };
    let capture_a = rig.capture();
    let snapshot_a = rig.engine.snapshot();

    // Publish newer authoritative definition/retained Preset template and cold
    // interval BEFORE consuming A. Do not start any Live controller.
    let mut definition_b = definition_a;
    definition_b.revision += 1;
    preset_pan(&mut definition_b, 75.);
    let mut live = rig.live.borrow_mut();
    let boundary = rig.publication.cold_boundary(&live).unwrap();
    let mut destination = (*snapshot_a).clone();
    destination.revision += 1;
    destination.dynamics = vec![definition_b].into();
    rig.engine.replace_snapshot(destination).unwrap();
    let snapshot_b = rig.engine.snapshot();
    live.install_definitions(snapshot_b.dynamics.iter().cloned())
        .unwrap();
    crate::runtime::output_scheduler::materialize_cold_preset_dependencies(
        &snapshot_a,
        &snapshot_b,
        &mut live,
    )
    .unwrap();
    let event = boundary.prepare(snapshot_a, snapshot_b, &live);
    rig.publication
        .installed_with_cold_event(rig.engine.snapshot(), Some(event));
    drop(live);

    let outcome = rig.consume(&mut pair, capture_a.clone(), &mut evaluator);
    assert_eq!(outcome.successful_attempts, 1);
    assert!(outcome.failed_attempts.is_empty());
    assert!(outcome.stopped.is_none());
    let first = pair.last_success().unwrap();
    assert!(Arc::ptr_eq(&first.capture, &capture_a));
    assert!(first.value.rendered.before_release.is_none());
    assert_preset_branch(
        &rig,
        &first.value.before,
        &evaluator.before,
        25.,
        definition,
        lane,
    );
    assert_preset_branch(
        &rig,
        &first.value.after,
        &evaluator.after,
        25.,
        definition,
        lane,
    );
    let old_catalogues = [
        first.value.before.origins.clone(),
        first.value.after.origins.clone(),
    ];
    let old_snapshots = old_catalogues.each_ref().map(|origins| origins.snapshot());
    let old_tables = [
        evaluator.before[0].last_valid.clone(),
        evaluator.after[0].last_valid.clone(),
    ];

    rig.clock.advance_millis(40);
    let capture_b = rig.capture();
    let (before, after) = pair.positions();
    let before_cold = rig.publication.cold_generations_since(before.cold).unwrap();
    let after_cold = rig.publication.cold_generations_since(after.cold).unwrap();
    assert_eq!(before_cold.len(), 1);
    assert_eq!(after_cold.len(), 1);
    let live = rig.live.borrow();
    let before_controls = live.controls_since(before.controls).unwrap().unwrap();
    let after_controls = live.controls_since(after.controls).unwrap().unwrap();
    let window = pair
        .prepare_window(
            &[capture_b.clone()],
            &before_cold,
            &before_controls,
            &after_cold,
            &after_controls,
            limits(),
        )
        .unwrap();
    drop(live);
    let outcome = pair.consume_window(window, &mut evaluator);
    assert_eq!(outcome.successful_attempts, 1);
    assert!(outcome.failed_attempts.is_empty());
    assert!(outcome.stopped.is_none());
    let second = pair.last_success().unwrap();
    assert!(Arc::ptr_eq(&second.capture, &capture_b));
    assert_preset_branch(
        &rig,
        &second.value.before,
        &evaluator.before,
        75.,
        definition,
        lane,
    );
    assert_preset_branch(
        &rig,
        &second.value.after,
        &evaluator.after,
        75.,
        definition,
        lane,
    );
    for index in 0..2 {
        assert_eq!(old_catalogues[index].snapshot(), old_snapshots[index]);
        assert!(
            old_tables[index]
                .iter()
                .flat_map(|r| &r.values)
                .all(|v| v.value == DynamicValue::Scalar(25.))
        );
    }
    assert!(rig.live.borrow().snapshot().instances.is_empty());
}
