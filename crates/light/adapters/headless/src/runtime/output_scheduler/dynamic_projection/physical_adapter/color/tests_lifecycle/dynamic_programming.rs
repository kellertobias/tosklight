//! TL-628: a saved semantic Color Dynamic across fixture replacement and new live-Group members.
//!
//! A typed whole Semantic Color Dynamic (`SemanticColor { basis: Whole }`, two whole keyframe
//! endpoints, `DynamicTargetBinding::LiveGroup`) is switched on in a real Programmer through
//! `ProgrammerRegistry::apply_dynamic_values` (`DynamicOn`) and recorded as a Cue through
//! `ProgrammingService::handle_cue_recording` → `ActiveShowService::commit_programming_cue`
//! (the shared `tests_lifecycle::Show` harness). Every generation below reopens the SQLite show
//! (`ShowStore::portable_document`), compiles it through `prepare_show_candidate`, prepares the
//! Engine and the Dynamic registry from that same compiled snapshot (the show-open pairing of
//! `show_compile_migrations::prepare_startup_runtime`), GOes the Cue through
//! `Engine::execute_playback` and samples the actual captured Cue Dynamic rows through
//! `prepare_captured_hybrid_frame` + `finalize_live_physical_frame` with the Color adapter lane
//! (the TL-603 `color_direct_transition::Rig::frame` pattern, here for several targets).
//!
//! Clocks are deterministic: every generation starts at the same instant, GOes at that instant
//! and captures the same frame schedule, so frame `k` of every generation is the same original
//! Dynamic phase. The sampled semantic request at each phase must be identical on RGB, RGBW
//! (White default 204), CMY+wheel and a UV-capable RGBWA+UV, while each destination decides its
//! own complete native Color footprint, re-simulated from encoded DMX through an independently
//! compiled forward model. A member added to the live Group after recording (in place, same
//! running instance) receives the current semantic sample with no Cue/definition rewrite and
//! never keeps its seeded White default.
//!
//! Not covered (root-owned): production injection of fitted writes into rendered DMX, the
//! production cold-install/cadence path for in-place patch replacement, and TL-613 witnesses.
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use crate::runtime::output_scheduler::dynamic_projection::CapturedDynamicInputs;
use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::{
    HybridFamilyRequirementReason, HybridFrameScratch, prepare_captured_hybrid_frame,
};
use light_core::programming::{PROGRAMMING_CONTRACT_VERSION, UvIntent};
use light_dynamics::DynamicDefinition;
use light_dynamics::{
    ActivationBoundary, ActivationPolicy, DynamicDefinitionSnapshot, DynamicFamilyRepresentation,
    DynamicInstanceOverrides, DynamicKeyframe, DynamicLane, DynamicLaneBody,
    DynamicOutputFrameScratch, DynamicPhaseSpreadMode, DynamicReference, DynamicRunMode,
    DynamicSemanticColorBasis, DynamicSemanticValue, DynamicSpeed, DynamicSpeedTransport,
    DynamicTargetBinding, DynamicValue, DynamicValueAddress, DynamicValueSource,
    KeyframeConfiguration, PhaseDistribution, PhaseOrdering, ProgrammingLaneBody,
    ProgrammingLaneConfiguration, Rational,
};
use light_engine::PreparedOutputFrame;
use light_fixture::forward::{CompiledColorFitting, UvFitStatus};
use light_playback::ActiveCueDynamicValue;

const DYNAMIC: u128 = 0x628_d1;
const LANE: u128 = 0x628_1a;
const LINK: u128 = 0x628_11;
/// GO at the generation's first instant, then one Dynamic period (1 s) in quarter steps.
const SCHEDULE: [i64; 5] = [0, 250, 250, 250, 250];

/// Endpoint A: the TL-557 magenta (White Blend 0.25, 5600 K, Duv −0.004) with a UV payload.
fn endpoint_a() -> ColorIntent {
    ColorIntent {
        uv: UvIntent { amount: 0.4 },
        ..super::super::tests::magenta()
    }
}

/// Endpoint B: the TL-557 warm white (White Blend 0.85, 3200 K, Duv 0.0035) without UV.
fn endpoint_b() -> ColorIntent {
    super::super::tests::warm_white()
}

/// The Cue's static base: blue, no White Blend, no UV. It must never leak into a sample.
fn base() -> ColorIntent {
    super::super::tests::intent([0., 0., 1.], 0.)
}

/// One whole Semantic Color lane, keyframed A (0) → B (0.5) → A, looping over 1 s. Members of
/// the live Group share one phase (span 0) so every member shows the same current sample.
fn color_dynamic() -> DynamicDefinition {
    let point = |position, intent: &ColorIntent| DynamicKeyframe {
        position,
        source: DynamicValueSource::Value {
            value: DynamicValue::Family(program(intent)),
        },
        interpolation: light_dynamics::ScalarInterpolation::Linear,
    };
    DynamicDefinition {
        id: Uuid::from_u128(DYNAMIC),
        pool_number: 28,
        revision: 1,
        name: "TL-628 whole Color".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::LiveGroup {
            group_id: GROUP.into(),
        },
        lanes: vec![DynamicLane {
            id: Uuid::from_u128(LANE),
            body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                address: DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::SemanticColor {
                        basis: DynamicSemanticColorBasis::Whole,
                    },
                    component: None,
                },
                configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                    points: vec![point(0., &endpoint_a()), point(0.5, &endpoint_b())],
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
            span_degrees: 0.,
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
    }
}

fn dynamic_on(definition: &DynamicDefinition) -> DynamicSemanticValue {
    DynamicSemanticValue::DynamicOn {
        instance_link: Uuid::from_u128(LINK),
        lane_id: definition.lanes[0].id,
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
    }
}

fn semantic(value: &AttributeValue) -> &ColorIntent {
    match value {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Semantic { intent } => intent,
            other => panic!("semantic Color sample expected, got {other:?}"),
        },
        other => panic!("Color program expected, got {other:?}"),
    }
}

fn requested(result: &PhysicalHeadResult<ColorAdapter>) -> &ColorIntent {
    result.requested.semantic().expect("semantic request")
}

fn reason(reason: &HybridFamilyRequirementReason) -> String {
    match reason {
        HybridFamilyRequirementReason::Input(_) => "input".into(),
        HybridFamilyRequirementReason::Current {
            address,
            requirement,
        } => format!("current {address:?} {requirement:?}"),
        HybridFamilyRequirementReason::Composition(requirement) => {
            format!("composition {requirement:?}")
        }
        HybridFamilyRequirementReason::LegacyOwnerOverlap => "legacy owner overlap".into(),
        HybridFamilyRequirementReason::ScalarBaselineChanged => "scalar baseline".into(),
    }
}

/// One reopened generation: Engine and Dynamic registry prepared from one compiled snapshot.
struct Live {
    engine: Engine,
    clock: Arc<ManualClock>,
    runtime: DynamicRuntime,
    origins: DynamicSourceOrigins,
    transaction: DynamicOutputFrameScratch,
    hybrid: HybridFrameScratch,
    lane: PhysicalAdapterLane<ColorAdapter>,
    cue_rows: Vec<ActiveCueDynamicValue>,
}

/// One finalized Live frame: its capture and one complete Color result per target.
struct Frame {
    capture: PreparedOutputFrame,
    results: Vec<PhysicalHeadResult<ColorAdapter>>,
}

impl Frame {
    fn of(&self, target: FixtureId) -> &PhysicalHeadResult<ColorAdapter> {
        self.results
            .iter()
            .find(|result| result.target == target)
            .expect("the target has a composed Color owner")
    }
}

impl Live {
    fn open(snapshot: EngineSnapshot) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let engine = Engine::with_programming_contract_support(
            ProgrammerRegistry::with_clock(clock.clone()),
            PROGRAMMING_CONTRACT_VERSION,
        );
        let prepared = engine.prepare_snapshot(snapshot).unwrap();
        let mut runtime = DynamicRuntime::with_native_color_models(
            engine.supported_programming_contract(),
            prepared.snapshot().native_color_sources.clone(),
        );
        runtime
            .install_definitions(prepared.snapshot().dynamics.iter().cloned())
            .unwrap();
        engine.install_prepared_snapshot(prepared);
        Self {
            engine,
            clock,
            runtime,
            origins: Default::default(),
            transaction: Default::default(),
            hybrid: Default::default(),
            lane: PhysicalAdapterLane::live(ColorAdapter::default()),
            cue_rows: Vec::new(),
        }
    }

    fn go(&self, cue: f64) {
        self.engine
            .execute_playback(EnginePlaybackCommand::Pool {
                number: PLAYBACK,
                action: PoolPlaybackAction::GoTo(CueNumber::try_from_legacy_f64(cue).unwrap()),
            })
            .unwrap();
    }

    /// One Live frame through the shared captured sampler, composer and engine finalizer.
    fn frame(&mut self, advance: i64) -> Frame {
        self.clock.advance_millis(advance);
        let capture = self.engine.prepare_output_frame(RenderOptions::default());
        let engine = &self.engine;
        let lane = &self.lane;
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
        self.cue_rows = capture.cue_dynamic_values().to_vec();
        let mut candidate = self.origins.clone();
        let scratch = &mut self.hybrid;
        let published = self
            .runtime
            .with_output_frame_transaction(&mut self.transaction, |runtime| {
                let prepared = prepare_captured_hybrid_frame(
                    engine,
                    &capture,
                    &[],
                    runtime,
                    &mut candidate,
                    &inputs,
                    scratch,
                    lane,
                    None,
                    |observation| lane.observe(observation),
                )?;
                finalize_live_physical_frame(engine, &capture, lane, prepared)
            })
            .unwrap();
        self.origins = candidate;
        let unexpected: Vec<_> = published
            .requirements
            .iter()
            .filter(|r| !matches!(r.reason, HybridFamilyRequirementReason::Composition(_)))
            .collect();
        assert!(unexpected.is_empty(), "no passive requirement");
        assert!(
            published.requirements.is_empty(),
            "no composition requirement: {:?}",
            published
                .requirements
                .iter()
                .map(|r| format!("{:?} {}", r.target, reason(&r.reason)))
                .collect::<Vec<_>>()
        );
        let color = ProgrammingOwner::Color.key();
        for (index, result) in published.results.iter().enumerate() {
            assert_eq!(result.owner, ProgrammingOwner::Color);
            assert_eq!(result.token, capture.frame_token(), "one captured token");
            assert!(
                published.results[..index]
                    .iter()
                    .all(|other| other.target != result.target),
                "one complete Color owner per target"
            );
            assert_eq!(
                published
                    .rendered
                    .resolved_values
                    .value(result.target, &color),
                Some(&result.value),
                "the engine received the same composed value"
            );
        }
        Frame {
            capture,
            results: published.results,
        }
    }

    /// The pre-master native raw vector of `target` captured for this frame.
    fn native(&self, capture: &PreparedOutputFrame, target: FixtureId) -> Vec<u32> {
        let baseline = self.engine.prepare_static_family_frame(capture, &[]);
        baseline
            .native_raw(capture, &capture.frame_token(), target)
            .unwrap()
            .raw()
            .to_vec()
    }
}

/// Decode one fixture's DMX bytes with the builder's sequential layout (fine bytes follow).
fn decode(mode: &light_fixture::FixtureMode, bytes: &[u8; 512]) -> Vec<u32> {
    let mut slot = 1usize;
    mode.channels
        .iter()
        .map(|channel| {
            let mut raw = u32::from(bytes[slot - 1]);
            for secondary in &channel.secondary_slots {
                raw = (raw << 8) | u32::from(bytes[usize::from(*secondary) - 1]);
            }
            slot += channel.resolution.bytes();
            raw
        })
        .collect()
}

/// The complete native Color footprint of `profile` is decided exactly once, and the written
/// output, encoded into DMX and decoded, re-simulates through an independently compiled
/// forward model to the published achieved output. Returns the full native output vector.
fn assert_physical(
    profile: &FixtureProfile,
    native: &[u32],
    result: &PhysicalHeadResult<ColorAdapter>,
) -> Vec<u32> {
    let name = &profile.name;
    assert_eq!(
        result.requested.semantic(),
        Some(semantic(&result.value)),
        "{name}: request is the composed sample"
    );
    let mut written: Vec<_> = result.writes.iter().map(|w| w.slot.channel_index).collect();
    written.sort_unstable();
    let footprint = color_channels(profile);
    assert_eq!(written, footprint, "{name}: every Color control, once");
    assert!(
        result
            .writes
            .iter()
            .all(|w| w.slot.destination == result.target)
    );
    let mut output = native.to_vec();
    for write in &result.writes {
        output[write.slot.channel_index as usize] = write.raw;
    }
    let mode = &profile.modes[0];
    let plan = mode.compile_encoding_plan().unwrap();
    let mut bytes = [0u8; 512];
    let values: Vec<_> = (0u32..).zip(output.iter().copied()).collect();
    plan.encode_split_by_index(&mut bytes, 1, 1, &values)
        .unwrap();
    let decoded = decode(mode, &bytes);
    assert_eq!(
        decoded, output,
        "{name}: encoded bytes decode to the writes"
    );
    let fitting = CompiledColorFitting::compile(profile, mode.id, None)
        .unwrap()
        .unwrap();
    let mut forward = fitting.forward().create_output();
    fitting.forward().evaluate(&decoded, &mut forward).unwrap();
    let forward = &forward[0];
    assert_eq!(result.achieved.known_xyz, forward.known_xyz, "{name}");
    assert_eq!(
        result.achieved.visible,
        forward.visible_complete.then_some(forward.known_xyz),
        "{name}"
    );
    let applied = result.quality.uv == UvFitStatus::Applied;
    assert_eq!(
        result.achieved.uv_drive,
        forward.portable_uv.map(|uv| uv.amount).filter(|_| applied),
        "{name}"
    );
    output
}

/// RGBW + White channel index in the builder layout (Intensity, Red U16, Green, Blue, White).
const WHITE: usize = 4;

/// Patch, record and reopen: the recorded Cue stores only the declarative `DynamicOn` row.
fn recorded_show(a: FixtureId) -> (Show, DynamicDefinition, String, Value) {
    let show = Show::new();
    show.patch(&fixture(&rgb(), a, 1, 1));
    show.group(&[a]);
    let definition = color_dynamic();
    light_dynamics::validate_definition(&definition).unwrap();
    // The Cue's static Color base, stored against the live Group: the typed composer layers
    // the whole Dynamic over a materialized static owner (see `HANDOFF.md`, gate G1).
    show.programmers.set_group(
        show.session,
        GROUP.into(),
        ProgrammingOwner::Color.key(),
        program(&base()),
    );
    show.put(
        "dynamic",
        &definition.id.to_string(),
        serde_json::to_value(&definition).unwrap(),
    );
    assert!(show.programmers.apply_dynamic_values(
        show.session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: a,
            attribute: ProgrammingOwner::Color.key(),
            value: dynamic_on(&definition),
        }],
        None,
    ));
    show.record(1.0);
    let (list_id, recorded) = show.cue_list();
    let cue = &recorded["cues"][0];
    assert_eq!(recorded["cues"].as_array().unwrap().len(), 1);
    assert!(
        cue["changes"].as_array().is_none_or(Vec::is_empty),
        "no flattened per-fixture Color: {cue}"
    );
    let group_changes = cue["group_changes"].as_array().unwrap();
    assert_eq!(group_changes.len(), 1, "only the live-Group base");
    assert_eq!(group_changes[0]["group_id"], GROUP);
    let stored_base: AttributeValue =
        serde_json::from_value(group_changes[0]["value"].clone()).unwrap();
    assert_eq!(stored_base, program(&base()), "the exact static base");
    let rows = cue["dynamic_changes"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "one recorded DynamicOn row");
    let row: light_playback::CueDynamicChange = serde_json::from_value(rows[0].clone()).unwrap();
    assert_eq!(row.fixture_id, a);
    assert_eq!(row.attribute, ProgrammingOwner::Color.key());
    assert_eq!(
        row.value,
        dynamic_on(&definition),
        "the exact reference, overrides and embedded fallback are stored"
    );
    assert_eq!(
        rows[0].pointer("/value/dynamic/dynamic_id"),
        Some(&Value::String(definition.id.to_string())),
        "the Cue references the pool Dynamic by identity"
    );
    (show, definition, list_id, recorded)
}

/// The stored pool Dynamic decodes to exactly the authored whole semantic endpoints.
fn assert_stored_definition(show: &Show, definition: &DynamicDefinition) {
    let document = show.document();
    let stored = document
        .object("dynamic", &definition.id.to_string())
        .expect("the pool Dynamic is stored");
    let stored: DynamicDefinition = serde_json::from_value(stored.body().clone()).unwrap();
    assert_eq!(&stored, definition, "the stored definition is unchanged");
    let DynamicLaneBody::Programming(body) = &stored.lanes[0].body else {
        panic!("typed Programming lane")
    };
    let ProgrammingLaneConfiguration::Keyframes(keyframes) = &body.configuration else {
        panic!("keyframes")
    };
    let endpoints: Vec<_> = keyframes
        .points
        .iter()
        .map(|point| match &point.source {
            DynamicValueSource::Value {
                value: DynamicValue::Family(value),
            } => semantic(value).clone(),
            other => panic!("whole semantic endpoint expected, got {other:?}"),
        })
        .collect();
    assert_eq!(endpoints, [endpoint_a(), endpoint_b()], "exact endpoints");
}

/// GO the reopened Cue and capture one period: the per-frame sample of `target`, checked
/// physically against `profile`. Returns `(sample, native output, UV status)` per frame.
fn sample_period(
    live: &mut Live,
    profile: &FixtureProfile,
    target: FixtureId,
) -> Vec<(ColorIntent, Vec<u32>, UvFitStatus)> {
    live.go(1.0);
    SCHEDULE
        .iter()
        .map(|&advance| {
            let frame = live.frame(advance);
            assert!(
                live.cue_rows.iter().any(|row| row.fixture_id == target
                    && matches!(row.value, DynamicSemanticValue::DynamicOn { .. })),
                "{}: the sample comes from the captured Cue row",
                profile.name
            );
            let result = frame.of(target);
            let native = live.native(&frame.capture, target);
            let output = assert_physical(profile, &native, result);
            (requested(result).clone(), output, result.quality.uv)
        })
        .collect()
}

#[test]
fn recorded_semantic_color_dynamic_keeps_intent_across_reopen_and_fixture_replacement() {
    let a = FixtureId::new();
    let (show, definition, list_id, recorded) = recorded_show(a);
    let replacements = [rgb(), rgbw_white_on(), cmy_wheel(), rgbwauv(None)];
    let mut reference: Option<Vec<ColorIntent>> = None;
    for profile in &replacements {
        let name = &profile.name;
        show.patch(&fixture(profile, a, 1, 1));
        assert_eq!(
            show.cue_list(),
            (list_id.clone(), recorded.clone()),
            "{name}: the stored Cue list is unchanged"
        );
        assert_stored_definition(&show, &definition);
        let snapshot = show.compile();
        assert_eq!(snapshot.dynamics.as_slice(), [definition.clone()]);
        assert_eq!(
            snapshot.cue_lists[0].cues[0].dynamic_changes[0].value,
            dynamic_on(&definition),
            "{name}: the compiled Cue keeps the declarative reference"
        );
        let mut live = Live::open(snapshot);
        let period = sample_period(&mut live, profile, a);
        let samples: Vec<_> = period.iter().map(|(sample, ..)| sample.clone()).collect();
        // Stored endpoints are reached exactly at the original keyframe phases.
        assert_eq!(samples[0], endpoint_a(), "{name}: phase 0 is endpoint A");
        assert_eq!(samples[2], endpoint_b(), "{name}: phase 0.5 is endpoint B");
        assert_eq!(samples[4], endpoint_a(), "{name}: the loop returns to A");
        assert_ne!(samples[1], endpoint_a(), "{name}: interior sample moves");
        assert!(
            samples.iter().all(|sample| *sample != base()),
            "{name}: the whole Dynamic replaces the static base"
        );
        match &reference {
            None => reference = Some(samples),
            Some(reference) => assert_eq!(
                &samples, reference,
                "{name}: same semantic sample (White Blend, white target, UV) at each phase"
            ),
        }
        // Phase 0 (A carries UV 0.4): the UV capability difference is explicit and passive.
        let (sample, output, uv_status) = &period[0];
        assert_eq!(sample.uv.amount, 0.4, "{name}: UV payload kept");
        if profile.name.contains("UV") {
            assert_eq!(*uv_status, UvFitStatus::Applied, "{name}");
            assert_eq!(
                *output.last().unwrap(),
                (0.4f64 * 255.).round() as u32,
                "{name}: UV driven"
            );
        } else {
            assert_eq!(
                *uv_status,
                UvFitStatus::Unsupported,
                "{name}: unsupported UV stays passive data"
            );
        }
        if profile.name == rgbw_white_on().name {
            for (index, (_, output, _)) in period.iter().enumerate() {
                assert_ne!(
                    output[WHITE], 204,
                    "{name} frame {index}: White is decided, never left at its default"
                );
            }
        }
        let counters = live.lane.adapter().counters();
        assert_eq!(
            counters.fitting_compiles, 1,
            "{name}: one fitter per generation"
        );
    }
    assert_eq!(show.cue_list(), (list_id, recorded), "no Cue rewrite");
}

#[test]
fn new_live_group_member_receives_the_current_semantic_dynamic_sample_without_rewrite() {
    let (a, b) = (FixtureId::new(), FixtureId::new());
    let (show, definition, list_id, recorded) = recorded_show(a);
    let profile = rgbw_white_on();
    show.patch(&fixture(&profile, a, 1, 1));
    let mut live = Live::open(show.compile());
    let before = sample_period(&mut live, &profile, a);
    let instances = live.runtime.instance_ids();
    assert_eq!(instances.len(), 1);

    // After recording, a new fixture joins the live Group; the show is saved and recompiled
    // and the generation is installed in place under the running Cue.
    show.patch(&fixture(&profile, b, 2, 40));
    show.group(&[a, b]);
    assert_eq!(show.cue_list(), (list_id, recorded), "no Cue rewrite");
    assert_stored_definition(&show, &definition);
    live.engine.replace_snapshot(show.compile()).unwrap();
    for (index, &advance) in SCHEDULE[1..].iter().enumerate() {
        let frame = live.frame(advance);
        assert_eq!(
            live.runtime.instance_ids(),
            instances,
            "same running instance"
        );
        assert!(
            live.cue_rows.iter().all(|row| row.fixture_id == a),
            "the stored Cue row still names only the recorded fixture"
        );
        let (first, second) = (frame.of(a), frame.of(b));
        let phase = &before[index + 1].0;
        assert_eq!(
            requested(first),
            phase,
            "frame {index}: original phase kept"
        );
        assert_eq!(
            requested(second),
            requested(first),
            "frame {index}: the new member receives the current sample"
        );
        let native = live.native(&frame.capture, b);
        assert_eq!(native[WHITE], 204, "the new member starts with White on");
        let output = assert_physical(&profile, &native, second);
        assert_eq!(
            second.writes.iter().map(|w| w.raw).collect::<Vec<_>>(),
            first.writes.iter().map(|w| w.raw).collect::<Vec<_>>(),
            "frame {index}: identical decisions, no stale channel"
        );
        assert_ne!(
            output[WHITE], 204,
            "frame {index}: White default never borrowed"
        );
    }
}
