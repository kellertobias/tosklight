use super::{
    DynamicFamilyRepresentation, DynamicSourceOccurrenceId, DynamicValue, DynamicValueAddress,
    RetainedExpressionTape, RetainedNodeId,
};
use light_core::{
    AttributeKey,
    programming::{IntentError, PROGRAMMING_CONTRACT_VERSION, TransitionRequirement},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Retained authored output, before live geometry and whole-owner composition.
/// Leaves keep their original address even if a paused lane is edited or deleted.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DynamicSampleExpression {
    /// Shared immutable history. Ordinary live samples wrap these handles without copying
    /// the historical graph; snapshots store one tape per instance and keyed root IDs.
    Retained {
        tape: Arc<RetainedExpressionTape>,
        root: RetainedNodeId,
    },
    LegacyScalar {
        attribute: AttributeKey,
        value: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        occurrence: Option<DynamicSourceOccurrenceId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    Programming {
        address: Arc<DynamicValueAddress>,
        value: DynamicValue,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        occurrence: Option<DynamicSourceOccurrenceId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    /// An Angle partner follows the immutable pre-Dynamic frame even while the
    /// authored axis is held. Retain its original address through lane hot edits.
    AngleCurrent { address: Arc<DynamicValueAddress> },
    /// Numeric Angle arithmetic pinned at one sampled phase. Current remains symbolic until
    /// the complete Pan/Tilt pair is evaluated for a physical destination.
    AngleNumeric {
        program: Arc<crate::AngleNumericProgram>,
    },
    /// Whole-family Size keeps its physical baseline and may exceed one. Resolve
    /// this after live geometry/source appearance is available, before fixture fitting.
    Scale {
        address: Arc<DynamicValueAddress>,
        base: DynamicValue,
        value: Arc<DynamicSampleExpression>,
        factor: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        baseline_occurrence: Option<DynamicSourceOccurrenceId>,
    },
    Transition {
        /// Absence means the eligible underlay, never a fabricated zero.
        from: Option<Arc<DynamicSampleExpression>>,
        to: Option<Arc<DynamicSampleExpression>>,
        progress: f32,
        reason: DynamicTransitionReason,
    },
    /// Producer provenance of the wrapped Required transition or whole-family Size node.
    /// Transparent to evaluation, equality, views and `shallow`. Only the sampler attaches an
    /// origin; retained tapes move it into their object table. Serialization drops the
    /// process-local origin, which then decodes as explicitly uncorrelated.
    Operation {
        #[serde(skip)]
        origin: Option<crate::DynamicOperationOrigin>,
        value: Arc<DynamicSampleExpression>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DynamicTransitionReason {
    Required {
        requirement: TransitionRequirement,
    },
    /// Resume progress is independent of the controller's activation influence.
    Resume {
        /// All lanes in this resume share one branch identity, including lanes
        /// added or removed while paused. Nested interrupted resumes keep theirs.
        occurrence_id: Uuid,
    },
}

mod view;
pub(crate) use view::{ExpressionNode, ExpressionNodeRef};

impl PartialEq for DynamicSampleExpression {
    fn eq(&self, other: &Self) -> bool {
        // TL-639 round 4: two plain leaves compare as the walk below compares their nodes,
        // without its work list and visited set. Wrapped, retained and composite expressions
        // take the walk.
        match (self, other) {
            (
                Self::Programming {
                    address: a,
                    value: x,
                    occurrence: p,
                    dependency_occurrence: d,
                },
                Self::Programming {
                    address: b,
                    value: y,
                    occurrence: q,
                    dependency_occurrence: e,
                },
            ) => return a == b && x == y && p == q && d == e,
            (
                Self::LegacyScalar {
                    attribute: a,
                    value: x,
                    occurrence: p,
                    dependency_occurrence: d,
                },
                Self::LegacyScalar {
                    attribute: b,
                    value: y,
                    occurrence: q,
                    dependency_occurrence: e,
                },
            ) => return a == b && x == y && p == q && d == e,
            (Self::AngleCurrent { address: a }, Self::AngleCurrent { address: b }) => {
                return a == b;
            }
            (Self::AngleNumeric { program: a }, Self::AngleNumeric { program: b }) => {
                return a == b;
            }
            _ => {}
        }
        let mut pending = vec![(ExpressionNodeRef::new(self), ExpressionNodeRef::new(other))];
        let mut visited = std::collections::HashSet::new();
        while let Some((left, right)) = pending.pop() {
            if !visited.insert((left.key(), right.key())) {
                continue;
            }
            let (Ok(left), Ok(right)) = (left.node(), right.node()) else {
                return false;
            };
            match (left, right) {
                (ExpressionNode::Legacy(a, x, p, d), ExpressionNode::Legacy(b, y, q, e))
                    if a == b && x == y && p == q && d == e => {}
                (
                    ExpressionNode::Programming(a, x, p, d),
                    ExpressionNode::Programming(b, y, q, e),
                ) if a == b && x == y && p == q && d == e => {}
                (ExpressionNode::Current(a), ExpressionNode::Current(b)) if a == b => {}
                (ExpressionNode::Numeric(a), ExpressionNode::Numeric(b)) if a == b => {}
                (
                    ExpressionNode::Scale {
                        address: a,
                        base: x,
                        factor: p,
                        value: u,
                        baseline_occurrence: o,
                    },
                    ExpressionNode::Scale {
                        address: b,
                        base: y,
                        factor: q,
                        value: v,
                        baseline_occurrence: z,
                    },
                ) if a == b && x == y && p == q && o == z => pending.push((u, v)),
                (
                    ExpressionNode::Transition {
                        from: a,
                        to: b,
                        progress: p,
                        reason: r,
                    },
                    ExpressionNode::Transition {
                        from: x,
                        to: y,
                        progress: q,
                        reason: s,
                    },
                ) if p == q && r == s => {
                    for (first, second) in [(a, x), (b, y)] {
                        match (first, second) {
                            (Some(first), Some(second)) => pending.push((first, second)),
                            (None, None) => {}
                            _ => return false,
                        }
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

impl DynamicSampleExpression {
    fn fresh_authored_occurrence_matches(
        &self,
        occurrence: Option<DynamicSourceOccurrenceId>,
    ) -> bool {
        match self {
            Self::LegacyScalar {
                occurrence: slot, ..
            }
            | Self::Programming {
                occurrence: slot, ..
            } => *slot == occurrence,
            Self::AngleNumeric { program } => program.occurrence == occurrence,
            Self::Scale { value, .. } => value.fresh_authored_occurrence_matches(occurrence),
            Self::Transition { from, to, .. } => {
                from.as_ref()
                    .is_none_or(|value| value.fresh_authored_occurrence_matches(occurrence))
                    && to
                        .as_ref()
                        .is_none_or(|value| value.fresh_authored_occurrence_matches(occurrence))
            }
            Self::Retained { .. } | Self::AngleCurrent { .. } => true,
            Self::Operation { value, .. } => value.fresh_authored_occurrence_matches(occurrence),
        }
    }

    /// Bind a newly sampled lane before it can enter held/resume history. This only walks
    /// the bounded fresh lane expression, never a retained historical tape.
    pub(crate) fn bind_fresh_authored_occurrence(
        &mut self,
        occurrence: Option<DynamicSourceOccurrenceId>,
    ) {
        if self.fresh_authored_occurrence_matches(occurrence) {
            return;
        }
        match self {
            Self::LegacyScalar {
                occurrence: slot, ..
            }
            | Self::Programming {
                occurrence: slot, ..
            } => *slot = occurrence,
            Self::AngleNumeric { program } => Arc::make_mut(program).occurrence = occurrence,
            Self::Scale { value, .. } => {
                Arc::make_mut(value).bind_fresh_authored_occurrence(occurrence)
            }
            Self::Transition { from, to, .. } => {
                if let Some(from) = from {
                    Arc::make_mut(from).bind_fresh_authored_occurrence(occurrence);
                }
                if let Some(to) = to {
                    Arc::make_mut(to).bind_fresh_authored_occurrence(occurrence);
                }
            }
            Self::Retained { .. } | Self::AngleCurrent { .. } => {}
            Self::Operation { value, .. } => {
                Arc::make_mut(value).bind_fresh_authored_occurrence(occurrence)
            }
        }
    }

    pub fn required_programming_contract(&self) -> u16 {
        let Ok(nodes) = ExpressionNodeRef::new(self).postorder(false) else {
            return PROGRAMMING_CONTRACT_VERSION;
        };
        if nodes.into_iter().any(|node| {
            matches!(
                node.node(),
                Ok(ExpressionNode::Programming(..)
                    | ExpressionNode::Current(..)
                    | ExpressionNode::Numeric(..)
                    | ExpressionNode::Scale { .. })
            )
        }) {
            PROGRAMMING_CONTRACT_VERSION
        } else {
            0
        }
    }

    /// Cold validation is iterative for both historical trees and flat retained roots.
    pub fn validate(&self) -> Result<(), IntentError> {
        if let Some(result) = RetainedExpressionTape::validate_plain_leaf(self) {
            return result;
        }
        RetainedExpressionTape::from_roots(&[Arc::new(self.clone())]).map(|_| ())
    }

    pub fn contains_angles(&self) -> bool {
        // TL-639: a plain leaf is its own only node.
        match self {
            Self::Programming { address, .. } => {
                return address.representation == DynamicFamilyRepresentation::Angles;
            }
            Self::LegacyScalar { .. } => return false,
            // TL-639 round 4: the Angle leaves are their own only node too.
            Self::AngleNumeric { .. } | Self::AngleCurrent { .. } => return true,
            _ => {}
        }
        ExpressionNodeRef::new(self)
            .postorder(false)
            .is_ok_and(|nodes| {
                nodes.into_iter().any(|node| match node.node() {
                    Ok(ExpressionNode::Current(_) | ExpressionNode::Numeric(_)) => true,
                    Ok(
                        ExpressionNode::Programming(address, ..)
                        | ExpressionNode::Scale { address, .. },
                    ) => address.representation == DynamicFamilyRepresentation::Angles,
                    _ => false,
                })
            })
    }

    /// Numeric Angle leaves must use complete Position forest composition even without an
    /// authored source catalogue. The legacy scalar bundler cannot evaluate symbolic Current.
    pub fn has_angle_numeric(&self) -> bool {
        ExpressionNodeRef::new(self)
            .postorder(false)
            .map_or(true, |nodes| {
                nodes
                    .into_iter()
                    .any(|node| matches!(node.node(), Ok(ExpressionNode::Numeric(_))))
            })
    }

    pub fn legacy_leaf(&self) -> Option<(&AttributeKey, f32)> {
        match ExpressionNodeRef::new(self).node().ok()? {
            ExpressionNode::Legacy(attribute, value, ..) => Some((attribute, value)),
            _ => None,
        }
    }

    pub fn programming_leaf(&self) -> Option<(&DynamicValueAddress, &DynamicValue)> {
        match ExpressionNodeRef::new(self).node().ok()? {
            ExpressionNode::Programming(address, value, ..) => Some((address, value)),
            _ => None,
        }
    }

    pub fn angle_current_address(&self) -> Option<&DynamicValueAddress> {
        match ExpressionNodeRef::new(self).node().ok()? {
            ExpressionNode::Current(address) => Some(address),
            _ => None,
        }
    }

    /// [`Self::shallow`] of an owned expression: one that is already shallow is kept rather
    /// than cloned and dropped (TL-639 round 6).
    pub(crate) fn into_shallow(self) -> Result<Self, IntentError> {
        match self {
            Self::Operation { .. } | Self::Retained { .. } => self.shallow(),
            shallow => Ok(shallow),
        }
    }

    /// Expose a single historical node. Children stay shared handles; this never reconstructs
    /// a recursive history and is suitable for callers that already use a heap task stack.
    pub(crate) fn shallow(&self) -> Result<Self, IntentError> {
        if let Self::Operation { value, .. } = self {
            return value.shallow();
        }
        let Self::Retained { tape, root } = self else {
            return Ok(self.clone());
        };
        use crate::RetainedExpressionNode as N;
        let child = |id| {
            Arc::new(Self::Retained {
                tape: tape.clone(),
                root: id,
            })
        };
        Ok(
            match tape.node(*root).ok_or_else(|| {
                IntentError("retained expression references an absent node".into())
            })? {
                N::LegacyScalar {
                    attribute,
                    value,
                    occurrence,
                    dependency_occurrence,
                } => Self::LegacyScalar {
                    attribute: attribute.clone(),
                    value: *value,
                    occurrence: *occurrence,
                    dependency_occurrence: dependency_occurrence.clone(),
                },
                N::Programming {
                    address,
                    value,
                    occurrence,
                    dependency_occurrence,
                } => Self::Programming {
                    address: Arc::new(address.clone()),
                    value: value.clone(),
                    occurrence: *occurrence,
                    dependency_occurrence: dependency_occurrence.clone(),
                },
                N::AngleCurrent { address } => Self::AngleCurrent {
                    address: Arc::new(address.clone()),
                },
                N::AngleNumeric { program } => Self::AngleNumeric {
                    program: Arc::clone(program),
                },
                N::Scale {
                    address,
                    base,
                    value,
                    factor,
                    baseline_occurrence,
                } => Self::Scale {
                    address: Arc::new(address.clone()),
                    base: base.clone(),
                    value: child(*value),
                    factor: *factor,
                    baseline_occurrence: *baseline_occurrence,
                },
                N::Transition {
                    from,
                    to,
                    progress,
                    reason,
                } => Self::Transition {
                    from: from.map(child),
                    to: to.map(child),
                    progress: *progress,
                    reason: *reason,
                },
            },
        )
    }

    pub(crate) fn visit_resume_occurrences(&self, visitor: &mut impl FnMut(Uuid)) {
        if let Ok(nodes) = ExpressionNodeRef::new(self).postorder(false) {
            for node in nodes {
                if let Ok(ExpressionNode::Transition {
                    reason: DynamicTransitionReason::Resume { occurrence_id },
                    ..
                }) = node.node()
                {
                    visitor(occurrence_id);
                }
            }
        }
    }

    /// Visit every reachable captured source identity without expanding shared history.
    /// The caller may deduplicate IDs when pruning an external immutable origin catalogue.
    pub fn visit_source_occurrences(
        &self,
        visitor: &mut impl FnMut(DynamicSourceOccurrenceId),
    ) -> Result<(), IntentError> {
        for node in ExpressionNodeRef::new(self).postorder(false)? {
            match node.node()? {
                ExpressionNode::Legacy(_, _, occurrence, dependency) => {
                    if let Some(id) = occurrence {
                        visitor(id);
                    }
                    if let Some(id) = dependency.and_then(|dependency| dependency.occurrence) {
                        visitor(id);
                    }
                }
                ExpressionNode::Programming(_, _, occurrence, dependency) => {
                    if let Some(id) = occurrence {
                        visitor(id);
                    }
                    if let Some(id) = dependency.and_then(|dependency| dependency.occurrence) {
                        visitor(id);
                    }
                }
                ExpressionNode::Numeric(program) => {
                    if let Some(id) = program.occurrence {
                        visitor(id);
                    }
                    program.visit_dependencies(&mut |dependency| {
                        if let Some(id) = dependency.occurrence {
                            visitor(id);
                        }
                    });
                }
                ExpressionNode::Scale {
                    baseline_occurrence,
                    ..
                } => {
                    if let Some(id) = baseline_occurrence {
                        visitor(id);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn has_source_occurrences(&self) -> bool {
        ExpressionNodeRef::new(self)
            .postorder(false)
            .map_or(true, |nodes| {
                nodes.into_iter().any(|node| match node.node() {
                    Ok(
                        ExpressionNode::Legacy(_, _, occurrence, dependency)
                        | ExpressionNode::Programming(_, _, occurrence, dependency),
                    ) => occurrence.is_some() || dependency.is_some(),
                    Ok(ExpressionNode::Numeric(program)) => {
                        let mut dependency = false;
                        program.visit_dependencies(&mut |_| dependency = true);
                        program.occurrence.is_some() || dependency
                    }
                    Ok(ExpressionNode::Scale {
                        baseline_occurrence,
                        ..
                    }) => baseline_occurrence.is_some(),
                    Err(_) => true,
                    _ => false,
                })
            })
    }

    /// Cold source verification visits shared original values once without resolving live
    /// Current, Point positions or destination fixture channels.
    pub(crate) fn visit_programming_values(
        &self,
        visitor: &mut impl FnMut(&DynamicValueAddress, &DynamicValue) -> Result<(), IntentError>,
    ) -> Result<(), IntentError> {
        for node in ExpressionNodeRef::new(self).postorder(false)? {
            match node.node()? {
                ExpressionNode::Programming(address, value, ..) => visitor(address, value)?,
                ExpressionNode::Numeric(program) => program.visit_materialized_values(visitor)?,
                ExpressionNode::Scale { address, base, .. } => {
                    let DynamicValue::Family(family) = base else {
                        return Err(IntentError(
                            "Dynamic Size requires a whole-family baseline".into(),
                        ));
                    };
                    visitor(
                        &DynamicValueAddress::whole_family(address.owner(), family)?,
                        base,
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Sum shared legacy paths in reverse topological order, retaining release influence.
    /// Typed fragments use family composition and do not become fabricated scalar zeros.
    pub fn visit_legacy_contributions(
        &self,
        mut visitor: impl FnMut(&AttributeKey, f32, f32),
    ) -> bool {
        if let Some((attribute, value)) = self.legacy_leaf() {
            visitor(attribute, value, 1.0);
            return true;
        }
        let Ok(nodes) = ExpressionNodeRef::new(self).postorder(true) else {
            return false;
        };
        let indices = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.key(), i))
            .collect::<std::collections::HashMap<_, _>>();
        let mut weights = vec![0.0_f64; nodes.len()];
        let Some(root_weight) = weights.last_mut() else {
            return true;
        };
        *root_weight = 1.0;
        let mut values = Vec::<(&AttributeKey, f64, f64)>::new();
        let mut only_legacy = true;
        for index in (0..nodes.len()).rev() {
            let weight = weights[index];
            if weight == 0.0 {
                continue;
            }
            match nodes[index].node() {
                Ok(ExpressionNode::Legacy(attribute, value, ..)) => {
                    if let Some((_, total, influence)) =
                        values.iter_mut().find(|(key, _, _)| *key == attribute)
                    {
                        *total += f64::from(value) * weight;
                        *influence += weight;
                    } else {
                        values.push((attribute, f64::from(value) * weight, weight));
                    }
                }
                Ok(ExpressionNode::Transition {
                    from, to, progress, ..
                }) => {
                    for (child, factor) in
                        [(from, 1.0 - f64::from(progress)), (to, f64::from(progress))]
                    {
                        if factor != 0.0
                            && let Some(child) = child
                        {
                            let Some(&child_index) = indices.get(&child.key()) else {
                                return false;
                            };
                            weights[child_index] += weight * factor;
                        }
                    }
                }
                _ => only_legacy = false,
            }
        }
        for (attribute, total, influence) in values {
            if influence > 0.0 {
                visitor(attribute, (total / influence) as f32, influence as f32);
            }
        }
        only_legacy
    }
}
