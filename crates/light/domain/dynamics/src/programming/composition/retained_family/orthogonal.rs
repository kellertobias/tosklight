//! Explicit Color controls keep the implicit defaults of their own transition branches.
//! A branch query conditions the complete base stack, including higher contributors and
//! sibling lanes from the same Resume occurrence. It never reads a fitted fixture channel.
use super::conditioning::{BranchState, condition_sources};
use super::*;
use crate::{CoupledExpressionRole, DynamicSourceOccurrenceId, DynamicTransitionReason};
use std::collections::HashMap;

#[derive(Eq, Hash, PartialEq)]
struct QueryKey {
    decisions: Vec<(Uuid, Uuid, Uuid, bool)>,
    overrides: Vec<(usize, Option<usize>)>,
}

impl From<&BranchState> for QueryKey {
    fn from(state: &BranchState) -> Self {
        let mut decisions = state
            .decisions
            .iter()
            .map(|choice| {
                (
                    choice.instance_id,
                    choice.controller_id,
                    choice.occurrence_id,
                    choice.incoming,
                )
            })
            .collect::<Vec<_>>();
        let mut overrides = state
            .overrides
            .iter()
            .map(|(index, node)| (*index, node.as_ref().map(|node| Arc::as_ptr(node) as usize)))
            .collect::<Vec<_>>();
        decisions.sort_unstable();
        overrides.sort_unstable();
        Self {
            decisions,
            overrides,
        }
    }
}

struct Query {
    // Keep Arc identities alive for the pointer keys throughout this coherent-frame query.
    state: BranchState,
    sources: Vec<Option<FamilyCompositionSample>>,
    base: Option<TracedValue>,
}

struct ScalarTrace {
    value: DynamicValue,
    trace: Option<FamilyTraceNodeId>,
}

struct Queries<'a, 'b> {
    source_base: &'a AttributeValue,
    source_base_trace: Option<FamilyTraceNodeId>,
    original_base: &'a AttributeValue,
    sources: &'a [FamilyCompositionSample],
    context: &'a FamilyCompositionContext<'b>,
    frame: &'a dyn WholeFamilyExpressionFrameResolver,
    keys: HashMap<QueryKey, usize>,
    entries: Vec<Query>,
    scratch: RetainedFamilyCompositionScratch,
    trace: Option<&'a mut FamilyTraceArena>,
}

impl Queries<'_, '_> {
    fn leaf_trace(
        &mut self,
        index: usize,
        component: ColorComponent,
        occurrence: Option<DynamicSourceOccurrenceId>,
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    ) -> Option<FamilyTraceNodeId> {
        let rank = self.sources[index].rank();
        self.trace.as_deref_mut().map(|trace| {
            let authored = trace.source(FamilyTraceSource {
                rank,
                footprint: FamilyTraceFootprint::Component(ProgrammingComponent::Color(component)),
                role: FamilyTraceRole::Authored,
                occurrence,
            });
            if let Some(dependency) = dependency_occurrence {
                let source = FamilyTraceSource {
                    rank,
                    footprint: FamilyTraceFootprint::Component(ProgrammingComponent::Color(
                        component,
                    )),
                    role: FamilyTraceRole::CalculationDependency,
                    occurrence: dependency.occurrence,
                };
                let dependency = trace.current_source(source, dependency);
                trace.bundle(vec![authored, dependency])
            } else {
                authored
            }
        })
    }
    fn insert(&mut self, state: BranchState) -> Result<usize, TransitionError> {
        let key = QueryKey::from(&state);
        if let Some(index) = self.keys.get(&key) {
            return Ok(*index);
        }
        let sources = condition_sources(self.sources, &state, self.source_base)?;
        let index = self.entries.len();
        self.entries.push(Query {
            state,
            sources,
            base: None,
        });
        self.keys.insert(key, index);
        Ok(index)
    }

    fn implicit(
        &mut self,
        query: usize,
        component: ColorComponent,
        address: &CompiledDynamicValueAddress,
    ) -> Result<ScalarTrace, TransitionError> {
        if self.entries[query].base.is_none() {
            // A conditioned branch reconstructs this source's raw endpoint. Lower sources
            // retain their real controls, while overridden sources are enveloped at their root.
            let control = |rank| {
                if self.entries[query]
                    .state
                    .overrides
                    .iter()
                    .any(|(index, _)| self.sources[*index].rank() == rank)
                {
                    FamilyEndpointOutputControl::Unchanged
                } else {
                    endpoint_output::control(rank, self.context)
                }
            };
            let output = self
                .context
                .endpoint_output
                .map(|output| FamilyEndpointOutputContext {
                    control: &control,
                    ..output
                });
            let context = endpoint_output::with_control(self.context, output);
            let value = compose_family_inputs_impl(
                ProgrammingOwner::Color,
                self.source_base,
                self.entries[query].sources.iter().flatten().cloned(),
                &context,
                self.frame,
                Some(self.original_base),
                &mut self.scratch,
                self.trace.is_some(),
            )?;
            let trace = match (
                self.trace.as_deref_mut(),
                value.trace,
                self.source_base_trace,
            ) {
                (Some(arena), Some(root), Some(base)) => {
                    Some(arena.append_graph_rebased(&self.scratch.trace, root, base))
                }
                _ => None,
            };
            self.entries[query].base = Some(TracedValue {
                value: value.value,
                trace,
            });
        }
        let base = self.entries[query]
            .base
            .as_ref()
            .expect("resolved query base");
        // UV is independent from visible appearance. An unknown portable UV prediction must
        // remain unavailable even when a visible-color prediction happens to be present.
        if component == ColorComponent::Uv
            && let AttributeValue::ColorProgram(color) = &base.value
            && let ColorProgram::Direct { portable, .. } = color.as_ref()
        {
            return portable
                .uv
                .map(|uv| ScalarTrace {
                    value: DynamicValue::Scalar(uv.amount),
                    trace: base.trace.map(|root| {
                        self.trace
                            .as_deref_mut()
                            .expect("traced UV query")
                            .dependency(root)
                    }),
                })
                .ok_or(TransitionError::Requires(
                    TransitionRequirement::ColorAppearance,
                ));
        }
        let adopted = adopt(
            base.value.clone(),
            address.address(),
            self.context,
            self.original_base,
        )?;
        let value =
            extract_compatible_dynamic_value(&adopted, address.address(), &self.context.edit)?
                .ok_or(TransitionError::Requires(
                    TransitionRequirement::ColorAppearance,
                ))?;
        let trace = base.trace.map(|root| {
            self.trace
                .as_deref_mut()
                .expect("traced implicit query")
                .dependency(root)
        });
        Ok(ScalarTrace { value, trace })
    }
}

enum Task {
    Prefix {
        end: usize,
        query: usize,
    },
    Node {
        index: usize,
        node: Option<Arc<DynamicSampleExpression>>,
        query: usize,
        raw: bool,
    },
    Output {
        index: usize,
        activation: f32,
    },
    RawKnown(FamilySample),
    Ownership {
        index: usize,
        activation: f32,
    },
    ApplyKnown(FamilySample),
    ApplyLeaf {
        value: DynamicValue,
        activation: f32,
        index: usize,
        occurrence: Option<DynamicSourceOccurrenceId>,
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    Mix(f32),
}

fn activation(sample: &FamilyCompositionSample) -> f32 {
    match sample {
        FamilyCompositionSample::Known(sample) => sample.activation_mix,
        FamilyCompositionSample::WholeExpression { activation_mix, .. }
        | FamilyCompositionSample::CoupledExpression { activation_mix, .. } => *activation_mix,
    }
}

fn has_explicit(node: &DynamicSampleExpression, component: ColorComponent) -> bool {
    use crate::programming::expression::{ExpressionNode, ExpressionNodeRef};
    ExpressionNodeRef::new(node)
        .postorder(true)
        .is_ok_and(|nodes| {
            nodes.into_iter().any(|node| {
                matches!(node.node(), Ok(ExpressionNode::Programming(address, ..))
            if address.component == Some(ProgrammingComponent::Color(component)))
            })
        })
}

/// Use a heap task stack for the source-prefix walk. The number of simultaneously active
/// effects must not consume the output thread's Rust call stack.
fn evaluate(
    component: ColorComponent,
    address: &CompiledDynamicValueAddress,
    initial: usize,
    queries: &mut Queries<'_, '_>,
) -> Result<ScalarTrace, TransitionError> {
    let mut tasks = vec![Task::Prefix {
        end: queries.sources.len(),
        query: initial,
    }];
    let mut values = Vec::new();
    while let Some(task) = tasks.pop() {
        match task {
            Task::Prefix { mut end, query } => {
                let mut found = false;
                while end > 0 {
                    end -= 1;
                    let Some(sample) = queries.entries[query].sources[end].clone() else {
                        continue;
                    };
                    match sample {
                        FamilyCompositionSample::Known(sample)
                            if sample.address.address().component
                                == Some(ProgrammingComponent::Color(component))
                                && sample.participates() =>
                        {
                            let controlled = matches!(sample.endpoint_control(queries.context), FamilyEndpointOutputControl::CrossfadeCurrent { mix } if mix < 1.0);
                            if !controlled {
                                tasks.push(Task::Ownership {
                                    index: end,
                                    activation: sample.activation_mix,
                                });
                            }
                            if controlled {
                                tasks.push(Task::Output {
                                    index: end,
                                    activation: sample.activation_mix,
                                });
                                match &sample.body {
                                    FamilySampleBody::ComponentExpression(expression) => tasks
                                        .push(Task::Node {
                                            index: end,
                                            node: Some(Arc::new(expression.expression().clone())),
                                            query,
                                            raw: true,
                                        }),
                                    FamilySampleBody::Materialized(_) => {
                                        tasks.push(Task::RawKnown(sample.clone()))
                                    }
                                }
                                if sample.activation_mix < 1.0 {
                                    tasks.push(Task::Prefix { end, query });
                                }
                            } else if !sample.component_needs_underlay() {
                                let (value, trace) = component_value_and_trace(
                                    &sample,
                                    None,
                                    queries.trace.as_deref_mut(),
                                    None,
                                )?;
                                values.push(ScalarTrace { value, trace });
                            } else if let FamilySampleBody::ComponentExpression(expression) =
                                &sample.body
                            {
                                // A retained component may share a Resume branch with a base
                                // writer in a sibling lane. Keep that occurrence correlated.
                                tasks.push(Task::Node {
                                    index: end,
                                    node: Some(Arc::new(expression.expression().clone())),
                                    query,
                                    raw: false,
                                });
                            } else {
                                tasks.push(Task::ApplyKnown(sample));
                                tasks.push(Task::Prefix { end, query });
                            }
                            found = true;
                            break;
                        }
                        FamilyCompositionSample::CoupledExpression {
                            expression,
                            activation_mix,
                            ..
                        } if activation_mix > 0.0 => {
                            let root = expression.expression().cloned().ok_or(
                                TransitionError::Requires(TransitionRequirement::CompatibleOwners),
                            )?;
                            if has_explicit(&root, component) {
                                let controlled = matches!(queries.sources[end].endpoint_control(queries.context), FamilyEndpointOutputControl::CrossfadeCurrent { mix } if mix < 1.0);
                                if !controlled {
                                    tasks.push(Task::Ownership {
                                        index: end,
                                        activation: activation_mix,
                                    });
                                }
                                if controlled {
                                    tasks.push(Task::Output {
                                        index: end,
                                        activation: activation_mix,
                                    });
                                }
                                tasks.push(Task::Node {
                                    index: end,
                                    node: Some(root),
                                    query,
                                    raw: controlled,
                                });
                                if controlled && activation_mix < 1.0 {
                                    tasks.push(Task::Prefix { end, query });
                                }
                                found = true;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if !found {
                    values.push(queries.implicit(query, component, address)?);
                }
            }
            Task::Node {
                index,
                node,
                query,
                raw,
            } => {
                let mut state = queries.entries[query].state.clone();
                state.replace(index, node.clone());
                if !node
                    .as_ref()
                    .is_some_and(|node| has_explicit(node, component))
                {
                    let query = queries.insert(state)?;
                    tasks.push(Task::Prefix { end: index, query });
                    continue;
                }
                let node = Arc::new(node.expect("explicit subtree").shallow()?);
                match node.as_ref() {
                    DynamicSampleExpression::Programming {
                        value,
                        occurrence,
                        dependency_occurrence,
                        ..
                    } => {
                        let activation = if raw {
                            1.0
                        } else {
                            activation(&queries.sources[index])
                        };
                        if activation == 1.0 {
                            values.push(ScalarTrace {
                                value: value.clone(),
                                trace: queries.leaf_trace(
                                    index,
                                    component,
                                    *occurrence,
                                    dependency_occurrence.clone(),
                                ),
                            });
                        } else {
                            let query = queries.insert(state)?;
                            tasks.push(Task::ApplyLeaf {
                                value: value.clone(),
                                activation,
                                index,
                                occurrence: *occurrence,
                                dependency_occurrence: dependency_occurrence.clone(),
                            });
                            tasks.push(Task::Prefix { end: index, query });
                        }
                    }
                    DynamicSampleExpression::Transition {
                        from,
                        to,
                        progress,
                        reason,
                    } => {
                        let branches: &[(bool, &Option<Arc<DynamicSampleExpression>>)] =
                            if *progress == 0.0 {
                                &[(false, from)]
                            } else if *progress == 1.0 {
                                &[(true, to)]
                            } else {
                                tasks.push(Task::Mix(*progress));
                                &[(true, to), (false, from)]
                            };
                        for &(incoming, child) in branches {
                            let mut state = state.clone();
                            if let DynamicTransitionReason::Resume { occurrence_id } = reason {
                                state.choose(
                                    queries.sources[index].rank(),
                                    *occurrence_id,
                                    incoming,
                                )?;
                            }
                            state.replace(index, child.clone());
                            let query = queries.insert(state)?;
                            tasks.push(Task::Node {
                                index,
                                node: child.clone(),
                                query,
                                raw,
                            });
                        }
                    }
                    _ => {
                        return Err(IntentError(
                            "explicit Color role has an invalid retained node".into(),
                        )
                        .into());
                    }
                }
            }
            Task::Ownership { index, activation } => {
                let value = values.last_mut().expect("completed source root");
                if let Some(trace) = queries.trace.as_deref_mut() {
                    let appearance = value.trace.expect("traced source root");
                    value.trace = Some(trace.control(
                        queries.sources[index].rank(),
                        FamilyTraceFootprint::Component(ProgrammingComponent::Color(component)),
                        appearance,
                        (activation < 1.0).then_some(appearance),
                    ));
                }
            }
            Task::RawKnown(sample) => {
                let (value, trace) =
                    component_value_and_trace(&sample, None, queries.trace.as_deref_mut(), None)?;
                values.push(ScalarTrace { value, trace });
            }
            Task::Output { index, activation } => {
                let endpoint = values.pop().expect("completed raw endpoint");
                let (value, appearance) = endpoint_output::component(
                    queries.sources[index].rank(),
                    address,
                    endpoint.value,
                    endpoint.trace,
                    queries.context,
                    queries.trace.as_deref_mut(),
                )?;
                let (value, appearance, prior) = if activation < 1.0 {
                    let underlay = values.pop().expect("controlled activation prefix");
                    let value = address
                        .transition(underlay.value, value)?
                        .sample(activation)?;
                    let appearance = queries.trace.as_deref_mut().map(|trace| {
                        trace.blend(
                            underlay.trace.expect("traced prefix"),
                            appearance.expect("traced envelope"),
                        )
                    });
                    (value, appearance, underlay.trace)
                } else {
                    (value, appearance, None)
                };
                let trace = queries.trace.as_deref_mut().map(|trace| {
                    endpoint_output::control_trace(
                        queries.sources[index].rank(),
                        FamilyTraceFootprint::Component(ProgrammingComponent::Color(component)),
                        appearance.expect("traced result"),
                        prior,
                        queries.context,
                        trace,
                    )
                });
                values.push(ScalarTrace { value, trace });
            }
            Task::ApplyKnown(sample) => {
                let underlay = values.pop().expect("completed component prefix");
                let target = sample.component_value(Some(&underlay.value))?;
                let value = address
                    .transition(underlay.value, target)?
                    .sample(sample.activation_mix)?;
                let trace = queries.trace.as_deref_mut().map(|arena| {
                    let incoming = sample_trace(&sample, arena);
                    arena.blend(underlay.trace.expect("traced Color underlay"), incoming)
                });
                values.push(ScalarTrace { value, trace });
            }
            Task::ApplyLeaf {
                value,
                activation,
                index,
                occurrence,
                dependency_occurrence,
            } => {
                let underlay = values.pop().expect("completed explicit leaf prefix");
                let value = address
                    .transition(underlay.value, value)?
                    .sample(activation)?;
                let incoming =
                    queries.leaf_trace(index, component, occurrence, dependency_occurrence);
                let trace = queries.trace.as_deref_mut().map(|arena| {
                    arena.blend(
                        underlay.trace.expect("traced Color underlay"),
                        incoming.expect("traced Color leaf"),
                    )
                });
                values.push(ScalarTrace { value, trace });
            }
            Task::Mix(progress) => {
                let to = values.pop().expect("completed incoming branch");
                let from = values.pop().expect("completed outgoing branch");
                let value = address.transition(from.value, to.value)?.sample(progress)?;
                let trace = queries.trace.as_deref_mut().map(|arena| {
                    arena.blend(
                        from.trace.expect("traced outgoing branch"),
                        to.trace.expect("traced incoming branch"),
                    )
                });
                values.push(ScalarTrace { value, trace });
            }
        }
    }
    ensure(
        values.len() == 1,
        "retained Color role has an invalid result stack",
    )?;
    Ok(values.pop().expect("one result"))
}

pub(super) fn compose(
    base: &AttributeValue,
    base_trace: Option<FamilyTraceNodeId>,
    actual: &AttributeValue,
    actual_trace: Option<FamilyTraceNodeId>,
    sources: &[FamilyCompositionSample],
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    original_base: &AttributeValue,
    trace: Option<&mut FamilyTraceArena>,
) -> Result<TracedValue, TransitionError> {
    if matches!(actual, AttributeValue::ColorProgram(color) if matches!(color.as_ref(), ColorProgram::Direct { .. }))
    {
        return Ok(TracedValue {
            value: actual.clone(),
            trace: actual_trace,
        });
    }
    let mut components = Vec::new();
    for sample in sources {
        match sample {
            FamilyCompositionSample::Known(sample) if orthogonal(sample.address.address()) => {
                if let Some(ProgrammingComponent::Color(component)) =
                    sample.address.address().component
                {
                    components.push(component);
                }
            }
            FamilyCompositionSample::CoupledExpression { expression, .. } => {
                components.extend(expression.roles().iter().filter_map(|role| match role {
                    CoupledExpressionRole::ColorOrthogonal(component) => Some(*component),
                    _ => None,
                }));
            }
            _ => {}
        }
    }
    components.sort_unstable();
    components.dedup();
    let mut queries = Queries {
        source_base: base,
        source_base_trace: base_trace,
        original_base,
        sources,
        context,
        frame,
        keys: HashMap::new(),
        entries: Vec::new(),
        scratch: RetainedFamilyCompositionScratch::default(),
        trace,
    };
    let initial = queries.insert(BranchState::default())?;
    queries.entries[initial].base = Some(TracedValue {
        value: actual.clone(),
        trace: actual_trace,
    });
    let mut edits = Vec::new();
    let mut trace_root = actual_trace;
    for component in components {
        let address = CompiledDynamicValueAddress::new(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::SemanticColor {
                    basis: DynamicSemanticColorBasis::Retain,
                },
                component: Some(ProgrammingComponent::Color(component)),
            },
            None,
        )?;
        let ScalarTrace {
            value,
            trace: incoming,
        } = evaluate(component, &address, initial, &mut queries)?;
        let DynamicValue::Scalar(value) = value else {
            return Err(IntentError("Color role did not produce a scalar".into()).into());
        };
        if let Some(arena) = queries.trace.as_deref_mut() {
            trace_root = Some(arena.write(
                trace_root.expect("traced Color base"),
                incoming.expect("traced orthogonal source"),
                FamilyTraceFootprint::Component(ProgrammingComponent::Color(component)),
                false,
            ));
        }
        edits.push(ComponentEdit::Scalar {
            component: ProgrammingComponent::Color(component),
            operation: ScalarEdit::Set(ScalarIntent::Value(value)),
        });
    }
    Ok(TracedValue {
        value: edit_family(actual, &edits, &context.edit)?,
        trace: trace_root,
    })
}
