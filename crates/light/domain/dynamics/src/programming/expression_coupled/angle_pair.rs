//! Runtime-only deferred Angle pairs. The original Target Current is retained intact until
//! the existing evaluator is bound to one physical destination and captured frame.
use super::*;

#[derive(Clone)]
pub(crate) struct CapturedPositionCurrent {
    pub value: AttributeValue,
    pub occurrence: Option<super::super::DynamicSourceOccurrenceId>,
}

#[derive(Clone)]
pub(crate) enum PositionAngleAxis {
    Materialized(CoupledComponentEndpoint),
    Numeric {
        lane_id: uuid::Uuid,
        address: Arc<CompiledDynamicValueAddress>,
        program: Arc<crate::AngleNumericProgram>,
        original: Arc<CapturedPositionCurrent>,
    },
    Current {
        lane_id: uuid::Uuid,
        address: Arc<CompiledDynamicValueAddress>,
        original: Arc<CapturedPositionCurrent>,
    },
}

#[derive(Clone)]
pub(crate) struct PositionAnglePairEndpoint {
    /// Pan, then Tilt. Both Current axes refer to one original captured family.
    pub axes: [PositionAngleAxis; 2],
}

impl PositionAnglePairEndpoint {
    pub fn validate(&self) -> Result<(), TransitionError> {
        let mut current: Option<&Arc<CapturedPositionCurrent>> = None;
        for (axis, component) in self
            .axes
            .iter()
            .zip([ProgrammingComponent::Pan, ProgrammingComponent::Tilt])
        {
            let address = match axis {
                PositionAngleAxis::Materialized(source) => {
                    source.address.validate_value(&source.value)?;
                    if let Some(dependency) = &source.dependency_occurrence {
                        dependency.validate(ProgrammingOwner::Position)?;
                    }
                    &source.address
                }
                PositionAngleAxis::Current {
                    address, original, ..
                }
                | PositionAngleAxis::Numeric {
                    address, original, ..
                } => {
                    original
                        .value
                        .validate_programming_address(ProgrammingOwner::Position.key_ref())?;
                    if original.value.spread_control_points() != 0
                        || !matches!(&original.value, AttributeValue::Position(_))
                    {
                        return Err(IntentError(
                            "Angle Current needs a complete captured Position family".into(),
                        )
                        .into());
                    }
                    if current.is_some_and(|known| !Arc::ptr_eq(known, original)) {
                        return Err(IntentError(
                            "Angle pair Current axes use different captured families".into(),
                        )
                        .into());
                    }
                    current = Some(original);
                    address
                }
            };
            if let PositionAngleAxis::Numeric { program, .. } = axis {
                program.validate()?;
                if &program.address != address.address() || !program.uses_current() {
                    return Err(IntentError(
                        "numeric Angle axis must use its captured Current and original address"
                            .into(),
                    )
                    .into());
                }
            }
            if address.address().representation != DynamicFamilyRepresentation::Angles
                || address.address().component != Some(component)
            {
                return Err(
                    IntentError("deferred Angle pair must contain Pan then Tilt".into()).into(),
                );
            }
        }
        Ok(())
    }

    pub fn materialize(
        &self,
        frame: &dyn WholeFamilyExpressionFrameResolver,
    ) -> Result<(AttributeValue, [CoupledComponentEndpoint; 2]), TransitionError> {
        // The immutable pair was cold-validated by from_position_forest. Destination frames
        // validate only adopted values; do not recompile a numeric DAG for every physical copy.
        let current = self.axes.iter().find_map(|axis| match axis {
            PositionAngleAxis::Current { original, .. }
            | PositionAngleAxis::Numeric { original, .. } => Some(original),
            _ => None,
        });
        let adopted = current
            .map(|original| match &original.value {
                AttributeValue::Position(value)
                    if matches!(value.as_ref(), PositionIntent::Angles { .. }) =>
                {
                    Ok(original.value.clone())
                }
                _ => frame.adopt_position_angles(&original.value),
            })
            .transpose()?;
        if let Some(value) = &adopted {
            value.validate_programming_address(ProgrammingOwner::Position.key_ref())?;
            if value.spread_control_points() != 0
                || !matches!(value,
                AttributeValue::Position(position) if matches!(position.as_ref(), PositionIntent::Angles { .. }))
            {
                return Err(IntentError(
                    "Position adoption returned an incomplete or non-Angle family".into(),
                )
                .into());
            }
        }
        let source =
            |axis: &PositionAngleAxis| -> Result<CoupledComponentEndpoint, TransitionError> {
                match axis {
                    PositionAngleAxis::Materialized(source) => Ok(source.clone()),
                    PositionAngleAxis::Numeric {
                        lane_id,
                        address,
                        program,
                        original,
                    } => {
                        let current = super::super::extract_compatible_dynamic_value(
                            adopted.as_ref().expect("Current family adopted once"),
                            address.address(),
                            &FamilyEditContext::default(),
                        )?
                        .ok_or(TransitionError::Requires(
                            TransitionRequirement::LiveJointAngles,
                        ))?;
                        let value = program.evaluate_validated(address, Some(&current))?;
                        address.validate_value(&value)?;
                        let compatible = matches!(&original.value, AttributeValue::Position(position)
                            if matches!(position.as_ref(), PositionIntent::Angles { .. }));
                        Ok(CoupledComponentEndpoint {
                            lane_id: *lane_id,
                            address: address.clone(),
                            value,
                            role: CoupledLeafRole::Authored,
                            occurrence: program.occurrence,
                            dependency_occurrence: Some(if compatible {
                                crate::DynamicSourceDependency::compatible(
                                    original.occurrence,
                                    address.address(),
                                )
                            } else {
                                crate::DynamicSourceDependency::unknown(original.occurrence)
                            }),
                        })
                    }
                    PositionAngleAxis::Current {
                        lane_id,
                        address,
                        original,
                    } => {
                        let value = super::super::extract_compatible_dynamic_value(
                            adopted.as_ref().expect("Current family adopted once"),
                            address.address(),
                            &FamilyEditContext::default(),
                        )?
                        .ok_or(TransitionError::Requires(
                            TransitionRequirement::LiveJointAngles,
                        ))?;
                        address.validate_value(&value)?;
                        let compatible = matches!(&original.value, AttributeValue::Position(position)
                        if matches!(position.as_ref(), PositionIntent::Angles { .. }));
                        Ok(CoupledComponentEndpoint {
                            lane_id: *lane_id,
                            address: address.clone(),
                            value,
                            role: CoupledLeafRole::Current,
                            occurrence: None,
                            dependency_occurrence: Some(if compatible {
                                crate::DynamicSourceDependency::compatible(
                                    original.occurrence,
                                    address.address(),
                                )
                            } else {
                                // The additive frame API has no exact field-transfer evidence.
                                crate::DynamicSourceDependency::unknown(original.occurrence)
                            }),
                        })
                    }
                }
            };
        let sources = [source(&self.axes[0])?, source(&self.axes[1])?];
        let [DynamicValue::Scalar(pan), DynamicValue::Scalar(tilt)] =
            [&sources[0].value, &sources[1].value]
        else {
            return Err(
                IntentError("deferred Angle pair axes are not scalar values".into()).into(),
            );
        };
        Ok((
            AttributeValue::Position(Arc::new(PositionIntent::angles(*pan, *tilt))),
            sources,
        ))
    }
}
