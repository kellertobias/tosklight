use crate::*;
use light_core::{AttributeKey, FixtureId};
use std::f64::consts::TAU;
use uuid::Uuid;

pub trait ScalarSourceResolver {
    fn current(&self, target: FixtureId, attribute: &AttributeKey) -> Option<f32>;
    fn preset(&self, preset_id: &str, target: FixtureId, attribute: &AttributeKey) -> Option<f32>;
}

#[derive(Clone, Copy)]
pub struct DynamicEvaluationContext<'a> {
    pub instance_id: Uuid,
    pub target: FixtureId,
    pub elapsed_millis: u64,
    pub cycle_duration_millis: u64,
    pub phase_degrees: f32,
    pub output_interval_millis: u64,
    /// Runtime-owned correlated Random envelope for this instance/target/group. Direct evaluator
    /// callers may leave it `None` for deterministic point sampling.
    pub random_envelope: Option<f32>,
    pub sources: &'a dyn ScalarSourceResolver,
}

pub struct DynamicEvaluator<'a> {
    definition: &'a DynamicDefinition,
}

impl<'a> DynamicEvaluator<'a> {
    pub const fn new(definition: &'a DynamicDefinition) -> Self {
        Self { definition }
    }

    pub fn sample_lane(
        &self,
        lane: &DynamicLane,
        context: DynamicEvaluationContext<'_>,
    ) -> Option<f32> {
        let duration =
            (context.cycle_duration_millis as f64 / lane.speed_multiplier.factor()).max(1.0);
        let interval_position = ((context.elapsed_millis as f64 / duration)
            + f64::from(context.phase_degrees) / 360.0)
            .rem_euclid(1.0) as f32;
        let width = lane.width.clamp(f32::EPSILON, 1.0);
        let position = ((interval_position - (1.0 - width) * 0.5) / width).clamp(0.0, 1.0);
        let value = match lane.mode {
            DynamicLaneMode::Keyframes => self.keyframes(lane, position, context)?,
            DynamicLaneMode::MaxMin => self.max_min(lane, position, context)?,
            DynamicLaneMode::MiddleAmplitude => self.middle_amplitude(lane, position, context)?,
            DynamicLaneMode::Random => self.random(lane, context)?,
        };
        let bounds = light_core::attribute_descriptor(&lane.attribute).normalized_bounds?;
        Some(value.clamp(bounds.min, bounds.max))
    }

    fn keyframes(
        &self,
        lane: &DynamicLane,
        position: f32,
        context: DynamicEvaluationContext<'_>,
    ) -> Option<f32> {
        let points = &lane.keyframes.points;
        let left_index = points
            .iter()
            .rposition(|point| point.position <= position)
            .unwrap_or(0);
        let left = &points[left_index];
        let right = points.get(left_index + 1).unwrap_or(&points[0]);
        let right_position = if left_index + 1 < points.len() {
            right.position
        } else {
            1.0
        };
        let progress = ((position - left.position)
            / (right_position - left.position).max(f32::EPSILON))
        .clamp(0.0, 1.0);
        let left_value = resolve_source(&left.source, &lane.attribute, context)?;
        let right_value = resolve_source(&right.source, &lane.attribute, context)?;
        let pivot = resolve_source(&points[0].source, &lane.attribute, context)?;
        let value =
            left_value + (right_value - left_value) * interpolate(progress, left.interpolation);
        Some(pivot + (value - pivot) * lane.keyframes.size)
    }

    fn max_min(
        &self,
        lane: &DynamicLane,
        position: f32,
        context: DynamicEvaluationContext<'_>,
    ) -> Option<f32> {
        let low = resolve_source(&lane.max_min.minimum, &lane.attribute, context)?;
        let high = resolve_source(&lane.max_min.maximum, &lane.attribute, context)?;
        let amount = periodic(lane.max_min.function, position, lane.max_min.pwm);
        let middle = (low + high) * 0.5;
        Some(middle + (low + (high - low) * amount - middle) * lane.max_min.size)
    }

    fn middle_amplitude(
        &self,
        lane: &DynamicLane,
        position: f32,
        context: DynamicEvaluationContext<'_>,
    ) -> Option<f32> {
        let middle = resolve_source(&lane.middle_amplitude.middle, &lane.attribute, context)?;
        let amount = periodic(
            lane.middle_amplitude.function,
            position,
            lane.middle_amplitude.pwm,
        ) * 2.0
            - 1.0;
        Some(middle + amount * lane.middle_amplitude.amplitude * lane.middle_amplitude.size)
    }

    fn random(&self, lane: &DynamicLane, context: DynamicEvaluationContext<'_>) -> Option<f32> {
        let group = self
            .definition
            .random_groups
            .iter()
            .find(|group| Some(group.id) == lane.random_group_id)?;
        let interval = group.decision_interval_millis.max(1);
        let decision = context.elapsed_millis / interval;
        let start = uniform(group.seed, context.instance_id, context.target, decision);
        let low = resolve_source(&group.low, &lane.attribute, context)?;
        if let Some(envelope) = context.random_envelope {
            let high = resolve_source(&group.high, &lane.attribute, context)?;
            return Some(low + (high - low) * envelope.clamp(0.0, 1.0));
        }
        if start > f64::from(group.start_probability) {
            return Some(low);
        }
        let gaussian = gaussian(group.seed, context.instance_id, context.target, decision);
        let duration = (group.mean_duration_millis as f64
            + gaussian * group.duration_spread_millis as f64)
            .round()
            .max(context.output_interval_millis.max(1) as f64) as u64;
        let elapsed = context.elapsed_millis % interval;
        if elapsed >= duration {
            return Some(low);
        }
        let high = resolve_source(&group.high, &lane.attribute, context)?;
        let progress = elapsed as f32 / duration as f32;
        let envelope = if group.attack_ratio > 0.0 && progress < group.attack_ratio {
            progress / group.attack_ratio
        } else if group.decay_ratio > 0.0 && progress > 1.0 - group.decay_ratio {
            (1.0 - progress) / group.decay_ratio
        } else {
            1.0
        };
        Some(low + (high - low) * envelope.clamp(0.0, 1.0))
    }
}

fn resolve_source(
    source: &ScalarSource,
    lane: &AttributeKey,
    context: DynamicEvaluationContext<'_>,
) -> Option<f32> {
    match source {
        ScalarSource::Current => context.sources.current(context.target, lane),
        ScalarSource::Value { value } => Some(*value),
        ScalarSource::Preset {
            preset_id,
            attribute,
            last_valid_by_target,
        } => context
            .sources
            .preset(preset_id, context.target, attribute)
            .or_else(|| {
                last_valid_by_target
                    .iter()
                    .find(|fallback| fallback.target == context.target)
                    .map(|fallback| fallback.value)
            }),
    }
}

fn periodic(function: PeriodicFunction, position: f32, pwm: PwmShape) -> f32 {
    match function {
        PeriodicFunction::Sinus => ((f64::from(position) * TAU).sin() * 0.5 + 0.5) as f32,
        PeriodicFunction::Cosinus => ((f64::from(position) * TAU).cos() * 0.5 + 0.5) as f32,
        PeriodicFunction::LinearUp => position,
        PeriodicFunction::LinearDown => 1.0 - position,
        PeriodicFunction::Pwm => pwm_value(position, pwm),
    }
}

fn pwm_value(position: f32, shape: PwmShape) -> f32 {
    let total = (shape.on + shape.off).max(f32::EPSILON);
    let on_end = shape.on / total;
    let attack_end = (shape.attack / total).min(on_end);
    let decay_start = on_end;
    let decay_end = (on_end + shape.decay / total).min(1.0);
    if attack_end > 0.0 && position < attack_end {
        interpolate(position / attack_end, shape.attack_interpolation)
    } else if position < decay_start {
        1.0
    } else if decay_end > decay_start && position < decay_end {
        1.0 - interpolate(
            (position - decay_start) / (decay_end - decay_start),
            shape.decay_interpolation,
        )
    } else {
        0.0
    }
}

fn interpolate(progress: f32, interpolation: ScalarInterpolation) -> f32 {
    match interpolation {
        ScalarInterpolation::Linear => progress,
        ScalarInterpolation::EaseIn => progress * progress,
        ScalarInterpolation::EaseOut => 1.0 - (1.0 - progress) * (1.0 - progress),
        ScalarInterpolation::EaseInOut => progress * progress * (3.0 - 2.0 * progress),
        ScalarInterpolation::Hold => 0.0,
        ScalarInterpolation::Drop => f32::from(progress >= 1.0),
    }
}

pub(crate) fn uniform(seed: u64, instance: Uuid, target: FixtureId, index: u64) -> f64 {
    let value = mixed(seed, instance, target, index);
    (value as f64 + 1.0) / (u64::MAX as f64 + 2.0)
}

pub(crate) fn gaussian(seed: u64, instance: Uuid, target: FixtureId, index: u64) -> f64 {
    let left = uniform(seed, instance, target, index.wrapping_mul(2));
    let right = uniform(
        seed,
        instance,
        target,
        index.wrapping_mul(2).wrapping_add(1),
    );
    (-2.0 * left.ln()).sqrt() * (TAU * right).cos()
}

fn mixed(seed: u64, instance: Uuid, target: FixtureId, index: u64) -> u64 {
    let mut value = seed ^ index.rotate_left(23);
    for byte in instance.as_bytes().iter().chain(target.0.as_bytes()) {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x9e37_79b1_85eb_ca87);
        value ^= value >> 29;
    }
    value
}
