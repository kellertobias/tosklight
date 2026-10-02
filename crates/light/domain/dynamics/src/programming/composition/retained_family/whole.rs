//! Retained whole-source graph and provenance state for the outer composition stack.
use super::*;

pub(super) struct Inputs {
    pub underlay: Option<TracedValue>,
    pub evaluation: crate::FamilyEvaluationContinuation,
    pub observer_nodes: Vec<Option<FamilyTraceNodeId>>,
    pub completion: Option<position_completion::PositionSourceCompletion>,
}

pub(super) enum AdvanceProgress {
    Complete(FamilySample),
    OperandReady(AttributeValue),
    NeedsMaterialization(base_evaluation::BaseMaterializationRequest),
}

pub(super) fn begin(
    index: usize,
    underlay: Option<TracedValue>,
    scratch: &RetainedFamilyCompositionScratch,
) -> Result<Inputs, TransitionError> {
    let FamilyCompositionSample::WholeExpression { expression, .. } = &scratch.sources[index]
    else {
        return Err(IntentError("whole graph task names a non-expression source".into()).into());
    };
    let evaluation = expression.begin_evaluation(underlay.as_ref().map(|value| &value.value))?;
    Ok(Inputs {
        underlay,
        evaluation,
        observer_nodes: Vec::new(),
        completion: None,
    })
}

pub(super) fn advance(
    index: usize,
    inputs: Inputs,
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<AdvanceProgress, TransitionError> {
    advance_with_graph(
        index,
        inputs,
        Vec::new(),
        context,
        frame,
        scratch,
        &mut PreparedPositionStageMatcher::disabled(),
    )
}
pub(super) fn advance_with_graph(
    index: usize,
    mut inputs: Inputs,
    uses: Vec<PreparedPositionStageUse>,
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    scratch: &mut RetainedFamilyCompositionScratch,
    matcher: &mut PreparedPositionStageMatcher<'_>,
) -> Result<AdvanceProgress, TransitionError> {
    let FamilyCompositionSample::WholeExpression {
        expression,
        rank,
        activation_mix,
    } = scratch.sources[index].clone()
    else {
        unreachable!("whole expression task")
    };
    struct Observer<'a> {
        trace: &'a mut FamilyTraceArena,
        nodes: &'a mut Vec<Option<FamilyTraceNodeId>>,
        rank: FamilySampleRank,
        underlay: Option<FamilyTraceNodeId>,
    }
    impl FamilyExpressionObserver for Observer<'_> {
        fn evaluated(
            &mut self,
            node: usize,
            step: FamilyExpressionStep<'_>,
            _: &AttributeValue,
        ) -> Result<(), TransitionError> {
            if self.nodes.len() <= node {
                self.nodes.resize(node + 1, None);
            }
            let source = |role, occurrence| FamilyTraceSource {
                rank: self.rank,
                footprint: FamilyTraceFootprint::Whole,
                role,
                occurrence,
            };
            let id = match step {
                FamilyExpressionStep::Underlay => self.underlay.ok_or(
                    TransitionError::Requires(TransitionRequirement::MaterializedEndpoints),
                )?,
                FamilyExpressionStep::Authored {
                    occurrence,
                    dependency_occurrence,
                    ..
                } => {
                    let authored = self
                        .trace
                        .source(source(FamilyTraceRole::Authored, occurrence));
                    if let Some(dependency) = dependency_occurrence {
                        let source = source(
                            FamilyTraceRole::CalculationDependency,
                            dependency.occurrence,
                        );
                        let dependency = self.trace.current_source(source, dependency);
                        self.trace.bundle(vec![authored, dependency])
                    } else {
                        authored
                    }
                }
                FamilyExpressionStep::Baseline { occurrence, .. } => self
                    .trace
                    .source(source(FamilyTraceRole::CalculationDependency, occurrence)),
                FamilyExpressionStep::Scale {
                    value,
                    baseline_occurrence,
                    trace,
                    ..
                } => {
                    let child = self.nodes[value].expect("compiled Scale child");
                    let baseline = self.trace.source(source(
                        FamilyTraceRole::CalculationDependency,
                        baseline_occurrence,
                    ));
                    self.trace.mapped_blend(baseline, child, trace.cloned())
                }
                FamilyExpressionStep::Transition {
                    from, to, trace, ..
                } => self.trace.mapped_blend(
                    self.nodes[from].expect("compiled outgoing child"),
                    self.nodes[to].expect("compiled incoming child"),
                    trace.cloned(),
                ),
            };
            self.nodes[node] = Some(id);
            Ok(())
        }
    }
    let underlay = inputs.underlay.as_ref();
    let evaluated = {
        let mut observer = scratch.trace_enabled.then(|| Observer {
            trace: &mut scratch.trace,
            nodes: &mut inputs.observer_nodes,
            rank,
            underlay: underlay.and_then(|value| value.trace),
        });
        let graph_enabled = matcher.graph_is_enabled();
        let reached_enabled = matcher.graph_reached_is_enabled();
        let PreparedPositionStageMatcher {
            graph_callback,
            graph_reached_callback,
            graph_plan_callback,
            stop_callback,
            ..
        } = matcher;
        let route = |node, kind| PreparedPositionGraphRoute {
            uses: uses.clone(),
            trigger_origins: scratch
                .source_origins
                .get(index)
                .cloned()
                .unwrap_or_default(),
            expression: PreparedPositionGraphExpression::Whole(expression.clone()),
            node,
            kind,
        };
        let mut reached = |node, kind| {
            graph_reached_callback
                .as_deref_mut()
                .expect("enabled reached callback")(&route(node, kind))
        };
        let reached: Option<&mut GraphOperationReachedCallback<'_>> = if reached_enabled {
            Some(&mut reached)
        } else {
            None
        };
        if graph_enabled {
            let mut selector = |node, kind| {
                graph_callback
                    .as_deref_mut()
                    .expect("enabled graph callback")(&route(node, kind))
            };
            let progress = if let Some(callback) = graph_plan_callback.as_deref_mut() {
                let mut plan = |node, kind| callback(&route(node, kind));
                inputs.evaluation.advance_with_graph_plan_and_reached(
                    frame,
                    observer
                        .as_mut()
                        .map(|value| value as &mut dyn FamilyExpressionObserver),
                    &mut plan,
                    reached,
                )?
            } else {
                inputs.evaluation.advance_with_graph_operand_and_reached(
                    frame,
                    observer
                        .as_mut()
                        .map(|value| value as &mut dyn FamilyExpressionObserver),
                    &mut selector,
                    reached,
                )?
            };
            match progress {
                GraphOperationProgress::Ordinary(progress) => progress,
                GraphOperationProgress::OperandReady(value) => {
                    if let Some(callback) = stop_callback.as_deref_mut() {
                        callback(
                            inputs
                                .evaluation
                                .graph_selected_depth()
                                .expect("selected graph depth"),
                        );
                    }
                    scratch
                        .tasks
                        .push(BaseTask::EvaluateWhole { index, inputs });
                    return Ok(AdvanceProgress::OperandReady(value));
                }
            }
        } else {
            inputs.evaluation.advance_with_reached(
                frame,
                observer
                    .as_mut()
                    .map(|value| value as &mut dyn FamilyExpressionObserver),
                reached,
            )?
        }
    };
    let target = match evaluated {
        crate::FamilyEvaluationProgress::Complete(Some(value)) => value,
        crate::FamilyEvaluationProgress::Complete(None) => {
            return Err(IntentError("inactive whole expression entered composition".into()).into());
        }
        crate::FamilyEvaluationProgress::NeedsMaterialization(request) => {
            let kind = match &request.operation {
                crate::FamilyMaterializationOperation::Transition {
                    reason: crate::DynamicTransitionReason::Required { .. },
                    ..
                } => Some(GraphOperationKind::Required),
                crate::FamilyMaterializationOperation::Transition {
                    reason: crate::DynamicTransitionReason::Resume { .. },
                    ..
                } => Some(GraphOperationKind::Resume),
                crate::FamilyMaterializationOperation::Scale { .. } => {
                    Some(GraphOperationKind::Size)
                }
                _ => None,
            };
            let graph_route = kind.map(|kind| PreparedPositionGraphRoute {
                uses: uses.clone(),
                trigger_origins: scratch
                    .source_origins
                    .get(index)
                    .cloned()
                    .unwrap_or_default(),
                expression: PreparedPositionGraphExpression::Whole(expression.clone()),
                node: request.node,
                kind,
            });
            scratch
                .tasks
                .push(BaseTask::EvaluateWhole { index, inputs });
            let mut request: base_evaluation::BaseMaterializationRequest = request.into();
            request.graph_route = graph_route;
            return Ok(AdvanceProgress::NeedsMaterialization(request));
        }
    };
    let expression_trace = scratch.trace_enabled.then(|| {
        inputs.observer_nodes[expression.trace_root_node()].expect("evaluated whole root")
    });
    if expression.owner() == ProgrammingOwner::Position {
        if inputs.completion.is_none() {
            inputs.completion = Some(position_completion::PositionSourceCompletion::new(
                TracedValue {
                    value: target,
                    trace: expression_trace,
                },
                rank,
                activation_mix,
                inputs.underlay.clone(),
                position_completion::PositionCompletionKind::Whole,
            )?);
        }
        match inputs
            .completion
            .as_mut()
            .expect("Position completion")
            .advance(context, frame, &mut scratch.trace, scratch.trace_enabled)?
        {
            position_completion::CompletionProgress::Complete(value) => {
                return Ok(AdvanceProgress::Complete(completed_position_sample(
                    index, value, scratch,
                )?));
            }
            position_completion::CompletionProgress::Needs(request) => {
                scratch
                    .tasks
                    .push(BaseTask::EvaluateWhole { index, inputs });
                return Ok(AdvanceProgress::NeedsMaterialization(request.into()));
            }
        }
    }
    let (target, expression_trace) = endpoint_output::whole(
        rank,
        expression.owner(),
        target,
        expression_trace,
        context,
        frame,
        &|source| expression.resolve_original_native_model(source),
        scratch.trace_enabled.then_some(&mut scratch.trace),
    )?;
    let (value, activation_transfer) = if activation_mix == 1.0 {
        (target, None)
    } else {
        let from = &underlay
            .expect("partial activation resolves its underlay")
            .value;
        if matches!(
            endpoint_output::control(rank, context),
            FamilyEndpointOutputControl::CrossfadeCurrent { .. }
        ) {
            endpoint_output::transition(
                expression.owner(),
                from,
                &target,
                activation_mix,
                context,
                frame,
                &|source| expression.resolve_original_native_model(source),
                scratch.trace_enabled,
            )?
        } else if scratch.trace_enabled {
            expression.transition_with_trace(from, &target, activation_mix, frame)?
        } else {
            (
                expression.transition(from, &target, activation_mix, frame)?,
                None,
            )
        }
    };
    let trace_node = if activation_mix == 1.0 {
        expression_trace
    } else {
        expression_trace.map(|incoming| {
            scratch.trace.mapped_blend(
                underlay
                    .and_then(|value| value.trace)
                    .expect("traced activation underlay"),
                incoming,
                activation_transfer,
            )
        })
    };
    let trace_node = trace_node.map(|appearance| {
        endpoint_output::control_trace(
            rank,
            FamilyTraceFootprint::Whole,
            appearance,
            underlay
                .and_then(|value| value.trace)
                .filter(|_| activation_mix < 1.0),
            context,
            &mut scratch.trace,
        )
    });
    let address = DynamicValueAddress::whole_family(expression.owner(), &value)?;
    let model = if let DynamicFamilyRepresentation::DirectColor { source } = &address.representation
    {
        Some(endpoint_output::resolve_native_model(
            source,
            context,
            &|source| expression.resolve_original_native_model(source),
        )?)
    } else {
        None
    };
    let address = Arc::new(CompiledDynamicValueAddress::new(address, model)?);
    // This internal boundary accepts only evaluator-verified outputs: Direct leaves were
    // checked cold, native math predicts complete recipes, frame results verify original models.
    // Calling FamilySample::new here would perform the same complete prediction a second time.
    address.validate_value(&DynamicValue::Family(value.clone()))?;
    let sample = FamilySample {
        address,
        body: FamilySampleBody::Materialized(DynamicValue::Family(value)),
        projection: None,
        fix_at: false,
        endpoint_output_exempt: false,
        trace_sources: None,
        trace_node,
        rank,
        // Resume/tree evaluation and activation already ran separately, exactly once.
        activation_mix: 1.0,
    };
    scratch.resolved[index] = Some(sample.clone());
    Ok(AdvanceProgress::Complete(sample))
}
