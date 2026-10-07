//! A bounded numeric Angle program pinned at one sampling instant. It contains no clock or
//! random generator: destination composition supplies one adopted scalar Current and evaluates
//! the same arithmetic for each physical copy. Current markers are never root-fixture results.
use super::*;
use light_core::programming::{
    IntentError, ProgrammingComponent, ProgrammingOwner, TransitionError, TransitionRequirement,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Persisted programs are small topological DAGs, bounded independently of recursive history.
pub const ANGLE_NUMERIC_MAX_NODES: usize = 128;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AngleNumericProgram {
    pub address: DynamicValueAddress,
    pub occurrence: Option<DynamicSourceOccurrenceId>,
    pub nodes: Vec<AngleNumericNode>,
    pub root: u32,
    /// Producer origins of this program's keyframe transition and controller `ScaleFrom`.
    /// Process-local and excluded from equality; retained tapes keep them in their table.
    #[serde(skip)]
    pub(crate) operations: AngleNumericOperationOrigins,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AngleNumericNode {
    Materialized {
        value: DynamicValue,
        dependency_occurrence: Option<DynamicSourceDependency>,
    },
    Current,
    Transition {
        from: u32,
        to: u32,
        progress: f32,
    },
    ScaleFrom {
        pivot: u32,
        value: u32,
        factor: f32,
    },
    WaveBetween {
        low: u32,
        high: u32,
        amount: f32,
        size: f32,
    },
    Around {
        middle: u32,
        amplitude: DynamicValue,
        amount: f64,
    },
}

/// Runtime-only applicability: an eligible but absent source must not fall through and perform
/// an eager scalar Current read on the root fixture.
pub enum AngleNumericSample {
    NotApplicable,
    Absent,
    Program(Arc<AngleNumericProgram>),
}

impl AngleNumericNode {
    fn children(&self) -> [Option<u32>; 2] {
        match *self {
            Self::Transition { from, to, .. } => [Some(from), Some(to)],
            Self::ScaleFrom { pivot, value, .. } => [Some(pivot), Some(value)],
            Self::WaveBetween { low, high, .. } => [Some(low), Some(high)],
            Self::Around { middle, .. } => [Some(middle), None],
            Self::Materialized { .. } | Self::Current => [None, None],
        }
    }
}

impl AngleNumericProgram {
    /// Cold validation of the complete owned DAG. Every node must reach the root, every child
    /// precedes its parent, and all scalar operations are finite in a Pan/Tilt degree address.
    pub fn validate(&self) -> Result<(), IntentError> {
        address::ensure(
            self.address.representation == DynamicFamilyRepresentation::Angles
                && matches!(
                    self.address.component,
                    Some(ProgrammingComponent::Pan | ProgrammingComponent::Tilt)
                ),
            "numeric Angle programs require a Pan or Tilt component",
        )?;
        address::ensure(
            !self.nodes.is_empty() && self.nodes.len() <= ANGLE_NUMERIC_MAX_NODES,
            "numeric Angle program node count is outside its bound",
        )?;
        address::ensure(
            (self.root as usize) < self.nodes.len(),
            "numeric Angle program root is invalid",
        )?;
        let compiled = CompiledDynamicValueAddress::new(self.address.clone(), None)?;
        for (index, node) in self.nodes.iter().enumerate() {
            for child in node.children().into_iter().flatten() {
                address::ensure(
                    (child as usize) < index,
                    "numeric Angle children must precede their parent",
                )?;
            }
            match node {
                AngleNumericNode::Materialized {
                    value,
                    dependency_occurrence,
                } => {
                    compiled.validate_source_value(value)?;
                    if let Some(dependency) = dependency_occurrence {
                        dependency.validate(ProgrammingOwner::Position)?;
                    }
                }
                AngleNumericNode::Current => {}
                AngleNumericNode::Transition { progress, .. } => {
                    address::ensure(
                        progress.is_finite() && (0.0..=1.0).contains(progress),
                        "numeric Angle transition progress is invalid",
                    )?;
                }
                AngleNumericNode::ScaleFrom { factor, .. } => {
                    address::ensure(
                        factor.is_finite() && *factor >= 0.,
                        "numeric Angle scale is invalid",
                    )?;
                }
                AngleNumericNode::WaveBetween { amount, size, .. } => {
                    address::ensure(
                        amount.is_finite()
                            && (0.0..=1.0).contains(amount)
                            && size.is_finite()
                            && *size >= 0.,
                        "numeric Angle wave is invalid",
                    )?;
                }
                AngleNumericNode::Around {
                    amplitude, amount, ..
                } => {
                    address::ensure(
                        matches!(amplitude, DynamicValue::Scalar(value) if value.is_finite() && *value >= 0.)
                            && amount.is_finite(),
                        "numeric Angle amplitude is invalid",
                    )?;
                }
            }
        }
        let mut reachable = vec![false; self.nodes.len()];
        reachable[self.root as usize] = true;
        for index in (0..self.nodes.len()).rev() {
            if reachable[index] {
                for child in self.nodes[index].children().into_iter().flatten() {
                    reachable[child as usize] = true;
                }
            }
        }
        address::ensure(
            reachable.iter().all(|used| *used),
            "numeric Angle program contains unreachable nodes",
        )
    }

    pub fn uses_current(&self) -> bool {
        self.nodes
            .iter()
            .any(|node| matches!(node, AngleNumericNode::Current))
    }

    /// Evaluate already pinned phase/envelope coefficients with the shared component math.
    /// No source resolver, clock, random stream or output continuity is consulted here.
    pub fn evaluate(
        &self,
        address: &CompiledDynamicValueAddress,
        current: Option<&DynamicValue>,
    ) -> Result<DynamicValue, TransitionError> {
        self.validate()?;
        address::ensure(
            address.address() == &self.address,
            "numeric Angle evaluator has a different address",
        )?;
        self.evaluate_validated(address, current)
    }

    /// The immutable destination compiler has already validated this graph and its address.
    /// Avoid recompilation and a full validation walk for every physical copy.
    pub(crate) fn evaluate_validated(
        &self,
        address: &CompiledDynamicValueAddress,
        current: Option<&DynamicValue>,
    ) -> Result<DynamicValue, TransitionError> {
        let mut values: Vec<DynamicValue> = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let value = match node {
                AngleNumericNode::Materialized { value, .. } => value.clone(),
                AngleNumericNode::Current => {
                    let value = current.ok_or(TransitionError::Requires(
                        TransitionRequirement::LiveJointAngles,
                    ))?;
                    address.validate_source_value(value)?;
                    value.clone()
                }
                AngleNumericNode::Transition { from, to, progress } => address
                    .transition(values[*from as usize].clone(), values[*to as usize].clone())?
                    .sample(*progress)?,
                AngleNumericNode::ScaleFrom {
                    pivot,
                    value,
                    factor,
                } => address.scale_from(
                    &values[*pivot as usize],
                    &values[*value as usize],
                    *factor,
                )?,
                AngleNumericNode::WaveBetween {
                    low,
                    high,
                    amount,
                    size,
                } => address.wave_between(
                    &values[*low as usize],
                    &values[*high as usize],
                    *amount,
                    *size,
                )?,
                AngleNumericNode::Around {
                    middle,
                    amplitude,
                    amount,
                } => address.around_wide(&values[*middle as usize], amplitude, *amount)?,
            };
            values.push(value);
        }
        Ok(values.swap_remove(self.root as usize))
    }

    pub fn visit_materialized_values(
        &self,
        visitor: &mut impl FnMut(&DynamicValueAddress, &DynamicValue) -> Result<(), IntentError>,
    ) -> Result<(), IntentError> {
        for node in &self.nodes {
            match node {
                AngleNumericNode::Materialized { value, .. } => visitor(&self.address, value)?,
                AngleNumericNode::Around { amplitude, .. } => visitor(&self.address, amplitude)?,
                _ => {}
            }
        }
        Ok(())
    }

    pub fn visit_dependencies(&self, visitor: &mut impl FnMut(&DynamicSourceDependency)) {
        for node in &self.nodes {
            if let AngleNumericNode::Materialized {
                dependency_occurrence: Some(dependency),
                ..
            } = node
            {
                visitor(dependency);
            }
        }
    }
}

#[cfg(test)]
mod tests;
