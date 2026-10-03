//! Explicit Color controls keep the implicit defaults of their own transition branches.
//! A branch query conditions the complete base stack, including higher contributors and
//! sibling lanes from the same Resume occurrence. It never reads a fitted fixture channel.
use super::conditioning::{BranchState, condition_sources};
use super::*;
use crate::{CoupledExpressionRole, DynamicSourceOccurrenceId, DynamicTransitionReason};
use std::collections::HashMap;

mod evaluation;
use evaluation::evaluate;

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
