//! The pending producer uses one captured branch, with independent runtime/catalogue state.
//! These are adapter tests; physical fitting and publication scheduling remain separate gates.
use super::super::hybrid::*;
use super::*;
use crate::runtime::dynamic_source_origins::{
    DynamicFixedSource, DynamicProgrammerSourceLane, DynamicRuntimeSourceCheckpoint,
    DynamicStaticProgrammerLane,
};
use light_engine::{
    FamilyProjectionEvidence, FamilyProjectionMetadata, PreloadBranch, PreloadFrameState,
    PreparedPreloadFrame,
};

struct CompatibleAngles;
impl HybridFrameResolver for CompatibleAngles {
    fn adopt(
        &self,
        _: HybridFrameContext<'_>,
        _: FixtureId,
        _: &AttributeValue,
        _: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("compatible pending Angles must not ask for physical adoption")
    }
}

struct Rig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    target: FixtureId,
    clock: Arc<ManualClock>,
}

impl Rig {
    fn new() -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        let target = FixtureId::new();
        programmers.start(session);
        programmers.set(
            session,
            target,
            ProgrammingOwner::Position.key(),
            position(10., 20.),
        );
        let mut definition = pan_definition();
        let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
            unreachable!()
        };
        let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
            unreachable!()
        };
        for point in &mut configuration.points {
            point.source = DynamicValueSource::Current;
        }
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
                        last_known_pool_number: definition.pool_number,
                        embedded_fallback: DynamicDefinitionSnapshot {
                            definition: Arc::new(definition),
                        },
                    },
                    overrides: DynamicInstanceOverrides {
                        size: 1.,
                        speed_multiplier: Rational::ONE,
                        phase_offset_degrees: 0.,
                    },
                    timing: Default::default(),
                },
            },],
            None
        ));
        Self {
            engine,
            programmers,
            session,
            target,
            clock,
        }
    }

    fn runtime(&self) -> DynamicRuntime {
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        runtime
            .install_definitions(self.engine.snapshot().dynamics.iter().cloned())
            .unwrap();
        runtime
    }
}

#[derive(Clone, Copy)]
enum InputRows {
    Matching,
    Live,
    OtherBranch,
}

type Sidecar = (
    ProgrammingOwner,
    AttributeValue,
    DynamicFamilySourceProjection,
);

#[allow(clippy::too_many_arguments)]
fn prepare(
    engine: &Engine,
    input: &PreparedPreloadFrame<'_>,
    state: &PreloadFrameState,
    branch: PreloadBranch,
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    scratch: &mut HybridFrameScratch,
    rows: InputRows,
    fail_observer: bool,
) -> Result<PreparedHybridFrame<Sidecar>, DynamicRuntimeError> {
    let mut candidate_origins = origins.clone();
    let result = runtime.with_output_frame_transaction(
        &mut DynamicOutputFrameScratch::default(),
        |runtime| {
            prepare_in_transaction(
                engine,
                input,
                state,
                branch,
                runtime,
                &mut candidate_origins,
                scratch,
                rows,
                fail_observer,
            )
        },
    );
    if result.is_ok() {
        *origins = candidate_origins;
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn prepare_in_transaction(
    engine: &Engine,
    input: &PreparedPreloadFrame<'_>,
    state: &PreloadFrameState,
    branch: PreloadBranch,
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    scratch: &mut HybridFrameScratch,
    rows: InputRows,
    fail_observer: bool,
) -> Result<PreparedHybridFrame<Sidecar>, DynamicRuntimeError> {
    let capture = input.frame();
    let snapshot = capture.snapshot();
    let addresser = capture.frame_addresser();
    let speeds = [DynamicSpeedTransport {
        effective_bpm: 120.,
        phase_origin_millis: 0,
        phase_reference_millis: 0,
        beat_phase: 0.,
        phase_advancing: true,
    }; 5];
    let before = match rows {
        InputRows::Matching => branch == PreloadBranch::BeforeRelease,
        InputRows::OtherBranch => branch != PreloadBranch::BeforeRelease,
        InputRows::Live => false,
    };
    let (values, source_rows) = if matches!(rows, InputRows::Live) {
        (
            capture.dynamic_programmer_values(),
            capture.dynamic_programmer_rows(),
        )
    } else if before {
        (
            &input.sources().dynamic_values_before,
            &input.sources().dynamic_rows_before,
        )
    } else {
        (
            &input.sources().dynamic_values_after,
            &input.sources().dynamic_rows_after,
        )
    };
    let inputs = CapturedDynamicInputs {
        now: capture.sampled_at(),
        speed_transports: &speeds,
        rate: 40,
        snapshot: &snapshot,
        programmer_values: values,
        programmer_rows: Some(source_rows),
        cue_values: input.cue_dynamic_values(),
        dynamic_playbacks: input.dynamic_playbacks(),
        playback_paused: input.playback_dynamics_paused(),
        addresser: &addresser,
        extra_programmer_values: &[],
        programmer_reconciliation_cache: None,
        force_source_reconciliation: false,
    };
    prepare_captured_preload_hybrid_frame(
        engine,
        input,
        state,
        branch,
        &[],
        runtime,
        origins,
        &inputs,
        scratch,
        &CompatibleAngles,
        None,
        |observation| {
            let mut projection = DynamicFamilySourceProjection::default();
            observation.project_fields(
                &ProgrammingFieldScope::for_value(observation.owner, observation.value)?,
                &mut projection,
            )?;
            if fail_observer {
                return Err(IntentError("injected pending observer failure".into()).into());
            }
            Ok((
                FamilyProjectionMetadata {
                    changed_at: None,
                    evidence: FamilyProjectionEvidence::Replace {
                        origin: None,
                        family_evidence: None,
                    },
                },
                (observation.owner, observation.value.clone(), projection),
            ))
        },
    )
}

#[test]
fn pending_current_pair_and_sources_stay_captured_after_later_edits_and_finalize_isolated() {
    let rig = Rig::new();
    let capture = rig.engine.prepare_output_frame(Default::default());
    let live = rig.engine.prepare_static_family_frame(&capture, &[]);
    assert_eq!(
        live.value(rig.target, &ProgrammingOwner::Position.key()),
        Some(&position(10., 20.))
    );
    let input = rig.engine.prepare_preload_frame(&capture, None);
    let mut state = PreloadFrameState::default();
    let mut runtime = rig.runtime();
    let mut origins = DynamicSourceOrigins::default();
    rig.clock.advance_millis(500);
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Position.key(),
        position(720., -90.),
    );
    let prepared = prepare(
        &rig.engine,
        &input,
        &state,
        PreloadBranch::AfterRelease,
        &mut runtime,
        &mut origins,
        &mut HybridFrameScratch::default(),
        InputRows::Matching,
        false,
    )
    .unwrap();
    assert!(prepared.requirements.is_empty());
    assert_eq!(
        prepared.sampled.samples.len(),
        2,
        "Pan and its static Tilt partner sample once"
    );
    assert_eq!(prepared.family_sidecars.len(), 1);
    let (owner, value, projection) = &prepared.family_sidecars[0];
    assert_eq!(*owner, ProgrammingOwner::Position);
    assert_eq!(value, &position(60., 40.));
    let entries = projection
        .entries()
        .expect("captured Current has source evidence");
    assert!(!entries.is_empty());
    assert!(entries.iter().all(|entry| matches!(
        entry.static_source().map(|source| &source.source),
        Some(DynamicStaticSource::Programmer {
            lane: DynamicStaticProgrammerLane::Preload,
            ..
        })
    )));
    // Both lanes pass through Current: their appearance belongs to the captured static
    // source, not a fabricated authored Dynamic leaf. Controller ownership is separate.
    assert!(
        !origins
            .snapshot()
            .records
            .iter()
            .any(|record| matches!(record.origin, DynamicSourceOrigin::Programmer { .. }))
    );
    DynamicRuntimeSourceCheckpoint::capture(runtime.snapshot(), &origins).unwrap();
    let rendered = rig
        .engine
        .render_prepared_preload_families(&input, None, prepared.token, &mut state)
        .unwrap();
    assert_eq!(rendered.source.sampled_at(), capture.sampled_at());
    assert_eq!(
        rendered
            .source
            .values()
            .value(rig.target, &ProgrammingOwner::Position.key()),
        Some(value)
    );
    // An accepted pending branch must not advance Live's continuity revision.
    let live = rig
        .engine
        .render_static_family_frame(&capture, live)
        .unwrap();
    assert_eq!(
        live.resolved_values
            .value(rig.target, &ProgrammingOwner::Position.key()),
        Some(&position(10., 20.))
    );
}

#[test]
fn pending_fixed_color_release_uses_separate_branch_sources_and_retires_only_after_branch() {
    let rig = Rig::new();
    let baseline = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Color.key(),
        baseline,
    );
    let color = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent {
            uv: UvIntent { amount: 0.8 },
            ..Default::default()
        },
    }));
    let fixed = DynamicSemanticValue::ProgrammingFixAt {
        mask: ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Color, None, color.clone())
            .unwrap(),
        timing: Default::default(),
    };
    for value in [fixed, DynamicSemanticValue::Release] {
        assert!(rig.programmers.apply_dynamic_values(
            rig.session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: rig.target,
                attribute: ProgrammingOwner::Color.key(),
                value,
            },],
            None
        ));
    }
    let capture = rig.engine.prepare_output_frame(Default::default());
    let input = rig.engine.prepare_preload_frame(&capture, None);
    let mut state = PreloadFrameState::default();
    assert!(rig.engine.preload_requires_before_release(&input, &state));
    let mut before_runtime = rig.runtime();
    let mut before_origins = DynamicSourceOrigins::default();
    let before = prepare(
        &rig.engine,
        &input,
        &state,
        PreloadBranch::BeforeRelease,
        &mut before_runtime,
        &mut before_origins,
        &mut HybridFrameScratch::default(),
        InputRows::Matching,
        false,
    )
    .unwrap();
    assert!(before.requirements.is_empty());
    assert_eq!(
        before
            .family_sidecars
            .iter()
            .find(|row| row.0 == ProgrammingOwner::Color)
            .map(|row| &row.1),
        Some(&color)
    );
    let projection = &before
        .family_sidecars
        .iter()
        .find(|row| row.0 == ProgrammingOwner::Color)
        .unwrap()
        .2;
    let fixed_record = projection
        .entries()
        .unwrap()
        .iter()
        .find_map(|entry| {
            matches!(
                entry.record().binding,
                DynamicSourceBinding::Fixed {
                    source: DynamicFixedSource::Programmer {
                        lane: DynamicProgrammerSourceLane::Preload,
                        ..
                    },
                    owner: ProgrammingOwner::Color,
                    ..
                }
            )
            .then(|| Arc::clone(entry.record()))
        })
        .expect("before Release keeps the actual pending Fixed source");
    // The after branch starts with the same retained catalogue but reconciles independently.
    let mut after_runtime = rig.runtime();
    let mut after_origins = before_origins.clone();
    let after = prepare(
        &rig.engine,
        &input,
        &state,
        PreloadBranch::AfterRelease,
        &mut after_runtime,
        &mut after_origins,
        &mut HybridFrameScratch::default(),
        InputRows::Matching,
        false,
    )
    .unwrap();
    assert!(after.requirements.is_empty());
    assert!(
        after
            .token
            .value(rig.target, &ProgrammingOwner::Color.key())
            .is_none()
    );
    assert!(after_origins.binding(&fixed_record.binding).is_none());
    assert_eq!(
        before_origins.binding(&fixed_record.binding),
        Some(fixed_record.occurrence_id)
    );
    assert_eq!(
        after_origins.get(fixed_record.occurrence_id),
        Some(&fixed_record)
    );
    let rendered = rig
        .engine
        .render_prepared_preload_families(&input, Some(before.token), after.token, &mut state)
        .unwrap();
    assert_eq!(
        rendered
            .before_release
            .unwrap()
            .values()
            .value(rig.target, &ProgrammingOwner::Color.key()),
        Some(&color)
    );
    assert!(
        rendered
            .source
            .values()
            .value(rig.target, &ProgrammingOwner::Color.key())
            .is_none()
    );
}

#[test]
fn pending_hybrid_rejects_live_and_other_branch_rows_before_runtime_or_source_mutation() {
    let rig = Rig::new();
    let capture = rig.engine.prepare_output_frame(Default::default());
    let input = rig.engine.prepare_preload_frame(&capture, None);
    let state = PreloadFrameState::default();
    let mut runtime = rig.runtime();
    let mut origins = DynamicSourceOrigins::default();
    let initial_runtime = runtime.snapshot();
    let initial_origins = origins.snapshot();
    for rows in [InputRows::Live, InputRows::OtherBranch] {
        for branch in [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease] {
            assert!(
                prepare_in_transaction(
                    &rig.engine,
                    &input,
                    &state,
                    branch,
                    &mut runtime,
                    &mut origins,
                    &mut HybridFrameScratch::default(),
                    rows,
                    false,
                )
                .is_err()
            );
            assert_eq!(runtime.snapshot(), initial_runtime);
            assert_eq!(origins.snapshot(), initial_origins);
        }
    }
}

#[test]
fn pending_release_cannot_reintroduce_a_normal_programmer_color_fixed_mask() {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let target = FixtureId::new();
    programmers.start(session);
    programmers.set(
        session,
        target,
        ProgrammingOwner::Color.key(),
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent::default(),
        })),
    );
    let color = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent {
            uv: UvIntent { amount: 0.7 },
            ..Default::default()
        },
    }));
    let fixed = DynamicSemanticValue::ProgrammingFixAt {
        mask: ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Color, None, color.clone())
            .unwrap(),
        timing: Default::default(),
    };
    let set = |value| light_programmer::DynamicProgrammerValueMutation::Set {
        fixture_id: target,
        attribute: ProgrammingOwner::Color.key(),
        value,
    };
    assert!(programmers.apply_dynamic_values(session, &[set(fixed)], None));
    programmers.arm_preload(session, true);
    clock.advance_millis(1);
    assert!(programmers.apply_dynamic_values(session, &[set(DynamicSemanticValue::Release)], None));
    let engine =
        Engine::with_programming_contract_support(programmers, PROGRAMMING_CONTRACT_VERSION);
    let capture = engine.prepare_output_frame(Default::default());
    let input = engine.prepare_preload_frame(&capture, None);
    let mut state = PreloadFrameState::default();
    let mut before_runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    let mut before_origins = DynamicSourceOrigins::default();
    let before = prepare(
        &engine,
        &input,
        &state,
        PreloadBranch::BeforeRelease,
        &mut before_runtime,
        &mut before_origins,
        &mut HybridFrameScratch::default(),
        InputRows::Matching,
        false,
    )
    .unwrap();
    assert_eq!(
        before
            .family_sidecars
            .iter()
            .find(|row| row.0 == ProgrammingOwner::Color)
            .map(|row| &row.1),
        Some(&color)
    );
    assert_eq!(before.family_sidecars.len(), 1);
    let mut after_runtime = before_runtime.fork_for_pending_preview();
    let mut after_origins = before_origins.clone();
    let after = prepare(
        &engine,
        &input,
        &state,
        PreloadBranch::AfterRelease,
        &mut after_runtime,
        &mut after_origins,
        &mut HybridFrameScratch::default(),
        InputRows::Matching,
        false,
    )
    .unwrap();
    assert!(after.requirements.is_empty());
    assert!(
        after.family_sidecars.is_empty(),
        "released normal Fixed must not be composed again"
    );
    assert!(
        after
            .token
            .value(target, &ProgrammingOwner::Color.key())
            .is_none()
    );
    let rendered = engine
        .render_prepared_preload_families(&input, Some(before.token), after.token, &mut state)
        .unwrap();
    assert!(
        rendered
            .source
            .values()
            .value(target, &ProgrammingOwner::Color.key())
            .is_none()
    );
    assert_eq!(
        rendered
            .before_release
            .unwrap()
            .values()
            .value(target, &ProgrammingOwner::Color.key()),
        Some(&color)
    );
}

#[test]
fn pending_hybrid_tokens_cannot_finalize_through_live_or_the_wrong_branch() {
    let rig = Rig::new();
    let capture = rig.engine.prepare_output_frame(Default::default());
    let live = rig.engine.prepare_static_family_frame(&capture, &[]);
    let input = rig.engine.prepare_preload_frame(&capture, None);
    let mut state = PreloadFrameState::default();
    for branch in [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease] {
        let prepared = prepare(
            &rig.engine,
            &input,
            &state,
            branch,
            &mut rig.runtime(),
            &mut DynamicSourceOrigins::default(),
            &mut HybridFrameScratch::default(),
            InputRows::Matching,
            false,
        )
        .unwrap();
        assert!(
            rig.engine
                .render_static_family_frame(&capture, prepared.token)
                .is_err()
        );
    }
    let before = prepare(
        &rig.engine,
        &input,
        &state,
        PreloadBranch::BeforeRelease,
        &mut rig.runtime(),
        &mut DynamicSourceOrigins::default(),
        &mut HybridFrameScratch::default(),
        InputRows::Matching,
        false,
    )
    .unwrap();
    assert!(
        rig.engine
            .render_prepared_preload_families(&input, None, before.token, &mut state)
            .is_err()
    );
    rig.engine
        .render_static_family_frame(&capture, live)
        .unwrap();
}

#[test]
fn failed_pending_observer_rolls_back_caller_transaction_and_retry_keeps_captured_current() {
    let rig = Rig::new();
    let capture = rig.engine.prepare_output_frame(Default::default());
    let input = rig.engine.prepare_preload_frame(&capture, None);
    let state = PreloadFrameState::default();
    let mut runtime = rig.runtime();
    let mut origins = DynamicSourceOrigins::default();
    let original_runtime = runtime.snapshot();
    let original_origins = origins.snapshot();
    let mut hybrid = HybridFrameScratch::default();
    let mut transaction = DynamicOutputFrameScratch::default();
    for fail in [true, false] {
        let mut candidate_origins = origins.clone();
        let result = runtime.with_output_frame_transaction(&mut transaction, |runtime| {
            prepare_in_transaction(
                &rig.engine,
                &input,
                &state,
                PreloadBranch::AfterRelease,
                runtime,
                &mut candidate_origins,
                &mut hybrid,
                InputRows::Matching,
                fail,
            )
        });
        if fail {
            assert!(result.is_err());
            assert_eq!(runtime.snapshot(), original_runtime);
            assert_eq!(origins.snapshot(), original_origins);
        } else {
            let prepared = result.unwrap();
            origins = candidate_origins;
            assert!(prepared.requirements.is_empty());
            assert_eq!(prepared.family_sidecars[0].1, position(60., 40.));
            DynamicRuntimeSourceCheckpoint::capture(runtime.snapshot(), &origins).unwrap();
        }
    }
}
