//! TL-600 whole Color activation across Direct source identities.
//!
//! A whole FixAT/Cue Color mask fades from its eligible underlay. Normally the underlay is
//! adopted into the mask's representation and interpolated natively. A foreign Direct
//! destination cannot supply that source identity on this fixture, so adoption reports
//! `Requires(ColorAppearance)`. The original whole from/to Transition is then sampled by the
//! caller's pinned frame resolver, which owns the captured appearance policy:
//! - exact endpoints (the requested Direct destination is restored unchanged at completion);
//! - a shared portable appearance interpolation at interior samples;
//! - an explicit hold of the source when an appearance is unknown.
//!
//! This layer never fabricates a native recipe: a Direct frame result must be one of the two
//! original tagged endpoints. Invalid input, missing models and unrelated requirements are
//! never caught here; they reach the caller unchanged.
use super::*;
use crate::{FamilyExpressionOperation, WholeFamilyExpressionFrameResolver};

pub(super) struct WholeActivation {
    pub value: AttributeValue,
    /// Some when the frame resolver sampled the representation crossing. The transfer is the
    /// resolver's own evidence (None is unknown), never exact native-component attribution.
    pub appearance: Option<Option<ProgrammingTransitionTrace>>,
}

impl WholeActivation {
    pub(super) fn native(value: AttributeValue) -> Self {
        Self {
            value,
            appearance: None,
        }
    }
}

/// Only a whole Color sample over a Color underlay may be sampled by the appearance resolver.
pub(super) fn routes_color_appearance(
    address: &DynamicValueAddress,
    underlay: &AttributeValue,
) -> bool {
    address.component.is_none()
        && matches!(
            address.representation,
            DynamicFamilyRepresentation::SemanticColor { .. }
                | DynamicFamilyRepresentation::DirectColor { .. }
        )
        && matches!(underlay, AttributeValue::ColorProgram(_))
}

pub(super) fn appearance_transition(
    underlay: &AttributeValue,
    target: &AttributeValue,
    sample: &FamilySample,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    traced: bool,
) -> Result<WholeActivation, TransitionError> {
    let requirement = TransitionRequirement::ColorAppearance;
    let operation = FamilyExpressionOperation::Transition {
        progress: sample.activation_mix,
    };
    let (value, transfer) = if traced {
        frame.resolve_with_trace(requirement, underlay, target, operation)?
    } else {
        (
            frame.resolve(requirement, underlay, target, operation)?,
            None,
        )
    };
    ensure(
        value.programming_owner() == Some(ProgrammingOwner::Color)
            && value.spread_control_points() == 0
            && !matches!(value, AttributeValue::GroupFamily(_)),
        "frame resolver returned an incompatible or unmaterialized Color family",
    )?;
    value.validate_programming_address(ProgrammingOwner::Color.key_ref())?;
    if let AttributeValue::ColorProgram(program) = &value
        && matches!(program.as_ref(), ColorProgram::Direct { .. })
    {
        // A foreign layout has no verifiable native recipe on this destination. Only the
        // original endpoints themselves (exact endpoint or explicit hold) may be Direct.
        ensure(
            &value == underlay || &value == target,
            "frame resolver fabricated a Direct recipe outside the original endpoints",
        )?;
    }
    Ok(WholeActivation {
        value,
        appearance: Some(transfer),
    })
}
