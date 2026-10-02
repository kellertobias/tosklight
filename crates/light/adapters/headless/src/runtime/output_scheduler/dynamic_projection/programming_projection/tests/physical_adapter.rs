//! TL-590 seam tests: a typed physical adapter plugged into the existing Live hybrid resolver,
//! observer and engine finalizer. The adapter's numeric model is a test model only.
use super::super::super::physical_adapter::test_adapter::*;
use super::super::super::physical_adapter::*;
use super::super::hybrid::*;
use super::*;
use light_engine::{PreloadBranch, PreloadFrameState, PreparedOutputFrame};

fn target(tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [0., tilt, 0.],
    )))
}

fn point_target(offset: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::new_v4(),
        },
        offset,
    )))
}

/// Pan keyframes read Current; Tilt is the automatic Current partner.
fn current_pan() -> DynamicDefinition {
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
    definition
}

/// One whole-family Target lane between two different Points: interpolation needs the frame.
fn point_to_point() -> DynamicDefinition {
    let mut definition = pan_definition();
    let endpoints = [point_target([0., 0., 0.]), point_target([40., 20., 0.])];
    definition.lanes.truncate(1);
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target { reference: None },
            component: None,
        },
        configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
            points: [0., 0.5]
                .into_iter()
                .zip(endpoints)
                .map(|(position, value)| DynamicKeyframe {
                    position,
                    source: DynamicValueSource::Value {
                        value: DynamicValue::Family(value),
                    },
                    interpolation: light_dynamics::ScalarInterpolation::Linear,
                })
                .collect(),
            size: 1.,
        }),
    });
    definition
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
}

/// Tampering applied between preparation and the finalizer.
enum Tamper<'a> {
    None,
    IncidentalHold,
    FinalizeWith(&'a PreparedOutputFrame),
    Inject(Box<PhysicalHeadResult<SeamAdapter>>),
}

impl Live {
    fn new(baseline: AttributeValue, definition: DynamicDefinition) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        let target = FixtureId::new();
        programmers.start(session);
        programmers.set(session, target, ProgrammingOwner::Position.key(), baseline);
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
            }],
            None
        ));
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        runtime.install_definitions([definition]).unwrap();
        Self {
            engine,
            programmers,
            session,
            target,
            clock,
            runtime,
            origins: Default::default(),
            transaction: Default::default(),
            hybrid: Default::default(),
        }
    }

    fn fixed_position(&self, fixture: FixtureId, pan: f32, tilt: f32) {
        let value = position(pan, tilt);
        self.programmers.set(
            self.session,
            fixture,
            ProgrammingOwner::Position.key(),
            value.clone(),
        );
        assert!(
            self.programmers.apply_dynamic_values(
                self.session,
                &[light_programmer::DynamicProgrammerValueMutation::Set {
                    fixture_id: fixture,
                    attribute: ProgrammingOwner::Position.key(),
                    value: DynamicSemanticValue::ProgrammingFixAt {
                        mask: light_dynamics::ProgrammingFamilyFixAt::from_family(
                            ProgrammingOwner::Position,
                            None,
                            value,
                        )
                        .unwrap(),
                        timing: Default::default(),
                    },
                }],
                None,
            )
        );
    }

    fn capture(&self) -> PreparedOutputFrame {
        self.clock.advance_millis(100);
        self.engine.prepare_output_frame(Default::default())
    }

    fn run(
        &mut self,
        capture: &PreparedOutputFrame,
        lane: &PhysicalAdapterLane<SeamAdapter>,
        tamper: Tamper<'_>,
    ) -> Result<PublishedPhysicalFrame<SeamAdapter>, DynamicRuntimeError> {
        let engine = &self.engine;
        let target = self.target;
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
                let mut prepared = prepare_captured_hybrid_frame(
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
                let finalize = match tamper {
                    Tamper::None => capture,
                    Tamper::IncidentalHold => {
                        lane.hold_frame(
                            &prepared.frame_token,
                            &[HybridFamilyRequirement {
                                target,
                                owner: ProgrammingOwner::Position,
                                reason: HybridFamilyRequirementReason::Composition(
                                    TransitionRequirement::LiveJointAngles,
                                ),
                            }],
                        )
                        .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
                        capture
                    }
                    Tamper::FinalizeWith(other) => other,
                    Tamper::Inject(stale) => {
                        prepared.family_sidecars.push(*stale);
                        capture
                    }
                };
                finalize_live_physical_frame(engine, finalize, lane, prepared)
            });
        if output.is_ok() {
            self.origins = candidate;
        }
        output
    }
}

fn lane(owners: &[ProgrammingOwner]) -> PhysicalAdapterLane<SeamAdapter> {
    PhysicalAdapterLane::live(SeamAdapter::new(owners))
}

#[test]
fn live_adapter_adopts_publishes_one_token_and_carries_continuity() {
    let mut live = Live::new(target(15.), current_pan());
    let lane = lane(&[ProgrammingOwner::Position]);
    let first = live.capture();
    let published = live.run(&first, &lane, Tamper::None).unwrap();
    assert_eq!(
        lane.adapter().adoptions.get(),
        1,
        "the deferred Pan/Tilt Current pair adopts its captured Target together once"
    );
    assert!(published.requirements.is_empty());
    assert_eq!(published.token, first.frame_token());
    assert!(published.token.matches_geometry(&published.geometry));
    let [result] = published.results.as_slice() else {
        panic!("one complete Position owner")
    };
    assert_eq!(result.token, published.token);
    assert_eq!(result.value, position(35., 15.));
    assert_eq!(result.requested.primary, 35.);
    assert_eq!(result.achieved.secondary, 15.);
    assert_eq!(
        result
            .writes
            .iter()
            .map(|write| write.raw)
            .collect::<Vec<_>>(),
        [3500, 1500]
    );
    assert!(
        result
            .writes
            .iter()
            .all(|write| write.slot.destination == live.target)
    );
    assert_eq!(result.quality.previous, None);
    assert!(
        result.provenance.sources.entries().is_none(),
        "adopted Current without transfer evidence keeps its source explicitly unknown"
    );
    assert!(
        result.provenance.controls.is_some(),
        "controller coverage is still known"
    );
    assert_eq!(
        published
            .rendered
            .resolved_values
            .value(live.target, &ProgrammingOwner::Position.key()),
        Some(&result.value),
        "the engine publishes the same composed value the sidecar describes"
    );
    assert_eq!(lane.last_accepted(), Some(first.frame_token()));
    let generation = lane.descriptor_generation();
    assert_eq!(generation, Some(first.generation()));

    // Continuity flows from the accepted frame only; descriptors stay compiled.
    live.programmers.set(
        live.session,
        live.target,
        ProgrammingOwner::Position.key(),
        target(-45.),
    );
    let second = live.capture();
    // A later edit must not reach the captured frame.
    live.programmers.set(
        live.session,
        live.target,
        ProgrammingOwner::Position.key(),
        position(-170., 5.),
    );
    let published = live.run(&second, &lane, Tamper::None).unwrap();
    let result = &published.results[0];
    assert_eq!(result.value, position(35., -45.));
    assert_eq!(
        lane.adapter().adoptions.get(),
        2,
        "the next captured Target is adopted once more for the whole pair"
    );
    assert!(result.quality.clipped, "negative Tilt clips at raw 0");
    assert_eq!(
        result.quality.previous,
        Some(SeamContinuity {
            primary: 35.,
            frames: 1
        })
    );
    assert_eq!(lane.adapter().compiles.get(), 1);
    assert!(published.released.is_empty());
}

#[test]
fn live_transition_is_solved_by_the_frame_adapter_inside_the_existing_resolver() {
    let mut live = Live::new(position(10., 20.), point_to_point());
    let lane = lane(&[ProgrammingOwner::Position]);
    let capture = live.capture();
    let published = live.run(&capture, &lane, Tamper::None).unwrap();
    let progress = lane.adapter().transitions.borrow().clone();
    assert_eq!(progress.len(), 1, "one frame solve for the Point crossing");
    assert!(progress[0] > 0. && progress[0] < 1., "{progress:?}");
    let [result] = published.results.as_slice() else {
        panic!("one Position owner")
    };
    let expected = position(40. * progress[0], 20. * progress[0]);
    assert_eq!(result.value, expected);
    assert_eq!(result.token, capture.frame_token());
    assert_eq!(
        published
            .rendered
            .resolved_values
            .value(live.target, &ProgrammingOwner::Position.key()),
        Some(&expected)
    );
}

#[test]
fn mixed_or_stale_tokens_fail_before_live_mutation_and_retry_succeeds() {
    let mut live = Live::new(target(15.), current_pan());
    let lane = lane(&[ProgrammingOwner::Position]);
    let first = live.capture();
    let other = live.engine.prepare_output_frame(Default::default());
    let runtime = live.runtime.snapshot();
    let origins = live.origins.snapshot();
    // Finalizing the prepared frame through another capture is rejected by the adapter.
    assert!(
        live.run(&first, &lane, Tamper::FinalizeWith(&other))
            .is_err()
    );
    assert_eq!(live.runtime.snapshot(), runtime);
    assert_eq!(live.origins.snapshot(), origins);
    assert_eq!(lane.last_accepted(), None);
    // Nothing was committed: the same capture still finalizes, and its sidecar becomes stale.
    let accepted = live.run(&first, &lane, Tamper::None).unwrap();
    let stale = accepted.results.into_iter().next().unwrap();
    let second = live.capture();
    let runtime = live.runtime.snapshot();
    let continuity = lane.continuity(live.target, ProgrammingOwner::Position);
    assert!(
        live.run(&second, &lane, Tamper::Inject(Box::new(stale)))
            .is_err()
    );
    assert_eq!(live.runtime.snapshot(), runtime);
    assert_eq!(lane.last_accepted(), Some(first.frame_token()));
    assert_eq!(
        lane.continuity(live.target, ProgrammingOwner::Position),
        continuity
    );
    // The rejected frame rendered nothing: Live continuity accepts the same capture now.
    live.run(&second, &lane, Tamper::None).unwrap();
    assert_eq!(lane.last_accepted(), Some(second.frame_token()));
    // An older capture and a Preload token are rejected before any runtime mutation.
    let runtime = live.runtime.snapshot();
    assert!(live.run(&other, &lane, Tamper::None).is_err());
    assert_eq!(live.runtime.snapshot(), runtime);
    let input = live.engine.prepare_preload_frame(&second, None);
    let state = PreloadFrameState::default();
    assert!(
        lane.begin_frame(&input.frame_token(&state, PreloadBranch::AfterRelease))
            .is_err()
    );
}

#[test]
fn adapter_failure_publishes_nothing_and_missing_destination_is_passive() {
    let mut live = Live::new(target(15.), current_pan());
    let lane = lane(&[ProgrammingOwner::Position]);
    let capture = live.capture();
    let runtime = live.runtime.snapshot();
    let origins = live.origins.snapshot();
    for failure in [SeamFailure::Resolve, SeamFailure::IncompleteWrites] {
        lane.adapter().failure.set(Some(failure));
        assert!(
            live.run(&capture, &lane, Tamper::None).is_err(),
            "{failure:?}"
        );
        assert_eq!(live.runtime.snapshot(), runtime);
        assert_eq!(live.origins.snapshot(), origins);
        assert_eq!(lane.last_accepted(), None);
        assert_eq!(
            lane.continuity(live.target, ProgrammingOwner::Position),
            None
        );
    }
    // No physical model: the owner stays scalar/static with a passive requirement.
    let missing = self::lane(&[ProgrammingOwner::Position]);
    missing
        .adapter()
        .failure
        .set(Some(SeamFailure::MissingDestination));
    let published = live.run(&capture, &missing, Tamper::None).unwrap();
    assert!(published.results.is_empty());
    assert!(
        published
            .requirements
            .iter()
            .any(|row| row.target == live.target && row.owner == ProgrammingOwner::Position)
    );
    assert_eq!(missing.last_accepted(), Some(capture.frame_token()));
    // The failed lane retries a later capture cleanly.
    lane.adapter().failure.set(None);
    let retry = live.capture();
    let published = live.run(&retry, &lane, Tamper::None).unwrap();
    assert_eq!(published.results[0].value, position(35., 15.));
}

#[test]
fn live_passive_hold_keeps_last_solve_and_recovery_uses_it() {
    let mut live = Live::new(target(15.), current_pan());
    let lane = lane(&[ProgrammingOwner::Position]);
    let first = live.capture();
    let first_result = live.run(&first, &lane, Tamper::None).unwrap();
    let initial = lane
        .continuity(live.target, ProgrammingOwner::Position)
        .unwrap();
    // A real Current dependency cannot adopt the referenced Point this frame.
    live.programmers.set(
        live.session,
        live.target,
        ProgrammingOwner::Position.key(),
        point_target([0., 40., 0.]),
    );
    let held = live.capture();
    let published = live.run(&held, &lane, Tamper::None).unwrap();
    assert!(
        published.results.is_empty(),
        "a hold never republishes a stale solve"
    );
    assert!(!published.requirements.is_empty());
    assert!(published.released.is_empty());
    assert_eq!(
        lane.continuity(live.target, ProgrammingOwner::Position),
        Some(initial)
    );
    assert_eq!(lane.last_accepted(), Some(held.frame_token()));
    // Reject a foreign hold token without changing either accepted or staged state.
    assert!(
        lane.hold_frame(&first.frame_token(), &published.requirements)
            .is_err()
    );
    assert_eq!(
        lane.continuity(live.target, ProgrammingOwner::Position),
        Some(initial)
    );
    live.programmers.set(
        live.session,
        live.target,
        ProgrammingOwner::Position.key(),
        target(30.),
    );
    let recovered = live.capture();
    let published = live.run(&recovered, &lane, Tamper::IncidentalHold).unwrap();
    assert_eq!(published.results[0].quality.previous, Some(initial));
    assert_eq!(
        lane.continuity(live.target, ProgrammingOwner::Position)
            .unwrap()
            .frames,
        2,
        "a produced result wins over an incidental requirement"
    );
    assert!(published.released.is_empty());
    assert_eq!(first_result.results[0].token, first.frame_token());
}

#[test]
fn shared_native_heads_disagree_before_finalization_and_equal_writes_retry() {
    let mut live = Live::new(target(15.), current_pan());
    let other = FixtureId::new();
    live.fixed_position(other, 35., 15.);
    let mut adapter = SeamAdapter::new(&[ProgrammingOwner::Position]);
    adapter.shared_destination = Some(live.target);
    let lane = PhysicalAdapterLane::live(adapter);
    let first = live.capture();
    let published = live.run(&first, &lane, Tamper::None).unwrap();
    assert_eq!(
        published.results.len(),
        2,
        "equal shared writes are accepted"
    );
    let initial = [live.target, other].map(|id| lane.continuity(id, ProgrammingOwner::Position));
    live.fixed_position(other, 70., 15.);
    let disagrees = live.capture();
    let runtime = live.runtime.snapshot();
    let origins = live.origins.snapshot();
    let error = live
        .run(&disagrees, &lane, Tamper::None)
        .err()
        .expect("shared conflict");
    assert!(
        error.to_string().contains("shared native control"),
        "{error}"
    );
    assert_eq!(live.runtime.snapshot(), runtime);
    assert_eq!(live.origins.snapshot(), origins);
    assert_eq!(lane.last_accepted(), Some(first.frame_token()));
    assert_eq!(
        [live.target, other].map(|id| lane.continuity(id, ProgrammingOwner::Position)),
        initial
    );
    live.fixed_position(other, 35., 15.);
    let retry = live.capture();
    assert_eq!(
        live.run(&retry, &lane, Tamper::None).unwrap().results.len(),
        2
    );
    assert_eq!(lane.last_accepted(), Some(retry.frame_token()));
}
