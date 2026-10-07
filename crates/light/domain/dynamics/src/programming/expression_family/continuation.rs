//! Runtime-only suspension of an immutable, already sampled whole-family graph.
use super::*;

/// One graph-local blocked operation. The node does not identify a cut across owners,
/// fixtures or independently compiled graphs. Original endpoints remain untouched.
#[derive(Clone)]
pub struct FamilyMaterializationRequest {
    pub node: usize,
    pub requirement: TransitionRequirement,
    pub operation: FamilyMaterializationOperation,
}

#[derive(Clone)]
pub enum FamilyMaterializationOperation {
    Underlay,
    Transition {
        from_node: usize,
        to_node: usize,
        from: AttributeValue,
        to: AttributeValue,
        progress: f32,
        reason: DynamicTransitionReason,
    },
    Scale {
        base: AttributeValue,
        value_node: usize,
        value: AttributeValue,
        factor: f32,
        baseline_occurrence: Option<super::super::DynamicSourceOccurrenceId>,
    },
}

pub enum FamilyEvaluationProgress {
    Complete(Option<AttributeValue>),
    NeedsMaterialization(FamilyMaterializationRequest),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GraphOperationKind {
    Resume,
    Required,
    Size,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GraphOperationOperand {
    ResumeOutgoing,
    ResumeIncoming,
    RequiredOutgoing,
    RequiredIncoming,
    SizeBaseline,
    SizeValue,
}
impl GraphOperationOperand {
    pub(crate) fn supports(self, kind: GraphOperationKind) -> bool {
        matches!(
            (self, kind),
            (
                Self::ResumeOutgoing | Self::ResumeIncoming,
                GraphOperationKind::Resume
            ) | (
                Self::RequiredOutgoing | Self::RequiredIncoming,
                GraphOperationKind::Required
            ) | (
                Self::SizeBaseline | Self::SizeValue,
                GraphOperationKind::Size
            )
        )
    }
}
/// Opt-in result of an actual dependency-ready reached Required/Size site. Materialize still
/// evaluates the original operation: a hard error propagates and a natural request stays as is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GraphReachedAction {
    Continue,
    Materialize,
}
impl GraphReachedAction {
    /// A completed inline value is discarded so the ordinary request path exposes the original
    /// operation with its endpoints. No value, clock or producer is sampled again.
    pub(crate) fn apply<T>(self, result: Result<T, TransitionError>) -> Result<T, TransitionError> {
        match (self, result) {
            (Self::Materialize, Ok(_)) => Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            )),
            (_, result) => result,
        }
    }
}
pub(crate) type GraphOperationReachedCallback<'a> =
    dyn FnMut(usize, GraphOperationKind) -> Result<GraphReachedAction, TransitionError> + 'a;
pub(crate) enum GraphOperationProgress<P> {
    Ordinary(P),
    OperandReady(AttributeValue),
}
#[derive(Clone, Copy)]
pub(crate) struct GraphOperationSelection {
    pub operand: GraphOperationOperand,
    pub depth: usize,
}
pub(crate) type GraphOperationPlanCallback<'a> = dyn FnMut(usize, GraphOperationKind) -> Result<Vec<GraphOperationSelection>, TransitionError>
    + 'a;
struct GraphOperandReplay {
    node: usize,
    operand: GraphOperationOperand,
    needed: Vec<bool>,
    depth: usize,
}

/// Completed nodes, original native source models and pending observer evidence survive
/// a yield. Keep the same coherent frame and observer arena while servicing requests.
/// This retains no outer compositor stack and provides no cross-owner cut identity.
pub struct FamilyEvaluationContinuation {
    expression: Arc<CompiledProgrammingFamilyExpression>,
    values: Vec<Option<AttributeValue>>,
    next_node: usize,
    materialization: Option<FamilyMaterializationRequest>,
    response: Option<(AttributeValue, Option<ProgrammingTransitionTrace>)>,
    failed: bool,
    graph_probe_configured: bool,
    graph_operand: Option<GraphOperandReplay>,
    graph_operand_ready: Option<AttributeValue>,
    graph_reached: Option<(Vec<bool>, Vec<bool>)>,
}

impl CompiledProgrammingFamilyExpression {
    /// Capture the eligible underlay once; subsequent yields cannot replace it with a
    /// newer frame's value. An absent required underlay yields a node-zero request.
    pub fn begin_evaluation(
        self: &Arc<Self>,
        eligible_underlay: Option<&AttributeValue>,
    ) -> Result<FamilyEvaluationContinuation, TransitionError> {
        let mut values = vec![None; self.graph.nodes.len()];
        if self.needs_underlay() {
            if let Some(value) = eligible_underlay {
                validate_whole(self.owner, value)?;
                values[0] = Some(value.clone());
            }
        }
        Ok(FamilyEvaluationContinuation {
            expression: self.clone(),
            values,
            next_node: if self.needs_underlay() { 0 } else { 1 },
            materialization: None,
            response: None,
            failed: false,
            graph_probe_configured: false,
            graph_operand: None,
            graph_operand_ready: None,
            graph_reached: None,
        })
    }
}

impl FamilyEvaluationContinuation {
    pub fn pending_materialization(&self) -> Option<&FamilyMaterializationRequest> {
        self.materialization.as_ref()
    }

    /// Invalid node, family, address or native-model proof leaves the request intact.
    /// The value is this operation's complete result, not a replacement authored endpoint.
    pub fn resume_materialization(
        &mut self,
        node: usize,
        value: AttributeValue,
        trace: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        if self.failed {
            return Err(
                IntentError("failed whole-family evaluation must be discarded".into()).into(),
            );
        }
        // Validation may call the retained native model. A normal invalid response stays
        // retryable; an unwinding callback permanently poisons this evaluation episode.
        self.failed = true;
        let result = self.resume_materialization_impl(node, value, trace);
        self.failed = false;
        result
    }

    fn resume_materialization_impl(
        &mut self,
        node: usize,
        value: AttributeValue,
        trace: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        let request = self.materialization.as_ref().ok_or_else(|| {
            IntentError("whole-family evaluation has no pending materialization".into())
        })?;
        if request.node != node {
            return Err(IntentError(
                "whole-family materialization response names another graph node".into(),
            )
            .into());
        }
        validate_whole(self.expression.owner, &value)?;
        if let AttributeValue::ColorProgram(program) = &value
            && let ColorProgram::Direct { recipe, .. } = program.as_ref()
        {
            let model = self
                .expression
                .resolve_original_native_model(&recipe.source)?;
            validate_original_recipe(recipe, model.as_ref())?;
        }
        self.response = Some((value, trace));
        self.materialization = None;
        Ok(())
    }

    /// Crate-only original graph operand replay. The selected child alone is evaluated;
    /// its authentic dependencies can suspend, but an unrelated sibling cannot block it.
    pub(crate) fn advance_with_graph_operand_and_reached(
        &mut self,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        observer: Option<&mut dyn FamilyExpressionObserver>,
        selector: &mut dyn FnMut(
            usize,
            GraphOperationKind,
        ) -> Result<Option<GraphOperationOperand>, TransitionError>,
        reached: Option<&mut GraphOperationReachedCallback<'_>>,
    ) -> Result<GraphOperationProgress<FamilyEvaluationProgress>, TransitionError> {
        let mut plan = |node, kind| {
            Ok(selector(node, kind)?
                .into_iter()
                .map(|operand| GraphOperationSelection { operand, depth: 0 })
                .collect())
        };
        self.advance_with_graph_plan_and_reached(frame, observer, &mut plan, reached)
    }
    pub(crate) fn graph_selected_depth(&self) -> Option<usize> {
        self.graph_operand.as_ref().map(|replay| replay.depth)
    }
    pub(crate) fn advance_with_graph_plan_and_reached(
        &mut self,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        observer: Option<&mut dyn FamilyExpressionObserver>,
        selector: &mut GraphOperationPlanCallback<'_>,
        reached: Option<&mut GraphOperationReachedCallback<'_>>,
    ) -> Result<GraphOperationProgress<FamilyEvaluationProgress>, TransitionError> {
        if self.failed {
            return Err(
                IntentError("failed whole-family evaluation must be discarded".into()).into(),
            );
        }
        self.failed = true;
        let result = (|| {
            if !self.graph_probe_configured {
                // Retained maps alone do not establish reachability after conditioning.
                let mut reachable = vec![false; self.expression.graph.nodes.len()];
                let mut pending = vec![self.expression.graph.root];
                while let Some(node) = pending.pop() {
                    if reachable[node] {
                        continue;
                    }
                    reachable[node] = true;
                    match &self.expression.graph.nodes[node] {
                        Node::Transition { from, to, .. } => {
                            pending.push(*from);
                            pending.push(*to);
                        }
                        Node::Scale { value, .. } => pending.push(*value),
                        _ => {}
                    }
                }
                let mut candidates = Vec::new();
                for (node, value) in self.expression.graph.nodes.iter().enumerate() {
                    if !reachable[node] {
                        continue;
                    }
                    let kind = match value {
                        Node::Transition {
                            reason: DynamicTransitionReason::Required { .. },
                            ..
                        } => GraphOperationKind::Required,
                        Node::Transition {
                            reason: DynamicTransitionReason::Resume { .. },
                            ..
                        } => GraphOperationKind::Resume,
                        Node::Scale { .. } => GraphOperationKind::Size,
                        _ => continue,
                    };
                    for selection in selector(node, kind)? {
                        ensure_graph(
                            selection.operand.supports(kind),
                            "graph operand has another operation role",
                        )?;
                        candidates.push((node, selection));
                    }
                }
                candidates.sort_by_key(|(_, selection)| selection.depth);
                let mut selected = None;
                let mut current_depth = None;
                let mut depth_reachable = reachable.clone();
                for (node, selection) in candidates {
                    if current_depth != Some(selection.depth) {
                        depth_reachable.clone_from(&reachable);
                    }
                    if !depth_reachable[node] {
                        continue;
                    }
                    ensure_graph(
                        current_depth != Some(selection.depth),
                        "graph operand has ambiguous compiled locations",
                    )?;
                    current_depth = Some(selection.depth);
                    let operand = selection.operand;
                    let root = match (&self.expression.graph.nodes[node], operand) {
                        (
                            Node::Transition { from, .. },
                            GraphOperationOperand::RequiredOutgoing
                            | GraphOperationOperand::ResumeOutgoing,
                        ) => Some(*from),
                        (
                            Node::Transition { to, .. },
                            GraphOperationOperand::RequiredIncoming
                            | GraphOperationOperand::ResumeIncoming,
                        ) => Some(*to),
                        (Node::Scale { value, .. }, GraphOperationOperand::SizeValue) => {
                            Some(*value)
                        }
                        (Node::Scale { .. }, GraphOperationOperand::SizeBaseline) => None,
                        _ => unreachable!("validated graph operand"),
                    };
                    let mut needed = vec![false; self.expression.graph.nodes.len()];
                    let mut pending = root.into_iter().collect::<Vec<_>>();
                    while let Some(child) = pending.pop() {
                        if needed[child] {
                            continue;
                        }
                        needed[child] = true;
                        match &self.expression.graph.nodes[child] {
                            Node::Transition { from, to, .. } => {
                                pending.push(*from);
                                pending.push(*to);
                            }
                            Node::Scale { value, .. } => pending.push(*value),
                            _ => {}
                        }
                    }
                    reachable.clone_from(&needed);
                    selected = Some(GraphOperandReplay {
                        node,
                        operand,
                        needed,
                        depth: selection.depth,
                    });
                }
                self.graph_operand = selected;
                self.graph_probe_configured = true;
            }
            if let Some(value) = &self.graph_operand_ready {
                return Ok(GraphOperationProgress::OperandReady(value.clone()));
            }
            let progress = self.advance_impl(frame, observer, reached)?;
            Ok(match &self.graph_operand_ready {
                Some(value) => GraphOperationProgress::OperandReady(value.clone()),
                None => GraphOperationProgress::Ordinary(progress),
            })
        })();
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    pub fn advance(
        &mut self,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        observer: Option<&mut dyn FamilyExpressionObserver>,
    ) -> Result<FamilyEvaluationProgress, TransitionError> {
        self.advance_with_reached(frame, observer, None)
    }
    pub(crate) fn advance_with_reached(
        &mut self,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        observer: Option<&mut dyn FamilyExpressionObserver>,
        reached: Option<&mut GraphOperationReachedCallback<'_>>,
    ) -> Result<FamilyEvaluationProgress, TransitionError> {
        if self.failed {
            return Err(
                IntentError("failed whole-family evaluation must be discarded".into()).into(),
            );
        }
        ensure_graph(
            !self.graph_probe_configured,
            "graph operand cannot use ordinary evaluation",
        )?;
        // Set the terminal guard before invoking any model, resolver or observer. If
        // the caller catches a panic, this half-evaluated node cannot be invoked again.
        self.failed = true;
        let result = self.advance_impl(frame, observer, reached);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    /// Only opt-in collection builds this root census. Ordinary table evaluation stays
    /// unchanged, but detached imported nodes cannot issue reached authority.
    fn reached_action(
        &mut self,
        node: usize,
        reached: Option<&mut GraphOperationReachedCallback<'_>>,
    ) -> Result<GraphReachedAction, TransitionError> {
        let Some(callback) = reached.filter(|_| self.response.is_none()) else {
            return Ok(GraphReachedAction::Continue);
        };
        let graph = &self.expression.graph;
        let (reachable, seen) = self.graph_reached.get_or_insert_with(|| {
            let mut reachable = vec![false; graph.nodes.len()];
            let mut pending = vec![graph.root];
            while let Some(id) = pending.pop() {
                if reachable[id] {
                    continue;
                }
                reachable[id] = true;
                match &graph.nodes[id] {
                    Node::Transition { from, to, .. } => {
                        pending.push(*from);
                        pending.push(*to);
                    }
                    Node::Scale { value, .. } => pending.push(*value),
                    _ => {}
                }
            }
            (reachable, vec![false; graph.nodes.len()])
        });
        let kind = match &graph.nodes[node] {
            Node::Transition {
                reason: DynamicTransitionReason::Required { .. },
                ..
            } => GraphOperationKind::Required,
            Node::Scale { .. } => GraphOperationKind::Size,
            _ => return Ok(GraphReachedAction::Continue),
        };
        if !reachable[node] || seen[node] {
            return Ok(GraphReachedAction::Continue);
        }
        // Mark before invoking callbacks: a yield/response never repeats it.
        seen[node] = true;
        callback(node, kind)
    }

    fn advance_impl(
        &mut self,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        mut observer: Option<&mut dyn FamilyExpressionObserver>,
        mut reached: Option<&mut GraphOperationReachedCallback<'_>>,
    ) -> Result<FamilyEvaluationProgress, TransitionError> {
        if let Some(request) = &self.materialization {
            return Ok(FamilyEvaluationProgress::NeedsMaterialization(
                request.clone(),
            ));
        }
        if !self.expression.participates() {
            return Ok(FamilyEvaluationProgress::Complete(None));
        }
        while self.next_node < self.expression.graph.nodes.len() {
            let node = self.next_node;
            if self
                .graph_operand
                .as_ref()
                .is_some_and(|replay| !replay.needed[node])
            {
                self.next_node += 1;
                continue;
            }
            let action = self.reached_action(node, reached.as_deref_mut())?;
            let evaluated =
                if let Some(response) = self.response.take() {
                    Ok(response)
                } else if node == 0 {
                    self.values[0].clone().map(|value| (value, None)).ok_or(
                        TransitionError::Requires(TransitionRequirement::MaterializedEndpoints),
                    )
                } else {
                    action.apply(self.expression.graph.node_result(
                        node,
                        self.expression.owner,
                        &self.values,
                        &self.expression.models,
                        frame,
                        observer.is_some(),
                    ))
                };
            let (value, transfer) = match evaluated {
                Ok(result) => result,
                Err(TransitionError::Requires(requirement)) => {
                    let operation = match &self.expression.graph.nodes[node] {
                        Node::Underlay => FamilyMaterializationOperation::Underlay,
                        Node::Transition {
                            from,
                            to,
                            progress,
                            reason,
                            ..
                        } => FamilyMaterializationOperation::Transition {
                            from_node: *from,
                            to_node: *to,
                            from: self.values[*from]
                                .as_ref()
                                .expect("completed outgoing child")
                                .clone(),
                            to: self.values[*to]
                                .as_ref()
                                .expect("completed incoming child")
                                .clone(),
                            progress: *progress,
                            reason: *reason,
                        },
                        Node::Scale {
                            base,
                            value,
                            factor,
                            baseline_occurrence,
                            ..
                        } => FamilyMaterializationOperation::Scale {
                            base: base.clone(),
                            value_node: *value,
                            value: self.values[*value]
                                .as_ref()
                                .expect("completed Size child")
                                .clone(),
                            factor: *factor,
                            baseline_occurrence: *baseline_occurrence,
                        },
                        Node::Value { .. } | Node::Baseline { .. } => {
                            unreachable!("validated constant node")
                        }
                    };
                    let request = FamilyMaterializationRequest {
                        node,
                        requirement,
                        operation,
                    };
                    self.materialization = Some(request.clone());
                    return Ok(FamilyEvaluationProgress::NeedsMaterialization(request));
                }
                Err(error) => {
                    self.failed = true;
                    return Err(error);
                }
            };
            if let Some(observer) = observer.as_deref_mut() {
                if let Err(error) =
                    self.expression
                        .graph
                        .observe_node(node, &value, transfer.as_ref(), observer)
                {
                    self.failed = true;
                    return Err(error);
                }
            }
            self.values[node] = Some(value);
            self.next_node += 1;
        }
        if let Some(replay) = &self.graph_operand {
            let value = match (&self.expression.graph.nodes[replay.node], replay.operand) {
                (
                    Node::Transition { from, .. },
                    GraphOperationOperand::RequiredOutgoing | GraphOperationOperand::ResumeOutgoing,
                ) => self.values[*from]
                    .as_ref()
                    .expect("replayed outgoing child")
                    .clone(),
                (
                    Node::Transition { to, .. },
                    GraphOperationOperand::RequiredIncoming | GraphOperationOperand::ResumeIncoming,
                ) => self.values[*to]
                    .as_ref()
                    .expect("replayed incoming child")
                    .clone(),
                (Node::Scale { base, .. }, GraphOperationOperand::SizeBaseline) => base.clone(),
                (Node::Scale { value, .. }, GraphOperationOperand::SizeValue) => self.values
                    [*value]
                    .as_ref()
                    .expect("replayed Size child")
                    .clone(),
                _ => unreachable!("validated graph operand"),
            };
            self.graph_operand_ready = Some(value.clone());
            return Ok(FamilyEvaluationProgress::Complete(Some(value)));
        }
        Ok(FamilyEvaluationProgress::Complete(Some(
            self.values[self.expression.graph.root]
                .as_ref()
                .expect("evaluated family root")
                .clone(),
        )))
    }
}

fn ensure_graph(condition: bool, message: &str) -> Result<(), TransitionError> {
    if condition {
        Ok(())
    } else {
        Err(IntentError(message.into()).into())
    }
}

#[cfg(test)]
mod tests;
