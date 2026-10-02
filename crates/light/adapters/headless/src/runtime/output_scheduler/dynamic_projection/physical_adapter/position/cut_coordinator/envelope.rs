//! One original owner-local envelope against independently constant captured peers. No
//! corresponding operation is inferred on another owner; each constant peer keeps its actual
//! completed program in both full mechanical endpoint cohorts. All work remains speculative.
use super::*;
use light_dynamics::{
    PositionCompletionStage, PositionEnvelopeLocator, PositionEnvelopeOperand,
    PositionStageOperandProgress,
};

pub(super) fn literal(value: &AttributeValue) -> bool {
    let AttributeValue::Position(value) = value else {
        return false;
    };
    let finite =
        |value: &ScalarIntent| matches!(value, ScalarIntent::Value(value) if value.is_finite());
    match value.as_ref() {
        PositionIntent::Angles {
            pan_degrees,
            tilt_degrees,
        } => finite(pan_degrees) && finite(tilt_degrees),
        PositionIntent::Target { offset_metres, .. } => offset_metres.iter().all(finite),
    }
}
pub(super) fn activation(sample: &FamilyCompositionSample) -> f32 {
    match sample {
        FamilyCompositionSample::Known(sample) => sample.activation_mix,
        FamilyCompositionSample::WholeExpression { activation_mix, .. }
        | FamilyCompositionSample::CoupledExpression { activation_mix, .. } => *activation_mix,
    }
}
pub(super) fn constant(peer: &Peer, batch: &Batch<'_>) -> Result<bool, TransitionError> {
    if peer.baseline_only || peer.requested.samples.is_empty() {
        return Ok(literal(&peer.requested.base));
    }
    let [FamilyCompositionSample::Known(sample)] = &peer.requested.samples[..] else {
        return Ok(false);
    };
    Ok(sample.activation_mix == 1.0
        && sample.address().address().component.is_none()
        && batch.noninterpolating_controls(&peer.captured)?
        && matches!(sample.materialized_value(), Some(light_dynamics::DynamicValue::Family(value)) if literal(value)))
}
fn initiator(peers: &[Peer], batch: &Batch<'_>) -> Result<Option<usize>, TransitionError> {
    let mut candidate = None;
    let mut nodes = 0usize;
    // Count all original slots, including suppressed owners; a gate cannot turn an
    // arbitrarily large captured source stack into an unbounded cheap-peer claim.
    for peer in peers {
        if peer.requested.samples.len() > MAX_NODES.saturating_sub(nodes) {
            return Ok(None);
        }
        nodes += peer.requested.samples.len();
    }
    for (index, peer) in peers.iter().enumerate() {
        if constant(peer, batch)? {
            continue;
        }
        let [sample] = &peer.requested.samples[..] else {
            return Ok(None);
        };
        if candidate.is_some()
            || (activation(sample) == 1.0 && batch.noninterpolating_controls(&peer.captured)?)
        {
            return Ok(None);
        }
        // The single original source may contain a retained expression, but replay work is
        // bounded before any loan. Its unresolved nested materialization stays passive below.
        let registry = peer.captured.registry();
        let mut pending = (0..registry.source_count())
            .map(|index| registry.source_root(index))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut visited = std::collections::HashSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node.clone()) {
                continue;
            }
            nodes += 1;
            if nodes > MAX_NODES {
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

fn completion(
    request: &PositionCompositionRequest,
) -> Option<&light_dynamics::PositionCompositionCompletionRequest> {
    let PositionCompositionOperation::Base { request, .. } = &request.operation else {
        return None;
    };
    match &request.operation {
        PositionCompositionBaseOperation::Completion(request) => Some(request),
        _ => None,
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
    let mut original_locator = None;
    let mut progress = None;
    let mut locators = Vec::new();
    for item in work.iter().filter(|item| item.peer == initiator) {
        let Some(request) = &item.request else {
            return Ok(None);
        };
        let Some(operation) = completion(request) else {
            return Ok(None);
        };
        let Some(locator) = item
            .evaluation
            .as_ref()
            .unwrap()
            .pending_envelope_locator(request.request_id)?
        else {
            return Ok(None);
        };
        if locator.stage() != operation.stage
            || original_locator
                .as_ref()
                .is_some_and(|original| original != &locator)
            || progress.is_some_and(|progress| progress != operation.progress)
        {
            return Ok(None);
        }
        original_locator = Some(locator.clone());
        progress = Some(operation.progress);
        locators.push((item.instance, locator));
    }
    let Some(locator) = original_locator else {
        return Ok(None);
    };
    let operands = match locator.stage() {
        PositionCompletionStage::EndpointOutput => [
            PositionEnvelopeOperand::EndpointCurrent,
            PositionEnvelopeOperand::EndpointTarget,
        ],
        PositionCompletionStage::Activation => [
            PositionEnvelopeOperand::ActivationUnderlay,
            PositionEnvelopeOperand::ActivationEndpoint,
        ],
    };
    let mut fitted = Vec::new();
    for operand in operands {
        let mut values = Vec::new();
        for item in work.iter() {
            let value = if item.peer == initiator {
                let locator = &locators
                    .iter()
                    .find(|(instance, _)| *instance == item.instance)
                    .ok_or_else(|| invalid("Position envelope omitted a physical copy"))?
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
                    .ok_or_else(|| invalid("Position envelope peer did not complete"))?
                    .clone()
            };
            if !literal(&value) {
                return Ok(None);
            }
            values.push(value);
        }
        let Some(result) = fit(observer, frame, peers, work, &values)? else {
            return Ok(None);
        };
        fitted.push(result);
    }
    for item in work.iter_mut().filter(|item| item.peer == initiator) {
        let destination = peers[initiator].descriptor.instances[item.instance].destination;
        let (Some(from), Some(to)) = (
            fitted_pair(&fitted[0][initiator], destination),
            fitted_pair(&fitted[1][initiator], destination),
        ) else {
            return Ok(None);
        };
        let value =
            CompiledProgrammingTransition::new(from, to, None)?.sample(progress.unwrap())?;
        let request = item.request.as_ref().unwrap();
        batch.resume(
            item.evaluation.as_mut().unwrap(),
            request.request_id,
            value,
            None,
        )?;
        let bound = bound(observer, frame, &peers[initiator], item.instance);
        let adoption = |original: &AttributeValue, address: &DynamicValueAddress| {
            bound.adopt(original, address)
        };
        match batch.advance(item.evaluation.as_mut().unwrap(), &bound, &adoption)? {
            PositionCompositionProgress::Complete(_) => item.request = None,
            // A second envelope or nested operation needs its own proven operand environment.
            PositionCompositionProgress::NeedsMaterialization(_) => return Ok(None),
        }
    }
    observe_completed(observer, batch, peers, work).map(Some)
}
fn replay(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peer: &Peer,
    instance: usize,
    locator: &PositionEnvelopeLocator,
    operand: PositionEnvelopeOperand,
) -> Result<Option<AttributeValue>, TransitionError> {
    let bound = bound(observer, frame, peer, instance);
    let adoption =
        |original: &AttributeValue, address: &DynamicValueAddress| bound.adopt(original, address);
    let mut evaluation = batch.begin_envelope_branch(
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
pub(super) fn fit(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    peers: &[Peer],
    work: &[Work],
    values: &[AttributeValue],
) -> Result<Option<Vec<PhysicalResolution<PositionAdapter>>>, TransitionError> {
    let members = work
        .iter()
        .map(|item| (item.peer, item.instance))
        .collect::<Vec<_>>();
    fit_members(observer, frame, peers, &members, values)
}
pub(super) fn fit_members(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    peers: &[Peer],
    members: &[(usize, usize)],
    values: &[AttributeValue],
) -> Result<Option<Vec<PhysicalResolution<PositionAdapter>>>, TransitionError> {
    if members.len() != values.len() {
        return Err(invalid("Position envelope has incomplete work membership"));
    }
    let mut seen = std::collections::HashSet::new();
    for &(peer, instance) in members {
        if peers
            .get(peer)
            .is_none_or(|peer| instance >= peer.descriptor.instances.len())
            || !seen.insert((peer, instance))
        {
            return Err(invalid(
                "Position operand has foreign or duplicate copy membership",
            ));
        }
    }
    let mut programs = Vec::new();
    for (index, peer) in peers.iter().enumerate() {
        let destinations = members
            .iter()
            .zip(values)
            .filter(|((peer, _), _)| *peer == index)
            .map(|((_, instance), value)| {
                Ok(PositionProgramDestination {
                    destination: peer.descriptor.instances[*instance].destination,
                    value: value.clone(),
                    provenance: PhysicalProvenance {
                        fields: ProgrammingFieldScope::for_value(
                            ProgrammingOwner::Position,
                            value,
                        )?,
                        sources: Default::default(),
                        controls: None,
                    },
                })
            })
            .collect::<Result<Vec<_>, TransitionError>>()?;
        if destinations.len() != peer.descriptor.instances.len() {
            return Err(invalid("Position envelope omitted peer copies"));
        }
        programs.push((peer.target, destinations));
    }
    let requests = peers
        .iter()
        .map(|peer| PhysicalRequest {
            frame,
            target: peer.target,
            owner: ProgrammingOwner::Position,
            descriptor: peer.descriptor.as_ref(),
            value: &peer.requested.base,
            previous: peer.previous.as_ref(),
        })
        .collect::<Vec<_>>();
    let views = programs
        .iter()
        .map(|(target, destinations)| (*target, destinations.as_slice()))
        .collect::<Vec<_>>();
    let resolved = observer
        .lane
        .adapter()
        .resolve_cohort_programs(&requests, &[], &views)?;
    if resolved.len() != peers.len()
        || resolved.iter().zip(peers).any(|(resolution, peer)| {
            peer.descriptor
                .instances
                .iter()
                .any(|instance| fitted_pair(resolution, instance.destination).is_none())
        })
    {
        return Ok(None);
    }
    Ok(Some(resolved))
}
