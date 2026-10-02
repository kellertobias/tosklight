use super::*;
use crate::validation::{valid_keyframes, valid_size, validate_pwm};
use light_core::programming::{IntentError, ProgrammingComponent};

impl ProgrammingLaneBody {
    pub fn validate(&self) -> Result<(), DynamicValidationError> {
        let typed = |error: IntentError| DynamicValidationError::Programming(error.to_string());
        self.address.validate().map_err(typed)?;
        let source =
            |source: &DynamicValueSource| source.validate_shape(&self.address).map_err(typed);
        let numeric = |size: f32| {
            if self.address.component.is_none() || !valid_size(size) {
                Err(DynamicValidationError::Lane(
                    "numeric configuration requires a component and finite size",
                ))
            } else {
                Ok(())
            }
        };
        match &self.configuration {
            ProgrammingLaneConfiguration::Keyframes(config) => {
                if !valid_keyframes(config) || !valid_size(config.size) {
                    return Err(DynamicValidationError::Lane("keyframe positions or size"));
                }
                if self.address.component.is_none() && config.size != 1.0 {
                    return Err(DynamicValidationError::Lane(
                        "whole-family keyframes require unit lane size",
                    ));
                }
                for point in &config.points {
                    source(&point.source)?;
                }
            }
            ProgrammingLaneConfiguration::MaxMin(config) => {
                numeric(config.size)?;
                validate_pwm(config.pwm)?;
                source(&config.minimum)?;
                source(&config.maximum)?;
            }
            ProgrammingLaneConfiguration::MiddleAmplitude(config) => {
                numeric(config.size)?;
                validate_pwm(config.pwm)?;
                source(&config.middle)?;
                let valid = match (&self.address.component, &config.amplitude) {
                    (Some(ProgrammingComponent::NativeColor(_)), DynamicValue::Native(_)) => true,
                    (Some(ProgrammingComponent::NativeColor(_)), _) => false,
                    (Some(_), DynamicValue::Scalar(value)) => value.is_finite() && *value >= 0.0,
                    _ => false,
                };
                if !valid {
                    return Err(DynamicValidationError::Lane(
                        "amplitude must be a nonnegative component delta",
                    ));
                }
            }
            ProgrammingLaneConfiguration::Random => numeric(1.0)?,
        }
        Ok(())
    }
}

/// Structural checks run before storage/install. The compilation pass additionally checks
/// pinned native bounds; those bounds must never be invented when the source model is absent.
pub(crate) fn validate_random_members(
    definition: &DynamicDefinition,
) -> Result<(), DynamicValidationError> {
    for group in &definition.random_groups {
        let mut previous: Option<&DynamicLane> = None;
        for lane in definition.lanes.iter().filter(|lane| {
            lane.mode() == DynamicLaneMode::Random && lane.random_group_id == Some(group.id)
        }) {
            match (&group.range, &lane.body) {
                (
                    DynamicRandomRange::LegacyScalar { low, high },
                    DynamicLaneBody::LegacyScalar(body),
                ) => {
                    crate::validation::validate_source(low, &body.attribute)?;
                    crate::validation::validate_source(high, &body.attribute)?;
                }
                (
                    DynamicRandomRange::Programming { low, high },
                    DynamicLaneBody::Programming(body),
                ) => {
                    low.validate_shape(&body.address)
                        .and_then(|_| high.validate_shape(&body.address))
                        .map_err(|e| DynamicValidationError::Programming(e.to_string()))?;
                    if let Some(prior) = previous {
                        let DynamicLaneBody::Programming(prior_body) = &prior.body else {
                            return Err(DynamicValidationError::Random);
                        };
                        let a = prior_body
                            .address
                            .component
                            .ok_or(DynamicValidationError::Random)?
                            .descriptor();
                        let b = body
                            .address
                            .component
                            .ok_or(DynamicValidationError::Random)?
                            .descriptor();
                        let speeds_match = u64::from(prior.speed_multiplier.numerator)
                            * u64::from(lane.speed_multiplier.denominator)
                            == u64::from(lane.speed_multiplier.numerator)
                                * u64::from(prior.speed_multiplier.denominator);
                        if a.unit != b.unit || a.domain != b.domain || !speeds_match {
                            return Err(DynamicValidationError::Random);
                        }
                    }
                    previous = Some(lane);
                }
                _ => return Err(DynamicValidationError::Random),
            }
        }
    }
    Ok(())
}
