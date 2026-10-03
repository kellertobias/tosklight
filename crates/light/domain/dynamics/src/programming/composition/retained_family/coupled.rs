//! Endpoint requests run on the shared heap task stack. A component endpoint selects its
//! compatible lower cohort before editing, rather than converting an already winning rival.
use super::*;
use crate::{
    CoupledCohortEndpoint, CoupledComponentEndpoint, CoupledEvaluationObserver,
    CoupledEvaluationStep, CoupledExpressionContext, CoupledExpressionFootprint, CoupledLeafRole,
};

mod advance;
use advance::CoupledAdvance;

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
    let task = CoupledAdvance {
        index,
        cursor,
        expression,
        rank,
        activation_mix,
        uses,
    };
    if matcher.graph_is_enabled() && task.expression.owner() == ProgrammingOwner::Position {
        task.configure_graph(&mut inputs, scratch, matcher)?;
    }
    let selected_graph = inputs
        .evaluation
        .as_ref()
        .is_some_and(|evaluation| evaluation.graph_operand_selected());
    if step == 0 {
        if task.needs_underlay(&inputs, selected_graph) {
            return Ok(task.schedule_underlay(step, inputs, scratch));
        }
        step = 1;
    }
    while let Some(endpoint) = task.expression.base_endpoints().get(step - 1) {
        if let Some(progress) = task.advance_endpoint(
            endpoint,
            step,
            &mut inputs,
            context,
            frame,
            scratch,
            matcher,
        )? {
            return Ok(progress);
        }
        step += 1;
    }

    let target = match task.evaluate_base(&mut inputs, frame, scratch, matcher)? {
        GraphOperationProgress::OperandReady(value) => {
            scratch.tasks.push(BaseTask::Coupled {
                index,
                cursor,
                step,
                inputs,
                uses: task.uses,
            });
            return Ok(AdvanceProgress::OperandReady(value));
        }
        GraphOperationProgress::Ordinary(Ok(value)) => value,
        GraphOperationProgress::Ordinary(Err(request)) => {
            return Ok(task.request_materialization(request, step, inputs, scratch));
        }
    };
    let expression_trace = scratch.trace_enabled.then(|| {
        inputs.observer_nodes[task.expression.trace_root_node()].expect("evaluated coupled root")
    });
    if task.expression.owner() == ProgrammingOwner::Position {
        return task.complete_position(
            target,
            expression_trace,
            step,
            inputs,
            context,
            frame,
            scratch,
        );
    }
    task.complete_whole(target, expression_trace, &inputs, context, frame, scratch)
}
