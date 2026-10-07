//! Stages of one coupled source task: graph-plan configuration, underlay scheduling,
//! per-endpoint cohort preparation/resume, base evaluation and endpoint output.
use super::*;

/// Parameters shared by every stage of one coupled source task.
pub(super) struct CoupledAdvance {
    pub(super) index: usize,
    pub(super) cursor: usize,
    pub(super) expression: Arc<CompiledCoupledExpression>,
    pub(super) rank: FamilySampleRank,
    pub(super) activation_mix: f32,
    pub(super) uses: Vec<PreparedPositionStageUse>,
}

type EvaluatedBase =
    GraphOperationProgress<Result<AttributeValue, crate::PositionMaterializationRequest>>;

impl CoupledAdvance {
    fn push_resume(
        &self,
        step: usize,
        inputs: BaseInputs,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) {
        scratch.tasks.push(BaseTask::Coupled {
            index: self.index,
            cursor: self.cursor,
            step,
            inputs,
            uses: self.uses.clone(),
        });
    }

    pub(super) fn configure_graph(
        &self,
        inputs: &mut BaseInputs,
        scratch: &RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<(), TransitionError> {
        let (index, uses, expression) = (self.index, &self.uses, &self.expression);
        if inputs.evaluation.is_none() {
            inputs.evaluation = Some(expression.begin_position_evaluation()?);
        }
        let route = |node, kind| PreparedPositionGraphRoute {
            uses: uses.clone(),
            trigger_origins: scratch
                .source_origins
                .get(index)
                .cloned()
                .unwrap_or_default(),
            expression: PreparedPositionGraphExpression::Coupled(expression.clone()),
            node,
            kind,
        };
        if let Some(callback) = matcher.graph_plan_callback.as_deref_mut() {
            let mut plan = |node, kind| callback(&route(node, kind));
            inputs
                .evaluation
                .as_mut()
                .expect("Position graph operand")
                .configure_graph_plan(&mut plan)?;
        } else {
            let mut selector = |node, kind| matcher.match_graph(&route(node, kind));
            inputs
                .evaluation
                .as_mut()
                .expect("Position graph operand")
                .configure_graph_operand(&mut selector)?;
        }
        Ok(())
    }

    pub(super) fn needs_underlay(&self, inputs: &BaseInputs, selected_graph: bool) -> bool {
        let expression = &self.expression;
        (if selected_graph {
            inputs
                .evaluation
                .as_ref()
                .unwrap()
                .graph_operand_needs_underlay()
        } else {
            expression.needs_base_underlay()
        }) || (!selected_graph && self.activation_mix < 1.0)
            || expression
                .base_endpoints()
                .iter()
                .enumerate()
                .any(|(endpoint_index, endpoint)| {
                    endpoint.cohort_sources().is_some()
                        && inputs.evaluation.as_ref().is_none_or(|evaluation| {
                            evaluation.graph_operand_endpoint_needed(endpoint_index)
                        })
                })
    }

    pub(super) fn schedule_underlay(
        &self,
        step: usize,
        inputs: BaseInputs,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> AdvanceProgress {
        let (index, cursor, uses) = (self.index, self.cursor, &self.uses);
        scratch.tasks.push(BaseTask::CollectCoupled {
            index,
            cursor,
            step,
            inputs,
            uses: uses.clone(),
        });
        let mut underlay_uses = uses.clone();
        underlay_uses.push(PreparedPositionStageUse::UnderlayFor {
            consumer_origins: scratch
                .source_origins
                .get(index)
                .cloned()
                .unwrap_or_default(),
        });
        scratch.tasks.push(BaseTask::Compose {
            end: cursor,
            uses: underlay_uses,
        });
        AdvanceProgress::Scheduled
    }

    /// Pushes the endpoint's value and returns None, or suspends/finishes the task; a
    /// suspended task takes `inputs` with it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn advance_endpoint(
        &self,
        endpoint: &crate::CoupledBaseEndpoint,
        step: usize,
        inputs: &mut BaseInputs,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<Option<AdvanceProgress>, TransitionError> {
        if matcher.graph_is_enabled()
            && !inputs
                .evaluation
                .as_ref()
                .unwrap()
                .graph_operand_endpoint_needed(step - 1)
        {
            inputs.values.push(None);
            return Ok(None);
        }
        if let Some(sources) = endpoint.cohort_sources() {
            // The child's prepared sources, lower prefix, trace and heap tasks survive
            // suspension. Its endpoint's original Arc remains in the parent's expression.
            if inputs.source_cohort.is_none() {
                self.begin_source_cohort(sources, step, inputs, context, scratch)?;
            }
            return self.resume_source_cohort(step, inputs, context, frame, scratch, matcher);
        }
        if let Some(components) = endpoint.cohort() {
            return self.schedule_component_cohort(components, step, inputs, scratch);
        }
        self.materialized_endpoint(endpoint, step, inputs, scratch)
    }

    fn begin_source_cohort(
        &self,
        sources: &[CoupledCohortEndpoint],
        step: usize,
        inputs: &mut BaseInputs,
        context: &FamilyCompositionContext<'_>,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Result<(), TransitionError> {
        let (index, rank, uses, expression) = (self.index, self.rank, &self.uses, &self.expression);
        let underlay = inputs
            .underlay
            .as_ref()
            .expect("source cohort lower prefix");
        let mut candidates = std::mem::take(&mut scratch.source_cohort_candidates);
        candidates.clear();
        let prepared = sources.iter().try_for_each(|source| {
            let candidate = match source {
                CoupledCohortEndpoint::Materialized(component) => {
                    FamilyCompositionSample::Known(verified_component(component, rank, 1.0)?)
                }
                CoupledCohortEndpoint::WholeExpression {
                    lane_id,
                    expression,
                } => FamilyCompositionSample::WholeExpression {
                    expression: expression.clone(),
                    rank: rank.with_dynamic_lane(*lane_id)?,
                    activation_mix: 1.0,
                },
            };
            candidates.push(candidate);
            Ok::<(), TransitionError>(())
        });
        if let Err(error) = prepared {
            scratch.source_cohort_candidates = candidates;
            return Err(error);
        }
        let mut nested = scratch.source_cohort_scratch.take().unwrap_or_default();
        let prepared = prepare_family_inputs_with_origins(
            ProgrammingOwner::Position,
            &underlay.value,
            candidates
                .drain(..)
                .enumerate()
                .map(|(member, sample)| (sample, vec![member])),
            &endpoint_output::with_control(context, None),
            &mut nested,
            scratch.trace_enabled,
        );
        scratch.source_cohort_candidates = candidates;
        let base_trace = match prepared {
            Ok(value) => value,
            Err(error) => {
                scratch.source_cohort_scratch = Some(nested);
                return Err(error);
            }
        };
        // SourceCohort endpoints consist only of ordinary materialized/whole sources,
        // never FixAT masks or Color orthogonals; base composition is the full pass.
        let ordered = std::mem::take(&mut nested.ordered);
        let mut child_uses = uses.clone();
        child_uses.push(PreparedPositionStageUse::SourceCohort {
            consumer_origins: scratch
                .source_origins
                .get(index)
                .cloned()
                .unwrap_or_default(),
            expression: expression.clone(),
            endpoint_nodes: expression
                .base_endpoint_nodes(step - 1)
                .unwrap_or_default()
                .to_vec(),
        });
        let evaluation = base_evaluation::BaseEvaluation::begin_with_route(
            &underlay.value,
            base_trace,
            &ordered,
            &underlay.value,
            &mut nested,
            child_uses,
        );
        nested.ordered = ordered;
        inputs.source_cohort = Some(Box::new(SourceCohortEvaluation {
            evaluation,
            scratch: nested,
        }));
        Ok(())
    }

    fn resume_source_cohort(
        &self,
        step: usize,
        inputs: &mut BaseInputs,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<Option<AdvanceProgress>, TransitionError> {
        let index = self.index;
        let mut child = inputs.source_cohort.take().expect("retained source cohort");
        match child.evaluation.advance_with_stage(
            &endpoint_output::with_control(context, None),
            frame,
            &mut child.scratch,
            matcher,
        ) {
            Ok(base_evaluation::BaseEvaluationProgress::OperandReady(value)) => {
                inputs.source_cohort = Some(child);
                self.push_resume(step, std::mem::take(inputs), scratch);
                Ok(Some(AdvanceProgress::OperandReady(value)))
            }
            Ok(base_evaluation::BaseEvaluationProgress::Pending) => {
                inputs.source_cohort = Some(child);
                self.push_resume(step, std::mem::take(inputs), scratch);
                Ok(Some(AdvanceProgress::Scheduled))
            }
            Ok(base_evaluation::BaseEvaluationProgress::NeedsMaterialization {
                source_index,
                request,
            }) => {
                let source_rank = request.source_rank(source_index, &child.scratch);
                let request = base_evaluation::BaseMaterializationRequest {
                    node: request.node,
                    source_origins: scratch
                        .source_origins
                        .get(index)
                        .cloned()
                        .unwrap_or_default(),
                    stage_route: request.stage_route.clone(),
                    graph_route: request.graph_route.clone(),
                    requirement: request.requirement,
                    operation: base_evaluation::BaseMaterializationOperation::SourceCohort {
                        endpoint_index: step - 1,
                        child_source_index: source_index,
                        source_rank,
                        request: Box::new(request),
                    },
                };
                inputs.source_cohort = Some(child);
                self.push_resume(step, std::mem::take(inputs), scratch);
                Ok(Some(AdvanceProgress::NeedsMaterialization(request)))
            }
            Ok(base_evaluation::BaseEvaluationProgress::Complete(resolved)) => {
                self.complete_source_cohort(child, resolved, step, inputs, scratch, matcher)
            }
            Err(error) => {
                child.evaluation.recycle(&mut child.scratch);
                scratch.source_cohort_scratch = Some(child.scratch);
                Err(error)
            }
        }
    }

    fn complete_source_cohort(
        &self,
        mut child: Box<SourceCohortEvaluation>,
        resolved: TracedValue,
        step: usize,
        inputs: &mut BaseInputs,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<Option<AdvanceProgress>, TransitionError> {
        let (index, uses, expression) = (self.index, &self.uses, &self.expression);
        if let Some(callback) = matcher.cohort_callback.as_deref_mut()
            && let Some(depth) = callback(
                uses,
                scratch
                    .source_origins
                    .get(index)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
                expression,
                expression.base_endpoint_nodes(step - 1).unwrap_or_default(),
            )?
        {
            let value = resolved.value.clone();
            inputs.source_cohort = Some(child);
            self.push_resume(step, std::mem::take(inputs), scratch);
            matcher.completed_stop(depth);
            return Ok(Some(AdvanceProgress::OperandReady(value)));
        }
        let underlay = inputs
            .underlay
            .as_ref()
            .expect("source cohort lower prefix");
        let trace = match (resolved.trace, underlay.trace) {
            (Some(root), Some(base)) => Some(scratch.trace.append_graph_rebased(
                &child.scratch.trace,
                root,
                base,
            )),
            _ => None,
        };
        child.evaluation.recycle(&mut child.scratch);
        scratch.source_cohort_scratch = Some(child.scratch);
        inputs.values.push(Some(TracedValue {
            value: resolved.value,
            trace,
        }));
        Ok(None)
    }

    fn schedule_component_cohort(
        &self,
        components: &[CoupledComponentEndpoint],
        step: usize,
        inputs: &mut BaseInputs,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Result<Option<AdvanceProgress>, TransitionError> {
        let (index, cursor, rank, uses) = (self.index, self.cursor, self.rank, &self.uses);
        let candidates = components
            .iter()
            .map(|component| verified_component(component, rank, 1.0))
            .collect::<Result<Vec<_>, _>>()?;
        scratch.tasks.push(BaseTask::CollectCoupled {
            index,
            cursor,
            step,
            inputs: std::mem::take(inputs),
            uses: uses.clone(),
        });
        let origins = vec![
            scratch
                .source_origins
                .get(index)
                .cloned()
                .unwrap_or_default();
            candidates.len()
        ];
        let mut endpoint_uses = uses.clone();
        endpoint_uses.push(PreparedPositionStageUse::UnsupportedCoupledEndpoint);
        scratch.tasks.push(BaseTask::Cohort {
            end: cursor,
            candidates,
            origins,
            uses: endpoint_uses,
        });
        Ok(Some(AdvanceProgress::Scheduled))
    }

    fn materialized_endpoint(
        &self,
        endpoint: &crate::CoupledBaseEndpoint,
        step: usize,
        inputs: &mut BaseInputs,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Result<Option<AdvanceProgress>, TransitionError> {
        let (index, cursor, rank, uses) = (self.index, self.cursor, self.rank, &self.uses);
        let (address, source_value) = endpoint.materialized().ok_or(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        ))?;
        if let DynamicValue::Family(value) = source_value {
            let trace = scratch.trace_enabled.then(|| {
                scratch.trace.source(FamilyTraceSource {
                    rank,
                    footprint: FamilyTraceFootprint::Whole,
                    role: FamilyTraceRole::Authored,
                    occurrence: None,
                })
            });
            inputs.values.push(Some(TracedValue {
                value: value.clone(),
                trace,
            }));
            return Ok(None);
        }
        let candidate = verified_endpoint(address.clone(), source_value.clone(), rank, 1.0)?;
        scratch.tasks.push(BaseTask::CollectCoupled {
            index,
            cursor,
            step,
            inputs: std::mem::take(inputs),
            uses: uses.clone(),
        });
        let mut endpoint_uses = uses.clone();
        endpoint_uses.push(PreparedPositionStageUse::UnsupportedCoupledEndpoint);
        scratch.tasks.push(BaseTask::Cohort {
            end: cursor,
            candidates: vec![candidate],
            origins: vec![
                scratch
                    .source_origins
                    .get(index)
                    .cloned()
                    .unwrap_or_default(),
            ],
            uses: endpoint_uses,
        });
        Ok(Some(AdvanceProgress::Scheduled))
    }

    /// Completed Position nodes and observer bindings survive a materialization request.
    /// Color keeps its established ordinary evaluator; it has its own family continuation.
    pub(super) fn evaluate_base(
        &self,
        inputs: &mut BaseInputs,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
        matcher: &mut PreparedPositionStageMatcher<'_>,
    ) -> Result<EvaluatedBase, TransitionError> {
        let (index, rank, uses, expression) = (self.index, self.rank, &self.uses, &self.expression);
        let mut evaluation = inputs.evaluation.take();
        let mut observer_nodes = std::mem::take(&mut inputs.observer_nodes);
        let evaluated = {
            let endpoints = Endpoints { expression, inputs };
            let mut observer = scratch.trace_enabled.then(|| Observer {
                trace: &mut scratch.trace,
                nodes: &mut observer_nodes,
                rank,
                inputs,
                expression,
            });
            if expression.owner() == ProgrammingOwner::Position {
                if evaluation.is_none() {
                    evaluation = Some(expression.begin_position_evaluation()?);
                }
                let evaluation = evaluation.as_mut().expect("Position evaluation");
                let observer = observer
                    .as_mut()
                    .map(|value| value as &mut dyn CoupledEvaluationObserver);
                let reached_enabled = matcher.graph_reached_is_enabled();
                let graph_enabled = matcher.graph_is_enabled();
                let mut reached = |node, kind| {
                    matcher
                        .graph_reached_callback
                        .as_deref_mut()
                        .expect("enabled reached callback")(
                        &PreparedPositionGraphRoute {
                            uses: uses.clone(),
                            trigger_origins: scratch
                                .source_origins
                                .get(index)
                                .cloned()
                                .unwrap_or_default(),
                            expression: PreparedPositionGraphExpression::Coupled(
                                expression.clone(),
                            ),
                            node,
                            kind,
                        },
                    )
                };
                let reached: Option<&mut GraphOperationReachedCallback<'_>> = if reached_enabled {
                    Some(&mut reached)
                } else {
                    None
                };
                let progress = if graph_enabled {
                    evaluation
                        .advance_graph_operand_with_reached(&endpoints, frame, observer, reached)
                } else {
                    evaluation
                        .advance_with_reached(&endpoints, frame, observer, reached)
                        .map(GraphOperationProgress::Ordinary)
                };
                progress.map(|progress| match progress {
                    GraphOperationProgress::OperandReady(value) => {
                        matcher.completed_stop(
                            evaluation
                                .graph_selected_depth()
                                .expect("selected graph depth"),
                        );
                        GraphOperationProgress::OperandReady(value)
                    }
                    GraphOperationProgress::Ordinary(
                        crate::PositionEvaluationProgress::Complete(value),
                    ) => GraphOperationProgress::Ordinary(Ok(value)),
                    GraphOperationProgress::Ordinary(
                        crate::PositionEvaluationProgress::NeedsMaterialization(request),
                    ) => GraphOperationProgress::Ordinary(Err(request)),
                })
            } else {
                expression
                    .evaluate_base_observed(
                        &endpoints,
                        frame,
                        observer
                            .as_mut()
                            .map(|value| value as &mut dyn CoupledEvaluationObserver),
                    )
                    .and_then(|value| {
                        value.ok_or_else(|| {
                            IntentError("inactive coupled base entered composition".into()).into()
                        })
                    })
                    .map(|value| GraphOperationProgress::Ordinary(Ok(value)))
            }
        };
        inputs.evaluation = evaluation;
        inputs.observer_nodes = observer_nodes;
        evaluated
    }

    pub(super) fn request_materialization(
        &self,
        request: crate::PositionMaterializationRequest,
        step: usize,
        inputs: BaseInputs,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> AdvanceProgress {
        let (index, uses, expression) = (self.index, &self.uses, &self.expression);
        self.push_resume(step, inputs, scratch);
        let kind = match &request.operation {
            crate::PositionMaterializationOperation::Transition {
                reason: crate::DynamicTransitionReason::Required { .. },
                ..
            } => Some(GraphOperationKind::Required),
            crate::PositionMaterializationOperation::Transition {
                reason: crate::DynamicTransitionReason::Resume { .. },
                ..
            } => Some(GraphOperationKind::Resume),
            crate::PositionMaterializationOperation::Scale { .. } => Some(GraphOperationKind::Size),
            _ => None,
        };
        let graph_route = kind.map(|kind| PreparedPositionGraphRoute {
            uses: uses.clone(),
            trigger_origins: scratch
                .source_origins
                .get(index)
                .cloned()
                .unwrap_or_default(),
            expression: PreparedPositionGraphExpression::Coupled(expression.clone()),
            node: request.node,
            kind,
        });
        let mut request: base_evaluation::BaseMaterializationRequest = request.into();
        request.graph_route = graph_route;
        AdvanceProgress::NeedsMaterialization(request)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn complete_position(
        &self,
        target: AttributeValue,
        expression_trace: Option<FamilyTraceNodeId>,
        step: usize,
        mut inputs: BaseInputs,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Result<AdvanceProgress, TransitionError> {
        let (index, rank, activation_mix) = (self.index, self.rank, self.activation_mix);
        if inputs.completion.is_none() {
            inputs.completion = Some(position_completion::PositionSourceCompletion::new(
                TracedValue {
                    value: target,
                    trace: expression_trace,
                },
                rank,
                activation_mix,
                inputs.underlay.clone(),
                position_completion::PositionCompletionKind::Coupled,
            )?);
        }
        match inputs
            .completion
            .as_mut()
            .expect("Position completion")
            .advance(context, frame, &mut scratch.trace, scratch.trace_enabled)?
        {
            position_completion::CompletionProgress::Complete(value) => Ok(
                AdvanceProgress::Complete(completed_position_sample(index, value, scratch)?),
            ),
            position_completion::CompletionProgress::Needs(request) => {
                self.push_resume(step, inputs, scratch);
                Ok(AdvanceProgress::NeedsMaterialization(request.into()))
            }
        }
    }

    pub(super) fn complete_whole(
        &self,
        target: AttributeValue,
        expression_trace: Option<FamilyTraceNodeId>,
        inputs: &BaseInputs,
        context: &FamilyCompositionContext<'_>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        scratch: &mut RetainedFamilyCompositionScratch,
    ) -> Result<AdvanceProgress, TransitionError> {
        let (index, rank, activation_mix, expression) =
            (self.index, self.rank, self.activation_mix, &self.expression);
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
        let controlled = matches!(
            endpoint_output::control(rank, context),
            FamilyEndpointOutputControl::CrossfadeCurrent { .. }
        );
        let (value, activation_transfer) = if activation_mix == 1.0 {
            (target, None)
        } else {
            let from = &inputs
                .underlay
                .as_ref()
                .expect("coupled activation prefix")
                .value;
            if controlled {
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
                let prior = inputs
                    .underlay
                    .as_ref()
                    .and_then(|value| value.trace)
                    .expect("traced activation prefix");
                if controlled {
                    scratch
                        .trace
                        .mapped_blend(prior, incoming, activation_transfer)
                } else {
                    scratch
                        .trace
                        .write(prior, incoming, FamilyTraceFootprint::Whole, true)
                }
            })
        };
        let trace_node = trace_node.map(|appearance| {
            endpoint_output::control_trace(
                rank,
                FamilyTraceFootprint::Whole,
                appearance,
                inputs
                    .underlay
                    .as_ref()
                    .and_then(|value| value.trace)
                    .filter(|_| activation_mix < 1.0),
                context,
                &mut scratch.trace,
            )
        });
        let address = DynamicValueAddress::whole_family(expression.owner(), &value)?;
        let model = match &address.representation {
            DynamicFamilyRepresentation::DirectColor { source } => Some(
                endpoint_output::resolve_native_model(source, context, &|source| {
                    expression.resolve_original_native_model(source)
                })?,
            ),
            _ => None,
        };
        let mut sample = verified_endpoint(
            Arc::new(CompiledDynamicValueAddress::new(address, model)?),
            DynamicValue::Family(value),
            rank,
            1.0,
        )?;
        sample.trace_node = trace_node;
        scratch.resolved[index] = Some(sample.clone());
        Ok(AdvanceProgress::Complete(sample))
    }
}
