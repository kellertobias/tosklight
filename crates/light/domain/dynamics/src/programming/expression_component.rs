//! Retained component evaluation after footprint arbitration. Angle components are bundled
//! into complete Position expressions before this boundary; these masks remain component-sized.
use super::{
    CompiledDynamicValueAddress, CompiledDynamicValueTransition, DynamicFamilyRepresentation,
    DynamicSampleExpression, DynamicValue, RetainedExpressionNode, RetainedExpressionTape,
    RetainedNodeId,
};
use light_core::programming::{TransitionError, TransitionRequirement};
use std::sync::Arc;

enum Node {
    Value {
        value: DynamicValue,
        occurrence: Option<super::DynamicSourceOccurrenceId>,
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    Underlay,
    Transition {
        from: usize,
        to: usize,
        progress: f32,
        cached: Option<CompiledDynamicValueTransition>,
    },
}

pub enum ComponentExpressionStep<'a> {
    Underlay,
    Authored {
        value: &'a DynamicValue,
        occurrence: Option<super::DynamicSourceOccurrenceId>,
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    Transition {
        from: usize,
        to: usize,
        progress: f32,
    },
}

pub trait ComponentExpressionObserver {
    fn evaluated(
        &mut self,
        node: usize,
        step: ComponentExpressionStep<'_>,
        value: &DynamicValue,
    ) -> Result<(), TransitionError>;
}

fn incompatible() -> TransitionError {
    TransitionError::Requires(TransitionRequirement::CompatibleOwners)
}

/// Mark only nodes that can affect the current sample. Exact transition endpoints preserve
/// their retained history, but do not demand a source model for the invisible branch.
fn active_nodes(tape: &RetainedExpressionTape, root: RetainedNodeId) -> Vec<bool> {
    let mut active = vec![false; tape.nodes.len()];
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let index = id.0 as usize;
        if active[index] {
            continue;
        }
        active[index] = true;
        match &tape.nodes[index] {
            RetainedExpressionNode::Transition { from, progress, .. } if *progress == 0.0 => {
                stack.extend(*from)
            }
            RetainedExpressionNode::Transition { to, progress, .. } if *progress == 1.0 => {
                stack.extend(*to)
            }
            node => stack.extend(node.children()),
        }
    }
    active
}

/// Cold-validated expression for one exact non-Angle component address. The original expression
/// stays available for interruption and persistence; evaluation uses a flat compiled tape.
pub struct CompiledComponentExpression {
    retained: Arc<DynamicSampleExpression>,
    address: Arc<CompiledDynamicValueAddress>,
    nodes: Vec<Node>,
    root: usize,
    needs_underlay: bool,
}

impl CompiledComponentExpression {
    pub fn new(
        expression: Arc<DynamicSampleExpression>,
        address: Arc<CompiledDynamicValueAddress>,
    ) -> Result<Self, TransitionError> {
        if address.address().component.is_none()
            || matches!(
                address.address().representation,
                DynamicFamilyRepresentation::Angles
            )
        {
            return Err(incompatible());
        }
        let tape = RetainedExpressionTape::from_roots(&[Arc::clone(&expression)])?;
        let root_id = tape.roots()[0];
        let active = active_nodes(&tape, root_id);
        let mut map = vec![None; tape.nodes.len()];
        let mut nodes = vec![Node::Underlay];
        let mut needs = vec![true];
        let mut constants: Vec<Option<DynamicValue>> = vec![None];
        for (source_index, source) in tape.nodes.iter().enumerate() {
            if !active[source_index] {
                continue;
            }
            let (node, dependent, constant) = match source {
                RetainedExpressionNode::Programming {
                    address: leaf_address,
                    value,
                    occurrence,
                    dependency_occurrence,
                } => {
                    if leaf_address != address.address() {
                        return Err(incompatible());
                    }
                    address.validate_source_value(value)?;
                    (
                        Node::Value {
                            value: value.clone(),
                            occurrence: *occurrence,
                            dependency_occurrence: dependency_occurrence.clone(),
                        },
                        false,
                        Some(value.clone()),
                    )
                }
                RetainedExpressionNode::Transition {
                    from, to, progress, ..
                } => {
                    let child = |id: Option<RetainedNodeId>| -> usize {
                        id.and_then(|id| map[id.0 as usize]).unwrap_or(0)
                    };
                    if *progress == 0.0 || *progress == 1.0 {
                        map[source_index] = Some(child(if *progress == 0.0 { *from } else { *to }));
                        continue;
                    }
                    let from = child(*from);
                    let to = child(*to);
                    if from == 0 && to == 0 {
                        map[source_index] = Some(0);
                        continue;
                    }
                    let cached = match (&constants[from], &constants[to]) {
                        (Some(a), Some(b)) => Some(address.transition(a.clone(), b.clone())?),
                        _ => None,
                    };
                    (
                        Node::Transition {
                            from,
                            to,
                            progress: *progress,
                            cached,
                        },
                        needs[from] || needs[to],
                        None,
                    )
                }
                RetainedExpressionNode::LegacyScalar { .. }
                | RetainedExpressionNode::AngleCurrent { .. }
                | RetainedExpressionNode::AngleNumeric { .. }
                | RetainedExpressionNode::Scale { .. } => return Err(incompatible()),
            };
            let index = nodes.len();
            nodes.push(node);
            needs.push(dependent);
            constants.push(constant);
            map[source_index] = Some(index);
        }
        let root = map[root_id.0 as usize].unwrap_or(0);
        Ok(Self {
            retained: expression,
            address,
            nodes,
            root,
            needs_underlay: root != 0 && needs[root],
        })
    }

    pub fn address(&self) -> &Arc<CompiledDynamicValueAddress> {
        &self.address
    }

    pub fn expression(&self) -> &DynamicSampleExpression {
        &self.retained
    }

    /// An exact release to the eligible underlay owns no component at this frame.
    pub fn participates(&self) -> bool {
        self.root != 0
    }

    pub fn needs_underlay(&self) -> bool {
        self.needs_underlay
    }

    pub fn trace_root_node(&self) -> usize {
        self.root
    }

    pub fn evaluate(
        &self,
        underlay: Option<&DynamicValue>,
    ) -> Result<Option<DynamicValue>, TransitionError> {
        self.evaluate_observed(underlay, None)
    }

    pub fn evaluate_observed(
        &self,
        underlay: Option<&DynamicValue>,
        mut observer: Option<&mut dyn ComponentExpressionObserver>,
    ) -> Result<Option<DynamicValue>, TransitionError> {
        if !self.participates() {
            return Ok(None);
        }
        if self.needs_underlay {
            let value = underlay.ok_or(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ))?;
            self.address.validate_value(value)?;
        }
        let mut values: Vec<Option<DynamicValue>> = vec![None; self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            let evaluated = match node {
                Node::Underlay => underlay.cloned(),
                Node::Value { value, .. } => Some(value.clone()),
                Node::Transition {
                    from,
                    to,
                    progress,
                    cached,
                } => Some(match cached {
                    Some(transition) => transition.sample(*progress)?,
                    None => {
                        let from = values[*from].as_ref().ok_or(TransitionError::Requires(
                            TransitionRequirement::MaterializedEndpoints,
                        ))?;
                        let to = values[*to].as_ref().ok_or(TransitionError::Requires(
                            TransitionRequirement::MaterializedEndpoints,
                        ))?;
                        self.address
                            .transition(from.clone(), to.clone())?
                            .sample(*progress)?
                    }
                }),
            };
            if let Some(observer) = observer.as_deref_mut()
                && let Some(value) = evaluated.as_ref()
            {
                let step = match node {
                    Node::Underlay => ComponentExpressionStep::Underlay,
                    Node::Value {
                        value,
                        occurrence,
                        dependency_occurrence,
                    } => ComponentExpressionStep::Authored {
                        value,
                        occurrence: *occurrence,
                        dependency_occurrence: dependency_occurrence.clone(),
                    },
                    Node::Transition {
                        from, to, progress, ..
                    } => ComponentExpressionStep::Transition {
                        from: *from,
                        to: *to,
                        progress: *progress,
                    },
                };
                observer.evaluated(index, step, value)?;
            }
            values[index] = evaluated;
        }
        Ok(values[self.root].take())
    }
}

#[cfg(test)]
mod tests;
