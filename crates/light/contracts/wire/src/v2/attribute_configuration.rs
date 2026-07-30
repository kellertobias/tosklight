//! Typed show-owned Attribute Registry and activation-group contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AttributeEncoderGroup {
    Intensity,
    Color,
    Position,
    Beam,
    Shapers,
    Focus,
    Control,
    Media,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AttributeValueType {
    Continuous,
    Color,
    Indexed,
    Control,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CustomAttributeLifecycle {
    Active,
    Retired,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeBounds {
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct CustomAttributeDescriptor {
    pub id: String,
    pub label: String,
    pub value_type: AttributeValueType,
    pub display_unit: Option<String>,
    pub physical_unit: Option<String>,
    pub normalized_bounds: Option<AttributeBounds>,
    pub domain_bounds: Option<AttributeBounds>,
    pub cyclic: bool,
    pub recordable: bool,
    pub lifecycle: CustomAttributeLifecycle,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributePlacement {
    pub attribute: String,
    pub encoder_group: AttributeEncoderGroup,
    pub encoder_page: u16,
    pub encoder_slot: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeActivationGroup {
    pub id: String,
    pub label: String,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeConfiguration {
    pub version: u16,
    pub custom_attributes: Vec<CustomAttributeDescriptor>,
    pub placements: Vec<AttributePlacement>,
    pub activation_groups: Vec<AttributeActivationGroup>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ConfiguredAttributeDescriptor {
    pub id: String,
    pub label: String,
    pub encoder_group: AttributeEncoderGroup,
    pub encoder_page: u16,
    pub encoder_slot: u8,
    pub value_type: AttributeValueType,
    pub display_unit: Option<String>,
    pub physical_unit: Option<String>,
    pub normalized_min: Option<f32>,
    pub normalized_max: Option<f32>,
    pub domain_min: Option<f32>,
    pub domain_max: Option<f32>,
    pub cyclic: bool,
    pub recordable: bool,
    pub built_in: bool,
    pub retired: bool,
    pub activation_group_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeConfigurationSnapshot {
    pub show_id: Option<Uuid>,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[ts(type = "number")]
    pub object_revision: u64,
    pub configuration: AttributeConfiguration,
    pub descriptors: Vec<ConfiguredAttributeDescriptor>,
    pub validation_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeConfigurationPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub custom_attributes: Option<Vec<CustomAttributeDescriptor>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub placements: Option<Vec<AttributePlacement>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub activation_groups: Option<Vec<AttributeActivationGroup>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeConfigurationUpdateRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_show_revision: u64,
    #[ts(type = "number")]
    pub expected_object_revision: u64,
    pub patch: AttributeConfigurationPatch,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeConfigurationUpdateOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub snapshot: AttributeConfigurationSnapshot,
    #[ts(type = "number")]
    pub event_sequence: u64,
}
