//! Endpoint requests run on the shared heap task stack. A component endpoint selects its
//! compatible lower cohort before editing, rather than converting an already winning rival.
use super::*;
use crate::{
    CoupledCohortEndpoint, CoupledComponentEndpoint, CoupledEvaluationObserver,
    CoupledEvaluationStep, CoupledExpressionContext, CoupledExpressionFootprint, CoupledLeafRole,
};

#[derive(Default)]
pub(super) struct BaseInputs {
    pub underlay: Option<TracedValue>,
    pub values: Vec<Option<TracedValue>>,
    pub evaluation: Option<crate::PositionEvaluationContinuation>,
    pub observer_nodes: Vec<Option<FamilyTraceNodeId>>,
    pub source_cohort: Option<Box<SourceCohortEvaluation>>,
    pub completion: Option<position_completion::PositionSourceCompletion>,
}

pub(super) struct SourceCohortEvaluation {
    pub evaluation: base_evaluation::BaseEvaluation,
    pub scratch: Box<RetainedFamilyCompositionScratch>,
}

/// Exact cohorts emit their original member offset alongside each actual prepared sample.
/// None denotes the source graph itself; a member index never comes from rank/value matching.
pub(super) fn prepare_with_membership(
    expression: Arc<CompiledCoupledExpression>,
    rank: FamilySampleRank,
    activation_mix: f32,
    owner: ProgrammingOwner,
) -> Result<Vec<(FamilyCompositionSample, Option<usize>)>, TransitionError> {
    let identity = rank.dynamic_identity().ok_or_else(|| {
        IntentError("coupled Dynamic expression requires a Dynamic source identity".into())
    })?;
    ensure(
        expression.owner() == owner,
        "composition contains a different owner",
    )?;
    ensure(
        activation_mix.is_finite() && (0.0..=1.0).contains(&activation_mix),
        "Dynamic activation influence must be between zero and one",
    )?;
    if activation_mix == 0.0 {
        return Ok(Vec::new());
    }
    // Exact endpoints regain their original narrow footprint. An exact whole-to-Red release
    // must stop excluding lower compatible Green/Blue contributions immediately.
    let mut active_roles = expression.roles().iter().copied().filter(|role| {
        !matches!(
            expression.footprint(*role),
            CoupledExpressionFootprint::Inactive
        )
    });
    let Some(role) = active_roles.next() else {
        return Ok(Vec::new());
    };
    if active_roles.next().is_none() {
        if let CoupledExpressionFootprint::Exact { address, value } = expression.footprint(role) {
            let mut sample =
                verified_endpoint(address.clone(), value.clone(), rank, activation_mix)?;
            if !expression.exact_leaf_sources().is_empty() {
                sample.trace_sources = Some(trace_component_sources(
                    expression.exact_leaf_sources(),
                    rank,
                )?);
            } else if let Some((role, occurrence, dependency)) = expression.exact_leaf_provenance()
            {
                sample.trace_sources = Some(trace_component_sources(
                    &[CoupledComponentEndpoint {
                        lane_id: identity.lane_id,
                        address: address.clone(),
                        value: value.clone(),
                        role,
                        occurrence,
                        dependency_occurrence: dependency.cloned(),
                    }],
                    rank,
                )?);
            }
            return Ok(vec![(FamilyCompositionSample::Known(sample), None)]);
        }
        if activation_mix == 1.0
            && let CoupledExpressionFootprint::ExactCohort { components } =
                expression.footprint(role)
        {
            return components
                .iter()
                .enumerate()
                .map(|(index, component)| {
                    verified_component(component, rank, activation_mix)
                        .map(|sample| (FamilyCompositionSample::Known(sample), Some(index)))
                })
                .collect();
        }
        if activation_mix == 1.0
            && let CoupledExpressionFootprint::ExactSources { sources } = expression.footprint(role)
        {
            return sources
                .iter()
                .enumerate()
                .map(|(index, source)| {
                    let sample = match source {
                        crate::CoupledCohortEndpoint::Materialized(component) => {
                            verified_component(component, rank, 1.0)
                                .map(FamilyCompositionSample::Known)
                        }
                        crate::CoupledCohortEndpoint::WholeExpression {
                            lane_id,
                            expression,
                        } => Ok(FamilyCompositionSample::WholeExpression {
                            expression: expression.clone(),
                            rank: rank.with_dynamic_lane(*lane_id)?,
                            activation_mix: 1.0,
                        }),
                    }?;
                    Ok((sample, Some(index)))
                })
                .collect();
        }
    }
    Ok(vec![(
        FamilyCompositionSample::CoupledExpression {
            expression,
            rank,
            activation_mix,
        },
        None,
    )])
}

pub(super) fn verified_component(
    component: &CoupledComponentEndpoint,
    rank: FamilySampleRank,
    activation_mix: f32,
) -> Result<FamilySample, TransitionError> {
    let rank = rank.with_dynamic_lane(component.lane_id)?;
    let mut sample = verified_endpoint(
        component.address.clone(),
        component.value.clone(),
        rank,
        activation_mix,
    )?;
    sample.trace_sources = Some(trace_component_sources(
        std::slice::from_ref(component),
        rank,
    )?);
    Ok(sample)
}

fn trace_component_sources(
    components: &[CoupledComponentEndpoint],
    rank: FamilySampleRank,
) -> Result<Arc<[super::super::trace::FamilyTraceLeaf]>, IntentError> {
    use super::super::trace::FamilyTraceLeaf;
    let identity = rank.dynamic_identity().ok_or_else(|| {
        IntentError("coupled Dynamic trace requires a Dynamic source identity".into())
    })?;
    Ok(components
        .iter()
        .flat_map(|component| {
            let mut sources = Vec::with_capacity(2);
            let footprint = component
                .address
                .address()
                .component
                .map_or(FamilyTraceFootprint::Whole, FamilyTraceFootprint::Component);
            let rank = FamilySampleRank {
                identity: FamilySampleIdentity::Dynamic {
                    instance_id: identity.instance_id,
                    controller_id: identity.controller_id,
                    lane_id: component.lane_id,
                },
                ..rank
            };
            let source = FamilyTraceSource {
                rank,
                footprint,
                role: match component.role {
                    CoupledLeafRole::Authored => FamilyTraceRole::Authored,
                    CoupledLeafRole::Current => FamilyTraceRole::CalculationDependency,
                },
                occurrence: component.occurrence,
            };
            if component.role == CoupledLeafRole::Authored {
                sources.push(FamilyTraceLeaf::Source(source));
            }
            if let Some(dependency) = component.dependency_occurrence.clone().or_else(|| {
                (component.role == CoupledLeafRole::Current)
                    .then(|| crate::DynamicSourceDependency::unknown(component.occurrence))
            }) {
                sources.push(FamilyTraceLeaf::Current(source, dependency));
            }
            sources
        })
        .collect::<Vec<_>>()
        .into())
}

pub(super) fn verified_endpoint(
    address: Arc<CompiledDynamicValueAddress>,
    value: DynamicValue,
    rank: FamilySampleRank,
    activation_mix: f32,
) -> Result<FamilySample, TransitionError> {
    address.validate_value(&value)?;
    Ok(FamilySample {
        address,
        body: FamilySampleBody::Materialized(value),
        projection: None,
        fix_at: false,
        endpoint_output_exempt: false,
        trace_sources: None,
        trace_node: None,
        rank,
        activation_mix,
    })
}

pub(super) struct Endpoints<'a> {
    pub(super) expression: &'a CompiledCoupledExpression,
    pub(super) inputs: &'a BaseInputs,
}
impl CoupledExpressionContext for Endpoints<'_> {
    fn materialize_base(
        &self,
        endpoint: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError> {
        match endpoint {
            Some((address, value)) => self
                .expression
                .base_endpoints()
                .iter()
                .position(|endpoint| {
                    endpoint
                        .materialized()
                        .is_some_and(|(known_address, known_value)| {
                            known_address.address() == address.address() && known_value == value
                        })
                })
                .and_then(|index| self.inputs.values.get(index))
                .and_then(Option::as_ref)
                .map(|value| value.value.clone()),
            None => self
                .inputs
                .underlay
                .as_ref()
                .map(|value| value.value.clone()),
        }
        .ok_or(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints,
        ))
    }
    fn orthogonal_underlay(
        &self,
        _: ColorComponent,
        _: &AttributeValue,
    ) -> Result<f32, TransitionError> {
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        ))
    }
    fn materialize_cohort(
        &self,
        components: &[CoupledComponentEndpoint],
    ) -> Result<AttributeValue, TransitionError> {
        self.expression
            .base_endpoints()
            .iter()
            .position(|endpoint| {
                endpoint
                    .cohort()
                    .is_some_and(|known| std::ptr::eq(known, components))
            })
            .and_then(|index| self.inputs.values.get(index))
            .and_then(Option::as_ref)
            .map(|value| value.value.clone())
            .ok_or(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ))
    }
    fn materialize_cohort_sources(
        &self,
        sources: &[CoupledCohortEndpoint],
    ) -> Result<AttributeValue, TransitionError> {
        self.expression
            .base_endpoints()
            .iter()
            .position(|endpoint| {
                endpoint
                    .cohort_sources()
                    .is_some_and(|known| std::ptr::eq(known, sources))
            })
            .and_then(|index| self.inputs.values.get(index))
            .and_then(Option::as_ref)
            .map(|value| value.value.clone())
            .ok_or(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ))
    }
}

pub(super) struct Observer<'a> {
    pub(super) trace: &'a mut FamilyTraceArena,
    pub(super) nodes: &'a mut Vec<Option<FamilyTraceNodeId>>,
    pub(super) rank: FamilySampleRank,
    pub(super) inputs: &'a BaseInputs,
    pub(super) expression: &'a CompiledCoupledExpression,
}
impl CoupledEvaluationObserver for Observer<'_> {
    fn evaluated(
        &mut self,
        node: usize,
        step: CoupledEvaluationStep<'_>,
        _: &AttributeValue,
    ) -> Result<(), TransitionError> {
        if self.nodes.len() <= node {
            self.nodes.resize(node + 1, None);
        }
        let id = match step {
            CoupledEvaluationStep::Underlay => self
                .inputs
                .underlay
                .as_ref()
                .and_then(|value| value.trace)
                .ok_or(TransitionError::Requires(
                    TransitionRequirement::MaterializedEndpoints,
                ))?,
            CoupledEvaluationStep::AnglePair { sources } => {
                let leaves = trace_component_sources(sources, self.rank)?
                    .iter()
                    .map(|source| self.trace.leaf(source.clone()))
                    .collect();
                self.trace.bundle(leaves)
            }
            CoupledEvaluationStep::Leaf {
                address,
                sources,
                role,
                occurrence,
                dependency_occurrence,
                ..
            } => {
                if sources.is_empty() {
                    let footprint = address
                        .address()
                        .component
                        .map_or(FamilyTraceFootprint::Whole, FamilyTraceFootprint::Component);
                    let source = FamilyTraceSource {
                        rank: self.rank,
                        footprint,
                        role: match role {
                            CoupledLeafRole::Authored => FamilyTraceRole::Authored,
                            CoupledLeafRole::Current => FamilyTraceRole::CalculationDependency,
                        },
                        occurrence,
                    };
                    if role == CoupledLeafRole::Current {
                        self.trace.current_source(
                            source,
                            dependency_occurrence.unwrap_or_else(|| {
                                crate::DynamicSourceDependency::unknown(occurrence)
                            }),
                        )
                    } else if let Some(dependency) = dependency_occurrence {
                        let authored = self.trace.source(source);
                        let dependency = self.trace.current_source(source, dependency);
                        self.trace.bundle(vec![authored, dependency])
                    } else {
                        self.trace.source(source)
                    }
                } else {
                    let leaves = trace_component_sources(sources, self.rank)?
                        .iter()
                        .map(|source| self.trace.leaf(source.clone()))
                        .collect();
                    self.trace.bundle(leaves)
                }
            }
            CoupledEvaluationStep::Cohort { components } => {
                let index = self
                    .expression
                    .base_endpoints()
                    .iter()
                    .position(|endpoint| {
                        endpoint
                            .cohort()
                            .is_some_and(|known| std::ptr::eq(known, components))
                    })
                    .ok_or(TransitionError::Requires(
                        TransitionRequirement::MaterializedEndpoints,
                    ))?;
                self.inputs
                    .values
                    .get(index)
                    .and_then(Option::as_ref)
                    .and_then(|value| value.trace)
                    .ok_or(TransitionError::Requires(
                        TransitionRequirement::MaterializedEndpoints,
                    ))?
            }
            CoupledEvaluationStep::SourceCohort { sources } => {
                let index = self
                    .expression
                    .base_endpoints()
                    .iter()
                    .position(|endpoint| {
                        endpoint
                            .cohort_sources()
                            .is_some_and(|known| std::ptr::eq(known, sources))
                    })
                    .ok_or(TransitionError::Requires(
                        TransitionRequirement::MaterializedEndpoints,
                    ))?;
                self.inputs
                    .values
                    .get(index)
                    .and_then(Option::as_ref)
                    .and_then(|value| value.trace)
                    .ok_or(TransitionError::Requires(
                        TransitionRequirement::MaterializedEndpoints,
                    ))?
            }
            CoupledEvaluationStep::Transition { from, to, .. } => self.trace.blend(
                self.nodes[from].expect("compiled outgoing coupled node"),
                self.nodes[to].expect("compiled incoming coupled node"),
            ),
            CoupledEvaluationStep::Scale {
                value,
                sources,
                baseline_occurrence,
                ..
            } => {
                let baseline = if sources.is_empty() {
                    self.trace.source(FamilyTraceSource {
                        rank: self.rank,
                        footprint: FamilyTraceFootprint::Whole,
                        role: FamilyTraceRole::CalculationDependency,
                        occurrence: baseline_occurrence,
                    })
                } else {
                    let leaves = trace_component_sources(sources, self.rank)?
                        .iter()
                        .map(|source| {
                            let leaf = self.trace.leaf(source.clone());
                            self.trace.dependency(leaf)
                        })
                        .collect();
                    self.trace.bundle(leaves)
                };
                self.trace
                    .blend(baseline, self.nodes[value].expect("compiled Size child"))
            }
        };
        self.nodes[node] = Some(id);
        Ok(())
    }
}

pub(super) enum AdvanceProgress {
    Scheduled,
    Complete(FamilySample),
    NeedsMaterialization(base_evaluation::BaseMaterializationRequest),
    OperandReady(AttributeValue),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn advance_with_stage(
    index: usize,
    cursor: usize,
    mut step: usize,
    mut inputs: BaseInputs,
    uses: Vec<PreparedPositionStageUse>,
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    scratch: &mut RetainedFamilyCompositionScratch,
    matcher: &mut PreparedPositionStageMatcher<'_>,
) -> Result<AdvanceProgress, TransitionError> {
    let FamilyCompositionSample::CoupledExpression {
        expression,
        rank,
        activation_mix,
    } = scratch.sources[index].clone()
    else {
        unreachable!("coupled source task")
    };
    if matcher.graph_is_enabled() && expression.owner() == ProgrammingOwner::Position {
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
    }
    let selected_graph = inputs
        .evaluation
        .as_ref()
        .is_some_and(|evaluation| evaluation.graph_operand_selected());
    if step == 0 {
        if (if selected_graph {
            inputs
                .evaluation
                .as_ref()
                .unwrap()
                .graph_operand_needs_underlay()
        } else {
            expression.needs_base_underlay()
        }) || (!selected_graph && activation_mix < 1.0)
            || expression
                .base_endpoints()
                .iter()
                .enumerate()
                .any(|(endpoint_index, endpoint)| {
                    endpoint.cohort_sources().is_some()
                        && inputs.evaluation.as_ref().map_or(true, |evaluation| {
                            evaluation.graph_operand_endpoint_needed(endpoint_index)
                        })
                })
        {
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
            return Ok(AdvanceProgress::Scheduled);
        }
        step = 1;
    }
    while let Some(endpoint) = expression.base_endpoints().get(step - 1) {
        if matcher.graph_is_enabled()
            && !inputs
                .evaluation
                .as_ref()
                .unwrap()
                .graph_operand_endpoint_needed(step - 1)
        {
            inputs.values.push(None);
            step += 1;
            continue;
        }
        if let Some(sources) = endpoint.cohort_sources() {
            // The child's prepared sources, lower prefix, trace and heap tasks survive
            // suspension. Its endpoint's original Arc remains in the parent's expression.
            if inputs.source_cohort.is_none() {
                let underlay = inputs
                    .underlay
                    .as_ref()
                    .expect("source cohort lower prefix");
                let mut candidates = std::mem::take(&mut scratch.source_cohort_candidates);
                candidates.clear();
                let prepared = sources.iter().try_for_each(|source| {
                    let candidate = match source {
                        CoupledCohortEndpoint::Materialized(component) => {
                            FamilyCompositionSample::Known(verified_component(
                                component, rank, 1.0,
                            )?)
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
            }
            let mut child = inputs.source_cohort.take().expect("retained source cohort");
            match child.evaluation.advance_with_stage(
                &endpoint_output::with_control(context, None),
                frame,
                &mut child.scratch,
                matcher,
            ) {
                Ok(base_evaluation::BaseEvaluationProgress::OperandReady(value)) => {
                    inputs.source_cohort = Some(child);
                    scratch.tasks.push(BaseTask::Coupled {
                        index,
                        cursor,
                        step,
                        inputs,
                        uses: uses.clone(),
                    });
                    return Ok(AdvanceProgress::OperandReady(value));
                }
                Ok(base_evaluation::BaseEvaluationProgress::Pending) => {
                    inputs.source_cohort = Some(child);
                    scratch.tasks.push(BaseTask::Coupled {
                        index,
                        cursor,
                        step,
                        inputs,
                        uses: uses.clone(),
                    });
                    return Ok(AdvanceProgress::Scheduled);
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
                    scratch.tasks.push(BaseTask::Coupled {
                        index,
                        cursor,
                        step,
                        inputs,
                        uses: uses.clone(),
                    });
                    return Ok(AdvanceProgress::NeedsMaterialization(request));
                }
                Ok(base_evaluation::BaseEvaluationProgress::Complete(resolved)) => {
                    if let Some(callback) = matcher.cohort_callback.as_deref_mut() {
                        if let Some(depth) = callback(
                            &uses,
                            scratch
                                .source_origins
                                .get(index)
                                .map(Vec::as_slice)
                                .unwrap_or_default(),
                            &expression,
                            expression.base_endpoint_nodes(step - 1).unwrap_or_default(),
                        )? {
                            let value = resolved.value.clone();
                            inputs.source_cohort = Some(child);
                            scratch.tasks.push(BaseTask::Coupled {
                                index,
                                cursor,
                                step,
                                inputs,
                                uses: uses.clone(),
                            });
                            matcher.completed_stop(depth);
                            return Ok(AdvanceProgress::OperandReady(value));
                        }
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
                    step += 1;
                    continue;
                }
                Err(error) => {
                    child.evaluation.recycle(&mut child.scratch);
                    scratch.source_cohort_scratch = Some(child.scratch);
                    return Err(error);
                }
            }
        }
        if let Some(components) = endpoint.cohort() {
            let candidates = components
                .iter()
                .map(|component| verified_component(component, rank, 1.0))
                .collect::<Result<Vec<_>, _>>()?;
            scratch.tasks.push(BaseTask::CollectCoupled {
                index,
                cursor,
                step,
                inputs,
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
            return Ok(AdvanceProgress::Scheduled);
        }
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
            step += 1;
            continue;
        }
        let candidate = verified_endpoint(address.clone(), source_value.clone(), rank, 1.0)?;
        scratch.tasks.push(BaseTask::CollectCoupled {
            index,
            cursor,
            step,
            inputs,
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
        return Ok(AdvanceProgress::Scheduled);
    }

    // Completed Position nodes and observer bindings survive a materialization request.
    // Color keeps its established ordinary evaluator; it has its own family continuation.
    let mut evaluation = inputs.evaluation.take();
    let mut observer_nodes = std::mem::take(&mut inputs.observer_nodes);
    let evaluated = {
        let endpoints = Endpoints {
            expression: &expression,
            inputs: &inputs,
        };
        let mut observer = scratch.trace_enabled.then(|| Observer {
            trace: &mut scratch.trace,
            nodes: &mut observer_nodes,
            rank,
            inputs: &inputs,
            expression: &expression,
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
                        expression: PreparedPositionGraphExpression::Coupled(expression.clone()),
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
                evaluation.advance_graph_operand_with_reached(&endpoints, frame, observer, reached)
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
                GraphOperationProgress::Ordinary(crate::PositionEvaluationProgress::Complete(
                    value,
                )) => GraphOperationProgress::Ordinary(Ok(value)),
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
    let target = match evaluated? {
        GraphOperationProgress::OperandReady(value) => {
            scratch.tasks.push(BaseTask::Coupled {
                index,
                cursor,
                step,
                inputs,
                uses,
            });
            return Ok(AdvanceProgress::OperandReady(value));
        }
        GraphOperationProgress::Ordinary(Ok(value)) => value,
        GraphOperationProgress::Ordinary(Err(request)) => {
            scratch.tasks.push(BaseTask::Coupled {
                index,
                cursor,
                step,
                inputs,
                uses: uses.clone(),
            });
            let kind = match &request.operation {
                crate::PositionMaterializationOperation::Transition {
                    reason: crate::DynamicTransitionReason::Required { .. },
                    ..
                } => Some(GraphOperationKind::Required),
                crate::PositionMaterializationOperation::Transition {
                    reason: crate::DynamicTransitionReason::Resume { .. },
                    ..
                } => Some(GraphOperationKind::Resume),
                crate::PositionMaterializationOperation::Scale { .. } => {
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
                expression: PreparedPositionGraphExpression::Coupled(expression.clone()),
                node: request.node,
                kind,
            });
            let mut request: base_evaluation::BaseMaterializationRequest = request.into();
            request.graph_route = graph_route;
            return Ok(AdvanceProgress::NeedsMaterialization(request));
        }
    };
    let expression_trace = scratch.trace_enabled.then(|| {
        inputs.observer_nodes[expression.trace_root_node()].expect("evaluated coupled root")
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
                position_completion::PositionCompletionKind::Coupled,
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
                scratch.tasks.push(BaseTask::Coupled {
                    index,
                    cursor,
                    step,
                    inputs,
                    uses: uses.clone(),
                });
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
