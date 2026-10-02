//! Split a compatible retained source into exact component footprints. The authored expression
//! remains intact; another component's leaf means this component's eligible underlay.
use super::{
    CompiledComponentExpression, CompiledDynamicValueAddress, DynamicFamilyRepresentation,
    DynamicSampleExpression, DynamicSemanticColorBasis, DynamicValueAddress,
    RetainedExpressionNode, RetainedExpressionTape, RetainedNodeId,
};
use crate::DynamicNativeModelResolver;
use light_core::programming::{
    NativeColorEditModel, ProgrammingComponent, TransitionError, TransitionRequirement,
};
use std::sync::Arc;

/// One authored lane may release one component while acquiring another. These compiled
/// projections retain their individual masks and must share the original source rank.
/// Their caller distinguishes fragments by component, never by invented lane identities.
pub struct CompiledComponentExpressionSet {
    expression: Arc<DynamicSampleExpression>,
    components: Vec<Arc<CompiledComponentExpression>>,
}

impl CompiledComponentExpressionSet {
    pub fn new(
        expression: Arc<DynamicSampleExpression>,
        native_models: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<Self, TransitionError> {
        let tape = RetainedExpressionTape::from_roots(&[Arc::clone(&expression)])?;
        let root = tape.roots()[0];
        let active = active_nodes(&tape, root);
        let mut addresses = Vec::new();
        for (index, node) in tape.nodes.iter().enumerate() {
            if !active[index] {
                continue;
            }
            match node {
                RetainedExpressionNode::Programming { address, .. } => {
                    collect_address(address, &mut addresses)?;
                }
                RetainedExpressionNode::Transition { .. } => {}
                RetainedExpressionNode::LegacyScalar { .. }
                | RetainedExpressionNode::AngleCurrent { .. }
                | RetainedExpressionNode::AngleNumeric { .. }
                | RetainedExpressionNode::Scale { .. } => return Err(incompatible()),
            }
        }
        let mut components = Vec::with_capacity(addresses.len());
        let mut native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>> = None;
        for address in addresses {
            let Some(projected) = project(&tape, root, &active, &address)? else {
                continue;
            };
            let model = if let DynamicFamilyRepresentation::DirectColor { source } =
                &address.representation
            {
                if native_model.is_none() {
                    native_model = Some(
                        native_models
                            .ok_or(TransitionError::Requires(
                                TransitionRequirement::NativeColorModel,
                            ))?
                            .resolve(source)?,
                    );
                }
                native_model.clone()
            } else {
                None
            };
            let address = Arc::new(CompiledDynamicValueAddress::new(address, model)?);
            let component = CompiledComponentExpression::new(projected, address)?;
            if component.participates() {
                components.push(Arc::new(component));
            }
        }
        Ok(Self {
            expression,
            components,
        })
    }

    pub fn expression(&self) -> &Arc<DynamicSampleExpression> {
        &self.expression
    }

    pub fn components(&self) -> &[Arc<CompiledComponentExpression>] {
        &self.components
    }
}

fn incompatible() -> TransitionError {
    TransitionError::Requires(TransitionRequirement::CompatibleOwners)
}

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

fn collect_address(
    address: &DynamicValueAddress,
    addresses: &mut Vec<DynamicValueAddress>,
) -> Result<(), TransitionError> {
    let Some(component) = address.component else {
        return Err(incompatible());
    };
    if address.representation == DynamicFamilyRepresentation::Angles {
        return Err(incompatible());
    }
    for prior in addresses.iter() {
        if !compatible_representation(&prior.representation, &address.representation) {
            return Err(incompatible());
        }
        if prior.component == Some(component) {
            return if prior == address {
                Ok(())
            } else {
                Err(incompatible())
            };
        }
        // Function bindings on one native channel are exclusive states, not independent masks.
        if let (Some(ProgrammingComponent::NativeColor(a)), ProgrammingComponent::NativeColor(b)) =
            (prior.component, component)
            && a.channel_id == b.channel_id
        {
            return Err(incompatible());
        }
    }
    addresses.push(address.clone());
    Ok(())
}

fn compatible_representation(
    a: &DynamicFamilyRepresentation,
    b: &DynamicFamilyRepresentation,
) -> bool {
    match (a, b) {
        (
            DynamicFamilyRepresentation::SemanticColor { basis: a },
            DynamicFamilyRepresentation::SemanticColor { basis: b },
        ) => {
            a == b
                || *a == DynamicSemanticColorBasis::Retain
                || *b == DynamicSemanticColorBasis::Retain
        }
        _ => a == b,
    }
}

/// Emit another flat tape rather than a recursively copied transition tree. Each component's
/// projected root keeps the source transition progress and reason, including Resume occurrence.
fn project(
    source: &RetainedExpressionTape,
    source_root: RetainedNodeId,
    active: &[bool],
    address: &DynamicValueAddress,
) -> Result<Option<Arc<DynamicSampleExpression>>, TransitionError> {
    let mut tape = RetainedExpressionTape::empty();
    let mut map = vec![None; source.nodes.len()];
    for (index, node) in source.nodes.iter().enumerate() {
        if !active[index] {
            continue;
        }
        let projected = match node {
            RetainedExpressionNode::Programming {
                address: leaf,
                value,
                occurrence,
                dependency_occurrence,
            } if leaf == address => Some(RetainedExpressionNode::Programming {
                address: leaf.clone(),
                value: value.clone(),
                occurrence: *occurrence,
                dependency_occurrence: dependency_occurrence.clone(),
            }),
            RetainedExpressionNode::Programming { .. } => None,
            RetainedExpressionNode::Transition {
                from,
                to,
                progress,
                reason,
            } => {
                let child = |id: Option<RetainedNodeId>| -> Option<RetainedNodeId> {
                    id.and_then(|id| map[id.0 as usize])
                };
                if *progress == 0.0 || *progress == 1.0 {
                    map[index] = child(if *progress == 0.0 { *from } else { *to });
                    continue;
                }
                let from = child(*from);
                let to = child(*to);
                (from.is_some() || to.is_some()).then_some(RetainedExpressionNode::Transition {
                    from,
                    to,
                    progress: *progress,
                    reason: *reason,
                })
            }
            RetainedExpressionNode::LegacyScalar { .. }
            | RetainedExpressionNode::AngleCurrent { .. }
            | RetainedExpressionNode::AngleNumeric { .. }
            | RetainedExpressionNode::Scale { .. } => return Err(incompatible()),
        };
        if let Some(node) = projected {
            let id = RetainedNodeId(u32::try_from(tape.nodes.len()).map_err(|_| incompatible())?);
            tape.nodes.push(node);
            map[index] = Some(id);
        }
    }
    let Some(root) = map[source_root.0 as usize] else {
        return Ok(None);
    };
    tape.roots.push(root);
    tape.validate()?;
    Ok(Some(Arc::new(DynamicSampleExpression::Retained {
        tape: Arc::new(tape),
        root,
    })))
}

#[cfg(test)]
mod tests;
