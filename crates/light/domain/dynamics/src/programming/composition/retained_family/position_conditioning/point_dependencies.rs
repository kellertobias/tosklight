//! Original Point references only. No achieved angles, source reads or history expansion.
use super::*;
use crate::programming::expression::{ExpressionNode, ExpressionNodeRef};
use crate::programming::expression_coupled::PositionAngleAxis;
use crate::{
    AngleNumericProgram, CoupledCohortEndpoint, CoupledComponentEndpoint, CoupledLeafRole,
};
use std::collections::BTreeSet;

/// A conservative census, not the minimal currently contributing branch. Missing Points
/// remain listed so deletion and recreation can invalidate their original dependents.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PositionPointDependencies {
    point_ids: Box<[Uuid]>,
    incomplete: bool,
}
impl PositionPointDependencies {
    pub fn point_ids(&self) -> &[Uuid] {
        &self.point_ids
    }
    /// Some symbolic Current/source lacks an enumerable original family. An inverse index
    /// must conservatively invalidate this owner instead of treating the list as exhaustive.
    pub fn incomplete(&self) -> bool {
        self.incomplete
    }
}

enum Task<'a> {
    Expression(ExpressionNodeRef<'a>),
    Forest(&'a [PositionForestNode], usize),
}
#[derive(Default)]
struct Census {
    ids: BTreeSet<Uuid>,
    incomplete: bool,
    expressions: HashSet<(usize, Option<RetainedNodeId>)>,
    forests: HashSet<(usize, usize)>,
    numeric: HashSet<(usize, bool)>,
}
impl Census {
    fn reference(&mut self, reference: &TargetReference) {
        if let TargetReference::Point { point_id } = reference {
            self.ids.insert(*point_id);
        }
    }
    fn family(&mut self, value: &AttributeValue) {
        if let AttributeValue::Position(value) = value {
            if let PositionIntent::Target { reference, .. } = value.as_ref() {
                self.reference(reference);
            }
        }
    }
    fn value(&mut self, value: &DynamicValue) {
        if let DynamicValue::Family(value) = value {
            self.family(value);
        }
    }
    fn address(&mut self, address: &DynamicValueAddress) {
        if let DynamicFamilyRepresentation::Target { reference } = &address.representation {
            if let Some(reference) = reference {
                self.reference(reference);
            } else if address.component.is_some() {
                self.incomplete = true;
            }
        }
    }
    fn component(&mut self, component: &CoupledComponentEndpoint) {
        self.address(component.address.address());
        self.value(&component.value);
        if component.role == CoupledLeafRole::Current
            && !matches!(
                &component.value,
                DynamicValue::Family(AttributeValue::Position(_))
            )
        {
            self.incomplete = true;
        }
    }
    fn numeric(&mut self, program: &AngleNumericProgram, original_current_known: bool) {
        if !self
            .numeric
            .insert((program as *const _ as usize, original_current_known))
        {
            return;
        }
        self.address(&program.address);
        if program.uses_current() && !original_current_known {
            self.incomplete = true;
        }
    }
    fn member<'a>(&mut self, member: &'a CoupledCohortEndpoint, tasks: &mut Vec<Task<'a>>) {
        match member {
            CoupledCohortEndpoint::Materialized(component) => self.component(component),
            CoupledCohortEndpoint::WholeExpression { expression, .. } => {
                tasks.push(Task::Expression(ExpressionNodeRef::new(
                    expression.expression(),
                )));
            }
        }
    }
}

pub(super) fn collect(
    captured: &CapturedProgram,
) -> Result<PositionPointDependencies, TransitionError> {
    let mut census = Census::default();
    census.family(&captured.base);
    let mut tasks = Vec::new();
    for (index, source) in captured.sources.iter().enumerate() {
        match source {
            FamilyCompositionSample::Known(source) => {
                census.address(source.address.address());
                match &source.body {
                    FamilySampleBody::Materialized(value) => census.value(value),
                    FamilySampleBody::ComponentExpression(expression) => {
                        tasks.push(Task::Expression(ExpressionNodeRef::new(
                            expression.expression(),
                        )));
                    }
                }
                if let Some(original) = &source.projection {
                    tasks.push(Task::Expression(ExpressionNodeRef::new(original)));
                }
                if source.trace_sources.as_ref().is_some_and(|sources| {
                    sources
                        .iter()
                        .any(|source| matches!(source, trace::FamilyTraceLeaf::Current(_, _)))
                }) {
                    // Trace-only scalar Current does not preserve its original Target family.
                    census.incomplete = true;
                }
            }
            FamilyCompositionSample::WholeExpression { expression, .. } => {
                tasks.push(Task::Expression(ExpressionNodeRef::new(
                    expression.expression(),
                )));
            }
            FamilyCompositionSample::CoupledExpression { expression, .. } => {
                if let Some(forest) = &captured.forests[index] {
                    tasks.push(Task::Forest(&forest.nodes, forest.root));
                } else if let Some(original) = expression.expression() {
                    tasks.push(Task::Expression(ExpressionNodeRef::new(original)));
                } else {
                    census.incomplete = true;
                }
            }
        }
    }
    while let Some(task) = tasks.pop() {
        match task {
            Task::Expression(expression) => {
                if !census.expressions.insert(expression.key()) {
                    continue;
                }
                match expression.node()? {
                    ExpressionNode::Programming(address, value, ..) => {
                        census.address(address);
                        census.value(value);
                    }
                    ExpressionNode::Legacy(..) => {}
                    ExpressionNode::Current(address) => {
                        census.address(address);
                        census.incomplete = true;
                    }
                    ExpressionNode::Numeric(program) => census.numeric(program, false),
                    ExpressionNode::Scale {
                        address,
                        base,
                        value,
                        ..
                    } => {
                        census.address(address);
                        census.value(base);
                        tasks.push(Task::Expression(value));
                    }
                    ExpressionNode::Transition { from, to, .. } => {
                        tasks.extend(from.into_iter().chain(to).map(Task::Expression));
                    }
                }
            }
            Task::Forest(nodes, index) => {
                if !census.forests.insert((nodes.as_ptr() as usize, index)) {
                    continue;
                }
                match nodes.get(index).ok_or_else(|| {
                    IntentError("Position dependency forest node is absent".into())
                })? {
                    PositionForestNode::AnglePair(pair) => {
                        for axis in &pair.axes {
                            match axis {
                                PositionAngleAxis::Materialized(component) => {
                                    census.component(component)
                                }
                                PositionAngleAxis::Current {
                                    address, original, ..
                                } => {
                                    census.address(address.address());
                                    census.family(&original.value);
                                }
                                PositionAngleAxis::Numeric {
                                    address,
                                    original,
                                    program,
                                    ..
                                } => {
                                    census.address(address.address());
                                    census.family(&original.value);
                                    census.numeric(program, true);
                                }
                            }
                        }
                    }
                    PositionForestNode::Whole {
                        expression,
                        sources,
                        ..
                    } => {
                        tasks.push(Task::Expression(ExpressionNodeRef::new(expression)));
                        for source in sources.iter() {
                            census.component(source);
                        }
                    }
                    PositionForestNode::Cohort(components) => {
                        for component in components.iter() {
                            census.component(component);
                        }
                    }
                    PositionForestNode::SourceCohort(members) => {
                        for member in members.iter() {
                            census.member(member, &mut tasks);
                        }
                    }
                    PositionForestNode::Transition { from, to, .. } => {
                        tasks.extend(
                            from.iter()
                                .chain(to)
                                .map(|&index| Task::Forest(nodes, index)),
                        );
                    }
                }
            }
        }
    }
    Ok(PositionPointDependencies {
        point_ids: census
            .ids
            .into_iter()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        incomplete: census.incomplete,
    })
}

#[cfg(test)]
#[path = "point_dependencies_tests.rs"]
mod tests;
