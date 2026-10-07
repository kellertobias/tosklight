//! TL-592 seam tests: the Color/UV adapter as the actual destination writer inside the existing
//! Live hybrid composer, observer and engine finalizer, including fixture replacement.
use super::super::super::physical_adapter::color::profiles::{patched, rgb, rgbw};
use super::super::super::physical_adapter::color::tests::{intent, program};
use super::super::super::physical_adapter::*;
use super::super::hybrid::*;
use super::*;
use light_engine::PreparedOutputFrame;
use light_fixture::FixtureProfile;
use light_fixture::forward::CompiledColorFitting;

/// One Recipe Red component lane at full: composed over the static magenta base it keeps the
/// complete semantic owner (one Color family) and sends it through the adapter every frame.
fn red_lane() -> DynamicDefinition {
    color_lane("Color red", ColorComponent::Red, 1.)
}

/// One constant semantic Color component lane (Recipe basis).
pub(super) fn color_lane(name: &str, component: ColorComponent, value: f32) -> DynamicDefinition {
    let mut definition = pan_definition();
    definition.name = name.into();
    definition.lanes.truncate(1);
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe,
            },
            component: Some(ProgrammingComponent::Color(component)),
        },
        configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
            points: [0., 0.5]
                .map(|position| DynamicKeyframe {
                    position,
                    source: DynamicValueSource::Value {
                        value: DynamicValue::Scalar(value),
                    },
                    interpolation: light_dynamics::ScalarInterpolation::Linear,
                })
                .to_vec(),
            size: 1.,
        }),
    });
    definition
}

pub(super) struct Live {
    pub(super) engine: Engine,
    pub(super) programmers: ProgrammerRegistry,
    pub(super) session: SessionId,
    pub(super) target: FixtureId,
    clock: Arc<ManualClock>,
    runtime: DynamicRuntime,
    origins: DynamicSourceOrigins,
    transaction: DynamicOutputFrameScratch,
    hybrid: HybridFrameScratch,
    definition: DynamicDefinition,
}

impl Live {
    fn new(profile: &FixtureProfile, base: &ColorIntent) -> Self {
        let live = Self::start(FixtureId::new(), base, red_lane());
        live.install(profile);
        live
    }

    /// A Live rig programming `base` on `target` with one running Dynamic `definition`. The
    /// caller installs the fixtures.
    pub(super) fn start(
        target: FixtureId,
        base: &ColorIntent,
        definition: DynamicDefinition,
    ) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        programmers.set(
            session,
            target,
            ProgrammingOwner::Color.key(),
            program(base),
        );
        let engine = Engine::with_programming_contract_support(
            programmers.clone(),
            PROGRAMMING_CONTRACT_VERSION,
        );
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        runtime.install_definitions([definition.clone()]).unwrap();
        let live = Self {
            engine,
            programmers,
            session,
            target,
            clock,
            runtime,
            origins: Default::default(),
            transaction: Default::default(),
            hybrid: Default::default(),
            definition,
        };
        assert!(live.programmers.apply_dynamic_values(
            session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: target,
                attribute: ProgrammingOwner::Color.key(),
                value: DynamicSemanticValue::DynamicOn {
                    instance_link: Uuid::new_v4(),
                    lane_id: live.definition.lanes[0].id,
                    dynamic: DynamicReference {
                        dynamic_id: Some(live.definition.id),
                        last_known_pool_number: live.definition.pool_number,
                        embedded_fallback: DynamicDefinitionSnapshot {
                            definition: Arc::new(live.definition.clone()),
                        },
                    },
                    overrides: DynamicInstanceOverrides {
                        size: 1.,
                        speed_multiplier: Rational::ONE,
                        phase_offset_degrees: 0.,
                    },
                    timing: Default::default(),
                },
            }],
            None
        ));
        live
    }

    /// Install (or replace) the destination fixture under the same programming target.
    fn install(&self, profile: &FixtureProfile) {
        self.install_fixtures(vec![patched(profile, self.target, 1)]);
    }

    pub(super) fn install_fixtures(&self, fixtures: Vec<light_fixture::PatchedFixture>) {
        self.engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: fixtures.into(),
                dynamics: vec![self.definition.clone()].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
    }

    pub(super) fn capture(&self) -> PreparedOutputFrame {
        self.clock.advance_millis(100);
        self.engine.prepare_output_frame(Default::default())
    }

    pub(super) fn run<A: PhysicalFamilyAdapter>(
        &mut self,
        capture: &PreparedOutputFrame,
        lane: &PhysicalAdapterLane<A>,
    ) -> Result<PublishedPhysicalFrame<A>, DynamicRuntimeError> {
        let engine = &self.engine;
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
        let mut candidate = self.origins.clone();
        let scratch = &mut self.hybrid;
        let output = self
            .runtime
            .with_output_frame_transaction(&mut self.transaction, |runtime| {
                let prepared = prepare_captured_hybrid_frame(
                    engine,
                    capture,
                    &[],
                    runtime,
                    &mut candidate,
                    &inputs,
                    scratch,
                    lane,
                    None,
                    |observation| lane.observe(observation),
                )?;
                finalize_live_physical_frame(engine, capture, lane, prepared)
            });
        if output.is_ok() {
            self.origins = candidate;
        }
        output
    }

    /// Re-simulate a published sidecar: the capture's own pre-master native values with the
    /// sidecar writes applied, through an independently compiled forward model.
    fn resimulate(
        &self,
        capture: &PreparedOutputFrame,
        profile: &FixtureProfile,
        result: &PhysicalHeadResult<ColorAdapter>,
    ) {
        let token = capture.frame_token();
        let baseline = self.engine.prepare_static_family_frame(capture, &[]);
        let mut raw = baseline
            .native_raw(capture, &token, self.target)
            .unwrap()
            .raw()
            .to_vec();
        for write in &result.writes {
            assert_eq!(write.slot.destination, self.target);
            raw[write.slot.channel_index as usize] = write.raw;
        }
        let fitting = CompiledColorFitting::compile(profile, profile.modes[0].id, None)
            .unwrap()
            .unwrap();
        let mut forward = fitting.forward().create_output();
        fitting.forward().evaluate(&raw, &mut forward).unwrap();
        assert_eq!(result.achieved.known_xyz, forward[0].known_xyz);
        assert_eq!(
            result.achieved.visible,
            forward[0].visible_complete.then_some(forward[0].known_xyz)
        );
    }
}

pub(super) fn semantic(value: &AttributeValue) -> &ColorIntent {
    let AttributeValue::ColorProgram(program) = value else {
        panic!("composed Color program")
    };
    let ColorProgram::Semantic { intent } = program.as_ref() else {
        panic!("semantic Color")
    };
    intent
}

#[test]
fn live_color_adapter_fits_inside_the_hybrid_seam_under_one_token() {
    let profile = rgb();
    let mut live = Live::new(&profile, &intent([1., 0., 1.], 0.));
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    let capture = live.capture();
    let published = live.run(&capture, &lane).unwrap();
    assert!(published.requirements.is_empty());
    assert_eq!(published.token, capture.frame_token());
    let [result] = published.results.as_slice() else {
        panic!("one complete Color owner")
    };
    assert_eq!(
        (result.target, result.owner),
        (live.target, ProgrammingOwner::Color)
    );
    assert_eq!(
        result.token, published.token,
        "writes, request and quality share one token"
    );
    assert_eq!(
        &result.requested,
        semantic(&result.value),
        "request is the composed value"
    );
    assert_eq!(
        published
            .rendered
            .resolved_values
            .value(live.target, &ProgrammingOwner::Color.key()),
        Some(&result.value),
        "the engine receives the same composed value the sidecar describes"
    );
    assert_eq!(
        result.writes.iter().map(|w| w.raw).collect::<Vec<_>>(),
        [65535, 0, 255]
    );
    assert_eq!(
        result.quality.color_match,
        light_fixture::forward::ColorMatch::Exact
    );
    live.resimulate(&capture, &profile, result);

    let second = live.capture();
    let published = live.run(&second, &lane).unwrap();
    assert_eq!(published.results[0].token, second.frame_token());
    assert_eq!(
        lane.continuity(live.target, ProgrammingOwner::Color)
            .unwrap()
            .heads[0]
            .controls
            .len(),
        3
    );
    let counters = lane.adapter().counters();
    assert_eq!(
        (counters.descriptor_compiles, counters.fitting_compiles),
        (1, 1),
        "one descriptor and fitter per generation"
    );
    assert_eq!((counters.resolves, counters.refits), (2, 0));
}

#[test]
fn fixture_replacement_recompiles_and_decides_every_new_color_control() {
    let base = intent([1., 0., 1.], 0.);
    let mut live = Live::new(&rgb(), &base);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    let first = live.capture();
    let before = live.run(&first, &lane).unwrap();
    let requested = before.results[0].requested.clone();
    let generation = lane.descriptor_generation();

    // RGB → RGBW under the same target, with White seeded nonzero by an earlier scalar edit.
    let replacement = rgbw();
    live.install(&replacement);
    live.programmers.set(
        live.session,
        live.target,
        light_core::AttributeKey("color.white".into()),
        AttributeValue::Normalized(0.8),
    );
    let second = live.capture();
    let after = live.run(&second, &lane).unwrap();
    assert_ne!(lane.descriptor_generation(), generation);
    let [result] = after.results.as_slice() else {
        panic!("one complete Color owner")
    };
    assert_eq!(result.requested, requested, "requested intent is unchanged");
    assert_eq!(result.token, second.frame_token());
    let baseline = live.engine.prepare_static_family_frame(&second, &[]);
    let seeded = baseline
        .native_raw(&second, &second.frame_token(), live.target)
        .unwrap();
    assert_eq!(
        seeded.raw()[4],
        204,
        "White is seeded nonzero before fitting"
    );
    assert_eq!(
        result.writes.iter().map(|w| w.raw).collect::<Vec<_>>(),
        [65535, 0, 255, 0],
        "every destination Color channel, including the new White, is decided"
    );
    assert_eq!(result.writes.len(), 4);
    live.resimulate(&second, &replacement, result);
    let counters = lane.adapter().counters();
    assert_eq!(
        (counters.descriptor_compiles, counters.fitting_compiles),
        (2, 2)
    );
}
