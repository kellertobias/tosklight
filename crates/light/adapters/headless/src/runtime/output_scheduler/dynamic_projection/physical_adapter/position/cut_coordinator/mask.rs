//! One owner-local partial mask, with complete independently constant mechanical peers.
//! Exact original locators authorize operand replay; no peer acquires another owner's mask.
use super::*;
use light_dynamics::{
    PositionMaskLocator, PositionMaskOperand, PositionMaskStage, PositionStageOperandProgress,
};

fn initiator(peers: &[Peer], batch: &Batch<'_>) -> Result<Option<usize>, TransitionError> {
    let mut candidate = None;
    let mut count = 0usize;
    for (index, peer) in peers.iter().enumerate() {
        count += peer.requested.samples.len();
        if count > MAX_NODES {
            return Ok(None);
        }
        if envelope::constant(peer, batch)? {
            continue;
        }
        if candidate.is_some() || !batch.noninterpolating_controls(&peer.captured)? {
            return Ok(None);
        }
        let mut masks = 0;
        for sample in peer.requested.samples.iter() {
            let partial = envelope::activation(sample) != 1.;
            if partial {
                let FamilyCompositionSample::Known(sample) = sample else {
                    return Ok(None);
                };
                if !sample.is_fix_at()
                    || sample.address().address().component.is_some()
                    || !(0.0..1.0).contains(&sample.activation_mix)
                    || sample.activation_mix == 0.
                {
                    return Ok(None);
                }
                masks += 1;
            }
        }
        if masks != 1 {
            return Ok(None);
        }
        // Prefix and suffix may contain retained runtime sources, but every original node
        // is bounded. A nested blocker is not replaced with a guessed underlay below.
        let registry = peer.captured.registry();
        let mut pending = (0..registry.source_count())
            .map(|i| registry.source_root(i))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut visited = std::collections::HashSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node.clone()) {
                continue;
            }
            count += 1;
            if count > MAX_NODES {
                return Ok(None);
            }
            match registry.node_view(&node)? {
                PositionSourceNodeView::Transition { from, to, .. } => {
                    pending.extend(from);
                    pending.extend(to);
                }
                PositionSourceNodeView::Whole { root, .. } => pending.push(root),
                PositionSourceNodeView::SourceCohort { members } => pending.extend(members),
                PositionSourceNodeView::Scale { value, .. } => pending.extend(value),
                PositionSourceNodeView::Leaf => {}
            }
        }
        candidate = Some(index);
    }
    Ok(candidate)
}

pub(super) fn attempt(
    observer: &mut PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
) -> Result<Option<Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>>, TransitionError>
{
    let Some(initiator) = initiator(peers, batch)? else {
        return Ok(None);
    };
    let mut work = Vec::new();
    let result = run(observer, frame, batch, peers, initiator, &mut work);
    recycle(batch, &mut work)?;
    match result {
        Err(TransitionError::Requires(_)) => Ok(None),
        other => other,
    }
}

fn run(
    observer: &mut PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    initiator: usize,
    work: &mut Vec<Work>,
) -> Result<Option<Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>>, TransitionError>
{
    for (peer_index, peer) in peers.iter().enumerate() {
        for instance in 0..peer.descriptor.instances.len() {
            let bound = bound(observer, frame, peer, instance);
            let adoption = |original: &AttributeValue, address: &DynamicValueAddress| {
                bound.adopt(original, address)
            };
            let evaluation = batch.begin_branch(
                &peer.captured,
                &peer.captured.registry().branch(),
                peer.descriptor.instances[instance].destination,
                &adoption,
            )?;
            work.push(Work {
                peer: peer_index,
                instance,
                evaluation: Some(evaluation),
                request: None,
            });
            let item = work.last_mut().unwrap();
            item.request =
                match batch.advance(item.evaluation.as_mut().unwrap(), &bound, &adoption)? {
                    PositionCompositionProgress::Complete(_) => None,
                    PositionCompositionProgress::NeedsMaterialization(request) => Some(request),
                };
            if peer_index != initiator && item.request.is_some() {
                return Ok(None);
            }
        }
    }
    let mut previous_stage = None;
    // One mask can suspend once for adoption and once for its transition. Any later
    // source/materialization needs its own proven environment and stays passive.
    for _ in 0..2 {
        let mut locators = Vec::new();
        let mut first = None;
        let mut progress = None;
        for item in work.iter().filter(|item| item.peer == initiator) {
            let Some(request) = &item.request else {
                return Ok(None);
            };
            let stage = match &request.operation {
                PositionCompositionOperation::MaskAdoption { address, .. } => {
                    if address.representation != light_dynamics::DynamicFamilyRepresentation::Angles
                    {
                        return Ok(None); // No unique inverse Angle -> Target distance exists.
                    }
                    PositionMaskStage::Adoption
                }
                PositionCompositionOperation::MaskTransition { progress: mix, .. } => {
                    if progress.is_some_and(|p| p != *mix) {
                        return Ok(None);
                    }
                    progress = Some(*mix);
                    PositionMaskStage::Transition
                }
                _ => return Ok(None),
            };
            let Some(locator) = item
                .evaluation
                .as_ref()
                .unwrap()
                .pending_mask_locator(request.request_id)?
            else {
                return Ok(None);
            };
            if locator.stage() != stage
                || first.as_ref().is_some_and(|original| original != &locator)
            {
                return Ok(None);
            }
            first = Some(locator.clone());
            locators.push((item.instance, locator));
        }
        let Some(locator) = first else {
            return Ok(None);
        };
        if previous_stage == Some(locator.stage())
            || previous_stage == Some(PositionMaskStage::Transition)
        {
            return Ok(None);
        }
        previous_stage = Some(locator.stage());
        let operands: &[PositionMaskOperand] = match locator.stage() {
            PositionMaskStage::Adoption => &[PositionMaskOperand::AdoptionInput],
            PositionMaskStage::Transition => &[
                PositionMaskOperand::TransitionFrom,
                PositionMaskOperand::TransitionTo,
            ],
        };
        let mut fitted = Vec::new();
        for &operand in operands {
            let mut values = Vec::new();
            for item in work.iter() {
                let value = if item.peer == initiator {
                    let locator = &locators
                        .iter()
                        .find(|(instance, _)| *instance == item.instance)
                        .ok_or_else(|| invalid("Position mask omitted a physical copy"))?
                        .1;
                    let Some(value) = replay(
                        observer,
                        frame,
                        batch,
                        &peers[item.peer],
                        item.instance,
                        locator,
                        operand,
                    )?
                    else {
                        return Ok(None);
                    };
                    value
                } else {
                    item.evaluation
                        .as_ref()
                        .and_then(|evaluation| evaluation.completed_value())
                        .ok_or_else(|| invalid("Position mask peer did not complete"))?
                        .clone()
                };
                if !envelope::literal(&value) {
                    return Ok(None);
                }
                values.push(value);
            }
            let Some(result) = envelope::fit(observer, frame, peers, work, &values)? else {
                return Ok(None);
            };
            fitted.push(result);
        }
        for item in work.iter_mut().filter(|item| item.peer == initiator) {
            let destination = peers[initiator].descriptor.instances[item.instance].destination;
            let Some(from) = fitted_pair(&fitted[0][initiator], destination) else {
                return Ok(None);
            };
            let value = if locator.stage() == PositionMaskStage::Transition {
                let Some(to) = fitted_pair(&fitted[1][initiator], destination) else {
                    return Ok(None);
                };
                CompiledProgrammingTransition::new(from, to, None)?.sample(progress.unwrap())?
            } else {
                from
            };
            batch.resume(
                item.evaluation.as_mut().unwrap(),
                item.request.as_ref().unwrap().request_id,
                value,
                None,
            )?;
            let bound = bound(observer, frame, &peers[initiator], item.instance);
            let adoption = |original: &AttributeValue, address: &DynamicValueAddress| {
                bound.adopt(original, address)
            };
            item.request =
                match batch.advance(item.evaluation.as_mut().unwrap(), &bound, &adoption)? {
                    PositionCompositionProgress::Complete(_) => None,
                    PositionCompositionProgress::NeedsMaterialization(request) => Some(request),
                };
        }
        if work.iter().all(|item| item.request.is_none()) {
            return observe_completed(observer, batch, peers, work).map(Some);
        }
    }
    Ok(None)
}

fn replay(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peer: &Peer,
    instance: usize,
    locator: &PositionMaskLocator,
    operand: PositionMaskOperand,
) -> Result<Option<AttributeValue>, TransitionError> {
    let bound = bound(observer, frame, peer, instance);
    let adoption =
        |original: &AttributeValue, address: &DynamicValueAddress| bound.adopt(original, address);
    let mut evaluation = batch.begin_mask_branch(
        &peer.captured,
        &peer.captured.registry().branch(),
        locator,
        operand,
        peer.descriptor.instances[instance].destination,
        &adoption,
    )?;
    let result = batch.advance_stage(&mut evaluation, &bound, &adoption);
    batch.recycle_stage(evaluation)?;
    Ok(match result? {
        PositionStageOperandProgress::OperandReady(value) => Some(value),
        PositionStageOperandProgress::Inactive
        | PositionStageOperandProgress::NeedsMaterialization(_) => None,
    })
}
