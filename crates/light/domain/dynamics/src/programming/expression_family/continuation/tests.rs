use super::*;
use crate::{DynamicSourceOccurrenceId, DynamicValueAddress};
use light_core::{NativeColorBinding, NativeColorValue, PhysicalDataQuality};
use std::cell::Cell;
use uuid::Uuid;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(x: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [x, 2., 3.],
    )))
}
fn leaf(owner: ProgrammingOwner, value: AttributeValue) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(DynamicValueAddress::whole_family(owner, &value).unwrap()),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn transition(
    from: Option<Arc<DynamicSampleExpression>>,
    to: Option<Arc<DynamicSampleExpression>>,
    occurrence: u128,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from,
        to,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(occurrence),
        },
    })
}
#[derive(Default)]
struct Unavailable {
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for Unavailable {
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
#[derive(Default)]
struct Observed {
    nodes: Vec<usize>,
    reasons: Vec<DynamicTransitionReason>,
    scales: Vec<(f32, Option<DynamicSourceOccurrenceId>)>,
}
impl FamilyExpressionObserver for Observed {
    fn evaluated(
        &mut self,
        node: usize,
        step: FamilyExpressionStep<'_>,
        _: &AttributeValue,
    ) -> Result<(), TransitionError> {
        assert!(
            !self.nodes.contains(&node),
            "completed graph node is observed once"
        );
        self.nodes.push(node);
        match step {
            FamilyExpressionStep::Transition { reason, .. } => self.reasons.push(reason),
            FamilyExpressionStep::Scale {
                factor,
                baseline_occurrence,
                ..
            } => self.scales.push((factor, baseline_occurrence)),
            _ => {}
        }
        Ok(())
    }
}
fn needed(progress: FamilyEvaluationProgress) -> FamilyMaterializationRequest {
    let FamilyEvaluationProgress::NeedsMaterialization(request) = progress else {
        panic!("expected suspension")
    };
    request
}
fn complete(progress: FamilyEvaluationProgress) -> Option<AttributeValue> {
    let FamilyEvaluationProgress::Complete(value) = progress else {
        panic!("expected completion")
    };
    value
}

#[test]
fn continuation_retains_completed_children_reason_and_pending_response() {
    let child = transition(
        Some(leaf(ProgrammingOwner::Position, angles(10., 0.))),
        Some(leaf(ProgrammingOwner::Position, angles(20., 0.))),
        11,
    );
    let mixed = transition(
        Some(leaf(ProgrammingOwner::Position, target(4.))),
        Some(child),
        22,
    );
    let expression = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            transition(
                Some(mixed),
                Some(leaf(ProgrammingOwner::Position, angles(60., 0.))),
                33,
            ),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    );
    let mut evaluation = expression.begin_evaluation(None).unwrap();
    let frame = Unavailable::default();
    let mut observer = Observed::default();
    let request = needed(evaluation.advance(&frame, Some(&mut observer)).unwrap());
    let completed = observer.nodes.clone();
    assert_eq!(completed.len(), 4);
    let FamilyMaterializationOperation::Transition {
        from,
        to,
        progress,
        reason,
        from_node,
        to_node,
    } = &request.operation
    else {
        panic!()
    };
    assert_eq!(*from, target(4.));
    assert_eq!(*to, angles(15., 0.));
    assert_eq!(*progress, 0.5);
    assert_eq!(
        *reason,
        DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(22)
        }
    );
    assert!(completed.contains(from_node));
    assert!(completed.contains(to_node));
    assert_eq!(expression.transition_reason(request.node), Some(*reason));
    assert_eq!(
        needed(evaluation.advance(&frame, Some(&mut observer)).unwrap()).node,
        request.node
    );
    assert_eq!(frame.calls.get(), 1);
    assert_eq!(observer.nodes, completed);
    assert!(
        evaluation
            .resume_materialization(request.node + 1, angles(30., 0.), None)
            .is_err()
    );
    assert!(
        evaluation
            .resume_materialization(request.node, AttributeValue::Normalized(0.5), None)
            .is_err()
    );
    assert_eq!(
        evaluation.pending_materialization().unwrap().node,
        request.node
    );
    evaluation
        .resume_materialization(request.node, angles(30., 0.), None)
        .unwrap();
    assert_eq!(
        complete(evaluation.advance(&frame, Some(&mut observer)).unwrap()),
        Some(angles(45., 0.))
    );
    assert_eq!(frame.calls.get(), 1);
    assert_eq!(
        observer.reasons,
        [11, 22, 33].map(|v| DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(v)
        })
    );
    let nodes = observer.nodes.clone();
    assert_eq!(
        complete(evaluation.advance(&frame, Some(&mut observer)).unwrap()),
        Some(angles(45., 0.))
    );
    assert_eq!(observer.nodes, nodes);
}

#[test]
fn size_suspension_keeps_original_baseline_factor_and_occurrence() {
    let point = |id, x| {
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Point {
                point_id: Uuid::from_u128(id),
            },
            [x, 2., 3.],
        )))
    };
    let base = point(81, 9.);
    let child = point(82, 20.);
    let resolved = target(17.);
    let occurrence = DynamicSourceOccurrenceId::new(Uuid::from_u128(88)).unwrap();
    let retained = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &base).unwrap(),
        ),
        base: DynamicValue::Family(base.clone()),
        value: leaf(ProgrammingOwner::Position, child.clone()),
        factor: 0.25,
        baseline_occurrence: Some(occurrence),
    });
    let expression = Arc::new(
        CompiledProgrammingFamilyExpression::new(retained, ProgrammingOwner::Position, None, None)
            .unwrap(),
    );
    let mut evaluation = expression.begin_evaluation(None).unwrap();
    let frame = Unavailable::default();
    let mut observer = Observed::default();
    let request = needed(evaluation.advance(&frame, Some(&mut observer)).unwrap());
    assert_eq!(request.requirement, TransitionRequirement::LiveTargetPoints);
    let FamilyMaterializationOperation::Scale {
        base: original,
        value,
        factor,
        baseline_occurrence,
        ..
    } = request.operation
    else {
        panic!()
    };
    assert_eq!(original, base);
    assert_eq!(value, child);
    assert_eq!(factor, 0.25);
    assert_eq!(baseline_occurrence, Some(occurrence));
    evaluation
        .resume_materialization(request.node, resolved.clone(), None)
        .unwrap();
    assert_eq!(
        complete(evaluation.advance(&frame, Some(&mut observer)).unwrap()),
        Some(resolved)
    );
    assert_eq!(observer.scales, vec![(0.25, Some(occurrence))]);
    assert_eq!(frame.calls.get(), 1);
}

#[test]
fn missing_underlay_yields_and_exact_release_remains_absent() {
    let expression = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            transition(
                None,
                Some(leaf(ProgrammingOwner::Position, angles(20., 4.))),
                91,
            ),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    );
    let mut evaluation = expression.begin_evaluation(None).unwrap();
    let frame = Unavailable::default();
    let mut observer = Observed::default();
    let request = needed(evaluation.advance(&frame, Some(&mut observer)).unwrap());
    assert_eq!(request.node, 0);
    assert!(matches!(
        request.operation,
        FamilyMaterializationOperation::Underlay
    ));
    assert_eq!(frame.calls.get(), 0);
    assert!(observer.nodes.is_empty());
    evaluation
        .resume_materialization(0, angles(8., 0.), None)
        .unwrap();
    assert_eq!(
        complete(evaluation.advance(&frame, Some(&mut observer)).unwrap()),
        Some(angles(14., 2.))
    );
    assert_eq!(observer.nodes[0], 0);
    let released = Arc::new(DynamicSampleExpression::Transition {
        from: Some(leaf(ProgrammingOwner::Position, target(3.))),
        to: None,
        progress: 1.,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::LiveTargetPoints,
        },
    });
    let released = Arc::new(
        CompiledProgrammingFamilyExpression::new(released, ProgrammingOwner::Position, None, None)
            .unwrap(),
    );
    assert_eq!(
        complete(
            released
                .begin_evaluation(None)
                .unwrap()
                .advance(&frame, None)
                .unwrap()
        ),
        None
    );
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn shared_retained_dependency_resumes_once_and_matches_ordinary_evaluation() {
    let shared = transition(
        Some(leaf(ProgrammingOwner::Position, target(2.))),
        Some(leaf(ProgrammingOwner::Position, angles(10., 0.))),
        51,
    );
    let expression = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            transition(Some(shared.clone()), Some(shared), 52),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    );
    let frame = Unavailable::default();
    let mut evaluation = expression.begin_evaluation(None).unwrap();
    let mut observer = Observed::default();
    let request = needed(evaluation.advance(&frame, Some(&mut observer)).unwrap());
    evaluation
        .resume_materialization(request.node, angles(6., 1.), None)
        .unwrap();
    assert_eq!(
        complete(evaluation.advance(&frame, Some(&mut observer)).unwrap()),
        Some(angles(6., 1.))
    );
    assert_eq!(frame.calls.get(), 1);
    struct Available;
    impl WholeFamilyExpressionFrameResolver for Available {
        fn resolve(
            &self,
            _: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            Ok(angles(6., 1.))
        }
    }
    assert_eq!(
        expression.evaluate_optional(None, &Available).unwrap(),
        Some(angles(6., 1.))
    );
}

struct NativeModel {
    source: NativeColorIdentity,
    binding: NativeColorBinding,
    predictions: std::sync::atomic::AtomicUsize,
    panic_prediction: std::sync::atomic::AtomicBool,
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self.binding).then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: u32::MAX,
            continuous: true,
        })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        if recipe.source != self.source || recipe.channels.len() != 1 {
            return Err(IntentError("wrong original recipe".into()));
        }
        self.predictions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        assert!(
            !self
                .panic_prediction
                .load(std::sync::atomic::Ordering::Relaxed),
            "native prediction callback unwound"
        );
        Ok(PortableColorEstimate {
            model_revision: self.source.model_revision,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.6,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        })
    }
}
struct Models(Arc<NativeModel>);
impl DynamicNativeModelResolver for Models {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        if source != &self.0.source {
            return Err(IntentError("source unavailable".into()));
        }
        Ok(self.0.clone())
    }
}
fn model() -> Arc<NativeModel> {
    Arc::new(NativeModel {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(201),
            profile_revision: 3,
            profile_digest: "original".into(),
            mode_id: Uuid::from_u128(202),
            head_id: Uuid::from_u128(203),
            path_id: Uuid::from_u128(204),
            model_revision: 7,
            native_layout_signature: "u32-uv".into(),
        },
        binding: NativeColorBinding {
            channel_id: Uuid::from_u128(205),
            function_id: Uuid::from_u128(206),
        },
        predictions: std::sync::atomic::AtomicUsize::new(0),
        panic_prediction: std::sync::atomic::AtomicBool::new(false),
    })
}
fn direct(model: &NativeModel, raw: u32) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: model.source.clone(),
            channels: vec![NativeColorValue {
                channel_id: model.binding.channel_id,
                function_id: model.binding.function_id,
                raw,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: model.source.model_revision,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        },
    }))
}
#[test]
fn direct_color_response_requires_the_original_native_identity_and_keeps_uv_arithmetic() {
    let model = model();
    let models = Models(model.clone());
    let semantic = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    let original = direct(&model, u32::MAX - 2);
    let expression = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            transition(
                Some(leaf(ProgrammingOwner::Color, original.clone())),
                Some(leaf(ProgrammingOwner::Color, semantic)),
                71,
            ),
            ProgrammingOwner::Color,
            None,
            Some(&models),
        )
        .unwrap(),
    );
    let mut evaluation = expression.begin_evaluation(None).unwrap();
    let frame = Unavailable::default();
    let request = needed(evaluation.advance(&frame, None).unwrap());
    let FamilyMaterializationOperation::Transition { from, .. } = &request.operation else {
        panic!()
    };
    assert_eq!(*from, original);
    let before = model.predictions.load(std::sync::atomic::Ordering::Relaxed);
    let mut foreign = model.source.clone();
    foreign.profile_revision += 1;
    let AttributeValue::ColorProgram(program) = direct(&model, 19) else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
        panic!()
    };
    let mut recipe = recipe.clone();
    recipe.source = foreign;
    assert!(
        evaluation
            .resume_materialization(
                request.node,
                AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
                    recipe,
                    portable: portable.clone()
                })),
                None
            )
            .is_err()
    );
    assert_eq!(
        evaluation.pending_materialization().unwrap().node,
        request.node
    );
    needed(evaluation.advance(&frame, None).unwrap());
    assert_eq!(
        model.predictions.load(std::sync::atomic::Ordering::Relaxed),
        before
    );
    evaluation
        .resume_materialization(request.node, original.clone(), None)
        .unwrap();
    assert_eq!(
        complete(evaluation.advance(&frame, None).unwrap()),
        Some(original)
    );
    assert_eq!(frame.calls.get(), 1);
    let native = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            transition(
                Some(leaf(ProgrammingOwner::Color, direct(&model, u32::MAX - 2))),
                Some(leaf(ProgrammingOwner::Color, direct(&model, u32::MAX))),
                72,
            ),
            ProgrammingOwner::Color,
            None,
            Some(&models),
        )
        .unwrap(),
    );
    let result = complete(
        native
            .begin_evaluation(None)
            .unwrap()
            .advance(&frame, None)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        Some(result.clone()),
        native.evaluate_optional(None, &frame).unwrap()
    );
    let AttributeValue::ColorProgram(program) = result else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, u32::MAX - 1);
    assert_eq!(portable.uv.unwrap().amount, 0.6);
}

#[test]
fn resumed_transfer_is_delivered_once_and_observer_failure_is_terminal() {
    let expression = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            transition(
                Some(leaf(ProgrammingOwner::Position, target(2.))),
                Some(leaf(ProgrammingOwner::Position, angles(10., 0.))),
                101,
            ),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    );
    let frame = Unavailable::default();
    struct Transfers {
        trace: Option<ProgrammingTransitionTrace>,
    }
    impl FamilyExpressionObserver for Transfers {
        fn evaluated(
            &mut self,
            _: usize,
            step: FamilyExpressionStep<'_>,
            _: &AttributeValue,
        ) -> Result<(), TransitionError> {
            if let FamilyExpressionStep::Transition { trace, .. } = step {
                self.trace = trace.cloned();
            }
            Ok(())
        }
    }
    let mut evaluation = expression.begin_evaluation(None).unwrap();
    let mut observer = Transfers { trace: None };
    let request = needed(evaluation.advance(&frame, Some(&mut observer)).unwrap());
    let transfer = ProgrammingTransitionTrace {
        from: ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::new([ProgrammingTraceField::Pan]),
            remap: Arc::default(),
        },
        to: ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::new([ProgrammingTraceField::Tilt]),
            remap: Arc::default(),
        },
    };
    evaluation
        .resume_materialization(request.node, angles(6., 1.), Some(transfer.clone()))
        .unwrap();
    complete(evaluation.advance(&frame, Some(&mut observer)).unwrap());
    assert_eq!(observer.trace, Some(transfer));
    struct Fails {
        calls: usize,
    }
    impl FamilyExpressionObserver for Fails {
        fn evaluated(
            &mut self,
            _: usize,
            _: FamilyExpressionStep<'_>,
            _: &AttributeValue,
        ) -> Result<(), TransitionError> {
            self.calls += 1;
            Err(IntentError("trace arena unavailable".into()).into())
        }
    }
    let mut failure = expression.begin_evaluation(None).unwrap();
    let mut observer = Fails { calls: 0 };
    assert!(failure.advance(&frame, Some(&mut observer)).is_err());
    assert!(failure.advance(&frame, Some(&mut observer)).is_err());
    assert_eq!(observer.calls, 1);
}

#[test]
fn focus_and_captured_underlay_follow_the_same_scalar_operation() {
    let mut underlay = AttributeValue::Normalized(0.2);
    let expression = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            transition(
                None,
                Some(leaf(
                    ProgrammingOwner::Focus,
                    AttributeValue::Normalized(0.8),
                )),
                111,
            ),
            ProgrammingOwner::Focus,
            None,
            None,
        )
        .unwrap(),
    );
    let mut evaluation = expression.begin_evaluation(Some(&underlay)).unwrap();
    underlay = AttributeValue::Normalized(0.9);
    let frame = Unavailable::default();
    assert_eq!(
        complete(evaluation.advance(&frame, None).unwrap()),
        Some(AttributeValue::Normalized(0.5))
    );
    assert_eq!(
        expression
            .evaluate_optional(Some(&underlay), &frame)
            .unwrap(),
        Some(AttributeValue::Normalized(0.85))
    );
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn callback_unwind_poison_prevents_repeated_resolver_and_observer_calls() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let expression = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            transition(
                Some(leaf(ProgrammingOwner::Position, target(2.))),
                Some(leaf(ProgrammingOwner::Position, angles(10., 0.))),
                121,
            ),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    );
    struct Panics {
        calls: Cell<usize>,
    }
    impl WholeFamilyExpressionFrameResolver for Panics {
        fn resolve(
            &self,
            _: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            self.calls.set(self.calls.get() + 1);
            panic!("resolver callback unwound");
        }
    }
    let frame = Panics {
        calls: Cell::new(0),
    };
    let mut evaluation = expression.begin_evaluation(None).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| evaluation.advance(&frame, None))).is_err());
    assert!(evaluation.advance(&frame, None).is_err());
    assert!(
        evaluation
            .resume_materialization(0, angles(5., 0.), None)
            .is_err()
    );
    assert_eq!(frame.calls.get(), 1);

    struct ObserverPanics {
        calls: usize,
    }
    impl FamilyExpressionObserver for ObserverPanics {
        fn evaluated(
            &mut self,
            _: usize,
            _: FamilyExpressionStep<'_>,
            _: &AttributeValue,
        ) -> Result<(), TransitionError> {
            self.calls += 1;
            panic!("observer callback unwound");
        }
    }
    let frame = Unavailable::default();
    let mut evaluation = expression.begin_evaluation(None).unwrap();
    let mut observer = ObserverPanics { calls: 0 };
    assert!(
        catch_unwind(AssertUnwindSafe(
            || evaluation.advance(&frame, Some(&mut observer))
        ))
        .is_err()
    );
    assert!(evaluation.advance(&frame, Some(&mut observer)).is_err());
    assert!(
        evaluation
            .resume_materialization(0, angles(5., 0.), None)
            .is_err()
    );
    assert_eq!(observer.calls, 1);
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn native_response_validation_unwind_leaves_pending_episode_terminal() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::atomic::Ordering;
    let model = model();
    let models = Models(model.clone());
    let semantic = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    let original = direct(&model, 17);
    let expression = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            transition(
                Some(leaf(ProgrammingOwner::Color, original.clone())),
                Some(leaf(ProgrammingOwner::Color, semantic)),
                131,
            ),
            ProgrammingOwner::Color,
            None,
            Some(&models),
        )
        .unwrap(),
    );
    let mut evaluation = expression.begin_evaluation(None).unwrap();
    let frame = Unavailable::default();
    let request = needed(evaluation.advance(&frame, None).unwrap());
    let predictions = model.predictions.load(Ordering::Relaxed);
    model.panic_prediction.store(true, Ordering::Relaxed);
    assert!(
        catch_unwind(AssertUnwindSafe(|| evaluation.resume_materialization(
            request.node,
            original.clone(),
            None
        )))
        .is_err()
    );
    assert!(evaluation.pending_materialization().is_some());
    assert!(evaluation.advance(&frame, None).is_err());
    assert!(
        evaluation
            .resume_materialization(request.node, original, None)
            .is_err()
    );
    assert_eq!(model.predictions.load(Ordering::Relaxed), predictions + 1);
    assert_eq!(frame.calls.get(), 1);
}
