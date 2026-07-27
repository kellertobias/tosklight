use light_core::{AttributeKey, AttributeValue, FixtureId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicDefinition {
    pub id: Uuid,
    pub pool_number: u16,
    pub revision: u64,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub target_binding: DynamicTargetBinding,
    pub lanes: Vec<DynamicLane>,
    #[serde(default)]
    pub random_groups: Vec<DynamicRandomGroup>,
    pub phase: PhaseDistribution,
    pub speed: DynamicSpeed,
    #[serde(default)]
    pub overall_speed_multiplier: Rational,
    #[serde(default)]
    pub run_mode: DynamicRunMode,
    pub default_activation: ActivationPolicy,
    #[serde(default)]
    pub activation_boundary: ActivationBoundary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicTargetBinding {
    LiveGroup { group_id: String },
    FrozenTargets { targets: Vec<FixtureId> },
    Targetless,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicLane {
    pub id: Uuid,
    pub attribute: AttributeKey,
    pub mode: DynamicLaneMode,
    pub keyframes: KeyframeConfiguration,
    pub max_min: MaxMinConfiguration,
    pub middle_amplitude: MiddleAmplitudeConfiguration,
    pub speed_multiplier: Rational,
    pub width: f32,
    #[serde(default)]
    pub random_group_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicLaneMode {
    Keyframes,
    MaxMin,
    MiddleAmplitude,
    Random,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyframeConfiguration {
    pub points: Vec<DynamicKeyframe>,
    #[serde(default = "default_lane_size")]
    pub size: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicKeyframe {
    pub position: f32,
    pub source: ScalarSource,
    pub interpolation: ScalarInterpolation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaxMinConfiguration {
    pub minimum: ScalarSource,
    pub maximum: ScalarSource,
    pub function: PeriodicFunction,
    #[serde(default = "default_lane_size")]
    pub size: f32,
    #[serde(default)]
    pub pwm: PwmShape,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MiddleAmplitudeConfiguration {
    pub middle: ScalarSource,
    pub amplitude: f32,
    pub function: PeriodicFunction,
    #[serde(default = "default_lane_size")]
    pub size: f32,
    #[serde(default)]
    pub pwm: PwmShape,
}

const fn default_lane_size() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScalarSource {
    Current,
    Value {
        value: f32,
    },
    Preset {
        preset_id: String,
        attribute: AttributeKey,
        #[serde(default)]
        last_valid_by_target: Vec<TargetScalarFallback>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TargetScalarFallback {
    pub target: FixtureId,
    pub value: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarInterpolation {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Hold,
    Drop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodicFunction {
    Sinus,
    Cosinus,
    LinearUp,
    LinearDown,
    Pwm,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PwmShape {
    pub attack: f32,
    pub on: f32,
    pub decay: f32,
    pub off: f32,
    pub attack_interpolation: ScalarInterpolation,
    pub decay_interpolation: ScalarInterpolation,
}

impl Default for PwmShape {
    fn default() -> Self {
        Self {
            attack: 0.0,
            on: 0.5,
            decay: 0.0,
            off: 0.5,
            attack_interpolation: ScalarInterpolation::Linear,
            decay_interpolation: ScalarInterpolation::Linear,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicRandomGroup {
    pub id: Uuid,
    pub seed: u64,
    pub low: ScalarSource,
    pub high: ScalarSource,
    pub decision_interval_millis: u64,
    pub start_probability: f32,
    pub mean_duration_millis: u64,
    pub duration_spread_millis: u64,
    pub attack_ratio: f32,
    pub decay_ratio: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseDistribution {
    pub ordering: PhaseOrdering,
    pub offset_degrees: f32,
    pub span_degrees: f32,
    pub block_size: u16,
    pub repeats: u16,
    pub wings: bool,
    #[serde(default)]
    pub anchors_degrees: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PhaseOrdering {
    Selection,
    GridLinear { angle_degrees: f32 },
    RadialOut { center_x: f32, center_z: f32 },
    RadialIn { center_x: f32, center_z: f32 },
    Axial { center_x: f32, center_z: f32 },
    RandomEachLoop { seed: u64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicSpeed {
    Fixed {
        duration_millis: u64,
    },
    SpeedGroup {
        group: SpeedGroup,
        beats_per_cycle: Rational,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SpeedGroup {
    A,
    B,
    C,
    D,
    E,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rational {
    pub numerator: u32,
    pub denominator: u32,
}

impl Rational {
    pub const ONE: Self = Self {
        numerator: 1,
        denominator: 1,
    };

    pub fn factor(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }
}

impl Default for Rational {
    fn default() -> Self {
        Self::ONE
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRunMode {
    #[default]
    Loop,
    OneShot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationPolicy {
    StartNow,
    JoinSyncNow,
    NextBoundary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationBoundary {
    #[default]
    Beat,
    Bar,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicReference {
    #[serde(default)]
    pub dynamic_id: Option<Uuid>,
    pub last_known_pool_number: u16,
    pub embedded_fallback: DynamicDefinitionSnapshot,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicDefinitionSnapshot {
    pub definition: Box<DynamicDefinition>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicSemanticValue {
    Static {
        value: AttributeValue,
        timing: DynamicValueTiming,
    },
    DynamicOn {
        instance_link: Uuid,
        dynamic: DynamicReference,
        lane_id: Uuid,
        overrides: DynamicInstanceOverrides,
        timing: DynamicValueTiming,
    },
    DynamicOff {
        instance_link: Uuid,
        timing: DynamicValueTiming,
    },
    FixAt {
        value: f32,
        timing: DynamicValueTiming,
    },
    Release,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DynamicValueTiming {
    #[serde(default)]
    pub fade_millis: Option<u64>,
    #[serde(default)]
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicAddressValue {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: DynamicSemanticValue,
    pub programmer_order: u64,
    /// Authoritative application-clock time used for priority-then-LTP Dynamic/FAT arbitration.
    #[serde(default)]
    pub changed_at_millis: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicInstanceOverrides {
    pub size: f32,
    pub speed_multiplier: Rational,
    pub phase_offset_degrees: f32,
}
