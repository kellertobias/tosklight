//! TL635: same-driver reports of reached operations, with genuine runtime emission authority.
//! These are domain replay tests. No physical fit or cross-owner acceptance is asserted here.
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

// The domain frame genuinely has no physical mount/joint or external Point data. Compatible
// operations must finish without calling this port; it never rejects a compatible calculation.
#[derive(Default)]
struct NoPhysicalContext {
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for NoPhysicalContext {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}

fn definition_template() -> DynamicDefinition {
    // Reuse authored configuration only. The new runtime creates fresh source witnesses;
    // no retained graph, operation annotation, emission or rank is rewritten.
    let original = sampled(1.);
    let mut definition = original
        .expression
        .operation_provenance()
        .unwrap()
        .handles()[0]
        .emission()
        .definition()
        .clone();
    definition.id = Uuid::new_v4();
    definition.lanes[0].id = Uuid::new_v4();
    definition
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
fn start_definition(
    definition: &DynamicDefinition,
    targets: &[FixtureId],
    duration: u64,
) -> (DynamicRuntime, Uuid) {
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
                ordered_targets: targets.to_vec(),
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 0,
            activation_delay_millis: 0,
            activation_duration_millis: duration,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    (runtime, instance)
}
fn sample_rank(sample: &DynamicRuntimeSample) -> FamilySampleRank {
    FamilySampleRank {
        priority: sample.priority,
        changed_at_millis: sample.activated_at_millis,
        changed_at_submillis_nanos: 0,
        stable_order: 0,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: sample.instance_id,
            controller_id: sample.controller_id,
            lane_id: sample.lane_id,
        },
    }
}
fn single_registry(
    sample: &DynamicRuntimeSample,
    base: &AttributeValue,
) -> CapturedPositionProgram {
    CapturedPositionProgram::new(
        Uuid::new_v4(),
        base,
        &[whole(
            Arc::new(sample.expression.clone()),
            sample_rank(sample),
            1.,
        )],
    )
    .unwrap()
}
fn completed(progress: PositionCompositionProgress) -> AttributeValue {
    let PositionCompositionProgress::Complete(value) = progress else {
        panic!("synchronous operation")
    };
    value
}
fn size_locator(report: &PositionGraphDiscoveryReport) -> PositionGraphOperationLocator {
    let candidates = report
        .reached
        .iter()
        .filter(|locator| locator.kind() == PositionGraphOperationKind::Size)
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 1, "one exact reached Size lexical use");
    candidates[0].clone()
}

#[test]
fn same_emission_size_reports_synchronous_and_pending_owners_without_resampling_current() {
    let targets = [FixtureId::new(), FixtureId::new()];
    let bases = [angles(4., 8.), target(0, 4., 8.)];
    let endpoint = angles(20., 40.);
    let mut definition = definition_template();
    set_literal_lane(
        &mut definition.lanes[0],
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &endpoint).unwrap(),
        DynamicValue::Family(endpoint.clone()),
    );
    let (mut runtime, instance) = start_definition(&definition, &targets, 0);
    let sources = CapturedSources {
        values: targets.into_iter().zip(bases.clone()).collect(),
        reads: Cell::new(0),
    };
    let samples = runtime
        .sample_programming(instance, 250, 1000, 10, &sources, &sources)
        .unwrap();
    assert_eq!(samples.len(), 2);
    assert_eq!(
        sources.reads.get(),
        2,
        "one real whole-family Size baseline per target"
    );
    let handles = targets.map(|target| {
        let sample = samples
            .iter()
            .find(|sample| sample.target == target)
            .unwrap();
        let provenance = sample.expression.operation_provenance().unwrap();
        assert!(provenance.is_complete());
        assert_eq!(provenance.handles().len(), 1);
        let handle = provenance.handles()[0].clone();
        assert_eq!(
            handle.site(),
            DynamicOperationSite::ControllerSize {
                role: DynamicControllerSizeRole::FamilyScale
            }
        );
        assert_eq!(handle.emission().instance_id(), instance);
        assert_eq!(handle.emission().targets(), targets);
        assert_eq!(handle.target(), target);
        handle
    });
    assert!(matches!(
        handles[0].correspondence(&handles[1]),
        DynamicOperationCorrespondence::Shared { historical: false }
    ));
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    for index in 0..2 {
        let sample = samples
            .iter()
            .find(|sample| sample.target == targets[index])
            .unwrap();
        let registry = single_registry(sample, &bases[index]);
        let mut driver = registry
            .branch()
            .begin_composition(&context, Default::default(), true)
            .unwrap();
        assert!(
            driver.graph_discovery_report().is_err(),
            "collection is opt in"
        );
        driver.enable_graph_discovery(16).unwrap();
        let progress = driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap();
        let report = driver.graph_discovery_report().unwrap();
        let locator = size_locator(&report);
        if index == 0 {
            assert_eq!(completed(progress), angles(12., 24.));
            assert!(report.complete);
            assert_eq!(
                frame.calls.get(),
                0,
                "ordinary compatible Size never requires physical context"
            );
            assert!(
                driver
                    .pending_graph_operation_locator(Uuid::new_v4())
                    .is_err()
            );
        } else {
            let request = needed(progress);
            assert!(!report.complete);
            assert_eq!(
                driver
                    .pending_graph_operation_locator(request.request_id)
                    .unwrap()
                    .unwrap(),
                locator
            );
            assert_eq!(
                needed(
                    driver
                        .advance(registry.capture_id(), &context, &frame)
                        .unwrap()
                )
                .request_id,
                request.request_id
            );
            assert_eq!(
                driver.graph_discovery_report().unwrap().reached,
                report.reached
            );
        }
        assert!(
            driver.enable_graph_discovery(16).is_err(),
            "cannot retrofit collection after evaluation"
        );
        assert_eq!(
            driver.graph_discovery_report().unwrap().reached,
            report.reached,
            "late enable is a rejected operation, not permission to clear existing authority"
        );
        for (operand, expected) in [
            (
                PositionGraphOperationOperand::SizeBaseline,
                bases[index].clone(),
            ),
            (PositionGraphOperationOperand::SizeValue, endpoint.clone()),
        ] {
            let mut replay = registry
                .branch()
                .begin_graph_operation_operand(
                    &locator,
                    operand,
                    &context,
                    Default::default(),
                    true,
                )
                .unwrap();
            replay.enable_graph_discovery(16).unwrap();
            assert_eq!(
                ready(
                    replay
                        .advance(registry.capture_id(), &context, &frame)
                        .unwrap()
                ),
                expected
            );
            assert!(replay.graph_discovery_report().unwrap().complete);
            assert_eq!(
                ready(
                    replay
                        .advance(registry.capture_id(), &context, &frame)
                        .unwrap()
                ),
                expected
            );
        }
        assert_eq!(
            sources.reads.get(),
            2,
            "reached locators and original operand replay never resample runtime Current"
        );
    }
}

#[test]
fn pending_required_prefix_does_not_report_its_later_size_and_retry_keeps_exact_driver() {
    let sample = sampled(1.5);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[whole(sample.expression, sample.rank, 1.)],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    parent.enable_graph_discovery(16).unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    let report = parent.graph_discovery_report().unwrap();
    assert!(!report.complete);
    assert_eq!(report.reached.len(), 1);
    assert_eq!(
        report.reached[0].kind(),
        PositionGraphOperationKind::Required
    );
    assert_eq!(
        parent
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .unwrap(),
        report.reached[0]
    );
    assert!(
        parent
            .resume_materialization(
                registry.capture_id(),
                request.request_id,
                AttributeValue::Normalized(0.5),
                None
            )
            .is_err()
    );
    assert!(parent.advance(Uuid::new_v4(), &context, &frame).is_err());
    assert_eq!(
        needed(
            parent
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        )
        .request_id,
        request.request_id
    );
    let repeated = parent.graph_discovery_report().unwrap();
    assert_eq!(repeated.reached, report.reached);
    assert_eq!(repeated.inspected_routes, report.inspected_routes);
    assert_eq!(frame.calls.get(), 1);
    parent
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    let size_request = needed(
        parent
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert_ne!(size_request.request_id, request.request_id);
    assert!(
        parent
            .pending_graph_operation_locator(request.request_id)
            .is_err()
    );
    let after = parent.graph_discovery_report().unwrap();
    assert_eq!(after.reached.len(), 2);
    let size = size_locator(&after);
    assert_eq!(
        parent
            .pending_graph_operation_locator(size_request.request_id)
            .unwrap()
            .unwrap(),
        size
    );
    parent
        .resume_materialization(
            registry.capture_id(),
            size_request.request_id,
            angles(28., 41.),
            None,
        )
        .unwrap();
    assert_eq!(
        completed(
            parent
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        angles(28., 41.)
    );
    assert!(parent.graph_discovery_report().unwrap().complete);
    assert_eq!(
        parent.graph_discovery_report().unwrap().reached,
        after.reached
    );
}

#[test]
fn synchronous_size_lexical_use_remains_exact_after_conditioning_and_foreign_inputs_reject() {
    let fixture = FixtureId::new();
    let base = angles(4., 8.);
    let endpoint = angles(20., 40.);
    let mut definition = definition_template();
    set_literal_lane(
        &mut definition.lanes[0],
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &endpoint).unwrap(),
        DynamicValue::Family(endpoint),
    );
    let (mut runtime, instance) = start_definition(&definition, &[fixture], 0);
    let sources = CapturedSources {
        values: HashMap::from([(fixture, base.clone())]),
        reads: Cell::new(0),
    };
    let sample = runtime
        .sample_programming(instance, 250, 1000, 10, &sources, &sources)
        .unwrap()
        .remove(0);
    let mut higher = sample_rank(&sample);
    higher.priority += 1;
    // A partial whole-expression consumer needs the lower source as its eligible
    // UnderlayFor. A trailing FixAT would instead consume the completed RootFinal.
    // This compositor-only consumer has no producer operation witness of its own.
    let originals = [
        whole(
            Arc::new(sample.expression.clone()),
            sample_rank(&sample),
            1.,
        ),
        whole(leaf(angles(100., 200.)), higher, 0.5),
    ];
    let capture = Uuid::new_v4();
    let registry = CapturedPositionProgram::new(capture, &base, &originals).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let mut driver = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    driver.enable_graph_discovery(16).unwrap();
    assert_eq!(
        completed(driver.advance(capture, &context, &frame).unwrap()),
        angles(56., 112.)
    );
    let underlay = size_locator(&driver.graph_discovery_report().unwrap());
    let mut without_mask = registry.branch();
    without_mask.replace_source(1, None).unwrap();
    let mut other = without_mask
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    other.enable_graph_discovery(16).unwrap();
    assert_eq!(
        completed(other.advance(capture, &context, &frame).unwrap()),
        angles(12., 24.)
    );
    let final_use = size_locator(&other.graph_discovery_report().unwrap());
    assert!(underlay.operation_node() == final_use.operation_node());
    assert_ne!(
        underlay, final_use,
        "RootFinal and UnderlayFor are different original uses"
    );
    let foreign = CapturedPositionProgram::new(capture, &base, &originals).unwrap();
    assert!(
        foreign
            .branch()
            .validate_graph_operation_operand(&underlay, PositionGraphOperationOperand::SizeValue)
            .is_err()
    );
    assert!(
        registry
            .branch()
            .validate_graph_operation_operand(
                &underlay,
                PositionGraphOperationOperand::RequiredIncoming
            )
            .is_err()
    );
    let mut wrong_use = without_mask
        .begin_graph_operation_operand(
            &underlay,
            PositionGraphOperationOperand::SizeValue,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    wrong_use.enable_graph_discovery(16).unwrap();
    assert!(matches!(
        wrong_use.advance(capture, &context, &frame).unwrap(),
        PositionGraphOperationOperandProgress::Inactive
    ));
    assert!(wrong_use.graph_discovery_report().unwrap().complete);
    let mut correct = without_mask
        .begin_graph_operation_operand(
            &final_use,
            PositionGraphOperationOperand::SizeValue,
            &context,
            wrong_use.into_scratch(),
            true,
        )
        .unwrap();
    assert!(correct.advance(Uuid::new_v4(), &context, &frame).is_err());
    assert_eq!(
        ready(correct.advance(capture, &context, &frame).unwrap()),
        angles(20., 40.)
    );
    assert_eq!(sources.reads.get(), 1);
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn discovery_limit_and_callback_unwind_are_terminal_but_late_enable_does_not_authorize_replay() {
    let sample = sampled(1.5);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[whole(sample.expression, sample.rank, 1.)],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    assert!(parent.enable_graph_discovery(0).is_err());
    parent.enable_graph_discovery(1).unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    parent
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    assert!(
        parent
            .advance(registry.capture_id(), &context, &frame)
            .is_err()
    );
    assert!(
        parent.graph_discovery_report().is_err(),
        "bounded refusal must not publish a partial successful catalogue"
    );
    assert!(
        parent
            .advance(registry.capture_id(), &context, &Frame::default())
            .is_err()
    );
    let mut panic_driver = registry
        .branch()
        .begin_composition(&context, parent.into_scratch(), true)
        .unwrap();
    panic_driver.enable_graph_discovery(16).unwrap();
    let panic_frame = Frame {
        panic: true,
        ..Default::default()
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| panic_driver.advance(
            registry.capture_id(),
            &context,
            &panic_frame
        )))
        .is_err()
    );
    assert!(panic_driver.graph_discovery_report().is_err());
    assert!(
        panic_driver
            .advance(registry.capture_id(), &context, &Frame::default())
            .is_err()
    );
    assert!(
        panic_driver
            .pending_graph_operation_locator(Uuid::new_v4())
            .is_err()
    );
}

#[test]
fn genuine_hot_edit_sourcecohort_reports_original_member_size_and_conditioned_removal_is_inactive()
{
    let fixture = FixtureId::new();
    let base = target(0, 4., 8.);
    let endpoint = target(0, 20., 40.);
    let mut definition = definition_template();
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 2,
            denominator: 1,
        },
    };
    // Stable authored lane order, established before the runtime creates any witness.
    // The whole source is the later original member, independent of random UUID order.
    definition.lanes[0].id = Uuid::from_u128(2);
    set_literal_lane(
        &mut definition.lanes[0],
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Origin),
            },
            component: None,
        },
        DynamicValue::Family(endpoint),
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
    let (mut runtime, instance) = start_definition(&definition, &[fixture], 1000);
    let sources = CapturedSources {
        values: HashMap::from([(fixture, base.clone())]),
        reads: Cell::new(0),
    };
    runtime
        .sample_programming(instance, 1100, 1000, 10, &sources, &sources)
        .unwrap();
    runtime.set_global_paused(true, 1100);
    runtime
        .sample_programming(instance, 1100, 1000, 10, &sources, &sources)
        .unwrap();
    definition.revision += 1;
    for (index, component) in [ProgrammingComponent::Pan, ProgrammingComponent::Tilt]
        .into_iter()
        .enumerate()
    {
        set_literal_lane(
            &mut definition.lanes[index],
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: Some(component),
            },
            DynamicValue::Scalar([50., 60.][index]),
        );
    }
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
        .sample_programming(instance, 1100, 1000, 10, &sources, &sources)
        .unwrap();
    runtime.set_global_paused(false, 1100);
    let samples = runtime
        .sample_programming(instance, 1350, 1000, 10, &sources, &sources)
        .unwrap();
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.instances.len(), 1);
    let state = &snapshot.instances[0];
    let resume = state
        .synchronized_resume_transition
        .expect("genuine runtime Resume");
    assert_eq!(state.controllers.len(), 1);
    let old_handles = samples
        .iter()
        .flat_map(|sample| {
            sample
                .expression
                .operation_provenance()
                .unwrap()
                .handles()
                .to_vec()
        })
        .filter(|handle| {
            handle.emission().definition().revision == 1
                && matches!(
                    handle.site(),
                    DynamicOperationSite::ControllerSize {
                        role: DynamicControllerSizeRole::FamilyScale
                    }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        old_handles.len(),
        1,
        "original held Size witness survives the actual hot edit"
    );
    assert_eq!(old_handles[0].emission().instance_id(), instance);
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&samples, &sources, None, &mut preparation).unwrap();
    assert!(prepared.requirements.is_empty());
    assert_eq!(prepared.families.len(), 1);
    let group = &prepared.families[0];
    assert_eq!(group.samples.len(), 1);
    let FamilyCompositionSample::CoupledExpression { expression, .. } = &group.samples[0] else {
        panic!("actual prepared Position forest")
    };
    assert!(
        expression
            .base_endpoints()
            .iter()
            .any(|endpoint| endpoint.cohort_sources().is_some()),
        "real retained whole Target plus TargetX creates SourceCohort"
    );
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &base, &group.samples).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    parent.enable_graph_discovery(32).unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    let locator = size_locator(&parent.graph_discovery_report().unwrap());
    assert!(
        parent
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .is_none(),
        "the outer pending Resume is not the completed member Size"
    );
    let rank = registry.node_source_rank(locator.operation_node()).unwrap();
    assert_eq!(
        rank.dynamic_identity().unwrap().lane_id,
        definition.lanes[0].id
    );
    let mut replay = registry
        .branch()
        .begin_graph_operation_operand(
            &locator,
            PositionGraphOperationOperand::SizeValue,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    replay.enable_graph_discovery(32).unwrap();
    assert_eq!(
        ready(
            replay
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        target(0, 20., 40.)
    );
    assert!(replay.graph_discovery_report().unwrap().complete);
    let mut incoming = registry.branch();
    incoming
        .choose_resume(
            PositionResumeScope {
                instance_id: instance,
                controller_id: state.controllers[0].id,
                occurrence_id: resume.occurrence_id,
            },
            PositionResumeEndpoint::Incoming,
        )
        .unwrap();
    let mut removed = incoming
        .begin_graph_operation_operand(
            &locator,
            PositionGraphOperationOperand::SizeValue,
            &context,
            replay.into_scratch(),
            true,
        )
        .unwrap();
    removed.enable_graph_discovery(32).unwrap();
    assert!(matches!(
        removed
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
        PositionGraphOperationOperandProgress::Inactive
    ));
    assert!(removed.graph_discovery_report().unwrap().complete);
    assert!(
        removed
            .graph_discovery_report()
            .unwrap()
            .reached
            .iter()
            .all(|candidate| candidate != &locator)
    );
    assert_eq!(
        needed(
            parent
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        )
        .request_id,
        request.request_id
    );
}

#[test]
fn only_bound_prestart_driver_can_enable_collection_and_reenable_preserves_its_state() {
    let sample = sampled(1.);
    let base = angles(0., 0.);
    let originals = [whole(sample.expression, sample.rank, 1.)];
    let capture = Uuid::new_v4();
    let context = FamilyCompositionContext::default();
    let mut unbound = begin_retained_position_composition(
        capture,
        &base,
        &originals,
        &context,
        Default::default(),
        true,
    )
    .unwrap();
    assert!(unbound.enable_graph_discovery(16).is_err());
    assert!(unbound.graph_discovery_report().is_err());
    assert_eq!(
        completed(
            unbound
                .advance(capture, &context, &Frame::default())
                .unwrap()
        ),
        angles(20., 30.)
    );
    let registry = CapturedPositionProgram::new(capture, &base, &originals).unwrap();
    let mut bound = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    assert!(bound.enable_graph_discovery(0).is_err());
    bound.enable_graph_discovery(16).unwrap();
    assert!(bound.enable_graph_discovery(32).is_err());
    let before = bound.graph_discovery_report().unwrap();
    assert!(before.reached.is_empty());
    assert_eq!(before.inspected_routes, 0);
    assert!(!before.complete);
    assert_eq!(
        completed(bound.advance(capture, &context, &Frame::default()).unwrap()),
        angles(20., 30.)
    );
    let accepted = bound.graph_discovery_report().unwrap();
    assert_eq!(accepted.reached.len(), 1);
    assert!(accepted.complete);
    assert!(bound.enable_graph_discovery(32).is_err());
    assert_eq!(
        bound.graph_discovery_report().unwrap().reached,
        accepted.reached
    );
    assert_eq!(
        completed(bound.advance(capture, &context, &Frame::default()).unwrap()),
        angles(20., 30.)
    );
}

mod graph_materialization_tests;
mod multi_source_resume_tests;
