//! TL-559 cross-representation Color transitions (Semantic↔Direct, or Direct across native
//! sources) for the existing hybrid resolver seam. The composer keeps ONE Color owner and calls
//! this only after the core complete-family transition returned `Requires(ColorAppearance)`.
//!
//! - Endpoints are exact: progress ≤ 0 returns `from`, progress ≥ 1 returns `to` unchanged, so a
//!   Direct destination restores its exact native recipe at completion.
//! - Interior samples blend portable appearance through the core complete-family operation
//!   `interpolate_programming_value` on the two semantic endpoints. A Direct endpoint becomes
//!   semantic through `semantic_color_adoption` of its source estimate (original model when
//!   available, else the recorded estimate): total XYZ, relative output and UV are kept, unknown
//!   UV is off, known black stays black. UV therefore fades between the two endpoint amounts and
//!   never keeps the previous endpoint's UV at completion.
//! - Unknown visible appearance on either side is never faded: the source value is held and the
//!   destination replaces it at completion (no fabricated smooth color fade).
//! - `Scale` across representations stays a passive requirement.
//!
//! Current adoption (`adopt_representation`) is the composer's request to express the current
//! Color value in a fade's destination representation for ONE composition; it is never stored:
//! - Direct → Semantic: the shared `semantic_color_adoption` of the source estimate (the
//!   recorded estimate when the original is unavailable; missing source A never blocks it).
//!   Unknown visible appearance stays `Requires(ColorAppearance)`.
//! - Semantic (or Direct of another source) → Direct of source S: only on a head whose verified
//!   identity IS S. The value is fitted through the normal per-head path under this frame's
//!   token (continuity included), each control's function is derived from the descriptor and the
//!   fitted raw, and the complete recipe is captured by the verified original model
//!   (`capture_direct_color`). Any other head keeps the passive requirement: no recipe of a
//!   foreign layout is ever invented.
use super::native::{PublishedColorHead, adoption_estimate, inspect_native_values};
use super::*;
use light_core::programming::{
    NativeColorObservation, capture_direct_color, interpolate_programming_value,
    semantic_color_adoption,
};
use light_dynamics::{
    DynamicFamilyRepresentation, DynamicNativeModelResolver, DynamicValueAddress,
    NativeColorModelCapability,
};

fn program(value: &AttributeValue) -> Option<&ColorProgram> {
    match value {
        AttributeValue::ColorProgram(program) => Some(program.as_ref()),
        _ => None,
    }
}

/// Only representation crossings belong to the frame; same-representation transitions stay
/// with the core operation.
fn crosses_representation(a: &ColorProgram, b: &ColorProgram) -> bool {
    match (a, b) {
        (ColorProgram::Semantic { .. }, ColorProgram::Semantic { .. }) => false,
        (ColorProgram::Direct { recipe: a, .. }, ColorProgram::Direct { recipe: b, .. }) => {
            a.source != b.source
        }
        _ => true,
    }
}

impl ColorAdapter {
    /// The semantic appearance an endpoint stands for; None when its visible appearance is
    /// unknown (the endpoint cannot take part in a smooth fade).
    fn portable_intent(
        program: &ColorProgram,
        models: &dyn DynamicNativeModelResolver,
    ) -> Result<Option<ColorIntent>, TransitionError> {
        match program {
            ColorProgram::Semantic { intent } => Ok(Some(intent.clone())),
            ColorProgram::Direct { .. } => {
                let estimate = adoption_estimate(program, models)?;
                if estimate.visible.is_none() {
                    return Ok(None);
                }
                Ok(Some(semantic_color_adoption(&estimate, None)?.intent))
            }
        }
    }

    pub(super) fn transition_representations(
        &self,
        frame: HybridFrameContext<'_>,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        let passive = || Err(TransitionError::Requires(requirement));
        let FamilyExpressionOperation::Transition { progress } = operation else {
            return passive();
        };
        let (Some(a), Some(b)) = (program(from), program(to)) else {
            return passive();
        };
        if requirement != TransitionRequirement::ColorAppearance || !crosses_representation(a, b) {
            return passive();
        }
        if !progress.is_finite() {
            return Err(invalid("Color transition progress is nonfinite"));
        }
        a.validate()?;
        b.validate()?;
        if progress <= 0.0 {
            return Ok((from.clone(), None));
        }
        if progress >= 1.0 {
            return Ok((to.clone(), None));
        }
        self.count(|c| c.representation_transitions += 1);
        let models = frame.native_models;
        let (Some(start), Some(end)) = (
            Self::portable_intent(a, models)?,
            Self::portable_intent(b, models)?,
        ) else {
            // Unknown appearance: hold the source until the destination takes over at 1.
            self.count(|c| c.representation_holds += 1);
            return Ok((from.clone(), None));
        };
        let semantic =
            |intent| AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }));
        let value = interpolate_programming_value(&semantic(start), &semantic(end), progress)?;
        Ok((value, None))
    }
}

impl ColorAdapter {
    pub(super) fn adopt_representation(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &ColorDescriptor,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
        previous: Option<&ColorContinuity>,
    ) -> Result<AttributeValue, TransitionError> {
        let passive = || {
            Err(TransitionError::Requires(
                TransitionRequirement::ColorAppearance,
            ))
        };
        let Some(current) = program(original) else {
            return passive();
        };
        current.validate()?;
        match (current, &address.representation) {
            (ColorProgram::Direct { .. }, DynamicFamilyRepresentation::SemanticColor { .. }) => {
                match Self::portable_intent(current, frame.native_models)? {
                    Some(intent) => Ok(AttributeValue::ColorProgram(Arc::new(
                        ColorProgram::Semantic { intent },
                    ))),
                    None => passive(),
                }
            }
            (
                ColorProgram::Direct { recipe, .. },
                DynamicFamilyRepresentation::DirectColor { source },
            ) if &recipe.source == source => Ok(original.clone()),
            (_, DynamicFamilyRepresentation::DirectColor { source }) => {
                let Some(intent) = Self::portable_intent(current, frame.native_models)? else {
                    return passive();
                };
                self.adopt_as_direct(frame, descriptor, target, intent, source, previous)
            }
            _ => passive(),
        }
    }

    fn adopt_as_direct(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &ColorDescriptor,
        target: FixtureId,
        intent: ColorIntent,
        source: &NativeColorIdentity,
        previous: Option<&ColorContinuity>,
    ) -> Result<AttributeValue, TransitionError> {
        let Some(head) = descriptor
            .heads
            .iter()
            .find(|h| h.destination == descriptor.root && h.native.as_ref() == Some(source))
        else {
            return Err(TransitionError::Requires(
                TransitionRequirement::ColorAppearance,
            ));
        };
        let model = match frame.native_models.resolve_capability(source)? {
            NativeColorModelCapability::Available(model) => model,
            NativeColorModelCapability::Unavailable(_) => {
                return Err(TransitionError::Requires(
                    TransitionRequirement::NativeColorModel,
                ));
            }
        };
        let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }));
        let fitted = self.resolve_heads(PhysicalRequest {
            frame,
            target,
            owner: ProgrammingOwner::Color,
            descriptor,
            value: &value,
            previous,
        })?;
        let values = inspect_native_values(
            head,
            &PublishedColorHead {
                token: frame.token,
                target,
                value: &value,
                writes: &fitted.writes,
            },
        )?;
        let captured = capture_direct_color(
            model.as_ref(),
            NativeColorObservation {
                source: source.clone(),
                values,
            },
        )?;
        self.count(|c| c.representation_adoptions += 1);
        Ok(captured.into_value())
    }
}
