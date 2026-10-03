//! Whole-family evaluation after lane footprint arbitration. This retains the authored
//! expression and resolves live frame requirements without fitting any destination fixture.
//! Component lanes and Angle Current tokens must first be bundled by their own compositor.
use super::{DynamicSampleExpression, DynamicValue};
use crate::{
    DynamicNativeModelResolver, DynamicTransitionReason, RetainedExpressionNode,
    RetainedExpressionTape, RetainedNodeId,
};
use light_core::{AttributeValue, NativeColorIdentity, programming::*};
use std::sync::Arc;

mod continuation;
pub use continuation::*;

/// The frame callback receives original whole-owner values and one required operation. It may
/// solve current world Points/joints, modeled Color appearance or fixture Zoom conventions from
/// one coherent frame. Unknown appearance must remain an explicit `Requires(ColorAppearance)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FamilyExpressionOperation {
    Transition { progress: f32 },
    Scale { factor: f32 },
}

pub trait WholeFamilyExpressionFrameResolver {
    /// Adopt one complete original captured Position Current into this destination's Angle
    /// pair. Runtime forests call this only during evaluation, never during preparation.
    fn adopt_position_angles(
        &self,
        _original: &AttributeValue,
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    }

    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError>;

    /// Produce value and field transfers in the same frame solve. A resolver without exact
    /// transfer evidence keeps its successful value and explicitly leaves the trace unknown.
    fn resolve_with_trace(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        self.resolve(requirement, from, to, operation)
            .map(|value| (value, None))
    }
}

/// Optional per-node evidence emitted during the same iterative value evaluation. Node IDs
/// refer to this compiled graph only; callers retain their own trace arena across frames.
pub enum FamilyExpressionStep<'a> {
    Underlay,
    Authored {
        value: &'a AttributeValue,
        occurrence: Option<super::DynamicSourceOccurrenceId>,
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    Baseline {
        value: &'a AttributeValue,
        occurrence: Option<super::DynamicSourceOccurrenceId>,
    },
    Scale {
        base: &'a AttributeValue,
        value: usize,
        factor: f32,
        baseline_occurrence: Option<super::DynamicSourceOccurrenceId>,
        trace: Option<&'a ProgrammingTransitionTrace>,
    },
    Transition {
        from: usize,
        to: usize,
        progress: f32,
        reason: DynamicTransitionReason,
        trace: Option<&'a ProgrammingTransitionTrace>,
    },
}

pub trait FamilyExpressionObserver {
    fn evaluated(
        &mut self,
        node: usize,
        step: FamilyExpressionStep<'_>,
        value: &AttributeValue,
    ) -> Result<(), TransitionError>;
}

enum Node {
    Underlay,
    Value {
        value: AttributeValue,
        occurrence: Option<super::DynamicSourceOccurrenceId>,
        dependency_occurrence: Option<crate::DynamicSourceDependency>,
    },
    Baseline {
        value: AttributeValue,
        occurrence: Option<super::DynamicSourceOccurrenceId>,
    },
    Scale {
        base: AttributeValue,
        value: usize,
        factor: f32,
        baseline_occurrence: Option<super::DynamicSourceOccurrenceId>,
        cached: Option<CompiledProgrammingTransition>,
    },
    Transition {
        from: usize,
        to: usize,
        progress: f32,
        reason: DynamicTransitionReason,
        cached: Option<CompiledProgrammingTransition>,
    },
}
impl Node {
    fn constant(&self) -> Option<&AttributeValue> {
        match self {
            Self::Value { value, .. } | Self::Baseline { value, .. } => Some(value),
            _ => None,
        }
    }
}
pub(super) fn retained_reachability(
    tape: &RetainedExpressionTape,
    root: RetainedNodeId,
    prune: bool,
) -> Vec<bool> {
    let mut reachable = vec![false; tape.nodes.len()];
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if std::mem::replace(&mut reachable[id.0 as usize], true) {
            continue;
        }
        match &tape.nodes[id.0 as usize] {
            RetainedExpressionNode::Transition {
                from, to, progress, ..
            } if prune => {
                if *progress != 1.0 {
                    pending.extend(from);
                }
                if *progress != 0.0 {
                    pending.extend(to);
                }
            }
            RetainedExpressionNode::Scale { value, factor, .. } if prune => {
                if *factor != 0.0 {
                    pending.push(*value);
                }
            }
            node => pending.extend(node.children()),
        }
    }
    reachable
}
struct CompiledFamilyGraph {
    nodes: Vec<Node>,
    retained_node_origins: Vec<Option<RetainedNodeId>>,
    root: usize,
    needs_underlay: bool,
}
impl CompiledFamilyGraph {
    fn compile(tape: &RetainedExpressionTape, root: RetainedNodeId) -> Self {
        let active = retained_reachability(tape, root, true);
        let mut nodes = vec![Node::Underlay];
        let mut retained_node_origins = vec![None];
        let mut underlay = vec![true];
        let mut ids = vec![None; tape.nodes.len()];
        for (index, source) in tape.nodes.iter().enumerate() {
            if !active[index] {
                continue;
            }
            let child = |id: RetainedNodeId| ids[id.0 as usize].expect("compiled earlier child");
            let node = match source {
                RetainedExpressionNode::Programming {
                    value: DynamicValue::Family(value),
                    occurrence,
                    dependency_occurrence,
                    ..
                } => Node::Value {
                    value: value.clone(),
                    occurrence: *occurrence,
                    dependency_occurrence: dependency_occurrence.clone(),
                },
                RetainedExpressionNode::Scale {
                    base: DynamicValue::Family(base),
                    value,
                    factor,
                    baseline_occurrence,
                    ..
                } => {
                    if *factor == 0.0 {
                        Node::Baseline {
                            value: base.clone(),
                            occurrence: *baseline_occurrence,
                        }
                    } else if *factor == 1.0 {
                        ids[index] = Some(child(*value));
                        continue;
                    } else {
                        Node::Scale {
                            base: base.clone(),
                            value: child(*value),
                            factor: *factor,
                            baseline_occurrence: *baseline_occurrence,
                            cached: None,
                        }
                    }
                }
                RetainedExpressionNode::Transition {
                    from,
                    to,
                    progress,
                    reason,
                } => {
                    if *progress == 0.0 {
                        ids[index] = Some(from.map_or(0, child));
                        continue;
                    }
                    if *progress == 1.0 {
                        ids[index] = Some(to.map_or(0, child));
                        continue;
                    }
                    let from = from.map_or(0, child);
                    let to = to.map_or(0, child);
                    if from == 0 && to == 0 {
                        ids[index] = Some(0);
                        continue;
                    }
                    Node::Transition {
                        from,
                        to,
                        progress: *progress,
                        reason: *reason,
                        cached: None,
                    }
                }
                _ => unreachable!("whole-family preflight"),
            };
            let needs = match &node {
                Node::Underlay => true,
                Node::Value { .. } | Node::Baseline { .. } => false,
                Node::Scale { value, .. } => underlay[*value],
                Node::Transition { from, to, .. } => underlay[*from] || underlay[*to],
            };
            ids[index] = Some(nodes.len());
            nodes.push(node);
            retained_node_origins.push(Some(RetainedNodeId(index as u32)));
            underlay.push(needs);
        }
        let root = ids[root.0 as usize].expect("compiled root");
        Self {
            needs_underlay: underlay[root],
            retained_node_origins,
            nodes,
            root,
        }
    }
    fn authored_values(&self) -> impl Iterator<Item = &AttributeValue> {
        self.nodes.iter().filter_map(|node| match node {
            Node::Value { value, .. }
            | Node::Baseline { value, .. }
            | Node::Scale { base: value, .. } => Some(value),
            _ => None,
        })
    }
    fn cache_transitions(
        &mut self,
        models: &[(
            NativeColorIdentity,
            Arc<dyn NativeColorEditModel + Send + Sync>,
        )],
    ) -> Result<(), TransitionError> {
        for index in 0..self.nodes.len() {
            let cached = match &self.nodes[index] {
                Node::Scale { base, value, .. } => self.nodes[*value]
                    .constant()
                    .map(|to| compile_pair(base.clone(), to.clone(), models))
                    .transpose()?,
                Node::Transition { from, to, .. } => {
                    match (self.nodes[*from].constant(), self.nodes[*to].constant()) {
                        (Some(from), Some(to)) => {
                            Some(compile_pair(from.clone(), to.clone(), models)?)
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            match &mut self.nodes[index] {
                Node::Scale { cached: slot, .. } | Node::Transition { cached: slot, .. } => {
                    *slot = cached
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn evaluate(
        &self,
        owner: ProgrammingOwner,
        underlay: Option<&AttributeValue>,
        models: &[(
            NativeColorIdentity,
            Arc<dyn NativeColorEditModel + Send + Sync>,
        )],
        frame: &dyn WholeFamilyExpressionFrameResolver,
        mut observer: Option<&mut dyn FamilyExpressionObserver>,
    ) -> Result<AttributeValue, TransitionError> {
        let mut values = vec![None; self.nodes.len()];
        if self.needs_underlay {
            let value = underlay.ok_or(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ))?;
            validate_whole(owner, value)?;
            values[0] = Some(value.clone());
            if let Some(observer) = observer.as_deref_mut() {
                self.observe_node(0, value, None, observer)?;
            }
        }
        for index in 1..self.nodes.len() {
            let (value, transfer) =
                self.node_result(index, owner, &values, models, frame, observer.is_some())?;
            if let Some(observer) = observer.as_deref_mut() {
                self.observe_node(index, &value, transfer.as_ref(), observer)?;
            }
            values[index] = Some(value);
        }
        values[self.root].take().ok_or(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints,
        ))
    }
    fn node_result(
        &self,
        index: usize,
        owner: ProgrammingOwner,
        values: &[Option<AttributeValue>],
        models: &[(
            NativeColorIdentity,
            Arc<dyn NativeColorEditModel + Send + Sync>,
        )],
        frame: &dyn WholeFamilyExpressionFrameResolver,
        traced: bool,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        Ok(match &self.nodes[index] {
            Node::Value { value, .. } | Node::Baseline { value, .. } => (value.clone(), None),
            Node::Scale {
                base,
                value,
                factor,
                cached,
                ..
            } => {
                let to = values[*value].as_ref().expect("evaluated child");
                let compiled = match cached {
                    Some(value) => value.clone(),
                    None => compile_pair(base.clone(), to.clone(), models)?,
                };
                let operation = FamilyExpressionOperation::Scale { factor: *factor };
                if traced {
                    resolve_operation_with_trace(
                        owner,
                        compiled.scale_with_trace(owner, *factor),
                        base,
                        to,
                        operation,
                        models,
                        frame,
                    )?
                } else {
                    (
                        resolve_operation(
                            owner,
                            compiled.scale(*factor),
                            base,
                            to,
                            operation,
                            models,
                            frame,
                        )?,
                        None,
                    )
                }
            }
            Node::Transition {
                from,
                to,
                progress,
                cached,
                ..
            } => {
                let from = values[*from].as_ref().expect("evaluated outgoing child");
                let to = values[*to].as_ref().expect("evaluated incoming child");
                let compiled = match cached {
                    Some(value) => value.clone(),
                    None => compile_pair(from.clone(), to.clone(), models)?,
                };
                let operation = FamilyExpressionOperation::Transition {
                    progress: *progress,
                };
                if traced {
                    resolve_operation_with_trace(
                        owner,
                        compiled.sample_with_trace(owner, *progress),
                        from,
                        to,
                        operation,
                        models,
                        frame,
                    )?
                } else {
                    (
                        resolve_operation(
                            owner,
                            compiled.sample(*progress),
                            from,
                            to,
                            operation,
                            models,
                            frame,
                        )?,
                        None,
                    )
                }
            }
            Node::Underlay => unreachable!("single graph underlay"),
        })
    }

    fn observe_node(
        &self,
        index: usize,
        value: &AttributeValue,
        transfer: Option<&ProgrammingTransitionTrace>,
        observer: &mut dyn FamilyExpressionObserver,
    ) -> Result<(), TransitionError> {
        let step = match &self.nodes[index] {
            Node::Underlay => FamilyExpressionStep::Underlay,
            Node::Value {
                value,
                occurrence,
                dependency_occurrence,
            } => FamilyExpressionStep::Authored {
                value,
                occurrence: *occurrence,
                dependency_occurrence: dependency_occurrence.clone(),
            },
            Node::Baseline { value, occurrence } => FamilyExpressionStep::Baseline {
                value,
                occurrence: *occurrence,
            },
            Node::Scale {
                base,
                value,
                factor,
                baseline_occurrence,
                ..
            } => FamilyExpressionStep::Scale {
                base,
                value: *value,
                factor: *factor,
                baseline_occurrence: *baseline_occurrence,
                trace: transfer,
            },
            Node::Transition {
                from,
                to,
                progress,
                reason,
                ..
            } => FamilyExpressionStep::Transition {
                from: *from,
                to: *to,
                progress: *progress,
                reason: *reason,
                trace: transfer,
            },
        };
        observer.evaluated(index, step, value)
    }
}

fn add_direct_recipe(value: &AttributeValue, recipes: &mut Vec<NativeColorRecipe>) {
    if let AttributeValue::ColorProgram(program) = value
        && let ColorProgram::Direct { recipe, .. } = program.as_ref()
        && !recipes.contains(recipe)
    {
        recipes.push(recipe.clone());
    }
}

fn add_direct_source(value: &AttributeValue, sources: &mut Vec<NativeColorIdentity>) {
    if let AttributeValue::ColorProgram(program) = value
        && let ColorProgram::Direct { recipe, .. } = program.as_ref()
        && !sources.contains(&recipe.source)
    {
        sources.push(recipe.source.clone());
    }
}

fn same_direct_source<'a>(
    from: &'a AttributeValue,
    to: &AttributeValue,
) -> Option<&'a NativeColorIdentity> {
    let (AttributeValue::ColorProgram(a), AttributeValue::ColorProgram(b)) = (from, to) else {
        return None;
    };
    let (ColorProgram::Direct { recipe: a, .. }, ColorProgram::Direct { recipe: b, .. }) =
        (a.as_ref(), b.as_ref())
    else {
        return None;
    };
    (a.source == b.source).then_some(&a.source)
}

fn compile_pair(
    from: AttributeValue,
    to: AttributeValue,
    models: &[(
        NativeColorIdentity,
        Arc<dyn NativeColorEditModel + Send + Sync>,
    )],
) -> Result<CompiledProgrammingTransition, TransitionError> {
    let model = same_direct_source(&from, &to)
        .and_then(|source| models.iter().find(|(identity, _)| identity == source))
        .map(|(_, model)| Arc::clone(model));
    CompiledProgrammingTransition::new(from, to, model)
}

pub(super) fn resolve_operation(
    owner: ProgrammingOwner,
    result: Result<AttributeValue, TransitionError>,
    from: &AttributeValue,
    to: &AttributeValue,
    operation: FamilyExpressionOperation,
    models: &[(
        NativeColorIdentity,
        Arc<dyn NativeColorEditModel + Send + Sync>,
    )],
    frame: &dyn WholeFamilyExpressionFrameResolver,
) -> Result<AttributeValue, TransitionError> {
    resolve_operation_result(
        owner,
        result.map(|value| (value, None)),
        from,
        to,
        operation,
        models,
        frame,
        false,
    )
    .map(|(value, _)| value)
}

pub(super) fn resolve_operation_with_trace(
    owner: ProgrammingOwner,
    result: Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError>,
    from: &AttributeValue,
    to: &AttributeValue,
    operation: FamilyExpressionOperation,
    models: &[(
        NativeColorIdentity,
        Arc<dyn NativeColorEditModel + Send + Sync>,
    )],
    frame: &dyn WholeFamilyExpressionFrameResolver,
) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
    resolve_operation_result(owner, result, from, to, operation, models, frame, true)
}

#[allow(clippy::too_many_arguments)]
fn resolve_operation_result(
    owner: ProgrammingOwner,
    result: Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError>,
    from: &AttributeValue,
    to: &AttributeValue,
    operation: FamilyExpressionOperation,
    models: &[(
        NativeColorIdentity,
        Arc<dyn NativeColorEditModel + Send + Sync>,
    )],
    frame: &dyn WholeFamilyExpressionFrameResolver,
    traced: bool,
) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
    match result {
        Ok(value) => Ok(value),
        Err(TransitionError::Requires(
            requirement @ (TransitionRequirement::LiveTargetPoints
            | TransitionRequirement::LiveJointAngles
            | TransitionRequirement::ColorAppearance
            | TransitionRequirement::ZoomConvention),
        )) => {
            let (resolved, trace) = if traced {
                frame.resolve_with_trace(requirement, from, to, operation)?
            } else {
                (frame.resolve(requirement, from, to, operation)?, None)
            };
            validate_whole(owner, &resolved)?;
            if let AttributeValue::ColorProgram(program) = &resolved
                && let ColorProgram::Direct { recipe, .. } = program.as_ref()
            {
                let model = models
                    .iter()
                    .find(|(identity, _)| identity == &recipe.source)
                    .map(|(_, model)| model)
                    .ok_or(TransitionError::Requires(
                        TransitionRequirement::NativeColorModel,
                    ))?;
                validate_original_recipe(recipe, model.as_ref())?;
            }
            Ok((resolved, trace))
        }
        Err(error) => Err(error),
    }
}

fn validate_original_recipe(
    recipe: &NativeColorRecipe,
    model: &dyn NativeColorEditModel,
) -> Result<(), TransitionError> {
    if model.source() != &recipe.source {
        return Err(
            IntentError("native Color model differs from pinned source identity".into()).into(),
        );
    }
    let predicted = model.predict(recipe)?;
    if predicted.model_revision != recipe.source.model_revision {
        return Err(IntentError(
            "native prediction differs from pinned source model revision".into(),
        )
        .into());
    }
    predicted.validate()?;
    Ok(())
}

fn validate_whole(owner: ProgrammingOwner, value: &AttributeValue) -> Result<(), TransitionError> {
    let correct_owner = match (owner, value) {
        (ProgrammingOwner::Focus, AttributeValue::Normalized(level)) => {
            ScalarDomain::UNIT.contains(*level)
        }
        (ProgrammingOwner::Focus, _) => false,
        (_, _) => value.programming_owner() == Some(owner),
    };
    if !correct_owner {
        return Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners,
        ));
    }
    if value.spread_control_points() != 0 || matches!(value, AttributeValue::GroupFamily(_)) {
        return Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints,
        ));
    }
    value.validate_programming_address(owner.key_ref())?;
    Ok(())
}

fn preflight_whole(
    tape: &RetainedExpressionTape,
    root: RetainedNodeId,
    owner: ProgrammingOwner,
) -> Result<(), TransitionError> {
    let reachable = retained_reachability(tape, root, false);
    for (index, node) in tape.nodes.iter().enumerate() {
        if !reachable[index] {
            continue;
        }
        match node {
            RetainedExpressionNode::Programming { address, value, .. }
            | RetainedExpressionNode::Scale {
                address,
                base: value,
                ..
            } => {
                if address.component.is_some() || address.owner() != owner {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::CompatibleOwners,
                    ));
                }
                let DynamicValue::Family(value) = value else {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::CompatibleOwners,
                    ));
                };
                validate_whole(owner, value)?;
            }
            RetainedExpressionNode::Transition { .. } => {}
            _ => {
                return Err(TransitionError::Requires(
                    TransitionRequirement::CompatibleOwners,
                ));
            }
        }
    }
    Ok(())
}

/// Cold-compiled whole-owner tree. The untouched expression remains available for snapshot,
/// inspection and interruption. A new coherent frame is supplied for every evaluation call.
pub struct CompiledProgrammingFamilyExpression {
    retained: Arc<DynamicSampleExpression>,
    owner: ProgrammingOwner,
    graph: CompiledFamilyGraph,
    models: Vec<(
        NativeColorIdentity,
        Arc<dyn NativeColorEditModel + Send + Sync>,
    )>,
}
impl CompiledProgrammingFamilyExpression {
    /// Original retained input node emitted as this graph node. Underlay has no origin.
    pub(crate) fn retained_node_origin(&self, node: usize) -> Option<RetainedNodeId> {
        self.graph
            .retained_node_origins
            .get(node)
            .copied()
            .flatten()
    }

    pub fn new(
        expression: Arc<DynamicSampleExpression>,
        owner: ProgrammingOwner,
        underlay_at_compile: Option<&AttributeValue>,
        native_models: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<Self, TransitionError> {
        let tape = RetainedExpressionTape::from_roots(std::slice::from_ref(&expression))?;
        let root = tape.roots()[0];
        preflight_whole(&tape, root, owner)?;
        let mut graph = CompiledFamilyGraph::compile(&tape, root);
        let mut sources = vec![];
        let participates = graph.root != 0;
        let compile_underlay = (participates && graph.needs_underlay)
            .then_some(underlay_at_compile)
            .flatten();
        if let Some(value) = compile_underlay {
            validate_whole(owner, value)?;
        }
        if participates {
            for value in graph.authored_values().chain(compile_underlay) {
                add_direct_source(value, &mut sources);
            }
        }
        let mut models = Vec::with_capacity(sources.len());
        if !sources.is_empty() {
            let native_models = native_models.ok_or(TransitionError::Requires(
                TransitionRequirement::NativeColorModel,
            ))?;
            for source in sources {
                let model = native_models.resolve(&source)?;
                if model.source() != &source {
                    return Err(IntentError(
                        "retained native model differs from original source".into(),
                    )
                    .into());
                }
                models.push((source, model));
            }
        }
        let mut recipes = vec![];
        if participates {
            for value in graph.authored_values().chain(compile_underlay) {
                add_direct_recipe(value, &mut recipes);
            }
        }
        for recipe in recipes {
            let model = models
                .iter()
                .find(|(identity, _)| identity == &recipe.source)
                .map(|(_, model)| model)
                .ok_or(TransitionError::Requires(
                    TransitionRequirement::NativeColorModel,
                ))?;
            validate_original_recipe(&recipe, model.as_ref())?;
        }
        graph.cache_transitions(&models)?;
        Ok(Self {
            retained: expression,
            owner,
            graph,
            models,
        })
    }

    /// Recompile a selected branch without treating the generic frame base as its eligible
    /// underlay. Coverage and cohort selection determine that later. Keep every model already
    /// pinned by this source, including models originally verified for an actual underlay.
    pub fn recompile_conditioned(
        &self,
        expression: Arc<DynamicSampleExpression>,
        native_models: &dyn DynamicNativeModelResolver,
    ) -> Result<Self, TransitionError> {
        struct Models<'a> {
            original: &'a CompiledProgrammingFamilyExpression,
            additional: &'a dyn DynamicNativeModelResolver,
        }
        impl DynamicNativeModelResolver for Models<'_> {
            fn resolve(
                &self,
                source: &NativeColorIdentity,
            ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
                if let Ok(model) = self.original.resolve_original_native_model(source) {
                    return Ok(model);
                }
                self.additional.resolve(source)
            }
        }
        let models = Models {
            original: self,
            additional: native_models,
        };
        let mut compiled = Self::new(expression, self.owner, None, Some(&models))?;
        for (source, model) in &self.models {
            if !compiled
                .models
                .iter()
                .any(|(existing, _)| existing == source)
            {
                compiled.models.push((source.clone(), model.clone()));
            }
        }
        Ok(compiled)
    }

    pub fn expression(&self) -> &DynamicSampleExpression {
        &self.retained
    }

    pub fn owner(&self) -> ProgrammingOwner {
        self.owner
    }

    pub fn participates(&self) -> bool {
        self.graph.root != 0
    }

    pub fn needs_underlay(&self) -> bool {
        self.participates() && self.graph.needs_underlay
    }

    pub fn trace_root_node(&self) -> usize {
        self.graph.root
    }

    /// Retained reason for a transition in this compiled graph. Node IDs are graph-local;
    /// a Required reason does not establish a common evaluation cut across owners.
    pub fn transition_reason(&self, node: usize) -> Option<DynamicTransitionReason> {
        match self.graph.nodes.get(node)? {
            Node::Transition { reason, .. } => Some(*reason),
            _ => None,
        }
    }

    /// Return the verified original source model cached at construction, never a destination
    /// profile model. A resolved Direct whole value with another identity needs cold recompilation.
    pub fn resolve_original_native_model(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, TransitionError> {
        self.models
            .iter()
            .find(|(identity, _)| identity == source)
            .map(|(_, model)| Arc::clone(model))
            .ok_or(TransitionError::Requires(
                TransitionRequirement::NativeColorModel,
            ))
    }

    /// Mix activation after evaluating the retained tree against its eligible prefix. The
    /// result's concrete representation may differ from either endpoint's stored address.
    pub fn transition(
        &self,
        from: &AttributeValue,
        to: &AttributeValue,
        progress: f32,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<AttributeValue, TransitionError> {
        self.transition_result(from, to, progress, frame, false)
            .map(|(value, _)| value)
    }

    pub(super) fn transition_with_trace(
        &self,
        from: &AttributeValue,
        to: &AttributeValue,
        progress: f32,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        self.transition_result(from, to, progress, frame, true)
    }

    fn transition_result(
        &self,
        from: &AttributeValue,
        to: &AttributeValue,
        progress: f32,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        traced: bool,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        validate_whole(self.owner, from)?;
        validate_whole(self.owner, to)?;
        let compiled = compile_pair(from.clone(), to.clone(), &self.models)?;
        let operation = FamilyExpressionOperation::Transition { progress };
        let result = if traced {
            resolve_operation_with_trace(
                self.owner,
                compiled.sample_with_trace(self.owner, progress),
                from,
                to,
                operation,
                &self.models,
                frame,
            )?
        } else {
            (
                resolve_operation(
                    self.owner,
                    compiled.sample(progress),
                    from,
                    to,
                    operation,
                    &self.models,
                    frame,
                )?,
                None,
            )
        };
        // A portable Direct underlay may participate in a modeled Semantic takeover without
        // its original source model. Require a pinned model only if Direct survives as output.
        if let AttributeValue::ColorProgram(program) = &result.0
            && let ColorProgram::Direct { recipe, .. } = program.as_ref()
        {
            self.resolve_original_native_model(&recipe.source)?;
        }
        Ok(result)
    }

    /// `eligible_underlay` is the same owner/instance baseline selected before this expression's
    /// footprint. It is read only if a retained endpoint is absent, never replaced by zero.
    pub fn evaluate_optional(
        &self,
        eligible_underlay: Option<&AttributeValue>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.evaluate_optional_observed(eligible_underlay, frame, None)
    }

    pub fn evaluate_optional_observed(
        &self,
        eligible_underlay: Option<&AttributeValue>,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        observer: Option<&mut dyn FamilyExpressionObserver>,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        if !self.participates() {
            return Ok(None);
        }
        if self.needs_underlay() {
            let value = eligible_underlay.ok_or(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ))?;
            validate_whole(self.owner, value)?;
        }
        self.graph
            .evaluate(self.owner, eligible_underlay, &self.models, frame, observer)
            .map(Some)
    }

    /// Compatibility entry point for callers that already have a mandatory complete underlay.
    pub fn evaluate(
        &self,
        eligible_underlay: &AttributeValue,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<AttributeValue, TransitionError> {
        self.evaluate_optional(Some(eligible_underlay), frame)?
            .ok_or(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "expression_family/origin_tests.rs"]
mod origin_tests;
