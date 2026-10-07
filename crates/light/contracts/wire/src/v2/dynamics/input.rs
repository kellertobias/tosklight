//! Tolerant wire input with explicit detection of mutually exclusive known body fields.
//! Derived field parsing lets the shared extractor log unknown paths without retaining values.
use super::*;
use serde::{Deserializer, de::Error};

enum Field<T> {
    Missing,
    Present(T),
}
impl<T> Default for Field<T> {
    fn default() -> Self {
        Self::Missing
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Field<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self::Present)
    }
}
impl<T> Field<T> {
    fn present(&self) -> bool {
        matches!(self, Self::Present(_))
    }
    fn required<E: Error>(self, field: &'static str) -> Result<T, E> {
        match self {
            Self::Present(value) => Ok(value),
            Self::Missing => Err(E::missing_field(field)),
        }
    }
}

#[derive(Deserialize)]
struct LaneInput {
    id: Uuid,
    speed_multiplier: DynamicRationalProjection,
    width: f32,
    #[serde(default)]
    random_group_id: Option<Uuid>,
    #[serde(default)]
    phase: Option<DynamicPhaseDistributionProjection>,
    #[serde(default)]
    attribute: Field<String>,
    #[serde(default)]
    mode: Field<DynamicLaneModeProjection>,
    #[serde(default)]
    keyframes: Field<DynamicKeyframeConfigurationProjection>,
    #[serde(default)]
    max_min: Field<DynamicMaxMinConfigurationProjection>,
    #[serde(default)]
    middle_amplitude: Field<DynamicMiddleAmplitudeConfigurationProjection>,
    #[serde(default)]
    programming: Field<DynamicProgrammingLaneProjection>,
}

impl<'de> Deserialize<'de> for DynamicLaneProjection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = LaneInput::deserialize(deserializer)?;
        let body = match input.programming {
            Field::Present(programming) => {
                if input.attribute.present()
                    || input.mode.present()
                    || input.keyframes.present()
                    || input.max_min.present()
                    || input.middle_amplitude.present()
                {
                    return Err(D::Error::custom(
                        "a typed Dynamic lane cannot also contain scalar configuration fields",
                    ));
                }
                DynamicLaneBodyProjection::Programming { programming }
            }
            Field::Missing => {
                DynamicLaneBodyProjection::LegacyScalar(DynamicLegacyScalarLaneProjection {
                    attribute: input.attribute.required("attribute")?,
                    mode: input.mode.required("mode")?,
                    keyframes: input.keyframes.required("keyframes")?,
                    max_min: input.max_min.required("max_min")?,
                    middle_amplitude: input.middle_amplitude.required("middle_amplitude")?,
                })
            }
        };
        Ok(Self {
            id: input.id,
            body,
            speed_multiplier: input.speed_multiplier,
            width: input.width,
            random_group_id: input.random_group_id,
            phase: input.phase,
        })
    }
}

#[derive(Deserialize)]
struct RandomGroupInput {
    id: Uuid,
    seed: u64,
    decision_interval_millis: u64,
    start_probability: f32,
    mean_duration_millis: u64,
    duration_spread_millis: u64,
    attack_ratio: f32,
    decay_ratio: f32,
    #[serde(default)]
    low: Field<DynamicScalarSourceProjection>,
    #[serde(default)]
    high: Field<DynamicScalarSourceProjection>,
    #[serde(default)]
    programming_range: Field<DynamicProgrammingRandomRangeProjection>,
}

impl<'de> Deserialize<'de> for DynamicRandomGroupProjection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = RandomGroupInput::deserialize(deserializer)?;
        let range = match input.programming_range {
            Field::Present(programming_range) => {
                if input.low.present() || input.high.present() {
                    return Err(D::Error::custom(
                        "a typed Random group cannot also contain scalar low/high fields",
                    ));
                }
                DynamicRandomRangeProjection::Programming { programming_range }
            }
            Field::Missing => DynamicRandomRangeProjection::LegacyScalar {
                low: input.low.required("low")?,
                high: input.high.required("high")?,
            },
        };
        Ok(Self {
            id: input.id,
            seed: input.seed,
            range,
            decision_interval_millis: input.decision_interval_millis,
            start_probability: input.start_probability,
            mean_duration_millis: input.mean_duration_millis,
            duration_spread_millis: input.duration_spread_millis,
            attack_ratio: input.attack_ratio,
            decay_ratio: input.decay_ratio,
        })
    }
}
