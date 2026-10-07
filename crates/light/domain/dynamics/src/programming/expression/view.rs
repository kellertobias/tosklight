//! Borrowed, shallow access to live trees and flat history. No traversal expands history.
use super::*;
use crate::RetainedExpressionNode as N;
use rustc_hash::FxHashSet as HashSet;

#[derive(Clone, Copy)]
pub(crate) enum ExpressionNodeRef<'a> {
    Tree(&'a DynamicSampleExpression),
    Tape(&'a RetainedExpressionTape, RetainedNodeId),
}

pub(crate) enum ExpressionNode<'a> {
    Legacy(
        &'a AttributeKey,
        f32,
        Option<DynamicSourceOccurrenceId>,
        Option<&'a crate::DynamicSourceDependency>,
    ),
    Programming(
        &'a DynamicValueAddress,
        &'a DynamicValue,
        Option<DynamicSourceOccurrenceId>,
        Option<&'a crate::DynamicSourceDependency>,
    ),
    Current(&'a DynamicValueAddress),
    Numeric(&'a crate::AngleNumericProgram),
    Scale {
        address: &'a DynamicValueAddress,
        base: &'a DynamicValue,
        value: ExpressionNodeRef<'a>,
        factor: f32,
        baseline_occurrence: Option<DynamicSourceOccurrenceId>,
    },
    Transition {
        from: Option<ExpressionNodeRef<'a>>,
        to: Option<ExpressionNodeRef<'a>>,
        progress: f32,
        reason: DynamicTransitionReason,
    },
}

impl<'a> ExpressionNodeRef<'a> {
    pub fn new(value: &'a DynamicSampleExpression) -> Self {
        match value {
            DynamicSampleExpression::Retained { tape, root } => Self::Tape(tape, *root),
            value => Self::Tree(value),
        }
    }

    pub fn key(self) -> (usize, Option<RetainedNodeId>) {
        match self {
            Self::Tree(value) => (value as *const _ as usize, None),
            Self::Tape(tape, root) => (tape as *const _ as usize, Some(root)),
        }
    }

    pub fn node(self) -> Result<ExpressionNode<'a>, IntentError> {
        Ok(match self {
            Self::Tree(value) => match value {
                DynamicSampleExpression::Retained { tape, root } => {
                    return Self::Tape(tape, *root).node();
                }
                // Provenance annotations are transparent; the wrapper keeps the node key.
                DynamicSampleExpression::Operation { value, .. } => {
                    return Self::new(value).node();
                }
                DynamicSampleExpression::LegacyScalar {
                    attribute,
                    value,
                    occurrence,
                    dependency_occurrence,
                } => ExpressionNode::Legacy(
                    attribute,
                    *value,
                    *occurrence,
                    dependency_occurrence.as_ref(),
                ),
                DynamicSampleExpression::Programming {
                    address,
                    value,
                    occurrence,
                    dependency_occurrence,
                } => ExpressionNode::Programming(
                    address,
                    value,
                    *occurrence,
                    dependency_occurrence.as_ref(),
                ),
                DynamicSampleExpression::AngleCurrent { address } => {
                    ExpressionNode::Current(address)
                }
                DynamicSampleExpression::AngleNumeric { program } => {
                    ExpressionNode::Numeric(program)
                }
                DynamicSampleExpression::Scale {
                    address,
                    base,
                    value,
                    factor,
                    baseline_occurrence,
                } => ExpressionNode::Scale {
                    address,
                    base,
                    value: Self::new(value),
                    factor: *factor,
                    baseline_occurrence: *baseline_occurrence,
                },
                DynamicSampleExpression::Transition {
                    from,
                    to,
                    progress,
                    reason,
                } => ExpressionNode::Transition {
                    from: from.as_ref().map(|value| Self::new(value)),
                    to: to.as_ref().map(|value| Self::new(value)),
                    progress: *progress,
                    reason: *reason,
                },
            },
            Self::Tape(tape, root) => match tape.node(root).ok_or_else(|| {
                IntentError("retained expression references an absent node".into())
            })? {
                N::LegacyScalar {
                    attribute,
                    value,
                    occurrence,
                    dependency_occurrence,
                } => ExpressionNode::Legacy(
                    attribute,
                    *value,
                    *occurrence,
                    dependency_occurrence.as_ref(),
                ),
                N::Programming {
                    address,
                    value,
                    occurrence,
                    dependency_occurrence,
                } => ExpressionNode::Programming(
                    address,
                    value,
                    *occurrence,
                    dependency_occurrence.as_ref(),
                ),
                N::AngleCurrent { address } => ExpressionNode::Current(address),
                N::AngleNumeric { program } => ExpressionNode::Numeric(program),
                N::Scale {
                    address,
                    base,
                    value,
                    factor,
                    baseline_occurrence,
                } => ExpressionNode::Scale {
                    address,
                    base,
                    value: Self::Tape(tape, *value),
                    factor: *factor,
                    baseline_occurrence: *baseline_occurrence,
                },
                N::Transition {
                    from,
                    to,
                    progress,
                    reason,
                } => ExpressionNode::Transition {
                    from: from.map(|id| Self::Tape(tape, id)),
                    to: to.map(|id| Self::Tape(tape, id)),
                    progress: *progress,
                    reason: *reason,
                },
            },
        })
    }

    /// The producer origin attached to a live tree node. Tape nodes use their object table.
    pub fn operation_origin(self) -> Option<&'a crate::DynamicOperationOrigin> {
        match self {
            Self::Tree(DynamicSampleExpression::Operation { origin, .. }) => origin.as_ref(),
            _ => None,
        }
    }

    /// Children precede their parents exactly once, including shared historical descendants.
    pub fn postorder(self, active_only: bool) -> Result<Vec<Self>, IntentError> {
        let mut pending = vec![(self, false)];
        let mut complete = HashSet::default();
        let mut active = HashSet::default();
        let mut ordered = Vec::new();
        while let Some((node, exit)) = pending.pop() {
            let key = node.key();
            if exit {
                active.remove(&key);
                complete.insert(key);
                ordered.push(node);
                continue;
            }
            if complete.contains(&key) {
                continue;
            }
            if !active.insert(key) {
                return Err(IntentError("retained expression contains a cycle".into()));
            }
            pending.push((node, true));
            match node.node()? {
                ExpressionNode::Scale { value, factor, .. } if !active_only || factor != 0.0 => {
                    pending.push((value, false))
                }
                ExpressionNode::Transition {
                    from, to, progress, ..
                } => {
                    if !active_only || progress != 0.0 {
                        if let Some(value) = to {
                            pending.push((value, false));
                        }
                    }
                    if !active_only || progress != 1.0 {
                        if let Some(value) = from {
                            pending.push((value, false));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(ordered)
    }
}
