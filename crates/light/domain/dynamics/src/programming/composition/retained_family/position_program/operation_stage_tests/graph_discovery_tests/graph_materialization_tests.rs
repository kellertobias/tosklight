//! TL556 opt-in materialization at an exact original reached Required/Size site. The
//! inline value is discarded, the original operation is requested through the ordinary path
//! and the original goal suffix resumes from the supplied result. Domain replay only: no
//! physical fit, cross-owner correspondence or output acceptance is asserted here.
use super::multi_source_resume_tests::{HotResume, fixed, hot_resume, registry as hot_registry};
use super::*;

struct SizeFixture {
    runtime: DynamicRuntime,
    sources: CapturedSources,
    samples: Vec<DynamicRuntimeSample>,
    targets: [FixtureId; 2],
    bases: [AttributeValue; 2],
}
impl SizeFixture {
    fn sample(&self, index: usize) -> &DynamicRuntimeSample {
        self.samples
            .iter()
            .find(|sample| sample.target == self.targets[index])
            .unwrap()
    }
    fn registry(&self, index: usize) -> CapturedPositionProgram {
        single_registry(self.sample(index), &self.bases[index])
    }
}
/// One actual controller Size .5 over two targets: an Angles baseline finishes inline and a
/// Target baseline suspends naturally (the same emission as the TL635 discovery fixture).
fn size_fixture() -> SizeFixture {
    let targets = [FixtureId::new(), FixtureId::new()];
    let bases = [angles(4., 8.), target(0, 4., 8.)];
    let endpoint = angles(20., 40.);
    let mut definition = definition_template();
    set_literal_lane(
        &mut definition.lanes[0],
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &endpoint).unwrap(),
        DynamicValue::Family(endpoint),
    );
    let (mut runtime, instance) = start_definition(&definition, &targets, 0);
    let sources = CapturedSources {
        values: targets.into_iter().zip(bases.clone()).collect(),
        reads: Cell::new(0),
    };
    let samples = runtime
        .sample_programming(instance, 250, 1000, 10, &sources, &sources)
        .unwrap();
    assert_eq!(sources.reads.get(), 2);
    SizeFixture {
        runtime,
        sources,
        samples,
        targets,
        bases,
    }
}
/// The algebraic test frame, but reporting the field transfer of its own solve so suffix
/// lineage is known rather than unknown.
#[derive(Default)]
struct TracedFrame {
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for TracedFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.resolve_with_trace(requirement, from, to, operation)
            .map(|(value, _)| value)
    }
    fn resolve_with_trace(
        &self,
        _: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        self.calls.set(self.calls.get() + 1);
        let compiled =
            CompiledProgrammingTransition::new(angle_value(from), angle_value(to), None)?;
        match operation {
            FamilyExpressionOperation::Transition { progress } => {
                compiled.sample_with_trace(ProgrammingOwner::Position, progress)
            }
            FamilyExpressionOperation::Scale { factor } => {
                compiled.scale_with_trace(ProgrammingOwner::Position, factor)
            }
        }
    }
}
fn traced_query(
    driver: &PositionCompositionContinuation,
    value: &AttributeValue,
) -> Option<FamilyTraceQuery> {
    let trace = driver.family_trace();
    let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, value).unwrap();
    trace.query_fields_with_base(trace.root().expect("parent trace root"), &fields)
}
fn begin(registry: &CapturedPositionProgram) -> PositionCompositionContinuation {
    registry
        .branch()
        .begin_composition(
            &FamilyCompositionContext::default(),
            Default::default(),
            true,
        )
        .unwrap()
}
/// Exact original locators come only from actual same-registry discovery.
fn discovered(
    registry: &CapturedPositionProgram,
    frame: &dyn WholeFamilyExpressionFrameResolver,
) -> PositionGraphDiscoveryReport {
    let mut driver = begin(registry);
    driver.enable_graph_discovery(32).unwrap();
    driver
        .advance(
            registry.capture_id(),
            &FamilyCompositionContext::default(),
            frame,
        )
        .unwrap();
    driver.graph_discovery_report().unwrap()
}
fn kind_locator(
    report: &PositionGraphDiscoveryReport,
    kind: PositionGraphOperationKind,
) -> PositionGraphOperationLocator {
    let candidates = report
        .reached
        .iter()
        .filter(|locator| locator.kind() == kind)
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 1, "one exact reached lexical use");
    candidates[0].clone()
}
fn base_scale(request: &BaseMaterializationRequest) -> (AttributeValue, AttributeValue, f32) {
    match &request.operation {
        PositionCompositionBaseOperation::Whole(FamilyMaterializationRequest {
            operation:
                FamilyMaterializationOperation::Scale {
                    base,
                    value,
                    factor,
                    ..
                },
            ..
        }) => (base.clone(), value.clone(), *factor),
        PositionCompositionBaseOperation::Coupled(PositionMaterializationRequest {
            operation:
                PositionMaterializationOperation::Scale {
                    base,
                    value,
                    factor,
                    ..
                },
            ..
        }) => {
            let (_, DynamicValue::Family(base)) = base.materialized().expect("captured baseline")
            else {
                panic!("whole Size baseline")
            };
            (base.clone(), value.clone(), *factor)
        }
        PositionCompositionBaseOperation::SourceCohort { request, .. } => base_scale(request),
        _ => panic!("original Size operation"),
    }
}
fn scale(request: &PositionCompositionRequest) -> (AttributeValue, AttributeValue, f32) {
    let PositionCompositionOperation::Base { request, .. } = &request.operation else {
        panic!("base operation")
    };
    base_scale(request)
}
fn required(request: &PositionCompositionRequest) -> (AttributeValue, AttributeValue, f32) {
    let PositionCompositionOperation::Base { request, .. } = &request.operation else {
        panic!("base operation")
    };
    let PositionCompositionBaseOperation::Whole(FamilyMaterializationRequest {
        operation:
            FamilyMaterializationOperation::Transition {
                from,
                to,
                progress,
                reason: DynamicTransitionReason::Required { .. },
                ..
            },
        ..
    }) = &request.operation
    else {
        panic!("original Required operation")
    };
    (from.clone(), to.clone(), *progress)
}
/// The transfer a fitted Size reports: the request's own original operation, not a resample.
fn scale_transfer(request: &PositionCompositionRequest) -> Option<ProgrammingTransitionTrace> {
    let (base, value, factor) = scale(request);
    let (_, transfer) = CompiledProgrammingTransition::new(base, value, None)
        .unwrap()
        .scale_with_trace(ProgrammingOwner::Position, factor)
        .unwrap();
    assert!(
        transfer.is_some(),
        "compatible Angles Size has a known transfer"
    );
    transfer
}
fn status(driver: &PositionCompositionContinuation) -> PositionGraphMaterializationStatus {
    driver.graph_materialization_status().unwrap()
}

#[test]
fn synchronous_angle_size_materializes_original_operation_without_inline_value() {
    let fixture = size_fixture();
    let registry = fixture.registry(0);
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let site = size_locator(&discovered(&registry, &frame));
    let mut driver = begin(&registry);
    assert!(
        driver.graph_materialization_status().is_err(),
        "materialization is opt in"
    );
    driver.enable_graph_materialization(&site).unwrap();
    assert_eq!(status(&driver), PositionGraphMaterializationStatus::Armed);
    let request = needed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert!(request.is_materialized_reached_site());
    assert_eq!(request.requirement, TransitionRequirement::LiveJointAngles);
    assert_eq!(
        scale(&request),
        (angles(4., 8.), angles(20., 40.), 0.5),
        "original baseline, value endpoint and factor; not the inline angles(12, 24)"
    );
    assert_eq!(
        driver
            .pending_graph_operation_locator(request.request_id)
            .unwrap()
            .unwrap(),
        site
    );
    assert_eq!(status(&driver), PositionGraphMaterializationStatus::Issued);
    assert_eq!(
        frame.calls.get(),
        0,
        "compatible Size needs no physical port"
    );
    let retry = needed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert_eq!(
        retry.request_id, request.request_id,
        "the same wait()-minted identity, not a locator-derived one"
    );
    assert!(retry.is_materialized_reached_site());
    driver
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(13., 27.),
            None,
        )
        .unwrap();
    assert_eq!(status(&driver), PositionGraphMaterializationStatus::Resumed);
    assert_eq!(
        completed(
            driver
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        angles(13., 27.)
    );
    assert_eq!(status(&driver), PositionGraphMaterializationStatus::Resumed);
}

#[test]
fn resumed_site_feeds_original_suffix_once_and_keeps_the_parent_goal() {
    let fixture = size_fixture();
    let sample = fixture.sample(0);
    let mut higher = sample_rank(sample);
    higher.priority += 1;
    // The partial higher whole source consumes the Size result as its UnderlayFor and
    // crossfades to a Target endpoint, which requires the counting algebraic frame once.
    let originals = [
        whole(Arc::new(sample.expression.clone()), sample_rank(sample), 1.),
        whole(leaf(target(0, 100., 200.)), higher, 0.5),
    ];
    let registry =
        CapturedPositionProgram::new(Uuid::new_v4(), &fixture.bases[0], &originals).unwrap();
    let context = FamilyCompositionContext::default();
    let inline_frame = TracedFrame::default();
    let mut plain = begin(&registry);
    plain.enable_graph_discovery(16).unwrap();
    let inline = completed(
        plain
            .advance(registry.capture_id(), &context, &inline_frame)
            .unwrap(),
    );
    assert_eq!(inline, angles(56., 112.));
    assert_eq!(inline_frame.calls.get(), 1, "the suffix runs once inline");
    let inline_trace = traced_query(&plain, &inline);
    assert!(inline_trace.is_some(), "known inline suffix lineage");
    let site = size_locator(&plain.graph_discovery_report().unwrap());
    let frame = TracedFrame::default();
    let mut driver = begin(&registry);
    driver.enable_graph_materialization(&site).unwrap();
    let request = needed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert!(request.is_materialized_reached_site());
    assert_eq!(
        frame.calls.get(),
        0,
        "the suffix after the site has not run before the response"
    );
    driver
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(14., 30.),
            scale_transfer(&request),
        )
        .unwrap();
    let fitted_final = completed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert_eq!(
        fitted_final,
        angles(14. + 0.5 * (100. - 14.), 30. + 0.5 * (200. - 30.)),
        "final = original suffix applied to the fitted Size result"
    );
    assert_eq!(
        frame.calls.get(),
        1,
        "the outer suffix is evaluated exactly once"
    );
    assert_eq!(
        completed(
            driver
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        fitted_final
    );
    assert_eq!(
        frame.calls.get(),
        1,
        "a completed goal never reruns its suffix"
    );
    assert_eq!(
        traced_query(&driver, &fitted_final),
        inline_trace,
        "the parent goal keeps its own source/suffix trace"
    );
}

#[test]
fn materialization_does_not_resample_runtime_or_change_the_pre_site_trace() {
    let fixture = size_fixture();
    let registry = fixture.registry(0);
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let before = fixture.runtime.snapshot();
    let reads = fixture.sources.reads.get();
    let (inline, inline_trace) = ordinary(&registry, Default::default());
    assert_eq!(inline, angles(12., 24.));
    let site = size_locator(&discovered(&registry, &frame));
    let mut driver = begin(&registry);
    driver.enable_graph_materialization(&site).unwrap();
    let request = needed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    // Answer with the value the inline operation would have produced: everything else in
    // the driver must be indistinguishable from the ordinary run.
    driver
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            inline.clone(),
            scale_transfer(&request),
        )
        .unwrap();
    let value = completed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert_eq!(value, inline);
    assert!(inline_trace.is_some(), "known inline Size lineage");
    assert_eq!(
        traced_query(&driver, &value),
        inline_trace,
        "same source/baseline trace as the non-materialized run"
    );
    assert_eq!(
        fixture.sources.reads.get(),
        reads,
        "no runtime Current resampling"
    );
    assert_eq!(
        fixture.runtime.snapshot(),
        before,
        "no producer clock, Random or history advance"
    );
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn foreign_registry_is_refused_at_enable_and_foreign_use_is_not_reached() {
    let fixture = size_fixture();
    let sample = fixture.sample(0);
    let mut higher = sample_rank(sample);
    higher.priority += 1;
    let originals = [
        whole(Arc::new(sample.expression.clone()), sample_rank(sample), 1.),
        whole(leaf(angles(100., 200.)), higher, 0.5),
    ];
    let capture = Uuid::new_v4();
    let registry = CapturedPositionProgram::new(capture, &fixture.bases[0], &originals).unwrap();
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let underlay = size_locator(&discovered(&registry, &frame));
    let foreign = CapturedPositionProgram::new(capture, &fixture.bases[0], &originals).unwrap();
    let mut refused = begin(&foreign);
    assert!(refused.enable_graph_materialization(&underlay).is_err());
    assert!(
        refused.graph_materialization_status().is_err(),
        "a refused enable arms nothing"
    );
    // The refused driver never started: it still accepts pre-start configuration and runs
    // its own ordinary goal unchanged.
    refused.enable_graph_discovery(16).unwrap();
    assert_eq!(
        completed(refused.advance(capture, &context, &frame).unwrap()),
        angles(56., 112.)
    );
    let mut foreign_operand = foreign
        .branch()
        .begin_graph_operation_operand(
            &size_locator(&refused.graph_discovery_report().unwrap()),
            PositionGraphOperationOperand::SizeValue,
            &context,
            refused.into_scratch(),
            true,
        )
        .unwrap();
    assert!(
        foreign_operand
            .enable_graph_materialization(&underlay)
            .is_err()
    );
    // Same registry, another lexical use: RootFinal is not the UnderlayFor site.
    let mut without_mask = registry.branch();
    without_mask.replace_source(1, None).unwrap();
    let mut other_use = without_mask
        .begin_composition(&context, foreign_operand.into_scratch(), true)
        .unwrap();
    other_use.enable_graph_materialization(&underlay).unwrap();
    assert_eq!(
        completed(other_use.advance(capture, &context, &frame).unwrap()),
        angles(12., 24.),
        "inline value of another use is not a materialized result"
    );
    assert_eq!(
        status(&other_use),
        PositionGraphMaterializationStatus::NotReached
    );
    assert_eq!(fixture.sources.reads.get(), 2);
}

#[test]
fn enable_requires_bound_unstarted_single_arming_and_failure_stays_terminal() {
    let sample = sampled(1.5);
    let base = angles(0., 0.);
    let originals = [whole(sample.expression, sample.rank, 1.)];
    let capture = Uuid::new_v4();
    let context = FamilyCompositionContext::default();
    let registry = CapturedPositionProgram::new(capture, &base, &originals).unwrap();
    let report = discovered(&registry, &Frame::default());
    let site = kind_locator(&report, PositionGraphOperationKind::Size);
    let mut unbound = begin_retained_position_composition(
        capture,
        &base,
        &originals,
        &context,
        Default::default(),
        true,
    )
    .unwrap();
    assert!(unbound.enable_graph_materialization(&site).is_err());
    assert!(unbound.graph_materialization_status().is_err());
    let mut started = begin(&registry);
    assert_eq!(
        completed(
            started
                .advance(capture, &context, &Frame::default())
                .unwrap()
        ),
        angles(28., 41.)
    );
    assert!(
        started.enable_graph_materialization(&site).is_err(),
        "cannot retrofit after evaluation"
    );
    let mut pending = begin(&registry);
    let request = needed(
        pending
            .advance(capture, &context, &NoPhysicalContext::default())
            .unwrap(),
    );
    assert!(
        pending.enable_graph_materialization(&site).is_err(),
        "cannot retrofit while a started goal is pending before the site"
    );
    assert!(pending.graph_materialization_status().is_err());
    pending
        .resume_materialization(capture, request.request_id, angles(20., 30.), None)
        .unwrap();
    assert!(
        !needed(
            pending
                .advance(capture, &context, &NoPhysicalContext::default())
                .unwrap()
        )
        .is_materialized_reached_site(),
        "the refused enable armed nothing"
    );
    let mut twice = begin(&registry);
    twice.enable_graph_materialization(&site).unwrap();
    assert!(twice.enable_graph_materialization(&site).is_err());
    assert_eq!(status(&twice), PositionGraphMaterializationStatus::Armed);
    let panic_frame = Frame {
        panic: true,
        ..Default::default()
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| twice.advance(
            capture,
            &context,
            &panic_frame
        )))
        .is_err()
    );
    assert!(twice.graph_materialization_status().is_err());
    assert!(twice.enable_graph_materialization(&site).is_err());
    assert!(twice.advance(capture, &context, &Frame::default()).is_err());
    assert!(twice.graph_materialization_status().is_err());
}

#[test]
fn removed_unneeded_inactive_or_stop_sites_never_materialize() {
    let sample = sampled(1.5);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[whole(sample.expression, sample.rank, 1.)],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let frame = Frame::default();
    let report = discovered(&registry, &frame);
    let required_site = kind_locator(&report, PositionGraphOperationKind::Required);
    let size_site = kind_locator(&report, PositionGraphOperationKind::Size);
    // Removed branch: the original source is gone.
    let mut removed = registry.branch();
    removed.replace_source(0, None).unwrap();
    let mut driver = removed
        .begin_composition(&context, Default::default(), true)
        .unwrap();
    driver.enable_graph_materialization(&size_site).unwrap();
    assert_eq!(
        completed(
            driver
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        angles(0., 0.)
    );
    assert_eq!(
        status(&driver),
        PositionGraphMaterializationStatus::NotReached
    );
    // Outside the operand's needed set: Required incoming excludes the Size ancestor.
    let mut operand = registry
        .branch()
        .begin_graph_operation_operand(
            &required_site,
            PositionGraphOperationOperand::RequiredIncoming,
            &context,
            driver.into_scratch(),
            true,
        )
        .unwrap();
    operand.enable_graph_materialization(&size_site).unwrap();
    assert_eq!(
        ready(
            operand
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        target(9, 30., 40.)
    );
    assert_eq!(
        operand.graph_materialization_status().unwrap(),
        PositionGraphMaterializationStatus::NotReached
    );
    // A Graph goal's own stop operation is never executed by that goal.
    let mut stop = registry
        .branch()
        .begin_graph_operation_operand(
            &size_site,
            PositionGraphOperationOperand::SizeValue,
            &context,
            operand.into_scratch(),
            true,
        )
        .unwrap();
    assert!(stop.enable_graph_materialization(&size_site).is_err());
    assert!(stop.graph_materialization_status().is_err());
    // An inactive enclosing Resume scope or fully covered Resume never reaches its member Size.
    let hot = hot_resume();
    let hot_registry = hot_registry(&hot, fixed(angles(100., 200.), 20, 2, 1.));
    let mut visible = hot_registry.branch();
    visible.replace_source(2, None).unwrap();
    let (parent, _, resume) = issue_resume(&hot_registry, &visible, &context);
    let member_size = size_locator(&parent.graph_discovery_report().unwrap());
    let mut inactive_branch = visible.clone();
    inactive_branch.replace_source(1, None).unwrap();
    let mut inactive = inactive_branch
        .begin_resume_operand(
            &resume,
            PositionResumeEndpoint::Outgoing,
            &context,
            stop.into_scratch(),
            true,
        )
        .unwrap();
    assert!(
        inactive.enable_graph_materialization(&member_size).is_err(),
        "an inactive enclosing goal is already complete and lends nothing"
    );
    let mut covered = hot_registry
        .branch()
        .begin_resume_operand(
            &resume,
            PositionResumeEndpoint::Outgoing,
            &context,
            inactive.into_scratch(),
            true,
        )
        .unwrap();
    covered.enable_graph_materialization(&member_size).unwrap();
    assert!(matches!(
        covered
            .advance(hot_registry.capture_id(), &context, &frame)
            .unwrap(),
        PositionResumeOperandProgress::Inactive
    ));
    assert_eq!(
        covered.graph_materialization_status().unwrap(),
        PositionGraphMaterializationStatus::NotReached
    );
    assert!(
        parent.graph_materialization_status().is_err(),
        "discovery alone never arms materialization"
    );
}

#[test]
fn inline_operation_error_is_not_masked_as_a_request() {
    struct Broken;
    impl WholeFamilyExpressionFrameResolver for Broken {
        fn resolve(
            &self,
            _: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            Err(IntentError("broken physical port".into()).into())
        }
    }
    let fixture = size_fixture();
    let registry = fixture.registry(1);
    let context = FamilyCompositionContext::default();
    let site = size_locator(&discovered(&registry, &NoPhysicalContext::default()));
    let mut ordinary_driver = begin(&registry);
    assert!(matches!(
        ordinary_driver.advance(registry.capture_id(), &context, &Broken),
        Err(TransitionError::Invalid(_))
    ));
    let mut driver = begin(&registry);
    driver.enable_graph_materialization(&site).unwrap();
    assert!(matches!(
        driver.advance(registry.capture_id(), &context, &Broken),
        Err(TransitionError::Invalid(_))
    ));
    assert!(driver.pending_materialization().is_none());
    assert!(driver.graph_materialization_status().is_err());
    assert!(
        driver
            .advance(
                registry.capture_id(),
                &context,
                &NoPhysicalContext::default()
            )
            .is_err()
    );
}

#[test]
fn naturally_pending_site_keeps_its_request_and_only_gains_the_marker() {
    let fixture = size_fixture();
    let registry = fixture.registry(1);
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let site = size_locator(&discovered(&registry, &frame));
    let mut plain = begin(&registry);
    let natural = needed(
        plain
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    let mut armed = begin(&registry);
    armed.enable_graph_materialization(&site).unwrap();
    let request = needed(
        armed
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert!(!natural.is_materialized_reached_site());
    assert!(request.is_materialized_reached_site());
    assert_eq!(request.requirement, natural.requirement);
    assert_eq!(scale(&request), scale(&natural));
    assert_eq!(scale(&request), (target(0, 4., 8.), angles(20., 40.), 0.5));
    assert_eq!(
        armed
            .pending_graph_operation_locator(request.request_id)
            .unwrap(),
        plain
            .pending_graph_operation_locator(natural.request_id)
            .unwrap()
    );
    assert_eq!(status(&armed), PositionGraphMaterializationStatus::Issued);
    for (driver, request) in [(&mut plain, &natural), (&mut armed, &request)] {
        driver
            .resume_materialization(
                registry.capture_id(),
                request.request_id,
                angles(11., 23.),
                None,
            )
            .unwrap();
        assert_eq!(
            completed(
                driver
                    .advance(registry.capture_id(), &context, &frame)
                    .unwrap()
            ),
            angles(11., 23.)
        );
    }
    assert_eq!(status(&armed), PositionGraphMaterializationStatus::Resumed);
    // A natural Required request keeps its own requirement rather than the forced one.
    let sample = sampled(1.5);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[whole(sample.expression, sample.rank, 1.)],
    )
    .unwrap();
    let site = kind_locator(
        &discovered(&registry, &frame),
        PositionGraphOperationKind::Required,
    );
    let mut plain = begin(&registry);
    let natural = needed(
        plain
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    let mut armed = begin(&registry);
    armed.enable_graph_materialization(&site).unwrap();
    let request = needed(
        armed
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert!(!natural.is_materialized_reached_site());
    assert!(request.is_materialized_reached_site());
    assert_ne!(
        natural.requirement,
        TransitionRequirement::LiveJointAngles,
        "fixture: the Target reference change suspends for its own reason"
    );
    assert_eq!(request.requirement, natural.requirement);
    assert_eq!(required(&request), required(&natural));
}

#[test]
fn required_before_size_keeps_actual_order_and_either_site_can_materialize() {
    let sample = sampled(1.5);
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &angles(0., 0.),
        &[whole(sample.expression, sample.rank, 1.)],
    )
    .unwrap();
    let context = FamilyCompositionContext::default();
    let report = discovered(&registry, &Frame::default());
    let required_site = kind_locator(&report, PositionGraphOperationKind::Required);
    let size_site = kind_locator(&report, PositionGraphOperationKind::Size);
    // Only the Required transition is blocked; the Size is synchronous in the algebraic frame.
    let blocked = Frame {
        block_to: Some(target(9, 30., 40.)),
        ..Default::default()
    };
    let mut plain = begin(&registry);
    let natural = needed(
        plain
            .advance(registry.capture_id(), &context, &blocked)
            .unwrap(),
    );
    plain
        .resume_materialization(
            registry.capture_id(),
            natural.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    assert_eq!(
        completed(
            plain
                .advance(registry.capture_id(), &context, &blocked)
                .unwrap()
        ),
        angles(28., 41.)
    );
    let plain_calls = blocked.calls.get();
    let frame = Frame {
        block_to: Some(target(9, 30., 40.)),
        ..Default::default()
    };
    let mut driver = begin(&registry);
    driver.enable_graph_materialization(&size_site).unwrap();
    let first = needed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert!(!first.is_materialized_reached_site());
    assert_eq!(first.requirement, natural.requirement);
    assert_eq!(
        driver
            .pending_graph_operation_locator(first.request_id)
            .unwrap()
            .unwrap(),
        required_site
    );
    assert_eq!(status(&driver), PositionGraphMaterializationStatus::Armed);
    driver
        .resume_materialization(
            registry.capture_id(),
            first.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    let second = needed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    assert_ne!(second.request_id, first.request_id);
    assert!(second.is_materialized_reached_site());
    assert_eq!(second.requirement, TransitionRequirement::LiveJointAngles);
    assert_eq!(
        scale(&second),
        (target(0, 4., 8.), angles(20., 30.), 1.5),
        "the Size value is the answered Required result"
    );
    assert_eq!(
        frame.calls.get(),
        plain_calls,
        "the inline Size is evaluated once (and discarded), exactly as often as in the plain run"
    );
    driver
        .resume_materialization(
            registry.capture_id(),
            second.request_id,
            angles(29., 43.),
            None,
        )
        .unwrap();
    assert_eq!(
        completed(
            driver
                .advance(registry.capture_id(), &context, &frame)
                .unwrap()
        ),
        angles(29., 43.)
    );
    // Arming the earlier Required site instead: the later Size runs inline from its response.
    let mut earlier = begin(&registry);
    earlier
        .enable_graph_materialization(&required_site)
        .unwrap();
    let synchronous = Frame::default();
    let request = needed(
        earlier
            .advance(registry.capture_id(), &context, &synchronous)
            .unwrap(),
    );
    assert!(request.is_materialized_reached_site());
    assert_eq!(request.requirement, TransitionRequirement::LiveJointAngles);
    assert_eq!(
        required(&request),
        (target(0, 10., 20.), target(9, 30., 40.), 0.5)
    );
    earlier
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(20., 30.),
            None,
        )
        .unwrap();
    assert_eq!(
        completed(
            earlier
                .advance(registry.capture_id(), &context, &synchronous)
                .unwrap()
        ),
        angles(28., 41.)
    );
    assert_eq!(
        status(&earlier),
        PositionGraphMaterializationStatus::Resumed
    );
}

#[test]
fn site_fires_once_and_a_second_reach_is_an_ambiguity_error() {
    let fixture = size_fixture();
    let registry = fixture.registry(0);
    let context = FamilyCompositionContext::default();
    let frame = NoPhysicalContext::default();
    let site = size_locator(&discovered(&registry, &frame));
    let mut driver = begin(&registry);
    driver.enable_graph_materialization(&site).unwrap();
    driver.enable_graph_discovery(16).unwrap();
    let request = needed(
        driver
            .advance(registry.capture_id(), &context, &frame)
            .unwrap(),
    );
    driver
        .resume_materialization(
            registry.capture_id(),
            request.request_id,
            angles(13., 27.),
            None,
        )
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            completed(
                driver
                    .advance(registry.capture_id(), &context, &frame)
                    .unwrap()
            ),
            angles(13., 27.)
        );
    }
    let report = driver.graph_discovery_report().unwrap();
    assert_eq!(
        report.reached,
        vec![site.clone()],
        "the site was reached once"
    );
    assert_eq!(report.inspected_routes, 1);
    assert_eq!(status(&driver), PositionGraphMaterializationStatus::Resumed);
    // A second reach of the armed site within one driver is refused. The genuine route and
    // original binding come from an actual natural request of the same registry.
    let target_fixture = fixture.registry(1);
    let mut natural = begin(&target_fixture);
    let pending = needed(
        natural
            .advance(target_fixture.capture_id(), &context, &frame)
            .unwrap(),
    );
    let PositionCompositionOperation::Base { request, .. } = &pending.operation else {
        panic!("base request")
    };
    let route = request.graph_route.clone().expect("actual graph route");
    let target_site = natural
        .pending_graph_operation_locator(pending.request_id)
        .unwrap()
        .unwrap();
    let mut armed = begin(&target_fixture);
    armed.enable_graph_materialization(&target_site).unwrap();
    let reach = |driver: &mut PositionCompositionContinuation| {
        super::super::super::graph_materialization::reached_action(
            (
                &mut driver.graph_discovery,
                &mut driver.graph_materialization,
            ),
            driver.origin_binding.as_ref(),
            &route,
        )
    };
    assert!(matches!(
        reach(&mut armed),
        Ok(GraphReachedAction::Materialize)
    ));
    assert!(reach(&mut armed).is_err(), "ambiguous second reach");
    // Through the real driver: an already fired site reached again fails the goal.
    let mut fired = begin(&target_fixture);
    fired.enable_graph_materialization(&target_site).unwrap();
    assert!(matches!(
        reach(&mut fired),
        Ok(GraphReachedAction::Materialize)
    ));
    assert!(
        fired
            .advance(target_fixture.capture_id(), &context, &frame)
            .is_err()
    );
    assert!(fired.graph_materialization_status().is_err());
    assert!(fired.pending_materialization().is_none());
}

fn issue_resume(
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
    parent.enable_graph_discovery(32).unwrap();
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

mod goal_tests;
