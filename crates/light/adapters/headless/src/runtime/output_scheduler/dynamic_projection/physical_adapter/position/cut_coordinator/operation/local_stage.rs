//! Owner-local stages never acquire shared graph authority. Every changing untouched peer
//! replays its exact inherited goal in child environments; only the named owner's original
//! stage and copies are selected. Opaque local locators, not equal progress, identify the cut.
use super::*;
use light_dynamics::{DynamicFamilyRepresentation, PositionCompletionStage, PositionMaskStage};

pub(super) fn mask(
    environment: &Environment,
    initiator: usize,
    original: &PositionMaskLocator,
) -> Result<Option<Vec<Option<PendingCut>>>, TransitionError> {
    let mut result = Vec::new();
    let mut original_progress = None;
    for item in &environment.work {
        if item.peer != initiator {
            result.push(None);
            continue;
        }
        let Some(request) = &item.request else {
            return Ok(None);
        };
        let Some(locator) = item
            .driver
            .as_ref()
            .unwrap()
            .mask_locator(request.request_id)?
        else {
            return Ok(None);
        };
        if &locator != original {
            return Ok(None);
        }
        let (operands, calculation) = match (&request.operation, locator.stage()) {
            (
                PositionCompositionOperation::MaskAdoption { address, .. },
                PositionMaskStage::Adoption,
            ) if address.representation == DynamicFamilyRepresentation::Angles => {
                ([PositionMaskOperand::AdoptionInput; 2], Calculation::Adopt)
            }
            (
                PositionCompositionOperation::MaskTransition { progress, .. },
                PositionMaskStage::Transition,
            ) if progress.is_finite() && *progress > 0. && *progress < 1. => {
                if original_progress.is_some_and(|original| original != *progress) {
                    return Ok(None);
                }
                original_progress = Some(*progress);
                (
                    [
                        PositionMaskOperand::TransitionFrom,
                        PositionMaskOperand::TransitionTo,
                    ],
                    Calculation::Blend(*progress),
                )
            }
            // A physical Angle pair cannot choose the distance of an inverse Target.
            _ => return Ok(None),
        };
        result.push(Some(PendingCut {
            operands: operands.map(|operand| Goal::Mask(locator.clone(), operand)),
            calculation,
        }));
    }
    Ok(Some(result))
}
fn completion(
    request: &PositionCompositionRequest,
) -> Option<&light_dynamics::PositionCompositionCompletionRequest> {
    let PositionCompositionOperation::Base { request, .. } = &request.operation else {
        return None;
    };
    let mut leaf = request;
    for _ in 0..MAX_NODES {
        match &leaf.operation {
            PositionCompositionBaseOperation::SourceCohort { request, .. } => leaf = request,
            PositionCompositionBaseOperation::Completion(request) => return Some(request),
            _ => return None,
        }
    }
    None
}
pub(super) fn envelope(
    environment: &Environment,
    initiator: usize,
    original: &PositionEnvelopeLocator,
) -> Result<Option<Vec<Option<PendingCut>>>, TransitionError> {
    let mut result = Vec::new();
    let mut original_progress = None;
    for item in &environment.work {
        if item.peer != initiator {
            result.push(None);
            continue;
        }
        let Some(request) = &item.request else {
            return Ok(None);
        };
        let Some(operation) = completion(request) else {
            return Ok(None);
        };
        let Some(locator) = item
            .driver
            .as_ref()
            .unwrap()
            .envelope_locator(request.request_id)?
        else {
            return Ok(None);
        };
        if &locator != original
            || locator.stage() != operation.stage
            || !operation.progress.is_finite()
            || operation.progress <= 0.
            || operation.progress >= 1.
            || original_progress.is_some_and(|original| original != operation.progress)
        {
            return Ok(None);
        }
        original_progress = Some(operation.progress);
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
        result.push(Some(PendingCut {
            operands: operands.map(|operand| Goal::Envelope(locator.clone(), operand)),
            calculation: Calculation::Blend(operation.progress),
        }));
    }
    Ok(Some(result))
}
