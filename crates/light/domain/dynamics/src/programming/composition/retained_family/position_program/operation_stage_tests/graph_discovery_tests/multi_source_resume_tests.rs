//! TL636 original multi-source Resume operands from actual retained runtime history.
//! The Frame maps Target coordinates algebraically; these tests claim no physical fitting.
use super::*;

pub(super) struct HotResume {
    pub(super) base: AttributeValue,
    fixture: FixtureId,
    sample: FamilyCompositionSample,
    scope: PositionResumeScope,
    progress: f32,
    pub(super) runtime: DynamicRuntime,
    pub(super) sources: CapturedSources,
}
pub(super) fn hot_resume() -> HotResume {
    let fixture = FixtureId::new();
    let base = target(0, 4., 8.);
    let mut definition = definition_template();
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 2,
            denominator: 1,
        },
    };
    definition.lanes[0].id = Uuid::from_u128(2);
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
    let now = 1350;
    let samples = runtime
        .sample_programming(instance, now, 1000, 10, &sources, &sources)
        .unwrap();
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.instances.len(), 1);
    let state = &snapshot.instances[0];
    let resume = state
        .synchronized_resume_transition
        .expect("actual runtime interruption");
    assert_eq!(state.id, instance);
    assert_eq!(state.controllers.len(), 1);
    let progress = (now - resume.started_at_millis) as f32 / resume.duration_millis as f32;
    assert_eq!(progress, 0.25);
    let scope = PositionResumeScope {
        instance_id: instance,
        controller_id: state.controllers[0].id,
        occurrence_id: resume.occurrence_id,
    };
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&samples, &sources, None, &mut preparation).unwrap();
    assert!(prepared.requirements.is_empty());
    assert_eq!(prepared.families.len(), 1);
    assert_eq!(prepared.families[0].samples.len(), 1);
    let sample = prepared.families[0].samples[0].clone();
    let FamilyCompositionSample::CoupledExpression {
        expression,
        activation_mix,
        ..
    } = &sample
    else {
        panic!("actual Position forest")
    };
    assert_eq!(*activation_mix, 1.);
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
fn rank(sample: &FamilyCompositionSample) -> FamilySampleRank {
    match sample {
        FamilyCompositionSample::Known(sample) => sample.rank,
        FamilyCompositionSample::WholeExpression { rank, .. }
        | FamilyCompositionSample::CoupledExpression { rank, .. } => *rank,
    }
}
pub(super) fn fixed(
    value: AttributeValue,
    priority: i16,
    order: usize,
    mix: f32,
) -> FamilyCompositionSample {
    let mut source = mask(value, order, mix);
    let FamilyCompositionSample::Known(sample) = &mut source else {
        unreachable!()
    };
    sample.rank.priority = priority;
    source
}
pub(super) fn registry(
    fixture: &HotResume,
    upper: FamilyCompositionSample,
) -> CapturedPositionProgram {
    CapturedPositionProgram::new(
        Uuid::new_v4(),
        &fixture.base,
        &[
            fixed(target(0, 2., 4.), 5, 0, 1.),
            fixture.sample.clone(),
            upper,
        ],
    )
    .unwrap()
}
fn resume_ready(progress: PositionResumeOperandProgress) -> AttributeValue {
    match progress {
        PositionResumeOperandProgress::OperandReady(value) => value,
        PositionResumeOperandProgress::NeedsMaterialization(request) => panic!(
            "unexpected operand dependency {}: {:?}",
            request.request_id, request.requirement
        ),
        PositionResumeOperandProgress::Inactive => {
            panic!("original Resume operand must remain active")
        }
    }
}
fn resume_pending(progress: PositionResumeOperandProgress) -> PositionCompositionRequest {
    let PositionResumeOperandProgress::NeedsMaterialization(request) = progress else {
        panic!("authentic earlier prefix dependency")
    };
    request
}
fn issue(
    registry: &CapturedPositionProgram,
    branch: &PositionProgramBranch,
    context: &FamilyCompositionContext<'_>,
) -> (
    PositionCompositionContinuation,
    PositionCompositionRequest,
    PositionResumeOperandLocator,
) {
    let mut parent = branch
        .begin_composition(context, Default::default(), true)
        .unwrap();
    let request = needed(
        parent
            .advance(
                registry.capture_id(),
                context,
                &NoPhysicalContext::default(),
            )
            .unwrap(),
    );
    let locator = parent
        .pending_resume_operand_locator(request.request_id)
        .unwrap()
        .expect("original pending Resume route");
    (parent, request, locator)
}
fn expected_resume(progress: f32) -> AttributeValue {
    angles(12. + progress * (50. - 12.), 24. + progress * (60. - 24.))
}
fn expected_parent(progress: f32) -> AttributeValue {
    let AttributeValue::Position(value) = expected_resume(progress) else {
        unreachable!()
    };
    let PositionIntent::Angles {
        pan_degrees: ScalarIntent::Value(pan),
        tilt_degrees: ScalarIntent::Value(tilt),
    } = value.as_ref()
    else {
        unreachable!()
    };
    angles(*pan + 0.5 * (100. - *pan), *tilt + 0.5 * (200. - *tilt))
}

#[test]
fn real_multi_source_resume_operands_exclude_non_idempotent_suffix_and_preserve_parent() {
    let hot = hot_resume();
    let registry = registry(&hot, fixed(angles(100., 200.), 20, 2, 0.5));
    assert_eq!(registry.source_count(), 3);
    let context = FamilyCompositionContext::default();
    let before = hot.runtime.snapshot();
    let reads = hot.sources.reads.get();
    let (mut parent, request, locator) = issue(&registry, &registry.branch(), &context);
    assert_eq!(locator.capture_id(), registry.capture_id());
    assert_eq!(locator.scope(), hot.scope);
    assert_eq!(
        registry.resume_scope(locator.operation_node()).unwrap(),
        Some(hot.scope)
    );
    assert!(
        parent
            .pending_resume_operand_locator(Uuid::new_v4())
            .is_err()
    );
    for (endpoint, expected) in [
        (PositionResumeEndpoint::Outgoing, target(0, 12., 24.)),
        (PositionResumeEndpoint::Incoming, angles(50., 60.)),
    ] {
        assert!(
            registry
                .branch()
                .at_resume_operand(hot.scope, endpoint)
                .is_err(),
            "legacy source replacement guard stays in place"
        );
        registry
            .branch()
            .validate_resume_operand(&locator, endpoint)
            .unwrap();
        let mut operand = registry
            .branch()
            .begin_resume_operand(&locator, endpoint, &context, Default::default(), true)
            .unwrap();
        operand.enable_graph_discovery(32).unwrap();
        assert_eq!(
            resume_ready(
                operand
                    .advance(
                        registry.capture_id(),
                        &context,
                        &NoPhysicalContext::default()
                    )
                    .unwrap()
            ),
            expected
        );
        assert!(operand.graph_discovery_report().unwrap().complete);
        assert_eq!(
            resume_ready(
                operand
                    .advance(
                        registry.capture_id(),
                        &context,
                        &NoPhysicalContext::default()
                    )
                    .unwrap()
            ),
            expected
        );
        assert!(
            operand
                .pending_resume_operand_locator(request.request_id)
                .is_err()
        );
        assert_eq!(
            ordinary(&registry, operand.into_scratch()),
            ordinary(&registry, Default::default()),
            "speculative operands cannot mutate original source order or parent field trace"
        );
    }
    assert_eq!(
        needed(
            parent
                .advance(
                    registry.capture_id(),
                    &context,
                    &NoPhysicalContext::default()
                )
                .unwrap()
        )
        .request_id,
        request.request_id
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
    parent
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            expected_resume(hot.progress),
            None,
        )
        .unwrap();
    assert_eq!(
        completed(
            parent
                .advance(
                    registry.capture_id(),
                    &context,
                    &NoPhysicalContext::default()
                )
                .unwrap()
        ),
        expected_parent(hot.progress)
    );
    assert_eq!(
        ordinary(&registry, Default::default()).0,
        expected_parent(hot.progress),
        "genuine Resume progress and original suffix execute exactly once"
    );
    assert!(
        parent
            .pending_resume_operand_locator(request.request_id)
            .is_err()
    );
    assert_eq!(
        hot.sources.reads.get(),
        reads,
        "no new producer Current sampling"
    );
    assert_eq!(
        hot.runtime.snapshot(),
        before,
        "operand replay cannot advance producer clocks/history"
    );
}

#[test]
fn original_resume_lexical_use_and_registry_are_authority_not_occurrence_uuid_alone() {
    let hot = hot_resume();
    let mut higher = rank(&hot.sample);
    higher.priority = 20;
    let registry = registry(&hot, whole(leaf(angles(100., 200.)), higher, 0.5));
    let context = FamilyCompositionContext::default();
    let (parent, request, underlay) = issue(&registry, &registry.branch(), &context);
    assert_eq!(underlay.scope(), hot.scope);
    let mut without_consumer = registry.branch();
    without_consumer.replace_source(2, None).unwrap();
    let (_, _, final_use) = issue(&registry, &without_consumer, &context);
    assert!(underlay.operation_node() == final_use.operation_node());
    assert_ne!(
        underlay, final_use,
        "same actual Resume has distinct UnderlayFor/RootFinal uses"
    );
    let original_sources = [
        fixed(target(0, 2., 4.), 5, 0, 1.),
        hot.sample.clone(),
        whole(leaf(angles(100., 200.)), higher, 0.5),
    ];
    let foreign =
        CapturedPositionProgram::new(registry.capture_id(), &hot.base, &original_sources).unwrap();
    assert!(
        foreign
            .branch()
            .validate_resume_operand(&underlay, PositionResumeEndpoint::Outgoing)
            .is_err()
    );
    for foreign_scope in [
        PositionResumeScope {
            instance_id: Uuid::new_v4(),
            ..hot.scope
        },
        PositionResumeScope {
            controller_id: Uuid::new_v4(),
            ..hot.scope
        },
    ] {
        assert!(matches!(
            registry
                .branch()
                .resume_scope_membership(foreign_scope)
                .unwrap(),
            PositionResumeScopeMembership::Absent
        ));
        assert!(
            registry
                .branch()
                .at_resume_operand(foreign_scope, PositionResumeEndpoint::Outgoing)
                .unwrap()
                .is_none()
        );
    }
    let mut wrong_use = without_consumer
        .begin_resume_operand(
            &underlay,
            PositionResumeEndpoint::Outgoing,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    assert!(matches!(
        wrong_use
            .advance(registry.capture_id(), &context, &Frame::default())
            .unwrap(),
        PositionResumeOperandProgress::Inactive
    ));
    let mut exact = without_consumer
        .begin_resume_operand(
            &final_use,
            PositionResumeEndpoint::Incoming,
            &context,
            wrong_use.into_scratch(),
            true,
        )
        .unwrap();
    assert!(
        exact
            .advance(Uuid::new_v4(), &context, &Frame::default())
            .is_err()
    );
    assert_eq!(
        resume_ready(
            exact
                .advance(
                    registry.capture_id(),
                    &context,
                    &NoPhysicalContext::default()
                )
                .unwrap()
        ),
        angles(50., 60.)
    );
    assert_eq!(
        parent.pending_materialization().unwrap().request_id,
        request.request_id
    );
}

#[test]
fn removed_chosen_or_fully_covered_resume_never_revives_its_original_member_sources() {
    let hot = hot_resume();
    let registry = registry(&hot, fixed(angles(100., 200.), 20, 2, 1.));
    let context = FamilyCompositionContext::default();
    let mut visible = registry.branch();
    visible.replace_source(2, None).unwrap();
    let (_, _, locator) = issue(&registry, &visible, &context);
    let mut removed = visible.clone();
    removed.replace_source(1, None).unwrap();
    let mut selected = visible.clone();
    selected
        .choose_resume(hot.scope, PositionResumeEndpoint::Incoming)
        .unwrap();
    assert!(matches!(
        removed.resume_scope_membership(hot.scope).unwrap(),
        PositionResumeScopeMembership::Inactive
    ));
    // Membership describes the original transition's structural presence, including an
    // already chosen branch. That presence must not revive its discarded outgoing operand.
    assert!(matches!(
        selected.resume_scope_membership(hot.scope).unwrap(),
        PositionResumeScopeMembership::Active(nodes) if !nodes.is_empty()
    ));
    for branch in [removed, selected, registry.branch()] {
        let mut operand = branch
            .begin_resume_operand(
                &locator,
                PositionResumeEndpoint::Outgoing,
                &context,
                Default::default(),
                true,
            )
            .unwrap();
        operand.enable_graph_discovery(32).unwrap();
        assert!(matches!(
            operand
                .advance(registry.capture_id(), &context, &Frame::default())
                .unwrap(),
            PositionResumeOperandProgress::Inactive
        ));
        assert!(operand.graph_discovery_report().unwrap().complete);
        assert!(
            operand.graph_discovery_report().unwrap().reached.is_empty(),
            "hidden original Size cannot execute through a removed Resume"
        );
        assert!(matches!(
            operand
                .advance(registry.capture_id(), &context, &Frame::default())
                .unwrap(),
            PositionResumeOperandProgress::Inactive
        ));
    }
}

#[test]
fn earlier_current_mask_dependency_replays_once_per_driver_and_nested_retry_keeps_parent_pending() {
    let hot = hot_resume();
    let mut lower_rank = rank(&hot.sample);
    lower_rank.priority = 8;
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &hot.base,
        &[
            whole(leaf(target(0, 2., 4.)), lower_rank, 1.),
            fixed(angles(9., 11.), 9, 0, 0.5),
            hot.sample.clone(),
            fixed(angles(100., 200.), 20, 1, 0.5),
        ],
    )
    .unwrap();
    let current = Sources {
        current: target(0, 0., 0.),
        calls: Cell::new(0),
    };
    let control = |rank| {
        if rank == lower_rank {
            FamilyEndpointOutputControl::CrossfadeCurrent { mix: 0.5 }
        } else {
            FamilyEndpointOutputControl::Unchanged
        }
    };
    let context = FamilyCompositionContext {
        endpoint_output: Some(FamilyEndpointOutputContext {
            control: &control,
            target: hot.fixture,
            current: &current,
            native_models: None,
        }),
        ..Default::default()
    };
    let frame = NoPhysicalContext::default();
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let prefix = needed(
        parent
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert!(
        parent
            .pending_mask_locator(prefix.request_id)
            .unwrap()
            .is_some(),
        "real earlier mask adoption is reached first"
    );
    assert!(
        parent
            .pending_resume_operand_locator(prefix.request_id)
            .unwrap()
            .is_none()
    );
    assert_eq!(current.calls.get(), 1);
    parent
        .resume_materialization(
            registry.capture_id(),
            prefix.request_id,
            angles(1., 2.),
            None,
        )
        .unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    let locator = parent
        .pending_resume_operand_locator(request.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(locator.scope(), hot.scope);
    let mut operand = registry
        .branch()
        .begin_resume_operand(
            &locator,
            PositionResumeEndpoint::Outgoing,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    operand.enable_graph_discovery(32).unwrap();
    let dependency = resume_pending(
        operand
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert!(
        operand
            .pending_mask_locator(dependency.request_id)
            .unwrap()
            .is_some()
    );
    assert!(
        operand
            .pending_resume_operand_locator(dependency.request_id)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        current.calls.get(),
        2,
        "each original prefix driver captures its eligible Current exactly once"
    );
    assert!(
        operand
            .resume_materialization(
                registry.capture_id(),
                dependency.request_id,
                AttributeValue::Normalized(0.5),
                None
            )
            .is_err()
    );
    assert!(
        operand
            .resume_materialization(registry.capture_id(), Uuid::new_v4(), angles(1., 2.), None)
            .is_err()
    );
    assert_eq!(
        resume_pending(
            operand
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        )
        .request_id,
        dependency.request_id
    );
    assert_eq!(current.calls.get(), 2);
    assert!(!operand.graph_discovery_report().unwrap().complete);
    operand
        .resume_materialization(
            registry.capture_id(),
            dependency.request_id,
            angles(1., 2.),
            None,
        )
        .unwrap();
    assert_eq!(
        resume_ready(
            operand
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        target(0, 12., 24.)
    );
    assert_eq!(
        resume_ready(
            operand
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        target(0, 12., 24.)
    );
    assert_eq!(
        current.calls.get(),
        2,
        "retry/Ready cannot reread captured prefix Current"
    );
    assert!(operand.graph_discovery_report().unwrap().complete);
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
fn operand_prefix_unwind_is_terminal_and_recovered_workspace_preserves_full_parent() {
    let hot = hot_resume();
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &hot.base,
        &[
            fixed(angles(9., 11.), 9, 0, 0.5),
            hot.sample.clone(),
            fixed(angles(100., 200.), 20, 1, 0.5),
        ],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let mut parent = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    let no_physical = NoPhysicalContext::default();
    let earlier = needed(
        parent
            .advance(registry.capture_id(), &context, &no_physical)
            .unwrap(),
    );
    assert!(
        parent
            .pending_mask_locator(earlier.request_id)
            .unwrap()
            .is_some()
    );
    parent
        .resume_materialization(
            registry.capture_id(),
            earlier.request_id,
            angles(4., 8.),
            None,
        )
        .unwrap();
    let request = needed(
        parent
            .advance(registry.capture_id(), &context, &no_physical)
            .unwrap(),
    );
    let locator = parent
        .pending_resume_operand_locator(request.request_id)
        .unwrap()
        .unwrap();
    // The earlier mask needs adoption before any frame transition can run. Panic at that
    // actual prefix callback, using the same pinned context for begin and advance.
    let adoption_calls = Cell::new(0);
    let panic_adoption =
        |_: &AttributeValue, _: &DynamicValueAddress| -> Result<AttributeValue, TransitionError> {
            adoption_calls.set(adoption_calls.get() + 1);
            panic!("intentional Resume prefix adoption unwind")
        };
    let panic_context = FamilyCompositionContext {
        resolve_adoption: Some(&panic_adoption),
        ..Default::default()
    };
    let mut operand = registry
        .branch()
        .begin_resume_operand(
            &locator,
            PositionResumeEndpoint::Incoming,
            &panic_context,
            Default::default(),
            true,
        )
        .unwrap();
    operand.enable_graph_discovery(32).unwrap();
    let panics = Frame {
        panic: true,
        ..Default::default()
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operand.advance(
            registry.capture_id(),
            &panic_context,
            &panics
        )))
        .is_err()
    );
    assert_eq!(
        adoption_calls.get(),
        1,
        "actual prefix callback unwound once"
    );
    assert!(
        operand
            .advance(registry.capture_id(), &panic_context, &Frame::default())
            .is_err()
    );
    assert_eq!(
        adoption_calls.get(),
        1,
        "failed replay cannot call adoption again"
    );
    assert!(operand.graph_discovery_report().is_err());
    assert!(
        operand
            .pending_resume_operand_locator(request.request_id)
            .is_err()
    );
    assert!(operand.pending_mask_locator(earlier.request_id).is_err());
    // Recover under the same legitimate adoption used by the original parent. The generic
    // ordinary helper has no adoption context and cannot finish this deliberately mixed prefix.
    let finish = |scratch| {
        let adoption = |from: &AttributeValue, _: &DynamicValueAddress| Ok(angle_value(from));
        let recovery_context = FamilyCompositionContext {
            resolve_adoption: Some(&adoption),
            ..Default::default()
        };
        let mut replay = registry
            .branch()
            .begin_composition(&recovery_context, scratch, true)
            .unwrap();
        let value = completed(
            replay
                .advance(registry.capture_id(), &recovery_context, &Frame::default())
                .unwrap(),
        );
        let trace = replay.family_trace();
        let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, &value).unwrap();
        (
            value,
            trace.query_fields_with_base(trace.root().unwrap(), &fields),
        )
    };
    assert_eq!(finish(operand.into_scratch()), finish(Default::default()));
    assert_eq!(
        parent.pending_materialization().unwrap().request_id,
        request.request_id
    );
}

#[test]
fn resume_locator_issued_inside_graph_operand_does_not_escape_removed_enclosing_goal() {
    let fixture = FixtureId::new();
    let base = target(0, 4., 8.);
    let mut definition = definition_template();
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 2,
            denominator: 1,
        },
    };
    set_literal_lane(
        &mut definition.lanes[0],
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &target(0, 20., 40.))
            .unwrap(),
        DynamicValue::Family(target(0, 20., 40.)),
    );
    let (mut runtime, instance) = start_definition(&definition, &[fixture], 1000);
    let controller = runtime.snapshot().instances[0].controllers[0].id;
    runtime
        .update_controller(controller, Some(1.), None, None)
        .unwrap();
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
    set_literal_lane(
        &mut definition.lanes[0],
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &angles(50., 60.)).unwrap(),
        DynamicValue::Family(angles(50., 60.)),
    );
    runtime.install_definitions([definition]).unwrap();
    runtime
        .sample_programming(instance, 1100, 1000, 10, &sources, &sources)
        .unwrap();
    runtime.set_global_paused(false, 1100);
    let samples = runtime
        .sample_programming(instance, 1350, 1000, 10, &sources, &sources)
        .unwrap();
    assert_eq!(samples.len(), 1);
    let sample = &samples[0];
    let snapshot = runtime.snapshot();
    let transition = snapshot.instances[0]
        .synchronized_resume_transition
        .unwrap();
    let scope = PositionResumeScope {
        instance_id: instance,
        controller_id: controller,
        occurrence_id: transition.occurrence_id,
    };
    assert_eq!(snapshot.instances[0].controllers[0].size, 1.);
    let mut occurrences = Vec::new();
    sample
        .expression
        .visit_resume_occurrences(&mut |id| occurrences.push(id));
    assert_eq!(occurrences, vec![scope.occurrence_id]);
    assert!(
        sample
            .expression
            .operation_provenance()
            .unwrap()
            .handles()
            .is_empty()
    );

    // Explicit compositor-unit assembly: this Required wrapper is NOT sampler-produced and
    // deliberately has no producer witness. It only supplies an original enclosing goal
    // around the unchanged genuine runtime Resume. No cross-owner correspondence is claimed.
    // Scale cannot wrap this mixed-address Resume: its validated value must have one exact
    // family address. A Required transition preserves the genuine mixed representation.
    let wrapped = Arc::new(DynamicSampleExpression::Transition {
        from: Some(Arc::new(sample.expression.clone())),
        to: Some(leaf(angles(80., 100.))),
        progress: 0.5,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::MaterializedEndpoints,
        },
    });
    let provenance = wrapped.operation_provenance().unwrap();
    assert_eq!(provenance.unattributed_operations(), 1);
    assert!(
        provenance.handles().is_empty(),
        "unit wrapper cannot impersonate a producer Required operation"
    );
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &base,
        &[
            fixed(angles(1., 2.), 5, 0, 1.),
            whole(wrapped, sample_rank(sample), 1.),
            fixed(angles(100., 200.), 20, 1, 0.5),
        ],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let mut full = registry
        .branch()
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    full.enable_graph_discovery(32).unwrap();
    assert_eq!(
        completed(
            full.advance(registry.capture_id(), &context, &Frame::default())
                .unwrap()
        ),
        angles(76.875, 136.25)
    );
    let report = full.graph_discovery_report().unwrap();
    assert!(report.complete);
    assert_eq!(report.reached.len(), 1);
    let outer = report.reached[0].clone();
    assert_eq!(outer.kind(), PositionGraphOperationKind::Required);
    let mut graph = registry
        .branch()
        .begin_graph_operation_operand(
            &outer,
            PositionGraphOperationOperand::RequiredOutgoing,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    let request = suspended(
        graph
            .advance(
                registry.capture_id(),
                &context,
                &NoPhysicalContext::default(),
            )
            .unwrap(),
    );
    let nested = graph
        .pending_resume_operand_locator(request.request_id)
        .unwrap()
        .expect("actual Resume within RequiredOutgoing goal");
    assert_eq!(nested.scope(), scope);
    let mut original = registry
        .branch()
        .begin_resume_operand(
            &nested,
            PositionResumeEndpoint::Outgoing,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        resume_ready(
            original
                .advance(
                    registry.capture_id(),
                    &context,
                    &NoPhysicalContext::default()
                )
                .unwrap()
        ),
        target(0, 20., 40.)
    );

    let mut removed_outer = registry.branch();
    removed_outer
        .replace_source(
            nested.operation_node().source_index(),
            Some(nested.operation_node()),
        )
        .unwrap();
    assert!(
        matches!(removed_outer.resume_scope_membership(scope).unwrap(), PositionResumeScopeMembership::Active(nodes) if !nodes.is_empty()),
        "the genuine Resume still exists; only its original enclosing RequiredOutgoing goal was removed"
    );
    let mut stale_goal = removed_outer
        .begin_resume_operand(
            &nested,
            PositionResumeEndpoint::Incoming,
            &context,
            original.into_scratch(),
            true,
        )
        .unwrap();
    stale_goal.enable_graph_discovery(32).unwrap();
    assert!(
        matches!(
            stale_goal
                .advance(registry.capture_id(), &context, &Frame::default())
                .unwrap(),
            PositionResumeOperandProgress::Inactive
        ),
        "nested locator must not fall back to a Full-parent Resume when its original goal vanished"
    );
    assert!(stale_goal.graph_discovery_report().unwrap().complete);
    assert_eq!(
        suspended(
            graph
                .advance(
                    registry.capture_id(),
                    &context,
                    &NoPhysicalContext::default()
                )
                .unwrap()
        )
        .request_id,
        request.request_id
    );
    assert_eq!(runtime.snapshot(), snapshot);
    assert_eq!(
        sources.reads.get(),
        0,
        "literal Size1 runtime requires no producer Current resampling"
    );
}

#[test]
fn resume_locator_issued_inside_mask_operand_does_not_escape_removed_stage_goal() {
    let hot = hot_resume();
    let registry = registry(&hot, fixed(target(9, 100., 200.), 20, 2, 0.5));
    let context = FamilyCompositionContext::default();
    let no_physical = NoPhysicalContext::default();
    let (mut parent, request, original_resume) = issue(&registry, &registry.branch(), &context);
    assert_eq!(original_resume.scope(), hot.scope);
    parent
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            expected_resume(hot.progress),
            None,
        )
        .unwrap();
    let upper_request = needed(
        parent
            .advance(registry.capture_id(), &context, &no_physical)
            .unwrap(),
    );
    let upper = parent
        .pending_mask_locator(upper_request.request_id)
        .unwrap()
        .expect("actual upper Target mask adoption");
    assert_eq!(upper.stage(), PositionMaskStage::Adoption);
    let mut stage = registry
        .branch()
        .begin_mask_operand(
            &upper,
            PositionMaskOperand::AdoptionInput,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    let PositionStageOperandProgress::NeedsMaterialization(inner) = stage
        .advance(registry.capture_id(), &context, &no_physical)
        .unwrap()
    else {
        panic!("actual original Resume remains an earlier mask-operand dependency")
    };
    let nested = stage
        .pending_resume_operand_locator(inner.request_id)
        .unwrap()
        .expect("Resume issued from original Stage goal");
    assert_eq!(nested.scope(), hot.scope);
    assert!(nested.operation_node() == original_resume.operation_node());
    assert_ne!(
        nested, original_resume,
        "enclosing mask goal is part of original replay authority"
    );
    let mut original = registry
        .branch()
        .begin_resume_operand(
            &nested,
            PositionResumeEndpoint::Outgoing,
            &context,
            Default::default(),
            true,
        )
        .unwrap();
    assert_eq!(
        resume_ready(
            original
                .advance(registry.capture_id(), &context, &no_physical)
                .unwrap()
        ),
        target(0, 12., 24.)
    );
    let mut removed_upper = registry.branch();
    removed_upper.replace_source(2, None).unwrap();
    assert!(
        matches!(removed_upper.resume_scope_membership(hot.scope).unwrap(), PositionResumeScopeMembership::Active(nodes) if !nodes.is_empty())
    );
    let mut removed = removed_upper
        .begin_resume_operand(
            &nested,
            PositionResumeEndpoint::Incoming,
            &context,
            original.into_scratch(),
            true,
        )
        .unwrap();
    removed.enable_graph_discovery(32).unwrap();
    assert!(
        matches!(
            removed
                .advance(registry.capture_id(), &context, &Frame::default())
                .unwrap(),
            PositionResumeOperandProgress::Inactive
        ),
        "removing the mask removes this enclosing operand goal, even though its former prefix Resume still exists"
    );
    assert!(removed.graph_discovery_report().unwrap().complete);
    let PositionStageOperandProgress::NeedsMaterialization(repeated) = stage
        .advance(registry.capture_id(), &context, &no_physical)
        .unwrap()
    else {
        panic!("speculative nested replay cannot advance its owning Stage driver")
    };
    assert_eq!(repeated.request_id, inner.request_id);
    assert_eq!(
        parent.pending_materialization().unwrap().request_id,
        upper_request.request_id
    );
}
