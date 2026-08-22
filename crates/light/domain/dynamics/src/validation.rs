use crate::*;
use light_core::attribute_descriptor;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DynamicValidationError {
    #[error("Dynamic pool number must be within 1-9999")]
    PoolNumber,
    #[error("Dynamic revision must be positive")]
    Revision,
    #[error("Dynamic needs at least one scalar lane")]
    Empty,
    #[error("Dynamic lane IDs and Random group IDs must be unique")]
    DuplicateIdentity,
    #[error("attribute {0} is not a known recordable continuous scalar")]
    UnsupportedAttribute(String),
    #[error("lane configuration is invalid: {0}")]
    Lane(&'static str),
    #[error("scalar source does not match its lane attribute")]
    SourceAttribute,
    #[error("Random group configuration or reference is invalid")]
    Random,
    #[error("phase distribution is invalid")]
    Phase,
    #[error("speed configuration is invalid")]
    Speed,
    #[error("target binding is invalid")]
    Targets,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicAliasingWarning {
    pub shortest_segment_millis: u64,
    pub output_interval_millis: u64,
    pub samples_per_segment: u64,
}

pub fn aliasing_warning(
    definition: &DynamicDefinition,
    effective_cycle_millis: u64,
    output_interval_millis: u64,
) -> Option<DynamicAliasingWarning> {
    let output_interval_millis = output_interval_millis.max(1);
    let shortest = definition
        .lanes
        .iter()
        .filter_map(|lane| {
            let fraction = match lane.mode {
                DynamicLaneMode::Keyframes => lane
                    .keyframes
                    .points
                    .windows(2)
                    .map(|pair| pair[1].position - pair[0].position)
                    .chain(
                        lane.keyframes
                            .points
                            .last()
                            .map(|point| 1.0 - point.position),
                    )
                    .filter(|fraction| *fraction > 0.0)
                    .reduce(f32::min),
                DynamicLaneMode::MaxMin => shortest_pwm_segment(lane.max_min.pwm),
                DynamicLaneMode::MiddleAmplitude => shortest_pwm_segment(lane.middle_amplitude.pwm),
                DynamicLaneMode::Random => definition
                    .random_groups
                    .iter()
                    .find(|group| Some(group.id) == lane.random_group_id)
                    .map(|group| {
                        group.decision_interval_millis as f32 / effective_cycle_millis.max(1) as f32
                    }),
            }?;
            let lane_speed = lane.speed_multiplier.factor().max(f64::EPSILON);
            Some(
                (effective_cycle_millis as f64 * f64::from(fraction) / lane_speed)
                    .round()
                    .max(1.0) as u64,
            )
        })
        .min()?;
    let samples = shortest / output_interval_millis;
    (samples < 4).then_some(DynamicAliasingWarning {
        shortest_segment_millis: shortest,
        output_interval_millis,
        samples_per_segment: samples,
    })
}

fn shortest_pwm_segment(shape: PwmShape) -> Option<f32> {
    [
        shape.attack,
        shape.on - shape.attack,
        shape.decay,
        shape.off - shape.decay,
    ]
    .into_iter()
    .filter(|segment| *segment > 0.0)
    .reduce(f32::min)
}

pub fn validate_definition(definition: &DynamicDefinition) -> Result<(), DynamicValidationError> {
    if !(1..=9999).contains(&definition.pool_number) {
        return Err(DynamicValidationError::PoolNumber);
    }
    if definition.revision == 0 {
        return Err(DynamicValidationError::Revision);
    }
    if definition.lanes.is_empty() {
        return Err(DynamicValidationError::Empty);
    }
    if matches!(
        &definition.target_binding,
        DynamicTargetBinding::LiveGroup { group_id } if group_id.trim().is_empty()
    ) || matches!(
        &definition.target_binding,
        DynamicTargetBinding::FrozenTargets { targets } if targets.is_empty()
    ) {
        return Err(DynamicValidationError::Targets);
    }
    let mut ids = HashSet::new();
    for group in &definition.random_groups {
        if !ids.insert(group.id) {
            return Err(DynamicValidationError::DuplicateIdentity);
        }
        validate_random(group)?;
    }
    let random_ids = definition
        .random_groups
        .iter()
        .map(|group| group.id)
        .collect::<HashSet<_>>();
    ids.clear();
    for lane in &definition.lanes {
        if !ids.insert(lane.id) {
            return Err(DynamicValidationError::DuplicateIdentity);
        }
        validate_lane(lane, &random_ids)?;
    }
    if !valid_phase(&definition.phase)
        || definition
            .lanes
            .iter()
            .filter_map(|lane| lane.phase.as_ref())
            .any(|phase| !valid_phase(phase))
    {
        return Err(DynamicValidationError::Phase);
    }
    validate_speed(&definition.speed)?;
    if !valid_rational(definition.overall_speed_multiplier) {
        return Err(DynamicValidationError::Speed);
    }
    if matches!(definition.speed, DynamicSpeed::Fixed { .. })
        && definition.default_activation != ActivationPolicy::StartNow
    {
        return Err(DynamicValidationError::Speed);
    }
    Ok(())
}

fn valid_phase(phase: &crate::PhaseDistribution) -> bool {
    phase.offset_degrees.is_finite()
        && phase.span_degrees.is_finite()
        && phase.block_size > 0
        && phase.repeats > 0
        && phase.anchors_degrees.iter().all(|value| value.is_finite())
}

fn validate_lane(
    lane: &DynamicLane,
    random_ids: &HashSet<uuid::Uuid>,
) -> Result<(), DynamicValidationError> {
    if !attribute_descriptor(&lane.attribute).supports_dynamics() {
        return Err(DynamicValidationError::UnsupportedAttribute(
            lane.attribute.0.to_string(),
        ));
    }
    if !valid_rational(lane.speed_multiplier)
        || !lane.width.is_finite()
        || lane.width < 0.0
        || !valid_size(lane.keyframes.size)
        || !valid_size(lane.max_min.size)
        || !valid_size(lane.middle_amplitude.size)
        || !lane.middle_amplitude.amplitude.is_finite()
        || lane.middle_amplitude.amplitude < 0.0
    {
        return Err(DynamicValidationError::Lane("numeric bounds"));
    }
    if lane.keyframes.points.len() < 2
        || lane.keyframes.points.first().map(|point| point.position) != Some(0.0)
        || lane
            .keyframes
            .points
            .windows(2)
            .any(|pair| pair[0].position >= pair[1].position)
        || lane
            .keyframes
            .points
            .last()
            .is_none_or(|point| point.position >= 1.0)
    {
        return Err(DynamicValidationError::Lane("keyframe positions"));
    }
    for source in lane_sources(lane) {
        validate_source(source, &lane.attribute)?;
    }
    validate_pwm(lane.max_min.pwm)?;
    validate_pwm(lane.middle_amplitude.pwm)?;
    if lane.mode == DynamicLaneMode::Random
        && lane
            .random_group_id
            .is_none_or(|id| !random_ids.contains(&id))
    {
        return Err(DynamicValidationError::Random);
    }
    Ok(())
}

fn lane_sources(lane: &DynamicLane) -> impl Iterator<Item = &ScalarSource> {
    lane.keyframes
        .points
        .iter()
        .map(|point| &point.source)
        .chain([&lane.max_min.minimum, &lane.max_min.maximum])
        .chain([&lane.middle_amplitude.middle])
}

fn validate_source(
    source: &ScalarSource,
    lane_attribute: &light_core::AttributeKey,
) -> Result<(), DynamicValidationError> {
    match source {
        ScalarSource::Current => Ok(()),
        ScalarSource::Value { value } if value.is_finite() => Ok(()),
        ScalarSource::Preset {
            attribute,
            last_valid_by_target,
            ..
        } if attribute == lane_attribute
            && last_valid_by_target
                .iter()
                .all(|fallback| fallback.value.is_finite()) =>
        {
            Ok(())
        }
        ScalarSource::Preset { .. } => Err(DynamicValidationError::SourceAttribute),
        ScalarSource::Value { .. } => Err(DynamicValidationError::Lane("non-finite source")),
    }
}

fn validate_random(group: &DynamicRandomGroup) -> Result<(), DynamicValidationError> {
    if group.decision_interval_millis == 0
        || group.mean_duration_millis == 0
        || !group.start_probability.is_finite()
        || !(0.0..=1.0).contains(&group.start_probability)
        || !group.attack_ratio.is_finite()
        || !group.decay_ratio.is_finite()
        || group.attack_ratio < 0.0
        || group.decay_ratio < 0.0
        || group.attack_ratio + group.decay_ratio > 1.0
    {
        return Err(DynamicValidationError::Random);
    }
    Ok(())
}

fn validate_pwm(shape: PwmShape) -> Result<(), DynamicValidationError> {
    let values = [shape.attack, shape.on, shape.decay, shape.off];
    if values
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        || shape.attack > shape.on
        || shape.decay > shape.off
        || shape.on + shape.off <= 0.0
    {
        return Err(DynamicValidationError::Lane("PWM partition"));
    }
    Ok(())
}

fn validate_speed(speed: &DynamicSpeed) -> Result<(), DynamicValidationError> {
    match speed {
        DynamicSpeed::Fixed { duration_millis } if *duration_millis > 0 => Ok(()),
        DynamicSpeed::SpeedGroup {
            beats_per_cycle, ..
        } if valid_rational(*beats_per_cycle) => Ok(()),
        _ => Err(DynamicValidationError::Speed),
    }
}

fn valid_rational(value: Rational) -> bool {
    value.numerator > 0 && value.denominator > 0
}

fn valid_size(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}
