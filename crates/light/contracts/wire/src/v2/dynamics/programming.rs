use super::*;
use crate::v2::{programming::ProgrammingAttributeValue, programming_intent::*};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicSemanticColorBasisProjection {
    Retain,
    Recipe,
    HueSaturation,
    Whole,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DynamicFamilyRepresentationProjection {
    Angles,
    Target {
        reference: Option<ProgrammingTargetReference>,
    },
    SemanticColor {
        basis: DynamicSemanticColorBasisProjection,
    },
    DirectColor {
        source: ProgrammingNativeColorIdentity,
    },
    Focus,
    Zoom {
        convention: ProgrammingOpeningConvention,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicValueAddressProjection {
    pub representation: DynamicFamilyRepresentationProjection,
    pub component: Option<ProgrammingComponent>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum DynamicValueProjection {
    Scalar(f32),
    Native(u32),
    Family(ProgrammingAttributeValue),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicValueFallbackProjection {
    pub target: Uuid,
    pub value: DynamicValueProjection,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicPresetTemplateProjection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub universal: Option<ProgrammingAttributeValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<DynamicPresetGroupTemplateProjection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixtures: Vec<DynamicPresetFixtureTemplateProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Box<DynamicPresetTemplateProjection>>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicPresetGroupTemplateProjection {
    pub group_id: String,
    pub value: ProgrammingAttributeValue,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicPresetFixtureTemplateProjection {
    pub fixture_id: Uuid,
    pub value: ProgrammingAttributeValue,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DynamicValueSourceProjection {
    Current,
    Value {
        value: DynamicValueProjection,
    },
    Preset {
        preset_id: String,
        address: DynamicValueAddressProjection,
        #[serde(default)]
        last_valid_by_target: Vec<DynamicValueFallbackProjection>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retained: Option<DynamicPresetTemplateProjection>,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicProgrammingLaneProjection {
    pub address: DynamicValueAddressProjection,
    pub configuration: DynamicProgrammingLaneConfigurationProjection,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "mode", content = "configuration", rename_all = "snake_case")]
pub enum DynamicProgrammingLaneConfigurationProjection {
    Keyframes(DynamicProgrammingKeyframesProjection),
    MaxMin(DynamicProgrammingMaxMinProjection),
    MiddleAmplitude(DynamicProgrammingMiddleAmplitudeProjection),
    Random,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicProgrammingKeyframesProjection {
    pub points: Vec<DynamicProgrammingKeyframeProjection>,
    #[serde(default = "default_lane_size")]
    pub size: f32,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicProgrammingKeyframeProjection {
    pub position: f32,
    pub source: DynamicValueSourceProjection,
    pub interpolation: DynamicScalarInterpolationProjection,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicProgrammingMaxMinProjection {
    pub minimum: DynamicValueSourceProjection,
    pub maximum: DynamicValueSourceProjection,
    pub function: DynamicPeriodicFunctionProjection,
    #[serde(default = "default_lane_size")]
    pub size: f32,
    #[serde(default)]
    pub pwm: DynamicPwmShapeProjection,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicProgrammingMiddleAmplitudeProjection {
    pub middle: DynamicValueSourceProjection,
    pub amplitude: DynamicValueProjection,
    pub function: DynamicPeriodicFunctionProjection,
    #[serde(default = "default_lane_size")]
    pub size: f32,
    #[serde(default)]
    pub pwm: DynamicPwmShapeProjection,
    #[serde(default)]
    pub invert_waveform: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema, TS)]
pub struct DynamicProgrammingRandomRangeProjection {
    pub low: DynamicValueSourceProjection,
    pub high: DynamicValueSourceProjection,
}
