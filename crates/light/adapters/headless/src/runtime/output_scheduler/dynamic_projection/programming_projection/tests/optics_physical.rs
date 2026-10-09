//! TL-558 seam tests: the Focus/Zoom adapters as the actual destination writers inside the
//! existing Live hybrid composer, observer and engine finalizer. Focus and Zoom run as
//! independent typed Dynamics over independent programmer owners.
use super::super::super::physical_adapter::optics::profiles::{
    multi_function_zoom, patched, spot_b, two_head_shared_zoom, wash_a,
};
use super::super::super::physical_adapter::optics::tests::{field, focus, zoom};
use super::super::super::physical_adapter::optics::{
    OpticsAdapter, OpticsLanes, family_owner, finalize_live_optics_frame,
};
use super::super::super::physical_adapter::*;
use super::super::hybrid::*;
use super::*;
use light_core::OpeningConvention;
use light_engine::PreparedOutputFrame;
use light_fixture::forward::CompiledOpticsForward;
use light_fixture::{FixtureProfile, OpticsFitStatus};

fn scalar_lane(address: DynamicValueAddress, value: f32) -> DynamicLane {
    lane_with(
        address,
        DynamicValueSource::Value {
            value: DynamicValue::Scalar(value),
        },
    )
}

fn lane_with(address: DynamicValueAddress, source: DynamicValueSource) -> DynamicLane {
    let mut lane = pan_definition().lanes.remove(0);
    lane.id = Uuid::new_v4();
    lane.body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address,
        configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
            points: [0., 0.5]
                .map(|position| DynamicKeyframe {
                    position,
                    source: source.clone(),
                    interpolation: light_dynamics::ScalarInterpolation::Linear,
                })
                .to_vec(),
            size: 1.,
        }),
    });
    lane
}

/// Focus lane at 75% and a Zoom lane at `degrees` in `convention`, one Dynamic.
fn optics_definition(degrees: f32, convention: OpeningConvention) -> DynamicDefinition {
    let mut definition = pan_definition();
    definition.name = "Focus and Zoom".into();
    definition.lanes = vec![
        scalar_lane(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Focus,
                component: Some(ProgrammingComponent::Focus),
            },
            0.75,
        ),
        scalar_lane(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Zoom { convention },
                component: Some(ProgrammingComponent::Zoom),
            },
            degrees,
        ),
    ];
    definition
}

/// Root fixture with every non-shared profile head patched as its own logical-head target.
fn patch_with_heads(profile: &FixtureProfile, target: FixtureId) -> light_fixture::PatchedFixture {
    let mut fixture = patched(profile, target, 1);
    let ids: Vec<_> = profile.modes[0]
        .heads
        .iter()
        .filter(|head| !head.master_shared)
        .map(|head| head.id)
        .collect();
    fixture.logical_heads = fixture
        .definition
        .heads
        .iter()
        .filter(|head| !head.shared)
        .zip(ids)
        .map(|(head, id)| light_fixture::PatchedHead {
            profile_head_id: Some(id),
            head_index: head.index,
            fixture_id: FixtureId::new(),
        })
        .collect();
    fixture
}

struct Live {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    target: FixtureId,
    clock: Arc<ManualClock>,
    runtime: DynamicRuntime,
    origins: DynamicSourceOrigins,
    transaction: DynamicOutputFrameScratch,
    hybrid: HybridFrameScratch,
    definition: DynamicDefinition,
    links: [Uuid; 2],
}

impl Live {
    fn new(profile: &FixtureProfile, definition: DynamicDefinition) -> Self {
        Self::with_zoom_base(profile, definition, field(30.))
    }

    /// `zoom_base` is the static programmer value under the Zoom effect (typed or legacy).
    fn with_zoom_base(
        profile: &FixtureProfile,
        definition: DynamicDefinition,
        zoom_base: AttributeValue,
    ) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        let target = FixtureId::new();
        programmers.start(session);
        // Independent static owners under the effects.
        programmers.set(session, target, ProgrammingOwner::Focus.key(), focus(0.2));
        programmers.set(session, target, ProgrammingOwner::Zoom.key(), zoom_base);
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
            links: [Uuid::new_v4(), Uuid::new_v4()],
        };
        live.engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![patch_with_heads(profile, target)].into(),
                dynamics: vec![live.definition.clone()].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        let mutations = [ProgrammingOwner::Focus, ProgrammingOwner::Zoom]
            .into_iter()
            .enumerate()
            .map(|(index, owner)| live.assign(index, owner))
            .collect::<Vec<_>>();
        assert!(
            live.programmers
                .apply_dynamic_values(session, &mutations, None)
        );
        live
    }

    /// Run a second Dynamic with a Zoom lane at `degrees` on the first logical-head target.
    /// Returns that head target.
    fn add_shared_zoom_head(&mut self, degrees: f32) -> FixtureId {
        let snapshot = self.engine.snapshot();
        let fixture = snapshot.fixtures[0].clone();
        let head = fixture.logical_heads[0].fixture_id;
        let mut second = optics_definition(degrees, OpeningConvention::Field);
        second.pool_number = 2;
        self.runtime
            .install_definitions([self.definition.clone(), second.clone()])
            .unwrap();
        self.engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![fixture].into(),
                dynamics: vec![self.definition.clone(), second.clone()].into(),
                revision: snapshot.revision + 1,
                ..Default::default()
            })
            .unwrap();
        self.programmers
            .set(self.session, head, ProgrammingOwner::Zoom.key(), field(30.));
        let mut assign = self.assign(1, ProgrammingOwner::Zoom);
        if let light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id,
            value:
                DynamicSemanticValue::DynamicOn {
                    instance_link,
                    lane_id,
                    dynamic,
                    ..
                },
            ..
        } = &mut assign
        {
            *fixture_id = head;
            *instance_link = Uuid::new_v4();
            *lane_id = second.lanes[1].id;
            dynamic.dynamic_id = Some(second.id);
            dynamic.last_known_pool_number = second.pool_number;
            dynamic.embedded_fallback = DynamicDefinitionSnapshot {
                definition: Arc::new(second.clone()),
            };
        }
        assert!(
            self.programmers
                .apply_dynamic_values(self.session, &[assign], None)
        );
        head
    }

    fn assign(
        &self,
        index: usize,
        owner: ProgrammingOwner,
    ) -> light_programmer::DynamicProgrammerValueMutation {
        light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: self.target,
            attribute: owner.key(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: self.links[index],
                lane_id: self.definition.lanes[index].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(self.definition.id),
                    last_known_pool_number: self.definition.pool_number,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(self.definition.clone()),
                    },
                },
                overrides: DynamicInstanceOverrides {
                    size: 1.,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.,
                },
                timing: Default::default(),
            },
        }
    }

    fn capture(&self) -> PreparedOutputFrame {
        self.clock.advance_millis(100);
        self.engine.prepare_output_frame(Default::default())
    }

    fn run(
        &mut self,
        capture: &PreparedOutputFrame,
        lanes: &OpticsLanes,
    ) -> Result<PublishedPhysicalFrame<OpticsAdapter>, DynamicRuntimeError> {
        self.run_with(capture, lanes, &[], None)
    }

    /// `run` that also stages `incidental` requirements after preparation and can finalize
    /// against another capture (`finalize`), which the finalizer must reject.
    fn run_with(
        &mut self,
        capture: &PreparedOutputFrame,
        lanes: &OpticsLanes,
        incidental: &[HybridFamilyRequirement],
        finalize: Option<&PreparedOutputFrame>,
    ) -> Result<PublishedPhysicalFrame<OpticsAdapter>, DynamicRuntimeError> {
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
                    lanes,
                    None,
                    |observation| lanes.observe(observation),
                )?;
                if !incidental.is_empty() {
                    lanes
                        .hold_frame(&prepared.frame_token, incidental)
                        .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
                }
                finalize_live_optics_frame(engine, finalize.unwrap_or(capture), lanes, prepared)
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
        result: &PhysicalHeadResult<OpticsAdapter>,
    ) -> Vec<u32> {
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
        let model = CompiledOpticsForward::compile(&profile.modes[0]).unwrap();
        let mut forward = model.create_output();
        model.evaluate(&raw, &mut forward).unwrap();
        let achieved = match result.owner {
            ProgrammingOwner::Focus => forward[0].focus.map(|f| f.percent / 100.),
            _ => forward[0].zoom.map(|z| z.degrees),
        };
        assert_eq!(result.achieved, achieved, "{:?}", result.owner);
        raw
    }
}

fn result(
    frame: &PublishedPhysicalFrame<OpticsAdapter>,
    owner: ProgrammingOwner,
) -> &PhysicalHeadResult<OpticsAdapter> {
    frame
        .results
        .iter()
        .find(|r| r.owner == owner)
        .unwrap_or_else(|| panic!("{owner:?} sidecar"))
}

#[test]
fn live_focus_and_zoom_dynamics_resolve_as_independent_owners_under_one_token() {
    let profile = wash_a();
    let mut live = Live::new(&profile, optics_definition(20., OpeningConvention::Field));
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    let published = live.run(&capture, &lanes).unwrap();
    assert!(published.requirements.is_empty());
    assert_eq!(published.results.len(), 2, "one sidecar per owner");
    let (f, z) = (
        result(&published, ProgrammingOwner::Focus),
        result(&published, ProgrammingOwner::Zoom),
    );
    for r in [f, z] {
        assert_eq!(
            r.token, published.token,
            "writes and readouts share one token"
        );
        assert_eq!(
            published
                .rendered
                .resolved_values
                .value(live.target, &r.owner.key()),
            Some(&r.value),
            "the engine receives the composed value the sidecar describes"
        );
        assert_eq!(r.writes.len(), 1);
        assert_eq!(r.quality.status, OpticsFitStatus::Fitted);
        live.resimulate(&capture, &profile, r);
    }
    assert_eq!(f.value, focus(0.75));
    assert_eq!(z.value, field(20.));
    assert_eq!((f.requested.value, z.requested.value), (0.75, 20.));
    assert_eq!(
        (
            f.writes[0].slot.channel_index,
            z.writes[0].slot.channel_index
        ),
        (2, 1),
        "disjoint native controls"
    );
    assert_eq!(z.writes[0].raw, 32768);
    assert!((z.achieved.unwrap() - 20.).abs() < 1e-3);
    assert!((f.achieved.unwrap() - 0.75).abs() <= 0.5 / 190. + 1e-9);
    let counters = lanes.counters();
    assert_eq!(
        (
            counters.descriptor_compiles,
            counters.fitting_compiles,
            counters.resolves
        ),
        (2, 1, 2),
        "one descriptor per owner, one shared fitter"
    );

    let second = live.capture();
    let published = live.run(&second, &lanes).unwrap();
    assert!(published.released.is_empty());
    for owner in [ProgrammingOwner::Focus, ProgrammingOwner::Zoom] {
        assert_eq!(result(&published, owner).token, second.frame_token());
    }
    assert_eq!(
        lanes
            .lane(light_fixture::OpticsFamily::Zoom)
            .continuity(live.target, ProgrammingOwner::Zoom)
            .unwrap()
            .control
            .unwrap()
            .2,
        32768
    );
}

#[test]
fn releasing_the_zoom_effect_keeps_focus_and_undo_restores_the_same_request() {
    let profile = wash_a();
    let mut live = Live::new(&profile, optics_definition(20., OpeningConvention::Field));
    let lanes = OpticsLanes::live();
    let first = live.capture();
    let before = live.run(&first, &lanes).unwrap();
    let focus_before = result(&before, ProgrammingOwner::Focus).writes.clone();

    assert!(live.programmers.apply_dynamic_values(
        live.session,
        &[light_programmer::DynamicProgrammerValueMutation::Release {
            fixture_id: live.target,
            attribute: ProgrammingOwner::Zoom.key(),
            instance_link: Some(live.links[1]),
        }],
        None
    ));
    let second = live.capture();
    let released = live.run(&second, &lanes).unwrap();
    let [only] = released.results.as_slice() else {
        panic!("only Focus keeps a physical sidecar")
    };
    assert_eq!(only.owner, ProgrammingOwner::Focus);
    assert_eq!(only.writes, focus_before, "Focus output is unchanged");
    assert_eq!(only.value, focus(0.75));
    let [gone] = released.released.as_slice() else {
        panic!("exactly Zoom is released")
    };
    assert_eq!(
        (gone.target, gone.owner),
        (live.target, ProgrammingOwner::Zoom)
    );
    assert!(
        lanes
            .lane(light_fixture::OpticsFamily::Zoom)
            .continuity(live.target, ProgrammingOwner::Zoom)
            .is_none()
    );
    assert_eq!(
        released
            .rendered
            .resolved_values
            .value(live.target, &ProgrammingOwner::Zoom.key()),
        Some(&field(30.)),
        "the static Zoom owner remains; the release is not a zero"
    );

    assert!(live.programmers.undo(live.session));
    let third = live.capture();
    let restored = live.run(&third, &lanes).unwrap();
    assert_eq!(restored.results.len(), 2);
    let z = result(&restored, ProgrammingOwner::Zoom);
    assert_eq!(
        z.requested,
        result(&before, ProgrammingOwner::Zoom).requested
    );
    assert_eq!(z.writes, result(&before, ProgrammingOwner::Zoom).writes);
    assert_eq!(
        result(&restored, ProgrammingOwner::Focus).writes,
        focus_before
    );
}

#[test]
fn a_zoom_effect_in_the_other_convention_is_a_requirement_not_a_conversion() {
    let profile = wash_a();
    // Beam lane over a Field static owner: no factor exists between the two conventions.
    let mut live = Live::new(&profile, optics_definition(20., OpeningConvention::Beam));
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    let published = live.run(&capture, &lanes).unwrap();
    let [only] = published.results.as_slice() else {
        panic!("Focus progresses independently")
    };
    assert_eq!(only.owner, ProgrammingOwner::Focus);
    assert!(
        published
            .requirements
            .iter()
            .any(|r| r.owner == ProgrammingOwner::Zoom && r.target == live.target),
        "the Zoom owner keeps its scalar path with a passive requirement"
    );
    assert_eq!(
        published
            .rendered
            .resolved_values
            .value(live.target, &ProgrammingOwner::Zoom.key()),
        Some(&field(30.)),
        "the stored Field request is not rewritten"
    );
}

/// Focus lane at 75% and a Zoom lane that reads its Current opening in `convention`.
fn current_zoom_definition(convention: OpeningConvention) -> DynamicDefinition {
    let mut definition = optics_definition(20., convention);
    definition.lanes[1] = lane_with(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Zoom { convention },
            component: Some(ProgrammingComponent::Zoom),
        },
        DynamicValueSource::Current,
    );
    definition
}

#[test]
fn legacy_native_zoom_current_is_adopted_through_the_forward_model() {
    let profile = wash_a();
    // A legacy exact raw on the U16 zoom: 16384 is 35° Field on the Wash A curve.
    let mut live = Live::with_zoom_base(
        &profile,
        current_zoom_definition(OpeningConvention::Field),
        AttributeValue::RawDmxExact(16384),
    );
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    let published = live.run(&capture, &lanes).unwrap();
    let z = result(&published, ProgrammingOwner::Zoom);
    assert_eq!(
        z.value,
        field(35.),
        "Current adopted as the measured opening"
    );
    assert_eq!(
        z.writes[0].raw, 16384,
        "adoption round-trips the native output"
    );
    assert_eq!(z.quality.status, OpticsFitStatus::Fitted);
    assert_eq!(
        result(&published, ProgrammingOwner::Focus).value,
        focus(0.75)
    );
    live.resimulate(&capture, &profile, z);
}

#[test]
fn legacy_zoom_current_in_the_other_convention_is_a_scoped_requirement_not_a_frame_failure() {
    let profile = wash_a();
    let mut live = Live::with_zoom_base(
        &profile,
        current_zoom_definition(OpeningConvention::Beam),
        AttributeValue::RawDmxExact(16384),
    );
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    let published = live.run(&capture, &lanes).unwrap();
    let [only] = published.results.as_slice() else {
        panic!("Focus progresses independently")
    };
    assert_eq!(only.owner, ProgrammingOwner::Focus);
    assert!(
        published
            .requirements
            .iter()
            .any(|r| r.owner == ProgrammingOwner::Zoom)
    );
    assert_eq!(
        published
            .rendered
            .resolved_values
            .value(live.target, &ProgrammingOwner::Zoom.key()),
        Some(&AttributeValue::RawDmxExact(16384)),
        "the legacy value keeps its scalar path"
    );
}

#[test]
fn two_heads_driving_one_shared_zoom_control_are_rejected_never_last_writer_wins() {
    let profile = two_head_shared_zoom();
    // Root (master head) Zoom 20°, logical head Zoom 40°: one native U16 control.
    let mut live = Live::new(&profile, optics_definition(20., OpeningConvention::Field));
    let head = live.add_shared_zoom_head(40.);
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    let error = live
        .run(&capture, &lanes)
        .err()
        .expect("conflicting shared writes");
    assert!(
        error
            .to_string()
            .contains("disagree on a shared native control"),
        "{error}"
    );
    for family in [
        light_fixture::OpticsFamily::Focus,
        light_fixture::OpticsFamily::Zoom,
    ] {
        assert!(
            lanes.lane(family).last_accepted().is_none(),
            "nothing committed"
        );
    }

    // The same opening on both heads is one consistent native write and is accepted.
    let mut live = Live::new(&profile, optics_definition(20., OpeningConvention::Field));
    let head_20 = live.add_shared_zoom_head(20.);
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    let published = live.run(&capture, &lanes).unwrap();
    let zooms: Vec<_> = published
        .results
        .iter()
        .filter(|r| r.owner == ProgrammingOwner::Zoom)
        .collect();
    assert_eq!(zooms.len(), 2);
    assert_eq!(zooms[0].writes, zooms[1].writes, "one slot, one raw");
    assert_eq!(zooms[0].writes[0].slot.destination, live.target);
    let inherited = zooms.iter().find(|r| r.target == head_20).unwrap();
    assert!(
        inherited.quality.shared,
        "the logical head drives the master's control"
    );
    assert_ne!(head, head_20);
}

#[test]
fn a_recalled_zoom_preset_reaches_its_opening_on_differing_optics_and_leaves_focus() {
    // One universal Zoom preset (25° Field); a Zoom effect reads it through Current.
    let preset = light_programmer::Preset {
        instance_id: None,
        name: "Zoom 25".into(),
        family: light_programmer::PresetFamily::Beam,
        number: 1,
        values: Default::default(),
        group_values: Default::default(),
        aim_at_fixture_number: None,
        universal_values: [(ProgrammingOwner::Zoom.key(), field(25.))].into(),
    };
    let mut raws = Vec::new();
    for profile in [wash_a(), spot_b()] {
        let mut live = Live::new(&profile, current_zoom_definition(OpeningConvention::Field));
        let mutations = light_application::materialize_preset_fixture_values(
            &preset,
            &[live.target],
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
        assert!(
            live.programmers
                .apply_normal_preset_recall(live.session, &mutations, "preset".into())
                .is_some()
        );
        let values = live.programmers.get(live.session).unwrap().values;
        let stored = |key: &light_core::AttributeKey| {
            values
                .iter()
                .find(|v| v.fixture_id == live.target && v.attribute == *key)
                .map(|v| v.value.clone())
        };
        assert_eq!(stored(&ProgrammingOwner::Zoom.key()), Some(field(25.)));
        assert_eq!(
            stored(&ProgrammingOwner::Focus.key()),
            Some(focus(0.2)),
            "a Zoom preset never touches Focus"
        );
        let lanes = OpticsLanes::live();
        let capture = live.capture();
        let published = live.run(&capture, &lanes).unwrap();
        let z = result(&published, ProgrammingOwner::Zoom);
        assert_eq!(z.value, field(25.));
        assert_eq!(z.requested.value, 25.);
        assert_eq!(z.quality.status, OpticsFitStatus::Fitted);
        assert!((z.achieved.unwrap() - 25.).abs() < 0.2, "{:?}", z.achieved);
        live.resimulate(&capture, &profile, z);
        raws.push(z.writes[0].raw);
    }
    assert_ne!(raws[0], raws[1], "same degrees, each fixture's own curve");
}

/// Focus and Zoom lanes that both read their Current (static, cue-faded) value.
fn current_optics_definition() -> DynamicDefinition {
    let mut definition = current_zoom_definition(OpeningConvention::Field);
    definition.lanes[0] = lane_with(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
        DynamicValueSource::Current,
    );
    definition
}

/// Cue 1: Focus 20 %, Zoom 10°. Cue 2: Focus 80 % in 1 s, Zoom 50° in 2 s (per-change times).
fn install_optics_cues(live: &Live) {
    use light_playback::*;
    let (focus_key, zoom_key) = (ProgrammingOwner::Focus.key(), ProgrammingOwner::Zoom.key());
    let mut first = Cue::new(1_u16.into());
    first.changes = vec![
        CueChange::set(live.target, focus_key.clone(), focus(0.2)),
        CueChange::set(live.target, zoom_key.clone(), field(10.)),
    ];
    let mut second = Cue::new(2_u16.into());
    let mut focus_change = CueChange::set(live.target, focus_key, focus(0.8));
    focus_change.fade_millis = Some(1_000);
    let mut zoom_change = CueChange::set(live.target, zoom_key, field(50.));
    zoom_change.fade_millis = Some(2_000);
    second.changes = vec![focus_change, zoom_change];
    second.fade_millis = 0;
    let list = CueList {
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
        id: light_core::CueListId::new(),
        name: "Optics".into(),
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
        speed_multiplier: 1.0,
        cues: vec![first, second],
    };
    let target = PlaybackTarget::CueList {
        cue_list_id: list.id,
    };
    let playback = PlaybackDefinition {
        number: 1,
        name: "Optics".into(),
        buttons: PlaybackDefinition::default_buttons(&target),
        button_count: 3,
        fader: PlaybackDefinition::default_fader(&target),
        has_fader: true,
        footprint: PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
        target,
    };
    let mut snapshot = (*live.engine.snapshot()).clone();
    snapshot.cue_lists = vec![list].into();
    snapshot.playbacks = vec![playback].into();
    snapshot.revision += 1;
    live.engine.replace_snapshot(snapshot).unwrap();
}

#[test]
fn cue_faded_focus_and_zoom_keep_their_own_times_through_independent_dynamics() {
    let profile = wash_a();
    let mut live = Live::new(&profile, current_optics_definition());
    // The cue is the only static owner of either family.
    for owner in [ProgrammingOwner::Focus, ProgrammingOwner::Zoom] {
        assert!(live.programmers.release_fixture_attribute(
            live.session,
            live.target,
            &owner.key()
        ));
    }
    install_optics_cues(&live);
    let go = || {
        live.engine
            .execute_playback(light_engine::EnginePlaybackCommand::Pool {
                number: 1,
                action: light_engine::PoolPlaybackAction::Go,
            })
            .unwrap()
    };
    go();
    live.clock.advance_millis(10);
    go();
    let lanes = OpticsLanes::live();
    let mut observed = Vec::new();
    // capture() advances 100 ms: sample at 500, 1000, 1500 and 2000 ms after the second GO.
    for advance in [400, 400, 400, 400] {
        live.clock.advance_millis(advance);
        let capture = live.capture();
        let published = live.run(&capture, &lanes).unwrap();
        let (f, z) = (
            result(&published, ProgrammingOwner::Focus),
            result(&published, ProgrammingOwner::Zoom),
        );
        live.resimulate(&capture, &profile, f);
        live.resimulate(&capture, &profile, z);
        observed.push((f.requested.value, z.requested.value));
    }
    let expected = [(0.5, 20.), (0.8, 30.), (0.8, 40.), (0.8, 50.)];
    for ((focus, zoom), (want_focus, want_zoom)) in observed.iter().zip(expected) {
        assert!((focus - want_focus).abs() < 1e-3, "{observed:?}");
        assert!((zoom - want_zoom).abs() < 1e-2, "{observed:?}");
    }
}

// TL-601: accepted continuity against authored native edits, changed optics response and failed
// finalization, through the real Live composer, observer and engine finalizer.

fn zoom_function_ids(profile: &FixtureProfile) -> Vec<Uuid> {
    profile.modes[0].channels[1]
        .functions
        .iter()
        .map(|f| f.id)
        .collect()
}

/// A Focus lane moving 0.2 → 0.8 → 0.2 over the Dynamic's period: its sample shows its clock.
fn moving_focus_lane() -> DynamicLane {
    let mut lane = lane_with(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
        DynamicValueSource::Current,
    );
    if let DynamicLaneBody::Programming(ProgrammingLaneBody {
        configuration: ProgrammingLaneConfiguration::Keyframes(keyframes),
        ..
    }) = &mut lane.body
    {
        for (point, value) in keyframes.points.iter_mut().zip([0.2, 0.8]) {
            point.source = DynamicValueSource::Value {
                value: DynamicValue::Scalar(value),
            };
        }
    }
    lane
}

fn zoom_result(
    frame: &PublishedPhysicalFrame<OpticsAdapter>,
) -> &PhysicalHeadResult<OpticsAdapter> {
    result(frame, ProgrammingOwner::Zoom)
}

#[test]
fn a_fresh_native_zoom_edit_is_adopted_and_written_in_the_edited_function() {
    let profile = multi_function_zoom();
    let ids = zoom_function_ids(&profile);
    let mut definition = current_zoom_definition(OpeningConvention::Field);
    definition.lanes[0] = moving_focus_lane();
    // Same Dynamic and clock without the edit: the unrelated Focus lane must match it.
    let rigs = [(); 2].map(|_| {
        Live::with_zoom_base(
            &profile,
            definition.clone(),
            AttributeValue::RawDmxExact(200),
        )
    });
    let [mut live, mut control] = rigs;
    let (lanes, control_lanes) = (OpticsLanes::live(), OpticsLanes::live());
    let mut frames = Vec::new();
    for frame in 0..4 {
        if frame == 2 {
            // The operator's authored native edit: the reversed narrow function (40° → 10°).
            live.programmers.set(
                live.session,
                live.target,
                ProgrammingOwner::Zoom.key(),
                AttributeValue::RawDmxExact(50),
            );
        }
        let capture = live.capture();
        let published = live.run(&capture, &lanes).unwrap();
        let control_capture = control.capture();
        let reference = control.run(&control_capture, &control_lanes).unwrap();
        let z = zoom_result(&published);
        live.resimulate(&capture, &profile, z);
        live.resimulate(
            &capture,
            &profile,
            result(&published, ProgrammingOwner::Focus),
        );
        assert_eq!(
            result(&published, ProgrammingOwner::Focus).requested,
            result(&reference, ProgrammingOwner::Focus).requested,
            "frame {frame}: the unrelated Focus Dynamic keeps its clock"
        );
        let raws = |frame: &PublishedPhysicalFrame<OpticsAdapter>| {
            result(frame, ProgrammingOwner::Focus)
                .writes
                .iter()
                .map(|w| (w.slot.channel_index, w.channel_id, w.function_id, w.raw))
                .collect::<Vec<_>>()
        };
        assert_eq!(raws(&published), raws(&reference));
        frames.push((
            z.value.clone(),
            z.writes[0].raw,
            z.quality.function_id,
            result(&published, ProgrammingOwner::Focus).requested.value,
        ));
    }
    assert_ne!(
        frames[0].3, frames[1].3,
        "the Focus lane is actually moving"
    );
    for (value, raw, function, _) in &frames[..2] {
        assert_eq!((*raw, *function), (200, Some(ids[2])), "{value:?}");
    }
    for (value, raw, function, _) in &frames[2..] {
        assert_eq!(
            (*raw, *function),
            (50, Some(ids[0])),
            "Current adoption and output follow the edited command, not the accepted wide raw"
        );
        let AttributeValue::Zoom(zoom) = value else {
            panic!("typed Zoom")
        };
        assert_eq!(zoom.convention, OpeningConvention::Field);
        let light_core::programming::ScalarIntent::Value(degrees) = zoom.opening_degrees else {
            panic!("materialized opening")
        };
        assert!(
            (degrees - (40. - 50. / 99. * 30.)).abs() < 1e-3,
            "{degrees}"
        );
    }
    assert_eq!(
        lanes
            .lane(light_fixture::OpticsFamily::Zoom)
            .continuity(live.target, ProgrammingOwner::Zoom)
            .unwrap()
            .control
            .unwrap()
            .2,
        50
    );
    let continuity = lanes
        .lane(light_fixture::OpticsFamily::Zoom)
        .continuity(live.target, ProgrammingOwner::Zoom)
        .unwrap();
    assert_eq!(
        continuity.baseline,
        Some(50),
        "witness of the edited command"
    );
    let counters = lanes.counters();
    assert_eq!(
        (counters.descriptor_compiles, counters.fitting_compiles),
        (2, 1),
        "no per-frame compilation"
    );
    assert_eq!(
        counters.stale_continuity, 1,
        "only the edit frame drops continuity"
    );
}

#[test]
fn a_native_edit_into_the_macro_stays_a_scoped_requirement_and_reinstalls_nothing() {
    let profile = multi_function_zoom();
    let ids = zoom_function_ids(&profile);
    let mut live = Live::with_zoom_base(
        &profile,
        current_zoom_definition(OpeningConvention::Field),
        AttributeValue::RawDmxExact(200),
    );
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    let first = live.run(&capture, &lanes).unwrap();
    assert_eq!(zoom_result(&first).writes[0].raw, 200);
    let focus_writes = result(&first, ProgrammingOwner::Focus).writes.clone();

    // The macro has no opening: Current cannot be adopted in either convention.
    let set = |live: &Live, raw| {
        live.programmers.set(
            live.session,
            live.target,
            ProgrammingOwner::Zoom.key(),
            AttributeValue::RawDmxExact(raw),
        )
    };
    set(&live, 110);
    let capture = live.capture();
    let published = live.run(&capture, &lanes).unwrap();
    let [only] = published.results.as_slice() else {
        panic!("Focus progresses independently")
    };
    assert_eq!(only.owner, ProgrammingOwner::Focus);
    assert_eq!(only.writes, focus_writes);
    assert!(
        published
            .requirements
            .iter()
            .any(|r| r.owner == ProgrammingOwner::Zoom && r.target == live.target),
        "a scoped passive Zoom requirement, not a frame failure"
    );
    assert_eq!(
        published
            .rendered
            .resolved_values
            .value(live.target, &ProgrammingOwner::Zoom.key()),
        Some(&AttributeValue::RawDmxExact(110)),
        "the authored command keeps its scalar path: no guess, no zero, no old output"
    );

    // Back to an adoptable command in the narrow function: written there, not the old 200.
    set(&live, 50);
    let capture = live.capture();
    let published = live.run(&capture, &lanes).unwrap();
    let z = zoom_result(&published);
    assert_eq!((z.writes[0].raw, z.quality.function_id), (50, Some(ids[0])));
    live.resimulate(&capture, &profile, z);
    assert_eq!(
        result(&published, ProgrammingOwner::Focus).writes,
        focus_writes
    );
}

#[test]
fn changed_zoom_response_with_stable_ids_drops_continuity_and_unrelated_edits_keep_it() {
    // The typed static Zoom owner is not a native command, so the zoom channel's pre-master
    // baseline is its profile default: raw 90, inside the narrow function.
    let mut profile = multi_function_zoom();
    profile.modes[0].channels[1].default_raw = 90;
    profile.validate().unwrap();
    let ids = zoom_function_ids(&profile);
    // A 30° Field lane: fitted in the narrow function at raw 33.
    let mut live = Live::new(&profile, optics_definition(30., OpeningConvention::Field));
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    let first = live.run(&capture, &lanes).unwrap();
    assert_eq!(zoom_result(&first).writes[0].raw, 33);
    let focus_writes = result(&first, ProgrammingOwner::Focus).writes.clone();

    // Unrelated show edit: cue lists change, the fixture list and response do not.
    install_optics_cues(&live);
    let before = lanes.counters();
    for _ in 0..2 {
        let capture = live.capture();
        let published = live.run(&capture, &lanes).unwrap();
        assert_eq!(zoom_result(&published).writes[0].raw, 33);
        assert_eq!(zoom_result(&published).quality.function_id, Some(ids[0]));
    }
    let after = lanes.counters();
    assert_eq!(
        after.fitting_compiles, before.fitting_compiles,
        "fitter reused"
    );
    assert_eq!(
        after.descriptor_compiles,
        before.descriptor_compiles + 2,
        "one cold descriptor per owner for the new generation, none per frame"
    );

    // Same channel and function UUIDs, but the narrow function now ends at 40 and the macro
    // covers the baseline 90.
    let mut changed = profile.clone();
    let channel = &mut changed.modes[0].channels[1];
    channel.functions[0].dmx_to = 40;
    channel.functions[1].dmx_from = 41;
    changed.validate().unwrap();
    assert_eq!(zoom_function_ids(&changed), ids);
    let snapshot = live.engine.snapshot();
    live.engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![patch_with_heads(&changed, live.target)].into(),
            revision: snapshot.revision + 1,
            ..(*snapshot).clone()
        })
        .unwrap();
    let capture = live.capture();
    let published = live.run(&capture, &lanes).unwrap();
    let z = zoom_result(&published);
    assert_eq!(
        z.quality.status,
        OpticsFitStatus::Ambiguous,
        "the baseline lies in the macro; the old raw is not reused"
    );
    assert!(z.quality.held);
    assert_eq!(z.writes[0].raw, 90, "held at the current baseline");
    assert_eq!(z.achieved, None, "unknown, not zero");
    assert_eq!(z.requested.value, 30., "the request is unchanged");
    live.resimulate(&capture, &changed, z);
    assert_eq!(
        result(&published, ProgrammingOwner::Focus).writes,
        focus_writes,
        "Focus is independent"
    );
}

#[test]
fn a_failed_live_finalization_never_installs_the_proposed_zoom_continuity() {
    let profile = two_head_shared_zoom();
    // Current adoption of the authored raw 16384 (35° Field) on the master-shared U16 zoom.
    let mut live = Live::with_zoom_base(
        &profile,
        current_zoom_definition(OpeningConvention::Field),
        AttributeValue::RawDmxExact(16384),
    );
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    let first = live.run(&capture, &lanes).unwrap();
    assert_eq!(zoom_result(&first).writes[0].raw, 16384);
    let zoom_lane = lanes.lane(light_fixture::OpticsFamily::Zoom);
    let focus_lane = lanes.lane(light_fixture::OpticsFamily::Focus);
    let accepted = (
        zoom_lane.continuity(live.target, ProgrammingOwner::Zoom),
        focus_lane.continuity(live.target, ProgrammingOwner::Focus),
        zoom_lane.last_accepted(),
    );

    // A fresh authored edit (12.5°) plus a second head asking 40° of the same control: the
    // proposed root write differs, and the frame fails before the finalizer commits.
    live.programmers.set(
        live.session,
        live.target,
        ProgrammingOwner::Zoom.key(),
        AttributeValue::RawDmxExact(49152),
    );
    let head = live.add_shared_zoom_head(40.);
    let capture = live.capture();
    let error = live.run(&capture, &lanes).err().expect("shared conflict");
    assert!(
        error
            .to_string()
            .contains("disagree on a shared native control")
    );
    assert_eq!(
        (
            zoom_lane.continuity(live.target, ProgrammingOwner::Zoom),
            focus_lane.continuity(live.target, ProgrammingOwner::Focus),
            zoom_lane.last_accepted(),
        ),
        accepted,
        "the proposed continuity of the failed frame is discarded"
    );
    assert!(zoom_lane.continuity(head, ProgrammingOwner::Zoom).is_none());
}

// TL-602: a passive Focus/Zoom requirement of a successful Live frame holds exactly its own
// accepted continuity (the TL-598 lane contract) instead of reading as a Release.

use light_fixture::OpticsFamily;

fn set_zoom_command(live: &Live, raw: u32) {
    live.programmers.set(
        live.session,
        live.target,
        ProgrammingOwner::Zoom.key(),
        AttributeValue::RawDmxExact(raw),
    );
}

/// Genuine source removal: the Dynamic instance of `owner` is released; its static owner stays.
fn release_dynamic(live: &Live, owner: ProgrammingOwner) {
    let index = usize::from(owner == ProgrammingOwner::Zoom);
    assert!(live.programmers.apply_dynamic_values(
        live.session,
        &[light_programmer::DynamicProgrammerValueMutation::Release {
            fixture_id: live.target,
            attribute: owner.key(),
            instance_link: Some(live.links[index]),
        }],
        None
    ));
}

fn incidental(target: FixtureId, owner: ProgrammingOwner) -> HybridFamilyRequirement {
    HybridFamilyRequirement {
        target,
        owner,
        reason: HybridFamilyRequirementReason::Composition(TransitionRequirement::ZoomConvention),
    }
}

/// A moving Focus lane beside a Zoom lane that adopts its authored native command (raw 200,
/// wide function) through Current. Raw 110 lies in the macro and makes Zoom passive.
fn macro_rig() -> (FixtureProfile, Live) {
    let profile = multi_function_zoom();
    let mut definition = current_zoom_definition(OpeningConvention::Field);
    definition.lanes[0] = moving_focus_lane();
    let live = Live::with_zoom_base(&profile, definition, AttributeValue::RawDmxExact(200));
    (profile, live)
}

fn assert_zoom_held(
    live: &Live,
    lanes: &OpticsLanes,
    capture: &PreparedOutputFrame,
    published: &PublishedPhysicalFrame<OpticsAdapter>,
    accepted: super::super::super::physical_adapter::optics::OpticsContinuity,
) {
    let [only] = published.results.as_slice() else {
        panic!("only Focus publishes a sidecar; no stale Zoom sidecar")
    };
    assert_eq!(
        (only.owner, &only.token),
        (ProgrammingOwner::Focus, &capture.frame_token())
    );
    assert!(
        published
            .requirements
            .iter()
            .any(|r| r.owner == ProgrammingOwner::Zoom && r.target == live.target),
        "a scoped passive Zoom requirement"
    );
    assert!(
        published.released.is_empty(),
        "a passive Zoom requirement is not a Release"
    );
    let zoom = lanes.lane(OpticsFamily::Zoom);
    assert_eq!(
        zoom.continuity(live.target, ProgrammingOwner::Zoom),
        Some(accepted),
        "the accepted Zoom continuity is held, not retired"
    );
    assert_eq!(zoom.last_accepted(), Some(capture.frame_token()));
    assert_eq!(
        lanes
            .lane(OpticsFamily::Focus)
            .continuity(live.target, ProgrammingOwner::Focus)
            .unwrap()
            .control
            .unwrap()
            .2,
        only.writes[0].raw,
        "Focus commits its own new solve in the same frame"
    );
}

#[test]
fn a_passive_macro_zoom_requirement_holds_its_continuity_without_release_and_removal_releases() {
    let (profile, mut live) = macro_rig();
    let ids = zoom_function_ids(&profile);
    let lanes = OpticsLanes::live();
    let zoom_lane = lanes.lane(OpticsFamily::Zoom);
    let first = live.capture();
    let solved = live.run(&first, &lanes).unwrap();
    assert_eq!(zoom_result(&solved).writes[0].raw, 200);
    let accepted = zoom_lane
        .continuity(live.target, ProgrammingOwner::Zoom)
        .unwrap();

    set_zoom_command(&live, 110);
    let mut focus_requests = vec![result(&solved, ProgrammingOwner::Focus).requested.value];
    for _ in 0..2 {
        let capture = live.capture();
        let held = live.run(&capture, &lanes).unwrap();
        assert_zoom_held(&live, &lanes, &capture, &held, accepted);
        focus_requests.push(result(&held, ProgrammingOwner::Focus).requested.value);
        assert_eq!(
            held.rendered
                .resolved_values
                .value(live.target, &ProgrammingOwner::Zoom.key()),
            Some(&AttributeValue::RawDmxExact(110)),
            "the authored command keeps its scalar path"
        );
        // A foreign or already accepted token holds nothing and changes nothing.
        assert!(
            lanes
                .hold_frame(&first.frame_token(), &held.requirements)
                .is_err()
        );
        assert_eq!(
            zoom_lane.continuity(live.target, ProgrammingOwner::Zoom),
            Some(accepted)
        );
    }
    assert!(
        focus_requests.windows(2).all(|pair| pair[0] != pair[1]),
        "the independent Focus Dynamic keeps moving: {focus_requests:?}"
    );

    // Recovery to the same authored command: both TL-601 witnesses still hold.
    let stale = lanes.counters().stale_continuity;
    set_zoom_command(&live, 200);
    let capture = live.capture();
    let recovered = live.run(&capture, &lanes).unwrap();
    let z = zoom_result(&recovered);
    assert_eq!(
        (z.writes[0].raw, z.quality.function_id),
        (200, Some(ids[2]))
    );
    live.resimulate(&capture, &profile, z);
    assert!(recovered.released.is_empty());
    assert_eq!(lanes.counters().stale_continuity, stale);
    let solve = capture.frame_token();

    // Genuine removal while held: Zoom releases with its last produced solve, never the hold.
    set_zoom_command(&live, 110);
    let capture = live.capture();
    let held = live.run(&capture, &lanes).unwrap();
    let recovered_continuity = zoom_lane
        .continuity(live.target, ProgrammingOwner::Zoom)
        .unwrap();
    assert_zoom_held(&live, &lanes, &capture, &held, recovered_continuity);
    release_dynamic(&live, ProgrammingOwner::Zoom);
    let capture = live.capture();
    let removed = live.run(&capture, &lanes).unwrap();
    let [gone] = removed.released.as_slice() else {
        panic!("exactly Zoom releases")
    };
    assert_eq!(
        (gone.target, gone.owner),
        (live.target, ProgrammingOwner::Zoom)
    );
    assert_eq!(
        gone.last_token, solve,
        "the Release names the original accepted solve token"
    );
    assert!(
        zoom_lane
            .continuity(live.target, ProgrammingOwner::Zoom)
            .is_none(),
        "removal retires continuity"
    );
    assert_eq!(
        result(&removed, ProgrammingOwner::Focus).token,
        capture.frame_token()
    );
}

#[test]
fn recovery_after_a_zoom_hold_respects_tl601_guards_and_holds_cover_only_their_own_keys() {
    let (profile, mut live) = macro_rig();
    let ids = zoom_function_ids(&profile);
    let lanes = OpticsLanes::live();
    let zoom_lane = lanes.lane(OpticsFamily::Zoom);
    let capture = live.capture();
    live.run(&capture, &lanes).unwrap();
    let accepted = zoom_lane
        .continuity(live.target, ProgrammingOwner::Zoom)
        .unwrap();
    set_zoom_command(&live, 110);
    let capture = live.capture();
    let held = live.run(&capture, &lanes).unwrap();
    assert_zoom_held(&live, &lanes, &capture, &held, accepted);

    // Baseline guard: a fresh edit drops the held raw 200. An incidental Zoom requirement in
    // the same frame never beats the produced result.
    let stale = lanes.counters().stale_continuity;
    set_zoom_command(&live, 50);
    let capture = live.capture();
    let recovered = live
        .run_with(
            &capture,
            &lanes,
            &[incidental(live.target, ProgrammingOwner::Zoom)],
            None,
        )
        .unwrap();
    let z = zoom_result(&recovered);
    assert_eq!((z.writes[0].raw, z.quality.function_id), (50, Some(ids[0])));
    assert_eq!(z.token, capture.frame_token());
    assert!(recovered.released.is_empty());
    assert_eq!(
        lanes.counters().stale_continuity,
        stale + 1,
        "the held continuity met the changed baseline witness and was dropped"
    );
    let produced = zoom_lane
        .continuity(live.target, ProgrammingOwner::Zoom)
        .unwrap();
    assert_eq!(
        (produced.control.unwrap().2, produced.baseline),
        (50, Some(50)),
        "the produced result wins over the incidental requirement"
    );

    // Zoom passive and Focus genuinely removed in one frame: the Zoom hold covers only Zoom.
    set_zoom_command(&live, 110);
    release_dynamic(&live, ProgrammingOwner::Focus);
    let capture = live.capture();
    let mixed = live.run(&capture, &lanes).unwrap();
    assert!(
        mixed.results.is_empty(),
        "no stale sidecar of either family"
    );
    let [gone] = mixed.released.as_slice() else {
        panic!("exactly Focus releases")
    };
    assert_eq!(
        (gone.target, gone.owner),
        (live.target, ProgrammingOwner::Focus)
    );
    assert!(
        lanes
            .lane(OpticsFamily::Focus)
            .continuity(live.target, ProgrammingOwner::Focus)
            .is_none()
    );
    assert_eq!(
        zoom_lane.continuity(live.target, ProgrammingOwner::Zoom),
        Some(produced)
    );

    // Response guard: same native UUIDs, the narrow function now ends at 90. Across the new
    // generation the macro command stays held; recovery to 50 drops the old response.
    let mut changed = profile.clone();
    let channel = &mut changed.modes[0].channels[1];
    channel.functions[0].dmx_to = 90;
    channel.functions[1].dmx_from = 91;
    changed.validate().unwrap();
    assert_eq!(zoom_function_ids(&changed), ids);
    let snapshot = live.engine.snapshot();
    live.engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![patch_with_heads(&changed, live.target)].into(),
            revision: snapshot.revision + 1,
            ..(*snapshot).clone()
        })
        .unwrap();
    let capture = live.capture();
    let held = live.run(&capture, &lanes).unwrap();
    assert!(held.released.is_empty());
    assert!(held.results.is_empty());
    assert_eq!(
        zoom_lane.continuity(live.target, ProgrammingOwner::Zoom),
        Some(produced),
        "a generation change alone neither releases nor rewrites the held continuity"
    );
    let stale = lanes.counters().stale_continuity;
    set_zoom_command(&live, 50);
    let capture = live.capture();
    let recovered = live.run(&capture, &lanes).unwrap();
    let z = zoom_result(&recovered);
    assert_eq!((z.writes[0].raw, z.quality.function_id), (50, Some(ids[0])));
    live.resimulate(&capture, &changed, z);
    assert_eq!(
        lanes.counters().stale_continuity,
        stale + 1,
        "the held continuity met the changed response witness and was dropped"
    );
    let continuity = zoom_lane
        .continuity(live.target, ProgrammingOwner::Zoom)
        .unwrap();
    assert_ne!(continuity.response, produced.response);
    assert_eq!(zoom_lane.last_accepted(), Some(capture.frame_token()));
}

#[test]
fn a_failed_live_finalization_with_a_zoom_hold_commits_nothing_and_the_retry_holds() {
    let (_, mut live) = macro_rig();
    let lanes = OpticsLanes::live();
    let capture = live.capture();
    live.run(&capture, &lanes).unwrap();
    let state = |lanes: &OpticsLanes, live: &Live| {
        [OpticsFamily::Focus, OpticsFamily::Zoom].map(|family| {
            let lane = lanes.lane(family);
            let owner = family_owner(family);
            (lane.continuity(live.target, owner), lane.last_accepted())
        })
    };
    let accepted = state(&lanes, &live);
    let zoom_accepted = accepted[1].0.unwrap();

    set_zoom_command(&live, 110);
    let capture = live.capture();
    let other = live.capture();
    let error = live
        .run_with(&capture, &lanes, &[], Some(&other))
        .err()
        .expect("the finalizer rejects another capture");
    assert!(error.to_string().contains("another capture"), "{error}");
    assert_eq!(
        state(&lanes, &live),
        accepted,
        "a failed finalizer commits neither the hold nor Focus"
    );
    assert!(lanes.released().is_empty());

    let capture = live.capture();
    let retried = live.run(&capture, &lanes).unwrap();
    assert_zoom_held(&live, &lanes, &capture, &retried, zoom_accepted);
}

#[test]
fn a_passive_convention_zoom_requirement_holds_and_the_original_solve_recovers() {
    let profile = wash_a();
    let mut live = Live::new(&profile, optics_definition(20., OpeningConvention::Field));
    let lanes = OpticsLanes::live();
    let zoom_lane = lanes.lane(OpticsFamily::Zoom);
    let capture = live.capture();
    let solved = live.run(&capture, &lanes).unwrap();
    let solve = capture.frame_token();
    let accepted = zoom_lane
        .continuity(live.target, ProgrammingOwner::Zoom)
        .unwrap();
    // The static owner under the Field lane changes to a Beam opening: no conversion exists.
    live.programmers.set(
        live.session,
        live.target,
        ProgrammingOwner::Zoom.key(),
        zoom(30., OpeningConvention::Beam),
    );
    let capture = live.capture();
    let held = live.run(&capture, &lanes).unwrap();
    assert_zoom_held(&live, &lanes, &capture, &held, accepted);
    assert_eq!(
        result(&held, ProgrammingOwner::Focus).writes,
        result(&solved, ProgrammingOwner::Focus).writes
    );

    live.programmers.set(
        live.session,
        live.target,
        ProgrammingOwner::Zoom.key(),
        field(30.),
    );
    let capture = live.capture();
    let recovered = live.run(&capture, &lanes).unwrap();
    assert!(recovered.released.is_empty());
    assert_eq!(
        zoom_result(&recovered).writes,
        zoom_result(&solved).writes,
        "recovery reaches the same solve"
    );
    // Removal after recovery still releases, naming the recovered solve.
    release_dynamic(&live, ProgrammingOwner::Zoom);
    let removal = live.capture();
    let removed = live.run(&removal, &lanes).unwrap();
    let [gone] = removed.released.as_slice() else {
        panic!("exactly Zoom releases")
    };
    assert_eq!(gone.owner, ProgrammingOwner::Zoom);
    assert_eq!(gone.last_token, capture.frame_token());
    assert_ne!(gone.last_token, solve);
    assert!(
        zoom_lane
            .continuity(live.target, ProgrammingOwner::Zoom)
            .is_none()
    );
}

/// No authored Focus command is passive through the real composer (a legacy Focus Current is a
/// frame error, not a requirement), so the Focus requirement row is supplied the way the
/// composer stages it: after preparation, before the finalizer.
#[test]
fn a_passive_focus_requirement_holds_only_focus_while_zoom_produces() {
    let profile = wash_a();
    let mut live = Live::new(&profile, optics_definition(20., OpeningConvention::Field));
    let lanes = OpticsLanes::live();
    let focus_lane = lanes.lane(OpticsFamily::Focus);
    let capture = live.capture();
    live.run(&capture, &lanes).unwrap();
    let solve = capture.frame_token();
    let accepted = focus_lane
        .continuity(live.target, ProgrammingOwner::Focus)
        .unwrap();

    // The Focus Dynamic no longer produces; its requirement is passive this frame.
    release_dynamic(&live, ProgrammingOwner::Focus);
    let capture = live.capture();
    let held = live
        .run_with(
            &capture,
            &lanes,
            &[incidental(live.target, ProgrammingOwner::Focus)],
            None,
        )
        .unwrap();
    let [only] = held.results.as_slice() else {
        panic!("only Zoom publishes a sidecar; no stale Focus sidecar")
    };
    assert_eq!(
        (only.owner, &only.token),
        (ProgrammingOwner::Zoom, &capture.frame_token())
    );
    assert!(held.released.is_empty(), "a passive Focus is not a Release");
    assert_eq!(
        focus_lane.continuity(live.target, ProgrammingOwner::Focus),
        Some(accepted)
    );
    assert_eq!(focus_lane.last_accepted(), Some(capture.frame_token()));
    assert_eq!(
        lanes
            .lane(OpticsFamily::Zoom)
            .continuity(live.target, ProgrammingOwner::Zoom)
            .unwrap()
            .control
            .unwrap()
            .2,
        only.writes[0].raw
    );

    // Without the requirement the same state is a genuine removal of the last produced solve.
    let capture = live.capture();
    let removed = live.run(&capture, &lanes).unwrap();
    let [gone] = removed.released.as_slice() else {
        panic!("exactly Focus releases")
    };
    assert_eq!(
        (gone.owner, &gone.last_token),
        (ProgrammingOwner::Focus, &solve)
    );
    assert!(
        focus_lane
            .continuity(live.target, ProgrammingOwner::Focus)
            .is_none()
    );
}
