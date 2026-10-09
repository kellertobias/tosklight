//! Integration of captured scalar output, final Point geometry and typed Current. The explicit
//! adoption rule below is a test model, not a physical Position solver or fitting contract.
use super::super::hybrid::*;
use super::*;
use crate::runtime::dynamic_source_origins::DynamicRuntimeSourceCheckpoint;

const POINT_X: &str = "point.position.x";
const POINT_Y: &str = "point.position.y";

fn patched_package(name: &str) -> light_fixture::PatchedFixture {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library")
        .join(name);
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode = &profile.modes[0];
    let mut fixture: light_fixture::PatchedFixture = serde_json::from_value(serde_json::json!({
        "fixture_id": FixtureId::new(),
        "definition": profile.resolved_definition(mode.id).unwrap(),
    }))
    .unwrap();
    fixture.logical_heads = mode
        .heads
        .iter()
        .enumerate()
        .filter(|(_, head)| !head.master_shared)
        .map(|(index, head)| light_fixture::PatchedHead {
            profile_head_id: Some(head.id),
            head_index: index as u16,
            fixture_id: FixtureId::new(),
        })
        .collect();
    fixture
}

fn point_definition() -> DynamicDefinition {
    let mut definition = pan_definition();
    definition.id = Uuid::new_v4();
    definition.pool_number = 2;
    definition.name = "Point motion before typed Current".into();
    definition.lanes = [POINT_X, POINT_Y]
        .map(|attribute| DynamicLane {
            id: Uuid::new_v4(),
            body: DynamicLaneBody::LegacyScalar(LegacyScalarLaneBody {
                attribute: AttributeKey(attribute.into()),
                mode: DynamicLaneMode::Keyframes,
                keyframes: KeyframeConfiguration {
                    points: [0., 0.5]
                        .map(|position| DynamicKeyframe {
                            position,
                            source: ScalarSource::Value { value: 0.55 },
                            interpolation: light_dynamics::ScalarInterpolation::Linear,
                        })
                        .to_vec(),
                    size: 1.,
                },
                max_min: MaxMinConfiguration {
                    minimum: ScalarSource::Value { value: 0. },
                    maximum: ScalarSource::Value { value: 1. },
                    function: PeriodicFunction::Sinus,
                    size: 1.,
                    pwm: Default::default(),
                },
                middle_amplitude: MiddleAmplitudeConfiguration {
                    middle: ScalarSource::Current,
                    amplitude: 1.,
                    function: PeriodicFunction::Sinus,
                    size: 1.,
                    pwm: Default::default(),
                    invert_waveform: false,
                },
            }),
            speed_multiplier: Rational::ONE,
            width: 1.,
            phase: None,
            random_group_id: None,
        })
        .to_vec();
    definition
}

fn target(tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::Target {
        reference: TargetReference::Origin,
        offset_metres: [
            ScalarIntent::Value(0.),
            ScalarIntent::Value(tilt),
            ScalarIntent::Value(0.),
        ],
    }))
}

fn error(error: impl std::fmt::Display) -> DynamicRuntimeError {
    DynamicRuntimeError::InvalidSample(error.to_string())
}

fn assert_angles(value: &AttributeValue, expected_pan: f32, expected_tilt: f32) {
    let AttributeValue::Position(intent) = value else {
        panic!("expected Position, got {value:?}")
    };
    let PositionIntent::Angles {
        pan_degrees: ScalarIntent::Value(pan),
        tilt_degrees: ScalarIntent::Value(tilt),
    } = intent.as_ref()
    else {
        panic!("expected complete materialized Angles, got {intent:?}")
    };
    assert!(
        (*pan - expected_pan).abs() < 1e-4,
        "Pan {pan} != {expected_pan}"
    );
    assert!(
        (*tilt - expected_tilt).abs() < 1e-4,
        "Tilt {tilt} != {expected_tilt}"
    );
}

#[derive(Clone, Copy)]
enum Failure {
    None,
    Callback,
    FinalCapture,
}

struct TestFrameResolver<'a> {
    mount: FixtureId,
    original: &'a AttributeValue,
    failure: Failure,
    adoptions: RefCell<Vec<(DynamicValueAddress, usize)>>,
}

impl HybridFrameResolver for TestFrameResolver<'_> {
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        fixture: FixtureId,
        value: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        assert_eq!(fixture, self.mount);
        assert_eq!(
            value, self.original,
            "typed Current must retain the original pre-Dynamic static owner"
        );
        assert_eq!(frame.geometry.sampled_at(), frame.capture.sampled_at());
        assert!(
            (frame.geometry.points()[0].offset_metres[0] - 20.).abs() < 0.0001,
            "frozen Point pose: {:?}",
            frame.geometry.points()
        );
        assert!(
            (frame.geometry.points()[0].offset_metres[1] - 10.).abs() < 0.0001,
            "dynamic Point pose: {:?}",
            frame.geometry.points()
        );
        let world = frame
            .geometry
            .mounts()
            .mount(fixture.0)
            .unwrap()
            .world_from_fixture
            .unwrap()
            .point([0.; 3]);
        let AttributeValue::Position(intent) = value else {
            unreachable!()
        };
        let PositionIntent::Target { offset_metres, .. } = intent.as_ref() else {
            unreachable!()
        };
        let ScalarIntent::Value(tilt) = offset_metres[1] else {
            unreachable!()
        };
        let mut counts = self.adoptions.borrow_mut();
        if let Some((_, calls)) = counts.iter_mut().find(|(known, _)| known == address) {
            *calls += 1;
        } else {
            counts.push((address.clone(), 1));
        }
        if matches!(self.failure, Failure::Callback) {
            return Err(IntentError("injected geometry callback failure".into()).into());
        }
        // Explicit integration-test model, not inverse aiming or a claim of source transfer.
        Ok(position((world[0] + world[1]) as f32, tilt))
    }
}

struct Harness {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    clock: Arc<ManualClock>,
    point: FixtureId,
    point_owner: FixtureId,
    mount: FixtureId,
    runtime: DynamicRuntime,
    origins: DynamicSourceOrigins,
    transaction: DynamicOutputFrameScratch,
    hybrid: HybridFrameScratch,
}

impl Harness {
    fn new() -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        let mut point = patched_package("tosklight--3d-point.toskfixture");
        let point_owner = point
            .logical_heads
            .first()
            .map_or(point.fixture_id, |head| head.fixture_id);
        point
            .freeze
            .targets
            .entry(point_owner)
            .or_default()
            .values
            .insert(
                AttributeKey(POINT_X.into()),
                AttributeValue::Normalized(0.6),
            );
        let mut mount = patched_package("jb-lighting--jbled-a7.toskfixture");
        mount.position_master = Some(point.fixture_id.0);
        mount.location.x = 2_000;
        mount.location.y = 3_000;
        let mount_id = mount.fixture_id;
        let mut pan = pan_definition();
        let DynamicLaneBody::Programming(body) = &mut pan.lanes[0].body else {
            unreachable!()
        };
        let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
            unreachable!()
        };
        for keyframe in &mut config.points {
            keyframe.source = DynamicValueSource::Current;
        }
        let point_dynamic = point_definition();
        let engine = Engine::with_programming_contract_support(
            programmers.clone(),
            PROGRAMMING_CONTRACT_VERSION,
        );
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![point.clone(), mount].into(),
                dynamics: vec![pan.clone(), point_dynamic.clone()].into(),
                ..Default::default()
            })
            .unwrap();
        for (definition, fixture, lanes) in [
            (&pan, mount_id, &pan.lanes[..1]),
            (&point_dynamic, point_owner, &point_dynamic.lanes[..]),
        ] {
            let link = Uuid::new_v4();
            let mutations = lanes
                .iter()
                .map(
                    |lane| light_programmer::DynamicProgrammerValueMutation::Set {
                        fixture_id: fixture,
                        attribute: lane.output_owner(),
                        value: DynamicSemanticValue::DynamicOn {
                            instance_link: link,
                            lane_id: lane.id,
                            dynamic: DynamicReference {
                                dynamic_id: Some(definition.id),
                                last_known_pool_number: definition.pool_number,
                                embedded_fallback: DynamicDefinitionSnapshot {
                                    definition: Arc::new(definition.clone()),
                                },
                            },
                            overrides: DynamicInstanceOverrides {
                                size: 1.,
                                speed_multiplier: Rational::ONE,
                                phase_offset_degrees: 0.,
                            },
                            timing: Default::default(),
                        },
                    },
                )
                .collect::<Vec<_>>();
            assert!(programmers.apply_dynamic_values(session, &mutations, None));
        }
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        runtime
            .install_definitions(engine.snapshot().dynamics.iter().cloned())
            .unwrap();
        Self {
            engine,
            programmers,
            session,
            clock,
            point: point.fixture_id,
            point_owner,
            mount: mount_id,
            runtime,
            origins: Default::default(),
            transaction: Default::default(),
            hybrid: Default::default(),
        }
    }

    fn capture(&self, tilt: f32, paused: bool) -> light_engine::PreparedOutputFrame {
        self.engine.set_tracked_overrides([]);
        // The shipped Point's neutral defaults underlie the legacy Dynamic. A newer scalar
        // Programmer AT would correctly outbid its older activation stamp under legacy LTP.
        self.programmers.set(
            self.session,
            self.mount,
            ProgrammingOwner::Position.key(),
            target(tilt),
        );
        self.engine
            .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
                paused,
            ))
            .unwrap();
        self.engine.prepare_output_frame(Default::default())
    }

    fn run(
        &mut self,
        capture: &light_engine::PreparedOutputFrame,
        failure: Failure,
        expected_tilt: f32,
    ) -> Result<
        (
            RenderResult,
            light_engine::PreparedFrameGeometry,
            AttributeValue,
        ),
        DynamicRuntimeError,
    > {
        let engine = &self.engine;
        let original = target(expected_tilt);
        // Neither the original static Current nor final Point geometry may read these later edits.
        self.programmers.set(
            self.session,
            self.mount,
            ProgrammingOwner::Position.key(),
            target(123.),
        );
        engine.set_tracked_overrides([POINT_X, POINT_Y].map(|attribute| {
            light_engine::TrackedOverride::new(
                self.point,
                AttributeKey(attribute.into()),
                AttributeValue::Normalized(0.99),
            )
        }));
        let snapshot = capture.snapshot();
        let addresser = capture.frame_addresser();
        let speeds = [DynamicSpeedTransport {
            effective_bpm: 120.,
            phase_origin_millis: 0,
            phase_reference_millis: 0,
            beat_phase: 0.,
            phase_advancing: true,
        }; 5];
        let inputs = CapturedDynamicInputs {
            now: capture.sampled_at(),
            speed_transports: &speeds,
            rate: 40,
            snapshot: &snapshot,
            programmer_values: capture.dynamic_programmer_values(),
            programmer_rows: Some(capture.dynamic_programmer_rows()),
            cue_values: capture.cue_dynamic_values(),
            dynamic_playbacks: capture.dynamic_playbacks(),
            playback_paused: capture.playback_dynamics_paused(),
            addresser: &addresser,
            extra_programmer_values: &[],
            programmer_reconciliation_cache: None,
            force_source_reconciliation: false,
        };
        let resolver = TestFrameResolver {
            mount: self.mount,
            original: &original,
            failure,
            adoptions: RefCell::new(Vec::new()),
        };
        let mut candidate_origins = self.origins.clone();
        let point_owner = self.point_owner;
        let scratch = &mut self.hybrid;
        let output = self
            .runtime
            .with_output_frame_transaction(&mut self.transaction, |runtime| {
                let prepared = prepare_captured_hybrid_frame(
                    engine,
                    capture,
                    &[],
                    runtime,
                    &mut candidate_origins,
                    &inputs,
                    scratch,
                    &resolver,
                    None,
                    |observation| {
                        assert_eq!(observation.target, resolver.mount);
                        assert_eq!(observation.owner, ProgrammingOwner::Position);
                        let mut sources = DynamicFamilySourceProjection::default();
                        observation.project_fields(
                            &ProgrammingFieldScope::for_value(
                                observation.owner,
                                observation.value,
                            )?,
                            &mut sources,
                        )?;
                        let controls = observation
                            .controls_for_fields(&ProgrammingFieldScope::from_component(
                                ProgrammingComponent::Tilt,
                            ))
                            .expect("Current Tilt retains known controller coverage");
                        assert_eq!(controls.len(), 1);
                        assert_eq!(
                            observation
                                .static_baseline
                                .value(resolver.mount, &ProgrammingOwner::Position.key()),
                            Some(&original)
                        );
                        Ok((
                            light_engine::FamilyProjectionMetadata {
                                changed_at: None,
                                evidence: light_engine::FamilyProjectionEvidence::Replace {
                                    origin: None,
                                    family_evidence: None,
                                },
                            },
                            (observation.value.clone(), sources),
                        ))
                    },
                )?;
                assert!(prepared.requirements.is_empty());
                assert_eq!(
                    prepared.family_sidecars.len(),
                    1,
                    "the complete Angle pair composes once"
                );
                let (value, _sources) = prepared.family_sidecars.into_iter().next().unwrap();
                let resolved_y = prepared
                    .token
                    .value(point_owner, &AttributeKey(POINT_Y.into()))
                    .unwrap()
                    .normalized()
                    .unwrap();
                assert!(
                    (resolved_y - 0.55).abs() < 1e-6,
                    "Point Y: {resolved_y}; scalar samples: {:?}",
                    prepared.sampled.samples
                );
                assert_eq!(
                    prepared
                        .sampled
                        .samples
                        .iter()
                        .filter(|sample| sample.legacy().is_some())
                        .count(),
                    2
                );
                assert_eq!(
                    prepared.sampled.after_runtime.global_paused,
                    inputs.playback_paused
                );
                DynamicRuntimeSourceCheckpoint::capture(runtime.snapshot(), &candidate_origins)
                    .map_err(error)?;
                // The helper has already returned through its completion proof and source retirement.
                // Only this caller's final operation may commit engine continuity.
                let rendered = if matches!(failure, Failure::FinalCapture) {
                    let other = engine.prepare_output_frame(Default::default());
                    engine.render_static_family_frame(&other, prepared.token)
                } else {
                    engine.render_static_family_frame(capture, prepared.token)
                }
                .map_err(error)?;
                assert!(std::ptr::eq(
                    rendered.points.as_slice(),
                    prepared.geometry.points()
                ));
                assert!(std::ptr::eq(
                    rendered.mounts.mounts(),
                    prepared.geometry.mounts().mounts()
                ));
                Ok((rendered, prepared.geometry, value))
            });
        if output.is_ok() {
            self.origins = candidate_origins;
        }
        assert!(
            resolver
                .adoptions
                .borrow()
                .iter()
                .all(|(_, calls)| *calls == 1),
            "adoption must be cached by address"
        );
        output
    }
}

#[test]
fn staged_point_freeze_and_current_share_one_capture_and_paused_pair_uses_live_tilt() {
    let mut h = Harness::new();
    let mut held = None;
    for (index, tilt) in [15., -45.].into_iter().enumerate() {
        h.clock.advance_millis(25);
        let capture = h.capture(tilt, index > 0);
        let (output, geometry, value) = h.run(&capture, Failure::None, tilt).unwrap();
        assert_angles(&value, 35., tilt);
        assert_eq!(
            output
                .resolved_values
                .value(h.mount, &ProgrammingOwner::Position.key()),
            Some(&value)
        );
        assert!(!output.resolved_values.materialised_by_name());
        if let Some((old, old_value)) = &held {
            let old: &light_engine::PreparedFrameGeometry = old;
            assert!((old.points()[0].offset_metres[1] - 10.).abs() < 0.0001);
            assert_angles(old_value, 35., 15.);
        } else {
            held = Some((geometry, value));
        }
    }
}

#[test]
fn failed_geometry_callback_and_final_render_rejection_roll_back_staged_history_and_sources() {
    let mut h = Harness::new();
    let capture = h.capture(15., false);
    let before = h.runtime.snapshot();
    let origins = h.origins.snapshot();
    for failure in [Failure::Callback, Failure::FinalCapture] {
        assert!(h.run(&capture, failure, 15.).is_err());
        assert_eq!(h.runtime.snapshot(), before);
        assert_eq!(h.origins.snapshot(), origins);
    }
    let (output, _, value) = h.run(&capture, Failure::None, 15.).unwrap();
    assert_angles(&value, 35., 15.);
    assert_eq!(output.sampled_at, capture.sampled_at());
}

struct UnavailableGeometry;
impl HybridFrameResolver for UnavailableGeometry {
    fn adopt(
        &self,
        _: HybridFrameContext<'_>,
        _: FixtureId,
        _: &AttributeValue,
        _: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    }
}

struct UntrustedAddresser;
impl light_core::FrameAddressResolver for UntrustedAddresser {
    fn generation(&self) -> u64 {
        panic!("the helper must use the capture's index")
    }
    fn frame_address(&self, _: FixtureId, _: &AttributeKey) -> Option<light_core::FrameAddress> {
        panic!("the helper must use the capture's index")
    }
}

fn simple_prepare(
    engine: &Engine,
    frame: &light_engine::PreparedOutputFrame,
    runtime: &mut DynamicRuntime,
    input_time: chrono::DateTime<chrono::Utc>,
) -> Result<
    PreparedHybridFrame<(
        ProgrammingOwner,
        AttributeValue,
        DynamicFamilySourceProjection,
    )>,
    DynamicRuntimeError,
> {
    let snapshot = frame.snapshot();
    let speeds = [DynamicSpeedTransport {
        effective_bpm: 120.,
        phase_origin_millis: 0,
        phase_reference_millis: 0,
        beat_phase: 0.,
        phase_advancing: true,
    }; 5];
    let inputs = CapturedDynamicInputs {
        now: input_time,
        speed_transports: &speeds,
        rate: 40,
        snapshot: &snapshot,
        programmer_values: frame.dynamic_programmer_values(),
        programmer_rows: Some(frame.dynamic_programmer_rows()),
        cue_values: frame.cue_dynamic_values(),
        dynamic_playbacks: frame.dynamic_playbacks(),
        playback_paused: frame.playback_dynamics_paused(),
        addresser: &UntrustedAddresser,
        extra_programmer_values: &[],
        programmer_reconciliation_cache: None,
        force_source_reconciliation: false,
    };
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    runtime.with_output_frame_transaction(&mut DynamicOutputFrameScratch::default(), |runtime| {
        prepare_captured_hybrid_frame(
            engine,
            frame,
            &[],
            runtime,
            &mut origins,
            &inputs,
            &mut scratch,
            &UnavailableGeometry,
            None,
            |observation| {
                let mut projection = DynamicFamilySourceProjection::default();
                observation.project_fields(
                    &ProgrammingFieldScope::for_value(observation.owner, observation.value)?,
                    &mut projection,
                )?;
                Ok((
                    light_engine::FamilyProjectionMetadata {
                        changed_at: None,
                        evidence: light_engine::FamilyProjectionEvidence::Replace {
                            origin: None,
                            family_evidence: None,
                        },
                    },
                    (observation.owner, observation.value.clone(), projection),
                ))
            },
        )
    })
}

fn set_fixed(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    fixture: FixtureId,
    owner: ProgrammingOwner,
    family: AttributeValue,
    fade_millis: u64,
) {
    assert!(programmers.apply_dynamic_values(
        session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: owner.key(),
            value: DynamicSemanticValue::ProgrammingFixAt {
                mask: ProgrammingFamilyFixAt::from_family(owner, None, family).unwrap(),
                timing: DynamicValueTiming {
                    fade_millis: Some(fade_millis),
                    delay_millis: None
                },
            },
        }],
        None
    ));
}

fn set_on(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    fixture: FixtureId,
    definition: &DynamicDefinition,
) {
    assert!(programmers.apply_dynamic_values(
        session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: definition.lanes[0].output_owner(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::new_v4(),
                lane_id: definition.lanes[0].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: definition.pool_number,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone())
                    }
                },
                overrides: DynamicInstanceOverrides {
                    size: 1.,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.
                },
                timing: Default::default(),
            },
        }],
        None
    ));
}

fn legacy_definition(attribute: &str, value: f32) -> DynamicDefinition {
    let mut definition = point_definition();
    definition.lanes.truncate(1);
    let DynamicLaneBody::LegacyScalar(body) = &mut definition.lanes[0].body else {
        unreachable!()
    };
    body.attribute = AttributeKey(attribute.into());
    for point in &mut body.keyframes.points {
        point.source = ScalarSource::Value { value };
    }
    definition
}

pub(super) fn test_cue_list(cues: Vec<light_playback::Cue>) -> light_playback::CueList {
    light_playback::CueList {
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
        id: light_core::CueListId::new(),
        name: "Hybrid sources".into(),
        priority: 10,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.,
        cues,
    }
}

pub(super) fn test_playback(
    cue_list_id: light_core::CueListId,
) -> light_playback::PlaybackDefinition {
    light_playback::PlaybackDefinition {
        number: 1,
        name: "Hybrid sources".into(),
        target: light_playback::PlaybackTarget::CueList { cue_list_id },
        buttons: [
            light_playback::PlaybackButtonAction::GoMinus,
            light_playback::PlaybackButtonAction::Go,
            light_playback::PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}

#[test]
fn hybrid_partial_fixed_is_applied_once_while_unresolved_position_remains_passive() {
    let mut h = Harness::new();
    // The existing Pan Current needs unavailable geometry; Focus is independent and must progress.
    h.programmers.set(
        h.session,
        h.mount,
        ProgrammingOwner::Focus.key(),
        AttributeValue::Normalized(0.2),
    );
    set_fixed(
        &h.programmers,
        h.session,
        h.mount,
        ProgrammingOwner::Focus,
        AttributeValue::Normalized(1.),
        1_000,
    );
    h.clock.advance_millis(500);
    let frame = h.capture(15., false);
    let prepared = simple_prepare(&h.engine, &frame, &mut h.runtime, frame.sampled_at()).unwrap();
    assert_eq!(prepared.family_sidecars.len(), 1);
    let (owner, value, sources) = &prepared.family_sidecars[0];
    assert_eq!(*owner, ProgrammingOwner::Focus);
    assert!(
        (value.normalized().unwrap() - 0.6).abs() < 1e-6,
        "one activation over .2, not a second fade over .6: {value:?}"
    );
    assert_eq!(
        sources.entries().unwrap().len(),
        2,
        "both fixed and static appearance remain traced"
    );
    assert!(
        prepared
            .requirements
            .iter()
            .any(|item| item.owner == ProgrammingOwner::Position)
    );
    assert!(
        prepared
            .requirements
            .iter()
            .all(|item| item.owner != ProgrammingOwner::Focus)
    );
    let output = h
        .engine
        .render_static_family_frame(&frame, prepared.token)
        .unwrap();
    assert_eq!(
        output
            .resolved_values
            .value(h.mount, &ProgrammingOwner::Focus.key()),
        Some(value)
    );
    assert_eq!(
        output
            .resolved_values
            .value(h.mount, &ProgrammingOwner::Position.key()),
        Some(&target(15.))
    );
}

#[test]
fn hybrid_rejects_source_inputs_from_another_sample_before_sampling() {
    let mut h = Harness::new();
    let frame = h.capture(15., false);
    let before = h.runtime.snapshot();
    let result = simple_prepare(
        &h.engine,
        &frame,
        &mut h.runtime,
        frame.sampled_at() + chrono::Duration::milliseconds(1),
    );
    assert!(
        matches!(result, Err(DynamicRuntimeError::InvalidSample(message)) if message.contains("do not belong"))
    );
    assert_eq!(h.runtime.snapshot(), before);
}

#[test]
fn hybrid_gates_legacy_pan_rgb_and_focus_against_typed_family_owners() {
    for (attribute, owner, base, fixed) in [
        (
            "pan",
            ProgrammingOwner::Position,
            position(0., 15.),
            position(90., -30.),
        ),
        (
            "color.red",
            ProgrammingOwner::Color,
            AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                intent: ColorIntent::default(),
            })),
            AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                intent: ColorIntent::default(),
            })),
        ),
        (
            "focus",
            ProgrammingOwner::Focus,
            AttributeValue::Normalized(0.2),
            AttributeValue::Normalized(0.9),
        ),
    ] {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        let fixture = FixtureId::new();
        programmers.start(session);
        programmers.set(session, fixture, owner.key(), base.clone());
        let mut cue = light_playback::Cue::new(1_u16.into());
        cue.dynamic_changes.push(light_playback::CueDynamicChange {
            fixture_id: fixture,
            attribute: owner.key(),
            automatic_restore: false,
            value: DynamicSemanticValue::ProgrammingFixAt {
                mask: ProgrammingFamilyFixAt::from_family(owner, None, fixed).unwrap(),
                timing: Default::default(),
            },
        });
        let cue_list = test_cue_list(vec![cue]);
        let playback = test_playback(cue_list.id);
        let definition = legacy_definition(attribute, 0.7);
        // Equal-time independent samples do not displace the existing static LTP winner.
        // Give this actual legacy edit a later clock so the preserved scalar lane is decisive.
        clock.advance_millis(1);
        set_on(&programmers, session, fixture, &definition);
        let engine =
            Engine::with_programming_contract_support(programmers, PROGRAMMING_CONTRACT_VERSION);
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                dynamics: vec![definition.clone()].into(),
                cue_lists: vec![cue_list].into(),
                playbacks: vec![playback].into(),
                ..Default::default()
            })
            .unwrap();
        engine
            .execute_playback(light_engine::EnginePlaybackCommand::Pool {
                number: 1,
                action: light_engine::PoolPlaybackAction::Go,
            })
            .unwrap();
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        runtime.install_definitions([definition]).unwrap();
        let frame = engine.prepare_output_frame(Default::default());
        let prepared = simple_prepare(&engine, &frame, &mut runtime, frame.sampled_at()).unwrap();
        assert!(
            prepared.family_sidecars.is_empty(),
            "{attribute} must not be overwritten by typed composition"
        );
        assert!(
            prepared.requirements.iter().any(|item| item.owner == owner
                && matches!(
                    item.reason,
                    HybridFamilyRequirementReason::LegacyOwnerOverlap
                )),
            "{attribute}"
        );
        assert_eq!(
            prepared
                .token
                .value(fixture, &AttributeKey(attribute.into())),
            Some(&AttributeValue::Normalized(0.7)),
            "legacy {attribute} remains in final scalar token"
        );
        if owner != ProgrammingOwner::Focus {
            assert_eq!(prepared.token.value(fixture, &owner.key()), Some(&base));
        }
    }
}

#[test]
fn hybrid_retains_final_move_in_black_underlay_when_scalar_intensity_changes_position() {
    use light_playback::{
        Cue, CueChange, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode,
    };
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    programmers.start(session);
    let mut fixture = patched_package("jb-lighting--jbled-a7.toskfixture");
    fixture.move_in_black_enabled = true;
    fixture.move_in_black_delay_millis = 0;
    let target = fixture
        .logical_heads
        .first()
        .map_or(fixture.fixture_id, |head| head.fixture_id);
    let cues = [(0., 0.), (1., 90.)]
        .into_iter()
        .enumerate()
        .map(|(index, (intensity, pan))| {
            let mut cue = Cue::new(((index + 1) as u16).into());
            cue.fade_millis = 0;
            cue.changes = vec![
                CueChange::set(
                    target,
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(intensity),
                ),
                CueChange::set(target, ProgrammingOwner::Position.key(), position(pan, 15.)),
            ];
            cue
        })
        .collect();
    let cue_list = CueList {
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
        id: light_core::CueListId::new(),
        name: "Hybrid MIB baseline".into(),
        priority: 10,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.,
        cues,
    };
    let playback = light_playback::PlaybackDefinition {
        number: 1,
        name: "Hybrid MIB".into(),
        target: light_playback::PlaybackTarget::CueList {
            cue_list_id: cue_list.id,
        },
        buttons: [
            light_playback::PlaybackButtonAction::GoMinus,
            light_playback::PlaybackButtonAction::Go,
            light_playback::PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    let engine = Engine::with_programming_contract_support(
        programmers.clone(),
        PROGRAMMING_CONTRACT_VERSION,
    );
    engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![cue_list].into(),
            playbacks: vec![playback].into(),
            ..Default::default()
        })
        .unwrap();
    engine
        .execute_playback(light_engine::EnginePlaybackCommand::Pool {
            number: 1,
            action: light_engine::PoolPlaybackAction::Go,
        })
        .unwrap();
    set_fixed(
        &programmers,
        session,
        target,
        ProgrammingOwner::Position,
        position(180., 15.),
        1_000,
    );
    assert!(programmers.apply_dynamic_values(
        session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: target,
            attribute: AttributeKey::intensity(),
            value: DynamicSemanticValue::FixAt {
                value: 1.,
                timing: Default::default()
            },
        }],
        None
    ));
    clock.advance_millis(500);
    let frame = engine.prepare_output_frame(Default::default());
    let original = engine.prepare_static_family_frame(&frame, &[]);
    assert_eq!(
        original.value(target, &ProgrammingOwner::Position.key()),
        Some(&position(90., 15.)),
        "dark static frame prepositions for the next Cue"
    );
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    let prepared = simple_prepare(&engine, &frame, &mut runtime, frame.sampled_at()).unwrap();
    assert_eq!(
        prepared
            .token
            .value(target, &ProgrammingOwner::Position.key()),
        Some(&position(0., 15.)),
        "scalar Intensity blocks MIB before typed composition"
    );
    assert!(prepared.family_sidecars.is_empty());
    assert!(
        prepared
            .requirements
            .iter()
            .any(|requirement| requirement.target == target
                && requirement.owner == ProgrammingOwner::Position
                && matches!(
                    requirement.reason,
                    HybridFamilyRequirementReason::ScalarBaselineChanged
                ))
    );
    let output = engine
        .render_static_family_frame(&frame, prepared.token)
        .unwrap();
    assert_eq!(
        output
            .resolved_values
            .value(target, &ProgrammingOwner::Position.key()),
        Some(&position(0., 15.))
    );
}
