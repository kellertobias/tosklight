//! One physical destination evaluates the original Position program against its captured
//! mount, calibration and accepted branch anchors. Temporary endpoint conversions never
//! advance continuity; only the final complete-cohort result is staged.
use super::*;
use light_core::programming::CompiledProgrammingTransition;
use light_dynamics::{DynamicFamilyRepresentation, WholeFamilyExpressionFrameResolver};

pub(super) struct PositionDestinationFrame<'a> {
    pub adapter: &'a PositionAdapter,
    pub frame: HybridFrameContext<'a>,
    pub descriptor: &'a PositionDescriptor,
    pub target: FixtureId,
    pub instance: &'a Arc<PositionInstance>,
    pub previous: Option<&'a PositionContinuity>,
    pub current: &'a current_cohort::CapturedCurrentCohorts,
    pub active_programs: &'a [FixtureId],
}
impl PositionDestinationFrame<'_> {
    pub fn adopt(
        &self,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        if address.owner() != ProgrammingOwner::Position
            || address.representation != DynamicFamilyRepresentation::Angles
        {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        if matches!(intent(original)?, PositionIntent::Angles { .. }) {
            return Ok(original.clone());
        }
        // One changing owner with proven constant static peers has an unambiguous complete
        // endpoint cohort. Multiple changing histories still need explicit branch choices.
        if let Some(adopted) = self.current.adopt_single_program_endpoint(
            self.adapter,
            self.frame,
            self.target,
            self.instance.destination,
            original,
            self.active_programs,
        )? {
            return Ok(adopted);
        }
        // No complete endpoint cohort: retain the gap passively rather than borrow a peer's
        // Current as though it were that peer's outgoing or incoming Dynamic endpoint.
        let owned = &self.descriptor.emitters;
        for emitter in self
            .instance
            .model
            .emitters()
            .filter(|e| owned.contains(&e.emitter_index))
        {
            for other in self
                .instance
                .model
                .emitters()
                .filter(|e| !owned.contains(&e.emitter_index))
            {
                if emitter.ancestor_axes.iter().any(|axis| {
                    other.ancestor_axes.contains(axis)
                        && matches!(
                            self.instance.model.axes()[*axis].role,
                            Some(PositionAxisRole::Pan | PositionAxisRole::Tilt)
                        )
                }) {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::LiveJointAngles,
                    ));
                }
            }
        }
        let descriptor = PositionDescriptor {
            root: self.descriptor.root,
            instances: vec![Arc::clone(self.instance)],
            emitters: self.descriptor.emitters.clone(),
            footprint: self
                .descriptor
                .footprint
                .iter()
                .copied()
                .filter(|s| s.destination == self.instance.destination)
                .collect(),
        };
        self.adapter.adopt_with_continuity(
            self.frame,
            &descriptor,
            self.target,
            original,
            address,
            self.previous,
        )
    }
}
impl WholeFamilyExpressionFrameResolver for PositionDestinationFrame<'_> {
    fn adopt_position_angles(
        &self,
        original: &AttributeValue,
    ) -> Result<AttributeValue, TransitionError> {
        if let Some(adopted) = self.current.adopt(
            self.adapter,
            self.frame,
            self.target,
            self.instance.destination,
            original,
        )? {
            return Ok(adopted);
        }
        self.adopt(
            original,
            &DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: None,
            },
        )
    }
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        if requirement == TransitionRequirement::LiveTargetPoints {
            // Resolve both references in this capture and interpolate world aim coordinates.
            return self
                .adapter
                .transition(
                    self.frame,
                    self.descriptor,
                    self.target,
                    requirement,
                    from,
                    to,
                    operation,
                )
                .map(|(value, _)| value);
        }
        if requirement != TransitionRequirement::LiveJointAngles {
            return Err(TransitionError::Requires(requirement));
        }
        let address = DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: None,
        };
        let from = self.adopt(from, &address)?;
        let to = self.adopt(to, &address)?;
        let transition = CompiledProgrammingTransition::new(from, to, None)?;
        match operation {
            FamilyExpressionOperation::Transition { progress } => transition.sample(progress),
            FamilyExpressionOperation::Scale { factor } => transition.scale(factor),
        }
    }
}
