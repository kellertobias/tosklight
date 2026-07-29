//! Portable Dynamic definitions and transport-safe reference projections.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicDefinitionProjection {
    pub id: Uuid,
    pub pool_number: u16,
    #[ts(type = "number")]
    pub revision: u64,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub icon: Option<String>,
    pub target_binding: DynamicTargetBindingProjection,
    pub lanes: Vec<DynamicLaneProjection>,
    pub random_groups: Vec<DynamicRandomGroupProjection>,
    pub phase_mode: DynamicPhaseSpreadModeProjection,
    pub phase: DynamicPhaseDistributionProjection,
    pub speed: DynamicSpeedProjection,
    pub overall_speed_multiplier: DynamicRationalProjection,
    pub run_mode: DynamicRunModeProjection,
    pub default_activation: DynamicActivationPolicyProjection,
    pub activation_boundary: DynamicActivationBoundaryProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicTargetBindingProjection {
    LiveGroup { group_id: String },
    FrozenTargets { targets: Vec<Uuid> },
    Targetless,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicLaneProjection {
    pub id: Uuid,
    pub attribute: String,
    pub mode: DynamicLaneModeProjection,
    pub keyframes: DynamicKeyframeConfigurationProjection,
    pub max_min: DynamicMaxMinConfigurationProjection,
    pub middle_amplitude: DynamicMiddleAmplitudeConfigurationProjection,
    pub speed_multiplier: DynamicRationalProjection,
    pub width: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub random_group_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub phase: Option<DynamicPhaseDistributionProjection>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicLaneModeProjection {
    Keyframes,
    MaxMin,
    MiddleAmplitude,
    Random,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicPhaseSpreadModeProjection {
    #[default]
    Uniform,
    PerLane,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicKeyframeConfigurationProjection {
    pub points: Vec<DynamicKeyframeProjection>,
    pub size: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicKeyframeProjection {
    pub position: f32,
    pub source: DynamicScalarSourceProjection,
    pub interpolation: DynamicScalarInterpolationProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicMaxMinConfigurationProjection {
    pub minimum: DynamicScalarSourceProjection,
    pub maximum: DynamicScalarSourceProjection,
    pub function: DynamicPeriodicFunctionProjection,
    pub size: f32,
    pub pwm: DynamicPwmShapeProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicMiddleAmplitudeConfigurationProjection {
    pub middle: DynamicScalarSourceProjection,
    pub amplitude: f32,
    pub function: DynamicPeriodicFunctionProjection,
    pub size: f32,
    pub pwm: DynamicPwmShapeProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicScalarSourceProjection {
    Current,
    Value {
        value: f32,
    },
    Preset {
        preset_id: String,
        attribute: String,
        last_valid_by_target: Vec<DynamicTargetScalarFallbackProjection>,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicTargetScalarFallbackProjection {
    pub target: Uuid,
    pub value: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicScalarInterpolationProjection {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Hold,
    Drop,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicPeriodicFunctionProjection {
    Sinus,
    Cosinus,
    LinearUp,
    LinearDown,
    Pwm,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicPwmShapeProjection {
    pub attack: f32,
    pub on: f32,
    pub decay: f32,
    pub off: f32,
    pub attack_interpolation: DynamicScalarInterpolationProjection,
    pub decay_interpolation: DynamicScalarInterpolationProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicRandomGroupProjection {
    pub id: Uuid,
    #[ts(type = "number")]
    pub seed: u64,
    pub low: DynamicScalarSourceProjection,
    pub high: DynamicScalarSourceProjection,
    #[ts(type = "number")]
    pub decision_interval_millis: u64,
    pub start_probability: f32,
    #[ts(type = "number")]
    pub mean_duration_millis: u64,
    #[ts(type = "number")]
    pub duration_spread_millis: u64,
    pub attack_ratio: f32,
    pub decay_ratio: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicPhaseDistributionProjection {
    pub ordering: DynamicPhaseOrderingProjection,
    pub offset_degrees: f32,
    pub span_degrees: f32,
    pub block_size: u16,
    pub repeats: u16,
    pub wings: bool,
    pub anchors_degrees: Vec<f32>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicPhaseOrderingProjection {
    Selection,
    GridLinear {
        angle_degrees: f32,
    },
    RadialOut {
        center_x: f32,
        center_z: f32,
    },
    RadialIn {
        center_x: f32,
        center_z: f32,
    },
    Axial {
        center_x: f32,
        center_z: f32,
    },
    RandomEachLoop {
        #[ts(type = "number")]
        seed: u64,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicSpeedProjection {
    Fixed {
        #[ts(type = "number")]
        duration_millis: u64,
    },
    SpeedGroup {
        group: DynamicSpeedGroupProjection,
        beats_per_cycle: DynamicRationalProjection,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub enum DynamicSpeedGroupProjection {
    A,
    B,
    C,
    D,
    E,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicRationalProjection {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRunModeProjection {
    Loop,
    OneShot,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicActivationPolicyProjection {
    StartNow,
    JoinSyncNow,
    NextBoundary,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicActivationBoundaryProjection {
    Beat,
    Bar,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicReferenceProjection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub dynamic_id: Option<Uuid>,
    pub last_known_pool_number: u16,
    pub embedded_fallback_id: Uuid,
    #[ts(type = "number")]
    pub embedded_fallback_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub embedded_fallback: Option<DynamicDefinitionProjection>,
}

#[derive(Clone, Copy, Debug, Deserialize, Default, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicValueTimingProjection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicInstanceOverridesProjection {
    pub size: f32,
    pub speed_multiplier: DynamicRationalProjection,
    pub phase_offset_degrees: f32,
}

impl Default for DynamicInstanceOverridesProjection {
    fn default() -> Self {
        Self {
            size: 1.0,
            speed_multiplier: DynamicRationalProjection {
                numerator: 1,
                denominator: 1,
            },
            phase_offset_degrees: 0.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicStartActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[serde(default)]
    #[schemars(length(max = 10_000))]
    pub targets: Vec<Uuid>,
    #[serde(default)]
    pub overrides: DynamicInstanceOverridesProjection,
    #[serde(default)]
    pub timing: DynamicValueTimingProjection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub undo_group: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicOffActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[serde(default)]
    pub timing: DynamicValueTimingProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicControllerValueActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub value: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub undo_group: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicFixAtActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[schemars(length(max = 10_000))]
    pub targets: Vec<Uuid>,
    pub attribute: String,
    pub value: f32,
    #[serde(default)]
    pub timing: DynamicValueTimingProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicInstanceActionOutcome {
    pub request_id: String,
    pub runtime_instance_id: Uuid,
    pub controller_id: Uuid,
    pub targets: Vec<Uuid>,
    pub started: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicControllerActionOutcome {
    pub request_id: String,
    pub controller_id: Uuid,
    pub changed: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicRuntimeSnapshotProjection {
    pub global_paused: bool,
    pub instances: Vec<DynamicRuntimeInstanceProjection>,
    pub definitions: Vec<DynamicDefinitionStatusProjection>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicDefinitionStatusProjection {
    pub dynamic_id: Uuid,
    pub target_count: usize,
    pub compatible_target_count: usize,
    pub missing_target_count: usize,
    pub unpatched_target_count: usize,
    pub lane_count: usize,
    pub supported_address_count: usize,
    pub skipped_address_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicRuntimeInstanceProjection {
    pub instance_id: Uuid,
    pub dynamic_id: Uuid,
    pub pool_number: u16,
    pub name: String,
    pub targets: Vec<Uuid>,
    pub pending: bool,
    pub pending_until_millis: Option<u64>,
    pub paused: bool,
    pub speed_source: String,
    pub activation_boundary: DynamicActivationBoundaryProjection,
    pub effective_cycle_millis: u64,
    pub effective_bpm: Option<f64>,
    pub beat_phase: Option<f64>,
    pub phase_advancing: bool,
    pub aliasing_warning: Option<String>,
    pub controllers: Vec<DynamicRuntimeControllerProjection>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicRuntimeControllerProjection {
    pub controller_id: Uuid,
    pub source: String,
    pub priority: i16,
    pub size: f32,
    pub speed_multiplier: f32,
    pub phase_offset_degrees: f32,
    pub paused: bool,
    pub winning: bool,
    pub releasing: bool,
    pub activation_mix: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicStartLiveActionRequest {
    pub dynamic_id: Uuid,
    pub request: DynamicStartActionRequest,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicOffLiveActionRequest {
    pub controller_id: Uuid,
    pub request: DynamicOffActionRequest,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicControllerLiveActionRequest {
    pub controller_id: Uuid,
    pub request: DynamicControllerValueActionRequest,
}
