//! TL-636 original Resume operands in the physical cut. A Resume locator is owner-local
//! authority issued by the exact outstanding request of each owner's own driver. Owners join one
//! cut only through the captured runtime scope (instance, controller, occurrence) carried by
//! their own issued locators; equal progress, value, rank or path never correlates owners.
//! Every child replays complete mechanical owners and copies; only root Full drivers publish.
use super::*;

impl Driver {
    /// Only the exact outstanding request of this driver issues Resume operand authority.
    pub(super) fn resume_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<PositionResumeOperandLocator>, TransitionError> {
        match self {
            Self::Full(driver) => driver.pending_resume_operand_locator(request_id),
            Self::Graph(driver) => driver.pending_resume_operand_locator(request_id),
            Self::Stage(driver) => driver.pending_resume_operand_locator(request_id),
            Self::Resume(driver) => driver.pending_resume_operand_locator(request_id),
        }
    }
}

/// One original Resume cut. Every copy pending on an issued locator of the initiator's exact
/// scope joins atomically with its own locator and its own original progress. An owner that is
/// not issued for that scope but whose original branch keeps it Active refuses the cut: the
/// reached-graph materialization seam issues Required/Size sites, never Resume authority.
/// Absent, Inactive and gated owners replay their inherited goal (or remain Constant).
pub(super) fn cut(
    environment: &Environment,
    peers: &[Peer],
    batch: &Batch<'_>,
    original: &PositionResumeOperandLocator,
    root_first: bool,
) -> Result<Option<Vec<Option<PendingCut>>>, TransitionError> {
    // The existing single-source runner keeps owning a top-level first Resume it supports.
    if root_first && eligible_programs(peers, batch)? {
        return Ok(None);
    }
    let scope = original.scope();
    let mut result = Vec::with_capacity(environment.work.len());
    let mut joined = false;
    for item in &environment.work {
        let peer = &peers[item.peer];
        if let (Some(request), Some(driver)) = (&item.request, &item.driver) {
            if let Some(locator) = driver.resume_locator(request.request_id)? {
                if locator.scope() == scope {
                    let Some(progress) = authenticate(peer, request, &locator)? else {
                        return Ok(None);
                    };
                    result.push(Some(PendingCut {
                        operands: [
                            PositionResumeEndpoint::Outgoing,
                            PositionResumeEndpoint::Incoming,
                        ]
                        .map(|endpoint| Goal::Resume(locator.clone(), endpoint)),
                        calculation: Calculation::Blend(progress),
                    }));
                    joined = true;
                    continue;
                }
            }
        }
        // Exact captured gates remove a suppressed owner before evaluation; its hidden
        // scopes gain no endpoint-choice authority (as in the existing Resume runner).
        if !peer.baseline_only
            && matches!(
                peer.captured
                    .registry()
                    .branch()
                    .resume_scope_membership(scope)?,
                PositionResumeScopeMembership::Active(_)
            )
        {
            #[cfg(test)]
            evidence(|evidence| evidence.resume_active_unissued += 1);
            return Ok(None);
        }
        result.push(None);
    }
    #[cfg(test)]
    if joined {
        let nested = environment
            .work
            .iter()
            .find(|item| item.request.is_some())
            .is_some_and(|item| !matches!(item.driver, Some(Driver::Full(_))));
        evidence(|evidence| {
            evidence.resume_cuts += 1;
            evidence.resume_nested_cuts += usize::from(nested);
        });
    }
    Ok(joined.then_some(result))
}

/// The owner's own locator, registry scope, runtime occurrence, original Transition node and
/// captured progress must all agree. Returns that owner's own original progress.
fn authenticate(
    peer: &Peer,
    request: &PositionCompositionRequest,
    locator: &PositionResumeOperandLocator,
) -> Result<Option<f32>, TransitionError> {
    let registry = peer.captured.registry();
    if locator.capture_id() != registry.capture_id()
        || request.capture_id != registry.capture_id()
        || registry.resume_scope(locator.operation_node())? != Some(locator.scope())
    {
        return Ok(None);
    }
    let PositionCompositionOperation::Base { request: base, .. } = &request.operation else {
        return Ok(None);
    };
    let mut leaf = base;
    let mut depth = 0;
    while let PositionCompositionBaseOperation::SourceCohort { request, .. } = &leaf.operation {
        depth += 1;
        if depth > MAX_NODES {
            return Ok(None);
        }
        leaf = request;
    }
    let (progress, reason) = match &leaf.operation {
        PositionCompositionBaseOperation::Whole(inner) => match &inner.operation {
            FamilyMaterializationOperation::Transition {
                progress, reason, ..
            } => (*progress, *reason),
            _ => return Ok(None),
        },
        PositionCompositionBaseOperation::Coupled(inner) => match &inner.operation {
            PositionMaterializationOperation::Transition {
                progress, reason, ..
            } => (*progress, *reason),
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    let DynamicTransitionReason::Resume { occurrence_id } = reason else {
        return Ok(None);
    };
    let Some(origin) = request.origin() else {
        return Ok(None);
    };
    if occurrence_id != locator.scope().occurrence_id
        || !progress.is_finite()
        || registry.source_node_for_origin(origin)?.as_ref() != Some(locator.operation_node())
        || !matches!(
            registry.node_view(locator.operation_node())?,
            PositionSourceNodeView::Transition { progress: original, .. } if original == progress
        )
    {
        return Ok(None);
    }
    Ok(Some(progress))
}
