//! TL-636 Resume operand bridge contracts through the captured batch, using genuine runtime
//! pause/hot-edit Resume occurrences. The test frame never fits physically; these checks
//! establish owner-local authority, original operands and lifecycle, not physical acceptance.
use super::*;

struct CapturedSources {
    values: HashMap<FixtureId, AttributeValue>,
    reads: Cell<usize>,
}
impl ScalarSourceResolver for CapturedSources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        panic!("typed sources only")
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
impl DynamicValueSourceResolver for CapturedSources {
    fn current(&self, fixture: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.reads.set(self.reads.get() + 1);
        let value = self.values.get(&fixture)?;
        match address.component {
            None => Some(DynamicValue::Family(value.clone())),
            Some(ProgrammingComponent::TargetX) => {
                let AttributeValue::Position(position) = value else {
                    return None;
                };
                let PositionIntent::Target { offset_metres, .. } = position.as_ref() else {
                    return None;
                };
                let ScalarIntent::Value(x) = offset_metres[0] else {
                    return None;
                };
                Some(DynamicValue::Scalar(x))
            }
            _ => None,
        }
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}

fn literal_definition(value: AttributeValue) -> DynamicDefinition {
    let point = |position| DynamicKeyframe {
        position,
        source: DynamicValueSource::Value {
            value: DynamicValue::Family(value.clone()),
        },
        interpolation: light_dynamics::ScalarInterpolation::Linear,
    };
    DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 1,
        revision: 1,
        name: "Captured Resume bridge".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::Targetless,
        lanes: vec![DynamicLane {
            id: Uuid::from_u128(2),
            body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                address: DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value)
                    .unwrap(),
                configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                    points: vec![point(0.), point(0.5)],
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
        speed: DynamicSpeed::SpeedGroup {
            group: SpeedGroup::A,
            beats_per_cycle: Rational {
                numerator: 2,
                denominator: 1,
            },
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::JoinSyncNow,
        activation_boundary: ActivationBoundary::Beat,
    }
}
fn set_literal_lane(lane: &mut DynamicLane, address: DynamicValueAddress, value: DynamicValue) {
    let DynamicLaneBody::Programming(body) = &mut lane.body else {
        panic!("typed lane")
    };
    body.address = address;
    let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
        panic!("keyframes")
    };
    for point in &mut configuration.points {
        point.source = DynamicValueSource::Value {
            value: value.clone(),
        };
    }
}
fn start_definition(definition: &DynamicDefinition, fixture: FixtureId) -> (DynamicRuntime, Uuid) {
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller: DynamicController {
                id: Uuid::new_v4(),
                source: DynamicControllerSource::Programmer {
                    programmer_id: Uuid::new_v4(),
                    instance_link: None,
                },
                priority: 10,
                activated_at_millis: 1,
                size: 0.5,
                speed_multiplier: 1.,
                phase_offset_degrees: 0.,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: vec![fixture],
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 0,
            activation_delay_millis: 0,
            activation_duration_millis: 1000,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    (runtime, instance)
}
/// Pause, hot-edit the authored lanes from Target to Angles, then unpause: the actual runtime
/// records one synchronized Resume occurrence. Nothing below rewrites its emitted graph.
fn interrupt(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    sources: &CapturedSources,
) -> (Vec<DynamicRuntimeSample>, PositionResumeScope, f32) {
    runtime
        .sample_programming(instance, 1100, 1000, 10, sources, sources)
        .unwrap();
    runtime.set_global_paused(false, 1100);
    let now = 1350;
    let samples = runtime
        .sample_programming(instance, now, 1000, 10, sources, sources)
        .unwrap();
    let snapshot = runtime.snapshot();
    let state = &snapshot.instances[0];
    let resume = state
        .synchronized_resume_transition
        .expect("actual runtime interruption");
    assert_eq!(state.id, instance);
    assert_eq!(state.controllers.len(), 1);
    let progress = (now - resume.started_at_millis) as f32 / resume.duration_millis as f32;
    (
        samples,
        PositionResumeScope {
            instance_id: instance,
            controller_id: state.controllers[0].id,
            occurrence_id: resume.occurrence_id,
        },
        progress,
    )
}
fn pause_and_edit(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    sources: &CapturedSources,
    definition: &mut DynamicDefinition,
    edit: impl FnOnce(&mut DynamicDefinition),
) {
    runtime
        .sample_programming(instance, 1100, 1000, 10, sources, sources)
        .unwrap();
    runtime.set_global_paused(true, 1100);
    runtime
        .sample_programming(instance, 1100, 1000, 10, sources, sources)
        .unwrap();
    definition.revision += 1;
    edit(definition);
    runtime.install_definitions([definition.clone()]).unwrap();
}

struct HotResume {
    base: AttributeValue,
    fixture: FixtureId,
    sample: FamilyCompositionSample,
    scope: PositionResumeScope,
    progress: f32,
    runtime: DynamicRuntime,
    sources: CapturedSources,
}
/// Multi-source Resume: Target whole Size and TargetX remain an actual SourceCohort.
fn hot_resume() -> HotResume {
    let fixture = FixtureId::new();
    let base = target(0, 4., 8.);
    let mut definition = literal_definition(target(0, 20., 40.));
    set_literal_lane(
        &mut definition.lanes[0],
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Origin),
            },
            component: None,
        },
        DynamicValue::Family(target(0, 20., 40.)),
    );
    let mut offset = definition.lanes[0].clone();
    offset.id = Uuid::from_u128(1);
    set_literal_lane(
        &mut offset,
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Origin),
            },
            component: Some(ProgrammingComponent::TargetX),
        },
        DynamicValue::Scalar(6.),
    );
    definition.lanes.push(offset);
    let (mut runtime, instance) = start_definition(&definition, fixture);
    let sources = CapturedSources {
        values: HashMap::from([(fixture, base.clone())]),
        reads: Cell::new(0),
    };
    pause_and_edit(
        &mut runtime,
        instance,
        &sources,
        &mut definition,
        |edited| {
            for (index, component) in [ProgrammingComponent::Pan, ProgrammingComponent::Tilt]
                .into_iter()
                .enumerate()
            {
                set_literal_lane(
                    &mut edited.lanes[index],
                    DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::Angles,
                        component: Some(component),
                    },
                    DynamicValue::Scalar([50., 60.][index]),
                );
            }
        },
    );
    let (samples, scope, progress) = interrupt(&mut runtime, instance, &sources);
    assert_eq!(progress, 0.25);
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&samples, &sources, None, &mut preparation).unwrap();
    assert!(prepared.requirements.is_empty());
    assert_eq!(prepared.families.len(), 1);
    assert_eq!(prepared.families[0].samples.len(), 1);
    let sample = prepared.families[0].samples[0].clone();
    let FamilyCompositionSample::CoupledExpression { expression, .. } = &sample else {
        panic!("actual Position forest")
    };
    assert!(
        expression
            .base_endpoints()
            .iter()
            .any(|endpoint| endpoint.cohort_sources().is_some()),
        "original Target whole Size and TargetX remain an actual SourceCohort"
    );
    HotResume {
        base,
        fixture,
        sample,
        scope,
        progress,
        runtime,
        sources,
    }
}
fn fixed(value: AttributeValue, priority: i16, order: usize, mix: f32) -> FamilyCompositionSample {
    ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Position, None, value)
        .unwrap()
        .compile(
            None,
            &FamilyEditContext::default(),
            FamilySampleRank {
                priority,
                changed_at_millis: 100 + order as u64,
                changed_at_submillis_nanos: 0,
                stable_order: order as u128,
                identity: FamilySampleIdentity::Fixed {
                    source: FamilyFixedSampleSource::Programmer,
                    row_index: order,
                },
            },
            mix,
        )
        .unwrap()
        .into()
}
fn owner(fixture: FixtureId, samples: Vec<FamilyCompositionSample>) -> Sampled {
    Sampled {
        logical: fixture,
        samples,
        current_reads: Cell::new(0),
    }
}
fn hot_owner(hot: &HotResume) -> Sampled {
    owner(
        hot.fixture,
        vec![
            fixed(target(0, 2., 4.), 5, 0, 1.),
            hot.sample.clone(),
            fixed(angles(100., 200.), 20, 2, 0.5),
        ],
    )
}
fn resume_ready(progress: PositionResumeOperandProgress) -> AttributeValue {
    match progress {
        PositionResumeOperandProgress::OperandReady(value) => value,
        PositionResumeOperandProgress::NeedsMaterialization(request) => {
            panic!("unexpected operand dependency {}", request.request_id)
        }
        PositionResumeOperandProgress::Inactive => panic!("original Resume must remain active"),
    }
}
fn angle_value(value: &AttributeValue) -> AttributeValue {
    let AttributeValue::Position(position) = value else {
        panic!("Position")
    };
    match position.as_ref() {
        PositionIntent::Angles { .. } => value.clone(),
        PositionIntent::Target { offset_metres, .. } => {
            let [ScalarIntent::Value(x), ScalarIntent::Value(y), _] = offset_metres else {
                panic!("literal offsets")
            };
            angles(*x, *y)
        }
    }
}
fn angle_adoption(
    from: &AttributeValue,
    _: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    Ok(angle_value(from))
}
/// Issue the owner-local locator from this evaluation's own genuine outstanding request.
fn issue(
    batch: &mut dyn HybridPositionBatchComposer<()>,
    program: &HybridCapturedPositionProgram,
    frame: &Frame,
    adoption: &FamilyAdoptionResolver<'_>,
) -> (
    HybridPositionEvaluation,
    PositionCompositionRequest,
    PositionResumeOperandLocator,
) {
    let mut parent = batch
        .begin_branch(
            program,
            &program.registry().branch(),
            program.target(),
            adoption,
        )
        .unwrap();
    let request = pending(batch.advance(&mut parent, frame, adoption).unwrap());
    let locator = parent
        .pending_resume_operand_locator(request.request_id)
        .unwrap()
        .expect("original pending Resume route");
    (parent, request, locator)
}

#[test]
fn resume_operand_bridge_replays_actual_multi_source_original_operands() {
    let hot = hot_resume();
    let before = hot.runtime.snapshot();
    let reads = hot.sources.reads.get();
    with_batch(&hot_owner(&hot), |batch, program, _, _| {
        let frame = Frame::default();
        let branch = program.registry().branch();
        let (mut parent, request, locator) = issue(batch, program, &frame, &no_adoption);
        assert_eq!(locator.capture_id(), program.registry().capture_id());
        assert_eq!(locator.scope(), hot.scope);
        assert_eq!(
            program
                .registry()
                .resume_scope(locator.operation_node())
                .unwrap(),
            Some(hot.scope)
        );
        let calls = frame.0.get();
        let destination = FixtureId::new();
        for (endpoint, expected) in [
            (PositionResumeEndpoint::Outgoing, target(0, 12., 24.)),
            (PositionResumeEndpoint::Incoming, angles(50., 60.)),
        ] {
            assert!(
                branch.at_resume_operand(hot.scope, endpoint).is_err(),
                "legacy single-source guard stays in place"
            );
            let mut operand = batch
                .begin_resume_branch(
                    program,
                    &branch,
                    &locator,
                    endpoint,
                    destination,
                    &no_adoption,
                )
                .unwrap();
            assert_eq!(operand.frame_token(), program.frame_token());
            assert_eq!(operand.target(), hot.fixture);
            assert_eq!(operand.destination(), destination);
            operand.registry().validate_branch(&branch).unwrap();
            for _ in 0..2 {
                assert_eq!(
                    resume_ready(
                        batch
                            .advance_resume(&mut operand, &frame, &no_adoption)
                            .unwrap()
                    ),
                    expected
                );
                assert!(operand.pending_request().is_none());
            }
            assert!(
                operand
                    .pending_resume_operand_locator(request.request_id)
                    .is_err(),
                "a ready operand holds no request authority"
            );
            batch.recycle_resume(operand).unwrap();
        }
        assert_eq!(
            frame.0.get(),
            calls,
            "original operands stop before the Resume arithmetic"
        );
        assert_eq!(
            pending(batch.advance(&mut parent, &frame, &no_adoption).unwrap()).request_id,
            request.request_id,
            "speculative operands do not consume the parent cut"
        );
        let p = hot.progress;
        let fitted = angles(12. + p * (50. - 12.), 24. + p * (60. - 24.));
        batch
            .resume(&mut parent, request.request_id, fitted, None)
            .unwrap();
        let PositionCompositionProgress::Complete(value) =
            batch.advance(&mut parent, &frame, &no_adoption).unwrap()
        else {
            panic!("the original suffix completes once after the fitted Resume")
        };
        assert_eq!(
            value,
            angles(21.5 + 0.5 * (100. - 21.5), 33. + 0.5 * (200. - 33.))
        );
        batch.recycle(parent).unwrap();
    });
    assert_eq!(
        hot.sources.reads.get(),
        reads,
        "no producer Current resampling"
    );
    assert_eq!(
        hot.runtime.snapshot(),
        before,
        "operand replay cannot advance producer clocks or history"
    );
}

#[test]
fn resume_locator_refuses_foreign_request_capture_or_failed_evaluation() {
    let hot = hot_resume();
    let sampled = hot_owner(&hot);
    with_batch(&sampled, |batch, program, engine, typed| {
        let frame = Frame::default();
        let destination = FixtureId::new();
        let branch = program.registry().branch();
        let outgoing = PositionResumeEndpoint::Outgoing;
        let (mut parent, request, locator) = issue(batch, program, &frame, &no_adoption);
        assert!(
            parent
                .pending_resume_operand_locator(Uuid::new_v4())
                .is_err(),
            "a fabricated request id has no authority"
        );
        // An equal capture UUID on another registry is not this owner's authority.
        let foreign = CapturedPositionProgram::new(
            program.registry().capture_id(),
            &hot.base,
            &sampled.samples,
        )
        .unwrap();
        let mut foreign_program = program.clone();
        foreign_program.registry = foreign.clone();
        for (owner, candidate) in [
            (program, &foreign.branch()),
            (&foreign_program, &branch),
            (&foreign_program, &foreign.branch()),
        ] {
            assert!(
                batch
                    .begin_resume_branch(
                        owner,
                        candidate,
                        &locator,
                        outgoing,
                        destination,
                        &no_adoption
                    )
                    .is_err()
            );
        }
        let mut wrong_target = program.clone();
        wrong_target.target = FixtureId::new();
        let other_capture = engine.prepare_output_frame(Default::default());
        let mut wrong_frame = program.clone();
        wrong_frame.token = other_capture.frame_token();
        for owner in [&wrong_target, &wrong_frame] {
            assert!(
                batch
                    .begin_resume_branch(
                        owner,
                        &branch,
                        &locator,
                        outgoing,
                        destination,
                        &no_adoption
                    )
                    .is_err()
            );
        }
        let mut operand = batch
            .begin_resume_branch(
                program,
                &branch,
                &locator,
                outgoing,
                destination,
                &no_adoption,
            )
            .unwrap();
        // Another capture of the same genuine occurrence is another owner, never correlated by
        // equal scope/value: its locator and request cannot drive this owner, nor ours its.
        with_batch(&sampled, |foreign_batch, other_program, _, _| {
            let (other, other_request, other_locator) =
                issue(foreign_batch, other_program, &frame, &no_adoption);
            assert_eq!(other_locator.scope(), locator.scope());
            assert!(
                batch
                    .begin_resume_branch(
                        program,
                        &branch,
                        &other_locator,
                        outgoing,
                        destination,
                        &no_adoption
                    )
                    .is_err()
            );
            assert!(
                parent
                    .pending_resume_operand_locator(other_request.request_id)
                    .is_err()
            );
            assert!(
                foreign_batch
                    .advance_resume(&mut operand, &frame, &no_adoption)
                    .is_err()
            );
            assert!(
                foreign_batch
                    .resume_resume(&mut operand, request.request_id, angles(1., 2.), None)
                    .is_err()
            );
            foreign_batch.recycle(other).unwrap();
        });
        assert_eq!(
            resume_ready(
                batch
                    .advance_resume(&mut operand, &frame, &no_adoption)
                    .unwrap()
            ),
            target(0, 12., 24.),
            "foreign loans neither execute nor poison the retained operand"
        );
        assert!(
            batch
                .resume_resume(&mut operand, request.request_id, angles(1., 2.), None)
                .is_err(),
            "the parent's request is not the operand's own response"
        );
        // Source failure is terminal for the operand; its workspace still recycles.
        *typed.failure.borrow_mut() =
            Some(IntentError("injected captured source failure".into()).into());
        assert!(
            batch
                .advance_resume(&mut operand, &frame, &no_adoption)
                .is_err()
        );
        *typed.failure.borrow_mut() = None;
        assert!(
            batch
                .advance_resume(&mut operand, &frame, &no_adoption)
                .is_err()
        );
        assert!(operand.pending_request().is_none());
        assert!(
            operand
                .pending_resume_operand_locator(request.request_id)
                .is_err()
        );
        batch.recycle_resume(operand).unwrap();
        // A failed Full parent keeps its domain cut but loses all nested locator authority.
        *typed.failure.borrow_mut() =
            Some(IntentError("injected captured source failure".into()).into());
        assert!(batch.advance(&mut parent, &frame, &no_adoption).is_err());
        *typed.failure.borrow_mut() = None;
        assert!(
            parent
                .pending_resume_operand_locator(request.request_id)
                .is_err(),
            "failed Full evaluation cannot issue Resume authority"
        );
        batch.recycle(parent).unwrap();
    });
}

#[test]
fn removed_enclosing_goal_is_inactive_not_full() {
    let fixture = FixtureId::new();
    let base = target(0, 4., 8.);
    let mut definition = literal_definition(target(0, 20., 40.));
    let (mut runtime, instance) = start_definition(&definition, fixture);
    let controller = runtime.snapshot().instances[0].controllers[0].id;
    runtime
        .update_controller(controller, Some(1.), None, None)
        .unwrap();
    let sources = CapturedSources {
        values: HashMap::from([(fixture, base.clone())]),
        reads: Cell::new(0),
    };
    pause_and_edit(
        &mut runtime,
        instance,
        &sources,
        &mut definition,
        |edited| {
            set_literal_lane(
                &mut edited.lanes[0],
                DynamicValueAddress::whole_family(ProgrammingOwner::Position, &angles(50., 60.))
                    .unwrap(),
                DynamicValue::Family(angles(50., 60.)),
            );
        },
    );
    let (samples, scope, _) = interrupt(&mut runtime, instance, &sources);
    assert_eq!(samples.len(), 1);
    let sample = &samples[0];
    let snapshot = runtime.snapshot();
    // Explicit compositor-unit assembly, as in the domain TL-636 test: this Required wrapper is
    // NOT sampler-produced and has no producer witness. It only supplies an original enclosing
    // goal around the unchanged genuine runtime Resume. No cross-owner correspondence is claimed.
    let wrapped = Arc::new(DynamicSampleExpression::Transition {
        from: Some(Arc::new(sample.expression.clone())),
        // A Point Target incoming endpoint keeps the enclosing Required a genuine pending
        // frame operation in this physical-free test frame, so it issues real authority.
        to: Some(Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(
                DynamicValueAddress::whole_family(
                    ProgrammingOwner::Position,
                    &target(9, 80., 100.),
                )
                .unwrap(),
            ),
            value: DynamicValue::Family(target(9, 80., 100.)),
            occurrence: None,
            dependency_occurrence: None,
        })),
        progress: 0.5,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::MaterializedEndpoints,
        },
    });
    assert!(wrapped.operation_provenance().unwrap().handles().is_empty());
    let rank = FamilySampleRank {
        priority: sample.priority,
        changed_at_millis: sample.activated_at_millis,
        changed_at_submillis_nanos: 0,
        stable_order: 0,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: sample.instance_id,
            controller_id: sample.controller_id,
            lane_id: sample.lane_id,
        },
    };
    let whole = FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                wrapped,
                ProgrammingOwner::Position,
                None,
                None,
            )
            .unwrap(),
        ),
        rank,
        activation_mix: 1.,
    };
    let sampled = owner(
        fixture,
        vec![
            fixed(angles(1., 2.), 5, 0, 1.),
            whole,
            fixed(angles(100., 200.), 20, 1, 0.5),
        ],
    );
    with_batch(&sampled, |batch, program, _, _| {
        let frame = Frame::default();
        let branch = program.registry().branch();
        let destination = FixtureId::new();
        let (mut parent, first, full) = issue(batch, program, &frame, &no_adoption);
        assert_eq!(full.scope(), scope);
        batch
            .resume(&mut parent, first.request_id, angles(27.5, 45.), None)
            .unwrap();
        let required = pending(batch.advance(&mut parent, &frame, &no_adoption).unwrap());
        let outer = parent
            .pending_graph_operation_locator(required.request_id)
            .unwrap()
            .expect("original enclosing Required goal");
        assert_eq!(outer.kind(), PositionGraphOperationKind::Required);
        let mut graph = batch
            .begin_graph_branch(
                program,
                &branch,
                &outer,
                PositionGraphOperationOperand::RequiredOutgoing,
                destination,
                &no_adoption,
            )
            .unwrap();
        let nested_request = graph_pending(
            batch
                .advance_graph(&mut graph, &frame, &no_adoption)
                .unwrap(),
        );
        assert!(
            parent
                .pending_resume_operand_locator(nested_request.request_id)
                .is_err(),
            "the Graph driver's request is not the Full parent's authority"
        );
        let nested = graph
            .pending_resume_operand_locator(nested_request.request_id)
            .unwrap()
            .expect("actual Resume within the RequiredOutgoing goal");
        assert_eq!(nested.scope(), scope);
        assert!(
            nested != full,
            "Graph and Full goals are distinct lexical uses"
        );
        let mut original = batch
            .begin_resume_branch(
                program,
                &branch,
                &nested,
                PositionResumeEndpoint::Outgoing,
                destination,
                &no_adoption,
            )
            .unwrap();
        assert_eq!(
            resume_ready(
                batch
                    .advance_resume(&mut original, &frame, &no_adoption)
                    .unwrap()
            ),
            target(0, 20., 40.)
        );
        batch.recycle_resume(original).unwrap();

        let mut removed_outer = branch.clone();
        removed_outer
            .replace_source(
                nested.operation_node().source_index(),
                Some(nested.operation_node()),
            )
            .unwrap();
        assert!(
            matches!(
                removed_outer.resume_scope_membership(scope).unwrap(),
                PositionResumeScopeMembership::Active(nodes) if !nodes.is_empty()
            ),
            "the genuine Resume still exists; only its enclosing goal was removed"
        );
        let calls = frame.0.get();
        let mut stale = batch
            .begin_resume_branch(
                program,
                &removed_outer,
                &nested,
                PositionResumeEndpoint::Incoming,
                destination,
                &no_adoption,
            )
            .unwrap();
        for _ in 0..2 {
            assert!(
                matches!(
                    batch
                        .advance_resume(&mut stale, &frame, &no_adoption)
                        .unwrap(),
                    PositionResumeOperandProgress::Inactive
                ),
                "a nested locator never falls back to a Full-parent Resume"
            );
            assert!(stale.pending_request().is_none());
        }
        assert!(
            stale
                .pending_resume_operand_locator(nested_request.request_id)
                .is_err()
        );
        assert_eq!(frame.0.get(), calls, "Inactive executes no frame operation");
        batch.recycle_resume(stale).unwrap();
        assert_eq!(
            graph_pending(
                batch
                    .advance_graph(&mut graph, &frame, &no_adoption)
                    .unwrap()
            )
            .request_id,
            nested_request.request_id,
            "the owning Graph driver keeps its original pending cut"
        );
        batch.recycle_graph(graph).unwrap();
        assert_eq!(
            parent.pending_request().unwrap().request_id,
            required.request_id
        );
        batch.recycle(parent).unwrap();
    });
    assert_eq!(runtime.snapshot(), snapshot);
    assert_eq!(
        sources.reads.get(),
        0,
        "Size1 literal runtime reads no Current"
    );
}

#[test]
fn terminal_unwind_discards_resume_operand() {
    let hot = hot_resume();
    let reads = hot.sources.reads.get();
    let sampled = owner(
        hot.fixture,
        vec![
            fixed(angles(9., 11.), 9, 0, 0.5),
            hot.sample.clone(),
            fixed(angles(100., 200.), 20, 1, 0.5),
        ],
    );
    with_batch(&sampled, |batch, program, _, _| {
        let frame = Frame::default();
        let branch = program.registry().branch();
        let (parent, request, locator) = issue(batch, program, &frame, &angle_adoption);
        assert_eq!(locator.scope(), hot.scope);
        let adoption_calls = Cell::new(0);
        let panic_adoption = |_: &AttributeValue,
                              _: &DynamicValueAddress|
         -> Result<AttributeValue, TransitionError> {
            adoption_calls.set(adoption_calls.get() + 1);
            panic!("intentional Resume prefix adoption unwind")
        };
        let incoming = PositionResumeEndpoint::Incoming;
        let mut operand = batch
            .begin_resume_branch(
                program,
                &branch,
                &locator,
                incoming,
                program.target(),
                &panic_adoption,
            )
            .unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                batch.advance_resume(&mut operand, &frame, &panic_adoption)
            }))
            .is_err()
        );
        assert_eq!(
            adoption_calls.get(),
            1,
            "the actual prefix callback unwound once"
        );
        assert!(
            batch
                .advance_resume(&mut operand, &frame, &angle_adoption)
                .is_err(),
            "an unwound Resume operand is terminal"
        );
        assert!(
            batch
                .resume_resume(&mut operand, request.request_id, angles(1., 2.), None)
                .is_err()
        );
        assert!(operand.pending_request().is_none());
        assert!(
            operand
                .pending_resume_operand_locator(request.request_id)
                .is_err()
        );
        assert!(operand.pending_mask_locator(request.request_id).is_err());
        assert_eq!(
            adoption_calls.get(),
            1,
            "a failed replay cannot call adoption again"
        );
        batch.recycle_resume(operand).unwrap();
        let mut fresh = batch
            .begin_resume_branch(
                program,
                &branch,
                &locator,
                incoming,
                program.target(),
                &angle_adoption,
            )
            .unwrap();
        assert_eq!(
            resume_ready(
                batch
                    .advance_resume(&mut fresh, &frame, &angle_adoption)
                    .unwrap()
            ),
            angles(50., 60.)
        );
        batch.recycle_resume(fresh).unwrap();
        assert_eq!(
            parent.pending_request().unwrap().request_id,
            request.request_id,
            "the unwound operand never consumed the original parent"
        );
        batch.recycle(parent).unwrap();
    });
    assert_eq!(hot.sources.reads.get(), reads);
}
