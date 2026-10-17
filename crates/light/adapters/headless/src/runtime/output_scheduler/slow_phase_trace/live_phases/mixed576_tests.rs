//! Private ignored exact mixed576 replay through LiveOutputBench's unchanged production
//! Dynamic transaction, FamilyLanes observer, family forks, stagings and ordered merge.
//! Input is generated once by the actual Stage scene/20-controller partition constructors.
//! Both worker cases read the same immutable bytes. No synthetic profiles or replacement
//! per-fixture Color model; no manually substituted semantic base values.
//!
//! Only test observation is added: existing phase spans/counters are active on every frame,
//! and full accepted requirements are copied by the existing cfg(test) publication observer.
//! These costs are present in both cases, not zero-overhead production timings. Thread-local
//! spans see the coordinator; worker CPU is not measured. A worker count alone does not reproduce
//! full-desktop Linux renderer contention. No UI, renderer, network, output
//! deadline gate or prior Linux show's random logical-head identity is reproduced here.
use super::*;
use crate::runtime::output_scheduler::LiveOutputBench;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::color::profiles::patched;
use family_preparation::{Count as FamilyCount, Stage};
use light_core::{FixtureId, ManualClock, SessionId, programming::SUPPORTED_PROGRAMMING_CONTRACT};
use light_dynamics::{
    DynamicDefinition, DynamicDefinitionSnapshot, DynamicInstanceOverrides, DynamicReference,
    DynamicRuntime, DynamicSemanticValue, Rational,
};
use light_engine::{Engine, EngineSnapshot, RenderOptions};
use light_fixture::{FixtureProfile, PatchedFixture, apply_runtime_profile_compatibility};
use light_programmer::{DynamicProgrammerValueMutation, ProgrammerRegistry};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy)]
struct ProbeFrames {
    warmup: usize,
    measured: usize,
}
impl ProbeFrames {
    fn from_env() -> Self {
        fn read(name: &str, default: usize, minimum: usize, maximum: usize) -> usize {
            let value = match std::env::var(name) {
                Ok(value) => value.parse().expect("integer frame count"),
                Err(std::env::VarError::NotPresent) => default,
                Err(error) => panic!("invalid {name}: {error}"),
            };
            assert!(
                (minimum..=maximum).contains(&value),
                "{name} outside bounded probe frame range"
            );
            value
        }
        Self {
            warmup: read("LIGHT_MIXED576_WARMUP_FRAMES", 4, 1, 32),
            measured: read("LIGHT_MIXED576_MEASURED_FRAMES", 32, 1, 256),
        }
    }
}
const STEP_MS: i64 = 25;
const SLOTS: usize = 8064;
const LANES: usize = 5408;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn uuid(value: &Value) -> Uuid {
    value.as_str().unwrap().parse().unwrap()
}
fn rows<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().unwrap()
}
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..")
}

struct Input {
    value: Value,
    digest: String,
    profiles: HashMap<Uuid, FixtureProfile>,
}
impl Input {
    fn load() -> Self {
        let path = PathBuf::from(
            std::env::var("LIGHT_MIXED576_INPUT").expect("immutable generator input path"),
        );
        assert!(path.is_absolute() && !path.is_symlink());
        let bytes = std::fs::read(&path).unwrap();
        let digest = sha(&bytes);
        assert_eq!(
            digest,
            std::env::var("LIGHT_MIXED576_INPUT_SHA256").expect("frozen input SHA256")
        );
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value["kind"],
            "private.production-js-mixed576-live-input/v1"
        );
        assert_eq!(rows(&value["patch"], "fixtures").len(), 576);
        assert_eq!(value["scene"]["patch"]["occupiedSlots"], SLOTS);
        assert_eq!(value["scene"]["patch"]["universeCount"], 16);
        assert_eq!(value["plan"]["dynamicTargetCount"], LANES);
        assert_eq!(rows(&value["plan"], "definitions").len(), 20);
        assert_eq!(rows(&value["plan"], "activations").len(), 20);
        let mut profiles = HashMap::new();
        for binding in rows(&value, "package_bindings") {
            let relative = binding["file"].as_str().unwrap();
            assert!(relative.starts_with("assets/fixture-library/") && !relative.contains(".."));
            let bytes = std::fs::read(repository().join(relative)).unwrap();
            assert_eq!(sha(&bytes), binding["sha256"].as_str().unwrap());
            let mut profile = light_fixture::read_fixture_package(&bytes).unwrap();
            assert_eq!(profile.id.0, uuid(&binding["profile_id"]));
            apply_runtime_profile_compatibility(&mut profile);
            profile.validate().unwrap();
            assert!(profiles.insert(profile.id.0, profile).is_none());
        }
        for binding in rows(&value, "source_bindings") {
            let relative = binding["file"].as_str().unwrap();
            assert!(relative.starts_with("tools/") && !relative.contains(".."));
            assert_eq!(
                sha(&std::fs::read(repository().join(relative)).unwrap()),
                binding["sha256"].as_str().unwrap()
            );
        }
        Self {
            value,
            digest,
            profiles,
        }
    }

    fn fixtures(&self) -> Vec<PatchedFixture> {
        let mut counts = BTreeMap::new();
        let mut occupied = BTreeSet::new();
        let mut universes = BTreeSet::new();
        let mut fixtures = Vec::new();
        for input in rows(&self.value["patch"], "fixtures") {
            let profile = &self.profiles[&uuid(&input["profile_id"])];
            let mode_id = uuid(&input["mode_id"]);
            let mode = profile
                .modes
                .iter()
                .find(|mode| mode.id == mode_id)
                .unwrap();
            *counts.entry(profile.name.as_str()).or_insert(0usize) += 1;
            let root = FixtureId(uuid(&input["fixture_id"]));
            let mut fixture = patched(profile, root, 1);
            fixture.definition = profile.resolved_definition(mode_id).unwrap();
            fixture.fixture_number = Some(
                input["fixture_number"]
                    .as_u64()
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
            fixture.name = input["name"].as_str().unwrap().to_owned();
            fixture.universe = None;
            fixture.address = None;
            fixture.split_patches = serde_json::from_value(input["split_patches"].clone()).unwrap();
            fixture.layer_id = input["layer_id"].as_str().unwrap().to_owned();
            fixture.location = serde_json::from_value(input["location"].clone()).unwrap();
            fixture.rotation = serde_json::from_value(input["rotation"].clone()).unwrap();
            fixture.logical_heads = serde_json::from_value(input["logical_heads"].clone()).unwrap();
            fixture.move_in_black_enabled = input["move_in_black_enabled"].as_bool().unwrap();
            fixture.move_in_black_delay_millis =
                input["move_in_black_delay_millis"].as_u64().unwrap();
            assert!(fixture.multipatch.is_empty());
            for split in &fixture.split_patches {
                let universe = split.universe.unwrap();
                let address = split.address.unwrap();
                universes.insert(universe);
                let footprint = mode
                    .splits
                    .iter()
                    .find(|m| m.number == split.split)
                    .unwrap()
                    .footprint;
                for offset in 0..footprint {
                    assert!(occupied.insert((universe, address + offset)));
                }
            }
            fixtures.push(fixture);
        }
        assert_eq!(counts.get("Robin LEDBeam 150"), Some(&224));
        assert_eq!(counts.get("Sunstrip LED RGB 42206"), Some(&96));
        assert_eq!(counts.get("Dimmer"), Some(&256));
        assert_eq!(counts.len(), 3);
        assert_eq!(occupied.len(), SLOTS);
        assert_eq!(universes.len(), 16);
        self.validate_descriptors(&fixtures);
        fixtures
    }

    fn validate_descriptors(&self, fixtures: &[PatchedFixture]) {
        let mut counts = BTreeMap::<String, usize>::new();
        for descriptor in rows(&self.value["plan"], "targetDescriptors") {
            let root = FixtureId(uuid(&descriptor["fixtureId"]));
            let fixture = fixtures.iter().find(|f| f.fixture_id == root).unwrap();
            let profile = &self.profiles[&fixture.definition.profile_id.unwrap().0];
            let mode = profile
                .modes
                .iter()
                .find(|m| Some(m.id) == fixture.definition.mode_id)
                .unwrap();
            let head_id = uuid(&descriptor["headId"]);
            let head = mode.heads.iter().find(|h| h.id == head_id).unwrap();
            let channels = mode
                .channels
                .iter()
                .filter(|c| c.head_id == head_id)
                .collect::<Vec<_>>();
            let mut actual = BTreeSet::new();
            for channel in &channels {
                let mapped = match channel.attribute.0.as_ref() {
                    "intensity" => Some("intensity"),
                    "color.red" | "color.cyan" => Some("color.red"),
                    "color.green" | "color.magenta" => Some("color.green"),
                    "color.blue" | "color.yellow" => Some("color.blue"),
                    "color.amber" => Some("color.amber"),
                    "color.white" => Some("color.white_blend"),
                    "color.uv" => Some("color.uv"),
                    "pan" => Some("position.pan"),
                    "tilt" => Some("position.tilt"),
                    _ => None,
                };
                if let Some(mapped) = mapped {
                    actual.insert(mapped.to_owned());
                }
            }
            if actual.contains("position.pan") || actual.contains("position.tilt") {
                actual.insert("position.pan".into());
                actual.insert("position.tilt".into());
            }
            if channels.iter().any(|c| c.reacts_to_virtual_intensity) {
                actual.insert("intensity".into());
            }
            let expected = rows(descriptor, "attributes")
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                actual, expected,
                "raw JS metadata must equal normalized runtime profile lane capabilities"
            );
            let target = if head.master_shared {
                root
            } else {
                fixture
                    .logical_heads
                    .iter()
                    .find(|h| h.profile_head_id == Some(head_id))
                    .unwrap()
                    .fixture_id
            };
            assert_eq!(target.0, uuid(&descriptor["target"]));
            for attribute in actual {
                *counts.entry(attribute).or_default() += 1;
            }
        }
        assert_eq!(counts.values().sum::<usize>(), LANES);
        for (key, value) in self.value["plan"]["laneCoverage"].as_object().unwrap() {
            assert_eq!(counts[key], value.as_u64().unwrap() as usize);
        }
    }
}

fn bench(input: &Input, workers: usize) -> (LiveOutputBench, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId(Uuid::from_u128(0x775e_0001));
    let mut state = programmers.start(session);
    state.id = light_core::ProgrammerId(Uuid::from_u128(0x775e_0003));
    programmers.restore(state);
    let engine = Arc::new(Engine::with_programming_contract_support(
        programmers.clone(),
        SUPPORTED_PROGRAMMING_CONTRACT,
    ));
    engine.set_output_workers(workers);
    let definitions = rows(&input.value["plan"], "definitions")
        .iter()
        .map(|v| serde_json::from_value::<DynamicDefinition>(v.clone()).unwrap())
        .collect::<Vec<_>>();
    let fixtures = input.fixtures();
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            dynamics: definitions.clone().into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    for (index, activation) in rows(&input.value["plan"], "activations").iter().enumerate() {
        let definition: DynamicDefinition =
            serde_json::from_value(activation["definition"].clone()).unwrap();
        assert_eq!(definition.id, definitions[index].id);
        let reference = DynamicReference {
            dynamic_id: Some(definition.id),
            last_known_pool_number: definition.pool_number,
            embedded_fallback: DynamicDefinitionSnapshot {
                definition: Arc::new(definition.clone()),
            },
        };
        let mut mutations = Vec::new();
        for target in rows(activation, "targets") {
            for lane in &definition.lanes {
                mutations.push(DynamicProgrammerValueMutation::Set {
                    fixture_id: FixtureId(uuid(target)),
                    attribute: lane.output_owner(),
                    value: DynamicSemanticValue::DynamicOn {
                        instance_link: definition.id,
                        dynamic: reference.clone(),
                        lane_id: lane.id,
                        overrides: DynamicInstanceOverrides {
                            size: 1.,
                            speed_multiplier: Rational::ONE,
                            phase_offset_degrees: 0.,
                        },
                        timing: Default::default(),
                    },
                });
            }
        }
        assert!(programmers.apply_dynamic_values(session, &mutations, None));
    }
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(SUPPORTED_PROGRAMMING_CONTRACT);
    runtime.derive_instance_ids_from(Uuid::from_u128(0x775e_0002));
    runtime.install_definitions(definitions).unwrap();
    let bench = LiveOutputBench::new(engine, runtime, 40, false).unwrap();
    assert!(bench.family_engaged());
    (bench, clock)
}

/// Same test-only Frame installation already used by the adjacent diagnostic unit tests.
fn traced(
    work: impl FnOnce() -> light_engine::RenderResult,
) -> (light_engine::RenderResult, Frame) {
    assert!(ACTIVE.with(|active| active.borrow().is_none()));
    ACTIVE.with(|active| {
        *active.borrow_mut() = Some(Frame {
            phases: [Duration::ZERO; COUNT],
            family: family_preparation::Summary::default(),
            generation: None,
            show_revision: None,
            sampled_at_millis: None,
            accepted: false,
        })
    });
    let rendered = work();
    let observation = ACTIVE.with(|active| active.borrow_mut().take()).unwrap();
    (rendered, observation)
}

#[derive(Debug, Eq, PartialEq)]
struct Fingerprint {
    dmx: String,
    semantics: String,
    physical: Vec<String>,
    color: Vec<(FixtureId, String)>,
    requirements: Vec<String>,
}
fn fingerprint(bench: &LiveOutputBench, rendered: &light_engine::RenderResult) -> Fingerprint {
    let universes = rendered
        .universes
        .iter()
        .map(|(u, b)| (*u, b.to_vec()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(universes.len(), 16);
    let dmx = sha(&serde_json::to_vec(&universes).unwrap());
    let mut semantics = rendered
        .resolved_values
        .values()
        .iter()
        .map(|((target, attribute), value)| {
            (target.0, attribute.0.to_string(), format!("{value:?}"))
        })
        .collect::<Vec<_>>();
    semantics.sort();
    let mut physical = rendered
        .physical
        .instances
        .iter()
        .map(|v| {
            format!(
                "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
                v.fixture_id,
                v.instance_id,
                v.native_raw,
                v.complete,
                v.color_support,
                v.position_support,
                v.optics_support,
                v.color_diagnostic,
                v.position_diagnostic,
                v.optics_diagnostic
            )
        })
        .collect::<Vec<_>>();
    physical.sort();
    Fingerprint {
        dmx,
        semantics: sha(&serde_json::to_vec(&semantics).unwrap()),
        physical,
        color: bench.readout_digest(rendered),
        requirements: bench.test_take_requirement_rows(),
    }
}

struct Case {
    fingerprints: Vec<Fingerprint>,
    report: Value,
}
fn case(input: &Input, workers: usize, frames: ProbeFrames) -> Case {
    let setup = Instant::now();
    let (bench, clock) = bench(input, workers);
    let setup_ns = setup.elapsed().as_nanos();
    let mut phases = [0u64; 14];
    let mut counts = [0u64; 22];
    let mut capture_ns = 0u128;
    let mut transaction_ns = 0u128;
    let mut fingerprints = Vec::new();
    let mut last_work = None;
    let whole = Instant::now();
    for tick in 0..frames.warmup + frames.measured {
        clock.advance_millis(STEP_MS);
        let mut sample = None;
        let (rendered, frame) = traced(|| {
            let output = bench.render(RenderOptions::default(), &[]).unwrap();
            assert!(output.hybrid);
            assert_eq!(output.dynamic_samples, LANES);
            capture_ns += if tick >= frames.warmup {
                output.capture.as_nanos()
            } else {
                0
            };
            transaction_ns += if tick >= frames.warmup {
                output.transaction.as_nanos()
            } else {
                0
            };
            last_work = Some(output.work);
            sample = Some(output.start_path);
            output.rendered
        });
        let _ = sample;
        let get = |kind: FamilyCount| frame.family.counts[kind as usize];
        assert_eq!(
            get(FamilyCount::RuntimeInstances),
            if tick == 0 { 0 } else { 20 },
            "the production trace counts before the first binding callback"
        );
        assert_eq!(
            bench.test_runtime_instance_count(),
            20,
            "all twenty instances must exist after every committed frame, including cold"
        );
        assert_eq!(get(FamilyCount::TypedLanes), 4224);
        assert_eq!(get(FamilyCount::ScalarSamples), 1184);
        assert_eq!(get(FamilyCount::FamilySamples), LANES as u64);
        if tick >= frames.warmup {
            if workers == 1 {
                assert!(get(FamilyCount::PreparationSerial) > 0);
                assert_eq!(get(FamilyCount::PreparationParallel), 0);
                assert_eq!(get(FamilyCount::ForkInvocations), 0);
            } else {
                assert!(get(FamilyCount::PreparationParallel) > 0);
                assert_eq!(get(FamilyCount::PreparationSerial), 0);
                assert!(get(FamilyCount::ForkInvocations) > 0);
                assert!(get(FamilyCount::ForkChunks) >= 4);
                assert!(get(FamilyCount::OwnerParallel) > 0);
            }
            for (i, v) in frame.family.counts.iter().enumerate() {
                counts[i] += v;
            }
            for (i, stage) in [
                Stage::InitialStatic,
                Stage::MissingFixedBases,
                Stage::RepeatStatic,
                Stage::BindAndControl,
                Stage::SamplingBeforeCallback,
                Stage::ScalarStatic,
                Stage::FinalGeometry,
                Stage::CompleteDeferred,
                Stage::PrepareFamilies,
                Stage::CohortSetup,
                Stage::PositionCompose,
                Stage::OwnerCompose,
                Stage::OrderedFinish,
                Stage::NativeProjection,
            ]
            .into_iter()
            .enumerate()
            {
                phases[i] += frame.family.micros(stage);
            }
        }
        fingerprints.push(fingerprint(&bench, &rendered));
    }
    Case {
        report: json!({ "workers": workers, "warmup":frames.warmup,"measured":frames.measured,"input_step_ms":STEP_MS,
        "setup_ns":setup_ns,"whole_loop_including_warmup_and_fingerprints_ns":whole.elapsed().as_nanos(),
        "capture_total_ns":capture_ns,"transaction_total_ns":transaction_ns,"family_phase_total_us":phases,
        "family_phase_order":["initial_static","missing_fixed_bases","repeat_static","bind_and_control","sampling_before_callback","scalar_static","final_geometry","complete_deferred","prepare_families","cohort_setup","position_compose","owner_compose","ordered_finish","native_projection"],
        "family_count_totals":counts,"cumulative_live_work":last_work.unwrap(),
        "preparation_parallel":counts[FamilyCount::PreparationParallel as usize],"preparation_serial":counts[FamilyCount::PreparationSerial as usize],
        "fork_invocations":counts[FamilyCount::ForkInvocations as usize],"fork_chunks":counts[FamilyCount::ForkChunks as usize],
        "owner_parallel":counts[FamilyCount::OwnerParallel as usize],
        "final_dmx_sha256":fingerprints.last().unwrap().dmx,
        "final_requirement_rows":fingerprints.last().unwrap().requirements.len() }),
        fingerprints,
    }
}

#[test]
#[ignore = "private full mixed576 measurement; requires explicit root-authorized quiet window"]
fn manual_exact_mixed576_serial_and_four_workers() {
    let input = Input::load();
    let frames = ProbeFrames::from_env();
    let serial = case(&input, 1, frames);
    let parallel = case(&input, 4, frames);
    assert_eq!(
        serial.fingerprints, parallel.fingerprints,
        "all captured outputs must agree by logical frame, including full DMX, semantic values, physical diagnostics, Color readout and full accepted requirements"
    );
    println!(
        "{}",
        json!({"benchmark":"private.exact-mixed576-live/v1","input_sha256":input.digest,
        "fixtures":576,"occupied_slots":SLOTS,"universes":16,"animated_addresses":LANES,"dynamic_instances":20,
        "semantic_equivalence":"all logical frames paired: full DMX/semantic/physical/Color/fullrequirements exact",
        "paired_frame_count": frames.warmup + frames.measured,
        "per_frame_fingerprints": serial.fingerprints.iter().map(|frame| json!({
            "dmx": frame.dmx, "semantics": frame.semantics,
            "physical": sha(&serde_json::to_vec(&frame.physical).unwrap()),
            "color": sha(&serde_json::to_vec(&frame.color).unwrap()),
            "requirements": sha(&serde_json::to_vec(&frame.requirements).unwrap())
        })).collect::<Vec<_>>(),
        "cases":[serial.report,parallel.report],
        "host_available_parallelism":std::thread::available_parallelism().map(|n|n.get()).ok(),
        "limits":"ManualClock25ms; no sleeps/pacing; trace clocks+coordinator counters active on every test frame; no workerCPU; sameprocess serial/fourworkers; configuredworkers nothostCPUquota; no network/GPU/outputdeadline gate; harnesslogicalheadUUIDs not historicalLinux IDs"})
    );
}
