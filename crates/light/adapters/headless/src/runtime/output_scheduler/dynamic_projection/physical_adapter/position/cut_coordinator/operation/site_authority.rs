//! Original site parameters and authentic producer authority for one exact owner-local
//! Required/Size locator. Shared by pending requests and synchronous peer candidates.
use super::*;

/// The original captured parameters of one exact owner-local Required/Size site.
pub(super) fn site_operation(
    peer: &Peer,
    locator: &PositionGraphOperationLocator,
) -> Result<Option<Operation>, TransitionError> {
    let operation = match peer
        .captured
        .registry()
        .node_view(locator.operation_node())?
    {
        PositionSourceNodeView::Transition {
            progress,
            reason: DynamicTransitionReason::Required { .. },
            ..
        } if progress.is_finite() && progress > 0. && progress < 1. => {
            Operation::Required { progress }
        }
        PositionSourceNodeView::Scale { factor, .. }
            if factor.is_finite() && factor > 0. && factor != 1. =>
        {
            Operation::Size { factor }
        }
        _ => return Ok(None),
    };
    let kind_matches = matches!(
        (operation, locator.kind()),
        (
            Operation::Required { .. },
            PositionGraphOperationKind::Required
        ) | (Operation::Size { .. }, PositionGraphOperationKind::Size)
    );
    Ok(kind_matches.then_some(operation))
}

/// Authentic producer authority for one exact original site: its single retained operation
/// handle names this peer, lane, instance and controller at the matching producer site.
pub(super) fn site_authority(
    peer: &Peer,
    locator: &PositionGraphOperationLocator,
    operation: Operation,
) -> Result<Option<DynamicOperationHandle>, TransitionError> {
    let registry = peer.captured.registry();
    let node = locator.operation_node();
    let expression = match registry.node_expression(node) {
        Ok(expression) => expression,
        Err(TransitionError::Requires(_)) => return Ok(None),
        Err(error) => return Err(error),
    };
    let DynamicSampleExpression::Retained { tape, root } = expression.as_ref() else {
        return Ok(None);
    };
    let handles = tape.operation_handles(*root)?;
    let [handle] = handles.as_slice() else {
        return Ok(None);
    };
    if handle.program_node().is_some() {
        return Ok(None);
    }
    let site_matches = matches!(
        (operation, handle.site()),
        (
            Operation::Required { .. },
            DynamicOperationSite::KeyframeTransition { .. }
        ) | (
            Operation::Size { .. },
            DynamicOperationSite::ControllerSize {
                role: DynamicControllerSizeRole::FamilyScale
            }
        )
    );
    let Some(identity) = registry.node_source_rank(node)?.dynamic_identity() else {
        return Ok(None);
    };
    if !site_matches
        || handle.target() != peer.target
        || handle.lane_id() != identity.lane_id
        || handle.emission().instance_id() != identity.instance_id
        || handle.emission().controller().id != identity.controller_id
    {
        return Ok(None);
    }
    Ok(Some(handle.clone()))
}
