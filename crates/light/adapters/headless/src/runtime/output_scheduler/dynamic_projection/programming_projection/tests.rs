use super::*;
use light_core::{ManualClock, SessionId, programming::*};
use light_dynamics::*;
use light_programmer::ProgrammerRegistry;
use std::cell::Cell;

mod color_arbitration;
mod color_direct_cues;
mod color_direct_dynamic;
mod color_direct_transition;
mod color_direct_whole_fade;
mod color_physical;
mod color_uv_widths;
mod composition_projection;
mod current_capture_memo;
mod current_dependencies;
mod media_color_physical;
mod optics_physical;
mod physical_adapter;
mod preload_hybrid;
mod source_evidence;
mod staged_geometry;

fn axis(component: ProgrammingComponent) -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(component),
    }
}

fn position(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}

fn no_adoption(
    _: FixtureId,
    _: &AttributeValue,
    _: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    panic!("compatible Current must not request geometry or a native model")
}

struct NoFrameConversion;
impl WholeFamilyExpressionFrameResolver for NoFrameConversion {
    fn resolve(
        &self,
        _: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("complete Angles in the same frame require no conversion")
    }
}

#[test]
fn typed_current_and_whole_size_read_the_same_captured_static_frame() {
    let clock = Arc::new(ManualClock::new(chrono::Utc::now()));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let fixture = FixtureId::new();
    programmers.start(session);
    programmers.set(
        session,
        fixture,
        ProgrammingOwner::Position.key(),
        position(30., 15.),
    );
    let engine = Engine::with_programming_contract_support(
        programmers.clone(),
        PROGRAMMING_CONTRACT_VERSION,
    );
    let capture = engine.prepare_output_frame(Default::default());
    let sources = TickSources::prepared(&engine, &capture, &[]);
    let typed = CapturedProgrammingSources::new(&sources, &no_adoption, None);
    clock.advance_millis(100);
    programmers.set(
        session,
        fixture,
        ProgrammingOwner::Position.key(),
        position(720., -90.),
    );
    assert!(
        sources.values.get().is_none(),
        "static resolution stays lazy"
    );
    assert_eq!(
        typed.current(fixture, &axis(ProgrammingComponent::Tilt)),
        Some(DynamicValue::Scalar(15.))
    );
    assert_eq!(
        typed.current_family_base(fixture, &axis(ProgrammingComponent::Pan)),
        Some(position(30., 15.))
    );
    typed.check().unwrap();
    let next_capture = engine.prepare_output_frame(Default::default());
    let next_sources = TickSources::prepared(&engine, &next_capture, &[]);
    let next = CapturedProgrammingSources::new(&next_sources, &no_adoption, None);
    assert_eq!(
        next.current(fixture, &axis(ProgrammingComponent::Tilt)),
        Some(DynamicValue::Scalar(-90.))
    );
}

struct StaticSource {
    target: FixtureId,
    family: AttributeValue,
}
impl DynamicTickSource for StaticSource {
    fn value(&self, target: FixtureId, key: &AttributeKey) -> Option<&AttributeValue> {
        (self.target == target && *key == ProgrammingOwner::Position.key()).then_some(&self.family)
    }
}
impl ScalarSourceResolver for StaticSource {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}

fn target_source() -> StaticSource {
    StaticSource {
        target: FixtureId::new(),
        family: AttributeValue::Position(Arc::new(PositionIntent::Target {
            reference: TargetReference::Origin,
            offset_metres: [
                ScalarIntent::Value(1.),
                ScalarIntent::Value(2.),
                ScalarIntent::Value(3.),
            ],
        })),
    }
}

#[test]
fn current_adoption_is_cached_per_address_and_preserves_original_size_baseline() {
    let source = target_source();
    let calls = Cell::new(0);
    let adopt = |target, original: &AttributeValue, _: &DynamicValueAddress| {
        assert_eq!(target, source.target);
        assert_eq!(original, &source.family);
        calls.set(calls.get() + 1);
        Ok(position(450., -30.))
    };
    let sources = CapturedProgrammingSources::new(&source, &adopt, None);
    for _ in 0..3 {
        assert_eq!(
            sources.current(source.target, &axis(ProgrammingComponent::Pan)),
            Some(DynamicValue::Scalar(450.))
        );
    }
    assert_eq!(
        calls.get(),
        1,
        "repeated Current tokens share one captured adoption"
    );
    assert_eq!(
        sources.current_family_base(source.target, &axis(ProgrammingComponent::Pan)),
        Some(source.family.clone())
    );
    assert_eq!(
        sources.current(FixtureId::new(), &axis(ProgrammingComponent::Pan)),
        None
    );
    sources.check().unwrap();
}

#[test]
fn unresolved_or_invalid_adoption_is_not_reported_as_an_absent_current() {
    let source = target_source();
    let calls = Cell::new(0);
    let adopt = |_, _: &AttributeValue, _: &DynamicValueAddress| {
        calls.set(calls.get() + 1);
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    };
    let sources = CapturedProgrammingSources::new(&source, &adopt, None);
    for _ in 0..2 {
        assert_eq!(
            sources.try_current(source.target, &axis(ProgrammingComponent::Tilt)),
            Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles
            )),
            "numeric Current must distinguish conversion from ordinary absence"
        );
        assert_eq!(
            sources.current(source.target, &axis(ProgrammingComponent::Tilt)),
            None
        );
        sources.check().unwrap();
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(
        sources.requirements(),
        vec![CurrentResolutionRequirement {
            target: source.target,
            address: axis(ProgrammingComponent::Tilt),
            requirement: TransitionRequirement::LiveJointAngles,
        }]
    );
    assert_eq!(
        sources.current(FixtureId::new(), &axis(ProgrammingComponent::Tilt)),
        None
    );
    assert_eq!(
        sources.try_current(FixtureId::new(), &axis(ProgrammingComponent::Tilt)),
        Ok(None)
    );
    assert_eq!(
        sources.requirements().len(),
        1,
        "ordinary absence does not create a notice"
    );
    let wrong = |_, value: &AttributeValue, _: &DynamicValueAddress| Ok(value.clone());
    let invalid = CapturedProgrammingSources::new(&source, &wrong, None);
    assert_eq!(
        invalid.current(source.target, &axis(ProgrammingComponent::Pan)),
        None
    );
    assert!(matches!(invalid.check(), Err(TransitionError::Invalid(_))));
}

#[test]
fn whole_current_size_rejects_a_scalar_at_a_position_address() {
    let source = StaticSource {
        target: FixtureId::new(),
        family: AttributeValue::Normalized(0.5),
    };
    let sources = CapturedProgrammingSources::new(&source, &no_adoption, None);
    assert_eq!(
        sources.current_family_base(source.target, &axis(ProgrammingComponent::Pan)),
        None
    );
    assert!(matches!(sources.check(), Err(TransitionError::Invalid(_))));
    assert!(
        sources.requirements().is_empty(),
        "invalid data is not a capability notice"
    );
    let component = CapturedProgrammingSources::new(&source, &no_adoption, None);
    assert_eq!(
        component.current(source.target, &axis(ProgrammingComponent::Tilt)),
        None
    );
    assert!(matches!(
        component.check(),
        Err(TransitionError::Invalid(_))
    ));
    assert!(component.requirements().is_empty());
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
                            source: DynamicValueSource::Value {
                                value: DynamicValue::Scalar(90.),
                            },
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

#[test]
fn shared_captured_sampler_emits_one_complete_angle_owner_and_live_static_partner_while_paused() {
    complete_pair_with_live_partner(false, false);
}

#[test]
fn first_sample_materializes_semantic_preset_before_composing_complete_angles() {
    complete_pair_with_live_partner(true, false);
}

#[test]
fn typed_preset_failure_rolls_back_and_retries_without_making_legacy_sampling_fallible() {
    complete_pair_with_live_partner(true, true);
}

fn complete_pair_with_live_partner(use_preset: bool, check_failed_preparation: bool) {
    let started = chrono::DateTime::from_timestamp_millis(1_000_000).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let fixture = FixtureId::new();
    programmers.start(session);
    programmers.set(
        session,
        fixture,
        ProgrammingOwner::Position.key(),
        position(0., 15.),
    );
    let mut definition = pan_definition();
    if use_preset {
        let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
            unreachable!()
        };
        let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
            unreachable!()
        };
        for point in &mut config.points {
            point.source = DynamicValueSource::Preset {
                preset_id: "3.1".into(),
                address: body.address.clone(),
                last_valid_by_target: vec![],
                retained: Some(Arc::new(DynamicPresetTemplate {
                    universal: Some(position(90., 0.)),
                    groups: if check_failed_preparation {
                        vec![DynamicPresetGroupTemplate {
                            group_id: "front".into(),
                            value: position(90., 0.),
                        }]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                })),
            };
        }
    }
    let pan_lane = definition.lanes[0].id;
    let authored_link = Uuid::new_v4();
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
            fixture_id: fixture,
            attribute: ProgrammingOwner::Position.key(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: authored_link,
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
                timing: Default::default(),
            },
        }],
        None
    ));
    let speed_groups = Mutex::new(std::array::from_fn(|_| {
        light_control::speed::SpeedGroupController::new(120., Default::default()).unwrap()
    }));
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    runtime
        .install_definitions(engine.snapshot().dynamics.iter().cloned())
        .unwrap();
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let mut composition = RetainedFamilyCompositionScratch::default();
    let mut origins = DynamicSourceOrigins::default();
    let mut retained_origin = None;
    for (index, expected_tilt) in [15., -45.].into_iter().enumerate() {
        if index > 0 {
            clock.advance_millis(25);
            programmers.set(
                session,
                fixture,
                ProgrammingOwner::Position.key(),
                position(0., expected_tilt),
            );
            engine
                .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
                .unwrap();
        }
        let frame = engine.prepare_output_frame(Default::default());
        let snapshot = frame.snapshot();
        let addresser = frame.frame_addresser();
        let speeds = capture_dynamic_speed_transports(
            &speed_groups,
            frame.sampled_at().timestamp_millis() as u64,
        );
        let inputs = CapturedDynamicInputs {
            now: frame.sampled_at(),
            speed_transports: &speeds,
            rate: 40,
            snapshot: &snapshot,
            programmer_values: frame.dynamic_programmer_values(),
            programmer_rows: Some(frame.dynamic_programmer_rows()),
            cue_values: frame.cue_dynamic_values(),
            dynamic_playbacks: frame.dynamic_playbacks(),
            playback_paused: frame.playback_dynamics_paused(),
            addresser: &addresser,
            extra_programmer_values: &[],
            programmer_reconciliation_cache: None,
            force_source_reconciliation: false,
        };
        let mut static_frame = engine.prepare_static_family_frame(&frame, &[]);
        let sources = PreparedFamilySources(&static_frame);
        let typed = CapturedProgrammingSources::new(&sources, &no_adoption, None)
            .with_source_transaction(&mut origins);
        if check_failed_preparation && index == 0 {
            // A present Group with an invalid nested dependency is a preparation error.
            // Its absence on the good capture is passive and keeps the universal template.
            let mut bad_snapshot = (*snapshot).clone();
            bad_snapshot.groups = vec![light_programmer::GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                source: Some(light_programmer::GroupFixtureSource::References {
                    references: vec![light_programmer::GroupReference {
                        group_id: "missing-nested-group".into(),
                        rule: light_programmer::SelectionRule::All,
                    }],
                }),
                ..Default::default()
            }]
            .into();
            let bad_snapshot = Arc::new(bad_snapshot);
            let bad_inputs = CapturedDynamicInputs {
                snapshot: &bad_snapshot,
                ..inputs
            };
            let mut legacy = runtime.fork_for_cold_install();
            let _ = sample_captured_dynamic_inputs(&mut legacy, &bad_inputs, &sources);
            assert!(legacy.has_pending_preset_sources());

            let before = runtime.snapshot();
            let mut transaction = DynamicOutputFrameScratch::default();
            let failed = runtime.with_output_frame_transaction(&mut transaction, |runtime| {
                sample_captured_programming_inputs(runtime, &bad_inputs, &typed)
            });
            let error = failed
                .err()
                .expect("typed preparation must reject the invalid Group");
            assert!(error.to_string().contains("missing-nested-group"));
            assert_eq!(
                runtime.snapshot(),
                before,
                "failed reconciliation is rolled back"
            );
            assert!(runtime.preset_source_instances().is_empty());
            assert_eq!(runtime.committed_sample_boundary(), None);
        }
        // Neither the lazy Current query nor the sampler may see this later write.
        programmers.set(
            session,
            fixture,
            ProgrammingOwner::Position.key(),
            position(180., 60.),
        );
        let sampled = sample_captured_programming_inputs(&mut runtime, &inputs, &typed).unwrap();
        if use_preset {
            assert!(!runtime.has_pending_preset_sources());
            let manifests = runtime.preset_source_instances();
            assert_eq!(manifests.len(), 1);
            assert!(!manifests[0].last_valid.is_empty());
        }
        assert_eq!(sampled.samples.len(), 2);
        let mut authored_origins = Vec::new();
        sampled
            .samples
            .iter()
            .find(|sample| sample.lane_id == pan_lane)
            .unwrap()
            .expression
            .visit_source_occurrences(&mut |id| authored_origins.push(id))
            .unwrap();
        assert_eq!(authored_origins.len(), 1);
        let origin = authored_origins[0];
        assert_eq!(*retained_origin.get_or_insert(origin), origin);
        let current_partner = sampled
            .samples
            .iter()
            .find(|sample| sample.expression.angle_current_address().is_some())
            .unwrap();
        current_partner
            .expression
            .visit_source_occurrences(&mut |_| {
                panic!("a static Current partner must not acquire the animated Pan's authorship");
            })
            .unwrap();
        assert!(
            sampled
                .samples
                .iter()
                .any(|sample| sample.expression.angle_current_address().is_some())
        );
        let prepared =
            prepare_captured_programming_samples(&sampled, &typed, &mut preparation).unwrap();
        assert!(prepared.legacy.is_empty());
        assert_eq!(prepared.families.len(), 1);
        let group = &prepared.families[0];
        assert_eq!(group.target, fixture);
        assert_eq!(group.owner, ProgrammingOwner::Position);
        assert_eq!(
            group.samples.len(),
            1,
            "the complete Angle pair competes once"
        );
        let result = compose_retained_dynamic_family_traced(
            group.owner,
            sources.value(fixture, &group.owner.key()).unwrap(),
            &group.samples,
            &FamilyCompositionContext::default(),
            &NoFrameConversion,
            &mut composition,
        )
        .unwrap();
        assert_eq!(result, position(90., expected_tilt));
        assert!(typed.requirements().is_empty());
        assert_eq!(sampled.after_runtime.global_paused, index > 0);
        let dependency = typed
            .current_family_occurrence(fixture, &axis(ProgrammingComponent::Tilt))
            .unwrap();
        assert_ne!(
            dependency, origin,
            "static Current never inherits the animated Pan's source"
        );
        let trace = composition.family_trace();
        let root = trace.root().unwrap();
        let pan = trace
            .sources_for_component(root, ProgrammingComponent::Pan)
            .unwrap();
        let tilt = trace
            .sources_for_component(root, ProgrammingComponent::Tilt)
            .unwrap();
        assert!(
            pan.iter().any(|source| source.occurrence == Some(origin)
                && source.role == FamilyTraceRole::Authored)
        );
        assert!(
            tilt.iter()
                .any(|source| source.occurrence == Some(dependency)
                    && source.role == FamilyTraceRole::CalculationDependency)
        );
        assert!(
            !tilt.iter().any(|source| source.occurrence == Some(origin)),
            "the pair stays atomic without inventing Tilt authorship"
        );
        typed.finish_source_bindings().unwrap();
        drop(typed);
        static_frame
            .project_family(
                fixture,
                ProgrammingOwner::Position,
                result.clone(),
                light_engine::FamilyProjectionMetadata {
                    changed_at: Some(frame.sampled_at()),
                    evidence: light_engine::FamilyProjectionEvidence::Replace {
                        origin: None,
                        family_evidence: None,
                    },
                },
            )
            .unwrap();
        assert_eq!(
            static_frame.value(fixture, &ProgrammingOwner::Position.key()),
            Some(&position(0., expected_tilt)),
            "queued output must not become Current for another family"
        );
        let rendered = engine
            .render_static_family_frame(&frame, static_frame)
            .unwrap();
        assert_eq!(
            rendered
                .resolved_values
                .value(fixture, &ProgrammingOwner::Position.key()),
            Some(&result),
            "an older running or paused Dynamic still controls its complete pair after a newer static edit"
        );
        assert!(matches!(
            origins.get(dependency).unwrap().origin,
            DynamicSourceOrigin::StaticBaseline { .. }
        ));
        let checkpoint =
            crate::runtime::dynamic_source_origins::DynamicRuntimeSourceCheckpoint::capture(
                runtime.snapshot(),
                &origins,
            )
            .unwrap();
        let checkpoint = serde_json::from_value::<
            crate::runtime::dynamic_source_origins::DynamicRuntimeSourceCheckpoint,
        >(serde_json::to_value(checkpoint).unwrap())
        .unwrap();
        let (restored, restored_origins) = checkpoint.restore().unwrap();
        assert_eq!(restored, runtime.snapshot());
        assert_eq!(restored_origins.snapshot(), origins.snapshot());
    }
}
