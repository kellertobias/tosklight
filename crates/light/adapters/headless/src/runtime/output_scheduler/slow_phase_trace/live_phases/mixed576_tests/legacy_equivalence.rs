//! Pair A measures the enabled hybrid pipeline with equivalent legacy Intensity input.
//! It does not measure active typed Color/Position lanes or replace the mixed576 acceptance.
use super::*;
mod observation;
use light_core::AttributeKey;
use light_dynamics::{
    ActivationBoundary, ActivationPolicy, DynamicLane, DynamicLaneBody, DynamicLaneMode,
    DynamicRunMode, DynamicSpeed, DynamicTargetBinding, KeyframeConfiguration,
    LegacyScalarLaneBody, MaxMinConfiguration, MiddleAmplitudeConfiguration, PeriodicFunction,
    PhaseDistribution, PwmShape, ScalarSource,
};
use observation::{observe, write_private};

struct Authored {
    fixtures: Vec<PatchedFixture>,
    definitions: Vec<DynamicDefinition>,
    target_count: usize,
}
impl Authored {
    fn new(fixtures: Vec<PatchedFixture>, targets: Vec<FixtureId>) -> Self {
        assert!(!targets.is_empty(), "legacy_pair_empty_targets");
        assert_eq!(
            targets.iter().map(|id| id.0).collect::<BTreeSet<_>>().len(),
            targets.len()
        );
        let count = targets.len().min(20);
        let definitions = (0..count)
            .map(|index| {
                let targets = targets.iter().skip(index).step_by(count).copied().collect();
                definition(index, targets)
            })
            .collect();
        Self {
            fixtures,
            definitions,
            target_count: targets.len(),
        }
    }
    fn from_input(input: &Input) -> Self {
        let targets = rows(&input.value["plan"], "targetDescriptors")
            .iter()
            .filter(|row| rows(row, "attributes").iter().any(|a| a == "intensity"))
            .map(|row| FixtureId(uuid(&row["target"])))
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), 1184);
        Self::new(input.fixtures(), targets)
    }
}
fn definition(index: usize, targets: Vec<FixtureId>) -> DynamicDefinition {
    DynamicDefinition {
        id: Uuid::from_u128(0x775a_1000 + index as u128),
        pool_number: index as u16 + 1,
        revision: 1,
        name: "Legacy equivalence Intensity".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::FrozenTargets { targets },
        lanes: vec![DynamicLane {
            id: Uuid::from_u128(0x775a_2000 + index as u128),
            body: DynamicLaneBody::LegacyScalar(LegacyScalarLaneBody {
                attribute: AttributeKey::intensity(),
                mode: DynamicLaneMode::MaxMin,
                keyframes: KeyframeConfiguration {
                    points: [0.0, 0.5]
                        .into_iter()
                        .map(|position| light_dynamics::DynamicKeyframe {
                            position,
                            source: ScalarSource::Value { value: 0.5 },
                            interpolation: light_dynamics::ScalarInterpolation::Linear,
                        })
                        .collect(),
                    size: 1.0,
                },
                max_min: MaxMinConfiguration {
                    minimum: ScalarSource::Value { value: 0.2 },
                    maximum: ScalarSource::Value { value: 0.8 },
                    function: PeriodicFunction::Sinus,
                    size: 1.0,
                    pwm: PwmShape::default(),
                },
                middle_amplitude: MiddleAmplitudeConfiguration {
                    middle: ScalarSource::Value { value: 0.5 },
                    amplitude: 0.3,
                    function: PeriodicFunction::Sinus,
                    size: 1.0,
                    pwm: PwmShape::default(),
                    invert_waveform: false,
                },
            }),
            speed_multiplier: Rational::ONE,
            width: 1.0,
            phase: None,
            random_group_id: None,
        }],
        random_groups: vec![],
        phase_spread_mode: Default::default(),
        spatial_mapping: Default::default(),
        phase: PhaseDistribution {
            ordering: light_dynamics::PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 0.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: vec![],
        },
        speed: DynamicSpeed::Fixed {
            duration_millis: 1000,
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::StartNow,
        activation_boundary: ActivationBoundary::Beat,
    }
}
fn make_bench(authored: &Authored, contract: u16) -> (LiveOutputBench, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId(Uuid::from_u128(0x775a_3000));
    let mut state = programmers.start(session);
    state.id = light_core::ProgrammerId(Uuid::from_u128(0x775a_3001));
    programmers.restore(state);
    let engine = Arc::new(Engine::with_programming_contract_support(
        programmers.clone(),
        contract,
    ));
    engine.set_output_workers(2);
    assert_eq!(
        engine.output_pool().as_ref().map(|pool| pool.workers()),
        Some(2)
    );
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: authored.fixtures.clone().into(),
            dynamics: authored.definitions.clone().into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap_or_else(|_| panic!("legacy_pair_snapshot_failed"));
    for definition in &authored.definitions {
        let DynamicTargetBinding::FrozenTargets { targets } = &definition.target_binding else {
            panic!("legacy_pair_nonfrozen_targets")
        };
        let reference = DynamicReference {
            dynamic_id: Some(definition.id),
            last_known_pool_number: definition.pool_number,
            embedded_fallback: DynamicDefinitionSnapshot {
                definition: Arc::new(definition.clone()),
            },
        };
        let mutations = targets
            .iter()
            .map(|target| DynamicProgrammerValueMutation::Set {
                fixture_id: *target,
                attribute: AttributeKey::intensity(),
                value: DynamicSemanticValue::DynamicOn {
                    instance_link: definition.id,
                    dynamic: reference.clone(),
                    lane_id: definition.lanes[0].id,
                    overrides: DynamicInstanceOverrides {
                        size: 1.,
                        speed_multiplier: Rational::ONE,
                        phase_offset_degrees: 0.,
                    },
                    timing: Default::default(),
                },
            })
            .collect::<Vec<_>>();
        assert!(programmers.apply_dynamic_values(session, &mutations, None));
    }
    let mut runtime = DynamicRuntime::with_programming_contract_support(contract);
    runtime.derive_instance_ids_from(Uuid::from_u128(0x775a_4000));
    runtime
        .install_definitions(authored.definitions.clone())
        .unwrap_or_else(|_| panic!("legacy_pair_definitions_failed"));
    let bench = LiveOutputBench::new(engine, runtime, 40, false)
        .unwrap_or_else(|_| panic!("legacy_pair_bench_failed"));
    assert_eq!(bench.family_engaged(), contract == 1);
    (bench, clock)
}
fn render(
    bench: &LiveOutputBench,
    clock: &ManualClock,
    contract: u16,
    targets: usize,
) -> super::super::super::super::LiveOutputFrame {
    clock.advance_millis(STEP_MS);
    let frame = bench
        .render(RenderOptions::default(), &[])
        .unwrap_or_else(|_| panic!("legacy_pair_render_failed"));
    assert_eq!(frame.hybrid, contract == 1);
    assert_eq!(frame.dynamic_samples, targets);
    frame
}
fn oracle(authored: &Authored, frames: usize, witness: Option<&Path>) -> Vec<Value> {
    let arms = [make_bench(authored, 0), make_bench(authored, 1)];
    let mut evidence = Vec::new();
    for tick in 0..frames {
        let observed = arms
            .iter()
            .enumerate()
            .map(|(contract, (bench, clock))| {
                let frame = render(bench, clock, contract as u16, authored.target_count);
                observe(bench, &frame.rendered, frame.hybrid)
            })
            .collect::<Vec<_>>();
        evidence.push(observation::evidence(tick, &observed, witness));
    }
    evidence
}
fn timed(authored: &Authored, contract: u16, frames: ProbeFrames) -> Value {
    let (bench, clock) = make_bench(authored, contract);
    for _ in 0..frames.warmup {
        drop(render(&bench, &clock, contract, authored.target_count));
    }
    let mut capture = Vec::new();
    let mut transaction = Vec::new();
    let mut publication = Vec::new();
    let mut render_drop = Vec::new();
    for _ in 0..frames.measured {
        clock.advance_millis(STEP_MS);
        let started = Instant::now();
        let frame = bench
            .render(RenderOptions::default(), &[])
            .unwrap_or_else(|_| panic!("legacy_pair_render_failed"));
        assert_eq!(frame.hybrid, contract == 1);
        assert_eq!(frame.dynamic_samples, authored.target_count);
        capture.push(frame.capture.as_nanos() as u64);
        transaction.push(frame.transaction.as_nanos() as u64);
        publication.push(frame.publication.as_nanos() as u64);
        drop(frame);
        render_drop.push(started.elapsed().as_nanos() as u64);
    }
    json!({"contract":contract,"hybrid":u8::from(contract==1),"workers":2,"measured_frames":frames.measured,"capture_nanos":distribution(capture),"transaction_nanos":distribution(transaction),"publication_nanos":distribution(publication),"render_and_drop_wall_nanos":distribution(render_drop)})
}
fn distribution(mut values: Vec<u64>) -> Value {
    values.sort_unstable();
    let n = values.len();
    assert!(n > 0);
    json!({"count":n,"min":values[0],"mean":values.iter().map(|v|*v as f64).sum::<f64>()/n as f64,"p50":values[n.div_ceil(2)-1],"p95":values[(n*95).div_ceil(100)-1],"max":values[n-1]})
}
#[test]
fn legacy_intensity_uses_real_hybrid_dispatch_and_identical_native_forward_output() {
    let mut fixtures = Vec::new();
    let mut targets = Vec::new();
    for (index, file) in [
        "generic--dimmer.toskfixture",
        "robe--robin-ledbeam-150.toskfixture",
    ]
    .iter()
    .enumerate()
    {
        let bytes = std::fs::read(repository().join("assets/fixture-library").join(file)).unwrap();
        let mut profile = light_fixture::read_fixture_package(&bytes).unwrap();
        apply_runtime_profile_compatibility(&mut profile);
        let id = FixtureId(Uuid::from_u128(0x775a_5000 + index as u128));
        let mut fixture = patched(&profile, id, 1 + index as u16 * 32);
        fixture.fixture_number = Some(index as u32 + 1);
        targets.push(id);
        fixtures.push(fixture);
    }
    let authored = Authored::new(fixtures, targets);
    let frames = oracle(&authored, 4, None);
    assert_eq!(frames.len(), 4);
    assert_ne!(
        frames[0]["dmx_sha256"], frames[3]["dmx_sha256"],
        "wave_must_animate_actual_output"
    );
}
#[test]
#[ignore = "private equivalent legacy576 pair; requires explicit root-authorized quiet window"]
fn manual_legacy576_contract_zero_and_one_pipeline_tax() {
    let destination = PathBuf::from(
        std::env::var("LIGHT_LEGACY_EQUIVALENCE_REPORT").expect("private evidence path required"),
    );
    let canonical = repository().join(".artifacts").canonicalize().unwrap();
    assert!(
        destination.is_absolute()
            && destination
                .parent()
                .unwrap()
                .canonicalize()
                .unwrap()
                .starts_with(canonical)
    );
    assert!(!destination.exists(), "private_evidence_already_exists");
    assert!(
        !destination.with_extension("mismatch.json").exists(),
        "private_mismatch_already_exists"
    );
    let input = Input::load();
    let authored = Authored::from_input(&input);
    let frames = ProbeFrames::from_env();
    let evidence = oracle(
        &authored,
        frames.warmup + frames.measured,
        Some(&destination.with_extension("mismatch.json")),
    );
    let mut rounds = Vec::new();
    for (round, order) in [[0, 1], [1, 0], [0, 1]].into_iter().enumerate() {
        for (position, contract) in order.into_iter().enumerate() {
            rounds.push(
                json!({"round":round,"position":position,"case":timed(&authored,contract,frames)}),
            );
        }
    }
    let private = json!({"input_sha256":input.digest,"authored_definitions":authored.definitions,"fixtures":authored.fixtures,"oracle":evidence,"timing":rounds});
    write_private(&destination, &private);
    println!(
        "{}",
        json!({"schema_version":1,"workload_code":1,"fixtures":576,"occupied_slots":SLOTS,"legacy_intensity_addresses":authored.target_count,"controllers":authored.definitions.len(),"oracle_frames":frames.warmup+frames.measured,"oracle_passed":1,"workers":2,"step_millis":STEP_MS,"rounds":3,"captures_forward_during_timing":0,"fingerprints_during_timing":0,"active_typed_lanes":0,"full_576_acceptance_claimed":0,"cfg_test_observers_present":1,"test_harness_timing_only":1,"production_overhead_claimed":0,"publication_enabled":0,"network_delivery_included":0,"ordered_playback_included":0,"cases":rounds})
    );
}
