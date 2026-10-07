//! Heap task-stack evaluation of one explicit Color component across the source prefix.
use super::*;

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

/// The pending task stack and completed value stack of one component evaluation.
struct Evaluation<'q, 'a, 'b> {
    component: ColorComponent,
    address: &'q CompiledDynamicValueAddress,
    queries: &'q mut Queries<'a, 'b>,
    tasks: Vec<Task>,
    values: Vec<ScalarTrace>,
}

/// Use a heap task stack for the source-prefix walk. The number of simultaneously active
/// effects must not consume the output thread's Rust call stack.
pub(super) fn evaluate(
    component: ColorComponent,
    address: &CompiledDynamicValueAddress,
    initial: usize,
    queries: &mut Queries<'_, '_>,
) -> Result<ScalarTrace, TransitionError> {
    let tasks = vec![Task::Prefix {
        end: queries.sources.len(),
        query: initial,
    }];
    let mut evaluation = Evaluation {
        component,
        address,
        queries,
        tasks,
        values: Vec::new(),
    };
    while let Some(task) = evaluation.tasks.pop() {
        evaluation.run(task)?;
    }
    let values = &mut evaluation.values;
    ensure(
        values.len() == 1,
        "retained Color role has an invalid result stack",
    )?;
    Ok(values.pop().expect("one result"))
}

impl Evaluation<'_, '_, '_> {
    fn run(&mut self, task: Task) -> Result<(), TransitionError> {
        match task {
            Task::Prefix { end, query } => self.prefix(end, query),
            Task::Node {
                index,
                node,
                query,
                raw,
            } => self.node(index, node, query, raw),
            Task::Ownership { index, activation } => {
                self.ownership(index, activation);
                Ok(())
            }
            Task::RawKnown(sample) => {
                let (value, trace) = component_value_and_trace(
                    &sample,
                    None,
                    self.queries.trace.as_deref_mut(),
                    None,
                )?;
                self.values.push(ScalarTrace { value, trace });
                Ok(())
            }
            Task::Output { index, activation } => self.output(index, activation),
            Task::ApplyKnown(sample) => self.apply_known(sample),
            Task::ApplyLeaf {
                value,
                activation,
                index,
                occurrence,
                dependency_occurrence,
            } => self.apply_leaf(value, activation, index, occurrence, dependency_occurrence),
            Task::Mix(progress) => self.mix(progress),
        }
    }

    /// Walk down from `end` to the nearest source with an explicit role for this component.
    fn prefix(&mut self, mut end: usize, query: usize) -> Result<(), TransitionError> {
        let component = self.component;
        let mut found = false;
        while end > 0 {
            end -= 1;
            let Some(sample) = self.queries.entries[query].sources[end].clone() else {
                continue;
            };
            match sample {
                FamilyCompositionSample::Known(sample)
                    if sample.address.address().component
                        == Some(ProgrammingComponent::Color(component))
                        && sample.participates() =>
                {
                    self.prefix_known(sample, end, query)?;
                    found = true;
                    break;
                }
                FamilyCompositionSample::CoupledExpression {
                    expression,
                    activation_mix,
                    ..
                } if activation_mix > 0.0 => {
                    let root =
                        expression
                            .expression()
                            .cloned()
                            .ok_or(TransitionError::Requires(
                                TransitionRequirement::CompatibleOwners,
                            ))?;
                    if has_explicit(&root, component) {
                        self.prefix_coupled(root, end, query, activation_mix);
                        found = true;
                        break;
                    }
                }
                _ => {}
            }
        }
        if !found {
            self.values
                .push(self.queries.implicit(query, component, self.address)?);
        }
        Ok(())
    }

    fn prefix_known(
        &mut self,
        sample: FamilySample,
        end: usize,
        query: usize,
    ) -> Result<(), TransitionError> {
        let controlled = matches!(sample.endpoint_control(self.queries.context), FamilyEndpointOutputControl::CrossfadeCurrent { mix } if mix < 1.0);
        if !controlled {
            self.tasks.push(Task::Ownership {
                index: end,
                activation: sample.activation_mix,
            });
        }
        if controlled {
            self.tasks.push(Task::Output {
                index: end,
                activation: sample.activation_mix,
            });
            match &sample.body {
                FamilySampleBody::ComponentExpression(expression) => self.tasks.push(Task::Node {
                    index: end,
                    node: Some(Arc::new(expression.expression().clone())),
                    query,
                    raw: true,
                }),
                FamilySampleBody::Materialized(_) => {
                    self.tasks.push(Task::RawKnown(sample.clone()))
                }
            }
            if sample.activation_mix < 1.0 {
                self.tasks.push(Task::Prefix { end, query });
            }
        } else if !sample.component_needs_underlay() {
            let (value, trace) =
                component_value_and_trace(&sample, None, self.queries.trace.as_deref_mut(), None)?;
            self.values.push(ScalarTrace { value, trace });
        } else if let FamilySampleBody::ComponentExpression(expression) = &sample.body {
            // A retained component may share a Resume branch with a base
            // writer in a sibling lane. Keep that occurrence correlated.
            self.tasks.push(Task::Node {
                index: end,
                node: Some(Arc::new(expression.expression().clone())),
                query,
                raw: false,
            });
        } else {
            self.tasks.push(Task::ApplyKnown(sample));
            self.tasks.push(Task::Prefix { end, query });
        }
        Ok(())
    }

    fn prefix_coupled(
        &mut self,
        root: Arc<DynamicSampleExpression>,
        end: usize,
        query: usize,
        activation_mix: f32,
    ) {
        let controlled = matches!(self.queries.sources[end].endpoint_control(self.queries.context), FamilyEndpointOutputControl::CrossfadeCurrent { mix } if mix < 1.0);
        if !controlled {
            self.tasks.push(Task::Ownership {
                index: end,
                activation: activation_mix,
            });
        }
        if controlled {
            self.tasks.push(Task::Output {
                index: end,
                activation: activation_mix,
            });
        }
        self.tasks.push(Task::Node {
            index: end,
            node: Some(root),
            query,
            raw: controlled,
        });
        if controlled && activation_mix < 1.0 {
            self.tasks.push(Task::Prefix { end, query });
        }
    }

    /// Descend one retained node of the explicit subtree, branching across transitions.
    fn node(
        &mut self,
        index: usize,
        node: Option<Arc<DynamicSampleExpression>>,
        query: usize,
        raw: bool,
    ) -> Result<(), TransitionError> {
        let component = self.component;
        let mut state = self.queries.entries[query].state.clone();
        state.replace(index, node.clone());
        if !node
            .as_ref()
            .is_some_and(|node| has_explicit(node, component))
        {
            let query = self.queries.insert(state)?;
            self.tasks.push(Task::Prefix { end: index, query });
            return Ok(());
        }
        let node = Arc::new(node.expect("explicit subtree").shallow()?);
        match node.as_ref() {
            DynamicSampleExpression::Programming {
                value,
                occurrence,
                dependency_occurrence,
                ..
            } => self.leaf(index, state, raw, value, *occurrence, dependency_occurrence),
            DynamicSampleExpression::Transition {
                from,
                to,
                progress,
                reason,
            } => {
                let branches: &[(bool, &Option<Arc<DynamicSampleExpression>>)] = if *progress == 0.0
                {
                    &[(false, from)]
                } else if *progress == 1.0 {
                    &[(true, to)]
                } else {
                    self.tasks.push(Task::Mix(*progress));
                    &[(true, to), (false, from)]
                };
                for &(incoming, child) in branches {
                    let mut state = state.clone();
                    if let DynamicTransitionReason::Resume { occurrence_id } = reason {
                        state.choose(
                            self.queries.sources[index].rank(),
                            *occurrence_id,
                            incoming,
                        )?;
                    }
                    state.replace(index, child.clone());
                    let query = self.queries.insert(state)?;
                    self.tasks.push(Task::Node {
                        index,
                        node: child.clone(),
                        query,
                        raw,
                    });
                }
                Ok(())
            }
            _ => Err(IntentError("explicit Color role has an invalid retained node".into()).into()),
        }
    }

    fn leaf(
        &mut self,
        index: usize,
        state: BranchState,
        raw: bool,
        value: &DynamicValue,
        occurrence: Option<DynamicSourceOccurrenceId>,
        dependency_occurrence: &Option<crate::DynamicSourceDependency>,
    ) -> Result<(), TransitionError> {
        let activation = if raw {
            1.0
        } else {
            activation(&self.queries.sources[index])
        };
        if activation == 1.0 {
            self.values.push(ScalarTrace {
                value: value.clone(),
                trace: self.queries.leaf_trace(
                    index,
                    self.component,
                    occurrence,
                    dependency_occurrence.clone(),
                ),
            });
        } else {
            let query = self.queries.insert(state)?;
            self.tasks.push(Task::ApplyLeaf {
                value: value.clone(),
                activation,
                index,
                occurrence,
                dependency_occurrence: dependency_occurrence.clone(),
            });
            self.tasks.push(Task::Prefix { end: index, query });
        }
        Ok(())
    }

    fn ownership(&mut self, index: usize, activation: f32) {
        let value = self.values.last_mut().expect("completed source root");
        if let Some(trace) = self.queries.trace.as_deref_mut() {
            let appearance = value.trace.expect("traced source root");
            value.trace = Some(trace.control(
                self.queries.sources[index].rank(),
                FamilyTraceFootprint::Component(ProgrammingComponent::Color(self.component)),
                appearance,
                (activation < 1.0).then_some(appearance),
            ));
        }
    }

    fn output(&mut self, index: usize, activation: f32) -> Result<(), TransitionError> {
        let queries = &mut *self.queries;
        let address = self.address;
        let values = &mut self.values;
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
                FamilyTraceFootprint::Component(ProgrammingComponent::Color(self.component)),
                appearance.expect("traced result"),
                prior,
                queries.context,
                trace,
            )
        });
        values.push(ScalarTrace { value, trace });
        Ok(())
    }

    fn apply_known(&mut self, sample: FamilySample) -> Result<(), TransitionError> {
        let underlay = self.values.pop().expect("completed component prefix");
        let target = sample.component_value(Some(&underlay.value))?;
        let value = self
            .address
            .transition(underlay.value, target)?
            .sample(sample.activation_mix)?;
        let trace = self.queries.trace.as_deref_mut().map(|arena| {
            let incoming = sample_trace(&sample, arena);
            arena.blend(underlay.trace.expect("traced Color underlay"), incoming)
        });
        self.values.push(ScalarTrace { value, trace });
        Ok(())
    }

    fn apply_leaf(
        &mut self,
        value: DynamicValue,
        activation: f32,
        index: usize,
        occurrence: Option<DynamicSourceOccurrenceId>,
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    ) -> Result<(), TransitionError> {
        let underlay = self.values.pop().expect("completed explicit leaf prefix");
        let value = self
            .address
            .transition(underlay.value, value)?
            .sample(activation)?;
        let incoming =
            self.queries
                .leaf_trace(index, self.component, occurrence, dependency_occurrence);
        let trace = self.queries.trace.as_deref_mut().map(|arena| {
            arena.blend(
                underlay.trace.expect("traced Color underlay"),
                incoming.expect("traced Color leaf"),
            )
        });
        self.values.push(ScalarTrace { value, trace });
        Ok(())
    }

    fn mix(&mut self, progress: f32) -> Result<(), TransitionError> {
        let to = self.values.pop().expect("completed incoming branch");
        let from = self.values.pop().expect("completed outgoing branch");
        let value = self
            .address
            .transition(from.value, to.value)?
            .sample(progress)?;
        let trace = self.queries.trace.as_deref_mut().map(|arena| {
            arena.blend(
                from.trace.expect("traced outgoing branch"),
                to.trace.expect("traced incoming branch"),
            )
        });
        self.values.push(ScalarTrace { value, trace });
        Ok(())
    }
}
